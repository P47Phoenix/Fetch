# Adversarial review: Sprint 0 development (0716084 D-7, 594dfeb E-1)

Re-run results: cargo fmt, clippy -D warnings, test: pass. check-release-features.sh: "guard OK". --self-test: OK.
python3 bench/selftest.py: PASSED (all checks). cargo-deny is not installed locally, so deny.toml effectiveness was not run (unverified).
Verdict: no BLOCKING findings.

## NON-BLOCKING

N1. Guard uses a hard-coded feature list, so a renamed or new forbidden feature passes.
Reproduced: adding `loopback2 = []` and `default = ["loopback2"]` in Cargo.toml gives "guard OK" (scripts/check-release-features.sh:13-14, 34-38).
There is no allowlist of permitted features. Suggest failing if any feature other than an explicit allowlist is enabled in the release tree.

N2. Marker detection relies on the convention that each gated code path pushes a marker (src/lib.rs:9-14).
Future code behind `#[cfg(feature="test-support")]` that does not push a marker leaves the binary marker-free. Only the tree check then protects it.
Reproduced: `RUSTFLAGS='--cfg feature="test-support"'` is caught by the marker only because the stub pushes one.
Suggest a lint that every cfg(feature) gate carries a marker.

N3. Marker strings do not match the contract text.
EPICS.md:133 and BENCHMARK.md sec 4 say the marker string `bench-loopback` appears in the binary and in `--version`.
The guard greps FETCH_MCP_MARKER_BENCH_LOOPBACK_V1 (script line 15), and src/main.rs has no `--version` at all (only `--build-info`).
measure.py:175 keys on "bench-loopback" in `--version`. E-8 must reconcile; today the two do not connect.

N4. `--gate` does not forbid `--settle` or `--parallel-idle` overrides (measure.py:160-164).
`--gate --settle 0.1` produces a gating record with an idle RSS taken before the 30 s settle (under-measure).
`settle_s` is recorded, so it is detectable, but the run is not refused.
Suggest that --gate require settle >= 30 and parallel-idle == 1 (docs sec 6/7 promise this).

N5. Runs without `--gate` return exit 0 and verdict PASS (measure.py:229-232).
Only the note field says advisory. A CI caller that forgets --gate gets a green result. Suggest a distinct verdict (ADVISORY) or a non-zero exit.

N6. `g4a-50mib-cl` has min_bytes 0 (scenarios.py), so an early-stop or too_large-by-error path is not validated beyond the text "too_large".
The outcome check is a substring match on the whole JSON result (measure.py:112). A response that merely mentions too_large passes.
Low risk; tighten to the error code field when the real server exists.

N7. `--child-env` can set LD_PRELOAD or MALLOC_* and still be accepted under --gate (measure.py:65, 168).
It is recorded, but the docs claim these are unset. Suggest refusing forbidden keys under --gate.

N8. CI toolchain install is unverified (ci.yml:26-63).
Only the fmt job runs `rustup show active-toolchain`. clippy, test and release-guard rely on the rustup proxy auto-installing rust-toolchain.toml.
Rustup 1.28+ (which I recall removed proxy auto-install) may fail those jobs on hosted runners. Not reproduced locally: rustup 1.29 here already has the toolchain.
Suggest an explicit `rustup toolchain install` step in every job. The pinned `targets` list (three aarch64 targets) also downloads in every job, which is slow and wasteful.

N9. docs/ci-branch-protection.md says `cargo deny --locked check`, but ci.yml passes `arguments: --locked` after the command, i.e. `check --locked`.
Verify that this flag is accepted by cargo-deny (not run locally). "Required status checks" are docs-only, so the acceptance criterion depends on manual owner action.

N10. The release guard job checks only the host build with default features.
It does not test a musl or aarch64 artifact. That is acceptable for D-7 (D-2 adds `--binary`), but a cfg by target could differ.

N11. .gitignore side effect: removing `Cargo.lock` from .gitignore (correct for D-7) now surfaces `spikes/a1/Cargo.lock` as untracked (git status). Decide whether to commit or ignore the spike lock.

N12. Scope notes.
The stub `src/lib.rs` and `src/main.rs` are needed to make the guard testable, and are a reasonable minimal addition.
`fixture-ca` is declared as a feature although BENCHMARK.md sec 8 decides not to adopt it (EPICS D-7 still names it, so this is consistent with EPICS but not with E-1).
BENCHMARK.md sec 10 records a GO recommendation while the harness has not been run on aarch64; it is stated as conditional, which is fine.

## Checked, no defect found
- Actions pinned by SHA; permissions contents: read; no pull_request_target, secrets, or caches; persist-credentials false.
- Release profile applied (`Finished release profile [optimized]`; Cargo.toml [profile.release] values match architecture sec 9).
- MiB/kB unit handling in measure.py (MIB_KB = 1024, VmHWM/VmRSS in kB); median via statistics.median over valid runs only; fixtures deterministic (selftest regenerates byte-identical); early-stop detected by the server byte counter; gz/5 MiB min_bytes correct.
- Feature enabled through `default = ["test-support"]` is caught by the tree check (reproduced, "guard FAIL").
