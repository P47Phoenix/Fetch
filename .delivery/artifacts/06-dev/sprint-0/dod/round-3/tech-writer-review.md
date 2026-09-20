# Technical Writer DoD review, Sprint 0 (D-7, E-1), round 3

Verdict: NOT_DONE (2 blocking, 4 non-blocking). All commands below were run on the branch (x86_64 host).

## Verified OK
- `python3 bench/selftest.py`: about 30 s, last line `SELFTEST PASSED` (doc says "about 1 minute", fine).
- `fixtures.py generate`, then the documented step-4 smoke command: 10 runs, JSONL with `host`, scenario and `summary` lines, verdict `ADVISORY_PASS`, never `PASS` in the summary.
- Exit codes match the doc: `--gate` off aarch64 gives 3; partial bench gate set gives 2 (`incomplete`, refused before the aarch64 check); `--child-env` under `--gate` refused; `--runs 2` without `--smoke` refused (2).
- Marker contract: `--version` prints `fetch-mcp 0.0.0` (release) and `fetch-mcp 0.0.0 FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback` (bench). This matches BENCHMARK.md sec 4, `src/lib.rs` comments, and the guard (greps `FETCH_MCP_MARKER_`, allowlist `default`).
- ci-branch-protection.md lists fmt, clippy, test, deny, release-guard: exactly the ci.yml job ids; states NOT YET CONFIGURED; states licence open (OQ-7); commands match ci.yml; dependabot.yml exists; pins consistent with Cargo.toml.
- Units (MiB), targets, valid-run rule, scenario table, placeholders (host section, PLACEHOLDER) are clear and consistent with EPICS E-1/E-8/D-7.

## Blocking
1. docs/BENCHMARK.md step 4 and sec 6: `--smoke` exits 0 with `ADVISORY_PASS` even at `--runs 2` (verified) and the docs never say that exit 0 there is NOT a pass. "0 pass" beside an ADVISORY_PASS example invites CI/contributors to treat a smoke exit 0 as a result. Add one sentence: exit 0 also occurs for advisory (non-`--gate`) and `--smoke` runs; only a `--gate` run with verdict `PASS` counts.
2. Licence contradiction: repo root has an Apache-2.0 `LICENSE` (initial commit) while docs/ci-branch-protection.md, deny.toml and PRD say the licence is undecided (OQ-7). Docs should state what the LICENSE file is (for example a GitHub initial placeholder, not the decision) or the file should be reconciled by the owner.

## Non-blocking
- README.md is a single "# Fetch" line; it does not point to docs/BENCHMARK.md quickstart or docs/ci-branch-protection.md.
- BENCHMARK.md quickstart step 3/5 do not say the current `fetch-mcp` binary is a skeleton with no MCP server, so a real (non-stand-in) measure.py run fails the handshake (verified: exit 2). Add a note that real runs need A-2/A-3a.
- EPICS E-8 says a bench build "logs a marker to stderr and reports it in the handshake"; code only prints it via `--version`. BENCHMARK sec 4 correctly documents `--version` only; note the EPICS deviation is deferred.
- Per-scenario lines show `verdict: PASS` even in advisory runs (summary is `ADVISORY_PASS`); doc says only the summary never reads PASS. Clarify.
