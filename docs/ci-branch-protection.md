# CI and branch protection (D-7)

**What is this?** CI (continuous integration) is a set of automatic checks that GitHub runs on every change. This file lists those checks and the rule that should stop broken changes reaching `main`. **Who needs it?** The repository owner, who must switch the rule on, and contributors who see a check fail. **What to do first:** read "Read this first" below, then follow "Owner quickstart".

Workflow file: `.github/workflows/ci.yml`. It uses hosted `ubuntu-latest` runners only. It has no self-hosted runner, no secrets and no `pull_request_target`.

## Read this first: what has and has not been checked

| Item | Status |
|---|---|
| Rule on `main` (required checks) | NOT YET CONFIGURED. The owner sets it after the first CI run. |
| The workflow on hosted GitHub Actions | Never run. |
| `cargo-deny` (the `deny` job) | Never run anywhere. It is not installed on the dev host. |
| `cargo-audit` | Never run anywhere. Not installed on the dev host. |
| Everything else | Checked only by running its commands locally, plus `actionlint` (a tool that checks workflow files for mistakes). |

Do not read "required checks" as "working checks". Until the first hosted run, the checks are unproven there.

## The checks

A "required status check" is a CI job that must pass before a change can be merged. The check names are the job ids in `ci.yml`, and they must match exactly.

| Check | Command | Plain meaning |
|---|---|---|
| `fmt` | `cargo fmt --check` | Code is formatted the standard way. |
| `clippy` | `cargo clippy --locked --all-targets -- -D warnings`, then the same with `--features bench-loopback`, then the A-1 spike crate (`spikes/a1`, a separate crate) with the same flags for the default and four HTTP-backend feature sets | Clippy (Rust's code-advice tool) finds no warnings. `-D warnings` turns every warning into a failure. |
| `test` | `cargo test --locked`, then with `--features bench-loopback`, then with `--features test-support`, then `python3 bench/selftest.py` | The tests pass in three feature setups, and the benchmark self-test passes. |
| `deny` | `cargo deny --locked check` (`deny.toml`) | Dependencies have no known security problems and follow our rules. |
| `release-guard` | `scripts/check-release-features.sh` and `--self-test` | A release build has no test-only features in it. |

"Locked" (`--locked`) means the build must use exactly the versions in `Cargo.lock`. A "feature" is an optional switch compiled into the program.

## Owner quickstart: turn on the rule

Do this after the first CI run. The check names only appear in GitHub once they have run.

1. Open the repository on GitHub. Go to Settings, then Branches.
2. Add a rule for `main`.
3. Tick "Require status checks to pass before merging".
4. Add these five checks: `fmt`, `clippy`, `test`, `deny`, `release-guard`.
5. Tick "Require branches to be up to date".
6. Tick "Require a pull request before merging".
7. Write it down in the sprint PR, using this template: "Configured DD-MM-YYYY: checks fmt, clippy, test, deny, release-guard; up-to-date and PR required".

Success: a pull request shows the five checks, and the merge button stays locked until they pass.

Later stories add more required checks (D-3, E-6). Add them to the rule when they land.

## What can go wrong

| Symptom | What it means | Fix |
|---|---|---|
| The five check names are not offered in the rule picker | The workflow has not run yet, so GitHub does not know the names. | Let CI run once on a pull request, then add the rule. |
| `fmt` fails | Code is not formatted. | Run `cargo fmt`, commit the result. |
| `clippy` fails | A warning was found. | Run the failing command from the table locally and fix what it prints. |
| `test` fails | A Rust test or the benchmark self-test failed. | Run the four commands in the table locally, in order. |
| `deny` fails | A dependency problem. This job has never been run yet, so a first failure may also be a set-up problem. | Read the job log. Do not assume the code is at fault. |
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
