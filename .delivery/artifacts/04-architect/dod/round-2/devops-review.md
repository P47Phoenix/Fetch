# DevOps/Release DoD Review, Stage 4 Architect, Round 2

STATUS: DONE (0 blocking, 7 non-blocking)

Round-1 blockers B1 (pinning) and B2 (runner trust/topology) are resolved.

## Verified
- Pins in architecture 9.1 match spikes/a1/Cargo.lock exactly (rmcp 3.4.0, reqwest 0.13.5, lol_html 2.9.0, rustls 0.23.45, ring 0.17.14, tokio 1.53.1, encoding_rs 0.8.41, webpki-roots 1.0.9, url 2.5.8). Toolchain 1.94.1, zigbuild 0.23.4 and ziglang 0.16.0 match the A-1 report.
- Cross-build gnu.2.17 + musl via zigbuild is spike-proven. ring avoids aws-lc. QEMU caveat and objdump GLIBC<=2.17 check are correct.
- Fork PRs never reach the self-hosted runner. Skipped-is-failure (required checks, release `needs:`), offline-runner = release blocked, and manual fallback are defined.
- Supply chain: deny.toml (advisories, bans openssl/native-tls/aws-lc, sources, licenses), nightly audit, feature-guard on release build.
- Stderr-only logging on stdio (FR-13): obs.rs, clippy print deny, panic hook. Distribution (OQ-7) impacts listed and not decided.

## Non-blocking
1. Same-repo label-approved smoke run executes PR code on the self-hosted runner. Require the maintainer to review the diff before labelling. Also set the repo setting "require approval for all outside collaborators" (not stated).
2. Ephemeral/destroyed-per-job runner is asserted but its mechanism (e.g. ephemeral registration plus a wrapper or VM on one host) is unspecified. Settle in D-2/E-6.
3. `cargo-deny` and `cargo-audit` versions are not pinned (unlike zigbuild). Pin with `--locked --version`.
4. `=` pins on rustls/ring/webpki-roots may conflict with reqwest's own requirements. Verify with `cargo update --locked` in A-2, and note that Dependabot bumps must move them together.
5. Artifacts built on hosted runners are downloaded to the benchmark runner. Have the bench preflight verify the binary sha256 against the build job's recorded value (build-provenance attestation would give this for free).
6. flate2 pin is deferred to A-3, so the dependency count of 14 is provisional until then. Acceptable, must be pinned when added.
7. Release checklist should include the musl DNS/.local caveat and webpki-roots refresh, as the doc already implies for D-4.
