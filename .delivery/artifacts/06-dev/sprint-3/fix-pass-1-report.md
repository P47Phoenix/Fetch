# Sprint 3 fix-pass 1 report (PR #7)

Fix commit afb1613 (after 39379c1). Verified from a clean `git archive HEAD`: fmt, clippy -D warnings x3 configs, cargo test --locked x3 (95+9, 96+11, 95+9), release build, guard and `--self-test`, bench/selftest.py (SELFTEST PASSED), actionlint clean.

## Blocking
1. arm64 CPU model: bench.yml now records `cpu:` from `lscpu` Model name (cpuinfo and implementer/part fallbacks) plus a `cpu_id:` line. Verified in CI run 35541676701 (head afb1613): arm64 gnu and musl `cpu: Neoverse-N2`, `cpu_id: implementer=0x41 part=0xd49 vendor=ARM`; amd64 `AMD EPYC 7763 64-Core Processor` and `Intel(R) Xeon(R) 6973P-C`. The false "now also records CPU part" claim is corrected in the dev report (gap 6) and BENCHMARK s15 (appended correction).
2. Stale docs: README status, BENCHMARK "Read this first" table, s3 host table (amd64 column filled from logs), glossary, s7, quickstart step 5 / identity text (native x86_64 valid) all updated to match s15 (gate runs exist in read-in-full form, not a G4a/G4b pass, image not measured).

## Non-blocking done
- s15 and dev report cite both runs: 35538774565 measured d635a67 (amd64 Xeon 8573C, 8370C), 35539714646 measured 39379c1 (amd64 AMD EPYC 7763, Xeon 6973P-C); "only one CI run" removed.
- A-9: header format documented in README (no tool description change: the string is unchanged); EPICS A-9 marked done with the format; new test `header_is_outside_the_max_length_window`; userinfo and fragment stripped from the echoed URL (`echo_url`, tests for both). Unmarked-plain-text caveat documented (README, EPICS).
- Review (4): `native_host(strict=True)` on the --gate path fails closed when binfmt_misc is unreadable; selftests added.
- Review (1): idle-bench step advisory (`[ "$rc" -le 1 ]`), label updated.
- Review (2): chose "documented as intended": redirect-chain5 is excluded from the gating peak figure but stays fail-closed on validity and its own target; comments, s15 note, selftest added.
- Review (3): fork-PR-skip caveat and cost/quota note next to the required-check invitation in docs/ci-branch-protection.md.
- Review (5) documented; (6) report.py skips malformed JSONL lines with a stderr message and the summary step is continue-on-error; (7) build-candidates.sh checks `cargo-zigbuild --version`; "chain not read" -> "not followed"; architect NB1 recorded in D-2 AC; s7 stale text fixed.
- Re-homing (container image and tag trigger -> D-2; g6/G4b -> E-4 with A-5/A-6; macOS reader known deviation) recorded in docs/EPICS.md and sprint-plan Revision 11 as PENDING OWNER ACKNOWLEDGEMENT (not acknowledged).
- dod/ and pr-review/ folders committed.

## CI (run 35541676701, head afb1613): all PR #7 checks green
fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench (gnu, musl), bench-product, bench-product-peak: pass. bench-gate: amd64 gnu 15m52s, amd64 musl 14m48s, arm64 gnu 15m34s, arm64 musl 15m27s, all pass, `--gate`, summaries PASS.

| cell | CPU | idle shipped MiB (target 10) | gating peak MiB (target 40) |
|---|---|---|---|
| amd64 gnu | AMD EPYC 7763 | 3.90 | 5.17 |
| amd64 musl | Xeon 6973P-C | 2.27 | 4.25 |
| arm64 gnu | Neoverse-N2 | 3.55 | 4.61 |
| arm64 musl | Neoverse-N2 | 2.16 | 4.20 |

Read-in-full peak, not a G4a pass; bare binaries, not the image.
