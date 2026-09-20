# Tech Writer review round 2, A-3b, PR #6 head 5c9a0dd (read-only)

DECISION: NOT_DONE (1 blocking: dangling reference to a BENCHMARK section that does not exist)

## Round-1 items
- Blocking 1 (TLS caveat): FIXED. README line 13 and docs/ci-branch-protection.md line 100 say TLS validation has no automated test, checked by hand.
- Blocking 2 (a3b-merge-gate wording): FIXED. ci-branch-protection.md line 100 now says "checks", most is behavioural, part is a heuristic source scan, "not a proof".
- NB1 architecture OQ-5: FIXED in the decisions table (l.28) and OQ table (l.369) "RESOLVED". Render-hook lines 62/464 still say "blocked by OQ-5" (l.464) and l.531 says OQ-5 remains OPEN (historical section, ok); l.464 is minor.
- NB2 out-of-order replies: FIXED (README l.69).
- NB3 substitute RSS smoke: FIXED (README l.19 states x86_64 5.7 MiB substitute, non-AC). Matches dev-report (VmHWM 5760 kB).
- NB4 "as markdown" note: FIXED (README l.72).

## Verified
- CI: run 35523134933 at head 5c9a0dd, all arm-bench jobs success, incl. bench-product-peak (ubuntu-24.04-arm, 5 MiB fetch, "peak exit=0"). ci run 35523134918 success.
- README stdio example runs at this head (initialize, tools/list, blocked_target loopback text).
- EPICS: E-8 acceptance items re-homed to E-2 (l.78) and G4a/E-4 (l.97-101), consistent.
- BENCHMARK edits (skeleton row, what-you-can-run) accurate.

## BLOCKING
1. README line 19 says the arm64 RSS smoke is recorded in "BENCHMARK.md section 14". There is no section 14 (file ends at section 13) and no arm64 peak number anywhere in docs. The task's "BENCHMARK arm64 numbers" are absent. The figure lives only in that job's step summary (not in the log, so I could not verify a number). Either add section 14 (advisory, single run, gnu, ubuntu-24.04-arm, VmHWM median/min-max copied from the job summary, not a gate) or remove the pointer and say the figure is in the CI job summary.

## NON_BLOCKING
1. BENCHMARK section 13 (l.437) still says the measured build "has no HTTP client, TLS" and "No fetch scenario has been run on the product"; the section-1 caveat covers it but add "(A-2 build, superseded by A-3b)" beside the l.450 claim "peak is completely unmeasured" now that an advisory arm64 peak run exists.
2. architecture.md l.464 (B-6 "blocked by OQ-5") should read "won't-do, OQ-5 no label".
3. ADR/dependency-ledger note that roots are embedded webpki-roots (no system trust store): ADR-001 covers it; I found no separate ledger note added in this diff. Confirm it was intended.
