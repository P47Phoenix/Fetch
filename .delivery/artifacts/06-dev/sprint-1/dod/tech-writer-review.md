# Technical Writer DoD review, Sprint 1 (A-2, A-3a), HEAD 0bcae4e

STATUS: NOT_DONE

Verified by running: `cargo build --locked`, `cargo test --locked` (40 + 7 pass), `python3 bench/selftest.py` (pass),
`scripts/check-release-features.sh --no-http-client` (OK), and the binary over stdio (initialize, then tools/call).
Observed: valid URL -> `isError` `error[not_implemented]: fetch is not implemented yet: ...`;
`http://127.0.0.1/` -> `error[blocked_target]: IP address is not public (loopback)`. Docs checked: README.md,
docs/BENCHMARK.md, docs/ci-branch-protection.md, architecture 6.1, ADR-006 amendment, src rustdoc, dev reports.

## Blocking

B1. README.md is stale and contradicts the code. It says the binary "has no MCP server yet, so it does nothing useful".
A-2 shipped a stdio MCP server with a `fetch` tool. README has no build, test or run-over-stdio steps, and does not
describe `fetch` behaviour (not_implemented for valid input, blocked_target / invalid_argument, isError convention).
The pre-MVP / do-not-register warning is present and must stay. Fails the "new contributor" criterion.

B2. docs/BENCHMARK.md contradicts the code in four places:
- Read-this-first table: "The `fetch-mcp` binary ... A skeleton. It has no MCP server yet."
- Glossary "Skeleton": "only prints its version. It has no MCP server yet."
- Quickstart "Current limitation" (line ~134): "A real measure.py run against it fails the handshake and exits 2 ... until A-2 lands."
- "What can go wrong" (line ~176): "Exit 2 ... server closed stdout ... The skeleton has no MCP server yet ... Nothing to fix."
A-2 has landed and the handshake now works. These would mislead a contributor who hits a different exit 2.
Also sec 4 says `--version` prints only the crate version (true, still fine).

B3. docs/ci-branch-protection.md: the release-guard is only half-documented. The `release-guard` row in "The checks"
does mention the no-HTTP-client check and that A-3b removes it (good). But the "Release-feature guard" section lists only
3 steps (build, feature allowlist, marker grep), the options table omits `--no-http-client`, and the "What can go wrong"
row for `release-guard` names only features/markers. A contributor whose `release-guard` fails on an HTTP crate has no
matching troubleshooting text. Script header confirms `--no-http-client` exists and A-3b deletes `HTTP_CLIENT_BAN`.

B4. The SSRF range table is not documented for a reader outside the source. Its source (ADR-003 `check_ip`: IANA
special-purpose registries plus cloud metadata addresses) appears only in the `ranges.rs` header comment and the A-3a dev
report. No README/docs page lists the blocked ranges, their source, or that only loopback is relaxable by test/bench policy.
Acceptable minimum: a short section (or link to ADR-003) in README or docs/. Source citation in rustdoc is fine and present.

## Non-blocking

N1. README does not give the check names in ci.yml (fine, they live in ci-branch-protection.md). Names there match the ci.yml
job ids exactly: fmt, clippy, test, deny, release-guard.
N2. Honesty caveats verified intact: cargo-audit never run; product gates unverified / no `--gate` run; branch protection
not configured; OQ-7 open (README, BENCHMARK, ci doc); OQ-4 open (BENCHMARK sec 4); 250 ms recorded not enforced
(BENCHMARK glossary + sec 12). OQ-3 appears only in architecture 6.1 (conditional); OQ-5 not mentioned in the reviewed docs.
Add a one-line open-questions list if the caller wants OQ-3/4/5/7 all visible to contributors.
N3. MiB units used consistently. One "16 GB" for the hosted VM RAM in BENCHMARK line ~391 is a machine spec, not a project
figure; consider "16 GiB" or leave with a note.
N4. Architecture 6.1 and the ADR-006 amendment match the code (invalid_argument shape `error[code]: field: message`, isError).
Architecture 6.1 lists `not_implemented` as a code only via error.rs; check it appears in the table if it is meant to be permanent
(it is an interim A-2/A-3a code). Suggest one note saying it is removed when A-3b lands.
N5. Plain-language style is kept in README, BENCHMARK and the CI doc (What / Who / What to do first headers present).
N6. src rustdoc is accurate (server.rs, main.rs, ranges.rs, ssrf/mod.rs); no code comment contradicts behaviour found.

## Fix list to reach DONE
1. Rewrite the README status paragraph and add: prerequisites, `cargo build --locked`, `cargo test --locked`, a copy-paste
stdio session (initialize, notifications/initialized, tools/call) with the two expected error outputs, the isError convention,
and keep "Do not register in a real MCP client".
2. Update the four stale skeleton statements in docs/BENCHMARK.md.
3. Add the no-HTTP-client step, the `--no-http-client` option and a failure row to docs/ci-branch-protection.md.
4. Add an SSRF blocked-range summary with source (ADR-003, IANA registries, cloud metadata) to docs.
