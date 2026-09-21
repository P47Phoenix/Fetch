# QA + docs review, DoD round 2, Sprint 4, PR #8 (head 3e02c70)

Decision: DONE. Blocking: 0. Non-blocking: 4. Read-only review; builds in `~/.cache/qa2-*` from a clean `git archive HEAD`.

## 1. Round-1 tech-writer blockers
- README "conversion limits" now covers: `converter_limit` (in the error-code list, with causes and the `raw=true` retry), no title, nothing escaped or neutralised (OQ-5 passthrough, ESC/RLO not removed), no link-density rule (ADR-002 tier 2 not implemented), landmark holdback loss, UTF-8 only, literal `<script>` text and the 1,024-attribute fail-closed. CLOSED.
- MPL-2.0: README names the four crates (`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`), states OQ-7 is still open and not decided, and points at `docs/ci-branch-protection.md` "Licence (OQ-7 still open)", which has versions, the MPL 3.2/3.3/3.4 consequences and the ways to avoid MPL. CLOSED.
- BENCHMARK "Read this first" table: row 1 now says #8 and six checks; the old "not decided ... before conversion exists" note is gone. CLOSED.
- Grep for `PR #7`, `five required checks`, `not decided before conversion exists` outside historical Sprint 3 artifacts: no stale hits (remaining `PR #7` hits are Sprint 3 records and BENCHMARK section 15, which is correctly about PR #7).
- EPICS A-4: status note "implemented, NOT Done", pending E-7, five follow-ups listed. CLOSED. Slightly stale but qualified: EPICS line 133 still says the checks run "in A-4 (Sprint 4)", then says they slipped until E-7 lands.

## 2. Figures versus CI logs (`gh run view --log`, both runs parsed)
- Run 35562347553 (f9e4c9d): every cell, every scenario in BENCHMARK s16 fix-pass table matches the log exactly (idle 4.52/2.36/3.85/2.65; peaks; gating peak 6.09/4.58/5.38/4.62; g6 8.87/9.52/8.10/13.16; hostile-attrs3 5.82/4.62/5.13/5.00; hostile-attrvalue3 15.54/14.67/14.49/15.16). 10 valid of 10 runs everywhere. Boundedness recomputed from the medians (for example 5.45/5.72 = 0.953; 5.75/5.72 = 1.006): matches.
- Run 35563535561 (3e02c70): all four cells PASS, `invalid: []`; values within about 1 MiB of the previous run (hostile-attrvalue3 amd64 gnu 16.22, amd64 musl 15.64, arm64 musl 14.81; g6 arm64 musl 12.88). Not cited in the docs; consistent with them.
- Labelling: hostile-attrs3, hostile-attrvalue3 and g6-concurrent10 are `RECORDED` in the log and in the docs; gating peak is the max of the five G4a medians; idle is the shipped binary. Honest.
- Overhead p95 from the logs: 75.0 ms amd64 gnu, 62.7 ms arm64 gnu (35562347553); 77.4 and 63.9 ms (35563535561). Docs' 62.7/75.0 are correct. The 59.9/56.7 ms figures in the ADR-005 allocator paragraph come from round-1 run 35557702662 (before the tag scanner). Not reconciled in the text (NB1).

## 3. Regression tests and mutations
- B1 mutant (old order: open-cap return before the drop rules): `drop_rules_hold_at_any_depth` and `landmark_is_found_past_the_open_element_cap` FAIL. Killed.
- B2 mutant (`scan.feed` check disabled): `too_many_attributes_fail_closed_and_the_cap_converts`, `converter_failure_stops_reading_the_body_early` and `tests/hostile_rss.rs` FAIL. With the refusal assertion also removed, the RSS assertions alone trip: peak 391 MiB (+384 MiB) on the first scenario. So the test is meaningful on memory, not only on the result.
- B3: the real `run:` block of the overhead step was extracted from bench.yml and run with the cargo command replaced by `exit 101` behind `tee`: rc 101 with the actual `set -euo pipefail`, rc 0 with `set -eu`. The step really fails. The test itself asserts `p95 <= 500`.
- `tests/hostile_rss.rs` repeated 5x: 5/5 pass, worst delta 16 MiB every time (bound 24 above baseline, 40 absolute), 1.14 s. Deterministic, not flaky. Margin to the 24 MiB bound is 8 MiB (NB3).

## 4. Non-blocking items claimed fixed
Spot-checked in code and docs: README duplicates and stale ref, A-4 qualified, EPICS status, tool description and `raw`, build.rs `-dirty` and `FETCH_MCP_COMMIT` (documented in BENCHMARK), N1 row bound (`ROW_BYTES` 256 KiB with a test), push-after-finish error (`finished` flag, test passes), dead param, ADR-002 amendment, M5 test (mutation killed above). No discrepancy found. Deferrals: five follow-ups are listed in EPICS A-4 and match the fix report; the report's Gaps section is candid (single run, musl overhead, A-4 not Done, OQ-7 open, scanner false positives, also documented in README).

## 5. Clean archive verification
fmt OK; clippy -D warnings x3 (default, bench-loopback, test-support) OK; cargo test --locked x3 all pass (147, 148, 147 unit tests plus integration, hostile_rss included); release build OK; `check-release-features.sh` "guard OK" and `--self-test` "self-test OK"; `cargo deny check` advisories/bans/licenses/sources ok; `bench/selftest.py` SELFTEST PASSED; actionlint on the workflows exit 0. `gh pr checks 8`: all 14 checks pass at head (fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench x4, bench-gate x4 cells, bench-product x2).

## Non-blocking
1. NB1: BENCHMARK ADR-005 paragraph gives 59.9/56.7 ms with no run id, and s16 gives 62.7/75.0 ms; amd64 rose 32% (56.7 to 75.0) and nothing says why (tag scanner cost or runner CPU variance). Add the run ids and one sentence.
2. NB2: docs say "single run per cell"; a second bench run at 3e02c70 now exists and agrees within about 1 MiB. Optionally cite it.
3. NB3: hostile_rss margin is 16 MiB of 24; adequate, but watch it if the fixtures change.
4. NB4: `cargo deny` prints a warning "unmatched license allowance" (does not fail); worth a look when deny.toml is next touched. EPICS line 133 wording is slightly stale.
