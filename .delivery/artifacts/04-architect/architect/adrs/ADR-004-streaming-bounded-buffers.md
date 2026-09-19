# ADR-004: Streaming, bounded-buffer pipeline (decompression, charset, raw path)

Status: Accepted for the streaming pipeline. The rule 'size error vs early stop' (Caps bullet: Window complete before the cap => success) is ACCEPTED (Product Owner, 2026-09-19; architecture.md 6.4); EPICS E-4, A-3 and PRD FR-07 are amended to match.
Date: 2026-09-19

## Context
Memory must be bounded by configuration, not by response size (FR-16, NFR-12). Spike: buffered pipeline costs ~10 MB for a 5 MB body (Vec + String, growth by doubling), 56 MB with a DOM converter; streaming with early stop: 6.1 MB / 8.7 MB deep; the buffered raw path was 11.1 MB and would not scale to the 50 MB case. Not exercised in the spike: gzip/br, redirects, non-UTF-8 charsets, TLS.

## Options
1. Buffer whole body (up to cap), then convert and paginate. Simple; ~2x body + converter. Rejected (measured).
2. Streaming push pipeline with per-stage bounded buffers and early stop (chosen).
3. Spill to temp file for the raw path. Adds disk state and cleanup; unnecessary because windowing needs no random access.

## Decision
Option 2. Stages, in order, all processing one chunk at a time:

`wire chunk -> wire byte counter -> decompress (flate2) -> decompressed byte counter -> charset decode (UTF-8) -> Converter/text -> Window sink -> stop or continue`

Budgets (design allocations; sum table in architecture.md 5.1):
- Wire chunk processed immediately; no accumulation; <= 64 KiB assumed, enforced by re-slicing larger chunks.
- Caps: `FETCH_MAX_BYTES` (default 5 MiB) counts BOTH wire bytes and decompressed bytes. `Content-Length` > cap aborts before reading (A-3). Hitting the cap before the Window is complete -> `too_large`; Window complete first -> success. This second half is ACCEPTED (Product Owner, 2026-09-19; architecture.md 6.4).
- Overall deadline `FETCH_TIMEOUT_MS` (default 15 s) wraps DNS, connect, TLS, headers, all redirect hops and body streaming (slow-drip defence).
- Early stop: as soon as the Window holds `max_length` chars after `start_index` plus one confirming char, drop the response.

Decompression (bombs):
- Advertise `Accept-Encoding: gzip` only. No brotli (window up to 16 MiB would eat 40% of the peak budget by itself), no zstd, no deflate.
- Decoding is done by us: reqwest's `gzip` feature is OFF (it would hide wire bytes and may not reject stacked or unknown encodings). The `Content-Encoding` header is checked BEFORE decode: accept absent/`identity` or a single `gzip`; anything else, or stacked encodings, -> `unsupported_encoding`. `flate2` (pure-Rust backend) decodes only the first gzip member; trailing bytes after it are ignored (still counted against the wire cap), never decoded, which also defeats multi-member bombs. Fixtures for stacked, unknown, multi-member and trailing-garbage cases are required in A-3.
- Inflate in bounded steps (<= 64 KiB output per step); decompressed byte counter enforces the same cap as the wire, so a 10 KB -> 5 MiB bomb costs at most cap bytes of CPU work and never more memory than one step. Because flate2 is driven with an explicit output buffer of <= 64 KiB, chunk size is bounded by construction; assert it in A-3 with a bomb fixture.
- Optional CPU guard: none beyond cap + deadline (both bound work).

Charset:
- Resolution order: BOM > `Content-Type` charset > `<meta charset>` / `<meta http-equiv>` in the first 1024 bytes (HTML only) > UTF-8. Hold only the first <= 4 KiB until the encoding is chosen.
- Non-HTML text: BOM > header > UTF-8.
- Decode to UTF-8 with `encoding_rs` streaming `Decoder` (carries partial multibyte sequences across chunks; invalid sequences replaced with U+FFFD, never fatal; unknown labels fall back to UTF-8). The converter and Window always see UTF-8, so char counting for pagination is well defined (ADR-006). lol_html is fed UTF-8.
- Decoder working buffer 16 KiB.
- Interaction with FR-09/A-8: the UTF-8-with-replacement decoder is required from A-3/A-4 (needed for char counting); non-UTF-8 charset support completes in A-8.

Raw path cap:
- `raw=true` and text/json/xml go through `convert::text` and the same Window; no body buffering. Memory is the same as the HTML path minus lol_html. Same caps. Large single-line JSON is safe because processing is chunked; the Window stores only the requested slice.
- Raw output can still contain a huge response slice only up to `max_length` (hard cap 100,000 chars by default, <= 0.4 MB).

Content-type gate happens at headers, before reading the body, so binary bodies are never read.

## Consequences
+ Memory independent of body size; 50 MB and 5 MB runs cost the same by construction.
+ Deep pages cost time O(start_index) not memory.
- Total content length is unknown for non-final pages (ADR-006).
- Charset prescan delays the first decode until up to 1024 bytes arrive (negligible).
- Gzip-only limits bandwidth savings vs br; irrelevant for memory goals and most servers serve gzip.
- Some servers send brotli regardless of Accept-Encoding: handled by `unsupported_encoding`.
- Per-chunk synchronous conversion on a single-threaded runtime: chunk size bound keeps latency low; concurrent fetches interleave at await points only.
- Not measured: gzip and TLS overhead; real emitter.

## What ARM data would flip it
- Enable brotli only if native aarch64 peak with a 5 MB brotli page (window <= 16 MiB, or the decoder limited to a smaller window via lgwin cap) stays <= 25 MB and a real-URL set shows meaningful brotli-only failures.
- Budgets (architecture.md 5.1): worst-case per-fetch delta W = 6.3 MiB, default concurrency 3, so 10 + 3 x 6.3 = 28.9 MiB versus the 40 gate (labelled budget, not measurement). Flip rule using MEASURED native aarch64 numbers: choose the largest n such that measured_idle + n x measured_worst_delta <= 32 (keeps >= 8 of the 40 as headroom). Lower n, or shrink chunk/step buffers, if the measured delta exceeds 6.3 MiB; raise n only from measurement (e.g. delta <= 3 MiB allows n = 7 at idle 10).
- If 16K/64K page-size kernels inflate RSS per buffer, shrink chunk/step buffers (64 KiB -> 16 KiB) first.
- Allocator effects (fragmentation from many small pushes) are handled in ADR-005.
