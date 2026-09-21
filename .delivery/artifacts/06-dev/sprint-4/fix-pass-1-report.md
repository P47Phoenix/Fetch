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
