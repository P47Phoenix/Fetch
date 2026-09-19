# QA review, round 3 (D-7, E-1)
STATUS: DONE

Probed: bench/selftest.py PASSED (all cases ok). Covered: missing 5 MiB reference -> INCOMPLETE exit 2; partial G4a under --gate refused; wrong binary kind; <10 runs; early stop INVALID; --gate overrides/env (GLIBC_TUNABLES, FETCH_*, LC_ALL, PATH, LD_PRELOAD, settle, parallel); nonexistent binary exit 2 no traceback; test-support and MARKER on shipped-kind refused; peak, idle and boundedness FAIL exit 1; ADVISORY_PASS never PASS. Manual re-probes: --gate --child-env exit 2; bench kind on unmarked stand-in exit 2; idle --smoke gives FAIL/advisory with note "never valid for NFR claims".
Median of >=10 valid runs, VmRSS (idle) / VmHWM (peak) from /proc, MiB=2^20 with 10/40 MiB targets, verified in measure.py; +20 MiB allocation calibration cases pass.
Release guard: allowlist-only tree check (renamed feature and unparseable output fail), marker grep catches renamed/aliased features, positive-control self-test builds each forbidden feature and requires both reasons. Transitive enablement surfaces in the root feature list of `cargo tree -e features`.
CI: fmt, clippy (default and bench-loopback), test, deny, release-guard plus self-test; SHA-pinned actions, hosted runners.

## Non-blocking
- Unverified, honestly stated in revision-2 report: GitHub Actions run, cargo-deny/cargo-audit, native aarch64 --gate, real-binary harness run (stand-in only); real --version marker path only in Rust unit test.
- A --smoke/non-gate ADVISORY_PASS exits 0; documented in BENCHMARK.md (advisory, not NFR-valid) and in the JSON note; harmless but a CI consumer must not treat rc 0 as a gate.
- Sec 11 delta bounds not script-enforced (owned by E-8); cargo test --features bench-loopback deferred to E-8.
- Guard tree check inspects only the root package's features (sufficient since the root is what builds; no dependency can turn on root features).
