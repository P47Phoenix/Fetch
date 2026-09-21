//! Character window over the output text stream (A-5, ADR-006, architecture 5.2). The `Window` sink keeps the
//! characters `[start, start + take)` of everything pushed through it, then looks for ONE confirming character past
//! the window (that is how "more content exists" is known without reading on) and raises a stop flag so the fetch
//! pump can drop the response. Memory is bounded by `take`; a "character" is one Unicode scalar value, so the split
//! is always on a character boundary. Pure logic, no I/O.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What a finished window holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOutput {
    /// The kept characters.
    pub text: String,
    /// The `start_index` asked for.
    pub start: u64,
    /// Characters kept (`text.chars().count()`).
    pub returned: u64,
    /// A character beyond the window was seen: more content exists.
    pub more: bool,
    /// Total characters in the stream, known only when the stream was read to its end (`more` is false).
    pub total: Option<u64>,
}

/// Keeps the characters `[skip, skip + take)` of the text pushed through it.
#[derive(Debug)]
pub struct Window {
    start: u64,
    skip: u64,
    take: u64,
    out: String,
    taken: u64,
    seen: u64,
    more: bool,
    stop: Arc<AtomicBool>,
}

fn count(s: &str) -> u64 {
    let n = if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    };
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Byte offset of the `n`th character of `s` (its length when `s` has fewer).
fn offset(s: &str, n: u64) -> usize {
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    if s.is_ascii() {
        return n.min(s.len());
    }
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

impl Window {
    #[must_use]
    pub fn new(start_index: u64, max_length: u64) -> Self {
        Self {
            start: start_index,
            skip: start_index,
            take: max_length,
            out: String::new(),
            taken: 0,
            seen: 0,
            more: false,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The flag raised as soon as the window is complete and a character beyond it was seen. The pump polls it
    /// and stops reading; it stays a plain shared flag so the sink can keep a mutable borrow of the window.
    #[must_use]
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    /// Feed the next slice of the stream. After the window is complete and confirmed this does nothing.
    pub fn push(&mut self, text: &str) {
        if self.more || text.is_empty() {
            return;
        }
        let mut rest = text;
        if self.skip > 0 {
            let n = count(rest);
            if n <= self.skip {
                self.skip -= n;
                self.seen = self.seen.saturating_add(n);
                return;
            }
            self.seen = self.seen.saturating_add(self.skip);
            rest = &rest[offset(rest, self.skip)..];
            self.skip = 0;
        }
        if self.taken < self.take {
            let room = self.take - self.taken;
            let n = count(rest);
            if n <= room {
                self.out.push_str(rest);
                self.taken += n;
                self.seen = self.seen.saturating_add(n);
                return;
            }
            let cut = offset(rest, room);
            self.out.push_str(&rest[..cut]);
            self.taken = self.take;
            self.seen = self.seen.saturating_add(room);
            rest = &rest[cut..];
        }
        if !rest.is_empty() {
            self.more = true;
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    /// End of the stream (or of the reading): the window's contents and what is known about the total.
    #[must_use]
    pub fn finish(self) -> WindowOutput {
        WindowOutput {
            returned: self.taken,
            total: (!self.more).then_some(self.seen),
            text: self.out,
            start: self.start,
            more: self.more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Window;

    fn run(start: u64, len: u64, parts: &[&str]) -> super::WindowOutput {
        let mut w = Window::new(start, len);
        for p in parts {
            w.push(p);
        }
        w.finish()
    }

    #[test]
    fn window_counts_characters_across_chunk_boundaries() {
        assert_eq!(run(0, 5, &["hello world"]).text, "hello");
        assert_eq!(run(2, 5, &["he", "llo w", "orld"]).text, "llo w");
        assert_eq!(run(1, 3, &["h\u{e9}", "llo"]).text, "\u{e9}ll");
        assert_eq!(run(0, 100, &["short"]).text, "short");
        assert_eq!(run(50, 5, &["short"]).text, "");
        assert_eq!(run(0, 0, &["anything"]).text, "");
    }

    #[test]
    fn more_needs_one_confirming_character_and_raises_the_stop_flag() {
        let mut w = Window::new(0, 3);
        let stop = w.stop_flag();
        w.push("abc");
        assert!(
            !stop.load(std::sync::atomic::Ordering::Relaxed),
            "exactly full is not yet confirmed"
        );
        w.push("d");
        assert!(stop.load(std::sync::atomic::Ordering::Relaxed));
        let o = w.finish();
        assert_eq!(
            (o.text.as_str(), o.more, o.total, o.returned),
            ("abc", true, None, 3)
        );
    }

    #[test]
    fn total_is_known_only_when_the_end_was_reached() {
        let o = run(0, 5, &["abc"]);
        assert_eq!((o.more, o.total), (false, Some(3)));
        let o = run(0, 3, &["abc"]);
        assert_eq!((o.more, o.total, o.text.as_str()), (false, Some(3), "abc"));
        let o = run(10, 5, &["abc", "de"]);
        assert_eq!((o.more, o.total, o.text.as_str()), (false, Some(5), ""));
        let o = run(1, 2, &["a\u{1F680}", "b", "c"]);
        assert_eq!(
            (o.text.as_str(), o.more, o.total),
            ("\u{1F680}b", true, None)
        );
    }

    #[test]
    fn every_split_of_multibyte_text_gives_the_same_window() {
        let text = "a\u{e9}\u{20ac}\u{1F680}b\u{e9}c\u{20ac}d";
        let want = run(2, 4, &[text]);
        assert_eq!(want.text, "\u{20ac}\u{1F680}b\u{e9}");
        let cuts: Vec<usize> = (0..=text.len())
            .filter(|i| text.is_char_boundary(*i))
            .collect();
        for &i in &cuts {
            assert_eq!(run(2, 4, &[&text[..i], &text[i..]]), want, "cut {i}");
        }
    }

    #[test]
    fn sequential_windows_reproduce_the_text_with_no_gap_or_overlap() {
        let text: String = (0..200)
            .map(|i| char::from_u32(0x61 + i % 3 * 0x100).unwrap_or('x'))
            .collect();
        let mid = text.char_indices().nth(100).map_or(0, |(i, _)| i);
        for len in [1u64, 7, 50, 199, 200, 201] {
            let mut got = String::new();
            let mut start = 0;
            loop {
                let o = run(start, len, &[&text[..mid], &text[mid..]]);
                got.push_str(&o.text);
                if !o.more {
                    break;
                }
                start += o.returned;
            }
            assert_eq!(got, text, "max_length {len}");
        }
    }

    #[test]
    fn huge_offsets_do_not_overflow() {
        let o = run(u64::MAX, u64::MAX, &["abc"]);
        assert_eq!((o.text.as_str(), o.total), ("", Some(3)));
        let o = run(0, u64::MAX, &["abc"]);
        assert_eq!((o.text.as_str(), o.more), ("abc", false));
    }
}
