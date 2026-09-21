# Architect + security review: PR #9 (Sprint 5, A-5 / A-6 / G4b), head cd99c95

Role: Solution + Security architect, read-only. Build and probes under ~/.cache (bench-loopback release build). DECISION: DONE, 0 blocking.

## 1. Window sink and early stop (memory)
- Retained memory is `out` <= `max_length_cap` (100 000 chars, about 400 KB worst case). Skipping to `start_index` buffers nothing: `skip` is decremented per chunk, only counts and slices.
- No quadratic behaviour: `count`/`offset` are O(chunk) once per push; ASCII fast path; `offset` runs once per window edge.
- Integer safety: `taken += n` only when `n <= room`; `seen` uses saturating_add; `start + returned` saturating; `usize::try_from` falls back to MAX/min(len). `start_index=u64::MAX, max_length=u64::MAX` verified over real stdio (clamped, "No content" message, no panic).
- Offsets are Unicode scalar values, cut at `char_indices` boundaries; multi-byte and split-chunk cases are unit-tested and the emoji/euro/e-acute probe returned correct windows.
- Early stop is polled after each wire chunk; response dropped, connection closed. Beyond-cap bodies (start_index 1e9 on an endless chunked body, >5 MiB body) end as `too_large`; a huge start_index on a 4.5 MiB body reads to the end (bounded by the cap) and reports the total.
- Real stdio probes, 3 concurrent fetches, VmHWM: 5 MiB-ish multibyte body with start 1.4M / near end / u64::MAX / raw+huge max_length; 60 MiB chunked; attribute bomb (1.3M attrs), 5 MB attribute value, 400k-deep nesting, each with windows and raw=true. VmHWM 6.0 to 9.4 MiB in all cases (limit 40). Hostile pages give `converter_limit` (retry raw) or a correct window in raw mode.
- No panic paths found (no unwrap/index on unchecked offsets; slicing uses computed char boundaries).

## 2. Content types
- Rejected before any body byte: everything not `text/*`, html/xhtml, the 6 listed application types, `+json`, `+xml`. Malformed types are rejected (`garbage`). Applies to raw=true as well. Declared-length over-cap check still happens (moved after type check, harmless).
- Missing or empty Content-Type: sniffed (HTML marker at start after BOM/whitespace, tag-name boundary checked), else text. Binary with no type is passed as lossy UTF-8 text (replacement chars); safe, no bytes reach stdout unescaped.
- stdout purity: NUL, ESC, BEL, CR, invalid UTF-8 in raw mode all came back as valid single-line JSON-RPC (serde escaping); server exited 0. Note ESC/control chars reach the model verbatim inside the JSON string (NB-2).
- `image/svg+xml` and `application/xhtml+xml`-like `+xml` types pass as text (SVG script text visible, never executed); acceptable.
- Charset: the `charset` parameter is ignored; all bodies are decoded as UTF-8 lossy. utf-16 or latin-1 pages give mojibake (NB-3).

## 3. Output and protocol safety
- Footer strings are static apart from decimal numbers computed by the server. A page cannot alter the real footer but its body can contain a look-alike line ("[More content available... start_index=N]") because the footer is separated only by a blank line. Effect is limited to the same URL and offset (no new URL, no tool other than fetch), so injection surface is no larger than the already-accepted OQ-5 (no label) posture (NB-1). OQ-5 not reopened.
- Echoed media type in `unsupported_content_type` is attacker-controlled but limited to 100 printable-ASCII chars (spaces and controls become `?`); tested. Small residual: a short arbitrary token is model-visible (NB-4).
- Panic=abort: no reachable panic found in window, render, classify, sniff.

## 4. Deviations
1. Total-length footer only on continuation/beyond-end: accepted. ADR-006 itself says total is unknown after early stop; footer on a fully-fitting first page would break "returned exactly as fetched". Consistent and tested.
2. G5 via 50 MiB chunked variant: accepted. Fixture cannot exceed the 5 MiB cap; probe confirmed start_index beyond cap gives `too_large` and low memory.
3. G1 duplicates g4a-5mib-full: accepted, redundant not weakening; record it in BENCHMARK.
4. text/* extras allowlist (javascript, ecmascript, ndjson, +json, +xml): accepted, all textual and safe as text; owner acknowledgement still needed since it is a product decision.
5. G4a redefined as window-at-end: accepted; read-to-end and gzip inflate state are still exercised. Note a plain offset-0 full read no longer exists as a scenario, but early-stop windows at start cover it (g4b-window-start).

## 5. CI / supply chain
- Diff touches only arm-bench.yml and bench.yml among workflows. ci.yml, scripts/, Cargo.toml, Cargo.lock, deny.toml unchanged: no new deps, release-guard, a3b-merge-gate and required-check set unchanged.
- arm-bench.yml: change limited to dropping the spike peak measurement (idle only, `[ "$r1" -le 1 ]`) and `.get()` tolerance in two summary snippets. No permission, pin, trigger or secret change. Not a required check.
- bench.yml: adds `--scenario g4b` to the existing gated peak step and comments only.

## Non-blocking
- NB-1 Footer look-alike can be forged by page body; consider a distinct delimiter later (out of scope per OQ-5).
- NB-2 Control chars/ESC pass through in raw and text; consider stripping C0 except \t \n \r.
- NB-3 charset ignored (non-UTF-8 pages garbled); document or add decoding later.
- NB-4 Echoed media type is attacker text (100 ASCII chars).
- NB-5 Ask owner to acknowledge deviations 1 to 5 formally in EPICS.
