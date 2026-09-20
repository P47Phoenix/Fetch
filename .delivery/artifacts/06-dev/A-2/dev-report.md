# A-2 Dev Report: Walking skeleton (stdio server with `fetch` schema)

Branch `sprint-1/skeleton-ssrf`. Local x86_64 (Fedora, kernel 7.2, 4 KiB pages). All memory and timing figures are advisory.

## What was built
- `src/config.rs`: immutable `Config` from env (FETCH_LOG, FETCH_TIMEOUT_MS, FETCH_MAX_BYTES, FETCH_MAX_LENGTH_CAP, FETCH_MAX_CONCURRENCY); invalid value exits 2 naming the variable, before the handshake.
- `src/error.rs`: `FetchError` (`invalid_argument`, `not_implemented`), stable codes, `error[code]: message` text.
- `src/obs.rs`: hand-rolled stderr logger. `#![deny(clippy::print_stdout)]` in the lib; the single `--version` print in main is explicitly allowed.
- `src/policy.rs`: `Policy` skeleton, fail-closed `Default`; `permit_loopback_for_tests()` exists only under `cfg(test)`, `test-support` or `bench-loopback`. A-3a fills the range table and checks.
- `src/server.rs`: rmcp `#[tool_router(server_handler)]` with one tool `fetch` (`url`, `max_length`, `start_index`, `raw`); errors from deserialisation are wrapped so the message starts with the field name; `validate_url` accepts only `http://` / `https://` (no URL parser yet). Valid input returns an `isError` result `error[not_implemented]: fetch is not implemented yet ...`. No network code.
- `src/main.rs`: `current_thread` tokio, `--version` unchanged (marker contract intact), serve over stdio.
- Release profile pin, features (`test-support`, `bench-loopback`) and guard script unchanged. Cargo.lock committed.

## Direct dependencies vs NFR-05 (<= 15)
| # | Crate | Requirement | Purpose | Notes |
|---|---|---|---|---|
| 1 | rmcp | `=3.4.0`, features server, transport-io, macros, schemars | MCP server, stdio | exact pin (R5) |
| 2 | tokio | `=1.53.1`, rt, macros, io-std, io-util | current_thread runtime | no `net` feature requested |
| 3 | serde | `1`, derive, std | params | |
| 4 | schemars | `1` | tool input JSON schema | |

4 of 15 used. dev-dependency `serde_json` (tests only, not shipped, not counted). No HTTP client, TLS or resolver crate (Sprint 1 gate holds). Transitive `tracing` comes via rmcp (R14; `cargo tree -i tracing` shows rmcp only); nothing installs a subscriber, so it emits nothing.

## Idle number (first real-product idle, advisory)
`bench/measure.py --binary target/release/fetch-mcp --binary-kind shipped --scenario idle --runs 10 --parallel-idle 5` (30 s settle, no --gate, x86_64, not native aarch64):
- VmRSS median **3.15 MiB** (3222 kB; min 3080, max 3280 kB), 10/10 valid, target 10 MiB, verdict ADVISORY_PASS.
- ready_ms (spawn to initialize result) median 0.99 ms (min 0.63, max 1.99); tools_list_ms median 0.18 ms.
- Caveats: parallel-idle 5 (advisory-only option), skeleton has no HTTP/TLS/converter so the number will rise; aarch64 and the D-1 re-measure still govern. Raw JSONL: `idle-advisory.jsonl` beside this report.
- Test-side readiness: `tests/stdio.rs::records_ready_ms_spawn_to_initialize_result` records 5 runs (about 1 ms, debug build), asserted only against a 10 s sanity bound, not 250 ms.
- Release binary 1,276,440 bytes (stripped, x86_64).

## Sprint 0 items
- NB-1 fixed: `main()` wraps `_main()`; any uncaught exception emits a REFUSED summary and exits 2; malformed `--child-env` (no `=`) is refused at parse time.
- NB-2 fixed: any `kind="peak"` scenario without `min_bytes` is refused (exit 2) instead of defaulting the floor to 0. No gate weakened.
- `bench/selftest.py` gained three checks (child-env refusal, missing-min_bytes refusal, uncaught exception exits 2); it passes.

## Tests
Unit (11 default, 12 with a feature): config defaults/overrides/invalid names variable; error codes/text; log thresholds; Policy default fail-closed and test constructor; scheme accept/reject; marker/version tests.
Integration `tests/stdio.rs` (spawns the real binary): initialize + tools/list exactly one `fetch` with the four schema properties and `required=["url"]`; 11 bad-input cases (missing url, wrong-type url, file:, ftp:, gopher:, negative and non-integer max_length/start_index, string start_index, non-bool raw) each rejected naming the field; valid input returns not-implemented; every stdout line is a JSON-RPC 2.0 frame after close (stdout purity); ready_ms recording; `--version` one line; invalid config exits non-zero with empty stdout.
Verified locally: fmt, clippy (default, bench-loopback, test-support) with -D warnings, cargo test --locked in all three configs, guard and `--self-test`, `bench/selftest.py`, actionlint, `cargo build --release --locked`.

## Deviations
1. rmcp 3.4 reports schema-deserialisation failures (missing url, negative numbers, wrong types) as an `isError` tool result whose text names the field, not as JSON-RPC invalid-params. Our own scheme check returns invalid-params. Both are rejections naming the field and the test accepts either; architecture 6 / ADR-006 say protocol-level for all. Making them uniform needs a custom handler; left for the architect to decide.
2. `Config` carries only the five variables A-2 needs; the rest arrive with their stories.
3. `serde_json` used as a dev-dependency.

## Open issues
- A-2 AC "Claude Code with a throwaway config lists `fetch`": not done; manual step on the author's machine.
- A-2 AC 250 ms on aarch64: not measured on aarch64 (only x86_64 above); needs the hosted arm64 run.
- Architecture 5.1 row j / R14 allocation-count test (rmcp result clones) not built: meaningful only with real result sizes; suggest A-5.
- NB-3 (deny/Dependabot coverage of spikes/a1) untouched; Sprint 3 as reviewed.

## Manual check checklist: "Claude Code lists `fetch`" (added in fix-pass 1; owner runs it, nothing was run or registered by the developer)
Use a THROWAWAY config so no real Claude Code setup is touched.
1. Build: `cargo build --release --locked`; note the absolute path of `target/release/fetch-mcp`.
2. Make an empty scratch directory and a scratch config file in it (for example `/tmp/fetch-check/.mcp.json`) containing one stdio server named `fetch` whose `command` is that absolute binary path. Do not use `~/.claude*` or any existing project config.
3. Start Claude Code from that scratch directory (project-scoped `.mcp.json`, approve the server when prompted) and run `/mcp`.
4. Expect: server `fetch` connected, exactly one tool `fetch`, with parameters `url` (required), `max_length`, `start_index`, `raw`. A call with a valid public URL returns `error[not_implemented]` (A-3b); `http://127.0.0.1/` returns `error[blocked_target]`.
5. Record the result (pass/fail, Claude Code version, date) in the UAT notes, then delete the scratch directory.

## Corrections (added in the A-3a cleanup commit; text above left as written)
Superseded: the `validate_url` helper and the "11 default unit" test count describe the A-2 state. URL validation is now `ssrf::check_url` (A-3a, hardened in fix-pass 1) and the counts are 42 unit + 10 integration in the default configuration. See `../sprint-1/fix-pass-1-report.md`.
