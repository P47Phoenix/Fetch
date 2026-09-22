# Sprint 7 QA / docs review (PR #11, head ba8a3c9, base main f4f2c1b)

Reviewer: delivery-team:quality, fresh and read-only. Builds ran from a clean `git archive` under ~/.cache/qa7.

DECISION: NOT_DONE (one blocking defect in the E-5 report generator; everything else verified).

## Gates from clean git archive
- cargo fmt --check: OK. clippy -D warnings x3 (default, bench-loopback, test-support): clean.
- cargo test --locked x3: all pass (177 / 178 / 177 lib tests, 1 ignored each; integration tests pass).
- scripts/check-release-features.sh: "guard OK"; --self-test: OK. cargo deny check: advisories, bans, licenses, sources ok.
- python3 bench/selftest.py: SELFTEST PASSED.
- gh pr checks 11: 14/14 pass on the head.

## AC to test mapping
- B-3 6 fails / 5 succeeds: `b3_a_chain_of_five...` asserts 5 redirects succeed with 6 connections; a 6-hop chain gives TooManyRedirects and the sixth redirect is never followed.
- B-3 redirect to blocked address and non-http(s): new test covers 9 encoded/IPv6 targets with only hop 1 connecting. Private, metadata, scheme and userinfo hops are covered by older tests (refusal_*, resolver hop tests).
- B-3 hung resolver bounded by the deadline: the test uses a fake never-answering resolver on hop 1 and a redirect hop, asserts `timeout` and that a concurrency-1 slot is not leaked.
- B-2 decimal, hex, octal, short, ::1, mapped, ULA, 0.0.0.0, :: : 16 request-URL spellings refused with no lookup and no connection; 7 loopback spellings refused at the redirect hop. All AC classes are asserted.

## Mutation testing (each mutant applied to a copy of src; `cargo test --lib`)
| # | Mutant | Result | New Sprint 7 test(s) that failed |
|---|---|---|---|
| M1 | MAX_REDIRECTS 5 to 6 | killed | b3_a_chain_of_five |
| M2 | MAX_REDIRECTS 5 to 4 | killed | b3_a_chain_of_five |
| M3 | skip IP-literal check on Redirect origin (drop per-hop revalidation of literals) | killed (4 fail) | b3_redirect_to_every_encoded, b2_redirect_hop |
| M4 | drop hex-form detection | killed (9 fail) | b2_encoded_and_ipv6, b2_redirect_hop |
| M5 | drop fc00::/7 ULA row | killed (6 fail) | b3_redirect_to_every_encoded, b2_encoded_and_ipv6 |
| M6 | remove fetch deadline (timeout_at) | killed (5 fail; run hit the 60 s test timeouts) | b3_a_hung_resolver |
| M7 | drop IPv4 0.0.0.0/8 unspecified row | killed (10 fail) | b2, b3 new tests |
| M8 | octal parsed as decimal | killed (6 fail) | b2_redirect_hop only |
Observation on M8: `b2_encoded_and_ipv6_forms_are_refused_as_the_request_url` still passed, because it accepts `blocked_target` OR `invalid_argument` and the URL cross-check (defence in depth) rejects the mismatch. Its octal case does not independently prove the ssrf parser; other tests do.
No mutant survived. Only M6 exercises the hung-resolver test; a deadline skipped on the redirect hop alone was not separately mutated (the test's second phase targets that path by construction).

## E-5 harness probes
smoke.py: no list, 1-URL list, missing categories and non-https all give INVALID, exit 2; userinfo and unknown category refused (selftest); `--dry-run` summary carries `dry_run: true`. The script contains no invented URL (the only URL text is a docstring example on example.org).
e5_report.py exit codes (probed with hand-built inputs):
- all inputs met and real 10-URL smoke: release, exit 0 (intended)
- dry-run smoke: 2. 11-URL smoke: 2. advisory (non --gate) idle: 2. missing smoke: 2. nonexistent files: 2. a FAIL gate plus missing inputs: 1.
- D-1 re-measure statement present in every report; figures carry a binary column.
Defects found:
- BLOCKING B1: `CONVERT_1MIB bytes=1024 ...` (wrong-size overhead input) is accepted and yields "release", exit 0. The regex captures `bytes` but never checks it equals 1048576.
- NB: a real-run smoke summary with ok=0 of 10 still gives "release". Defensible under "non-gating" but the report should at least not release on 0/10.
- NB: the overhead line's `arch` is unchecked, so a QEMU/emulated measurement text (`arch=x86_64 (qemu)`) passes. Native-only is enforced for the --gate JSONL by measure.py, not for the overhead file. Smoke/gate summaries are forgeable hand-edited JSON; nothing ties the idle file to the shipped binary or the peak file to the bench build.

## Docs accuracy
- No document claims E-5 or A-4 Done. dev-report, sprint-plan Revision 16 and state.md (local, gitignored) all say E-5 OPEN, A-4 not Done, no URL list invented. Sprint 6 facts match: PR #10 merged as f4f2c1b (confirmed via gh), Sprint 6 dod files exist.
- The 6 of 8 pts deficit is disclosed accurately (B-3 3 + B-2 3 delivered; E-5 2 carried). Deviations (E-5 ACs 2-4 unmet, B-3/B-2 tests over existing behaviour, no CI wiring of smoke) are disclosed correctly.
- The hung-getaddrinfo limitation (blocking-pool thread not cancellable, persistent hang leaks threads) is disclosed, and stated as not fixed.
- NB: Revision 16 and the dev report say sprints 8 (6 pts) and 12 (7 pts) are the only ones with room. The plan table caps at 8 per sprint: sprints 9 and 10 (7 each) have 1 pt of room, 12 has 1 pt, and only Sprint 8 (room 2) can absorb a 2-pt E-5. The claim that 12 has room for E-5 is wrong.
- NB: docs/EPICS.md line 531 still reads "E-5 landed in Sprint 7" (pre-existing forward-looking text, not touched by the PR) and now conflicts with E-5 being open. README has no Sprint 7 status update.

## Summary
BLOCKING: 1 (report accepts a wrong-size CONVERT_1MIB measurement and can emit "release"). Fix: require bytes == 1048576 in `overhead()` (treat mismatch as missing) and add a selftest case; optionally reject arch text with qemu and a 0/10 smoke.
