# Test fixtures for A-4 / E-5 / E-7 (local-fixture rework, Sprint 13)

**What is this?** A-4's conversion-quality check (>= 95% conversion success, >= 50% median token reduction,
no `<script>` text survives), E-7's "50-URL offline snapshot" and E-5's "10-URL live smoke" were all originally
specced around a URL list the project owner was going to supply. That list never arrived across 12 sprints
(Sprint 2 through Sprint 12), because it depends on live external websites that change over time and on the
owner's spare time to go compile a list of them. **Sprint 13 replaces the live-URL dependency with a local,
owner-authored HTML fixture directory.** This document says exactly what to drop in and where, and exactly
what command closes A-4/E-5/E-7 once you do.

**Who needs this?** The project owner (Michael), the only person who can supply real fixture content.
**What to do first:** read "What to generate" below, drop files into `bench/corpus/`, then run the
one command in "How to regenerate the manifest and report".

## Status

A-4, E-5 and E-7 remain **NOT DONE** as of Sprint 13. That is expected and correct: the tooling is ready
(`bench/corpus_check.py`), but only three tiny, clearly-marked placeholder fixtures exist
(`bench/corpus/example-*.html`), which is not a real corpus and does not pretend to be one — it
exists only to prove the harness works end to end. They are **not** part of the real fixture set: swap them
out (or add alongside them) when the real fixtures land.

## What to generate

Drop `.html` files directly into `bench/corpus/`. Each file is one fixture; the harness serves it
from a loopback HTTP server and runs it through the real `fetch-mcp` `fetch` tool, then compares raw vs
converted token counts and checks for literal `<script` text in the output.

**How many:** the sprint plan's original E-7 target was 50 files (offline snapshot) and E-5's was 10 (used the
same way here, since both are now local fixtures run through the identical pipeline — there is no live-vs-
offline distinction left to justify two separate counts). A smaller set (say 15-20) is still useful and will
still produce a PASS/FAIL verdict; the AC math (>= 95% success, >= 50% median reduction) just gets noisier
with very few files, the way three tiny placeholders currently are.

**Suggested content diversity** (mirror real-world pages the server is likely to see):

- **Article / prose-heavy pages.** Long-form text, headings, links, a few images with `alt` text. This is the
  main case the median-token-reduction target is about (markdown strips markup overhead around real prose).
- **Navigation / boilerplate-heavy pages.** A page where most of the DOM is nav bars, footers, cookie
  banners, ads, or repeated site chrome around a small amount of real content — this is where the biggest
  token reductions should show up, and where a converter regression (boilerplate not stripped) would be most
  visible.
- **Table-heavy pages.** Data tables, comparison grids, pricing tables — exercises the markdown table
  converter path.
- **Code-documentation-heavy pages.** API reference or how-to pages with `<pre>`/`<code>` blocks, inline
  `<code>` spans, and syntax-highlighted code (often wrapped in extra `<span>` markup that should collapse
  cleanly in the conversion).
- **Pages with inline `<script>`/`<style>`.** At least a few files should contain real `<script>` and
  `<style>` elements (inline JS, tracking snippets, embedded CSS) so the "no `<script>` text survives" check
  actually exercises something. Per the known limitation documented in the README ("Literal `<script>` text
  can still appear in the output from sources that are not script elements"), avoid putting the literal
  substring `<script` inside ordinary prose or an `alt`/title attribute of a fixture meant to pass this check
  -- that is a documented converter limitation (escaped or quoted mentions of the word can survive), not a
  bug in a `<script>` *element's* removal, and it will trip the harness's textual check as a false positive
  for that one fixture.
- **Edge cases.** An empty `<body>`, malformed/unclosed HTML, a page with no `<title>`, a page with unusual
  encodings or characters, a very short page (a few words) — these exercise the "no crash, no hang, graceful
  result" side of the success-rate target more than the token-reduction target.

**Naming convention:** `<NN>-<short-slug>.html`, zero-padded two-digit index, lower-kebab-case slug, for
example `01-news-article.html`, `02-nav-heavy-blog.html`, `17-empty-body.html`. The index is only for stable
ordering in directory listings and diffs; the harness does not parse or require it.

**Where to drop them:** `bench/corpus/`. Nothing else needs to change — the harness discovers every
`*.html` file in that directory automatically.

**Size:** no hard cap, but keep individual fixtures reasonably realistic (a real captured page, a few KB to a
few hundred KB) rather than the multi-MiB memory-benchmark fixtures used elsewhere in `bench/` (those are a
different concern, E-2/E-4's peak-RSS harness, and live in `bench/fixtures/` directly, not under `corpus/`).

## How to regenerate the manifest and produce the A-4/E-5 pass/fail report

```sh
# 1. Build a bench-loopback binary (needed so the harness's loopback fetch is not refused by the SSRF policy):
cargo build --locked --release --features bench-loopback

# 2. Regenerate the sha256 manifest for whatever is currently in bench/corpus/ (review the diff):
python3 bench/corpus_check.py manifest

# 3. Run the conversion-quality check (also regenerates the manifest by default; pass --no-manifest to skip):
python3 bench/corpus_check.py check --binary target/release/fetch-mcp --out bench/corpus/report.json
```

Exit codes: `0` PASS (both targets met, no script leak), `1` FAIL (a target missed or a leak found, details in
the JSON report), `2` no fixtures found yet (the current, expected state until the owner drops files in).

The report (`report.json` if `--out` is given, otherwise stdout only) is a plain JSON object with
`verdict`, `success_rate`, `median_token_reduction`, `script_leaks`, and a `rows` array with one entry per
fixture (raw/converted token counts, whether it had a `<script>` tag, whether it "leaked" literal `<script`
text). A-4's AC is satisfied once this reports `verdict: "PASS"` on the real (owner-supplied) corpus; the
placeholder examples currently checked in do not, and are not meant to.

## Why this replaces the old E-5/E-7 URL-list AC text

- E-7 was specced as "capture 50-URL offline snapshots with a committed sha256 manifest" from a
  project-owner-supplied URL list. No such list ever arrived (Sprint 2 through Sprint 12); no E-7 harness code
  existed at all before Sprint 13 (`bench/` had no script referencing E-7 or a URL-list format).
- E-5 was specced as a "10-URL live smoke" against a project-owner-supplied list of real HTTPS URLs
  (`bench/smoke.py`, `bench/e5_report.py`, both built in Sprint 7 and still functional and unchanged by this
  rework for anyone who *does* want to run a real live smoke later); the list also never arrived.
- Both blockers were external-website-dependent and outside the owner's easy control (sites change,
  disappear, or are simply hard to make time to go pick 10-50 of). A local, owner-authored fixture directory
  removes that dependency entirely: the owner writes or captures a handful of `.html` files once, on their own
  schedule, with no live network involved in either generating or running the check.
- `bench/smoke.py` and `bench/e5_report.py` are left in place, unmodified, for anyone who later wants to run a
  literal 10-URL *live* smoke against real external sites -- that path still works and is not deprecated, it
  is simply no longer the only path, and no longer the one A-4/E-5/E-7 are blocked on.
