# Technical Writer review, Sprint 5 (PR #9, head cd99c95)

Decision: NOT_DONE (5 blocking, all stale or understated text; no overclaiming found).

## Verified accurate
- G4b figures in README, BENCHMARK s17, dev report, EPICS, plan Rev 14 match the logs of run 35641694726 (head b182c4d): gating peak 4.38/4.46/5.41/6.06 MiB, idle 2.36/2.59/3.91/4.62, chunked ratios max 1.048, all four cells PASS.
- cd99c95 runs (ci 35651351749, arm-bench 35651351874, bench 35651351728) are all success, including arm-bench `bench (gnu)`/`bench (musl)`.
- "Memory gate closed" is stated as the plan defines it (G4a + G4b, Sprint 6 may start) with the E-7-dependent A-4 checks, container image, macOS reader and branch protection explicitly excluded (README, BENCHMARK rows and s17, EPICS, plan). A-4 is stated NOT Done everywhere (E-7 lists missing).
- The five owner-ack deviations are disclosed in EPICS A-5, plan Rev 14, dev report and BENCHMARK s17.
- OQ-7 (four MPL-2.0 crates) is still open in README and ci-branch-protection.md. Branch protection "not configured" is stated consistently. MiB units are used.
- Tool description in README matches src/server.rs. Footers, clamp, beyond-end, content-type list and sniffing match the code (src/server.rs render, src/convert/mod.rs).
- Copy-paste example run at this head (debug build, offline target dir under ~/.cache): ids 1, 2, 4, 5 and the max_length 0 reply match the README text exactly; id 3 reached example.com (network was available) and returned the expected markdown. cargo test: 167 + 1 + 9 passed; selftest: SELFTEST PASSED.

## Blocking
1. README, "What has not been checked yet": says CI "pass on pull request #8", and the G4b bullet says the two arm-bench `bench` jobs "are red on the PR ... owner decision pending". At cd99c95 all arm-bench jobs are green and the owner decision was taken (spike measures idle only). BENCHMARK s17 already says "resolved"; README contradicts it.
2. Plan Revision 14 ends with "pending with the owner: the two red arm-bench jobs ... fix not made". Stale: fix made (arm-bench.yml, idle only). The dev report "CI" section still lists them as Fail (the addendum corrects it, the section above it does not).
3. BENCHMARK "Read this first" row 1 says most recent PR is #8 / bench-gate passed on #8. The same table says "G4b scenarios are not run (stubs only, section 5)", which contradicts the G4b PASS in the next clause and s17. Section 5 scenario table still says the g4b rows are "defined, args/windows are E-2".
4. Run-count statements: s17 says "One CI run per cell, one runner instance each", but bench also passed on this PR in runs 35639776795, 35643750672, 35643950832 and 35651351728 (cd99c95). The last shows gating peaks 6.16/4.38/5.45/4.46, outside the cited "4.38 to 6.06" range. State that reruns exist and their range, as done for G4a. Plan line 71 calls G4a "single run" while README/EPICS list three runs.
5. docs/ci-branch-protection.md "Read this first": "most recently #5, all five checks" is stale (six required checks incl. `a3b-merge-gate`, latest PR #9). It also describes arm-bench job `bench` without the idle-only change.

## Non-blocking
- EPICS "Re-homed from E-2" bullet still says G4b scenarios are "marked not implemented" (historic, add "since Sprint 5 implemented").
- BENCHMARK line 141 ("fetch scenarios need Sprint 3 wiring") and line 383 ("G4a, G4b still to do") are historic and could carry a pointer to s16/s17.
- README omits `application/x-javascript`, `ecmascript` and XHTML-as-HTML from the content-type list (the developer's "extras" text says javascript/ndjson/+json/+xml only).
- README says `--version` prints the git commit; a local build without git prints `commit=unknown`.
- Dev report ends "CI results on the final head are in the coordinator reply"; record them in the report (all green, listed above).
