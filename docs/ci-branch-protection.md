# CI and branch protection (D-7)

**What is this?** CI (continuous integration) is a set of automatic checks that GitHub runs on every change. This file lists those checks and the rule that should stop broken changes reaching `main`. **Who needs it?** The repository owner, who must switch the rule on, and contributors who see a check fail. **What to do first:** read "Read this first" below, then follow "Owner quickstart".

Workflow file: `.github/workflows/ci.yml`. It uses hosted `ubuntu-latest` runners only, with no secrets and no `pull_request_target`. There is no self-hosted runner and none is planned (ADR-007).

A third workflow, `.github/workflows/bench.yml` (E-2, E-3, E-4), runs job `bench-gate` (amd64 and arm64, gnu and musl, native hosted runners): `--gate` idle RSS on the shipped binary (strict 10 MiB) and the peak scenarios on the bench-loopback build (since E-4 the complete G4a set with conversion, plus `redirect-chain5` and the recorded-only `g6-concurrent10`). Every `--gate` run also refuses unless the shipped and bench builds report the same commit and `Cargo.lock` hash. A step outside the pass condition, `bench/public_check.py` (shipped-vs-bench fetch of one public HTTPS page), is advisory and needs the network. The job ids and matrix names did not change with E-4, so nothing in the required-check list changes. It fails on a missed target, INVALID/INCOMPLETE or a refusal. It is NOT a required check yet: the owner may add `bench-gate (amd64, gnu)`, `bench-gate (amd64, musl)`, `bench-gate (arm64, gnu)` and `bench-gate (arm64, musl)` after watching a few runs (it takes about 16 minutes and also runs nightly and on pushes to main; fork PRs skip it). **Caveat before making it required:** a fork PR skips the job, and a skipped job counts as passing for branch protection, so on a fork PR the required memory gate would report success without measuring anything. Decide that knowingly. **Cost and quota:** each run is 4 cells of about 16 minutes each (about 64 runner minutes), triggered nightly, on every push to main and on same-repo PRs that touch `bench/**`, `src/**`, `build.rs`, `Cargo.*`, the build script or the workflow; arm64 standard runners were free for public repos when checked, which was not verified beyond the jobs running.

A second workflow, `.github/workflows/arm-bench.yml`, runs the advisory native arm64 measurements on `ubuntu-24.04-arm`: job `bench` measures the A-1 spike, job `bench-product` (A-3a) builds and measures the real `fetch-mcp` release binary (idle RSS and `ready_ms`, recorded, not gated). Job `bench-product-peak` (A-3b) runs one 5 MiB fetch on the bench-loopback build and records VmHWM (BENCHMARK section 14). All three are advisory: do NOT add any of them to the required checks.

## Read this first: what has and has not been checked

| Item | Status |
|---|---|
| Rule on `main` (required checks) | NOT YET CONFIGURED. The first CI run has now happened, so the owner can set it. |
| The workflow on hosted GitHub Actions | Has run on several pull requests (first #2, most recently #5), and all five checks (`fmt`, `clippy`, `test`, `deny`, `release-guard`) have passed each time. That is still not a trend claim, and branch protection is not configured yet (see below). |
| `cargo-deny` (the `deny` job) | Has run in those hosted runs and passed.  It is still not installed on the dev host. |
| `cargo-audit` | Never run anywhere. Not installed on the dev host. |
| Everything else | Checked only by running its commands locally, plus `actionlint` (a tool that checks workflow files for mistakes). |

Do not read "required checks" as "working checks". They have passed on hosted runners in a handful of runs, which shows they can work there, not that they are stable.

## The checks

A "required status check" is a CI job that must pass before a change can be merged. The check names are the job ids in `ci.yml`, and they must match exactly.

| Check | Command | Plain meaning |
|---|---|---|
| `fmt` | `cargo fmt --check` | Code is formatted the standard way. |
| `clippy` | `cargo clippy --locked --all-targets -- -D warnings`, then the same with `--features bench-loopback`, then the same with `--features test-support`, then the A-1 spike crate (`spikes/a1`, a separate crate) with the same flags for the default set and four feature sets (spread across three HTTP backends) | Clippy (Rust's code-advice tool) finds no warnings. `-D warnings` turns every warning into a failure. |
| `test` | `cargo test --locked` (includes the SSRF range-table, `check_url`, resolver-filter and per-hop unit tests), then with `--features bench-loopback`, then with `--features test-support`, then `python3 bench/selftest.py` | The tests pass in three feature setups, and the benchmark self-test passes. |
| `deny` | `cargo deny --locked check` (`deny.toml`) | Dependencies have no known security problems and follow our rules. |
| `release-guard` | `scripts/check-release-features.sh` and `--self-test` | The release build (a) enables no feature outside the allowlist (`cargo tree -e features`), (b) has no `FETCH_MCP_MARKER_` string in the built binary, which covers both `test-support` and `bench-loopback` (the markers are compiled in only with those features), (the Sprint 1 no-HTTP-client and no-tokio-`net` checks were removed by A-3b, 2026-09-20, when the client landed; the SSRF gate for the client is the `a3b-merge-gate` job below). The self-test builds each forbidden feature and shows the guard fails, and shows the HTTP-client and tokio-net checks fail on fake input and can see the real tree. |

"Locked" (`--locked`) means the build must use exactly the versions in `Cargo.lock`. A "feature" is an optional switch compiled into the program.

## Owner quickstart: turn on the rule

Do this now that CI has run (first on pull request #2). The check names only appear in GitHub once they have run.

1. Open the repository on GitHub. Go to Settings, then Branches.
2. Add a rule for `main`.
3. Tick "Require status checks to pass before merging".
4. Add these six checks: `fmt`, `clippy`, `test`, `deny`, `release-guard`, `a3b-merge-gate`.
5. Tick "Require branches to be up to date".
6. Tick "Require a pull request before merging".
7. Write it down in the sprint PR, using this template: "Configured DD-MM-YYYY: checks fmt, clippy, test, deny, release-guard, a3b-merge-gate; up-to-date and PR required".

Success: a pull request shows the six checks, and the merge button stays locked until they pass.

Later stories add more required checks (D-3, E-6, D-2). Add them to the rule when they land.

### Planned end state: two platform gate jobs (NOT YET EXISTING)

Per ADR-007 the release image is multi-arch and both platforms are hard-gated on their own native hosted runner. Once the release workflow exists, the required set becomes the six checks above plus two platform gate jobs:

| Check | Runner | Status |
|---|---|---|
| Platform gate for `linux/amd64` | `ubuntu-24.04` | NOT YET EXISTING. Job id not chosen; match it exactly once it exists. |
| Platform gate for `linux/arm64` | `ubuntu-24.04-arm` | NOT YET EXISTING. Job id not chosen; match it exactly once it exists. |

A-3a needed no new job: the existing `release-guard` job (id unchanged) is the required release-build check for `test-support` and `bench-loopback`. A-3b adds the job `a3b-merge-gate` (closes architect N7): it runs the named test `a3b_merge_gate`, the four-refusal tests, the dial-once test and the parser differential on the release profile, and fails if any of those tests did not run. Today the required set is `fmt`, `clippy`, `test`, `deny`, `release-guard`, `a3b-merge-gate`, matching the job ids in `ci.yml`. Do not add the two gate names until they have run once, because GitHub only offers names it has seen. A skipped required job counts as a failure for the release: the publish job needs both gates, and the release is blocked unless both pass.

### Publish job and image visibility (planned)

- Only the publish job gets `packages: write` (and `id-token` and `attestations` write if provenance is added). Every other job keeps the default `contents: read`.
- The publish job has no fork or pull request trigger. It runs only from a maintainer-controlled event on the default branch or a tag, after both platform gates pass on the same image digest. It adds tags to the tested digest and never rebuilds.
- Candidate images are pushed by digest and left untagged. Fork pull requests run the same hosted jobs with no secrets and never push an image.
- GHCR packages are private by default when a workflow first pushes them. Making the package public is a manual setting and is the distribution act that open question OQ-7 (licence and distribution) must come before. Until then anonymous `docker pull` fails. This document does not decide OQ-7.

## What can go wrong

| Symptom | What it means | Fix |
|---|---|---|
| The six check names are not offered in the rule picker | The workflow has not run yet, so GitHub does not know the names. | Let CI run once on a pull request, then add the rule. |
| `fmt` fails | Code is not formatted. | Run `cargo fmt`, commit the result. |
| `clippy` fails | A warning was found. | Run the failing command from the table locally and fix what it prints. |
| `test` fails | A Rust test or the benchmark self-test failed. | Run the four commands in the table locally, in order. |
| `deny` fails | A dependency problem. This job has passed only a few times so far, so a failure may also be a set-up problem. | Read the job log. Do not assume the code is at fault. |
| `release-guard` fails | The message after `guard FAIL:` says which check: a feature outside the allowlist or a `FETCH_MCP_MARKER_` marker in the binary. | See "Release-feature guard" below. Do not edit the allowlist to make it pass. |
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

The Sprint 1 steps that banned any HTTP client crate, raw socket crate and the tokio `net` feature were removed by A-3b (2026-09-20) as planned: the real client (reqwest, rustls with ring, tokio `net`) is now a legitimate dependency. What replaces them is the `a3b-merge-gate` job (see above), which checks that the client dials only SSRF-validated addresses. Most of it is behavioural (real sockets, connection counters), but part is a heuristic source scan (no socket types outside the client, `lookup_host` only in `fetch/dns.rs`, exactly one `dns_resolver` call): a text check that a determined change can evade, not a proof. TLS/certificate validation has no automated test at all; it was checked by hand only. `cargo deny` still bans openssl, native-tls and aws-lc.

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

## Licence (OQ-7 still open)

An Apache-2.0 `LICENSE` file has existed since the initial commit. **The project's licence and distribution are still open under OQ-7, with the project owner. Nothing here decides them.** `Cargo.toml` has `publish = false` until they are decided.

What changed with A-4 (facts, from the Sprint 4 architect review; `deny.toml` `[licenses]` covers dependencies):

- `deny.toml` is **not** independent of OQ-7 any more. Adding the HTML tokenizer `lol_html` (BSD-3-Clause) brought in four crates licensed **MPL-2.0**, allowed by four per-crate exceptions in `deny.toml`: `cssparser` 0.36.0, `cssparser-macros` 0.6.1 (a proc-macro, compile time only, not linked into the binary), `dtoa-short` 0.3.5 and `selectors` 0.37.0. Nothing else in `Cargo.lock` is MPL-2.0 (`cargo deny check` passes with exactly these four), so the exception list is minimal for the current tree. The exceptions name crates without versions, so a future major of one of them would be accepted silently.
- MPL-2.0 is file-level weak copyleft. (a) Using the crates unmodified from crates.io does not extend MPL-2.0 to this project's own files; an Apache-2.0 licence, or whatever OQ-7 chooses, stays possible (MPL-2.0 section 3.3, "Larger Work"). (b) Modifying any of their files, for example by patching or vendoring a fork, obliges publishing those modifications under MPL-2.0. (c) Distributing the executable form (the statically linked binary, and therefore the GHCR image) requires telling recipients how to obtain the source of the covered crates (section 3.2: exact crate names and versions, for example a pointer to crates.io plus `Cargo.lock`) and preserving their copyright and licence notices (section 3.4). There is no obligation to release this project's source because of them. (d) The MPL-2.0 patent grant and termination terms apply to the contributors of those files.
- Practical consequence: the container image (D-2) and the dependency audit (D-6) need a third-party notices file with the licence texts (the same file also has to cover `webpki-roots`, CDLA-Permissive-2.0, and the BSD-3-Clause part of `encoding_rs`).
- Ways to avoid MPL-2.0 entirely, if the owner wants that: ADR-002 option C (the permissively licensed `html5gum` tokenizer with more own code, not measured, kept as the fallback behind the `Converter` trait), a hand-written tokenizer, or `lol_html` 3.x if its dependency tree drops the selector crates (not checked).

The owner decides OQ-7 (licence and distribution); until then the README and this section only state the facts above.
