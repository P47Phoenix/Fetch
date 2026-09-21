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

/// What a `Content-Type` means for the tool (A-6, FR-08).
#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    /// `text/html` or `application/xhtml+xml`: converted to markdown.
    Html,
    /// `text/*`, JSON, XML and a few other textual types: returned as text.
    Text,
    /// No usable `Content-Type`: sniffed from the body.
    Unknown,
    /// Anything else (images, PDF, archives, `application/octet-stream`, malformed types). Holds the media type,
    /// cleaned for display, because the header is upstream-controlled text.
    Unsupported(String),
}

/// Textual media types besides `text/*`, JSON and XML (and their `+json` / `+xml` suffix types).
const TEXTUAL: [&str; 6] = [
    "application/json",
    "application/xml",
    "application/javascript",
    "application/x-javascript",
    "application/ecmascript",
    "application/x-ndjson",
];

/// The media type for display: printable ASCII only, at most 100 characters. It is echoed into an error, and the
/// header is written by the server we are fetching from.
fn display_type(essence: &str) -> String {
    essence
        .chars()
        .take(100)
        .map(|c| if c.is_ascii_graphic() { c } else { '?' })
        .collect()
}

#[must_use]
pub fn classify(content_type: Option<&str>) -> Kind {
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
        e if e.starts_with("text/")
            || TEXTUAL.contains(&e)
            || e.ends_with("+json")
            || e.ends_with("+xml") =>
        {
            Kind::Text
        }
        e => Kind::Unsupported(display_type(e)),
    }
}

/// The response is a media type the tool does not return (A-6). It is refused before any body byte is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedType(pub String);

/// The converter for a response: `text/html` and `application/xhtml+xml` are converted, other text types are
/// passed through, a missing `Content-Type` is sniffed (HTML markers at the start of the body, otherwise text) and
/// every other type is refused, `raw` or not.
///
/// # Errors
/// [`UnsupportedType`] naming the media type when it is not text.
pub fn for_response(
    mode: Mode,
    content_type: Option<&str>,
    base: Option<Url>,
) -> Result<Box<dyn Converter>, UnsupportedType> {
    let kind = classify(content_type);
    if let Kind::Unsupported(t) = kind {
        return Err(UnsupportedType(t));
    }
    if mode == Mode::Raw {
        return Ok(Box::new(Passthrough));
    }
    Ok(match kind {
        Kind::Html => Box::new(markdown::MarkdownConverter::new(base)),
        Kind::Text | Kind::Unsupported(_) => Box::new(Passthrough),
        Kind::Unknown => Box::new(Sniff::new(base)),
    })
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

    /// The body starts (after whitespace and a byte-order mark) with `<!doctype html`, `<html`, `<head` or `<body`,
    /// case-insensitively, the tag name ending there (`<htmlx` is not `<html`).
    fn looks_like_html(head: &str) -> bool {
        let head = head.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        let lower: String = head.chars().take(16).flat_map(char::to_lowercase).collect();
        if let Some(rest) = lower.strip_prefix("<!doctype html") {
            return rest
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '>');
        }
        ["<html", "<head", "<body"].iter().any(|m| {
            lower.strip_prefix(m).is_some_and(|rest| {
                rest.chars()
                    .next()
                    .is_none_or(|c| c.is_whitespace() || c == '>' || c == '/')
            })
        })
    }

    fn commit(&mut self, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        let held = std::mem::take(&mut self.held);
        let (mut c, held): (Box<dyn Converter>, &str) = if Self::looks_like_html(&held) {
            // A byte-order mark is not page text.
            let html = held.strip_prefix('\u{feff}').unwrap_or(&held);
            (
                Box::new(markdown::MarkdownConverter::new(self.base.take())),
                html,
            )
        } else {
            (Box::new(Passthrough), &held)
        };
        let r = c.push(held, out);
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
        let head = self
            .held
            .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
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
    use super::{classify, for_response, Kind, Mode, UnsupportedType};

    fn run(mode: Mode, ct: Option<&str>, parts: &[&str]) -> String {
        let mut c = for_response(mode, ct, None).expect("a supported type");
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
        for t in [
            "text/xml",
            "application/xml; charset=utf-8",
            "application/ld+json",
            "application/rss+xml",
            "application/javascript",
        ] {
            assert_eq!(classify(Some(t)), Kind::Text, "{t}");
        }
    }

    #[test]
    fn binary_and_unknown_types_are_refused_with_the_type_named_raw_or_not() {
        for t in [
            "image/png",
            "application/pdf",
            "application/octet-stream",
            "video/mp4",
            "IMAGE/PNG; foo=bar",
            "garbage",
        ] {
            let want = t.split(';').next().unwrap().trim().to_ascii_lowercase();
            for mode in [Mode::Markdown, Mode::Raw] {
                let e = for_response(mode, Some(t), None).err();
                assert_eq!(e, Some(UnsupportedType(want.clone())), "{t} {mode:?}");
            }
        }
    }

    #[test]
    fn a_hostile_content_type_is_cleaned_before_it_can_be_echoed() {
        let e = for_response(Mode::Markdown, Some("x/\r\n\u{1b}[31m\u{e9}y"), None).err();
        assert_eq!(e, Some(UnsupportedType("x/???[31m?y".into())));
        let long = format!("a/{}", "b".repeat(5000));
        assert_eq!(classify(Some(&long)), Kind::Unsupported(long[..100].into()));
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

    #[test]
    fn sniffing_needs_a_real_html_marker_at_the_start() {
        let html = |b: &str| run(Mode::Markdown, None, &[b]);
        assert_eq!(
            html("\u{feff}\n <HTML><body><h1>A</h1></body></html>"),
            "# A"
        );
        assert_eq!(
            html("<head><title>t</title></head><body><p>x</p></body>"),
            "x"
        );
        assert_eq!(html("<body><p>x</p></body>"), "x");
        assert_eq!(html("<!DOCTYPE html>"), "");
        // Not markers: a tag that only starts with one, or a marker that is not at the start.
        assert_eq!(html("<htmlx><p>x</p></htmlx>"), "<htmlx><p>x</p></htmlx>");
        assert_eq!(
            html("<!doctype htmlfoo><p>x</p>"),
            "<!doctype htmlfoo><p>x</p>"
        );
        assert_eq!(
            html("intro <html><p>x</p></html>"),
            "intro <html><p>x</p></html>"
        );
        // Binary-looking bytes without a type are text, decoded with replacement characters upstream.
        assert_eq!(html("\u{fffd}PNG\u{1a}"), "\u{fffd}PNG\u{1a}");
        // A long untyped body that starts as HTML is still decided (512 held bytes at most).
        let long = format!("<html><body><p>{}</p></body></html>", "w".repeat(2000));
        assert_eq!(html(&long), "w".repeat(2000));
    }
}
