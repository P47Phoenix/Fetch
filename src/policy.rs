//! `Policy` (architecture 14.1, R12). Fail-closed by default: it permits no non-public target. `ssrf::check_url`,
//! the resolver filter and per-hop revalidation all decide through [`Policy::check_ip`]. The only way to
//! permit loopback is the constructor below, compiled only with `test-support` or `bench-loopback` (never in
//! a release build; asserted by scripts/check-release-features.sh) or in unit tests of this crate.

use crate::ssrf::ranges::{self, Blocked, Kind};
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    allow_loopback: bool,
    /// Exact hostnames (canonical, lower-case, no trailing dot -- see `ssrf::canonical_name`) for which the
    /// private-range check is relaxed. Empty by default (C-2): an empty list is inert and behaves exactly like
    /// [`Policy::default`]. Even when non-empty, has no effect unless `allow_private_hosts_enabled` is also
    /// true (OQ-4, resolved 2026-09-22): a separate master switch.
    allow_private_hosts: Vec<String>,
    /// Master switch (OQ-4, resolved 2026-09-22, default `false`): independent of whether `allow_private_hosts`
    /// is populated. Only when this is `true` does a matching hostname in the list relax anything.
    allow_private_hosts_enabled: bool,
}

impl Default for Policy {
    /// Fail-closed: blocks everything non-public, loopback included.
    fn default() -> Self {
        Self {
            allow_loopback: false,
            allow_private_hosts: Vec::new(),
            allow_private_hosts_enabled: false,
        }
    }
}

impl Policy {
    /// Whether loopback targets are permitted. Always false for [`Policy::default`].
    #[must_use]
    pub fn allows_loopback(&self) -> bool {
        self.allow_loopback
    }

    /// Build a policy with a private-host allowlist (C-2), master switch left at its default (`false`,
    /// disabled) -- equivalent to `with_allow_private_hosts_gated(hosts, false)`. Kept for callers (and
    /// existing tests) that only care about the list itself with the switch off; production wiring goes
    /// through [`Policy::with_allow_private_hosts_gated`] via [`Config`](crate::config::Config).
    #[must_use]
    pub fn with_allow_private_hosts(hosts: Vec<String>) -> Self {
        Self::with_allow_private_hosts_gated(hosts, false)
    }

    /// Build a policy with a private-host allowlist (C-2) AND its master switch (OQ-4, resolved 2026-09-22).
    /// `hosts` should already be canonical (see [`crate::ssrf::canonical_name`]). The list only relaxes
    /// anything when `enabled` is `true`; an empty list, or `enabled = false`, is identical to
    /// [`Policy::default`] for relaxation purposes.
    #[must_use]
    pub fn with_allow_private_hosts_gated(hosts: Vec<String>, enabled: bool) -> Self {
        Self {
            allow_loopback: false,
            allow_private_hosts: hosts,
            allow_private_hosts_enabled: enabled,
        }
    }

    /// The configured allowlist, for tests and diagnostics.
    #[must_use]
    pub fn allow_private_hosts(&self) -> &[String] {
        &self.allow_private_hosts
    }

    /// Whether the allowlist master switch is enabled (OQ-4). For tests and diagnostics.
    #[must_use]
    pub fn allow_private_hosts_enabled(&self) -> bool {
        self.allow_private_hosts_enabled
    }

    /// `Err` with the block classification when `ip` may not be dialled. Loopback is relaxable only when
    /// [`Policy::allows_loopback`]; the IPv4 "private" (RFC 1918) category is relaxable only through
    /// [`Policy::check_ip_for_host`], for an exact allowlisted hostname -- this method alone never relaxes it.
    /// link-local, CGNAT, IPv6 unique-local (ULA), multicast, reserved and the cloud metadata addresses are
    /// blocked for every policy. Addresses embedding an IPv4 address are judged by it.
    ///
    /// # Errors
    /// The [`Blocked`] classification (category word only, never the address).
    pub fn check_ip(&self, ip: IpAddr) -> Result<(), Blocked> {
        match ranges::classify(ip) {
            None => Ok(()),
            Some(b) if b.kind == Kind::Loopback && self.allow_loopback => Ok(()),
            // The IP-literal path (this method) never consults the hostname allowlist at all, regardless of
            // the master switch -- see `check_ip_for_host` for the only relaxable path, and its own gate.
            Some(b) => Err(b),
        }
    }

    /// [`Policy::check_ip`], additionally relaxing the private-range check when `hostname` is an exact match in
    /// the configured allowlist (C-2) AND the master switch is enabled (OQ-4, resolved 2026-09-22). `hostname`
    /// should already be canonical. Only the `"private"` category (RFC 1918 / private-use) is relaxable this
    /// way -- metadata addresses, link-local, CGNAT and every other category stay blocked even for an
    /// allowlisted hostname with the switch on, and a disabled switch (the default) or an empty allowlist both
    /// behave exactly like [`Policy::check_ip`]. Callers must pass this only for the ORIGINAL request's
    /// hostname, never for a redirect hop: a redirect always uses [`Policy::check_ip`] instead.
    ///
    /// # Errors
    /// As [`Policy::check_ip`].
    pub fn check_ip_for_host(&self, ip: IpAddr, hostname: &str) -> Result<(), Blocked> {
        match self.check_ip(ip) {
            Ok(()) => Ok(()),
            Err(b) if b.kind == Kind::Private => {
                if self.allow_private_hosts_enabled
                    && self.allow_private_hosts.iter().any(|h| h == hostname)
                {
                    Ok(())
                } else {
                    Err(b)
                }
            }
            Err(b) => Err(b),
        }
    }

    /// The policy the binary serves with. Fail-closed ([`Policy::default`]) in every build except one compiled
    /// with the compile-time-only `bench-loopback` feature (E-8), which additionally permits loopback. There is
    /// no runtime switch for loopback: no env var or flag can change that result. `allow_private_hosts` and
    /// `allow_private_hosts_enabled` (from `FETCH_ALLOW_PRIVATE_HOSTS` / `FETCH_ALLOW_PRIVATE_HOSTS_ENABLED`,
    /// C-2 / OQ-4) are threaded through independently and default to an empty list and `false` (inert).
    #[must_use]
    pub fn for_build(allow_private_hosts: Vec<String>, allow_private_hosts_enabled: bool) -> Self {
        #[cfg(feature = "bench-loopback")]
        {
            let mut p = Self::permit_loopback_for_tests();
            p.allow_private_hosts = allow_private_hosts;
            p.allow_private_hosts_enabled = allow_private_hosts_enabled;
            p
        }
        #[cfg(not(feature = "bench-loopback"))]
        {
            Self::with_allow_private_hosts_gated(allow_private_hosts, allow_private_hosts_enabled)
        }
    }

    /// Test/bench-only policy that permits loopback (fixture server on 127.0.0.1). Absent from release builds.
    #[cfg(any(test, feature = "test-support", feature = "bench-loopback"))]
    #[must_use]
    pub fn permit_loopback_for_tests() -> Self {
        Self {
            allow_loopback: true,
            allow_private_hosts: Vec::new(),
            allow_private_hosts_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;

    #[test]
    fn default_is_fail_closed() {
        assert!(!Policy::default().allows_loopback());
    }

    #[test]
    fn default_blocks_loopback_private_and_metadata_and_passes_public() {
        let p = Policy::default();
        for a in [
            "127.0.0.1",
            "::1",
            "10.0.0.1",
            "169.254.169.254",
            "::ffff:127.0.0.1",
            "fd00:ec2::254",
        ] {
            assert!(p.check_ip(a.parse().unwrap()).is_err(), "{a}");
        }
        for a in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert!(p.check_ip(a.parse().unwrap()).is_ok(), "{a}");
        }
    }

    #[test]
    fn loopback_policy_relaxes_only_loopback() {
        let p = Policy::permit_loopback_for_tests();
        for a in ["127.0.0.1", "127.9.9.9", "::1", "::ffff:127.0.0.1"] {
            assert!(p.check_ip(a.parse().unwrap()).is_ok(), "{a}");
        }
        for a in [
            "0.0.0.0",
            "10.0.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "168.63.129.16",
            "100.64.0.1",
            "::",
            "fe80::1",
            "fd00:ec2::254",
            "224.0.0.1",
            "255.255.255.255",
            "::ffff:10.0.0.1",
        ] {
            assert!(p.check_ip(a.parse().unwrap()).is_err(), "{a}");
        }
    }

    #[cfg(not(feature = "bench-loopback"))]
    #[test]
    fn for_build_is_fail_closed_without_bench_feature() {
        let p = Policy::for_build(Vec::new(), false);
        assert_eq!(p, Policy::default());
        assert!(!p.allows_loopback());
        for a in ["127.0.0.1", "127.9.9.9", "::1"] {
            assert!(p.check_ip(a.parse().unwrap()).is_err(), "{a}");
        }
    }

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn for_build_permits_only_loopback_with_bench_feature() {
        let p = Policy::for_build(Vec::new(), false);
        assert!(p.allows_loopback());
        for a in ["127.0.0.1", "127.9.9.9", "::1"] {
            assert!(p.check_ip(a.parse().unwrap()).is_ok(), "{a}");
        }
        for a in [
            "0.0.0.0",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "::",
            "fe80::1",
            "fd00:ec2::254",
            "::ffff:10.0.0.1",
        ] {
            assert!(p.check_ip(a.parse().unwrap()).is_err(), "{a}");
        }
        assert!(p.check_ip("8.8.8.8".parse().unwrap()).is_ok());
    }

    #[test]
    fn test_constructor_permits_loopback() {
        assert!(Policy::permit_loopback_for_tests().allows_loopback());
    }

    #[cfg(not(feature = "bench-loopback"))]
    #[test]
    fn for_build_preserves_the_allowlist_without_bench_feature() {
        let p = Policy::for_build(vec!["printer.lan".to_string()], true);
        assert_eq!(p.allow_private_hosts(), &["printer.lan".to_string()]);
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "printer.lan")
            .is_ok());
    }

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn for_build_preserves_the_allowlist_with_bench_feature() {
        let p = Policy::for_build(vec!["printer.lan".to_string()], true);
        assert_eq!(p.allow_private_hosts(), &["printer.lan".to_string()]);
        assert!(p.allows_loopback());
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "printer.lan")
            .is_ok());
    }

    // --- C-2 / OQ-4 (resolved 2026-09-22): allowlist mechanism + master switch ---

    #[test]
    fn empty_allowlist_is_inert_and_matches_default() {
        let p = Policy::with_allow_private_hosts(Vec::new());
        assert_eq!(p, Policy::default());
        assert!(p
            .check_ip_for_host("10.0.0.1".parse().unwrap(), "internal.example")
            .is_err());
    }

    #[test]
    fn allowlisted_hostname_relaxes_private_range_only_for_exact_match_when_enabled() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "printer.lan")
            .is_ok());
        assert!(p
            .check_ip_for_host("10.0.0.1".parse().unwrap(), "printer.lan")
            .is_ok());
        assert!(p
            .check_ip_for_host("172.16.0.1".parse().unwrap(), "printer.lan")
            .is_ok());
        // a different hostname is not allowlisted, even resolving to the same private address
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "other.example")
            .is_err());
        // case/substring is not a match
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "notprinter.lan")
            .is_err());
    }

    #[test]
    fn allowlist_never_relaxes_metadata_link_local_or_cgnat_even_when_enabled() {
        let p = Policy::with_allow_private_hosts_gated(vec!["metadata.example".to_string()], true);
        for a in [
            "169.254.169.254", // AWS/GCP metadata
            "168.63.129.16",   // Azure wire server
            "169.254.1.1",     // link-local
            "100.64.0.1",      // CGNAT
            "127.0.0.1",       // loopback
            "0.0.0.0",         // unspecified
            "224.0.0.1",       // multicast
        ] {
            assert!(
                p.check_ip_for_host(a.parse().unwrap(), "metadata.example")
                    .is_err(),
                "{a}"
            );
        }
    }

    #[test]
    fn allowlist_does_not_touch_public_addresses() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        assert!(p
            .check_ip_for_host("8.8.8.8".parse().unwrap(), "printer.lan")
            .is_ok());
    }

    /// Table-driven regression matrix for OQ-4's master switch (resolved 2026-09-22): switch off + list
    /// populated must still fail closed (no relaxation, defense in depth); switch on + list populated relaxes
    /// only for listed hostnames; switch on + empty list relaxes nothing (nothing to relax). IP-literal
    /// rejection (via `check_ip`, never consulted by the allowlist at all) and every non-private blocked
    /// category are unaffected by the switch in every row.
    #[test]
    fn master_switch_regression_matrix() {
        struct Case {
            name: &'static str,
            hosts: &'static [&'static str],
            enabled: bool,
            hostname: &'static str,
            private_ip: &'static str,
            expect_relaxed: bool,
        }
        let cases = [
            Case {
                name: "switch off, list populated -> fail closed",
                hosts: &["printer.lan"],
                enabled: false,
                hostname: "printer.lan",
                private_ip: "192.168.1.5",
                expect_relaxed: false,
            },
            Case {
                name: "switch on, list populated, matching host -> relaxed",
                hosts: &["printer.lan"],
                enabled: true,
                hostname: "printer.lan",
                private_ip: "192.168.1.5",
                expect_relaxed: true,
            },
            Case {
                name: "switch on, list populated, non-matching host -> fail closed",
                hosts: &["printer.lan"],
                enabled: true,
                hostname: "other.example",
                private_ip: "192.168.1.5",
                expect_relaxed: false,
            },
            Case {
                name: "switch on, empty list -> nothing to relax",
                hosts: &[],
                enabled: true,
                hostname: "printer.lan",
                private_ip: "192.168.1.5",
                expect_relaxed: false,
            },
            Case {
                name: "switch off, empty list -> fail closed (default posture)",
                hosts: &[],
                enabled: false,
                hostname: "printer.lan",
                private_ip: "192.168.1.5",
                expect_relaxed: false,
            },
        ];
        for c in cases {
            let p = Policy::with_allow_private_hosts_gated(
                c.hosts.iter().map(|s| s.to_string()).collect(),
                c.enabled,
            );
            let result = p.check_ip_for_host(c.private_ip.parse().unwrap(), c.hostname);
            assert_eq!(result.is_ok(), c.expect_relaxed, "{}", c.name);

            // IP-literal rejection, metadata/loopback/CGNAT and redirect-hop revalidation (check_ip) are
            // never weakened by any combination of switch/list state.
            for a in [
                "169.254.169.254",
                "127.0.0.1",
                "100.64.0.1",
                "0.0.0.0",
                "fd00:ec2::254",
            ] {
                assert!(
                    p.check_ip(a.parse().unwrap()).is_err(),
                    "{}: {a} must stay blocked via check_ip",
                    c.name
                );
                assert!(
                    p.check_ip_for_host(a.parse().unwrap(), c.hostname).is_err(),
                    "{}: {a} must stay blocked even for the allowlisted host",
                    c.name
                );
            }
        }
    }
}
