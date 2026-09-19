# Architect DoD review, Sprint 0 (D-7, E-1), round 3
Verdict: DONE (no blocking findings).

## Conformance
- Toolchain: rust-toolchain.toml channel 1.94.1, minimal, clippy+rustfmt, aarch64 gnu/musl/apple-darwin targets. Matches sec 9.1. MSRV absent (deferred to D-2, correct).
- Release profile: opt-level "s", lto, codegen-units 1, panic abort, strip. Matches sec 9 / item 9 exactly.
- Features: default=[], test-support, bench-loopback (no fixture-CA feature; BENCHMARK.md sec 8 does not adopt one, consistent with E-1 decision authority). Cargo.lock committed; CI uses --locked.
- Licence/publish (OQ-7): Cargo.toml has publish=false only, no `license` field; deny.toml [licenses] is a dependency allow-list, [licenses.private] ignore=true. No pre-emption. deny.toml bans/sources/advisories match sec 9.2.
- Release guard: scripts/check-release-features.sh does cargo tree -e features allowlist (default only) plus marker grep on the binary, with positive-control self-test; marker contract in src/lib.rs matches ADR-003/sec 9.2. CI release-guard job runs both.
- Harness (bench/scenarios.py, docs/BENCHMARK.md): G0/G4a/G4b gate split and G1..G7 mapping match sec 11.0/11.1; idle on shipped binary, peak on bench build; 10-run/INVALID rule, median gating, native-aarch64 exit code 3. Unimplemented G4b/G6/late-landmark scenarios are flagged implemented=False and refuse (exit 2), not silently passed.
- Open decisions OQ-3/4/5: nothing in code pre-empts them. Dependencies empty (exact pins per sec 9.1 due when added in A-2/A-3).

## Findings
1. NON-BLOCKING divergence: g4a-50mib-cl min_bytes is 1, architecture 11.1 says expected_min_bytes for G7a is zero. Deliberate, commented in scenarios.py and documented in BENCHMARK.md line 73; stricter than 0 (only confirms request reached server), cannot mask an invalid sample. Accept; if strict literalness is wanted, set 0 (measure.py already defaults missing min_bytes to 0).
2. NON-BLOCKING observation: repo-root LICENSE (Apache-2.0) exists from the Initial commit, not Sprint 0. Cargo.toml does not reference it, so OQ-7 stays open, but the human should confirm it is not read as the decided licence.
3. NON-BLOCKING: hosted PR CI omits items sec 9.2 lists (cargo audit, aarch64 cross-build, QEMU smoke); these belong to later stories (D-2/D-3/D-6), not D-7's skeleton. Track, do not block.
