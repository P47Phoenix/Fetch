//! `FetchError`: stable error codes and mapping to tool-result text (architecture 6). A-2 carries only the
//! variants it can produce; A-3a/A-3b/A-7 add the rest.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// Parameter violation. Surfaces as a protocol-level invalid-params error naming the field.
    InvalidArgument {
        field: &'static str,
        message: String,
    },
    /// Valid input, but fetching does not exist yet (Sprint 1 skeleton). Removed by A-3b.
    NotImplemented,
}

impl FetchError {
    /// Stable machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "invalid_argument",
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
        assert!(FetchError::NotImplemented
            .tool_text()
            .contains("not implemented yet"));
    }
}
