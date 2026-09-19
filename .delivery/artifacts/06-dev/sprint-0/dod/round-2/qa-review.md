# QA review, Sprint 0 (D-7, E-1), DoD round 2

STATUS: NOT_DONE (1 blocking, several non-blocking)

## Verified by running
- `python3 bench/selftest.py`: SELFTEST PASSED (19 checks): known-memory idle/peak (+20 MiB seen), pass/fail exit codes, boundedness FAIL, early stop -> INVALID, <10 runs refused, wrong binary kind refused (both directions), unimplemented scenario refused, tampered fixture refused, --gate exit 3 off aarch64.
- `scripts/check-release-features.sh --self-test`: OK (clean passes; test-support and bench-loopback each fail on both tree and marker; unknown feature, garbage tree, unknown marker fail).
- Guard probes in a /tmp copy: `default = ["lb2"]`, `lb2 = ["bench-loopback"]` (renamed alias plus transitive enablement) -> guard FAILS on tree (bench-loopback, lb2) and marker. A renamed empty feature `sneaky` -> FAILS (allowlist). Missing binary -> exit 2. Guard is robust to rename/transitive cases.
- Units, targets (10,240 / 40,960 kB), median-of-10, VmRSS/VmHWM fields, ADVISORY_PASS labelling for non-gate runs match BENCHMARK.md and E-1 ACs.

## Blocking
B1. Boundedness (and gate completeness) can silently pass. In measure.py the boundedness check runs only if both the scenario and its `bounded_vs` reference (`g4a-5mib-full`) are in the run. Probe: `--scenario g4a-50mib-cl` with `STANDIN_TOOLARGE_ALLOC_MIB=30` (a 30 MiB blow-up on the 50 MB path) gave verdict ADVISORY_PASS, no `boundedness` key, empty `missed`. Under `--gate` the same run prints PASS. More generally `--gate` never requires the full gate set (idle, or all G4a scenarios), so a partial run yields `verdict: PASS`. Fix: when a scenario has `bounded_vs` and the reference is absent, mark INVALID/REFUSED (or auto-include the reference); under `--gate` require the complete scenario set for the claimed gate; add selftest cases for both.

## Non-blocking
N1. `--gate` override refusal is a denylist (LD_PRELOAD, RUST_LOG, MALLOC_*). `GLIBC_TUNABLES`, `LC_ALL`, `FETCH_LOG`, `PATH`, and any `FETCH_*` config env are still accepted, and later keys override PINNED_ENV. Use an allowlist under --gate.
N2. `g4a-50mib-cl` early-stop floor is `min_bytes=1`, so its valid-run check is nearly vacuous (acceptable by design for a Content-Length reject; note it).
N3. Nonexistent `--binary` raises an uncaught traceback (exit 1, which collides with "target missed"); `Server()` is created outside the try in `sample`. Idle/peak with a `--smoke` run exits 0 even though never valid for claims (CI may misread).
N4. Shipped-kind refusal checks only the `bench-loopback` literal; a binary carrying only the `test-support` marker is accepted as shipped. Selftest cannot exercise the real `--version` marker path (stand-in only; real crate has `version_line` unit test).
N5. Selftest `--gate` override checks only assert rc in (2,3); on this host they hit exit 3 first, so the override-refusal logic is unexercised.
N6. CI covers D-7 ACs on paper (fmt, clippy, test, deny, release-guard with self-test, SHA-pinned actions, --locked, hosted only, `-p` build). Branch protection is owner-configured and not verifiable; `cargo test --features bench-loopback` deferred to E-8 (stated in ci.yml).

## Honesty of unverified items
Dev reports (D-7, E-1, revision-1) state plainly: GitHub Actions not run, cargo-deny and cargo-audit not installed/not run, native aarch64 `--gate` path not run, no real MCP server measured, A-1 re-run not done (deviation recorded). I confirmed cargo-deny and cargo-audit are absent here and did not verify these either; they remain unverified.
