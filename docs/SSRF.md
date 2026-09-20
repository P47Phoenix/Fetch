# SSRF range table

**What is this?** SSRF (server-side request forgery) is when a program is tricked into fetching an address inside a private network. Fetch defends against it by refusing to connect to any address on the lists below. **Who needs it?** Contributors changing the blocking code, and anyone asking "why was my URL refused?". **What to do first:** read "How the rule works", then find your address in the tables.

The authoritative source is the code: [`src/ssrf/ranges.rs`](../src/ssrf/ranges.rs) (tables `V4_BLOCKED` and `V6_BLOCKED`, and the embedded-IPv4 handling in `classify_v6`). If this page and the code disagree, the code wins; please fix this page. The design and its reasons are in [ADR-003](../.delivery/artifacts/04-architect/architect/adrs/ADR-003-ssrf-dns-resolve-and-pin.md) (with its dated amendment). The list follows the IANA special-purpose address registries plus the cloud metadata addresses. Status today: A-3a (the checking code) is done; the download that will use it arrives in A-3b, so nothing is fetched yet.

## How the rule works

- Fail closed: an address on these lists is refused, and anything the code cannot understand is refused too.
- Every rule below applies to every request, on every redirect step, and to every address a host name resolves to. One blocked address is enough to refuse the whole host.
- Only loopback (`127.0.0.0/8` and `::1`) can be relaxed, and only by a test or benchmark build policy (`test-support` or `bench-loopback`, which never ship in a release). The shipped program blocks loopback like everything else. Cloud metadata addresses are blocked under every policy.
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

## IPv6: addresses that contain an IPv4 address

These forms are judged by the IPv4 table above, using the IPv4 address inside them. So `::ffff:10.0.0.1` is blocked because `10.0.0.1` is.

| Prefix | Form | Source |
|---|---|---|
| ::ffff:0:0/96 | IPv4-mapped | RFC 4291 |
| ::ffff:0:0:0/96 | IPv4-translated (SIIT); IPv4 address is the low 32 bits | RFC 2765 |
| 64:ff9b::/96 | NAT64; IPv4 address is the low 32 bits | RFC 6052 |
| 2002::/16 | 6to4; IPv4 address is in bits 16 to 48 | RFC 3056 |

## Deliberately not blocked

Public addresses pass, including neighbours of blocked ranges (for example `100.63.255.255`, `172.32.0.0` and `168.63.129.17`). Unallocated IPv6 space outside `2000::/3` is not blocked as a whole; that wider choice is flagged for story B-2. A private-host allowlist (OQ-4) is not decided and does not exist.

## Changing the table

The tables are data in `ranges.rs`; adding a range needs no other code change. Add a test with the first, a middle and the last address of the new range and its neighbours, cite the source in the same row of this page, and run `cargo test --locked`.
