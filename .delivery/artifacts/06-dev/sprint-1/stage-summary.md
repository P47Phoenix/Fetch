# Stage 6 Development, Sprint 1: summary (routing metadata)
Status: DONE. Stories A-2 (3 pts) and A-3a (5 pts). Branch `sprint-1/skeleton-ssrf`, PR #5 merged to main as 090e86c (2026-09-20). Hosted CI 8/8 green.

## What shipped
- A-2: stdio MCP server skeleton exposing exactly one tool, `fetch` (url required; max_length, start_index, raw), input validation, error/`isError` convention, FETCH_LOG stderr logging, `not_implemented` result for valid calls. Reports: A-2/dev-report.md.
- A-3a: SSRF core (`ssrf::ranges` full table, IP-literal and URL-level checks, resolver filter that resolves once and refuses on any blocked answer, per-hop revalidation, fail-closed default `Policy`, `test-support` constructor). Docs: docs/SSRF.md, ADR-003 amendments. Reports: A-3a/dev-report.md, fix-pass-1-report.md, cleanup-report.md, docs-fix-report.md.

## Exit evidence
- tools/list exactly `fetch`; stdout purity test plus `deny(clippy::print_stdout)`.
- ready_ms about 1 ms (median 1.058 ms) on hosted aarch64; 250 ms target met with wide margin.
- Every range table-tested (including RFC 9780 dummy prefix added in cleanup); default policy fail-closed.
- Release guard is a required check; no HTTP client crate in the tree.

## Reviewer verdicts (from dod/, dod-round-2/, pr-review/)
- Round 1: architect DONE (0 blocking, 7 non-blocking; N1 and N2 must close before A-3b starts); developer DONE (0/4); PO ACCEPT WITH CONDITIONS (no blocking); QA DONE (no blocking); tech-writer NOT_DONE (fixed in docs-fix pass).
- Round 2: developer DONE (0 blocking, 4 non-blocking); QA DONE (0 blocking, 5 non-blocking); tech-writer DONE (0 blocking, 3 non-blocking).
- PR review: pr-review/pr5-code-review.md; cleanup commit 61fe86c addressed the cheap findings before merge.

## Deferred, non-blocking
- `bench-loopback` is inert: main uses `Policy::default()` (E-8, Sprint 2, wires it).
- No port policy yet.
- QA F1: rmcp deserialisation-error prefix not changed to `error[invalid_argument]:`; behaviour documented in README instead.
- IANA IPv6 special-purpose registry re-check was done from memory, not a network lookup.
- ISATAP and non-well-known NAT64 prefixes are accepted residuals (documented, not blocked).
- Architect N1 and N2 must be closed before A-3b starts (see dod/architect-review.md).

## Owner items
- Branch protection not configured (docs/ci-branch-protection.md).
- A-2 Claude Code check with a throwaway config not confirmed done.
- cargo-audit never run (nightly audit is D-3).
- amd64 gate mode in bench/measure.py not done.
