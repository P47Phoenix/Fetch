//! Immutable `Config` parsed once from the environment (architecture 7). An invalid value is a startup error
//! naming the variable (main prints it to stderr and exits non-zero). A-2 parses the subset it needs; the
//! remaining variables (user agent, allowed ports, robots) arrive with the stories that use them.

use crate::obs::Level;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub log_level: Level,
    pub timeout_ms: u64,
    pub max_bytes: u64,
    pub max_length_cap: u64,
    pub max_concurrency: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: Level::Warn,
            timeout_ms: 15_000,
            max_bytes: 5_242_880,
            max_length_cap: 100_000,
            max_concurrency: 3,
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
        ] {
            let e = Config::from_lookup(with(&[(k, v)])).unwrap_err();
            assert!(e.starts_with(k), "{e}");
        }
    }
}
