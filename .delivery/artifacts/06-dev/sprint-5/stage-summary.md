# Sprint 5 stage summary (record)

Sprint 5: Paginate, content types, G4b. Branch `sprint-5/paginate-content-types-g4b`, merged as PR #9, merge commit b633e60 (2026-09-21).

## Status
| Story | Pts | Status |
|---|---|---|
| A-5 | 3 | DONE (window, clamp, continuation, beyond-end, char boundaries, early stop, chunked in/beyond cap). |
| A-6 | 3 | DONE (content-type gating before any body byte, untyped-body sniffing, `raw=true`). |
| E-7 | 2 | NOT DONE. The owner's 50-URL and 10-URL lists were not supplied. A-4 therefore stays not Done (95%/50% checks unmet). |

## G4b
PASS on all four hosted native cells (CI run 35641694726, docs/BENCHMARK.md section 17): gating peak 4.38 to 6.06 MiB (target 40), shipped-binary idle 2.36 to 4.62 MiB (target 10), 50 MiB chunked window ratios at most 1.048 (bound 1.10). By the plan this closes the memory gate (G4a plus G4b). Not covered: E-7-dependent A-4 checks.

## Review rounds
Four DoD reviews (architect, QA, tech writer, PR code review) then a fix pass (commit 01cffe1) and a round-2 review: all approved. Reports: `dod/` and `pr-review/` in this folder.

## Deviations pending owner acknowledgement
1. Total-length footer only on continuation or beyond-end replies (deviates from ADR-006 item 4; first pages stay exactly as fetched).
2. G5 `too_large` proved via the 50 MiB chunked variant.
3. G1 duplicates g4a-5mib-full.
4. `text/*` extra types chosen by the developer.
5. G4a read-in-full scenarios redefined as window-at-end.

## Deferred follow-ups (not fixed)
- `g4b-window-start` `max_bytes` ceiling.
- `resolve_window` coupling.
- `hostile-attrs3` WINDOW.
- Header-less binary sniffing (a NUL-free binary body without a Content-Type can pass as text).
- Empty-page message wording.
- Possible timing flake in the bench selftest.
- Unit test for the clamp note.
- Parameter deserialisation error prefix: folded into A-7 (Sprint 6).
- BENCHMARK s17: "first run" wording, an unbalanced parenthesis, and run ranges that omit the 01cffe1 runs.

## Owner items still open
E-7 lists (impact: Sprint 7 E-5 needs the 10-URL smoke result; A-4 stays not Done), OQ-7, OQ-3 and OQ-4 (before Sprint 10), acknowledgement of the E-2 re-homed items and the deviations above, branch protection (`a3b-merge-gate` required), A-2 throwaway-config check, cargo-audit.
