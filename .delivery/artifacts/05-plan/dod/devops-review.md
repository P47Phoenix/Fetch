# DevOps DoD Review: Stage 5 Sprint Plan

Reviewer: DevOps validator. Artifacts: sprint-plan.md, docs/EPICS.md (D-2, D-3, D-6, E-2, E-6), architecture sec 9, PRD v0.4.
Result: NOT_DONE (3 blocking, 4 non-blocking).

## Blocking

B1. No CI exists before Sprint 9, but the DoD requires "CI green" from Sprint 0/1.
Sprint plan DoD (line 85-90) and branch convention require fmt/clippy/test `--locked` and "CI green" per story, per sprint PR. The only pipeline story is D-2 (Sprint 9). Hosted PR checks (fmt, clippy, test, `test-support` absence check, cargo-deny/audit, SHA-pinned actions, toolchain pin) are needed from Sprint 1 (A-3a merge gate relies on the release-build assertion). Fix: add a small hosted-CI story (about 2 pts) to Sprint 0 or 1, or split D-2 into D-2a (hosted PR checks, Sprint 1) and D-2b (zigbuild aarch64 gnu+musl release pipeline).

B2. Circular / mis-sequenced dependency between D-2 (Sprint 9) and E-6 (Sprint 12).
D-2 AC says the release workflow depends on the aarch64 test and memory-gate jobs as required status checks. The memory-gate job is E-6 and the aarch64 tests are D-3 (Sprint 11), both after D-2. The release pipeline cannot be finished or verified in Sprint 9 as written, and "no tagged build before M3, release requires memory gate" cannot be enforced mechanically until Sprint 12. Also the MVP (Sprint 9) has no enforced gate. Fix: move E-6 and D-3 before or with D-2 (Sprint 8-9), or state that the release workflow is gate-less until Sprint 12 and forbid tagging until then (and make D-6 the only tagger).

B3. OQ-7 (distribution and licence) timing is inconsistent with the stories that need it.
OQ-7 is due "before Sprint 12" (D-6), but D-2 (release artifacts and naming, Sprint 9) and D-4 (install guide, Sprint 9, declared MVP complete) both depend on the distribution channel and licence (LICENSE file, Cargo metadata, artifact publication target). Fix: require OQ-7 decided before Sprint 9 (or move D-4/D-2 publication parts to Sprint 12); update EPICS OQ table deadline.

## Non-blocking

N1. Self-hosted runner setup is not a story. D-2 covers workflows only; registering the ARM runner, isolating it (ephemeral or dedicated user, no fork PRs, secrets scope per architecture 9.2), and documenting its OS/RAM has no owner or sprint. Runner is tracked as a risk (good) but only "confirm access before Sprint 0 exit". Add a runner-provisioning task (Sprint 0/3) and a check of runner availability at the start of Sprints 3, 4, 9, 12.

N2. musl and gnu binaries are needed by E-2 in Sprint 3 (harness runs both), but the cargo-zigbuild pipeline and version pins land in Sprint 9. Sprint 3 needs an interim documented build script (from the A-1 spike); note it in E-2 and pin zig/cargo-zigbuild versions from the start, not at D-2.

N3. Branch/PR convention is workable (one sprint branch, draft PR to main), but "no pushes without owner instruction" conflicts with "draft PR opened at sprint start" (a push is required). Clarify that the owner's sprint start instruction covers the branch push and PR creation. Also state that per-story native aarch64 benchmarks (DoD) are manual on PRs since self-hosted jobs run only on main/tags/nightly/dispatch, and that Sprint 0 branch already carries A-1 (close-out via PR).

N4. Sprint 12 concentrates E-6, D-6 and the OQ-5/OQ-7 dependencies with the runner in one 7-point sprint, with no slack for a runner outage. Consider pulling E-6 earlier (see B2).

## Criteria check
- cargo-zigbuild aarch64 gnu+musl: scheduled Sprint 9, needed Sprint 3 (N2), CI need from Sprint 1 (B1).
- Self-hosted ARM runner: tracked dependency (yes), provisioning not scheduled (N1).
- Memory gate in CI: G4 manual in Sprint 4 (acceptable), CI gate E-6 in Sprint 12 (B2).
- Release gating before M3: stated in plan and risks; enforcement mechanism late (B2).
- Branch/PR convention: workable (N3).
- OQ-7 timing: inconsistent (B3).
