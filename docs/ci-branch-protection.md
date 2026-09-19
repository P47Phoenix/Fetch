# CI and branch protection (D-7)

Workflow: `.github/workflows/ci.yml` (hosted `ubuntu-latest` only; no self-hosted runner, no secrets, no `pull_request_target`).

## Required status checks on `main`

Status: NOT YET CONFIGURED. The workflow exists on the sprint branch; the rule on `main` must be set by the owner after the first CI run (the check names appear in GitHub only once they have run). The `test` check runs only `cargo test --locked` (default features); the `bench-loopback` tests are added by E-8 and do not run yet.

The owner configures these in GitHub (Settings, Branches, rule for `main`, "Require status checks to pass before merging") and records it in the sprint PR (template: "Configured DD-MM-YYYY: checks fmt, clippy, test, deny, release-guard; up-to-date and PR required"). The check names are the job ids:

| Check | Command |
|---|---|
| `fmt` | `cargo fmt --check` |
| `clippy` | `cargo clippy --locked --all-targets -- -D warnings`, then the same with `--features bench-loopback`, then the A-1 spike crate (`spikes/a1`, separate crate) with the same flags for the default and four HTTP-backend feature sets |
| `test` | `cargo test --locked`, then with `--features bench-loopback`, then with `--features test-support`, then `python3 bench/selftest.py` |
| `deny` | `cargo deny --locked check` (`deny.toml`) |
| `release-guard` | `scripts/check-release-features.sh` and `--self-test` |

Also enable "Require branches to be up to date" and "Require a pull request before merging". Add later required checks as stories land (D-3, E-6).

## Pins

- Toolchain: `rust-toolchain.toml`, channel `1.94.1` (bumps are their own PR with a benchmark re-run).
- `Cargo.lock` is committed; every CI command uses `--locked`.
- Every action is pinned by full commit SHA with a version comment (`actions/checkout` v4.2.2, `EmbarkStudios/cargo-deny-action` v2.1.1, both peeled to the commit SHA with `git ls-remote`).
- Release profile in `Cargo.toml`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (A-1 values; D-1 finalises).

## Release-feature guard

`scripts/check-release-features.sh` builds `-p fetch-mcp --release --locked`, asserts via `cargo tree -e features` that the root package enables only allowlisted features (`default`; anything else, including a renamed or new feature, fails), and greps the binary for the `FETCH_MCP_MARKER_` prefix (any marker fails). `--self-test` builds each forbidden feature and requires the guard to fail for both reasons, and checks a renamed feature and an unknown marker (positive controls). `--features CSV` runs the same tree and marker checks on a release build with those features (the mode the self-test uses to prove the guard fails on `test-support` and `bench-loopback`; it is expected to exit non-zero for them, and CI never passes it). `--binary PATH` greps an existing artifact (used by D-2).

## Dependencies and advisories

`deny` runs `cargo deny check` including RustSec advisories (`deny.toml` `[advisories]`, yanked = deny). A scheduled advisory audit (nightly, on the default branch) is D-3. `.github/dependabot.yml` opens weekly update PRs for Cargo and GitHub Actions (action pins stay full-SHA; Dependabot updates the SHA and comment). `cargo audit` is not installed here; `cargo deny check advisories` covers the same database.

## Licence

An Apache-2.0 `LICENSE` file exists from the initial commit. The project's licence and distribution remain open under OQ-7 and are not decided here; `Cargo.toml` has `publish = false` until they are decided. Nothing in CI, `deny.toml` or the benchmark docs depends on the choice (`deny.toml` `[licenses]` covers dependencies only).
