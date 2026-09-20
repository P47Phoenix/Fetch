# Developer DoD review, Sprint 0 (D-7, E-1), round 3

STATUS: DONE (no blocking findings)

## Gates run (all local, this review)
- cargo fmt --check: ok
- clippy --locked --all-targets -D warnings: ok; with --features bench-loopback: ok
- cargo test --locked: ok (default, bench-loopback, test-support)
- scripts/check-release-features.sh: guard OK; --self-test: OK
- python3 bench/selftest.py: SELFTEST PASSED
- Not run by me: GitHub Actions, cargo-deny, native aarch64 (as the reports also state).

## AC conformance
D-7: fmt/clippy/test jobs, --locked, toolchain pin, SHA-pinned actions, deny job, release guard with self-test and `-p`, pinned release profile, publish=false: met.
Branch protection: owner-configured, documented in docs/ci-branch-protection.md: acceptable.
E-1: unit (MiB), targets, host placeholders (not guessed), protocol, TLS decision, loopback/binary mapping, Goal 2/3/5 methods, go/no-go: met.
E-1 AC 50-URL list: deferred to owner/E-7 (named); no URLs invented.
E-1 A-1 re-run: deviation recorded, which the AC permits. A-1 figures cited in BENCHMARK.md sec 10 trace to 03-spike/a1/a1-spike-report.md (no fabrication).
Revision-2 report claims spot-checked true: license field absent, ci.yml clippy runs bench-loopback, INCOMPLETE verdict documented and selftested, guard/self-test pass.

## Non-blocking
1. D-7 AC "E-8 unit tests run with --features bench-loopback in hosted CI": ci.yml runs clippy with the feature but not `cargo test --features bench-loopback`. Deferred to E-8 (report says so); add the CI step when E-8 tests land.
2. Real-binary `--version` path is covered only by the Rust unit test plus the stand-in; first real-binary harness run belongs to E-2/G4a.
3. Untracked dod/round-2 and round-3 dirs are uncommitted review artifacts.
