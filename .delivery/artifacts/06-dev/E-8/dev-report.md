# E-8 dev report: bench-loopback compile-time feature

Gap closed: `bench-loopback` was inert because main always used `Policy::default()`.

Changes
- `src/policy.rs`: `Policy::for_build()`: loopback-permitting only under `cfg(feature = "bench-loopback")`, otherwise `Policy::default()` (fail-closed). No runtime switch. `test-support` alone does not affect it.
- `src/main.rs`: `serve()` uses `Policy::for_build()`; build markers (from `build_markers()`) are logged to stderr at startup (`warn build marker ...`), satisfying the E-8 "logs a marker" AC. Marker string and `--version` unchanged, so the release guard is unaffected.
- Tests, both directions:
  - unit: `for_build_is_fail_closed_without_bench_feature` (default and test-support builds); `for_build_permits_only_loopback_with_bench_feature` (127/8 and ::1 pass; 0.0.0.0, RFC1918, link-local, metadata, CGNAT, ::, fe80, ULA, mapped-private still refuse; public passes).
  - stdio integration (`ssrf_checks_...`): loopback literals and `localhost` are `blocked_target` in default builds and reach `not_implemented` (policy passed) in a bench build; non-loopback refusals unchanged in all builds. Startup-stderr test tolerates the bench marker line.

Verification (clean `git archive HEAD` extract, /tmp/e8): fmt OK; clippy -D warnings clean for default, bench-loopback, test-support; cargo test --locked: default 44+10, bench-loopback 45+10, test-support 44+10, all pass; release build (-p fetch-mcp) OK; check-release-features.sh "guard OK" and --self-test OK; bench/selftest.py PASSED; actionlint clean (run in repo; the archive has no .git, which actionlint requires).

Not started: A-3b, E-7. Not done here: commit/Cargo.lock hash in `--version` (AC mentions it; belongs with the build script / D-3 per lib.rs note).

## Correction (A-3b fix-pass 1, 2026-09-20; history above kept)
The note above says the `--version` commit and Cargo.lock hash belongs "with the build script / D-3". That is wrong: D-3 is the aarch64 test suite and has no such acceptance criterion. The item is re-homed to E-4 (it must land before G4a, end of Sprint 4) in docs/EPICS.md, together with the E-8 handshake, idle-delta, size-delta and public-host cross-check items (E-2/E-4). The comment in src/lib.rs was corrected to match.
