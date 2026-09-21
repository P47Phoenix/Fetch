# Sprint 6 stage summary (record)

Sprint 6: Clear errors, private-IP test depth. Branch `sprint-6/a7-error-taxonomy`, PR #10 (draft, not yet merged at the time of writing).

## Status
| Story | Pts | Status |
|---|---|---|
| A-7 | 3 | DONE with one deviation (below). Uniform `error[invalid_argument]: <field>: <why>` for JSON-object argument failures; HTTP 4xx/5xx wording (408, 425, 429 say retry after a delay); generic `internal` message with detail on stderr; flag and text tests per cause. |
| B-1 | 5 | DONE. The A-3a code already met every AC; added the end-to-end blocked-class test (17 addresses by literal, by name and mixed). No code change. |

## Review rounds
Code review (REQUEST_CHANGES, 2 blocking) and QA/docs (NOT_DONE, 1 blocking), fix pass 0b4f1e0 (429 wording, lowercase log level, schema `default` removed, test hygiene, a3b-merge-gate runs the b1 test), round 2 (2 doc-only blockers, README status text), fixed in a doc commit. CI 14/14 at 0b4f1e0. Reports: `dod/` and `pr-review/` in this folder.

## Deviations pending owner acknowledgement
1. A-7 AC 3: "unexpected internal error keeps running" holds for `Err` paths only; a panic aborts (`panic = "abort"`).
2. `internal` is unit-tested only (no input triggers it), so the stderr line and empty stdout are not tested end to end.
3. Argument validation moved from serde `deserialize_with` into `FetchParams::parse` (fields are `serde_json::Value`); non-object `arguments` still get rmcp's -32601.
4. `.github/workflows/ci.yml` edited: a3b-merge-gate also runs the b1 test (required-check set unchanged).
Plus the five Sprint 5 deviations still unacknowledged.

## Follow-ups
- Loopback positive control for the B-1 test in a bench-loopback build (skipped).
- E-7 lists still missing: A-4's 95%/50% AC and Sprint 7's E-5 (10-URL smoke) stay blocked; Sprints 6 and 7 are full, only 8 and 12 have room for E-7's 2 pts.
- OQ-3, OQ-4, OQ-7, branch protection, A-2 throwaway check, cargo-audit remain open.
