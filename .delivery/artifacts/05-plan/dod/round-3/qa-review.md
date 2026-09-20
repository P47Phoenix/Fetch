# QA Review, Stage 5 Plan, round 3

STATUS: NOT_DONE (1 blocking, 5 non-blocking)

## Criteria checked
| Criterion | Result |
|---|---|
| Verifiable ACs per story (Sprints 0-4) | Pass. A-3a, A-3b, E-2, E-3, E-4, E-7, E-8 ACs are Given/When/Then with measurable outcomes. |
| Per-story DoD achievable in its sprint | Pass with N1. Benchmark DoD for A-3b scoped to a manual smoke; A-4/E-4 use the Sprint 3 harness; E-8 exempt. |
| Harness reaches fixture server; shipped binary fail-closed | Pass. Compile-time `bench-loopback` (loopback only, other ranges still refused), off by default, absent from release via D-7 guard (features + marker string); harness refuses peak on unmarked binary and idle on marked binary; A-3b four-refusal test runs on the shipped-profile build. Depends on owner confirming E-8 (PROPOSED) before Sprint 2. |
| Bench vs shipped justified with deltas | Partial, see B1 and N2. |
| SSRF suite, 50-URL snapshots, 95%/50%, G4a/G4b scheduled; ARM dependency | Pass. E-7 S2 (fallback defined), 95%/50% in A-4 S4, G4a end S4, G4b end S5, B-5 S8 (90% coverage), runner checks at S3,4,5,7,9,10,11,12. |
| A-3b cannot land without A-3a | Pass. Merge gate has 4 testable conditions; A-3a is merged in Sprint 1 before the Sprint 2 branch exists; client must call `check_url`/resolver filter, and the dial-once and four-refusal tests fail otherwise. See N3. |
| Thresholds measurable | Pass. 10 MB idle (30 s after tools/list), 40 MB peak VmHWM, median of 10 valid runs, max of per-scenario medians, MB definition fixed in E-1. |
| QEMU RSS never gating | Pass. Stated in G0, Sprint 3, G4a/G4b, risks, D-3 AC, E-2 native preflight. |

## Blocking

B1. The binary gated is not the binary shipped. D-1 (LTO, opt-level, panic=abort, strip) is Sprint 8, but idle is gated on the "shipped release binary" at G0, Sprint 3, G4a (S4) and G4b (S5), and E-5 (S7) reports before D-1. Nothing schedules an idle/peak measurement on the final-profile binary before the MVP release (S9); E-6 only arrives in S12. Profile changes move RSS and binary size, so "gated on the shipped binary" is untestable as written. Fix (either): pin the final release profile in Sprint 3 (interim Cargo profile in E-2, D-1 only codifies it), or add an AC to D-1 (S8) and D-2 (S9) that re-runs E-3/E-4 idle and peak on the final-profile aarch64 binary (gnu and musl) with the same 10 MB/40 MB targets, and make it an MVP release precondition. Also name the profile used in G4a/G4b.

## Non-blocking
N1. E-7 overflow fallback moves the 95%/50% data to Sprint 5 while the Sprint 4 exit still lists the check; state explicitly that Sprint 4 exit then excludes it (plan says A-4 not Done, but exit text does not reflect it).
N2. Shipped-vs-bench delta (idle delta, binary size delta, one public-host 5 MB fetch) has no numeric tolerance, yet is "part of the pass". Add a bound (e.g. idle delta <= 0.5 MB, shipped public-host peak within 10% of bench peak, non-gating floor of <= 40 MB) so a diverging bench build can fail G4a.
N3. Add an automated check, not only a PR checklist, for the A-3b merge gate (e.g. the client constructor takes a non-defaultable `Policy` type, or a CI test that no HTTP dial exists outside the validated-set path).
N4. G4b (S5) has no owning story or AC; it hangs on A-5/A-6 DoD and E-4 text. Add an explicit AC to A-5/A-6 that the G1/G3 harness runs are recorded at G4b, and state harness maintenance for those scenarios is in scope.
N5. Unit ambiguity: "5 MB page" versus "5 MiB cap" and E-2 "at least 10 runs" versus "median of 10". Fix to one definition post-E-1 and make the harness reject fewer than 10 valid runs.
