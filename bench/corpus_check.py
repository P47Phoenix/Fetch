#!/usr/bin/env python3
"""E-7/A-4/E-5 rework (Sprint 13, OQ-owner decision): local-fixture conversion-quality harness.

Replaces the old plan (a committed 50-URL / 10-URL LIVE list from the owner, which never arrived across 12
sprints) with a harness that needs no network and no owner-supplied URL list at all: it reads whatever
`.html` files the owner drops into a local directory, serves them from a loopback HTTP server, fetches each
one through the real `fetch-mcp` binary (built with the `bench-loopback` feature so it may dial 127.0.0.1),
and checks the same A-4 conversion-quality criteria the sprint plan always specified:

  - >= 95% of fixtures convert successfully (no `error` in the MCP result)
  - median token reduction (raw whitespace-token count vs converted markdown whitespace-token count) >= 50%
  - no literal `<script` text (case-insensitive) survives in the converted output of any fixture whose
    source actually contained a `<script` tag

It also regenerates `sha256sums.txt`, a plain, tool-agnostic manifest (one `sha256(hex)  filename` line per
fixture, sorted by filename) so the fixture set itself is reproducible and reviewable in a diff, without
committing to any one bench script's internal JSON format.

See docs/TEST-FIXTURES.md for what fixture files the owner should drop in and where.

  corpus_check.py check   [--dir bench/corpus] [--binary PATH] [--out FILE]
  corpus_check.py manifest [--dir bench/corpus]   regenerate sha256sums.txt (review the diff!)

`check` also regenerates the manifest as a side effect (a fixture set that has drifted from its manifest is
itself worth surfacing) unless `--no-manifest` is given.

Exit codes (`check`): 0 = PASS (both quality targets met, script-stripping held); 1 = FAIL (a target missed,
or a `<script` leak found); 2 = no fixtures found yet (expected until the owner drops files in -- this is not
an error, it is the current, documented state of A-4/E-5/E-7).
"""
import argparse, hashlib, http.server, json, os, re, sys, threading, time

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_DIR = os.path.join(HERE, "corpus")
SUCCESS_TARGET = 0.95
REDUCTION_TARGET = 0.50
TOKEN_RE = re.compile(r"\S+")
SCRIPT_RE = re.compile(r"<script", re.IGNORECASE)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def list_fixtures(directory):
    if not os.path.isdir(directory):
        return []
    return sorted(f for f in os.listdir(directory) if f.endswith(".html"))


def write_manifest(directory, names):
    path = os.path.join(directory, "sha256sums.txt")
    lines = [f"{sha256_of(os.path.join(directory, n))}  {n}" for n in names]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + ("\n" if lines else ""))
    return path


class _Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):  # noqa: N802 (stdlib method name)
        name = self.path.lstrip("/")
        path = os.path.join(self.directory, name)  # set by _make_handler
        if ".." in name or not os.path.isfile(path):
            self.send_error(404)
            return
        with open(path, "rb") as f:
            body = f.read()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def _make_handler(directory):
    class Bound(_Handler):
        pass

    Bound.directory = directory
    return Bound


def fetch_via_mcp(binary, base_url, name, raw):
    """One `fetch` tool call through the real binary over stdio. Returns (ok, text, error)."""
    sys.path.insert(0, HERE)
    import measure  # local import: reuses the pinned-env stdio driver already used by smoke.py/measure.py

    env = dict(measure.PINNED_ENV)
    s = measure.Server(binary, env)
    try:
        s.handshake()
        args = {"url": f"{base_url}/{name}"}
        if raw:
            args["raw"] = True
        r = s.call("tools/call", {"name": "fetch", "arguments": args}, timeout=30)
        res = r.get("result")
        if not isinstance(res, dict):
            return False, "", f"no result: {json.dumps(r)[:160]}"
        text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
        if res.get("isError"):
            return False, text, text[:200]
        return True, text, None
    except Exception as e:  # noqa: BLE001
        return False, "", f"{type(e).__name__}: {e}"
    finally:
        s.close()


def run_check(directory, binary, out_path, regen_manifest):
    names = list_fixtures(directory)
    if not names:
        msg = (
            f"No fixture files found in {directory} (expected until the owner drops files in -- see "
            "docs/TEST-FIXTURES.md). A-4/E-5/E-7 stay NOT DONE, as documented; this is not a tooling failure."
        )
        print(msg)
        return 2
    if regen_manifest:
        write_manifest(directory, names)

    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _make_handler(directory))
    port = httpd.server_address[1]
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    base_url = f"http://127.0.0.1:{port}"
    time.sleep(0.05)

    rows = []
    try:
        for name in names:
            raw_path = os.path.join(directory, name)
            raw_html = open(raw_path, encoding="utf-8", errors="replace").read()
            raw_tokens = len(TOKEN_RE.findall(raw_html))
            had_script = bool(SCRIPT_RE.search(raw_html))

            ok, text, err = fetch_via_mcp(binary, base_url, name, raw=False)
            conv_tokens = len(TOKEN_RE.findall(text)) if ok else None
            reduction = (
                (raw_tokens - conv_tokens) / raw_tokens if ok and raw_tokens > 0 else None
            )
            script_leaked = ok and had_script and bool(SCRIPT_RE.search(text))
            rows.append(
                dict(
                    name=name,
                    ok=ok,
                    error=err,
                    raw_tokens=raw_tokens,
                    conv_tokens=conv_tokens,
                    reduction=reduction,
                    had_script=had_script,
                    script_leaked=script_leaked,
                )
            )
    finally:
        httpd.shutdown()

    n = len(rows)
    n_ok = sum(1 for r in rows if r["ok"])
    success_rate = n_ok / n if n else 0.0
    reductions = sorted(r["reduction"] for r in rows if r["reduction"] is not None)
    median_reduction = (
        reductions[len(reductions) // 2]
        if len(reductions) % 2
        else (reductions[len(reductions) // 2 - 1] + reductions[len(reductions) // 2]) / 2
    ) if reductions else None
    leaks = [r["name"] for r in rows if r["script_leaked"]]

    success_pass = success_rate >= SUCCESS_TARGET
    reduction_pass = median_reduction is not None and median_reduction >= REDUCTION_TARGET
    script_pass = not leaks
    verdict = "PASS" if (success_pass and reduction_pass and script_pass) else "FAIL"

    report = {
        "verdict": verdict,
        "fixture_count": n,
        "success_rate": round(success_rate, 4),
        "success_target": SUCCESS_TARGET,
        "success_pass": success_pass,
        "median_token_reduction": round(median_reduction, 4) if median_reduction is not None else None,
        "reduction_target": REDUCTION_TARGET,
        "reduction_pass": reduction_pass,
        "script_leaks": leaks,
        "script_pass": script_pass,
        "rows": rows,
    }
    text = json.dumps(report, indent=2)
    print(text)
    if out_path:
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(text + "\n")
    return 0 if verdict == "PASS" else 1


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_check = sub.add_parser("check")
    p_check.add_argument("--dir", default=DEFAULT_DIR)
    p_check.add_argument("--binary", default=os.environ.get("FETCH_MCP_BINARY", "target/release/fetch-mcp"))
    p_check.add_argument("--out")
    p_check.add_argument("--no-manifest", action="store_true")

    p_manifest = sub.add_parser("manifest")
    p_manifest.add_argument("--dir", default=DEFAULT_DIR)

    a = ap.parse_args(argv)
    if a.cmd == "manifest":
        names = list_fixtures(a.dir)
        path = write_manifest(a.dir, names)
        print(f"wrote {path} ({len(names)} fixtures)")
        return 0
    return run_check(a.dir, a.binary, a.out, regen_manifest=not a.no_manifest)


if __name__ == "__main__":
    sys.exit(main())
