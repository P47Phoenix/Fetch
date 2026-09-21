#!/usr/bin/env python3
"""E-5 live smoke runner (non-gating): fetch each URL of an owner-supplied list through the MCP server and record what came back.

  smoke.py --binary BIN --list FILE [--out FILE] [--timeout SEC] [--child-env K=V ...] [--dry-run]

LIST FILE: one URL per line, `#` starts a comment, blank lines ignored. Optional second field (space or tab) is the category:
tls, redirect, json, text, html (E-5: "network, TLS, redirects, JSON and plain text"). Example line:  https://example.org/data.json  json

The list is the OWNER's (E-7, still unsupplied); this tool never invents one. A real run (no --dry-run) is refused unless the list has
exactly 10 distinct https:// URLs, none with userinfo, and covers the categories tls, redirect, json and text at least once. --dry-run
relaxes count, scheme and category rules so the harness can be exercised against local fixtures; its records carry `"dry_run": true`
and e5_report.py will never accept them as the smoke result.

Output: one JSONL record per URL (kind "smoke") then one summary (kind "smoke-summary"). Exit 0 when the run completed (a URL that failed is
a recorded result, not a gate: the smoke is non-gating), 2 when the list is invalid or the harness failed. Same pinned child environment as measure.py.
"""
import argparse, json, sys, time, urllib.parse
import measure

CATEGORIES = ("tls", "redirect", "json", "text", "html")
REQUIRED = ("tls", "redirect", "json", "text")
WANT = 10


def parse_list(path):
    """Returns (entries, problems). An entry is {"url", "category"}; nothing is defaulted or invented."""
    entries, problems = [], []
    with open(path, encoding="utf-8") as f:
        for n, raw in enumerate(f, 1):
            line = raw.split("#", 1)[0].strip()
            if not line: continue
            parts = line.split()
            if len(parts) > 2: problems.append(f"line {n}: expected 'URL [category]', got {len(parts)} fields"); continue
            cat = parts[1].lower() if len(parts) == 2 else None
            if cat is not None and cat not in CATEGORIES: problems.append(f"line {n}: unknown category {parts[1]!r} (use one of {', '.join(CATEGORIES)})"); continue
            entries.append({"url": parts[0], "category": cat, "line": n})
    return entries, problems


def validate(entries, dry_run):
    """Rules for a real run. A dry run only needs at least one entry and http(s) URLs."""
    bad = []
    if not entries: return ["the list has no URLs"]
    seen = set()
    for e in entries:
        u = urllib.parse.urlparse(e["url"])
        if u.scheme not in ("http", "https") or not u.hostname: bad.append(f"line {e['line']}: not an absolute http(s) URL"); continue
        if u.username or u.password: bad.append(f"line {e['line']}: URL has userinfo (credentials and authenticated pages are excluded)")
        if e["url"] in seen: bad.append(f"line {e['line']}: duplicate URL")
        seen.add(e["url"])
        if not dry_run and u.scheme != "https": bad.append(f"line {e['line']}: real smoke URLs must be https (the smoke checks TLS)")
    if not dry_run:
        if len(entries) != WANT: bad.append(f"the list has {len(entries)} URLs; E-5 needs exactly {WANT}")
        missing = [c for c in REQUIRED if not any(e["category"] == c for e in entries)]
        if missing: bad.append(f"no URL is tagged with category {', '.join(missing)} (E-5 needs network, TLS, redirects, JSON and plain text)")
    return bad


def fetch_one(binary, env, entry, timeout):
    s, t0 = None, time.monotonic()
    rec = {"kind": "smoke", **{k: entry[k] for k in ("url", "category")}}
    try:
        s = measure.Server(binary, env)
        s.handshake()
        r = s.call("tools/call", {"name": "fetch", "arguments": {"url": entry["url"]}}, timeout=timeout)
        res = r.get("result")
        if not isinstance(res, dict):
            rec.update(ok=False, error=f"no result: {json.dumps(r)[:160]}")
        else:
            text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
            rec["chars"] = len(text)
            if res.get("isError"): rec.update(ok=False, error=text[:200])
            else:
                # A redirected result begins with `URL: <final>\nStatus: <n>` (A-9); a plain fetch has no header.
                redirected = text.startswith("URL: ")
                rec.update(ok=len(text.strip()) > 0, redirected=redirected, final_url=text.split("\n", 1)[0][5:] if redirected else entry["url"])
                if not rec["ok"]: rec["error"] = "empty content"
    except Exception as e:  # noqa: BLE001
        rec.update(ok=False, error=f"{type(e).__name__}: {e}")
    finally:
        if s: s.close()
    rec["seconds"] = round(time.monotonic() - t0, 2)
    return rec


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--binary", required=True); ap.add_argument("--list", required=True); ap.add_argument("--out")
    ap.add_argument("--timeout", type=int, default=90); ap.add_argument("--child-env", action="append", default=[]); ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args(argv)
    lines = []

    def emit(o):
        s = json.dumps(o); print(s, flush=True); lines.append(s)

    try:
        entries, problems = parse_list(a.list)
    except OSError as e:
        emit({"kind": "smoke-summary", "verdict": "INVALID", "dry_run": a.dry_run, "reason": f"cannot read the list: {e}"}); return 2
    problems += validate(entries, a.dry_run) if not problems else []
    if problems:
        emit({"kind": "smoke-summary", "verdict": "INVALID", "dry_run": a.dry_run, "reason": "; ".join(problems)}); _write(a.out, lines); return 2
    env = dict(kv.split("=", 1) for kv in a.child_env)
    recs = [fetch_one(a.binary, env, e, a.timeout) for e in entries]
    for r in recs: emit(r)
    ok = sum(1 for r in recs if r["ok"])
    emit({"kind": "smoke-summary", "verdict": "DONE", "dry_run": a.dry_run, "gating": False, "list_count": len(entries), "ok": ok, "failed": len(recs) - ok,
          "failed_urls": [r["url"] for r in recs if not r["ok"]], "redirected": sum(1 for r in recs if r.get("redirected")),
          "by_category": {c: [sum(1 for r in recs if r["category"] == c and r["ok"]), sum(1 for r in recs if r["category"] == c)] for c in CATEGORIES},
          "binary_version": measure.version_of(a.binary, env)})
    _write(a.out, lines)
    return 0


def _write(path, lines):
    if path:
        with open(path, "w", encoding="utf-8") as f: f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    sys.exit(main())
