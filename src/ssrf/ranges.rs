//! Static blocked-range tables (architecture 3/14.1, ADR-003 `check_ip`). Pure: no I/O, no runtime.
//! The tables are data; adding a range needs no code change. Source of every row: ADR-003 (IANA special-purpose
//! registries plus the cloud metadata addresses). A blocked address is classified, never reported by value, so
//! error text cannot reveal topology (architecture 6.3).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Why an address is blocked. Only `Loopback` can be relaxed (by the test/bench-only policy); everything else,
/// metadata addresses included, is blocked for every policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Loopback,
    Other,
}

/// A blocked address: its relaxability and a category word safe to show the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    pub kind: Kind,
    pub category: &'static str,
}

const fn v4(a: u8, b: u8, c: u8, d: u8) -> u32 {
    u32::from_be_bytes([a, b, c, d])
}

use Kind::{Loopback, Other};

/// IPv4 blocked ranges: (network, prefix length, kind, category).
pub const V4_BLOCKED: &[(u32, u8, Kind, &str)] = &[
    (v4(0, 0, 0, 0), 8, Other, "unspecified"),
    (v4(10, 0, 0, 0), 8, Other, "private"),
    (v4(100, 64, 0, 0), 10, Other, "shared address space (CGNAT)"),
    (v4(127, 0, 0, 0), 8, Loopback, "loopback"),
    (v4(169, 254, 169, 254), 32, Other, "cloud metadata"),
    (v4(169, 254, 0, 0), 16, Other, "link-local"),
    (v4(172, 16, 0, 0), 12, Other, "private"),
    (v4(192, 0, 0, 0), 24, Other, "reserved"),
    (v4(192, 0, 2, 0), 24, Other, "documentation"),
    (v4(192, 88, 99, 0), 24, Other, "reserved"),
    (v4(192, 168, 0, 0), 16, Other, "private"),
    (v4(198, 18, 0, 0), 15, Other, "benchmarking"),
    (v4(198, 51, 100, 0), 24, Other, "documentation"),
    (v4(203, 0, 113, 0), 24, Other, "documentation"),
    (v4(224, 0, 0, 0), 4, Other, "multicast"),
    (v4(240, 0, 0, 0), 4, Other, "reserved"), // includes 255.255.255.255
    (v4(168, 63, 129, 16), 32, Other, "cloud metadata"), // Azure wire server: public range, listed explicitly
];

const fn v6(s: [u16; 8]) -> u128 {
    let mut out = 0u128;
    let mut i = 0;
    while i < 8 {
        out = (out << 16) | s[i] as u128;
        i += 1;
    }
    out
}

/// IPv6 blocked ranges: (network, prefix length, kind, category). Embedded-IPv4 ranges are handled after this
/// table (see [`classify`]); ranges here are blocked whole.
pub const V6_BLOCKED: &[(u128, u8, Kind, &str)] = &[
    (v6([0, 0, 0, 0, 0, 0, 0, 0]), 128, Other, "unspecified"),
    (v6([0, 0, 0, 0, 0, 0, 0, 1]), 128, Loopback, "loopback"),
    (
        v6([0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254]),
        128,
        Other,
        "cloud metadata",
    ),
    (v6([0xfc00, 0, 0, 0, 0, 0, 0, 0]), 7, Other, "unique local"),
    (v6([0xfe80, 0, 0, 0, 0, 0, 0, 0]), 10, Other, "link-local"),
    (v6([0xfec0, 0, 0, 0, 0, 0, 0, 0]), 10, Other, "site-local"),
    (v6([0xff00, 0, 0, 0, 0, 0, 0, 0]), 8, Other, "multicast"),
    (v6([0x0100, 0, 0, 0, 0, 0, 0, 0]), 64, Other, "discard-only"),
    (
        v6([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0]),
        32,
        Other,
        "documentation",
    ),
    (
        v6([0x2001, 0, 0, 0, 0, 0, 0, 0]),
        23,
        Other,
        "reserved (protocol assignments, Teredo)",
    ),
    (
        v6([0x0064, 0xff9b, 1, 0, 0, 0, 0, 0]),
        48,
        Other,
        "reserved (local-use NAT64)",
    ),
    // Fix-pass 1 additions. ::/96 comes after `::` and `::1` so those keep their own category and kind.
    (
        v6([0, 0, 0, 0, 0, 0, 0, 0]),
        96,
        Other,
        "deprecated IPv4-compatible",
    ),
    (
        v6([0x3fff, 0, 0, 0, 0, 0, 0, 0]),
        20,
        Other,
        "documentation",
    ),
    (
        v6([0x5f00, 0, 0, 0, 0, 0, 0, 0]),
        16,
        Other,
        "reserved (SRv6 SIDs)",
    ),
];

const fn mask32(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix as u32)
    }
}

const fn mask128(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix as u32)
    }
}

/// Classify an IPv4 address: `Some` if blocked.
#[must_use]
pub fn classify_v4(ip: Ipv4Addr) -> Option<Blocked> {
    let n = u32::from(ip);
    V4_BLOCKED
        .iter()
        .find(|(net, p, _, _)| n & mask32(*p) == *net)
        .map(|&(_, _, kind, category)| Blocked { kind, category })
}

/// Classify an IPv6 address: `Some` if blocked. Order: the whole-range table first (so `::1` stays loopback
/// and `::` unspecified), then addresses that embed an IPv4 address, which are judged by the IPv4 table:
/// IPv4-mapped `::ffff:0:0/96`, IPv4-translated (SIIT) `::ffff:0:0:0/96`, NAT64 `64:ff9b::/96` and 6to4
/// `2002::/16`. IPv4-compatible `::/96` is blocked whole by the table (fail closed, even for public embedded).
#[must_use]
pub fn classify_v6(ip: Ipv6Addr) -> Option<Blocked> {
    let n = u128::from(ip);
    if let Some(&(_, _, kind, category)) = V6_BLOCKED
        .iter()
        .find(|(net, p, _, _)| n & mask128(*p) == *net)
    {
        return Some(Blocked { kind, category });
    }
    embedded_v4(n).and_then(classify_v4)
}

/// The IPv4 address embedded in a transition-mechanism IPv6 address, if any.
fn embedded_v4(n: u128) -> Option<Ipv4Addr> {
    let low32 = |n: u128| Ipv4Addr::from((n & 0xffff_ffff) as u32);
    let in_prefix = |net: u128, p: u8| n & mask128(p) == net;
    if in_prefix(v6([0, 0, 0, 0, 0, 0xffff, 0, 0]), 96)
        || in_prefix(v6([0, 0, 0, 0, 0xffff, 0, 0, 0]), 96)
        || in_prefix(v6([0x0064, 0xff9b, 0, 0, 0, 0, 0, 0]), 96)
    {
        Some(low32(n))
    } else if in_prefix(v6([0x2002, 0, 0, 0, 0, 0, 0, 0]), 16) {
        Some(Ipv4Addr::from(((n >> 80) & 0xffff_ffff) as u32))
    } else {
        None
    }
}

/// Classify any address.
#[must_use]
pub fn classify(ip: IpAddr) -> Option<Blocked> {
    match ip {
        IpAddr::V4(a) => classify_v4(a),
        IpAddr::V6(a) => classify_v6(a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn blocked(s: &str) -> bool {
        classify(ip(s)).is_some()
    }

    #[test]
    fn ipv4_blocked_table_first_middle_last_of_each_range() {
        // (first, a middle, last) of every range in the table.
        let cases = [
            ("0.0.0.0", "0.1.2.3", "0.255.255.255"),
            ("10.0.0.0", "10.20.30.40", "10.255.255.255"),
            ("100.64.0.0", "100.100.100.200", "100.127.255.255"),
            ("127.0.0.0", "127.0.0.1", "127.255.255.255"),
            ("169.254.0.0", "169.254.169.254", "169.254.255.255"),
            ("172.16.0.0", "172.20.1.1", "172.31.255.255"),
            ("192.0.0.0", "192.0.0.8", "192.0.0.255"),
            ("192.0.2.0", "192.0.2.77", "192.0.2.255"),
            ("192.88.99.0", "192.88.99.1", "192.88.99.255"),
            ("192.168.0.0", "192.168.1.1", "192.168.255.255"),
            ("198.18.0.0", "198.19.0.1", "198.19.255.255"),
            ("198.51.100.0", "198.51.100.9", "198.51.100.255"),
            ("203.0.113.0", "203.0.113.9", "203.0.113.255"),
            ("224.0.0.0", "230.1.2.3", "239.255.255.255"),
            ("240.0.0.0", "250.1.2.3", "255.255.255.255"),
        ];
        for (first, mid, last) in cases {
            for a in [first, mid, last] {
                assert!(blocked(a), "{a} must be blocked");
            }
        }
    }

    #[test]
    fn ipv4_public_and_range_neighbours_pass() {
        for a in [
            "1.0.0.0",
            "1.1.1.1",
            "8.8.8.8",
            "9.255.255.255",
            "11.0.0.0",
            "100.63.255.255",
            "100.128.0.0",
            "126.255.255.255",
            "128.0.0.0",
            "169.253.255.255",
            "169.255.0.0",
            "172.15.255.255",
            "172.32.0.0",
            "192.0.1.0",
            "192.0.3.0",
            "192.88.98.255",
            "192.88.100.0",
            "192.167.255.255",
            "192.169.0.0",
            "198.17.255.255",
            "198.20.0.0",
            "198.51.99.255",
            "198.51.101.0",
            "203.0.112.255",
            "203.0.114.0",
            "223.255.255.255",
            "168.63.129.15",
            "168.63.129.17",
            "93.184.216.34",
        ] {
            assert!(!blocked(a), "{a} must pass");
        }
    }

    #[test]
    fn metadata_addresses_are_blocked_and_never_loopback_kind() {
        for a in [
            "169.254.169.254",
            "168.63.129.16",
            "fd00:ec2::254",
            "100.100.100.200",
        ] {
            let b = classify(ip(a)).unwrap_or_else(|| panic!("{a} must be blocked"));
            assert_eq!(b.kind, Kind::Other, "{a}");
        }
        assert_eq!(
            classify(ip("168.63.129.16")).unwrap().category,
            "cloud metadata"
        );
        assert_eq!(
            classify(ip("fd00:ec2::254")).unwrap().category,
            "cloud metadata"
        );
    }

    #[test]
    fn only_loopback_addresses_have_loopback_kind() {
        for a in [
            "127.0.0.1",
            "127.255.255.255",
            "::1",
            "::ffff:127.0.0.1",
            "64:ff9b::7f00:1",
            "2002:7f00:1::",
        ] {
            assert_eq!(classify(ip(a)).unwrap().kind, Kind::Loopback, "{a}");
        }
        for a in [
            "10.0.0.1",
            "0.0.0.0",
            "::",
            "fe80::1",
            "169.254.169.254",
            "::ffff:10.0.0.1",
        ] {
            assert_eq!(classify(ip(a)).unwrap().kind, Kind::Other, "{a}");
        }
    }

    #[test]
    fn ipv6_blocked_table() {
        for a in [
            "::",
            "::1",
            "fc00::",
            "fd00::1",
            "fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            "fd00:ec2::254",
            "fe80::",
            "fe80::1",
            "febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            "fec0::",
            "feff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            "ff00::",
            "ff02::1",
            "100::",
            "100::ffff:ffff:ffff:ffff",
            "2001:db8::",
            "2001:db8:ffff::1",
            "2001::1",
            "2001:0:4136:e378:8000:63bf:3fff:fdd2", // Teredo
            "2001:1ff::1",
            "2001:4:112::1", // AS112-v6 is globally reachable but sits inside 2001::/23: blocked (fail closed)
            "64:ff9b:1::1",
            "64:ff9b:1:ffff::1",
            // ::/96 IPv4-compatible: first, last, public-embedded, bracket-free spellings
            "::2",
            "::8.8.8.8",
            "::808:808",
            "::ffff:ffff",
            // 3fff::/20 (RFC 9637)
            "3fff::",
            "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
            "3fff:abc::1",
            // 5f00::/16 (RFC 9602)
            "5f00::",
            "5f00:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            // SIIT ::ffff:0:0:0/96 with private embedded
            "::ffff:0:0:0",
            "::ffff:0:ffff:ffff",
        ] {
            assert!(blocked(a), "{a} must be blocked");
        }
    }

    #[test]
    fn ipv6_public_and_neighbours_pass() {
        for a in [
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4001:81b::200e",
            "2001:200::1", // just past 2001::/23
            "2001:db7::1",
            "2001:db9::1",
            "100:0:0:1::1", // just past 100::/64
            "fbff::1",      // just below fc00::/7
            "fe7f::1",      // just below fe80::/10
            "64:ff9a::1",
            "64:ff9b:2::1",
            "2003::1",
            "2001:4860::1",
            "::1:0:0",         // just past ::/96
            "::ffff:1:0:0",    // outside both mapped and SIIT /96
            "3fff:1000::1",    // just past 3fff::/20
            "3ffe::1",         // just below 3fff::/20
            "5eff:ffff::1",    // just below 5f00::/16
            "5f01::1",         // just past 5f00::/16
            "2620:4f:8000::1", // direct delegation AS112, globally reachable
        ] {
            assert!(!blocked(a), "{a} must pass");
        }
    }

    #[test]
    fn embedded_ipv4_is_judged_by_the_ipv4_table() {
        // (address, blocked)
        let cases = [
            // IPv4-mapped
            ("::ffff:127.0.0.1", true),
            ("::ffff:10.1.2.3", true),
            ("::ffff:169.254.169.254", true),
            ("::ffff:168.63.129.16", true),
            ("::ffff:8.8.8.8", false),
            ("::ffff:7f00:1", true),
            // IPv4-compatible
            ("::10.0.0.1", true),
            ("::127.0.0.1", true),
            ("::192.168.1.1", true),
            ("::8.8.8.8", true), // ::/96 is blocked whole, even with a public embedded address
            // IPv4-translated (SIIT) ::ffff:0:0:0/96
            ("::ffff:0:127.0.0.1", true),
            ("::ffff:0:a00:1", true),
            ("::ffff:0:169.254.169.254", true),
            ("::ffff:0:8.8.8.8", false),
            // NAT64 64:ff9b::/96
            ("64:ff9b::10.0.0.1", true),
            ("64:ff9b::169.254.169.254", true),
            ("64:ff9b::7f00:1", true),
            ("64:ff9b::8.8.8.8", false),
            // 6to4 2002::/16 (embedded v4 is bits 16..48)
            ("2002:0a00:0001::", true),
            ("2002:7f00:0001::1", true),
            ("2002:a9fe:a9fe::1", true),
            ("2002:c0a8:0101::1", true),
            ("2002:0808:0808::1", false),
        ];
        for (a, want) in cases {
            assert_eq!(blocked(a), want, "{a}");
        }
    }

    /// Property-style, exhaustive over the table: for every blocked IPv4 range, its first, last and a middle
    /// address are blocked in every embedded form (mapped, SIIT, NAT64, 6to4), and the embedded form of the
    /// address just outside the range is blocked only if that neighbour is itself blocked.
    #[test]
    fn every_blocked_v4_range_is_blocked_in_every_embedded_form() {
        let embed = |n: u32| -> [Ipv6Addr; 4] {
            let hi = (n >> 16) as u16;
            let lo = (n & 0xffff) as u16;
            [
                Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, hi, lo),
                Ipv6Addr::new(0, 0, 0, 0, 0xffff, 0, hi, lo),
                Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, hi, lo),
                Ipv6Addr::new(0x2002, hi, lo, 0, 0, 0, 0, 1),
            ]
        };
        for &(net, prefix, _, cat) in V4_BLOCKED {
            let size = 1u64 << (32 - u32::from(prefix));
            let last = u32::try_from(u64::from(net) + size - 1).unwrap();
            let mid = u32::try_from(u64::from(net) + size / 2).unwrap();
            for n in [net, mid, last] {
                assert!(classify_v4(Ipv4Addr::from(n)).is_some(), "{cat} {n:#x} v4");
                for e in embed(n) {
                    assert!(classify_v6(e).is_some(), "{cat}: {e} must be blocked");
                }
            }
            for n in [net.checked_sub(1), last.checked_add(1)]
                .into_iter()
                .flatten()
            {
                let want = classify_v4(Ipv4Addr::from(n)).is_some();
                for e in embed(n) {
                    // 6to4 form with a public embedded v4 is allowed; mapped/compatible/NAT64 likewise.
                    assert_eq!(
                        classify_v6(e).is_some(),
                        want,
                        "{cat} neighbour {n:#x} as {e}"
                    );
                }
            }
        }
    }

    /// Every blocked IPv6 whole range: first, last and middle address blocked.
    #[test]
    fn every_blocked_v6_range_first_middle_last() {
        for &(net, prefix, _, cat) in V6_BLOCKED {
            let host_bits = 128 - u32::from(prefix);
            let span = if host_bits == 128 {
                u128::MAX
            } else {
                (1u128 << host_bits) - 1
            };
            for n in [net, net + span / 2, net + span] {
                assert!(
                    classify_v6(Ipv6Addr::from(n)).is_some(),
                    "{cat}: {}",
                    Ipv6Addr::from(n)
                );
            }
        }
    }

    #[test]
    fn new_v6_rows_first_last_and_outside() {
        // (first, last, one before first, one after last); None where the neighbour is meaningless.
        let cases = [
            ("::", "::ffff:ffff", None, Some("::1:0:0")),
            (
                "3fff::",
                "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
                Some("3ffe:ffff:ffff:ffff:ffff:ffff:ffff:ffff"),
                Some("3fff:1000::"),
            ),
            (
                "5f00::",
                "5f00:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
                Some("5eff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"),
                Some("5f01::"),
            ),
            (
                "::ffff:0:0:0",
                "::ffff:0:ffff:ffff",
                Some("::fffe:ffff:ffff:ffff"),
                Some("::ffff:1:0:0"),
            ),
        ];
        for (first, last, before, after) in cases {
            assert!(blocked(first) && blocked(last), "{first} {last}");
            for o in before.into_iter().chain(after) {
                assert!(!blocked(o), "{o} must pass");
            }
        }
        assert_eq!(classify(ip("::1")).unwrap().kind, Kind::Loopback);
        assert_eq!(classify(ip("::")).unwrap().category, "unspecified");
        assert_eq!(classify(ip("::8.8.8.8")).unwrap().kind, Kind::Other);
    }

    #[test]
    fn tables_are_well_formed() {
        for &(net, p, _, _) in V4_BLOCKED {
            assert!(
                p <= 32 && net & !mask32(p) == 0,
                "v4 row {net:#x}/{p} has host bits set"
            );
        }
        for &(net, p, _, _) in V6_BLOCKED {
            assert!(
                p <= 128 && net & !mask128(p) == 0,
                "v6 row {net:#x}/{p} has host bits set"
            );
        }
    }
}
