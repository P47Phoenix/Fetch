# QA review: A-3b and E-8 (PR #6, head c053d8c)

Role: QA validator, read-only. Date 2026-09-20. Clean `git archive HEAD` extract in /tmp/qa; mutation copy in /tmp/mut (repo not modified).

## Decision: NOT_DONE (one blocking item; everything else passes)

## Blocking
1. B1. The A-3b AC and the Sprint 2 exit criterion require a non-gating manual RSS smoke (one 5 MiB fetch, VmHWM) on native aarch64, noted in the PR. It was not done (host is x86_64; dev report gap 1). The x86_64 figure (idle 3884 kB, VmHWM 5760 kB) is honest but is not the AC. Fix: run it on the hosted arm64 runner (`ubuntu-24.04-arm`, ADR-007) or on a Pi, and note it in the PR; or record an explicit owner waiver in the PR and sprint plan. Status is effectively CODE_COMPLETE (structural checks all pass, empirical criterion pending).

## Non-blocking
1. N1. E-8 `--version` commit + Cargo.lock hash AC is deferred "to D-3", but D-3 is "Test suite on aarch64" and has no such AC. Deferral is acceptable for Sprint 2 (nothing in Sprint 2 exit needs it; only G4a/E-4 compares the two binaries) but it is currently unowned. Re-home it as an explicit AC in E-2 or E-4 (must land before G4a, end of Sprint 4) and correct the note in `src/lib.rs` and the E-8 dev report.
2. N2. E-8 items needing the harness (handshake marker, idle-RSS delta, size delta, public-host cross-check) are correctly not testable yet; add them to the E-2/E-4 AC map so they are not lost.
3. N3. TLS has no automated test (manual only against example.com/httpbin.org). Accepted by design (no fixture CA); certificate validation regression would go unseen. Track in B-2/B-5 or E-2.
4. N4. Mutation survivors: (a) `cross_check` forced to succeed: no test fails (backstop is untested through the client; the differential test covers the core side). (b) Removing the wire-byte cap: no test fails (redundant for identity, only matters for gzip wire size). (c) Dropping the userinfo check in `cross_check`: equivalent mutant (`check_url` already refuses). Add one client-level test for (a) and one for a gzip body larger than the cap on the wire.
5. N5. Truncation is only unit-tested at the body layer (`gzip_corrupt_and_truncated_fail`). No client-level test for a truncated gzip (`bad_response`) or a short-Content-Length identity body (`network_error`). Empty body with `Content-Encoding: gzip` (0 bytes) is accepted as an empty page (`finish` skips when fed == 0); decide whether that should be `bad_response`.
6. N6. Header byte limit is enforced only after parse (32 KiB post-check; hyper buffer about 400 KiB, reqwest 0.13.5 exposes no setter). Header bomb fails cleanly (test passes with a 2 MiB header), so the AC is met; memory bound is about 400 KiB per hop. Acceptable, keep the ADR note.
7. N7. Timing tests use 400 ms deadlines; 10 full runs plus 5 bench-loopback runs and one run under 8 busy-loop CPU hogs: zero failures. Mutation removing the deadline hangs the suite rather than failing an assertion (detected only by timeout); add a test-level `tokio::time::timeout` guard.
8. N8. E-7 (50-URL snapshots, 2 pts) is in the Sprint 2 exit criteria but not in this PR; a sprint-level, not a story-level, gap (flagged in the dev report; the sprint plan's overflow rule covers it).
9. N9. Branch protection for `a3b-merge-gate` must be configured by the owner (documented in `docs/ci-branch-protection.md`, six checks); until then the gate is advisory.

## Verification (clean archive extract)
| Check | Result |
|---|---|
| cargo fmt --check | OK |
| clippy --all-targets -D warnings: default, bench-loopback, test-support | clean x3 |
| cargo test --locked default / bench-loopback / test-support | 85+9 / 86+10 / 85+9, all pass |
| Flakiness: 10x full suite (test-support), 5x bench-loopback, 1x under CPU load | 0 failures |
| cargo build --release --locked | OK |
| scripts/check-release-features.sh, --self-test | guard OK, self-test OK |
| bench/selftest.py | SELFTEST PASSED |
| PR #6 CI (gh): fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench x3 | all SUCCESS |
| Direct dependencies | 8 of 15 (NFR-05 OK); reqwest, rustls, webpki-roots, flate2, tokio pinned exact (`=`); `url` dev-only |

## Mutation spot checks (scratch copy, lib tests with test-support)
Killed: Pinned resolver accepts any name (a3b_merge_gate fails); Content-Length check removed; redirect revalidation weakened (3 tests fail); stacked-encoding check removed; semaphore +5 (10-concurrent test fails); decompressed cap removed (3 fail); header check removed; multi-member stop removed and no-deadline (both by hang, see N7).
Survived: see N4. Conclusion: guard tests do fail when the guard is removed for every security-relevant control (dial route, per-hop validation, caps, encodings, concurrency).

## AC map (A-3b) : status
- 10 MiB with Content-Length aborts before read: PASS (`content_length_over_the_cap...`; mutation killed).
- 10 MiB chunked stops at limit, too_large, connection closed: PASS.
- Never-finishing server times out, connection closed (and slow drip): PASS.
- 10 concurrent, no cross-contamination; 3 run, 7 queue, deadline from permit, queue bounded: PASS.
- No cookies/credentials/auth/referer across hops: PASS.
- Manual redirect loop: every hop through check_url + resolver filter + Pinned dial; bound 5 and `too_many_redirects`: PASS.
- Dial-once test: PASS. Client has no Default (compile-time ambiguity probe) and Pinned is the only `dns_resolver`: PASS.
- Differential test vs `url::Url::parse` (about 30k corpus, 14 IPv4 x all spellings, 150k fuzz, non-vacuity counters, positive control `checker_flags_a_broken_parser`): PASS, and it can fail. Runtime cross-check exists (N4 caveat). Sprint 1 guard checks deleted, self-test still passes: PASS.
- Four refusals through the real client (loopback, metadata, private-resolving/mixed name, redirect to private): PASS; run on the release profile in `a3b-merge-gate`.
- gzip: identity or single gzip only, stacked/unknown/multi-member/trailing garbage per ADR-004, 64 KiB output steps, bomb: PASS.
- Header size/count limits, header bomb fails cleanly: PASS (N6).
- Manual native-aarch64 RSS smoke: FAIL/pending (B1).
- UTF-8 streaming decoder with replacement: PASS (unit tests, split sequences).

## a3b-merge-gate meaning
It is a real, machine-checked gate but partly a tripwire. Strong parts: no-Default compile trick; behavioural proof that the per-hop client dials only the validated host and opens zero connections for other names (`accepted()` counters) or blocked answers; exactly one `.dns_resolver(` call taking `Pinned`. Weak part: the source-text scan (banned tokens like `TcpStream`, `connector`) can be evaded by a determined change; the dev report says so. The CI job also asserts each named test actually ran and printed `ok`, so a renamed or filtered-out test fails the job. Acceptable for the stated risk; B-1/B-2 own deeper testing. It only blocks merge once added to branch protection (N9).

## E-8 AC map
- Feature permits only 127.0.0.0/8 and ::1; other ranges still refuse; unit test each: PASS (`for_build_permits_only_loopback_with_bench_feature`).
- Absent in default/release: loopback refused; guard fails on feature or marker; four-refusal test on release profile: PASS (guard self-test builds each forbidden feature and shows failure).
- Marker to stderr at startup: PASS. Handshake reporting: pending E-2 (N2).
- Only-source-difference is the cfg'd constructor: PASS (`Policy::for_build`; `test-support` alone does not affect it). Size/idle delta record, public-host cross-check: pending E-4 (N2).
- `--version` commit and Cargo.lock hash: NOT implemented; deferral analysis in N1 (acceptable if re-homed).
- Release workflow never sets feature/uploads bench binary: no release workflow exists yet (D-2); guard covers it.

## Error convention
`FetchError::tool_text()` yields `error[<code>]: <message>`; codes stable and unit-tested (`codes_and_text_are_stable`); `tests/stdio.rs` asserts `error[{code}]` prefix for every refused URL. Transport errors carry categories only, no address or port (test). PASS.

## Behaviour notes (gzip / truncation / limits)
gzip accepted only as identity/absent/one gzip (case-insensitive); br/deflate/zstd/unknown/stacked give `unsupported_encoding`; corrupt or truncated gzip gives `bad_response`; decompressed cap gives `too_large` in steps of at most 32 KiB; only the first member is decoded and trailing bytes are ignored. Short-Content-Length identity truncation surfaces as `network_error` (untested at client level, N5).
