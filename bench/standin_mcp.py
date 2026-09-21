#!/usr/bin/env python3
"""Stand-in for the MCP server, for harness self-test only (E-1). NOT the product.
Speaks minimal MCP over stdio. `fetch` streams the URL body (discarding it), refusing above a 5 MiB
cap with a too_large error, then holds STANDIN_ALLOC_MIB of touched memory so peak RSS is known.
Env: STANDIN_IDLE_ALLOC_MIB (at start), STANDIN_ALLOC_MIB (on successful fetch),
STANDIN_EARLY_STOP=<bytes> (read only that many bytes: must be flagged invalid), STANDIN_TOOLARGE_ALLOC_MIB (on too_large, to test boundedness FAIL), STANDIN_NO_REDIRECT=1 (do not follow redirects: redirect-chain must be INVALID), STANDIN_BENCH=1 (--version marker), STANDIN_DELAY_MS (startup delay, timing self-test), STANDIN_NO_HOSTILE_REFUSAL=1 (convert the hostile-attrs page instead of refusing it: hostile-attrs3 must be INVALID), STANDIN_NO_TOTAL=1 (the past-the-end reply omits the total: the harness's length probe must fail), STANDIN_NO_WINDOW_STOP=1 (never stop at the window).
"""
import http.client, json, os, sys, time, urllib.parse

MIB = 1024 * 1024
CAP = 5 * MIB
if "--version" in sys.argv:
    print("standin-mcp 0.0.0 commit=selftest lock=selftest" + (" bench-loopback" if os.environ.get("STANDIN_BENCH") else "")
          + (" FETCH_MCP_MARKER_TEST_SUPPORT_V1:test-support" if os.environ.get("STANDIN_TESTSUPPORT") else ""))
    sys.exit(0)
time.sleep(int(os.environ.get("STANDIN_DELAY_MS", "0")) / 1000)   # startup delay: timing self-test only
hold = [b"\x01" * (int(os.environ.get("STANDIN_IDLE_ALLOC_MIB", "0")) * MIB)]


def fetch(args):
    """Returns (error_code_or_None, text). Emulates the product's window: reads until start+take+1 bytes (one confirming character) then
    stops (early stop, A-5), or to the end for the total; a start_index at or past the end is the empty-content message; text carries the
    same footers as the product so the harness's window checks can be exercised. One byte counts as one character (the fixtures are ASCII)."""
    url = args["url"]; start = int(args.get("start_index", 0)); take = int(args.get("max_length", 5000))
    for _hop in range(6):   # follows up to 5 redirects, reading each hop's body (redirect-chain scenario)
        u = urllib.parse.urlparse(url)
        c = http.client.HTTPConnection(u.hostname, u.port, timeout=60)
        c.request("GET", u.path); r = c.getresponse()
        loc = r.getheader("Location")
        if r.status in (301, 302, 303, 307, 308) and loc and not os.environ.get("STANDIN_NO_REDIRECT"):
            r.read(); c.close(); url = urllib.parse.urljoin(url, loc); continue
        break
    if "hostile-attrs" in url and not os.environ.get("STANDIN_NO_HOSTILE_REFUSAL"):   # the product refuses the attribute bomb after the first slices
        r.read(64 * 1024); c.close(); return "converter_limit", ""
    cl = r.getheader("Content-Length")
    if cl and int(cl) > CAP:
        c.close(); return "too_large", ""
    stop = int(os.environ.get("STANDIN_EARLY_STOP", "0")) or None
    n, more = 0, False
    while True:
        b = r.read(64 * 1024)
        if not b: break
        n += len(b)
        if stop and n >= stop: break
        if n > CAP: c.close(); return "too_large", ""
        if n >= start + take + 1 and not os.environ.get("STANDIN_NO_WINDOW_STOP"): more = True; break
    c.close()
    if not more and start >= n: return None, (f"[No content at start_index={start}: the content is {n} characters long.]" if not os.environ.get("STANDIN_NO_TOTAL") else "[No content.]")
    text = "x" * min(take, max(0, n - start))
    if more: return None, text + f"\n\n[More content available. Call fetch again with start_index={start + take} to continue.]"
    return None, text + (f"\n\n[Total length: {n} characters.]" if start > 0 else "")


for line in sys.stdin:
    m = json.loads(line)
    if "id" not in m: continue
    if m["method"] == "initialize":
        res = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "standin", "version": "0"}}
    elif m["method"] == "tools/list":
        res = {"tools": [{"name": "fetch", "inputSchema": {"type": "object"}}]}
    else:
        err, text = fetch(m["params"]["arguments"])
        if err:
            hold.append(b"\x01" * (int(os.environ.get("STANDIN_TOOLARGE_ALLOC_MIB", "0")) * MIB))
            res = {"isError": True, "content": [{"type": "text", "text": err}]}
        else:
            hold.append(b"\x01" * (int(os.environ.get("STANDIN_ALLOC_MIB", "0")) * MIB))
            res = {"content": [{"type": "text", "text": text}]}
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": res}) + "\n"); sys.stdout.flush()
