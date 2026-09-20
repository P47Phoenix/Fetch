# PR #6 code review — A-3b guarded streaming fetch (+ E-8)

- **Branch**: `sprint-2/guarded-fetch`, head `c053d8c`, base `main` @ `090e86c`
- **Reviewer**: independent code review (read-only; nothing edited, pushed, commented or merged)
- **Date**: 2026-09-20
- **Decision**: REQUEST_CHANGES (one blocking item, small and localised)

## Scope reviewed

Full diff `090e86c..c053d8c` (31 files, +3656/-182), with attention to:

- `src/fetch/mod.rs`, `src/fetch/body.rs`, `src/fetch/dns.rs`, `src/fetch/tests.rs` (new)
- `src/ssrf/mod.rs`, `src/ssrf/resolver.rs`, `src/ssrf/differential.rs` (new)
- `src/policy.rs`, `src/server.rs`, `src/error.rs`, `src/lib.rs`, `src/main.rs`, `src/config.rs`
- `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.github/workflows/ci.yml`, `scripts/check-release-features.sh`
- `tests/stdio.rs`, `README.md`, `docs/ci-branch-protection.md`

### Verification actually run locally

| Check | Result |
|---|---|
| `cargo test --locked` (lib + `tests/stdio.rs` + doc) | 85 + 9 passed, 0 failed |
| `cargo clippy --locked --all-targets -- -D warnings` | clean |
| `scripts/check-release-features.sh --self-test` | `self-test OK`, exit 0 (clean release build passes; `test-support` and `bench-loopback` builds each fail for **both** the tree and the marker reason; renamed feature, garbage tree text and unknown marker all fail as designed) |
| Release binary size (`opt-level="s"`, LTO, strip) | 2.90 MiB |
| Locked package count | 180 |
| Empirical reqwest error-Debug probe (scratch crate in `/tmp`, not in this repo) | confirmed finding B-1 below |

---

## Blocking

### B-1 — `map_transport` classifies transport errors by string-matching a Debug string that contains the request URL (confidence 95)

`src/fetch/mod.rs:276-295`

```rust
let mut chain = format!("{e:?}").to_ascii_lowercase();
...
if chain.contains("too large") || chain.contains("too many headers") || chain.contains("header") {
    return FetchError::BadResponse("the response headers are too large or malformed".into());
}
if e.is_connect() { return FetchError::Network("could not connect to the host".into()); }
```

`reqwest::Error`'s `Debug` impl (reqwest 0.13.5 `src/error.rs`) prints the URL:

```rust
if let Some(ref url) = self.inner.url { builder.field("url", &url.as_str()); }
```

and `async_impl/client.rs:3123` attaches the request URL to every send-path error. So the URL text is part of `chain`,
and the `contains("header")` / `contains("too large")` probes match on the **URL**, not on the transport failure.

Reproduced with a standalone probe against the same pinned reqwest/rustls versions:

```
is_connect=true is_timeout=false
debug=reqwest::Error { kind: Request, url: "http://127.0.0.1:9/api/headers",
      source: hyper_util::client::legacy::Error(Connect, ConnectError("tcp connect error", 127.0.0.1:9,
      Os { code: 111, kind: ConnectionRefused, message: "Connection refused" })) }
lower_contains_header=true
```

Consequences:

- A plain connection failure for any URL whose host/path/query contains the substring `header`
  (`https://example.com/api/headers`, `?header=1`, `x-headers.example.com`, …) is reported as
  `error[bad_response]: the response headers are too large or malformed` instead of
  `error[network_error]: could not connect to the host`. The header branch runs *before* `e.is_connect()`,
  so the misclassification always wins.
- The classification is remotely steerable: redirect `Location` values come from the upstream server, and
  `run()` re-enters `fetch` with that URL, so a server can make the next hop's transport failure surface as a
  `bad_response`.
- `README.md` (this PR) publishes the code list as the tool's error contract, and `src/error.rs` documents the
  codes as "stable machine-readable"; `tests/stdio.rs` asserts on the `error[<code>]` prefix. A code that depends
  on the spelling of the URL is not stable.

The existing test `connection_refused_is_a_category_without_the_address` does not catch this only because its
fixture URL happens to be `http://public.test:<port>/`.

Note the *sources* walked afterwards use `Display` (`s.to_string()`), which for `hyper_util::…::Error` is
`client error (Connect)` and carries no address — so only the top-level `{e:?}` is the problem, and no address
is actually leaked into the message today. This is a classification bug, not a disclosure bug.

**Suggested fix** (keeps the existing heuristic, removes the URL from the matched text):

```rust
fn map_transport(e: reqwest::Error) -> FetchError {
    if e.is_timeout() {
        return FetchError::Timeout("the request timed out".into());
    }
    let is_connect = e.is_connect();
    let e = e.without_url();                      // reqwest 0.13.5 src/error.rs:91
    let mut chain = format!("{e:?}").to_ascii_lowercase();
    ...
}
```

Please also add a regression case to `src/fetch/tests.rs` — e.g. extend
`connection_refused_is_a_category_without_the_address` to also request `/api/headers` on the dead port and assert
`network_error`.

---

## Non-blocking

### N-1 — `error.rs` doc comment claims a port policy that does not exist yet (confidence 85)

`src/error.rs:13` describes `BlockedTarget` as "Port policy, non-public address, blocked redirect", but
`split_host_port` (`src/ssrf/mod.rs:130-140`) accepts every port except 0, and nothing else filters ports. Any
public host is reachable on 25, 6379, 11211, … This looks like a deliberate deferral (`src/config.rs:3` says the
allowed-ports variable "arrive[s] with the stories that use them"), so the code is fine — the comment should stop
promising it, or point at the story that lands it (C-1/B-3).

### N-2 — `dns::normalize` and `ssrf::parse_host` normalise trailing dots differently (confidence 88)

`src/fetch/dns.rs:34-36` uses `trim_end_matches('.')` (all trailing dots); `src/ssrf/mod.rs:166` uses
`strip_suffix('.')` (exactly one). `cross_check` (`src/fetch/mod.rs:339`) compares the two, and `Pinned::resolve`
uses only the former. There is no divergence today because `parse_host` rejects a host with an empty label, so
`example.com..` never reaches either path — but the SSRF core's whole design is "the two parsers must agree", and
two spellings of "normalise" on either side of that comparison is a latent trap. Consider having `parse_host`
call `dns::normalize`, or at least fix the `normalize` doc comment, which says "drops one trailing dot" while the
code drops all of them (the unit test at `dns.rs:100` only covers the single-dot case).

### N-3 — `rustls-platform-verifier` (and `rustls-native-certs`, `openssl-probe`) are linked although ADR-001 chose webpki-roots (confidence 90 on the fact, informational)

`reqwest`'s `rustls-no-provider` feature unconditionally enables `dep:rustls-platform-verifier`
(reqwest 0.13.5 `Cargo.toml:174-177`), which on Linux pulls `rustls-native-certs` → `openssl-probe`. The code
never uses it (`hop_client` always calls `tls_backend_preconfigured` with the webpki-roots store), and reqwest
0.13.5 offers no `__rustls`-only feature to opt out, so there is nothing to change here. Worth a line in the
dependency ledger / ADR-001 so the next reader does not think a system trust store is in play: it is compiled
into the graph but never consulted. `deny.toml`'s openssl bans still hold (`openssl-probe` is a path prober, not
openssl).

### N-4 — Timing-tight tests are the likeliest CI flakes (confidence 75, below the report bar but called out because test reliability was in scope)

- `a_server_that_never_finishes_times_out_and_the_connection_is_closed` — 400 ms total budget
- `a_slow_drip_body_cannot_outlive_the_overall_deadline` — 500 ms
- `queueing_for_a_slot_is_bounded_by_the_timeout` — 600 ms timeout with a 100 ms pre-sleep
- `ten_calls_run_three_at_a_time_and_each_gets_its_own_body` — asserts `(2..=3).contains(&peak)`; a slow accept
  loop on a shared runner could observe `peak == 1`

All passed locally (lib suite in 1.21 s). They run twice in CI (`test` job on dev, `a3b-merge-gate` on release,
the latter right after an LTO release build on a shared runner), so if flakes appear, these four are where to
look first. No change requested.

### N-5 — `check_head` byte accounting under-counts by ~4 bytes per header (confidence 90, negligible impact)

`src/fetch/mod.rs:299` sums `k.len() + v.len()` and omits `": "` + CRLF, so the effective ceiling is up to
~256 B above the documented 32 KiB at the 64-header limit. Harmless; noting only because `MAX_HEADER_BYTES` is
documented as "names plus values", which is in fact what the code does — so this is arguably just a doc/intent
question, not a defect.

### N-6 — A fresh `reqwest::Client` per redirect hop (confidence 70, design note)

`hop_client` builds a new `Client` (and clones the rustls `ClientConfig`) for every hop. This is the mechanism
that makes `Pinned` the only dial route, so it is correct and intentional — but it means a new hyper connection
pool and TLS setup per hop, which is worth having the E-2 bench confirm against the 40 MiB peak target on a
redirect chain, not just on a single-hop 5 MiB page.

---

## What I checked and found correct

**SSRF / DNS rebinding.** The contract holds end to end: `validate_target` resolves once, refuses the *whole*
answer set if any address is blocked, and returns only the validated `SocketAddr`s; `hop_client` installs
`dns::Pinned` as the client's sole resolver so a second lookup cannot change what is dialled; redirects are
followed by our own loop with `redirect::Policy::none()` and each hop is re-validated with `Origin::Redirect`;
`cross_check` refuses any disagreement between the hand-rolled parser and the `url` crate on scheme, port, host,
username or password. IP literals never touch a resolver. `no_proxy`, `pool_max_idle_per_host(0)`, `http1_only`,
`referer(false)` and the absence of the cookie feature all line up with NFR-07, and the tests assert the absence
of `cookie:`, `authorization:`, `proxy-authorization:` and `referer:` on every hop. I tried to construct a bypass
through backslash authorities, `@` in the authority, tab/newline stripping, trailing dots, IDN, bracketed IPv6,
zone ids, and WHATWG IPv4 spellings — every one is either rejected by `check_url` (fail closed) or caught by
`cross_check`. The differential corpus (>10 000 inputs, plus every radix/part-count spelling of 14 addresses,
plus a deterministic fuzz corpus) with non-vacuity counters is a genuinely strong gate, and it is wired as a
required CI check that also greps the log to prove the tests ran.

**Unbounded memory.** `body::Pipeline` re-slices wire chunks to 64 KiB, counts decompressed bytes against the cap
inside `Out::write` before emitting, stops a gzip stream at the first member (so multi-member bombs are
defeated), and holds at most 3 carry bytes in the UTF-8 decoder. `Content-Length` over the cap aborts before a
body byte is read; the wire cap is checked before the crossing chunk is fed. `Window` is bounded by
`max_length_cap` (100 000 chars default). The TLS config and the webpki root store are built lazily in a
`OnceLock`, so idle RSS does not pay for them. Nothing accumulates the body.

**Panics under `panic = abort`.** I walked every indexing and arithmetic site in the new non-test code.
`Utf8Stream.carry` can never be indexed past 3 (the carry only ever holds an *incomplete* prefix, max 3 bytes);
`parse_ipv4_whatwg`'s shift/add cannot overflow `u64` given the `>255` and `limit` guards and `parts.len() <= 4`;
`split_host_port`'s `authority[..=close]` / `[close+1..]` are ASCII-bracket boundaries. I specifically checked the
`Instant::now() + self.limits.timeout` in `fetch()` — tokio's `Add<Duration>` panics on overflow, but
`Duration::from_millis(u64::MAX)` is ~1.8e16 s, well inside `i64` seconds, so no `FETCH_TIMEOUT_MS` value can
trip it. No `unwrap`/`expect`/`panic!` in non-test code. `obs::stderr_line` correctly avoids `eprintln!`'s
broken-pipe panic, and `tests/stdio.rs::closed_stderr_pipe_does_not_abort_or_panic` covers it.

**Stdout purity.** `#![deny(clippy::print_stdout)]` on both crate roots, the single `#[allow]` on `--version`
(which exits before any MCP traffic), CI runs `clippy -D warnings` over three feature sets, and
`tests/stdio.rs::finish_and_assert_pure` parses every stdout line as a JSON-RPC 2.0 frame across five scenarios
including config failure, a non-`initialize` first frame and a 5000-deep frame.

**Dependency weight and deny policy.** 4 new direct deps (reqwest, rustls, webpki-roots, flate2) = 8 of the
NFR-05 budget of 15, all exactly pinned; `url` and `serde_json` are dev-only. `flate2` uses the pure-Rust
`rust_backend`. `deny.toml` correctly adds only `CDLA-Permissive-2.0` for webpki-roots and keeps the
openssl/native-tls/aws-lc bans, which `rustls = { features = ["ring", ...] }` and reqwest's
`rustls-no-provider` (as opposed to `rustls`, which would pull aws-lc-rs) respect.

**Release-feature guard integrity.** Ran `--self-test` end to end: it passes, and all four positive controls
genuinely fail. Marker strings survive `strip = true` (they are `.rodata`, reachable from `main`), and the
two-layer design (tree allowlist + marker grep) means a renamed or newly added forbidden feature is caught by
layer 1 even without a marker. `Policy::for_build()` is the only path from a feature to a relaxed policy, it has
no runtime switch, and `policy.rs`'s cfg-gated tests assert both directions. The removal of the Sprint 1
no-HTTP-client / no-tokio-net checks is deliberate, documented in the script header and in
`docs/ci-branch-protection.md`, and replaced by the `a3b-merge-gate` job.

**Documentation.** `README.md` and `docs/ci-branch-protection.md` are updated accurately and in plain language;
the branch-protection list correctly grows to six required checks; OQ-5 is recorded consistently. The
`a3b-merge-gate` job's log-grep (asserting each named test printed `... ok`) is a good anti-vacuity measure.
