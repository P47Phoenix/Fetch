# PR #8 code review — Sprint 4 (A-4 conversion, E-4 G4a / build identity)

Reviewer: independent code review (read-only; no edits, pushes, PR comments or merges).
Scope: `sprint-4/convert-memory-gate` head `9d3dc21` against `main` `c13159e` (30 files, +3358/-60).
Commits reviewed: `d5a6dd3` (carry-forward cleanup), `7384c97` (A-4), `8b5b489` + `9d3dc21` (E-4).

**DECISION: REQUEST_CHANGES** — 2 blocking, 9 non-blocking.

---

## What was reviewed

| Area | Files |
|---|---|
| Streaming converter | `src/convert/mod.rs`, `src/convert/markdown.rs`, `src/convert/markdown/tests.rs` |
| Fetch wiring / error surface | `src/fetch/mod.rs`, `src/fetch/tests.rs`, `src/error.rs`, `src/server.rs`, `src/lib.rs` |
| Build identity | `build.rs`, `scripts/build-candidates.sh`, `tests/stdio.rs` |
| Bench harness / gates | `bench/measure.py`, `bench/scenarios.py`, `bench/selftest.py`, `bench/report.py`, `bench/public_check.py`, `.github/workflows/bench.yml` |
| Supply chain | `Cargo.toml`, `Cargo.lock`, `deny.toml` |
| Docs / reports | `README.md`, `docs/BENCHMARK.md` §16, `docs/EPICS.md`, dev reports |

Checks run locally: `cargo fmt --check` clean; converter driven directly from a throwaway out-of-tree crate
(`MarkdownConverter` is `pub`) to reproduce behaviour and measure RSS on hostile 5 MiB inputs. No repository
file other than this report was modified.

Overall this is careful, well-documented work. Memory boundedness holds up under adversarial probing (see
"Verified good" below), the honest-gaps section of the A-4 report is unusually complete, and the E-4 identity
plumbing is fail-closed in the right places. Two defects should be fixed before merge.

---

## Blocking

### B1. Subtree drop rules stop working past `OPEN_CAP` — `<script>`, `<style>` and `<nav>` content leaks into the converted output (confidence 95)

`src/convert/markdown.rs:545-565`, `Md::start`:

```rust
if self.open >= OPEN_CAP && !void {
    return None;          // <-- returns BEFORE the drop rules below
}
// drop rules
let drop = DROP_TAGS.contains(&name) || ...
```

Once 256 counted elements are open, every subsequent non-void start tag returns `None` before the
`DROP_TAGS` / `hidden_by_attrs` / `noisy` checks run, so no `Skip` is created. The element itself produces no
markdown, but its **text still flows** through the `doc_text!` handler and is emitted, because `Md::allowed()`
only consults `self.skip`.

Reproduced (converter driven directly, release build):

```
depth=255  script_leaked=false style_leaked=false out="body"
depth=256  script_leaked=true  style_leaked=true  out="SECRET_JS_PAYLOAD().x{color:red}body"
depth=400  script_leaked=true  style_leaked=true  out="SECRET_JS_PAYLOAD().x{color:red}body"
depth=300  nav_leaked=true
```

(input: `"<div>"*depth + "<script>SECRET_JS_PAYLOAD()</script><style>.x{color:red}</style><p>body</p>" + "</div>"*depth`)

Why it matters:

* It contradicts the module contract stated in `markdown.rs:6-11` and the A-4 report ("dropped with their whole
  subtree: script style noscript …"), and it directly undermines the E-7 acceptance criterion "no `<script>` in
  the output".
* `OPEN_CAP` (256) sits far below lol_html's own ceiling — the A-4 report itself notes ~5,000 open elements
  convert fine and ~20,000 hit the 2 MiB limit — so there is a wide, easily reachable band where the drop rules
  are silently off. A hostile page needs only 256 nested `<div>`s to push arbitrary `<script>` text into the
  model's context; this converter is a trust boundary for LLM input, so silently disabling the filter is worse
  than failing closed.
* It is not listed in the A-4 report's "Honest gaps", so it reads as unintended rather than accepted.

Fix: move the drop-rule block (and, ideally, the landmark check) above the `self.open >= OPEN_CAP` early return,
so a dropped subtree still registers its `Skip` regardless of depth. `Skip` entries are already not counted
against `open` (`counted = !matches!(act.kind, Kind::Skip(_))`, `markdown.rs:1089`), so this does not weaken the
cap. Add a regression test at depth `OPEN_CAP + 1` asserting no `script`/`style`/`nav` text in the output.

Related (same root cause, fold into the fix): the landmark check at `markdown.rs:567-582` is also unreachable
past the cap, so a `<main>` nested deeper than 256 elements is ignored and the page falls back to whole-body
mode.

### B2. The A-4 performance gate in `bench.yml` can never fail — `tee` masks the test's exit status (confidence 90)

`.github/workflows/bench.yml`, new step *"Conversion overhead of a 1 MiB page (A-4 AC: p95 <= 500 ms on
aarch64; BENCHMARK.md section 9 method)"*:

```yaml
run: |
  set -eu
  cargo test --locked --release --lib conversion_overhead_1mib -- --ignored --nocapture 2>&1 | tee out/convert-overhead.txt
```

GitHub Actions runs `run:` blocks with `bash -e {0}`, and the block sets `set -eu` without `-o pipefail`. A
pipeline's exit status is that of its last command, so `cargo test` failing (including the
`assert!(p95 <= 500.0, ...)` at `src/convert/markdown/tests.rs:512`) is swallowed by `tee` and the step goes
green. The step has no `continue-on-error`, and its name asserts an acceptance criterion, so a green run reads
as "the 500 ms AC is enforced in CI" when nothing enforces it.

This is exactly the class of false-pass path this project otherwise guards against carefully (`gate="none"`
scenarios are deliberately kept fail-closed; `selftest.py` asserts the refusal exit codes).

Fix, whichever matches the intent:
* enforce — `set -euo pipefail` (or `PIPESTATUS`/process substitution); or
* record only — add `continue-on-error: true`, drop the AC claim from the step name, and say in the step and in
  BENCHMARK.md that the figure is recorded, not gated.

Either way the two should agree. Today the step name, the missing `continue-on-error`, and the A-4 report's
"wired into bench.yml … to record it" say three different things.

---

## Non-blocking

### N1. `Tbl::row` can hold ~16 MiB, contradicting its own comment (confidence 82)
`markdown.rs:44-46` says "Table cell and row buffers (ADR-002: row buffer <= 64 KB)", but the code caps each
*cell* at `CELL_CAP` (64 KiB) and allows `ROW_CELLS` (256) of them, i.e. up to 16 MiB in `self.tbl.row` before
`close_row` runs. In practice the 5 MiB `max_bytes` default bounds it, and my probe of a 5 MiB single-row table
peaked at +6 MB RSS and then returned `Limit` — but `FETCH_MAX_BYTES` is env-configurable, so the stated
invariant is not the enforced one. Either enforce a running row-byte total against 64 KiB (calling
`abandon_table` when it is crossed) or correct the comment and ADR reference.

### N2. `<base href>` is ignored, so link destinations can be wrong on pages that use it (confidence 80)
`base` is in `HEAD_LEGAL` and `<head>` is a drop tag, and `Md::structure` has no `base` arm, so relative `href`
and `src` always resolve against the final request URL (`markdown.rs:782-803`). Pages that set `<base href>`
will get systematically wrong absolute links. Not in the report's honest-gaps list; worth either implementing
(capture `base.href` before the head is skipped) or recording as a known approximation alongside the other
tier-1 caveats.

### N3. `MarkdownConverter::push` silently succeeds after `finish` (confidence 80)
`markdown.rs:1175-1177`: after `finish` takes `self.rw`, a later `push` hits `let Some(rw) = self.rw.as_mut() else { return Ok(()) }`
and reports success while discarding the input. `failed` is false on that path, so the caller gets no signal.
A `debug_assert!`, or returning `ConvertError::Malformed`, would make the trait contract explicit. Same shape in
`Sniff` is fine because it delegates.

### N4. `build.rs` reports `HEAD` even when the tree is dirty (confidence 82)
`build.rs:78-92` uses `git rev-parse --verify HEAD` only, and `scripts/build-candidates.sh` compares against the
same value, so a build from a modified working tree claims a commit that does not describe its source. BENCHMARK
§ "Build identity" claims the bench build is "provably the same commit and lock file as the shipped one" — true
for the *pair*, but not for the source-to-commit mapping. Cheap hardening: append `-dirty` when
`git status --porcelain` is non-empty (keeping the 40-hex form the scripts match on is then a deliberate choice —
the regex in `build-candidates.sh`/`measure.py` would need widening), or at minimum note the assumption in the doc.

### N5. `FETCH_MCP_COMMIT` can forge the identity the gate checks (confidence 80)
`build.rs:79-83` lets the environment set the commit, and `measure.py --gate` / `build-candidates.sh` only check
that the two binaries *agree* and are 40 hex. Setting `FETCH_MCP_COMMIT` on both builds satisfies every check.
That is an acceptable escape hatch for `git archive`, but it should be stated in BENCHMARK.md §"Build identity"
next to the `commit=unknown` sentence, which currently implies the only non-git outcome is `unknown`.

### N6. `rerun-if-changed=.git/HEAD` is emitted unconditionally and misses worktrees (confidence 80)
`build.rs:96-104`: the directive is printed even when `.git` does not exist (cargo then treats the missing path
as changed and reruns the script on every build), and `fs::read_to_string(".git/HEAD")` fails when `.git` is a
*file* (git worktrees and submodules), so the branch ref is never registered and a branch switch will not rebuild
— leaving a stale `commit=` in `--version`. Gate a single `println!` on `Path::new(".git").exists()` and resolve
the gitdir when `.git` is a file.

### N7. `measure.py::call_many` surfaces a bare `queue.Empty` on timeout (confidence 80)
`bench/measure.py`, `call_many`: `self.q.get(timeout=max(0.1, end - time.time()))` raises `queue.Empty` with no
context when a concurrent sample stalls, unlike the sibling `call()` paths which raise descriptive
`RuntimeError`s. Given `g6-concurrent10` queues 7 requests behind `FETCH_MAX_CONCURRENCY=3`, a slow runner will
produce an opaque failure. Wrap it with the scenario name and how many replies were still outstanding.

### N8. `public_check.py` depends on a live third-party page from CI (confidence 80)
`DEFAULT_URL` is a specific Wikipedia article whose size and markup can change at any time, and the step makes an
outbound network call on every bench run. It is advisory (`continue-on-error: true`, verdict recorded), and
BENCHMARK §16 already flags the "about 2 MiB, not 5 MiB" deviation — but the *stability* of the reference page is
not recorded. Record the observed `output_chars`/page size in the JSONL record (it already captures
`output_chars`) and say in the doc that a change in the page invalidates comparison across runs.

### N9. Dead parameter in the converter test helper (confidence 85)
`src/convert/markdown/tests.rs:13-14`: `fn run_chunks(html: &str, chunks: &[&str])` opens with `let _ = html;`.
The parameter is never used — drop it and update the ~6 call sites, or the next reader will assume the helper
cross-checks whole-vs-chunked (which is what `md()` + `run_chunks` do *at the call sites*, not here).

---

## Verified good (no action)

* **Memory boundedness.** Driving the converter with 5 MiB hostile inputs (one `<tr>` of 20 KiB cells, 5 MiB of
  nested `<blockquote>`, 5 MiB of unclosed `<td>`, a single 5 MiB paragraph, a 1 M-entity flood, a realistic
  5 MiB page): worst single output step 328 KiB, worst RSS delta 6 MB, and the pathological shapes return
  `ConvertError::Limit` rather than growing. Nothing accumulates the page.
* **Panic safety under `panic = "abort"`.** No indexing, `unwrap` or recursion on input in the new converter
  paths. `incomplete_entity_len` (`markdown.rs:1012-1028`) always lands the split on a `&` byte, so
  `buf.split_at(split)` cannot straddle a char boundary; `lock()` handles poisoning; the `h1`..`h6` byte
  subtraction is only reachable from the matched arm; `abandon_table` → `put_main` cannot recurse (the cell is
  taken first).
* **No credential leak through the new converter base URL.** `read_body` passes `parsed` (the final hop) as the
  link base, but `ssrf::check_url` rejects userinfo on the first hop and on every redirect target
  (`src/ssrf/mod.rs:85`), so a base carrying `user:pass@` cannot reach `Url::join`. The new
  `a9_echo_has_no_userinfo_or_fragment_end_to_end` test covers the echo path end to end.
* **SSRF/guard surface unchanged.** `fetch_as` only threads a `Mode` through; validation order, the pinned
  dialer, the redirect loop, the deadline and the concurrency slot are untouched, and `fetch` is preserved as
  `fetch_as(.., Mode::Raw, ..)` so all A-3b tests still exercise the same path.
* **Converter failure is fail-closed.** `take_failure` is checked after every chunk and after `pipe.finish()`,
  and `conv.finish()` is mapped to `converter_limit`; the server returns a tool error and discards the partial
  window rather than returning truncated markdown.
* **Stdout purity.** `#![deny(clippy::print_stdout)]` still holds; `build.rs`'s `println!`s are cargo
  directives; `tests/stdio.rs` asserts `--version` is exactly one line and now also parses the identity fields.
* **`deny.toml` MPL-2.0 exceptions are complete and minimal.** Of the 29 crates the lock gains, exactly four are
  MPL-2.0 (`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`) and all four are listed. `servo_arc`,
  historically MPL-2.0, is `MIT OR Apache-2.0` at the locked version; `encoding_rs`
  (`(Apache-2.0 OR MIT) AND BSD-3-Clause`), `foldhash` (Zlib) and the rest are covered by the existing allow
  list. `cargo deny` will catch a future lock bump that reintroduces MPL elsewhere.
* **`build.rs` SHA-256 is correct and deterministic.** The compression loop matches FIPS 180-4 (verified by
  hand), there are no timestamps or host paths, and `embedded_lock_hash_matches_cargo_lock` cross-checks against
  `sha256sum` when available.
* **Gate plumbing is fail-closed where it claims to be.** `--gate` now refuses without `--peer-binary`, on a
  missing/`unknown`/mismatched identity, and `selftest.py` asserts each refusal and its exit code.
  `g6-concurrent10` is correctly `RECORDED` (excluded from `missed`) while still failing closed on validity, and
  the G4b stubs still refuse as "not implemented".
* **Docs are honest.** BENCHMARK §16 states the single-run limitation, the `g6` queueing caveat
  (`FETCH_MAX_CONCURRENCY=3`, so not 10 simultaneous downloads), the ~2 MiB public page deviation, and that a
  G4a pass does not close the memory gate. `cargo fmt --check` is clean.

---

## Suggested disposition

Fix **B1** (correctness/trust boundary) and reconcile **B2** (gate intent vs behaviour); both are small. N1-N9
can be folded into the same fix-pass or deferred with a line in the dev report. Nothing here calls the G4a
figures in BENCHMARK §16 into question: B1 affects output content, not memory, and B2 affects only the
overhead-recording step, not the `--gate` runs.
