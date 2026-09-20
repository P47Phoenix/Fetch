# Sprint 0 revision 1 report (D-7, E-1)

## Finding to resolution

| Finding | Resolution |
|---|---|
| Architect BLOCKING: `license = "MIT"` pre-empts OQ-7 and contradicts LICENSE | LICENSE is the Apache-2.0 text, so `Cargo.toml` now says `license = "Apache-2.0"` (mirrors the file only, commented as OQ-7 open); `publish = false` kept. OQ-7 not decided. `deny.toml` audited: its `[licenses]` allow-list is dependency-only permissive; comments now say it does not decide the crate's own licence. |
| Architect: no cargo audit / Dependabot | Added `.github/dependabot.yml` (cargo, github-actions, weekly). `cargo deny check` already covers RustSec advisories; nightly audit is D-3. Noted in docs/ci-branch-protection.md. |
| Architect: unused `fixture-ca` | Removed feature, marker and guard entry; consistent with BENCHMARK sec 8 (fixture-CA not adopted). |
| Architect: untracked spikes/a1/Cargo.lock | Ignored via `spikes/a1/.gitignore` (spike has its own ignore file). |
| Tech-writer + adversarial BLOCKING: marker/`--version` contract | Chosen contract (matches architecture 9 R12/ADR-003 "fixed marker string" and EPICS E-8 literal `bench-loopback` in `--version`): each marker is `FETCH_MCP_MARKER_<FEATURE>_V1:<feature-name>`. `main.rs` now implements `--version` (crate version plus markers; replaces the odd `--build-info`). Bench build prints `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback`; release prints none. Harness (literal `bench-loopback`) and guard (`FETCH_MCP_MARKER_` prefix) both work unchanged in meaning. BENCHMARK sec 4 states the one contract. EPICS not edited (its wording is now satisfied literally). Unit tests added, including a bench-feature test. |
| Note, not silently picked | EPICS E-8 also requires commit and Cargo.lock hash in `--version` and a startup stderr marker. Not implemented (needs build script; owned by E-8/D-3). BENCHMARK sec 4 says so explicitly. EPICS and architecture do not conflict on the marker. |
| Adversarial: hard-coded feature list, renamed feature passes | Guard now parses the root package feature list from `cargo tree` and fails on anything outside `ALLOWED_FEATURES=(default)`; binary check fails on any `FETCH_MCP_MARKER_` prefix. Self-test keeps positive controls per real forbidden feature and adds: renamed `loopback2` fails, allowlisted passes, garbage tree fails, unknown marker fails. |
| Adversarial: `--gate` overrides; non-gate PASS | `--gate` refuses `--settle` != 30, `--parallel-idle` > 1, `LD_PRELOAD`/`MALLOC_*`/`RUST_LOG` child env. Non-gate pass verdict is `ADVISORY_PASS`. |
| Adversarial: `min_bytes` 0 and substring `too_large` | `g4a-50mib-cl` min_bytes 1; `too_large` must be an `isError` result with the text in content. |
| Adversarial: only fmt job installs toolchain | Added `rustup show active-toolchain` to clippy, test and release-guard jobs. `cargo check --locked` form: CI does not use `cargo check`; all commands use `--locked`. |
| QA gap: no peak/boundedness FAIL test; smoke PASS | Selftest adds peak FAIL, boundedness FAIL (stand-in `STANDIN_TOOLARGE_ALLOC_MIB`), ADVISORY_PASS, and `--gate` override refusals. |
| Tech-writer non-blocking (quickstart, worked example, fixtures-before-run, gate vs scenario names, branch protection pending, bench-loopback tests not run) | BENCHMARK.md Quickstart section with prerequisites, self-test, generate step, worked smoke example (verified), gating example, gate/scenario glossary. ci-branch-protection.md: "NOT YET CONFIGURED", bench-loopback tests not run yet, record template. |

## Verification
Run and passing: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` (and with `--features bench-loopback`), `scripts/check-release-features.sh`, `--self-test`, `python3 bench/selftest.py` (all ok), `actionlint`, worked quickstart smoke example.
Not run: `cargo deny` and `cargo audit` (not installed); GitHub-hosted CI; native aarch64 `--gate` path (selftest override-refusal cases hit the non-native exit 3 first on this host).
