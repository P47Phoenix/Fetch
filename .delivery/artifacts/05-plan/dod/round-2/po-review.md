# Stage 5 Plan DoD review, round 2: Product Owner (value and scope)

Artifacts: `.delivery/artifacts/05-plan/po/sprint-plan.md`, `docs/EPICS.md`, `docs/PRD.md` v0.4.
Verdict: NOT_DONE (2 blocking, 6 non-blocking). Round 1 PO findings (milestone remap, OQ dues, D-2 checks) are resolved.

## Verified OK
- Arithmetic: 33 stories (E7, A10, B6, C3, D7); 96 points; per-sprint sums 8,8,7,8,8,6,8,8,6,7,7,8,7 all <= 8; MVP list sums to 72; deferred list sums to 24. No bare A-3 in live text (only historical split notes); 100,000 cap consistent (200,000 appears only as history); story counts and totals consistent across the three files.
- E-4 size rule (Content-Length -> too_large; chunked in-window succeeds; chunked past cap -> too_large) is identical in PRD Decisions, FR-07, US-7, A-3b, C-3, E-4, G4.
- A-3a/A-3b split consistent: Sprint 1 = A-2 + A-3a (8), Sprint 2 = A-3b + E-7 (7); merge gate present; four-refusal AC lives in A-3b; B-1..B-3 framed as depth on A-3a/A-3b.
- MVP dependencies met by Sprint 9: D-2 (D-1 S8, D-7 S0, A-1, OQ-7 before S9), D-4 (D-2, E-5 S7), E-5 (E-3 S3, E-4 S4, A-7 S6), B-1..B-5, E-7 before A-4. Non-MVP dependencies (C-2 after C-1, D-6 after D-3, E-6 after D-2, B-6 after A-4) also ordered correctly.
- PRD milestones M0-M5 and OQ dues (OQ-5 before S2, OQ-4 before S10, OQ-3 before S11, OQ-7 before S9) match sprint order and EPICS "Open Items". OQ-3/4/5/7 are shown Open and explicitly not decided (OQ-5 working default flagged "NOT a decision").
- FR/NFR traceability: every FR-01..16 and NFR-01..15 (NFR-03 superseded) maps to a story with testable AC. Goals 1a-1d, 4, 6 mapped (E-3, E-4, E-2/E-5/E-6, B-5, D-4).

## Blocking

B1. Memory gate G4 (end of Sprint 4) depends on features scheduled in Sprint 5.
E-2 gating scenarios (architecture 11.1) include G1 "window at end" (pagination, A-5, S5), G3 `raw=true` (A-6, S5), and window-beyond-cap. G4 is defined as max of per-scenario medians over that set, but E-4 depends only on A-3b/A-4, and A-5/A-6 land in Sprint 5 (non-gating re-run only). At S4 the full gating set cannot run, so "memory gate passed, continue" (and PRD Goal 1 go/no-go, decision rule) is claimed before it is fully evaluable. Fix: state which scenarios G4 covers at S4 (raw is lower-memory per architecture S3 3.1 vs S2 6.3 MiB, so risk is small) and make the Sprint 5 end re-run of G1/G3 a gating item (or move a minimal window/raw slice earlier); reflect in PRD M2 and EPICS decision gate 2.

B2. Stale Sprint 2 reference in sprint-plan Risks table (line 125): "Sprint 2 committed at 6; overflow takes A-9 out first". After revision 1 Sprint 2 is 7 points (A-3b + E-7), A-9 is in Sprint 3, and the plan text (Sprint 2 Gate) says E-7 slips first. Contradicts the plan body; correct the row.

## Non-blocking

N1. Goal 2 wording drift: PRD Goal 2 still reads "success rate on 50-URL set (static HTML, JSON, plain text, redirects)", but E-1/A-4 measure offline HTML conversion success only, at Sprint 4, before A-6 (JSON/text) and B-3 (redirects). No story runs the 10-URL live smoke or a post-Sprint-7 end-to-end success rate. Align the PRD Goal 2 metric text with the accepted architecture definition and name an owner story for the live smoke (E-5 is a candidate).
N2. Goal 3/2 testability: A-4 does not define the tokenizer/token-count method, the baseline (raw HTML?), or "converts successfully" (non-empty output? no error?). Define in E-1/A-4 so 95%/50% are reproducible.
N3. Goal 5 (p95 overhead < 1 MB <= 500 ms) is only covered as conversion time (A-4/NFR-02); no AC measures end-to-end overhead excluding remote time. Add a line to E-5 or A-4 or reword Goal 5.
N4. PRD FR-11 says robots check "enabled by default" while OQ-3 (default on or off) is open; C-1 (S10) also needs a robots default before OQ-3 is due (before S11). Reword FR-11 to "default per OQ-3" so the OQ is not pre-decided.
N5. A-3b (Sprint 2) ACs reference `FETCH_MAX_CONCURRENCY` and `FETCH_TIMEOUT_MS`, whose parsing is C-1 (Sprint 10, post-MVP). State they are compiled defaults until C-1.
N6. EPICS story-map "Priority" columns are stale legacy ranks (e.g. A-9 =12 but Sprint 3, D-1 =12 but Sprint 8, C-1 =11 but Sprint 10, D-6 =15 but Sprint 12). Drop or regenerate from the sprint table.
