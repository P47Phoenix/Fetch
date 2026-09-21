# Sprint 6 QA and docs review (A-7, B-1), PR #10 head ec9dab0

Decision: NOT_DONE (1 blocking, a false doc claim; code is sound).

## Gates (clean `git archive` of ec9dab0, build under ~/.cache)
- fmt OK; clippy -D warnings x3 (default, bench-loopback, test-support) OK.
- cargo test --locked x3 all pass (172 / 173 / 172 lib tests; stdio 9 / 15 / 9).
- release guard OK, guard --self-test OK, cargo deny OK (advisories, bans, licenses, sources), bench/selftest.py SELFTEST PASSED.
- CI on the head: 14/14 checks pass (gh pr checks 10; runs 35659245519 ci, 35659245653 arm-bench, 35659247200 bench).

## Real stdio (bench-loopback build, local HTTP server)
All results `isError: true` unless noted.
- missing/null url: `error[invalid_argument]: url: is required`; url 5: `url: must be a string`.
- max_length -1, 1.5, "9", 1e30, 2^64: `max_length: must be a non-negative whole number`; 0: `max_length: must be at least 1`.
- start_index -3 / "7": `start_index: must be a non-negative whole number`; raw "yes" / 1: `raw: must be true or false`.
- unknown extras: accepted (isError false, body returned). ftp scheme, empty url: `error[invalid_argument]: url: ...`.
- 404: `error[http_error]: the server refused the request with HTTP status 404 (Not Found); retrying ... unlikely to help`; 500: `the server failed with HTTP status 500 (Internal Server Error); it may work if retried later`.
- DNS (.invalid): `error[dns_failure]: hostname did not resolve`; timeout: `error[timeout]: the request timed out`; too_large: `error[too_large]: ...size limit`; image/png: `error[unsupported_content_type]: ...image/png...`.
- Blocked 10.0.0.1, 169.254.169.254, [fd00::1], [::ffff:10.0.0.1]: `error[blocked_target]: IP address is not public (<category>)`, no address echoed.
- Non-object `arguments` ([] , "s", 5): JSON-RPC -32601 `tools/call`, identical to main and as README documents; absent arguments: uniform `url: is required` (main gave the rmcp prefix). Server kept serving after every case, exit 0, stdout carried only JSON-RPC, stderr only the startup warn line.

## AC verdicts
- A-7 AC1 (flag + cause name, all seven causes): PASS (stdio above).
- A-7 AC2 (test asserts flag and text per cause): PASS (`a7_each_cause_is_flagged_and_names_itself`, `every_cause_sets_the_flag_and_names_the_cause`, `every_argument_failure_...`, stdio `bad_input_is_rejected_with_the_field_named` now asserts the exact prefix).
- A-7 AC3 (generic internal, keeps running, no stdout): PASS for Err paths, unit level only. Disclosed in EPICS: panic = abort. NOT disclosed: `Internal` is only unit-tested (no real path produces it in a running server; detail-to-stderr is not asserted by any test). See non-blocking 1.
- B-1 all four ACs: PASS. 17 addresses x (name, mixed public-first, mixed private-first, literal); refused as blocked_target, `srv.accepted() == 0`, lookups counted once per name, literals never resolved, message contains no address. Verified by running a mutation (see below). Caveat: uses FakeResolver, not the OS resolver (the OS path is covered by the existing `system_resolver_...` test).

## Schema
Advertised tool schema is NOT byte-identical to main: `url` gains `"default": null` (required is still `["url"]`; types, descriptions, max_length/start_index/raw unchanged). Harmless functionally, but EPICS states "the advertised schema is unchanged" (BLOCKING 1). Fix: drop the default on `url` (schemars attribute) or reword to "unchanged apart from a `default: null` on `url`".

## Mutation tests (on a scratch copy; all killed)
1. Swap 4xx wording to the 5xx wording: `a7_each_cause...` and `every_cause_sets_the_flag...` fail.
2. Remove the `max_length == 0` check: `every_argument_failure_...` fails.
3. `Internal` Display leaks the detail: `an_internal_error_is_generic_and_leaks_no_detail` fails.
4. (extra) `raw` boolean inverted: `every_argument_failure_...` fails.
5. (extra) resolver skips blocked answers in a mixed set: 5 tests fail incl. the B-1 test.

## Docs accuracy
- README error section matches observed behaviour (prefix, generic internal text, 4xx/5xx wording, remaining rmcp shapes for `arguments` non-object) : OK.
- EPICS: A-4 not Done, E-7 not done: consistent (sprint plan Rev 15, stage-summary). B-1 "no code change" true; A-7 status accurate except the schema claim and the omissions below.
- Sprint 5 record vs PR #9: merge commit b633e60, merged 2026-09-21, title, "four DoD reviews + fix pass 01cffe1 + round 2": consistent; G4b run 35641694726 all four cells success (it ran at head b182c4d, and BENCHMARK s17 says so); numbers match BENCHMARK. Plan Rev 15 re-run list (35562347553, 35563535561, 35633732179) is supported by the existing s16 text. PR #9 head 01cffe1 CI runs all success. No overclaim found.
- The commit message of c3ea062 says "state" but `.delivery/state.md` is gitignored, so it is not in the PR; its content (sprint_5 line, A-4 not Done) is consistent with the record.

## Blocking
1. EPICS A-7 status claims "the advertised schema is unchanged"; `url` gained `"default": null`.

## Non-blocking
1. Undisclosed: `FetchError::Internal` is only unit-tested and no current server path is exercised end to end; the stderr detail line and "no stdout write" are not asserted. Add one sentence to the A-7 deviation note.
2. Undisclosed design change: validation moved from serde `deserialize_with` into `FetchParams::parse` (fields are now `serde_json::Value` in a public struct); mention in the A-7 status or the ADR-006 amendment.
3. `.delivery/state.md` is gitignored; the "state" in commit c3ea062's subject is not reviewable from the PR.
4. B-1 test uses FakeResolver; OS resolver is covered separately (fine, worth a note).
