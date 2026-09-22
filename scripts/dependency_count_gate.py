#!/usr/bin/env python3
"""D-6 / NFR-05: fail if Cargo.toml's [dependencies] table lists more than --max direct dependencies.

Deliberately dependency-free (stdlib only, no `toml` crate/package) so it can run in a bare `ubuntu-latest`
Python before any Rust or pip install step. Parses only the [dependencies] table (not [dev-dependencies] or
[build-dependencies], which NFR-05 does not count): every top-level `key = ...` or `key.<subkey> = ...` /
`[dependencies.key]` line inside that table is one direct dependency, counted once even if it spans multiple
lines (an inline table `{ ... }` or a `[dependencies.key]` sub-table).

Usage: dependency_count_gate.py Cargo.toml --max 15
"""
import argparse
import re
import sys

SECTION_RE = re.compile(r"^\s*\[([^\]]+)\]\s*(#.*)?$")
KEY_RE = re.compile(r'^\s*("?[A-Za-z0-9_.-]+"?)\s*=')


def count_dependencies(text):
    names = set()
    section = None
    for raw in text.splitlines():
        m = SECTION_RE.match(raw)
        if m:
            section = m.group(1).strip()
            if section.startswith("dependencies."):
                # [dependencies.foo] sub-table form: one dependency, named by the sub-table.
                names.add(section[len("dependencies."):])
            continue
        if section == "dependencies":
            km = KEY_RE.match(raw)
            if km:
                names.add(km.group(1).strip('"'))
    return sorted(names)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cargo_toml")
    ap.add_argument("--max", type=int, default=15)
    a = ap.parse_args(argv)

    with open(a.cargo_toml, encoding="utf-8") as f:
        text = f.read()

    deps = count_dependencies(text)
    print(f"[dependencies] in {a.cargo_toml}: {len(deps)} direct ({', '.join(deps)})")
    if len(deps) > a.max:
        print(f"::error::direct dependency count {len(deps)} exceeds NFR-05's limit of {a.max}", file=sys.stderr)
        return 1
    print(f"within NFR-05 (<= {a.max})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
