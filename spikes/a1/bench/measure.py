#!/usr/bin/env python3
"""usage: measure.py <binary> <url> [runs] [prefix-cmd...]  -> prints idle VmRSS / peak VmHWM (kB) per run and medians.
Idle = VmRSS after initialize + tools/list. Peak = VmHWM after one fetch (max_length=50000, start_index=0) returned."""
import json, subprocess, sys, statistics, time
def status(pid):
    d = {}
    for l in open(f"/proc/{pid}/status"):
        if l.startswith(("VmRSS", "VmHWM")): d[l.split(":")[0]] = int(l.split()[1])
    return d
def rpc(p, msg, want_id=None):
    p.stdin.write((json.dumps(msg) + "\n").encode()); p.stdin.flush()
    if want_id is None: return None
    while True:
        line = p.stdout.readline()
        if not line: raise RuntimeError("eof")
        m = json.loads(line)
        if m.get("id") == want_id: return m
def run(binary, url, args):
    p = subprocess.Popen(args + [binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    pid = p.pid
    rpc(p, {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"m","version":"0"}}}, 1)
    rpc(p, {"jsonrpc":"2.0","method":"notifications/initialized"})
    rpc(p, {"jsonrpc":"2.0","id":2,"method":"tools/list"}, 2)
    time.sleep(0.5)
    idle = status(pid)
    r = rpc(p, {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fetch","arguments":{"url":url,"max_length":50000,**json.loads(__import__("os").environ.get("FETCH_ARGS","{}"))}}}, 3)
    ok = "result" in r and not r["result"].get("isError")
    n = len(json.dumps(r))
    time.sleep(0.3)
    after = status(pid)
    p.stdin.close(); p.wait(timeout=5)
    return idle["VmRSS"], idle["VmHWM"], after["VmRSS"], after["VmHWM"], ok, n
if __name__ == "__main__":
    binary, url = sys.argv[1], sys.argv[2]
    runs = int(sys.argv[3]) if len(sys.argv) > 3 else 10
    res = [run(binary, url, sys.argv[4:]) for _ in range(runs)]
    idle = [r[0] for r in res]; peak = [r[3] for r in res]; post = [r[2] for r in res]
    print(json.dumps({"binary": binary, "idle_rss_kB_median": statistics.median(idle), "peak_hwm_kB_median": statistics.median(peak),
        "post_rss_kB_median": statistics.median(post), "idle_min_max": [min(idle), max(idle)], "peak_min_max": [min(peak), max(peak)],
        "all_ok": all(r[4] for r in res), "resp_bytes": res[0][5]}))
