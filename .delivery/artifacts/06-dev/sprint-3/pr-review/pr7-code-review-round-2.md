# PR #7 independent code review, round 2

Branch `sprint-3/harness-idle-rss`, head `37b2823`, base `main` `35a3450`.
Round 1 reviewed `39379c1` (APPROVE, 0 blocking, 7 non-blocking). This round re-reviews the whole diff
`35a3450..37b2823` (28 files, +1025/-64) with focus on the fix pass (`afb1613` + `37b2823`, diff `39379c1..37b2823`,
20 files, +446/-51).

**DECISION: APPROVE** — 0 blocking, 3 non-blocking.

## What was executed for this review

| Check | Result |
|---|---|
| `cargo test --locked` | 95 + 9 pass, 0 fail |
| `cargo test --locked --features bench-loopback` | 96 + 11 pass, 0 fail (includes the A-9 e2e stdio test and `header_is_outside_the_max_length_window`) |
| `cargo clippy --locked --all-targets --features bench-loopback` | clean, no warnings |
| `python3 bench/selftest.py` | SELFTEST PASSED, including the 3 new `native_host` strict cases and the 2 new `redirect-chain5` cases |
| `echo_url` edge cases | reproduced against `url 2.5.8` (the pinned version) in a scratch crate, 12 inputs — see below |

Working tree clean at `37b2823`.

## Round-1 items: each verified

| # | Round-1 item | Fix | Verdict |
|---|---|---|---|
| NB-1 | `idle-bench` documented advisory but hard-failed the job | `bench.yml:129` now ends the step with `[ "$rc" -le 1 ]`; label and header comment updated | **Fixed correctly.** `measure.py:367` returns 1 only for `missed`; `idle-bench` has no boundedness reference, so exit 1 can only mean "over the 10 MiB idle target". Exit 2/3 (INVALID/refused/plumbing) still fail. `[ ... ]` is the last command in the script, so it is the step status. The final AND-chain still includes `steps.idle_bench.outcome`, which is now consistent with the documented intent. |
| NB-2 | `redirect-chain5` is `gate="none"` yet can fail the gating peak run | Chose "documented as intended": `scenarios.py:26-27` comment, `bench.yml:9-11` header, BENCHMARK §15 design note, plus a new selftest asserting the INVALID path counts in `summary["invalid"]` | **Fixed correctly**, and in the safe direction (fail-closed). Behaviour unchanged; the documentation now matches the code, which was the discrepancy. |
| NB-3 | Fork-PR skip becomes a required-check false-pass | `docs/ci-branch-protection.md:7` now carries an explicit caveat ("a skipped job counts as passing for branch protection, so on a fork PR the required memory gate would report success without measuring anything. Decide that knowingly") plus a cost/quota note | **Fixed correctly.** The decision is now on the record next to the invitation. |
| NB-4 | `native_host()` treated an unreadable `binfmt_misc` as native | `native_host(..., strict=False)`; the `--gate` path calls `native_host(strict=True)` (`measure.py:280`), which returns `(False, "cannot read ... so native execution is unproven")` on `OSError`; 3 selftests | **Fixed correctly.** Tightens the gate only (false-fail, never false-pass). Empirically safe on the real targets: run 35541676701 at `afb1613` — which contains this change — passed `--gate` on all four hosted cells, so `binfmt_misc` is readable there. `docs/BENCHMARK.md:164` documents the new exit-3 cause. |
| NB-5 | A-9 header is unmarked plain text and forgeable by a non-redirected page | Documented, not changed: README "What works today" bullet and `docs/EPICS.md` A-9 both state it is "a convenience for citing, not proof of provenance"; OQ-5 already decided "no label" | **Fixed correctly** for a deliberate trade-off — the choice is now visible to a reader of the README rather than buried in a dev report. |
| NB-6 | `report.py load()` could raise `JSONDecodeError` and redden a green gate | `load()` now skips malformed lines per line with a stderr note (`report.py:15-25`), **and** the summary step carries `continue-on-error: true` (`bench.yml:145`) | **Fixed correctly**, belt and braces. `report.py` is exit-0-always and reporting-only; with `continue-on-error` it can no longer influence the job result at all. `med.get(...)` returning `None` for an INVALID scenario is still guarded by `if i and ib` at `report.py:52`. |
| NB-7 | `build-candidates.sh` pinned ziglang but only probed cargo-zigbuild's presence | `scripts/build-candidates.sh:20-21` now runs `cargo-zigbuild --version` and compares to `$ZIGBUILD_VERSION` | **Fixed**, with a small residual (NB-2r below). Verified in CI at `afb1613`, where the build step succeeded, so `cargo-zigbuild --version` does produce the expected string on the real toolchain. |
| Obs | "chain not read" should be "not followed" | Corrected in `bench/selftest.py:253`, `scenarios.py` and the dev report | Fixed. |
| Blocking 1 | arm64 CPU model never recorded; the claim that it was, was false | `bench.yml:62-68` reads `lscpu` `Model name` with `/proc/cpuinfo` and implementer/part fallbacks, plus a `cpu_id:` line; the false claim is retracted in the dev report gap 6 and in an appended BENCHMARK §15 correction | **Fixed and verified in CI** (run 35541676701: arm64 `cpu: Neoverse-N2`, `cpu_id: implementer=0x41 part=0xd49 vendor=ARM`). The retraction is explicit rather than a silent rewrite, which is the right call. |
| Blocking 2 | Stale docs (README status, BENCHMARK "Read this first", §3 host table, glossary, §7, §11, §13, quickstart/identity text) | All updated; §11 and §13 stale lines are marked "(Historical, Sprint N)" instead of being rewritten | **Fixed.** Spot-checked against the numbers in §15 and the QA review's log-verified figures: consistent. The amd64 column of the §3 host table is filled from the logs and correctly flags that the runner CPU varies between runs. |

Also verified: the QA DoD review's noted test gap ("nothing tests the header against a `max_length` window") is closed by
`src/server.rs:178-187` `header_is_outside_the_max_length_window`.

## Can `--gate` now false-pass?

No. I re-walked every gate precondition and every change in the fix pass; all four code changes that touch the gate move
in the fail-closed direction or are reporting-only.

- `native_host(strict=True)` (`measure.py:280`) only adds a refusal. Refusal is exit 3, the step does `exit "$rc"`, the
  job's final step requires `success`.
- `redirect-chain5` stays out of `gating_peak_kB` (`measure.py:352`, `gate != "none"`) but its FAIL still appends to
  `missed` and an INVALID run to `invalid` — now covered by an explicit selftest.
- The `idle-bench` relaxation is on a **non-`--gate`** invocation of a `gate="none"` scenario against a bench binary; it
  cannot produce a product figure (`measure.py:364` forces `ADVISORY_PASS` without `--gate`, and `report.py` prints
  `gating=False`). The relaxation is bounded to exit 1 = "over the 10 MiB idle target", which is the documented
  advisory E-8 delta reading.
- `continue-on-error: true` on the summary step removes an influence on the job result rather than adding one; the step
  is not in the final AND-chain.
- The CI wiring point from round 1 still holds: the final step compares `steps.<id>.outcome` (pre-`continue-on-error`),
  the AND-chain is the script's last command so its status is the step's status, and the values are quoted so a skipped
  step (`""`) evaluates false.

**Mis-measurement of a shipped/bench binary:** unchanged and still sound. `build-candidates.sh` builds shipped first
(no features) and bench second (`--features bench-loopback`), copying each artifact out before the next build, so the
two cannot be transposed; `check_identity` independently enforces host `e_machine`, the `fetch-mcp ` version prefix and
marker/kind agreement; `SHIPPED_FORBIDDEN_MARKERS` rejects a marker-carrying binary claimed as shipped. `strip = true`
in `[profile.release]` does not remove the `.rodata` marker string, as the passing CI runs confirm.

## New Rust code under `panic = "abort"`, stdout purity, URL echo

**`panic = "abort"`:** `echo_url` (`src/fetch/mod.rs:320-326`) cannot panic — `Url::clone`, `set_username`,
`set_password` return `Result` (discarded; they can only fail for cannot-be-a-base URLs, which `http`/`https` never
are), `set_fragment` and `to_string` are infallible. The listener thread added to `tests/stdio.rs:268-283` still uses
`read(..).unwrap_or(0)` and `let _ = write!(..)`, so it cannot abort the process.

**stdout purity:** the new A-9 end-to-end test (`tests/stdio.rs:266`) ends in `finish_and_assert_pure()`, which drains
stdout after closing stdin and asserts every line parses as a `jsonrpc: "2.0"` frame. The header's embedded newlines
live inside a JSON string, so they cannot break framing. Header injection through the URL remains impossible (the `url`
crate strips ASCII CR/LF/TAB at parse time).

**URL echo correctness** — reproduced `echo_url` against the pinned `url 2.5.8`:

| input | echoed |
|---|---|
| `https://user:secret@b.example:8443/p?q=1#frag` | `https://b.example:8443/p?q=1` |
| `http://:pw@example.com/` | `http://example.com/` (no stray `@`) |
| `http://user@example.com/` | `http://example.com/` |
| `http://[2001:db8::1]:8080/a%20b?x=%2F#f` | `http://[2001:db8::1]:8080/a%20b?x=%2F` (IPv6 brackets, port, percent-encoding preserved verbatim) |
| `http://[::1]/` | `http://[::1]/` |
| `http://example.com` (empty path) | `http://example.com/` |
| `http://example.com/p%41th/%e2%82%ac?a=b%20c#z` | `http://example.com/p%41th/%e2%82%ac?a=b%20c` (no re-encode, no case folding of escapes) |
| `http://example.com/#` (empty fragment) | `http://example.com/` |

No double-encoding, no dropped port, no `@` residue, IPv6 literals intact. Default-port elision
(`https://example.com:443/x` → `https://example.com/x`) and path normalisation come from `Url::parse`, not from
`echo_url`, and match the URL actually dialled, so the echo stays truthful.

Note the userinfo strip is defence in depth rather than a live path: `ssrf::check_url` rejects userinfo on the initial
URL (`InvalidArgument`) and on every redirect hop (`BlockedTarget`, `src/ssrf/mod.rs:85`), and `cross_check`
(`src/fetch/mod.rs:360`) additionally requires `username().is_empty() && password().is_none()` before the URL can reach
`echo_url`. The fragment strip is the behavioural change, and it is correct: a fragment is never sent on the wire, so
echoing one from a `Location: /final#secret` would have been misleading. Covered by both new tests.

`Fetched.final_url` is consumed only by `server::with_header`; nothing re-dials it, so the sanitisation cannot affect
request behaviour.

## CI security

Unchanged from round 1 and still good: `permissions: contents: read`, `persist-credentials: false`, both actions
pinned by commit SHA, `pull_request` (never `pull_request_target`), no secrets, fork PRs skipped so fork code never runs
on a measuring runner. The fix pass adds no new action, no new network fetch and no new permission. The new CPU-probe
shell is safe under `set -eu`: every substitution is `|| true`-guarded or ends in a pipeline whose last element
(`head`) exits 0, and each `[ -n "$cpu" ] || cpu=...` is a non-final AND-OR member, which `set -e` exempts.
Toolchain installs remain version-pinned but not hash-pinned; that is now recorded as a D-2 acceptance criterion
(`docs/EPICS.md`, architect NB1).

---

## Non-blocking findings

### NB-1r. The ADR-004 doc comment now documents `echo_url` instead of `content_encoding_is_gzip` (confidence 92)

`src/fetch/mod.rs:316-326`. `echo_url` was inserted between an existing doc comment and the function it described:

```rust
/// Accept absent, `identity`, or exactly one `gzip` (checked before any decode); everything else, stacked
/// encodings included, is `unsupported_encoding` (ADR-004).
/// The URL echoed back in the A-9 header: the request URL without userinfo or fragment (...).
pub(crate) fn echo_url(u: &Url) -> String {
```

The result is a doc comment whose first two lines describe a different function, and `content_encoding_is_gzip`
(directly below) is now the only undocumented function in the module. No behaviour change, and clippy does not catch
it. In a codebase this deliberate about rationale-bearing doc comments (every other item carries one, most citing an
ADR), this reads as a genuine slip from the insertion point rather than a choice.

Fix: move the two ADR-004 lines back down so they sit immediately above `fn content_encoding_is_gzip`, leaving
`echo_url` with its own two-line doc.

### NB-2r. `build-candidates.sh` checks the cargo-zigbuild pin by substring while the ziglang pin is exact (confidence 82)

`scripts/build-candidates.sh:20-23`:

```bash
zbv=$(cargo-zigbuild --version 2>/dev/null || true)
[[ $zbv == *"$ZIGBUILD_VERSION"* ]] || { ...; exit 2; }
zv=$(python3 -m ziglang version 2>/dev/null || true)
[[ $zv == "$ZIG_VERSION" ]] || { ...; exit 2; }
```

Round-1 NB-7 asked for parity with the ziglang check and the version is now read, which is the substance of the fix.
The residual is that `*"0.23.4"*` also accepts `0.23.40`, `0.23.4-dev` or a string that merely contains the number,
whereas the line below it demands an exact match — so the script's two pins are enforced to different standards, in a
file whose header advertises both as pins. Low practical risk (CI installs the exact version with `--version 0.23.4`),
but it is the one direction of this check that fails open.

Fix: `[[ $zbv == "cargo-zigbuild $ZIGBUILD_VERSION" ]]`, or compare the second field
(`[[ $(awk '{print $2}' <<<"$zbv") == "$ZIGBUILD_VERSION" ]]`), keeping the existing error message.

### NB-3r. The host record still writes `native_gate_host` from the non-strict probe (confidence 80)

`bench/measure.py:309`:

```python
host = host_record(); host["native_aarch64"] = native_aarch64()[0]; host["native_gate_host"] = native_host()[0]
```

The gate path is safe — a run whose `binfmt_misc` is unreadable is refused at line 280 and never reaches line 309 — so
this cannot cause a false pass. But on an **advisory** run (`arm-bench.yml`, a developer's machine, a container) the
emitted JSONL asserts `"native_gate_host": true` on exactly the hosts the strict rule calls unproven. Since the JSONL
is the durable evidence that later sections of `docs/BENCHMARK.md` quote, a record that is more confident than the gate
is worth aligning.

Fix: record both, e.g. `host["native_gate_host"] = native_host(strict=True)[0]` (matching what `--gate` would decide),
or add `host["native_gate_host_reason"] = native_host(strict=True)[1]` so an unreadable `binfmt_misc` is visible in the
artifact.

---

## Observations (no action requested)

- **Evidence chain at the head SHA.** The green four-cell `--gate` evidence (run 35541676701) measured `afb1613`;
  `37b2823` on top of it changes only `.delivery/` and one line of `docs/BENCHMARK.md`, so no gated code differs
  between the measured commit and the PR head. Worth stating in the merge record rather than fixing.
- **README now says "two CI runs"** while BENCHMARK §15's appended correction documents a third (35541676701). The
  quoted ranges (idle 2.16-4.06, peak 4.14-5.27 MiB) still bound run 3's figures, so the README understates rather
  than overstates — the safe direction for this project's documentation standard, but a one-word drift.
- **§15 internal nit:** "The two runs agree within about 0.15 MiB per cell, except the amd64 gnu idle (4.06 against
  3.92...)" — that exception is 0.14 MiB, i.e. inside the stated bound.
- **BENCHMARK troubleshooting table has no exit-3 row.** The unreadable-`binfmt_misc` refusal is described in the §7
  narrative (line 164) but a developer hitting exit 3 locally will look at the table first.
- **Strict binfmt will refuse `--gate` on hosts without the binfmt_misc module loaded** (some containers, some minimal
  distros). That is the intended fail-closed behaviour and the docs say so; noting it because it changes the local
  developer experience of `--gate`, not just CI.
- **`echo_url` is `pub(crate)`** but used only inside `src/fetch/`; `fn` plus `#[cfg(test)]` visibility would be
  tighter. No warning, no impact.
- **Scope change pending acknowledgement.** Revision 11 of the sprint plan and `docs/EPICS.md` re-home four E-2
  acceptance items (container-image measurement, the tag trigger with digest gate, `g6-concurrent10`/G4b, the macOS
  `/usr/bin/time -l` reader) to D-2 and E-4, marked **PENDING OWNER ACKNOWLEDGEMENT (not acknowledged)**. That is a
  planning decision, correctly recorded and correctly flagged as unacknowledged — outside a code review's remit, but
  it means "E-2 done" is conditional on that acknowledgement.
