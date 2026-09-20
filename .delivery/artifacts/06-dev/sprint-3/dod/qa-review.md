# QA review: Sprint 3 (E-2, E-3, A-9), PR #7, head 39379c1

Validator: delivery-team:quality, read-only, fresh. Decision: NOT_DONE (1 blocking, cheap to fix).

## Method
Clean `git archive HEAD` in /tmp/qa; CI logs read with `gh run view --job ... --log`; gate probed live with the release binary; A-9 mutants applied to the copy only.

## Clean-tree results (all pass)
- cargo fmt --check OK; clippy -D warnings x3 (default, bench-loopback, test-support) OK.
- cargo test --locked x3: 92+9, 93+11, 92+9 passed, 0 failed.
- release build OK; guard (`check-release-features.sh`) "guard OK"; `--self-test` OK.
- bench/selftest.py: 63 checks, SELFTEST PASSED. actionlint (run inside the repo): clean.

## Gate honesty (measure.py --gate), probed
- /bin/true, shell script printing a fetch-mcp version, bench-kind on shipped binary: refused, exit 2 (ELF/e_machine/version/marker identity).
- --runs 5, --smoke, target override, --settle/--parallel-idle/--child-env: exit 2. Non-native (QEMU handler / non-x86_64,aarch64): exit 3, checked after other refusals.
- Real binary, child killed after handshake in 1, 3 and 10 of 10 samples: 9/7/0 valid, verdict INVALID, exit 2 (never PASS). Honest run: 10/10, PASS, exit 0 (4.03 MiB idle on this amd64 host, consistent with CI 3.92-4.06).
- Selftest covers: early-stop INVALID, missing 5 MiB reference INCOMPLETE, peak/idle over target FAIL exit 1, boundedness >1.10 FAIL, non-gate pass = ADVISORY_PASS never PASS, gate PASS only with complete set, uncaught exception exit 2.
- MiB = 2^20 (kB/1024); median via statistics.median over valid samples; fewer than 10 valid => INVALID.
- Workflow: each gate step continue-on-error but a final step fails the job unless idle, idle-bench and peak all succeeded; fork PRs skipped; no pull_request_target.
Verdict: gate cannot pass on wrong, short or invalid data. Note: kill-after-handshake is caught only because /proc/<pid>/status disappears/lacks VmRSS; an INVALID sample is never dropped silently.

## Reported numbers vs CI logs
The dev report and BENCHMARK section 15 cite run 35538774565, whose head SHA is d635a67 (not 39379c1). Those numbers match the logs exactly: amd64 gnu 4.06/5.27, musl 2.27/4.37, arm64 gnu 3.55/4.66, musl 2.16/4.14; 10/10 valid, summary PASS, gating true; boundedness max 1.029 (<=1.03 holds); redirect-chain5 4.96/6.97/4.32/6.44 match.
The head run 35539714646 (39379c1) also PASSes all four cells with slightly different figures: amd64 gnu 3.92 idle/5.17 peak, amd64 musl 2.27/4.25, arm64 gnu 3.55/4.65, arm64 musl 2.16/4.20; boundedness max 1.015. The 39379c1 diff to bench.yml is only a trailing space on the cpu echo line.
Section 15 hardware line "amd64 Intel Xeon (8573C and 8370C seen)" is right for the cited run but the head run landed on AMD EPYC 7763 and Xeon 6973P-C, so runner CPUs vary.

## AC verification
E-2: single command harness, local fixture server, stdio drive, median of 10 with min/max, <10 valid rejected, MiB, fixtures with committed sha256 and refusal on mismatch, gzip, 50 MiB CL and chunked, late-landmark, redirect-chain5 (invalid if chain unread), shipped/bench identity and refusal rules, hosted matrix workflow (nightly, dispatch, same-repo PR) with pinned zigbuild/ziglang, native preflight refusing QEMU, gnu and musl: MET.
E-2 not met or partial: (a) CPU model not recorded on arm64 (see blocking); (b) harness runs bare binaries not the container image, and the script builds binaries not images; (c) macOS `/usr/bin/time -l` reader absent; (d) g6-concurrent10 and G4b scenarios are defined but implemented=False (refuse to run); (e) E-8 items: listed as open in the report, which the AC permits.
E-3: idle <= 10 MiB strict, shipped binary, 30 s after initialize + tools/list, non-zero exit with value and target on miss: MET on amd64 and arm64 (both libcs), 2.16-4.06 MiB.
A-9: MET. Header only after redirect; nothing otherwise.

## A-9 mutants (all killed)
1. header always (redirects==0 check removed): unit test `header_is_added_only_after_a_redirect` fails.
2. header never: same unit test fails.
3. final_url empty: `relative_redirect_is_followed...` fails on the new final_url assertion.
4. wrong status field: unit test fails.
5. (extra) call site drops with_header: stdio e2e test fails (needs `--features bench-loopback`, which CI runs).
Gap: nothing tests the header against a `max_length` window. It is outside the window structurally (applied after `window.into_text()`), but no test would catch a regression that moves it inside.

## Gap judgement
Acceptable deferrals: container-image measurement (D-2 builds the image; record in EPICS/plan so the E-2 AC and Sprint 3 exit "against the image" are formally re-homed, owner ack), g6 and G4b scenarios (owned by E-4 and A-5/A-6), macOS reader (no macOS gate host; note as a known AC deviation), load-avg repeat rule and dry-run discard (not in the E-2 AC text, only loadavg recording is; loadavg is recorded), public-host cross-check, binary size explanation, `--version` hash (E-4).
AC miss: arm64 CPU model.
Other: bench-gate not a required check (owner decision after a few runs, agreed); single-run evidence, variance unknown (two runs now exist and agree within ~0.15 MiB).

## Blocking
1. E-2 AC "CPU model recorded" is not met on arm64: all four arm64 runs logged `cpu:` empty, and the dev report/section 15 say the workflow "now also records CPU part", but the 39379c1 workflow change is only a trailing space. Fix the step (e.g. lscpu or /proc/cpuinfo "CPU part"/"Model name") and correct the claim.
