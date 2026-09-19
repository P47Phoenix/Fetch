#!/usr/bin/env python3
"""Harness self-test on the stand-in binary (standin_mcp.py). Proves: deterministic fixtures, known-memory
measurement, pass/fail vs targets, INVALID on early stop, refusals. Run: python3 bench/selftest.py"""
import json, os, platform, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fixtures

STANDIN = os.path.join(HERE, "standin_mcp.py")
fails = []


def run(*args, settle=True):
    p = subprocess.run([sys.executable, os.path.join(HERE, "measure.py"), "--binary", STANDIN, *(["--settle", "0.3"] if settle else []), *args],
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
    # peak target missed -> FAIL (exit 1), and non-gate passes are labelled advisory
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--child-env", "STANDIN_ALLOC_MIB=20", "--peak-target-mib", "1")
    check("peak target missed -> exit 1 with FAIL verdict", rc == 1 and r[-1]["verdict"] == "FAIL" and "g4a-5mib-full" in r[-1]["missed"], f"rc={rc}")
    # boundedness broken: too_large path holds 30 MiB more than the 5 MiB run -> FAIL on boundedness
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--scenario", "g4a-50mib-cl", "--peak-target-mib", "1000",
                "--child-env", "STANDIN_TOOLARGE_ALLOC_MIB=30")
    check("boundedness > 1.10 -> exit 1 with FAIL verdict", rc == 1 and r[-1]["verdict"] == "FAIL"
          and any("boundedness" in m for m in r[-1]["missed"]), f"rc={rc} {r[-1].get('boundedness')}")
    rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--idle-target-mib", "1000")
    check("non-gate pass is ADVISORY_PASS, never PASS", r[-1]["verdict"] == "ADVISORY_PASS", r[-1]["verdict"])
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
    rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", settle=False)
    check("--gate complete set refused off native aarch64 (exit 3)" if platform.machine() != "aarch64" else "--gate host is aarch64 (skip)",
          rc == 3 or platform.machine() == "aarch64", f"rc={rc}")
    for flag, val in (("--settle", "1"), ("--parallel-idle", "2")):
        rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", flag, val, settle=False)
        check(f"--gate with {flag} refused by the override rule, exit 2 on any host", rc == 2 and "forbids" in r[-1]["reason"], f"rc={rc}")
    for kv in ("GLIBC_TUNABLES=x", "FETCH_TIMEOUT_MS=1", "LC_ALL=en_US", "PATH=/tmp", "LD_PRELOAD=x"):
        rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", "--child-env", kv, settle=False)
        check(f"--gate refuses child env {kv.split('=')[0]}, exit 2 on any host", rc == 2 and "child env" in r[-1]["reason"], f"rc={rc}")
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--gate", settle=False)
    check("--gate with a partial G4a set refused as incomplete, exit 2", rc == 2 and "incomplete" in r[-1]["reason"], f"rc={rc}")
    # boundedness reference absent must never PASS (the QA probe): 30 MiB blow-up on the 50 MB path alone
    for g in ([], ["--smoke"]):
        rc, r = run(*F, *bk, "--scenario", "g4a-50mib-cl", "--peak-target-mib", "1000", "--child-env", "STANDIN_TOOLARGE_ALLOC_MIB=30", *g)
        check("50 MB scenario without the 5 MiB reference -> INCOMPLETE, exit 2, not a pass",
              rc == 2 and r[-1]["verdict"] == "INCOMPLETE" and r[-1]["incomplete"], f"rc={rc} {r[-1]['verdict']}")
    # nonexistent binary and test-support-only marker
    p = subprocess.run([sys.executable, os.path.join(HERE, "measure.py"), "--binary", "/nonexistent/x", "--binary-kind", "shipped",
                        "--scenario", "idle", *F], capture_output=True, text=True)
    check("nonexistent binary -> refused, exit 2, no traceback", p.returncode == 2 and "Traceback" not in p.stderr, f"rc={p.returncode}")
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--child-env", "STANDIN_TESTSUPPORT=1", "--idle-target-mib", "1000")
    check("shipped kind refuses a test-support-only marker, exit 2", rc == 2, f"rc={rc}")
    # tampered fixture refused
    with open(os.path.join(d, "html_5mib.html"), "r+b") as f: f.seek(100); f.write(b"Z")
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle")
    check("fixture hash mismatch refused, exit 2", rc == 2)


# --- handshake must require JSON-RPC results (QA B1), EOF must fail fast (Dev N1), binary identity (QA B2)
import stat, time, struct
with tempfile.TemporaryDirectory() as d2:
    fixtures.generate(d2)
    F = ["--fixtures-dir", d2]
    def script(name, body):
        p = os.path.join(d2, name)
        with open(p, "w") as f: f.write("#!" + sys.executable + "\n" + body)
        os.chmod(p, 0o755); return p
    HDR = 'import sys, json\nif "--version" in sys.argv: print("fetch-mcp 0.0.0"); sys.exit(0)\n'
    errsrv = script("errsrv.py", HDR + 'for l in sys.stdin:\n    m = json.loads(l)\n    if "id" in m: print(json.dumps({"jsonrpc":"2.0","id":m["id"],"error":{"code":-32603,"message":"x"}}), flush=True)\n')
    notool = script("notool.py", HDR + 'for l in sys.stdin:\n    m = json.loads(l)\n    if "id" in m: print(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":{"tools":[]}}), flush=True)\n')
    crash = script("crash.py", 'import sys, json\nif "--version" in sys.argv: print("fetch-mcp 0.0.0 bench-loopback"); sys.exit(0)\nfor l in sys.stdin:\n    m = json.loads(l)\n    if m.get("method") == "tools/call": sys.exit(1)\n    if "id" in m: print(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":{"tools":[{"name":"fetch"}]}}), flush=True)\n')
    for nm, b in (("error-only server", errsrv), ("server without a fetch tool", notool)):
        p = subprocess.run([sys.executable, os.path.join(HERE, "measure.py"), "--binary", b, "--binary-kind", "shipped", "--scenario", "idle",
                            "--settle", "0.2", "--smoke", *F], capture_output=True, text=True)
        recs = [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
        sc = next((r for r in recs if r.get("scenario") == "idle"), {})
        check(f"{nm}: INVALID, 0 valid runs, exit 2", p.returncode == 2 and sc.get("valid_runs") == 0 and recs[-1]["verdict"] == "INVALID", f"rc={p.returncode} {sc.get('invalid_reasons')}")
    t0 = time.time()
    p = subprocess.run([sys.executable, os.path.join(HERE, "measure.py"), "--binary", crash, "--binary-kind", "bench", "--scenario", "g4a-5mib-full",
                        "--smoke", "--runs", "2", *F], capture_output=True, text=True)
    recs = [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
    sc = next((r for r in recs if r.get("scenario") == "g4a-5mib-full"), {})
    check("server that exits mid-fetch fails fast with 'server closed stdout' (EOF sentinel), not a 120 s hang",
          time.time() - t0 < 30 and any("server closed stdout" in x for x in sc.get("invalid_reasons", [])), f"{time.time() - t0:.1f}s {sc.get('invalid_reasons')}")
    # identity: a real-looking ELF header is needed for the positive case
    import measure
    def elf(name, machine, data_byte=1, extra=b""):
        e = bytearray(64); e[:4] = b"\x7fELF"; e[4] = 2; e[5] = data_byte
        e[18:20] = machine.to_bytes(2, "little" if data_byte == 1 else "big")
        p = os.path.join(d2, name)
        with open(p, "wb") as f: f.write(bytes(e) + extra)
        return p
    host = platform.machine(); em = measure.ELF_MACHINE[host]; other = 183 if em != 183 else 62
    V = "fetch-mcp 0.0.0"
    check("identity: host-arch ELF with fetch-mcp version passes as shipped", measure.check_identity(elf("ok", em), V, "shipped") is None)
    check("identity: stand-in script refused", measure.check_identity(STANDIN, "standin-mcp 0.0.0", "shipped") is not None)
    check("identity: script printing a fetch-mcp version still refused (not ELF)", measure.check_identity(errsrv, V, "shipped") is not None)
    check("identity: wrong-arch ELF refused", "e_machine" in (measure.check_identity(elf("wa", other), V, "shipped") or ""))
    check("identity: ELF with a foreign --version refused", measure.check_identity(elf("fv", em), "standin-mcp 0.0.0", "shipped") is not None)
    check("identity: shipped ELF carrying a marker token refused", measure.check_identity(elf("mk", em, extra=b"FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback"), V, "shipped") is not None)
    check("identity: bench ELF without the marker refused", measure.check_identity(elf("bn", em), V + " bench-loopback", "bench") is not None)
    check("identity: bench ELF with the marker passes", measure.check_identity(elf("bm", em, extra=b"FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback"), V + " bench-loopback", "bench") is None)
    check("identity: big-endian byte order decoded", measure.check_identity(elf("be", em, 2), V, "shipped") is None)

print("SELFTEST", "FAILED: " + ", ".join(fails) if fails else "PASSED")
sys.exit(1 if fails else 0)
