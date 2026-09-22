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
    /// private-range check is relaxed. Empty by default (C-2, OQ-4 still open): an empty list is inert and
    /// behaves exactly like [`Policy::default`].
    allow_private_hosts: Vec<String>,
}

impl Default for Policy {
    /// Fail-closed: blocks everything non-public, loopback included.
    fn default() -> Self {
        Self {
            allow_loopback: false,
            allow_private_hosts: Vec::new(),
        }
    }
}

impl Policy {
    /// Whether loopback targets are permitted. Always false for [`Policy::default`].
    #[must_use]
    pub fn allows_loopback(&self) -> bool {
        self.allow_loopback
    }

    /// Build a policy with a private-host allowlist (C-2). `hosts` should already be canonical (see
    /// [`crate::ssrf::canonical_name`]); an empty list is identical to [`Policy::default`]. This is a mechanism
    /// only -- OQ-4 (whether/how product policy should use it) is still open, so nothing calls this with a
    /// non-empty list except a caller that explicitly opted in via `FETCH_ALLOW_PRIVATE_HOSTS`.
    #[must_use]
    pub fn with_allow_private_hosts(hosts: Vec<String>) -> Self {
        Self {
            allow_loopback: false,
            allow_private_hosts: hosts,
        }
    }

    /// The configured allowlist, for tests and diagnostics.
    #[must_use]
    pub fn allow_private_hosts(&self) -> &[String] {
        &self.allow_private_hosts
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
            Some(b) => Err(b),
        }
    }

    /// [`Policy::check_ip`], additionally relaxing the private-range check when `hostname` is an exact match in
    /// the configured allowlist (C-2). `hostname` should already be canonical. Only the `"private"` category
    /// (RFC 1918 / private-use) is relaxable this way -- metadata addresses, link-local, CGNAT and every other
    /// category stay blocked even for an allowlisted hostname, and an empty allowlist behaves exactly like
    /// [`Policy::check_ip`]. Callers must pass this only for the ORIGINAL request's hostname, never for a
    /// redirect hop (architecture OQ-4 answer): a redirect always uses [`Policy::check_ip`] instead.
    ///
    /// # Errors
    /// As [`Policy::check_ip`].
    pub fn check_ip_for_host(&self, ip: IpAddr, hostname: &str) -> Result<(), Blocked> {
        match self.check_ip(ip) {
            Ok(()) => Ok(()),
            Err(b) if b.kind == Kind::Private => {
                if self.allow_private_hosts.iter().any(|h| h == hostname) {
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
    /// no runtime switch for loopback: no env var or flag can change that result. `allow_private_hosts` (from
    /// `FETCH_ALLOW_PRIVATE_HOSTS`, C-2) is threaded through independently and defaults to empty (inert).
    #[must_use]
    pub fn for_build(allow_private_hosts: Vec<String>) -> Self {
        #[cfg(feature = "bench-loopback")]
        {
            let mut p = Self::permit_loopback_for_tests();
            p.allow_private_hosts = allow_private_hosts;
            p
        }
        #[cfg(not(feature = "bench-loopback"))]
        {
            Self::with_allow_private_hosts(allow_private_hosts)
        }
    }

    /// Test/bench-only policy that permits loopback (fixture server on 127.0.0.1). Absent from release builds.
    #[cfg(any(test, feature = "test-support", feature = "bench-loopback"))]
    #[must_use]
    pub fn permit_loopback_for_tests() -> Self {
        Self {
            allow_loopback: true,
            allow_private_hosts: Vec::new(),
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
        let p = Policy::for_build(Vec::new());
        assert_eq!(p, Policy::default());
        assert!(!p.allows_loopback());
        for a in ["127.0.0.1", "127.9.9.9", "::1"] {
            assert!(p.check_ip(a.parse().unwrap()).is_err(), "{a}");
        }
    }

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn for_build_permits_only_loopback_with_bench_feature() {
        let p = Policy::for_build(Vec::new());
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
        let p = Policy::for_build(vec!["printer.lan".to_string()]);
        assert_eq!(p.allow_private_hosts(), &["printer.lan".to_string()]);
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "printer.lan")
            .is_ok());
    }

    #[cfg(feature = "bench-loopback")]
    #[test]
    fn for_build_preserves_the_allowlist_with_bench_feature() {
        let p = Policy::for_build(vec!["printer.lan".to_string()]);
        assert_eq!(p.allow_private_hosts(), &["printer.lan".to_string()]);
        assert!(p.allows_loopback());
        assert!(p
            .check_ip_for_host("192.168.1.5".parse().unwrap(), "printer.lan")
            .is_ok());
    }

    // --- C-2: allowlist mechanism (OQ-4 still open; empty by default) ---

    #[test]
    fn empty_allowlist_is_inert_and_matches_default() {
        let p = Policy::with_allow_private_hosts(Vec::new());
        assert_eq!(p, Policy::default());
        assert!(p
            .check_ip_for_host("10.0.0.1".parse().unwrap(), "internal.example")
            .is_err());
    }

    #[test]
    fn allowlisted_hostname_relaxes_private_range_only_for_exact_match() {
        let p = Policy::with_allow_private_hosts(vec!["printer.lan".to_string()]);
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
    fn allowlist_never_relaxes_metadata_link_local_or_cgnat() {
        let p = Policy::with_allow_private_hosts(vec!["metadata.example".to_string()]);
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
        let p = Policy::with_allow_private_hosts(vec!["printer.lan".to_string()]);
        assert!(p
            .check_ip_for_host("8.8.8.8".parse().unwrap(), "printer.lan")
            .is_ok());
    }
}
