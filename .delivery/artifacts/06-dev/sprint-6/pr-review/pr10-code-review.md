# PR #10 — independent code and security review

**Branch:** `sprint-6/a7-error-taxonomy` (head `ec9dab0`) vs `main` (`b633e60`)
**Scope:** A-7 (error taxonomy) and B-1 (end-to-end blocked-class SSRF test)
**Reviewer:** independent review agent, read-only (no edits, pushes, comments or merges)
**Date:** 2026-09-21

## Decision

**REQUEST_CHANGES** — two small, in-scope message/logging defects in the artifact the PR exists to deliver.
Nothing found is a security regression; the change is otherwise well executed.

## What was reviewed

- Full diff `main...HEAD` for `src/`, `tests/`, `Cargo.*`, `.github/`:
  `src/error.rs`, `src/server.rs`, `src/fetch/tests.rs`, `tests/stdio.rs` only.
  `src/ssrf/*`, `src/policy.rs`, `src/fetch/dns.rs`, `src/fetch/body.rs`, `src/convert/*`, `Cargo.toml`,
  `Cargo.lock` and `.github/workflows/*` are **unchanged** — consistent with the EPICS note that B-1 is test
  depth on existing A-3a code, and with A-7 touching only error text and argument validation.
- Docs diff: `README.md`, `docs/EPICS.md`, `.delivery/artifacts/05-plan/po/sprint-plan.md`, Sprint 5 records.
- Verification run locally (`CARGO_TARGET_DIR=~/.cache/fetch-review-target`):
  - `cargo test --all-features` — **189 tests pass** (173 lib, 15 stdio, 1 hostile_rss), 0 failed.
  - `cargo clippy --all-targets --all-features -- -D warnings` — clean.
  - `cargo fmt --check` — clean.
  - Black-box probe of the built binary over stdio with 14 hostile/edge argument shapes (see Appendix).

## Blocking

### B1. 4xx retry advice is wrong for 429 and 408 (confidence 88)

`src/error.rs:96-99` — every 4xx is given the same closing clause:

```
400..=499 => "the server refused the request with HTTP status {code}{reason}; retrying the same URL is unlikely to help"
```

For `429 Too Many Requests` the resulting text is:

> `error[http_error]: the server refused the request with HTTP status 429 (Too Many Requests); retrying the same URL is unlikely to help`

Retrying the same URL after a delay is exactly the correct response to a 429, and likewise for `408 Request
Timeout` (and `425 Too Early`). A-7's acceptance criterion is that the message tells the agent *what it can do
about it*; here it tells the agent the opposite of the right thing, and an LLM client that follows the text will
abandon a request it should have backed off and retried. This is the one place in the taxonomy where the new
advice is actively misleading rather than merely terse.

**Suggested fix** — carve the retryable 4xx out before the general arm, e.g.

```rust
408 | 425 | 429 => write!(f, "the server refused the request with HTTP status {code}{reason}; it may work if retried after a delay"),
400..=499 => write!(f, "... ; retrying the same URL is unlikely to help"),
```

and add the 429 case to the table in `server.rs::every_cause_sets_the_flag_and_names_the_cause`.

### B2. Internal-error log line breaks the project's own stderr line format (confidence 90)

`src/server.rs:113-118`:

```rust
crate::obs::stderr_line(format_args!("ERROR internal {detail}"));
```

`src/obs.rs:1-3` documents the line format as `level event key=value ...`, `Level::as_str` emits lowercase
(`error`, `warn`, `info`, `debug`), and every other call site follows it — `src/main.rs:25,36,44` emit
`error config ...`, `warn build marker ...`, `error serve ...`. `tests/stdio.rs:425` even asserts
`err.starts_with("error serve")`. The new line is the only uppercase one in the codebase, so a log consumer or
operator filtering on the documented `^error ` prefix silently loses exactly the diagnostics A-7 introduced this
line to preserve.

**Suggested fix:** `format_args!("error internal {detail}")` (one word). Note the level threshold is not an
issue: `enabled()` emits `Error` at every configured level, so bypassing `obs::log` does not change *whether* it
is written, only its shape.

## Non-blocking

### N1. Timeout budget of 400 ms used for non-timeout cases — flake risk (confidence 80)

`src/fetch/tests.rs:212` builds the shared client with `limits(400, 1 << 20, 3)` and then reuses it for the 404,
500 and `image/png` cases as well as the intended timeout case. Every other success-shaped test in the file uses
5000–20000 ms; 400 ms is reserved elsewhere (lines 634, 659, 765) for tests where the timeout *is* the subject.
On a loaded self-hosted ARM runner a loopback round trip plus client construction under parallel test load can
exceed 400 ms, turning those three assertions into intermittent `error[timeout]` failures. Suggest a 5000 ms
client for the status/content-type cases and keeping the 400 ms one only for the timeout case.

### N2. Nothing asserts that the internal detail actually reaches stderr (confidence 85)

`an_internal_error_is_generic_and_leaks_no_detail` (`src/server.rs:422-435`) asserts the caller text and the
absence of the detail, which is the important half. The other half of the A-7 AC — "the detail is logged" — is
unasserted, and `FetchError::Internal` has no end-to-end producer test (it is only reachable from
`src/fetch/mod.rs:143,293,312`). Related design nit: `error_result` is a `#[must_use]` constructor-shaped
function with a hidden I/O side effect, so every unit test that calls it writes to the test harness's stderr.
Consider `error_result(e, &mut dyn FnMut(fmt::Arguments))` or a separate `log_internal(e)` call at the one
production site, which would also make the logging testable.

### N3. `arguments` that is not a JSON object still yields a JSON-RPC error, and nothing pins it (confidence 82)

Probing the built binary: `"arguments": []`, `"arguments": "x"` and `"arguments": 5` all return JSON-RPC
`-32601 "tools/call"` — not an `isError` result, and not even the semantically correct `-32602`. ADR-006
amendment 2026-09-19 says parameter rejection is "never a JSON-RPC error"; README documents the `[]` case
honestly as an rmcp limitation, so this is a known, pre-existing gap rather than a regression (the same shape
occurs on `main`). Two improvements: (a) `tests/stdio.rs` pins the deep-nesting rmcp behaviour but not this one,
so an rmcp upgrade could change it without any test noticing — add a case; (b) the doc comment at
`src/server.rs:31-34` ("every argument failure ... is the same `error[invalid_argument]`") reads as absolute and
would be clearer with "for an object `arguments`; a non-object `arguments` is rejected by rmcp before our code
runs (see README)".

Confirmed improvements from this PR in the same area: `"arguments": null` and a missing `arguments` key now
return `error[invalid_argument]: url: is required` instead of an rmcp-shaped failure.

### N4. B-1's new test is not in the release-profile merge gate (confidence 80)

`.github/workflows/ci.yml:98` filters the `a3b-merge-gate` job to
`a3b_merge_gate refusal_ dial_once differential`.
`b1_every_blocked_class_by_name_mixed_and_literal_is_refused_before_any_connection` matches none of these, so
the broadest blocked-class test in the suite runs only on the dev profile in the `test` job. Given B-1's subject
matter, consider adding it to the gate list (and to the `grep -Eq` proof loop below it).

### N5. Test-quality nits in `b1_...` (confidence 80)

- The doc comment claims "the message never carries the address" for the whole test, but the literal half of the
  loop (`src/fetch/tests.rs:517-522`) asserts only `code(&res) == "blocked_target"`. The property does hold —
  `check_url` emits `IP address is not public ({category})` — it is simply unasserted.
- `assert_eq!(srv.accepted(), 0)` has no positive control: it would also pass if the fixture server never
  accepted anything. The `r.count() == lookups` assertion partly compensates. A single successful fetch through
  the same `srv` at the top of the test would close it.
- `93.184.216.34` is a real public address used as the "public" filler. It is never dialled (the policy refuses
  the whole answer set if any member is blocked, `src/ssrf/resolver.rs:30-64`), and the documentation ranges are
  themselves blocked (`src/ssrf/ranges.rs:39,43,44`), so this is the correct choice — noted only so a future
  reader does not "fix" it into a blocked range and make the mixed cases pass for the wrong reason.

### N6. Schema `required` is now asserted by hand (confidence 80)

`#[schemars(extend("required" = ["url"]))]` (`src/server.rs:36`) replaces the derived `required` list because
every field is `Json` + `#[serde(default)]`. The serde types and the advertised schema (`#[schemars(with = ...)]`)
are now independent, so a future field cannot be made required by the compiler — it must be added to the
`extend` list by hand. `tests/stdio.rs:132` guards today's value; worth a comment at the attribute pointing at
that test.

## Checks that came back clean

| Area | Result |
| --- | --- |
| Internal detail leakage to the caller | Clean. `Display for Internal` returns `INTERNAL_MESSAGE` only; asserted by test. |
| Attacker-controlled text in messages | Clean. HTTP reason phrases come from `reqwest`'s fixed canonical table, not the wire. Content types go through `convert::display_type` (printable ASCII, ≤100 chars, essence only). Redirect and network messages are static strings. No upstream data reaches an error message. |
| Address/URL leakage | Clean. `blocked_target`/`dns_failure` give a category only; verified by `b1_...` and by probe (`http://\0IGNORE.../` returns only "host contains whitespace, control or non-ASCII characters", no echo of the input). |
| Log-line injection via `Internal` | Clean. All three `Internal` constructions use fixed strings, so no newline forging in the stderr line. (Would become a concern if a formatted detail is ever added — worth a comment.) |
| stdout purity | Clean. `error_result` writes to stderr; `finish_and_assert_pure` passes in all 15 stdio tests, including the probe run. |
| Panics under `panic = "abort"` | Clean. New code has no `unwrap`/`expect`/indexing/arithmetic. `StatusCode::from_u16` errors are handled; `Number::as_u64` is checked; `Window` uses `saturating_add` and the `render` footer uses `saturating_add`. |
| Argument edge cases | Clean. Probe: `-1`, `1.5`, `1e30`, `2^64`, `"9"`, `true`, `0`, `[...]`, `null`, absent, `u64::MAX` `start_index`, 3000-char host, extra unknown fields — all return an `isError` `error[invalid_argument]: <field>: ...` (or proceed correctly), no panic, no protocol error, no hang. |
| Sprint 5 behavioural regressions | Clean. `pagination_end_to_end`, `content_types_end_to_end`, `redirect_result_begins_with_final_url_and_status...`, clamp-note semantics (`p.max_length.filter(...)`) and the `raw` default all unchanged and passing. |
| SSRF/guard regressions | Clean. No change under `src/ssrf/`, `src/policy.rs`, `src/fetch/dns.rs`; `a3b_merge_gate`, the four refusals, `dial_once` and the differential corpus all pass. |
| Tests assert text **and** `isError` | Yes. `failure_text` and `error_result` assertions check `is_error == Some(true)` plus exact message text in every new case. |
| Flaky tests | One risk only, N1. |
| CI/workflow changes | None in the diff; see N4 for a suggested addition. |
| Docs accuracy | Good. README's rewritten error section, the EPICS A-7/B-1 status notes (including the honest `panic = "abort"` deviation) and sprint-plan revision 15 all match the code. |

## Appendix — stdio probe results (built binary, `--all-features`)

```
arguments array    -> PROTOCOL ERROR {"code": -32601, "message": "tools/call"}
arguments string   -> PROTOCOL ERROR {"code": -32601, "message": "tools/call"}
arguments null     -> isError=True  error[invalid_argument]: url: is required
no arguments key   -> isError=True  error[invalid_argument]: url: is required
arguments number   -> PROTOCOL ERROR {"code": -32601, "message": "tools/call"}
max_length 1e30    -> isError=True  error[invalid_argument]: max_length: must be a non-negative whole number
max_length 2^64    -> isError=True  error[invalid_argument]: max_length: must be a non-negative whole number
start_index u64max -> isError=True  error[dns_failure]: hostname did not resolve   (no overflow, window saturates)
url as array       -> isError=True  error[invalid_argument]: url: must be a string
url with NUL + LF  -> isError=True  error[invalid_argument]: url: host contains whitespace, control or non-ASCII characters
url 3000-char host -> isError=True  error[invalid_argument]: url: malformed host name
max_length true    -> isError=True  error[invalid_argument]: max_length: must be a non-negative whole number
raw 1              -> isError=True  error[invalid_argument]: raw: must be true or false
unknown extra key  -> isError=True  error[blocked_target]: IP address is not public (private)
stdout: every line a JSON-RPC 2.0 frame; stderr: build markers only; exit 0.
```
