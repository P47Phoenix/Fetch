# ADR-003: SSRF defence by resolve-once, validate-all, pin, and per-hop revalidation

Status: Accepted
Date: 2026-09-19

## Context
A prompt-injected agent can supply URLs pointing at loopback, RFC 1918, link-local, cloud-metadata or other non-public addresses, directly, via DNS, via redirects, or via IP encodings. Validating a hostname and then letting the HTTP stack resolve again permits DNS rebinding (TOCTOU). FR-05, FR-06, B-1..B-3, B-5. Security-critical: must be correct by construction, table-testable, and not reliant on unstable `std` `is_global`.

## Options
1. Validate URL string/host only. Rejected (rebinding, encodings, DNS names).
2. Resolve ourselves, validate, then rewrite the URL to the IP. Breaks TLS SNI/cert name matching and Host header unless carefully overridden.
3. Custom resolver plugged into the HTTP client: resolve once, validate every address, return only the validated set; the connector dials from that set (chosen). Plus explicit pre-check of IP-literal hosts (connectors skip the resolver for literals).
4. Network-level sandbox (firewall/netns): strong but outside the binary, not portable to the single-binary goal; recommended as documentation-level defence in depth only.

## Decision
Option 3.

Pipeline per hop (initial request and each redirect):
1. Parse with the `url` crate (WHATWG host parsing normalises decimal `2130706433`, hex `0x7f000001`, octal, short `127.1`, into `Host::Ipv4`). Operate on the parsed `Host` enum, never on the raw string.
2. Scheme allowlist: `http`, `https` only (also for redirect targets).
3. Reject URLs with userinfo (`user:pass@`), IPv6 zone ids, and empty hosts.
4. If `Host` is an IP: run `check_ip` immediately (this covers literals; the client will not call the resolver for them).
5. If `Host` is a domain: the client calls our `Resolve` impl, which does one `getaddrinfo`-equivalent lookup (tokio `lookup_host`), then `check_ip` on EVERY returned address. If any address is blocked, the whole answer is refused (covers mixed public/private answers). Otherwise return exactly the validated addresses; the connector dials only those. No second lookup exists in the path, so rebinding between check and connect cannot change the target.
6. Redirects: client redirect policy is `none`; our loop follows up to 5 hops, resolving `Location` against the current URL, and reruns steps 1-5 per hop. A 6th hop -> `too_many_redirects`. Non-http(s) redirect Location -> `blocked_target` (the initial URL with a non-http(s) scheme or userinfo is instead invalid params, ADR-006; single rule).
7. Proxies disabled; connection pooling disabled (ADR-001), so no connection validated in one context is reused.
8. The resolver's blocked decision returns an error mapped to `blocked_target` before any TCP connect.

`check_ip` (own static tables in `ssrf/ranges.rs`, table-driven tests):
- IPv4 blocked: 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10 (CGNAT), 127.0.0.0/8, 169.254.0.0/16 (incl. 169.254.169.254), 172.16.0.0/12, 192.0.0.0/24, 192.0.2.0/24, 192.88.99.0/24, 192.168.0.0/16, 198.18.0.0/15, 198.51.100.0/24, 203.0.113.0/24, 224.0.0.0/4, 240.0.0.0/4 (includes 255.255.255.255), plus the single address 168.63.129.16 (Azure wire server; public-range so listed explicitly; always blocked, never allowlistable, like 169.254.169.254 and fd00:ec2::254).
- IPv6 blocked: ::/128, ::1/128, fc00::/7, fe80::/10, fec0::/10, ff00::/8, 100::/64, 2001:db8::/32, 2001::/23 (IETF protocol assignments incl. Teredo), 64:ff9b:1::/48.
- IPv6 with embedded IPv4: IPv4-mapped `::ffff:0:0/96` and IPv4-compatible `::/96`: extract the IPv4 and apply the IPv4 table. NAT64 `64:ff9b::/96` and 6to4 `2002::/16`: extract the embedded IPv4 and apply the IPv4 table (a public-embedded 6to4 address is allowed; a private-embedded one is blocked).
- The list is data; adding a range needs no code change (B-5).

Scheme/port policy:
- Ports: any port 1-65535 is permitted on public addresses by default (the blocked-range check is the primary control; a fixed 80/443 list would break legitimate sites and gives modest protection because the LLM can already probe public hosts). Optional `FETCH_ALLOWED_PORTS` restricts. Port 0 rejected. Reviewed by Security: acceptable residual risk is external port scanning via the agent from the user's IP.
- `https` upgrade is not automatic; `http` stays permitted (FR-02).

Allowlist (OQ-4, undecided by architecture): `Policy` carries an allowlist of hostnames, empty by default. If the human enables it: applies only to the hostname of the original request or a redirect hop that matches exactly; relaxes the private-range check only; never relaxes scheme, port, userinfo, or the cloud-metadata address 169.254.169.254 and fd00:ec2::254; IP-literal hosts are not allowlistable. Blanket block = empty list. See architecture.md section 12.

Bench-only loopback exception (E-8, CONFIRMED by the user 2026-09-19; not an OQ-4 decision): the fail-closed policy also blocks the loopback fixture server the memory benchmark needs. The chosen route is a compile-time Cargo feature `bench-loopback`: off by default, it permits only 127.0.0.0/8 and ::1; every other blocked range (private, link-local, metadata, CGNAT, ULA, and so on) still refuses, and scheme, port and userinfo rules are unchanged. It produces a second binary from the same commit and Cargo.lock by the same pinned pipeline, and that binary embeds a fixed marker string. Why not a runtime switch: an env var or flag would ship a bypass in the release binary that a prompt-injected agent or hostile config could flip, and would overlap the OQ-4 allowlist question; a compile-time feature leaves the shipped artifact without the code path. Release guard (D-7, and D-2 on every artifact): `cargo tree -e features` plus a marker-string grep on the actual artifact assert that `test-support` and `bench-loopback` (and the fixture-CA feature) are absent; the guard has a self-test with a positive control. The default `Policy` stays fail-closed. Measurement fidelity: idle RSS is gated on the shipped binary and peak RSS on the bench build; the delta is bounded (idle delta <= 0.5 MiB, one public-host 5 MiB fetch on the shipped binary within 10% of the bench peak and <= 40 MiB, same commit and Cargo.lock hash); see architecture.md 11 item 7.

Testing: fake `Resolve` injected in tests: private, mixed, IPv4-mapped, first-public-then-private (rebinding) answers; encodings table; redirect chains; test-only loopback policy behind `cfg(test)`/`test-support`, and the bench-only `bench-loopback` feature; both verified absent from release builds (D-7/D-2 guard).

## Consequences
+ Check and connect use the same addresses (no TOCTOU); works with TLS naming unchanged.
+ Pure functions and tables; coverage gate >= 90% feasible.
+ Blocked errors are categorical and fire before any connection (no LAN oracle).
- Depends on reqwest's `dns_resolver` hook and on documented behaviour that IP literals bypass it (mitigated by the explicit pre-check plus tests that assert both paths).
- Pooling off costs handshakes.
- Happy Eyeballs: multiple validated addresses are all safe to try; the connector may try them in order.
- Blocked list may over-block (e.g. CGNAT 100.64/10 used by Tailscale/overlay networks for legitimate remote hosts): that is intentional default-deny; OQ-4 is the escape valve.
- IPv6-only ARM hosts with NAT64 gateways: embedded-IPv4 handling must be validated in tests to avoid false blocks.
- The system resolver (`getaddrinfo`) can return CNAME chains transparently; only final addresses are checked, which is what matters.
- Local `/etc/hosts` entries resolving names to private addresses are blocked unless allowlisted (desired).

## What ARM data would flip it
Nothing in this ADR is performance- or ARM-dependent; ARM only matters for libc resolver behaviour: on the musl static build, musl's stub resolver (no `nsswitch`, different search/timeout behaviour, no mDNS `.local`) may fail names that glibc resolves. If musl is shipped (ADR-005) and `.local`/split-DNS home-lab names fail on the ARM runner, either ship gnu, or add a documented limitation. The design decision (own resolver returning validated addresses) is unchanged either way. Revisit pooling only under ADR-001 criteria.

## Amendment 2026-09-19 (A-3a fix-pass 1, dated note; the Accepted decision text above is left as written)
1. **The `url` crate is no longer mandated for `check_url`** (step 1 above). A-3a implements a strict hand-rolled host parser in `src/ssrf/mod.rs` that follows WHATWG special-scheme authority delimiters (`/ ? # \`) and WHATWG numeric-IPv4 folding (decimal, hex, octal, 1 to 4 parts), and FAILS CLOSED on anything ambiguous: a host whose last label is numeric is either a valid IPv4 or refused, never treated as a name; userinfo (`@` anywhere in the authority), `%`, control, space, non-ASCII, zone ids and extra slashes are refused. Why: it drops a direct dependency (4 of 15 today) and the parser's whole surface is the small set of accepted shapes, so a refusal is the default. The `url` crate stays reachable transitively once A-3b adds reqwest, and A-3b must prove there is no parser differential (see the A-3b merge-gate note in architecture 14.1 and docs/EPICS.md).
2. **`src/ssrf/ranges.rs` is now the authoritative table.** The IPv6 list above is extended (all fail closed, none relaxable): `::/96` is blocked WHOLE (IPv4-compatible, deprecated by RFC 4291 section 2.5.5.1; supersedes the earlier "extract the IPv4" rule for `::/96`, so `::8.8.8.8` is refused), `3fff::/20` (RFC 9637 documentation) `5f00::/16` (RFC 9602 SRv6 SIDs) and `100:0:0:1::/64` (RFC 9780 dummy IPv6 prefix, added in the cleanup commit) are blocked, and IPv4-translated SIIT `::ffff:0:0:0/96` (RFC 2765) joins mapped, NAT64 and 6to4 as an embedded-IPv4 form judged by the IPv4 table. Rows `2001::/23` and `64:ff9b:1::/48` are ADR-003 rows, not extras. Explicit `169.254.169.254/32` and `fd00:ec2::254/128` rows sit ahead of their containing ranges for the category label only. Where this ADR text and `ranges.rs` differ, `ranges.rs` (with its tests) wins.

