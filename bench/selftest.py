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
    import measure, io, contextlib
    from scenarios import SCENARIOS
    SCENARIOS["g4b-raw"]["implemented"] = False   # G4b is implemented since Sprint 5; a scenario flagged unimplemented is still refused
    try:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = measure.main(["--binary", STANDIN, *F, *bk, "--scenario", "g4b-raw"])
    finally:
        SCENARIOS["g4b-raw"]["implemented"] = True
    check("unimplemented scenario refused", rc == 2 and "not implemented" in buf.getvalue(), f"rc={rc}")
    rc, _ = run(*F, "--binary-kind", "shipped", "--scenario", "idle", "--child-env", "NOEQUALS", "--smoke", "--runs", "1")
    check("malformed --child-env is a refusal (exit 2), not a crash-as-'target missed' (NB-1)", rc == 2, f"rc={rc}")
    floor = SCENARIOS["g4b-raw"].pop("min_bytes")   # NB-2: an implemented peak scenario without min_bytes must be refused
    try:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = measure.main(["--binary", STANDIN, *F, *bk, "--scenario", "g4b-raw"])
    finally:
        SCENARIOS["g4b-raw"]["min_bytes"] = floor
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
# --- G4b (A-5, A-6): the window scenarios are real. Window resolution is a pure function of the probed lengths.
from scenarios import WINDOW, CAP
man = fixtures.load_manifest()   # from the committed manifest, not the disk: the unit-test job has no generated fixtures
for _m in man["fixtures"].values():
    if _m.get("encoding") == "gzip": _m["size"] = _m["compressed_size_reference"]
end = measure.resolve_window(dict(SCENARIOS["g4b-window-end"]), lambda route, raw: 1_000_000, man)
check("resolve_window end: last WINDOW characters, no continuation footer allowed, total footer required",
      end["args"]["start_index"] == 1_000_000 - WINDOW and end["args"]["max_length"] == WINDOW and "More content available" in end["text_must_not"]
      and "[Total length: 1000000 characters.]" in end["text_must"], str(end))
short = measure.resolve_window(dict(SCENARIOS["g4b-window-end"]), lambda route, raw: 50, man)
check("resolve_window end: a page shorter than the window starts at 0 and needs no total footer", short["args"]["start_index"] == 0 and "text_must" not in short)
st = measure.resolve_window(dict(SCENARIOS["g4b-window-start"]), lambda route, raw: 1 / 0, man)
check("resolve_window start: start 0, needs the continuation footer, no probe", st["args"]["start_index"] == 0 and st["text_must"] == ["More content available"])
inc = measure.resolve_window(dict(SCENARIOS["g4b-chunked-window-in-cap"]), lambda route, raw: 4_000_000 if route == "/5mb.html" else 1 / 0, man)
check("resolve_window in-cap: window ends 200,000 characters before the end of the 5 MiB page's output, floor is start + WINDOW",
      inc["args"]["start_index"] == 4_000_000 - 3 * WINDOW and inc["min_bytes"](man) == 4_000_000 - 2 * WINDOW, str(inc["args"]))
bc = measure.resolve_window(dict(SCENARIOS["g4b-window-beyond-cap"]), lambda route, raw: 1 / 0, man)
check("resolve_window beyond-cap: start_index is the cap in characters", bc["args"]["start_index"] == CAP)
try:
    measure.resolve_window(dict(SCENARIOS["g4b-raw"]), lambda route, raw: 123, man); raw_ok = False
except RuntimeError:
    raw_ok = True
check("resolve_window raw: a probe that disagrees with the fixture size is refused (independent check of the raw total)", raw_ok)
rc, r = gate_run(scenarios=("g4a", "g4b"))
g4b = [x for x in r if x.get("gate") == "G4b"]
check("G4b: complete set (window at start, at end, raw, chunked window in cap, window beyond cap) -> 5 scenarios x 10 valid runs, gate PASS, exit 0",
      rc == 0 and len(g4b) == 5 and all(x["valid_runs"] == 10 and x["verdict"] == "PASS" for x in g4b) and r[-1]["verdict"] == "PASS",
      f"rc={rc} {[(x['scenario'], x['verdict'], x['invalid_reasons']) for x in g4b]} {r[-1].get('missed')} {r[-1].get('invalid')}")
check("G4b: the resolved window is recorded per scenario", all(x.get("window") and "start_index" in x["window"] for x in g4b))
check("G4b: both 50 MiB chunked cases carry a boundedness ratio against the 5 MiB window-at-end peak",
      set(r[-1].get("boundedness", {})) >= {"g4b-chunked-window-in-cap", "g4b-window-beyond-cap"}, str(r[-1].get("boundedness")))
rc, r = gate_run(scenarios=("g4b",))
check("G4b alone is a complete gate set (its bounded cases reference G1, which is in the group), exit 0", rc == 0 and r[-1]["verdict"] == "PASS", f"rc={rc} {r[-1]}")
rc, r = gate_run({"STANDIN_TOOLARGE_ALLOC_MIB": "30"}, scenarios=("g4a", "g4b"))
check("G4b: a 50 MiB chunked case that blows up beyond 1.10 x the 5 MiB peak -> FAIL naming it, exit 1",
      rc == 1 and r[-1]["verdict"] == "FAIL" and "g4b-window-beyond-cap (boundedness)" in r[-1]["missed"], f"rc={rc} {r[-1].get('missed')}")
rc, r = gate_run({"STANDIN_ALLOC_MIB": "60"}, scenarios=("g4b",))
check("G4b: a peak over 40 MiB -> FAIL, exit 1", rc == 1 and r[-1]["verdict"] == "FAIL", f"rc={rc} {r[-1].get('verdict')}")
rc, r = gate_run({"STANDIN_EARLY_STOP": "1000"}, scenarios=("g4b",))
check("G4b: a server that stops reading early on the window-at-end scenarios -> INVALID, exit 2", rc == 2 and r[-1]["verdict"] == "INVALID", f"rc={rc} {r[-1].get('verdict')}")
rc, r = gate_run({"STANDIN_NO_TOTAL": "1"}, scenarios=("g4b",))
check("G4b: a reply that does not state the total (probe fails) -> INVALID scenarios, exit 2, never a default window",
      rc == 2 and r[-1]["verdict"] == "INVALID" and any("length probe" in " ".join(x.get("invalid_reasons", [])) for x in r if x.get("kind") == "scenario"), f"rc={rc} {r[-1].get('invalid')}")

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

# --- E-5 smoke runner and report generator (no owner list exists: everything here uses local fixtures and synthetic records)
import http.server, threading
class _H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/old": self.send_response(302); self.send_header("Location", "/data.json"); self.send_header("Content-Length", "0"); self.end_headers(); return
        body = b'{"a": 1}' if self.path.endswith(".json") else b"plain text page\n"
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
_srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _H); threading.Thread(target=_srv.serve_forever, daemon=True).start()
_base = f"http://127.0.0.1:{_srv.server_address[1]}"
def _smoke(lines, *extra):
    with tempfile.TemporaryDirectory() as td:
        lp = os.path.join(td, "list.txt")
        with open(lp, "w") as f: f.write("\n".join(lines) + "\n")
        p = subprocess.run([sys.executable, os.path.join(HERE, "smoke.py"), "--binary", STANDIN, "--list", lp, *extra], capture_output=True, text=True)
        return p.returncode, [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
rc, r = _smoke(["# local fixtures", f"{_base}/old redirect", f"{_base}/data.json json", f"{_base}/page.txt text"], "--dry-run")
check("smoke.py --dry-run on local fixtures: 3 fetched, labelled dry_run, exit 0", rc == 0 and r[-1]["verdict"] == "DONE" and r[-1]["dry_run"] and r[-1]["ok"] == 3 and r[-1]["list_count"] == 3, f"rc={rc} {r[-1:]}")
rc, r = _smoke([f"{_base}/data.json json", f"{_base}/page.txt text"])
check("smoke.py real run refuses a list that is not 10 https URLs with all categories (never invents URLs), exit 2", rc == 2 and r[-1]["verdict"] == "INVALID" and "exactly 10" in r[-1]["reason"] and "https" in r[-1]["reason"], f"rc={rc} {r[-1:]}")
rc, r = _smoke([f"{_base}/x weird"], "--dry-run")
check("smoke.py: an unknown category is refused, exit 2", rc == 2 and "unknown category" in r[-1]["reason"], f"rc={rc}")
rc, r = _smoke(["https://user:pw@example.org/ tls"], "--dry-run")
check("smoke.py: userinfo in a list URL is refused", rc == 2 and "userinfo" in r[-1]["reason"], f"rc={rc}")
rc, r = _smoke([f"http://127.0.0.1:1/never json"], "--dry-run", "--timeout", "20")
check("smoke.py: a failing URL is a recorded result, not a crash (exit 0, failed=1)", rc == 0 and r[-1]["failed"] == 1, f"rc={rc} {r[-1:]}")

def _report(**files):
    with tempfile.TemporaryDirectory() as td:
        args = []
        for k, v in files.items():
            fp = os.path.join(td, k); open(fp, "w").write(v); args += ["--" + k.replace("_", "-"), fp]
        p = subprocess.run([sys.executable, os.path.join(HERE, "e5_report.py"), *args], capture_output=True, text=True)
        return p.returncode, p.stdout
def _gate(verdict, gating=True): return json.dumps({"kind": "summary", "gating": gating, "verdict": verdict, "missed": ["x"] if verdict == "FAIL" else []}) + "\n"
_ov = lambda ms: f"CONVERT_1MIB bytes=1048576 median_ms=50.0 p95_ms={ms} max_ms=90.0 arch=aarch64\n"
def _sm(dry=False, n=10): return json.dumps({"kind": "smoke-summary", "verdict": "DONE", "dry_run": dry, "list_count": n, "ok": n, "failed": 0, "failed_urls": [], "redirected": 1, "by_category": {"tls": [1, 1]}}) + "\n"
rc, out = _report()
check("e5_report: no inputs -> 'not decided', exit 2, lists what is missing (never a default pass)", rc == 2 and "**Decision: not decided**" in out and "no results file" in out, f"rc={rc}")
rc, out = _report(idle=_gate("PASS"), peak=_gate("PASS"), overhead=_ov(60.0), smoke=_sm(dry=True))
check("e5_report: a dry-run smoke is never the smoke result -> 'not decided', exit 2", rc == 2 and "dry run" in out, f"rc={rc}")
rc, out = _report(idle=_gate("PASS"), peak=_gate("PASS", gating=False), overhead=_ov(60.0), smoke=_sm())
check("e5_report: an advisory (non --gate) memory run is not evidence -> 'not decided', exit 2", rc == 2 and "advisory" in out, f"rc={rc}")
rc, out = _report(idle=_gate("PASS"), peak=_gate("PASS"), overhead=_ov(60.0), smoke=_sm())
check("e5_report: everything met and a real 10-URL smoke -> 'release' with the config change and the D-1 caveat, exit 0", rc == 0 and "**Decision: release**" in out and "claude mcp add" in out and "D-1" in out, f"rc={rc}")
rc, out = _report(idle=_gate("PASS"), peak=_gate("PASS"), overhead=_ov(700.0), smoke=_sm())
check("e5_report: a missed target -> 'do not release' with the gap, exit 1", rc == 1 and "**Decision: do not release**" in out and "Gap and follow-up" in out, f"rc={rc}")
rc, out = _report(idle=_gate("PASS"), peak=_gate("PASS"), overhead=_ov(60.0), smoke=_sm(n=9))
check("e5_report: a smoke over 9 URLs is not the 10-URL smoke -> 'not decided'", rc == 2 and "not 10" in out, f"rc={rc}")
_srv.shutdown()

print("SELFTEST", "FAILED: " + ", ".join(fails) if fails else "PASSED")
sys.exit(1 if fails else 0)
