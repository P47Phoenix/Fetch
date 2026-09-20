#!/usr/bin/env bash
# Release-feature guard (D-7, architecture R12, ADR-003).
# Fails a release build that enables ANY feature outside the allowlist, or whose binary contains ANY
# FETCH_MCP_MARKER_ token (markers exist only for test-support and bench-loopback, so a renamed or
# newly added forbidden feature is still caught):
#   1. `cargo tree -e features`: the root package may enable only ALLOWED_FEATURES;
#   2. the built binary contains no FETCH_MCP_MARKER_ string.
# Usage: check-release-features.sh [--features <list>]   guard a release build
#        check-release-features.sh --self-test          positive controls (must fail)
#        check-release-features.sh --binary <path>      marker grep on an existing artifact
#        check-release-features.sh --no-http-client     Sprint 1 gate (A-3a): no HTTP client crate in the tree
#   The plain run (no args) does the feature/marker guard AND the no-HTTP-client check. A-3b deliberately removes
#   the no-HTTP-client check (delete HTTP_CLIENT_BAN and its calls) when the client lands.
set -euo pipefail

PKG=fetch-mcp
# Features a release build may enable. Add here only by deliberate review; test-support and
# bench-loopback must never be listed. Used only for the tree check.
ALLOWED_FEATURES=(default)
# Forbidden features that exist today (self-test builds each one; it must fail the guard).
FORBIDDEN_FEATURES=(test-support bench-loopback)
MARKER_PREFIX=FETCH_MCP_MARKER_

# Crates that are (or pull in) an HTTP client. Sprint 1 gate: none may appear anywhere in the dependency tree
# (normal, build and dev edges, all features), so the crate is testable without network code.
HTTP_CLIENT_BAN='reqwest|hyper|hyper-util|hyper-tls|hyper-rustls|ureq|isahc|surf|attohttpc|minreq|curl|curl-sys|awc|h2|http-body|http-body-util|tower-http|tungstenite|tokio-tungstenite|reqwest-middleware|httpc|nyquest'

# <tree text, "name vX.Y.Z" per line>: fails if any banned crate name appears.
check_no_http_client_text() {
  local hits
  hits=$(grep -E "^($HTTP_CLIENT_BAN) v" <<<"$1" || true)
  if [[ -n $hits ]]; then
    echo "guard FAIL: HTTP client crate in the dependency tree (Sprint 1 gate):" >&2
    echo "$hits" >&2
    return 1
  fi
}

check_no_http_client() {
  local tree
  tree=$(cargo tree -p "$PKG" --locked -e all --all-features --prefix none --format '{p}')
  [[ $tree == "$PKG "* ]] || { echo "guard FAIL: cannot read cargo tree output" >&2; return 1; }
  check_no_http_client_text "$tree"
}

check_markers() { # <binary>
  local bin=$1
  [[ -f $bin ]] || { echo "guard: binary not found: $bin" >&2; return 2; }
  if grep -aqF "$MARKER_PREFIX" "$bin"; then
    echo "guard FAIL: marker ($MARKER_PREFIX...) found in $bin" >&2; return 1
  fi
}

# Reads `cargo tree` text (first line = root package, "name vX (path) [f1,f2]"), fails on any
# enabled root feature that is not allowlisted.
check_tree_text() { # <tree text>
  local root feats f a ok rc=0
  root=$(head -n1 <<<"$1")
  [[ $root =~ \[([^]]*)\]$ ]] || { echo "guard FAIL: cannot parse feature list from: $root" >&2; return 1; }
  feats=${BASH_REMATCH[1]}
  IFS=, read -r -a list <<<"$feats"
  for f in "${list[@]}"; do
    [[ -z $f ]] && continue
    ok=0
    for a in "${ALLOWED_FEATURES[@]}"; do [[ $f == "$a" ]] && ok=1; done
    if [[ $ok -eq 0 ]]; then echo "guard FAIL: feature $f enabled (not in allowlist)" >&2; rc=1; fi
  done
  return "$rc"
}

check_tree() { # <features csv>
  local feats=$1 tree args=(tree -p "$PKG" --locked -e features --prefix none --format '{p} [{f}]')
  [[ -n $feats ]] && args+=(--features "$feats")
  tree=$(cargo "${args[@]}")
  check_tree_text "$tree"
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
  local tmp f rc=0 out
  tmp=$(mktemp -d); trap 'rm -rf "$tmp"' RETURN
  echo "self-test: clean build must pass"
  guard_build "" "$tmp/clean" || { echo "self-test FAIL: clean build rejected" >&2; return 1; }
  for f in "${FORBIDDEN_FEATURES[@]}"; do
    echo "self-test: build with $f must FAIL the guard (tree and marker)"
    if out=$(guard_build "$f" "$tmp/$f" 2>&1); then
      echo "self-test FAIL: guard passed a build with $f (vacuous)" >&2; rc=1
    elif ! grep -q "guard FAIL: marker" <<<"$out" || ! grep -q "guard FAIL: feature $f enabled" <<<"$out"; then
      echo "self-test FAIL: $f rejected for the wrong reason (need tree and marker detection)" >&2; rc=1
    fi
  done
  echo "self-test: renamed/unknown feature must FAIL the tree check"
  if check_tree_text "fetch-mcp v0.0.0 (/x) [default,loopback2]" 2>/dev/null; then
    echo "self-test FAIL: unknown feature loopback2 passed" >&2; rc=1
  fi
  echo "self-test: allowlisted features must PASS the tree check"
  check_tree_text "fetch-mcp v0.0.0 (/x) [default]" || { echo "self-test FAIL: default rejected" >&2; rc=1; }
  check_tree_text "fetch-mcp v0.0.0 (/x) []" || { echo "self-test FAIL: empty rejected" >&2; rc=1; }
  echo "self-test: unparseable tree output must FAIL"
  if check_tree_text "garbage" 2>/dev/null; then echo "self-test FAIL: garbage passed" >&2; rc=1; fi
  echo "self-test: unknown marker in a binary must FAIL the marker check"
  printf 'x%sNEW_THING_V1y' "$MARKER_PREFIX" >"$tmp/fake.bin"
  if check_markers "$tmp/fake.bin" 2>/dev/null; then echo "self-test FAIL: unknown marker passed" >&2; rc=1; fi
  echo "self-test: an HTTP client crate in the tree must FAIL the no-HTTP-client check"
  for c in reqwest hyper ureq h2 hyper-util; do
    if check_no_http_client_text "$(printf 'fetch-mcp v0.0.0 (/x)\nserde v1.0.0\n%s v0.1.0\n' "$c")" 2>/dev/null; then
      echo "self-test FAIL: $c passed the no-HTTP-client check" >&2; rc=1
    fi
  done
  echo "self-test: a tree without HTTP client crates must PASS (similarly named crates are not banned)"
  check_no_http_client_text "$(printf 'fetch-mcp v0.0.0 (/x)\nserde v1.0.0\nhttparse v1.0.0\nhyperlink v1.0.0\n')" \
    || { echo "self-test FAIL: clean tree rejected" >&2; rc=1; }
  echo "self-test: the real tree must pass, and the check must be able to see it (non-vacuous)"
  check_no_http_client || { echo "self-test FAIL: real tree rejected" >&2; rc=1; }
  [[ $(cargo tree -p "$PKG" --locked -e all --all-features --prefix none --format '{p}' | wc -l) -gt 10 ]] \
    || { echo "self-test FAIL: cargo tree output implausibly small (check would be vacuous)" >&2; rc=1; }
  [[ $rc -eq 0 ]] && echo "self-test OK"
  return "$rc"
}

case "${1:-}" in
  --self-test) self_test ;;
  --binary) check_markers "${2:?path}" ;;
  --no-http-client) check_no_http_client && echo "no-http-client OK" ;;
  --features) guard_build "${2:?csv}" "${CARGO_TARGET_DIR:-target}" ;;
  "") guard_build "" "${CARGO_TARGET_DIR:-target}"; check_no_http_client; echo "guard OK" ;;
  *) echo "usage: $0 [--self-test | --binary PATH | --features CSV | --no-http-client]" >&2; exit 2 ;;
esac
