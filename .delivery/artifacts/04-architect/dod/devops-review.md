# DevOps DoD Review - Stage 4 (Architect)

Reviewer role: DevOps validator (delivery-team:operations). Scope: architecture.md sections 7-9, 11, 14; ADR-001, ADR-005 (others skimmed for build/ops impact). Artifact not edited.

Verdict: NOT_DONE (2 blocking, 9 non-blocking). Both blockers are small doc edits.

## Blocking

### B1. Reproducibility / pinning is asserted, not specified
- Section 9: "`rust-toolchain.toml` pins stable and MSRV". "Stable" is a moving channel, not a pin. Binary size (NFR-13), RSS (NFR-10..12) and the published "claim proven" benchmark all depend on the exact rustc and std. A benchmark report that cannot name a fixed toolchain is not reproducible (PRD NFR-14, Goal 1d).
- R5 says "pin exact version" for rmcp but no mechanism is stated. Needed: `rmcp = "=3.4.0"` (exact requirement, not caret), and the same for the other high-churn direct deps (reqwest 0.13, lol_html, rustls/ring).
- Not stated anywhere: `Cargo.lock` committed (it is a binary crate, so it must be), `--locked` on every CI/release build, exact `cargo-zigbuild` (0.23.4) and `ziglang` (0.16.0) versions pinned in CI (the spike used them, the architecture does not require them), GitHub Actions pinned by commit SHA, and deterministic-build flags (`--remap-path-prefix`, `SOURCE_DATE_EPOCH`) or an explicit statement that bit-for-bit reproducibility is not a goal (checksums then attest only "what we built").
- Required fix: add a "Pinning and reproducibility" bullet list to section 9 with the above, and change "pins stable" to an exact toolchain version (for example `channel = "1.xx.y"`) with a bump procedure that re-runs the benchmark.

### B2. CI memory gate depends on an unspecified self-hosted runner (security and availability)
- Sections 9/11 and PRD OQ-9 put the aarch64 tests and the NFR-10..12 gate on "the author's native aarch64 runner on their cluster", while also saying "GitHub Actions per config". The design never says how these connect: self-hosted runner registration, labels, ephemeral vs persistent, who may trigger it.
- If the repo is public (OQ-7 is open and leans that way), a self-hosted runner executing fork PR code on the author's home cluster, which also hosts the TrueNAS/Home Assistant services the PRD names as the protected assets, is a real risk. The gate needs: fork PRs never run on the self-hosted label (benchmark on push to main / tag / manual dispatch only), ephemeral or containerised job execution with no cluster credentials, and network isolation from the LAN.
- The memory gate is release-blocking (M2 decision gate, M4/NFR-15) but has a single point of failure. Needed: defined behaviour when the runner is offline (release blocked, not silently skipped; QEMU must never substitute per section 9), and a required-check configuration so "skipped" is not "green".
- Required fix: add a short "Runner topology and trust" subsection: trigger matrix (PR / main / tag), which jobs are hosted vs self-hosted, fork-PR policy, skipped-is-failure rule, offline behaviour.

## Non-blocking

1. Gate definition ambiguity (section 11 item 6): "CI adds a 10% regression margin (E-6)" does not say whether the margin is relative to the absolute targets (which would allow 44 MB) or to a stored baseline (better: fail if median regresses > 10% vs last main baseline AND always fail above absolute targets). Also say who stores the baseline (artifact/branch).
2. Gate duration: 30 s idle x >= 10 fresh processes is 5 minutes for idle alone, plus 7 scenarios x 10 runs. Run idle samples in parallel processes (RSS is per-process) or run the full matrix nightly and a reduced smoke gate per PR. Record in E-6.
3. Runner environment noise: if the runner is a Kubernetes pod, record kernel, page size, cgroup limits and whether the harness runs with pinned CPU. Section 11 item 5 covers most fields; add container/cgroup and "shared node load" to the report. VmHWM is per-process so noise is mostly CPU latency, not RSS; fine for the gate, matters for NFR-02 latency numbers.
4. Supply chain (section 9): "cargo audit and cargo deny" is thin. Specify `deny.toml` sections (advisories, bans incl. `openssl`/`native-tls`/`aws-lc-sys` to enforce FR-15 and the ring decision, sources = crates.io only, licences), and run audit on a schedule (nightly) as well as per-PR, since advisories arrive without code changes. The licence allow-list can be permissive-only now, independent of OQ-7; only the project's own licence waits on OQ-7. Consider `cargo vet` or at minimum Dependabot/Renovate for the pinned deps and webpki-roots (R11 staleness has no owner).
5. Release integrity: SHA256 checksums only. Consider signed checksums or GitHub build provenance attestations and an SBOM (`cargo cyclonedx`); decide with OQ-7. macOS arm64 binaries will need ad-hoc codesign at minimum (Gatekeeper on downloaded binaries); note notarization as out of scope or in scope.
6. Distribution (OQ-7): the channel-agnostic claim is right, but implications should be listed: `cargo install fetch-mcp` builds from source on the user's machine (glibc/musl choice and profile still apply, but no zig, and the RSS claim covers only released binaries); crates.io needs `Cargo.toml` metadata and licence; GitHub releases need the artifact naming scheme (target triple in name) fixed now so docs (D-4) and any install script do not churn. UA string embeds a repo URL that must be a build-time constant, not a TBD in code.
7. gnu.2.17 build: the spike showed the gnu binary cannot run under qemu-user without a loader. Section 9 permits QEMU for "functional tests", which for gnu needs `QEMU_LD_PREFIX` with an aarch64 sysroot. Either state this or say QEMU tests cover musl only. Also verify with `objdump -T` that the required GLIBC symbol versions are <= 2.17 (ldd alone will not show this); add to the FR-15 check.
8. Feature-guard CI (R12): also assert the bench-only fixture-CA feature (section 11 item 7) is absent from release builds, with the same `cargo tree -e features` check.
9. Ops surface: no `--version`/`--help` flag and no startup log line. Add `--version` (crate version, commit, target, libc) and a single `info` line at start to stderr; it also feeds the benchmark report's "binary sha, commit, libc" fields. Note that config errors exit non-zero before the MCP handshake, so the client shows a generic spawn failure; the stderr text naming the variable is the only diagnostic (document in D-4). `FETCH_LOG` value semantics for invalid input should follow the same exit-non-zero rule. `no_proxy()` is fixed, so corporate-proxy users cannot use the tool; document as a known limit.

## Checked and acceptable

- cargo-zigbuild aarch64 gnu.2.17 and musl: feasible, proven in spike (about 35 s per target, both exit 0); ring avoids the aws-lc C toolchain. macOS built on a native macOS runner (no zig) is sound.
- Native aarch64 for benchmarks; QEMU never used for memory (well argued, evidence from spike: 13.0/19.5 MB vs 4.7/6.1 MB).
- Logging: stderr only, hand-rolled logger, stdout never written by app code, clippy deny on print macros, e2e stdout-purity test (FR-13), `panic = "abort"` with stderr message. Adequate. Minor addition: install a panic hook that writes to stderr explicitly, and confirm rmcp itself does not log to stdout.
- Config surface: env-only, validated at startup, immutable, sane defaults, no per-call policy override.
- Allocator/libc decision criteria in ADR-005 are objective and testable; building both from Sprint 1 is the right way to gather ARM data early.
- Dependency budget (13 of 15) and no OpenSSL/aws-lc are consistent with NFR-05 and FR-15.
