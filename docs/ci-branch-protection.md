# CI and branch protection (D-7)

**What is this?** CI (continuous integration) is a set of automatic checks that GitHub runs on every change. This file lists those checks and the rule that should stop broken changes reaching `main`. **Who needs it?** The repository owner, who must switch the rule on, and contributors who see a check fail. **What to do first:** read "Read this first" below, then follow "Owner quickstart".

Workflow file: `.github/workflows/ci.yml`. It uses hosted `ubuntu-latest` runners only, with no secrets and no `pull_request_target`. There is no self-hosted runner and none is planned (ADR-007).

A second workflow, `.github/workflows/arm-bench.yml`, runs the advisory native arm64 measurement of the A-1 spike on `ubuntu-24.04-arm`. Its job id is `bench`. It is advisory: do NOT add it to the required checks.

## Read this first: what has and has not been checked

| Item | Status |
|---|---|
| Rule on `main` (required checks) | NOT YET CONFIGURED. The first CI run has now happened, so the owner can set it. |
| The workflow on hosted GitHub Actions | Run once, on pull request #2. All five checks (`fmt`, `clippy`, `test`, `deny`, `release-guard`) passed on commit 740c0fb (run 35470977286). One green run is not a trend. |
| `cargo-deny` (the `deny` job) | Ran once, in that hosted run, and passed. It is still not installed on the dev host. |
| `cargo-audit` | Never run anywhere. Not installed on the dev host. |
| Everything else | Checked only by running its commands locally, plus `actionlint` (a tool that checks workflow files for mistakes). |

Do not read "required checks" as "working checks". They passed once on hosted runners, which shows they can work there, not that they are stable.

## The checks

A "required status check" is a CI job that must pass before a change can be merged. The check names are the job ids in `ci.yml`, and they must match exactly.

| Check | Command | Plain meaning |
|---|---|---|
| `fmt` | `cargo fmt --check` | Code is formatted the standard way. |
| `clippy` | `cargo clippy --locked --all-targets -- -D warnings`, then the same with `--features bench-loopback`, then the A-1 spike crate (`spikes/a1`, a separate crate) with the same flags for the default set and four feature sets (spread across three HTTP backends) | Clippy (Rust's code-advice tool) finds no warnings. `-D warnings` turns every warning into a failure. |
| `test` | `cargo test --locked`, then with `--features bench-loopback`, then with `--features test-support`, then `python3 bench/selftest.py` | The tests pass in three feature setups, and the benchmark self-test passes. |
| `deny` | `cargo deny --locked check` (`deny.toml`) | Dependencies have no known security problems and follow our rules. |
| `release-guard` | `scripts/check-release-features.sh` and `--self-test` | A release build has no test-only features in it. |

"Locked" (`--locked`) means the build must use exactly the versions in `Cargo.lock`. A "feature" is an optional switch compiled into the program.

## Owner quickstart: turn on the rule

Do this now that the first CI run (pull request #2) has happened. The check names only appear in GitHub once they have run.

1. Open the repository on GitHub. Go to Settings, then Branches.
2. Add a rule for `main`.
3. Tick "Require status checks to pass before merging".
4. Add these five checks: `fmt`, `clippy`, `test`, `deny`, `release-guard`.
5. Tick "Require branches to be up to date".
6. Tick "Require a pull request before merging".
7. Write it down in the sprint PR, using this template: "Configured DD-MM-YYYY: checks fmt, clippy, test, deny, release-guard; up-to-date and PR required".

Success: a pull request shows the five checks, and the merge button stays locked until they pass.

Later stories add more required checks (D-3, E-6, D-2). Add them to the rule when they land.

### Planned end state: two platform gate jobs (NOT YET EXISTING)

Per ADR-007 the release image is multi-arch and both platforms are hard-gated on their own native hosted runner. Once the release workflow exists, the required set becomes the five checks above plus two platform gate jobs:

| Check | Runner | Status |
|---|---|---|
| Platform gate for `linux/amd64` | `ubuntu-24.04` | NOT YET EXISTING. Job id not chosen; match it exactly once it exists. |
| Platform gate for `linux/arm64` | `ubuntu-24.04-arm` | NOT YET EXISTING. Job id not chosen; match it exactly once it exists. |

Today the required set is still `fmt`, `clippy`, `test`, `deny`, `release-guard`, matching the job ids in `ci.yml`. Do not add the two gate names until they have run once, because GitHub only offers names it has seen. A skipped required job counts as a failure for the release: the publish job needs both gates, and the release is blocked unless both pass.

### Publish job and image visibility (planned)

- Only the publish job gets `packages: write` (and `id-token` and `attestations` write if provenance is added). Every other job keeps the default `contents: read`.
- The publish job has no fork or pull request trigger. It runs only from a maintainer-controlled event on the default branch or a tag, after both platform gates pass on the same image digest. It adds tags to the tested digest and never rebuilds.
- Candidate images are pushed by digest and left untagged. Fork pull requests run the same hosted jobs with no secrets and never push an image.
- GHCR packages are private by default when a workflow first pushes them. Making the package public is a manual setting and is the distribution act that open question OQ-7 (licence and distribution) must come before. Until then anonymous `docker pull` fails. This document does not decide OQ-7.

## What can go wrong

| Symptom | What it means | Fix |
|---|---|---|
| The five check names are not offered in the rule picker | The workflow has not run yet, so GitHub does not know the names. | Let CI run once on a pull request, then add the rule. |
| `fmt` fails | Code is not formatted. | Run `cargo fmt`, commit the result. |
| `clippy` fails | A warning was found. | Run the failing command from the table locally and fix what it prints. |
| `test` fails | A Rust test or the benchmark self-test failed. | Run the four commands in the table locally, in order. |
| `deny` fails | A dependency problem. This job has passed only once so far, so a failure may also be a set-up problem. | Read the job log. Do not assume the code is at fault. |
| `release-guard` fails | A test-only feature or a `FETCH_MCP_MARKER_` marker ended up in a release build. | See "Release-feature guard" below. |
| The merge button is not locked | The rule is not configured yet (this is the current state). | Follow the Owner quickstart. |

## Pins

A "pin" fixes a version so builds do not change by surprise.

- Toolchain: `rust-toolchain.toml`, channel `1.94.1`. Bumps are their own PR with a benchmark re-run.
- `Cargo.lock` is committed. Every CI command uses `--locked`.
- Every action is pinned by full commit SHA with a version comment. `actions/checkout` is v4.2.2 and `EmbarkStudios/cargo-deny-action` is v2.1.1. Both were peeled to the commit SHA with `git ls-remote`.
- The release profile in `Cargo.toml` is: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. These are the A-1 values. D-1 finalises them.

## Release-feature guard

**Why:** two features, `test-support` and `bench-loopback`, weaken safety checks so tests can run. They must never be in a release.

`scripts/check-release-features.sh` does this:

1. It builds `-p fetch-mcp --release --locked`.
2. It asserts, through `cargo tree -e features`, that the root package enables only allow-listed features. Only `default` is allowed. Anything else, including a renamed or new feature, fails.
3. It searches the binary for the `FETCH_MCP_MARKER_` prefix. Any marker fails.

Options:

| Option | What it does |
|---|---|
| `--self-test` | Builds each forbidden feature and requires the guard to fail for both reasons. It also checks a renamed feature and an unknown marker (positive controls: cases that must fail, to prove the guard can fail). |
| `--features CSV` | Runs the same tree and marker checks on a release build with those features. The self-test uses it to prove the guard fails on `test-support` and `bench-loopback`. It is expected to exit non-zero for them. CI never passes it. |
| `--binary PATH` | Searches an existing artifact (used by D-2). |

## Dependencies and advisories

- `deny` runs `cargo deny check`, including RustSec advisories (`deny.toml` `[advisories]`, yanked = deny). RustSec is a public list of known security problems in Rust packages.
- A scheduled advisory audit (nightly, on the default branch) is D-3. It does not exist yet.
- `.github/dependabot.yml` opens weekly update PRs for Cargo and GitHub Actions. Action pins stay full-SHA. Dependabot updates the SHA and the comment.
- `cargo audit` is not installed here. `cargo deny check advisories` covers the same database.

## Licence

An Apache-2.0 `LICENSE` file has existed since the initial commit. The project's licence and distribution are still open under OQ-7 and are not decided here. `Cargo.toml` has `publish = false` until they are decided. Nothing in CI, `deny.toml` or the benchmark docs depends on the choice. (`deny.toml` `[licenses]` covers dependencies only.)
