#!/usr/bin/env python3
"""Stand-in for the MCP server, for harness self-test only (E-1). NOT the product.
Speaks minimal MCP over stdio. `fetch` streams the URL body (discarding it), refusing above a 5 MiB
cap with a too_large error, then holds STANDIN_ALLOC_MIB of touched memory so peak RSS is known.
Env: STANDIN_IDLE_ALLOC_MIB (at start), STANDIN_ALLOC_MIB (on successful fetch),
STANDIN_EARLY_STOP=<bytes> (read only that many bytes: must be flagged invalid), STANDIN_BENCH=1 (--version marker).
"""
import http.client, json, os, sys, urllib.parse

MIB = 1024 * 1024
CAP = 5 * MIB
if "--version" in sys.argv:
    print("standin-mcp 0.0.0 commit=selftest lock=selftest" + (" bench-loopback" if os.environ.get("STANDIN_BENCH") else ""))
    sys.exit(0)
hold = [b"\x01" * (int(os.environ.get("STANDIN_IDLE_ALLOC_MIB", "0")) * MIB)]


def fetch(url):
    u = urllib.parse.urlparse(url)
    c = http.client.HTTPConnection(u.hostname, u.port, timeout=60)
    c.request("GET", u.path); r = c.getresponse()
    cl = r.getheader("Content-Length")
    if cl and int(cl) > CAP:
        c.close(); return "too_large"
    stop = int(os.environ.get("STANDIN_EARLY_STOP", "0")) or None
    n = 0
    while True:
        b = r.read(64 * 1024)
        if not b: break
        n += len(b)
        if stop and n >= stop: break
        if n > CAP: c.close(); return "too_large"
    c.close(); return None


for line in sys.stdin:
    m = json.loads(line)
    if "id" not in m: continue
    if m["method"] == "initialize":
        res = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "standin", "version": "0"}}
    elif m["method"] == "tools/list":
        res = {"tools": [{"name": "fetch", "inputSchema": {"type": "object"}}]}
    else:
        err = fetch(m["params"]["arguments"]["url"])
        if err:
            res = {"isError": True, "content": [{"type": "text", "text": err}]}
        else:
            hold.append(b"\x01" * (int(os.environ.get("STANDIN_ALLOC_MIB", "0")) * MIB))
            res = {"content": [{"type": "text", "text": "ok"}]}
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": res}) + "\n"); sys.stdout.flush()
