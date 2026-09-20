# PO DoD Review - Stage 5 (Plan)

STATUS: NOT_DONE (2 blocking findings)

Checked: sprint-plan.md, docs/EPICS.md (31 stories), docs/PRD.md v0.4.

## Verified OK
- Counts: 31 stories (E6, A10, B6, C3, D6); story points sum to 92; sprint sums 6,8,6,7,8,6,8,8,6,7,7,8,7 = 92; MVP list sums to 68; deferred list sums to 24. No sprint over 8.
- Traceability: FR-01..FR-16 and NFR-01, 02, 04-15 each map to at least one story (NFR-03 superseded). Goals 1a-1d, 3, 4, 5, 6 have owning stories.
- E-4 size rule (Content-Length -> too_large; chunked succeeds if window fits, else too_large) is applied consistently in FR-07, A-3b, E-4, US-7.
- A-3a/A-3b split is consistent (merge gate, four-refusal AC in A-3b, dependencies, sprints).
- No stale "concurrency 4"; 200,000 appears only as a labelled historical note in C-3; 30 stories absent; A-3 mentions are historical split notes only.
- Safety before release: A-3a in Sprint 1, B-5/M3 in Sprint 8, no tag before M3; OQ-5 due before Sprint 2 and OQ-3/4/7 left open, not silently decided.

## Blocking
1. PRD section 9 milestones and section 10 OQ due dates are stale versus the plan. PRD says M1=Sprint 1 with text fetched in Claude Code (A-3b is Sprint 2), M2=Sprints 2-4 (A-5/A-6/A-7 are Sprints 5-6), M3 (Safety)=Sprints 5-6 (plan: reached in Sprint 8), M5=Sprints 10-11 (plan: Sprint 12). OQ-4 is "Before M3" (Sprint 5-6 by the PRD) but the plan defers it to Sprint 10; OQ-3 "Before M4" vs Sprint 11. Plan and EPICS invoke "M3" without reconciling. Fix: re-map M0-M5 to the 13-sprint order and restate OQ due dates in sprint terms (OQ-5 before S2, OQ-4 before S10, OQ-3 before S11, OQ-7 before S12).
2. D-2 (Sprint 9, MVP) requires the aarch64 test and memory-gate jobs to be required status checks the release workflow depends on, but those jobs are created by D-3 (Sprint 11) and E-6 (Sprint 12). D-2 cannot meet that AC at Sprint 9, so MVP "delivered when claimed" is incoherent (and D-3 depends on D-2). Fix: move that AC to D-3/E-6, or state MVP release relies on the documented manual bench run and E-5 until then.

## Non-blocking
- D-4 (Sprint 9, MVP) documents config errors exiting before handshake and env-var behaviour that C-1 delivers in Sprint 10; either scope D-4's text to defaults or re-check docs in C-1.
- Goal 2 (>= 95% success on the 50-URL set) has no story-level AC; only the Epic A success metric mentions it. Add an AC (A-4 or E-1 follow-up).
- C-3 AC "larger body -> size error at 1 MB" omits the accepted chunked in-window success case; align wording with the size rule.
- A-2 last AC (register in Claude Code, Sprint 1) sits awkwardly with the release rule "pre-M3 builds not registered in a real MCP client"; the Sprint 2 throwaway-config note handles it, so mirror that in A-2.
- Sprint plan risk table records OQ-5 "default assumption: label"; acceptable as flagged, but make sure it is not treated as a decision.
- Sprint plan line 24 ("keep their sprints or close to them") is vague; EPICS says exactly E-2/E-3/E-4 keep Sprints 3/3/4.
