//! Converter tests. The small fixtures here are LOCAL UNIT-TEST FIXTURES written for this file. They are NOT the
//! E-7 50-URL snapshot set (which does not exist yet) and prove nothing about the 95% / 50% conversion-quality
//! acceptance check.

use super::{MarkdownConverter, HOLDBACK};
use crate::convert::{ConvertError, Converter};
use reqwest::Url;

fn base() -> Option<Url> {
    Url::parse("https://example.com/dir/page.html").ok()
}

fn run_chunks(html: &str, chunks: &[&str]) -> Result<String, ConvertError> {
    let _ = html;
    let mut c = MarkdownConverter::new(base());
    let mut out = String::new();
    for ch in chunks {
        c.push(ch, &mut |s| out.push_str(s))?;
    }
    c.finish(&mut |s| out.push_str(s))?;
    Ok(out)
}

fn md(html: &str) -> String {
    run_chunks(html, &[html]).unwrap_or_else(|e| format!("ERR {e:?}"))
}

/// Split at every char boundary of size `n` bytes (rounded up to a boundary).
fn split(html: &str, n: usize) -> Vec<&str> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < html.len() {
        let mut j = (i + n).min(html.len());
        while !html.is_char_boundary(j) {
            j += 1;
        }
        v.push(&html[i..j]);
        i = j;
    }
    v
}

#[test]
fn headings_paragraphs_and_links() {
    let out = md(
        "<h1>Title</h1><p>Hello <a href=\"/x?a=1&amp;b=2\">world</a>.</p><h3>Sub</h3><p>Bye</p>",
    );
    assert_eq!(
        out,
        "# Title\n\nHello [world](https://example.com/x?a=1&b=2).\n\n### Sub\n\nBye"
    );
}

#[test]
fn relative_links_resolve_and_unsafe_ones_lose_the_destination() {
    let out = md(
        "<p><a href=rel>a</a> <a href=\"javascript:alert(1)\">b</a> <a href=\"data:text/html,x\">c</a> \
         <a href=\"#top\">d</a> <a href=\"mailto:a@b.c\">e</a> <a>f</a></p>",
    );
    assert_eq!(
        out,
        "[a](https://example.com/dir/rel) b c d [e](mailto:a@b.c) f"
    );
}

#[test]
fn long_href_is_dropped_and_parentheses_are_encoded() {
    let long = format!("<a href=\"/{}\">t</a>", "a".repeat(3000));
    assert_eq!(md(&long), "t");
    assert_eq!(
        md("<a href=\"/a(b)\">t</a>"),
        "[t](https://example.com/a%28b%29)"
    );
}

#[test]
fn emphasis_code_and_lazy_markers() {
    assert_eq!(
        md("<p>a <em>b</em> <strong> c </strong>d <code>x</code></p>"),
        "a *b* **c** d `x`"
    );
    assert_eq!(md("<p><b> </b>text<i></i></p>"), "text");
    assert_eq!(md("<p><a href=/x> </a>t</p>"), "t");
}

#[test]
fn lists_nested_and_ordered() {
    let out = md("<ul><li>a<ul><li>b</li><li>c</li></ul></li><li>d</li></ul><ol start=3><li>x</li><li>y</li></ol>");
    assert_eq!(out, "- a\n  - b\n  - c\n- d\n\n3. x\n4. y");
}

#[test]
fn unclosed_list_items_and_paragraphs() {
    assert_eq!(
        md("<ul><li>a<li>b<li>c</ul><p>one<p>two"),
        "- a\n- b\n- c\n\none\n\ntwo"
    );
}

#[test]
fn pre_and_code_blocks() {
    let out = md("<p>x</p><pre><code class=\"language-rust\">fn main() {\n    let a = 1;\n\n    a\n}\n</code></pre><p>y</p>");
    assert_eq!(
        out,
        "x\n\n```rust\nfn main() {\n    let a = 1;\n\n    a\n}\n```\n\ny"
    );
    assert_eq!(md("<pre>\n  a &lt; b\n</pre>"), "```\na < b\n```");
    // a fence inside the code cannot close ours
    let out = md("<pre>a\n```\nb</pre>");
    assert_eq!(out, "```\na\n``\u{200b}`\nb\n```");
}

#[test]
fn blockquote() {
    assert_eq!(
        md("<blockquote><p>q1</p><p>q2</p></blockquote>after"),
        "> q1\n\n> q2\n\nafter"
    );
}

#[test]
fn hr_br_images() {
    assert_eq!(md("a<br>b<br><br>c<hr>d"), "a\nb\n\nc\n\n---\n\nd");
    assert_eq!(
        md("<img alt=\"Logo  here\" src=\"/l.png\"><img src=/n.png><img alt=x src=\"data:image/png;base64,AAAA\">"),
        "![Logo here](https://example.com/l.png)"
    );
}

#[test]
fn entities_decode_including_split_across_chunks() {
    let html = "<p>a &amp; b &lt;c&gt; &copy; &#169; &#x263A; &nbsp;x AT&T</p>";
    let whole = md(html);
    assert_eq!(whole, "a & b <c> \u{a9} \u{a9} \u{263a} x AT&T");
    for n in 1..=7 {
        assert_eq!(
            run_chunks(html, &split(html, n)).unwrap(),
            whole,
            "chunk size {n}"
        );
    }
}

#[test]
fn script_style_and_chrome_are_dropped() {
    let html = "<html><head><title>T</title><style>p{}</style><script>var a='<p>x</p>';</script></head><body>\
        <nav><ul><li><a href=/a>Home</a></li></ul></nav><header class=\"x\">hd</header>\
        <div hidden>h1</div><div aria-hidden=\"true\">h2</div><div style=\"color:red; Display: None\">h3</div>\
        <div role=\"navigation\">h4</div><div class=\"cookie-banner\">h5</div><div class=\"wrap sidebar\">h6</div>\
        <div id=\"newsletter\">h7</div><noscript>h8</noscript><svg><style>x</style><text>h9</text></svg>\
        <p>Keep <script>bad()</script>this</p><form><input value=v><button>Go</button><p>form text</p></form>\
        <footer>foot</footer><aside>side</aside></body></html>";
    assert_eq!(md(html), "hd\n\nKeep this\n\nform text");
    assert!(!md(html).contains("script"));
}

#[test]
fn class_heuristic_needs_a_whole_token_and_spares_body() {
    assert_eq!(
        md("<body class=\"has-sidebar modal-open\"><div class=\"with-sidebar\">kept</div></body>"),
        "kept"
    );
    assert_eq!(
        md("<div class=\"sidebar-left\">gone</div><div class=\"share-links\">gone</div>x"),
        "x"
    );
}

#[test]
fn unclosed_head_ends_at_body_content() {
    assert_eq!(
        md("<head><title>t</title><meta charset=utf-8><p>after</p>"),
        "after"
    );
}

#[test]
fn landmark_holdback_keeps_only_the_landmark() {
    let out = md(
        "<div>before</div><h1>Site</h1><main><h2>Real</h2><p>content</p></main><div>after</div>",
    );
    assert_eq!(out, "## Real\n\ncontent");
    let out = md(
        "<p>chrome</p><article><p>one</p></article><div>between</div><article><p>two</p></article>",
    );
    assert_eq!(out, "one\n\ntwo");
    let out = md("<p>x</p><div role=\"main\"><p>y</p></div><p>z</p>");
    assert_eq!(out, "y");
}

#[test]
fn no_landmark_means_whole_body() {
    assert_eq!(md("<p>a</p><div>b</div>"), "a\n\nb");
}

#[test]
fn late_landmark_beyond_the_holdback_is_ignored() {
    let filler = "<p>0123456789012345678901234567890123456789</p>".repeat(HOLDBACK / 40);
    let html = format!("{filler}<main><p>late</p></main><p>tail</p>");
    let out = md(&html);
    assert!(out.starts_with("0123456789"));
    assert!(out.contains("late") && out.ends_with("tail"));
}

#[test]
fn tables_become_pipe_rows() {
    let out = md("<table><thead><tr><th>A</th><th>B|C</th></tr></thead><tbody><tr><td>1</td><td><b>2</b> x</td></tr><tr><td>3<td>4</table>after");
    assert_eq!(
        out,
        "| A | B\\|C |\n| --- | --- |\n| 1 | **2** x |\n| 3 | 4 |\n\nafter"
    );
    assert_eq!(
        md("<table><tr><td>a</td><td></td></tr><tr><td></td></tr></table>"),
        "| a |  |"
    );
}

#[test]
fn oversized_cell_degrades_to_plain_text() {
    let big = "word ".repeat(20_000);
    let out = md(&format!(
        "<table><tr><td>{big}</td><td>second</td></tr><tr><td>next row</td></tr></table>tail"
    ));
    assert!(out.contains("word word") && out.contains("next row") && out.ends_with("tail"));
    assert!(!out.contains('|'));
}

#[test]
fn whitespace_is_collapsed_and_nul_replaced() {
    assert_eq!(
        md("<p>  a \n\t b   <span> c </span>\u{a0}d </p>"),
        "a b c d"
    );
    assert_eq!(md("<p>a\0b</p>"), "a\u{fffd}b");
}

#[test]
fn every_chunk_size_gives_the_same_output() {
    let html = "<html><head><title>T</title></head><body><h1>H &amp; H</h1><p>para <a href=/l>link &lt;x&gt;</a> \
        and <em>em</em>.</p><ul><li>one<li>two</ul><pre><code class=language-py>x = 1\ny = 2\n</code></pre>\
        <table><tr><th>h<th>i<tr><td>1<td>2</table><nav>skip</nav><blockquote>q</blockquote>\u{1f600} caf\u{e9}</body></html>";
    let whole = md(html);
    assert!(whole.contains("# H & H") && whole.contains("caf\u{e9}"));
    for n in 1..=40 {
        assert_eq!(
            run_chunks(html, &split(html, n)).unwrap(),
            whole,
            "chunk size {n}"
        );
    }
}

#[test]
fn output_is_utf8_and_char_boundary_safe_on_multibyte_input() {
    let html = "<p>\u{1f600}\u{4e2d}\u{6587}&#x1F600;</p>";
    assert_eq!(md(html), "\u{1f600}\u{4e2d}\u{6587}\u{1f600}");
}

// ---- hostile input: none of these may panic, hang or grow without bound -----------------------------------

fn no_panic(html: &str) -> Result<String, ConvertError> {
    run_chunks(html, &split(html, 4096))
}

#[test]
fn hostile_deep_nesting_is_bounded() {
    for tag in [
        "div",
        "span",
        "ul",
        "blockquote",
        "table",
        "b",
        "a href=/x",
        "li",
        "nav",
        "p",
    ] {
        let name = tag.split(' ').next().unwrap_or(tag);
        let html =
            format!("<{tag}>").repeat(200_000) + "text" + &format!("</{name}>").repeat(200_000);
        let r = no_panic(&html);
        assert!(r.is_ok() || r == Err(ConvertError::Limit), "{tag}: {r:?}");
    }
}

#[test]
fn hostile_huge_attribute_and_tag_hit_the_limit_not_memory() {
    let html = format!("<div class=\"{}\">x</div>", "a".repeat(8 * 1024 * 1024));
    assert_eq!(no_panic(&html), Err(ConvertError::Limit));
    let html = format!("<{}>x", "a".repeat(8 * 1024 * 1024));
    let r = no_panic(&html);
    assert!(r.is_ok() || r == Err(ConvertError::Limit));
    let html = format!("<p title=\"{}\">y</p>", "<>&\"'".repeat(100_000));
    assert!(no_panic(&html).is_ok());
}

#[test]
fn hostile_unclosed_and_malformed_markup() {
    for html in [
        "<",
        "<<<<<<",
        "<a href=\"",
        "<!--",
        "<!-- x -- y --!>z",
        "<![CDATA[ x ]]>",
        "<p><b><i></p></b></i>text",
        "<table><td><table><td><pre>x</table>y",
        "<pre><pre><pre>x",
        "</p></div></li></ul></table></pre></blockquote>text",
        "<ul></ul></ul></ul><li></li>x",
        "<a><a><a href=/x>t",
        "<select><xmp><option>x</select>",
        "<svg><foreignObject><p>x</p></foreignObject></svg>y",
        "&#xFFFFFFFF; &#0; &#55296; &#x110000; &bogus; &amp",
        "<textarea><p>x</textarea>y",
        "<script><!-- <script> </script> -->z",
        "<plaintext><p>x",
        "<body><body><html><html>x",
        "<img alt=\"a\" src=\"\"><img alt src=/x>",
    ] {
        let r = no_panic(html);
        assert!(r.is_ok(), "{html:?}: {r:?}");
    }
}

#[test]
fn hostile_nul_control_and_replacement_characters() {
    let html = "<p>\0\0<b>\0</b>\u{1}\u{7f}\u{fffd}</p><pre>\0\n\0</pre><table><tr><td>\0</table>";
    let r = no_panic(html);
    assert!(r.is_ok(), "{r:?}");
    assert!(!r.unwrap_or_default().contains('\0'));
}

#[test]
fn hostile_link_amplification_is_bounded() {
    // Short relative links resolve to long absolute ones; output is capped instead of exploding.
    let long_base = Url::parse(&format!("https://example.com/{}/", "d".repeat(1900))).unwrap();
    let html = "<a href=x>y</a>".repeat(400_000);
    let mut c = MarkdownConverter::new(Some(long_base));
    let mut total = 0usize;
    let mut worst = 0usize;
    let mut res = Ok(());
    for part in split(&html, 64 * 1024) {
        res = c.push(part, &mut |s| {
            total += s.len();
            worst = worst.max(s.len());
        });
        if res.is_err() {
            break;
        }
    }
    assert!(res.is_ok() || res == Err(ConvertError::Limit));
    assert!(worst <= 5 * 1024 * 1024, "one step produced {worst} bytes");
}

#[test]
fn many_table_cells_and_rows_are_bounded() {
    // 4000 unclosed cells: ROW_CELLS (256) bounds what is kept
    let html = format!("<table><tr>{}</tr></table>", "<td>x".repeat(4000));
    assert!(no_panic(&html).is_ok());
    // hundreds of thousands of unclosed cells nest without end in the tokenizer: its memory limit answers
    let html = format!("<table><tr>{}</tr></table>", "<td>x".repeat(300_000));
    let r = no_panic(&html);
    assert!(r.is_ok() || r == Err(ConvertError::Limit), "{r:?}");
    let html = format!("<table>{}</table>", "<tr><td>x</td></tr>".repeat(100_000));
    assert!(no_panic(&html).is_ok());
}

#[test]
fn invalid_utf8_never_reaches_the_converter_but_replacement_text_is_fine() {
    // The body pipeline (fetch::body) turns invalid bytes into U+FFFD before conversion; here the text carries them.
    let html = "<p>a\u{fffd}\u{fffd}b</p>";
    assert_eq!(md(html), "a\u{fffd}\u{fffd}b");
}

#[test]
fn pseudo_random_soup_never_panics() {
    // A tiny xorshift stream over a tag-heavy alphabet: deterministic, no dev-dependency.
    let alphabet = [
        "<p>",
        "</p>",
        "<div>",
        "</div>",
        "<a href=/q>",
        "</a>",
        "<ul>",
        "<li>",
        "</ul>",
        "<table>",
        "<tr>",
        "<td>",
        "</table>",
        "<pre>",
        "</pre>",
        "<b>",
        "</b>",
        "<em>",
        "<h2>",
        "</h2>",
        "<br>",
        "<hr>",
        "<main>",
        "</main>",
        "<nav>",
        "</nav>",
        "&amp;",
        "&",
        "#",
        "text ",
        "\n",
        "<!--",
        "-->",
        "<script>",
        "</script>",
        "<img alt=a src=/i>",
        "<blockquote>",
        "</blockquote>",
        "<",
        ">",
        "\"",
        "=",
        "\u{e9}",
        "\u{1f600}",
        "<head>",
        "</head>",
        "<body>",
    ];
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    for round in 0..300 {
        let mut html = String::new();
        for _ in 0..(50 + round) {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            html.push_str(alphabet[(x % alphabet.len() as u64) as usize]);
        }
        let whole = run_chunks(&html, &[&html]);
        assert!(whole.is_ok(), "{html:?}");
        let n = 1 + (x % 9) as usize;
        assert_eq!(
            run_chunks(&html, &split(&html, n)),
            whole,
            "chunking changed output for {html:?}"
        );
    }
}

#[test]
fn memory_stays_bounded_on_a_large_page() {
    // 8 MiB of realistic markup through 64 KiB steps: the largest single output step and the pending output
    // buffer stay small (nothing accumulates the page).
    let unit = "<div class=\"c\"><h2>Heading</h2><p>Some <a href=\"/p\">linked</a> text &amp; more.</p><ul><li>a<li>b</ul></div>\n";
    let html = unit.repeat(8 * 1024 * 1024 / unit.len());
    let mut c = MarkdownConverter::new(base());
    let (mut worst, mut total) = (0usize, 0usize);
    for part in split(&html, 64 * 1024) {
        assert!(c
            .push(part, &mut |s| {
                worst = worst.max(s.len());
                total += s.len();
            })
            .is_ok());
    }
    assert!(c.finish(&mut |s| total += s.len()).is_ok());
    assert!(total > 1024 * 1024);
    assert!(worst <= HOLDBACK + 256 * 1024, "largest step {worst}");
}

/// A-4 AC: converting a 1 MiB page costs at most 500 ms p95 on aarch64 (BENCHMARK.md section 9: page held in
/// memory, network excluded, 5 warm-up calls, 100 timed calls, p95). `#[ignore]` because a debug build says
/// nothing: run `cargo test --release --lib conversion_overhead_1mib -- --ignored --nocapture` on the target host.
#[test]
#[ignore = "timing: run in release on the target host"]
fn conversion_overhead_1mib() {
    let unit = "<div class=\"c\"><h2>Heading</h2><p>Some <a href=\"/p\">linked</a> <em>text</em> &amp; more words here.</p><ul><li>a<li>b</ul><pre><code>x = 1</code></pre></div>\n";
    let html = unit.repeat(1_048_576 / unit.len() + 1);
    let html = &html[..html
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= 1_048_576)
        .last()
        .unwrap_or(0)];
    let one = || {
        let mut c = MarkdownConverter::new(base());
        let mut n = 0usize;
        for part in split(html, 64 * 1024) {
            assert!(c.push(part, &mut |s| n += s.len()).is_ok());
        }
        assert!(c.finish(&mut |s| n += s.len()).is_ok());
        n
    };
    for _ in 0..5 {
        one();
    }
    let mut ms: Vec<f64> = (0..100)
        .map(|_| {
            let t = std::time::Instant::now();
            one();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    let p95 = ms[94];
    eprintln!(
        "CONVERT_1MIB bytes={} median_ms={:.1} p95_ms={p95:.1} max_ms={:.1} arch={}",
        html.len(),
        ms[49],
        ms[99],
        std::env::consts::ARCH
    );
    assert!(p95 <= 500.0, "p95 {p95} ms over the 500 ms target");
}
