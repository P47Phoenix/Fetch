//! rmcp stdio handler: registers the `fetch` tool, validates its input (FR-01, FR-02) and runs it through the
//! guarded [`FetchClient`] (A-3b). Tool futures must be `Send` (A-1 spike). Every failure is an `isError` result
//! `error[<code>]: <message>` naming the field or cause, the same shape rmcp 3.4 uses for schema-deserialisation
//! failures (ADR-006 amendment 2026-09-19). On success the text is returned as fetched: no label, wrapper or
//! notice is added (OQ-5 decided NO on 2026-09-20, ADR-006 note).
//!
//! Interim scope: the body is decoded as UTF-8 and the requested character window (`start_index`,
//! `max_length`) is kept while the whole body is still read and size-checked; HTML conversion, early stop,
//! pagination messages and `raw` handling arrive with A-4, A-5 and A-6.

use crate::config::Config;
use crate::fetch::dns::SystemResolver;
use crate::fetch::{FetchClient, Fetched, Limits};
use crate::policy::Policy;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router, ErrorData as McpError,
};
use serde::{Deserialize, Deserializer};
use std::sync::Arc;

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

/// Default `max_length` (FR-02).
const DEFAULT_MAX_LENGTH: u64 = 5000;

/// Keeps the characters `[skip, skip + take)` of the text pushed through it; memory is bounded by `take`.
#[derive(Debug)]
pub struct Window {
    skip: u64,
    take: u64,
    out: String,
    taken: u64,
}

impl Window {
    #[must_use]
    pub fn new(start_index: u64, max_length: u64) -> Self {
        Self {
            skip: start_index,
            take: max_length,
            out: String::new(),
            taken: 0,
        }
    }

    pub fn push(&mut self, text: &str) {
        for ch in text.chars() {
            if self.skip > 0 {
                self.skip -= 1;
            } else if self.taken < self.take {
                self.out.push(ch);
                self.taken += 1;
            } else {
                return;
            }
        }
    }

    #[must_use]
    pub fn into_text(self) -> String {
        self.out
    }
}

#[derive(Clone)]
pub struct Fetch {
    client: Arc<FetchClient<SystemResolver>>,
    max_length_cap: u64,
}

impl Fetch {
    /// The guarded client for `policy` with the limits of `cfg`. The policy is required: there is no default.
    #[must_use]
    pub fn new(policy: Policy, cfg: &Config) -> Self {
        Self {
            client: Arc::new(FetchClient::new(
                policy,
                Arc::new(SystemResolver),
                Limits::from_config(cfg),
            )),
            max_length_cap: cfg.max_length_cap,
        }
    }
}

/// FR-14 (A-9): when a redirect was followed the text begins with the final URL and HTTP status, one line each and a
/// blank line; with no redirect nothing is added. The header sits outside the `max_length` window.
#[must_use]
pub fn with_header(f: &Fetched, body: String) -> String {
    if f.redirects == 0 {
        return body;
    }
    format!("URL: {}\nStatus: {}\n\n{body}", f.final_url, f.status)
}

#[tool_router(server_handler)]
impl Fetch {
    #[tool(description = "Fetch a URL and return its content as markdown")]
    async fn fetch(
        &self,
        Parameters(p): Parameters<FetchParams>,
    ) -> Result<CallToolResult, McpError> {
        let max_length = p
            .max_length
            .unwrap_or(DEFAULT_MAX_LENGTH)
            .min(self.max_length_cap);
        let mut window = Window::new(p.start_index.unwrap_or(0), max_length);
        let result = self
            .client
            .fetch(&p.url, &mut |text: &str| window.push(text))
            .await;
        Ok(match result {
            Ok(f) => CallToolResult::success(vec![ContentBlock::text(with_header(
                &f,
                window.into_text(),
            ))]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.tool_text())]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{with_header, Window};
    use crate::fetch::Fetched;

    fn fetched(redirects: usize) -> Fetched {
        Fetched {
            final_url: "https://b.example/final".into(),
            status: 200,
            redirects,
            wire_bytes: 4,
        }
    }

    #[test]
    fn header_is_added_only_after_a_redirect() {
        assert_eq!(with_header(&fetched(0), "body".into()), "body");
        assert_eq!(
            with_header(&fetched(2), "body".into()),
            "URL: https://b.example/final\nStatus: 200\n\nbody"
        );
    }

    #[test]
    fn header_is_outside_the_max_length_window() {
        // The window is applied to the body first; the header is prepended after, so it never eats the window
        // and never counts against max_length (FR-14).
        let body = win(0, 4, &["abcdefgh"]);
        assert_eq!(body, "abcd");
        let out = with_header(&fetched(1), body);
        assert_eq!(out, "URL: https://b.example/final\nStatus: 200\n\nabcd");
        assert!(out.chars().count() > 4);
        assert!(out.ends_with("\n\nabcd"));
    }

    fn win(start: u64, len: u64, parts: &[&str]) -> String {
        let mut w = Window::new(start, len);
        for p in parts {
            w.push(p);
        }
        w.into_text()
    }

    #[test]
    fn window_counts_characters_across_chunk_boundaries() {
        assert_eq!(win(0, 5, &["hello world"]), "hello");
        assert_eq!(win(2, 5, &["he", "llo w", "orld"]), "llo w");
        assert_eq!(win(1, 3, &["h\u{e9}", "llo"]), "\u{e9}ll");
        assert_eq!(win(0, 100, &["short"]), "short");
        assert_eq!(win(50, 5, &["short"]), "");
        assert_eq!(win(0, 0, &["anything"]), "");
    }
}
