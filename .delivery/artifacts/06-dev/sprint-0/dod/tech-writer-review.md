# Technical Writer DoD review, Sprint 0 (D-7, E-1)

STATUS: NOT_DONE

## Verified
- `python3 bench/selftest.py` runs and prints SELFTEST PASSED (15 checks, rc 0). `bench/measure.py --help` works and matches BENCHMARK.md exit codes. `bench/fixtures.py generate` works. `scripts/check-release-features.sh` usage matches the doc.
- Unit (MiB), targets, valid-run rule (>=10 valid, early-stop check), scenarios table, and placeholders (runner/OS/RAM/CPU marked PLACEHOLDER) are clear and consistent with EPICS E-1/E-2.
- ci-branch-protection.md lists five checks (fmt, clippy, test, deny, release-guard) that exactly match ci.yml job ids and commands. Pins listed exist (rust-toolchain.toml, deny.toml).

## Findings
1. BLOCKING: marker/`--version` contract contradicts the code. BENCHMARK.md sec 4 and EPICS E-8/D-7 say the marker string `bench-loopback` appears in the binary and in `--version`. The guard script greps `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1`, and src/main.rs has only `--build-info`, no `--version`. measure.py keys on `bench-loopback` in `--version`. Docs must state the real, single contract (or mark it as unreconciled, owned by E-8) and not claim the guard greps `bench-loopback`. Same finding as adversarial-review.md.
2. NON-BLOCKING: BENCHMARK.md has no contributor quickstart. It mentions `python3 bench/selftest.py` in one line but not prerequisites (Python 3, Linux only), runtime, expected output, or that self-test uses a stand-in server and its figures are not product measurements. Step 3 of the aarch64 procedure uses `--scenario ...` with no worked example.
3. NON-BLOCKING: BENCHMARK.md sec 5 says `bench/fixtures/` is regenerated with `fixtures.py generate`, but does not say to do this before the first run (measure.py refuses on a missing or mismatched manifest); the selftest handles this itself.
4. NON-BLOCKING: BENCHMARK.md is dense (single-paragraph sec 4 and 7). A short glossary of gate names (G0/G4a/G4b vs scenario IDs G1..G7) up front would help new readers.
5. NON-BLOCKING: ci-branch-protection.md does not say the `test` check runs no bench-loopback tests yet (only a ci.yml comment), and says the owner "records it in the sprint PR" without a template. Dev report notes branch protection is not yet configured; the doc does not mark it pending.
6. NON-BLOCKING: BENCHMARK.md sec 10 recommends GO but the E-1 dev report lists the 50-URL list, A-1 re-run as NOT DONE; sec 9 flags the URL gap, sec 10 flags the re-run. Consistent.
