#!/usr/bin/env python3
"""Self-test for scripts/regression_gate.py (E-6). Proves: PASS within threshold, FAIL beyond it,
--update seeds/refreshes a baseline, and a missing baseline entry is advisory (exit 0), not a false PASS
or silent gate bypass. Run: python3 scripts/regression_gate_selftest.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "regression_gate.py")
fails = []


def check(name, cond, detail=""):
    print(("ok   " if cond else "FAIL ") + name + (f"  [{detail}]" if detail else ""))
    if not cond:
        fails.append(name)


def write_jsonl(path, recs):
    with open(path, "w") as f:
        for r in recs:
            f.write(json.dumps(r) + "\n")


def idle_jsonl(d, median_kb):
    p = os.path.join(d, "idle.jsonl")
    write_jsonl(p, [{"kind": "scenario", "scenario": "idle", "median_kB": median_kb}])
    return p


def peak_jsonl(d, gating_kb):
    p = os.path.join(d, "peak.jsonl")
    write_jsonl(p, [{"kind": "summary", "gating_peak_kB": gating_kb}])
    return p


def run(*args):
    p = subprocess.run([sys.executable, SCRIPT, *args], capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr


with tempfile.TemporaryDirectory() as d:
    baseline = os.path.join(d, "baseline.json")
    with open(baseline, "w") as f:
        json.dump({"amd64-gnu": {"idle_kB": 4000, "peak_kB": 6000, "updated_from_run": "seed"}}, f)

    idle = idle_jsonl(d, 4100)   # +2.5%
    peak = peak_jsonl(d, 6200)   # +3.3%
    rc, out, err = run("--idle", idle, "--peak", peak, "--baseline", baseline, "--platform", "amd64-gnu")
    check("within threshold exits 0", rc == 0, f"rc={rc} err={err}")

    idle = idle_jsonl(d, 5000)   # +25%, over 10%
    peak = peak_jsonl(d, 6100)
    rc, out, err = run("--idle", idle, "--peak", peak, "--baseline", baseline, "--platform", "amd64-gnu")
    check("idle regression beyond threshold exits 1", rc == 1, f"rc={rc}")
    check("idle regression names the metric in stderr", "idle (VmRSS)" in err, err)

    idle = idle_jsonl(d, 4100)
    peak = peak_jsonl(d, 7000)   # +16.7%, over 10%
    rc, out, err = run("--idle", idle, "--peak", peak, "--baseline", baseline, "--platform", "amd64-gnu")
    check("peak regression beyond threshold exits 1", rc == 1, f"rc={rc}")

    idle = idle_jsonl(d, 4100)
    peak = peak_jsonl(d, 6200)
    rc, out, err = run("--idle", idle, "--peak", peak, "--baseline", baseline, "--platform", "arm64-musl")
    check("missing baseline entry is advisory, exits 0, never a silent regression-hide of the absolute gate", rc == 0, f"rc={rc}")
    check("missing baseline entry says so on stderr", "no baseline entry" in err, err)

    idle = idle_jsonl(d, 3000)
    peak = peak_jsonl(d, 5000)
    rc, out, err = run("--update", "--idle", idle, "--peak", peak, "--baseline", baseline, "--platform", "amd64-gnu", "--run-id", "999")
    check("--update exits 0", rc == 0, f"rc={rc}")
    with open(baseline) as f:
        written = json.load(f)
    check("--update writes the new medians", written["amd64-gnu"]["idle_kB"] == 3000 and written["amd64-gnu"]["peak_kB"] == 5000, str(written.get("amd64-gnu")))
    check("--update records the run id", written["amd64-gnu"]["updated_from_run"] == "999", str(written.get("amd64-gnu")))

    rc, out, err = run("--idle", idle_jsonl(d, 3050), "--peak", peak_jsonl(d, 5050), "--baseline", baseline, "--platform", "amd64-gnu")
    check("updated baseline is honoured on the next gate check", rc == 0, f"rc={rc}")

if fails:
    print(f"\n{len(fails)} FAILED: {fails}")
    sys.exit(1)
print("\nall regression_gate self-tests passed")
