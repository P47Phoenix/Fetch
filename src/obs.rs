//! Minimal stderr logger (architecture 8). Hand-rolled to avoid `tracing-subscriber` weight. Never writes to
//! stdout, which carries only MCP frames (FR-13). Line format: `level event key=value ...`.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

/// True when a message at `msg` level is emitted under threshold `max`.
#[must_use]
pub fn enabled(max: Level, msg: Level) -> bool {
    msg <= max
}

/// Write one line to stderr if `msg` is enabled under `max`.
pub fn log(max: Level, msg: Level, event: &str, detail: fmt::Arguments<'_>) {
    if enabled(max, msg) {
        eprintln!("{} {event} {detail}", msg.as_str());
    }
}

#[cfg(test)]
mod tests {
    use super::{enabled, Level};

    #[test]
    fn thresholds() {
        assert!(enabled(Level::Warn, Level::Error));
        assert!(!enabled(Level::Warn, Level::Info));
        assert_eq!(Level::parse("debug"), Some(Level::Debug));
        assert_eq!(Level::parse("loud"), None);
    }
}
