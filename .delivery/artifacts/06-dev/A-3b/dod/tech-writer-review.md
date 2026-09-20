# Tech Writer review, A-3b, PR #6 head c053d8c (read-only)

DECISION: NOT_DONE (2 blocking overclaim/omission items, both small doc fixes)

## Verified accurate
- README copy-paste stdio session runs at this head: initialize (rmcp 3.4.0), tools/list (description "Fetch a URL and return its content as markdown"), blocked_target loopback text, invalid_argument text, live https fetch of example.com returns text. Error code list matches src/error.rs (11 codes). Config default 15 s / FETCH_MAX_BYTES match src/config.rs.
- README Status honestly says branch protection is off, no --gate run, OQ-5 no label (2026-09-20), HTML->markdown not done.
- docs/ci-branch-protection.md: "Rule on main NOT YET CONFIGURED"; a3b-merge-gate is listed as an owner step (Owner quickstart step 4, six checks). Not claimed configured. ci.yml job a3b-merge-gate exists and matches the described tests.
- docs/BENCHMARK.md line 17 and section 13 correctly say idle RSS predates the client and must be re-measured; arm64 figure is advisory, A-2 build. No stale idle figure presented as current for A-3b.
- PRD OQ-5, EPICS B-6/OQ table, ADR-006 amendment 2026-09-20: consistently "no label", B-6 won't-do.
- docs/SSRF.md consistent with src/ssrf (not re-derived range by range; no A-3b changes to it).

## BLOCKING
1. TLS not covered by any automated test is documented nowhere in README/docs (only in dev-report). README says "https with the built-in trust anchors" and ci-branch-protection says the gate "proves the client dials only SSRF-validated addresses" with no TLS caveat. Add a one-line caveat: TLS/certificate validation was verified manually only.
2. docs/ci-branch-protection.md line ~100 says a3b-merge-gate "proves" the dial-only-validated property, but part of a3b_merge_gate is a heuristic source scan (no socket types, lookup_host only in fetch/dns.rs, one dns_resolver call). Reword to "checks", and state the scan is heuristic (a string/grep check, not a proof).

## NON_BLOCKING
1. architecture.md section 1 decisions table (line 28) and OQ table (line 369) still say OQ-5 "Open"; only the 2026-09-20 amendment (line 552) records "no label". Also lines 62/403/464 keep the OQ-5 render hook wording. Add "RESOLVED: no label" inline.
2. README "What to expect" lists replies 1-5 in request order, but real output puts id 3 (network fetch) last, after ids 4 and 5. Say replies may arrive out of order; match by id.
3. No aarch64 RSS with the client compiled in is stated in BENCHMARK (good) but README does not mention that the A-3b RSS smoke (x86_64, 5.7 MiB VmHWM, non-AC) was substitute only; fine to leave in dev-report.
4. tools/list description still says "as markdown" while output is unconverted; README notes it, consider a note beside the description bullet.
