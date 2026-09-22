//! Charset detection and decoding (A-8; architecture 14.1). Priority order: (a) the `Content-Type` response
//! header's `charset=` parameter, if `encoding_rs` recognizes the label; (b) for an HTML response, a
//! `<meta charset="...">` or `<meta http-equiv="Content-Type" content="...charset=...">` tag found within the
//! first ~1024 bytes of the (decompressed) body -- the standard HTML5 sniffing window; (c) UTF-8. An unrecognized
//! label at either step falls back to the next step rather than failing the fetch. Meta sniffing scans raw
//! ASCII bytes (never decoded text): every charset used in practice for HTML is ASCII-compatible in this
//! prefix, so this is safe to do before the body's own encoding is known.
//!
//! Pure logic, no I/O: `crate::fetch::mod` wires the header and the body-prefix bytes in.

pub use encoding_rs::Encoding;
use encoding_rs::UTF_8;

/// The `charset=` parameter of a `Content-Type` header value, if present and recognized by `encoding_rs`.
/// `None` when there is no charset parameter, or its label is not one `encoding_rs` knows.
#[must_use]
pub fn from_content_type(content_type: &str) -> Option<&'static Encoding> {
    let (_, params) = content_type.split_once(';')?;
    for param in params.split(';') {
        let param = param.trim();
        let lower = param.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("charset=") else {
            continue;
        };
        let value = param[param.len() - rest.len()..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Encoding::for_label(value.as_bytes());
    }
    None
}

/// Whether a `Content-Type` header value names `text/html` (ignoring parameters), the only type the meta
/// sniffing window applies to.
#[must_use]
pub fn is_html(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|ct| {
        ct.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("text/html")
    })
}

/// Sniff a `<meta charset>` / `<meta http-equiv="Content-Type" ...>` declaration from `window`, conventionally
/// the first ~1024 bytes of the (decompressed) body. `None` when no such tag is found, or its charset label is
/// not recognized. Byte-level and ASCII-only by design (see the module docs).
#[must_use]
pub fn sniff_meta(window: &[u8]) -> Option<&'static Encoding> {
    // Lossy is fine: only the ASCII byte range is ever matched below, and a non-ASCII byte simply cannot form
    // part of a tag name or attribute name/value we look for.
    let text = String::from_utf8_lossy(window);
    let lower = text.to_ascii_lowercase();
    let mut idx = 0;
    while let Some(pos) = lower[idx..].find("<meta") {
        let start = idx + pos;
        let Some(end_rel) = lower[start..].find('>') else {
            break;
        };
        let tag = &lower[start..start + end_rel];
        if let Some(enc) = charset_attr(tag) {
            return Some(enc);
        }
        if let Some(enc) = http_equiv_charset(tag) {
            return Some(enc);
        }
        idx = start + end_rel + 1;
    }
    None
}

/// The value of attribute `name="..."` (or `name='...'` or unquoted) in a lower-cased tag body. Naive but
/// sufficient for the ASCII-only, well-formed `<meta>` tags this sniffing targets.
fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=");
    let pos = tag.find(&pat)?;
    let rest = tag[pos + pat.len()..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        Some(&r[..r.find('"')?])
    } else if let Some(r) = rest.strip_prefix('\'') {
        Some(&r[..r.find('\'')?])
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

/// `<meta charset="...">`. Rejects a spurious match inside another attribute's value (e.g. the `content=`
/// attribute of an `http-equiv` tag) because that value is quoted from an earlier position in `tag`, so the
/// naive scan above picks up trailing quote noise that `Encoding::for_label` will not recognize; the real
/// `charset` attribute is unambiguous.
fn charset_attr(tag: &str) -> Option<&'static Encoding> {
    let v = attr_value(tag, "charset")?.trim();
    Encoding::for_label(v.as_bytes())
}

/// `<meta http-equiv="Content-Type" content="...;charset=...">`.
fn http_equiv_charset(tag: &str) -> Option<&'static Encoding> {
    let equiv = attr_value(tag, "http-equiv")?;
    if !equiv.trim().eq_ignore_ascii_case("content-type") {
        return None;
    }
    let content = attr_value(tag, "content")?;
    let pos = content.find("charset=")?;
    let v = content[pos + "charset=".len()..].trim();
    let end = v
        .find(|c: char| c == ';' || c.is_whitespace())
        .unwrap_or(v.len());
    let v = v[..end].trim_matches('"').trim_matches('\'');
    Encoding::for_label(v.as_bytes())
}

/// Decode `bytes` (the full, capped body) as `encoding`, producing valid UTF-8 text. A BOM in `bytes` overrides
/// `encoding` (per the Encoding Standard) and invalid sequences become U+FFFD -- both handled by `encoding_rs`.
#[must_use]
pub fn decode(encoding: &'static Encoding, bytes: &[u8]) -> String {
    encoding.decode(bytes).0.into_owned()
}

/// [`UTF_8`], re-exported for callers comparing a detected encoding against the default.
#[must_use]
pub fn utf8() -> &'static Encoding {
    UTF_8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_charset_is_recognized_case_insensitively_and_quoted() {
        for ct in [
            "text/html; charset=iso-8859-1",
            "text/html;charset=ISO-8859-1",
            "text/html; charset=\"ISO-8859-1\"",
            "text/html; boundary=x; charset=iso-8859-1",
        ] {
            assert_eq!(
                from_content_type(ct),
                Some(encoding_rs::WINDOWS_1252),
                "{ct}"
            );
        }
        assert_eq!(
            from_content_type("text/plain; charset=utf-8"),
            Some(encoding_rs::UTF_8)
        );
    }

    #[test]
    fn header_without_or_with_unrecognized_charset_is_none() {
        assert_eq!(from_content_type("text/html"), None);
        assert_eq!(
            from_content_type("text/html; charset=bogus-not-a-charset"),
            None
        );
        assert_eq!(from_content_type("text/html; charset="), None);
    }

    #[test]
    fn is_html_matches_only_text_html_ignoring_parameters() {
        assert!(is_html(Some("text/html; charset=utf-8")));
        assert!(is_html(Some("Text/HTML")));
        assert!(!is_html(Some("text/plain")));
        assert!(!is_html(None));
    }

    #[test]
    fn meta_charset_attribute_is_sniffed() {
        let html = b"<html><head><meta charset=\"ISO-8859-1\"><title>t</title></head></html>";
        assert_eq!(sniff_meta(html), Some(encoding_rs::WINDOWS_1252));
        let html = b"<meta charset=iso-8859-1>";
        assert_eq!(sniff_meta(html), Some(encoding_rs::WINDOWS_1252));
    }

    #[test]
    fn meta_http_equiv_content_type_charset_is_sniffed() {
        let html = b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=ISO-8859-1\">";
        assert_eq!(sniff_meta(html), Some(encoding_rs::WINDOWS_1252));
        // attribute order does not matter
        let html = b"<meta content=\"text/html; charset=ISO-8859-1\" http-equiv=\"Content-Type\">";
        assert_eq!(sniff_meta(html), Some(encoding_rs::WINDOWS_1252));
    }

    #[test]
    fn no_meta_tag_or_unrecognized_charset_is_none() {
        assert_eq!(
            sniff_meta(b"<html><head><title>t</title></head></html>"),
            None
        );
        assert_eq!(sniff_meta(b"<meta charset=\"totally-bogus\">"), None);
        assert_eq!(sniff_meta(b""), None);
    }

    #[test]
    fn decode_handles_bom_and_replaces_invalid_bytes() {
        assert_eq!(decode(encoding_rs::WINDOWS_1252, b"caf\xe9"), "café");
        // a UTF-8 BOM overrides the given encoding, per the Encoding Standard
        assert_eq!(decode(encoding_rs::WINDOWS_1252, b"\xef\xbb\xbfhi"), "hi");
        assert_eq!(decode(UTF_8, b"a\xffb"), "a\u{FFFD}b");
    }
}
