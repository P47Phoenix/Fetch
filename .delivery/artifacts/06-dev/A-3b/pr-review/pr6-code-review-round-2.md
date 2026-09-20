# PR #6 code review — round 2 (A-3b guarded streaming fetch + E-8)

- **Branch**: `sprint-2/guarded-fetch`; review head as commissioned: `5c9a0dd`; base `main` @ `090e86c`
- **Also present on the branch**: `d4da4ef` ("arm64 RSS smoke figures, fix-pass report"), committed while this review
  was running. It touches only `docs/BENCHMARK.md` (new section 14), the fix-pass report and a DoD round-2 note —
  **no source, workflow, manifest or script change**, so every code finding below applies unchanged to `d4da4ef`.
  One untracked file is in the tree (`.delivery/artifacts/06-dev/A-3b/dod-round-2/qa-review.md`).
- **Reviewer**: independent code review, round 2 (read-only; nothing edited, pushed, commented or merged in this repo)
- **Date**: 2026-09-20
- **Decision**: **APPROVE** — the round-1 blocking item is fixed correctly, and the fix pass introduced no regression
  in any of the areas re-checked.

## Scope of this round

1. Confirm round-1 **B-1** (`map_transport` string-matching a Debug string that embeds the request URL) is genuinely fixed.
2. Re-check the fix pass (`c053d8c..5c9a0dd`) for regressions in: SSRF / DNS pinning, memory bounds, panics, stdout
   purity, dependencies, the release-feature guard, and the CI workflow change (new arm64 job).

Files changed by the fix pass: `src/fetch/mod.rs`, `src/fetch/dns.rs`, `src/fetch/tests.rs`, `src/ssrf/mod.rs`,
`src/error.rs`, `src/lib.rs`, `.github/workflows/arm-bench.yml`, plus docs/ADR/EPICS text.
**No change** to `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.github/workflows/ci.yml`, `scripts/check-release-features.sh`,
`src/fetch/body.rs`, `src/server.rs`, `src/policy.rs`, `src/main.rs` or `tests/stdio.rs`.

### Verification actually run locally

| Check | Result |
|---|---|
| `cargo test --locked` (lib + `tests/stdio.rs` + doc) | **91 + 9 passed, 0 failed** (was 85 + 9 in round 1) |
| The six new/changed tests run by name (`--exact`) | all 7 named tests pass |
| `cargo test --locked --release --lib -- a3b_merge_gate refusal_ dial_once differential` (the `a3b-merge-gate` job's command) | **10 passed, 0 failed** on the release profile |
| `cargo clippy --locked --all-targets -- -D warnings` | clean |
| `scripts/check-release-features.sh --self-test` | `self-test OK`, exit 0; all five positive controls still fail as designed |
| `actionlint` over `.github/workflows/` | clean, exit 0 |
| **Negative control for the B-1 fix** (scratch `git archive` extract in `/tmp`, fix reverted) | the new regression test **FAILS** with `bad_response` — see below |
| **Empirical Debug probe** (scratch extract, `eprintln!` of the matched text) | URL is gone from the matched text — see below |

---

## B-1 (round-1 blocking) — fixed correctly, and the regression test is non-vacuous

`src/fetch/mod.rs:276-299`:

```rust
fn map_transport(e: reqwest::Error) -> FetchError {
    if e.is_timeout() { … }
    // Classify from the error's kind first and strip the URL before any text is inspected: reqwest's Debug output
    // embeds the request URL, and the URL (path, query or a redirect Location) is upstream-controlled text.
    let is_connect = e.is_connect();
    let e = e.without_url();
    let mut chain = format!("{e:?}").to_ascii_lowercase();
    …
    if is_connect { return FetchError::Network("could not connect to the host".into()); }
```

This is exactly the fix suggested in round 1: `is_connect()` is captured **before** the move, `without_url()` is applied
**before** the Debug text is produced, and the `contains("header")` probe therefore no longer sees the URL.

**Empirically confirmed on the fixed tree** (probe printing the matched text from inside `map_transport`, on a
connection-refused fetch of `http://public.test:<dead port>/api/headers`):

```
PROBE_DEBUG=reqwest::Error { kind: Request, source: hyper_util::client::legacy::Error(Connect,
  ConnectError("tcp connect error", 127.0.0.1:42831, Os { code: 111, kind: ConnectionRefused, … })) }
```

No `url:` field, no path, no query. Round 1's probe on the same code path showed
`url: "http://127.0.0.1:9/api/headers"` in that same position.

**The new regression test is non-vacuous.** `src/fetch/tests.rs:720-742`
(`url_text_containing_header_never_changes_the_transport_class`) covers all three vectors round 1 named — a path
(`/api/headers`), a query (`?header=1&too-large=too%20many%20headers`), and an **upstream-controlled redirect
`Location`** whose next hop cannot connect. I reverted only the two added lines in a scratch extract outside the repo
and re-ran it:

```
test fetch::tests::url_text_containing_header_never_changes_the_transport_class ... FAILED
assertion `left == right` failed: http://public.test:45121/api/headers:
  Err(BadResponse("the response headers are too large or malformed"))
  left: "bad_response"   right: "network_error"
```

So the test would have caught the original defect, and it passes on the fixed code. The remotely-steerable
classification path (redirect `Location` → next hop's transport failure) is closed. **B-1 is resolved.**

---

## Regression re-check by area

**SSRF / DNS pinning — no regression; the round-1 N-2 latent trap is closed properly.**
The fix pass replaced `dns::normalize` (`trim_end_matches('.')`, *all* trailing dots) with a single shared
`ssrf::canonical_name` (`strip_suffix('.')`, exactly one root dot, then ASCII lower case), and routed all three
consumers through it: `ssrf::parse_host` (`src/ssrf/mod.rs:188`), `dns::Pinned` (`src/fetch/dns.rs:9,45,53`) and the
client's `cross_check` (`src/fetch/mod.rs:343`). I verified there is now exactly one definition and no other spelling
of "normalise" in `src/`. The change is **fail-closed**: it narrows what `Pinned` will answer for (one dot, not many),
and `parse_host` still rejects empty labels, so `example.com..` never reaches either side. `parse_host`'s new
`Ok(Host::Name(canonical_name(host)))` is byte-for-byte equivalent to the old `trimmed.to_ascii_lowercase()`.
`hop_client` still passes the already-canonical `Host::Name(n)` into `Pinned::new`, and `Host::Ip` still passes `""`
so a literal hop refuses every name lookup. `redirect::Policy::none()` + own loop + per-hop `Origin::Redirect`
re-validation, `no_proxy`, `pool_max_idle_per_host(0)`, `http1_only`, `referer(false)` are untouched. The differential
corpus, the four refusal tests, `dial_once_…` and `a3b_merge_gate` all pass on **both** the dev and release profiles.
A new `cross_check_refuses_every_disagreement_with_the_core` test pins the backstop directly, including that
`http://PUBLIC.test.:8080/x` agrees with `http://public.test:8080/x` (case + root dot) while host, port, scheme,
userinfo and unparsable inputs are all refused.

**Memory bounds — untouched and better covered.** `src/fetch/body.rs` is byte-identical to the round-1 version
(64 KiB re-slicing, decompressed-byte cap inside `Out::write`, first-gzip-member-only, 3-byte UTF-8 carry), as is the
`Content-Length` pre-check and the wire cap in `read_body`. The fix pass only *added* tests, and they are good ones:
`the_wire_byte_cap_applies_to_a_gzip_body_that_decodes_under_the_cap` uses incompressible bytes over a **chunked**
response so no `Content-Length` pre-check can pre-empt the client's own counting — that is a real exercise of the
wire cap, not of the declared-length shortcut.

**Panics under `panic = abort`** — the only new non-test code is `strip_root_dot` / `canonical_name`
(`strip_suffix`, `to_ascii_lowercase`; no indexing, no arithmetic) and two lines in `map_transport`
(`is_connect()`, `without_url()`; neither panics). No new `unwrap`/`expect`/`panic!`/indexing in non-test code. The
new `.expect()` in the test helper `get()` is deliberate and desirable: it wraps every test fetch in a 60 s
`tokio::time::timeout` so a broken deadline fails the suite instead of hanging CI. The release-profile gate run
confirms Cargo ignores `panic = "abort"` for the test harness, so that guard behaves as intended there too.

**Stdout purity** — no change. `#![deny(clippy::print_stdout)]` on both crate roots, the single `--version`
`#[allow]`, and all nine `tests/stdio.rs` scenarios still pass; clippy is clean across the tree.

**Dependencies** — `Cargo.toml`, `Cargo.lock` and `deny.toml` are **unchanged** by the fix pass. The dependency count
(8 of the NFR-05 budget of 15), the exact pins, `flate2` `rust_backend`, the `rustls` ring-only feature set and the
openssl/native-tls/aws-lc bans are all as reviewed in round 1. Round-1 **N-3** was answered honestly rather than
papered over: ADR-001 and EPICS D-1 now record that `rustls-platform-verifier` and `openssl-probe` are compiled in by
reqwest's `rustls-no-provider` and unused at runtime, and that D-1 should account for their size rather than try to
remove them. That matches what I found in the tree.

**Release-feature guard** — `scripts/check-release-features.sh` is unchanged. `--self-test` still passes end to end
with all five positive controls failing as designed (`bench-loopback` build fails on both the tree and the marker
layer; renamed feature, unparseable tree text and unknown marker each fail). `Policy::for_build()` remains the only
feature-to-policy path.

**CI workflow (new arm64 job `bench-product-peak`)** — sound and genuinely advisory:
- **Pinned actions**: `actions/checkout@11bd719…` (v4.2.2) and `actions/upload-artifact@ea165f8d…` (v4.6.2) — the same
  SHAs already used by the two existing jobs in the file, not floating tags.
- **No secrets exposure**: the workflow keeps `permissions: contents: read` at file level with no job-level override,
  the job references no `secrets.*`, checkout sets `persist-credentials: false`, the trigger is `pull_request`
  (never `pull_request_target`), and the uploaded artifact is `out/` only — platform text, the JSONL results and the
  exit code. The `bench-loopback` binary itself is never uploaded.
- **Advisory only**: the job id is not among the six required checks (`fmt`, `clippy`, `test`, `deny`,
  `release-guard`, `a3b-merge-gate`); `measure.py` is invoked **without** `--gate`, and the step accepts exit 1
  ("target missed") while failing on 2/3 ("invalid or refused"), which is the right split.
- **The harness invocation is valid**: `g4a-5mib-full` exists in `bench/scenarios.py`, is `implemented=True`, has a
  `min_bytes` early-stop floor, and is a `bench`-binary scenario matching `--binary-kind bench`; `--runs` defaults to
  10, so the ≥10-valid-runs rule is satisfied without `--smoke`; all the `--gate`-only refusals in `measure.py` are
  correctly behind `if a.gate:`. The YAML block-scalar heredoc is well formed and `actionlint` is clean.

**Round-1 non-blocking items** — N-1 (`BlockedTarget` doc promising a port policy) corrected and pointed at C-1/B-3;
N-2 closed by `canonical_name`; N-3 recorded in ADR-001 and D-1; N-4 (timing-tight tests) all passed here and,
per the fix-pass report, five further repeats with no failure — no change requested; N-5 left as-is (a doc/intent
question, as round 1 said); N-6 turned into an explicit E-2 acceptance note for a redirect-chain memory check.
A useful extra was added unprompted: a B-3 note that `tokio::net::lookup_host` runs on the blocking pool and that
B-3/B-5 must bound pool exhaustion and a hung resolver.

---

## Blocking

None.

---

## Non-blocking

### N2-1 — `docs/ci-branch-protection.md` does not list the new advisory arm64 job (confidence 88)

`docs/ci-branch-protection.md:7` still reads "job `bench` … job `bench-product` … **Both** are advisory: do NOT add
either to the required checks." The header comment in `.github/workflows/arm-bench.yml` was updated to name
`bench-product-peak`, but this file — the one the owner follows when configuring branch protection — was not. The
owner quickstart lists the right six checks, so nothing can go wrong by following it; the sentence is simply now
incomplete. Suggest: "job `bench`, job `bench-product` and job `bench-product-peak` … All three are advisory".

### N2-2 — `map_transport`'s doc still overstates what the matched text excludes (confidence 85)

`src/fetch/mod.rs:275` says "Categories only: never the address, port or the transport's own text (which can name
them)." After `without_url()` the URL is gone, but the probe above shows the **resolved peer socket address** is still
present in `chain` via the source's `Debug` (`ConnectError("tcp connect error", 127.0.0.1:42831, …)`). That text is
only ever *matched*, never emitted — every returned message is a fixed category string, so there is no disclosure and
no remaining steerability (an IP literal cannot contain `header` or `too large`). The comment describes the returned
message, but sits above the code that builds the matched text, which is where a future reader is most likely to add a
`log` or `stderr_line` of `chain`. Suggest tightening the wording to "the returned message is a category only; `chain`
is matched, never emitted, and may contain the peer address", or moving the sentence onto the `return` arms.

### N2-3 — the new job's summary step can turn the advisory job red on a harness schema change (confidence 82)

`.github/workflows/arm-bench.yml` (job summary step) indexes `o['valid_runs']`, `o['runs']` and `o['verdict']`
directly while catching only `OSError`. A missing-file case degrades gracefully; a renamed key in `measure.py`'s
`scenario` record raises `KeyError`, fails the `if: always()` step and marks the whole advisory job failed —
noise on every PR until someone notices the cause is the summary, not the measurement. `median_MiB` already uses
`.get`; suggest the same for the other three, or widening the `except` to `Exception`.

### N2-4 — `A-3b/dev-report.md` "Honest gaps" #1 is now contradicted by the record (confidence 85)

Gap 1 still reads "**Native aarch64 RSS smoke NOT done.** This host is x86_64." That was true at `c053d8c`; the fix
pass added `bench-product-peak`, and `docs/BENCHMARK.md` section 14 plus `fix-pass-1-report.md` now record the hosted
arm64 run (VmHWM median 4.77 MiB, 10/10 valid, advisory). The dev report is a dated historical artifact and the
fix-pass report supersedes it, so this is bookkeeping, not a correctness problem — but gaps 2 and 13 of the same list
are still live, so a reader cannot tell which entries are stale without cross-referencing. Suggest a one-line
"superseded by fix-pass 1" marker on gap 1 (gap 13 remains accurate: the `--version` commit/lock hash is still
unimplemented and was re-homed to E-4).

---

## What I re-checked and found correct

The full round-1 "found correct" list still holds: SSRF end-to-end contract, dial-once pinning, per-hop
re-validation, the cross-check backstop, the differential corpus with its non-vacuity counters and positive control,
bounded body handling, the absence of panics under `panic = abort`, stdout purity, the dependency ledger and
`deny.toml` policy, and the two-layer release-feature guard. Nothing in the fix pass weakened any of them, and the
three things it did change to the SSRF surface (one canonical name form, a narrower `Pinned` match, a direct
`cross_check` unit test) each tighten it. The documentation changes are candid rather than promotional — the new TLS
"no automated test" caveat in `README.md` and `docs/ci-branch-protection.md`, and the downgrade of the merge gate's
source scan from "proves" to "checks … a heuristic … not a proof", both make the record more accurate than it was.
