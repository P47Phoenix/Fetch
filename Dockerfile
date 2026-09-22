# D-2: multi-stage, multi-arch image for fetch-mcp.
#
# Builds NATIVELY for the platform the build runs on (the release.yml workflow runs this once per
# platform, on ubuntu-24.04 for linux/amd64 and ubuntu-24.04-arm for linux/arm64 - no QEMU, no
# cross-compilation, so `cargo-zigbuild` is not needed here; see the D-2 deviation note in the
# Sprint 9 dev report).
#
# Two build targets share this file, selected by `--target`:
#   shipped (default): the D-7/D-1 pinned release profile, no extra features - the artifact that
#                       may eventually be published (gated on OQ-7).
#   bench             : same profile plus `--features bench-loopback` (E-8) - used only to measure
#                       peak RSS in CI; this image is never pushed anywhere.
#
# Base images are pinned by digest, not by mutable tag (architecture 9.3): rust:1-slim for the build
# stage (build tooling, not shipped) and gcr.io/distroless/cc-debian12 for the runtime stage (glibc +
# libgcc + libssl-less minimal userland, no shell, nonroot user built in). Digests are re-pinned in
# their own PR when bumped (D-2 AC).

# rust:1.94.1-slim-trixie, linux/amd64 and linux/arm64 manifest index digest (verified with
# `skopeo inspect docker://docker.io/library/rust:1.94.1-slim-trixie` on 2026-09-21).
FROM rust@sha256:cf09adf8c3ebaba10779e5c23ff7fe4df4cccdab8a91f199b0c142c53fef3e1a AS build
WORKDIR /src

# Toolchain pin matches rust-toolchain.toml; rustup honours the file once the source is copied.
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY src ./src
COPY build.rs ./build.rs

ARG BUILD_KIND=shipped
# `.dockerignore` excludes `.git` (and everything else except the files explicitly COPY'd above) from
# the build context, so build.rs's git lookup would report `commit=unknown` even though only tracked
# source files are ever actually copied into the image; release.yml passes the real commit SHA it
# checked out (`git rev-parse HEAD`) through this build-arg, which build.rs's documented
# `FETCH_MCP_COMMIT` escape hatch consumes verbatim (E-4/E-8 identity check: shipped and bench images
# built from the same commit must report the same commit and Cargo.lock hash).
ARG FETCH_MCP_COMMIT=unknown
ENV FETCH_MCP_COMMIT=${FETCH_MCP_COMMIT}
RUN set -eu; \
    rustup show active-toolchain; \
    if [ "$BUILD_KIND" = "bench" ]; then FEATURES="--features bench-loopback"; else FEATURES=""; fi; \
    cargo build --release --locked -p fetch-mcp $FEATURES; \
    install -Dm755 target/release/fetch-mcp /out/fetch-mcp; \
    # D-7/D-2 guard: the release-feature guard's marker grep must find no forbidden-feature marker
    # in a `shipped` image; a `bench` image is expected to carry the bench-loopback marker (that is
    # what the release-guard job asserts *is* present, the mirror image of the shipped check).
    if [ "$BUILD_KIND" = "shipped" ]; then \
      if grep -aqF FETCH_MCP_MARKER_ /out/fetch-mcp; then \
        echo "shipped build carries a forbidden-feature marker" >&2; exit 1; \
      fi; \
    fi

# gcr.io/distroless/cc-debian12:nonroot manifest index digest (glibc runtime, no shell/package
# manager, linux/amd64+arm64, built-in `nonroot` user uid 65532; verified with
# `skopeo inspect docker://gcr.io/distroless/cc-debian12:nonroot` on 2026-09-21).
# OQ-7 follow-up (PR #13 review NB-7): no `LABEL org.opencontainers.image.*` is set here yet.
# `org.opencontainers.image.source` in particular is what GHCR uses to link a package to its
# repository, which can make the package inherit the repository's (public) visibility - do not add
# that label until OQ-7 is decided and re-verified, or it could silently defeat the private-package
# guard in release.yml's `merge` job.
FROM gcr.io/distroless/cc-debian12@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f AS shipped
COPY --from=build /out/fetch-mcp /usr/local/bin/fetch-mcp
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/fetch-mcp"]

FROM gcr.io/distroless/cc-debian12@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f AS bench
COPY --from=build /out/fetch-mcp /usr/local/bin/fetch-mcp
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/fetch-mcp"]
