//! Body converters (A-4; architecture 5.1, ADR-002). A [`Converter`] turns the decoded UTF-8 text stream of a
//! response into the text the tool returns, one bounded step at a time. Nothing accumulates the page: the
//! HTML converter is a streaming tokenizer plus a small state machine ([`markdown`]), so memory does not depend
//! on the page size. The trait is the seam that lets the converter be swapped (ADR-002, A-4 AC).

pub mod markdown;
pub(crate) mod tagscan;
pub mod window;

use reqwest::Url;

/// Why a converter stopped. Both end the call with an error result that suggests `raw=true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    /// The rewriter's memory limit or the output guard was hit (`converter_limit`).
    Limit,
    /// The HTML could not be processed.
    Malformed,
}

/// A streaming text converter. Output is a pure function of the byte stream, independent of how it is chunked.
pub trait Converter: Send {
    /// Convert the next slice of the stream, passing zero or more output slices to `out`.
    ///
    /// # Errors
    /// [`ConvertError`] when the converter cannot continue (it then ignores further input).
    fn push(&mut self, text: &str, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError>;
    /// End of stream: flush what is still held.
    ///
    /// # Errors
    /// [`ConvertError`] when the stream ended in a state the converter cannot finish.
    fn finish(&mut self, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError>;
}

/// The text is returned as it arrived (raw mode and non-HTML content).
#[derive(Debug, Default)]
pub struct Passthrough;

impl Converter for Passthrough {
    fn push(&mut self, text: &str, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        out(text);
        Ok(())
    }
    fn finish(&mut self, _out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        Ok(())
    }
}

/// What the caller asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Return the body text unchanged.
    Raw,
    /// Convert HTML to markdown; anything else is returned as text.
    Markdown,
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Html,
    Text,
    Unknown,
}

fn classify(content_type: Option<&str>) -> Kind {
    let Some(ct) = content_type else {
        return Kind::Unknown;
    };
    let essence = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "" => Kind::Unknown,
        "text/html" | "application/xhtml+xml" => Kind::Html,
        _ => Kind::Text,
    }
}

/// The converter for a response: `text/html` and `application/xhtml+xml` are converted, a missing
/// `Content-Type` is sniffed (HTML markers at the start), everything else is passed through (content-type
/// rejection and the full sniffing rules are A-6).
#[must_use]
pub fn for_response(
    mode: Mode,
    content_type: Option<&str>,
    base: Option<Url>,
) -> Box<dyn Converter> {
    if mode == Mode::Raw {
        return Box::new(Passthrough);
    }
    match classify(content_type) {
        Kind::Html => Box::new(markdown::MarkdownConverter::new(base)),
        Kind::Text => Box::new(Passthrough),
        Kind::Unknown => Box::new(Sniff::new(base)),
    }
}

/// How much of the start of an untyped body is held to decide between HTML and text.
const SNIFF_BYTES: usize = 512;

/// Holds at most [`SNIFF_BYTES`] of an untyped body, then commits to the HTML converter or to passthrough.
struct Sniff {
    base: Option<Url>,
    held: String,
    chosen: Option<Box<dyn Converter>>,
}

impl Sniff {
    fn new(base: Option<Url>) -> Self {
        Self {
            base,
            held: String::new(),
            chosen: None,
        }
    }

    fn looks_like_html(head: &str) -> bool {
        let lower: String = head.chars().take(16).flat_map(char::to_lowercase).collect();
        ["<!doctype html", "<html", "<head", "<body"]
            .iter()
            .any(|m| lower.starts_with(m))
    }

    fn commit(&mut self, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        let mut c: Box<dyn Converter> = if Self::looks_like_html(self.held.trim_start()) {
            Box::new(markdown::MarkdownConverter::new(self.base.take()))
        } else {
            Box::new(Passthrough)
        };
        let held = std::mem::take(&mut self.held);
        let r = c.push(&held, out);
        self.chosen = Some(c);
        r
    }
}

impl Converter for Sniff {
    fn push(&mut self, text: &str, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        if let Some(c) = self.chosen.as_mut() {
            return c.push(text, out);
        }
        self.held.push_str(text);
        let head = self.held.trim_start();
        if self.held.len() >= SNIFF_BYTES || (!head.is_empty() && !head.starts_with('<')) {
            return self.commit(out);
        }
        Ok(())
    }
    fn finish(&mut self, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        if self.chosen.is_none() {
            self.commit(out)?;
        }
        match self.chosen.as_mut() {
            Some(c) => c.finish(out),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, for_response, Kind, Mode};

    fn run(mode: Mode, ct: Option<&str>, parts: &[&str]) -> String {
        let mut c = for_response(mode, ct, None);
        let mut out = String::new();
        for p in parts {
            c.push(p, &mut |s| out.push_str(s)).unwrap();
        }
        c.finish(&mut |s| out.push_str(s)).unwrap();
        out
    }

    #[test]
    fn content_type_classes() {
        assert_eq!(classify(Some("text/html; charset=utf-8")), Kind::Html);
        assert_eq!(classify(Some("TEXT/HTML")), Kind::Html);
        assert_eq!(classify(Some("application/xhtml+xml")), Kind::Html);
        assert_eq!(classify(Some("text/plain")), Kind::Text);
        assert_eq!(classify(Some("application/json")), Kind::Text);
        assert_eq!(classify(None), Kind::Unknown);
        assert_eq!(classify(Some("")), Kind::Unknown);
    }

    #[test]
    fn non_html_and_raw_are_returned_unchanged() {
        let html = "<h1>Hi</h1>";
        assert_eq!(run(Mode::Markdown, Some("text/plain"), &[html]), html);
        assert_eq!(
            run(Mode::Markdown, Some("application/json"), &["{\"a\":1}"]),
            "{\"a\":1}"
        );
        assert_eq!(run(Mode::Raw, Some("text/html"), &[html]), html);
    }

    #[test]
    fn html_is_converted() {
        assert_eq!(
            run(Mode::Markdown, Some("text/html"), &["<h1>Hi</h1>"]),
            "# Hi"
        );
    }

    #[test]
    fn missing_content_type_is_sniffed_across_chunks() {
        assert_eq!(
            run(Mode::Markdown, None, &["  <!DOC", "TYPE html><h1>A</h1>"]),
            "# A"
        );
        assert_eq!(run(Mode::Markdown, None, &["<h1>", "x</h1>"]), "<h1>x</h1>");
        assert_eq!(
            run(Mode::Markdown, None, &["plain <b>text</b>"]),
            "plain <b>text</b>"
        );
        assert_eq!(
            run(
                Mode::Markdown,
                None,
                &["<html><body><p>a</p></body></html>"]
            ),
            "a"
        );
        assert_eq!(run(Mode::Markdown, None, &[""]), "");
    }
}
