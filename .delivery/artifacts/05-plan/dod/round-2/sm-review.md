# Scrum Master review, Stage 5 round 2 (sprint feasibility)

SKILL_LOADED: product-delivery. Verdict: DONE (0 blocking, 3 non-blocking).

## Recomputed points (from docs/EPICS.md story sizes)
| Sprint | Stories | Pts |
|---|---|---|
| 0 | E-1 3, A-1 3, D-7 2 | 8 |
| 1 | A-2 3, A-3a 5 | 8 |
| 2 | A-3b 5, E-7 2 | 7 |
| 3 | E-2 5, E-3 2, A-9 1 | 8 |
| 4 | A-4 5, E-4 3 | 8 |
| 5 | A-5 3, A-6 3 | 6 |
| 6 | A-7 3, B-1 5 | 8 |
| 7 | B-3 3, B-2 3, E-5 2 | 8 |
| 8 | B-5 3, D-1 2, D-5 1 | 6 |
| 9 | D-2 5, D-4 2 | 7 |
| 10 | C-1 3, C-2 2, C-3 2 | 7 |
| 11 | B-4 3, A-8 2, D-3 3 | 8 |
| 12 | B-6 2, E-6 3, D-6 2 | 7 |
Total 96. No sprint above 8. Matches plan and EPICS tables.

MVP list (22 stories) sums to 72; deferred (11 stories) sums to 24; 72+24=96. Sprints 0-9 sum to 74, minus non-MVP A-9 (1) and D-5 (1) = 72, so MVP at end of Sprint 9 holds. v1.0 at Sprint 12 holds (all stories placed).

## Dependency order
Checked every story against EPICS story maps. No story precedes a dependency. CI: D-7 in Sprint 0 precedes all required-check use (A-3a release-guard in S1, D-2 reuse in S9). D-1 (S8) before D-2 (S9). B-1 (S6) before B-3/B-2 (S7) before B-5 (S8). E-7 (S2) before A-4 (S4). E-2/E-3/E-4 order consistent with G4. D-2 (S9) before D-3 (S11), E-6 (S12), D-6 (S12).
Same-sprint dependencies, all sequenced or fallback stated: E-4 after A-4 (S4, stated); E-3 after E-2 (S3); C-2 after C-1 (S10); D-4 after D-2 (S9); E-7 with A-3b (S2, EPICS allows curl).

## Gates, entry/exit, risks
Sprints 0-4 each have goal, dependencies, entry, exit; gates G0, G4, plus S1, S2, S3 gates present, measurable. Risks table lists ARM runner (with sprint-by-sprint blocks and 3/4/9/12 checks), OQ-5 before Sprint 2 (Entry of S2 and risks), OQ-3/4/7 due sprints. ARM runner and QEMU rule explicit.

## Findings
1. Non-blocking: Risks table row "A-3b estimate" says "Sprint 2 committed at 6; overflow takes A-9 out first". Stale after revision 1: Sprint 2 is 7 pts and A-9 is in Sprint 3; Sprint 2 text says E-7 slips first. Fix wording.
2. Non-blocking: Sprints 5-12 lack per-sprint entry/exit criteria (acknowledged in revision log; required only for 0-4). Refine before Sprint 5.
3. Non-blocking: Sprint 9 lists D-4 depending on D-2 in the same sprint with no stated in-sprint order; add "D-2 first". Sprint 2 (7) and Sprint 12 (7) carry only 1 pt slack under the 8 ceiling; Sprint 11 has D-3 needing the ARM runner at ceiling.
