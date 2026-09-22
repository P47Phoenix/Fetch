#!/usr/bin/env python3
"""B-5: line-coverage gate for the SSRF, redirect and pagination modules.

Parses an lcov.info file (from `cargo llvm-cov --lcov`) and fails (exit 1) unless the
combined line coverage across the given source files is at least --min percent. Also
prints a per-file breakdown so a regression is easy to locate.

Usage: coverage_gate.py lcov.info <file> [<file> ...] --min 90
"""
from __future__ import annotations

import sys


def parse_lcov(path: str) -> dict[str, tuple[int, int]]:
    """Return {source_file: (lines_hit, lines_found)}."""
    result: dict[str, tuple[int, int]] = {}
    current: str | None = None
    hit = found = 0
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line.startswith("SF:"):
                current = line[3:]
            elif line.startswith("DA:"):
                # DA:<line>,<hit count>[,...]
                _, rest = line.split(":", 1)
                _, count = rest.split(",", 1)
                found += 1
                if int(count.split(",", 1)[0]) > 0:
                    hit += 1
            elif line == "end_of_record":
                if current is not None:
                    result[current] = (hit, found)
                current = None
                hit = found = 0
    return result


def main(argv: list[str]) -> int:
    if not argv or argv[0].startswith("--"):
        print("usage: coverage_gate.py <lcov.info> <file...> --min <pct>", file=sys.stderr)
        return 2
    lcov_path = argv[0]
    rest = argv[1:]
    min_pct = 90.0
    files: list[str] = []
    i = 0
    while i < len(rest):
        if rest[i] == "--min":
            min_pct = float(rest[i + 1])
            i += 2
        else:
            files.append(rest[i])
            i += 1

    coverage = parse_lcov(lcov_path)

    total_hit = total_found = 0
    ok = True
    print(f"{'file':<40} {'hit':>6} {'found':>6} {'pct':>7}")
    for f in files:
        match = None
        for src, (hit, found) in coverage.items():
            if src == f or src.endswith("/" + f):
                match = (hit, found)
                break
        if match is None:
            print(f"{f:<40} {'no coverage data (file not exercised or path mismatch)':>}")
            ok = False
            continue
        hit, found = match
        pct = (100.0 * hit / found) if found else 100.0
        total_hit += hit
        total_found += found
        print(f"{f:<40} {hit:>6} {found:>6} {pct:>6.1f}%")

    # AC (B-5): "line coverage of SSRF, redirect and pagination modules is at least 90%" is a combined
    # figure across those modules, not a per-file floor; a per-file number below it is reported, not failing.
    overall = (100.0 * total_hit / total_found) if total_found else 100.0
    print(f"\noverall ({len(files)} files): {total_hit}/{total_found} = {overall:.1f}% (min {min_pct}%)")
    if overall < min_pct:
        ok = False

    if not ok:
        print("\nCOVERAGE GATE FAILED", file=sys.stderr)
        return 1
    print("\nCOVERAGE GATE PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
