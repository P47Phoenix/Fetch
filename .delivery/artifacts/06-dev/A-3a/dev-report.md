# A-3a Dev Report: SSRF core (range table, resolver filter, fail-closed policy)

Branch `sprint-1/skeleton-ssrf`, PR #5. Code commit 307e368, workflow fix 2nd commit, this report 3rd. Nothing here performs network I/O; `fetch` still returns `not_implemented` for URLs that pass the checks.

## What was built
- `src/ssrf/ranges.rs`: static v4 and v6 tables (data, no code per range), `classify`, `classify_v4`, `classify_v6`. `Kind::Loopback` is the only relaxable class; metadata addresses are `Kind::Other` (never relaxable).
- `src/ssrf/mod.rs`: `check_url(url, &Policy, Origin)` returning `CheckedUrl { https, host, port }`; hand-rolled strict host parser (WHATWG numeric-IPv4 folding, bracketed IPv6 via `std`, zone ids, userinfo, whitespace/control/non-ASCII/`%`, port 0 and range refused); `localhost` and `*.localhost` names blocked unless the loopback test policy is used.
- `src/ssrf/resolver.rs`: `Resolver` trait (async, RPITIT, no crate), `resolve_validated` (one lookup, every answer checked, whole answer refused if any is blocked, returns only the deduplicated validated set), `validate_target`, `revalidate_hop` (same checks, `Origin::Redirect`, failures are `blocked_target`, non-http(s) refused). `FakeResolver` test double counts lookups.
- `src/policy.rs`: `Policy::check_ip`; default fail-closed; `permit_loopback_for_tests()` unchanged (cfg(test) / `test-support` / `bench-loopback`), relaxes 127/8 and ::1 (and their embedded forms) only.
- `src/error.rs`: `BlockedTarget` and `DnsFailure` variants (`blocked_target`, `dns_failure`). Messages carry a category word only, never an address (tests assert no leak).
- `src/server.rs`: `validate_url` removed; the tool calls `ssrf::check_url`. Rejections are now uniform (see deviation 1).

## Ranges table and source
Every row is ADR-003 `check_ip` (IANA special-purpose registries plus cloud metadata). Compared with the list in the task: all task entries are present and identical. ADR-003 has two IPv6 rows the task list lacked, which I added (following the architecture as instructed): `2001::/23` (IETF protocol assignments incl. Teredo) and `64:ff9b:1::/48` (local-use NAT64). Also included: `fd00:ec2::254/128` and `169.254.169.254/32` as explicit rows ahead of their containing ranges, so their category reads "cloud metadata"; `168.63.129.16/32`; `100.100.100.200` is covered by CGNAT (tested).

| Family | Blocked |
|---|---|
| IPv4 | 0.0.0.0/8, 10/8, 100.64/10, 127/8 (loopback), 169.254/16 (+ 169.254.169.254/32), 172.16/12, 192.0.0/24, 192.0.2/24, 192.88.99/24, 192.168/16, 198.18/15, 198.51.100/24, 203.0.113/24, 224/4, 240/4 (incl. 255.255.255.255), 168.63.129.16/32 |
| IPv6 whole | ::/128, ::1/128 (loopback), fd00:ec2::254/128, fc00::/7, fe80::/10, fec0::/10, ff00::/8, 100::/64, 2001:db8::/32, 2001::/23, 64:ff9b:1::/48 |
| IPv6 embedded IPv4, judged by the v4 table | ::ffff:0:0/96 mapped, ::/96 compatible, 64:ff9b::/96 NAT64 (low 32 bits), 2002::/16 6to4 (bits 16..48). A public embedded address is allowed (ADR-003 and architecture 5 item 10). |

Flagged differences and judgement calls:
1. ADR-003 adds `2001::/23` and `64:ff9b:1::/48`; followed.
2. IPv4-compatible `::a.b.c.d` with a public embedded IPv4 (for example `::8.8.8.8`) is ALLOWED per ADR-003 ("extract the IPv4 and apply the IPv4 table"). It is not a routable address in practice; blocking all of `::/96` would be stricter. Suggest B-2 decides; a one-line table change.
3. The host is refused, not resolved, when its last label is numeric but does not parse as IPv4 (`1.2.3.4.5`, `256.1.1.1`): WHATWG says such a host can never be a name.

## Tests (default config: 40 unit + 7 integration; 41 with bench-loopback)
- ranges: first/middle/last of every v4 row; neighbours pass; v6 blocked table; v6 neighbours pass; embedded-IPv4 case table; metadata never loopback-kind; loopback kind only for loopback; table well-formedness (no host bits). Property-style: every blocked v4 range's first/mid/last is blocked in mapped, compatible, NAT64 and 6to4 form and the neighbours outside a range are blocked in embedded form only if blocked in v4; every v6 range first/mid/last.
- check_url: scheme allowlist, userinfo (including `public@127.0.0.1`), zone ids, empty host, bad ports, whitespace/percent/non-ASCII, IP literals in dotted, decimal, octal, hex, mixed, short, trailing-dot and bracketed forms, numeric hosts never become names, localhost names, redirect origin gives `blocked_target`, initial gives `invalid_argument` naming `url`. Property-style: every blocked v4 range endpoint in dotted, decimal, hex, octal and bracketed mapped/compatible spelling is refused.
- resolver: public set returned exactly; mixed answer refused in both orders for seven private answer kinds (no leak in message); dns failure; literals never reach the resolver; rebinding shape (first lookup public, second private): validated set is the first and exactly one lookup happens; hop revalidation refuses file/ftp/gopher/javascript/data, userinfo, literals in odd spellings, private-resolving name, `localhost`, relative and scheme-relative locations.
- integration (`tests/stdio.rs`): blocked literals, userinfo, zone id, file: over the real stdio binary, all `isError` with the expected `error[code]`.
- Verified locally: `cargo fmt --check`; clippy `-D warnings` and `cargo test --locked` in default, `bench-loopback`, `test-support`; `scripts/check-release-features.sh` and `--self-test`; `python3 bench/selftest.py`; `actionlint`; `cargo build --release --locked` (1,289,256 bytes, x86_64).

## Dependencies (NFR-05 <= 15)
| # | Crate | Change |
|---|---|---|
| 1-4 | rmcp, tokio, serde, schemars | unchanged from A-2 |
| - | none added | std::net parsing, own tables, own WHATWG numeric-IPv4 folding. The architecture planned the `url` crate (dependency 5); not needed for the checks, so 4 of 15. |

Tests use `tokio::test` (existing tokio features rt, macros). No HTTP client crate anywhere in the tree (rmcp does not pull one).

## Guard changes
- `scripts/check-release-features.sh`: the existing guard already covered `test-support` and `bench-loopback` (feature allowlist via `cargo tree -e features`, plus `FETCH_MCP_MARKER_` grep on the actual release binary; self-test builds each forbidden feature and requires both detections). No new CI job was needed; job ids fmt, clippy, test, deny, release-guard unchanged.
- New: no-HTTP-client assertion over `cargo tree -e all --all-features` (banned: reqwest, hyper, hyper-util, hyper-tls, hyper-rustls, ureq, isahc, surf, attohttpc, minreq, curl, curl-sys, awc, h2, http-body(-util), tower-http, tungstenite and others). Run by the default guard and as `--no-http-client`. Self-test: fake trees containing reqwest, hyper, ureq, h2 and hyper-util must fail; a tree with `httparse`/`hyperlink` passes (no false positives); the real tree passes and cargo tree output size is checked so it cannot be vacuous. A-3b must remove this one check deliberately (documented in the script header).
- `docs/ci-branch-protection.md` updated (release-guard scope, bench-product advisory).
- Limit: the release binary is stripped, so there is no symbol to grep; the marker string (compiled only with the features) is the artifact-level proof, as designed in D-7.

## ARM: real fetch-mcp release binary on hosted arm64 (advisory, recorded, not gated)
Job `bench-product` in `arm-bench.yml` (run 35482584789), native aarch64, Ubuntu 24.04.5, kernel 6.17.0-1022-azure, CPU part 0xd49, 4 vCPU, 16 GB, 4 KiB pages, release build `--locked`, gnu, binary sha256 6781639d..., `harness --binary-kind shipped --scenario idle`, no `--gate`, 10/10 valid:
- Idle VmRSS: median 2.59 MiB (2652 kB; min 2640, max 2656 kB) against the 10 MiB target. Advisory pass (skeleton only, no HTTP/TLS/converter yet).
- ready_ms (spawn to initialize result): median 1.058 ms (0.977 to 1.067). tools_list_ms median 0.243 ms.
- The 250 ms readiness figure: recorded 1.06 ms on aarch64, well within 250 ms for a process spawn; this is evidence, the harness does not enforce 250 ms, and container start (ADR-007) is not included. Raw data: `arm/` beside this report.
- Only gnu was measured for the product (musl candidate is D-2 territory). Job is not a required check.

## Deviations
1. (a) Uniform validation errors: rmcp 3.4 wraps `Parameters` extraction and reports failures as `isError`; forcing JSON-RPC errors for these needs a custom `call_tool` that bypasses rmcp extraction, which is a hack. Instead our own scheme/userinfo/blocked-literal checks now also return `isError` `error[invalid_argument|blocked_target]: url: ...`. ADR-006 amended with a dated note; architecture 6.1 row updated; `tests/stdio.rs` now fails on any protocol-level error. Behaviour change vs A-2: a valid-scheme URL that is a blocked literal now yields `blocked_target` rather than `not_implemented`.
2. `Policy` stays in `src/policy.rs` (A-2 layout) rather than `ssrf/mod.rs`; ssrf depends on it. E-8 wording "ssrf::policy" is satisfied by `crate::policy`. Trivial to move later.
3. `check_url` takes a `Policy` and an `Origin`; architecture sketch was `check_url(url)`. Needed for loopback relaxation and the initial-versus-redirect error rule.
4. Authority parsing rejects (not normalises) percent-encoded hosts, non-ASCII hosts (IDNs must be given as punycode) and any control/space in the authority. Stricter than WHATWG on purpose (fail closed, no parser differential).

## Open issues
- Relative or scheme-relative redirect `Location` resolution is not done here (needs a URL parser); `revalidate_hop` takes an absolute URL and refuses anything else. A-3b/B-3 must resolve against the current URL and pass the absolute string. Redirect count limit is B-3.
- Port policy: any port 1-65535 permitted (ADR-003), only port 0 rejected.
- No production `Resolver` implementation (needs tokio `net`, added with A-3b). The A-3b merge gate (`a3b_merge_gate`, client constructor taking a `Policy`) is A-3b work.
- `::a.b.c.d` public-embedded compatible addresses allowed per ADR-003 (see above).
- Not verified: `cargo deny` locally (not installed; runs in CI); A-2 AC "Claude Code lists fetch" remains a manual step.
