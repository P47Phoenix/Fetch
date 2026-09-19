# CI and branch protection (D-7)

Workflow: `.github/workflows/ci.yml` (hosted `ubuntu-latest` only; no self-hosted runner, no secrets, no `pull_request_target`).

## Required status checks on `main`

The owner configures these in GitHub (Settings, Branches, rule for `main`, "Require status checks to pass before merging") and records it in the sprint PR. The check names are the job ids:

| Check | Command |
|---|---|
| `fmt` | `cargo fmt --check` |
| `clippy` | `cargo clippy --locked --all-targets -- -D warnings` |
| `test` | `cargo test --locked` |
| `deny` | `cargo deny --locked check` (`deny.toml`) |
| `release-guard` | `scripts/check-release-features.sh` and `--self-test` |

Also enable "Require branches to be up to date" and "Require a pull request before merging". Add later required checks as stories land (D-3, E-6).

## Pins

- Toolchain: `rust-toolchain.toml`, channel `1.94.1` (bumps are their own PR with a benchmark re-run).
- `Cargo.lock` is committed; every CI command uses `--locked`.
- Every action is pinned by full commit SHA with a version comment (`actions/checkout` v4.2.2, `EmbarkStudios/cargo-deny-action` v2.1.1, both peeled to the commit SHA with `git ls-remote`).
- Release profile in `Cargo.toml`: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (A-1 values; D-1 finalises).

## Release-feature guard

`scripts/check-release-features.sh` builds `-p fetch-mcp --release --locked`, asserts via `cargo tree -e features` that `test-support`, `bench-loopback` and `fixture-ca` are not enabled, and greps the binary for their marker strings. `--self-test` builds each forbidden feature and requires the guard to fail (positive control). `--binary PATH` greps an existing artifact (used by D-2).
