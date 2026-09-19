#!/usr/bin/env bash
# Release-feature guard (D-7, architecture R12, ADR-003).
# Asserts test-support, bench-loopback and fixture-ca are absent from a release build:
#   1. `cargo tree -e features` shows none of them enabled;
#   2. the built binary contains none of their marker strings.
# Usage: check-release-features.sh [--features <list>]   guard a release build
#        check-release-features.sh --self-test          positive controls (must fail)
#        check-release-features.sh --binary <path>      marker grep on an existing artifact
set -euo pipefail

PKG=fetch-mcp
FEATURES=(test-support bench-loopback fixture-ca)
MARKERS=(FETCH_MCP_MARKER_TEST_SUPPORT_V1 FETCH_MCP_MARKER_BENCH_LOOPBACK_V1 FETCH_MCP_MARKER_FIXTURE_CA_V1)

check_markers() { # <binary>
  local bin=$1 rc=0 m
  [[ -f $bin ]] || { echo "guard: binary not found: $bin" >&2; return 2; }
  for m in "${MARKERS[@]}"; do
    if grep -aqF "$m" "$bin"; then echo "guard FAIL: marker $m found in $bin" >&2; rc=1; fi
  done
  return "$rc"
}

check_tree() { # <features csv>
  local feats=$1 tree rc=0 f args=(tree -p "$PKG" --locked -e features --prefix none --format '{p} [{f}]')
  [[ -n $feats ]] && args+=(--features "$feats")
  tree=$(cargo "${args[@]}")
  for f in "${FEATURES[@]}"; do
    if grep -qE "\[([^]]*,)?$f(,[^]]*)?\]" <<<"$tree"; then
      echo "guard FAIL: feature $f enabled in cargo tree" >&2; rc=1
    fi
  done
  return "$rc"
}

guard_build() { # <features csv> <target dir>
  local feats=$1 tdir=$2 rc=0 bargs=(build --release --locked -p "$PKG")
  [[ -n $feats ]] && bargs+=(--features "$feats")
  check_tree "$feats" || rc=1
  CARGO_TARGET_DIR=$tdir cargo "${bargs[@]}"
  check_markers "$tdir/release/$PKG" || rc=1
  return "$rc"
}

self_test() {
  local tmp f rc=0
  tmp=$(mktemp -d); trap 'rm -rf "$tmp"' RETURN
  echo "self-test: clean build must pass"
  guard_build "" "$tmp/clean" || { echo "self-test FAIL: clean build rejected" >&2; return 1; }
  for f in "${FEATURES[@]}"; do
    echo "self-test: build with $f must FAIL the guard"
    if out=$(guard_build "$f" "$tmp/$f" 2>&1); then
      echo "self-test FAIL: guard passed a build with $f (vacuous)" >&2; rc=1
    elif ! grep -q "guard FAIL: marker" <<<"$out" || ! grep -q "guard FAIL: feature $f" <<<"$out"; then
      echo "self-test FAIL: $f rejected for the wrong reason (need tree and marker detection)" >&2; rc=1
    fi
  done
  [[ $rc -eq 0 ]] && echo "self-test OK"
  return "$rc"
}

case "${1:-}" in
  --self-test) self_test ;;
  --binary) check_markers "${2:?path}" ;;
  --features) guard_build "${2:?csv}" "${CARGO_TARGET_DIR:-target}" ;;
  "") guard_build "" "${CARGO_TARGET_DIR:-target}"; echo "guard OK" ;;
  *) echo "usage: $0 [--self-test | --binary PATH | --features CSV]" >&2; exit 2 ;;
esac
