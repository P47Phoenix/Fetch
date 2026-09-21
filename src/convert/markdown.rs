//! Streaming HTML to markdown (A-4, ADR-002 option B): `lol_html` (send API) tokenizes the stream and a small
//! state machine ([`Md`]) writes markdown. There is no DOM: state is a handful of counters and stacks with hard
//! caps, so memory is bounded whatever the page size or shape. Output is a pure function of the input bytes, not
//! of how the stream is chunked (text nodes are re-joined, entities carried across chunks).
//!
//! Rules (ADR-002 tier 1 and 3; tier 2, link density, is deliberately NOT implemented, see the dev report):
//! * dropped with their whole subtree: `script style noscript template svg canvas iframe object embed dialog
//!   head title nav footer aside select textarea button`, `[hidden]`, `[aria-hidden=true]`, roles
//!   navigation/banner/contentinfo/complementary/search, inline `display:none` / `visibility:hidden`, and
//!   containers whose class or id token is a noise word (cookie, consent, banner, popup, modal, advert, ad,
//!   sidebar, share, newsletter). `form` containers are traversed (ASP.NET pages wrap the body in one).
//! * landmark holdback: output is held (at most [`HOLDBACK`] bytes) until a `main`, `article` or `[role=main]`
//!   start tag; the held part is then discarded and only landmark subtrees are emitted. If the holdback fills
//!   or the stream ends first, everything is emitted (whole-body mode).
//! * mapping: h1-h6, p, br, hr, ul/ol/li (nested, numbered), a (absolute href), em/strong, code, pre/code
//!   (fenced), blockquote, img (only with alt), tables (pipe rows, separator after a header row).
//!
//! Hostile input: every state stack and buffer is capped; `lol_html`'s own memory limit turns a huge tag or
//! absurd nesting into [`ConvertError::Limit`], and the attribute-count guard (`tagscan`) refuses a tag with more
//! than `tagscan::ATTR_CAP` attributes before `lol_html` allocates for them (its limit does not count them);
//! nothing here indexes, unwraps or recurses on input.

use super::tagscan::Scan;
use super::{ConvertError, Converter};
use lol_html::html_content::TextType;
use lol_html::send::{Element, HtmlRewriter, Settings};
use lol_html::{doc_text, element, end_tag, MemorySettings};
use reqwest::Url;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// `lol_html` memory limit per rewriter (architecture 5.1 row f).
const REWRITER_LIMIT: usize = 2 * 1024 * 1024;
/// Landmark holdback (architecture 5.1 row h).
pub const HOLDBACK: usize = 256 * 1024;
/// Hard bound on output produced by one input slice (guards amplification such as many relative links).
const OUT_HARD: usize = 4 * 1024 * 1024;
/// Open elements that get an end handler; deeper elements are ignored (text still flows).
const OPEN_CAP: u32 = 256;
/// Inline frames (emphasis, code, link) open at once.
const FRAME_CAP: usize = 64;
/// List nesting tracked (deeper lists are flattened).
const LIST_CAP: usize = 64;
/// Indent and quote levels actually drawn.
const DRAW_CAP: usize = 6;
/// Link and image destination bound (ADR-002).
const HREF_CAP: usize = 2048;
/// Table cell buffer (ADR-002: 64 KB) and row buffer. ADR-002 said row <= 64 KB; a cell alone may use 64 KB, so
/// the row bound is 256 KB in total (a row over it is abandoned and its text emitted as plain lines).
const CELL_CAP: usize = 64 * 1024;
const ROW_BYTES: usize = 256 * 1024;
const ROW_CELLS: usize = 256;
/// Longest tail of an unfinished character reference carried across text chunks.
const ENTITY_TAIL: usize = 32;
const ALT_CAP: usize = 512;

const DROP_TAGS: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "canvas", "iframe", "object", "embed",
    "dialog", "head", "title", "nav", "footer", "aside", "select", "textarea", "button", "input",
];
/// Elements with optional end tags or no end tag: attribute based hiding is not applied (an unmatched end tag
/// would leave everything after them hidden).
const OPTIONAL_END: &[&str] = &[
    "p", "li", "dt", "dd", "tr", "td", "th", "thead", "tbody", "tfoot", "option", "optgroup",
    "colgroup", "html", "body",
];
const CLASS_DROP_TAGS: &[&str] = &["div", "section", "ul", "ol", "form", "span"];
const NOISE_WORDS: &[&str] = &[
    "cookie",
    "cookies",
    "consent",
    "banner",
    "popup",
    "modal",
    "advert",
    "advertisement",
    "ad",
    "ads",
    "sidebar",
    "share",
    "newsletter",
];
const HEAD_LEGAL: &[&str] = &[
    "title", "base", "link", "meta", "script", "style", "noscript", "template",
];
const HIDDEN_ROLES: &[&str] = &[
    "navigation",
    "banner",
    "contentinfo",
    "complementary",
    "search",
];

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    /// Output is held until a landmark or the holdback fills.
    Hold,
    /// A landmark was seen: only landmark subtrees are emitted.
    Landmark,
    /// No landmark: everything is emitted.
    Whole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameKind {
    Em,
    Strong,
    Code,
    Link,
}

struct Frame {
    kind: FrameKind,
    open: &'static str,
    close: String,
    written: bool,
}

struct Skip {
    tag: String,
    depth: u32,
    id: u64,
}

struct ListCtx {
    ordered: bool,
    next: u64,
}

struct PreState {
    lang: String,
    fence_open: bool,
    first: bool,
    nl: usize,
    ticks: u8,
}

#[derive(Default)]
struct Tbl {
    active: bool,
    broken: bool,
    cell: Option<String>,
    row: Vec<String>,
    row_has_th: bool,
    rows: u32,
}

/// What to undo when an element's end tag arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    Block(u8),
    Heading,
    List,
    Item,
    Quote,
    Pre,
    Frame(FrameKind),
    Table,
    Row,
    Cell,
    Skip(u64),
}

#[derive(Debug, Clone, Copy)]
struct EndAct {
    kind: Kind,
    landmark: bool,
}

/// Attributes the converter looks at, gathered in one pass.
#[derive(Default)]
struct Attrs {
    hidden: bool,
    aria_hidden: bool,
    role: String,
    style_hidden: bool,
    class: String,
    id: String,
    href: Option<String>,
    src: Option<String>,
    alt: Option<String>,
    start: Option<u64>,
}

impl Attrs {
    fn read(el: &Element<'_, '_>) -> Self {
        let mut a = Self::default();
        for at in el.attributes() {
            match at.name().as_str() {
                "hidden" => a.hidden = true,
                "aria-hidden" => a.aria_hidden = at.value().trim().eq_ignore_ascii_case("true"),
                "role" => a.role = at.value().trim().to_ascii_lowercase(),
                "style" => {
                    let s: String = at
                        .value()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .flat_map(char::to_lowercase)
                        .collect();
                    a.style_hidden = s.contains("display:none") || s.contains("visibility:hidden");
                }
                "class" => a.class = at.value(),
                "id" => a.id = at.value(),
                "href" => a.href = Some(unescape(&at.value())),
                "src" => a.src = Some(unescape(&at.value())),
                "alt" => a.alt = Some(unescape(&at.value())),
                "start" => a.start = at.value().trim().parse().ok(),
                _ => {}
            }
        }
        a
    }

    fn noisy(&self) -> bool {
        let hit = |tok: &str| {
            let t = tok.to_ascii_lowercase();
            NOISE_WORDS.iter().any(|w| {
                t == *w
                    || (t.len() > w.len()
                        && t.starts_with(w)
                        && matches!(t.as_bytes().get(w.len()), Some(b'-' | b'_')))
            })
        };
        self.class.split_whitespace().any(hit) || (!self.id.is_empty() && hit(self.id.trim()))
    }

    fn hidden_by_attrs(&self) -> bool {
        self.hidden
            || self.aria_hidden
            || self.style_hidden
            || HIDDEN_ROLES.contains(&self.role.as_str())
    }
}

/// The markdown writer. All fields are bounded (see the caps above).
struct Md {
    base: Option<Url>,
    out: String,
    gate: Gate,
    landmark_open: u32,
    skip: Option<Skip>,
    skip_seq: u64,
    open: u32,
    overflow: bool,
    // line state
    wrote_any: bool,
    want_break: u8,
    line_open: bool,
    line_content: bool,
    pending_space: bool,
    li_marker: Option<String>,
    head_marker: Option<String>,
    quote: usize,
    lists: Vec<ListCtx>,
    frames: Vec<Frame>,
    pre: Option<PreState>,
    tbl: Tbl,
    tbl_depth: u32,
    // text
    tail: String,
}

impl Md {
    fn new(base: Option<Url>) -> Self {
        Self {
            base,
            out: String::new(),
            gate: Gate::Hold,
            landmark_open: 0,
            skip: None,
            skip_seq: 0,
            open: 0,
            overflow: false,
            wrote_any: false,
            want_break: 0,
            line_open: false,
            line_content: false,
            pending_space: false,
            li_marker: None,
            head_marker: None,
            quote: 0,
            lists: Vec::new(),
            frames: Vec::new(),
            pre: None,
            tbl: Tbl::default(),
            tbl_depth: 0,
            tail: String::new(),
        }
    }

    fn allowed(&self) -> bool {
        self.skip.is_none() && (self.gate != Gate::Landmark || self.landmark_open > 0)
    }

    fn in_cell(&self) -> bool {
        self.tbl.cell.is_some()
    }

    // ---- output primitives ------------------------------------------------------------------------------

    fn put(&mut self, s: &str) {
        if let Some(cell) = self.tbl.cell.as_mut() {
            if cell.len() + s.len() > CELL_CAP {
                self.abandon_table();
                return self.put_main(s);
            }
            cell.push_str(s);
        } else {
            self.put_main(s);
        }
    }

    fn put_main(&mut self, s: &str) {
        if self.out.len() + s.len() > OUT_HARD {
            self.overflow = true;
            return;
        }
        self.out.push_str(s);
        if !s.is_empty() {
            self.wrote_any = true;
        }
        // The holdback ends deterministically where the output crosses it, not where a slice boundary falls.
        if self.gate == Gate::Hold && self.out.len() > HOLDBACK {
            self.gate = Gate::Whole;
        }
    }

    /// Ask for `n` line breaks before the next content (lazy: nothing is written until content arrives).
    fn block(&mut self, n: u8) {
        self.close_frames();
        if self.in_cell() {
            self.pending_space = true;
            return;
        }
        self.want_break = self.want_break.max(n);
        self.pending_space = false;
        self.head_marker = None;
    }

    fn close_frames(&mut self) {
        while let Some(f) = self.frames.pop() {
            if f.written {
                self.put(&f.close);
            }
        }
    }

    /// Everything that must precede the first character of content: line breaks, the line prefix (quote,
    /// list indent, marker), a collapsed space, and the inline opens that are still pending.
    fn begin_content(&mut self) {
        if !self.in_cell() {
            if self.want_break > 0 {
                if self.wrote_any {
                    for _ in 0..self.want_break {
                        self.put_main("\n");
                    }
                }
                self.want_break = 0;
                self.line_open = false;
                self.line_content = false;
            }
            if !self.line_open {
                self.write_prefix();
                self.line_open = true;
            }
        }
        if self.pending_space && self.line_content_or_cell() {
            self.put(" ");
        }
        self.pending_space = false;
        for i in 0..self.frames.len() {
            if !self.frames[i].written {
                let open = self.frames[i].open;
                self.frames[i].written = true;
                self.put(open);
            }
        }
    }

    fn line_content_or_cell(&self) -> bool {
        match &self.tbl.cell {
            Some(c) => !c.is_empty(),
            None => self.line_content,
        }
    }

    fn write_prefix(&mut self) {
        let mut p = String::new();
        for _ in 0..self.quote.min(DRAW_CAP) {
            p.push_str("> ");
        }
        let depth = self.lists.len().min(DRAW_CAP + 1);
        if depth > 0 {
            for _ in 0..(depth - 1) * 2 {
                p.push(' ');
            }
        }
        let (item, head) = (self.li_marker.take(), self.head_marker.take());
        if let Some(m) = &item {
            p.push_str(m);
        } else if depth > 0 && head.is_none() {
            p.push_str("  ");
        }
        if let Some(m) = &head {
            p.push_str(m);
        }
        self.put_main(&p);
    }

    fn write_str_content(&mut self, s: &str) {
        self.begin_content();
        self.put(s);
        self.line_content = true;
    }

    /// Inline text with whitespace collapsed (state kept across calls).
    fn inline(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\0' {
                self.write_char('\u{FFFD}');
            } else if c.is_whitespace() {
                if self.line_content_or_cell() {
                    self.pending_space = true;
                }
            } else {
                self.write_char(c);
            }
        }
    }

    fn write_char(&mut self, c: char) {
        self.begin_content();
        let mut b = [0u8; 4];
        self.put(c.encode_utf8(&mut b));
        self.line_content = true;
    }

    // ---- text -------------------------------------------------------------------------------------------

    fn text(&mut self, chunk: &str, last: bool, decode: bool) {
        if !self.allowed() {
            self.tail.clear();
            return;
        }
        let mut buf = std::mem::take(&mut self.tail);
        buf.push_str(chunk);
        let mut keep = 0;
        if decode && !last {
            keep = incomplete_entity_len(&buf);
        }
        let split = buf.len() - keep;
        let (ready, rest) = buf.split_at(split);
        self.tail = rest.to_string();
        let decoded = if decode {
            html_escape::decode_html_entities(ready)
        } else {
            std::borrow::Cow::Borrowed(ready)
        };
        if self.pre.is_some() && !self.in_cell() {
            self.pre_text(&decoded);
        } else {
            self.inline(&decoded);
        }
    }

    fn pre_text(&mut self, s: &str) {
        for c in s.chars() {
            let Some(pre) = self.pre.as_mut() else {
                return;
            };
            if c == '\r' {
                continue;
            }
            let first = std::mem::replace(&mut pre.first, false);
            if c == '\n' {
                // the parser drops one newline right after <pre>; blank lines before the code are dropped
                if !first && pre.fence_open {
                    pre.nl += 1;
                }
                continue;
            }
            if !pre.fence_open && c.is_whitespace() {
                continue;
            }
            let c = if c == '\0' { '\u{FFFD}' } else { c };
            if !pre.fence_open {
                pre.fence_open = true;
                let lang = std::mem::take(&mut pre.lang);
                self.begin_content();
                self.put("```");
                self.put(&lang);
                self.put("\n");
            }
            let Some(pre) = self.pre.as_mut() else {
                return;
            };
            let nl = std::mem::take(&mut pre.nl);
            pre.ticks = if c == '`' {
                pre.ticks.saturating_add(1)
            } else {
                0
            };
            let zw = pre.ticks >= 3;
            if zw {
                pre.ticks = 1;
            }
            for _ in 0..nl {
                self.put("\n");
            }
            if zw {
                self.put("\u{200b}");
            }
            let mut b = [0u8; 4];
            self.put(c.encode_utf8(&mut b));
        }
    }

    // ---- elements ---------------------------------------------------------------------------------------

    fn start(&mut self, name: &str, void: bool, at: &Attrs) -> Option<EndAct> {
        let none = |kind| {
            Some(EndAct {
                kind,
                landmark: false,
            })
        };
        // inside a dropped subtree only the matching end tag matters
        if let Some(sk) = self.skip.as_mut() {
            if sk.tag == name {
                if void {
                    return None;
                }
                sk.depth = sk.depth.saturating_add(1);
                return if sk.depth <= OPEN_CAP {
                    none(Kind::Skip(sk.id))
                } else {
                    None
                };
            }
            if sk.tag == "head" && !HEAD_LEGAL.contains(&name) {
                self.skip = None; // a body-content tag ends an unclosed <head>
            } else {
                return None;
            }
        }
        // Drop rules first: a dropped subtree is a `Skip`, which is not counted in `open`, so it must be
        // registered at any depth (past the cap its text would otherwise flow into the output).
        let attr_ok = !OPTIONAL_END.contains(&name) && !void;
        let drop = DROP_TAGS.contains(&name)
            || (attr_ok
                && (at.hidden_by_attrs() || (CLASS_DROP_TAGS.contains(&name) && at.noisy())));
        if drop {
            if void {
                return None;
            }
            self.skip_seq += 1;
            let id = self.skip_seq;
            self.skip = Some(Skip {
                tag: name.to_string(),
                depth: 1,
                id,
            });
            return none(Kind::Skip(id));
        }
        let over_cap = self.open >= OPEN_CAP && !void;
        // landmark: the first one (Hold -> Landmark) is honoured at any depth; nested ones only under the cap
        let mut landmark = false;
        if name == "main" || name == "article" || at.role == "main" {
            match self.gate {
                Gate::Hold => {
                    self.reset_output();
                    self.gate = Gate::Landmark;
                    self.landmark_open = 1;
                    landmark = true;
                }
                Gate::Landmark if !over_cap => {
                    self.landmark_open = self.landmark_open.saturating_add(1);
                    landmark = true;
                }
                Gate::Landmark | Gate::Whole => {}
            }
        }
        if over_cap {
            return landmark.then_some(EndAct {
                kind: Kind::None,
                landmark: true,
            });
        }
        let kind = self.structure(name, void, at);
        if kind == Kind::None && !landmark {
            return None;
        }
        Some(EndAct { kind, landmark })
    }

    fn reset_output(&mut self) {
        self.out.clear();
        self.wrote_any = false;
        self.want_break = 0;
        self.line_open = false;
        self.line_content = false;
        self.pending_space = false;
        self.frames.clear();
        self.li_marker = None;
        self.head_marker = None;
        self.tail.clear();
        // structure opened before the landmark (a layout table, a wrapper list) must not shape its content
        self.lists.clear();
        self.quote = 0;
        self.pre = None;
        self.tbl = Tbl::default();
        self.tbl_depth = 0;
    }

    fn structure(&mut self, name: &str, void: bool, at: &Attrs) -> Kind {
        if !self.allowed() {
            // outside every landmark nothing is emitted; only landmark starts matter
            return Kind::None;
        }
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let n = name
                    .as_bytes()
                    .get(1)
                    .map_or(1, |b| usize::from(b - b'0'))
                    .clamp(1, 6);
                self.block(2);
                self.head_marker = Some(format!("{} ", "#".repeat(n)));
                Kind::Heading
            }
            "p" | "figure" => self.block2(),
            "div" | "section" | "main" | "article" | "header" | "address" | "form" | "dl"
            | "dt" | "dd" | "figcaption" | "summary" | "details" | "hgroup" | "fieldset"
            | "legend" | "center" | "menu" | "caption" | "body" | "html" => {
                self.block(1);
                Kind::Block(1)
            }
            "br" => {
                if self.in_cell() || self.pre.is_some() {
                    self.pending_space = true;
                } else if self.wrote_any {
                    self.want_break = (self.want_break + 1).min(2);
                    self.pending_space = false;
                }
                Kind::None
            }
            "hr" => {
                self.block(2);
                self.write_str_content("---");
                self.block(2);
                Kind::None
            }
            "ul" | "ol" => {
                self.block(if self.lists.is_empty() { 2 } else { 1 });
                if self.lists.len() < LIST_CAP {
                    self.lists.push(ListCtx {
                        ordered: name == "ol",
                        next: at.start.unwrap_or(1),
                    });
                    Kind::List
                } else {
                    Kind::Block(1)
                }
            }
            "li" => {
                self.block(1);
                let m = match self.lists.last_mut() {
                    Some(l) if l.ordered => {
                        let n = l.next;
                        l.next = l.next.saturating_add(1);
                        format!("{n}. ")
                    }
                    _ => "- ".to_string(),
                };
                if !self.in_cell() {
                    self.li_marker = Some(m);
                }
                Kind::Item
            }
            "blockquote" => {
                self.block(2);
                self.quote += 1;
                Kind::Quote
            }
            "pre" => {
                self.block(2);
                if !self.in_cell() {
                    self.pre = Some(PreState {
                        lang: String::new(),
                        fence_open: false,
                        first: true,
                        nl: 0,
                        ticks: 0,
                    });
                }
                Kind::Pre
            }
            "code" | "tt" | "kbd" | "samp" => {
                if let Some(pre) = self.pre.as_mut() {
                    if !pre.fence_open {
                        if let Some(l) = language_of(&at.class) {
                            pre.lang = l;
                        }
                    }
                    return Kind::None;
                }
                self.push_frame(FrameKind::Code, "`", "`".to_string())
            }
            "em" | "i" | "cite" | "dfn" => self.push_frame(FrameKind::Em, "*", "*".to_string()),
            "strong" | "b" => self.push_frame(FrameKind::Strong, "**", "**".to_string()),
            "a" => {
                if let Some(pos) = self.frames.iter().rposition(|f| f.kind == FrameKind::Link) {
                    // an <a> cannot contain an <a>: the earlier one ends here
                    while self.frames.len() > pos {
                        if let Some(f) = self.frames.pop() {
                            if f.written {
                                self.put(&f.close);
                            }
                        }
                    }
                }
                match at.href.as_deref().and_then(|h| self.resolve(h)) {
                    Some(dest) => self.push_frame(FrameKind::Link, "[", format!("]({dest})")),
                    None => Kind::None,
                }
            }
            "img" => {
                self.image(at);
                Kind::None
            }
            "table" => {
                if self.tbl_depth == 0 {
                    self.block(2);
                    self.tbl = Tbl {
                        active: true,
                        ..Tbl::default()
                    };
                }
                self.tbl_depth += 1;
                Kind::Table
            }
            "tr" => {
                if self.tbl_depth == 1 && self.tbl.active && !self.tbl.broken {
                    self.close_cell();
                    self.close_row();
                    self.tbl.row_has_th = false;
                } else if !void {
                    self.block(1);
                }
                Kind::Row
            }
            "td" | "th" => {
                if self.tbl_depth == 1 && self.tbl.active && !self.tbl.broken {
                    self.close_frames();
                    self.close_cell();
                    self.tbl.cell = Some(String::new());
                    self.pending_space = false;
                    if name == "th" {
                        self.tbl.row_has_th = true;
                    }
                } else {
                    self.pending_space = true;
                }
                Kind::Cell
            }
            _ => Kind::None,
        }
    }

    fn block2(&mut self) -> Kind {
        self.block(2);
        Kind::Block(2)
    }

    fn push_frame(&mut self, kind: FrameKind, open: &'static str, close: String) -> Kind {
        if self.frames.len() >= FRAME_CAP {
            return Kind::None;
        }
        self.frames.push(Frame {
            kind,
            open,
            close,
            written: false,
        });
        Kind::Frame(kind)
    }

    fn resolve(&self, href: &str) -> Option<String> {
        let href = href.trim();
        if href.is_empty() || href.len() > HREF_CAP || href.starts_with('#') {
            return None;
        }
        let url = match &self.base {
            Some(b) => b.join(href).ok()?,
            None => Url::parse(href).ok()?,
        };
        if !matches!(url.scheme(), "http" | "https" | "mailto" | "tel") {
            return None;
        }
        let s = url.as_str();
        if s.len() > HREF_CAP {
            return None;
        }
        Some(
            s.replace('(', "%28")
                .replace(')', "%29")
                .replace(['<', '>'], ""),
        )
    }

    fn image(&mut self, at: &Attrs) {
        let alt: String = at
            .alt
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|c| !matches!(c, '[' | ']' | '\0'))
            .take(ALT_CAP)
            .collect();
        if alt.is_empty() {
            return;
        }
        let Some(src) = at
            .src
            .as_deref()
            .and_then(|s| self.resolve(s))
            .filter(|s| s.starts_with("http"))
        else {
            return;
        };
        self.write_str_content(&format!("![{alt}]({src})"));
    }

    // ---- end tags ---------------------------------------------------------------------------------------

    fn end(&mut self, act: EndAct) {
        match act.kind {
            Kind::None => {}
            Kind::Block(n) => self.block(n),
            Kind::Heading => self.block(2),
            Kind::List => {
                self.lists.pop();
                self.block(if self.lists.is_empty() { 2 } else { 1 });
            }
            Kind::Item => {
                self.li_marker = None;
                self.block(1);
            }
            Kind::Quote => {
                self.quote = self.quote.saturating_sub(1);
                self.block(2);
            }
            Kind::Pre => self.end_pre(),
            Kind::Frame(kind) => self.close_frame(kind),
            Kind::Table => {
                self.tbl_depth = self.tbl_depth.saturating_sub(1);
                if self.tbl_depth == 0 {
                    self.close_cell();
                    self.close_row();
                    self.tbl = Tbl::default();
                    self.block(2);
                }
            }
            Kind::Row => {
                if self.tbl_depth == 1 && self.tbl.active && !self.tbl.broken {
                    self.close_cell();
                    self.close_row();
                } else {
                    self.block(1);
                }
            }
            Kind::Cell => {
                if self.tbl_depth == 1 && self.tbl.active && !self.tbl.broken {
                    self.close_frames();
                    self.close_cell();
                }
            }
            Kind::Skip(id) => {
                let done = match self.skip.as_mut() {
                    Some(sk) if sk.id == id => {
                        sk.depth = sk.depth.saturating_sub(1);
                        sk.depth == 0
                    }
                    _ => false,
                };
                if done {
                    self.skip = None;
                }
            }
        }
        if act.landmark && self.gate == Gate::Landmark {
            self.landmark_open = self.landmark_open.saturating_sub(1);
            if self.landmark_open == 0 {
                self.block(2);
            }
        }
    }

    fn close_frame(&mut self, kind: FrameKind) {
        let Some(pos) = self.frames.iter().rposition(|f| f.kind == kind) else {
            return;
        };
        while self.frames.len() > pos {
            if let Some(f) = self.frames.pop() {
                if f.written {
                    self.put(&f.close);
                }
            }
        }
    }

    fn end_pre(&mut self) {
        if let Some(pre) = self.pre.take() {
            if pre.fence_open {
                self.put("\n```");
            }
            self.line_open = false;
            self.line_content = false;
            self.want_break = 2;
        }
        self.block(2);
    }

    // ---- tables -----------------------------------------------------------------------------------------

    fn close_cell(&mut self) {
        let Some(cell) = self.tbl.cell.take() else {
            return;
        };
        let row_bytes: usize = self.tbl.row.iter().map(String::len).sum();
        if row_bytes + cell.len() > ROW_BYTES {
            self.tbl.cell = Some(cell);
            self.abandon_table();
            return;
        }
        if self.tbl.row.len() < ROW_CELLS {
            self.tbl.row.push(cell.replace('|', "\\|"));
        }
    }

    fn close_row(&mut self) {
        let row = std::mem::take(&mut self.tbl.row);
        if row.iter().all(String::is_empty) {
            return;
        }
        let mut line = String::from("|");
        for c in &row {
            line.push(' ');
            line.push_str(c);
            line.push_str(" |");
        }
        let header = self.tbl.row_has_th && self.tbl.rows == 0;
        self.tbl.rows += 1;
        self.want_break = self.want_break.max(1);
        self.write_str_content(&line);
        if header {
            let mut sep = String::from("|");
            for _ in &row {
                sep.push_str(" --- |");
            }
            self.want_break = 1;
            self.line_content = false;
            self.line_open = false;
            self.write_str_content(&sep);
        }
        self.want_break = 1;
        self.line_open = false;
        self.line_content = false;
    }

    /// A cell outgrew its buffer: this is a layout table. What is collected is written out as plain lines and
    /// the rest of the table is treated as ordinary blocks.
    fn abandon_table(&mut self) {
        self.tbl.broken = true;
        let mut parts: Vec<String> = std::mem::take(&mut self.tbl.row);
        if let Some(c) = self.tbl.cell.take() {
            parts.push(c);
        }
        for p in parts {
            if !p.is_empty() {
                self.want_break = self.want_break.max(1);
                self.write_str_content(&p);
            }
        }
        self.want_break = 1;
        self.line_open = false;
        self.line_content = false;
    }

    // ---- end of stream ----------------------------------------------------------------------------------

    fn finish(&mut self) {
        // a character reference left unfinished at the end is text
        if !self.tail.is_empty() {
            let t = std::mem::take(&mut self.tail);
            if self.allowed() {
                let d = html_escape::decode_html_entities(&t).into_owned();
                if self.pre.is_some() && !self.in_cell() {
                    self.pre_text(&d);
                } else {
                    self.inline(&d);
                }
            }
        }
        self.close_frames();
        self.close_cell();
        self.close_row();
        if self.pre.is_some() {
            self.end_pre();
        }
        self.close_frames();
    }
}

/// Attribute values arrive as written in the source: character references are still encoded.
fn unescape(s: &str) -> String {
    html_escape::decode_html_entities(s).into_owned()
}

/// Bytes at the end of `s` that may be the start of a character reference still being streamed.
fn incomplete_entity_len(s: &str) -> usize {
    let start = s.len().saturating_sub(ENTITY_TAIL + 1);
    let mut from = start;
    while !s.is_char_boundary(from) {
        from += 1;
    }
    let window = &s[from..];
    let Some(amp) = window.rfind('&') else {
        return 0;
    };
    let rest = &window[amp + 1..];
    if rest.len() <= ENTITY_TAIL && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '#') {
        window.len() - amp
    } else {
        0
    }
}

/// `language-rust` / `lang-rust` from a class list, sanitised for a fence info string.
fn language_of(class: &str) -> Option<String> {
    class.split_whitespace().find_map(|t| {
        let l = t
            .strip_prefix("language-")
            .or_else(|| t.strip_prefix("lang-"))?;
        let l: String = l
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '#' | '.' | '_'))
            .take(20)
            .collect();
        (!l.is_empty()).then_some(l)
    })
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// The tokenizer; its own HTML output is discarded (only the handlers matter).
type Rewriter = HtmlRewriter<'static, fn(&[u8])>;

/// The lol_html driver. Holds the rewriter (fed one slice at a time) and the shared writer state.
pub struct MarkdownConverter {
    rw: Option<Rewriter>,
    md: Arc<Mutex<Md>>,
    failed: bool,
    finished: bool,
    /// Raw-stream attribute counter, fed before the rewriter (see `tagscan`).
    scan: Scan,
}

impl MarkdownConverter {
    /// `base` is the URL the page came from (relative links and images are resolved against it).
    #[must_use]
    pub fn new(base: Option<Url>) -> Self {
        let md = Arc::new(Mutex::new(Md::new(base)));
        let on_element = {
            let md = Arc::clone(&md);
            element!("*", move |el: &mut Element<'_, '_>| {
                let name = el.tag_name();
                let void = is_void(&name) || !el.can_have_content();
                let at = Attrs::read(el);
                let act = lock(&md).start(&name, void, &at);
                if let (Some(act), false) = (act, void) {
                    let st = Arc::clone(&md);
                    let counted = !matches!(act.kind, Kind::Skip(_));
                    if counted {
                        lock(&md).open += 1;
                    }
                    el.on_end_tag(end_tag!(move |_e| {
                        let mut g = lock(&st);
                        if counted {
                            g.open = g.open.saturating_sub(1);
                        }
                        g.end(act);
                        Ok(())
                    }))?;
                }
                Ok(())
            })
        };
        let on_text = {
            let md = Arc::clone(&md);
            doc_text!(move |t| {
                let decode = matches!(t.text_type(), TextType::Data | TextType::RCData);
                lock(&md).text(t.as_str(), t.last_in_text_node(), decode);
                Ok(())
            })
        };
        let sink: fn(&[u8]) = |_| {};
        let rw = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![on_element],
                document_content_handlers: vec![on_text],
                memory_settings: MemorySettings {
                    max_allowed_memory_usage: REWRITER_LIMIT,
                    ..MemorySettings::new()
                },
                strict: false,
                ..Settings::new_send()
            },
            sink,
        );
        Self {
            rw: Some(rw),
            md,
            failed: false,
            finished: false,
            scan: Scan::new(),
        }
    }

    fn fail(&mut self, e: ConvertError) -> Result<(), ConvertError> {
        self.rw = None;
        self.failed = true;
        Err(e)
    }

    fn drain(&mut self, out: &mut dyn FnMut(&str), last: bool) -> Result<(), ConvertError> {
        let mut g = lock(&self.md);
        if g.overflow {
            drop(g);
            return self.fail(ConvertError::Limit);
        }
        if g.gate == Gate::Hold {
            if !last {
                return Ok(());
            }
            g.gate = Gate::Whole;
        }
        if !g.out.is_empty() {
            out(&g.out);
            g.out.clear();
            if g.out.capacity() > 1024 * 1024 {
                g.out.shrink_to(64 * 1024);
            }
        }
        Ok(())
    }
}

fn map_err(e: &lol_html::errors::RewritingError) -> ConvertError {
    match e {
        lol_html::errors::RewritingError::MemoryLimitExceeded(_) => ConvertError::Limit,
        _ => ConvertError::Malformed,
    }
}

impl Converter for MarkdownConverter {
    fn push(&mut self, text: &str, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        if self.failed {
            return Err(ConvertError::Malformed);
        }
        if self.finished {
            return Err(ConvertError::Malformed);
        }
        // Before the rewriter sees the bytes: a tag with a hostile number of attributes costs ~140 bytes each
        // inside `lol_html` and is not covered by its memory limit.
        if !self.scan.feed(text.as_bytes()) {
            return self.fail(ConvertError::Limit);
        }
        let Some(rw) = self.rw.as_mut() else {
            return Err(ConvertError::Malformed);
        };
        if let Err(e) = rw.write(text.as_bytes()) {
            return self.fail(map_err(&e));
        }
        self.drain(out, false)
    }

    fn finish(&mut self, out: &mut dyn FnMut(&str)) -> Result<(), ConvertError> {
        if self.failed || self.finished {
            return Err(ConvertError::Malformed);
        }
        self.finished = true;
        if let Some(rw) = self.rw.take() {
            if let Err(e) = rw.end() {
                return self.fail(map_err(&e));
            }
        }
        lock(&self.md).finish();
        self.drain(out, true)
    }
}

#[cfg(test)]
mod tests;
