# Technical Writer DoD review, Sprint 4 (PR #8, head 9d3dc21)

Role: Technical Writer (validator, read-only apart from this file). Skill: delivery-team:operations.

DECISION: NOT_DONE (3 blocking documentation items; all are text fixes, no code change)

## Verified accurate
- BENCHMARK s16 and README G4a numbers match CI run 35557702662 logs: all 4 cells summary PASS, 10 valid runs per scenario, idle 2.35/2.59/3.91/4.62, gating peak, g6 medians 8.18/13.33/8.85/8.82, redirect-chain5, public-host ratios 0.997 to 1.02, identity ok (commit acddaaeb..., same Cargo.lock hash).
- No overclaiming found on: G4a pass is not the gate closing (README, BENCHMARK, EPICS, sprint plan, E-4 report); single run per cell; g6 shown as a range and RECORDED; 2 MiB vs 5 MiB public-host deviation; image unmeasured; ADR-005 libc choice "NOT made"; A-4 "not Done until E-7 check" (A-4 report, README). MiB units used throughout (kB only as Linux /proc unit, defined).
- README copy-paste example re-run at this head: `--version` prints commit=9d3dc21... ; tools/list, blocked_target and invalid_argument replies match the README text; tool description string matches.

## BLOCKING
1. README says nothing user-facing about conversion limits: error code `converter_limit` (exists in src/error.rs, suggests `raw=true`) is missing from the README "codes today" list; the honest limits (no `<title>`, no markdown escaping of `*`, `_`, `[`, no link-density rule, landmark holdback can drop content outside a small `main`/`article`, untuned noise heuristics, UTF-8 only) live only in the A-4 dev report. Add a short "Conversion limits" paragraph and add `converter_limit` to the list.
2. MPL-2.0 exceptions are not disclosed outside deny.toml and the A-4 report. README and docs/ci-branch-protection.md "Licence" section still say only Apache-2.0 and that "nothing in CI, deny.toml or the benchmark docs depends on the choice"; that is now untrue: four Servo crates (cssparser, cssparser-macros, dtoa-short, selectors, via lol_html) are MPL-2.0 per-crate exceptions, which bears on OQ-7 (still open). Disclose and link the OQ-7 question.
3. BENCHMARK "Read this first" table is stale and self-contradictory: row 1 says CI ran "most recently #5" with "all five required checks" (now #8, a3b-merge-gate is a sixth); the "Product memory gates" row still carries the pre-Sprint-4 note ("The old note follows. Not decided ... before A-4 conversion exists") after the new G4a PASS text. Remove the old note; refresh row 1.

## NON-BLOCKING
1. README status paragraph repeats "the container image has not been measured yet (the runs use bare binaries; D-2 builds the image)" twice in one sentence.
2. README "CI ... pass on pull request #7 (Sprint 3)" is stale; PR #8 CI (run 35557702662 plus the 14-check run) also passes.
3. EPICS A-4 section has no status note that A-4 is not Done pending the E-7 95%/50% check (E-4 has one); add for symmetry.
4. .delivery/state.md is stale: sprint_4 says "no implementation yet" and does not record A-4/E-4 or the G4a result; update at close.
5. A-4 report gap 2 says aarch64 1 MiB overhead "not measured" while section 7 records 59.8 ms from CI; reword to "measured in CI on gnu only, advisory". Section numbers there run 5, 7, 6.
6. README still says the tool description does not mention the A-9 header; it also does not mention conversion or raw; consider a longer description in a later story (A-6).
7. Reader-facing gap: html-escape's internal `unsafe` and lol_html one major behind (A-4 gaps 7) are not in user docs; fine for dev report only.
