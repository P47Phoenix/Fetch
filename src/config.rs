//! Immutable `Config` parsed once from the environment (architecture 7). An invalid value is a startup error
//! naming the variable (main prints it to stderr and exits non-zero). C-1 (Sprint 10) adds the `FETCH_ROBOTS_TXT`
//! and `FETCH_ALLOW_PRIVATE_HOSTS` variables; the remaining variable (user agent, allowed ports) arrives with
//! the story that uses it.
//!
//! `FETCH_ROBOTS_TXT` is parsed and validated but is a placeholder only (C-1): OQ-3 (whether fetches should
//! respect robots.txt by default) is still open, so today's behaviour -- robots.txt is never fetched or
//! enforced -- is unchanged regardless of the value set here. `FETCH_ALLOW_PRIVATE_HOSTS` wires a mechanism
//! (C-2) whose default is an empty, inert allowlist; OQ-4 (whether/how to use it) is still open.

use crate::obs::Level;

/// Placeholder toggle for future robots.txt handling (B-4, Sprint 11). Parsed and validated now so the env var
/// name and syntax are stable, but neither variant changes behaviour today: robots.txt is not fetched or
/// enforced by either setting until B-4 implements it. The default ([`RobotsMode::Ignore`]) preserves today's
/// behaviour and does NOT represent a decision on OQ-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsMode {
    Ignore,
    Enforce,
}

impl RobotsMode {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ignore" => Some(Self::Ignore),
            "enforce" => Some(Self::Enforce),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub log_level: Level,
    pub timeout_ms: u64,
    pub max_bytes: u64,
    pub max_length_cap: u64,
    pub max_concurrency: u64,
    /// Placeholder only; see the module docs and [`RobotsMode`]. Not yet enforced (B-4).
    pub robots_txt: RobotsMode,
    /// Hostnames (exact, canonical lowercase) for which the private-IP-range check is relaxed (C-2). Empty by
    /// default, which is inert and identical to today's fail-closed behaviour for everyone who does not set
    /// `FETCH_ALLOW_PRIVATE_HOSTS`. Does not relax the metadata-address, scheme or port rules, and does not
    /// apply to IP-literal hosts or to redirect hops (see `ssrf::Policy`).
    pub allow_private_hosts: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: Level::Warn,
            timeout_ms: 15_000,
            max_bytes: 5_242_880,
            max_length_cap: 100_000,
            max_concurrency: 3,
            robots_txt: RobotsMode::Ignore,
            allow_private_hosts: Vec::new(),
        }
    }
}

impl Config {
    /// Parse from a variable lookup (injectable for tests). Unset means the default.
    ///
    /// # Errors
    /// A message naming the offending variable when its value is invalid.
    pub fn from_lookup<F: Fn(&str) -> Option<String>>(get: F) -> Result<Self, String> {
        let mut c = Self::default();
        if let Some(v) = get("FETCH_LOG") {
            c.log_level = Level::parse(&v)
                .ok_or_else(|| format!("FETCH_LOG: expected error|warn|info|debug, got {v:?}"))?;
        }
        for (name, slot) in [
            ("FETCH_TIMEOUT_MS", &mut c.timeout_ms),
            ("FETCH_MAX_BYTES", &mut c.max_bytes),
            ("FETCH_MAX_LENGTH_CAP", &mut c.max_length_cap),
            ("FETCH_MAX_CONCURRENCY", &mut c.max_concurrency),
        ] {
            if let Some(v) = get(name) {
                *slot = match v.parse::<u64>() {
                    Ok(n) if n > 0 => n,
                    _ => return Err(format!("{name}: expected a positive integer, got {v:?}")),
                };
            }
        }
        if let Some(v) = get("FETCH_ROBOTS_TXT") {
            c.robots_txt = RobotsMode::parse(&v)
                .ok_or_else(|| format!("FETCH_ROBOTS_TXT: expected ignore|enforce, got {v:?}"))?;
        }
        if let Some(v) = get("FETCH_ALLOW_PRIVATE_HOSTS") {
            c.allow_private_hosts = parse_allow_private_hosts(&v)?;
        }
        Ok(c)
    }

    /// Parse from the process environment.
    ///
    /// # Errors
    /// See [`Config::from_lookup`].
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }
}

/// Comma-separated hostnames for `FETCH_ALLOW_PRIVATE_HOSTS`: trimmed, lower-cased via
/// [`crate::ssrf::canonical_name`], no empty entries, no IP literals (an IP literal cannot be allowlisted --
/// `ssrf::Policy` judges IP-literal hosts before any hostname is known).
fn parse_allow_private_hosts(v: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in v.split(',') {
        let t = raw.trim();
        if t.is_empty() {
            return Err(format!(
                "FETCH_ALLOW_PRIVATE_HOSTS: empty hostname in {v:?}"
            ));
        }
        if t.parse::<std::net::IpAddr>().is_ok() || t.starts_with('[') {
            return Err(format!(
                "FETCH_ALLOW_PRIVATE_HOSTS: {t:?} is an IP literal, which cannot be allowlisted"
            ));
        }
        out.push(crate::ssrf::canonical_name(t));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::obs::Level;

    fn with<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn defaults() {
        let c = Config::from_lookup(with(&[])).unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(
            (
                c.timeout_ms,
                c.max_bytes,
                c.max_length_cap,
                c.max_concurrency
            ),
            (15_000, 5_242_880, 100_000, 3)
        );
    }

    #[test]
    fn overrides() {
        let c = Config::from_lookup(with(&[
            ("FETCH_LOG", "debug"),
            ("FETCH_MAX_CONCURRENCY", "7"),
        ]))
        .unwrap();
        assert_eq!((c.log_level, c.max_concurrency), (Level::Debug, 7));
    }

    #[test]
    fn invalid_values_name_the_variable() {
        for (k, v) in [
            ("FETCH_LOG", "loud"),
            ("FETCH_TIMEOUT_MS", "abc"),
            ("FETCH_MAX_BYTES", "0"),
            ("FETCH_MAX_LENGTH_CAP", "-1"),
            ("FETCH_ROBOTS_TXT", "sometimes"),
            ("FETCH_ALLOW_PRIVATE_HOSTS", "a.example,,b.example"),
            ("FETCH_ALLOW_PRIVATE_HOSTS", "10.0.0.1"),
            ("FETCH_ALLOW_PRIVATE_HOSTS", "[::1]"),
        ] {
            let e = Config::from_lookup(with(&[(k, v)])).unwrap_err();
            assert!(e.starts_with(k), "{e}");
        }
    }

    #[test]
    fn robots_txt_default_is_ignore_and_is_a_placeholder_only() {
        let c = Config::from_lookup(with(&[])).unwrap();
        assert_eq!(c.robots_txt, super::RobotsMode::Ignore);
    }

    #[test]
    fn robots_txt_accepts_ignore_and_enforce() {
        for (v, want) in [
            ("ignore", super::RobotsMode::Ignore),
            ("IGNORE", super::RobotsMode::Ignore),
            ("enforce", super::RobotsMode::Enforce),
        ] {
            let c = Config::from_lookup(with(&[("FETCH_ROBOTS_TXT", v)])).unwrap();
            assert_eq!(c.robots_txt, want, "{v}");
        }
    }

    #[test]
    fn allow_private_hosts_default_is_empty() {
        let c = Config::from_lookup(with(&[])).unwrap();
        assert!(c.allow_private_hosts.is_empty());
    }

    #[test]
    fn allow_private_hosts_parses_and_canonicalizes() {
        let c = Config::from_lookup(with(&[(
            "FETCH_ALLOW_PRIVATE_HOSTS",
            " Printer.LAN. , nas.internal ",
        )]))
        .unwrap();
        assert_eq!(c.allow_private_hosts, vec!["printer.lan", "nas.internal"]);
    }
}
