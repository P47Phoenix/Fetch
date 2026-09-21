//! rmcp stdio handler: registers the `fetch` tool, validates its input (FR-01, FR-02) and runs it through the
//! guarded [`FetchClient`] (A-3b). Tool futures must be `Send` (A-1 spike). Every failure is an `isError` result
//! `error[<code>]: <message>` naming the field or cause, the same shape rmcp 3.4 uses for schema-deserialisation
//! failures (ADR-006 amendment 2026-09-19). On success the text is returned as fetched: no label, wrapper or
//! notice is added (OQ-5 decided NO on 2026-09-20, ADR-006 note).
//!
//! Pagination (A-5, ADR-006): the body is decoded as UTF-8, HTML is converted to markdown as it streams
//! (A-4; `raw=true` skips the conversion) and the character window (`start_index`, `max_length`) is kept as
//! the text goes by. Reading stops as soon as the window is complete and one more character has been seen
//! (early stop), so the result can say "more content" without reading on. Continuation and clamp notes are a
//! footer outside the offsets; the total length is stated only when the stream was read to its end.

use crate::config::Config;
use crate::convert::window::{Window, WindowOutput};
use crate::convert::Mode;
use crate::error::FetchError;
use crate::fetch::dns::SystemResolver;
use crate::fetch::{FetchClient, Fetched, Limits};
use crate::policy::Policy;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router, ErrorData as McpError,
};
use serde::{Deserialize, Deserializer};
use std::sync::atomic::Ordering;
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

/// The tool text for a finished window (ADR-006 items 3 to 5): the kept characters, then a footer that is outside
/// the offsets. `clamped_to` is set when `max_length` was reduced to the cap.
///
/// * more content exists: the `start_index` that continues;
/// * the end was reached on a continuation page (`start_index` > 0): the total length;
/// * `start_index` at or beyond the end: an empty-content message with the total length;
/// * a first page that holds everything carries no footer, so a short page is returned exactly as fetched.
#[must_use]
pub fn render(w: &WindowOutput, clamped_to: Option<u64>) -> String {
    let mut notes: Vec<String> = Vec::new();
    if let Some(cap) = clamped_to {
        notes.push(format!(
            "[max_length was reduced to {cap} characters, the maximum.]"
        ));
    }
    if let Some(total) = w.total.filter(|t| w.start >= *t) {
        notes.insert(
            0,
            format!(
                "[No content at start_index={}: the content is {total} characters long.]",
                w.start
            ),
        );
        return notes.join("\n");
    }
    if w.more {
        notes.push(format!(
            "[More content available. Call fetch again with start_index={} to continue.]",
            w.start.saturating_add(w.returned)
        ));
    } else if let (true, Some(total)) = (w.start > 0, w.total) {
        notes.push(format!("[Total length: {total} characters.]"));
    }
    if notes.is_empty() {
        return w.text.clone();
    }
    format!("{}\n\n{}", w.text, notes.join("\n"))
}

#[tool_router(server_handler)]
impl Fetch {
    #[tool(
        description = "Fetch a URL and return its content. HTML pages are converted to markdown (scripts, styles and navigation dropped); other text is returned as is. Set raw=true for the unconverted body. Output is limited to max_length characters from start_index; when truncated, the result ends with the start_index to continue from."
    )]
    async fn fetch(
        &self,
        Parameters(p): Parameters<FetchParams>,
    ) -> Result<CallToolResult, McpError> {
        if p.max_length == Some(0) {
            let e = FetchError::InvalidArgument {
                field: "max_length",
                message: "must be at least 1".into(),
            };
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                e.tool_text(),
            )]));
        }
        let asked = p.max_length.unwrap_or(DEFAULT_MAX_LENGTH);
        let max_length = asked.min(self.max_length_cap);
        let clamped_to = (asked > max_length).then_some(max_length);
        let mut window = Window::new(p.start_index.unwrap_or(0), max_length);
        let done = window.stop_flag();
        let mode = if p.raw.unwrap_or(false) {
            Mode::Raw
        } else {
            Mode::Markdown
        };
        let result = self
            .client
            .fetch_until(
                &p.url,
                mode,
                &mut |text: &str| window.push(text),
                &move || done.load(Ordering::Relaxed),
            )
            .await;
        Ok(match result {
            Ok(f) => CallToolResult::success(vec![ContentBlock::text(with_header(
                &f,
                render(&window.finish(), clamped_to),
            ))]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.tool_text())]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{render, with_header};
    use crate::convert::window::{Window, WindowOutput};
    use crate::fetch::Fetched;

    fn fetched(redirects: usize) -> Fetched {
        Fetched {
            final_url: "https://b.example/final".into(),
            status: 200,
            redirects,
            wire_bytes: 4,
        }
    }

    fn win(start: u64, len: u64, parts: &[&str]) -> WindowOutput {
        let mut w = Window::new(start, len);
        for p in parts {
            w.push(p);
        }
        w.finish()
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
        let body = win(0, 4, &["abcdefgh"]).text;
        assert_eq!(body, "abcd");
        let out = with_header(&fetched(1), body);
        assert_eq!(out, "URL: https://b.example/final\nStatus: 200\n\nabcd");
        assert!(out.chars().count() > 4);
        assert!(out.ends_with("\n\nabcd"));
    }

    #[test]
    fn a_page_that_fits_is_returned_exactly_as_fetched() {
        assert_eq!(render(&win(0, 100, &["short page"]), None), "short page");
        assert_eq!(render(&win(0, 10, &["exactly 10"]), None), "exactly 10");
    }

    #[test]
    fn a_truncated_page_states_the_next_start_index() {
        let out = render(&win(0, 5, &["hello world"]), None);
        assert_eq!(
            out,
            "hello\n\n[More content available. Call fetch again with start_index=5 to continue.]"
        );
        let out = render(&win(3, 4, &["h\u{e9}llo w\u{e9}rld"]), None);
        assert!(out.starts_with("lo w\n\n"), "{out}");
        assert!(out.contains("start_index=7 "), "{out}");
    }

    #[test]
    fn the_last_page_of_a_continuation_states_the_total_length() {
        let out = render(&win(6, 50, &["hello world"]), None);
        assert_eq!(out, "world\n\n[Total length: 11 characters.]");
    }

    #[test]
    fn start_index_at_or_past_the_end_is_an_empty_content_message() {
        for start in [11, 12, 1_000_000] {
            assert_eq!(
                render(&win(start, 5, &["hello world"]), None),
                format!("[No content at start_index={start}: the content is 11 characters long.]")
            );
        }
        assert_eq!(
            render(&win(0, 5, &[""]), None),
            "[No content at start_index=0: the content is 0 characters long.]"
        );
    }

    #[test]
    fn a_clamp_is_stated() {
        let out = render(&win(0, 5, &["hello world"]), Some(5));
        assert_eq!(
            out,
            "hello\n\n[max_length was reduced to 5 characters, the maximum.]\n\
             [More content available. Call fetch again with start_index=5 to continue.]"
        );
        assert_eq!(
            render(&win(0, 50, &["hi"]), Some(50)),
            "hi\n\n[max_length was reduced to 50 characters, the maximum.]"
        );
        assert_eq!(
            render(&win(9, 5, &["hi"]), Some(5)),
            "[No content at start_index=9: the content is 2 characters long.]\n\
             [max_length was reduced to 5 characters, the maximum.]"
        );
    }
}
