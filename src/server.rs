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
//! footer outside the offsets; the total length is stated only when the stream was read to its end. Content
//! types other than text are refused (A-6) before the body is read.

use crate::config::Config;
use crate::convert::window::{Window, WindowOutput};
use crate::convert::Mode;
use crate::error::FetchError;
use crate::fetch::dns::SystemResolver;
use crate::fetch::{FetchClient, Fetched, Limits};
use crate::policy::Policy;
use rmcp::serde_json::Value as Json;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router, ErrorData as McpError,
};
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Removes the `default: null` that `#[serde(default)]` would add to the schema (the field stays required).
fn drop_default(schema: &mut schemars::Schema) {
    schema.remove("default");
}

/// The tool arguments. The fields are captured as JSON and validated by [`FetchParams::parse`] so that every
/// argument failure in a JSON object (missing, wrong type, out of range) is the same
/// `error[invalid_argument]: <field>: <why>` result as our other checks (A-7; rmcp's own prefix
/// `failed to deserialize parameters:` no longer appears for these). Arguments that are not a JSON object are still
/// rejected by rmcp before this code runs (pinned in `tests/stdio.rs`). The advertised schema shows the real types.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(extend("required" = ["url"]))]
pub struct FetchParams {
    /// URL to fetch (http or https only)
    #[serde(default)]
    #[schemars(with = "String", transform = drop_default)]
    pub url: Json,
    /// Maximum number of characters to return (default 5000)
    #[serde(default)]
    #[schemars(with = "Option<u64>")]
    pub max_length: Json,
    /// Character offset to start from (default 0)
    #[serde(default)]
    #[schemars(with = "Option<u64>")]
    pub start_index: Json,
    /// Return the raw body without HTML simplification
    #[serde(default)]
    #[schemars(with = "Option<bool>")]
    pub raw: Json,
}

/// Validated arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub url: String,
    pub max_length: Option<u64>,
    pub start_index: Option<u64>,
    pub raw: bool,
}

fn bad(field: &'static str, message: &str) -> FetchError {
    FetchError::InvalidArgument {
        field,
        message: message.into(),
    }
}

fn optional_u64(field: &'static str, v: &Json) -> Result<Option<u64>, FetchError> {
    match v {
        Json::Null => Ok(None),
        Json::Number(n) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| bad(field, "must be a non-negative whole number")),
        _ => Err(bad(field, "must be a non-negative whole number")),
    }
}

impl FetchParams {
    /// # Errors
    /// `InvalidArgument` naming the first offending field.
    pub fn parse(&self) -> Result<Args, FetchError> {
        let url = match &self.url {
            Json::String(u) => u.clone(),
            Json::Null => return Err(bad("url", "is required")),
            _ => return Err(bad("url", "must be a string")),
        };
        let max_length = optional_u64("max_length", &self.max_length)?;
        if max_length == Some(0) {
            return Err(bad("max_length", "must be at least 1"));
        }
        let raw = match &self.raw {
            Json::Null => false,
            Json::Bool(b) => *b,
            _ => return Err(bad("raw", "must be true or false")),
        };
        Ok(Args {
            url,
            max_length,
            start_index: optional_u64("start_index", &self.start_index)?,
            raw,
        })
    }
}

/// The `isError` result for a failure (A-7). An unexpected internal failure gets the generic text and its detail goes
/// to stderr only, so nothing but JSON-RPC ever reaches stdout and the server keeps serving.
#[must_use]
pub fn error_result(e: &FetchError) -> CallToolResult {
    if let FetchError::Internal(detail) = e {
        crate::obs::stderr_line(format_args!("error internal {detail}"));
    }
    CallToolResult::error(vec![ContentBlock::text(e.tool_text())])
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
            client: Arc::new(
                FetchClient::new(policy, Arc::new(SystemResolver), Limits::from_config(cfg))
                    .with_robots_mode(cfg.robots_txt),
            ),
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
        description = "Fetch a URL and return its content. HTML pages are converted to markdown (scripts, styles and navigation dropped); other text, JSON and XML are returned as is; images and other binary types are refused. Set raw=true for the unconverted body. Output is limited to max_length characters from start_index; when truncated, the result ends with the start_index to continue from."
    )]
    async fn fetch(
        &self,
        Parameters(p): Parameters<FetchParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = match p.parse() {
            Ok(p) => p,
            Err(e) => return Ok(error_result(&e)),
        };
        let asked = p.max_length.unwrap_or(DEFAULT_MAX_LENGTH);
        let max_length = asked.min(self.max_length_cap);
        // The note is for a caller who asked for more than the cap; a default that exceeds a configured cap is not the caller's request.
        let clamped_to = p.max_length.filter(|m| *m > max_length).map(|_| max_length);
        let mut window = Window::new(p.start_index.unwrap_or(0), max_length);
        let done = window.stop_flag();
        let mode = if p.raw { Mode::Raw } else { Mode::Markdown };
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
            Err(e) => error_result(&e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{error_result, render, with_header, FetchParams};
    use crate::convert::window::{Window, WindowOutput};
    use crate::error::{FetchError, INTERNAL_MESSAGE};
    use crate::fetch::Fetched;
    use rmcp::serde_json::json;

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

    fn params(v: rmcp::serde_json::Value) -> FetchParams {
        rmcp::serde_json::from_value(v).expect("any JSON object deserialises")
    }

    #[test]
    fn every_argument_failure_is_an_invalid_argument_result_naming_the_field() {
        for (args, field) in [
            (json!({}), "url"),
            (json!({"url": 5}), "url"),
            (
                json!({"url": "https://a.test", "max_length": -1}),
                "max_length",
            ),
            (
                json!({"url": "https://a.test", "max_length": 1.5}),
                "max_length",
            ),
            (
                json!({"url": "https://a.test", "max_length": 0}),
                "max_length",
            ),
            (
                json!({"url": "https://a.test", "max_length": "9"}),
                "max_length",
            ),
            (
                json!({"url": "https://a.test", "start_index": -3}),
                "start_index",
            ),
            (
                json!({"url": "https://a.test", "start_index": "7"}),
                "start_index",
            ),
            (json!({"url": "https://a.test", "raw": "yes"}), "raw"),
        ] {
            let e = params(args.clone()).parse().unwrap_err();
            let r = error_result(&e);
            assert_eq!(r.is_error, Some(true), "{args}");
            let text = format!("{:?}", r.content);
            assert!(
                text.contains(&format!("error[invalid_argument]: {field}: ")),
                "{args}: {text}"
            );
        }
        let ok = params(
            json!({"url": "https://a.test", "max_length": 7, "start_index": 2, "raw": true}),
        )
        .parse()
        .unwrap();
        assert_eq!(
            (ok.max_length, ok.start_index, ok.raw),
            (Some(7), Some(2), true)
        );
        assert!(
            !params(json!({"url": "https://a.test", "raw": null}))
                .parse()
                .unwrap()
                .raw
        );
    }

    /// A-7: every cause gives `isError: true` and text naming that cause.
    #[test]
    fn every_cause_sets_the_flag_and_names_the_cause() {
        let causes: Vec<(FetchError, &str)> = vec![
            (FetchError::HttpStatus(404), "error[http_error]: the server refused the request with HTTP status 404 (Not Found)"),
            (FetchError::HttpStatus(500), "error[http_error]: the server failed with HTTP status 500 (Internal Server Error)"),
            (FetchError::HttpStatus(429), "error[http_error]: the server refused the request with HTTP status 429 (Too Many Requests); it may work if retried after a delay"),
            (FetchError::DnsFailure("hostname did not resolve".into()), "error[dns_failure]: hostname did not resolve"),
            (FetchError::Timeout("the request timed out".into()), "error[timeout]: the request timed out"),
            (FetchError::BlockedTarget("host is not public".into()), "error[blocked_target]: host is not public"),
            (FetchError::TooLarge("the response is larger than the size limit".into()), "error[too_large]: the response is larger"),
            (FetchError::UnsupportedContentType("image/png".into()), "error[unsupported_content_type]: image/png"),
            (FetchError::UnsupportedEncoding("Content-Encoding must be gzip or absent".into()), "error[unsupported_encoding]: "),
            (FetchError::TooManyRedirects, "error[too_many_redirects]: too many redirects"),
            (FetchError::Network("could not connect to the host".into()), "error[network_error]: could not connect"),
            (FetchError::BadResponse("the gzip body is corrupt".into()), "error[bad_response]: the gzip body"),
            (FetchError::ConverterLimit("x".into()), "error[converter_limit]: x"),
        ];
        for (e, want) in causes {
            let r = error_result(&e);
            assert_eq!(r.is_error, Some(true), "{e:?}");
            let text = format!("{:?}", r.content);
            assert!(text.contains(want), "{e:?}: {text}");
        }
    }

    /// A-7: an unexpected internal error is a generic error result; its detail is not shown to the caller.
    #[test]
    fn an_internal_error_is_generic_and_leaks_no_detail() {
        let r = error_result(&FetchError::Internal("secret detail 10.0.0.1".into()));
        assert_eq!(r.is_error, Some(true));
        let text = format!("{:?}", r.content);
        assert!(
            text.contains(&format!("error[internal]: {INTERNAL_MESSAGE}")),
            "{text}"
        );
        assert!(
            !text.contains("secret") && !text.contains("10.0.0.1"),
            "{text}"
        );
    }
}
