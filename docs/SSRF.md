# SSRF range table

**What is this?** SSRF (server-side request forgery) is when a program is tricked into fetching an address inside a private network. Fetch defends against it by refusing to connect to any address on the lists below. **Who needs it?** Contributors changing the blocking code, and anyone asking "why was my URL refused?". **What to do first:** read "How the rule works", then find your address in the tables.

The authoritative source is the code: [`src/ssrf/ranges.rs`](../src/ssrf/ranges.rs) (tables `V4_BLOCKED` and `V6_BLOCKED`, and the embedded-IPv4 handling in `classify_v6`). If this page and the code disagree, the code wins; please fix this page. The design and its reasons are in [ADR-003](../.delivery/artifacts/04-architect/architect/adrs/ADR-003-ssrf-dns-resolve-and-pin.md) (with its dated amendment). The list is checked against the IANA special-purpose address registries (every IPv4 and IPv6 row that is not globally reachable is blocked, including the RFC 9780 dummy prefix) plus the cloud metadata addresses. Two globally reachable registry blocks are handled deliberately: `2620:4f:8000::/48` (AS112 delegation) passes, and the globally reachable parts of `2001::/23` are blocked whole (fail closed). Status today: A-3a (the checking code) is done; the download that will use it arrives in A-3b, so nothing is fetched yet.

## How the rule works

- Fail closed: an address on these lists is refused, and anything the code cannot understand is refused too.
- Every rule below applies to every request, on every redirect step, and to every address a host name resolves to. One blocked address is enough to refuse the whole host.
- Loopback can be relaxed (`127.0.0.0/8`, `::1`, and loopback in any embedded form such as `::ffff:127.0.0.1`, SIIT, NAT64 or 6to4 of a `127.x` address), but only by a test or benchmark build policy (`test-support` or `bench-loopback`, which never ship in a release); the shipped program blocks loopback like everything else. The IPv4 "private" (RFC 1918) category can also be relaxed, but only for a hostname explicitly listed in `FETCH_ALLOW_PRIVATE_HOSTS` (`ssrf::Policy::check_ip_for_host`, C-2, OQ-4 still open) -- see [README: Configuration](../README.md#configuration-environment-variables). Every other category, cloud metadata addresses included, is blocked under every policy.
- A refusal is reported as `error[blocked_target]: ...` with a category word such as "private" or "loopback". The address itself is never shown.

## IPv4

| Range | Category | Source |
|---|---|---|
| 0.0.0.0/8 | unspecified | IANA IPv4 special-purpose registry, RFC 1122 |
| 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 | private | RFC 1918 |
| 100.64.0.0/10 | shared address space (CGNAT) | RFC 6598 |
| 127.0.0.0/8 | loopback (the only relaxable row) | RFC 1122 |
| 169.254.0.0/16 | link-local | RFC 3927 |
| 169.254.169.254/32 | cloud metadata | AWS, GCP and Azure instance metadata (ADR-003) |
| 168.63.129.16/32 | cloud metadata (Azure wire server; a public range, listed on its own) | Azure (ADR-003) |
| 192.0.0.0/24 | reserved | RFC 6890 |
| 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 | documentation | RFC 5737 |
| 192.88.99.0/24 | reserved | RFC 7526 |
| 198.18.0.0/15 | benchmarking | RFC 2544 |
| 224.0.0.0/4 | multicast | RFC 5771 |
| 240.0.0.0/4 | reserved (includes 255.255.255.255) | RFC 1112 |

## IPv6: whole ranges

| Range | Category | Source |
|---|---|---|
| ::/128 | unspecified | RFC 4291 |
| ::1/128 | loopback (relaxable) | RFC 4291 |
| fd00:ec2::254/128 | cloud metadata (AWS, IPv6) | AWS (ADR-003) |
| fc00::/7 | unique local | RFC 4193 |
| fe80::/10 | link-local | RFC 4291 |
| fec0::/10 | site-local (deprecated) | RFC 3879 |
| ff00::/8 | multicast | RFC 4291 |
| 100::/64 | discard-only | RFC 6666 |
| 2001:db8::/32 | documentation | RFC 3849 |
| 2001::/23 | protocol assignments, including Teredo and other sub-blocks. Blocked whole, even the parts that are globally reachable | RFC 2928 and the IANA registry |
| 64:ff9b:1::/48 | local-use NAT64 | RFC 8215 |
| ::/96 | deprecated IPv4-compatible, blocked whole (fail closed) | RFC 4291 section 2.5.5.1 |
| 3fff::/20 | documentation | RFC 9637 |
| 5f00::/16 | SRv6 segment identifiers | RFC 9602 and the IANA registry |
| 100:0:0:1::/64 | dummy IPv6 prefix (not forwardable) | RFC 9780 |

## IPv6: addresses that contain an IPv4 address

These forms are judged by the IPv4 table above, using the IPv4 address inside them. So `::ffff:10.0.0.1` is blocked because `10.0.0.1` is.

| Prefix | Form | Source |
|---|---|---|
| ::ffff:0:0/96 | IPv4-mapped | RFC 4291 |
| ::ffff:0:0:0/96 | IPv4-translated (SIIT); IPv4 address is the low 32 bits | RFC 2765 |
| 64:ff9b::/96 | NAT64; IPv4 address is the low 32 bits | RFC 6052 |
| 2002::/16 | 6to4; IPv4 address is in bits 16 to 48 | RFC 3056 |

## URL-level rules (applied before any address check)

These are enforced by `check_url` in `src/ssrf/mod.rs`, under the shipped policy:

- Only `http` and `https` URLs are accepted.
- The names `localhost` and `*.localhost` are refused by name.
- Every IPv4 spelling is folded to one address before the table is applied: decimal (`2130706433`), octal (`0177.0.0.1`), hex (`0x7f.1`), and 1 to 4 parts (`127.1`). A host whose last label is numeric is either a valid IPv4 address or refused, never treated as a name.
- Userinfo (anything before an `@`) is refused, and so is a zone id (`%` in the host).
- Non-ASCII hosts are refused whole: an internationalised name typed in Unicode is refused, not converted. Only the ASCII punycode form (`xn--...`) is accepted, and it is passed to the resolver like any name.
- Port `0`, or a port that does not fit 16 bits, is refused. There is no port allowlist yet.
- One trailing dot on a name is stripped; two are refused.

## Deliberately not blocked

Public addresses pass, including neighbours of blocked ranges (for example `100.63.255.255`, `172.32.0.0` and `168.63.129.17`). Unallocated IPv6 space outside `2000::/3` is not blocked as a whole; that wider choice is flagged for story B-2. A private-host allowlist mechanism (`ssrf::Policy::check_ip_for_host`, wired to `FETCH_ALLOW_PRIVATE_HOSTS`, story C-2) exists in the code and is inert by default (empty allowlist, identical to today's fail-closed behaviour for every deployment that does not set it). It relaxes only the IPv4 "private" (RFC 1918) category for the exact original request hostname; whether it should ever be enabled in a real deployment, and under what governance, is OQ-4, which is still **not decided**. See [README: Configuration](../README.md#configuration-environment-variables) for the full semantics.

Accepted residual risks (decision: do not over-block; fail-closed everywhere the code can tell the address is non-public):

- **ISATAP.** An address such as `2606:4700::5efe:a00:1` (public prefix, interface id `::5efe:` plus a private IPv4) passes, because ISATAP has no fixed prefix and is only meaningful if the host has an ISATAP interface, which is not normal in a container. Blocking every `::5efe:` interface id was considered and not done, to avoid over-blocking; revisit if a deployment ever has an ISATAP interface.
- **Non-well-known NAT64 prefixes.** Only `64:ff9b::/96` (RFC 6052) and `64:ff9b:1::/48` are recognised. A network-specific NAT64 prefix that embeds a private IPv4 cannot be detected from the address alone.

## robots.txt sub-fetch (B-4)

When `FETCH_ROBOTS_TXT=enforce`, the `robots.txt` fetch that gates a request goes through the same guarded
`FetchClient` as the request itself: the same SSRF checks (this page's rules apply to it exactly the same way,
including redirect-hop checks), no separate connection-security carve-out, capped at 512 KB, and bounded by its
own sub-deadline (at most a quarter of the configured timeout) rather than the whole outer deadline. It is
origin-only and checked on the initial hop only (a redirect target's own `robots.txt` is not separately
fetched). Any failure -- SSRF refusal, timeout, non-2xx, truncation, malformed content -- fails open (treated
as "no restrictions"); see [README: Configuration](../README.md#configuration-environment-variables) for the
full semantics.

## Changing the table

The tables are data in `ranges.rs`; adding a range needs no other code change. Add a test with the first, a middle and the last address of the new range and its neighbours, cite the source in the same row of this page, and run `cargo test --locked`.
