//! `Policy` skeleton (architecture 14.1, R12). Fail-closed by default: it permits no non-public target.
//! A-3a fills in the range table, IP-literal check, resolver filter and per-hop revalidation. The only way to
//! permit loopback is the constructor below, compiled only with `test-support` or `bench-loopback` (never in
//! a release build; asserted by scripts/check-release-features.sh) or in unit tests of this crate.

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
    fn test_constructor_permits_loopback() {
        assert!(Policy::permit_loopback_for_tests().allows_loopback());
    }
}
