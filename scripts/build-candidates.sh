#!/usr/bin/env bash
# Interim candidate build (E-2; D-2 adopts the same pins). Builds the SHIPPED and BENCH (bench-loopback) release binaries
# for the NATIVE architecture of this host, gnu (glibc 2.17 floor) and musl, from the current commit with the pinned
# release profile, using cargo-zigbuild + ziglang at the versions pinned below (architecture 9.1).
# Usage: scripts/build-candidates.sh <out-dir>     -> <out-dir>/fetch-mcp-<arch>-<libc>-<shipped|bench>
# Needs: cargo-zigbuild $ZIGBUILD_VERSION and ziglang $ZIG_VERSION on PATH (CI installs them; see .github/workflows/bench.yml).
# Never enables any feature except bench-loopback for the bench binary; the D-7 guard still checks the shipped ones.
set -euo pipefail

ZIGBUILD_VERSION=0.23.4
ZIG_VERSION=0.16.0

out=${1:?usage: build-candidates.sh <out-dir>}
case "$(uname -m)" in
  x86_64) arch=x86_64 ;;
  aarch64) arch=aarch64 ;;
  *) echo "build-candidates: unsupported host $(uname -m)" >&2; exit 2 ;;
esac

zbv=$(cargo-zigbuild --version 2>/dev/null || true)
[[ $zbv == "cargo-zigbuild $ZIGBUILD_VERSION" ]] || { echo "build-candidates: cargo-zigbuild is '$zbv', need $ZIGBUILD_VERSION (cargo install --locked cargo-zigbuild --version $ZIGBUILD_VERSION)" >&2; exit 2; }
zv=$(python3 -m ziglang version 2>/dev/null || true)
[[ $zv == "$ZIG_VERSION" ]] || { echo "build-candidates: ziglang is '$zv', need $ZIG_VERSION (pip install ziglang==$ZIG_VERSION)" >&2; exit 2; }

# Build identity (E-4, re-homed from E-8): every candidate's `--version` must carry the commit and Cargo.lock hash it was built
# from (build.rs), and the shipped and bench builds must agree with each other, with `git rev-parse HEAD` and with
# `sha256sum Cargo.lock`. The binaries are native, so they can be run here. A mismatch fails the build (exit 1).
ident() { "$1" --version | sed -n 's/.* \(commit=[0-9a-f]\{40\} cargo-lock=[0-9a-f]\{64\}\)\( .*\)\{0,1\}$/\1/p'; }
want_commit=$(git rev-parse --verify HEAD)
want_lock=$(sha256sum Cargo.lock | cut -d' ' -f1)
want_id="commit=$want_commit cargo-lock=$want_lock"

mkdir -p "$out"
for libc in gnu musl; do
  target="$arch-unknown-linux-$libc"
  zt=$target; [[ $libc == gnu ]] && zt="$target.2.17"
  rustup target add "$target"
  for kind in shipped bench; do
    feat=(); [[ $kind == bench ]] && feat=(--features bench-loopback)
    cargo zigbuild --release --locked --target "$zt" "${feat[@]}"
    cp "target/$target/release/fetch-mcp" "$out/fetch-mcp-$arch-$libc-$kind"
    echo "built $out/fetch-mcp-$arch-$libc-$kind $(stat -c %s "$out/fetch-mcp-$arch-$libc-$kind") bytes"
  done
  sid=$(ident "$out/fetch-mcp-$arch-$libc-shipped"); bid=$(ident "$out/fetch-mcp-$arch-$libc-bench")
  if [[ $sid != "$want_id" || $bid != "$want_id" ]]; then
    echo "build-candidates: build identity mismatch for $arch-$libc" >&2
    echo "  expected: $want_id" >&2; echo "  shipped : ${sid:-<none>}" >&2; echo "  bench   : ${bid:-<none>}" >&2
    echo "  (a '<sha>-dirty' commit in --version means tracked files differ from HEAD: build from a clean checkout)" >&2
    exit 1
  fi
  echo "identity ok for $arch-$libc: $want_id"
done
