//! Stdout-purity and protocol integration tests (A-2): spawn the real binary and speak JSON-RPC over stdio.
//! Every stdout line must be a JSON-RPC frame; nothing else may appear (FR-13).

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    raw: Vec<String>,
    next_id: u64,
    spawned: Instant,
}

impl Session {
    fn start() -> Self {
        let spawned = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_fetch-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn fetch-mcp");
        let out = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            lines,
            raw: vec![],
            next_id: 0,
            spawned,
        }
    }

    fn send(&mut self, v: &Value) {
        let s = self.stdin.as_mut().expect("stdin open");
        writeln!(s, "{v}").unwrap();
        s.flush().unwrap();
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let l = self
                .lines
                .recv_timeout(Duration::from_secs(20))
                .expect("reply within 20s");
            self.raw.push(l.clone());
            let v: Value = serde_json::from_str(&l)
                .unwrap_or_else(|e| panic!("non-JSON on stdout: {l:?} ({e})"));
            if v.get("id") == Some(&json!(id)) {
                return v;
            }
        }
    }

    fn handshake(&mut self) -> f64 {
        let r = self.call(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}),
        );
        let ready_ms = self.spawned.elapsed().as_secs_f64() * 1000.0;
        assert!(r["result"].is_object(), "initialize: {r}");
        self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        ready_ms
    }

    fn tool_call(&mut self, args: &Value) -> Value {
        self.call("tools/call", json!({"name":"fetch","arguments":args}))
    }

    /// Close stdin, wait for exit, and assert every stdout line seen is a JSON-RPC 2.0 frame.
    fn finish_and_assert_pure(mut self) {
        drop(self.stdin.take());
        while let Ok(l) = self.lines.recv_timeout(Duration::from_secs(10)) {
            self.raw.push(l);
        }
        let _ = self.child.wait();
        assert!(!self.raw.is_empty());
        for l in &self.raw {
            let v: Value =
                serde_json::from_str(l).unwrap_or_else(|e| panic!("stray stdout {l:?}: {e}"));
            assert_eq!(v["jsonrpc"], "2.0", "not a JSON-RPC frame: {l}");
        }
    }
}

/// The rejection text. Every validation failure is an `isError` tool result (rmcp 3.4 does this for schema
/// failures; our own checks match it, ADR-006 amendment 2026-09-19); a JSON-RPC error would fail this helper.
fn rejection_text(r: &Value) -> Option<String> {
    assert!(
        r["error"].is_null(),
        "validation must not be a protocol error: {r}"
    );
    (r["result"]["isError"] == true).then(|| {
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    })
}

#[test]
fn initialize_lists_exactly_one_fetch_tool_with_schema() {
    let mut s = Session::start();
    s.handshake();
    let r = s.call("tools/list", json!({}));
    let tools = r["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1, "{r}");
    assert_eq!(tools[0]["name"], "fetch");
    let props = tools[0]["inputSchema"]["properties"]
        .as_object()
        .expect("properties");
    for f in ["url", "max_length", "start_index", "raw"] {
        assert!(props.contains_key(f), "schema lacks {f}: {props:?}");
    }
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["url"]));
    assert!(
        props["url"].get("default").is_none(),
        "url must not advertise a default: {props:?}"
    );
    s.finish_and_assert_pure();
}

/// D-5: the tool description is at most 150 words (NFR-09) and names every parameter plus the
/// `start_index` continuation pattern, read from the real `tools/list` response (not a copy of the literal).
#[test]
fn d5_tool_description_is_concise_and_names_every_parameter() {
    let mut s = Session::start();
    s.handshake();
    let r = s.call("tools/list", json!({}));
    let desc = r["result"]["tools"][0]["description"]
        .as_str()
        .expect("description string")
        .to_owned();
    let words = desc.split_whitespace().count();
    assert!(
        words <= 150,
        "description is {words} words, over the 150-word cap: {desc}"
    );
    let lower = desc.to_lowercase();
    for term in ["url", "max_length", "start_index", "raw"] {
        assert!(
            lower.contains(term),
            "description missing parameter {term}: {desc}"
        );
    }
    assert!(
        desc.to_lowercase().contains("start_index to continue")
            || desc.to_lowercase().contains("continue from"),
        "description must describe the start_index continuation pattern: {desc}"
    );
    s.finish_and_assert_pure();
}

#[test]
fn bad_input_is_rejected_with_the_field_named() {
    let mut s = Session::start();
    s.handshake();
    let cases = [
        (json!({}), "url"),
        (json!({"url": 5}), "url"),
        (json!({"url": "file:///etc/passwd"}), "url"),
        (json!({"url": "ftp://example.com/x"}), "url"),
        (json!({"url": "gopher://example.com"}), "url"),
        (
            json!({"url": "https://example.com", "max_length": -1}),
            "max_length",
        ),
        (
            json!({"url": "https://example.com", "max_length": 1.5}),
            "max_length",
        ),
        (
            json!({"url": "https://example.com", "start_index": -3}),
            "start_index",
        ),
        (
            json!({"url": "https://example.com", "start_index": 2.5}),
            "start_index",
        ),
        (
            json!({"url": "https://example.com", "start_index": "7"}),
            "start_index",
        ),
        (json!({"url": "https://example.com", "raw": "yes"}), "raw"),
    ];
    for (args, field) in cases {
        let r = s.tool_call(&args);
        let m = rejection_text(&r).unwrap_or_else(|| panic!("{args} must be rejected: {r}"));
        assert!(
            m.starts_with(&format!("error[invalid_argument]: {field}: ")),
            "{args}: error must be error[invalid_argument] naming {field}: {m}"
        );
    }
    s.finish_and_assert_pure();
}

/// `arguments` that is not a JSON object never reaches our validation: rmcp answers with a JSON-RPC error (A-7 README note).
#[test]
fn non_object_arguments_get_the_documented_rmcp_protocol_error() {
    let mut s = Session::start();
    s.handshake();
    let r = s.call("tools/call", json!({"name": "fetch", "arguments": []}));
    assert_eq!(r["error"]["code"], -32601, "{r}");
    s.finish_and_assert_pure();
}

/// A loopback port nothing listens on (bound, then released).
fn closed_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn ssrf_checks_refuse_blocked_literals_and_userinfo_before_anything_else() {
    let mut s = Session::start();
    s.handshake();
    // E-8: the loopback targets are refused by the shipped/default policy and pass the policy (so the client
    // dials, and the closed port refuses the connection) only in a `bench-loopback` build. Everything else is
    // refused in every build.
    let loopback = if cfg!(feature = "bench-loopback") {
        "network_error"
    } else {
        "blocked_target"
    };
    let p = closed_port();
    for (u, code) in [
        (format!("http://127.0.0.1:{p}/"), loopback),
        (format!("http://2130706433:{p}/"), loopback),
        (format!("http://[::1]:{p}/"), loopback),
        (
            "http://169.254.169.254/latest/meta-data".to_string(),
            "blocked_target",
        ),
        ("http://10.0.0.1/".to_string(), "blocked_target"),
        (format!("http://localhost:{p}/"), loopback),
        (
            "http://user:pw@example.com/".to_string(),
            "invalid_argument",
        ),
        ("http://[fe80::1%25eth0]/".to_string(), "invalid_argument"),
        ("file:///etc/passwd".to_string(), "invalid_argument"),
    ] {
        let r = s.tool_call(&json!({ "url": u }));
        let m = rejection_text(&r).unwrap_or_else(|| panic!("{u} must be rejected: {r}"));
        assert!(m.starts_with(&format!("error[{code}]")), "{u}: {m}");
    }
    s.finish_and_assert_pure();
}

/// End to end through the real binary: only a `bench-loopback` build may reach the loopback fixture. The body is
/// returned as fetched (no untrusted-content label, OQ-5 decided NO) and the character window is applied.
#[cfg(feature = "bench-loopback")]
#[test]
fn bench_build_fetches_a_loopback_fixture_and_returns_the_window_unlabelled() {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        while let Ok((mut c, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = c.read(&mut buf);
            let body = "0123456789 h\u{e9}llo";
            let _ = write!(
                c,
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
        }
    });
    let mut s = Session::start();
    s.handshake();
    let r = s.tool_call(
        &json!({"url": format!("http://127.0.0.1:{port}/"), "start_index": 2, "max_length": 10}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_ne!(r["result"]["isError"], true, "{r}");
    assert_eq!(
        r["result"]["content"].as_array().map(Vec::len),
        Some(1),
        "{r}"
    );
    assert_eq!(
        r["result"]["content"][0]["text"],
        "23456789 h\n\n[More content available. Call fetch again with start_index=12 to continue.]",
        "{r}"
    );
    let r = s.tool_call(&json!({"url": format!("http://127.0.0.1:{port}/")}));
    assert_eq!(
        r["result"]["content"][0]["text"], "0123456789 h\u{e9}llo",
        "{r}"
    );
    s.finish_and_assert_pure();
}

/// A-9 end to end: with a redirect the text begins with the final URL and status; without one nothing is added.
#[cfg(feature = "bench-loopback")]
#[test]
fn redirect_result_begins_with_final_url_and_status_and_plain_fetch_has_no_header() {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        while let Ok((mut c, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let n = c.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let _ = if head.starts_with("GET /old") {
                write!(c, "HTTP/1.1 301 Moved\r\nConnection: close\r\nContent-Length: 0\r\nLocation: /new\r\n\r\n")
            } else {
                write!(
                    c,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\n\r\nbody"
                )
            };
        }
    });
    let mut s = Session::start();
    s.handshake();
    let r = s.tool_call(&json!({"url": format!("http://127.0.0.1:{port}/old")}));
    assert_eq!(
        r["result"]["content"][0]["text"],
        format!("URL: http://127.0.0.1:{port}/new\nStatus: 200\n\nbody"),
        "{r}"
    );
    let r = s.tool_call(&json!({"url": format!("http://127.0.0.1:{port}/new")}));
    assert_eq!(r["result"]["content"][0]["text"], "body", "{r}");
    s.finish_and_assert_pure();
}

/// Readiness (A-2 AC "ready within 250 ms" on aarch64): RECORDED, not asserted here. The hard number belongs to
/// the native-aarch64 benchmark run; unit-test hosts are too noisy. Prints ready_ms to stderr (`--nocapture`).
#[test]
fn records_ready_ms_spawn_to_initialize_result() {
    let mut runs = vec![];
    for _ in 0..5 {
        let mut s = Session::start();
        runs.push(s.handshake());
        s.finish_and_assert_pure();
    }
    runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!("ready_ms (spawn -> initialize result, 5 runs, recorded not gated): {runs:.1?} median={:.1}", runs[2]);
    assert!(
        runs.iter().all(|m| *m < 10_000.0),
        "initialize took implausibly long: {runs:?}"
    );
}

#[test]
fn version_flag_prints_one_line_and_exits() {
    let out = Command::new(env!("CARGO_BIN_EXE_fetch-mcp"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.starts_with("fetch-mcp "), "{s}");
    assert_eq!(s.lines().count(), 1);
    // E-4: build identity (commit is 40 hex chars, `<40 hex>-dirty` when tracked files were modified, or `unknown` outside a
    // git checkout; lock hash is SHA-256 hex).
    let words: Vec<&str> = s.split_whitespace().collect();
    let commit = words
        .iter()
        .find_map(|w| w.strip_prefix("commit="))
        .expect("commit= in --version");
    let sha = commit.strip_suffix("-dirty").unwrap_or(commit);
    assert!(
        commit == "unknown" || (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())),
        "{s}"
    );
    let lock = words
        .iter()
        .find_map(|w| w.strip_prefix("cargo-lock="))
        .expect("cargo-lock= in --version");
    assert!(
        lock.len() == 64 && lock.bytes().all(|b| b.is_ascii_hexdigit()),
        "{s}"
    );
}

#[test]
fn invalid_config_exits_nonzero_before_handshake() {
    let out = Command::new(env!("CARGO_BIN_EXE_fetch-mcp"))
        .env("FETCH_LOG", "loud")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "stdout must stay empty on config error"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("FETCH_LOG"));
}

/// Regression (fix-pass 1): `eprintln!` on a broken-pipe stderr panics, and under `panic = abort` that kills the
/// process. The child sleeps in `sh` until the parent has dropped the read end of the stderr pipe, then execs the
/// server with `FETCH_LOG=info` so the startup log line hits EPIPE. The exit must be a normal code (stdin is
/// closed, so the server ends with the "connection closed" path, code 1), never a signal or a panic (101).
#[test]
fn closed_stderr_pipe_does_not_abort_or_panic() {
    let mut child = Command::new("sh")
        .args([
            "-c",
            "sleep 0.5; exec \"$0\"",
            env!("CARGO_BIN_EXE_fetch-mcp"),
        ])
        .env("FETCH_LOG", "debug")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    drop(child.stderr.take()); // read end closed: every stderr write is EPIPE
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "must exit normally (1), not abort/panic: {:?}",
        out.status
    );
}

/// F-2 (QA): a first message that is not `initialize` ends the session (exit 1, rmcp behaviour, kept), but the
/// client's frame must not be echoed to stderr at the default log level.
#[test]
fn non_initialize_first_message_exits_1_without_echoing_it() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fetch-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized","params":{{"note":"CLIENT_SECRET_MARKER"}}}}"#
    )
    .unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "no stdout frames: {:?}", out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("CLIENT_SECRET_MARKER"), "echoed: {err}");
    // A bench build announces itself with a `warn build marker` line first; the default build has none.
    let err = err
        .lines()
        .filter(|l| !l.starts_with("warn build marker "))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(err.starts_with("error serve"), "{err}");
}

/// F-1 (QA), pins rmcp 3.4.0 behaviour: a frame nested beyond serde_json's recursion limit cannot be parsed, so
/// rmcp cannot recover its id and sends no reply (a client waits for its own timeout). The server must stay
/// alive and keep stdout pure: a following request is answered. Known limitation, see fix-pass-1 report.
#[test]
fn too_deeply_nested_frame_gets_no_reply_but_server_survives() {
    let mut s = Session::start();
    s.handshake();
    let depth = 5000;
    let frame = format!(
        r#"{{"jsonrpc":"2.0","id":900,"method":"tools/call","params":{{"name":"fetch","arguments":{}{}}}}}"#,
        "[".repeat(depth),
        "]".repeat(depth)
    );
    let stdin = s.stdin.as_mut().unwrap();
    writeln!(stdin, "{frame}").unwrap();
    stdin.flush().unwrap();
    let r = s.tool_call(&json!({"url":"http://127.0.0.1/"}));
    assert_eq!(
        r["result"]["isError"], true,
        "server must still answer: {r}"
    );
    assert!(
        s.raw.iter().all(|l| !l.contains("\"id\":900")),
        "pinned rmcp behaviour changed (a reply to the deep frame now exists): revisit F-1"
    );
    s.finish_and_assert_pure();
}

/// Serve `body` as `text/html` to every connection (loopback fixture for `bench-loopback` builds).
#[cfg(feature = "bench-loopback")]
fn serve_html(body: Vec<u8>) -> u16 {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = std::sync::Arc::new(body);
    std::thread::spawn(move || {
        while let Ok((mut c, _)) = listener.accept() {
            let body = body.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = c.read(&mut buf);
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = c.write_all(&body);
            });
        }
    });
    port
}

#[cfg(feature = "bench-loopback")]
fn tool_text(r: &Value) -> String {
    r["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Peak RSS (`VmHWM`, KiB) of a running process.
#[cfg(all(feature = "bench-loopback", target_os = "linux"))]
fn peak_rss_kib(pid: u32) -> usize {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next().and_then(|n| n.parse().ok()))
        .expect("VmHWM")
}

/// Code review B1 end to end through the real binary: script, style, iframe and nav text must not reach the
/// output at any nesting depth (it used to leak from 256 open elements on). Failing closed is allowed.
#[cfg(feature = "bench-loopback")]
#[test]
fn real_stdio_drop_rules_hold_at_any_depth() {
    let mut s = Session::start();
    s.handshake();
    for depth in [255usize, 256, 300, 10_000] {
        let html = format!(
            "{}<script>SECRET_JS()</script><style>.SECRETCSS{{x:y}}</style><iframe>SECRETIFRAME</iframe>\
             <nav>SECRETNAV</nav><div hidden>SECRETHIDDEN</div><p>body</p>{}",
            "<div>".repeat(depth),
            "</div>".repeat(depth)
        );
        let port = serve_html(html.into_bytes());
        let r = s.tool_call(&json!({"url": format!("http://127.0.0.1:{port}/")}));
        let text = tool_text(&r);
        assert!(!text.contains("SECRET"), "depth {depth} leaked: {text:?}");
        if r["result"]["isError"] == true {
            assert!(text.contains("converter_limit"), "depth {depth}: {text}");
        } else {
            assert!(text.contains("body"), "depth {depth}: {text:?}");
        }
    }
    s.finish_and_assert_pure();
}

/// Architect B2 end to end: an attribute bomb is refused (`converter_limit`) and a 1.9 MiB single attribute
/// converts, three fetches at a time, with the peak RSS of the real server process under the 40 MiB target.
#[cfg(all(feature = "bench-loopback", target_os = "linux"))]
#[test]
fn real_stdio_attribute_bomb_is_refused_and_peak_rss_stays_under_40_mib() {
    let bomb = format!("<div {}>x</div>", "a ".repeat(950 * 1024));
    let big_attr = format!("<img alt=\"{}\">x", "z".repeat(1900 * 1024));
    let mut s = Session::start();
    s.handshake();
    let pid = s.child.id();
    for (name, html, refused) in [("bomb", bomb, true), ("huge attribute", big_attr, false)] {
        let port = serve_html(html.into_bytes());
        // three concurrent calls: FETCH_MAX_CONCURRENCY defaults to 3
        for id in 101..104u64 {
            s.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
                "params":{"name":"fetch","arguments":{"url": format!("http://127.0.0.1:{port}/")}}}));
        }
        let mut seen = 0;
        while seen < 3 {
            let l = s
                .lines
                .recv_timeout(Duration::from_secs(30))
                .expect("reply within 30s");
            s.raw.push(l.clone());
            let v: Value = serde_json::from_str(&l).expect("json");
            if v["id"].as_u64().is_some_and(|i| (101..104).contains(&i)) {
                seen += 1;
                let text = tool_text(&v);
                if refused {
                    assert_eq!(v["result"]["isError"], true, "{name}: {v}");
                    assert!(text.contains("converter_limit"), "{name}: {text}");
                } else {
                    assert_ne!(v["result"]["isError"], true, "{name}: {text}");
                }
            }
        }
        let peak = peak_rss_kib(pid) / 1024;
        eprintln!("stdio hostile rss after {name} x3: peak {peak} MiB");
        assert!(peak <= 40, "{name}: server peak RSS {peak} MiB > 40 MiB");
    }
    s.finish_and_assert_pure();
}

/// Serve `body` with the given `Content-Type` (none when empty) to every connection.
#[cfg(feature = "bench-loopback")]
fn serve_typed(content_type: &'static str, body: Vec<u8>) -> u16 {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = std::sync::Arc::new(body);
    std::thread::spawn(move || {
        while let Ok((mut c, _)) = listener.accept() {
            let body = body.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = c.read(&mut buf);
                let ct = if content_type.is_empty() {
                    String::new()
                } else {
                    format!("Content-Type: {content_type}\r\n")
                };
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n{ct}Content-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = c.write_all(&body);
            });
        }
    });
    port
}

/// A-5 end to end (FR-02, FR-04): four sequential calls over a 20,000-character page reproduce it with no gap or
/// overlap, the footer states the next start_index, the last page states the total, a start_index past the end is
/// an empty-content message (not an error), max_length above the cap is clamped and says so, and 0 is rejected.
#[cfg(feature = "bench-loopback")]
#[test]
fn pagination_end_to_end() {
    let page: String = (0..20_000)
        .map(|i| {
            if i % 9 == 0 {
                '\u{20ac}'
            } else {
                char::from(b'a' + u8::try_from(i % 26).unwrap())
            }
        })
        .collect();
    let port = serve_typed("text/plain; charset=utf-8", page.clone().into_bytes());
    let url = format!("http://127.0.0.1:{port}/");
    let mut s = Session::start();
    s.handshake();
    let (mut got, mut start, mut calls) = (String::new(), 0_u64, 0);
    loop {
        let r = s.tool_call(&json!({"url": url, "max_length": 5000, "start_index": start}));
        assert_ne!(r["result"]["isError"], true, "{r}");
        let t = tool_text(&r);
        calls += 1;
        match t.split_once("\n\n[More content available. Call fetch again with start_index=") {
            Some((content, rest)) => {
                assert_eq!(content.chars().count(), 5000, "{t}");
                got.push_str(content);
                let next: u64 = rest.trim_end_matches(" to continue.]").parse().unwrap();
                assert_eq!(next, start + 5000);
                start = next;
            }
            None => {
                let (content, note) = t.split_once("\n\n[Total length: ").unwrap();
                assert_eq!(note, "20000 characters.]");
                got.push_str(content);
                break;
            }
        }
    }
    assert_eq!((calls, got.as_str()), (4, page.as_str()));

    let r = s.tool_call(&json!({"url": url, "start_index": 20_000}));
    assert_ne!(r["result"]["isError"], true, "{r}");
    assert_eq!(
        tool_text(&r),
        "[No content at start_index=20000: the content is 20000 characters long.]"
    );
    let r = s.tool_call(&json!({"url": url, "start_index": 9_000_000_000_u64}));
    assert!(
        tool_text(&r).contains("the content is 20000 characters long"),
        "{r}"
    );

    let r = s.tool_call(&json!({"url": url, "max_length": 500_000}));
    let t = tool_text(&r);
    assert!(
        t.contains("[max_length was reduced to 100000 characters, the maximum.]"),
        "{t}"
    );
    assert!(
        t.starts_with(&page),
        "the clamp still returns the whole 20,000-char page"
    );
    let r = s.tool_call(&json!({"url": url, "max_length": 0}));
    let t = rejection_text(&r).expect("max_length 0 is rejected");
    assert!(t.starts_with("error[invalid_argument]: max_length:"), "{t}");
    s.finish_and_assert_pure();
}

/// A-6 end to end (FR-08): text types are returned as text (HTML converted unless raw=true), binary types are an
/// `isError` result naming the type (raw or not), an untyped body is sniffed.
#[cfg(feature = "bench-loopback")]
#[test]
fn content_types_end_to_end() {
    let html = "<html><body><h1>Hi</h1><p>there</p></body></html>";
    let mut s = Session::start();
    s.handshake();
    let call = |s: &mut Session, ct: &'static str, body: &str, extra: Value| {
        let port = serve_typed(ct, body.as_bytes().to_vec());
        let mut args = json!({"url": format!("http://127.0.0.1:{port}/")});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        s.tool_call(&args)
    };
    let r = call(&mut s, "text/html", html, json!({}));
    assert_eq!(tool_text(&r), "# Hi\n\nthere", "{r}");
    let r = call(&mut s, "text/html", html, json!({"raw": true}));
    assert_eq!(tool_text(&r), html, "{r}");
    let r = call(&mut s, "application/json", "{\"a\": [1, 2]}", json!({}));
    assert_eq!(tool_text(&r), "{\"a\": [1, 2]}", "{r}");
    let r = call(&mut s, "application/xml", "<a><b>1</b></a>", json!({}));
    assert_eq!(tool_text(&r), "<a><b>1</b></a>", "{r}");
    let r = call(&mut s, "", html, json!({}));
    assert_eq!(tool_text(&r), "# Hi\n\nthere", "{r}");
    let r = call(&mut s, "", "just some words <b>here</b>", json!({}));
    assert_eq!(tool_text(&r), "just some words <b>here</b>", "{r}");
    for (ct, raw) in [
        ("image/png", false),
        ("application/pdf", false),
        ("image/png", true),
    ] {
        let r = call(&mut s, ct, "\u{fffd}PNG", json!({"raw": raw}));
        let t = rejection_text(&r).expect("binary types are an isError result");
        assert!(
            t.starts_with("error[unsupported_content_type]:") && t.contains(ct),
            "{t}"
        );
    }
    s.finish_and_assert_pure();
}
