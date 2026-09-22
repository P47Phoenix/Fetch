//! Bounded body pipeline (ADR-004): wire chunk -> (gzip) -> decompressed byte cap -> UTF-8 stream decoder ->
//! text sink. Everything is processed one bounded step at a time; nothing accumulates the body. Pure logic, no I/O.

use flate2::write::GzDecoder;
use std::io::{self, Write};

/// Largest slice fed downstream at once. Wire chunks bigger than this are re-sliced; gzip output steps are at
/// most flate2's 32 KiB internal buffer, so both are within the ADR-004 bound of 64 KiB.
pub const STEP: usize = 64 * 1024;

/// Why feeding the pipeline stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    /// The decompressed (or identity) byte count would exceed the cap.
    TooLarge,
    /// The gzip stream is corrupt or truncated.
    Corrupt,
}

/// Streaming UTF-8 decoder with replacement: invalid sequences become U+FFFD (maximal-subpart rule), a
/// multi-byte sequence split across chunks is carried (at most 3 bytes), nothing else is buffered.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    carry: [u8; 4],
    len: usize,
}

impl Utf8Stream {
    /// Decode `data`, calling `emit` with valid text slices (and `"\u{FFFD}"` for each invalid sequence).
    pub fn push(&mut self, mut data: &[u8], emit: &mut dyn FnMut(&str)) {
        while self.len > 0 && !data.is_empty() {
            self.carry[self.len] = data[0];
            match std::str::from_utf8(&self.carry[..=self.len]) {
                Ok(s) => {
                    emit(s);
                    self.len = 0;
                    data = &data[1..];
                }
                Err(e) if e.error_len().is_none() => {
                    self.len += 1;
                    data = &data[1..];
                }
                Err(_) => {
                    // The carried prefix is a maximal invalid subpart; the new byte starts afresh.
                    emit("\u{FFFD}");
                    self.len = 0;
                }
            }
        }
        while !data.is_empty() {
            match std::str::from_utf8(data) {
                Ok(s) => {
                    emit(s);
                    return;
                }
                Err(e) => {
                    let (valid, rest) = data.split_at(e.valid_up_to());
                    // Bytes before `valid_up_to` are valid UTF-8 by construction.
                    if let Ok(v) = std::str::from_utf8(valid) {
                        if !v.is_empty() {
                            emit(v);
                        }
                    }
                    match e.error_len() {
                        Some(n) => {
                            emit("\u{FFFD}");
                            data = &rest[n..];
                        }
                        None => {
                            self.carry[..rest.len()].copy_from_slice(rest);
                            self.len = rest.len();
                            return;
                        }
                    }
                }
            }
        }
    }

    /// End of input: an incomplete trailing sequence becomes one replacement character.
    pub fn finish(&mut self, emit: &mut dyn FnMut(&str)) {
        if self.len > 0 {
            emit("\u{FFFD}");
            self.len = 0;
        }
    }
}

/// Final stage: counts bytes against the cap, then decodes to text for the sink.
struct Out<'a> {
    sink: &'a mut (dyn FnMut(&str) + Send),
    utf8: Utf8Stream,
    decoded: u64,
    cap: u64,
    overflow: bool,
}

impl Write for Out<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = b.len() as u64;
        if self.decoded.saturating_add(n) > self.cap {
            self.overflow = true;
            return Err(io::Error::other("decompressed size cap exceeded"));
        }
        self.decoded += n;
        self.utf8.push(b, &mut *self.sink);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

enum Stage<'a> {
    Identity(Out<'a>),
    /// `finished` is set once the first gzip member has ended; later bytes are trailing data, ignored
    /// (already counted against the wire cap) and never decoded, which also defeats multi-member bombs.
    Gzip {
        dec: Box<GzDecoder<Out<'a>>>,
        finished: bool,
    },
}

/// Decoder for one response body.
pub struct Pipeline<'a> {
    stage: Stage<'a>,
    fed: u64,
}

impl<'a> Pipeline<'a> {
    /// `gzip` selects a single-member gzip decode; `cap` bounds the decompressed byte count.
    pub fn new(gzip: bool, cap: u64, sink: &'a mut (dyn FnMut(&str) + Send)) -> Self {
        let out = Out {
            sink,
            utf8: Utf8Stream::default(),
            decoded: 0,
            cap,
            overflow: false,
        };
        let stage = if gzip {
            Stage::Gzip {
                dec: Box::new(GzDecoder::new(out)),
                finished: false,
            }
        } else {
            Stage::Identity(out)
        };
        Self { stage, fed: 0 }
    }

    /// Feed one wire chunk (any size; it is re-sliced to [`STEP`]).
    ///
    /// # Errors
    /// [`BodyError::TooLarge`] when the decompressed cap is hit, [`BodyError::Corrupt`] on a bad gzip stream.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), BodyError> {
        for piece in chunk.chunks(STEP) {
            self.fed += piece.len() as u64;
            match &mut self.stage {
                Stage::Identity(out) => out.write_all(piece).map_err(|_| BodyError::TooLarge)?,
                Stage::Gzip { dec, finished } => {
                    let mut rest = piece;
                    while !*finished && !rest.is_empty() {
                        match dec.write(rest) {
                            Ok(0) => *finished = true,
                            Ok(n) => rest = &rest[n..],
                            Err(_) if dec.get_ref().overflow => return Err(BodyError::TooLarge),
                            Err(_) => return Err(BodyError::Corrupt),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// End of body: flushes the decoder and the UTF-8 carry.
    ///
    /// # Errors
    /// [`BodyError::Corrupt`] when a gzip body ended before the gzip stream did.
    pub fn finish(mut self) -> Result<(), BodyError> {
        match &mut self.stage {
            Stage::Identity(out) => out.utf8.finish(&mut *out.sink),
            Stage::Gzip { dec, finished } => {
                if !*finished && self.fed > 0 {
                    if dec.header().is_none() {
                        return Err(BodyError::Corrupt); // the gzip header never completed
                    }
                    dec.try_finish().map_err(|_| BodyError::Corrupt)?;
                }
                let out = dec.get_mut();
                out.utf8.finish(&mut *out.sink);
            }
        }
        Ok(())
    }
}

/// Byte-accumulating counterpart to [`Pipeline`] (A-8): decompresses and cap-enforces exactly as [`Pipeline`]
/// does, but accumulates the raw decoded bytes instead of decoding them as UTF-8. Used only while the response
/// charset is not yet known to be UTF-8: a short lookahead sniffs a `<meta charset>` tag from [`RawPipeline::buffered`]
/// without ending the stream, and the rare non-UTF-8 case decodes the full (still capped) body with `encoding_rs`
/// once [`RawPipeline::finish`] returns it. Deliberately a separate, small state machine rather than a generic
/// [`Pipeline`] (duplicates the identity/gzip dispatch) to keep the common streaming UTF-8 path untouched.
pub struct RawPipeline {
    stage: RawStage,
    fed: u64,
}

enum RawStage {
    Identity(RawOut),
    Gzip {
        dec: Box<GzDecoder<RawOut>>,
        finished: bool,
    },
}

struct RawOut {
    buf: Vec<u8>,
    cap: u64,
    overflow: bool,
}

impl Write for RawOut {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = b.len() as u64;
        if (self.buf.len() as u64).saturating_add(n) > self.cap {
            self.overflow = true;
            return Err(io::Error::other("decompressed size cap exceeded"));
        }
        self.buf.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl RawPipeline {
    /// `gzip` selects a single-member gzip decode; `cap` bounds the accumulated byte count.
    #[must_use]
    pub fn new(gzip: bool, cap: u64) -> Self {
        let out = RawOut {
            buf: Vec::new(),
            cap,
            overflow: false,
        };
        let stage = if gzip {
            RawStage::Gzip {
                dec: Box::new(GzDecoder::new(out)),
                finished: false,
            }
        } else {
            RawStage::Identity(out)
        };
        Self { stage, fed: 0 }
    }

    /// Feed one wire chunk (any size; it is re-sliced to [`STEP`]).
    ///
    /// # Errors
    /// [`BodyError::TooLarge`] when the cap is hit, [`BodyError::Corrupt`] on a bad gzip stream.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), BodyError> {
        for piece in chunk.chunks(STEP) {
            self.fed += piece.len() as u64;
            match &mut self.stage {
                RawStage::Identity(out) => out.write_all(piece).map_err(|_| BodyError::TooLarge)?,
                RawStage::Gzip { dec, finished } => {
                    let mut rest = piece;
                    while !*finished && !rest.is_empty() {
                        match dec.write(rest) {
                            Ok(0) => *finished = true,
                            Ok(n) => rest = &rest[n..],
                            Err(_) if dec.get_ref().overflow => return Err(BodyError::TooLarge),
                            Err(_) => return Err(BodyError::Corrupt),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The decoded bytes accumulated so far, without ending the stream (A-8 charset sniffing peek).
    #[must_use]
    pub fn buffered(&self) -> &[u8] {
        match &self.stage {
            RawStage::Identity(out) => &out.buf,
            RawStage::Gzip { dec, .. } => &dec.get_ref().buf,
        }
    }

    /// End of body: returns the accumulated decoded bytes.
    ///
    /// # Errors
    /// [`BodyError::Corrupt`] when a gzip body ended before the gzip stream did.
    pub fn finish(mut self) -> Result<Vec<u8>, BodyError> {
        match &mut self.stage {
            RawStage::Identity(out) => Ok(std::mem::take(&mut out.buf)),
            RawStage::Gzip { dec, finished } => {
                if !*finished && self.fed > 0 {
                    if dec.header().is_none() {
                        return Err(BodyError::Corrupt); // the gzip header never completed
                    }
                    dec.try_finish().map_err(|_| BodyError::Corrupt)?;
                }
                Ok(std::mem::take(&mut dec.get_mut().buf))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BodyError, Pipeline, RawPipeline, Utf8Stream, STEP};
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn utf8(chunks: &[&[u8]]) -> String {
        let mut d = Utf8Stream::default();
        let mut out = String::new();
        for c in chunks {
            d.push(c, &mut |s| out.push_str(s));
        }
        d.finish(&mut |s| out.push_str(s));
        out
    }

    #[test]
    fn utf8_split_across_chunks() {
        let euro = "€".as_bytes(); // 3 bytes
        assert_eq!(utf8(&[&euro[..1], &euro[1..2], &euro[2..]]), "€");
        assert_eq!(utf8(&[b"a", &euro[..2], &euro[2..], b"b"]), "a€b");
        let rocket = "🚀".as_bytes();
        assert_eq!(utf8(&[&rocket[..3], &rocket[3..]]), "🚀");
    }

    #[test]
    fn utf8_replaces_invalid_and_truncated() {
        assert_eq!(utf8(&[b"a\xffb"]), "a\u{FFFD}b");
        // Truncated sequence carried, then an ASCII byte: one replacement, then the byte.
        assert_eq!(utf8(&[b"\xe2\x82", b"x"]), "\u{FFFD}x");
        // Incomplete at end of input.
        assert_eq!(utf8(&[b"ok\xe2\x82"]), "ok\u{FFFD}");
        // Lone continuation bytes: one replacement each.
        assert_eq!(utf8(&[b"\x80\x80"]), "\u{FFFD}\u{FFFD}");
        // Same answer as std's lossy conversion for a mixed input, whatever the chunking.
        let data: Vec<u8> = b"h\xc3\xa9llo \xf0\x9f\x9a\x80 \xe2\x82 \xc0\xaf end\xf0\x9f".to_vec();
        let want = String::from_utf8_lossy(&data).into_owned();
        for split in 0..=data.len() {
            assert_eq!(
                utf8(&[&data[..split], &data[split..]]),
                want,
                "split {split}"
            );
        }
    }

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn run(gzip: bool, cap: u64, chunks: &[&[u8]]) -> (Result<(), BodyError>, String, usize) {
        let mut out = String::new();
        let mut biggest = 0usize;
        let r = {
            let mut sink = |s: &str| {
                biggest = biggest.max(s.len());
                out.push_str(s);
            };
            let mut p = Pipeline::new(gzip, cap, &mut sink);
            let mut r = Ok(());
            for c in chunks {
                r = p.feed(c);
                if r.is_err() {
                    break;
                }
            }
            if r.is_ok() {
                r = p.finish();
            }
            r
        };
        (r, out, biggest)
    }

    #[test]
    fn identity_cap_is_exact() {
        assert_eq!(run(false, 5, &[b"hello"]).1, "hello");
        assert_eq!(run(false, 5, &[b"hel", b"lo!"]).0, Err(BodyError::TooLarge));
    }

    #[test]
    fn gzip_roundtrip_and_trailing_garbage_and_multimember() {
        let z = gz(b"hello world");
        assert_eq!(run(true, 100, &[&z]).1, "hello world");
        let mut split: Vec<&[u8]> = z.chunks(3).collect();
        assert_eq!(run(true, 100, &split).1, "hello world");
        split.clear();

        let mut garbage = z.clone();
        garbage.extend_from_slice(b"\x00\x01garbage-after-member");
        let (r, text, _) = run(true, 100, &[&garbage]);
        assert_eq!((r, text.as_str()), (Ok(()), "hello world"));

        let mut multi = gz(b"first ");
        multi.extend_from_slice(&gz(b"second"));
        let (r, text, _) = run(true, 100, &[&multi]);
        assert_eq!((r, text.as_str()), (Ok(()), "first "));
    }

    #[test]
    fn gzip_bomb_is_capped_in_bounded_steps() {
        let bomb = gz(&vec![b'a'; 20 * 1024 * 1024]);
        assert!(bomb.len() < 40 * 1024, "fixture must be a real bomb");
        let (r, text, biggest) = run(true, 1024 * 1024, &[&bomb]);
        assert_eq!(r, Err(BodyError::TooLarge));
        assert!(text.len() <= 1024 * 1024);
        assert!(biggest <= STEP, "output step {biggest} exceeds 64 KiB");
    }

    #[test]
    fn gzip_corrupt_and_truncated_fail() {
        assert_eq!(
            run(true, 100, &[b"this is not gzip data at all"]).0,
            Err(BodyError::Corrupt)
        );
        let z = gz(&vec![b'x'; 10_000]);
        assert_eq!(
            run(true, 100_000, &[&z[..z.len() / 2]]).0,
            Err(BodyError::Corrupt)
        );
        // Header only.
        assert_eq!(run(true, 100, &[&z[..10]]).0, Err(BodyError::Corrupt));
    }

    #[test]
    fn large_wire_chunk_is_resliced() {
        let data = vec![b'z'; 300 * 1024];
        let (r, text, biggest) = run(false, 1 << 30, &[&data]);
        assert_eq!((r, text.len()), (Ok(()), data.len()));
        assert!(biggest <= STEP);
    }

    // ---- RawPipeline (A-8) ---------------------------------------------------------------------------

    #[test]
    fn raw_pipeline_identity_roundtrips_and_caps() {
        let mut p = RawPipeline::new(false, 5);
        p.feed(b"hel").unwrap();
        assert_eq!(p.buffered(), b"hel");
        p.feed(b"lo").unwrap();
        assert_eq!(p.finish().unwrap(), b"hello");

        let mut p = RawPipeline::new(false, 4);
        assert_eq!(p.feed(b"hello"), Err(BodyError::TooLarge));
    }

    #[test]
    fn raw_pipeline_gzip_roundtrips_and_buffered_peek_works_mid_stream() {
        let z = gz(b"hello world");
        let mut p = RawPipeline::new(true, 100);
        for piece in z.chunks(3) {
            p.feed(piece).unwrap();
        }
        assert_eq!(p.finish().unwrap(), b"hello world");

        let z = gz(b"peekable prefix here");
        let mut p = RawPipeline::new(true, 100);
        p.feed(&z[..z.len() / 2]).unwrap();
        // whatever the decoder produced so far is a valid prefix of the final text
        let peek = p.buffered().to_vec();
        p.feed(&z[z.len() / 2..]).unwrap();
        let whole = p.finish().unwrap();
        assert!(whole.starts_with(&peek));
        assert_eq!(whole, b"peekable prefix here");
    }

    #[test]
    fn raw_pipeline_corrupt_gzip_fails() {
        let mut p = RawPipeline::new(true, 100);
        assert_eq!(
            p.feed(b"this is not gzip data at all"),
            Err(BodyError::Corrupt)
        );
    }
}
