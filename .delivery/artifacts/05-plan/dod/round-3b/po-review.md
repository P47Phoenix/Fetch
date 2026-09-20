# PO DoD review, Stage 5 Plan, round 3b

STATUS: DONE (0 blocking, 5 non-blocking)

## Checks
- G4a (end S4): every scenario needs only A-3b (S2), A-4 (S4, first in sprint), E-2/E-3 (S3), E-8 (S2), E-7 not required. No window, early-stop or raw dependency. PASS.
- G4b (end S5): A-5 and A-6 (S5) own window/early-stop/raw scenarios; gate blocks S6. PASS.
- Recount: EPICS epics E 8 stories/21 pts, A 10/33, B 6/19, C 3/7, D 7/17 = 34 stories, 97 pts. Sprint sums 8,8,8,8,8,6,8,8,6,7,7,8,7 = 97; none above 8. MVP list (23 stories) = 73 pts; deferred 11 stories = 24 pts. Matches plan, PRD, EPICS.
- MVP in S9: D-2 (deps A-1, D-1 S8, D-7, OQ-7 before S9) and D-4 (D-2) in S9; B-5 S8 after B-1 S6, B-2/B-3 S7; E-5 S7 after E-3/E-4/A-7. No forward dependency found.
- FR-01..16, NFR-01..15 (NFR-03 superseded), Goals 1a-6 all trace to stories with Given/When/Then ACs.
- A-3a/A-3b split and E-4 size rule consistent across plan, EPICS, PRD (FR-07, section 1).
- Milestones M0-M5 and OQ due dates (OQ-5 before S2, OQ-3 and OQ-4 before S10, OQ-7 before S9) consistent across the three files. OQ-3/4/5/7 remain Open. E-8 recorded as user-confirmed and stated not to decide OQ-4.

## Non-blocking
1. EPICS A-3b AC and plan Sprint 2 DoD say the "full memory gate is E-3/E-4 in Sprints 3-4"; complete gate closes end of S5 (G4b). Stale wording.
2. EPICS "Suggested Sprint Order" tail suggests moving D-1 into S7 and D-2 into S8; S7 would reach 10 pts (above ceiling) and D-2 would precede M3 close. Stale option.
3. EPICS says v1.0 moved to S12 "because the re-estimate added 5 points"; total has since grown by 9 (historical, imprecise).
4. C-1 "placeholder default" for the robots toggle is a shipped behaviour in S10 while OQ-3 is due before S10; C-1 story map does not list OQ-3 as dependency. Clarify that the placeholder is not documented as the default.
5. D-1 re-measure needs the E-2 harness, not listed in D-1 dependencies. Also who tags the MVP (D-2 "tag push") vs D-6 as v1.0 tagger is implied, not stated.
