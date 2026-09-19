# Docs plain-language rewrite report

Scope: README.md, docs/BENCHMARK.md, docs/ci-branch-protection.md. No code, scripts, CI, PRD, EPICS, architecture or ADR touched.

## What changed
- Each doc opens with a 3-sentence "what is this / who needs it / what to do first".
- BENCHMARK.md: added a "what has and has not been checked" table, a 40-row glossary (VmRSS, VmHWM, MiB, median, gate, stand-in, loopback, SSRF, INCOMPLETE, ADVISORY_PASS, etc.), and a quickstart split into five numbered steps, each with command and success signal. Added a "What can go wrong" table (symptom, meaning, fix). Long paragraphs in sections 3, 4, 7 turned into lists and tables. Section titles and numbers 1-10 and the `#quickstart-contributors` anchor are unchanged (Cargo.toml cites "sec 8"; bench/*.py cite the file).
- ci-branch-protection.md: verification-status table, plain-meaning column on the checks table, an "Owner quickstart" (7 steps), a "What can go wrong" table, definitions of pin, locked, positive control, RustSec. Check names still exactly `fmt`, `clippy`, `test`, `deny`, `release-guard` (match ci.yml job ids).
- README.md: 3-sentence opener, status, licence note, and a "where to go next" table incl. a link to the new glossary.

## Facts preserved
Targets 10 MB idle / 40 MB peak, 1.10x boundedness, all exit codes 0/1/2/3, `--gate` refusal rules, binary identity rules, marker contract, bench-vs-shipped bounds, 5 MB cap and fixture sizes, A-1 figures, OQ-7 undecided (Apache LICENSE exists, publish=false), 50-URL list deferred, skeleton not to be registered, ADVISORY_PASS is not a result, hosted Actions/cargo-deny/cargo-audit/native aarch64 never run. The G7a zero-byte floor caveat is kept.

## Commands run to verify
`python3 bench/selftest.py` (SELFTEST PASSED, ~33 s), `fixtures.py generate`, the step-4 stand-in smoke run (exit 0, ADVISORY_PASS, 3 JSON lines, under 1 s), a `--gate` run on this x86_64 host (REFUSED, not aarch64), the skeleton binary (INVALID, `server closed stdout`, exit 2), `cargo fmt --check`, `scripts/check-release-features.sh --self-test` (OK), markdown link and anchor check (all resolve).

## Trade-offs
- Docs are longer (BENCHMARK ~2.7k to ~4.6k words) because terms are defined and steps spelled out. Detail sections 1-10 stay as reference so referrers do not break.
- The glossary is unnumbered so section numbers are stable.

## Assumptions
- "What can go wrong" fixes for gate refusals follow the documented rules only. The `deny` row says a first failure could be a set-up problem, since it has never run.
- Step 5 and the gating flow could not be run here (needs aarch64); described from the existing rules, and the REFUSED behaviour on x86_64 was confirmed.

## Risks
- Redundant wording increases the number of places to update if a rule changes (for example the exit-code table appears once, but the refusal list appears in step 5 and the problems table).
- Removed a verbatim duplicated "Binary identity" paragraph in BENCHMARK.md quickstart step 5 (identical text, no information lost).
- The glossary definition of INCOMPLETE ("also used when a gating run leaves out scenarios") is a loose paraphrase: the code reports that as a refusal with reason `incomplete`, exit 2. Worth a reviewer check.

## Open questions
- None new. OQ-7, OQ-4, OQ-9 and the 50-URL list remain open as before.

## Readability (rough: mean words per sentence, prose only; longest sentence)
| File | Before | After |
|---|---|---|
| BENCHMARK.md | 19.3 (max 88) | 11.5 (max 39) |
| ci-branch-protection.md | 15.3 (max 42) | 10.0 (max 38) |
| README.md | 11.0 (max 20) | 11.8 (max 24), but 37 to 193 words of orientation |
Not a formal readability score; a proxy only. No external beginner test was done.
