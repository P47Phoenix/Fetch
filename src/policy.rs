//! `Policy` (architecture 14.1, R12). Fail-closed by default: it permits no non-public target. `ssrf::check_url`,
//! the resolver filter and per-hop revalidation all decide through [`Policy::check_ip`]. The only way to
//! permit loopback is the constructor below, compiled only with `test-support` or `bench-loopback` (never in
//! a release build; asserted by scripts/check-release-features.sh) or in unit tests of this crate.

use crate::ssrf::ranges::{self, Blocked, Kind};
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    allow_loopback: bool,
}

impl Default for Policy {
    /// Fail-closed: blocks everything non-public, loopback included.
    fn default() -> Self {
        Self {
            allow_loopback: false,
        }
    }
}

impl Policy {
    /// Whether loopback targets are permitted. Always false for [`Policy::default`].
    #[must_use]
    pub fn allows_loopback(&self) -> bool {
        self.allow_loopback
    }

    /// `Err` with the block classification when `ip` may not be dialled. Loopback is the only relaxable class
    /// (and only when [`Policy::allows_loopback`]); private, link-local, CGNAT, ULA, multicast, reserved and the
    /// cloud metadata addresses are blocked for every policy. Addresses embedding an IPv4 address are judged by it.
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

    /// Test/bench-only policy that permits loopback (fixture server on 127.0.0.1). Absent from release builds.
    #[cfg(any(test, feature = "test-support", feature = "bench-loopback"))]
    #[must_use]
    pub fn permit_loopback_for_tests() -> Self {
        Self {
            allow_loopback: true,
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

    #[test]
    fn test_constructor_permits_loopback() {
        assert!(Policy::permit_loopback_for_tests().allows_loopback());
    }
}
