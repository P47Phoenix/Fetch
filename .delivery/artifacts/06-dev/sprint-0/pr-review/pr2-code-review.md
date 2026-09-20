# Independent PR review — PR #2 (code and CI)

- **PR:** https://github.com/P47Phoenix/Fetch/pull/2 — "Sprint 0: spikes, benchmark harness, CI, release guard, ADR-007 (GO)"
- **Branch:** `sprint-0/spikes` → `main`; head `d32445af4d58f1207c35fed90f5a882465b77b4d`
- **Reviewer:** independent code review (no prior review artifacts read)
- **Verdict:** **APPROVE** — 0 blocking findings, 9 non-blocking
- **Scope:** `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `src/`, `scripts/check-release-features.sh`,
  `.github/workflows/*.yml`, `.github/dependabot.yml`, `bench/*.py`, `spikes/a1` (isolation only).
  Planning/DoD docs were deliberately not reviewed; `docs/ci-branch-protection.md` was read only to
  cross-check required-check names against real job ids.

## Commands run (all green)

| Command | Result |
|---|---|
| `cargo fmt --check` | pass (rc 0) |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo clippy --locked --all-targets --features bench-loopback -- -D warnings` | pass |
| `cargo test --locked` | pass (2 tests) |
| `python3 bench/selftest.py` | **SELFTEST PASSED** (47 checks, rc 0) |
| `scripts/check-release-features.sh --self-test` | **self-test OK** (rc 0) |
| `actionlint` (both workflows) | clean (rc 0) |
| `gh pr checks 2` on head `d32445a` | `fmt`, `clippy`, `test`, `deny`, `release-guard`, `bench (gnu)`, `bench (musl)` — all pass |

Toolchain used: rustc/cargo 1.94.1 (matches `rust-toolchain.toml`), Python 3.14.6.

## Blocking findings

**None.**

## What I verified directly (not just read)

### 1. The release guard cannot be fooled by today's forbidden features

Built both variants into throwaway target dirs and grepped the artifacts:

```
clean release build   -> 0 occurrences of FETCH_MCP_MARKER_ ; --version = "fetch-mcp 0.0.0"
--features bench-loopback -> 1 occurrence ; --version = "fetch-mcp 0.0.0 FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback"
```

The marker literals survive `strip = true` + `lto = true` + `opt-level = "s"`, so the grep in
`check_markers` is real, not vacuous. The `cargo tree` arm is also real — I confirmed the root line
format the parser depends on:

```
default:              fetch-mcp v0.0.0 (/…/Fetch) [default]
--features bench-loopback: fetch-mcp v0.0.0 (/…/Fetch) [bench-loopback,default]
```

`ALLOWED_FEATURES=(default)` is a deny-by-default allowlist, so a *renamed* or *newly added*
forbidden feature (no marker) is still caught by the tree arm, and a feature whose marker was
stripped is still caught by the tree arm. The two arms are genuinely independent, and the self-test
asserts both fire (`self-test FAIL: $f rejected for the wrong reason` guards against single-arm
rejection). `set -e` semantics were checked: `[[ -n $feats ]] && bargs+=(…)` is exempt (non-final
element of an `&&` list), `tree=$(cargo …)` is not (a cargo failure aborts). No false-pass path found.

Ordering in CI is also safe: the no-arg invocation builds the *clean* binary into `target/`, and the
`--self-test` invocation builds every forbidden variant into `mktemp -d` dirs, so no marker-carrying
artifact is ever left in `target/`.

### 2. `spikes/a1` cannot affect the release build

`cargo metadata --no-deps` at the repo root reports exactly one workspace member
(`fetch-mcp@0.0.0`) and `workspace_root = /…/Fetch`. Run inside `spikes/a1`, it reports its own
`workspace_root = /…/Fetch/spikes/a1`. The root `Cargo.toml` has no `[workspace]` table, `spikes/a1`
is not a path dependency, and the root `Cargo.lock` contains only `fetch-mcp` (7 lines). `git ls-files`
confirms `spikes/a1/bins/`, `spikes/a1/target/` and the large spike fixture are untracked (covered by
`.gitignore`). Isolation is sound.

### 3. Workflow security

- Triggers are `pull_request` / `push: [main]` / `workflow_dispatch` only. **No `pull_request_target`,
  no `workflow_run`, no `issue_comment`.** Fork PRs get the read-only token and no secrets.
- `permissions: contents: read` at workflow level in *both* files; no job-level escalation; no
  `secrets.*` reference anywhere.
- `persist-credentials: false` on every checkout.
- Every `uses:` is pinned to a 40-hex SHA. I dereferenced all three against the upstream tags:
  - `actions/checkout@11bd719…` = `v4.2.2` ✔
  - `actions/upload-artifact@ea165f8…` = `v4.6.2` ✔
  - `EmbarkStudios/cargo-deny-action@3c63498…` = annotated tag `v2.1.1` → commit `3c63498…` ✔
- The only `${{ }}` interpolations inside `run:` blocks are `matrix.libc` (values `gnu`/`musl`, workflow-controlled)
  and `github.ref` in a `concurrency` group. **No untrusted `github.event.*` interpolation** — no script-injection surface.
- `actionlint` is clean.

### 4. The benchmark gate has no currently reachable false pass

- `--gate` refuses target overrides, `--smoke`, `--settle != 30`, `--parallel-idle > 1`, *any*
  `--child-env` (allowlist is empty), an incomplete gate scenario set, and a non-native-aarch64 host
  (exit 3, checked last so the other refusals are exercisable off-target). All asserted by the self-test.
- `--gate` additionally byte-inspects the binary (`check_identity`): ELF magic, host `e_machine`,
  `fetch-mcp ` version prefix, and marker presence consistent with the claimed kind. A stand-in
  script, a wrong-arch ELF, or a marker-stripped "bench" build is refused. Self-test covers all nine cases.
- A bench `--gate` run is currently *impossible* to pass by construction: the required G4a set
  includes `g4a-late-landmark`, which is `implemented=False`, so the run is refused (exit 2). Nothing
  in this PR claims a gating number — every recorded result is `ADVISORY_PASS` / `gating=false`.
- Non-gate passes are labelled `ADVISORY_PASS`, never `PASS`, with an explicit `note`. Asserted.
- Boundedness without its 5 MiB reference yields `INCOMPLETE` + exit 2, never a pass (asserted for
  both the plain and `--smoke` paths).
- Fixture integrity: `verify()` pins exact size + sha256 for plain fixtures and the *decompressed*
  payload + a compressed-size band for the gzip fixture (robust to zlib build differences). The
  self-test regenerates fixtures into a temp dir and checks them against the committed manifest, so
  a drifted `manifest.json` fails CI. A tampered fixture is refused (exit 2). Verified.
- `bench/fixtures.py html_chunks` padding invariant (`pad >= 7`) holds: the loop only appends a
  section when `written + len(s) + len(FOOT) + 7 <= total`, so the post-break slack is always ≥ 7.

## Non-blocking findings

### NB-1 — `measure.py` returns exit 1 on an uncaught exception, colliding with the documented "target missed" code (medium)

`bench/measure.py:8-11` documents `1 = a target missed`, `2 = INVALID/refused (plumbing failure)`.
An unexpected Python exception exits 1, not 2. Reproduced:

```
$ python3 bench/measure.py --binary ./standin_mcp.py --binary-kind shipped \
      --scenario idle --fixtures-dir $D --child-env NOEQUALS
ValueError: dictionary update sequence element #0 has length 1; 2 is required   # measure.py:269
rc=1
```

`.github/workflows/arm-bench.yml:108` accepts the step when `[ "$r1" -le 1 ] && [ "$rc" -le 1 ]`, so a
harness crash is indistinguishable from a legitimate "target missed" and the job goes green. Impact is
contained today (arm-bench is explicitly advisory and is not a required check), but the contract
inverts in the false-pass direction and the same acceptance shape is likely to be copied into the
future gate job.

*Suggested fix:* wrap `main()`'s body in `try/except Exception` and `return refuse(f"harness error: …", 2)`;
separately validate `--child-env` entries contain `=` at parse time (the `--gate` path already rejects
`NOEQUALS` cleanly at line 262 with exit 2 — only the non-gate path crashes).

### NB-2 — peak scenarios without `min_bytes` silently get a zero early-stop floor (medium, latent)

`bench/measure.py:299`: `scen["min_bytes_v"] = scen["min_bytes"](manifest) if "min_bytes" in scen else 0`.
Seven `kind="peak"` scenarios have no `min_bytes` key, six of them gate scenarios:

```
g4a-late-landmark (G4a), g4b-window-start, g4b-window-end, g4b-raw,
g4b-chunked-window-in-cap, g4b-window-beyond-cap (all G4b), g6-concurrent10 (none)
```

All are `implemented=False` today, so they are refused before measurement — that is why this is not
blocking. But the failure mode when E-2/A-5/A-6 flip `implemented=True` is silent: the early-stop
validity check (`sent >= need`) becomes `sent >= 0`, i.e. always true, and a server that aborts the
body after a few kB would produce a flattering VmHWM that is scored as a valid gate sample.

*Suggested fix:* make the floor explicit rather than defaulted — refuse (exit 2) any
`kind == "peak"` scenario that lacks a `min_bytes` callable, the same way `implemented=False` is refused.

### NB-3 — `cargo-deny` and Dependabot do not cover `spikes/a1`, which CI compiles and runs (low)

`deny.toml` and the `deny` job operate on the root manifest only, and `.github/dependabot.yml`
registers `package-ecosystem: cargo` for `/` only. Meanwhile the `clippy` job builds `spikes/a1`
with four feature sets (rmcp, tokio, reqwest/hyper/ureq, rustls/ring, lol_html, htmd, html2md,
html2text, mimalloc) and `arm-bench` builds *and executes* the resulting binary. That dependency tree
is never advisory-, licence- or ban-checked, and its pinned `spikes/a1/Cargo.lock` will never receive
security updates. Risk is genuinely low (hosted runners, read-only token, no secrets, throwaway crate),
but it is unmonitored third-party build-script and proc-macro execution in CI.

*Suggested fix:* either add a `directory: /spikes/a1` Dependabot entry plus a spike `cargo deny` step,
or record an explicit decision that the spike tree is out of supply-chain scope and delete the crate at
the end of Sprint 0.

### NB-4 — the `deny` gate currently has no coverage to speak of (low, informational)

`fetch-mcp` has zero dependencies, so `cargo deny check` today inspects an empty graph. The job is a
correctly configured placeholder, not evidence that anything has been audited. Worth stating plainly
so the green check is not read as "dependencies are clean" — `docs/ci-branch-protection.md` already
flags that `cargo-audit` has never run, which is consistent.

Also note the workflow sets `arguments: --locked`, which *replaces* the action's default
`--all-features`. Combined with `[graph] all-features = false`, the scan covers default features only.
That is harmless now (all features are empty and dependency-free) but will silently narrow coverage
once optional dependencies exist behind features.

### NB-5 — `spikes/a1/build_all.sh` masks build failures and can copy a stale binary (low)

```sh
b() { name=$1; feats=$2; cargo build --release --features "$feats" -q 2>&1 | grep -E "^error" -A5 || true; cp target/release/a1 bins/$name; }
```

`set -e` is defeated by the pipeline plus `|| true`, and the `cp` runs unconditionally. If a combo
fails to compile, the *previous* combo's `target/release/a1` is copied to `bins/<name>` and
benchmarked under the wrong label. Not used by CI (both workflows call `cargo build` directly), so
this only affects the historical x86_64 spike numbers that inform the GO verdict — worth knowing
before those numbers are cited again.

*Suggested fix:* drop `|| true`, or `cargo build … && cp …`.

### NB-6 — the server-side byte counter is an upper bound, which is the wrong direction for early-stop detection (low, documented)

`bench/serve.py:63-68` increments the counter *before* `wfile.write`, so it counts bytes handed to the
kernel, not bytes the client consumed. For the 5 MiB/50 MiB fixtures this is fine (the write blocks
long before the floor is reached — the self-test catches `STANDIN_EARLY_STOP=1000` at ~320–590 kB),
but a future scenario whose `min_bytes` floor is smaller than the socket buffer could let an
early-stopping client pass. The limitation is documented in the module docstring; flagging it so the
E-2 window scenarios pick floors well above socket-buffer size.

### NB-7 — no per-job `timeout-minutes` in `ci.yml` (low)

`arm-bench.yml` sets `timeout-minutes: 45`; `ci.yml` sets none, so every job inherits the 6-hour
default. The `clippy` job builds a ~200-crate tree four times; a hung network fetch or a linker stall
would burn 6 hours of runner time before failing. Suggest `timeout-minutes: 20` (or 30 for `clippy`).

### NB-8 — `cancel-in-progress: true` also applies to `push: main` (low)

`ci.yml:12-14` groups on `ci-${{ github.ref }}` with `cancel-in-progress: true`. For PRs this is
correct (`refs/pull/N/merge` is per-PR). For `push` to `main`, two merges in quick succession cancel
the first commit's run, leaving a `main` commit with no completed CI result. Low impact given
PR-required merges, but consider `cancel-in-progress: ${{ github.event_name == 'pull_request' }}`.

### NB-9 — minor harness/script hygiene (low)

- `measure.py` `Server.close()` (`:172-174`) calls `self.p.kill()` in the `except` branch without a
  following `wait()`, and never closes `self.p.stdout`; zombies/FDs linger until Python's next
  `subprocess` cleanup. Bounded (10 runs × scenarios) but untidy.
- `scripts/check-release-features.sh` prints `guard OK` on the no-arg path but not on the
  `--features` path (`:94-95`) — cosmetic asymmetry in an otherwise carefully symmetric script.
- `self_test`'s `trap 'rm -rf "$tmp"' RETURN` does not fire if `set -e` aborts the shell mid-function,
  leaking a temp dir on failure. Trivial.

## Cross-checks that came out clean

- **Required-check names.** `docs/ci-branch-protection.md` lists `fmt`, `clippy`, `test`, `deny`,
  `release-guard` — these match the job ids in `ci.yml` exactly. It correctly states that `arm-bench`'s
  job id is `bench` and must **not** be added to required checks; `arm-bench.yml` indeed uses job id `bench`.
- **Feature hygiene.** `default = []`; `test-support` and `bench-loopback` are declared but enable
  nothing, and each has a cfg-gated marker literal. `publish = false` blocks accidental crates.io
  publication. `Cargo.lock` is now tracked (removed from `.gitignore`), which is required for `--locked`.
- **CI feature coverage.** `test` runs the suite under default, `bench-loopback` and `test-support`;
  `clippy` runs `-D warnings` under default and `bench-loopback`. The `bench_build_version_carries_marker`
  test only compiles under `bench-loopback` and is therefore actually exercised by the second `cargo test` run.
- **`src/`.** `build_markers()` uses per-element `#[cfg(feature = …)]` inside the array literal, so a
  disabled feature's string is not compiled in at all (confirmed by the 0-hit grep on the clean
  binary). `main.rs` printing `--version` is what keeps the marker reachable from `rodata` so the
  guard's grep cannot go vacuous — that coupling is deliberate and documented.
- **Toolchain.** `rust-toolchain.toml` pins `1.94.1` exactly with `clippy`/`rustfmt` and the three
  aarch64/darwin targets; `rustup show active-toolchain` as the first step installs it. Matches the
  local rustc used for this review.
- **arm-bench wrapper binaries.** `exec "$RUNNER_TEMP/a1" "$@"` preserves the pid, so `/proc/<pid>/status`
  measures the spike, not the shell — the stated intent holds. The `printf` argument order
  (`"$2"` → marker, `"$RUNNER_TEMP"` → path) is correct. The `a1-shipped` wrapper carries no forbidden
  marker and `a1-bench` carries `bench-loopback`, matching each scenario's required `binary` kind; both
  would be refused by `check_identity` under `--gate`, which is exactly why the workflow never passes `--gate`.

## Decision

**APPROVE.** The release guard and the benchmark gate are both defended by real, independently
verified positive controls; nothing in this PR can ship a bench- or test-only feature; `spikes/a1` is
a separate workspace that cannot reach the release build; and the workflows are minimally
privileged, SHA-pinned and free of untrusted interpolation. NB-1 and NB-2 are the two worth fixing
before the gate becomes a required check.
