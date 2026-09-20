//! The only two name-resolution pieces in the crate: [`SystemResolver`] (the real lookup, injected into the
//! SSRF core's [`Resolver`] seam) and [`Pinned`] (what the HTTP client is allowed to dial).
//!
//! `Pinned` is the sole dial route (ADR-003 step 5, architect N2b): the client is built per hop with a resolver
//! that returns exactly the `Validated.addrs` for exactly the validated host and refuses every other name, so the
//! client can neither re-resolve (rebinding) nor fall back to its own system resolver. IP-literal hosts never
//! reach any resolver; they were judged by `check_url` and cross-checked against the client's URL parser.

use crate::ssrf::resolver::Resolver;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};

/// Upper bound on the answers taken from one lookup (architect N5). Order is preserved (Happy Eyeballs order);
/// only the addresses kept are validated and dialled, so truncation cannot widen what is reachable.
pub const MAX_ANSWERS: usize = 16;

/// The real resolver: one `getaddrinfo`-equivalent lookup through tokio (`lookup_host`, on the blocking pool).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemResolver;

impl Resolver for SystemResolver {
    fn resolve(&self, host: &str) -> impl Future<Output = io::Result<Vec<IpAddr>>> + Send {
        let target = (host.to_string(), 0u16);
        async move {
            let answers = tokio::net::lookup_host(target).await?;
            Ok(answers.map(|a| a.ip()).take(MAX_ANSWERS).collect())
        }
    }
}

/// Lower-case, without a trailing dot: the form `check_url` yields for names.
pub(super) fn normalize(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// Resolver handed to the HTTP client for one hop: the validated set for the validated host, nothing else.
#[derive(Debug, Clone)]
pub struct Pinned {
    host: String,
    addrs: Vec<SocketAddr>,
}

impl Pinned {
    #[must_use]
    pub fn new(host: &str, addrs: Vec<SocketAddr>) -> Self {
        Self {
            host: normalize(host),
            addrs,
        }
    }
}

impl Resolve for Pinned {
    fn resolve(&self, name: Name) -> Resolving {
        let answer = (normalize(name.as_str()) == self.host).then(|| self.addrs.clone());
        Box::pin(async move {
            match answer {
                Some(addrs) => Ok(Box::new(addrs.into_iter()) as Addrs),
                None => Err("name is not the validated target".into()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize, SystemResolver};
    use crate::error::FetchError;
    use crate::policy::Policy;
    use crate::ssrf::resolver::{resolve_validated, Resolver};

    #[tokio::test]
    async fn system_resolver_resolves_localhost_and_the_core_refuses_it() {
        let name = "localhost";
        let answers = SystemResolver.resolve(name).await.unwrap();
        assert!(!answers.is_empty() && answers.iter().all(|a| a.is_loopback()));
        let refused = resolve_validated(&SystemResolver, &Policy::default(), "localhost", 80).await;
        assert!(
            matches!(refused, Err(FetchError::BlockedTarget(_))),
            "{refused:?}"
        );
        let ok = resolve_validated(
            &SystemResolver,
            &Policy::permit_loopback_for_tests(),
            "localhost",
            80,
        )
        .await;
        assert!(ok.is_ok_and(|v| !v.is_empty()));
    }

    #[tokio::test]
    async fn system_resolver_reports_failure_for_an_invalid_name() {
        let name = "no-such-host.invalid";
        assert!(SystemResolver.resolve(name).await.is_err());
    }

    #[test]
    fn normalize_lowercases_and_drops_one_trailing_dot() {
        assert_eq!(normalize("Example.COM."), "example.com");
    }
}
