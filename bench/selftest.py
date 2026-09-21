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
    if fails:
        print("FAIL selftest aborted: fixtures did not verify, later phases need them")
        sys.exit(1)
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
    # timings: recorded, sane, valid-only, never gating
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--peak-target-mib", "1000")
    sc = scen(r, "g4a-5mib-full"); T = sc["timings"]
    check("timings: all five *_ms stats present with median/min/max, no p95 at 10 samples",
          all(k in T for k in ("ready_ms", "tools_list_ms", "first_byte_ms", "fetch_ms", "total_ms")) and all("p95" not in T[k] for k in ("ready_ms", "total_ms")), str(T)[:200])
    check("timings: non-negative, ready <= total, first_byte <= fetch <= total, min<=median<=max",
          all(x["valid"] and 0 <= x["ready_ms"] <= x["total_ms"] and 0 <= x["first_byte_ms"] <= x["fetch_ms"] <= x["total_ms"] for x in sc["sample_timings"])
          and all(T[k]["min"] >= 0 and T[k]["min"] <= T[k]["median"] <= T[k]["max"] for k in ("ready_ms", "fetch_ms", "total_ms")))
    check("timings: labelled advisory + stand-in + not gated; ISO UTC stamps on run and samples",
          T["label"].startswith("advisory") and T["standin"] and T["gated"] is False and r[0]["run_start_utc"].endswith("Z")
          and r[-1]["run_end_utc"] >= r[0]["run_start_utc"] and all(x["start_utc"].endswith("Z") for x in sc["sample_timings"]))
    rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--runs", "20", "--idle-target-mib", "1000")
    check("timings: p95 present at 20 samples, idle has no fetch timings",
          "p95" in scen(r, "idle")["timings"]["ready_ms"] and "fetch_ms" not in scen(r, "idle")["timings"], str(scen(r, "idle")["timings"])[:160])
    # invalid samples' times excluded from timing medians
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--child-env", "STANDIN_EARLY_STOP=1000")
    sc = scen(r, "g4a-5mib-full")
    check("timings: invalid samples excluded from timing stats (present per sample, absent from medians)",
          sc["valid_runs"] == 0 and "fetch_ms" not in sc["timings"] and all(not x["valid"] and "fetch_ms" in x for x in sc["sample_timings"]))
    # timing never changes a memory verdict/exit code: same memory outcome for fast vs slowed server (delay is timing only)
    fast = run(*F, *bk, "--scenario", "g4a-5mib-full", "--peak-target-mib", "1000")
    slow = run(*F, *bk, "--scenario", "g4a-5mib-full", "--peak-target-mib", "1000", "--child-env", "STANDIN_DELAY_MS=300")
    check("timings: gating verdict/exit unaffected by timing (300 ms slower ready, same verdict and exit code)",
          fast[0] == slow[0] == 0 and fast[1][-1]["verdict"] == slow[1][-1]["verdict"]
          and scen(slow[1], "g4a-5mib-full")["timings"]["ready_ms"]["median"] > scen(fast[1], "g4a-5mib-full")["timings"]["ready_ms"]["median"] + 200)
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
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--child-env", "NOEQUALS", "--smoke", "--runs", "1")
    check("malformed --child-env is a refusal (exit 2), not a crash-as-'target missed' (NB-1)", rc == 2, f"rc={rc}")
    import measure, io, contextlib
    from scenarios import SCENARIOS
    SCENARIOS["g4b-raw"]["implemented"] = True   # NB-2: an implemented peak scenario without min_bytes must be refused
    try:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = measure.main(["--binary", STANDIN, *F, *bk, "--scenario", "g4b-raw"])
    finally:
        SCENARIOS["g4b-raw"]["implemented"] = False
    check("peak scenario lacking min_bytes refused (NB-2)", rc == 2 and "min_bytes" in buf.getvalue(), f"rc={rc}")
    real = measure._main
    measure._main = lambda argv=None: 1 / 0
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            rc = measure.main([])
    finally:
        measure._main = real
    check("uncaught harness exception exits 2, never 1 (NB-1)", rc == 2, f"rc={rc}")
    rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", settle=False)
    if platform.machine() in ("aarch64", "x86_64"):
        # native amd64 and arm64 both gate: the stand-in script then fails the identity rule (exit 2), never gates
        check("--gate on a native host reaches the identity check and refuses the stand-in, exit 2", rc == 2 and "binary identity" in r[-1]["reason"], f"rc={rc} {r[-1].get('reason')}")
    else:
        check("--gate refused off a native aarch64/x86_64 host (exit 3)", rc == 3, f"rc={rc}")
    for flag, val in (("--settle", "1"), ("--parallel-idle", "2")):
        rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", flag, val, settle=False)
        check(f"--gate with {flag} refused by the override rule, exit 2 on any host", rc == 2 and "forbids" in r[-1]["reason"], f"rc={rc}")
    for kv in ("GLIBC_TUNABLES=x", "FETCH_TIMEOUT_MS=1", "LC_ALL=en_US", "PATH=/tmp", "LD_PRELOAD=x"):
        rc, r = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--gate", "--child-env", kv, settle=False)
        check(f"--gate refuses child env {kv.split('=')[0]}, exit 2 on any host", rc == 2 and "child env" in r[-1]["reason"], f"rc={rc}")
    rc, r = run(*F, *bk, "--scenario", "g4a-5mib-full", "--gate", settle=False)
    check("--gate with a partial G4a set refused as incomplete, exit 2", rc == 2 and "incomplete" in r[-1]["reason"], f"rc={rc}")
    # boundedness reference absent must never PASS (the QA probe): 30 MiB blow-up on the 50 MiB path alone
    for g in ([], ["--smoke"]):
        rc, r = run(*F, *bk, "--scenario", "g4a-50mib-cl", "--peak-target-mib", "1000", "--child-env", "STANDIN_TOOLARGE_ALLOC_MIB=30", *g)
        check("50 MiB scenario without the 5 MiB reference -> INCOMPLETE, exit 2, not a pass",
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

# --- native_host preflight (amd64 + arm64 hosted runners; QEMU never gates) and gate-mode verdicts (any native host)
import measure, io, contextlib
with tempfile.TemporaryDirectory() as bd:
    check("native_host: x86_64 with no binfmt is native", measure.native_host("x86_64", bd)[0])
    check("native_host: aarch64 with no binfmt is native", measure.native_host("aarch64", bd)[0])
    check("native_host: riscv64 refused", not measure.native_host("riscv64", bd)[0])
    with open(os.path.join(bd, "qemu-x86_64"), "w") as f: f.write("enabled\ninterpreter /usr/bin/qemu-x86_64-static\n")
    check("native_host: x86_64 with a qemu-x86_64 handler refused", not measure.native_host("x86_64", bd)[0])
    check("native_host: a qemu-x86_64 handler does not refuse aarch64", measure.native_host("aarch64", bd)[0])
    with open(os.path.join(bd, "qemu-aarch64"), "w") as f: f.write("enabled\ninterpreter /usr/bin/qemu-aarch64-static\n")
    check("native_host: aarch64 with a qemu-aarch64 handler refused", not measure.native_host("aarch64", bd)[0])
    check("native_host: binfmt name alone (no qemu interpreter) does not refuse", (lambda: (open(os.path.join(bd, "qemu-aarch64"), "w").write("enabled\ninterpreter /usr/bin/other\n"), measure.native_host("aarch64", bd)[0])[1])())
    missing = os.path.join(bd, "no-such-dir")
    check("native_host: unreadable binfmt dir fails closed under strict (gate)", not measure.native_host("x86_64", missing, strict=True)[0])
    check("native_host: unreadable binfmt dir only records native when not strict", measure.native_host("x86_64", missing)[0])
    with tempfile.TemporaryDirectory() as empty:
        check("native_host: readable (empty) binfmt dir passes under strict", measure.native_host("x86_64", empty, strict=True)[0])

GOOD_ID = "commit=" + "a" * 40 + " cargo-lock=" + "b" * 64

def gate_run(extra_env=None, scenarios=("g4a",), peer=STANDIN, version=None):
    """In-process --gate run of the stand-in as a bench build with identity and host checks satisfied by patches
    (a stand-in cannot be an ELF). Exercises the real gating verdict/exit-code paths, whatever the host arch."""
    saved = (measure.native_host, measure.check_identity, measure.version_of, dict(measure.PINNED_ENV))
    measure.native_host = lambda *a, **k: (True, "ok")
    measure.check_identity = lambda *a, **k: None
    measure.version_of = version or (lambda *a, **k: "fetch-mcp 0.0.0 " + GOOD_ID + " bench-loopback")
    measure.PINNED_ENV.update(extra_env or {})
    buf = io.StringIO()
    try:
        with tempfile.TemporaryDirectory() as gd, contextlib.redirect_stdout(buf):
            fixtures.generate(gd)
            rc = measure.main(["--binary", STANDIN, "--binary-kind", "bench", *[x for sc in scenarios for x in ("--scenario", sc)], "--gate", "--fixtures-dir", gd, *(["--peer-binary", peer] if peer else [])])
    finally:
        measure.native_host, measure.check_identity, measure.version_of = saved[:3]
        measure.PINNED_ENV.clear(); measure.PINNED_ENV.update(saved[3])
    return rc, [json.loads(l) for l in buf.getvalue().splitlines() if l.startswith("{")]

rc, r = gate_run()
check("gate mode: complete G4a set under budget -> summary PASS (not ADVISORY_PASS), exit 0, gating recorded",
      rc == 0 and r[-1]["verdict"] == "PASS" and r[-1]["gating"] is True, f"rc={rc} {r[-1].get('verdict')} {r[-1].get('missed')} {r[-1].get('incomplete')}")
rc, r = gate_run({"STANDIN_ALLOC_MIB": "60"})
check("gate mode: peak over 40 MiB -> FAIL, exit 1", rc == 1 and r[-1]["verdict"] == "FAIL", f"rc={rc} {r[-1].get('verdict')}")
rc, r = gate_run({"STANDIN_EARLY_STOP": "1000"})
check("gate mode: early-stopping server -> INVALID, exit 2", rc == 2 and r[-1]["verdict"] == "INVALID", f"rc={rc} {r[-1].get('verdict')}")

# --- E-4: build identity (commit and Cargo.lock hash in --version) and the peer-binary rule under --gate
ID2 = "commit=" + "c" * 40 + " cargo-lock=" + "b" * 64
check("build identity: parsed from a --version line", measure.build_identity("fetch-mcp 0.0.0 " + GOOD_ID + " bench-loopback") == ("a" * 40, "b" * 64))
check("build identity: equal pairs pass", measure.check_build_identity("fetch-mcp 0.0.0 " + GOOD_ID, "fetch-mcp 0.0.0 " + GOOD_ID + " bench-loopback") is None)
check("build identity: differing commit refused", "differ" in (measure.check_build_identity("fetch-mcp 0.0.0 " + GOOD_ID, "fetch-mcp 0.0.0 " + ID2) or ""))
check("build identity: differing Cargo.lock hash refused", "differ" in (measure.check_build_identity("fetch-mcp 0.0.0 " + GOOD_ID, "fetch-mcp 0.0.0 commit=" + "a" * 40 + " cargo-lock=" + "d" * 64) or ""))
check("build identity: missing identity refused (old binary or stand-in)", measure.check_build_identity("fetch-mcp 0.0.0", "fetch-mcp 0.0.0 " + GOOD_ID) is not None)
check("build identity: a -dirty commit is refused with the reason", "modified working tree" in (measure.check_build_identity("fetch-mcp 0.0.0 commit=" + "a" * 40 + "-dirty cargo-lock=" + "b" * 64, "fetch-mcp 0.0.0 " + GOOD_ID) or ""))
check("build identity: unknown commit refused", "unknown" in (measure.check_build_identity("fetch-mcp 0.0.0 commit=unknown cargo-lock=" + "b" * 64, "fetch-mcp 0.0.0 commit=unknown cargo-lock=" + "b" * 64) or ""))
rc, r = gate_run(peer=None)
check("gate mode: no --peer-binary -> REFUSED, exit 2", rc == 2 and r[-1]["verdict"] == "REFUSED" and "--peer-binary" in r[-1]["reason"], f"rc={rc} {r[-1]}")
n = [0]
def alternating(*a, **k):   # measured binary and peer disagree on the commit
    n[0] += 1
    return "fetch-mcp 0.0.0 " + (GOOD_ID if n[0] == 1 else ID2) + " bench-loopback"
rc, r = gate_run(version=alternating)
check("gate mode: bench and shipped report different commits -> REFUSED, exit 2", rc == 2 and r[-1]["verdict"] == "REFUSED" and "differ" in r[-1]["reason"], f"rc={rc} {r[-1]}")
rc, r = gate_run(scenarios=("g4a", "g6-concurrent10"))
g6 = next((x for x in r if x.get("scenario") == "g6-concurrent10"), {})
check("g6-concurrent10: 10 concurrent calls, 10 valid runs, verdict RECORDED (never PASS/FAIL), outside the gating peak, gate run still PASS",
      rc == 0 and g6.get("verdict") == "RECORDED" and g6.get("valid_runs") == 10 and g6.get("gate") == "none" and r[-1]["verdict"] == "PASS", f"rc={rc} {g6.get('verdict')} {g6.get('invalid_reasons')}")
rc, r = gate_run({"STANDIN_ALLOC_MIB": "60"}, scenarios=("g4a", "g6-concurrent10"))
check("g6-concurrent10: a huge peak is recorded, not a miss (missed lists only gating scenarios)", "g6-concurrent10" not in r[-1].get("missed", []) and "g6-concurrent10" not in r[-1].get("invalid", []), str(r[-1].get("missed")))
# Fix-pass 1: hostile-HTML peaks (3 concurrent), recorded like g6, still fail closed on validity
rc, r = gate_run(scenarios=("g4a", "hostile-attrs3", "hostile-attrvalue3"))
ha = next((x for x in r if x.get("scenario") == "hostile-attrs3"), {})
hv = next((x for x in r if x.get("scenario") == "hostile-attrvalue3"), {})
check("hostile-attrs3 / hostile-attrvalue3: 3 concurrent calls, 10 valid runs each, verdict RECORDED, outside the gating peak, gate run still PASS",
      rc == 0 and ha.get("verdict") == "RECORDED" and hv.get("verdict") == "RECORDED" and ha.get("valid_runs") == 10 and hv.get("valid_runs") == 10
      and ha.get("gate") == "none" and r[-1]["verdict"] == "PASS", f"rc={rc} {ha.get('verdict')} {ha.get('invalid_reasons')} {hv.get('verdict')} {hv.get('invalid_reasons')}")
rc, r = gate_run({"STANDIN_NO_HOSTILE_REFUSAL": "1"}, scenarios=("g4a", "hostile-attrs3"))
check("hostile-attrs3: a server that converts the attribute bomb instead of refusing it is INVALID (unexpected outcome), exit 2",
      rc == 2 and "hostile-attrs3" in r[-1].get("invalid", []), f"rc={rc} {r[-1].get('invalid')}")
rc, r = gate_run({"STANDIN_ALLOC_MIB": "60"}, scenarios=("g4a", "hostile-attrvalue3"))
check("hostile-attrvalue3: a huge peak is recorded, not a miss", "hostile-attrvalue3" not in r[-1].get("missed", []) and "hostile-attrvalue3" not in r[-1].get("invalid", []), str(r[-1].get("missed")))
check("fixtures: hostile bodies are deterministic, sized just under 2 MiB, and shaped as documented",
      fixtures.hostile_body("attrs") == fixtures.hostile_body("attrs") and len(fixtures.hostile_body("attrs")) in range(fixtures.HOSTILE_SIZE - 4, fixtures.HOSTILE_SIZE + 1)
      and len(fixtures.hostile_body("attrvalue")) == fixtures.ATTRVALUE_SIZE and fixtures.ATTRVALUE_SIZE <= 0.95 * 2 * 1024 * 1024 and fixtures.hostile_body("attrs").count(b" a") > 900_000)
for g in ("g4b-window-start", "g4b-window-end", "g4b-raw", "g4b-chunked-window-in-cap", "g4b-window-beyond-cap"):
    with tempfile.TemporaryDirectory() as gd:
        fixtures.generate(gd)
        rc, r = run("--fixtures-dir", gd, "--binary-kind", "bench", "--child-env", "STANDIN_BENCH=1", "--scenario", g)
    check(f"G4b scaffold {g}: defined, refused as not implemented (needs A-5/A-6, Sprint 5), exit 2", rc == 2 and "not implemented" in r[-1].get("reason", ""), f"rc={rc} {r[-1]}")

# --- redirect-chain scenario (recorded, never in the gating peak) and the E-2 fixtures
with tempfile.TemporaryDirectory() as rd:
    fixtures.generate(rd)
    rc, r = run("--fixtures-dir", rd, "--binary-kind", "bench", "--child-env", "STANDIN_BENCH=1", "--scenario", "redirect-chain5", "--scenario", "g4a-late-landmark")
    rec = scen(r, "redirect-chain5")
    check("redirect-chain5: 5 hops followed, 10 valid runs, recorded outside the gating peak figure, but its own target and validity still count",
          rec["valid_runs"] == 10 and rec["gate"] == "none" and "gating_peak_MiB" in r[-1] and scen(r, "g4a-late-landmark")["valid_runs"] == 10, f"rc={rc} {rec['invalid_reasons']}")
    rc, r = run("--fixtures-dir", rd, "--binary-kind", "bench", "--child-env", "STANDIN_BENCH=1", "--scenario", "redirect-chain5", "--child-env", "STANDIN_NO_REDIRECT=1")
    check("redirect-chain5: a client that does not follow the redirects is INVALID (chain not followed), not a pass", rc == 2 and r[-1]["verdict"] == "INVALID", f"rc={rc}")
    check("redirect-chain5: INVALID counts in the summary (intended: a gate=none scenario still fails closed on validity)", "redirect-chain5" in r[-1]["invalid"], str(r[-1].get("invalid")))

# --- report.py tolerates a truncated / malformed JSONL (carry-forward): skips the bad line, warns, keeps the good ones
with tempfile.TemporaryDirectory() as td:
    good = json.dumps({"kind": "scenario", "scenario": "idle", "binary_kind": "shipped", "metric": "VmRSS", "valid_runs": 10, "runs": 10,
                       "median_kB": 3000, "median_MiB": 2.93, "samples_kB": [3000], "target_kB": 10240, "verdict": "PASS"})
    jp = os.path.join(td, "idle.jsonl")
    with open(jp, "w") as f: f.write("{\"kind\": \"scenario\", \"scen\n" + good + "\n{not json}\nplain text line\n" + good[:20])
    p = subprocess.run([sys.executable, os.path.join(HERE, "report.py"), "--idle", jp], capture_output=True, text=True)
    check("report.py: malformed JSONL lines are skipped with a warning, valid ones reported, exit 0",
          p.returncode == 0 and len(set(p.stderr.splitlines())) == 3 and p.stderr.count("skipped a malformed JSONL line") >= 3 and "| idle | shipped |" in p.stdout, f"rc={p.returncode} {p.stderr!r}")
    p = subprocess.run([sys.executable, os.path.join(HERE, "report.py"), "--idle", os.path.join(td, "missing.jsonl")], capture_output=True, text=True)
    check("report.py: a missing file yields an empty report, exit 0", p.returncode == 0 and "| scenario |" in p.stdout, p.stderr)

print("SELFTEST", "FAILED: " + ", ".join(fails) if fails else "PASSED")
sys.exit(1 if fails else 0)
