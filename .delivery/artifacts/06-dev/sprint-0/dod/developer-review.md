# Developer DoD review: Sprint 0 (D-7, E-1)

STATUS: DONE (no blocking findings)

## Verified by running
- cargo fmt --check, clippy --locked --all-targets -D warnings, cargo test --locked: pass (1 test).
- scripts/check-release-features.sh --self-test: pass (clean passes; test-support, bench-loopback, fixture-ca each rejected for both tree and marker reasons; not vacuous).
- python3 bench/selftest.py: SELFTEST PASSED (15 checks).
- Action SHAs confirmed against git ls-remote (checkout v4.2.2; cargo-deny-action v2.1.1 peeled commit 3c63498...).

## D-7 AC
- fmt/clippy/test job definitions with --locked: met (hosted run not possible locally; verify on first PR).
- Toolchain pin, Cargo.lock committed, SHA-pinned actions: met.
- deny.toml: written to spec; cargo-deny not installed, NOT RUN (non-blocking; first hosted CI run is the check).
- Guard + self-test + -p build: met. "E-8 tests with --features bench-loopback": deferred to E-8 (no tests exist), noted in ci.yml.
- Release profile pinned, publish=false, licenses.private.ignore: met.
- Branch protection: deferred, owner = repo owner, documented in docs/ci-branch-protection.md.
- No self-hosted: met.

## E-1 AC
- Targets and single unit (MiB): met. Host placeholders: met as specified. Protocol/fixtures/client/proc read: met (some fixtures E-2, stated).
- TLS decision, loopback/binary statement, Goal 2/3/5 methods: met.
- 50-URL and 10-URL lists: DEFERRED, owner = owner / E-7; sprint-plan Sprint 0 exit lists the URL list, so this is an open exit item.
- A-1 re-run under protocol: deviation recorded (no aarch64 host); G0 owner = owner with runner.
- Go/no-go: preliminary conditional GO; figures quoted are A-1 x86_64 results, labelled as such. No fabricated numbers.

## Findings
1. non-blocking: cargo-deny never run locally; deny.toml unvalidated until hosted CI.
2. non-blocking: root .gitignore edit (Cargo.lock) is outside listed scope but required by AC; disclosed. spikes/a1/Cargo.lock still untracked, owner decision.
3. non-blocking: 50-URL list missing but plan Sprint 0 exit expects it; must be resolved or explicitly re-deferred before sprint close.
4. non-blocking: Cargo.toml lists aarch64 targets in toolchain not used by CI yet; harmless.
5. non-blocking: bench harness is skeleton-scope (G4b args, late-landmark fixture, macOS path deferred to E-2); no scope creep observed.
