# Technical Writer review, Sprint 3 (PR #7, head 39379c1)

Role: Technical Writer, read-only validation. Verified against code at head, `gh run view --log` of runs 35538774565 (commit d635a67) and 35539714646 (commit 39379c1), and by running the README stdio example (ids 1, 2, 4, 5; offline) and `python3 bench/selftest.py` (SELFTEST PASSED).

DECISION: NOT_DONE (3 blocking, all stale "not yet measured" statements that now contradict section 15)

## Verified OK
- Section 15 figures match the job logs of run 35538774565 exactly (idle medians, min-max, gating peaks 5394/4776/4476/4236 kB = 5.27/4.66/4.37/4.14 MiB, all `--gate`, verdict PASS). Hosts 8573C and 8370C are both in that run's log.
- No overclaiming in section 15 and the dev report: peak called read-in-full and "NOT a G4a pass", single CI run, not a trend, image not measured, G4b and g6 not run, strict idle gate.
- ci-branch-protection.md: `bench-gate` explicitly "NOT a required check yet", owner decides after runs; `a3b-merge-gate` remains in the owner quickstart; arm-bench jobs still advisory. Cell names match the job names in the runs.
- A-9 behaviour matches code (`with_header`: `URL: <final>\nStatus: <code>\n\n`, only when redirects > 0, outside the max_length window); documented in the dev report (marked a design choice). Copy-paste README example still works at this head (blocked_target, invalid_argument, tools/list output as documented). MiB units consistent (kB rows in glossary defined). Exit 3 wording updated consistently (table, section 6, glossary-adjacent text). arm64 quota caveat honestly stated in the dev report. Sprint plan Revision 10 and EPICS Sprint 3 rows consistent.

## Blocking
1. README.md "What has not been checked" still says "There is no `--gate` run on amd64 or arm64" and "The memory gates ... have not been run". Section 15 records gate runs on both platforms (read-in-full form, not a G4a/G4b pass). README was not touched by the PR. Reword: gate runs exist (idle strict and read-in-full peak, one CI run, section 15); G4a/G4b verdicts are still pending.
2. docs/BENCHMARK.md "Read this first" table contradicts section 15: rows "benchmark scripts ... Never in a `--gate` run", "Native aarch64 ... No `--gate` run exists yet", "Native amd64 ... NOT YET MEASURED on a hosted amd64 runner", "Product memory gates ... NOT YET RUN". These are the first thing a reader sees. Update to point at section 15 with the same caveats.
3. docs/BENCHMARK.md section 3 host table still shows the amd64 column as "NOT YET MEASURED" (all rows) and the "Measured?" arm64 row as "advisory spike only"; the glossary and section 7 still say only gnu is measured / musl not built for the product, but bench.yml now builds and gates gnu and musl. Fill amd64 facts from run logs (Ubuntu 24.04.5, Xeon, 4 KiB pages) and fix the gnu/musl statements.

## Non-blocking
1. The A-9 header format (`URL:`/`Status:` lines, blank line) is not documented in README. The README's tools/list description quote and "What to expect" say nothing about it, and the "Not yet done" list omits it; the tool description string is unchanged ("as markdown"). Add one README bullet (format, only after a redirect, outside max_length) and note it in the id-3 expectation. The EPICS A-9 story is not marked done.
2. Section 15 cites only run 35538774565 (commit d635a67), but a second run exists at the head commit (35539714646, all four cells PASS). It differs slightly (amd64 gnu idle 3.92, peaks 5294/4766/4348/4300 kB, amd64 host Xeon 6973P-C). Add one line: two runs, both pass, small variance; the dev report says "only one CI run" which is now outdated.
3. Section 15 states 8573C and 8370C only; run 2 saw 6973P-C. Update if run 2 is cited.
4. Section 15 title says "CI run ... PR #7"; add the commit SHA the run measured (d635a67, not the head).
5. Quickstart step 5 still describes the identity check as "aarch64 for a gating run" and "reached only on aarch64"; native x86_64 is now also valid.
6. Quickstart line 141 still says "E-8 marker ... not built yet" (acceptable, matches dev report gap 4).
7. Nothing verified the arm64 free-quota claim beyond jobs running; dev report says so honestly, but ci-branch-protection.md gives no quota caveat (16 min x 4 cells per run, nightly plus main pushes and PRs).
8. Container image not measured: stated in section 15 and dev report; repeat in README status if item 1 is reworded.

REPORT: /var/home/meconnelly/Documents/GitHub/Fetch/.delivery/artifacts/06-dev/sprint-3/dod/tech-writer-review.md
