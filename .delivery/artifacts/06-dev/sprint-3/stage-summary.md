# Sprint 3 stage summary (Product Owner record)

Sprint 3: Harness and idle RSS. Branch `sprint-3/harness-idle-rss`, merged as PR #7, merge commit c13159e (2026-09-20).

## Status
| Story | Pts | Status |
|---|---|---|
| A-9 | 1 | DONE |
| E-3 | 2 | DONE (idle RSS strict gate on both hosted platforms) |
| E-2 | 5 | DONE, CONDITIONAL: four acceptance items re-homed, PENDING OWNER ACKNOWLEDGEMENT. The owner has not acknowledged them. |

Sprint 3 exit is met in the read-in-full form only. It is not a G4a pass.

## Evidence
- PR #7 merged, c13159e. CI 14/14 green at head 37b2823 (fix-pass 1 commit afb1613, run 35541676701 for gate numbers).
- Reviewers: architect DONE; QA round 2 DONE; PR review round 2 APPROVE.
- Sources: `fix-pass-1-report.md`, `dev-report.md`, `dod/`, `dod-round-2/`, `pr-review/`.

## Gate numbers (run 35541676701, head afb1613)
Gating peak is the read-in-full peak. Bare binaries, not the container image. Single run. NOT a G4a pass.

| cell | idle MiB (target 10) | peak MiB (target 40) |
|---|---|---|
| amd64 gnu | 3.90 | 5.17 |
| amd64 musl | 2.27 | 4.25 |
| arm64 gnu | 3.55 | 4.61 |
| arm64 musl | 2.16 | 4.20 |

## E-2 items re-homed (pending owner acknowledgement)
1. Container-image measurement and tag trigger with digest gate -> D-2 (Sprint 9).
2. `g6-concurrent10` and G4b scenario implementations -> E-4 (Sprint 4), and A-5 / A-6 for G4b runs.
3. macOS `/usr/bin/time -l` reader: not built, known deviation (no macOS gate host).
4. Architect NB1 (no hash pin for `ziglang`, cargo-zigbuild not lock-checked) -> D-2 acceptance criteria.

## Carry-forward: first-commit cleanup for Sprint 4 (before A-4 / E-4 work)
1. Move ADR-004 doc lines back above `content_encoding_is_gzip` in `src/fetch/mod.rs`.
2. `scripts/build-candidates.sh`: exact-match check of the cargo-zigbuild pin.
3. `measure.py`: `native_gate_host` should use the strict probe.
4. Host JSONL cpu is `unknown` on arm64: record the `lscpu` model.
5. Docs still say "two CI runs" and "pass on pull request #5": correct.
6. Add an e2e redirect test for userinfo strip.
7. Add a selftest for `report.py` malformed JSONL.

## Owner items still open
- E-7 URL lists (50-URL and 10-URL): NOT supplied. E-7 moves to Sprint 5 unless supplied before Sprint 4 entry.
- Branch protection: add `a3b-merge-gate` (bench-gate optional).
- Acknowledge the re-homed E-2 items above.
- A-2 throwaway-config check (Claude Code).
- cargo-audit: never run.
- OQ-3, OQ-4, OQ-7 remain open (not blocking Sprint 4).
