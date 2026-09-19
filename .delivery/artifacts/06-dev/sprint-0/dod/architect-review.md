# Architect DoD review: Sprint 0 (D-7 0716084, E-1 594dfeb)

Role: Solution Architect validator. Status: NOT_DONE (1 blocking).

## Conforming
- Stub layout: single crate, lib + bin, no deps (sec 3, modular monolith); exact dependency pins correctly deferred to A-2/A-3a.
- Features test-support, bench-loopback, fixture-ca: off by default, additive, compile-time only, marker strings per sec 9.2/R12.
- Release profile: opt-level s, lto, cgroup=1, panic abort, strip (sec 9). Toolchain 1.94.1 minimal, clippy/rustfmt, aarch64 gnu/musl/darwin targets (sec 9.1). Cargo.lock committed, --locked used.
- deny.toml: advisories, bans (openssl, openssl-sys, native-tls, aws-lc-sys, aws-lc-rs), crates.io-only sources, permissive licence allow-list (sec 9.2).
- Guard: cargo tree -e features plus artifact marker grep, -p release build, positive-control self-test that checks both detection paths (sec 9.2 R12).
- CI: hosted only, no pull_request_target, no self-hosted, read-only permissions, actions SHA-pinned (sec 9.2).
- Harness (docs/BENCHMARK.md, bench/): MiB unit; idle 10 MiB shipped / peak 40 MiB bench / 1.10x bound; median of 10 valid, fewer = INVALID; 30 s settle; fresh process; byte-counter validity; hash-checked seeded fixtures; native-ARM preflight (exit 3); G0/G4a/G4b split and G1..G7 IDs match sec 11.0/11.1; G4a set (5 MB, gz, late landmark, 50 MB CL, 50 MB chunked) and G4b set match; G6 non-gating; E-8 bounds (0.5 MB idle delta, 10% public-host, same commit/lock hash) stated. Unimplemented scenarios are refused, not skipped.
- E-1 TLS decision (no fixture-ca build; manual real-host run; NFR-11 plain-HTTP caveat kept) is the decision sec 11 item 8 explicitly left to E-1; flagged as author recommendation for owner review.
- OQ-3/4/5/7 not decided by the harness or docs (E-8 doc states OQ-4 not decided).

## Findings
1. BLOCKING: Cargo.toml sets `license = "MIT"`. The project licence is OQ-7 (open, sec 9.2) and the repo LICENSE is Apache-2.0 (Initial commit), so this is an unapproved decision and contradicts the repo. Remove the field (publish=false makes it unnecessary) or align after OQ-7.
2. Non-blocking: CI omits `cargo audit` (sec 9.2 lists it for PRs; deny advisories covers RustSec partly), Dependabot config, aarch64 cross-build and QEMU musl smoke. Plan D-7 AC does not require them; track for D-2/D-3 and add audit plus Dependabot early.
3. Non-blocking: fixture-ca feature is retained in Cargo.toml though E-1 declines to adopt it. Acceptable as guard scaffolding (sec 9.2 names it); note it is unused and may be removed at E-8/D-2 if no reader appears. Also ensure ADR/architecture text is updated to record E-1's decision.
4. Non-blocking: G0 A-1 re-run and aarch64 measurements not done (recorded deviation, no host); branch protection is documentation only. Both must be closed for the G0 exit and required-check rule (skipped = failure).
5. Non-blocking: 50-URL list not done in E-1 (deferred to owner/E-7); .gitignore edit and spikes/a1/Cargo.lock left untracked, which architecture 9.1 cites as the pin source, so it should be committed.
