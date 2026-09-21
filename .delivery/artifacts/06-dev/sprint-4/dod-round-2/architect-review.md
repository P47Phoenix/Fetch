# Architect + security review, round 2: PR #8 (head 3e02c70, code f9e4c9d)

Role: Solution + Security architect, read-only. Method: `git archive 3e02c70` to ~/.cache/arch2/src, offline `--locked` release build,
`cargo test --release --locked` (147 lib + 1 hostile_rss + 9 stdio integration pass, 1 ignored bench), `check-release-features.sh` OK,
`--self-test` OK (needs TMPDIR outside the small /tmp quota). Probes: uncommitted example `examples/probe.rs` in the scratch copy, driving
`MarkdownConverter` exactly as the fetch path does (16 KiB slices, and 1-byte slices for most cases), 75 scenarios, one process each (VmHWM).
Not done: a fresh real-stdio run of my own scenarios (the repo's own real-stdio tests, which pass, cover depth 255/256/300/10000 drops and a
3-concurrent attribute bomb under 40 MiB; my 3-concurrent runs were 3 threads in one process on the same converter). No clippy/deny re-run (CI did).

## Decision: DONE (0 blocking)

## B1 (drop rules past the depth cap): fixed, not bypassable in my probes
Script, style, iframe, nav (and `aria-hidden`) output is dropped at depth 255, 256, 10000 (whole and 1-byte slices), inside 300 nested
table/tr/td, ul/li, after 300 unclosed `<p>` plus unclosed b/i/span/a, inside `<svg>`, `<noscript>`, `<template>`, `<textarea>`, with
attributes on script and uppercase tags. Nesting deeper than about 20,000 (and 100,000 `<div>`, 100,000 `<table><tr><td>`) fails closed `Limit`.
Code: the drop block now sits before the `over_cap` return in `Md::start` (markdown.rs 549-568); skips are not counted in `open`.

## B2 (memory bound): fixed, could not be defeated
Per-process VmHWM (baseline 3 MiB), worst single scenario 8.5 MiB (a 1.9 MiB table cell / pre / entity text, which is legitimate content).
3 concurrent (threads): table_bigrow+pre_flood+entity_noend 17.2 MiB; bigvalue+4 MB of tags each just under the cap+valued-attribute mix 12.6 MiB;
table+unquoted 1.9 MiB value+unbalanced quote 15.0 MiB; doctype+PI+svg 12.4 MiB. All under 40 MiB (gate 5.8 to 17.6 MiB peak).
Refused (`Limit`, 5-7 MiB peak, before any allocation): 1025 attributes; `<div` with no `>` for 1.9 MiB (tiny attrs and words); duplicate attrs
(`id=a` x380k); `>` inside quoted values then a bomb; `<a title="> > >">` then bomb; attribute bomb inside `<svg>`/foreignObject; attribute bomb
spelled as `a/`, `=x`, `b\x0c`, `b"c`, `b\0`, multibyte names; bombs inside comments, CDATA, `<script>`, `<style>`, `<textarea>`, `<?` (over-approximation
works as designed, at the cost of the false refusals below); 1-byte slicing gives the same verdicts (chunk boundaries do not matter).
Passed cheaply (not bombs, correct): 1000-attribute tags repeated 2000 times (4 MB, 7.5 MiB); a 1.9 MiB attribute value quoted or unquoted (7 MiB);
unbalanced quotes (7 MiB); end tag with 950k attributes (7 MiB, lol_html does not retain them); doctype/PI bombs (7 MiB); entity floods incl. `&#` x900k (8 MiB);
250 nested `<a>` with 7 KB hrefs (5 MiB); table with 190k cells (6 MiB).
Remaining property, stated honestly: the guard is a model of lol_html's lexer plus an over-approximation, not a proof. I found no counter-example
in the classes above (the only lexer disagreements possible are in the safe direction, extra candidates). Failure is fail-closed: `Limit` maps to `converter_limit`.

## False refusals (acceptable, documented)
`a <b` then more than 1024 words with no `>` is refused (measured: 1000 words ok, 1030 refused; a 2 MiB such page refused). Also refused: 1025+ attributes in a real tag,
a `<`+letter inside a script/style/comment/CDATA/textarea followed by 1024+ tokens with no `>` (large inline JS with many `<x` comparisons and no `>` anywhere near is
the realistic case). README lines 80 and 99 and the module header state it, `raw=true` bypasses. Acceptable for a fail-closed gate. Not measured yet: the false-refusal rate on the E-7 real pages (follow-up).

## Other re-checks
- MPL-2.0: cargo tree shows exactly cssparser 0.36.0, cssparser-macros 0.6.1 (proc-macro), dtoa-short 0.3.5, selectors 0.37.0; deny.toml has those four exceptions and nothing else. README line 21 and ci-branch-protection Licence section are accurate (unmodified use leaves own files under their licence, source-availability duty on distribution, vendoring/modifying triggers MPL-2.0 for the changes). OQ-7 stated as still open with the owner, `publish = false` kept. Not decided in the code.
- No content label added (OQ-5 open): no label in src/convert.
- build.rs: `FETCH_MCP_COMMIT` verbatim when set (documented as TRUSTED and unchecked, git-archive/Docker escape hatch); else `git rev-parse HEAD` (40 hex) plus `-dirty` on tracked changes (`status --porcelain --untracked-files=no`); `unknown` without git. Deterministic (no time, host, network); a failing `git status` yields `-dirty` (fails safe, gate tooling matches 40 hex only). rerun-if-changed covers src, Cargo.*, packed-refs, worktrees, env. In my `git archive` build the commit is `unknown`, as documented.
- Release guard, a3b-merge-gate, ci.yml: diff 9d3dc21..3e02c70 touches only bench.yml under .github; check-release-features.sh unchanged; guard OK and self-test OK.
- Workflow: bench.yml overhead step now `set -euo pipefail` (only functional change; other edits are comments and two extra recorded scenarios). `tee` no longer masks a failing assertion (the dev report shows it proven locally, exit 101). Permissions, triggers, fork-skip unchanged.

## Non-blocking
1. README line 95 and the module header say elements marked `hidden`, `aria-hidden`, `display:none` are dropped "with their whole content", but `attr_ok` exempts optional-end and void tags (`OPTIONAL_END`: p, li, td, tr and so on; comment in markdown.rs 60-61). `<p hidden>HID</p>` and `<p style="display:none">` are emitted at every depth (verified at depth 10, 256, 10000; `div` variants are dropped). Not a depth regression and not a script/style bypass, but the doc overstates; document the exemption or handle the p/li case.
2. The 1024-attribute guard costs one 8-state step per byte inside candidate tags (1 MiB of `<a a a` in 1-byte slices took about 1.3 to 3 ms per 2 KB, i.e. fine); no CPU concern seen (worst 67 ms for 4 MB).
3. False-refusal rate on real pages is unmeasured (E-7); record it when E-7 exists, and consider stopping candidate scans inside known raw-text contexts only if it shows up (not needed now).
4. Carried from round 1, still open and recorded in EPICS: no early stop before A-5, untyped-body sniffing (A-6), no link-density rule, ADR-002 status, lol_html 3.x.
5. Environment note: the self-test and `pwd` hit a Disk quota error on /tmp; results above used TMPDIR under ~/.cache.
