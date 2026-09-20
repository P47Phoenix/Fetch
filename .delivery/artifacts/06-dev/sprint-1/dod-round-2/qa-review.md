# QA independent DoD validation, round 2: Sprint 1 (A-2, A-3a)
Branch sprint-1/skeleton-ssrf, HEAD 1ce36a0 (PR #5). Validator: QA. Method: black-box against a fresh `cargo build --release --locked` binary plus throwaway harnesses in `$CLAUDE_JOB_DIR/tmp` (`drive.py`, `abuse.py`, `qa/` crate using `fetch-mcp` as a path dependency). Nothing committed except this report.

## Verdict: STATUS DONE. No blocking findings. 5 non-blocking findings.

## Evidence
| Check | Result |
|---|---|
| `cargo test --locked` | 42 unit + 10 stdio integration pass; clippy `-D warnings` and `fmt --check` clean |
| tools/list | exactly 1 tool `fetch`; schema has url, max_length, start_index, raw |
| Bad-input matrix (22 cases: missing/null/int url, file:, ftp:, negative, float, string, u64 overflow, wrong-type raw, empty url, userinfo, args as array/string/null) | every bad case is `isError` and names the field (url / max_length / start_index / raw), except the two below (F1) |
| Scheme matrix (file, ftp, gopher, data, javascript, ws, wss, blob, jar, ssh, empty, mixed case HtTpS) | only http/https accepted; others `invalid_argument` |
| Protocol abuse (stdin close, immediate close, no newline, garbage before/mid session, CRLF, notifications before initialize, tools/call before init, unknown method/tool, duplicate init, batch array, odd id types, 1 MB url, 20 MB line, 100k and 1M nesting, NUL/binary, invalid UTF-8) | no abort, no hang (all under 0.1 s); stdout only JSON-RPC 2.0 lines in every case; stderr empty or the single withheld-details line |
| Closed stderr (fd closed with `2>&-`, /dev/null, /dev/full, closed stdout) | no abort/panic, exit 0 or 1 cleanly |
| Fuzz: 300 randomised sessions (mutated JSON, byte flips, type confusion) | 0 hangs, 0 impure stdout lines, exit codes only 0/1 |
| Ready time | 0.4 to 1.5 ms spawn to initialize result on x86_64 |
| `cargo tree` (normal, build) | tokio 1.53.1 (rt, macros, io-std, io-util), tokio-util, libc; no hyper/reqwest/ureq/h2/curl, no mio/socket2, no tokio `net` |
| Guard `--self-test` | passes (takes ~90 s); positive controls real |
| Guard mutation tests (mine) | tokio `net` added to a copy: `--no-http-client` exits 1 and lists mio/socket2; new feature `loopback2`: "feature loopback2 enabled (not in allowlist)"; `--features test-support` build carries `FETCH_MCP_MARKER_TEST_SUPPORT_V1` (grep count 1) so the marker check is non-vacuous |
| Release binary | 0 `FETCH_MCP_MARKER` tokens; 0 hits for test-support / bench-loopback / permit_loopback; `--version` prints `fetch-mcp 0.0.0` only |
| CI names vs docs/ci-branch-protection.md | ci.yml job ids fmt, clippy, test, deny, release-guard match the doc; all PR #5 checks green (fmt, clippy, test, deny, release-guard, bench x2, bench-product) |

## Independent SSRF table (written from IANA v4/v6 special-purpose registries and RFCs, not copied)
Harness: `$CLAUDE_JOB_DIR/tmp/qa/src/main.rs`, run with `cargo run --offline`. FAILS=0.
- IPv4: first and last address of 17 ranges (0/8, 10/8, 100.64/10, 127/8, 169.254/16, 172.16/12, 192.0.0/24, 192.0.2/24, 192.88.99/24, 192.168/16, 198.18/15, 198.51.100/24, 203.0.113/24, 224/4, 240/4 incl. 255.255.255.255, plus 168.63.129.16 and 169.254.169.254) blocked; 31 public neighbours and public special cases (e.g. 100.63.255.255, 100.128.0.0, 172.15.255.255, 172.32.0.0, 168.63.129.15/17, 192.31.196.1, 192.52.193.1, 192.175.48.1) pass.
- IPv6: 48 blocked (::, ::1, ::/96 compat incl. public embedded, mapped, SIIT ::ffff:0:x, NAT64 64:ff9b::/96 with private embedded, 64:ff9b:1::/48, 100::/64, 2001::/23 incl. Teredo and 2001:2::/10:: sub-blocks, 2001:db8::/32, 6to4 2002:(private)::, 3fff::/20, 5f00::/16, fc00::/7, fe80::/10, fec0::/10, ff00::/8, fd00:ec2::254) at first/last; public neighbours (2001:200::, 2001:1fff::, 2002:808:808::, ::ffff:8.8.8.8, 64:ff9b::8.8.8.8, 3ffe::, 3fff:1000::, 5eff::, 5f01::, fbff::, fe7f::) pass.
- URL spellings (~65): decimal, octal, hex, mixed, short (127.1, 127.0.1, 0, 0x, 00), trailing dots, bracketed v6, mapped/compat/SIIT/NAT64/6to4/Teredo in brackets, zone ids (`%25`, `%`), userinfo tricks (`good.com@127.0.0.1`, `127.0.0.1:80@good.com`, `127.0.0.1%2f@good.com`), backslash and `#@` (authority correctly ends at `\` and `#`), fullwidth/Arabic-Indic/circled digits and ideographic dots (refused as non-ASCII), punycode (passes as a name, goes to resolver), 4294967296, 256.256.256.256, 5-part, 08/09.0.0.1 (refused as invalid numeric): all blocked or refused; every URL that passed `check_url` with an IP host is also blocked-free per `check_ip`.
- Resolver (injectable): public+private, private-first, public+mapped-loopback, v6 public+ULA, public+metadata all refused after exactly 1 lookup; all-public returns deduped validated set with 1 lookup; empty answer is `dns_failure`. Redirect revalidation (`revalidate_hop`): 127.0.0.1, ::1, 10.0.0.1, ftp:, file:, userinfo all `blocked_target`; a public name passes; a hop for an IP literal does no lookup.

## Doc vs code (docs/SSRF.md vs src/ssrf/ranges.rs)
Every row of the IPv4 table, the IPv6 whole-range table and the embedded-IPv4 table matches `V4_BLOCKED`, `V6_BLOCKED` and `embedded_v4` (prefixes, categories, mapped vs SIIT). "Deliberately not blocked" examples verified. Differences are omissions and one imprecision, see F3 and F4.

## Findings (all non-blocking)
F1. Deserialisation errors do not use the `error[invalid_argument]:` shape. Repro: tools/call `fetch` with `{}` returns text `failed to deserialize parameters: missing field \`url\``; `{"url":"https://example.com","max_length":-1}` returns `failed to deserialize parameters: max_length: invalid value: integer \`-1\`, expected u64`. The field is named (AC met) but the code prefix differs from the SSRF/scheme errors (`error[invalid_argument]: url: ...`). ADR-006 amendment says this is the rmcp shape; flag for consistency in A-3b/C.
F2. Undeserialisable frames are silently dropped, so a client waiting on the id hangs. Repro: send `{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fetch","arguments":[[[...200 deep...]]]}}` after init: no reply for id 3 (tested by the dev, `too_deeply_nested_frame_gets_no_reply_but_server_survives`). Also `arguments: []` returns error -32601 with message `tools/call` (wrong code; -32602 expected). Invalid UTF-8 or NUL bytes end the session with exit 1. Notification `notifications/initialized` sent before `initialize` ends the session with exit 1 and no stdout. None aborts or hangs the server; a well-behaved client is unaffected.
F3. docs/SSRF.md says only loopback (127.0.0.0/8 and ::1) can be relaxed. In code `::ffff:127.0.0.1` and the other embedded-loopback forms (SIIT, NAT64, 6to4 of 127.x) are also relaxed under the test/bench policy (policy.rs test `loopback_policy_relaxes_only_loopback` asserts `::ffff:127.0.0.1` passes). Test-only, but the doc should say "loopback in any embedded form".
F4. docs/SSRF.md does not describe the URL-level rules that `check_url` enforces: `localhost` and `*.localhost` names blocked by name under the default policy, IPv4 spelling folding, zone ids, userinfo, non-ASCII/IDN hosts refused wholesale (only punycode accepted), port 0 refused. Non-ASCII refusal is a behaviour worth stating because IDN URLs supplied in Unicode are refused rather than converted.
F5. Not blocking but noted: unknown extra arguments (`"extra":1`) are accepted and ignored; `max_length: 0` is accepted at this stage (range checks belong to A-5/C).

## Remains unverified
- A-2 aarch64 ready <= 250 ms: only measured on x86_64 here (about 1 ms). The aarch64 figure rests on the CI `bench-product` job on `ubuntu-24.04-arm`, which I did not re-run or read.
- A-2 "Claude Code lists `fetch` from a throwaway config": not run (needs Claude Code on the author's machine).
- A-3a "release-guard is a *required* check": branch protection is NOT YET CONFIGURED per docs/ci-branch-protection.md (owner action); the job exists, is green, and its names match.
- Real DNS behaviour, TOCTOU/rebinding through a real dialler, the hyper/`url` crate parser differential, and any actual fetch are A-3b/B-1/B-2 scope and untestable here (no client exists).
- `cargo deny` and `cargo audit` not run locally (tools not installed); relied on the green `deny` job in PR #5.
- Behaviour on 32-bit or non-Linux platforms; musl build of the product binary.
