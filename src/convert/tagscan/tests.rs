use super::{Scan, ATTR_CAP};
use lol_html::element;
use lol_html::send::{Element, HtmlRewriter, Settings};
use std::sync::{Arc, Mutex};

fn peak_of(chunks: &[&[u8]]) -> (bool, u32) {
    let mut s = Scan::new();
    let mut ok = true;
    for c in chunks {
        ok &= s.feed(c);
    }
    (ok, s.peak())
}

/// The most attributes `lol_html` itself reports on any element of `html`.
fn lexer_max(html: &[u8]) -> usize {
    let max = Arc::new(Mutex::new(0usize));
    let m = Arc::clone(&max);
    let mut rw = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("*", move |el: &mut Element<'_, '_>| {
                let n = el.attributes().len();
                let mut g = m.lock().unwrap();
                *g = (*g).max(n);
                Ok(())
            })],
            ..Settings::new_send()
        },
        |_: &[u8]| {},
    );
    let _ = rw.write(html);
    let _ = rw.end();
    let v = *max.lock().unwrap();
    v
}

#[test]
fn counts_the_attribute_shapes_of_the_lexer() {
    let cases: &[(&str, u32)] = &[
        ("<div>", 0),
        ("<div a>", 1),
        ("<div a b c>", 3),
        ("<div a=1 b='2' c=\"3\">", 3),
        ("<div a=\"1\"b=\"2\">", 2),
        ("<div a/b/c>", 3),
        ("<div a = 1 b>", 2),
        ("<div a=\">\" b c>", 3),
        ("<div a='>' b c>", 3),
        ("<div\ta\nb\rc\x0cd>", 4),
        ("<div =x y>", 2),
        ("<a b><c d e>", 2),
    ];
    for (html, want) in cases {
        let (ok, peak) = peak_of(&[html.as_bytes()]);
        assert!(ok, "{html}");
        assert_eq!(peak, *want, "{html}");
        assert_eq!(
            lexer_max(html.as_bytes()),
            *want as usize,
            "{html} vs lol_html"
        );
    }
}

#[test]
fn a_tag_over_the_cap_is_reported_and_the_cap_itself_is_allowed() {
    let at = format!("<div {}>", "a ".repeat(ATTR_CAP as usize));
    assert!(peak_of(&[at.as_bytes()]).0);
    let over = format!("<div {}>", "a ".repeat(ATTR_CAP as usize + 1));
    assert!(!peak_of(&[over.as_bytes()]).0);
}

#[test]
fn quoted_gt_and_rawtext_contexts_cannot_hide_a_tag() {
    let bomb = "a ".repeat(ATTR_CAP as usize + 10);
    let hostile = [
        // a quoted '>' inside the tag
        format!("<div x=\">\" {bomb}>"),
        format!("<div x='>' {bomb}>"),
        // comment / script / style / svg contexts that flip quote parity for a naive scanner
        format!("<!-- <a \" --><div x=\">\" {bomb}>"),
        format!("<script>a<b \"</script><div x=\">\" {bomb}>"),
        format!("<style>a<b '</style><div x='>' {bomb}>"),
        format!("<svg><style><div x=\">\" {bomb}></style>"),
        format!("<textarea><a \"</textarea><div x=\">\" {bomb}>"),
        format!("<a b='<c d=\"<e x=\">\" {bomb}>"),
    ];
    for h in &hostile {
        assert!(
            !peak_of(&[h.as_bytes()]).0,
            "not caught: {}",
            &h[..40.min(h.len())]
        );
    }
}

#[test]
fn chunk_boundaries_do_not_matter() {
    let html = format!(
        "<p>x</p><div x=\">\" {}>tail",
        "a ".repeat(ATTR_CAP as usize + 5)
    );
    for n in [1usize, 2, 3, 7, 64, 1000] {
        let chunks: Vec<&[u8]> = html.as_bytes().chunks(n).collect();
        assert!(!peak_of(&chunks).0, "chunk size {n}");
    }
}

#[test]
fn ordinary_pages_are_not_refused() {
    let page = "<!doctype html><html><head><meta charset=utf-8><title>t</title></head><body>\
        <p class=\"a b\" id=x>1 < 2 and 3 > 2 <b>bold</b></p><a href=\"/x?a=1&b=2\" title='t'>l</a>\
        <img src=a.png alt=\"x\"><script>if (a<b && c>d) {}</script></body></html>";
    assert!(peak_of(&[page.as_bytes()]).0);
}

/// Differential check against the real lexer: on random tag soup the guard's peak is never below what
/// `lol_html` reports (it may be above: it over-approximates). xorshift, fixed seed, deterministic.
#[test]
fn never_undercounts_the_real_lexer_on_random_tag_soup() {
    const TOKENS: &[&str] = &[
        "<",
        "<a",
        "<div",
        "<script>",
        "</script>",
        "<style>",
        "</style>",
        "<svg>",
        "</svg>",
        "<textarea>",
        "</textarea>",
        "<title>",
        "</title>",
        "<!--",
        "-->",
        "<![CDATA[",
        "]]>",
        "<!",
        "<?",
        "</",
        ">",
        "/>",
        "\"",
        "'",
        "=",
        " ",
        "  ",
        "\t",
        "\n",
        "\r",
        "\x0c",
        "a",
        "b",
        "x=1",
        "y='2'",
        "z=\"3\"",
        "k",
        "/",
        "=\"",
        "='",
        "<xmp>",
        "</xmp>",
        "<plaintext>",
        "<math>",
        "<iframe>",
        "<noscript>",
        "&gt;",
        "é",
    ];
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..4000 {
        let mut doc = String::new();
        let n = 5 + (next() % 60) as usize;
        for _ in 0..n {
            doc.push_str(TOKENS[(next() % TOKENS.len() as u64) as usize]);
        }
        let real = lexer_max(doc.as_bytes());
        let (_, peak) = peak_of(&[doc.as_bytes()]);
        assert!(
            peak as usize >= real,
            "guard undercounts: guard {peak} < lexer {real} on {doc:?}"
        );
    }
}
