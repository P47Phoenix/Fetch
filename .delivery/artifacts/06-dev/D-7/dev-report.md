# D-7 dev report

## Done
Root crate `fetch-mcp` (single crate per architecture sec 1, no `[workspace]`), `src/lib.rs`, `src/main.rs`, `rust-toolchain.toml` (1.94.1), pinned `[profile.release]` (opt-level s, lto, codegen-units 1, panic abort, strip), `deny.toml`, `.github/workflows/ci.yml`, `scripts/check-release-features.sh` (+ `--self-test`, `--binary`), `docs/ci-branch-protection.md`. Features `test-support`, `bench-loopback`, `fixture-ca` exist as empty compile-time flags that embed marker strings.

## Commands run (local)
- `cargo fmt --check`: pass
- `cargo clippy --locked --all-targets -- -D warnings`: pass
- `cargo test --locked`: pass (1 test)
- `scripts/check-release-features.sh`: pass ("guard OK")
- `scripts/check-release-features.sh --self-test`: pass (clean passes; each of the three features fails the guard on both tree and marker detection)
- `actionlint` and python yaml load on ci.yml: pass; `shellcheck`: no findings
- Action SHAs from `git ls-remote`: checkout v4.2.2 = 11bd7190..., cargo-deny-action v2.1.1 tag is annotated, peeled commit 3c634983...

## AC checklist
- fmt/clippy/test on hosted CI with --locked: workflow written; NOT RUN (Actions cannot run here)
- rust-toolchain pin, Cargo.lock committed, --locked, SHA-pinned actions: pass
- deny.toml sections (advisories, bans x5, sources, licenses + private ignore): written; cargo-deny NOT installed, NOT RUN locally
- release guard with self-test, `-p` release build: pass locally; CI run NOT RUN. "E-8 tests with --features bench-loopback": deferred (no tests yet; comment in ci.yml)
- release profile pinned, publish=false: pass
- branch protection: documented only in docs/ci-branch-protection.md; owner must configure (NOT DONE, cannot)
- no self-hosted / fork safety: pass by inspection (hosted only, read-only permissions)

## Deviations
- Edited root `.gitignore` (outside listed scope) to stop ignoring `Cargo.lock`, required by the commit-lockfile AC. Side effect: `spikes/a1/Cargo.lock` now shows untracked; NOT staged by me (architecture references it for version values; owner may commit it).
- Architecture pins direct deps exactly; the stub has no dependencies, so none pinned yet.
- Dependabot config not added (architecture mentions it; not in D-7 ACs).
- Toolchain `targets` includes aarch64 targets; CI does not use them yet.
