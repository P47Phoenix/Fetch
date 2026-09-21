# Sprint 4 stage summary (Product Owner record)

Sprint 4: Convert and memory gate. Branch `sprint-4/convert-memory-gate`, merged as PR #8, merge commit c86f445.

## Status
| Story | Pts | Status |
|---|---|---|
| E-4 | 3 | DONE. G4a PASS in all four cells (amd64/arm64 x gnu/musl) on native GitHub-hosted runners. Evidence: docs/BENCHMARK.md section 16; runs 35562347553, 35563535561, 35633732179. This is NOT the memory gate closing (G4b, end of Sprint 5, closes it). |
| A-4 | 5 | Implemented, NOT DONE. The 95% conversion success, 50% median token reduction and no-`<script>` checks need the E-7 snapshot set, which needs the owner's URL lists (not supplied). |

## Evidence
- PR #8 merge c86f445; CI 14/14.
- Round-2 verdicts: architect DONE, QA DONE, PR review APPROVE (0 blocking each). Sources: `fix-pass-1-report.md`, `dod/`, `dod-round-2/`, `pr-review/`.

## Findings fixed during the sprint
- Depth-256 script/style leak (drop rule leaked past 256 open elements).
- Attribute-bomb memory bypass (ATTR_CAP 1024 pre-scan, `converter_limit`).
- bench.yml pipefail / no-op hole (A-4 step now fails on no-op).

## Owner items still open
- E-7: 50-URL list and 10-URL live smoke list not supplied.
- OQ-7 (licence). Fact feeding OQ-7 and D-6 third-party notices, not a decision: MPL-2.0 exceptions now exist via lol_html: cssparser, cssparser-macros, dtoa-short, selectors.
- Acknowledge the re-homed E-2 items: image measurement and tag trigger to D-2; g6 and G4b scenarios to E-4/A-5/A-6; macOS reader deviation.
- Branch protection: `a3b-merge-gate` required; `bench-gate` optional.
- A-2 throwaway-config check.
- cargo-audit never run.
- OQ-3 and OQ-4 due before Sprint 10.

## Follow-ups (not fixed)
- E-7 no-`<script>` check needs a rule for literal `<script>` text (img alt, `<xmp>`, escaped text).
- Early stop arrives with A-5.
- Sniffing untyped bodies is A-6.
- Evaluate lol_html 3.x (attribute memory accounting).
- Conversion overhead on musl not measured (gnu: 62.7 ms arm64, 75.0 ms amd64).
- p/li hidden-element exemption (optional-end/void tags).
- Self-nested drop-tag gap.
