//! Immutable `Config` parsed once from the environment (architecture 7). An invalid value is a startup error
//! naming the variable (main prints it to stderr and exits non-zero). C-1 (Sprint 10) adds the `FETCH_ROBOTS_TXT`
//! and `FETCH_ALLOW_PRIVATE_HOSTS` variables; the remaining variable (user agent, allowed ports) arrives with
//! the story that uses it.
//!
//! `FETCH_ROBOTS_TXT=enforce` (B-4) genuinely fetches and enforces the target origin's `robots.txt`: on the
//! initial hop only (a redirect is not re-checked), through the same guarded `FetchClient` used for the fetch
//! itself (SSRF-checked, size-capped at 512 KB), failing OPEN (proceeding as if nothing were disallowed) on any
//! error fetching, reading or parsing it -- missing, non-2xx, refused by SSRF, timed out, truncated at the cap,
//! malformed, whatever. A disallowed path is refused with `error[robots_disallowed]`. The default
//! ([`RobotsMode::Ignore`]) is unchanged: robots.txt is never fetched or enforced unless an operator opts in.
//! OQ-3 is RESOLVED (2026-09-22, owner decision,
//! Sprint 13): the default stays `ignore` BY DESIGN, not by omission, because network-level ACLs elsewhere in
//! the operator's infrastructure are the intended control point for this concern; `FETCH_ROBOTS_TXT=enforce`
//! remains available for operators who want it, but it is not the shipped default and will not become the
//! default. `FETCH_ALLOW_PRIVATE_HOSTS` wires a mechanism (C-2): a per-hostname allowlist that relaxes only the
//! RFC 1918 "private" range check. OQ-4 is RESOLVED (2026-09-22, owner decision, Sprint 13): the mechanism
//! ships, available and enabled by operator choice, gated behind a SEPARATE master switch,
//! `FETCH_ALLOW_PRIVATE_HOSTS_ENABLED` (default `false`). Even a non-empty `FETCH_ALLOW_PRIVATE_HOSTS` list has
//! no effect unless the master switch is explicitly set to `true` -- defense in depth, so a populated list left
//! over from a prior config (or set defensively "just in case") cannot silently relax anything. The recommended
//! posture for most operators is to leave the master switch at its default (`false`) and never populate the
//! list at all.

use crate::obs::Level;

/// robots.txt handling (B-4, Sprint 11). [`RobotsMode::Enforce`] genuinely fetches and enforces the target
/// origin's robots.txt (initial hop only, fail-open on any fetch/parse error, SSRF-checked and 512 KB capped
/// like a normal fetch -- see the module docs); [`RobotsMode::Ignore`], the default, never fetches it. The
/// default preserves the pre-B-4 behaviour; OQ-3 is resolved (2026-09-22) as staying `ignore` by design (whether to enforce it by
/// default).
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
    /// See the module docs and [`RobotsMode`]. Enforced (B-4) only when set to [`RobotsMode::Enforce`]; the
    /// default is [`RobotsMode::Ignore`].
    pub robots_txt: RobotsMode,
    /// Hostnames (exact, canonical lowercase) for which the private-IP-range check is relaxed (C-2). Empty by
    /// default, which is inert and identical to today's fail-closed behaviour for everyone who does not set
    /// `FETCH_ALLOW_PRIVATE_HOSTS`. Does not relax the metadata-address, scheme or port rules, and does not
    /// apply to IP-literal hosts or to redirect hops (see `ssrf::Policy`).
    pub allow_private_hosts: Vec<String>,
    /// Master switch for the `allow_private_hosts` mechanism (OQ-4, resolved 2026-09-22). Default `false`
    /// (disabled): even a non-empty `allow_private_hosts` list has no effect unless this is explicitly `true`
    /// (`FETCH_ALLOW_PRIVATE_HOSTS_ENABLED=true`). This is a separate, independent gate from whether the list
    /// itself is empty -- a single kill switch an operator can flip to hard-disable the whole
    /// allowlist-relaxation mechanism regardless of list contents.
    pub allow_private_hosts_enabled: bool,
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
            allow_private_hosts_enabled: false,
        }
    }
}

/// `true`/`false`, case-insensitive, matching the convention used by [`RobotsMode::parse`] and every other
/// boolean-shaped `FETCH_*` variable: no other spelling (`1`/`0`, `yes`/`no`, `on`/`off`) is accepted, and an
/// invalid value is a startup error naming the variable.
fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
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
        if let Some(v) = get("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED") {
            c.allow_private_hosts_enabled = parse_bool(&v).ok_or_else(|| {
                format!("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED: expected true|false, got {v:?}")
            })?;
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

/// Comma-separated hostnames for `FETCH_ALLOW_PRIVATE_HOSTS`: trimmed, validated and canonicalized via
/// [`crate::ssrf::validate_allowlist_hostname`] (rejects exactly what `ssrf::parse_host` would fold to an IP
/// literal, or refuse outright -- ports, slashes, brackets, decimal/octal/hex IPv4 spellings, ...), no empty
/// entries. An all-whitespace (or empty) value is treated as an empty list, so `FETCH_ALLOW_PRIVATE_HOSTS=`
/// does not hard-fail startup; a stray comma between real entries (e.g. `"a.example,,b.example"`) is still
/// rejected, since that comma separates two real entries rather than being incidental whitespace.
fn parse_allow_private_hosts(v: &str) -> Result<Vec<String>, String> {
    if v.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for raw in v.split(',') {
        let t = raw.trim();
        if t.is_empty() {
            return Err(format!(
                "FETCH_ALLOW_PRIVATE_HOSTS: empty hostname in {v:?}"
            ));
        }
        match crate::ssrf::validate_allowlist_hostname(t) {
            Ok(name) => out.push(name),
            Err(reason) => {
                return Err(format!("FETCH_ALLOW_PRIVATE_HOSTS: {t:?} {reason}"));
            }
        }
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
            ("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED", "yes"),
        ] {
            let e = Config::from_lookup(with(&[(k, v)])).unwrap_err();
            assert!(e.starts_with(k), "{e}");
        }
    }

    /// The default is `Ignore` (OQ-3 resolved 2026-09-22: `ignore` stays the default by design), but -- unlike before B-4 -- `Enforce` is now a real,
    /// working mechanism, not a no-op placeholder.
    #[test]
    fn robots_txt_default_is_ignore_but_enforce_is_a_real_mechanism() {
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

    // --- OQ-4 (resolved 2026-09-22): master switch, separate from list contents ---

    #[test]
    fn allow_private_hosts_enabled_default_is_false() {
        let c = Config::from_lookup(with(&[])).unwrap();
        assert!(!c.allow_private_hosts_enabled);
    }

    #[test]
    fn allow_private_hosts_enabled_accepts_true_and_false_case_insensitively() {
        for (v, want) in [
            ("true", true),
            ("TRUE", true),
            ("True", true),
            ("false", false),
            ("FALSE", false),
        ] {
            let c = Config::from_lookup(with(&[("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED", v)])).unwrap();
            assert_eq!(c.allow_private_hosts_enabled, want, "{v}");
        }
    }

    /// The switch is independent of the list: a populated list with the switch left at its default (false)
    /// still parses fine and the list is retained, but (per `Policy`, see policy.rs tests) has no effect.
    #[test]
    fn allow_private_hosts_enabled_is_independent_of_list_contents() {
        let c = Config::from_lookup(with(&[
            ("FETCH_ALLOW_PRIVATE_HOSTS", "printer.lan"),
            // switch not set: stays false even though the list is populated
        ]))
        .unwrap();
        assert_eq!(c.allow_private_hosts, vec!["printer.lan".to_string()]);
        assert!(!c.allow_private_hosts_enabled);

        let c2 = Config::from_lookup(with(&[
            ("FETCH_ALLOW_PRIVATE_HOSTS", "printer.lan"),
            ("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED", "true"),
        ]))
        .unwrap();
        assert!(c2.allow_private_hosts_enabled);

        let c3 =
            Config::from_lookup(with(&[("FETCH_ALLOW_PRIVATE_HOSTS_ENABLED", "true")])).unwrap();
        assert!(c3.allow_private_hosts.is_empty());
        assert!(c3.allow_private_hosts_enabled);
    }

    #[test]
    fn allow_private_hosts_empty_or_whitespace_value_is_an_empty_list() {
        for v in ["", "   ", "\t"] {
            let c = Config::from_lookup(with(&[("FETCH_ALLOW_PRIVATE_HOSTS", v)])).unwrap();
            assert!(c.allow_private_hosts.is_empty(), "{v:?}");
        }
    }

    #[test]
    fn allow_private_hosts_rejects_ip_literal_spellings_beyond_plain_dotted_form() {
        for v in [
            "0xa000001",  // hex whole-address form of 10.0.0.1
            "10.1",       // short IPv4 form
            "0177.0.0.1", // octal form of 127.0.0.1
            "2130706433", // decimal form of 127.0.0.1
            "host:8080",  // a port is not a hostname
            "host/path",  // a slash is not a hostname
        ] {
            let e = Config::from_lookup(with(&[("FETCH_ALLOW_PRIVATE_HOSTS", v)])).unwrap_err();
            assert!(e.starts_with("FETCH_ALLOW_PRIVATE_HOSTS"), "{v}: {e}");
        }
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
