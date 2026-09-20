# Docs pass 2 report

Scope: README.md, docs/BENCHMARK.md, docs/ci-branch-protection.md. No code, CI or state files touched.

## Review findings (independent-review-4 tech-writer)
1. INVALID glossary no longer lists fixture mismatch; says it is a refusal (exit 2). Now lists failed handshake, matching the troubleshooting table.
2. Identity refusal documented as reachable only on aarch64 (exit 3 comes first elsewhere); glossary-style troubleshooting row says so. Verified against measure.py order (aarch64 check before identity).
3. Step 3 now says both builds overwrite target/release/fetch-mcp; rebuild or copy/rename.
4. Advisory verdicts now include INVALID and INCOMPLETE.
5. "A-2 and A-3a" replaced by A-2 (stdio server with the fetch schema, per EPICS). A-3a is SSRF core, not needed for the handshake.
6. ci-branch-protection: "default set and four feature sets (spread across three HTTP backends)".
7. README: one paragraph on unchecked items plus link; publish = false explained.
8. Not done (optional reference-section jargon glossary); left as is.
9. "Gate names versus scenario IDs" is now a heading.

## Stale statements
Hosted Actions and cargo-deny ran once on PR #2; all five checks passed on 740c0fb (run 35470977286); "one green run is not a trend". Still NOT run: cargo-audit, native aarch64, real MCP server measurement. Branch protection still NOT configured (row added to BENCHMARK status table).

## Verification (clean git archive HEAD plus edited docs)
- python3 bench/selftest.py: SELFTEST PASSED.
- All relative links and anchors in the three files resolve.
- Check names still match ci.yml job ids; section numbers and anchors unchanged.
