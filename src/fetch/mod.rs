//! Guarded streaming fetch client (A-3b; architecture 14.1, ADR-001, 003, 004).
//!
//! Every request, and every redirect hop, goes through the SSRF core in the same order:
//! `validate_target` (scheme, userinfo, IP-literal checks, then ONE resolver lookup with every answer checked)
//! -> cross-check with the HTTP stack's own URL parser -> a client built for that hop whose only dial route is
//! [`dns::Pinned`] (the validated address set) -> streamed, size-capped body. Redirects are followed by our loop
//! (the client's redirect policy is `none`); connections are never pooled, proxies, cookies and credentials are
//! never sent, and one overall deadline covers everything after a concurrency slot is granted.
//!
//! Merge gate (EPICS A-3b): [`FetchClient::new`] takes a [`Policy`] and there is no `Default`; the named test
//! `a3b_merge_gate` (in `tests.rs`) is a required CI check.

pub mod body;
pub mod dns;

#[cfg(test)]
mod tests;

use crate::config::Config;
use crate::convert::{self, ConvertError, Mode};
use crate::error::FetchError;
use crate::policy::Policy;
use crate::ssrf::resolver::{validate_target, Resolver};
use crate::ssrf::{CheckedUrl, Host, Origin};
use reqwest::header::{
    HeaderMap, ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, LOCATION,
};
use reqwest::{Client, StatusCode, Url};
use std::sync::{Arc, Mutex, Once, OnceLock, PoisonError};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::{timeout, timeout_at, Instant};

/// Redirect hops followed before `too_many_redirects` (ADR-003 step 6; the configurable limit and its tests are B-3).
pub const MAX_REDIRECTS: usize = 5;
/// Response header count limit, enforced by the HTTP parser (`http1_max_headers`).
pub const MAX_HEADER_COUNT: usize = 64;
/// Response header bytes limit (names plus values), checked as soon as the head arrives. The parser's own buffer
/// (hyper default, about 400 KiB, not configurable through reqwest 0.13) bounds what is read before this check.
pub const MAX_HEADER_BYTES: usize = 32 * 1024;
const USER_AGENT_VALUE: &str = concat!("fetch-mcp/", env!("CARGO_PKG_VERSION"));

/// Compiled-in limits (C-1 adds environment parsing for them; `Config` already carries the defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    pub max_bytes: u64,
    pub concurrency: usize,
}

impl Limits {
    /// The limits a [`Config`] asks for.
    #[must_use]
    pub fn from_config(c: &Config) -> Self {
        Self {
            timeout: Duration::from_millis(c.timeout_ms),
            max_bytes: c.max_bytes,
            concurrency: usize::try_from(c.max_concurrency).unwrap_or(1).max(1),
        }
    }
}

/// What a completed fetch read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The URL the body came from: the request URL, or the last redirect target (A-9).
    pub final_url: String,
    /// Final HTTP status (2xx).
    pub status: u16,
    /// Redirect hops followed.
    pub redirects: usize,
    /// Body bytes read off the wire.
    pub wire_bytes: u64,
}

/// The guarded client. No `Default`: a policy must be chosen by the caller (merge gate).
pub struct FetchClient<R: Resolver> {
    policy: Policy,
    resolver: Arc<R>,
    limits: Limits,
    slots: Semaphore,
    tls: OnceLock<Result<Arc<rustls::ClientConfig>, String>>,
}

impl<R: Resolver> FetchClient<R> {
    #[must_use]
    pub fn new(policy: Policy, resolver: Arc<R>, limits: Limits) -> Self {
        Self {
            policy,
            resolver,
            slots: Semaphore::new(limits.concurrency),
            limits,
            tls: OnceLock::new(),
        }
    }

    /// Fetch `url`, streaming the body as UTF-8 text slices into `sink` (each at most 64 KiB). Memory does not
    /// depend on the body size: nothing is accumulated here.
    ///
    /// # Errors
    /// A [`FetchError`] for a refused target, resolution, transport, timeout, size, encoding or status failure.
    pub async fn fetch(
        &self,
        url: &str,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Fetched, FetchError> {
        self.fetch_as(url, Mode::Raw, sink).await
    }

    /// [`fetch`](Self::fetch) with a conversion mode: [`Mode::Markdown`] converts an HTML body to markdown
    /// (A-4) as it streams; anything else, and [`Mode::Raw`], is passed through as text. Memory still does not
    /// depend on the body size.
    ///
    /// # Errors
    /// As [`fetch`](Self::fetch), plus `converter_limit` when the converter's memory or output limit is hit.
    pub async fn fetch_as(
        &self,
        url: &str,
        mode: Mode,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Fetched, FetchError> {
        // Queue for a slot for at most the timeout; the fetch deadline starts when the slot is granted.
        let _slot = match timeout(self.limits.timeout, self.slots.acquire()).await {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => return Err(FetchError::Internal("concurrency limiter closed".into())),
            Err(_) => {
                return Err(FetchError::Timeout(
                    "timed out waiting for a free fetch slot".into(),
                ))
            }
        };
        let deadline = Instant::now() + self.limits.timeout;
        match timeout_at(deadline, self.run(url, mode, sink, deadline)).await {
            Ok(r) => r,
            // Dropping the future closes any open connection.
            Err(_) => Err(FetchError::Timeout("the request timed out".into())),
        }
    }

    async fn run(
        &self,
        url: &str,
        mode: Mode,
        sink: &mut (dyn FnMut(&str) + Send),
        deadline: Instant,
    ) -> Result<Fetched, FetchError> {
        let mut current = url.to_string();
        let mut origin = Origin::Initial;
        for hop in 0..=MAX_REDIRECTS {
            // 1. SSRF core: one lookup, every answer checked, only the validated set survives.
            let v = validate_target(&*self.resolver, &self.policy, &current, origin).await?;
            // 2. The URL the client will use must mean what the core validated.
            let parsed = cross_check(&current, &v.url)?;
            // 3. A client whose only dial route is the validated set.
            let client = self.hop_client(&v.url, v.addrs, deadline)?;
            let resp = client
                .get(parsed.clone())
                .header(ACCEPT_ENCODING, "gzip")
                .header(ACCEPT, "text/html,text/plain;q=0.9,*/*;q=0.5")
                .send()
                .await
                .map_err(map_transport)?;
            check_head(resp.headers())?;
            let status = resp.status();
            if is_redirect(status) {
                let Some(loc) = resp.headers().get(LOCATION) else {
                    return Err(FetchError::HttpStatus(status.as_u16()));
                };
                let loc = loc
                    .to_str()
                    .map_err(|_| FetchError::BadResponse("redirect Location is not text".into()))?;
                let next = parsed.join(loc).map_err(|_| {
                    FetchError::BadResponse("redirect Location is not a valid URL".into())
                })?;
                if hop == MAX_REDIRECTS {
                    return Err(FetchError::TooManyRedirects);
                }
                current = next.as_str().to_string();
                origin = Origin::Redirect;
                continue; // dropping `resp` closes the connection without reading the body
            }
            if !status.is_success() {
                return Err(FetchError::HttpStatus(status.as_u16()));
            }
            let wire_bytes = self.read_body(resp, mode, &parsed, sink).await?;
            return Ok(Fetched {
                final_url: echo_url(&parsed),
                status: status.as_u16(),
                redirects: hop,
                wire_bytes,
            });
        }
        Err(FetchError::TooManyRedirects)
    }

    async fn read_body(
        &self,
        mut resp: reqwest::Response,
        mode: Mode,
        base: &Url,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<u64, FetchError> {
        let cap = self.limits.max_bytes;
        let gzip = content_encoding_is_gzip(resp.headers())?;
        // A declared length over the cap aborts before a single body byte is read.
        if resp.content_length().is_some_and(|n| n > cap) {
            return Err(too_large());
        }
        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut conv = convert::for_response(mode, content_type.as_deref(), Some(base.clone()));
        // The pipeline's sink cannot fail, so a converter failure is parked here and checked per chunk.
        let failure: Mutex<Option<ConvertError>> = Mutex::new(None);
        let mut convert_step = |text: &str| {
            let mut f = failure.lock().unwrap_or_else(PoisonError::into_inner);
            if f.is_none() {
                if let Err(e) = conv.push(text, &mut *sink) {
                    *f = Some(e);
                }
            }
        };
        let mut pipe = body::Pipeline::new(gzip, cap, &mut convert_step);
        let mut wire = 0u64;
        while let Some(chunk) = resp.chunk().await.map_err(map_transport)? {
            wire = wire.saturating_add(chunk.len() as u64);
            if wire > cap {
                return Err(too_large());
            }
            pipe.feed(&chunk).map_err(map_body)?;
            take_failure(&failure)?;
        }
        pipe.finish().map_err(map_body)?;
        take_failure(&failure)?;
        conv.finish(&mut *sink).map_err(map_convert)?;
        Ok(wire)
    }

    /// Build the client for one hop. Pooling, proxies, cookies, redirects and HTTP/2 are off; the resolver is
    /// [`dns::Pinned`], so the client dials only the addresses the SSRF core validated for this host.
    fn hop_client(
        &self,
        target: &CheckedUrl,
        addrs: Vec<std::net::SocketAddr>,
        deadline: Instant,
    ) -> Result<Client, FetchError> {
        let host = match &target.host {
            Host::Name(n) => n.as_str(),
            Host::Ip(_) => "", // a literal never consults the resolver; any name lookup is refused
        };
        install_ring_provider();
        let remaining = deadline.saturating_duration_since(Instant::now());
        let mut b = Client::builder()
            .dns_resolver(Arc::new(dns::Pinned::new(host, addrs)))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .pool_max_idle_per_host(0)
            .http1_only()
            .http1_max_headers(MAX_HEADER_COUNT)
            .referer(false)
            .user_agent(USER_AGENT_VALUE)
            .connect_timeout(remaining);
        if target.https {
            b = b.tls_backend_preconfigured(self.tls_config()?);
        }
        b.build()
            .map_err(|_| FetchError::Internal("could not build the HTTP client".into()))
    }

    /// TLS config with the ring provider and the embedded trust anchors, built on first use (ADR-001 mitigation:
    /// idle RSS does not pay for it).
    fn tls_config(&self) -> Result<rustls::ClientConfig, FetchError> {
        let built = self.tls.get_or_init(|| {
            let roots = rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map(|b| Arc::new(b.with_root_certificates(roots).with_no_client_auth()))
            .map_err(|e| e.to_string())
        });
        match built {
            Ok(cfg) => Ok(rustls::ClientConfig::clone(cfg)),
            Err(_) => Err(FetchError::Internal("TLS configuration failed".into())),
        }
    }
}

/// reqwest built with `rustls-no-provider` refuses to build any client until a process-wide crypto provider is
/// installed, https or not. Installed once, ring only (ADR-001; aws-lc is banned in deny.toml). An `Err` means
/// one is already installed, which is fine.
fn install_ring_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn too_large() -> FetchError {
    FetchError::TooLarge("the response is larger than the size limit".into())
}

fn is_redirect(s: StatusCode) -> bool {
    matches!(s.as_u16(), 301 | 302 | 303 | 307 | 308)
}

fn map_convert(e: ConvertError) -> FetchError {
    match e {
        ConvertError::Limit => FetchError::ConverterLimit(
            "the page is too complex to convert within the memory limit; retry with raw=true"
                .into(),
        ),
        ConvertError::Malformed => FetchError::ConverterLimit(
            "the HTML could not be converted; retry with raw=true".into(),
        ),
    }
}

fn take_failure(f: &Mutex<Option<ConvertError>>) -> Result<(), FetchError> {
    match *f.lock().unwrap_or_else(PoisonError::into_inner) {
        Some(e) => Err(map_convert(e)),
        None => Ok(()),
    }
}

fn map_body(e: body::BodyError) -> FetchError {
    match e {
        body::BodyError::TooLarge => too_large(),
        body::BodyError::Corrupt => FetchError::BadResponse("the gzip body is corrupt".into()),
    }
}

/// Categories only: never the address, port or the transport's own text (which can name them).
fn map_transport(e: reqwest::Error) -> FetchError {
    if e.is_timeout() {
        return FetchError::Timeout("the request timed out".into());
    }
    // The matched chain may contain the peer socket address, but it is only searched and never emitted.
    // Classify from the error's kind first and strip the URL before any text is inspected: reqwest's Debug output
    // embeds the request URL, and the URL (path, query or a redirect Location) is upstream-controlled text.
    let is_connect = e.is_connect();
    let e = e.without_url();
    let mut chain = format!("{e:?}").to_ascii_lowercase();
    let mut src = std::error::Error::source(&e);
    while let Some(s) = src {
        chain.push(' ');
        chain.push_str(&s.to_string().to_ascii_lowercase());
        src = s.source();
    }
    if chain.contains("too large") || chain.contains("too many headers") || chain.contains("header")
    {
        return FetchError::BadResponse("the response headers are too large or malformed".into());
    }
    if is_connect {
        return FetchError::Network("could not connect to the host".into());
    }
    FetchError::Network("the connection failed".into())
}

/// Header limits after the head is parsed (the count is enforced by the parser).
fn check_head(h: &HeaderMap) -> Result<(), FetchError> {
    let bytes: usize = h.iter().map(|(k, v)| k.as_str().len() + v.len()).sum();
    if h.len() > MAX_HEADER_COUNT || bytes > MAX_HEADER_BYTES {
        return Err(FetchError::BadResponse(
            "the response headers are too large".into(),
        ));
    }
    Ok(())
}

/// The URL echoed back in the A-9 header: the request URL without userinfo or fragment (neither is sent to the
/// server, and credentials must never be reflected into the model's context).
pub(crate) fn echo_url(u: &Url) -> String {
    let mut u = u.clone();
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.set_fragment(None);
    u.to_string()
}

/// Accept absent, `identity`, or exactly one `gzip` (checked before any decode); everything else, stacked
/// encodings included, is `unsupported_encoding` (ADR-004).
fn content_encoding_is_gzip(h: &HeaderMap) -> Result<bool, FetchError> {
    let mut values = h.get_all(CONTENT_ENCODING).iter();
    let Some(first) = values.next() else {
        return Ok(false);
    };
    let unsupported =
        || FetchError::UnsupportedEncoding("Content-Encoding must be gzip or absent".into());
    if values.next().is_some() {
        return Err(unsupported());
    }
    match first.to_str().map(|s| s.trim().to_ascii_lowercase()) {
        Ok(v) if v == "gzip" => Ok(true),
        Ok(v) if v == "identity" || v.is_empty() => Ok(false),
        _ => Err(unsupported()),
    }
}

/// The URL the client will send must mean what the SSRF core validated: same scheme, port and host. The core's
/// parser is hand-rolled, the client's is the `url` crate; any disagreement is refused (architect N2a). The
/// address side is neutralised separately by dialling only the validated set.
fn cross_check(raw: &str, v: &CheckedUrl) -> Result<Url, FetchError> {
    let refuse = || FetchError::BlockedTarget("URL is ambiguous and was refused".into());
    let u = Url::parse(raw).map_err(|_| refuse())?;
    let scheme_ok = u.scheme() == if v.https { "https" } else { "http" };
    let port_ok = u.port_or_known_default() == Some(v.port);
    let host_ok = match (&v.host, u.host_str()) {
        (Host::Ip(ip), Some(h)) => {
            h.trim_matches(['[', ']']).parse::<std::net::IpAddr>() == Ok(*ip)
        }
        (Host::Name(n), Some(h)) => crate::ssrf::canonical_name(h) == *n,
        (_, None) => false,
    };
    if scheme_ok && port_ok && host_ok && u.username().is_empty() && u.password().is_none() {
        Ok(u)
    } else {
        Err(refuse())
    }
}
