//! `FetchError`: stable error codes and mapping to tool-result text (architecture 6). A-2 carries only the
//! variants it can produce; A-3a/A-3b/A-7 add the rest.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// Parameter violation. Surfaces as an `isError` tool result (`error[invalid_argument]: field: msg`), per the ADR-006 amendment.
    InvalidArgument {
        field: &'static str,
        message: String,
    },
    /// Port policy, non-public address, blocked redirect (architecture 6.1). The message gives a category only,
    /// never the resolved address, and is produced before any connection.
    BlockedTarget(String),
    /// The name did not resolve (NXDOMAIN, resolver error, empty answer).
    DnsFailure(String),
    /// Valid input, but fetching does not exist yet. Removed by A-3b.
    NotImplemented,
}

impl FetchError {
    /// Stable machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::BlockedTarget(_) => "blocked_target",
            Self::DnsFailure(_) => "dns_failure",
            Self::NotImplemented => "not_implemented",
        }
    }

    /// `error[<code>]: <message>` text for an `isError` tool result.
    #[must_use]
    pub fn tool_text(&self) -> String {
        format!("error[{}]: {}", self.code(), self)
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { field, message } => write!(f, "{field}: {message}"),
            Self::BlockedTarget(m) | Self::DnsFailure(m) => f.write_str(m),
            Self::NotImplemented => f.write_str(
                "fetch is not implemented yet: this build validates input but performs no network requests",
            ),
        }
    }
}

impl std::error::Error for FetchError {}

#[cfg(test)]
mod tests {
    use super::FetchError;

    #[test]
    fn codes_and_text_are_stable() {
        let e = FetchError::InvalidArgument {
            field: "url",
            message: "missing".into(),
        };
        assert_eq!(e.code(), "invalid_argument");
        assert_eq!(e.tool_text(), "error[invalid_argument]: url: missing");
        assert_eq!(FetchError::NotImplemented.code(), "not_implemented");
        let b = FetchError::BlockedTarget("host is not public".into());
        assert_eq!(b.tool_text(), "error[blocked_target]: host is not public");
        assert_eq!(FetchError::DnsFailure("x".into()).code(), "dns_failure");
        assert!(FetchError::NotImplemented
            .tool_text()
            .contains("not implemented yet"));
    }
}
