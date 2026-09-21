//! `FetchError`: stable error codes and mapping to tool-result text (architecture 6). A-3b adds the transport
//! variants; A-7 refines them into cause-specific messages.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// Parameter violation. Surfaces as an `isError` tool result (`error[invalid_argument]: field: msg`), per the ADR-006 amendment.
    InvalidArgument {
        field: &'static str,
        message: String,
    },
    /// Non-public address, blocked scheme or userinfo, blocked redirect (architecture 6.1). There is no port policy
    /// yet: every port except 0 is allowed until the allowed-ports setting lands with C-1/B-3. The message gives a category only,
    /// never the resolved address, and is produced before any connection.
    BlockedTarget(String),
    /// The name did not resolve (NXDOMAIN, resolver error, empty answer).
    DnsFailure(String),
    /// The body (wire or decompressed) exceeded `FETCH_MAX_BYTES`, or `Content-Length` announced more.
    TooLarge(String),
    /// The overall deadline (queueing excluded from the fetch budget, DNS, connect, TLS, headers, every
    /// redirect hop and the body) elapsed, or the call queued for a slot longer than the timeout.
    Timeout(String),
    /// The response is not a text type (an image, a PDF, `application/octet-stream`...). Names the media type (A-6).
    UnsupportedContentType(String),
    /// `Content-Encoding` other than absent, `identity` or a single `gzip` (ADR-004).
    UnsupportedEncoding(String),
    /// A non-2xx final status. Cause-specific structure arrives with A-7.
    HttpStatus(u16),
    /// More redirect hops than the fixed bound (ADR-003 step 6; the configurable limit is B-3).
    TooManyRedirects,
    /// Connection, TLS or transport failure. Category only: the message never carries an address.
    Network(String),
    /// A malformed response: oversized or too many headers, bad redirect `Location`, corrupt gzip.
    BadResponse(String),
    /// The HTML converter hit a memory or output limit (or could not process the page). `raw=true` returns the text unconverted.
    ConverterLimit(String),
    /// An unexpected internal failure on an `Err` path (panics abort instead).
    Internal(String),
}

impl FetchError {
    /// Stable machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::BlockedTarget(_) => "blocked_target",
            Self::DnsFailure(_) => "dns_failure",
            Self::TooLarge(_) => "too_large",
            Self::Timeout(_) => "timeout",
            Self::UnsupportedContentType(_) => "unsupported_content_type",
            Self::UnsupportedEncoding(_) => "unsupported_encoding",
            Self::HttpStatus(_) => "http_error",
            Self::TooManyRedirects => "too_many_redirects",
            Self::Network(_) => "network_error",
            Self::BadResponse(_) => "bad_response",
            Self::ConverterLimit(_) => "converter_limit",
            Self::Internal(_) => "internal",
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
            Self::BlockedTarget(m)
            | Self::DnsFailure(m)
            | Self::TooLarge(m)
            | Self::Timeout(m)
            | Self::UnsupportedContentType(m)
            | Self::UnsupportedEncoding(m)
            | Self::Network(m)
            | Self::BadResponse(m)
            | Self::ConverterLimit(m)
            | Self::Internal(m) => f.write_str(m),
            Self::HttpStatus(code) => write!(f, "the server answered with HTTP status {code}"),
            Self::TooManyRedirects => f.write_str("too many redirects"),
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
        assert_eq!(FetchError::TooLarge("x".into()).code(), "too_large");
        assert_eq!(FetchError::Timeout("x".into()).code(), "timeout");
        assert_eq!(
            FetchError::HttpStatus(404).tool_text(),
            "error[http_error]: the server answered with HTTP status 404"
        );
        assert_eq!(FetchError::TooManyRedirects.code(), "too_many_redirects");
        let b = FetchError::BlockedTarget("host is not public".into());
        assert_eq!(b.tool_text(), "error[blocked_target]: host is not public");
        assert_eq!(FetchError::DnsFailure("x".into()).code(), "dns_failure");
    }
}
