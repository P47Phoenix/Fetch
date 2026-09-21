# Sprint 5 dev report: A-5 pagination, A-6 content types, G4b

PR: https://github.com/P47Phoenix/Fetch/pull/9 (draft). E-7 is not in this PR.

## G4b (hosted native runners, run 35641694726, median of 10 valid runs, peak on the bench build)

| cell | gating peak MiB (limit 40) | in-cap ratio | beyond-cap ratio | verdict |
|---|---|---|---|---|
| amd64 gnu | 6.06 | 1.025 | 0.986 | PASS |
| amd64 musl | 4.46 | 1.006 | 1.000 | PASS |
| arm64 gnu | 5.41 | 1.000 | 0.981 | PASS |
| arm64 musl | 4.38 | 1.048 | 0.987 | PASS |

Both ratios are within 1.10 of the 5 MiB window-at-end peak in every cell. Shipped-binary idle (median of 10, VmRSS, limit 10 MiB): amd64 gnu 4.62, amd64 musl 2.36, arm64 gnu 3.91, arm64 musl 2.59 MiB, all PASS. Per-scenario peaks and ratios for every scenario are in docs/BENCHMARK.md section 17.
Targets were not changed. The plan says a G4b pass closes the memory gate (Sprint 6 may start). E-7-dependent checks (A-4: 95% conversion, 50% token reduction, no script content) remain undone, so A-4 is not Done.

## CI
Pass: fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench-product, bench-product-peak, all four bench-gate cells.
The two arm-bench `bench (gnu)`/`bench (musl)` jobs were red on the first heads (they measure the Sprint 0 spike, which has no start_index/max_length). Resolved by owner decision (see the addendum): they measure idle only since cd99c95.

## Fixes during CI
The unit-test job has no generated fixtures; bench/selftest.py now reads sizes from the committed manifest (commit b182c4d).

## Gaps
- E-7 not done; A-4 acceptance checks undone.
- "[Total length: N characters.]" is stated only on a continuation page or beyond-end reply (deviation from ADR-006 item 4, keeps first pages as fetched).
- Architecture G5 "5 MiB window beyond the cap" cannot give too_large (fixture is under the cap); the 50 MiB chunked variant is used.
- G1 duplicates g4a-5mib-full. G4a full-read scenarios were redefined as window-at-end.
- text/* extras (javascript, ndjson) allowlist is my choice.
- No macOS reader; container image not measured; branch protection not configured.

## Decisions needing owner acknowledgement
See docs/EPICS.md (A-5 status) and plan Revision 14: Total-length footer only on continuation/beyond-end (ADR-006 item 4 deviation); G5 via the 50 MiB chunked variant; G1 duplicates g4a-5mib-full; text/* extras chosen by the developer; G4a read-in-full scenarios redefined as window-at-end.

## Addendum: arm-bench spike jobs
Owner authorised (AskUserQuestion answer "Drop spike peak, keep idle"): .github/workflows/arm-bench.yml now measures only the spike idle figure in `bench (gnu)`/`bench (musl)` and the summary step tolerates records without `metric`/`valid_runs`/`runs`/`verdict`. The spike peak is no longer measured because the spike has no pagination; Sprint 0 peak evidence remains in git history and BENCHMARK s11/s15. No other job, action pin or permission changed; actionlint clean. Final-head results at cd99c95: all 14 checks green (fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench-product, bench-product-peak, bench (gnu), bench (musl), bench-gate x4; runs ci 35651351749, arm-bench 35651351874, bench 35651351728). bench-gate at cd99c95: gating peaks amd64 gnu 6.16, amd64 musl 4.46, arm64 gnu 5.45, arm64 musl 4.38 MiB; shipped idle 4.74, 2.37, 3.91, 2.59 MiB; G4b ratios at most 1.021; PASS.

## Fix pass (tech-writer, QA, code review, architect findings)
Done: README, BENCHMARK, plan, EPICS, ci-branch-protection staleness; run counts and ranges (BENCHMARK s17, verified against the CI logs of runs 35639776795, 35641694726, 35643750672, 35651351728); ADR-006 dated amendment; README notes (content types, sniffing and binary bodies, control characters, charset until A-8, empty-page message, `bench-loopback` for the stdio tests, error-prefix uniformity to A-7). Code: NB-1 bench/report.py tolerates partial records; NB-2 the clamp note appears only when the caller passed a max_length above the cap.
Skipped, with reasons: NB-4 (max_bytes ceiling on g4b-window-start) and NB-6/7 (moving hostile-attrs3 to WINDOW, resolve_window coupling) change harness validity semantics and were not re-verified; NB-3 refusal of binary magic in the untyped-body sniff path is a behaviour change for A-6 follow-up, stated precisely in the README instead; NB-5 documented (empty-page message kept).
