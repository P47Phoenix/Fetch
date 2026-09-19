# Architect DoD review, Stage 6 Sprint 0 (D-7, E-1), round 2

Role: Solution Architect validator. Branch sprint-0/spikes. Verdict: NOT_DONE (1 blocking, 3 non-blocking).

## Conforming
- Stub layout: single binary crate `fetch-mcp` (src/lib.rs, src/main.rs); no dependencies; sec 3 modules arrive in later sprints. OK.
- Features: `test-support`, `bench-loopback`, both off by default, marker-string contract (`FETCH_MCP_MARKER_<F>_V1:<name>`) matches sec 9 R12 / ADR-003. No fixture-CA feature (consistent with sec 11 item 8 left to E-1). OK.
- Release profile: opt-level "s", lto, codegen-units 1, panic abort, strip = exactly sec 9. OK.
- Toolchain: rust-toolchain.toml channel 1.94.1, minimal, clippy+rustfmt, aarch64 gnu/musl/apple-darwin targets = sec 9.1. MSRV deferred to D-2 as designed.
- deny.toml: advisories, bans (openssl, openssl-sys, native-tls, aws-lc-sys, aws-lc-rs), sources (crates.io only, no git), permissive dependency licence allow-list = sec 9.2. Allow-list governs dependencies only.
- Release guard: cargo tree feature allowlist (default only) plus marker grep on the artifact plus self-test positive controls; CI job present, actions SHA-pinned, hosted runners only, no pull_request_target. Matches sec 9.2 / R12.
- Harness: G0/G4a/G4b split, targets 10/40 MiB and 1.10 bound, median of >= 10 valid runs, 30 s idle settle, pinned env (no LD_PRELOAD/MALLOC/RUST_LOG), native-aarch64 preflight with qemu binfmt check, shipped vs bench binary-kind check via marker, --gate refuses overrides. G4a scenarios (5 MiB full, gz, late-landmark, 50 MiB CL, 50 MiB chunked) and G4b (window start/end, raw, chunked-in-cap, beyond-cap), G6 non-gating all present and correctly mapped to G1..G7 per sec 11.0/11.1. Unimplemented scenarios are refused, not silently passed.
- OQ-3, OQ-4, OQ-5: not pre-empted (no robots, allowlist or untrusted-label code).

## Findings
1. BLOCKING. Cargo.toml `license = "Apache-2.0"` states the crate's licence in manifest metadata. Architecture sec 9.2 and sec 12 say the project's own licence waits on OQ-7 (open, human decision). The comment "matches the repo LICENSE" does not help: the LICENSE file is the repo-creation template (commit c92e344), not a recorded OQ-7 decision. `publish = false` is correct and sufficient to block crates.io. Fix: remove the `license` field (and keep `[licenses.private] ignore = true`), or record an explicit human OQ-7 decision first. Do not add `license-file` either.
2. NON-BLOCKING. Sec 11 item 7 numeric bounds (idle delta bench vs shipped <= 0.5 MB, same commit and Cargo.lock hash, public-host 5 MB comparison) are not enforced in measure.py. Acceptable for Sprint 0 skeleton; must land by E-8 / G4a. Track it.
3. NON-BLOCKING. `--gate` forbids `--parallel-idle > 1`, stricter than sec 11 step 2 which permits parallel idle. Conservative; note as an intentional divergence in docs/BENCHMARK.md if not already.
4. NON-BLOCKING. `bench-loopback` is an empty feature today (no 127.0.0.0/8 and ::1-only policy yet); acceptable until E-8, but the guard's binary marker is the only current proof. Confirm E-8 keeps the marker contract.
