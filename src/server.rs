//! rmcp stdio handler: registers the `fetch` tool and validates its input (FR-01, FR-02). Tool futures must be
//! `Send` (A-1 spike). The URL goes through `ssrf::check_url` (scheme, userinfo, IP-literal checks; no DNS, no
//! network). Every rejection is an `isError` result `error[<code>]: <message>` naming the field, the same shape
//! rmcp 3.4 uses for schema-deserialisation failures (ADR-006 amendment 2026-09-19). A URL that passes still
//! returns a clear "not implemented yet" tool error; no fetching.

use crate::error::FetchError;
use crate::policy::Policy;
use crate::ssrf::{check_url, Origin};
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

#[derive(Clone)]
pub struct Fetch {
    // Fail-closed by default. A-3b routes every dial (and every redirect hop) through it.
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
        if let Err(e) = check_url(&p.url, &self.policy, Origin::Initial) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                e.tool_text(),
            )]));
        }
        Ok(CallToolResult::error(vec![ContentBlock::text(
            FetchError::NotImplemented.tool_text(),
        )]))
    }
}
