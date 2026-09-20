# DevOps DoD review, Stage 5 Plan, round 2

Verdict: DONE (0 blocking, 4 non-blocking). Round 1 blockers B1-B3 are resolved.

## Gate criteria
| Criterion | Result |
|---|---|
| Hosted PR CI scheduled before needed | Pass. D-7 in Sprint 0 (fmt, clippy, test, deny, SHA-pinned actions, toolchain pin, release-feature guard skeleton). DoD requires it from Sprint 0. |
| cargo-zigbuild aarch64 gnu+musl | Pass. A-1 spike proved it; interim pinned script in E-2 (Sprint 3) before G4 (Sprint 4); D-2 (Sprint 9) adopts the same pins. |
| Self-hosted ARM runner | Pass. Provisioned per arch 9.2 in E-2 (Sprint 3); only manual use before that. Fork PRs excluded, jobs only on main/tags/nightly/dispatch. |
| Memory gate in CI | Pass with note. G4 is a manual/nightly bench run at end of Sprint 4; E-6 makes it a required check in Sprint 12. Plan is explicit that MVP relies on the documented manual bench run plus E-5. No story depends on a not-yet-existing job (D-2 no longer claims D-3/E-6). |
| No tagged build before M3 | Pass. Stated in branch convention, Sprint 2 note, risks; D-6 is the v1.0 tagger. M3 (Sprint 8) precedes D-2 (Sprint 9). |
| No dependency on later-sprint check | Pass. The test-support absence check (A-3a, Sprint 1) uses the D-7 skeleton from Sprint 0. |
| Branch/PR convention | Workable. One sprint branch and draft PR to main, opened at sprint start, no pushes without owner instruction (covered by sprint-start instruction). |
| Runner availability tracked | Pass. Top flag 3, risks table, checks at Sprints 3, 4, 9, 12; no runner means G4 unevaluated. |
| OQ-7 before first need | Pass. Due before Sprint 9; first need is D-2/D-4 in Sprint 9. Still OPEN, tracked. |

## Non-blocking findings
1. Runner availability checks omit Sprints 5-8 and 11 (D-3 lands in Sprint 11, needs the runner; Sprint 5 re-runs idle/peak). Add Sprints 5 and 11 to the check list.
2. Branch protection / required-check configuration is not an explicit D-7 AC (a solo repo needs it set for "required check" to be real, including the Sprint 1 release-build check). Add one line to D-7.
3. Nightly and main-triggered self-hosted workflow that E-2 references ("nightly and on main and tag builds") has no explicit owning story before D-2 (Sprint 9); state that E-2 creates the minimal main/nightly/dispatch workflow, or that Sprint 3-8 runs are manual.
4. Sprint N branch is "created from main" while the prior sprint PR must be merged first; Sprint 0 branch already carries A-1 work. Note the sequencing rule (merge N-1 before branching N) and what happens if a gate blocks the merge (e.g. G4 fail).
