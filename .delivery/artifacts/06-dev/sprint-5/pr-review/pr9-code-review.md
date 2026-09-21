# PR #9 code review — Sprint 5 (A-5 pagination, A-6 content types, G4b bench gate)

- Branch: `sprint-5/paginate-content-types-g4b`, head `cd99c95` vs `main` `c86f445`
- Scope reviewed: full diff, 20 files, +1299 / -165
- Reviewer: independent code review (read-only; no edits, pushes, comments or merges)
- Date: 2026-09-21

## Decision

**APPROVE** — no blocking findings. Seven non-blocking findings below.

## What was verified

Static review of the whole diff plus these runs (build dir and `TMPDIR` under `~/.cache`, repo untouched):

| Check | Result |
|---|---|
| `cargo test --locked --all-targets` | 167 lib + 1 + 9 integration tests pass, exit 0 |
| `cargo test --locked --features bench-loopback --test stdio` | 15 pass, incl. `pagination_end_to_end`, `content_types_end_to_end`, exit 0 |
| `bench/fixtures.py generate && bench/selftest.py` | `SELFTEST PASSED`, exit 0 |
| `git diff` over `src/ssrf`, `src/policy.rs`, `src/fetch/dns.rs`, `src/fetch/body.rs`, `src/convert/markdown*`, `src/convert/tagscan*`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, `build.rs`, `scripts/` | empty — no guard, converter-core, dependency or build-surface change |

### Pagination arithmetic (`src/convert/window.rs`) — clean

Walked `Window::push` against the stated priorities and found nothing:

- **Char vs byte offsets / UTF-8 boundaries.** `count()` uses `len()` only on the verified-ASCII fast path and `chars().count()` otherwise; `offset()` returns either `n.min(len)` on the ASCII path or a `char_indices()` index, so every `&rest[..cut]` / `&rest[cut..]` slice lands on a boundary. No possible byte-boundary panic (relevant under `panic = abort`). `every_split_of_multibyte_text_gives_the_same_window` exhaustively re-splits at every boundary of a mixed 2/3/4-byte string and compares — strong test.
- **Overflow.** `skip -= n` is guarded by `n <= self.skip`; `take - taken` by `taken < take`; every `seen` accumulation is `saturating_add`; `render` uses `start.saturating_add(returned)`. `huge_offsets_do_not_overflow` covers `u64::MAX` for both parameters, and the integration test drives `u64::MAX` start/length through the real client.
- **Off-by-one at the window edge.** `more` requires one *confirming* character past the window, so an exactly-full window is not reported as truncated. `more_needs_one_confirming_character_and_raises_the_stop_flag` and `sequential_windows_reproduce_the_text_with_no_gap_or_overlap` (lengths 1, 7, 50, 199, 200, 201 over a 200-char multibyte string) pin the boundary, and `four_sequential_windows_reproduce_the_page` / `pagination_end_to_end` confirm it end to end through HTTP.
- **Bounded memory.** `Window.out` is capped at `take` characters, `take = asked.min(max_length_cap)` (default cap 100 000, `src/config.rs:22`), so ≤ ~400 KiB worst case. `Sniff.held` stays bounded at `SNIFF_BYTES` + one chunk.

### Early stop and the size cap (`src/fetch/mod.rs`)

The read loop order is correct and fails closed: `wire > cap` → `too_large` is checked **before** `pipe.feed`, and `stop()` is polled **after** `take_failure`. So a body that reaches the cap before the window completes is still `too_large`, and a converter error is never swallowed by the early return. Skipping `pipe.finish()` / `conv.finish()` on the early-stop path is correct — the window is already complete and confirmed, and held-back converter output can only be *past* the window. `chunked_window_inside_the_cap_succeeds_and_beyond_the_cap_is_too_large` covers all four corners; `early_stop_ends_the_read_once_the_window_is_confirmed` proves the connection really drops (server-side byte counter, 32 MiB body).

### Content types (`src/convert/mod.rs`) and echo safety

`display_type()` reduces the media type to ≤100 printable-ASCII characters before it reaches `FetchError::UnsupportedContentType`, which is the right guard for upstream-controlled header text (`a_hostile_content_type_is_cleaned_before_it_can_be_echoed`). Moving the type check ahead of the `Content-Length` check is a deliberate, documented reordering and `a_refused_type_reads_no_body` proves no body byte is read. Refusing binary types in `raw` mode too is the safe call.

### CI

`bench.yml` fails closed: each gate step is `continue-on-error: true` + explicit `exit "$rc"`, and the final `if: always()` step re-checks `steps.{idle,peak,idle_bench}.outcome` as the last command under the default `bash -e -o pipefail`. The `conversion_overhead_1mib` step already uses `set -euo pipefail` with an explicit `test result: ok. 1 passed` assertion — no silently-passing step. `permissions: contents: read`, actions pinned by SHA, `persist-credentials: false`, no `pull_request_target`, fork PRs skipped. `arm-bench.yml` keeps `[ "$r1" -le 1 ]` as the last command of the step, so the spike's idle result still gates the step. No injection of untrusted event data into `run:` blocks.

### Stdout purity

Both new stdio tests end with `finish_and_assert_pure()`; no new write path touches stdout.

---

## Non-blocking findings

### NB-1 — `bench/report.py` crashes on the new partial scenario record (confidence 88)

`bench/measure.py:439` emits an INVALID record for a scenario whose window could not be resolved, carrying only `kind, scenario, gate, verdict, valid_runs, runs, invalid_reasons`. `bench/report.py:46` indexes `o['binary_kind']`, `o['metric']` and `o['target_kB']` directly, so it raises `KeyError: 'binary_kind'` on exactly that record.

Impact is diagnostics only, not a false pass: the bench.yml "Job summary" step is `continue-on-error: true` and the gate still fails via the peak step's exit 2. But the summary dies precisely in the case you most need it (a length probe failed in CI), and the artifact is the only other copy. Note the same hardening *was* applied to the inline summary in `arm-bench.yml` (`o.get('metric','n/a')`, etc.) in this PR but not to `report.py`.

Fix: use `.get(..., 'n/a')` for `binary_kind`, `metric`, `valid_runs`, `runs`, and guard `o['target_kB']/MIB`.

### NB-2 — clamp note fires when the caller never passed `max_length` (confidence 90)

`src/server.rs:156`: `clamped_to = (asked > max_length).then_some(max_length)` where `asked = p.max_length.unwrap_or(DEFAULT_MAX_LENGTH)` (5000). `FETCH_MAX_LENGTH_CAP` accepts any positive integer (`src/config.rs:41-50`), so an operator setting the cap below 5000 makes *every* call that omits `max_length` come back with `[max_length was reduced to <cap> characters, the maximum.]` — telling the model its argument was reduced when it never supplied one.

Fix: compute the clamp against the *requested* value only, e.g. `p.max_length.filter(|a| *a > self.max_length_cap).map(|_| max_length)`.

### NB-3 — the A-6 binary refusal is header-only; an untyped binary body still comes back as replacement text (confidence 85)

`for_response` refuses on `Kind::Unsupported`, but a response with **no** (or empty) `Content-Type` is `Kind::Unknown` → `Sniff` (markdown) or `Passthrough` (`src/convert/mod.rs:136,142`). Neither sniffs for *binary*, only for HTML, so a PNG/PDF served without the header returns up to `max_length` U+FFFD characters in both modes. The tool description now promises "images and other binary types are refused"; omitting one header defeats it. `hostile_bodies_never_panic_and_come_back_as_replacement_text` asserts this as intended behaviour, so it is a conscious choice — flagging it because it is the one hole left in the A-6 story.

Not a memory or panic risk (output is capped and valid UTF-8), just wasted tokens and a promise the tool cannot keep. Cheap fix if wanted: in `Sniff::commit`, treat the held prefix as binary when it contains any WHATWG "binary data byte" (0x00–0x08, 0x0B, 0x0E–0x1A, 0x1C–0x1F) and refuse with the same error.

### NB-4 — `g4b-window-start` does not actually verify the early stop it is named for (confidence 85)

`bench/scenarios.py:57` gives the early-stop scenario `min_bytes = WINDOW` (100 000) — a **floor**, never a ceiling. A build that dropped early stop entirely and read all 5 MiB before returning the first 100 000 characters would still satisfy `text_must = ["More content available"]` and the floor, so it would PASS. The early-stop guarantee is currently carried only by the Rust test `early_stop_ends_the_read_once_the_window_is_confirmed`.

I accept that the project deliberately avoids byte ceilings (the `hostile-attrs3` comment explains why for a *refusal* path). But this path is different: the client drops the connection at a deterministic point well inside a 5 MiB fixture, so a generous ceiling (say 2 MiB served) would be stable and would make G4b's headline scenario mean what its name says.

### NB-5 — an empty first page is wrapped in a note (confidence 82)

`src/server.rs:112`: `w.total.filter(|t| w.start >= *t)` is true for `start = 0, total = 0`, so a genuinely empty body — or an HTML page whose conversion yields nothing, e.g. all-script/all-nav — returns `[No content at start_index=0: the content is 0 characters long.]` instead of empty text. That contradicts the module header ("a first page that holds everything carries no footer, so a short page is returned exactly as fetched") and the OQ-5 "no label, wrapper or notice on success" rule. The unit test pins the current behaviour, so it is intentional — but `start_index=0` is the one case where the message has nothing to tell the caller.

Suggestion: restrict the branch to `w.start > 0`, or confirm with the PO that the no-wrapper rule is scoped to non-empty content.

### NB-6 — `g4b-chunked-window-in-cap` borrows another fixture's character density (confidence 80)

`resolve_window`'s `in-cap` case probes `/5mb.html` and applies `total - 3 × WINDOW` as the `start_index` for `/50mb-chunked.html`. That is only valid while `html_5mib` and `html_50mib` convert at the same characters-per-byte ratio; the margin is 200 000 characters (~4 % of the 5 MiB cap). Fail-closed if the generator ever diverges (the scenario would return `too_large` instead of `ok` → INVALID → exit 2), so no false-pass risk — but the coupling is invisible from `fixtures.py` and will be a puzzling CI failure. Consider probing `/50mb-chunked.html` itself with a `beyond-cap`-style request, or asserting the two fixtures' density relationship in `selftest.py`.

### NB-7 — `hostile-attrs3` keeps a stale `max_length: 5 * MIB` (confidence 80)

`bench/scenarios.py:49` is the only peak scenario still carrying the Sprint-4 `5 * MIB` argument; every sibling moved to `WINDOW`. It is harmless today (the value is clamped to 100 000 and the scenario expects `converter_limit`, so the clamp note never appears in a checked reply), but it reads as an oversight next to the others and would start emitting a clamp note if the expectation ever changed. Suggest `WINDOW` for consistency.

---

## Notes for the record (no action)

- `redirect-chain5` was left without an explicit window and now runs with the 5000-character default, which raises the question of whether early stop could starve its `min_bytes` floor. It cannot: the final body is `REDIR_FINAL = 4096` bytes, so the converted output is at most 4096 characters and the window never fills. Stable, since both are constants.
- `Window::new(_, 0)` leaves `total = None` and `more = true` on any non-empty stream. Unreachable in production (`max_length = 0` is rejected at `src/server.rs`), but worth knowing if `Window` is ever reused.
- `Kind::Unsupported(_) => Box::new(Passthrough)` in `for_response` is unreachable (the variant returns `Err` above). Harmless defensive arm.
- Removing the spike's peak measurement from `arm-bench.yml` leaves the product peak covered by the `bench-gate` job (now G4a **and** G4b on all four cells), and `bench-product-peak` still records `g4a-5mib-full` advisorily. The `.get()` hardening of that job's inline summary correctly anticipates the new partial record — see NB-1 for the copy that was missed.
