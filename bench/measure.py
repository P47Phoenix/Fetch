#!/usr/bin/env python3
"""Benchmark harness: drive an MCP server binary over stdio, read /proc/<pid>/status, emit JSONL.

  measure.py --binary PATH --binary-kind shipped|bench --scenario idle [--scenario g4a ...] [--runs 10]

Scenarios: see scenarios.py (`g4a`/`g4b` expand to groups). Idle = VmRSS after initialize + tools/list + settle
(default 30 s). Peak = VmHWM after one `fetch` returned. Every sample is a fresh process.
Exit codes: 0 all targets met and report VALID; 1 a target missed; 2 INVALID, INCOMPLETE or refused (fewer than 10 valid
runs, fixture hash mismatch, wrong binary kind or missing/non-executable binary, unimplemented scenario, --gate override or
incomplete gate set, a boundedness scenario without its 5 MiB reference); 3 --gate requested but host is not native aarch64
(checked after every other --gate rule, so those refusals are exercisable anywhere).
Without --gate the run is advisory (record has gating=false). MB = MiB (2**20). Linux only here (macOS: E-2).
"""
import argparse, concurrent.futures as cf, json, os, platform, queue, statistics, subprocess, sys, threading, time
import fixtures, serve
from scenarios import SCENARIOS, GROUPS

MIB_KB = 1024                      # kB per MiB: /proc reports kB (KiB)
IDLE_TARGET_MIB, PEAK_TARGET_MIB, BOUND_RATIO = 10, 40, 1.10
PINNED_ENV = {"FETCH_LOG": "warn", "LC_ALL": "C"}   # plus PATH; RUST_LOG, LD_PRELOAD, MALLOC_* deliberately absent
GATE_CHILD_ENV_ALLOW = frozenset()   # --gate allowlist for --child-env keys: none. The pinned env is the whole environment.
SHIPPED_FORBIDDEN_MARKERS = ("bench-loopback", "test-support", "FETCH_MCP_MARKER_")  # any of these in --version => not "shipped"


def required_gate_set(kind, names):
    """Scenarios a --gate run of this binary kind must contain (architecture 11.0): shipped -> idle; bench -> every
    scenario of each gate group (G4a/G4b) the run touches, or the whole G4a group if it touches neither."""
    if kind == "shipped": return ["idle"]
    groups = [g for g in ("g4a", "g4b") if any(n in GROUPS[g] for n in names)] or ["g4a"]
    return [n for g in groups for n in GROUPS[g]]


def proc_status(pid):
    d = {}
    with open(f"/proc/{pid}/status") as f:
        for l in f:
            if l.startswith(("VmRSS", "VmHWM")):
                d[l.split(":")[0]] = int(l.split()[1])
    return d


def read_file(p, default="unknown"):
    try:
        with open(p) as f: return f.read().strip()
    except OSError:
        return default


def host_record():
    mem = next((l.split()[1] for l in read_file("/proc/meminfo", "").splitlines() if l.startswith("MemTotal")), "unknown")
    cpu = next((l.split(":", 1)[1].strip() for l in read_file("/proc/cpuinfo", "").splitlines()
                if l.startswith(("model name", "Model", "Hardware"))), "unknown")
    return {"kind": "host", "machine": platform.machine(), "kernel": platform.release(), "cpu": cpu,
            "mem_total_kB": mem, "pagesize": os.sysconf("SC_PAGE_SIZE"),
            "thp": read_file("/sys/kernel/mm/transparent_hugepage/enabled"),
            "governor": read_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            "loadavg_before": os.getloadavg(), "cgroup_mem_max": read_file("/sys/fs/cgroup/memory.max"),
            "container": os.path.exists("/.dockerenv") or os.path.exists("/run/.containerenv"),
            "os": read_file("/etc/os-release", "").split("PRETTY_NAME=")[-1].split("\n")[0].strip('"'),
            "runner": "PLACEHOLDER: author's native aarch64 runner (name/OS/RAM recorded in docs/BENCHMARK.md when known)"}


def native_aarch64():
    """Preflight (architecture 11.2): aarch64, no qemu binfmt handler for aarch64. Returns (ok, reason)."""
    if platform.machine() != "aarch64": return False, f"machine is {platform.machine()}, not aarch64"
    try:
        for n in os.listdir("/proc/sys/fs/binfmt_misc"):
            if "aarch64" in n and "qemu" in read_file(f"/proc/sys/fs/binfmt_misc/{n}", ""): return False, "qemu aarch64 binfmt registered"
    except OSError:
        pass
    return True, "ok"


class Server:
    """One fresh child process plus a stdout reader thread (so a hung server times out instead of blocking)."""
    def __init__(self, binary, env_extra):
        env = {"PATH": os.environ.get("PATH", ""), **PINNED_ENV, **env_extra}
        self.p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l) for l in self.p.stdout] or self.q.put(None), daemon=True).start()
        self._id = 0

    def send(self, method, params=None, notify=False):
        m = {"jsonrpc": "2.0", "method": method, **({"params": params} if params is not None else {})}
        if not notify:
            self._id += 1; m["id"] = self._id
        self.p.stdin.write((json.dumps(m) + "\n").encode()); self.p.stdin.flush()
        return None if notify else self._id

    def call(self, method, params=None, timeout=120):
        want = self.send(method, params)
        end = time.time() + timeout
        while True:
            line = self.q.get(timeout=max(0.1, end - time.time()))
            if line is None: raise RuntimeError("server closed stdout")
            m = json.loads(line)
            if m.get("id") == want: return m

    def handshake(self):
        self.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "bench", "version": "0"}}, 30)
        self.send("notifications/initialized", notify=True)
        self.call("tools/list", timeout=30)

    def close(self):
        try: self.p.stdin.close(); self.p.wait(timeout=5)
        except Exception: self.p.kill()


def sample(binary, env_extra, scen, srv, base_url, settle):
    """One fresh-process sample -> dict(valid, reason, rss_kB, hwm_kB)."""
    s = None
    try:
        s = Server(binary, env_extra)
        s.handshake()
        if scen["kind"] == "idle":
            time.sleep(settle)
            st = proc_status(s.p.pid)
            return {"valid": True, "rss_kB": st["VmRSS"], "hwm_kB": st["VmHWM"]}
        srv.reset()
        r = s.call("tools/call", {"name": "fetch", "arguments": {"url": base_url + scen["route"], **scen.get("args", {})}})
        st = proc_status(s.p.pid)  # VmHWM read after the call returned, before exit
        res = r.get("result", {})
        text = json.dumps(res)
        got_err = bool(res.get("isError"))
        # too_large must be a tool error whose content text (not any field) says so.
        err_text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
        outcome_ok = (got_err and "too_large" in err_text) if scen["expect"] == "too_large" else (not got_err and "result" in r)
        sent, need = srv.bytes_sent(scen["route"]), scen["min_bytes_v"]
        reason = None if outcome_ok else f"unexpected outcome (expected {scen['expect']})"
        reason = reason or (None if sent >= need else f"early stop: server wrote {sent} < expected_min_bytes {need}")
        return {"valid": reason is None, "reason": reason, "rss_kB": st["VmRSS"], "hwm_kB": st["VmHWM"], "bytes_sent": sent}
    except Exception as e:  # any handshake/protocol failure is an invalid sample, never silently dropped
        return {"valid": False, "reason": f"{type(e).__name__}: {e}", "rss_kB": 0, "hwm_kB": 0}
    finally:
        if s: s.close()


def version_of(binary, env_extra):
    try:
        return subprocess.run([binary, "--version"], capture_output=True, text=True, timeout=10,
                              env={"PATH": os.environ.get("PATH", ""), **PINNED_ENV, **env_extra}).stdout.strip()
    except Exception as e:
        return f"unavailable: {e}"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--binary-kind", required=True, choices=["shipped", "bench"])
    ap.add_argument("--scenario", action="append", required=True)
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--settle", type=float, default=30.0, help="idle settle seconds after tools/list (default 30)")
    ap.add_argument("--parallel-idle", type=int, default=1, help="idle samples may run in parallel processes")
    ap.add_argument("--fixtures-dir", default=os.path.join(fixtures.HERE, "fixtures"))
    ap.add_argument("--out", help="also append JSONL to this file")
    ap.add_argument("--child-env", action="append", default=[], help="KEY=VAL added to the pinned child env (recorded)")
    ap.add_argument("--gate", action="store_true", help="produce gating numbers: needs native aarch64, forbids overrides and --smoke")
    ap.add_argument("--smoke", action="store_true", help="allow <10 runs; result is never valid for NFR claims")
    ap.add_argument("--idle-target-mib", type=float, default=IDLE_TARGET_MIB, help="diagnostic override (not with --gate)")
    ap.add_argument("--peak-target-mib", type=float, default=PEAK_TARGET_MIB, help="diagnostic override (not with --gate)")
    a = ap.parse_args(argv)
    lines = []
    def emit(o):
        s = json.dumps(o, default=str); print(s, flush=True); lines.append(s)
    def finish(code):
        if a.out:
            with open(a.out, "a") as f: f.write("\n".join(lines) + "\n")
        return code
    def refuse(msg, code=2):
        emit({"kind": "summary", "verdict": "REFUSED" if code != 1 else "FAIL", "reason": msg}); return finish(code)

    names = [n for s in a.scenario for n in GROUPS.get(s, [s])]
    unknown = [n for n in names if n not in SCENARIOS]
    if unknown: return refuse(f"unknown scenario {unknown}")
    if a.gate:
        if a.smoke or (a.idle_target_mib, a.peak_target_mib) != (IDLE_TARGET_MIB, PEAK_TARGET_MIB):
            return refuse("--gate forbids --smoke and target overrides")
        if a.settle != 30.0 or a.parallel_idle != 1:
            return refuse("--gate forbids --settle other than 30 and --parallel-idle > 1")
        missing = [n for n in required_gate_set(a.binary_kind, names) if n not in names]
        if missing: return refuse(f"--gate incomplete: required gate scenarios missing from the run: {missing}")
        forbidden = [kv for kv in a.child_env if kv.split("=", 1)[0] not in GATE_CHILD_ENV_ALLOW]
        if forbidden: return refuse(f"--gate forbids any child env override (allowlist is empty): {forbidden}")
        ok, why = native_aarch64()
        if not ok: return refuse(f"--gate refused: {why} (QEMU/non-native RSS never gates)", 3)
    if a.runs < 10 and not a.smoke: return refuse(f"--runs {a.runs} < 10 valid runs required (use --smoke for a non-gating check)")
    for n in names:
        if not SCENARIOS[n]["implemented"]: return refuse(f"scenario {n} not implemented yet (needs E-2 fixtures/args or A-5/A-6)")
    env_extra = dict(kv.split("=", 1) for kv in a.child_env)

    bad = fixtures.verify(a.fixtures_dir)
    if bad: return refuse("fixture hash mismatch: " + "; ".join(bad))
    manifest = fixtures.load_manifest()

    if not (os.path.isfile(a.binary) and os.access(a.binary, os.X_OK)): return refuse(f"binary not found or not executable: {a.binary}")
    ver = version_of(a.binary, env_extra)
    marker = "bench-loopback" in ver
    if a.binary_kind == "bench" and not marker: return refuse("--binary-kind bench but --version lacks the bench-loopback marker")
    carried = [m for m in SHIPPED_FORBIDDEN_MARKERS if m in ver]
    if a.binary_kind == "shipped" and carried: return refuse(f"--binary-kind shipped but --version carries a forbidden-feature marker: {carried}")
    for n in names:
        need = SCENARIOS[n].get("binary", "bench")
        if need != a.binary_kind: return refuse(f"scenario {n} is gated on the {need} binary, got {a.binary_kind}")

    host = host_record(); host["native_aarch64"] = native_aarch64()[0]
    emit({**host, "binary": a.binary, "binary_kind": a.binary_kind, "version": ver, "binary_sha256": fixtures.sha256_file(a.binary),
          "child_env": {**PINNED_ENV, **env_extra}, "runs": a.runs, "settle_s": a.settle, "gating": a.gate,
          "fixture_sha256": {k: v["sha256"] for k, v in manifest["fixtures"].items()}, "unit": "MiB=2^20 bytes"})

    srv = serve.FixtureServer(a.fixtures_dir, manifest).start()
    base = f"http://127.0.0.1:{srv.port}"
    medians, invalid, missed, incomplete = {}, [], [], []
    try:
        for n in names:
            scen = dict(SCENARIOS[n]); scen["min_bytes_v"] = scen["min_bytes"](manifest) if "min_bytes" in scen else 0
            if scen["kind"] == "idle" and a.parallel_idle > 1:
                with cf.ThreadPoolExecutor(a.parallel_idle) as ex:
                    ss = list(ex.map(lambda _: sample(a.binary, env_extra, scen, srv, base, a.settle), range(a.runs)))
            else:
                ss = [sample(a.binary, env_extra, scen, srv, base, a.settle) for _ in range(a.runs)]
            good = [x for x in ss if x["valid"]]
            key = "rss_kB" if scen["kind"] == "idle" else "hwm_kB"
            vals = [x[key] for x in good]
            target_kb = (a.idle_target_mib if scen["kind"] == "idle" else a.peak_target_mib) * MIB_KB
            rec = {"kind": "scenario", "scenario": n, "gate": scen["gate"], "metric": "VmRSS" if key == "rss_kB" else "VmHWM",
                   "binary_kind": a.binary_kind, "runs": a.runs, "valid_runs": len(good), "samples_kB": vals,
                   "invalid_reasons": sorted({x["reason"] for x in ss if not x["valid"]}), "target_kB": target_kb}
            if len(good) < 10 and not a.smoke:
                rec["verdict"] = "INVALID"; invalid.append(n)
            elif vals:
                med = statistics.median(vals); medians[n] = med
                rec.update(median_kB=med, min_kB=min(vals), max_kB=max(vals), median_MiB=round(med / MIB_KB, 2),
                           verdict="PASS" if med <= target_kb else "FAIL")
                if scen.get("kind") == "peak" and rec["verdict"] == "FAIL": missed.append(n)
                if scen["kind"] == "idle" and rec["verdict"] == "FAIL": missed.append(n)
            else:
                rec["verdict"] = "INVALID"; invalid.append(n)
            emit(rec)
    finally:
        srv.stop()

    # Gate aggregates: max of per-scenario medians (peak) and 50 MiB boundedness (<= 1.10 x the 5 MiB run).
    summ = {"kind": "summary", "gating": a.gate, "binary_kind": a.binary_kind, "targets_MiB": {"idle": a.idle_target_mib, "peak": a.peak_target_mib}}
    peaks = {n: m for n, m in medians.items() if SCENARIOS[n]["kind"] == "peak" and SCENARIOS[n]["gate"] != "none"}
    if peaks: summ["gating_peak_kB"] = max(peaks.values()); summ["gating_peak_MiB"] = round(max(peaks.values()) / MIB_KB, 2)
    for n in names:
        ref = SCENARIOS[n].get("bounded_vs")
        if not ref or n not in medians: continue
        if ref not in medians:   # reference absent or invalid: boundedness unchecked, so never a PASS
            incomplete.append(f"{n} (boundedness reference {ref} has no valid result)"); continue
        ratio = medians[n] / medians[ref]; summ.setdefault("boundedness", {})[n] = round(ratio, 3)
        if ratio > BOUND_RATIO: missed.append(n + " (boundedness)")
    summ["missed"] = missed; summ["invalid"] = invalid; summ["incomplete"] = incomplete
    # A pass without --gate is advisory and must not read as a gate PASS.
    summ["verdict"] = "INVALID" if invalid else ("FAIL" if missed else ("INCOMPLETE" if incomplete else ("PASS" if a.gate else "ADVISORY_PASS")))
    summ["note"] = "" if a.gate else "advisory: not a gating run (no --gate); never valid for NFR claims"
    emit(summ)
    return finish(2 if invalid else 1 if missed else 2 if incomplete else 0)


if __name__ == "__main__":
    sys.exit(main())
