#!/usr/bin/env python3
"""Deterministic fixture generator, manifest writer and verifier (Python 3 stdlib only).

Unit: 1 MiB = 2**20 bytes everywhere (docs/BENCHMARK.md section 1).
E-1 skeleton fixtures: html_5mib, html_5mib_gz, html_50mib. E-2 adds late-landmark and others.

  fixtures.py generate [--dir DIR]           write fixtures into DIR (default bench/fixtures)
  fixtures.py write-manifest [--dir DIR]     regenerate manifest.json from DIR (review the diff!)
  fixtures.py verify [--dir DIR]             exit 2 on any missing file, size or sha256 mismatch
"""
import argparse, gzip, hashlib, json, os, random, sys

MIB = 1024 * 1024
HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "manifest.json")
SEED = 1
WORDS = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua".split()
HEAD = ("<!doctype html><html><head><meta charset=utf-8><title>Fixture</title><style>body{font:14px sans-serif}</style>"
        "<script>var x=1;</script></head><body><nav><a href='/'>Home</a></nav><main>")
FOOT = "</main></body></html>"
# Sizes: the 5 MiB fixture is 1 KiB under the 5 MiB cap so a correct server does not answer too_large.
SIZES = {"html_5mib": 5 * MIB - 1024, "html_50mib": 50 * MIB}


def html_chunks(total, seed=SEED):
    """Yield ASCII HTML as bytes, exactly `total` bytes, byte-identical per (total, seed)."""
    rng = random.Random(seed)
    words = lambda n: " ".join(rng.choice(WORDS) for _ in range(n))
    written, i = len(HEAD), 0
    yield HEAD.encode()
    while True:
        s = (f"<section class='c{i}'><h2>Heading {i}</h2><p>{words(60)} <a href='https://example.com/{i}'>link {i}</a> "
             f"<em>{words(5)}</em> <strong>{words(4)}</strong></p><ul><li>{words(8)}</li><li>{words(8)}</li>"
             f"<li><code>{words(3)}</code></li></ul><div><div><span>{words(12)}</span></div></div></section>\n")
        if written + len(s) + len(FOOT) + 7 > total:
            break
        yield s.encode(); written += len(s); i += 1
    pad = total - written - len(FOOT)  # >= 7 by the loop condition
    yield ("<!--" + "x" * (pad - 7) + "-->").encode()
    yield FOOT.encode()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def generate(d):
    os.makedirs(d, exist_ok=True)
    for name, size in SIZES.items():
        with open(os.path.join(d, name + ".html"), "wb") as f:
            for c in html_chunks(size):
                f.write(c)
    src = os.path.join(d, "html_5mib.html")
    with open(src, "rb") as fi, open(os.path.join(d, "html_5mib.html.gz"), "wb") as fo:
        # mtime=0 and fixed level: bytes depend on the zlib build; re-commit the manifest if zlib changes.
        with gzip.GzipFile(filename="", mode="wb", fileobj=fo, compresslevel=6, mtime=0) as g:
            g.write(fi.read())


FILES = {"html_5mib": "html_5mib.html", "html_5mib_gz": "html_5mib.html.gz", "html_50mib": "html_50mib.html"}


def build_manifest(d):
    return {"version": 1, "unit": "MiB=2^20", "seed": SEED, "fixtures": {
        n: {"file": f, "size": os.path.getsize(os.path.join(d, f)), "sha256": sha256_file(os.path.join(d, f))}
        for n, f in FILES.items()}}


def load_manifest():
    with open(MANIFEST) as f:
        return json.load(f)


def verify(d, manifest=None):
    """Return list of problems (empty = OK)."""
    manifest = manifest or load_manifest()
    bad = []
    for n, m in manifest["fixtures"].items():
        p = os.path.join(d, m["file"])
        if not os.path.exists(p):
            bad.append(f"{n}: missing {p}"); continue
        if os.path.getsize(p) != m["size"]:
            bad.append(f"{n}: size {os.path.getsize(p)} != {m['size']}"); continue
        if sha256_file(p) != m["sha256"]:
            bad.append(f"{n}: sha256 mismatch")
    return bad


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["generate", "write-manifest", "verify"])
    ap.add_argument("--dir", default=os.path.join(HERE, "fixtures"))
    a = ap.parse_args(argv)
    if a.cmd == "generate":
        generate(a.dir); print("generated in", a.dir)
    elif a.cmd == "write-manifest":
        with open(MANIFEST, "w") as f:
            json.dump(build_manifest(a.dir), f, indent=2); f.write("\n")
        print("wrote", MANIFEST)
    else:
        bad = verify(a.dir)
        for b in bad: print("FIXTURE MISMATCH:", b, file=sys.stderr)
        print("fixtures OK" if not bad else "fixtures REFUSED")
        return 2 if bad else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
