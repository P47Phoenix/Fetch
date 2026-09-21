# A-4 dev report: HTML to markdown conversion (Sprint 4, part 1)

Branch `sprint-4/convert-memory-gate`. Commits: `d5a6dd3` Sprint 4: carry-forward cleanup; `7384c97` A-4. E-4 (G4a runs, allocator record, shipped-vs-bench record) is NOT in this report: it is still to come in the same PR.

## 1. Carry-forward cleanup (commit d5a6dd3)

| Item | Done |
|---|---|
| (a) ADR-004 doc lines above `content_encoding_is_gzip` | moved back (they had drifted above `echo_url`) |
| (b) cargo-zigbuild pin | `scripts/build-candidates.sh` now compares the whole `--version` line to `cargo-zigbuild 0.23.4` (was a substring match) |
| (c) `native_gate_host` | `bench/measure.py` records `native_host(strict=True)` |
| (d) arm64 cpu | `host_record()` falls back to `lscpu` "Model name" when `/proc/cpuinfo` has none (was `unknown`) |
| (e) stale docs | README "pull request #5" -> "#7 (Sprint 3)", "two CI runs" -> three (README, BENCHMARK rows 18, 19, 221, section 15 heading and the README anchor) |
| (f) end-to-end A-9 test | `a9_echo_has_no_userinfo_or_fragment_end_to_end`: redirect with request-URL and Location fragments, real client, `with_header` output has no fragment, `#`, `@` or credential text. Userinfo cannot reach the echo at all (refused on the first hop and on a redirect Location, `invalid_argument` / `blocked_target`), so the test proves the credential is absent from those refusal texts too |
| (g) report.py malformed JSONL selftest | added to `bench/selftest.py` (three bad lines skipped with a warning each, good line reported, exit 0; missing file gives an empty report) |

## 2. A-4 design and what was built

Streaming, no DOM (ADR-002 option B). `src/convert/mod.rs` holds the `Converter` trait (`push`, `finish`), `Passthrough`, content-type selection and a 512-byte sniff for an untyped body (HTML markers at the start; the full A-6 rules are A-6). `src/convert/markdown.rs` drives `lol_html` (send API, so the tool future stays `Send`) with one `element!("*")` handler and one document text handler; state lives in one `Arc<Mutex<Md>>`.

Wiring: `FetchClient::fetch_as(url, Mode, sink)`; `fetch` stays the raw primitive (all A-3b tests unchanged). The server maps `raw=true` to `Mode::Raw` (this is the trivial part of A-6; A-6's content-type rejection and its ACs are NOT done). The converter sits inside `read_body`, fed by the existing gzip + cap + UTF-8 pipeline; the base URL for links is the final request URL. New error `converter_limit` (message suggests `raw=true`) for the tokenizer memory limit, the output guard, or an unprocessable page.

Rules implemented: headings, paragraphs, br, hr, nested and numbered lists (`start` honoured), links (resolved absolute; `javascript:`, `data:`, fragment-only and over 2 KiB destinations lose the link and keep the text; parentheses encoded), em/strong, inline code, fenced `pre`/`code` with language and a guard against embedded fences, blockquote, images only with alt (http/https src), pipe tables with a separator after a header row (cell 64 KiB, 256 cells per row; a bigger cell degrades to plain lines), entities decoded across chunk boundaries (32-byte carry) and in attribute values, whitespace collapsed with state kept across chunks, NUL replaced by U+FFFD. Drop rules (whole subtree): script, style, noscript, template, svg, canvas, iframe, object, embed, dialog, head, title, nav, footer, aside, select, textarea, button, input; `[hidden]`, `[aria-hidden=true]`, navigation/banner/contentinfo/complementary/search roles, inline `display:none`/`visibility:hidden`; class/id whole-token noise words (cookie, consent, banner, popup, modal, advert, ad, sidebar, share, newsletter) on div/section/ul/ol/form/span only. `form` containers are traversed. Landmark holdback: output held until a `main`/`article`/`[role=main]` start (held part discarded, only landmark subtrees emitted) or until 256 KiB (then whole-body mode); the switch happens where the output crosses the limit, not at a slice boundary, so output does not depend on chunking.

No early stop: the G4a scenarios are full-consumption by definition and early stop is the A-5 `Window` path.

### Dependencies (NFR-05: 10 of 15 direct)
- `lol_html =2.9.0`, `default-features = false`. Justification: buffered DOM converters measured 56.4 / 56.5 / 184.6 MiB in the A-1 spike (fail 40 MiB); lol_html streaming measured 6.1-8.7 MiB. 2.9.0 is the spike-measured version (3.0.1 exists; moving is a re-measure, not done). Licence BSD-3-Clause.
- `html-escape =0.2.15`, `default-features = false`: full HTML5 named-reference table (2125), no dependencies, MIT, so no hand-kept entity table. Note: it contains a small amount of `unsafe` internally (`from_utf8_unchecked`); accepted, flagged for review.
- **deny.toml change (legitimately required):** lol_html brings four Servo crates under MPL-2.0 (cssparser, cssparser-macros, dtoa-short, selectors). MPL-2.0 is NOT added to the global allow list; it is allowed by four per-crate `exceptions`, with a comment. `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok (two pre-existing "license not encountered" warnings). System allocator unchanged.
- a3b-merge-gate and the release guard: no change needed; both pass (`a3b_merge_gate refusal_ dial_once differential` 10 passed in release; `check-release-features.sh` and `--self-test` OK).

## 3. Tests
- 34 converter tests plus fetch integration (HTML over the real client, gzip HTML, untyped sniff, `converter_limit`, raw and text/plain untouched). Total: 133 lib tests default, 134 with bench-loopback, 133 with test-support; 9 / 11 / 9 integration tests. All pass.
- Chunk-size invariance: every chunk size 1..40 gives identical output on a mixed page; a deterministic 300-round pseudo-random tag-soup test compares whole vs randomly chunked output and never panics.
- Hostile inputs (all in `src/convert/markdown/tests.rs`): 200,000-deep nesting for ten tag kinds, 8 MiB attributes and tag names, unclosed/misnested markup, malformed comments/CDATA/entities, NUL and control characters, link amplification (2 KiB base with 400,000 short relative links: output capped), 300,000 unclosed table cells, invalid-UTF-8 replacement text (the body pipeline replaces invalid bytes before conversion, tested there since A-3b). No panics; extreme cases return `Limit`.
- Local fixtures are labelled as local unit-test fixtures only.

## 4. Measurements (x86_64 Fedora host, advisory, NOT G4a; native `cargo build --release`, not the zigbuild pipeline)

| Item | Before A-4 (8192b48) | After A-4 | Delta |
|---|---|---|---|
| Shipped release binary size | 3,037,832 B | 3,680,456 B | +642,624 B (+628 KiB) |
| Idle RSS, shipped, median of 10 (3 s settle) | 4.05 MiB | 4.80 MiB | +0.75 MiB |
| bench build, 5 MiB HTML read-in-full peak (VmHWM median of 10) | 5.24 MiB (unconverted) | 6.27 MiB (converted) | +1.03 MiB |
| bench, 5 MiB gzipped | 5.28 | 6.26 | |
| bench, late-landmark | 5.31 | 6.42 | |
| bench, 50 MiB Content-Length | 4.93 | 5.83 | |
| bench, 50 MiB chunked | 5.34 | 6.04 | |
| boundedness ratios (vs 5 MiB) | 0.94, 1.019 | 0.929, 0.963 | |
| 1 MiB conversion, p95 (100 calls, 5 warm-up) | | 29.9 ms (median 29.7) | target 500 ms on aarch64: NOT measured on aarch64 yet |

The idle delta (+0.75 MiB) exceeds the 0.5 MiB figure that E-8 bounds for the shipped-vs-bench delta, but that bound is between two binaries of one commit, not before/after a feature; it is recorded here and the gnu/zigbuild numbers on the hosted runners are E-4's evidence. All figures are x86_64 gnu, advisory harness runs (not `--gate`).

## 5. Verification (clean `git archive` of 7384c97, same source as the PR head apart from this report)
From a clean `git archive HEAD` extraction (fresh target dir): `cargo fmt --check` OK; `cargo clippy --locked --all-targets -D warnings` OK for default, `bench-loopback`, `test-support`; `cargo test --locked` 133 / 134 / 133 lib tests pass (1 ignored: the timing test); release build OK (3,680,456 B); `check-release-features.sh` guard OK and `--self-test` OK; `cargo deny check` (v0.20.2, installed locally) advisories/bans/licenses/sources ok; `bench/selftest.py` PASSED; `actionlint` clean (run in the repo, an extracted archive has no `.git` for it).

## 7. CI
Draft PR #8, head 8a? (see PR). All 14 checks passed: a3b-merge-gate, bench (gnu, musl), bench-product (gnu), bench-product-peak (gnu), clippy, deny, fmt, release-guard, test, and bench-gate on amd64 gnu/musl and arm64 gnu/musl (run 35556074053). The bench-gate cells are hosted-runner `--gate` runs of the harness with the A-4 converter in the bench build, but they are the E-2/E-3 workflow as it stood, not the E-4 G4a evaluation; no G4a verdict is claimed here.

Recorded from the run logs (advisory reading, not a G4a pass): 1 MiB conversion p95 on the hosted arm64 gnu runner 59.8 ms (median 59.6; target 500 ms), amd64 gnu 51.6 ms; gating peak (max of per-scenario medians) 5.32 MiB on the arm64 gnu cell and 6.18 MiB on the amd64 gnu cell, boundedness ratios 0.93 / 1.01 (arm64) and 0.96 / 1.006 (amd64). Other cells' figures are in their job logs.

## 6. Honest gaps
1. **95% conversion success / 50% median token reduction / no `<script>` on the E-7 50-URL set: NOT DONE.** The set does not exist; no corpus was fabricated. Per the plan's E-7 fallback, A-4 is not Done until this is met; G4a is unaffected.
2. **1 MiB overhead on aarch64 not measured.** A `#[ignore]` timing test implements the BENCHMARK section 9 method and is wired into `bench.yml` (gnu cells, amd64 and arm64) to record it; the arm64 figure is whatever CI reports.
3. **Tier-2 link-density rule (ADR-002) deliberately not implemented.** It cannot be tuned without the URL set and, untuned, it deletes legitimate link lists (index pages, references). Conservative bias per the ADR. Also not implemented: dropping `<header>` outside article/main (kept; nav inside it is still dropped).
4. **Tokenizer memory limit vs real pages.** Unclosed `<p>`, `<li>`, `<td>` or `<div>` nest without end in lol_html's stack: about 5,000 open ones convert, 20,000 return `converter_limit` at the 2 MiB limit. ADR-002 says raise to 4 MiB if more than 2% of the URL set hits this; cannot be evaluated without the set.
5. **Quality not tuned:** noise-word heuristics, landmark holdback and table handling follow the ADR but were never run against real pages. Known approximations: no markdown escaping of `*`, `_`, `[` in text; backticks in inline code are not escaped; layout tables larger than 64 KiB per cell degrade to plain lines; a page whose `main`/`article` is a small teaser loses everything outside it (ADR limitation); the `<title>` is dropped.
6. Charset: input is the UTF-8 stream (non-UTF-8 charsets are A-8). Content-type rejection and full sniffing are A-6; non-HTML types (text/*, JSON, and also binary types) are passed through as text for now.
7. html-escape includes `unsafe` internally; lol_html 2.9.0 is one major behind 3.0.1.
8. Measurements are x86_64 only and not from the pinned zigbuild pipeline; no aarch64 number for anything in this report. E-4 (G4a) is not started in this commit.
