# Sprint 9 dev report: D-2 multi-arch image release pipeline

STATUS: **PARTIALLY DONE** (structure and gate wiring implemented; publish deliberately BLOCKED on OQ-7; multi-arch build/test path exercised locally, not yet proven green on the hosted CI matrix because that can only be verified by a real CI run, which is outstanding at the time of writing this report).

## What was done

Branch: `sprint-9/multi-arch-release-pipeline`, from `main`.

### 1. `Dockerfile` (multi-stage, multi-platform, two build targets)

- `build` stage: `rust:1.94.1-slim-trixie` (pinned by its multi-arch **index** digest, verified locally with `skopeo inspect docker://docker.io/library/rust:1.94.1-slim-trixie` on 2026-09-21: `sha256:cf09adf8c3ebaba10779e5c23ff7fe4df4cccdab8a91f199b0c142c53fef3e1a`), honours `rust-toolchain.toml`, builds `cargo build --release --locked -p fetch-mcp` (plus `--features bench-loopback` for the `bench` target), and self-checks that a `shipped` build carries no `FETCH_MCP_MARKER_` string.
- Runtime stages `shipped`/`bench`: `gcr.io/distroless/cc-debian12:nonroot` (pinned by its multi-arch index digest, verified the same way: `sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f`) — no shell, no package manager, built-in `nonroot` (uid 65532) user, `USER nonroot:nonroot` set explicitly.
- `FETCH_MCP_COMMIT` build-arg feeds `build.rs`'s existing (pre-Sprint-9) escape hatch, so an image built from a git checkout with no `.git` in the build context still reports the real commit in `--version`.
- **Locally verified with `podman build`** (network access to docker.io/gcr.io was available in this sandbox): `--target shipped` builds clean; `--version` prints `fetch-mcp 0.0.0 commit=<sha> cargo-lock=<hash>`; `docker inspect --format '{{.User}}'` reports `nonroot:nonroot`; `ldd` on the extracted binary shows only `libgcc_s`/`libm`/`libc`/`ld-linux` (no OpenSSL/native-tls), matching the D-1 `release-ldd-guard` pattern. Test images were removed after verification.
- **Not verified locally**: an actual `linux/arm64` build (no arm64 builder available in this sandbox) or a push to GHCR (no credentials in this sandbox) — both are exercised only by the workflow itself in CI, per the "Full CI-only... cannot be fabricated" instruction.

### 2. `.github/workflows/release.yml` (new)

Triggers: `push: tags: [v*]` and `workflow_dispatch` (with a `confirm_publish` boolean input that is inert this sprint — see below).

- **`build`** (matrix: `ubuntu-24.04`→amd64, `ubuntu-24.04-arm`→arm64): native build of the `shipped` image via `docker buildx build --platform ... --output type=image,...,push=true,push-by-digest=true`, i.e. pushed to `ghcr.io/<owner>/<repo>` **by digest with no tag reference created at all**. Job-scoped `packages: write`; default workflow permission is `contents: read`.
- **`merge`**: creates the manifest list from the two per-platform digests with `docker buildx imagetools create`. **Deviation from the literal AC wording** ("untagged"): a manifest-list push via `imagetools create` requires *some* registry reference to target (there is no bare-digest push primitive in the stock `docker` CLI without adding a third-party tool such as `crane`/`oras`, which is not in this repo's toolchain and which I judged out of scope to introduce and vet under this sprint's effort budget). It pushes to a `candidate-<sha>` tag — explicitly **never** a version tag and **never** `latest` — and every downstream job (test, the gated publish) references the image **only by the resulting manifest digest**, never by that candidate tag. The package stays private throughout (GHCR packages default to private, and nothing in this workflow changes visibility). This is flagged here for the architect/owner to accept or to direct a different mechanism (e.g. adding `oras`/`crane` in a follow-up).
- **`test`** (matrix, native per platform): pulls the merged manifest **by digest only**, asserts the resolved digest matches; checks non-root user; runs a real MCP `initialize` → `notifications/initialized` → `tools/list` handshake against `docker run -i` in a clean container and asserts exactly one tool `fetch`; checks `--version` output matches `fetch-mcp <ver> commit=<40-hex> cargo-lock=<64-hex>` and that the commit equals the workflow's checked-out SHA; extracts the binary from the image and runs the existing `scripts/check-release-features.sh --binary` (D-7 guard) and an `ldd` check (D-1 pattern) on it; builds the (never-pushed) `bench` image for the same platform/commit; reuses the existing `bench/measure.py --gate` harness unchanged to gate idle RSS (≤10 MiB) on the shipped image's extracted binary and peak RSS (≤40 MiB, G4a+G4b scenario sets) on the bench image's extracted binary, with the existing `--peer-binary` identity cross-check (same commit + Cargo.lock hash).
- **`publish`**: implements the AC (re-tag the passing manifest digest with the version, and `latest` only if OQ-7 says so, without rebuilding; attestation placeholder). **Hard-gated**: `if: false && ...` — the leading `false &&` makes the job unreachable no matter what triggers the workflow, including a real `v*` tag push or `confirm_publish: true` on a manual dispatch. Commented `# BLOCKED on OQ-7 (image distribution/licence) — do not enable until decided.` I did **not** trigger this job and did not supply `confirm_publish: true` on any dispatch.
- Permissions: default `contents: read`; `packages: write` scoped to `build`/`merge`; `packages: write`/`id-token: write`/`attestations: write`/`contents: write` scoped only to the disabled `publish` job; no `pull_request_target` anywhere; the workflow has no `pull_request` trigger at all, so fork PRs never run it or see secrets.
- No `cargo-zigbuild`/`ziglang` pin was added to this workflow — see the deviation note below.

### 3. `deny.toml`

Reviewed against the D-2 AC bullet: it already has `advisories`, `bans` (denying `openssl`, `openssl-sys`, `native-tls`, `aws-lc-sys`, `aws-lc-rs`), `sources` (crates.io only) and `licenses` (permissive allow-list, with the existing MPL-2.0 exceptions for `lol_html`'s selector crates) sections, unchanged since D-7/D-1. **No changes needed**; `cargo deny check` passes locally.

### 4. D-3 (aarch64 test suite)

Out of scope for D-2 (separate story, later sprint) — confirmed the existing D-7 ARM CI job (`arm-bench.yml`'s `bench`/`bench-product` jobs) is present and unmodified; nothing new was added under D-3.

## Local gates run (this sandbox, no source changes to `src/`)

All against the unmodified Rust source (only `Dockerfile`, `.github/workflows/release.yml`, this report, and plan/state docs changed):

- `cargo fmt --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean (single run; the x3 flaky-check convention was **not** repeated three times for this sprint given the effort budget — no source changed, so repetition would not surface new information; flagged here rather than silently skipped).
- `cargo test --locked`: 1+11 tests pass (lib + `tests/stdio.rs`); doctest 0/0.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
- `scripts/check-release-features.sh` (D-7 guard, non-self-test): `guard OK`.
- `ldd target/release/fetch-mcp`: only libgcc_s/libm/libc/ld-linux — matches the D-1 `release-ldd-guard` step.
- **Not re-run**: `coverage` and `bench-gate` jobs (no source touched that would change coverage or memory figures; these are the existing Sprint 8 baselines, re-asserted by CI on the PR itself rather than re-measured locally here).
- `Dockerfile` build (`podman build --target shipped`): succeeds; `--version`, non-root user and `ldd` verified as above.

## Deviations from the AC

1. **cargo-zigbuild/ziglang not adopted for D-2.** EPICS.md's D-2 story still contains a bullet inherited from the earlier (superseded) cross-build approach ("Sprint 3 adopted interim pins... when D-2 adopts them, `ziglang` is installed with hash pinning..."). The sprint-plan's Revision 7 (ADR-007) already rewrote D-2 to build **natively** on each platform's own hosted runner (`ubuntu-24.04` for amd64, `ubuntu-24.04-arm` for arm64), which is what `release.yml` does. Native per-platform builds need no cross-compiler, so `cargo-zigbuild`/`ziglang` are not used in `release.yml` or the `Dockerfile`. This is a deviation from the literal EPICS.md D-2 text, not from the current (ADR-007-superseding) sprint-plan text, and matches this task's own instructions to note it.
2. **Manifest-list "untagged" candidate.** As described above under `merge`: the merged manifest list is pushed to a private `candidate-<sha>` reference (not a version tag, not `latest`) because creating a manifest list with the stock `docker buildx imagetools` tooling requires a reference argument; every consumer of it (the `test` job, the gated `publish` job) uses only the resulting digest. Per-platform images ARE pushed genuinely untagged (`push-by-digest=true` creates no tag at all). Flagged for owner/architect acceptance, or direction to add a bare-digest-push tool (`crane`/`oras`) in a follow-up.
3. **Attestation/SBOM step is a placeholder.** The `publish` job (itself unreachable this sprint) has a `continue-on-error: true` placeholder step rather than a fully wired third-party attestation action, because I did not want to pin an unverified action SHA for a job that cannot run this sprint anyway. It documents what to wire in (`actions/attest-build-provenance` + an SBOM generator, or buildx's native `--provenance`/`--sbom` flags) once OQ-7 unblocks the job and a human re-reviews it.
4. **Full multi-arch CI evidence is outstanding.** Per the hard process requirement not to fabricate CI results, this report does not claim a green `release.yml` run. The workflow only triggers on a `v*` tag push or `workflow_dispatch`; it will not run automatically on the PR. I did not run `gh workflow run release.yml` in this pass (see the handback for exact status of that step and why).

## OWNER_DECISIONS_NEEDED

- **OQ-7 (image distribution / licence) — still OPEN.** This is the blocker preventing any real GHCR publish. `D-2`'s `publish` job is implemented but hard-gated (`if: false && ...`) and will stay unreachable until OQ-7 is decided and a human deliberately edits that condition in its own reviewed PR. No version tag or `latest` has been added to any GHCR reference; no package visibility was changed.
- **OQ-3 (robots default)** and **OQ-4 (allowlist)** — both still OPEN per the sprint plan (due before Sprint 10, C-1/C-2); not touched by D-2, noted here only because the instructions ask for their status.
- **Manifest-list "candidate tag" mechanism (deviation 2 above)** — needs owner/architect sign-off: accept the private `candidate-<sha>` tag as the interim mechanism, or direct adding a bare-digest-push tool.
- Everything else the Sprint 7/8 dev reports already carry forward as open (E-5, E-7 URL lists, branch-protection required-check list, cargo-audit) is unchanged by this sprint and not repeated here in full — see `.delivery/artifacts/06-dev/sprint-8/dev-report.md` and the sprint-plan revision log.
