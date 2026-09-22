//! Resolver filter and per-hop revalidation (ADR-003 steps 5 and 6). The resolver is a trait so tests inject
//! answers and A-3b supplies the real one; nothing here performs I/O. Contract: ONE lookup per validated
//! target, EVERY answer checked, and only the validated set returned for dialling, so a second lookup (DNS
//! rebinding) cannot change what is dialled.

use super::{check_url, CheckedUrl, Host, Origin};
use crate::error::FetchError;
use crate::policy::Policy;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};

/// A name resolver. Implementations must not cache across calls in a way that hides a second lookup from tests.
///
/// `Send + Sync` so one resolver can be shared (for example in an `Arc`) by concurrent tool tasks, and so the
/// futures of [`resolve_validated`], [`validate_target`] and [`revalidate_hop`] are `Send` (rmcp requires it).
/// The trait returns `impl Future`, so it is used through generics, not as `dyn Resolver`.
pub trait Resolver: Send + Sync {
    /// Resolve `host` to its full answer set.
    fn resolve(&self, host: &str) -> impl Future<Output = io::Result<Vec<IpAddr>>> + Send;
}

/// A target that passed every check: the addresses to dial (and only those).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validated {
    pub url: CheckedUrl,
    pub addrs: Vec<SocketAddr>,
}

/// Resolve `host` once, refuse if ANY answer is blocked, and return exactly the validated set.
///
/// `origin` gates the C-2 allowlist relaxation ([`Policy::check_ip_for_host`]): it applies only for
/// [`Origin::Initial`] (the exact hostname of the original request), never for [`Origin::Redirect`], per the
/// architecture's OQ-4 answer text -- a redirect hop to a private host is always refused, allowlisted or not.
///
/// # Errors
/// `DnsFailure` on a resolver error or an empty answer; `BlockedTarget` if any answer is blocked (the whole
/// answer is refused, which covers mixed public/private answers). The message never contains an address.
pub async fn resolve_validated<R: Resolver>(
    resolver: &R,
    policy: &Policy,
    host: &str,
    port: u16,
    origin: Origin,
) -> Result<Vec<SocketAddr>, FetchError> {
    let answers = resolver
        .resolve(host)
        .await
        .map_err(|_| FetchError::DnsFailure("hostname did not resolve".to_string()))?;
    if answers.is_empty() {
        return Err(FetchError::DnsFailure(
            "hostname did not resolve".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(answers.len());
    for ip in answers {
        let checked = match origin {
            Origin::Initial => policy.check_ip_for_host(ip, host),
            Origin::Redirect => policy.check_ip(ip),
        };
        if let Err(b) = checked {
            return Err(FetchError::BlockedTarget(format!(
                "hostname resolves to a non-public address ({})",
                b.category
            )));
        }
        let sa = SocketAddr::new(ip, port);
        if !out.contains(&sa) {
            out.push(sa);
        }
    }
    Ok(out)
}

/// Full validation of one URL: [`check_url`], then (for a name) the resolver filter. An IP literal is already
/// judged by `check_url` and is returned as its single address with no lookup (IP literals are never
/// allowlistable, C-2, since `check_url` has no hostname to match against the allowlist).
///
/// # Errors
/// As [`check_url`] and [`resolve_validated`].
pub async fn validate_target<R: Resolver>(
    resolver: &R,
    policy: &Policy,
    url: &str,
    origin: Origin,
) -> Result<Validated, FetchError> {
    let checked = check_url(url, policy, origin)?;
    let addrs = match &checked.host {
        Host::Ip(ip) => vec![SocketAddr::new(*ip, checked.port)],
        Host::Name(n) => resolve_validated(resolver, policy, n, checked.port, origin).await?,
    };
    Ok(Validated {
        url: checked,
        addrs,
    })
}

/// Per-hop redirect revalidation: the same scheme, userinfo, IP-literal and resolver checks applied to an
/// absolute redirect target, with failures reported as `blocked_target` (non-http(s) schemes included).
/// Resolving a relative `Location` against the current URL is the redirect loop's job (A-3b/B-3); pass the
/// absolute result here. The redirect-count limit is B-3.
///
/// # Errors
/// `BlockedTarget` for a refused target; `DnsFailure` if the new name does not resolve.
pub async fn revalidate_hop<R: Resolver>(
    resolver: &R,
    policy: &Policy,
    location: &str,
) -> Result<Validated, FetchError> {
    validate_target(resolver, policy, location, Origin::Redirect).await
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Injectable resolver: a fixed table, or a per-call script (for the rebinding shape), counting lookups.
    pub struct FakeResolver {
        scripts: Mutex<HashMap<String, Vec<io::Result<Vec<IpAddr>>>>>,
        pub calls: AtomicUsize,
    }

    impl FakeResolver {
        pub fn new() -> Self {
            Self {
                scripts: Mutex::new(HashMap::new()),
                calls: AtomicUsize::new(0),
            }
        }
        /// Answer for the n-th call for `host` (in order); the last answer repeats.
        pub fn on(self, host: &str, answers: &[&str]) -> Self {
            let v = answers.iter().map(|a| a.parse().unwrap()).collect();
            self.scripts
                .lock()
                .unwrap()
                .entry(host.into())
                .or_default()
                .push(Ok(v));
            self
        }
        pub fn fail(self, host: &str) -> Self {
            self.scripts
                .lock()
                .unwrap()
                .entry(host.into())
                .or_default()
                .push(Err(io::Error::other("nxdomain")));
            self
        }
        pub fn count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl Resolver for FakeResolver {
        fn resolve(&self, host: &str) -> impl Future<Output = io::Result<Vec<IpAddr>>> + Send {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            let mut map = self.scripts.lock().unwrap();
            let r = match map.get_mut(host) {
                Some(list) if !list.is_empty() => {
                    let i = n.min(list.len() - 1);
                    match &list[i] {
                        Ok(v) => Ok(v.clone()),
                        Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
                    }
                }
                _ => Err(io::Error::other("no such host")),
            };
            std::future::ready(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::FakeResolver;
    use super::*;

    fn assert_send<T: Send>(_: &T) {}
    fn assert_send_sync<T: Send + Sync>() {}

    /// Compile-time check: a resolver is shareable across tasks and every validation future is `Send`.
    #[test]
    fn resolver_and_validation_futures_are_send() {
        assert_send_sync::<FakeResolver>();
        let r = FakeResolver::new();
        let p = Policy::default();
        assert_send(&resolve_validated(&r, &p, "a.example", 80, Origin::Initial));
        assert_send(&validate_target(
            &r,
            &p,
            "http://a.example/",
            Origin::Initial,
        ));
        assert_send(&revalidate_hop(&r, &p, "http://a.example/"));
    }

    fn sa(s: &str, port: u16) -> SocketAddr {
        SocketAddr::new(s.parse().unwrap(), port)
    }

    #[tokio::test]
    async fn public_answer_returns_exactly_the_validated_set() {
        let r = FakeResolver::new().on(
            "example.com",
            &["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"],
        );
        let v = validate_target(
            &r,
            &Policy::default(),
            "https://example.com/x",
            Origin::Initial,
        )
        .await
        .unwrap();
        assert_eq!(
            v.addrs,
            vec![
                sa("93.184.216.34", 443),
                sa("2606:2800:220:1:248:1893:25c8:1946", 443)
            ]
        );
        assert_eq!(r.count(), 1, "resolve once");
    }

    #[tokio::test]
    async fn mixed_public_and_private_answer_is_refused_whole() {
        for private in [
            "10.0.0.5",
            "127.0.0.1",
            "169.254.169.254",
            "::1",
            "::ffff:192.168.0.1",
            "fd00:ec2::254",
            "168.63.129.16",
        ] {
            for order in [["8.8.8.8", private], [private, "8.8.8.8"]] {
                let r = FakeResolver::new().on("mixed.example", &order);
                let e = validate_target(
                    &r,
                    &Policy::default(),
                    "http://mixed.example/",
                    Origin::Initial,
                )
                .await
                .unwrap_err();
                assert_eq!(e.code(), "blocked_target", "{order:?}");
                assert!(
                    !e.to_string().contains(private),
                    "message leaks the address: {e}"
                );
                assert_eq!(r.count(), 1);
            }
        }
    }

    #[tokio::test]
    async fn all_private_answer_is_refused() {
        let r = FakeResolver::new().on("internal.example", &["192.168.1.10"]);
        let e = validate_target(
            &r,
            &Policy::default(),
            "http://internal.example/",
            Origin::Initial,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "blocked_target");
    }

    #[tokio::test]
    async fn resolver_error_and_empty_answer_are_dns_failures() {
        let r = FakeResolver::new()
            .fail("gone.example")
            .on("empty.example", &[]);
        for h in ["gone.example", "empty.example", "unknown.example"] {
            let e = validate_target(
                &r,
                &Policy::default(),
                &format!("http://{h}/"),
                Origin::Initial,
            )
            .await
            .unwrap_err();
            assert_eq!(e.code(), "dns_failure", "{h}");
        }
    }

    #[tokio::test]
    async fn ip_literals_never_reach_the_resolver() {
        let r = FakeResolver::new();
        for u in [
            "http://127.0.0.1/",
            "http://0x7f.1/",
            "http://[::1]/",
            "http://169.254.169.254/",
        ] {
            assert!(validate_target(&r, &Policy::default(), u, Origin::Initial)
                .await
                .is_err());
        }
        // a public literal passes with no lookup, returning itself
        let v = validate_target(
            &r,
            &Policy::default(),
            "http://8.8.8.8:81/",
            Origin::Initial,
        )
        .await
        .unwrap();
        assert_eq!(v.addrs, vec![sa("8.8.8.8", 81)]);
        assert_eq!(r.count(), 0);
    }

    #[tokio::test]
    async fn rebinding_shape_only_the_first_lookup_is_used_and_no_second_happens() {
        // first answer public, second private: the validated set is the first; only one lookup is made.
        let r = FakeResolver::new()
            .on("rebind.example", &["93.184.216.34"])
            .on("rebind.example", &["10.0.0.1"]);
        let v = validate_target(
            &r,
            &Policy::default(),
            "http://rebind.example/",
            Origin::Initial,
        )
        .await
        .unwrap();
        assert_eq!(v.addrs, vec![sa("93.184.216.34", 80)]);
        assert_eq!(r.count(), 1);
    }

    #[tokio::test]
    async fn duplicate_answers_are_collapsed() {
        let r = FakeResolver::new().on("dup.example", &["8.8.8.8", "8.8.8.8"]);
        let v = validate_target(
            &r,
            &Policy::default(),
            "http://dup.example/",
            Origin::Initial,
        )
        .await
        .unwrap();
        assert_eq!(v.addrs.len(), 1);
    }

    #[tokio::test]
    async fn loopback_policy_allows_localhost_answer_but_not_private_or_mixed_private() {
        let p = Policy::permit_loopback_for_tests();
        let r = FakeResolver::new()
            .on("localhost", &["127.0.0.1", "::1"])
            .on("lan.example", &["127.0.0.1", "10.0.0.1"]);
        let v = validate_target(&r, &p, "http://localhost:9000/", Origin::Initial)
            .await
            .unwrap();
        assert_eq!(v.addrs, vec![sa("127.0.0.1", 9000), sa("::1", 9000)]);
        assert!(
            validate_target(&r, &p, "http://lan.example/", Origin::Initial)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn hop_revalidation_refuses_non_http_schemes_userinfo_literals_and_private_names() {
        let r = FakeResolver::new()
            .on("ok.example", &["8.8.8.8"])
            .on("evil.example", &["10.0.0.9"]);
        let p = Policy::default();
        for loc in [
            "file:///etc/passwd",
            "ftp://ok.example/",
            "gopher://ok.example/",
            "javascript:alert(1)",
            "data:text/plain,x",
            "http://user@ok.example/",
            "http://127.0.0.1/",
            "http://2130706433/",
            "http://[::ffff:127.0.0.1]/",
            "http://169.254.169.254/latest",
            "http://evil.example/",
            "http://localhost/",
            "//ok.example/",
            "/relative",
        ] {
            let e = revalidate_hop(&r, &p, loc).await.unwrap_err();
            assert_eq!(e.code(), "blocked_target", "{loc}: {e}");
        }
        let v = revalidate_hop(&r, &p, "https://ok.example/next")
            .await
            .unwrap();
        assert_eq!(v.addrs, vec![sa("8.8.8.8", 443)]);
        assert!(v.url.https);
    }

    #[tokio::test]
    async fn c2_allowlisted_hostname_resolves_to_a_private_address() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        let r = FakeResolver::new().on("printer.lan", &["192.168.1.5"]);
        let v = validate_target(&r, &p, "http://printer.lan/", Origin::Initial)
            .await
            .unwrap();
        assert_eq!(v.addrs, vec![sa("192.168.1.5", 80)]);
    }

    #[tokio::test]
    async fn c2_allowlist_does_not_relax_metadata_answers() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        let r = FakeResolver::new().on("printer.lan", &["169.254.169.254"]);
        assert!(
            validate_target(&r, &p, "http://printer.lan/", Origin::Initial)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn c2_allowlist_does_not_cover_a_different_hostname() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        let r = FakeResolver::new().on("other.lan", &["192.168.1.5"]);
        assert!(
            validate_target(&r, &p, "http://other.lan/", Origin::Initial)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn c2_allowlist_never_applies_to_a_redirect_hop_even_to_the_same_hostname() {
        let p = Policy::with_allow_private_hosts_gated(vec!["printer.lan".to_string()], true);
        let r = FakeResolver::new().on("printer.lan", &["192.168.1.5"]);
        let e = revalidate_hop(&r, &p, "http://printer.lan/")
            .await
            .unwrap_err();
        assert_eq!(e.code(), "blocked_target");
    }

    #[tokio::test]
    async fn hop_to_name_that_now_resolves_private_is_refused() {
        let r = FakeResolver::new()
            .on("a.example", &["8.8.8.8"])
            .on("b.example", &["192.168.0.1"]);
        assert!(revalidate_hop(&r, &Policy::default(), "http://a.example/")
            .await
            .is_ok());
        assert!(revalidate_hop(&r, &Policy::default(), "http://b.example/")
            .await
            .is_err());
    }
}
