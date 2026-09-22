# Sprint 12 dev report: Labelling, CI gate, v1.0 (B-6, E-6, D-6 = 7 pts)

Branch: `sprint-12/labelling-ci-gate-v1`, off `main` at `152e2ac` (Sprint 11 merge).
Scope per the Sprints 6-12 table row 12 exactly: **B-6, E-6, D-6**. This is the final planned sprint.

## Fix-pass 1 (after the first hosted CI run, PR #16): E-6 baseline reseed

The first hosted CI run (35702663022) caught a real bug in this sprint's own initial baseline seeding, not in
the tripwire logic itself. `bench-gate (amd64, musl)` **FAILED**: the absolute-target gate PASSED cleanly
(gating peak 5.58 MiB against the 40 MiB target, idle 2.29 MiB against 10 MiB), but the new E-6 regression
step failed the job, because the initial `bench/baseline.json` seed (amd64-musl peak 4567 kB) was taken from
`docs/BENCHMARK.md` section 17's **Sprint 5** G4b figures, and this run measured 5712 kB (+25%, over the 10%
band). That gap is not CI noise: real, disclosed memory growth landed between Sprint 5 and Sprint 11 (A-8
charset decoding, `docs/BENCHMARK.md` section 19: "gzip-HTML with no charset header went from streaming to
~4x-cap buffered", an explicitly accepted trade-off that stays well under the 40 MiB target but was never
reflected in a baseline seeded from Sprint 5 data). The other three cells (amd64-gnu, arm64-gnu, arm64-musl)
all passed both the absolute gate and the regression check against the same stale seed, so this was
specifically an amd64-musl gap, not a general problem with the 10% threshold.

**Fix:** reseeded `bench/baseline.json` from this PR's own bench-gate run (35702663022) rather than from the
Sprint 5 doc figures. This is legitimate, not a way to dodge the check: this sprint's `src/`, `Cargo.toml` and
`Cargo.lock` are byte-identical to `main` at `152e2ac` (no Rust source was touched — see "Files touched"
below), so this run's own measured figures **are** current-main figures, exactly what a properly-bootstrapped
baseline should start from. New values (kB): amd64-gnu idle 4676/peak 6126, amd64-musl idle 2348/peak 5712,
arm64-gnu idle 4030/peak 5462, arm64-musl idle 2520/peak 4412. Pushed as a follow-up commit on this branch;
re-running CI to confirm all four `bench-gate` legs now pass is the next step. From this point on, the
`update-baseline` job keeps the baseline current automatically on every push to `main`, so a one-sprint-old
seed cannot recur this way.

## Summary of what was delivered

| Story | Result |
|---|---|
| B-6 (labelling) | CLOSED as won't-do, re-confirmed. No code change. OQ-5 was decided "no label" in Sprint 2 (Revision 10); the result envelope has carried no wrapper/notice since Sprint 5 (A-5/A-6), and remains that way. |
| E-6 (CI memory-regression gate) | Implemented inside the existing `bench-gate` job/matrix: a new 10%-vs-last-main-baseline regression tripwire, additive to the pre-existing strict 10 MiB/40 MiB absolute gate. Self-tested. NOT added to branch protection (no repository-settings access — same constraint as D-3/D-7). |
| D-6 (licensing, dependency audit, release tag) | Audit and dependency-count mechanisms implemented and green (`cargo audit`, `cargo deny`, an 11-of-15 direct-dependency check). Release-notes generation implemented inside the still-blocked `publish` job. **The `v1.0` git tag is deliberately NOT created** — OQ-7 is still open, and D-6's own AC couples the tag to an OQ-7 licence decision this agent has no authority to make. |

No Rust source changed this sprint (`src/`, `Cargo.toml`, `Cargo.lock` untouched). All changes are CI workflows, two new Python scripts (plus a self-test), one new committed baseline file, and documentation.

## B-6: Untrusted-content labelling — CLOSED, re-confirmed

Re-checked the Sprint 12 entry criterion ("OQ-5 decided") and the story's own AC bullet 3 ("Given OQ-5 is decided as 'no label', when the story is reviewed, then it is closed as won't-do"). Both were already satisfied as of Sprint 2 (Revision 10, 2026-09-20) and Sprint 5 (A-5's decision write-up: "`[Total length: N characters.]`... keeps a small page 'returned as fetched' (OQ-5 decision: no untrusted-content label)"). `src/server.rs`'s module doc and `tests/stdio.rs` already state and assert the behaviour. No implementation, test, or doc gap was found. Nothing was changed for B-6 beyond the confirmation write-up in `docs/EPICS.md`.

## E-6: CI regression gate on aarch64

**What the AC asks for vs. what already existed.** Bullet 1 ("CI runs the E-3/E-4 checks against the built binary on hosted amd64/arm64") was already fully satisfied by the pre-existing `bench-gate` job in `.github/workflows/bench.yml` (added in Sprints 3-5). What did **not** exist: the AC's regression-tripwire condition ("the median idle or peak RSS exceeds the absolute target, **or regresses more than 10% against the stored last-main baseline**... E-6 is a regression tripwire, while the release gate (E-3/E-5) is the strict absolute target"). That is what this sprint adds.

**Design.** Rather than a new job (which would need a new required-check name that then has to go through the same manual branch-protection process as everything else), the tripwire runs as an additional step inside the existing four-cell `bench-gate` matrix:

- `bench/baseline.json` — a new file, committed to the repo, one entry per `{arch}-{libc}` cell (`amd64-gnu`, `amd64-musl`, `arm64-gnu`, `arm64-musl`), each `{idle_kB, peak_kB, updated_from_run, updated_utc}`. Seeded from the last recorded PASS figures in `docs/BENCHMARK.md` section 17 (G4b, CI run 35641694726).
- `scripts/regression_gate.py` — new script. `regression_gate.py --idle out/idle.jsonl --peak out/peak.jsonl --baseline bench/baseline.json --platform <cell> [--max-regression-pct 10]` reads the current run's idle median (`idle.jsonl`'s `idle` scenario record's `median_kB`) and gating-peak median (`peak.jsonl`'s summary `gating_peak_kB`), compares each against the stored baseline, and exits 1 if either regressed more than the threshold — **even if the absolute target is still comfortably met**. It never lowers or raises the absolute 10 MiB/40 MiB targets that `bench/measure.py --gate` already enforces independently; the two checks are separate steps with separate exit codes, both required for the job to pass. A missing baseline entry (first run for a cell) is advisory (prints and exits 0) rather than a false pass being silently baked in, or a false gate failure on day one.
- `bench.yml` — new step `regression` in `bench-gate` (after the existing `peak` step), wired into the job's final "fail if any gating step failed" check alongside `idle`/`idle_bench`/`peak`. New job `update-baseline` (`needs: bench-gate`, `if: github.ref == 'refs/heads/main' && github.event_name == 'push'`), one matrix leg per cell, downloads that cell's uploaded `bench-gate-*` artifact, calls `regression_gate.py --update` to compute the new medians, and commits+pushes `bench/baseline.json` back to `main` with `[skip ci]` (retries up to 5 times on a push race via `git pull --rebase`). This job is the only place in any workflow in this repo that requests `contents: write` (everything else stays `contents: read`, matching architecture 9.2's least-privilege rule); it never runs on a PR, so a PR cannot move its own goalposts.
- `scripts/regression_gate_selftest.py` — new self-test (same style/pattern as the pre-existing `bench/selftest.py`), wired into `ci.yml`'s `test` job. Proves: PASS within 10%, FAIL beyond 10% (idle and peak checked independently, each names itself in the failure message), a missing baseline entry is advisory not a silent pass, and `--update` writes and is then honoured on the next check. Ran locally: all 9 assertions pass (see "Test evidence" below).

**Deviations from the AC text, disclosed rather than silently narrowed:**
1. *"the memory-gate job is added as a required status check"* — **NOT done.** No agent working on this repository has permission to change GitHub branch-protection settings (the same constraint D-3 disclosed in Sprint 11 for the `test`→`test (amd64)`/`test (arm64)` rename, and D-7 disclosed from Sprint 0 onward for the original six checks). The mechanism is real, self-tested, and runs on every PR and push today; `docs/ci-branch-protection.md` now documents it and lists it explicitly as something the owner can add once satisfied (with the pre-existing fork-PR-skip caveat still applying, unchanged by this sprint).
2. *"the release workflow from D-2 is updated to depend on it"* — **NOT done.** `release.yml`'s `publish` job is already hard-gated `if: false && ...` (blocked on OQ-7 since Sprint 9/D-2, `.delivery` Revision 18-19). Adding a `needs: bench-gate` dependency to a job that cannot run under any trigger today would be untestable and would touch a block of that workflow that already carries careful, previously-reviewed language about exactly what a human must do to unblock it (see the comment above the `publish:` job). Wiring the dependency is left for whoever performs that OQ-7-driven edit, so it can be reviewed together with the rest of the unblocking change rather than added speculatively now.
3. *Scope of "median idle or peak RSS"* — read as the two headline figures the harness already gates (idle scenario median, gating-peak summary median), not every recorded/advisory scenario (`redirect-chain5`, `g6-concurrent10`, `hostile-attrs3`, `hostile-attrvalue3`). Those remain recorded-only exactly as E-4 defined them; this sprint did not change what is gating versus recorded.

**Test evidence (local):**
```
$ python3 scripts/regression_gate_selftest.py
ok   within threshold exits 0
ok   idle regression beyond threshold exits 1
ok   idle regression names the metric in stderr
ok   peak regression beyond threshold exits 1
ok   missing baseline entry is advisory, exits 0, never a silent regression-hide of the absolute gate
ok   missing baseline entry says so on stderr
ok   --update exits 0
ok   --update writes the new medians
ok   --update records the run id
ok   updated baseline is honoured on the next gate check
all regression_gate self-tests passed
```
`bench/selftest.py` (pre-existing harness self-test) still passes unchanged, confirming the new step did not disturb the existing harness. All four `bench.yml`/`ci.yml` workflow files were parsed with `yaml.safe_load` to confirm no syntax errors from the edits (a quoting bug — an unquoted step `name:` containing a colon — was caught and fixed this way during development).

## D-6: Licensing, dependency audit and release tag

**AC bullet 1** ("`cargo audit` and `cargo deny` report neither reports high or critical issues") — **DONE.** `deny.toml`'s `[advisories]` section already ran the RustSec advisory database via `cargo deny check` (job `deny`, since Sprint 0/D-7), but `cargo-audit` itself — the tool D-6 names specifically — had never been installed or run anywhere, an open item carried and reflagged in every sprint's carried-items list since Sprint 5. New CI job `audit` (`ci.yml`) installs `cargo-audit` 0.22.2 (pinned by version; RustSec ships no first-party SHA-pinnable GitHub Action, so this follows the same "install by pinned version" pattern already used for `cargo-zigbuild`/`cargo-llvm-cov` elsewhere in this repo) and runs `cargo audit --deny warnings`. Ran clean locally before this PR opened:
```
$ cargo audit --deny warnings
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1261 security advisories (from ~/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (209 crate dependencies)
(no output after this — 0 vulnerabilities found, exit 0)
```

**AC bullet 2, first half** ("LICENSE is present") — already true since the initial commit (Apache-2.0), unchanged.
**AC bullet 2, second half** ("direct dependencies are at most 15") — now machine-checked, not just tracked by a hand-kept comment: new CI job `dependency-count` runs `scripts/dependency_count_gate.py Cargo.toml --max 15`, a small stdlib-only TOML-`[dependencies]`-table parser (deliberately no `toml` package dependency, so it can run in a bare Python step before any Rust/pip install). Local run: `[dependencies] in Cargo.toml: 11 direct (encoding_rs, flate2, html-escape, lol_html, reqwest, rmcp, rustls, schemars, serde, tokio, webpki-roots) — within NFR-05 (<= 15)`.
**AC bullet 2, "given a licence is selected per OQ-7"** — **explicitly NOT satisfied**, and not claimed to be. OQ-7 (image distribution and licence) is still open. The pre-existing Apache-2.0 `LICENSE` file is a fact about this repository's own source licence, decided at the initial commit before OQ-7 was even raised as a question — it is not an answer to OQ-7, which is about whether/how the *container image* is distributed publicly and under what terms. `docs/ci-branch-protection.md` now states this distinction explicitly in a new "D-6 status" paragraph, so a future reader cannot mistake the existing LICENSE file for an OQ-7 decision.

**AC bullet 3** ("release notes link the benchmark report") — mechanism implemented, not yet run. Added a new step, "D-6: create the GitHub release with notes linking the benchmark report and both digests", to `release.yml`'s `publish` job. It builds release notes linking `docs/BENCHMARK.md` at the tag ref, states the manifest digest and image reference, and summarises the audit/deny results, then runs `gh release create <version> --notes-file ... --verify-tag`. **This step cannot run today**: it lives inside the `publish` job, which remains hard-gated `if: false && startsWith(github.ref, 'refs/tags/v') && inputs.confirm_publish == 'true'` exactly as D-2 left it in Sprint 9 (blocked on OQ-7; a human must deliberately remove the `false &&` in its own reviewed PR, per the detailed comment already in that job from Sprint 9). This sprint did not touch that gate condition.

**Why no `v1.0` tag was created.** This was an explicit instruction for this sprint, and independently follows from the plan itself: D-6's AC ties tagging to "a licence selected per OQ-7", and the sprint plan's risk table states plainly that "Publishing a public image (GHCR) is a distribution act... Publishing before OQ-7 is answered would decide licence/distribution by default." Creating a `v1.0` git tag against `main` — even without enabling the image `publish` job — would be the kind of milestone action that should follow a coordinator's full review of this sprint's PR, not something decided unilaterally by the sprint developer. D-6 is therefore **implemented (mechanism) but NOT Done** by its own AC, in the same "shipped inert, pending an owner OQ decision" pattern already used for B-4 (Sprint 11, robots.txt), C-1 (Sprint 10, robots default placeholder) and C-2 (Sprint 10, allowlist mechanism).

## Files touched

- `.github/workflows/bench.yml` — new `regression` step in `bench-gate`; new `update-baseline` job.
- `.github/workflows/ci.yml` — new `audit` job, new `dependency-count` job, new self-test step in `test`.
- `.github/workflows/release.yml` — new release-notes step in the still-blocked `publish` job.
- `bench/baseline.json` — new, seeded baseline for the regression tripwire.
- `scripts/regression_gate.py` — new.
- `scripts/regression_gate_selftest.py` — new.
- `scripts/dependency_count_gate.py` — new.
- `docs/ci-branch-protection.md` — new "E-6 memory-regression tripwire" section; updated `audit`/`dependency-count`/`test (amd64)`/`test (arm64)` references throughout the required-checks table and owner quickstart; new "D-6 status" paragraph in the Licence section.
- `docs/EPICS.md` — Sprint 12 status write-ups appended to the B-6, E-6 and D-6 story sections.
- `README.md` — corrected the stale "`cargo-audit` has never been run" line; added a Sprint 12 status bullet summarising B-6/E-6/D-6.

## Owner decisions still needed (unchanged in substance, carried forward from Sprint 11's Revision 22, plus one new item)

- **OQ-3** (robots.txt default policy — enforce vs. ignore).
- **OQ-4** (private-host allowlist activation/governance).
- **OQ-7** (image distribution/licence) — blocks D-2's `publish` job, the release-notes step added this sprint, and the `v1.0` tag itself (D-6).
- **Branch protection on `main` is still not configured** — the single largest carried-forward action item across every sprint of this engagement (D-7, Sprint 0, through today). This sprint added two more checks (`audit`, `dependency-count`) and one new regression condition inside an existing job (`bench-gate`'s new `regression` step) to the list of things the owner can make required once ready; none of them can be made required by any agent.
- **D-3's `test`→`test (amd64)`/`test (arm64)` rename** (Sprint 11) is still un-reconciled with branch protection, because branch protection still does not exist to reconcile it with.
- E-5/E-7 owner URL lists (10-URL smoke / 50-URL snapshot set) — still not supplied, unchanged since Sprint 2.
- Previously-carried items, unchanged: Sprint 5 deviations acknowledgement, the `panic=abort` deviation (A-7), `coverage`/`release-ldd-guard` not yet in required checks, A-2's throwaway-config check.
- **New this sprint:** whether/when to remove the `false &&` guard on `release.yml`'s `publish` job (requires an OQ-7 decision plus a deliberate, separately-reviewed PR per that job's own comment) — the release-notes step added this sprint sits inside that same gate and inherits the same block.

## Definition of Done checklist

- All new/changed acceptance criteria have a passing check: `regression_gate_selftest.py` (9 assertions), `dependency_count_gate.py` run against the real `Cargo.toml` (11 <= 15), `cargo audit --deny warnings` (0 vulnerabilities), all workflow YAML re-parsed for syntax validity.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` all pass locally (unchanged from Sprint 11 — no Rust source was touched this sprint).
- No new direct Rust dependency (`Cargo.toml` untouched; still 11 of <= 15, now machine-checked).
- stdout/stderr discipline unaffected (no server code changed).
- Docs updated where behaviour is user-visible or CI-visible: `docs/ci-branch-protection.md`, `docs/EPICS.md`, `README.md`.
- Self-review complete; CI status for the opened PR is reported separately once it has run.
