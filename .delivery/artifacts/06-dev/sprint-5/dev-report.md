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
Fail (advisory, not required): arm-bench `bench (gnu)` and `bench (musl)`. They measure the Sprint 0 A-1 spike, which has no start_index/max_length, so the now window-based peak scenarios resolve as INVALID and the summary step then trips on the missing `metric` key. Fix needed in .github/workflows/arm-bench.yml (drop the spike peak measurement); an attempt to edit it was blocked by the permission classifier, so it is left for the owner.

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
See docs/EPICS.md (A-5 status) and plan Revision 14: Total-length footer only on continuation/beyond-end (ADR-006 item 4 deviation); G5 via the 50 MiB chunked variant; G1 duplicates g4a-5mib-full; text/* extras chosen by the developer; G4a read-in-full scenarios redefined as window-at-end. Also pending with the owner: the arm-bench spike jobs.
