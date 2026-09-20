# Scrum Master DoD review, Stage 5 Plan, round 3

Scope: sprint feasibility. Artifacts: sprint-plan.md, docs/EPICS.md. Not edited.

## Recomputed points (from EPICS story maps)
| S | Stories | Pts |
|---|---|---|
| 0 | E-1 3, A-1 3, D-7 2 | 8 |
| 1 | A-2 3, A-3a 5 | 8 |
| 2 | A-3b 5, E-7 2, E-8 1 | 8 |
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
Total 97 pts, 34 stories (3+2+3+3+2+2+2+3+3+2+3+3+3). No sprint above 8. Matches plan and EPICS.

## MVP / v1.0
Sprints 0-9 sum 75, minus A-9 and D-5 = 73 (matches). Deferred 24 pts (A-8, A-9, C-1..3, B-4, B-6, D-3, D-5, D-6, E-6) = 97 - 73. MVP end of S9, v1.0 S12: confirmed. All MVP dependencies (D-2 needs D-1 S8, D-7; D-4 needs D-2) land earlier or in-sprint with stated order.

## Dependency order
Checked every story's EPICS dependencies against its sprint: all valid. In-sprint orderings (E-7 after A-3b in S2; A-4 before E-4 in S4; E-2 before E-3 in S3; C-1 before C-2/C-3 in S10; D-2 before D-4 in S9) are stated or implied. No backward dependencies.

## Entry/exit/gates
S0-S4 have goals, entry, exit, gates (G0, G4a). S5 has entry (G4a passed, runner) and exit (G4b) in the summary table and gate text. Sprints 6-12 detail deferred by stated plan (acceptable outside 0-5).

## Risks / external dependencies
ARM runner listed as hard dependency with per-sprint checks and risk row; OQ-5 due before Sprint 2 in flags, S2 entry, risks, EPICS. OQ-3/4/7 due sprints consistent.

## Internal consistency
No stale rows found; numbers (97/73/34, G4a/G4b, E-8 in S2) agree across plan and EPICS.

## Findings
Blocking: none.
Non-blocking:
1. Sprint 5 lacks its own goal/dependency/exit block (only table row + G4b text); expand before Sprint 5.
2. Sprint 2 and Sprint 3 zero slack; overflow rule covers S2 only; S3 relies on A-9 drop.
3. E-8 remains PROPOSED and gates S2 entry/E-2; needs owner confirmation and OQ-5 decision before S2.
