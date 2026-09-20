# PO DoD review, Stage 5 Plan, round 3

Status: NOT_DONE (1 blocking, 4 non-blocking)

## Verified OK
- Counts: 34 stories (A10, B6, C3, D7, E8), 97 pts, per-sprint sums 8,8,8,8,8,6,8,8,6,7,7,8,7 = 97, none over 8.
- MVP 73 pts recomputed from the 23-story list; post-MVP 24; Sprints 0-9 = 75 minus A-9 and D-5 = 73. PRD/EPICS/plan agree.
- FR-01..16 and NFR-01..15 all trace to stories with testable ACs (NFR-03 superseded). Goals 1a-1d, 2-6 covered (Goal 6 by D-4, Goal 1d by E-5/E-6).
- A-3a/A-3b split consistent (5+5, merge gate, four-refusal AC in A-3b, Sprint 1 at 8). E-4 size rule (three cases) consistent across PRD FR-07, A-3b, E-4, C-3.
- Milestones M0-M5 and OQ due dates (OQ-5 before S2, OQ-4 before S10, OQ-3 before S11, OQ-7 before S9) match sprint order in PRD, EPICS, plan.
- OQ-3/4/5/7 remain open. E-8 is compile-time only, explicitly rejects the runtime switch because it overlaps OQ-4, and states it does not decide OQ-4. No smuggled decision.

## Blocking
B1. G4a claims scenarios that need windowing delivered in A-5 (Sprint 5). The size rule ("succeed if the requested window completes under the cap"), early stop, "window at start", "late-landmark holdback-full", "window beyond the cap" and "chunked in-cap succeeds" all need the `Window` sink (`convert::window`, start_index/max_length, "done" detection). Architecture section 15 maps A-5 to `convert::window`. A-3b (S2) and A-4 (S4) have no AC delivering it, and A-5 (S5) is after G4a (end S4). So G4a is not achievable as written. Fix: either pull a minimal Window (max_length/start_index, done, early stop) into A-3b/A-4 with an explicit AC and re-check points, or move the window-dependent scenarios to G4b and keep G4a to full-consumption ones. State which; do not silently reinterpret "window" as byte-level.

## Non-blocking
N1. E-5 report and release decision (S7) precede D-1 release profile (S8: LTO, opt-level, panic=abort), so reported idle/peak are not from the shipping profile. Add a re-run of the gate on the D-1 profile before the MVP release (S9) rather than relying only on the manual bench run.
N2. C-1 (S10) sets the robots default "per OQ-3" but OQ-3 is due before S11. State that C-1 parses the variable with no default decision (placeholder), so OQ-3 is not decided by C-1; EPICS Open Items lists "C-1 default" as blocked by OQ-3 with a later due date.
N3. E-8 user confirmation (needed before S2; G4a peak depends on it) is only in plan flags, not in EPICS Open Items or PRD section 10 with an owner and due date. Add an entry so the confirmation is tracked; G4a wording "peak on bench build" is otherwise contingent.
N4. D-2 AC asserts absence of `test-support` and "the bench fixture-CA feature" but not `bench-loopback`; align with D-7/E-8 wording.
