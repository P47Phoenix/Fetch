#!/usr/bin/env python3
"""Harness self-test on the stand-in binary (standin_mcp.py). Proves: deterministic fixtures, known-memory
measurement, pass/fail vs targets, INVALID on early stop, refusals. Run: python3 bench/selftest.py"""
import json, os, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fixtures

STANDIN = os.path.join(HERE, "standin_mcp.py")
fails = []


def run(*args):
    p = subprocess.run([sys.executable, os.path.join(HERE, "measure.py"), "--binary", STANDIN, "--settle", "0.3", *args],
                       capture_output=True, text=True)
    recs = [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
    return p.returncode, recs


def check(name, cond, detail=""):
    print(("ok   " if cond else "FAIL ") + name + (f"  [{detail}]" if detail else ""))
    if not cond: fails.append(name)


def scen(recs, n): return next(r for r in recs if r.get("scenario") == n)


with tempfile.TemporaryDirectory() as d:
    fixtures.generate(d)
    check("fixtures regenerate byte-identical to committed manifest", not fixtures.verify(d), "; ".join(fixtures.verify(d)))
    F = ["--fixtures-dir", d]
    # known-memory proof: alloc 0 vs 20 MiB must differ by ~20 MiB (idle and peak)
    rc0, r0 = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--child-env", "STANDIN_IDLE_ALLOC_MIB=0")
    rc1, r1 = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--child-env", "STANDIN_IDLE_ALLOC_MIB=20")
    base, big = scen(r0, "idle")["median_kB"], scen(r1, "idle")["median_kB"]
    check("idle: +20 MiB allocation is measured (+-1.5 MiB)", abs((big - base) / 1024 - 20) < 1.5, f"base {base} kB, big {big} kB")
    check("idle: 10 valid runs, min/median/max present", scen(r1, "idle")["valid_runs"] == 10 and "min_kB" in scen(r1, "idle"))
    bk = ["--binary-kind", "bench", "--child-env", "STANDIN_BENCH=1"]
    p0 = run(*F, *bk, "--scenario", "g4a-5mib-full", "--child-env", "STANDIN_ALLOC_MIB=0")[1]
    rc, p1 = run(*F, *bk, "--scenario", "g4a-5mib-full", "--child-env", "STANDIN_ALLOC_MIB=20", "--peak-target-mib", "1000")
    pb, pg = scen(p0, "g4a-5mib-full")["median_kB"], scen(p1, "g4a-5mib-full")["median_kB"]
    check("peak: +20 MiB allocation is measured in VmHWM (+-1.5 MiB)", abs((pg - pb) / 1024 - 20) < 1.5, f"base {pb} kB, big {pg} kB")
    # pass / fail against targets
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--idle-target-mib", "1000")
    check("target met -> exit 0", rc == 0, f"rc={rc}")
    rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--idle-target-mib", "1")
    check("target missed -> exit 1 with FAIL verdict", rc == 1 and r[-1]["verdict"] == "FAIL", f"rc={rc}")
    # full G4a group incl. boundedness on the stand-in
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--scenario", "g4a-5mib-gz", "--scenario", "g4a-50mib-cl",
                "--scenario", "g4a-50mib-chunked", "--peak-target-mib", "1000")
    check("50 MiB CL/chunked -> too_large valid, boundedness <= 1.10", rc == 0 and all(v <= 1.10 for v in r[-1].get("boundedness", {"x": 9}).values()),
          f"rc={rc} bounded={r[-1].get('boundedness')} peak={r[-1].get('gating_peak_MiB')} MiB")
    # early-stop sample must be INVALID
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--child-env", "STANDIN_EARLY_STOP=1000")
    check("early stop -> INVALID, exit 2", rc == 2 and r[-1]["verdict"] == "INVALID", f"rc={rc} {scen(r, 'g4a-5mib-full')['invalid_reasons']}")
    # refusals
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--runs", "3")
    check("fewer than 10 runs refused, exit 2", rc == 2)
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "g4a-5mib-full")
    check("peak scenario on shipped binary refused", rc == 2)
    rc, _ = run(*F, "--binary-kind", "bench", "--scenario", "idle")
    check("bench kind without marker refused", rc == 2)
    rc, _ = run(*F, *bk, "--scenario", "idle")
    check("idle on bench-marked binary refused", rc == 2)
    rc, _ = run(*F, *bk, "--scenario", "g4b-raw")
    check("unimplemented scenario refused", rc == 2)
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate")
    import platform
    check("--gate refused off native aarch64 (exit 3)" if platform.machine() != "aarch64" else "--gate host is aarch64 (skip)",
          rc == 3 or platform.machine() == "aarch64", f"rc={rc}")
    # tampered fixture refused
    with open(os.path.join(d, "html_5mib.html"), "r+b") as f: f.seek(100); f.write(b"Z")
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle")
    check("fixture hash mismatch refused, exit 2", rc == 2)

print("SELFTEST", "FAILED: " + ", ".join(fails) if fails else "PASSED")
sys.exit(1 if fails else 0)
