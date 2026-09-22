# Sprint 8 QA / DoD Review — PR #12 (sprint-8/ssrf-suite-release-profile)

Head: eda0408b872123f6bec1d49e99678da7936e3874 | Main: 0926ba8
Reviewer: fresh QA validator, read-only, independently re-verified (no prior-agent claims trusted).

## Scope
B-5 (SSRF suite + 90% coverage gate, 3pt), D-1 (release profile finalize + re-measure, 2pt), D-5 (tool description <=150 words, 1pt). E-5 (2pt) intentionally out of scope this sprint.

## B-5 — SSRF suite + coverage gate
- Gate enforces combined >=90% line coverage across exactly the ACs' files: `.github/workflows/ci.yml:92-100` → `scripts/coverage_gate.py lcov.info src/ssrf/mod.rs src/ssrf/ranges.rs src/ssrf/resolver.rs src/ssrf/differential.rs src/fetch/mod.rs src/convert/window.rs --min 90`. PASS.
- Gate genuinely fails: fed synthetic 10%-coverage lcov → "COVERAGE GATE FAILED", exit 1; 100%-coverage lcov → "COVERAGE GATE PASSED", exit 0. Not a no-op. PASS.
- "Combined vs per-file" interpretation is explicitly disclosed in three places: `scripts/coverage_gate.py:72-74` code comment, docs/BENCHMARK.md §18, dev-report.md. Defensible reading, honestly flagged, not silently substituted. PASS.
- Rebinding/range/differential tests are real: read `src/ssrf/resolver.rs:309-324` (rebinding test — FakeResolver returns public IP first, private second, asserts only first lookup used, second never made) plus confirmed table-driven range/differential tests exist. PASS.

## D-1 — Release profile + re-measure
- Cargo.toml `[profile.release]` unchanged in this PR (`git diff main...eda0408 -- Cargo.toml` empty); profile traces to D-7 pin (commit 0716084), untouched since. PASS.
- docs/BENCHMARK.md §18 figures cross-checked against CI run 35682100887 job logs (e.g. amd64-gnu 5.99 MiB peak) — exact match, including build identity (commit + Cargo.lock hash). PASS.
- **Disclosed, non-blocking**: the measured commit is 8d606cc (one commit behind head eda0408); the eda0408 diff is docs-only, so the binary is unchanged, and BENCHMARK.md §18 states this. Report should say "measured at 8d606cc, unchanged through eda0408" for max precision — wording nit only.
- Run 35682100887 was triggered via `workflow_dispatch`, not the PR's own `pull_request` event — bench.yml's `paths` filter genuinely doesn't match this PR's diff (docs/CI/coverage-script/test changes only, no src/Cargo.*). This is disclosed in BENCHMARK.md §18. Assessed as an acceptable, honestly-disclosed substitution, not a blocking process gap — though a standing policy to bench-gate all D-1/release-profile-labeled PRs regardless of path filter is worth considering going forward.
- `release-ldd-guard` job read directly (`ci.yml:63-77`): builds release, runs `ldd`, greps for ssl/crypto/native-tls, exits 1 on match. Genuine check, not a stub. PASS.

## D-5 — Tool description
- `tests/stdio.rs:139-169` spawns the real binary over stdio, calls `tools/list`, reads the description from the live JSON-RPC response (not a hardcoded copy), asserts word count <=150, and checks for url/max_length/start_index/raw and a continuation-pattern phrase. PASS.
- Confirmed passing in a fresh local `cargo test --locked` run against the real binary. PASS.

## Docs / process consistency
- No document claims E-5 or A-4 "Done"; all references say "not Done"/"remains open," and E-5's missing owner 10-URL list is reiterated (dev-report.md:31, sprint-plan.md:306). PASS.
- Sprint 7 merge (0926ba8, 6/8 pts) recorded accurately in sprint-plan.md. PASS.
- Plan Revision 17 / state.md consistent with Sprint 8 scope (B-5+D-1+D-5 = 6pt). PASS.
- The E-5 plan-table vs Revision-16-narrative inconsistency is explicitly flagged in dev-report.md:31 / sprint-plan.md:306, not silently resolved. PASS.

## Clean-checkout validation (run at eda0408, TMPDIR/CARGO_TARGET_DIR under ~/.cache)
- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean, PASS (run once, time-boxed).
- `cargo test --locked`: 177+1+11 tests pass, 0 failures (includes D-5 test and SSRF refusal test). PASS.
- `cargo deny check`: advisories/bans/licenses/sources all ok; two cosmetic unmatched-license-allowance warnings (BSD-2-Clause, CC0-1.0), non-blocking.
- Did not regenerate full coverage data via local `cargo llvm-cov` or rerun `bench/selftest.py` locally (time-boxed); relied on CI's green `coverage` job plus independent synthetic-lcov proof of gate logic (see B-5). Disclosed, not a silent skip.
- `gh pr checks 12` on head eda0408: all 8 checks green (a3b-merge-gate, clippy, coverage, deny, fmt, release-guard, release-ldd-guard, test), confirmed against correct head SHA, none stale/skipped. PASS.

## Verdict
No blocking issues found. B-5, D-1, D-5 genuinely meet their ACs; disclosures around the combined-coverage reading, the bench workflow_dispatch substitution, and the E-5 table/narrative inconsistency are all honest and present in the docs, not hidden.

**DECISION: DONE**

**BLOCKING**: 0

**NON_BLOCKING**: 3
1. BENCHMARK.md §18 should say figures were measured at 8d606cc (unchanged through eda0408 since that diff is docs-only) rather than implying measurement at head — wording precision only.
2. bench.yml's `paths` filter doesn't cover doc/CI-only PRs touching D-1 release-profile work; consider a standing policy to bench-gate such PRs regardless of path filter.
3. Full local coverage regeneration (`cargo llvm-cov`) and `bench/selftest.py` were not rerun locally in this review (time-boxed); relied on CI's green run plus independent gate-logic proof — acceptable but noted for completeness.
