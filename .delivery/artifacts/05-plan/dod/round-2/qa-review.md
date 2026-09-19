# QA Review, Stage 5 Plan, Round 2

Reviewer: QA Engineer (delivery-team:quality). Scope: testability of `.delivery/artifacts/05-plan/po/sprint-plan.md` and `docs/EPICS.md`.
Status: NOT_DONE (1 blocking, 6 non-blocking). Round-1 QA blockers (B1 snapshots and 95%/50% checks, B2 A-3b benchmark DoD) are resolved by E-7, the A-4 AC and the DoD scoping.

## Gate checklist

| Criterion | Result |
|---|---|
| Every story has verifiable ACs | Pass for Sprints 0-4 stories (Given/When/Then with numeric or binary outcomes). |
| Per-story DoD achievable in scheduled sprint | Pass except the loopback issue in B1 (E-2 to E-4 measure a binary that cannot reach the fixture server). |
| SSRF suite scheduled | Pass: A-3a unit S1, A-3b integration S2, B-1 S6, B-2/B-3 S7, B-5 suite + 90% coverage gate S8. |
| 50-URL snapshots, 95%/50% | Pass: E-7 S2 captures with sha256 manifest, A-4 S4 runs both checks. |
| Memory gate | Scheduled (E-3 S3, E-4 and G4 S4). Blocked in practice by B1. |
| ARM runner dependency | Mostly handled (see NB3). |
| A-3b cannot land without A-3a | Pass with caveat (NB5): procedural, backed by the four-refusal integration test and the required feature-guard check. |
| Go/no-go measurable | G0 and G4 numeric, median of 10 valid runs, MB definition from E-1. Minor gap NB4. |
| QEMU RSS never gating | Pass: E-2 preflight refuses QEMU, G4 states it, D-3 fallback tests only, E-6 blocks without a native runner. |

## Blocking

### B1. No story defines how the gating binary reaches the loopback fixture server
The default `Policy` is fail-closed and blocks loopback (A-3a, ADR-003). The only loopback route is the `test-support` constructor, and the release build must not contain it (D-7 guard, architecture 9 R12). E-2, E-3, E-4 and G4 measure the gnu and musl release binaries against a fixture server on 127.0.0.1 (architecture 11.2: same host over loopback). Architecture 11 item 7 only defines a bench-only fixture-CA feature for TLS; nothing defines a bench-only loopback or policy hook, or says the measured binary is the shipped one. As written, either every fetch scenario returns `blocked_target` (no valid samples, report INVALID), or the measured binary contains test code the guard forbids, so the memory gate does not measure the release artifact.
Required fix: decide in E-1 (Sprint 0) and add ACs to E-2/E-4 and the D-7 guard stating how loopback is permitted for benchmarking. Options: a bench-only feature that is absent from the shipped release profile and asserted absent by the guard, with the shipped binary's idle RSS and one non-loopback smoke cross-checked (delta recorded); or fixture host on a non-loopback allowlisted name if OQ-4 ships. State which binary G4 evaluates.

## Non-blocking

- NB1. Sprint 2 overflow rule is arithmetically wrong. "E-7 slips to Sprint 3 in place of A-9" gives E-2 5 + E-3 2 + E-7 2 = 9 points, above the 8 ceiling. Also the risk table says "Sprint 2 committed at 6" while the sprint is 7 points. Fix the numbers or drop E-3 or E-7 to Sprint 4 explicitly.
- NB2. A-4 (S4) depends on E-7 (S2), and the 95%/50% exit depends on the E-1 URL list; if E-7 slips, A-4 exit is untestable. Add "E-7 merged" to Sprint 4 entry.
- NB3. ARM runner availability is checked only at S3, 4, 9, 12. Stories with the native-benchmark DoD rule or runner-dependent ACs sit in S5 (A-5, A-6), S7 (E-5 report), S10 (C-3), S11 (D-3, A-8). Add runner checks (or a note that S5 end re-run needs it) for those sprints.
- NB4. G0 wording omits "median of 10" and the MB definition. A-1 spike measured before E-1 fixes the protocol; state that G0 uses the E-1 protocol or record its deviation.
- NB5. The A-3b merge gate is a PR procedure. A-3b's "dials only the validated IP set" has no explicit test (an injectable resolver returning a different answer on a second lookup); the four-refusal test does not prove it. Rebinding depth waits until B-1 (S6), leaving S2-S6 with an unproven dial-once property. Add one such test to A-3b.
- NB6. A-2's Claude Code "fetch appears" check and the S2 RSS smoke are manual and non-gating; acceptable, but record evidence in the PR as the plan says.

## Summary
Testability is largely sound and round-1 gaps are closed. One blocking gap (B1) makes the memory gate unexecutable as specified; fix before Stage 6.
