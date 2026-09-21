# Sprint 4 fix pass 1 report (PR #8)

Code head f9e4c9d; docs commits follow it. CI run for code head: ci 35562347581, bench 35562347553, arm-bench 35562347560, all success (4 bench-gate cells success).

## Blocking
- B1: drop and landmark rules now run before the open-element cap in `Md::start`. Tests at depth 255/256/300/10000 (converter, whole and chunked, and real stdio). Mutation (old order) killed by them.
- B2: reproduced (1.6 MiB `a a a` tag: about 112 MB VmHWM; 2 MB: 140 MB). Lowering the lol_html limit was rejected (2000 nested `<p>` fail at 128 KiB). Fix: raw pre-scan `convert/tagscan.rs`, ATTR_CAP 1024 attributes per tag, fail closed `converter_limit`, over-approximating (false positive on prose like `a <b` plus 1024 words without `>`; `raw=true` still works). After: bomb about 5.8 MB, 2 MB attribute about 5-7 MiB per fetch. Tests: `tests/hostile_rss.rs` (3 concurrent, peak <= 40 MiB, <= 24 MiB above baseline), real-stdio test, differential test of the scanner against lol_html. Scanner-disabled mutant killed. Bench: `hostile-attrs3`, `hostile-attrvalue3` are RECORDED, not gates (generated pages, no hostile gate in PRD/architecture); they still fail closed on validity. Architecture 5.1 row f updated.
- B3: bench.yml overhead step has `set -euo pipefail`. Proven locally: failing assertion exits 101 with pipefail, 0 without.
- B4: README conversion limits section, `converter_limit`, four MPL-2.0 crates disclosed, OQ-7 STILL OPEN (not decided), ci-branch-protection Licence section rewritten, BENCHMARK Read-this-first refreshed.

## Non-blocking done
README duplicates and stale PR ref, A-4 qualified, EPICS A-4 status and follow-ups and stale refs, state.md (local), A-4 report correction, tool description (conversion, `raw`), `<base href>` gap, build.rs (`-dirty`, `FETCH_MCP_COMMIT`, rerun-if-changed for packed-refs and worktrees), N1 row bound (256 KiB, comment fixed), push-after-finish error, `call_many` stall context, dead `html` param, ADR-002 Accepted with amendment (tier 2 not implemented), M5 test (converter failure aborts reading early; mutation killed).

## G4a re-run (median of 10, MiB), all PASS
See BENCHMARK.md section 16 addendum for the full table. Gating peak: amd64 gnu 6.09, amd64 musl 4.58, arm64 gnu 5.38, arm64 musl 4.62. Idle 4.52 / 2.36 / 3.85 / 2.65. Boundedness at most 1.015. Hostile (RECORDED): hostile-attrs3 5.82 / 4.62 / 5.13 / 5.00; hostile-attrvalue3 15.54 / 14.67 / 14.49 / 15.16. g6-concurrent10 8.87 / 9.52 / 8.10 / 13.16. Overhead 1 MiB p95: 62.7 ms arm64 gnu, 75.0 ms amd64 gnu.

## Verification
Clean `git archive HEAD`: fmt, clippy -D warnings x3, cargo test x3, release build, guard and self-test, cargo deny, bench/selftest.py, actionlint: all pass.

## Gaps
- Single CI run per cell. musl overhead not measured.
- Five items recorded as follow-ups in EPICS (literal `<script>` rule for E-7 check, early stop until A-5, untyped sniffing A-6, lol_html 3.x, musl overhead).
- A-4 is NOT Done: 95%/50% checks wait for E-7 (owner URL lists). OQ-7 open with owner.
- Scanner false positives on unclosed-`<` prose above 1024 words.

## Final cleanup (after DoD round 2 and the PR review round 2; all reviewers approved with 0 blocking)
- bench.yml A-4 p95 step: now fails unless `test result: ok. 1 passed; 0 failed; 0 ignored;` is in the output and a `CONVERT_1MIB bytes=` line exists (no more `|| true`). Proven locally by running the extracted `run:` block: real test rc 0 with the summary line written; test renamed (filter matches nothing, "0 passed") rc 1 "expected exactly 1 test"; measurement line removed rc 1 "no CONVERT_1MIB measurement line". (The `test <name> ... ok` line is not matched: stdout/stderr interleave with --nocapture.)
- `hostile-attrvalue3`: page reduced from 2 MiB - 4 KiB (4,152 B under the cliff) to 1.75 MiB (12.5% margin); selftest asserts <= 95% of the limit. `hostile-attrs3` floor left at 3 x 4 KiB on purpose, documented (a ceiling/served-bytes check is not stable across socket buffering). The BENCHMARK table figures for hostile-attrvalue3 (14.5 to 15.5 MiB) were measured at the old size; the final CI run gives the new ones.
- Docs: README hidden/display:none exemption for optional-end/void tags (also module header); README attribute-guard over-approximation wording with the 0 refusals across 2,509 real HTML files; README known gap for self-nested drop tags past 256 open elements (257 `<nav>`); BENCHMARK `-dirty` best effort; public_check.py live Wikipedia page caveat; the two overhead p95 sets reconciled with run ids (35557702662: 59.9/56.7 ms; 35562347553: 62.7/75.0; 35563535561: 63.9/77.4 on arm64/amd64 gnu) and the variance called unexplained; second run 35563535561 cited, "single run" dropped where a second run exists, no trend claim kept; EPICS stale "run in A-4" wording; deny.toml note for the two unmatched-licence warnings (BSD-2-Clause, CC0-1.0).
- markdown.rs row bound: the sum was over unescaped lengths while stored cells are `|`-escaped, and the comment said 256 KiB; the check now counts escaped bytes with an O(1) running `row_bytes`, comment corrected (stored row <= 256 KiB, about 320 KiB with the cell being collected). New test `a_row_of_pipe_heavy_cells_is_bounded_on_escaped_size`.
- Verification (clean `git archive c017e11`, target dir and TMPDIR under ~/.cache): fmt, clippy -D warnings x3, cargo test --locked x3 (148/149/148 lib tests plus integration), guard and --self-test, cargo deny (advisories, bans, licenses, sources ok), bench/selftest.py, actionlint: all pass.
- CI on c017e11 (run ids 35633732082 ci, 35633732080 bench, 35633732179 bench-gate): all 14 checks pass, including the four bench-gate cells (10/10 valid runs, `invalid: []`, hostile-attrs3 about 5.0 to 6.1 MiB, hostile-attrvalue3 (new 1.75 MiB page) about 12.9 to 16.3 MiB per-sample range) and the hardened A-4 step.
