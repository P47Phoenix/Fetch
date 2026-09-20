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
        assert!(m.contains(field), "{args}: error must name {field}: {m}");
    }
    s.finish_and_assert_pure();
}

#[test]
fn ssrf_checks_refuse_blocked_literals_and_userinfo_before_anything_else() {
    let mut s = Session::start();
    s.handshake();
    // E-8: the loopback targets are refused by the shipped/default policy and pass the policy (reaching the
    // not-yet-implemented fetch) only in a `bench-loopback` build. Everything else is refused in every build.
    let loopback = if cfg!(feature = "bench-loopback") {
        "not_implemented"
    } else {
        "blocked_target"
    };
    for (u, code) in [
        ("http://127.0.0.1/", loopback),
        ("http://2130706433/", loopback),
        ("http://[::1]:8080/", loopback),
        ("http://169.254.169.254/latest/meta-data", "blocked_target"),
        ("http://localhost/", loopback),
        ("http://user:pw@example.com/", "invalid_argument"),
        ("http://[fe80::1%25eth0]/", "invalid_argument"),
        ("file:///etc/passwd", "invalid_argument"),
    ] {
        let r = s.tool_call(&json!({ "url": u }));
        let m = rejection_text(&r).unwrap_or_else(|| panic!("{u} must be rejected: {r}"));
        assert!(m.starts_with(&format!("error[{code}]")), "{u}: {m}");
    }
    s.finish_and_assert_pure();
}

#[test]
fn valid_input_reports_not_implemented_without_fetching() {
    let mut s = Session::start();
    s.handshake();
    for args in [
        json!({"url": "https://example.com"}),
        json!({"url": "http://example.com/a", "max_length": 10, "start_index": 0, "raw": true}),
    ] {
        let r = s.tool_call(&args);
        assert_eq!(r["result"]["isError"], true, "{r}");
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("not implemented yet"), "{text}");
        assert!(text.starts_with("error[not_implemented]"), "{text}");
    }
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
