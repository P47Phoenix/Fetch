//! rmcp stdio handler: registers the `fetch` tool and validates its input (FR-01, FR-02). Tool futures must be
//! `Send` (A-1 spike). Valid input currently returns a clear "not implemented yet" tool error; no fetching.

use crate::error::FetchError;
use crate::policy::Policy;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router, ErrorData as McpError,
};
use serde::{Deserialize, Deserializer};

/// Deserialise `T`, prefixing any error with the field name so a validation error always names the field.
fn named<'de, D: Deserializer<'de>, T: Deserialize<'de>>(field: &str, d: D) -> Result<T, D::Error> {
    T::deserialize(d).map_err(|e| serde::de::Error::custom(format!("{field}: {e}")))
}
fn de_url<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    named("url", d)
}
fn de_max_length<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    named("max_length", d)
}
fn de_start_index<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    named("start_index", d)
}
fn de_raw<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    named("raw", d)
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FetchParams {
    /// URL to fetch (http or https only)
    #[serde(deserialize_with = "de_url")]
    pub url: String,
    /// Maximum number of characters to return (default 5000)
    #[serde(default, deserialize_with = "de_max_length")]
    pub max_length: Option<u64>,
    /// Character offset to start from (default 0)
    #[serde(default, deserialize_with = "de_start_index")]
    pub start_index: Option<u64>,
    /// Return the raw body without HTML simplification
    #[serde(default, deserialize_with = "de_raw")]
    pub raw: Option<bool>,
}

/// Scheme check without a URL parser (the `url` crate arrives with A-3a): only `http://` and `https://`.
///
/// # Errors
/// `InvalidArgument` naming `url` for any other scheme or a missing host part.
pub fn validate_url(url: &str) -> Result<(), FetchError> {
    let bad = |m: &str| FetchError::InvalidArgument {
        field: "url",
        message: m.to_string(),
    };
    let Some((scheme, rest)) = url.split_once(':') else {
        return Err(bad("must be an absolute http or https URL"));
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return Err(bad("scheme must be http or https"));
    }
    match rest.strip_prefix("//") {
        Some(host) if !host.is_empty() => Ok(()),
        _ => Err(bad("must be an absolute http or https URL with a host")),
    }
}

#[derive(Clone)]
pub struct Fetch {
    // Held for A-3a/A-3b, which route every dial through it. Fail-closed by default.
    #[allow(dead_code)]
    policy: Policy,
}

impl Fetch {
    #[must_use]
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }
}

#[tool_router(server_handler)]
impl Fetch {
    #[tool(description = "Fetch a URL and return its content as markdown")]
    async fn fetch(
        &self,
        Parameters(p): Parameters<FetchParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = validate_url(&p.url) {
            return Err(McpError::invalid_params(e.to_string(), None));
        }
        Ok(CallToolResult::error(vec![ContentBlock::text(
            FetchError::NotImplemented.tool_text(),
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_url;

    #[test]
    fn accepts_http_and_https() {
        for u in [
            "http://example.com",
            "https://example.com/a?b=c",
            "HTTPS://Example.com",
        ] {
            assert!(validate_url(u).is_ok(), "{u}");
        }
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        for u in [
            "file:///etc/passwd",
            "ftp://example.com",
            "gopher://x",
            "javascript:alert(1)",
            "example.com",
            "",
            "http:",
            "http://",
        ] {
            let e = validate_url(u).unwrap_err();
            assert!(e.to_string().starts_with("url:"), "{u}: {e}");
        }
    }
}
