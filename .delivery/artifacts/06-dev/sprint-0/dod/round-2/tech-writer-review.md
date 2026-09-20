# Technical Writer DoD review, Sprint 0 round 2

STATUS: DONE (no blocking findings)

Verified by running:
- `python3 bench/selftest.py` ended `SELFTEST PASSED` (about 27 s; doc says about 1 minute, fine).
- The Quickstart worked example (fixtures generate, then measure.py smoke on the stand-in with STANDIN_BENCH=1) ran: 10/10 valid runs, verdict ADVISORY_PASS, rc 0.
- `scripts/check-release-features.sh` printed `guard OK`.
- Marker contract matches across src/lib.rs, src/main.rs (`--version`), the guard (`FETCH_MCP_MARKER_` prefix grep, feature allowlist) and BENCHMARK.md sec 4 and measure.py (literal `bench-loopback`).
- Features `test-support` and `bench-loopback` in Cargo.toml match the guard FORBIDDEN_FEATURES and the docs. The release profile in ci-branch-protection.md matches Cargo.toml.
- Required checks fmt, clippy, test, deny, release-guard match the ci.yml job ids and commands exactly. The doc says NOT YET CONFIGURED. The bench-loopback tests are stated as not yet run, and the ci.yml comment agrees.
- Units (MiB), the targets table, the valid-run rule (at least 10 valid samples, exit codes) and the scenario table agree with EPICS E-1/E-8. Placeholders are marked PLACEHOLDER (host) and NOT DONE (50-URL list).

Non-blocking:
1. docs/BENCHMARK.md sec 4 says a release build "prints no marker" and the harness checks only `bench-loopback`, so a `test-support` marker on a shipped-kind binary is not refused by the harness (the guard covers releases). Consider one sentence noting this.
2. Quickstart step 4 tells readers to use `target/release/fetch-mcp`. It does not say to build it first (`cargo build --release --locked`). Add that.
3. docs/ci-branch-protection.md mentions `.github/dependabot.yml`; it exists, so OK. The `--features` guard mode is not documented in the doc (only in the script usage). Minor.
