# ADR-002: Streaming HTML-to-markdown converter and main-content extraction

Status: Proposed (memory case proven on x86 with a crude emitter; quality unproven)
Date: 2026-09-19

## Context
Spike results (x86_64, 5 MiB fixture): buffered DOM converters fail the 40 MiB peak: htmd 56.4 MiB, html2md 56.5 MiB, html2text 184.6 MiB. Streaming reqwest + lol_html (`send` API) with early stop: 6.1 MiB (first page), 8.7 MiB (deep page), binary 3193 kB. The spike emitter is crude: headings/lists/paragraphs only; entities not decoded; links, emphasis, code/pre dropped. Tool futures must be `Send`, so lol_html's default `Rc` handlers fail to compile; `lol_html::send` with `Arc<Mutex<..>>` works.

FR-03 wants markdown with headings, links, lists, code blocks, script/style/nav chrome removed "where detectable". Readability-style main-content extraction normally scores DOM subtrees, which needs the whole tree in memory.

## Options
A. DOM converter on a size-capped input (e.g. first 512 KB): bounded, but truncates real pages, and DOM cost scales (~8x page size in the spike: 41 MiB on 5 MiB), so a 512 KB cap would cost ~4-5 MiB; still loses content past the cap. Rejected as default.
B. lol_html streaming rewriter + own markdown emitter, with streaming boilerplate rules and a bounded holdback for landmark selection (chosen).
C. Tokenizer-level crate (html5gum) + own emitter: decodes entities itself and is pure streaming; not measured in the spike; more code (own tree-builder-lite state). Kept as fallback behind the trait.
D. No conversion (raw only): fails goal 3.

## Decision
Option B, behind a `Converter` trait: `push(&mut self, text: &str, sink: &mut Window) -> ControlFlow`, `finish(...)`. Input is UTF-8 already (charset stage, ADR-004). Output is a pure function of the byte stream, independent of chunk boundaries.

Main-content extraction without a DOM, three tiers, all single pass and bounded:
1. Drop rules (element and attribute selectors via lol_html): `script, style, noscript, template, svg, canvas, iframe, object, embed, dialog`, and form CONTROLS (`input, select, textarea, button, label` chrome) but NOT `form` container elements: their descendants are traversed normally, so ASP.NET-style pages that wrap the whole body in one `<form>` keep their content (conservative bias, NB-4; tuned on the 50-URL set at A-4)`, `nav, footer, aside`, `header` when not inside `article/main`, `[hidden]`, `[aria-hidden=true]`, `[role=navigation|banner|contentinfo|complementary|search]`, inline `style` containing `display:none`/`visibility:hidden`, HTML comments. Class/id token heuristics (`cookie`, `consent`, `banner`, `modal`, `popup`, `advert`, `ad-`, `sidebar`, `share`, `newsletter`) drop matching containers; the list is data, tuned against the 50-URL set.
2. Link-density rule: each block (paragraph/list/div-level text run, buffered up to 64 KB) is dropped if link text / total text > 0.8 and total text < 200 chars (menu-like). Decided at block close, so streaming-compatible.
3. Landmark holdback: converted output is held in a buffer <= 256 KiB until either (a) a `<main>`, `<article>` or `[role=main]` start tag is seen: discard the held output that came before it and emit only that subtree from then on (landmark mode, exits at its end tag; if several `<article>` exist all are emitted); (b) holdback fills, or (c) EOF without a landmark: flush everything as whole-body mode. This is deterministic and needs no DOM. Limitation: a landmark first appearing after 256 KiB of converted text is ignored.

Element mapping: h1-h6 to `#`; p, br; ul/ol/li with nesting (depth cap 256, deeper flattened); a with href (absolute resolution against the final URL; `javascript:` and `data:` hrefs dropped; href <= 2 KiB); em/strong; code inline; pre/code fenced blocks preserving whitespace; blockquote; hr; img as `![alt](src)` only when alt present (else dropped); tables emitted row-per-line as pipe rows with header separator after a `th` row (row buffer <= 64 KB); entities decoded (entity crate chosen in A-4; carry a <= 32-byte tail across chunk boundaries so `&am|p;` decodes); whitespace collapsed outside `pre` with state kept across chunks.

Memory: lol_html `MemorySettings.max_allowed_memory_usage` = 2 MiB per rewriter; emitter caps as in architecture.md 5.1. Overflow -> `converter_limit` error suggesting `raw=true`.

## Consequences
+ Peak stays near spike numbers (6-9 MiB) plus emitter growth; large headroom under 40 MiB.
+ No DOM; converter is swappable and independently testable (goldens, chunk-boundary property test).
- Quality risk (R2): heuristic drop lists over- or under-strip; a page whose content sits inside a class named `sidebar`-like tokens may lose text; single-pass cannot do full readability scoring or global text-density comparison; tables with rowspan/colspan are approximated; `<main>` misuse (e.g. a wrapper around the whole page) degrades to whole-body mode gracefully.
- lol_html requires `Send` state; code is more contorted than a DOM walk.
- Tuning needs the 50-URL set (E-1); thresholds are provisional.
- Quality vs any external reference is unproven; PRD has no incumbent, so acceptance is Goal 3 (>= 50% median token reduction) plus FR-03 checks.

Quality risk controls: golden fixtures; 50-URL set metrics in CI; `raw=true` always available; whole-body fallback; conservative bias (prefer keeping text over dropping it) when uncertain; a conversion mode env override is NOT added in v1 (config surface stays small).

## What ARM data would flip it
- If ARM peak with the real emitter on a 5 MiB page exceeds ~25 MiB (over 60% of target, unexpected given 8.7 MiB on x86), re-check lol_html limits, emitter buffers and allocator (ADR-005) before changing converter.
- If `converter_limit` errors exceed 2% on the URL set on any platform, raise the lol_html limit (2 -> 4 MiB) within budget or move to option C.
- If html5gum-based prototype uses >= 20% less memory AND yields equal or better golden results, swap via the trait.
- Not an ARM-only question: if token reduction < 50% or FR-03 goldens fail after tuning, revisit option A as a hybrid (DOM for the first N KB) with explicit memory numbers.
