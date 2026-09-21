# QA review, DoD round 2, Sprint 3, PR #7 (head 37b2823)

Decision: DONE (0 blocking, 6 non-blocking). Read-only review; verified from a clean `git archive HEAD` in /tmp/qa2.

## 1. QA blocker: arm64 CPU model
Run 35541676701 (head afb1613, success, all 4 bench-gate cells): "Platform facts" step logs `cpu: Neoverse-N2` and `cpu_id: implementer=0x41 part=0xd49 vendor=ARM` in both arm64 cells; amd64 gnu AMD EPYC 7763, amd64 musl Xeon 6973P-C. RESOLVED.
Caveat (NB-1): the JSONL `host` record written by measure.py still has `"cpu": "unknown"` on arm64 (4 lines, both arm64 cells). The model is captured only in the workflow log step, not in the evidence artifact.

## 2. Docs consistency and figures
- Figures in s15 correction match the run logs exactly (idle/peak MiB): amd64 gnu 3.90/5.17, amd64 musl 2.27/4.25, arm64 gnu 3.55/4.61, arm64 musl 2.16/4.20; all PASS, 10/10 valid. Run 1 = 35538774565 = d635a67 and run 2 = 35539714646 = 39379c1 verified via `gh run view`. Run 3 head is afb1613 as stated.
- 37b2823 vs afb1613 diff: only fix-pass-1-report.md and one BENCHMARK.md line (the CPU correction); nothing contradicts CI.
- Grep for stale "not yet measured / no gate run": none left. README status, BENCHMARK Read-this-first table, glossary, s3/s7/quickstart are consistent with s15 (gates ran in read-in-full form, not a G4a/G4b pass, container image not measured).
- Remaining staleness (NB-2): README, BENCHMARK table rows 18-19 and the s15 header/anchor still say "two CI runs" / cite only 2 runs; run 3 appears only in the correction paragraph. README line 18 says CI "pass on pull request #5" (stale, now #7). Not misleading about the gate, just lagging.
- Section 15 first paragraph still reads as "arm64 CPU model NOT recorded" then points to the correction; acceptable, labelled as an appended correction.

## 3. Overclaim and deferrals
No overclaim found: peak explicitly "NOT a G4a pass", G4b/g6 not run, E-8 cross-check open, bare binaries not image. Deferrals (container image + tag trigger -> D-2; g6/G4b -> E-4 with A-5/A-6; macOS reader deviation) are recorded in docs/EPICS.md (lines 106, 433), sprint-plan Revision 11 and the fix report as PENDING OWNER ACKNOWLEDGEMENT, not acknowledged. OK. The owner still has to acknowledge them (process item, not a code defect).

## 4. New code
- echo_url: mutation test, removed each of set_username / set_password / set_fragment separately: 3/3 killed (`echoed_url_has_no_userinfo_or_fragment` fails in all; `redirect_fragment_is_not_echoed_in_final_url` also fails for the fragment mutant). Note the userinfo mutants are killed only by the unit test, not by the redirect end-to-end test (NB-3).
- native_host(strict): probe: unreadable dir + strict -> (False, "cannot read ... native execution is unproven"); non-strict -> ok; qemu handler -> False. Gate path (measure.py:280) uses strict=True and refuses with exit 3. OK.
- redirect-chain5: scenarios.py comment, bench.yml header comment, BENCHMARK s15 note and selftest all agree: gate "none" (excluded from gating peak figure) but fail-closed on INVALID and on median over target. Selftest covers valid, non-following client -> INVALID rc 2, and summary listing. Consistent.
- report.py: a line that starts with `{` but is not valid JSON is skipped with a stderr note, no crash; file-open errors return empty. Probed with a mixed file, rc 0. No selftest covers this (NB-4).
- bench.yml idle-bench step: `[ "$rc" -le 1 ]`, so exit 1 is advisory and 2/3 fail; comment, s15 note match. Logged `idle_bench_exit=0` in all cells.

## 5. Clean-archive checks (all pass)
cargo fmt --check OK; clippy -D warnings x3 (default, bench-loopback, test-support) clean; cargo test --locked x3: 95+9, 96+11, 95+9 passed, 0 failed; release build OK; check-release-features.sh "guard OK" and --self-test "self-test OK"; bench/selftest.py "SELFTEST PASSED"; actionlint clean (no output, exit 0).

## 6. CI status
`gh pr checks 7` returns nothing (PR rollup empty at the time). Runs for 37b2823 (ci, bench, arm-bench) were IN PROGRESS when checked, so the head commit has no completed result yet. Last completed runs (ci, bench, arm-bench) are on afb1613, all success; 37b2823 is docs-only relative to it. Merge should wait for the 37b2823 runs to go green (NB-5).

## Non-blocking
1. JSONL host record still has cpu "unknown" on arm64; only the workflow log has Neoverse-N2.
2. Docs say "two CI runs" / cite two runs in README, table rows 18-19 and s15 heading; README says "#5".
3. Userinfo stripping is only pinned by the unit test, not an end-to-end redirect test.
4. report.py malformed-line tolerance has no selftest.
5. CI on head 37b2823 not yet complete at review time.
6. Doc comment above `echo_url` (src/fetch/mod.rs ~316) is prefixed by the orphaned "Accept absent, identity, or exactly one gzip" lines that belong to `content_encoding_is_gzip`; cosmetic.
