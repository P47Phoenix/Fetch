# Docs fix report: Sprint 1 tech-writer DoD findings (PR #5)

Scope: docs only (README.md, docs/BENCHMARK.md, docs/ci-branch-protection.md, new docs/SSRF.md). No code, script, workflow or ADR edits.

## Blocking findings
- B1 README: rewritten (what it is, status, prerequisites, build, test, stdio session with initialize, tools/list, not_implemented, blocked_target and invalid_argument results, isError convention, FETCH_LOG). Do-not-register warning kept and made specific (before M3; manual Claude Code check on the owner's machine, throwaway config).
- B2 BENCHMARK: four stale skeleton claims replaced (read-this-first table, glossary, quickstart limitation, exit 2 row). New section 13 with real-product advisory numbers (x86_64 dev machine 3.15 MiB from A-2; hosted arm64 gnu 2.59 MiB, ready_ms about 1 ms), labelled advisory, not gates. --gate rules unchanged.
- B3 ci-branch-protection.md: guard section now lists all five steps (features, markers, no HTTP client crate, no raw socket crate, no tokio net), the `--no-http-client` option, a specific failure row, and the A-3b removal note.
- B4 docs/SSRF.md: readable IPv4, IPv6 and embedded-IPv4 tables with source RFCs, checked against src/ssrf/ranges.rs; links to ranges.rs and ADR-003; states only loopback is relaxable.

## Non-blocking
N1 check names already correct. N2 OQ-3/4/5/7 now listed in README. N3 "16 GB" changed to "16 GiB (as reported)". N4 not_implemented stated as temporary until A-3b (README). N5/N6 unchanged (fine).

## Verification (clean `git archive HEAD` extract of the docs commit)
cargo build --locked, cargo test --locked (42 + 10 pass), bench/selftest.py (SELFTEST PASSED), guard `--no-http-client` and plain run (guard OK), README session run verbatim: all five expected outputs matched, stderr empty; fixtures generate; all relative links and anchors in the four docs resolve; ranges.rs tables match docs/SSRF.md.
