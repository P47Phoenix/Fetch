# Architect + CI security review, PR #7 (head 39379c1 vs main 35a3450)

Decision: DONE. Blocking: 0.

Verified from a clean `git archive HEAD`: cargo test --locked passes (92 unit, 9 integration), bench/selftest.py passes. cargo-deny is not installed here, but Cargo.toml, Cargo.lock and deny.toml are byte-unchanged in the diff, so the dependency and licence posture is unchanged.

## CI workflow (bench.yml)
- Both actions are SHA-pinned (checkout v4.2.2, upload-artifact v4.6.2). `permissions: contents: read` only. No secrets. Trigger is `pull_request`, not `pull_request_target`. Fork PRs are skipped by the same-repo `if`. `persist-credentials: false`.
- Expression injection: the only `${{ }}` values in `run:` come from the matrix (constants) and step outcomes. No PR-controlled strings (title, branch) are interpolated.
- Artifact contents are `out/` only: platform facts, JSONL, exit codes. No tokens. The platform.txt content is uname/cpuinfo/meminfo, which is non-sensitive.
- ci.yml is untouched. The required-check set is unchanged: bench-gate is explicitly NOT required, and docs/ci-branch-protection.md says so and lists the 4 cell names for the owner to add later. The D-7 release guard (check-release-features.sh) still runs on the shipped candidate only.

## Interim cross-build
- The flow builds natively on each arch, with zig only as the linker. That gives the glibc 2.17 floor for gnu and a musl link on a runner without a musl toolchain. Versions are pinned (cargo-zigbuild 0.23.4 with `--locked`, ziglang==0.16.0), and the script re-checks the ziglang version and fails with exit 2 on a mismatch. The release profile is unchanged, and `--locked` is used for the product build.
- Residual supply-chain gap (NB1): pip has no hash pin (`--require-hashes`) for ziglang, and cargo-zigbuild is built from crates.io (checksum-verified by the registry, but its transitive dependencies are taken from its own lockfile). Both are build-time tools on an ephemeral runner with a read-only token and no secrets, so the impact is limited to build integrity of an interim candidate. Recommend a hash pin when D-2 adopts these pins.

## A-9 source change
- `final_url` is `parsed.as_str()`, the `url::Url` re-serialization. The url crate percent-encodes control characters, CR/LF and spaces, so a crafted redirect Location cannot inject extra header lines or forge the "Status:" line. Status is a u16 that has already been checked as 2xx.
- No panics: there is no indexing or unwrap, only `format!`. It is safe under panic=abort. The header is added to the text content only. There is no stdout logging, so stdout purity is preserved. No new network, DNS or SSRF surface: the value is already-validated data that is echoed back, and every hop is still resolved and checked once (the test asserts the lookup count).
- The header sits outside the max_length window by design (FR-14). It is bounded by URL length, which is limited by the request line the server accepts, and it is only emitted after a redirect (NB2).
- Tests cover the no-redirect and redirect cases, plus a stdio test. `Fetched` lost `Copy` (String field); no callers were affected.

## Targets and spoofing
- No target was loosened: the idle 10 MiB (strict) and peak 40 MiB gates are unchanged, and the harness still refuses overrides in `--gate` mode. `--gate` widened from aarch64-only to aarch64 or x86_64 with a per-architecture QEMU binfmt check. That change is legitimate, per ADR-007 native hosted runners.
- Spoofing: `--binary-kind shipped` refuses a binary whose `--version` contains bench-loopback, test-support or FETCH_MCP_MARKER_. `--binary-kind bench` requires the marker. The ELF machine type is checked. `idle` refuses a bench binary, so a bench build cannot be reported as shipped idle. The peak on the bench build is labelled as read-in-full and "NOT a G4a pass" in the workflow and docs, which is honest. The `idle-bench` scenario has gate "none".
- BENCHMARK.md section 15 reports results without claiming G4a.

## Non-blocking
1. NB1: no hash pin for ziglang (pip) or a lockfile check for the cargo-zigbuild install. Pin the hashes when D-2 takes over.
2. NB2: the redirect header echoes the final URL including any userinfo or query, and adds bytes beyond max_length. Consider stripping userinfo (redirect-to-credential URLs are server-controlled, so low risk).
3. NB3: BENCHMARK.md section 7 still says "Today only the gnu product binary is measured... musl not built for the product", which is stale after this PR. Update it.
4. NB4: bench.yml runs on a nightly schedule and on pushes to main. The runs take about 16 minutes on 4 cells and are non-required. This is a cost note only.
5. NB5: `cargo deny` was not run locally (tool absent). Confirm the CI deny job is green on the PR.
