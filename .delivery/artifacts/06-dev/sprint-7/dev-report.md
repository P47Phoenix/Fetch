# Sprint 7 dev report (B-3, B-2, E-5)

Branch `sprint-7/redirects-encodings-report`; base main f4f2c1b (Sprint 6 merged).

## B-3 redirect limit and per-hop revalidation: implemented
Most of the behaviour already existed from A-3b/B-1 (manual redirect loop, `MAX_REDIRECTS = 5`, per-hop `validate_target`). New tests in `src/fetch/tests.rs`:
- `b3_a_chain_of_five_redirects_succeeds_and_a_chain_of_six_fails`: 5 hops succeed (6 connections); a 6th redirect gives `too_many_redirects` and is never followed.
- `b3_redirect_to_every_encoded_blocked_form_is_refused_without_a_second_connection`: decimal, hex, octal, short, IPv4-mapped (dotted and hex), ULA, unspecified as redirect targets give `blocked_target`; only hop 1 connects. (Existing tests already cover private, metadata, non-http(s) and userinfo targets.)
- `b3_a_hung_resolver_is_bounded_by_the_deadline_on_every_hop_and_frees_the_slot`: closes the A-3b fix-pass note. The fetch deadline (`timeout_at`) wraps the lookup on the first and redirect hops; the concurrency slot is released. Limitation stated, not fixed: a real `getaddrinfo` blocked in the tokio blocking pool cannot be cancelled when the future is dropped; it holds one blocking thread until the OS resolver returns. The pool default (512) far exceeds 3 concurrent fetches x 6 lookups, but a persistently hung resolver leaks threads until it times out. No code change was made; flagged for the owner/architect.

## B-2 encoded and IPv6 forms: implemented (tests)
- `b2_encoded_and_ipv6_forms_are_refused_as_the_request_url`: 16 spellings (decimal, hex, mixed-case hex, octal, single-number, short forms, `0`, `0.0.0.0`, `::1`, expanded `::1`, mapped forms, `fc00::/7`, `::`) are refused before any lookup or connection.
- `b2_redirect_hop_refuses_every_loopback_spelling` (default policy).
- Existing ssrf unit tests already cover the parser and range table.

## E-5 benchmark report and release decision: harness done, story OPEN
Added:
- `bench/smoke.py`: runs the owner's 10-URL list through the server (list file: `URL [category]`, categories tls/redirect/json/text/html). A real run is refused unless the list has exactly 10 distinct https URLs, no userinfo, and covers tls, redirect, json, text. `--dry-run` relaxes those rules and labels output `dry_run: true`. Non-gating.
- `bench/e5_report.py`: builds the report and decision from idle/peak `--gate` JSONL, the `CONVERT_1MIB` line and the smoke JSONL. Exit 0 release, 1 do not release, 2 not decided. A missing, advisory, dry-run or wrong-size input is never a pass. Always states the D-1 re-measure governs the tag and labels figures by binary.
- Self-tests in `bench/selftest.py` (already run in CI): dry-run against a local fixture server, list validation, and every decision branch on synthetic records.

Not done (blocked): the live 10-URL smoke run and a published "release"/gap report in `docs/`. No URL list was invented. Owner must supply: a list of exactly 10 public https URLs (no cookies/auth), covering TLS, at least one redirect, one JSON, one plain-text page. Plan impact: the plan defines no fallback; E-5 cannot close, Goals 2 and 3 have no live evidence, no release decision can be published, MVP tagging waits. Sprint 7 is 8 pts, so it lands at 6 delivered plus E-5 carried; Sprints 8 (6 pts) and 12 (7 pts) are the only sprints with room. The report can be generated from CI artifacts the moment the list exists (`smoke.py` then `e5_report.py`); CI wiring of the smoke step was deliberately not added because it needs the list committed.

## Deviations from AC
- E-5 ACs 2-4 (smoke result, "release" text, published report) not met: missing owner input.
- B-3/B-2: no production code change was necessary; the stories are delivered as test depth over existing behaviour.

## Owner decisions needed
OQ-3, OQ-4, OQ-7 remain open and were not decided. E-5 list (above). E-7 50-URL list still outstanding.
