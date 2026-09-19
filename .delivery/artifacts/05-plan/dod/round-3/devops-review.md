# DevOps DoD Review, Stage 5 Plan, Round 3

Reviewer: DevOps validator. Artifacts: sprint-plan.md (rev 2), docs/EPICS.md (D-2, D-3, D-7, E-2, E-6, E-8).
Result: DONE (0 blocking, 6 non-blocking). All round-2 DevOps findings (1-4) are resolved.

## Gate criteria check
- Hosted PR CI: D-7 in Sprint 0, before any code story needs it. Branch protection AC included. PASS.
- cargo-zigbuild gnu+musl: interim pinned script in E-2 (Sprint 3), when first needed; D-2 (Sprint 9) adopts the same pins. PASS.
- Self-hosted ARM runner: provisioned and isolated in E-2 (Sprint 3); Sprints 0-2 use the existing host manually; availability checked at Sprints 3, 4, 5, 7, 9, 10, 11, 12; tracked as top flag and risk row. PASS.
- Memory gate in CI: manual G4a/G4b at S4/S5; CI job E-6 (S12) does not block MVP because D-2 no longer depends on it (manual bench run plus E-5 are the stated equivalent). No story depends on a job created later. PASS.
- No tagged build before M3: no release workflow exists before D-2 (S9, after M3 at S8), so enforcement is structural as well as by rule. PASS.
- Two-binary approach: workable (same commit, same pinned script, labelled figures, marker-based refusals in the harness, delta record). PASS.
- Branch/PR convention: workable; sprint-start instruction covers push and draft PR; gate-block rule defined. PASS.
- OQ-7 due before Sprint 9, the first sprint with a story needing it (D-2, D-4). PASS.

## Non-blocking
N1. D-2 guard AC (EPICS line ~415) names `test-support` and "bench fixture-CA feature" but not `bench-loopback`, and does not say the marker-string grep runs on the actual cross-built aarch64/macOS artifacts. D-7 checks the hosted x86_64 build; the shipped artifact is a different build. Add: run the D-7 guard (cargo tree plus marker grep) on every D-2 artifact and fail the release job on a hit.
N2. D-7 guard has no positive control (a build with the feature that must fail the guard); until A-3a/E-8 land it can pass vacuously. Add a self-test, and make sure the release build uses `-p <crate>` so a workspace bench member cannot unify `bench-loopback` into the release build. Also run E-8 unit tests with `--features bench-loopback` in hosted CI.
N3. E-2 should assert that the shipped and bench binaries report the same commit and Cargo.lock hash (from `--version`), not only "built by the same script".
N4. D-2 lists only gnu and apple-darwin artifacts, while E-2/G4 gate gnu and musl and D-4 documents the musl build. State whether musl is a shipped artifact; state the macOS build path (hosted macos runner vs zigbuild SDK).
N5. Release-profile config (D-1, S8) lands after the gates (S3-S5) and E-5 (S7). The S9 manual bench run on the D-2 artifact covers it; state explicitly that it is a required re-measure on the D-1 profile.
N6. Housekeeping: `cargo deny` licenses check fails on an unlicensed workspace crate before OQ-7 (set `publish = false` and `licenses.private.ignore` in D-7); A-1 spike code must pass clippy `-D warnings` in the Sprint 0 PR; workflow_dispatch/nightly workflows run only once on main, so the E-2 workflow cannot be exercised on the sprint branch (manual runs until merge, as the plan allows); branch protection should not require reviewers for the solo owner.
