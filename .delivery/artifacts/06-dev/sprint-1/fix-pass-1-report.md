# Fix-pass 1 report: Sprint 1 DoD reviews (A-2, A-3a), branch sprint-1/skeleton-ssrf (PR #5)

## Final range tables (authoritative: src/ssrf/ranges.rs; every row fail-closed, only loopback rows relaxable by the test/bench policy)

### IPv4 (`V4_BLOCKED`), unchanged
| Range | Category | Source |
|---|---|---|
| 0.0.0.0/8 | unspecified | IANA v4 special-purpose, RFC 1122 |
| 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 | private | RFC 1918 |
| 100.64.0.0/10 | CGNAT | RFC 6598 |
| 127.0.0.0/8 | loopback (relaxable) | RFC 1122 |
| 169.254.0.0/16 | link-local | RFC 3927 |
| 169.254.169.254/32 | cloud metadata | AWS/GCP/Azure IMDS (ADR-003) |
| 168.63.129.16/32 | cloud metadata | Azure wire server (ADR-003) |
| 192.0.0.0/24 | reserved | RFC 6890 |
| 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 | documentation | RFC 5737 |
| 192.88.99.0/24 | reserved | RFC 7526 |
| 198.18.0.0/15 | benchmarking | RFC 2544 |
| 224.0.0.0/4 | multicast | RFC 5771 |
| 240.0.0.0/4 | reserved (incl. 255.255.255.255) | RFC 1112 |

### IPv6 whole-range rows (`V6_BLOCKED`)
| Range | Category | Source | Status |
|---|---|---|---|
| ::/128 | unspecified | RFC 4291 | ADR-003 |
| ::1/128 | loopback (relaxable) | RFC 4291 | ADR-003 |
| fd00:ec2::254/128 | cloud metadata | AWS IMDS v6 | ADR-003 |
| fc00::/7 | unique local | RFC 4193 | ADR-003 |
| fe80::/10 | link-local | RFC 4291 | ADR-003 |
| fec0::/10 | site-local | RFC 3879 (deprecated) | ADR-003 |
| ff00::/8 | multicast | RFC 4291 | ADR-003 |
| 100::/64 | discard-only | RFC 6666 | ADR-003 |
| 2001:db8::/32 | documentation | RFC 3849 | ADR-003 |
| 2001::/23 | protocol assignments incl. Teredo 2001::/32, 2001:1::/32 PCP/TURN, 2001:2::/48 benchmarking, 2001:3::/32 AMT, 2001:4:112::/48 AS112-v6 (blocked though globally reachable: fail closed), ORCHID 2001:10::/28, 2001:20::/28 | RFC 2928 + IANA registry | ADR-003 |
| 64:ff9b:1::/48 | local-use NAT64 | RFC 8215 | ADR-003 |
| ::/96 | deprecated IPv4-compatible, blocked whole | RFC 4291 2.5.5.1 | NEW (architect N-security, fail-closed) |
| 3fff::/20 | documentation | RFC 9637 | NEW |
| 5f00::/16 | SRv6 SIDs | RFC 9602 (IANA registry) | NEW (found in registry re-check; QA F-3 also named it) |
| 100:0:0:1::/64 | dummy IPv6 prefix, RFC 9780 | added later in the A-3a cleanup commit (missed by this pass) | NEW |

### IPv6 embedded-IPv4 forms (judged by the IPv4 table after the rows above)
| Prefix | Form | Source | Status |
|---|---|---|---|
| ::ffff:0:0/96 | IPv4-mapped | RFC 4291 | ADR-003 |
| ::ffff:0:0:0/96 | IPv4-translated (SIIT), embedded v4 = low 32 bits | RFC 2765 | NEW |
| 64:ff9b::/96 | NAT64 | RFC 6052 | ADR-003 |
| 2002::/16 | 6to4, v4 in bits 16..48 | RFC 3056 | ADR-003 |

IANA IPv6 special-purpose registry re-check: everything else listed is either covered above (::1, ::, ::ffff:0:0/96, 64:ff9b::/96, 64:ff9b:1::/48, 100::/64, 2001::/23 and its sub-blocks, 2001:db8::/32, 2002::/16, fc00::/7, fe80::/10) or is globally reachable and intentionally passes (2620:4f:8000::/48 direct delegation AS112, verified passing). Not added, with reason: blocking all space outside 2000::/3 (unallocated global unicast): a wider design decision than this pass, current tests treat neighbours such as fbff::1 as passing; flagged for B-2.

## What changed
- ranges.rs: added rows `::/96`, `3fff::/20`, `5f00::/16`; SIIT added to embedded-v4 forms; `::/96` removed from embedded extraction (it is a whole-block row now, so `::8.8.8.8` is refused and `::127.0.0.1` is `Other` kind, not relaxable). Tests: first/last/outside for each new row, public neighbours, `::1` and `::` keep their kind/category, embedded property test now covers mapped, SIIT, NAT64 and 6to4; mod.rs tests: bracketed spellings (`[::8.8.8.8]`, `[::ffff:0:127.0.0.1]`, `[::ffff:0:a00:1]`, `[0:0:0:0:ffff:0:7f00:1]`, `[3fff::1]`, `[5f00::1]`) refused, every blocked v4 range refused in SIIT spelling too, public addresses (mapped, SIIT, NAT64, 6to4, 2606:4700:4700::1111) still pass.
- Robustness: `obs::stderr_line` writes with `writeln!` and ignores errors; used by obs.rs and both main.rs sites (no more `eprintln!` in non-test src). Test `closed_stderr_pipe_does_not_abort_or_panic` (verified it FAILS on the old code, then passes).
- QA F-2 (non-initialize first message): exit 1 is rmcp's handshake behaviour and is kept (a server cannot proceed without initialize). Our part fixed: the rmcp error text (which embeds a Debug dump of the frame) is no longer written to stderr unless `FETCH_LOG=debug`; default line is `error serve session ended abnormally (details withheld; ...)`. Test pins exit 1, empty stdout, no client marker in stderr. Note: rmcp 3.4.0's dump omitted `params` in the case I checked, so the leak was structural rather than payload; the guard is defensive.
- QA F-1 (deeply nested JSON gets no reply): rmcp/serde_json behaviour (recursion limit 128 makes the frame unparseable, so the id cannot be recovered; rmcp drops the frame). Not in our code; no small fix without replacing the codec. Test `too_deeply_nested_frame_gets_no_reply_but_server_survives` pins: no reply to that id, server stays alive, next call answered, stdout pure; it fails if rmcp starts replying so the note can be revisited. ISSUE NOTE for a later sprint (A-7/B-3 hardening): "rmcp 3.4.0 sends no response to a frame exceeding the JSON nesting limit; clients rely on their own timeout. Evaluate an upstream issue/PR or a pre-parse depth check returning -32700/-32600 with a null id."
- Guard (scripts/check-release-features.sh): added RAW_NET_BAN (mio, socket2, net2, async-io, polling, smol, async-std, async-net) to the tree name ban, and `check_no_tokio_net_text` asserting tokio features `net`/`full` are not enabled anywhere in the graph (`cargo tree -e features -i tokio --all-features`), with non-vacuity (the tokio feature list must contain `rt`). Self-test: positive controls (mio, socket2, tokio net, tokio full must fail), negative control (current features and a `netlike` name pass), real tree passes. Comments state that A-3b MUST remove these checks (list of functions, calls, flag and self-test blocks). Not done: an end-to-end negative control that edits Cargo.toml (would need a changed Cargo.lock, which `--locked` forbids); the text-level controls exercise the same functions.
- Docs (design only): ADR-003 dated amendment (`url` crate no longer mandated, why, new rows, ranges.rs authoritative); architecture 9.1 (dependency list, count 13 of 15, `url` pin removed), 5/6 step 1 and threat item 2 wording, R13, A-3b row gains the differential-test and Validated.addrs merge-gate requirement, dated amendment section at the end (also 14.1 note: `policy` top-level module, `Resolver` trait needs tokio `net` in A-3b); docs/EPICS.md A-3b gains the differential-test AC and the guard-removal instruction; stale "protocol error path" wording fixed in architecture 6.2 and `src/error.rs`.
- A-2 "Claude Code lists fetch": manual checklist appended to .delivery/artifacts/06-dev/A-2/dev-report.md (nothing run or registered).

## Skipped / not done, with reasons
- Blocking all non-2000::/3 space: design decision for B-2 (see above).
- Architect N4 remainder (architecture 14.1 wording beyond the amendment note), N5 (bound resolver answer count), N6, N7: A-3b/E-2 inputs, not this pass.
- Architect ADR-006 ratification note and ADR index entry: architect action.
- README.md, docs/BENCHMARK.md, docs/ci-branch-protection.md: left for the tech-writer pass as instructed.
- QA F-4 (float `5.0`, extra fields, -32601 for non-object arguments) and dev N4 (label length): not requested; unchanged.

## Verification (local, all exit 0)
fmt; clippy -D warnings in default, bench-loopback, test-support; cargo test --locked in all three (42/43/42 unit + 10 integration); guard plain and --self-test; python3 bench/selftest.py; actionlint; cargo build --release --locked.

## Correction (A-3a cleanup commit)
The statement above that "everything else listed" in the IANA IPv6 special-purpose registry is covered was not fully true: `100:0:0:1::/64` (Dummy IPv6 Prefix, RFC 9780, not forwardable) was missing and is now blocked (found by developer DoD round 2, N1). After a further re-check of the registry, no other non-globally-reachable row is missing; the globally reachable rows (`2620:4f:8000::/48`, and parts of `2001::/23`) are handled as stated above. ISATAP and non-well-known NAT64 prefixes are documented as accepted residuals in docs/SSRF.md. History above is left unchanged.
