# QA Review (Stage 5 DoD): testability of the plan

Reviewed: `.delivery/artifacts/05-plan/po/sprint-plan.md`, `docs/EPICS.md`. Status: **NOT_DONE** (2 blocking).

## Blocking

**B1. 50-URL offline snapshot set has no sprint that creates it.**
E-1 (Sprint 0) only "lists the URLs"; E-2 (Sprint 3) fixtures cover 5/50 MB, gzip and slow-drip, not the snapshots. A-4 (Sprint 4) exit uses "curated set" for token reduction (>= 50%, no `<script>`), and the Epic A metric (>= 95% conversion success) is not in any sprint exit or gate. Fix: add a task (E-1 or E-2) to capture snapshots with committed sha256 manifest, and add the >= 95% success and >= 50% median reduction checks to Sprint 4 exit (or name the sprint where they run).

**B2. DoD benchmark rule is unsatisfiable for A-3b in Sprint 2.**
DoD requires a native-aarch64 benchmark run for A-3b, but the harness (E-2) lands in Sprint 3 and Sprint 2 exit lists no benchmark. Fix: exempt A-3b from the per-story benchmark (covered by E-3/E-4 in Sprints 3-4) or move a minimal RSS smoke into Sprint 2; also state which stories in "E-*" are exempt (spikes E-1).

## Non-blocking

1. Merge gate for A-3b is a PR checklist. Items (3) and (4) are testable (four-refusal integration tests; fail-closed default test). Items (2) "every hop goes through check_url" and "test constructor absent from release builds" have no named mechanism. Suggest a CI step (e.g. `cargo build --release` plus `cargo tree -e features` / symbol grep for `test-support`) and a required check, so the gate is not reviewer memory.
2. G0 "targets look achievable" is not measurable. State numeric rule (e.g. A-1 measured idle and peak within X% headroom of 10/40 MB) and require ARM host recorded before exit.
3. G4 does not state: MB definition (10^6 vs MiB, decided in E-1), both gnu and musl binaries, or that QEMU/non-native figures never count. QEMU exclusion exists only in E-2 preflight and the risk table; add it to G4 and Sprint 3 gate text. E-6 and D-3 handling are fine.
4. Inconsistency: E-2 says gating peak = max of per-scenario medians incl. the 10-concurrent scenario; E-4 says concurrency is "recorded and reported" only. Clarify whether G6 gates. Also the 10% boundedness checks are in Sprint 4 exit but not in the G4 pass condition.
5. ARM runner dependency: Sprint 1 exit ("ready within 250 ms on ARM") is not listed under Sprint 1 dependencies. Add. Sprint 0 exit should require ARM access confirmed (the risk table says so; exit criteria do not).
6. Sprint 4 (8 pts) holds A-4, E-4 and the gate with no slack; a G4 miss has a defined path, but suggest keeping E-4 first-priority if A-4 slips.
7. A-3a test-only constructor absence "asserted" needs a concrete test (see 1).

## Criteria check

| Criterion | Result |
|---|---|
| Verifiable AC per story | Pass (Given/When/Then, measurable) |
| DoD tests/clippy/fmt | Pass (fmt --check, clippy -D warnings, test --locked) |
| SSRF suite scheduling | Pass: unit S1, integration S2, depth S6, suite/coverage S8, before any tag |
| 50-URL set | Fail (B1) |
| Memory gate S3/S4 with ARM dependency | Pass with gaps (NB 3, 5) |
| A-3b/A-3a merge gate testable | Partial (NB 1) |
| Go/no-go measurable (10/40 MB, median of 10) | G3/G4 pass; G0 vague (NB 2) |
| QEMU never gating | Pass in substance, weak in G4 text (NB 3) |
