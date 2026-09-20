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

async fn get(
    c: &FetchClient<FakeResolver>,
    url: &str,
) -> (Result<super::Fetched, FetchError>, String, usize) {
    let mut text = String::new();
    let mut biggest = 0usize;
    let r = c
        .fetch(url, &mut |s: &str| {
            biggest = biggest.max(s.len());
            text.push_str(s);
        })
        .await;
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
    assert_eq!(r.count(), 2, "one lookup per hop, none more");
    assert_eq!(srv.accepted(), 2, "no connection reuse");
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

// ---- dial-once and dial-only-validated ---------------------------------------------------------------

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
