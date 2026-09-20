# QA review round 2: A-3b / PR #6 (head 5c9a0dd)

Read-only. Clean `git archive HEAD` extract in /tmp/qa2; mutants in a scratch copy /tmp/mut2. Date 2026-09-20.

## Decision: NOT_DONE (one blocking item, documentation only; all code checks pass)

## Blocking
1. B1 (was round-1 B1, only half fixed). The native aarch64 5 MiB smoke really ran, but its result is recorded nowhere
   the AC asks for. The AC says "noted in the PR". Not in docs/BENCHMARK.md (unchanged by 5c9a0dd), not in the PR body or
   comments, not in the dev report, and the fix report named in the task (`A-3b/fix-pass-1-report.md`) does not exist
   (only `sprint-1/fix-pass-1-report.md`). The `gh run view --log` output contains no measurement numbers either: measure.py
   output is sent to /dev/null and the numbers go only to $GITHUB_STEP_SUMMARY and the `arm-bench-product-peak` artifact
   (which expires). Fix: add a short advisory, single-run entry to docs/BENCHMARK.md (or the PR body) and the sprint
   record, with the figures below, and write the missing fix report.

## Verified numbers (artifact `arm-bench-product-peak` of run 35523134933, downloaded and read)
Job bench-product-peak on ubuntu-24.04-arm: machine aarch64, kernel 6.17.0-1022-azure, 4 vCPU, 16 GB, page 4096,
Ubuntu 24.04.5. ELF check "ARM aarch64", binary sha256 a3c671ec..., `--version` shows the bench-loopback marker. Scenario
g4a-5mib-full, 10 fresh processes, 10/10 valid, VmHWM samples kB 4892 4752 4752 4896 4768 4896 4892 4896 4880 4636,
median 4.77 MiB (target 40, not a gate), summary ADVISORY_PASS, gating=false, exit 0. It is a single run on one runner
instance, on a bench-loopback build; label it advisory, not evidence for G4a. (Round-1 x86_64 figure was 5760 kB, same order.)
The job is a real fetch: harness self-test passed, and measure.py marks early-stop runs INVALID (0 invalid).

## Round-1 blocking items
- Code-review B-1 (map_transport URL steering): FIXED. `is_connect` is read first and `without_url()` applied before the
  text scan. New test `url_text_containing_header_never_changes_the_transport_class` covers path, query and an
  upstream redirect Location containing "header"/"too large"; all give `network_error`. Mutant "remove without_url()":
  test fails (killed).
- QA B1: see B1 above (job real and correct, recording missing).

## Non-blocking fixes landed
N-1 error.rs doc corrected (no port policy yet). N-2 one shared `ssrf::canonical_name` used by core, Pinned, cross_check;
unit test for `a.b..`. QA N4 cross_check test and wire-byte-cap gzip test, N5 truncated gzip / short Content-Length /
empty gzip (decided empty page) tests, N7 test-level timeout guard in `get()`, N1/N2 E-8 items re-homed in EPICS, N3
TLS caveat, docs (README, gate wording, ADR-001 note, OQ-5). All present in the diff.

Mutants (scratch, lib tests, test-support): M1 remove URL strip -> 1 test fails. M2 cross_check forced ok -> new
`cross_check_refuses_every_disagreement_with_the_core` fails (was a survivor in round 1). M3 canonical_name trims all dots
-> 2 tests fail (dns unit test, differential fuzz). All killed.

## Clean-extract verification
| Check | Result |
|---|---|
| fmt --check | OK |
| clippy --all-targets -D warnings: default, bench-loopback, test-support | clean x3 |
| cargo test --locked default / bench-loopback / test-support | 91+9 / 92+10 / 91+9, all pass |
| Timing repeat, lib suite x5 (test-support) | 5/5 pass (1.2-1.8 s) |
| release build --locked | OK |
| check-release-features.sh, --self-test | guard OK, self-test OK |
| bench/selftest.py | SELFTEST PASSED |
| PR #6 CI | all 10 checks pass, incl. a3b-merge-gate, bench-product-peak |

## Non-blocking
1. N1. Job summary/artifact are the only record of the measurement; consider printing the results line into the job log too (tee measure output) so `gh run view --log` shows it and it does not expire.
2. N2. Regression: none found. No overclaims in code or comments; BENCHMARK.md "read this first" table still says nothing about the product peak, so it is not overclaiming, only silent.
3. N3. Carry-overs from round 1 still open by design: branch protection for a3b-merge-gate (owner), E-7 (sprint level), TLS untested.
