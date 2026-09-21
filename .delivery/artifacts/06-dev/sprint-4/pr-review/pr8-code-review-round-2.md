# PR #8 code review — round 2 (Sprint 4: A-4 conversion, E-4 G4a / build identity, fix-pass 1)

Reviewer: independent code review (read-only; no edits to tracked files, no push, no PR comment, no merge).
Scope: `sprint-4/convert-memory-gate` head `3e02c70` against `main` `c13159e` (43 files, +4781/-78).
Round 1 reviewed `9d3dc21`; this round re-checks its two blocking findings and reviews the fix pass
(`f9e4c9d` code, `abdeaed` + `3e02c70` docs/CI) as new code.

**DECISION: APPROVE** — 0 blocking, 8 non-blocking.

---

## What was checked, and how

Everything below was reproduced locally (target dir and `TMPDIR` under `~/.cache`, repo untouched):

| Check | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --locked --all-targets --features bench-loopback -- -D warnings` | clean |
| `cargo test --release --locked` | 147 + 1 + 9 pass |
| `cargo test --locked --features bench-loopback` (debug, as `ci.yml` runs it) | 13 stdio + hostile_rss pass |
| `bench/fixtures.py generate` + `bench/selftest.py` | all `ok` (incl. the 5 new fix-pass checks) |
| Independent differential fuzz of `tagscan` vs the real `lol_html` lexer | 18,000 docs × 4 chunkings, **0 undercounts** |
| Independent old-vs-new conversion differential (`git archive 9d3dc21` built as a second crate) | 18,000 conversions, **0 output differences** |
| Independent drop-rule leak sweep (11 payloads × depths 0/200/255/256/257/300/1000/5000 × 2 chunkings) | **no leak** |
| Independent hostile-RSS sweep, 21 shapes *not* in `tests/hostile_rss.rs`, 3 concurrent | worst **+6 MiB**, all bounded |
| Real-file corpus: 2,509 on-disk HTML pages + 1,501 real JS files inlined in `<script>` | **0 false refusals** |

---

## Round-1 findings: both fixed, verified independently

### B1 (script/style/nav leaking past 256 open elements) — FIXED, correctly

`markdown.rs:550-592` now runs the drop rules, then computes `over_cap`, and only returns early *after* the
landmark check. `Kind::Skip` is still uncounted in `open` (`markdown.rs:1108`), so registering skips at any
depth does not weaken the cap, and the skip stack is a single `Option<Skip>` with a depth counter — no new
growth.

Reproduced the old failure on `9d3dc21` and its absence on `3e02c70`, driving `MarkdownConverter` directly:
`script`, `style`, `nav`, `iframe`, `noscript`, `template`, `<svg><style>`, `hidden`, `aria-hidden` and
`<head><title>` payloads at depths 0, 200, 255, 256, 257, 300, 1,000 and 5,000, whole and in 7-byte chunks —
no `SECRET` marker reaches the output at any depth. The landmark case also works past the cap
(`<div>×300<main>…` yields only the `<main>` content). `tests/stdio.rs::real_stdio_drop_rules_hold_at_any_depth`
covers the same end to end through the real binary at depth 10,000 and passes.

**No conversion regression.** 6,000 randomly assembled realistic pages (40 fragment kinds: headings, lists,
tables, `pre`/`code`, blockquotes, entities, landmarks, unclosed `p`/`td`, …) × 3 chunkings = 18,000
conversions produced byte-identical output on `9d3dc21` and `3e02c70`. The only differences are the intended
ones at depth ≥ 256.

### B2 (`tee` masking the A-4 500 ms assertion) — FIXED

`.github/workflows/bench.yml:114` is now `set -euo pipefail`, with a comment saying why. The step name still
asserts the AC and there is no `continue-on-error`, so name and behaviour now agree. (One residual no-op path
remains — see NB-1.)

### Round-1 non-blocking items

N1 (row bound), N2 (`<base href>` documented in README:98), N3 (push-after-finish now `Malformed`),
N4 (`-dirty`), N5 (`FETCH_MCP_COMMIT` documented in BENCHMARK §"Build identity marker"), N6 (worktree/packed-refs
safe rerun directives), N7 (`call_many` stall context), N9 (dead `html` param) are all addressed.
N8 (public reference page stability) is not — see NB-7.

---

## New code (fix pass): reviewed hard, found sound

### `src/convert/tagscan.rs` — the attribute pre-scan

**Correctness of the over-approximation.** The argument in the module doc holds, and I checked each leg:

* *Grammar fidelity.* The eight states mirror `lol_html-2.9.0/src/parser/state_machine/syntax/tag/{mod,attributes}.rs`
  transition by transition, including the fused `before_attribute_name`/`self_closing_start_tag` state (lol_html
  reconsumes the non-`>` byte in `before_attribute_name`, which is exactly `put(BA, c)`), `after_attribute_name`
  (`_ => finish_attr; start_attr` ↔ `start(AN, c+1)`), and `>` being inert inside quoted values. `is_ws` is byte
  for byte the DSL's `whitespace` (`arm_pattern/mod.rs:15`: `b' ' | b'\n' | b'\r' | b'\t' | b'\x0C'`), and
  `is_ascii_alphabetic` is the DSL's `alpha` (`b'a'..=b'z' | b'A'..=b'Z'`).
* *Merge-by-max is safe.* Transitions depend only on `(state, byte)` and counts only increment, so if `c1 >= c2`
  in the same state then every successor of `c1` dominates the corresponding successor of `c2`. The real tag's
  candidate is always one of the merged ones, so the reported peak is never below the lexer's count.
* *No end-tag bypass.* `</a b c …>` starts no candidate (the byte after `<` is `/`). That is not a hole:
  `lexer/actions.rs:294-301` creates an `AttributeOutline` only `if let Some(StartTag { .. })`, so end-tag
  attributes cost lol_html nothing.
* *Chunk boundaries.* `after_lt` is carried across `feed` calls, and the memchr-style skip-ahead is entered only
  when no candidate is live *and* the previous byte was not `<`; a trailing `<` correctly leaves `after_lt = true`.
  Verified with chunk sizes 1, 3, 7, 17, 64, 1000 and whole-buffer.
* *No quadratic behaviour, no panic.* `step` is O(8) per byte; a candidate stuck in a quoted value only forces
  byte-by-byte walking (still O(n)). Every access is `get`/`get_mut`, counts are `i64` and bail at `ATTR_CAP + 1`,
  so nothing can overflow or index out of range under `panic = "abort"`.

**Independent differential fuzz.** I copied `tagscan.rs` out of tree and ran it against the real `lol_html`
lexer over 18,000 random tag-soup documents (6 seeds, a 65-token alphabet that adds uppercase tags, `</a`/`</div`
end tags with attributes, NUL bytes, `<\0a`, `<!doctype>`, `<template>`, `<annotation-xml encoding="text/html">`,
`<!--->`, foreign-content and rawtext switches), each at 4 chunkings: **zero undercounts**. This is a separate
corpus and seed set from `src/convert/tagscan/tests.rs`, which itself is a good test (the
`quoted_gt_and_rawtext_contexts_cannot_hide_a_tag` and `never_undercounts_the_real_lexer_on_random_tag_soup`
cases are the right ones).

**Placement.** `push` feeds the scanner *before* `rw.write`, and a chunk that trips the guard is never handed to
lol_html, so the rewriter can hold at most ~`ATTR_CAP` outlines (~140 KB) at any time. The scanner and the
rewriter see the identical byte slice (`text.as_bytes()` in both cases), so encoding cannot desynchronise them.

**Boundedness beyond the tested shapes.** I swept 21 adversarial shapes that are *not* in `tests/hostile_rss.rs`
(5 MiB each, 3 concurrent): nested `<svg>`, `<b>`, `<table>`, `<li>`, `<blockquote>`, `<template>`, `<main>`,
`<nav>`, `<td>`, `<svg><i>`; 5 MiB single comment, doctype, CDATA in SVG, tag name, bogus comment, unclosed
quoted value; 5 MiB of end tags, of `<i a=1 b=2>`, of `<script>x`, of `&amp;`, of `&#`. Worst peak RSS delta
**+6 MiB** (the `<svg>` case, lol_html's `ns_stack`); every pathological shape returns `ConvertError::Limit`
rather than growing. I found no further memory class that lol_html's limiter misses.

### Other fix-pass code

* **`ROW_BYTES` (N1).** `close_cell` puts the cell back before calling `abandon_table`, and `abandon_table`
  takes it first (`markdown.rs:983`), so the `put` → `abandon_table` → `put_main` recursion round 1 checked for
  still cannot occur. See NB-4 for two small imprecisions.
* **`push`/`finish` after `finish`.** Both now return `ConvertError::Malformed`; `finished` is a separate flag
  from `failed`, so the fail-closed contract is explicit. Covered by
  `convert::markdown::tests::push_or_finish_after_finish_is_an_error`.
* **`build.rs`.** `-dirty` marker, the trusted-`FETCH_MCP_COMMIT` note, and worktree/submodule/packed-ref-safe
  `rerun-if-changed` are all correct; `git rev-parse --git-dir`/`--git-common-dir` is the right way to do this.
  `check_build_identity` rejects `-dirty` *before* `build_identity`'s `[0-9a-f]{40}` regex would otherwise
  silently accept the 40-hex prefix — that ordering matters and is right. `build-candidates.sh` builds
  sequentially, so no concurrent-`git status` lock race.
* **CI.** `permissions: contents: read`, no `pull_request_target`, fork PRs skipped, actions pinned by SHA,
  no untrusted interpolation in the new `run:` blocks. The final "Fail the job" step still gates on
  `steps.{idle,peak,idle_bench}.outcome`.
* **Bench gates.** The new `outcome` rule (`measure.py:271`) keeps `too_large` semantics and adds
  `converter_limit` in the same fail-closed shape. `hostile-attrs3`/`hostile-attrvalue3` are `gate="none"` and
  `recorded_only`, so they cannot inflate or deflate the gating peak, and `selftest.py` proves both the RECORDED
  verdict and the INVALID path (`STANDIN_NO_HOSTILE_REFUSAL`). I found no path by which a hostile scenario turns
  a FAIL into a PASS.
* **Stdout purity.** `#![deny(clippy::print_stdout)]` holds under clippy with `bench-loopback`; the new tests
  print to stderr and `finish_and_assert_pure` still runs.
* **Supply chain.** `Cargo.toml`, `Cargo.lock` and `deny.toml` are untouched by the fix pass — round 1's
  MPL-2.0 analysis still stands.

---

## Non-blocking

### NB-1. The A-4 500 ms step still has a silent no-op path (confidence 88)

`bench.yml:114-118`. `pipefail` fixes the `tee` masking, but `cargo test --locked --release --lib
conversion_overhead_1mib -- --ignored` **exits 0 when the filter matches nothing** — I checked:

```
$ cargo test --locked --release --lib conversion_overhead_1mib_typo_xyz -- --ignored
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 148 filtered out
$ echo $?   -> 0
```

and the following `grep CONVERT_1MIB … || true` swallows the absence of the measurement. So renaming or
re-`#[ignore]`-ing the test turns the step whose name asserts an acceptance criterion back into a no-op. Cheap
fix: drop the `|| true` (or `tee` the grep into a required check) so a missing `CONVERT_1MIB` line fails the
step, and optionally pass `--exact`.

### NB-2. `hostile-attrvalue3` sits 0.2% under the converter-limit cliff; crossing it fails the whole bench job (confidence 90)

`bench/fixtures.py:26` makes the page `2 MiB - 4096` and `scenarios.py` declares `expect="ok"`. I measured the
actual cut-off with a bisection on the shipped converter:

* largest single quoted attribute value that still converts: **2,097,129 bytes**
* the value this scenario serves: **2,092,977 bytes**
* **headroom: 4,152 bytes (0.2%)**

It is deterministic today (I confirmed `Ok` at chunk sizes 7, 64, 1024, 8 KiB, 16 KiB and 64 KiB), so nothing is
broken. But if it ever crosses — a `lol_html` patch that changes `Arena` growth accounting, a `REWRITER_LIMIT`
change, or an edit to the `pre`/`post` constants — the reply becomes `converter_limit`, the scenario is INVALID,
`measure.py` exits 2, the `peak` step fails and the `bench-gate` job fails on all four cells. Either give the
page real headroom (e.g. `HOSTILE_SIZE = 1 MiB` for the `attrvalue` page — the point is one huge attribute, not
one *maximal* attribute) or let the scenario accept either outcome, and record the margin in BENCHMARK §16.

### NB-3. The user-facing docs understate the guard: a page with no over-cap tag can be refused (confidence 85)

README:99 says "A page with a tag that has more than 1,024 attributes is refused with `converter_limit`". The
guard over-approximates by design, so a page with *no such tag* can also be refused: any `<` immediately
followed by an ASCII letter, followed by ≥ 1,025 whitespace-separated tokens before the next `>`, in any
context. I reproduced this with `<p>if a<b then <1200 words></p>`, with a `<pre>` code listing, and with
`<script>for(i=0;i<n;i++){ … }</script>` where the script body has no `>`.

How likely is it in practice? Much less than that phrasing suggests — I ran the shipped converter over every
HTML file I could find on this machine (2,509 pages) and over 1,501 real JavaScript files inlined into a
`<script>` tag: **0 refusals**. So this is a documentation-precision point, not a functional one; it fails
closed with a message that suggests `raw=true`. Still, the fix-pass report's own gap list phrases it as "prose
like `a <b` plus 1024 words"; the README should say the same in the user's words, since the refusal is
user-visible.

### NB-4. Two imprecisions in the new row bound (confidence 82)

`markdown.rs:937-942`. (a) `self.tbl.row.iter().map(String::len).sum()` is recomputed on every `close_cell`,
i.e. O(`ROW_CELLS`) per cell — linear overall with a 256× constant, but a 5 MiB page of `<td>` costs ~200 M
`String::len` calls for no reason; a running byte counter on `Tbl` is a two-line change. (b) the sum is over the
already-`|`-escaped cells while the comparison adds the *un*escaped new cell, so the stored row can reach
`ROW_BYTES + CELL_CAP` = 320 KiB, not the 256 KiB the comment states.

### NB-5. The `-dirty` marker is best-effort, and BENCHMARK overstates it (confidence 82)

`build.rs` emits explicit `rerun-if-changed` directives, which disables cargo's default "any file in the
package" rule. The declared set is `src`, `Cargo.toml`, `Cargo.lock`, `build.rs` and the git HEAD/index/ref
files. So modifying a tracked file outside that set (`tests/`, `bench/`, `scripts/`, `docs/`, `README.md`,
`.github/`) makes `git status --porcelain -uno` non-empty without re-running `build.rs`, and a cached build
keeps reporting a clean 40-hex commit. That is arguably the right trade (those files do not affect the shipped
binary), but BENCHMARK.md:558 says `-dirty` is appended "when `git status --porcelain --untracked-files=no` is
non-empty" without qualification. One clause would fix it. CI always builds from a fresh checkout, so no
practical risk there.

### NB-6. Pre-existing: a drop tag self-nested past `OPEN_CAP` silently empties the rest of the page (confidence 85)

`markdown.rs:532-543`: past `OPEN_CAP` the nested same-name start tag increments `sk.depth` but returns `None`,
so no end handler is registered and the matching end tag never decrements it. `sk.depth` can then never reach 0
and everything after is dropped. Confirmed on both trees:

```
self-nested <nav> depth=256: old=Ok("after") new=Ok("after")
self-nested <nav> depth=257: old=Ok("")      new=Ok("")
```

Identical before and after the fix pass, so **not** introduced here and not a B1 regression — but it is content
loss with no error, and the B1 fix makes skips reachable at more depths, so it is worth a line in the honest-gaps
list (or a `saturating_add` that stops at `OPEN_CAP` so the depth stays balanced).

### NB-7. Round-1 N8 (public reference page stability) not addressed (confidence 80)

`bench/public_check.py` still pins a specific live Wikipedia article whose size and markup can change between
runs, and BENCHMARK §16 records the "~2 MiB not 5 MiB" deviation but not that a change in the page invalidates
cross-run comparison. Advisory step, so low impact; one sentence in the doc closes it.

### NB-8. `hostile-attrs3`'s validity floor is too low to notice a regression in early refusal (confidence 80)

`scenarios.py`: `min_bytes=lambda m: 3 * 4096`. The page is ~2 MiB and the product is expected to refuse it
after the first slices, so the floor cannot distinguish "refused after 4 KiB" from "read the whole 2 MiB and
then refused". Since the scenario's value is the RSS figure, that is mostly fine, but a *ceiling* (or recording
the served byte count in the JSONL) would make the early-refusal property observable.

---

## Also verified good (no action)

* **Fail-closed conversion path unchanged.** `fetch_as` still calls `take_failure` after every chunk and after
  `pipe.finish()`, so a guard trip stops reading the body immediately; `map_convert` turns both `Limit` and
  `Malformed` into `converter_limit` with the `raw=true` hint.
* **Attribute-guard memory claim holds end to end.** In a debug build (the profile `ci.yml` uses)
  `tests/hostile_rss.rs` peaks at 26 MiB against its 40 MiB assertion and +16 MiB against its +24 MiB assertion;
  the real-binary stdio test peaks at 25 MiB. Margins are adequate, though the +16/+24 one is the tightest
  number in the suite and worth watching if the test ever moves to musl or a different allocator.
* **No panics added.** `tagscan` and the `markdown.rs` deltas contain no indexing, `unwrap`, `expect` or
  recursion on input; `Scan::peak`'s `try_from(...).unwrap_or` is `#[cfg(test)]` and infallible anyway.
* **`selftest.py` additions are real tests, not smoke.** The `-dirty` refusal, the RECORDED verdicts, the
  INVALID path for a stand-in that converts the bomb, and the determinism/shape of the generated bodies are all
  asserted with their reasons.
* **Docs are honest about status.** A-4 is explicitly NOT Done pending E-7, OQ-7 is explicitly still open, the
  G4a pass is explicitly not the memory gate closing, and BENCHMARK §16's fix-pass addendum states what changed
  and that nothing was loosened.

---

## Suggested disposition

Merge. NB-1 and NB-2 are one-line changes that protect CI from future silent failure and future noise; folding
them plus the NB-3 README clause into this PR would be ideal, but none of them blocks. NB-4 to NB-8 can be
carried as follow-ups in EPICS alongside the five already recorded there.

Nothing found in this round calls the G4a figures in BENCHMARK §16 into question.
