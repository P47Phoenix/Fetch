//! Attribute-count guard (fix-pass 1, architect B2). `lol_html` keeps a 48-byte outline (plus doubling growth)
//! for every attribute of the start tag being lexed, and none of that is counted by its memory limit: one tag of
//! about 1.6 MiB of `a a a ...` cost about 114 MB of RSS. The guard counts attributes on the raw stream, BEFORE
//! `lol_html` sees the bytes, and reports when any one tag exceeds [`ATTR_CAP`], so the converter fails closed
//! (`converter_limit`) instead of materialising them.
//!
//! Fidelity: the tag grammar below mirrors `lol_html`'s lexer (whitespace = space, LF, CR, TAB, FF; a quoted value
//! is followed directly by a new attribute name). Whether a `<` really starts a tag depends on the tokenizer's
//! context (comments, `<script>`, `<svg>` foreign content, ...), which this guard does not model. Instead it
//! over-approximates: a candidate tag is started at EVERY `<` followed by an ASCII letter, whatever the context,
//! and the candidates are kept as one maximum attribute count per grammar state (the future of a candidate
//! depends only on its state, so keeping the maximum per state loses nothing). A real tag always begins at some
//! `<letter`, and the candidate that starts there follows it exactly, so the guard never counts fewer attributes
//! than the lexer sees; the price is that text such as `a <b` followed by thousands of words and no `>` is also
//! refused (fail closed, `raw=true` still works). Work per byte is bounded (8 states), memory is constant.

/// Attributes one tag may have. A real tag has a few dozen at most; each costs the lexer ~140 bytes.
pub const ATTR_CAP: u32 = 1024;

// Grammar states (index into `Scan::cnt`).
const TN: usize = 0; // tag name
const BA: usize = 1; // before attribute name (also the self-closing `/` state)
const AN: usize = 2; // attribute name
const AF: usize = 3; // after attribute name
const BV: usize = 4; // before attribute value
const DQ: usize = 5; // double-quoted value
const SQ: usize = 6; // single-quoted value
const UQ: usize = 7; // unquoted value
const N: usize = 8;
const NONE: i64 = -1;

const fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t' | 0x0c)
}

/// Incremental scanner; feed it the same bytes, in the same order, that go to the rewriter.
pub struct Scan {
    /// Largest attribute count among candidate tags currently in each state, or [`NONE`].
    cnt: [i64; N],
    /// The previous byte was `<`.
    after_lt: bool,
    /// Largest count any candidate reached (for the tests and diagnostics).
    peak: i64,
}

impl Default for Scan {
    fn default() -> Self {
        Self::new()
    }
}

impl Scan {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cnt: [NONE; N],
            after_lt: false,
            peak: 0,
        }
    }

    /// The largest attribute count any candidate tag has reached so far.
    #[cfg(test)]
    #[must_use]
    pub fn peak(&self) -> u32 {
        u32::try_from(self.peak).unwrap_or(u32::MAX)
    }

    fn live(&self) -> bool {
        self.cnt.iter().any(|&c| c != NONE)
    }

    /// Consume `bytes`. Returns `false` as soon as some candidate tag exceeds [`ATTR_CAP`] attributes.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        let mut i = 0;
        while i < bytes.len() {
            if !self.live() && !self.after_lt {
                // no candidate: skip to the next '<'
                match bytes
                    .get(i..)
                    .and_then(|r| r.iter().position(|&b| b == b'<'))
                {
                    Some(p) => {
                        i += p + 1;
                        self.after_lt = true;
                        continue;
                    }
                    None => return true,
                }
            }
            let Some(&b) = bytes.get(i) else { break };
            i += 1;
            if !self.step(b) {
                return false;
            }
        }
        true
    }

    fn step(&mut self, b: u8) -> bool {
        let mut next = [NONE; N];
        let mut put = |s: usize, c: i64| {
            if let Some(slot) = next.get_mut(s) {
                if c > *slot {
                    *slot = c;
                }
            }
        };
        let mut over = false;
        let mut peak = self.peak;
        for (s, &c) in self.cnt.iter().enumerate() {
            if c == NONE {
                continue;
            }
            // a new attribute starts: count it
            let mut start = |to: usize, put: &mut dyn FnMut(usize, i64)| {
                peak = peak.max(c + 1);
                if c + 1 > i64::from(ATTR_CAP) {
                    over = true;
                }
                put(to, c + 1);
            };
            match s {
                TN => {
                    if is_ws(b) || b == b'/' {
                        put(BA, c);
                    } else if b != b'>' {
                        put(TN, c);
                    }
                }
                BA => {
                    if is_ws(b) || b == b'/' {
                        put(BA, c);
                    } else if b != b'>' {
                        start(AN, &mut put);
                    }
                }
                AN => {
                    if is_ws(b) {
                        put(AF, c);
                    } else if b == b'=' {
                        put(BV, c);
                    } else if b == b'/' {
                        put(BA, c);
                    } else if b != b'>' {
                        put(AN, c);
                    }
                }
                AF => {
                    if is_ws(b) {
                        put(AF, c);
                    } else if b == b'/' {
                        put(BA, c);
                    } else if b == b'=' {
                        put(BV, c);
                    } else if b != b'>' {
                        start(AN, &mut put);
                    }
                }
                BV => {
                    if is_ws(b) {
                        put(BV, c);
                    } else if b == b'"' {
                        put(DQ, c);
                    } else if b == b'\'' {
                        put(SQ, c);
                    } else if b != b'>' {
                        put(UQ, c);
                    }
                }
                DQ => put(if b == b'"' { BA } else { DQ }, c),
                SQ => put(if b == b'\'' { BA } else { SQ }, c),
                _ => {
                    // UQ
                    if is_ws(b) {
                        put(BA, c);
                    } else if b != b'>' {
                        put(UQ, c);
                    }
                }
            }
        }
        if self.after_lt && b.is_ascii_alphabetic() {
            put(TN, 0);
        }
        self.after_lt = b == b'<';
        self.cnt = next;
        self.peak = peak;
        !over
    }
}

#[cfg(test)]
mod tests;
