# Sprint 0 revision 2 report (D-7, E-1)

## Blocking
- QA B1 (boundedness/gate completeness): measure.py now marks a scenario with `bounded_vs` whose reference has no valid result as verdict `INCOMPLETE` (exit 2, `summary.incomplete` lists it), never PASS/ADVISORY_PASS. `--gate` computes the required set (`required_gate_set`: shipped -> `idle`; bench -> every scenario of each G4a/G4b group touched, default G4a) and refuses with reason "incomplete" if any is missing. Selftest: the probe (`g4a-50mib-cl`, `STANDIN_TOOLARGE_ALLOC_MIB=30`, with and without `--smoke`) gives INCOMPLETE exit 2; partial G4a under `--gate` refused.
- Architect (OQ-7): `license` field and its comment removed from Cargo.toml (`publish = false` kept, LICENSE untouched). deny.toml comments only say licence is OQ-7 open and cover dependencies; not changed. docs/ci-branch-protection.md Licence section now states no licence field, OQ-7 undecided.

## Non-blocking
- N1: `--gate` child env is an allowlist, empty (`GATE_CHILD_ENV_ALLOW`); any `--child-env` refused (GLIBC_TUNABLES, FETCH_*, LC_ALL, PATH, LD_PRELOAD all tested).
- N2: architecture 11.2 sets `expected_min_bytes` for G7a to zero, so the floor cannot be raised without inventing a value. Kept at 1 byte, comment cites 11.2, documented in BENCHMARK.md sec 5.
- N3: nonexistent/non-executable binary refused with exit 2 before any spawn; `Server()` creation moved inside the try in `sample`. `--smoke` exit 0 unchanged (advisory, documented).
- N4: `shipped` refused if `--version` has `bench-loopback`, `test-support` or any `FETCH_MCP_MARKER_`; standin gained `STANDIN_TESTSUPPORT`; selftest case added; documented in BENCHMARK.md sec 4. The real `--version` path is still only covered by the Rust `version_line` unit test.
- N5: override and completeness refusals now run before the native-aarch64 check, so selftest asserts exit 2 with the specific reason on any host; a complete idle gate still gives exit 3 off aarch64.
- N6/clippy: `vec_init_then_push` fixed in src/lib.rs (cfg-gated array `.to_vec()`); ci.yml clippy job also runs `--features bench-loopback`; documented in ci-branch-protection.md. `cargo test --features bench-loopback` in CI remains E-8 (unchanged).
- Quickstart: cargo build step added (now step 3, later steps renumbered).
- Guard `--features` mode documented in ci-branch-protection.md.
- Sec 11 item 7 delta bounds: BENCHMARK.md sec 4 now says no script enforces them yet; owned by E-8 (commit/lock-hash in `--version`) and the manual G4a report until then.

## Verification (all run locally)
cargo fmt --check ok; clippy --locked --all-targets and with bench-loopback -D warnings ok; cargo test --locked (also bench-loopback, test-support) ok; check-release-features.sh and --self-test OK; python3 bench/selftest.py PASSED; actionlint ok.
Not run: GitHub Actions, cargo-deny/cargo-audit (not installed), native aarch64 `--gate` path, real-binary harness run (only the stand-in).
