# Technical Writer DoD review, round 2 (Sprint 1, HEAD 1ce36a0)

STATUS: DONE (0 blocking, 3 non-blocking)

## Verified
- Clean `git archive HEAD` extract: `cargo build --locked` OK, `--version` prints `fetch-mcp 0.0.0`, `cargo test --locked` all groups ok (42 + 10), `python3 bench/selftest.py` ends `SELFTEST PASSED`.
- README stdio session: 5 reply lines, all texts match README exactly (initialize rmcp 3.4.0, tools/list, not_implemented, blocked_target loopback, invalid_argument url). stderr empty by default.
- Guards: `scripts/check-release-features.sh` guard OK, `--self-test` OK, `--no-http-client` OK.
- Relative links and anchors in the four docs: no broken file or anchor.
- docs/SSRF.md vs src/ssrf/ranges.rs: all 17 IPv4 rows and 14 IPv6 rows plus the four embedded-IPv4 forms match (prefixes, categories, relaxable rows).
- CI check names fmt, clippy, test, deny, release-guard match ci.yml job ids. Action pins/versions match. `gh pr checks 5` shows all five passing, so the README claim holds.
- Architecture 6.1/6.2/9.1 and ADR-003/ADR-006 amendments present and consistent with behaviour; `url` pin gone from 9.1.
- Honesty caveats intact: pre-MVP, no real-client registration before M3, cargo-audit never run, G4a/G4b not run, branch protection not configured, OQ-3/4/5/7 open, numbers labelled advisory. Units MiB. Release wording is container image (ADR-007). No stale "no MCP server" or "handshake fails" claims.

## Non-blocking
1. BENCHMARK.md "Read this first" and ci-branch-protection.md say the workflow ran once, on PR #2 (commit 740c0fb); README says CI passes on PR #5 and PR #5 checks are green. Not a contradiction, but the "run once" tables are stale. Refresh with the PR #5 run.
2. BENCHMARK.md section 7 still says "Both gnu and musl binaries are run" and glossary says "Skeleton" (fine); the musl line reads oddly next to "gnu build only" in section 13. Clarify it is the plan.
3. src/error.rs:18 says "Sprint 1 skeleton"; src/lib.rs says "walking skeleton". Accurate history, but the docs elsewhere avoid the word. Optional wording tweak.
