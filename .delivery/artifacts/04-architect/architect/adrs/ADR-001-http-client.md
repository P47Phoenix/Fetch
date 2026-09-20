# ADR-001: HTTP client

Status: Accepted (ARM confirmation pending; see flip criteria)
Date: 2026-09-19

## Context
Need TLS HTTP GET with streaming body, timeouts, gzip, custom DNS resolution (for SSRF pinning, ADR-003), no C build burden on cross-compile, small RSS. Spike (x86_64, plain HTTP, 5 MB body) buffered raw baselines: reqwest idle 3926 kB / peak 15060 kB, binary 2799 kB; hyper+rustls idle 3754 / peak 13792, binary 2436 kB; ureq idle 3660 / peak 13976, binary 2440 kB. Differences ~0.2 MB idle, ~1.3 MB peak, ~0.4 MB binary between reqwest and hyper. Conversion cost dominates (DOM converters +41 MB). ureq is sync and needs `spawn_blocking` under rmcp.

## Options
1. reqwest 0.13, `default-features=false`, rustls with `ring` provider (installed at start), features for streaming and gzip only.
2. hyper 1.x + hyper-util + hyper-rustls directly: ~0.36 MiB smaller binary (spike: 2799 vs 2436 kB), ~1.3 MiB lower buffered peak (spike), but we own redirects, decompression, timeouts, resolver plumbing.
3. ureq 3.x: sync; blocking thread per fetch; less control over streaming cancel.

## Decision
Option 1 with these settings:
- HTTP/1.1 only (no `http2` feature): avoids h2 flow-control windows and buffers, keeps memory predictable.
- We send `Accept-Encoding: gzip` ourselves; reqwest `gzip`/brotli/zstd/deflate features are ALL off, and gzip is decoded by `flate2` in our pipeline so wire and decompressed bytes are counted separately (ADR-004). `flate2` is a direct dependency (14 of 15).
- Redirect policy `none`; redirects handled by our loop (ADR-003).
- `no_proxy()`; no cookie store; no default headers beyond User-Agent, Accept, Accept-Encoding.
- `pool_max_idle_per_host(0)` (no connection reuse; a fetch tool is call-per-URL, and reuse would carry state across policy contexts).
- Custom `dns_resolver` (ADR-003).
- Root certificates: embedded `webpki-roots` so the static musl binary and bare containers work; platform verifier is the alternative (needs system roots, spike section 5.3).
- No aws-lc; ring only.

## Consequences
+ Less code to own (timeouts, gzip, TLS glue); resolver hook exists.
+ ring builds with zig cc (proven).
- ~1.3 MiB more peak and ~0.4 MiB more binary than raw hyper (spike). Inside budget (worst-case per-fetch delta 6.3 MiB, architecture.md 5.1).
- reqwest pulls more transitive crates (audit surface); direct-crate count is unaffected.
- HTTP/1.1-only fails on rare h2-only origins.
- webpki-roots freezes trust anchors at build time (release cadence needed).
- Pool disabled means one TLS handshake per call and per redirect hop; CPU/latency cost on tiny pages, checked against NFR-02 (<= 500 ms overhead p95).

## What ARM data would flip it
- Switch to hyper+hyper-util+hyper-rustls if native aarch64 measurements show idle > 8 MiB (less than 20% headroom under the 10 MiB target) AND hyper-direct recovers >= 1 MiB idle or peak; or if binary > 10 MiB (NFR-13).
- Enable HTTP/2 only if a real-URL test set shows >= 2% failures traced to h2-only origins, and the ARM peak stays within budget with h2 on.
- Switch roots to platform verifier if webpki-roots costs measurable idle RSS on ARM (compare idle with and without loading roots at start, lazily building the TLS config on first fetch is the first mitigation).
- Re-enable pooling only if per-call latency fails NFR-02 due to handshakes and ADR-003 review confirms safety.
