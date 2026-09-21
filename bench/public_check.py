#!/usr/bin/env python3
"""E-8 / E-4 shipped-binary public-host cross-check (recorded, NOT a gate figure).

  public_check.py --shipped BIN --bench BIN [--url URL] [--runs 5] [--out FILE]

Fetches one real public HTTPS page with the SHIPPED build and the BENCH build (fresh process each, VmHWM read after the call
returned) and checks the E-8 bound: shipped median within 10% of the bench median and at or below 40 MiB. The default page is
the largest public HTML page found that fits the 5 MiB cap (about 2 MiB); no public page of exactly 5 MiB is known, so this is
NOT a 5 MiB figure and the record says so. It uses the network, so it is advisory in CI (a network failure is exit 2, a missed
bound is exit 1). Same pinned child environment as measure.py; both builds must share commit and Cargo.lock hash.
"""
import argparse, json, statistics, sys
import measure

DEFAULT_URL = "https://en.wikipedia.org/wiki/List_of_minor_planets:_1%E2%80%931000"
MIB_KB = 1024


def one(binary, url, max_length):
    s = None
    try:
        s = measure.Server(binary, {})
        s.handshake()
        r = s.call("tools/call", {"name": "fetch", "arguments": {"url": url, "max_length": max_length}}, timeout=120)
        st = measure.proc_status(s.p.pid)
        res = r.get("result")
        if not isinstance(res, dict) or res.get("isError"):
            return {"valid": False, "reason": json.dumps(res if res is not None else r)[:200]}
        text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
        return {"valid": True, "hwm_kB": st["VmHWM"], "output_chars": len(text)}
    except Exception as e:  # noqa: BLE001
        return {"valid": False, "reason": f"{type(e).__name__}: {e}"}
    finally:
        if s: s.close()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--shipped", required=True); ap.add_argument("--bench", required=True)
    ap.add_argument("--url", default=DEFAULT_URL); ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--max-length", type=int, default=5 * 1024 * 1024); ap.add_argument("--out")
    a = ap.parse_args(argv)
    out = {"kind": "public-host-cross-check", "url": a.url, "runs": a.runs, "max_length": a.max_length, "gating": False,
           "note": "recorded, not a gate figure; page is the largest public HTML page found under the cap, not 5 MiB"}
    for k, b in (("shipped", a.shipped), ("bench", a.bench)):
        out[k] = {"version": measure.version_of(b, {}), "samples": [one(b, a.url, a.max_length) for _ in range(a.runs)]}
    med, bad = {}, []
    for k in ("shipped", "bench"):
        good = [x["hwm_kB"] for x in out[k]["samples"] if x["valid"]]
        if len(good) < a.runs: bad.append(f"{k}: {len(good)}/{a.runs} valid ({sorted({x.get('reason', '') for x in out[k]['samples'] if not x['valid']})})")
        if good: med[k] = statistics.median(good); out[k]["median_MiB"] = round(med[k] / MIB_KB, 2); out[k]["samples_kB"] = good
    idn = measure.check_build_identity(out["shipped"]["version"], out["bench"]["version"])
    if idn: bad.append("build identity: " + idn)
    if bad or len(med) < 2:
        out["verdict"] = "INVALID"; out["reason"] = "; ".join(bad) or "no valid samples"; code = 2
    else:
        ratio = med["shipped"] / med["bench"]
        out["ratio_shipped_over_bench"] = round(ratio, 3)
        ok = abs(ratio - 1) <= 0.10 and med["shipped"] <= 40 * MIB_KB
        out["verdict"] = "PASS" if ok else "FAIL"; code = 0 if ok else 1
    line = json.dumps(out)
    print(line)
    if a.out:
        with open(a.out, "a") as f: f.write(line + "\n")
    return code


if __name__ == "__main__":
    sys.exit(main())
