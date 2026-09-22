# PR #12 — independent code + security review

**Repo:** P47Phoenix/Fetch · **Branch:** `sprint-8/ssrf-suite-release-profile` · **Range:** `0926ba8..eda0408`
**Reviewer:** independent review agent (read-only) · **Date:** 2026-09-21

**DECISION: REQUEST_CHANGES** — 2 blocking, 6 non-blocking.

Both blocking items are integrity-of-the-gate issues, not correctness bugs in shipped code. No production
code changed in this PR, so there is no SSRF/guard regression risk. The blockers are that the two new gates
measure and enforce materially less than the PR and its docs claim they do.

---

## 1. Scope of the change (priority 6 — confirmed)

`git diff 0926ba8..eda0408 --stat`:

| File | + |
|---|---|
| `.delivery/artifacts/05-plan/po/sprint-plan.md` | 6 |
| `.delivery/artifacts/06-dev/sprint-8/dev-report.md` | 41 |
| `.github/workflows/ci.yml` | 46 |
| `docs/BENCHMARK.md` | 19 |
| `scripts/coverage_gate.py` | 93 |
| `tests/stdio.rs` | 31 |

**`git diff 0926ba8..eda0408 -- src/` is empty.** `Cargo.toml`, `Cargo.lock` and `deny.toml` are byte-identical
across the range (verified: `git diff --stat 0926ba8..eda0408 -- Cargo.toml Cargo.lock deny.toml` returns nothing;
`sha256(Cargo.lock)` is `65a5a943…4cce1` at both `8d606cc` and `eda0408`).

**Conclusion: zero production-code and zero dependency change. No SSRF, redirect, pagination or TLS-backend
regression is reachable from this diff.** Everything below concerns whether the *new gates* are worth what they claim.

---

## 2. BLOCKING

### B1 — The coverage gate's denominator is 65% test code; it passes at ~73% real production coverage (confidence 95)

**Where:** `.github/workflows/ci.yml` (`coverage` job, the `scripts/coverage_gate.py` invocation) and
`docs/BENCHMARK.md` §18.

I reproduced the exact CI command locally (`cargo llvm-cov 0.9.1`, same flags, same file list). It reproduces
the documented figure precisely — `1832/1876 = 97.7%`, gate PASSED. That figure is *accurate*; the problem is
what it is measuring.

Splitting the same lcov data at each file's trailing `#[cfg(test)] mod tests {` boundary:

| file | production hit/found | prod % | test-code hit/found | test % |
|---|---|---|---|---|
| `src/ssrf/mod.rs` | 152/152 | 100.0% | 250/251 | 99.6% |
| `src/ssrf/ranges.rs` | 49/54 | 90.7% | 279/281 | 99.3% |
| `src/ssrf/resolver.rs` | 91/91 | 100.0% | 197/197 | 100.0% |
| `src/fetch/mod.rs` | 256/287 | 89.2% | 0/0 | — |
| `src/convert/window.rs` | 73/73 | 100.0% | 70/70 | 100.0% |
| `src/ssrf/differential.rs` | **0/0** | — | 415/420 | 98.8% |
| **combined** | **621/657 = 94.5%** | | **1211/1219 = 99.3%** | |

**1219 of the 1876 gated lines (65.0%) are test code**, which is ~99.3% covered by construction. Two compounding
causes:

1. **`src/ssrf/differential.rs` is listed as a gated coverage target but is a 100% test-only module.** It is
   declared `#[cfg(test)] mod differential;` at `src/ssrf/mod.rs:13` and contains no production code at all. It
   contributes **420 lines — 22.4% of the entire denominator** — of guaranteed-~99% coverage. Including a
   test-only file in a production-coverage gate is wrong independently of the combined-vs-per-file question.
2. The four other files carry large inline `#[cfg(test)] mod tests` blocks that `cargo llvm-cov` counts, and
   nothing in the job excludes them.

**Impact — the gate has ~22 points of dead headroom.** Holding the test-code hits fixed at 1211, the gate
passes as long as `(1211 + prod_hit)/1876 >= 0.90`, i.e. `prod_hit >= 478` of 657 production lines. **Real
production line coverage across the SSRF/redirect/pagination modules can fall from today's 94.5% to ~72.8%
and `coverage` will still report PASS.** For a gate whose stated AC (B-5) is "line coverage of SSRF, redirect
and pagination modules is at least 90%", that is a gate that does not enforce its AC.

**Suggested fix (either, or both):**
- Drop `src/ssrf/differential.rs` from the gated file list — it is not a module under test, it *is* a test.
- Exclude test regions from the measurement, e.g. build the coverage run with
  `--ignore-filename-regex` extended to real test files plus `RUSTFLAGS`/`cfg` handling, or move the inline
  `#[cfg(test)] mod tests` blocks into sibling `tests.rs` files (as `src/fetch/mod.rs` already does — which is
  exactly why it is the only honest number in the table) and exclude those paths.

Note the second point cuts both ways: `src/fetch/mod.rs` is the one file with its tests already externalised,
and it is also the only file below 90%. That is not a coincidence — it is the only file being measured fairly.

### B2 — Neither new job is a required check, and `docs/BENCHMARK.md` states that one of them is (confidence 92)

**Where:** `docs/BENCHMARK.md` §18 vs `docs/ci-branch-protection.md` (unchanged by this PR).

`docs/BENCHMARK.md` §18 asserts:

> CI job `release-ldd-guard` (`.github/workflows/ci.yml`) makes this a **required**, machine-checked assertion
> … rather than a one-off manual check.

This is not true as merged:

- `docs/ci-branch-protection.md` is the repo's single source of truth for the required set, and it is **not in
  this PR's diff**. It still names exactly six checks: `fmt`, `clippy`, `test`, `deny`, `release-guard`,
  `a3b-merge-gate`. Neither `coverage` nor `release-ldd-guard` was added to that list, nor to the "Owner
  quickstart" step 4, nor to the "The checks" table.
- That same document records branch protection as **"NOT YET CONFIGURED"**, so at present *no* check blocks
  merge, required-set membership notwithstanding.
- `a3b-merge-gate` is a peer job, not an aggregator — it has no awareness of, and no dependency on, the two new
  jobs. There is no `needs:` edge and no aggregate gate job in `ci.yml`, so adding a job does not implicitly
  gate anything.

The jobs will run and go red visibly on the PR, which has real value — but "required" is a specific,
owner-actionable claim about branch protection, and shipping it as documentation that is false invites exactly
the false confidence the D-1/B-5 stories were written to remove.

**Suggested fix:** update `docs/ci-branch-protection.md` (checks table, Owner quickstart step 4, and the
"Read this first" status row) to add `coverage` and `release-ldd-guard`, and soften §18's "required" to
"machine-checked in CI; add to branch protection per `docs/ci-branch-protection.md`" until the owner has
actually configured it.

---

## 3. NON-BLOCKING

### N1 — `--ignore-filename-regex` in the `coverage` job is a complete no-op (confidence 93)

`.github/workflows/ci.yml`:

```
--ignore-filename-regex '^(src/main\.rs|src/server\.rs|src/obs\.rs|src/convert/(html|boilerplate)\.rs|spikes/)'
```

`cargo llvm-cov` matches this against the **absolute** path it writes to `SF:`. In my run every record is
`SF:/var/home/meconnelly/Documents/GitHub/Fetch/src/…`, so the `^` anchor can never match, and
`src/main.rs`, `src/server.rs` and `src/obs.rs` are all **still present** in the generated `lcov.info`.
The regex excludes nothing.

This is currently harmless — `coverage_gate.py` only sums the six explicitly-named files, so the extra records
do not affect the verdict — but it is dead configuration that silently does nothing, and the uploaded
`coverage-lcov` artifact is not the filtered artifact it appears to be. Drop the `^` (or anchor on `/`) if the
filtering is meant to do something.

### N2 — `src/fetch/mod.rs` (the redirect loop) is at 89.2%, below the AC's 90% (confidence 88)

The combined-not-per-file reading is deliberate and is documented in both `coverage_gate.py:78-79` and
BENCHMARK §18, so this is an owner decision rather than a concealed defect. It is worth an explicit
acknowledgement, though, that the single module sitting below the AC threshold is the **redirect loop** —
the most security-relevant of the six, and the one the SSRF story exists to protect. Recommend either lifting
it over 90% or recording the deviation in `docs/EPICS.md` alongside the other A-5 deviations.

### N3 — The ldd guard cannot see statically-linked TLS (confidence 85, mitigated)

`ldd` reports only *dynamic* linkage. The most realistic way OpenSSL re-enters a Rust project is
`openssl-sys` with the `vendored` feature, which links `libssl`/`libcrypto` **statically** — `ldd` would show
nothing and the guard would pass silently.

**Mitigated, not a hole:** `deny.toml` already denies `openssl`, `openssl-sys`, `native-tls`, `aws-lc-sys` and
`aws-lc-rs` at the dependency-graph level, and the `deny` job *is* in the required set. The ldd guard is
therefore genuine defence-in-depth rather than the primary control. Worth a one-line comment in the job saying
so, to stop a future reader retiring the `deny` bans on the strength of the ldd check.

### N4 — The ldd guard is brittle against a fully static target (confidence 82)

```
set -euo pipefail
ldd target/release/fetch-mcp | tee ldd.txt
```

On a fully static binary `ldd` prints `not a dynamic executable` and **exits 1**; with `pipefail` the job then
fails spuriously. Only the default gnu target is built today so this does not bite, but the shipped matrix
includes musl (per `bench.yml`), and anyone extending this job to musl will hit it. The failure mode is safe
(fails closed), just confusing. Consider `ldd … || true` with an explicit "not a dynamic executable" success
branch.

Also noted, not faulted: the job builds only the gnu target, so musl release artifacts are unguarded; and the
grep runs over the full `ldd` output including paths, so a runner workspace path containing `ssl`/`crypto`
would false-positive (not the case on `/home/runner/work/Fetch/Fetch`).

### N5 — `coverage_gate.py` minor robustness (confidence 80)

Verified **fail-closed** on every input I could construct — missing file, empty file, truncated record with no
`end_of_record`, malformed `DA:` line, and a mistyped module path all exit 1. Two residual nits:

- `parse_lcov` does `result[current] = (hit, found)` — a **duplicate `SF:` record overwrites** rather than
  accumulates. No duplicates exist in the current output (18 unique `SF:` records, none repeated), but
  per-instantiation or per-binary records would silently undercount.
- With **no file arguments** the gate prints `0/0 = 100.0%` and exits 0 — a vacuous pass. Only reachable by
  editing the workflow, but a `if not files: return 2` guard is one line.

### N6 — Both stdio tests hardcode the parameter list instead of deriving it from the live schema (confidence 80)

`tests/stdio.rs` — the new `d5_tool_description_is_concise_and_names_every_parameter` is **genuine, non-vacuous
coverage**: it starts a real `Session`, performs the handshake, issues a live `tools/list`, and reads
`result.tools[0].description` off the response (priority 5 — answered: real, not hardcoded). It passes in my run.

The gap is that "names every parameter" is implemented against a literal `["url", "max_length", "start_index",
"raw"]`, the same literal the sibling `initialize_lists_exactly_one_fetch_tool_with_schema` uses. Adding a fifth
parameter to `inputSchema` would be caught by **neither** test, despite the name promising otherwise. Deriving
the loop from `tools[0]["inputSchema"]["properties"]` keys would make the test self-maintaining and match its
name. Minor also: `lower.contains("raw")` matches incidental substrings ("drawn", "raw" inside another word).

---

## 4. CI security review (priority 2 — clean)

| Check | Result |
|---|---|
| Expression injection in `run:` blocks | **None.** The only `${{ }}` in `ci.yml` is `concurrency.group: ci-${{ github.ref }}`; no interpolation appears in any `run:` block. Both new jobs use fully static scripts. |
| `pull_request_target` | Not used anywhere in `ci.yml`. |
| Token permissions | Workflow-level `permissions: contents: read`; **no job-level override** in either new job. Minimal — `upload-artifact` needs nothing beyond it. |
| `persist-credentials` | `false` on both new checkouts, consistent with the existing jobs. |
| SHA pinning | Preserved. `actions/checkout@11bd719…` (v4.2.2), `taiki-e/install-action@d850aa8…` (v2.63.3), `actions/upload-artifact@ea165f8…` (v4.6.2) — all full-SHA pinned with version comments. I could not verify offline that the `taiki-e` SHA is genuinely tag v2.63.3; worth one `gh api` confirmation since that action downloads a prebuilt `cargo-llvm-cov`. |
| Secrets | Neither new job references any secret. |

No findings.

## 5. Bench-gate provenance (priority 4 — verified, with one caveat)

The build-identity claim in §18 checks out exactly:

- Recorded commit `8d606cc75bd0cbd34720088ada22c59f63e33528` — matches `git rev-parse 8d606cc`.
- Recorded `Cargo.lock` hash `65a5a943b0f1f2aa31f26b87c3958c5413bfa2a57c889076e3b7a4ed0454cce1` — matches
  `git show 8d606cc:Cargo.lock | sha256sum` exactly.
- **The measurement is valid for the PR head.** `8d606cc..eda0408` touches only `docs/BENCHMARK.md` and
  `.delivery/…/dev-report.md`; `Cargo.lock` is byte-identical at both commits, so the binary measured at
  `8d606cc` is the binary `eda0408` would produce.
- The `--gate` harness independently re-checks that the shipped and bench builds agree on commit + lock hash
  (per `docs/ci-branch-protection.md`), so a mismatched pair would have failed the run rather than been recorded.

**Caveat (accepted, not faulted):** the figures are hand-transcribed from a `workflow_dispatch` run into
Markdown. Nothing machine-binds run [35682100887] to this PR — `bench-gate` is not a required check and its
`pull_request` path filters legitimately do not match a docs/CI/tests-only diff. Verifying the table means
opening the run link by hand. The PR states this limitation openly and the reasoning is sound; I record it only
so the owner knows the figures rest on reviewer trust rather than on a gate.

---

## 6. What is good here

- The coverage gate **fails closed** on every malformed/missing-input case I could construct (5/5), including
  the subtle truncated-lcov case. That is better than most hand-rolled gate scripts.
- The documented local figure (97.7%, 1832/1876) reproduces **exactly**, which says the numbers in §18 were
  really measured rather than estimated.
- The new stdio test is real integration coverage against the live `tools/list` response, not a literal
  restatement of the description constant.
- CI security hygiene (SHA pinning, `persist-credentials: false`, minimal permissions, no interpolation) is
  maintained without exception in the new jobs.
- The build-identity bookkeeping for the bench re-measure is honest and fully verifiable, including an
  explicit statement of why the job did not auto-trigger.

## 7. Recommended path to approval

1. Remove `src/ssrf/differential.rs` from the gated file list, and exclude inline `#[cfg(test)]` regions (or
   move them to sibling `tests.rs`); re-baseline `--min` against the resulting production-only figure (94.5%
   today, so a 90% floor still passes with margin). **[B1]**
2. Update `docs/ci-branch-protection.md` to add `coverage` and `release-ldd-guard`, and correct the "required"
   wording in `docs/BENCHMARK.md` §18. **[B2]**
3. Optional in this PR: fix the no-op `--ignore-filename-regex` anchor **[N1]** and record the `fetch/mod.rs`
   89.2% deviation **[N2]**.
