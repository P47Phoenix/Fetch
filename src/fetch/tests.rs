//! Tests for the guarded client, run against a real loopback HTTP server through the real client. Loopback is
//! reachable only through `Policy::permit_loopback_for_tests()` (unit-test cfg); the default policy is used for
//! the refusal tests. Names (`public.test`) resolve through the injectable fake resolver.

use super::dns::Pinned;
use super::{FetchClient, Limits, MAX_HEADER_BYTES, MAX_HEADER_COUNT, MAX_REDIRECTS};
use crate::error::FetchError;
use crate::policy::Policy;
use crate::ssrf::resolver::testing::FakeResolver;
use crate::ssrf::{check_url, Origin};
use flate2::{write::GzEncoder, Compression};
use std::future::Future;
use std::io::Write as _;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Handler =
    Arc<dyn Fn(TcpStream, String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

struct Server {
    port: u16,
    accepted: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
    fn heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }
}

async fn spawn_server(handler: Handler) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let heads = Arc::new(Mutex::new(Vec::new()));
    let (acc, hd) = (accepted.clone(), heads.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            acc.fetch_add(1, Ordering::SeqCst);
            let (handler, hd) = (handler.clone(), hd.clone());
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                hd.lock().unwrap().push(head.clone());
                handler(sock, head).await;
            });
        }
    });
    Server {
        port,
        accepted,
        heads,
    }
}

fn handler<F, Fut>(f: F) -> Handler
where
    F: Fn(TcpStream, String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Arc::new(move |s, h| Box::pin(f(s, h)))
}

/// A fixed response: `Connection: close`, an explicit `Content-Length`.
fn fixed(status: &'static str, extra: &'static [&'static str], body: Vec<u8>) -> Handler {
    let body = Arc::new(body);
    handler(move |mut s, _| {
        let body = body.clone();
        async move {
            let mut head = format!(
                "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
                body.len()
            );
            for h in extra {
                head.push_str(h);
                head.push_str("\r\n");
            }
            head.push_str("\r\n");
            let _ = s.write_all(head.as_bytes()).await;
            let _ = s.write_all(&body).await;
            let _ = s.shutdown().await;
        }
    })
}

fn limits(timeout_ms: u64, max_bytes: u64, concurrency: usize) -> Limits {
    Limits {
        timeout: Duration::from_millis(timeout_ms),
        max_bytes,
        concurrency,
    }
}

fn client(policy: Policy, r: &Arc<FakeResolver>, l: Limits) -> FetchClient<FakeResolver> {
    FetchClient::new(policy, r.clone(), l)
}

fn loopback() -> Policy {
    Policy::permit_loopback_for_tests()
}

fn public_resolver() -> Arc<FakeResolver> {
    Arc::new(FakeResolver::new().on("public.test", &["127.0.0.1"]))
}

async fn get<R: crate::ssrf::resolver::Resolver>(
    c: &FetchClient<R>,
    url: &str,
) -> (Result<super::Fetched, FetchError>, String, usize) {
    let mut text = String::new();
    let mut biggest = 0usize;
    // Test-level guard: a removed or broken deadline must fail here, not hang the suite.
    let r = tokio::time::timeout(
        Duration::from_secs(60),
        c.fetch(url, &mut |s: &str| {
            biggest = biggest.max(s.len());
            text.push_str(s);
        }),
    )
    .await
    .expect("fetch hung past the test-level guard: the client deadline is not working");
    (r, text, biggest)
}

fn code(r: &Result<super::Fetched, FetchError>) -> &'static str {
    match r {
        Ok(_) => "ok",
        Err(e) => e.code(),
    }
}

async fn wait_for(flag: &AtomicBool) -> bool {
    for _ in 0..100 {
        if flag.load(Ordering::SeqCst) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

fn gz(data: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

// ---- happy path ---------------------------------------------------------------------------------------

#[tokio::test]
async fn fetches_a_small_body_through_a_validated_name() {
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain"],
        b"hello".to_vec(),
    ))
    .await;
    let r = public_resolver();
    let c = client(loopback(), &r, limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/x", srv.port)).await;
    let f = res.unwrap();
    assert_eq!(
        (f.status, f.redirects, f.wire_bytes, text.as_str()),
        (200, 0, 5, "hello")
    );
    let head = &srv.heads()[0];
    assert!(head.starts_with("GET /x HTTP/1.1\r\n"), "{head}");
    assert!(head
        .to_ascii_lowercase()
        .contains(&format!("host: public.test:{}", srv.port)));
}

#[tokio::test]
async fn non_success_status_is_reported() {
    let srv = spawn_server(fixed("404 Not Found", &[], b"nope".to_vec())).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(res, Err(FetchError::HttpStatus(404)));
    assert!(text.is_empty());
}

// ---- A-7: each cause end to end, flag and message text through the tool result --------------------------

/// The tool result text for a failed fetch, asserting `isError: true`.
fn failure_text(res: Result<super::Fetched, FetchError>) -> String {
    let r = crate::server::error_result(&res.expect_err("the fetch must fail"));
    assert_eq!(r.is_error, Some(true));
    format!("{:?}", r.content)
}

#[tokio::test]
async fn a7_each_cause_is_flagged_and_names_itself() {
    let base = |port| format!("http://public.test:{port}/");
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    // Only the timeout case needs a short deadline.
    let short = client(loopback(), &public_resolver(), limits(400, 1 << 20, 3));
    for (status, want) in [
        (
            "404 Not Found",
            "error[http_error]: the server refused the request with HTTP status 404 (Not Found)",
        ),
        (
            "500 Internal Server Error",
            "error[http_error]: the server failed with HTTP status 500 (Internal Server Error)",
        ),
    ] {
        let srv = spawn_server(fixed(status, &[], b"x".to_vec())).await;
        let (res, _, _) = get(&c, &base(srv.port)).await;
        let t = failure_text(res);
        assert!(t.contains(want), "{t}");
    }
    // DNS failure.
    let dns = client(
        Policy::default(),
        &Arc::new(FakeResolver::new().fail("gone.test")),
        limits(5000, 1 << 20, 3),
    );
    let (res, _, _) = get(&dns, "http://gone.test/").await;
    assert!(failure_text(res).contains("error[dns_failure]: hostname did not resolve"));
    // Blocked target.
    let (res, _, _) = get(&dns, "http://10.0.0.1/").await;
    assert!(failure_text(res).contains("error[blocked_target]: "));
    // Timeout.
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\npartial")
            .await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }))
    .await;
    let (res, _, _) = get(&short, &base(srv.port)).await;
    assert!(failure_text(res).contains("error[timeout]: the request timed out"));
    // Too large.
    let small = client(loopback(), &public_resolver(), limits(5000, 10, 3));
    let srv = spawn_server(fixed("200 OK", &[], vec![b'a'; 100])).await;
    let (res, _, _) = get(&small, &base(srv.port)).await;
    assert!(
        failure_text(res).contains("error[too_large]: the response is larger than the size limit")
    );
    // Unsupported type.
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: image/png"],
        vec![0x89, b'P', b'N', b'G'],
    ))
    .await;
    let (res, _, _) = get(&c, &base(srv.port)).await;
    let t = failure_text(res);
    assert!(
        t.contains("error[unsupported_content_type]: ") && t.contains("image/png"),
        "{t}"
    );
}

// ---- the four refusals (EPICS A-3b: not Done until these pass) --------------------------------------

#[tokio::test]
async fn refusal_loopback_ip_literal() {
    let srv = spawn_server(fixed("200 OK", &[], b"secret".to_vec())).await;
    let r = Arc::new(FakeResolver::new());
    let c = client(Policy::default(), &r, limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://127.0.0.1:{}/", srv.port)).await;
    assert_eq!(code(&res), "blocked_target", "{res:?}");
    assert!(text.is_empty());
    assert_eq!(srv.accepted(), 0, "no connection may be attempted");
    assert_eq!(r.count(), 0, "a literal is never resolved");
}

#[tokio::test]
async fn refusal_metadata_ip_literal() {
    let r = Arc::new(FakeResolver::new());
    let c = client(Policy::default(), &r, limits(5000, 1 << 20, 3));
    for u in [
        "http://169.254.169.254/latest/meta-data/",
        "http://[fd00:ec2::254]/",
        "http://0xa9fea9fe/",
        "http://2852039166/",
    ] {
        let (res, _, _) = get(&c, u).await;
        assert_eq!(code(&res), "blocked_target", "{u}: {res:?}");
    }
    assert_eq!(r.count(), 0);
}

#[tokio::test]
async fn refusal_name_resolving_to_a_private_address() {
    let srv = spawn_server(fixed("200 OK", &[], b"lan".to_vec())).await;
    // Even with loopback permitted, a private answer (alone or mixed with a public one) is refused whole.
    let r = Arc::new(
        FakeResolver::new()
            .on("private.test", &["10.1.2.3"])
            .on("private.test", &["10.1.2.3"]),
    );
    let c = client(loopback(), &r, limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://private.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "blocked_target", "{res:?}");
    let mixed = Arc::new(FakeResolver::new().on("mixed.test", &["127.0.0.1", "192.168.0.10"]));
    let c = client(loopback(), &mixed, limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://mixed.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "blocked_target", "{res:?}");
    assert_eq!(srv.accepted(), 0, "refused before any connection");
}

#[tokio::test]
async fn refusal_redirect_to_a_private_address() {
    for target in [
        "http://10.0.0.1/admin",
        "http://169.254.169.254/latest/meta-data/",
        "http://[fd00::1]:9/",
        "http://192.168.1.1:8080/",
        "http://0xa000001/",
    ] {
        let loc: &'static str = Box::leak(format!("Location: {target}").into_boxed_str());
        let extra: &'static [&'static str] = Box::leak(vec![loc].into_boxed_slice());
        let srv = spawn_server(fixed("302 Found", extra, Vec::new())).await;
        // Loopback permitted so the first hop can reach the fixture; the redirect target is still refused.
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert_eq!(code(&res), "blocked_target", "{target}: {res:?}");
        assert_eq!(srv.accepted(), 1, "only the first hop is ever contacted");
    }
}

#[tokio::test]
async fn redirect_to_non_http_or_userinfo_target_is_blocked() {
    for target in [
        "file:///etc/passwd",
        "ftp://public.test/x",
        "http://user:pw@public.test/",
    ] {
        let loc: &'static str = Box::leak(format!("Location: {target}").into_boxed_str());
        let extra: &'static [&'static str] = Box::leak(vec![loc].into_boxed_slice());
        let srv = spawn_server(fixed("301 Moved", extra, Vec::new())).await;
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert_eq!(code(&res), "blocked_target", "{target}: {res:?}");
    }
}

// ---- redirects ---------------------------------------------------------------------------------------

#[tokio::test]
async fn relative_redirect_is_followed_and_every_hop_is_resolved_once() {
    let srv = spawn_server(handler(|mut s, head| async move {
        let ok = head.starts_with("GET /final");
        let resp = if ok {
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\n\r\ndone"
        } else {
            "HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /final?x=1\r\n\r\n"
        };
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await;
    let r = public_resolver();
    let c = client(loopback(), &r, limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/start", srv.port)).await;
    let f = res.unwrap();
    assert_eq!((f.redirects, text.as_str()), (1, "done"));
    assert_eq!(
        f.final_url,
        format!("http://public.test:{}/final?x=1", srv.port)
    );
    assert_eq!(r.count(), 2, "one lookup per hop, none more");
    assert_eq!(srv.accepted(), 2, "no connection reuse");
}

#[test]
fn echoed_url_has_no_userinfo_or_fragment() {
    let u = url::Url::parse("https://user:secret@b.example:8443/p?q=1#frag").unwrap();
    assert_eq!(super::echo_url(&u), "https://b.example:8443/p?q=1");
}

#[tokio::test]
async fn redirect_fragment_is_not_echoed_in_final_url() {
    let srv = spawn_server(handler(|mut s, head| async move {
        let resp = if head.starts_with("GET /final") {
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\n\r\ndone"
        } else {
            "HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /final#secret\r\n\r\n"
        };
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/start", srv.port)).await;
    assert_eq!(
        res.unwrap().final_url,
        format!("http://public.test:{}/final", srv.port)
    );
}

/// A-9 end to end: the header text a redirected call returns never carries userinfo or a fragment, from either the
/// request URL or the redirect Location. Userinfo cannot survive to the echo at all (the SSRF core refuses it on
/// the first hop and on every redirect target), so the credential is proved absent from the refusal text too.
#[tokio::test]
async fn a9_echo_has_no_userinfo_or_fragment_end_to_end() {
    let srv = spawn_server(handler(|mut s, head| async move {
        let resp = if head.starts_with("GET /final") {
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\n\r\ndone"
        } else {
            "HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /final?a=1#locfrag\r\n\r\n"
        };
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(
        &c,
        &format!("http://public.test:{}/start#reqfrag", srv.port),
    )
    .await;
    let f = res.unwrap();
    let echoed = crate::server::with_header(&f, text);
    assert!(echoed.starts_with(&format!(
        "URL: http://public.test:{}/final?a=1\nStatus: 200\n\ndone",
        srv.port
    )));
    for bad in ["reqfrag", "locfrag", "#", "@", "secret"] {
        assert!(!echoed.contains(bad), "{bad} leaked into {echoed}");
    }
    // userinfo on the request URL, and on a redirect Location: refused, never echoed
    let (res, _, _) = get(
        &c,
        &format!("http://u:secret@public.test:{}/start", srv.port),
    )
    .await;
    let e = res.unwrap_err();
    assert!(
        ["invalid_argument", "blocked_target"].contains(&e.code()),
        "{e:?}"
    );
    assert!(!e.tool_text().contains("secret"));
    let srv2 = spawn_server(fixed(
        "302 Found",
        &["Location: http://u:secret@public.test/x"],
        Vec::new(),
    ))
    .await;
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv2.port)).await;
    let e = res.unwrap_err();
    assert_eq!(e.code(), "blocked_target");
    assert!(!e.tool_text().contains("secret"));
}

#[tokio::test]
async fn redirect_loop_stops_at_the_bound() {
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /again\r\n\r\n")
            .await;
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(res, Err(FetchError::TooManyRedirects));
    assert_eq!(srv.accepted(), MAX_REDIRECTS + 1);
}

/// B-3: the server redirects `/n` to `/n+1` until `/{len}`, which answers 200.
async fn chain_server(len: usize) -> Server {
    spawn_server(handler(move |mut s, head| async move {
        let n: usize = head
            .split_whitespace()
            .nth(1)
            .and_then(|p| p.trim_start_matches('/').parse().ok())
            .unwrap_or(0);
        let resp = if n >= len {
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok".to_string()
        } else {
            format!("HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /{}\r\n\r\n", n + 1)
        };
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await
}

/// B-3: a chain of exactly `MAX_REDIRECTS` (5) hops succeeds; the sixth redirect fails with too_many_redirects.
#[tokio::test]
async fn b3_a_chain_of_five_redirects_succeeds_and_a_chain_of_six_fails() {
    assert_eq!(MAX_REDIRECTS, 5);
    let ok = chain_server(5).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/0", ok.port)).await;
    let f = res.unwrap();
    assert_eq!((f.redirects, text.as_str()), (5, "ok"));
    assert_eq!(ok.accepted(), 6);

    let bad = chain_server(6).await;
    let (res, text, _) = get(&c, &format!("http://public.test:{}/0", bad.port)).await;
    assert_eq!(res, Err(FetchError::TooManyRedirects));
    assert!(text.is_empty());
    assert_eq!(bad.accepted(), 6, "the sixth redirect is never followed");
}

/// B-3 / B-2: a redirect to any encoded or IPv6 spelling of a blocked address is refused; only hop 1 connects.
#[tokio::test]
async fn b3_redirect_to_every_encoded_blocked_form_is_refused_without_a_second_connection() {
    for target in [
        "http://167772161/",
        "http://0xa000001/",
        "http://012.0.0.1/",
        "http://10.1/",
        "http://[::ffff:10.0.0.1]/",
        "http://[::ffff:a00:1]/",
        "http://[fc00::1]/",
        "http://0.0.0.0/",
        "http://[::]/",
    ] {
        let loc: &'static str = Box::leak(format!("Location: {target}").into_boxed_str());
        let extra: &'static [&'static str] = Box::leak(vec![loc].into_boxed_slice());
        let srv = spawn_server(fixed("302 Found", extra, Vec::new())).await;
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert_eq!(code(&res), "blocked_target", "{target}: {res:?}");
        assert_eq!(srv.accepted(), 1, "{target}: only the first hop connects");
    }
}

/// B-2 at `revalidate_hop` (the function the redirect loop calls; the loop itself is covered by the b3_ redirect test above) with the default policy: loopback in every encoded and IPv6 spelling is refused.
#[tokio::test]
async fn b2_revalidate_hop_refuses_every_loopback_spelling() {
    let r = FakeResolver::new();
    for t in [
        "http://2130706433/",
        "http://0x7f000001/",
        "http://0177.0.0.1/",
        "http://127.1/",
        "http://[::1]/",
        "http://[::ffff:127.0.0.1]/",
        "http://[::ffff:7f00:1]/",
    ] {
        let e = crate::ssrf::resolver::revalidate_hop(&r, &Policy::default(), t)
            .await
            .expect_err(t);
        assert_eq!(e.code(), "blocked_target", "{t}: {e:?}");
    }
    assert_eq!(r.count(), 0);
}

/// B-2 end to end: encoded and IPv6 spellings given as the request URL are refused before any connection or lookup.
#[tokio::test]
async fn b2_encoded_and_ipv6_forms_are_refused_as_the_request_url() {
    let srv = spawn_server(fixed("200 OK", &[], b"lan".to_vec())).await;
    let r = Arc::new(FakeResolver::new());
    let c = client(Policy::default(), &r, limits(5000, 1 << 20, 3));
    for host in [
        "2130706433",
        "0x7f000001",
        "0X7F.0.0.1",
        "0177.0.0.1",
        "017700000001",
        "127.1",
        "127.0.1",
        "0",
        "0.0.0.0",
        "[::1]",
        "[0:0:0:0:0:0:0:1]",
        "[::ffff:127.0.0.1]",
        "[::ffff:7f00:1]",
        "[fc00::1]",
        "[fd00::1]",
        "[::]",
    ] {
        let (res, text, _) = get(&c, &format!("http://{host}:{}/", srv.port)).await;
        let e = res.expect_err(host);
        assert_eq!(e.code(), "blocked_target", "{host}: {e:?}");
        assert!(text.is_empty(), "{host}");
    }
    assert_eq!(r.count(), 0, "encoded literals never reach the resolver");
    assert_eq!(srv.accepted(), 0, "no connection to any encoded form");
}

/// Resolver that never answers for one name.
struct HangResolver {
    hang_on: &'static str,
}
impl crate::ssrf::resolver::Resolver for HangResolver {
    fn resolve(
        &self,
        host: &str,
    ) -> impl Future<Output = std::io::Result<Vec<std::net::IpAddr>>> + Send {
        let hang = host == self.hang_on;
        async move {
            if hang {
                std::future::pending::<()>().await;
            }
            Ok(vec!["127.0.0.1".parse().unwrap()])
        }
    }
}

/// B-3 note (A-3b fix-pass 1): a resolver that never answers is bounded by the fetch deadline on the first hop and
/// on a redirect hop, and the concurrency slot is released so later fetches still run.
#[tokio::test]
async fn b3_a_hung_resolver_is_bounded_by_the_deadline_on_every_hop_and_frees_the_slot() {
    let srv = spawn_server(fixed(
        "302 Found",
        &["Location: http://hang.test/"],
        Vec::new(),
    ))
    .await;
    let c = FetchClient::new(
        loopback(),
        Arc::new(HangResolver {
            hang_on: "hang.test",
        }),
        limits(400, 1 << 20, 1),
    );
    let t = std::time::Instant::now();
    let (res, _, _) = get(&c, "http://hang.test/").await;
    assert_eq!(code(&res), "timeout", "{res:?}");
    assert!(t.elapsed() < Duration::from_secs(5));
    // Redirect hop hangs. Concurrency is 1: a leaked slot would make later attempts queue.
    for _ in 0..3 {
        let t = std::time::Instant::now();
        let (res, _, _) = get(&c, &format!("http://ok.test:{}/", srv.port)).await;
        assert_eq!(code(&res), "timeout", "{res:?}");
        assert!(t.elapsed() < Duration::from_secs(5));
    }
    assert_eq!(srv.accepted(), 3, "each attempt reached hop 1");
}

// ---- dial-once and dial-only-validated ---------------------------------------------------------------

/// B-1 end to end: every blocked class reached by name (alone or mixed with a public answer) and by literal is
/// refused before a connection is made, and the message never carries the address.
#[tokio::test]
async fn b1_every_blocked_class_by_name_mixed_and_literal_is_refused_before_any_connection() {
    let srv = spawn_server(fixed("200 OK", &[], b"lan".to_vec())).await;
    let blocked = [
        "127.0.0.1",
        "127.9.9.9",
        "10.0.0.1",
        "172.16.0.1",
        "172.31.255.254",
        "192.168.1.1",
        "169.254.169.254",
        "169.254.0.1",
        "100.64.0.1",
        "0.0.0.0",
        "::1",
        "::",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "::ffff:10.0.0.1",
        "::ffff:127.0.0.1",
    ];
    // A public address (example.com's) mixed into the blocked answers: the whole answer must be refused, in either order.
    // The FakeResolver stands in for the OS resolver, so this covers our filter, not getaddrinfo's behaviour.
    const PUBLIC: &str = "93.184.216.34";
    let mut resolver = FakeResolver::new();
    for (i, ip) in blocked.iter().enumerate() {
        resolver = resolver
            .on(&format!("alone{i}.test"), &[ip])
            .on(&format!("mixed{i}.test"), &[PUBLIC, ip])
            .on(&format!("mixedfirst{i}.test"), &[ip, PUBLIC]);
    }
    let r = Arc::new(resolver);
    let c = client(Policy::default(), &r, limits(5000, 1 << 20, 3));
    let mut lookups = 0;
    for (i, ip) in blocked.iter().enumerate() {
        for kind in ["alone", "mixed", "mixedfirst"] {
            let (res, text, _) = get(&c, &format!("http://{kind}{i}.test:{}/", srv.port)).await;
            lookups += 1;
            assert_eq!(code(&res), "blocked_target", "{kind} {ip}: {res:?}");
            assert!(text.is_empty());
            let msg = res.unwrap_err().tool_text();
            assert!(!msg.contains(ip) && !msg.contains(PUBLIC), "{msg}");
        }
        let literal = if ip.contains(':') {
            format!("http://[{ip}]:{}/", srv.port)
        } else {
            format!("http://{ip}:{}/", srv.port)
        };
        let (res, _, _) = get(&c, &literal).await;
        assert_eq!(code(&res), "blocked_target", "literal {ip}: {res:?}");
        let msg = res.unwrap_err().tool_text();
        assert!(!msg.contains(ip), "literal {ip}: {msg}");
    }
    assert_eq!(
        r.count(),
        lookups,
        "names resolve once each, literals never"
    );
    // `accepted()` counts real connections (positive control: `fetches_a_small_body_through_a_validated_name` and
    // `dial_once_...` assert accepted() == 1 for an allowed loopback fetch against the same helper).
    assert_eq!(srv.accepted(), 0, "no connection to any blocked answer");
}

#[tokio::test]
async fn dial_once_a_second_private_answer_is_never_looked_up_or_dialled() {
    let srv = spawn_server(fixed("200 OK", &[], b"pinned".to_vec())).await;
    // Rebinding shape: public (here loopback, permitted) on the first lookup, private on any second one.
    let r = Arc::new(
        FakeResolver::new()
            .on("public.test", &["127.0.0.1"])
            .on("public.test", &["10.0.0.1"]),
    );
    let c = client(loopback(), &r, limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(text, "pinned");
    assert_eq!(r.count(), 1, "the client must not re-resolve");
    assert_eq!(srv.accepted(), 1);
}

// ---- sizes, timeouts ---------------------------------------------------------------------------------

#[tokio::test]
async fn content_length_over_the_cap_aborts_before_reading_the_body() {
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10485760\r\n\r\nfirst-bytes")
            .await;
        tokio::time::sleep(Duration::from_secs(30)).await; // never sends the rest
    }))
    .await;
    let c = client(
        loopback(),
        &public_resolver(),
        limits(20_000, 5 * 1024 * 1024, 3),
    );
    let started = std::time::Instant::now();
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "too_large", "{res:?}");
    assert!(text.is_empty(), "no body byte may reach the sink");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "must not wait for the body"
    );
}

#[tokio::test]
async fn chunked_body_over_the_cap_stops_reading_and_closes_the_connection() {
    let sent = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let (sent2, done2) = (sent.clone(), done.clone());
    let srv = spawn_server(handler(move |mut s, _| {
        let (sent, done) = (sent2.clone(), done2.clone());
        async move {
            let _ = s
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let chunk = vec![b'a'; 16 * 1024];
            loop {
                let frame = [format!("{:x}\r\n", chunk.len()).as_bytes(), &chunk, b"\r\n"].concat();
                if s.write_all(&frame).await.is_err() {
                    break;
                }
                sent.fetch_add(chunk.len(), Ordering::SeqCst);
                tokio::task::yield_now().await;
            }
            done.store(true, Ordering::SeqCst);
        }
    }))
    .await;
    let cap = 256 * 1024;
    let c = client(loopback(), &public_resolver(), limits(20_000, cap, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "too_large", "{res:?}");
    assert!(text.len() as u64 <= cap);
    assert!(
        wait_for(&done).await,
        "the connection must be closed once the cap is hit"
    );
    assert!(
        sent.load(Ordering::SeqCst) < 32 * 1024 * 1024,
        "server was not throttled to a stop"
    );
}

#[tokio::test]
async fn a_server_that_never_finishes_times_out_and_the_connection_is_closed() {
    let closed = Arc::new(AtomicBool::new(false));
    let closed2 = closed.clone();
    let srv = spawn_server(handler(move |mut s, _| {
        let closed = closed2.clone();
        async move {
            let _ = s
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\npartial")
                .await;
            let mut b = [0u8; 16];
            while matches!(s.read(&mut b).await, Ok(n) if n > 0) {}
            closed.store(true, Ordering::SeqCst);
        }
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(400, 1 << 20, 3));
    let started = std::time::Instant::now();
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "timeout", "{res:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        wait_for(&closed).await,
        "the client must close the connection on timeout"
    );
}

#[tokio::test]
async fn a_slow_drip_body_cannot_outlive_the_overall_deadline() {
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await;
        for _ in 0..100 {
            if s.write_all(b"1\r\nx\r\n").await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(500, 1 << 20, 3));
    let started = std::time::Instant::now();
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "timeout", "{res:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

// ---- request shape (NFR-07) ----------------------------------------------------------------------------

#[tokio::test]
async fn no_cookies_credentials_or_referer_are_sent_across_hops() {
    let srv = spawn_server(handler(|mut s, head| async move {
        let resp = if head.starts_with("GET /two") {
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok"
        } else {
            "HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nSet-Cookie: sid=1\r\nLocation: /two\r\n\r\n"
        };
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/one", srv.port)).await;
    assert!(res.is_ok(), "{res:?}");
    let heads = srv.heads();
    assert_eq!(heads.len(), 2);
    for h in heads {
        let l = h.to_ascii_lowercase();
        for banned in [
            "cookie:",
            "authorization:",
            "proxy-authorization:",
            "referer:",
        ] {
            assert!(!l.contains(banned), "{banned} present in {h}");
        }
        assert!(l.contains("accept-encoding: gzip"), "{h}");
        assert!(l.contains("user-agent: fetch-mcp/"), "{h}");
    }
}

// ---- concurrency ---------------------------------------------------------------------------------------

#[tokio::test]
async fn ten_calls_run_three_at_a_time_and_each_gets_its_own_body() {
    let inflight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (i2, p2) = (inflight.clone(), peak.clone());
    let srv = spawn_server(handler(move |mut s, head| {
        let (inflight, peak) = (i2.clone(), p2.clone());
        async move {
            let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(100)).await;
            let n = head
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches("/n")
                .to_string();
            let body = format!("body-{n}");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = s.write_all(resp.as_bytes()).await;
            inflight.fetch_sub(1, Ordering::SeqCst);
        }
    }))
    .await;
    let c = Arc::new(client(
        loopback(),
        &public_resolver(),
        limits(15_000, 1 << 20, 3),
    ));
    let mut tasks = Vec::new();
    for i in 0..10 {
        let (c, port) = (c.clone(), srv.port);
        tasks.push(tokio::spawn(async move {
            let (res, text, _) = get(&c, &format!("http://public.test:{port}/n{i}")).await;
            (i, res.is_ok(), text)
        }));
    }
    for t in tasks {
        let (i, ok, text) = t.await.unwrap();
        assert!(ok, "call {i} failed");
        assert_eq!(text, format!("body-{i}"), "cross-contamination");
    }
    let peak = peak.load(Ordering::SeqCst);
    assert!(
        (2..=3).contains(&peak),
        "at most 3 in flight, concurrency exercised (peak {peak})"
    );
}

#[tokio::test]
async fn queueing_for_a_slot_is_bounded_by_the_timeout() {
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx")
            .await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }))
    .await;
    let c = Arc::new(client(
        loopback(),
        &public_resolver(),
        limits(600, 1 << 20, 1),
    ));
    let url = format!("http://public.test:{}/", srv.port);
    let (c2, u2) = (c.clone(), url.clone());
    let first = tokio::spawn(async move { get(&c2, &u2).await.0 });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let (res, _, _) = get(&c, &url).await;
    assert_eq!(code(&res), "timeout", "{res:?}");
    assert_eq!(code(&first.await.unwrap()), "timeout");
}

// ---- gzip (ADR-004) ------------------------------------------------------------------------------------

#[tokio::test]
async fn gzip_bodies_decode_and_encoding_rules_hold() {
    let plain = "héllo wörld ".repeat(500);
    let cases: Vec<(&'static str, Vec<u8>, &'static str)> = vec![
        ("Content-Encoding: gzip", gz(plain.as_bytes()), "ok"),
        ("Content-Encoding: GZIP", gz(plain.as_bytes()), "ok"),
        (
            "Content-Encoding: identity",
            plain.clone().into_bytes(),
            "ok",
        ),
        ("X-None: 1", plain.clone().into_bytes(), "ok"),
        (
            "Content-Encoding: gzip, gzip",
            gz(&gz(plain.as_bytes())),
            "unsupported_encoding",
        ),
        (
            "Content-Encoding: br",
            vec![1, 2, 3],
            "unsupported_encoding",
        ),
        (
            "Content-Encoding: deflate",
            vec![1, 2, 3],
            "unsupported_encoding",
        ),
        (
            "Content-Encoding: zstd",
            vec![1, 2, 3],
            "unsupported_encoding",
        ),
        (
            "Content-Encoding: nonsense",
            vec![1, 2, 3],
            "unsupported_encoding",
        ),
        (
            "Content-Encoding: gzip",
            b"definitely not gzip".to_vec(),
            "bad_response",
        ),
    ];
    for (hdr, body, want) in cases {
        let extra: &'static [&'static str] = Box::leak(vec![hdr].into_boxed_slice());
        let srv = spawn_server(fixed("200 OK", extra, body)).await;
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert_eq!(code(&res), want, "{hdr}: {res:?}");
        if want == "ok" {
            assert_eq!(text, plain, "{hdr}");
        }
    }
}

#[tokio::test]
async fn stacked_encoding_sent_as_two_headers_is_rejected() {
    let extra: &'static [&'static str] = &["Content-Encoding: gzip", "Content-Encoding: gzip"];
    let srv = spawn_server(fixed("200 OK", extra, gz(b"x"))).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "unsupported_encoding", "{res:?}");
}

#[tokio::test]
async fn multi_member_and_trailing_garbage_gzip_decode_only_the_first_member() {
    let mut multi = gz(b"first-member ");
    multi.extend_from_slice(&gz(b"second-member"));
    let mut trailing = gz(b"only-member");
    trailing.extend_from_slice(&[0u8; 64]);
    trailing.extend_from_slice(b"trailing garbage");
    for (body, want) in [(multi, "first-member "), (trailing, "only-member")] {
        let srv = spawn_server(fixed("200 OK", &["Content-Encoding: gzip"], body)).await;
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(text, want);
    }
}

#[tokio::test]
async fn gzip_bomb_is_capped_with_bounded_output_steps() {
    let bomb = gz(&vec![b'a'; 32 * 1024 * 1024]);
    assert!(bomb.len() < 64 * 1024, "fixture must be small on the wire");
    let srv = spawn_server(fixed("200 OK", &["Content-Encoding: gzip"], bomb)).await;
    let c = client(
        loopback(),
        &public_resolver(),
        limits(10_000, 1024 * 1024, 3),
    );
    let (res, text, biggest) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "too_large", "{res:?}");
    assert!(text.len() <= 1024 * 1024);
    assert!(
        biggest <= 64 * 1024,
        "decompressed step {biggest} exceeds 64 KiB"
    );
}

// ---- header limits -------------------------------------------------------------------------------------

#[tokio::test]
async fn header_bombs_fail_cleanly() {
    let many: String = (0..(MAX_HEADER_COUNT * 3))
        .map(|i| format!("X-H{i}: v\r\n"))
        .collect();
    let big_bytes: String = (0..3)
        .map(|i| format!("X-Big{i}: {}\r\n", "a".repeat(MAX_HEADER_BYTES)))
        .collect();
    let one_huge = format!("X-Huge: {}\r\n", "b".repeat(2 * 1024 * 1024));
    for (name, headers) in [
        ("count", many),
        ("bytes", big_bytes),
        ("one huge header", one_huge),
    ] {
        let srv = spawn_server(handler(move |mut s, _| {
            let headers = headers.clone();
            async move {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n{headers}\r\nok"
                );
                let _ = s.write_all(head.as_bytes()).await;
                let _ = s.shutdown().await;
            }
        }))
        .await;
        let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
        let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
        assert_eq!(code(&res), "bad_response", "{name}: {res:?}");
        assert!(text.is_empty());
    }
}

// ---- transport errors never leak addresses -----------------------------------------------------------------

#[tokio::test]
async fn connection_refused_is_a_category_without_the_address() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    }; // listener dropped: nothing listens here
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{port}/")).await;
    let Err(e) = res else {
        panic!("expected an error")
    };
    assert_eq!(e.code(), "network_error");
    let t = e.tool_text();
    assert!(
        !t.contains("127.0.0.1") && !t.contains(&port.to_string()),
        "{t}"
    );
}

#[tokio::test]
async fn url_text_containing_header_never_changes_the_transport_class() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    }; // nothing listens here
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    // (a) the request URL itself mentions "header" (path, query, host label)
    for url in [
        format!("http://public.test:{port}/api/headers"),
        format!("http://public.test:{port}/?header=1&too-large=too%20many%20headers"),
    ] {
        let (res, _, _) = get(&c, &url).await;
        assert_eq!(code(&res), "network_error", "{url}: {res:?}");
    }
    // (b) an upstream-controlled redirect Location mentions "header"; the next hop fails to connect
    let srv = spawn_server(handler(move |mut s, _| async move {
        let resp = format!(
            "HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: http://public.test:{port}/api/headers?header=too-large\r\n\r\n"
        );
        let _ = s.write_all(resp.as_bytes()).await;
    }))
    .await;
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "network_error", "{res:?}");
}

// ---- backstops and truncation through the client (QA N4, N5) -------------------------------------------------

#[test]
fn cross_check_refuses_every_disagreement_with_the_core() {
    use super::cross_check;
    let core = |u: &str| check_url(u, &Policy::default(), Origin::Initial).unwrap();
    let v = core("http://public.test:8080/x");
    assert!(cross_check("http://public.test:8080/x", &v).is_ok());
    assert!(
        cross_check("http://PUBLIC.test.:8080/x", &v).is_ok(),
        "case and root dot agree"
    );
    for (raw, why) in [
        ("http://other.test:8080/x", "host"),
        ("http://public.test:9090/x", "port"),
        ("https://public.test:8080/x", "scheme"),
        ("http://user@public.test:8080/x", "userinfo"),
        ("not a url", "unparsable"),
    ] {
        let r = cross_check(raw, &v);
        assert!(
            matches!(r, Err(FetchError::BlockedTarget(_))),
            "{why}: {r:?}"
        );
    }
    let ip = core("http://8.8.8.8/");
    assert!(cross_check("http://8.8.8.8/", &ip).is_ok());
    assert!(
        cross_check("http://8.8.4.4/", &ip).is_err(),
        "different literal"
    );
}

/// Send `body` as one chunked message so no Content-Length pre-check can pre-empt the client's own counting.
fn chunked(extra: &'static str, body: Vec<u8>) -> Handler {
    let body = Arc::new(body);
    handler(move |mut s, _| {
        let body = body.clone();
        async move {
            let head = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n{extra}\r\n"
            );
            let _ = s.write_all(head.as_bytes()).await;
            for piece in body.chunks(1000) {
                let _ = s
                    .write_all(format!("{:x}\r\n", piece.len()).as_bytes())
                    .await;
                let _ = s.write_all(piece).await;
                let _ = s.write_all(b"\r\n").await;
            }
            let _ = s.write_all(b"0\r\n\r\n").await;
            let _ = s.shutdown().await;
        }
    })
}

#[tokio::test]
async fn the_wire_byte_cap_applies_to_a_gzip_body_that_decodes_under_the_cap() {
    // Incompressible bytes: the gzip stream is longer than its content, so content < cap < wire.
    let mut x = 0x2545_f491_u32;
    let data: Vec<u8> = (0..4096)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect();
    let wire = gz(&data);
    assert!(wire.len() > data.len());
    let cap = data.len() as u64;
    let srv = spawn_server(chunked("Content-Encoding: gzip\r\n", wire)).await;
    let c = client(loopback(), &public_resolver(), limits(5000, cap, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "too_large", "{res:?}");
}

#[tokio::test]
async fn a_truncated_gzip_body_is_a_bad_response() {
    let full = gz("some text to compress ".repeat(200).as_bytes());
    let cut = full[..full.len() / 2].to_vec();
    let srv = spawn_server(chunked("Content-Encoding: gzip\r\n", cut)).await; // well-formed chunking, short gzip
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "bad_response", "{res:?}");
}

#[tokio::test]
async fn an_identity_body_shorter_than_its_content_length_is_a_network_error() {
    let srv = spawn_server(handler(|mut s, _| async move {
        let _ = s
            .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 100\r\n\r\nshort")
            .await;
        let _ = s.shutdown().await;
    }))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "network_error", "{res:?}");
}

/// Decision (QA N5): a `Content-Encoding: gzip` response with a zero-byte body is an empty page, not an error.
/// Empty 200 responses that still carry the encoding header are common, and nothing was truncated.
#[tokio::test]
async fn an_empty_body_marked_gzip_is_an_empty_page() {
    let srv = spawn_server(fixed("200 OK", &["Content-Encoding: gzip"], Vec::new())).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    assert!(res.is_ok(), "{res:?}");
    assert!(text.is_empty());
}

#[tokio::test]
async fn unresolvable_name_is_dns_failure() {
    let c = client(
        Policy::default(),
        &Arc::new(FakeResolver::new().fail("gone.test")),
        limits(5000, 1 << 20, 3),
    );
    let (res, _, _) = get(&c, "http://gone.test/").await;
    assert_eq!(code(&res), "dns_failure", "{res:?}");
}

// ---- the merge gate (EPICS A-3b; required CI check) --------------------------------------------------------

/// Compile-time proof that a type has no `Default`: with one, `probe()` is ambiguous and this test would not build.
trait AmbiguousIfDefault<A> {
    fn probe() {}
}
impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
impl<T: Default> AmbiguousIfDefault<u8> for T {}

/// Source files of the crate, without this test module.
fn non_test_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs")
                && p.file_name().is_some_and(|n| n != "tests.rs")
            {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    out.push((p.to_string_lossy().replace('\\', "/"), text));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    out
}

#[tokio::test]
async fn a3b_merge_gate() {
    // (1) The client constructor takes a Policy and the client has no Default: no unguarded construction.
    <FetchClient<FakeResolver> as AmbiguousIfDefault<_>>::probe();
    let _: fn(Policy, Arc<FakeResolver>, Limits) -> FetchClient<FakeResolver> = FetchClient::new;

    // (2) Dial route: the per-hop client's only resolver is Pinned. It dials the validated host and refuses every
    // other name, with no fallback to a system resolver (the second name never reaches a socket).
    let srv = spawn_server(fixed("200 OK", &[], b"pinned-ok".to_vec())).await;
    let r = Arc::new(FakeResolver::new());
    let c = client(loopback(), &r, limits(5000, 1 << 20, 3));
    let checked = check_url(
        &format!("http://a.test:{}/", srv.port),
        &loopback(),
        Origin::Initial,
    )
    .unwrap();
    let addrs = vec![std::net::SocketAddr::from(([127, 0, 0, 1], srv.port))];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let hop = c.hop_client(&checked, addrs, deadline).unwrap();
    let ok = hop
        .get(format!("http://a.test:{}/", srv.port))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.text().await.unwrap(), "pinned-ok");
    let before = srv.accepted();
    for other in ["b.test", "localhost", "a.test.evil.test"] {
        let e = hop
            .get(format!("http://{other}:{}/", srv.port))
            .send()
            .await;
        assert!(
            e.is_err(),
            "{other} must not be dialled by a client pinned to a.test"
        );
    }
    assert_eq!(
        srv.accepted(),
        before,
        "no connection for a name outside the validated set"
    );
    assert_eq!(
        r.count(),
        0,
        "the client never asked our resolver either (the core resolves, once, before the client)"
    );

    // (3) A blocked answer never reaches the dial route: refused with zero connections.
    let private = Arc::new(FakeResolver::new().on("lan.test", &["10.9.9.9"]));
    let c = client(loopback(), &private, limits(5000, 1 << 20, 3));
    let (res, _, _) = get(&c, &format!("http://lan.test:{}/", srv.port)).await;
    assert_eq!(code(&res), "blocked_target");
    assert_eq!(srv.accepted(), before);

    // (4) A build with any other dial route fails: name resolution and sockets exist in exactly one place.
    let mut dns_resolver_calls = 0;
    for (path, text) in non_test_sources() {
        for banned in [
            "TcpStream",
            "TcpSocket",
            "UdpSocket",
            "ToSocketAddrs",
            "std::net::TcpListener",
        ] {
            assert!(
                !text.contains(banned),
                "{path} uses {banned}: a second dial route"
            );
        }
        if text.contains("lookup_host") {
            assert!(
                path.ends_with("src/fetch/dns.rs"),
                "lookup_host outside fetch/dns.rs: {path}"
            );
        }
        for banned in [
            "resolve_to_addrs",
            ".resolve(\"",
            "connector",
            "Connector",
            "hyper::",
            "http::uri",
        ] {
            assert!(
                !text.contains(banned) || path.ends_with("src/ssrf/resolver.rs"),
                "{path} mentions {banned}"
            );
        }
        dns_resolver_calls += text.matches(".dns_resolver(").count();
        if text.contains(".dns_resolver(") {
            assert!(
                path.ends_with("src/fetch/mod.rs") && text.contains("dns::Pinned::new("),
                "{path}"
            );
        }
    }
    assert_eq!(
        dns_resolver_calls, 1,
        "exactly one client is built, and its resolver is Pinned"
    );
}

#[test]
fn pinned_is_send_and_sync_for_the_client_builder() {
    fn ok<T: Send + Sync + 'static>() {}
    ok::<Pinned>();
}

// ---- A-4: HTML to markdown through the real client ---------------------------------------------------------

async fn get_as(
    c: &FetchClient<FakeResolver>,
    url: &str,
    mode: crate::convert::Mode,
) -> (Result<super::Fetched, FetchError>, String) {
    let mut text = String::new();
    let r = tokio::time::timeout(
        Duration::from_secs(60),
        c.fetch_as(url, mode, &mut |s: &str| text.push_str(s)),
    )
    .await
    .expect("fetch hung");
    (r, text)
}

const PAGE: &str = "<html><head><title>T</title><script>evil()</script></head><body><nav>menu</nav>\
    <h1>Hello</h1><p>See <a href=\"/next\">next</a> &amp; more.</p><ul><li>a<li>b</ul></body></html>";
const PAGE_MD: &str = "# Hello\n\nSee [next](http://public.test:PORT/next) & more.\n\n- a\n- b";

#[tokio::test]
async fn html_is_converted_and_links_resolve_against_the_final_url() {
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html; charset=utf-8"],
        PAGE.as_bytes().to_vec(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    res.unwrap();
    assert_eq!(text, PAGE_MD.replace("PORT", &srv.port.to_string()));
    // raw mode and non-HTML types come back untouched
    let (_, raw) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Raw,
    )
    .await;
    assert_eq!(raw, PAGE);
    let srv2 = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain"],
        PAGE.as_bytes().to_vec(),
    ))
    .await;
    let (_, plain) = get_as(
        &c,
        &format!("http://public.test:{}/", srv2.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    assert_eq!(plain, PAGE);
}

#[tokio::test]
async fn gzip_html_is_converted_too() {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(PAGE.as_bytes()).unwrap();
    let gz = e.finish().unwrap();
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html", "Content-Encoding: gzip"],
        gz,
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    res.unwrap();
    assert_eq!(text, PAGE_MD.replace("PORT", &srv.port.to_string()));
}

#[tokio::test]
async fn untyped_html_is_sniffed_and_a_converter_limit_is_a_clear_error() {
    let srv = spawn_server(fixed("200 OK", &[], PAGE.as_bytes().to_vec())).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    res.unwrap();
    assert!(text.starts_with("# Hello"));
    // unclosed elements nest without end in the tokenizer: past its memory limit the call fails cleanly
    let hostile = "<div>x".repeat(60_000).into_bytes();
    let srv = spawn_server(fixed("200 OK", &["Content-Type: text/html"], hostile)).await;
    let (res, _) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    let e = res.unwrap_err();
    assert_eq!(e.code(), "converter_limit");
    assert!(e.tool_text().contains("raw=true"));
}

/// A converter failure ends the read at the next chunk (mutant M5, QA review): the user-visible error would be
/// the same if the reader carried on to the end (`conv.finish` reports it again), so the test looks at the
/// server: with the abort it has sent a few MiB at most of a 32 MiB body, without it all of it.
#[tokio::test]
async fn converter_failure_stops_reading_the_body_early() {
    let sent = Arc::new(AtomicUsize::new(0));
    let total: usize = 32 << 20;
    let s2 = sent.clone();
    let h = handler(move |mut s, _| {
        let sent = s2.clone();
        async move {
            let head = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/html\r\nContent-Length: {total}\r\n\r\n"
            );
            if s.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            // more attributes than the converter allows in one tag, then plain text for the rest
            let bomb = format!("<div {}>", "a ".repeat(4000)).into_bytes();
            let mut written = bomb.len();
            if s.write_all(&bomb).await.is_err() {
                return;
            }
            sent.fetch_add(bomb.len(), Ordering::SeqCst);
            let filler = vec![b'x'; 16 * 1024];
            while written < total {
                let n = filler.len().min(total - written);
                match s.write_all(&filler[..n]).await {
                    Ok(()) => {
                        written += n;
                        sent.fetch_add(n, Ordering::SeqCst);
                    }
                    Err(_) => return,
                }
            }
            let _ = s.shutdown().await;
        }
    });
    let srv = spawn_server(h).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 64 << 20, 3));
    let (res, _) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    assert_eq!(res.unwrap_err().code(), "converter_limit");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let sent = sent.load(Ordering::SeqCst);
    assert!(
        sent < total / 2,
        "the reader carried on after the converter failed: {sent} of {total} bytes were sent"
    );
}

// ---- A-5 / A-6: window, early stop, content types -------------------------------------------------------------

use crate::convert::window::{Window, WindowOutput};

async fn windowed(
    c: &FetchClient<FakeResolver>,
    url: &str,
    mode: crate::convert::Mode,
    start: u64,
    len: u64,
) -> (Result<super::Fetched, FetchError>, WindowOutput) {
    let mut w = Window::new(start, len);
    let stop = w.stop_flag();
    let r = tokio::time::timeout(
        Duration::from_secs(60),
        c.fetch_until(url, mode, &mut |s: &str| w.push(s), &move || {
            stop.load(Ordering::Relaxed)
        }),
    )
    .await
    .expect("fetch hung");
    (r, w.finish())
}

/// A chunked (no `Content-Length`) body of `total` bytes of `fill`, 16 KiB per chunk; counts what it managed to send.
fn chunked_stream(
    content_type: &'static str,
    total: usize,
    fill: u8,
    sent: Arc<AtomicUsize>,
) -> Handler {
    handler(move |mut s, _| {
        let sent = sent.clone();
        async move {
            let head = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\n\r\n"
            );
            if s.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            let block = vec![fill; 16 * 1024];
            let mut written = 0;
            while written < total {
                let n = block.len().min(total - written);
                let mut frame = format!("{n:x}\r\n").into_bytes();
                frame.extend_from_slice(&block[..n]);
                frame.extend_from_slice(b"\r\n");
                if s.write_all(&frame).await.is_err() {
                    return;
                }
                written += n;
                sent.fetch_add(n, Ordering::SeqCst);
            }
            let _ = s.write_all(b"0\r\n\r\n").await;
            let _ = s.shutdown().await;
        }
    })
}

/// Early stop (ADR-004, architecture 5.2): once the window is full and one more character was seen, the read stops
/// and the connection is dropped. Judged from the server: of a 32 MiB body only a small part is ever sent.
#[tokio::test]
async fn early_stop_ends_the_read_once_the_window_is_confirmed() {
    let sent = Arc::new(AtomicUsize::new(0));
    let total: usize = 32 << 20;
    let srv = spawn_server(chunked_stream("text/plain", total, b'x', sent.clone())).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 64 << 20, 3));
    let url = format!("http://public.test:{}/", srv.port);
    let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, 0, 100).await;
    let fetched = res.unwrap();
    assert!(w.more && w.total.is_none());
    assert_eq!(w.text, "x".repeat(100));
    assert!(
        fetched.wire_bytes < 1 << 20,
        "read {} bytes",
        fetched.wire_bytes
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let sent = sent.load(Ordering::SeqCst);
    assert!(
        sent < total / 4,
        "the reader carried on after the window: {sent} of {total} bytes were sent"
    );
}

/// The E-4 rule (architecture 6.4): a chunked over-cap body succeeds when the window completes inside the cap and
/// is `too_large` when the cap is reached first; a declared length over the cap is `too_large` whatever the window.
#[tokio::test]
async fn chunked_window_inside_the_cap_succeeds_and_beyond_the_cap_is_too_large() {
    let sent = Arc::new(AtomicUsize::new(0));
    let srv = spawn_server(chunked_stream("text/plain", 8 << 20, b'y', sent)).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 1 << 20, 3));
    let url = format!("http://public.test:{}/", srv.port);
    let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, 500_000, 1000).await;
    res.unwrap();
    assert_eq!((w.text.len(), w.more), (1000, true));
    let (res, _) = windowed(&c, &url, crate::convert::Mode::Markdown, 2 << 20, 1000).await;
    assert_eq!(code(&res), "too_large");
    // reading to the end of an over-cap body (the skipped part reaches the cap) is too_large as well
    let (res, _) = windowed(&c, &url, crate::convert::Mode::Markdown, u64::MAX, u64::MAX).await;
    assert_eq!(code(&res), "too_large");
    let cl = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain"],
        vec![b'z'; 2 << 20],
    ))
    .await;
    let (res, _) = windowed(
        &c,
        &format!("http://public.test:{}/", cl.port),
        crate::convert::Mode::Markdown,
        0,
        10,
    )
    .await;
    assert_eq!(code(&res), "too_large");
}

/// FR-04 through the real client: four sequential windows of a 20,000-character page (multi-byte characters
/// included) reproduce it with no gap or overlap; the last one knows the total; a window past the end is empty.
#[tokio::test]
async fn four_sequential_windows_reproduce_the_page() {
    let page: String = (0..20_000)
        .map(|i| match i % 7 {
            0 => '\u{e9}',
            3 => '\u{20ac}',
            5 => '\u{1F680}',
            _ => char::from(b'a' + u8::try_from(i % 26).unwrap()),
        })
        .collect();
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain; charset=utf-8"],
        page.clone().into_bytes(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let url = format!("http://public.test:{}/", srv.port);
    let (mut got, mut start, mut calls) = (String::new(), 0u64, 0);
    loop {
        let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, start, 5000).await;
        res.unwrap();
        calls += 1;
        got.push_str(&w.text);
        if !w.more {
            assert_eq!(w.total, Some(20_000));
            break;
        }
        start += w.returned;
    }
    assert_eq!((calls, got.as_str()), (4, page.as_str()));
    let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, 20_000, 5000).await;
    res.unwrap();
    assert_eq!((w.text.as_str(), w.total), ("", Some(20_000)));
}

#[tokio::test]
async fn binary_content_types_are_refused_naming_the_type_and_text_types_are_returned() {
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    for (ct, body) in [
        ("image/png", &b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"[..]),
        ("application/pdf", b"%PDF-1.7\n%\xe2\xe3\xcf\xd3"),
        ("application/octet-stream", b"\0\x01\x02"),
    ] {
        let hdr: &'static str = Box::leak(format!("Content-Type: {ct}").into_boxed_str());
        let srv = spawn_server(fixed(
            "200 OK",
            std::slice::from_ref(Box::leak(Box::new(hdr))),
            body.to_vec(),
        ))
        .await;
        let url = format!("http://public.test:{}/", srv.port);
        for mode in [crate::convert::Mode::Markdown, crate::convert::Mode::Raw] {
            let (res, text) = get_as(&c, &url, mode).await;
            let e = res.unwrap_err();
            assert_eq!(e.code(), "unsupported_content_type", "{ct}");
            assert!(e.tool_text().contains(ct), "{}", e.tool_text());
            assert_eq!(text, "", "no body text may reach the sink");
        }
    }
    for (ct, body) in [
        ("application/json", "{\"a\":1}"),
        ("application/xml", "<a>1</a>"),
        ("text/csv; charset=utf-8", "a,b\n1,2"),
        ("application/ld+json", "{}"),
    ] {
        let hdr: &'static str = Box::leak(format!("Content-Type: {ct}").into_boxed_str());
        let srv = spawn_server(fixed(
            "200 OK",
            std::slice::from_ref(Box::leak(Box::new(hdr))),
            body.as_bytes().to_vec(),
        ))
        .await;
        let (res, text) = get_as(
            &c,
            &format!("http://public.test:{}/", srv.port),
            crate::convert::Mode::Markdown,
        )
        .await;
        res.unwrap();
        assert_eq!(text, body, "{ct}");
    }
}

/// An over-cap declared length of a binary type is reported as the type (the cheaper, more useful answer), and a
/// refused type never reads the body.
#[tokio::test]
async fn a_refused_type_reads_no_body() {
    let sent = Arc::new(AtomicUsize::new(0));
    let srv = spawn_server(chunked_stream("image/png", 32 << 20, 0, sent.clone())).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 64 << 20, 3));
    let (res, _) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    assert_eq!(code(&res), "unsupported_content_type");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(sent.load(Ordering::SeqCst) < 4 << 20);
}

/// Hostile bodies through the whole path (panic = abort in the product, so none of this may panic): an untyped
/// binary body, invalid UTF-8 declared as text, NUL bytes and a lone BOM.
#[tokio::test]
async fn hostile_bodies_never_panic_and_come_back_as_replacement_text() {
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let mut noise = Vec::new();
    let mut x = 0x1234_5678_u32;
    for _ in 0..200_000 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        noise.push(u8::try_from(x >> 24).unwrap());
    }
    for (hdrs, body) in [
        (&[][..], noise.clone()),
        (&["Content-Type: text/plain"][..], noise.clone()),
        (&["Content-Type: text/html"][..], noise.clone()),
        (&[][..], b"\xef\xbb\xbf".to_vec()),
        (&[][..], vec![0u8; 100_000]),
        (&["Content-Type: text/plain"][..], Vec::new()),
    ] {
        let srv = spawn_server(fixed("200 OK", hdrs, body)).await;
        let url = format!("http://public.test:{}/", srv.port);
        for (start, len) in [(0, 10), (150_000, 100), (u64::MAX, 1), (7, u64::MAX)] {
            let (res, _) = windowed(&c, &url, crate::convert::Mode::Markdown, start, len).await;
            assert!(
                matches!(code(&res), "ok" | "converter_limit"),
                "{}",
                code(&res)
            );
        }
    }
}

// ---- A-8: charset decoding -------------------------------------------------------------------------------

/// "café" in ISO-8859-1 / windows-1252 (`é` = 0xE9), the byte encoding a naive UTF-8 decode would mangle.
const CAFE_LATIN1: &[u8] = b"caf\xe9";

#[tokio::test]
async fn iso_8859_1_header_charset_decodes_correctly() {
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain; charset=iso-8859-1"],
        CAFE_LATIN1.to_vec(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "café");
}

#[tokio::test]
async fn iso_8859_1_meta_charset_with_no_header_charset_decodes_correctly() {
    let mut body =
        b"<html><head><meta charset=\"ISO-8859-1\"></head><body>caf\xe9</body></html>".to_vec();
    // pad well past a trivial case and keep the meta tag inside the sniff window
    body.extend_from_slice(&[b' '; 10]);
    let srv = spawn_server(fixed("200 OK", &["Content-Type: text/html"], body)).await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert!(text.contains("café"), "{text:?}");
}

#[tokio::test]
async fn iso_8859_1_meta_charset_survives_gzip() {
    let page =
        b"<html><head><meta charset=\"ISO-8859-1\"></head><body>caf\xe9</body></html>".to_vec();
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html", "Content-Encoding: gzip"],
        gz(&page),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert!(text.contains("café"), "{text:?}");
}

#[tokio::test]
async fn no_charset_anywhere_defaults_to_utf8_with_replacement() {
    // No Content-Type charset and no <meta charset>: invalid UTF-8 is replaced, not fatal (existing behaviour).
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html"],
        b"<html><body>bad: \xff end</body></html>".to_vec(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert!(text.contains("bad: \u{FFFD} end"), "{text:?}");

    // Same for a plain-text response with no Content-Type at all.
    let srv2 = spawn_server(fixed("200 OK", &[], b"na\xffve".to_vec())).await;
    let (res2, text2, _) = get(&c, &format!("http://public.test:{}/", srv2.port)).await;
    res2.unwrap();
    assert_eq!(text2, "na\u{FFFD}ve");
}

#[tokio::test]
async fn unrecognized_header_charset_falls_back_to_utf8_instead_of_failing_the_fetch() {
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/plain; charset=totally-bogus-charset"],
        "hello".as_bytes().to_vec(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "hello");
}

/// A very large HTML page with no charset markers stays streamed and early-abort still works: the A-8 sniff
/// lookahead only ever delays the first few KiB, never the whole (potentially large) body.
#[tokio::test]
async fn plain_utf8_html_with_no_charset_markers_still_streams_and_aborts_early_on_converter_failure(
) {
    let sent = Arc::new(AtomicUsize::new(0));
    let total: usize = 32 << 20;
    let s2 = sent.clone();
    let h = handler(move |mut s, _| {
        let sent = s2.clone();
        async move {
            let head = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/html\r\nContent-Length: {total}\r\n\r\n"
            );
            if s.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            let bomb = format!("<div {}>", "a ".repeat(4000)).into_bytes();
            let mut written = bomb.len();
            if s.write_all(&bomb).await.is_err() {
                return;
            }
            sent.fetch_add(bomb.len(), Ordering::SeqCst);
            let filler = vec![b'x'; 16 * 1024];
            while written < total {
                let n = filler.len().min(total - written);
                match s.write_all(&filler[..n]).await {
                    Ok(()) => {
                        written += n;
                        sent.fetch_add(n, Ordering::SeqCst);
                    }
                    Err(_) => return,
                }
            }
            let _ = s.shutdown().await;
        }
    });
    let srv = spawn_server(h).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 64 << 20, 3));
    let (res, _) = get_as(
        &c,
        &format!("http://public.test:{}/", srv.port),
        crate::convert::Mode::Markdown,
    )
    .await;
    assert_eq!(res.unwrap_err().code(), "converter_limit");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let sent = sent.load(Ordering::SeqCst);
    assert!(
        sent < total / 2,
        "the A-8 sniff lookahead defeated early-abort streaming: {sent} of {total} bytes were sent"
    );
}

/// Finding #1 (round-2 review): `max_length` must still stop a header-charset (non-gzip, whole-body-decode) fetch
/// early -- before this fix `decode_whole_body` never checked `stop()` inside its read loop, so the early-stop
/// window was silently defeated on this path and the full body was always read regardless of `max_length`.
#[tokio::test]
async fn header_charset_decode_stops_early_when_the_window_is_satisfied() {
    let sent = Arc::new(AtomicUsize::new(0));
    let total: usize = 32 << 20;
    let s2 = sent.clone();
    let h = handler(move |mut s, _| {
        let sent = s2.clone();
        async move {
            let head = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Length: {total}\r\n\r\n"
            );
            if s.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            let block = vec![b'x'; 16 * 1024];
            let mut written = 0;
            while written < total {
                let n = block.len().min(total - written);
                match s.write_all(&block[..n]).await {
                    Ok(()) => {
                        written += n;
                        sent.fetch_add(n, Ordering::SeqCst);
                    }
                    Err(_) => return,
                }
            }
            let _ = s.shutdown().await;
        }
    });
    let srv = spawn_server(h).await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 64 << 20, 3));
    let url = format!("http://public.test:{}/", srv.port);
    let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, 0, 100).await;
    let fetched = res.unwrap();
    assert!(w.more);
    assert!(
        fetched.wire_bytes < (1 << 20),
        "read {} of {total} bytes; max_length did not stop the header-charset decode early",
        fetched.wire_bytes
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        sent.load(Ordering::SeqCst) < total / 4,
        "the server kept sending after the window should have stopped the read"
    );
}

/// Finding #2 (round-2 review): once #1 is fixed, an early stop mid-gzip-stream must not call
/// `RawPipeline::finish()` on the truncated stream (which errors as `BodyError::Corrupt`/`bad_response`) --
/// it must return `Ok` with the wire bytes read so far, like every other early-stop site.
#[tokio::test]
async fn gzip_html_early_stop_mid_stream_succeeds_instead_of_bad_response() {
    let mut page = String::from("<html><body>");
    page.push_str(&"hello world ".repeat(200_000)); // several chunks once gzipped and chunk-fed
    page.push_str("</body></html>");
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html", "Content-Encoding: gzip"],
        gz(page.as_bytes()),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(60_000, 16 << 20, 3));
    let url = format!("http://public.test:{}/", srv.port);
    let (res, w) = windowed(&c, &url, crate::convert::Mode::Markdown, 0, 50).await;
    res.unwrap_or_else(|e| panic!("expected success on early stop, got {}: {e}", e.code()));
    assert!(w.more);
}

/// Finding #11 (round-2 review): a header charset must win over a conflicting `<meta charset>` in the body --
/// previously only "no header charset" cases were tested against the meta tag.
#[tokio::test]
async fn header_charset_wins_over_conflicting_meta_charset() {
    // Header says UTF-8 (so the fast streaming path is used); body's <meta> falsely claims ISO-8859-1 but the
    // body bytes are themselves valid UTF-8 -- if meta ever won here, decoding as Latin-1 would mangle "café".
    let body = "<html><head><meta charset=\"ISO-8859-1\"></head><body>café</body></html>";
    let srv = spawn_server(fixed(
        "200 OK",
        &["Content-Type: text/html; charset=utf-8"],
        body.as_bytes().to_vec(),
    ))
    .await;
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/", srv.port)).await;
    res.unwrap();
    assert!(text.contains("café"), "{text:?}");
}

// ---- B-4: robots.txt enforcement mechanism (inert unless RobotsMode::Enforce; OQ-3 still open) ------------

/// Serves `robots_status`/`robots_body` for `GET /robots.txt`, `200 OK`/`target_body` for anything else.
fn robots_server(
    robots_status: &'static str,
    robots_body: Vec<u8>,
    target_body: Vec<u8>,
) -> Handler {
    let robots_body = Arc::new(robots_body);
    let target_body = Arc::new(target_body);
    handler(move |mut s, head| {
        let (robots_body, target_body) = (robots_body.clone(), target_body.clone());
        async move {
            let is_robots = head.starts_with("GET /robots.txt ");
            let (status, body): (&str, &[u8]) = if is_robots {
                (robots_status, &robots_body)
            } else {
                ("200 OK", &target_body)
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            if s.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            let _ = s.write_all(body).await;
            let _ = s.shutdown().await;
        }
    })
}

fn enforcing(r: &Arc<FakeResolver>, l: Limits) -> FetchClient<FakeResolver> {
    FetchClient::new(loopback(), r.clone(), l).with_robots_mode(crate::config::RobotsMode::Enforce)
}

#[tokio::test]
async fn disallowed_path_is_refused_with_an_explanatory_error_when_enforced() {
    let srv = spawn_server(robots_server(
        "200 OK",
        b"User-agent: *\nDisallow: /private\n".to_vec(),
        b"secret".to_vec(),
    ))
    .await;
    let c = enforcing(&public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/private", srv.port)).await;
    let e = res.unwrap_err();
    assert_eq!(e.code(), "robots_disallowed");
    assert!(e.to_string().contains("/private"), "{e}");
    assert!(text.is_empty());
}

#[tokio::test]
async fn a_path_not_covered_by_disallow_proceeds_when_enforced() {
    let srv = spawn_server(robots_server(
        "200 OK",
        b"User-agent: *\nDisallow: /private\n".to_vec(),
        b"hello".to_vec(),
    ))
    .await;
    let c = enforcing(&public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/public", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "hello");
}

#[tokio::test]
async fn missing_or_404_robots_txt_lets_the_fetch_proceed() {
    let srv = spawn_server(robots_server(
        "404 Not Found",
        b"nope".to_vec(),
        b"hello".to_vec(),
    ))
    .await;
    let c = enforcing(&public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/private", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "hello");
}

#[tokio::test]
async fn ignore_mode_the_default_never_enforces_robots_txt() {
    let srv = spawn_server(robots_server(
        "200 OK",
        b"User-agent: *\nDisallow: /private\n".to_vec(),
        b"secret".to_vec(),
    ))
    .await;
    // No `with_robots_mode` call: RobotsMode::Ignore, the default, is a no-op (unchanged from before B-4).
    let c = client(loopback(), &public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/private", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "secret");
}

#[tokio::test]
async fn robots_txt_fetch_is_size_capped_at_512kb() {
    // The only Disallow rule sits well past the 512 KB cap: truncation means it is never read or applied.
    let mut body = vec![b'#'; 600 * 1024];
    body.push(b'\n');
    body.extend_from_slice(b"User-agent: *\nDisallow: /private\n");
    let srv = spawn_server(robots_server("200 OK", body, b"hello".to_vec())).await;
    let c = enforcing(&public_resolver(), limits(20_000, 8 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/private", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "hello");
}

#[tokio::test]
async fn robots_txt_fetch_goes_through_the_same_ssrf_checks_and_a_blocked_redirect_still_lets_the_fetch_proceed(
) {
    // robots.txt redirects to a private address; the redirect is refused by the same SSRF core as any other
    // fetch, so the robots fetch fails -- which (per the AC) means "no restrictions", not a blocked target fetch.
    let h = handler(move |mut s, head| async move {
        let is_robots = head.starts_with("GET /robots.txt ");
        if is_robots {
            let resp = "HTTP/1.1 302 Found\r\nConnection: close\r\nLocation: http://10.0.0.1/robots.txt\r\nContent-Length: 0\r\n\r\n";
            let _ = s.write_all(resp.as_bytes()).await;
        } else {
            let body = b"hello";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            if s.write_all(resp.as_bytes()).await.is_err() {
                return;
            }
            let _ = s.write_all(body).await;
        }
        let _ = s.shutdown().await;
    });
    let srv = spawn_server(h).await;
    let c = enforcing(&public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/x", srv.port)).await;
    res.unwrap();
    assert_eq!(text, "hello");
}

#[tokio::test]
async fn robots_check_does_not_run_on_a_redirect_hop() {
    // The original target's robots.txt allows everything; the redirect target's own (different host, but same
    // fixture server here) robots.txt would disallow /private -- but B-4 only checks the ORIGINAL target's
    // robots.txt, so the redirect is followed and the fetch succeeds.
    let target = spawn_server(robots_server(
        "200 OK",
        b"User-agent: *\nDisallow: /private\n".to_vec(),
        b"redirected-body".to_vec(),
    ))
    .await;
    let h = handler(move |mut s, head| {
        let target_port = target.port;
        async move {
            let is_robots = head.starts_with("GET /robots.txt ");
            if is_robots {
                let body = b"User-agent: *\n"; // allow everything at the origin
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes()).await;
                let _ = s.write_all(body).await;
            } else {
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nConnection: close\r\nLocation: http://public.test:{target_port}/private\r\nContent-Length: 0\r\n\r\n"
                );
                let _ = s.write_all(resp.as_bytes()).await;
            }
            let _ = s.shutdown().await;
        }
    });
    let origin = spawn_server(h).await;
    let c = enforcing(&public_resolver(), limits(5000, 1 << 20, 3));
    let (res, text, _) = get(&c, &format!("http://public.test:{}/x", origin.port)).await;
    res.unwrap();
    assert_eq!(text, "redirected-body");
}
