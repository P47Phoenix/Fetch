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
pub mod charset;
pub mod dns;

#[cfg(test)]
mod tests;

use crate::config::{Config, RobotsMode};
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
/// B-4: robots.txt is capped well below a normal page (the AC: "small, at most 512 KB").
const ROBOTS_MAX_BYTES: usize = 512 * 1024;
/// Finding #6 (round-2 review): the robots.txt sub-fetch's own deadline is at most the configured timeout
/// divided by this, so a hanging robots.txt server cannot consume the whole outer deadline.
const ROBOTS_SUB_DEADLINE_FRACTION: u32 = 4;
/// A-8: the standard HTML5 `<meta charset>` sniffing window, in decoded bytes.
const SNIFF_WINDOW: usize = 1024;
/// A-8: wire bytes [`FetchClient::sniff_prefix`] will read chasing [`SNIFF_WINDOW`] decoded bytes, before giving
/// up and sniffing whatever it has (clamped to the response's own cap). Generous headroom over `SNIFF_WINDOW`
/// for gzip-compressed responses; small next to a real body, so early-abort stays effectively intact.
const SNIFF_WIRE_BUDGET: u64 = 16 * 1024;

/// The result of [`FetchClient::sniff_prefix`].
struct Sniff {
    /// Decoded bytes gathered for the meta-charset scan (empty when `stopped_early` is set).
    decoded: Vec<u8>,
    /// The raw wire bytes read during the peek, to be replayed into whichever pipeline decodes the rest.
    wire_prefix: Vec<u8>,
    /// `Some(wire_bytes)` when `stop()` fired during the peek: the caller returns it unchanged (early-stop
    /// contract), and `decoded`/`wire_prefix` are not meaningful.
    stopped_early: Option<u64>,
}

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
    /// B-4: [`RobotsMode::Ignore`] (the default, and every construction that does not call
    /// [`FetchClient::with_robots_mode`]) is a no-op -- robots.txt is never fetched or checked. Set to
    /// [`RobotsMode::Enforce`], it is a real, working mechanism: [`FetchClient::check_robots`] fetches the
    /// target origin's robots.txt on the initial hop (through this same guarded client) and refuses a
    /// disallowed path. Kept out of [`FetchClient::new`]'s signature deliberately: that signature is pinned by
    /// the A-3b merge gate (`a3b_merge_gate`), so adding the mechanism could not change it.
    robots: RobotsMode,
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
            robots: RobotsMode::Ignore,
        }
    }

    /// Sets the robots.txt enforcement mode (B-4; `Config.robots_txt`, `FETCH_ROBOTS_TXT`). Builder-style so
    /// [`FetchClient::new`]'s signature -- pinned by the merge gate -- does not change; every caller that does
    /// not use this stays exactly as before ([`RobotsMode::Ignore`], the default).
    #[must_use]
    pub fn with_robots_mode(mut self, mode: RobotsMode) -> Self {
        self.robots = mode;
        self
    }

    /// Fetch `url`, streaming the body as UTF-8 text slices into `sink` (each at most 64 KiB). Memory does not
    /// depend on the body size for the common case (streamed UTF-8, whatever the charset source). One path is
    /// an exception (A-8): an HTML response that is both gzip-compressed and has no explicit `charset=` on
    /// either the `Content-Type` header or a `<meta>` tag buffers the whole (still capped) body plus a decoded
    /// copy before it can be sniffed and converted -- up to roughly `4 * max_bytes` for that one case. See
    /// `README.md` / `docs/BENCHMARK.md` for the concrete bound.
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
    /// (A-4) as it streams; other text, and [`Mode::Raw`], is passed through as text. Memory still does not
    /// depend on the body size for the common case; see the caveat on [`fetch`](Self::fetch) for the one
    /// exception (gzip HTML with no charset markers).
    ///
    /// # Errors
    /// As [`fetch`](Self::fetch), plus `converter_limit` when the converter's memory or output limit is hit and
    /// `unsupported_content_type` when the response is not text (A-6).
    pub async fn fetch_as(
        &self,
        url: &str,
        mode: Mode,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Fetched, FetchError> {
        self.fetch_until(url, mode, sink, &|| false).await
    }

    /// [`fetch_as`](Self::fetch_as) that stops reading as soon as `stop` returns true (A-5 early stop): it is
    /// polled after every wire chunk, the response is then dropped (the connection closes) and the call succeeds
    /// with what the sink received. A body that is over the size cap, or whose cap is reached before `stop` turns
    /// true, is `too_large` as ever.
    ///
    /// # Errors
    /// As [`fetch_as`](Self::fetch_as).
    pub async fn fetch_until(
        &self,
        url: &str,
        mode: Mode,
        sink: &mut (dyn FnMut(&str) + Send),
        stop: &(dyn Fn() -> bool + Send + Sync),
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
        match timeout_at(deadline, self.run(url, mode, sink, stop, deadline, true)).await {
            Ok(r) => r,
            // Dropping the future closes any open connection.
            Err(_) => Err(FetchError::Timeout("the request timed out".into())),
        }
    }

    /// `check_robots` is false only for the recursive call [`FetchClient::check_robots`] itself makes to fetch
    /// robots.txt: that fetch must not trigger ANOTHER robots.txt check on robots.txt (infinite recursion).
    /// Every other caller passes `true`.
    async fn run(
        &self,
        url: &str,
        mode: Mode,
        sink: &mut (dyn FnMut(&str) + Send),
        stop: &(dyn Fn() -> bool + Send + Sync),
        deadline: Instant,
        check_robots: bool,
    ) -> Result<Fetched, FetchError> {
        let mut current = url.to_string();
        let mut origin = Origin::Initial;
        for hop in 0..=MAX_REDIRECTS {
            // 1. SSRF core: one lookup, every answer checked, only the validated set survives.
            let v = validate_target(&*self.resolver, &self.policy, &current, origin).await?;
            // 2. The URL the client will use must mean what the core validated.
            let parsed = cross_check(&current, &v.url)?;
            // B-4: robots.txt, checked once for the original target only (never a redirect hop), and only when
            // enforcement is switched on (RobotsMode::Enforce; the default, Ignore, skips this whole step --
            // zero behaviour change from before B-4).
            if check_robots
                && hop == 0
                && origin == Origin::Initial
                && self.robots == RobotsMode::Enforce
            {
                self.check_robots(&v.url, &parsed, deadline).await?;
            }
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
            let wire_bytes = self.read_body(resp, mode, &parsed, sink, stop).await?;
            return Ok(Fetched {
                final_url: echo_url(&parsed),
                status: status.as_u16(),
                redirects: hop,
                wire_bytes,
            });
        }
        Err(FetchError::TooManyRedirects)
    }

    /// B-4: robots.txt for `checked`'s origin, fetched through [`FetchClient::run`] directly -- the same SSRF
    /// checks and redirect handling as the fetch it is gating, capped at [`ROBOTS_MAX_BYTES`], and inside the
    /// concurrency slot already held for that fetch (no separate slot is acquired, so this cannot deadlock a
    /// `FETCH_MAX_CONCURRENCY=1` server against itself). Bounded by a sub-deadline (finding #6, round-2 review):
    /// `min(outer deadline, ROBOTS_SUB_DEADLINE_FRACTION of the configured timeout)`, so a hanging robots.txt
    /// server cannot consume the whole outer deadline and turn "fail open quickly" into "time out slowly" --
    /// the gated fetch itself still gets the rest of the outer deadline either way. Per the AC, any failure
    /// fetching or reading it -- missing, a non-2xx status, refused by SSRF, timed out, truncated at the cap,
    /// whatever -- is treated as "no restrictions": only a rule that actually parses out of what was read can
    /// refuse the fetch.
    ///
    /// # Errors
    /// [`FetchError::RobotsDisallowed`] when robots.txt disallows `parsed`'s path for our user agent.
    async fn check_robots(
        &self,
        checked: &CheckedUrl,
        parsed: &Url,
        deadline: Instant,
    ) -> Result<(), FetchError> {
        let robots_url = format!(
            "{}://{}/robots.txt",
            if checked.https { "https" } else { "http" },
            robots_authority(checked)
        );
        let mut text = String::new();
        let len = std::sync::atomic::AtomicUsize::new(0);
        let mut sink = |s: &str| {
            if text.len() < ROBOTS_MAX_BYTES {
                text.push_str(s);
            }
            len.store(text.len(), std::sync::atomic::Ordering::Relaxed);
        };
        let stop = || len.load(std::sync::atomic::Ordering::Relaxed) >= ROBOTS_MAX_BYTES;
        let sub_deadline =
            deadline.min(Instant::now() + self.limits.timeout / ROBOTS_SUB_DEADLINE_FRACTION);
        // `run` recurses into itself here (fetching robots.txt is just another guarded fetch), so the call is
        // boxed to give the compiler a finitely-sized future. A timeout on the sub-deadline is just another
        // fetch failure here: fail-open discards it exactly like any other `run` error.
        let _ = timeout_at(
            sub_deadline,
            Box::pin(self.run(
                &robots_url,
                Mode::Raw,
                &mut sink,
                &stop,
                sub_deadline,
                false,
            )),
        )
        .await;
        if crate::robots::is_allowed(&text, USER_AGENT_VALUE, parsed.path()) {
            Ok(())
        } else {
            Err(FetchError::RobotsDisallowed(format!(
                "robots.txt disallows fetching {}",
                parsed.path()
            )))
        }
    }

    async fn read_body(
        &self,
        mut resp: reqwest::Response,
        mode: Mode,
        base: &Url,
        sink: &mut (dyn FnMut(&str) + Send),
        stop: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<u64, FetchError> {
        let cap = self.limits.max_bytes;
        let gzip = content_encoding_is_gzip(resp.headers())?;
        // A type we do not return is refused before a body byte is read (A-6); so is a declared length over the cap.
        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut conv = convert::for_response(mode, content_type.as_deref(), Some(base.clone()))
            .map_err(|t| FetchError::UnsupportedContentType(unsupported_text(&t.0)))?;
        if resp.content_length().is_some_and(|n| n > cap) {
            return Err(too_large());
        }
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

        // A-8: charset detection, priority (a) header, (b) HTML <meta> sniff, (c) UTF-8. A header charset that
        // `encoding_rs` recognizes as non-UTF-8 is known up front, so the (rare) full-body decode path starts
        // immediately. Otherwise (no header charset, or it names UTF-8/is unrecognized) an HTML response gets a
        // short lookahead (`sniff_prefix`) to check for a <meta charset> tag; every other case -- including the
        // large majority of real HTML, which is UTF-8 with no charset markers at all -- keeps the original
        // streaming, early-abort path untouched.
        let header_charset = content_type.as_deref().and_then(charset::from_content_type);
        if let Some(enc) = header_charset.filter(|e| !std::ptr::eq(*e, charset::utf8())) {
            let wire = self
                .decode_whole_body(
                    &mut resp,
                    gzip,
                    cap,
                    enc,
                    Vec::new(),
                    &mut convert_step,
                    &failure,
                    stop,
                )
                .await?;
            take_failure(&failure)?;
            if stop() {
                return Ok(wire);
            }
            conv.finish(&mut *sink).map_err(map_convert)?;
            return Ok(wire);
        }
        if header_charset.is_none() && charset::is_html(content_type.as_deref()) && gzip {
            // A-8 tradeoff: unlike the identity case below, a gzip body cannot be sniffed a few KiB at a time --
            // flate2's write-style gzip decoder does not reliably flush a small amount of pending output before
            // it is finished, so `RawPipeline::buffered` cannot be trusted as a partial peek here. A gzip HTML
            // response with no header charset therefore buffers the whole (still capped) body, exactly the
            // documented tradeoff for the rare non-UTF-8 case, just taken slightly more often (any gzip HTML
            // page with no explicit charset, whatever its actual encoding turns out to be).
            let mut raw = body::RawPipeline::new(true, cap);
            let mut wire = 0u64;
            while let Some(chunk) = resp.chunk().await.map_err(map_transport)? {
                wire = wire.saturating_add(chunk.len() as u64);
                if wire > cap {
                    return Err(too_large());
                }
                raw.feed(&chunk).map_err(map_body)?;
                if stop() {
                    // Early stop: the gzip stream is truncated here, so `raw.finish()` would fail
                    // (`BodyError::Corrupt`) on the incomplete stream -- return before calling it,
                    // exactly like the other early-stop sites in this function.
                    return Ok(wire);
                }
            }
            let bytes = raw.finish().map_err(map_body)?;
            let window = &bytes[..bytes.len().min(SNIFF_WINDOW)];
            let enc = charset::sniff_meta(window).unwrap_or_else(charset::utf8);
            let text = charset::decode(enc, &bytes);
            push_chunked(&text, &mut convert_step, &|| has_failure(&failure));
            take_failure(&failure)?;
            conv.finish(&mut *sink).map_err(map_convert)?;
            return Ok(wire);
        }
        if header_charset.is_none() && charset::is_html(content_type.as_deref()) {
            let sniff = self.sniff_prefix(&mut resp, gzip, cap, stop).await?;
            if let Some(hit) = sniff.stopped_early {
                return Ok(hit);
            }
            let window = &sniff.decoded[..sniff.decoded.len().min(SNIFF_WINDOW)];
            if let Some(enc) =
                charset::sniff_meta(window).filter(|e| !std::ptr::eq(*e, charset::utf8()))
            {
                let wire = self
                    .decode_whole_body(
                        &mut resp,
                        gzip,
                        cap,
                        enc,
                        sniff.wire_prefix,
                        &mut convert_step,
                        &failure,
                        stop,
                    )
                    .await?;
                take_failure(&failure)?;
                if stop() {
                    return Ok(wire);
                }
                conv.finish(&mut *sink).map_err(map_convert)?;
                return Ok(wire);
            }
            // UTF-8 (default or explicitly sniffed): replay the buffered prefix through a fresh, normal
            // streaming pipeline, then continue streaming the rest of the body exactly as the fast path below.
            let mut pipe = body::Pipeline::new(gzip, cap, &mut convert_step);
            pipe.feed(&sniff.wire_prefix).map_err(map_body)?;
            take_failure(&failure)?;
            let mut wire = sniff.wire_prefix.len() as u64;
            while let Some(chunk) = resp.chunk().await.map_err(map_transport)? {
                wire = wire.saturating_add(chunk.len() as u64);
                if wire > cap {
                    return Err(too_large());
                }
                pipe.feed(&chunk).map_err(map_body)?;
                take_failure(&failure)?;
                if stop() {
                    return Ok(wire);
                }
            }
            pipe.finish().map_err(map_body)?;
            take_failure(&failure)?;
            conv.finish(&mut *sink).map_err(map_convert)?;
            return Ok(wire);
        }

        // Fast path: charset is UTF-8 (by header, or default for a non-HTML type), unchanged from before A-8.
        let mut pipe = body::Pipeline::new(gzip, cap, &mut convert_step);
        let mut wire = 0u64;
        while let Some(chunk) = resp.chunk().await.map_err(map_transport)? {
            wire = wire.saturating_add(chunk.len() as u64);
            if wire > cap {
                return Err(too_large());
            }
            pipe.feed(&chunk).map_err(map_body)?;
            take_failure(&failure)?;
            if stop() {
                return Ok(wire); // early stop: dropping `resp` closes the connection
            }
        }
        pipe.finish().map_err(map_body)?;
        take_failure(&failure)?;
        conv.finish(&mut *sink).map_err(map_convert)?;
        Ok(wire)
    }

    /// A-8 charset sniffing: reads wire chunks into `wire_prefix` (and, decompressed, into a [`body::RawPipeline`])
    /// until [`SNIFF_WINDOW`] decoded bytes have been gathered, the wire budget ([`SNIFF_WIRE_BUDGET`], clamped to
    /// `cap`) is spent, or the body ends -- never more than a few KiB, so a large body's early-abort behaviour is
    /// unaffected by this lookahead. `stopped_early` is `Some(wire_bytes)` when `stop()` fired during the peek
    /// (the caller returns that count as-is, matching the early-stop contract).
    ///
    /// Identity bodies only (finding #8, round-2 review): its one call site is reached only when `read_body`'s
    /// own gzip-HTML branch (which handles the gzip case separately, see its docs) was not taken, so `gzip` is
    /// always `false` here. [`body::RawPipeline::new`] still takes a `gzip` flag generally, so this passes it
    /// through rather than hard-coding `false`, keeping the method honest about what it decompresses instead of
    /// silently assuming identity.
    async fn sniff_prefix(
        &self,
        resp: &mut reqwest::Response,
        gzip: bool,
        cap: u64,
        stop: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Sniff, FetchError> {
        let wire_budget = SNIFF_WIRE_BUDGET.min(cap);
        let mut raw = body::RawPipeline::new(gzip, cap);
        let mut wire_prefix = Vec::new();
        while (wire_prefix.len() as u64) < wire_budget && raw.buffered().len() < SNIFF_WINDOW {
            let Some(chunk) = resp.chunk().await.map_err(map_transport)? else {
                break;
            };
            if (wire_prefix.len() as u64).saturating_add(chunk.len() as u64) > cap {
                return Err(too_large());
            }
            wire_prefix.extend_from_slice(&chunk);
            raw.feed(&chunk).map_err(map_body)?;
            if stop() {
                return Ok(Sniff {
                    decoded: Vec::new(),
                    wire_prefix: Vec::new(),
                    stopped_early: Some(wire_prefix.len() as u64),
                });
            }
        }
        Ok(Sniff {
            decoded: raw.buffered().to_vec(),
            wire_prefix,
            stopped_early: None,
        })
    }

    /// A-8 non-UTF-8 path: decodes the body with `encoding_rs`, streaming (via [`body::Pipeline::with_encoding`])
    /// exactly like the UTF-8 fast path rather than buffering it whole -- so `stop()` (max_length / A-5 window)
    /// ends the read as soon as it fires, the same early-stop contract as every other site in `read_body`.
    /// `wire_prefix` is any bytes [`FetchClient::sniff_prefix`] already read off the wire (fed first); the rest
    /// of `resp` is then read to completion or until `stop()`.
    #[allow(clippy::too_many_arguments)]
    async fn decode_whole_body(
        &self,
        resp: &mut reqwest::Response,
        gzip: bool,
        cap: u64,
        encoding: &'static charset::Encoding,
        wire_prefix: Vec<u8>,
        convert_step: &mut (dyn FnMut(&str) + Send),
        failure: &Mutex<Option<ConvertError>>,
        stop: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<u64, FetchError> {
        let mut pipe = body::Pipeline::with_encoding(gzip, cap, encoding, convert_step);
        let mut wire = wire_prefix.len() as u64;
        if !wire_prefix.is_empty() {
            pipe.feed(&wire_prefix).map_err(map_body)?;
            take_failure(failure)?;
        }
        while let Some(chunk) = resp.chunk().await.map_err(map_transport)? {
            wire = wire.saturating_add(chunk.len() as u64);
            if wire > cap {
                return Err(too_large());
            }
            pipe.feed(&chunk).map_err(map_body)?;
            take_failure(failure)?;
            if stop() {
                return Ok(wire); // early stop: dropping `resp` closes the connection
            }
        }
        pipe.finish().map_err(map_body)?;
        take_failure(failure)?;
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

fn unsupported_text(media_type: &str) -> String {
    format!(
        "the response is {media_type}, which is not text; only HTML, plain text, JSON and XML are returned"
    )
}

/// A-8: push `text` to `convert_step` in pieces at (or below) [`body::STEP`], each ending on a char boundary so
/// it is valid UTF-8 on its own -- used by the whole-body decode paths, which have all of `text` at once rather
/// than getting it in wire-sized pieces the way the streaming pipeline does. `failed` is checked between pieces
/// so a converter failure recorded mid-decode (`convert_step` cannot itself return an error) aborts the rest of
/// the push instead of continuing to decode a body whose conversion has already failed.
fn push_chunked(
    text: &str,
    convert_step: &mut (dyn FnMut(&str) + Send),
    failed: &dyn Fn() -> bool,
) {
    let mut rest = text;
    while !rest.is_empty() {
        if failed() {
            return;
        }
        let mut end = rest.len().min(body::STEP);
        while end > 0 && !rest.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            end = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        convert_step(&rest[..end]);
        rest = &rest[end..];
    }
}

/// B-4: `host[:port]` for a robots.txt request URL, port included only when it is not the scheme's default.
fn robots_authority(u: &CheckedUrl) -> String {
    let host = match &u.host {
        Host::Name(n) => n.clone(),
        Host::Ip(std::net::IpAddr::V4(v4)) => v4.to_string(),
        Host::Ip(std::net::IpAddr::V6(v6)) => format!("[{v6}]"),
    };
    let default_port = if u.https { 443 } else { 80 };
    if u.port == default_port {
        host
    } else {
        format!("{host}:{}", u.port)
    }
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

fn has_failure(f: &Mutex<Option<ConvertError>>) -> bool {
    f.lock().unwrap_or_else(PoisonError::into_inner).is_some()
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
