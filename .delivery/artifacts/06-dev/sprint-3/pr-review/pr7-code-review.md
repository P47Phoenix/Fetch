# PR #7 independent code review

Branch `sprint-3/harness-idle-rss`, head `39379c1`, base `main` `35a3450`.
Scope reviewed: full diff `35a3450..39379c1` (21 files, +602/-36).

**DECISION: APPROVE** — 0 blocking, 7 non-blocking.

## What was reviewed

- `src/fetch/mod.rs`, `src/server.rs`, `src/fetch/tests.rs`, `tests/stdio.rs` (A-9 final URL / status header)
- `bench/measure.py`, `scenarios.py`, `serve.py`, `fixtures.py`, `selftest.py`, `standin_mcp.py`, `report.py`, `manifest.json`
- `.github/workflows/bench.yml` (new), `scripts/build-candidates.sh` (new)
- `docs/BENCHMARK.md`, `docs/ci-branch-protection.md`, `.delivery/**`

Executed locally: `python3 bench/selftest.py` on x86_64 — **SELFTEST PASSED**, 41/41 checks, including the new
`native_host` matrix, the in-process gate PASS/FAIL/INVALID paths and both `redirect-chain5` cases.

## Gate correctness: can it false-pass?

I could not find a path where a real over-budget measurement reports green. The `--gate` preconditions are
checked in a sound order in `bench/measure.py:270-305` and every one of them is exercised by `selftest.py`:

| Guard | Where | Verified |
|---|---|---|
| No `--smoke` / target override / `--settle` / `--parallel-idle` | `measure.py:271-274` | selftest |
| Complete gate set (`required_gate_set`) | `measure.py:275-276`, `:48-53` | selftest (partial G4a set refused) |
| Empty `--child-env` allowlist | `measure.py:277-278` | selftest (6 keys) |
| Native host, no QEMU binfmt for the host arch | `measure.py:279-280`, `:118-129` | selftest (6 cases) |
| >= 10 valid runs | `measure.py:281`, `:336` | selftest |
| Fixture sha256 | `measure.py:290-291` | selftest |
| ELF identity: host `e_machine`, `fetch-mcp ` version, marker/kind agreement | `measure.py:300-302`, `:93-112` | selftest (8 cases) |
| ADVISORY never reads as PASS | `measure.py:363` | selftest |

CI wiring is also correct on the point that most often breaks: the final step compares
`steps.<id>.outcome`, which is the result *before* `continue-on-error` is applied, so the
`continue-on-error: true` on the three measurement steps does not hide a failure. The trailing
`[ A = success ] && [ B = success ] && [ C = success ]` is the last command in the script, so its status is
the step's exit status (bash's errexit exemption for non-final AND-OR members does not apply to the script's
final status). Values are quoted, so a skipped step (`""`) evaluates false.

The Rust A-9 change is correct and well covered. `final_url` is taken from `parsed` at
`src/fetch/mod.rs:168`, which is the `cross_check`ed URL of the hop that actually returned 2xx, so it is the
body's true origin. Header injection via the URL is not possible: the `url` crate strips ASCII CR/LF/TAB
during parsing, so `Url::as_str()` cannot contain a newline. stdout purity is asserted by the new end-to-end
test (`finish_and_assert_pure`). The new listener thread in `tests/stdio.rs` cannot panic
(`read(..).unwrap_or(0)`, `let _ = write!(..)`), which matters under `panic = "abort"`.

---

## Non-blocking findings

### NB-1. `idle-bench` is documented as advisory but hard-fails the bench job (confidence 88)

`.github/workflows/bench.yml:110-119` — the step is named *"Idle RSS of the bench build (recorded, E-8 delta;
not a gate)"*, runs `measure.py` **without** `--gate`, and ends with `exit "$rc"`. `measure.py` returns 1 when
the median misses the 10 MiB idle target (`measure.py:341, :366`), so an advisory over-target number fails the
step, and the final step requires it:

```
[ "${{ steps.idle.outcome }}" = success ] && [ "${{ steps.peak.outcome }}" = success ] && [ "${{ steps.idle_bench.outcome }}" = success ]
```

This contradicts three places: the workflow header comment ("The job fails if either gating run misses ... An
ADVISORY verdict is never a result"), `bench/scenarios.py:11-13` ("never a gate figure itself (gate 'none')"),
and the house pattern in `arm-bench.yml`, which uses `[ "$rc" -le 1 ]` for advisory runs so that exit 1 (a
result) is reported but only 2/3 (plumbing) is fatal.

Fix: use `[ "$rc" -le 1 ]` in the `idle_bench` step (keeping it in the final AND-chain then catches only
INVALID/refused), or drop `steps.idle_bench` from the final chain, or update the comments to say it gates.
Measured deltas are within ±0.01 MiB, so there is no practical risk today.

### NB-2. `redirect-chain5` is `gate="none"` yet can fail the gating peak run (confidence 82)

`.github/workflows/bench.yml:121-131` passes `--scenario g4a --scenario redirect-chain5` to the **gating**
peak invocation. `measure.py:351` excludes `gate == "none"` scenarios from `gating_peak_kB`, but **not** from
the verdict: a FAIL appends to `missed` (`:342` → exit 1) and fewer than 10 valid runs appends to `invalid`
(`:337` → exit 2). So a scenario that `scenarios.py:26` and `docs/BENCHMARK.md` §5 both describe as "recorded,
outside the gating peak" can turn the gate red.

Same class as NB-1: behaviour is stricter than the documentation, i.e. fails closed, not open. Either run
`redirect-chain5` in its own non-gating invocation, or drop the "outside the gating peak" wording. Measured
4.32-6.97 MiB against a 40 MiB target, so there is margin.

### NB-3. The fork-PR skip becomes a required-check false-pass the moment the owner acts on the new doc (confidence 80)

`.github/workflows/bench.yml:44` skips the job for fork PRs via a job-level `if:`. A *skipped* job reports as
success to GitHub branch protection. That is harmless while the check is advisory — but this PR adds
`docs/ci-branch-protection.md:7`, which invites the owner to add all four `bench-gate (...)` contexts as
required checks. At that point a fork PR satisfies the memory gate by never running it.

Recommendation: add that caveat to `docs/ci-branch-protection.md` alongside the invitation (fork PRs will
report the required check as passed without measuring), so the owner makes the decision knowingly.

### NB-4. `native_host()` treats an unreadable `binfmt_misc` as native (confidence 78)

`bench/measure.py:124-128`:

```python
try:
    for n in os.listdir(binfmt_dir): ...
except OSError:
    pass
return True, "ok"
```

If `/proc/sys/fs/binfmt_misc` is not mounted or not visible (common inside containers), the preflight returns
native and `--gate` proceeds, so a QEMU-emulated environment could produce gating RSS. The heuristic is
inherited from E-1 and is adequate for the hosted runners the workflow actually targets (which the job also
records in `out/platform.txt`), and `check_identity` independently rejects a wrong-arch ELF. Worth considering
recording the `OSError` in the host record, or refusing `--gate` when the directory is absent *and*
`host["container"]` is true.

### NB-5. The A-9 header is unmarked plain text and absent without a redirect, so a page can forge it (confidence 80)

`src/server.rs:120-127` — `with_header` prepends `URL: ...\nStatus: ...\n\n` only when `redirects != 0`, and
the header is lexically indistinguishable from body content. A page served with **no** redirect can begin its
own content with `URL: https://trusted.example/\nStatus: 200\n\n`, and neither the model nor the client can
tell it from the harness-emitted provenance header. The URL→header direction is safe (the `url` crate strips
CR/LF), but the body→header direction is open.

This is a property of the FR-14 format, which the dev report already flags as a design choice ("Header format
is my choice"), and OQ-5 resolved "no untrusted-content label" — so it is a deliberate trade-off, not a
defect. Flagging it so the choice is on the record: emitting the header unconditionally (including
`redirects: 0`) would at least remove the "absent header is forgeable" half.

### NB-6. The job-summary step can fail a job whose gates passed (confidence 78)

`bench/report.py:15-18` — `load()` catches only `OSError`; a truncated or malformed JSONL line beginning with
`{` raises `json.JSONDecodeError`, which is uncaught. The "Job summary" step
(`.github/workflows/bench.yml:133-144`) has no `continue-on-error`, so a reporting hiccup turns a green gate
red. `arm-bench.yml` wraps its inline summary in `try/except`; `report.py` does not. Suggest catching
`(OSError, ValueError)` in `load()`, or adding `continue-on-error: true` to the summary step.

### NB-7. `build-candidates.sh` pins ziglang's version but only checks cargo-zigbuild's presence (confidence 80)

`scripts/build-candidates.sh:20-23` verifies `python3 -m ziglang version` equals `$ZIG_VERSION` but checks
cargo-zigbuild only with `cargo zigbuild --help >/dev/null`. A developer with a stale cargo-zigbuild on PATH
gets a silent version drift in a script whose header advertises both pins. CI installs the pin explicitly so
the CI path is fine. Suggest `cargo zigbuild --version` compared against `$ZIGBUILD_VERSION`, matching the
ziglang check.

---

## Observations (no action requested)

- **`redirect-chain5`'s validity floor proves "followed", not "read".** `serve.py:83-89` increments the counter
  *before* `wfile.write`, so `bytes_sent` measures what the server handed to the socket. `src/fetch/mod.rs:161`
  deliberately drops each redirect response without reading its body — and still clears the
  `5 * REDIR_BODY + REDIR_FINAL` floor, because the server writes all six bodies regardless. That is the right
  outcome (the floor catches a client that does not follow the chain, as `STANDIN_NO_REDIRECT` in the selftest
  shows), but the dev-report wording "invalid if the chain was not read" should read "not followed".
- **`MAX_REDIRECTS = 5` and `/redir/5` sit exactly on the boundary.** The chain uses the full redirect budget
  (hop 0-4 are 302s, hop 5 is the 200). Intentional and correct, but any future reduction of `MAX_REDIRECTS`
  silently turns `redirect-chain5` into an INVALID run rather than an obvious failure.
- **`late_chunks` assumes the loop runs at least twice.** `bench/fixtures.py` — `pad = total - written -
  len(tail)` is only guaranteed `>= 7` after one successful iteration. Unreachable at `LATE_SIZE = 5 MiB - 1024`.
- **`Fetched` dropped `Copy`** (`src/fetch/mod.rs:61`). A semver-visible change on a public type; irrelevant for
  a binary crate at this stage.
- **CI security posture is good**: `permissions: contents: read`, `persist-credentials: false`, both actions
  pinned by commit SHA, `pull_request` (not `pull_request_target`), no secrets, fork code never runs. Toolchain
  installs are version-pinned (`cargo-zigbuild 0.23.4 --locked`, `ziglang==0.16.0`) though not hash-pinned.
- **`GITHUB_PATH` ordering works**: the venv `bin` is prepended, so later steps' `python3` resolves to the venv
  interpreter that can `import ziglang`; the bench scripts are stdlib-only, so this is harmless.
- **Timing**: 60 min timeout against ~16 min measured per cell, with four uncached release builds plus a
  from-source `cargo install cargo-zigbuild`. Adequate headroom, but it is the tightest number in the workflow.
