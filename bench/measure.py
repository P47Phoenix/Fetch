#!/usr/bin/env python3
"""Benchmark harness: drive an MCP server binary over stdio, read /proc/<pid>/status, emit JSONL.

  measure.py --binary PATH --binary-kind shipped|bench --scenario idle [--scenario g4a ...] [--runs 10]

Scenarios: see scenarios.py (`g4a`/`g4b` expand to groups). Idle = VmRSS after initialize + tools/list + settle
(default 30 s). Peak = VmHWM after one `fetch` returned. Every sample is a fresh process.
Exit codes: 0 all targets met and report VALID; 1 a target missed; 2 INVALID, INCOMPLETE or refused (fewer than 10 valid
runs, fixture hash mismatch, wrong binary kind or missing/non-executable binary, unimplemented scenario, --gate override or
incomplete gate set, a boundedness scenario without its 5 MiB reference); 3 --gate requested but host is not a native aarch64 or x86_64 host (hosted arm64 / amd64 runners; QEMU never gates)
(checked after every other --gate rule, so those refusals are exercisable anywhere).
Without --gate the run is advisory (record has gating=false). Sizes are MiB (2**20). Linux only (macOS /usr/bin/time -l is not implemented: open gap).
"""
import argparse, concurrent.futures as cf, datetime, json, math, os, platform, queue, re, statistics, subprocess, sys, threading, time
import fixtures, serve
from scenarios import SCENARIOS, GROUPS, WINDOW, CAP

MIB_KB = 1024                      # kB per MiB: /proc reports kB (KiB)
IDLE_TARGET_MIB, PEAK_TARGET_MIB, BOUND_RATIO = 10, 40, 1.10
PINNED_ENV = {"FETCH_LOG": "warn", "LC_ALL": "C"}   # plus PATH; RUST_LOG, LD_PRELOAD, MALLOC_* deliberately absent
GATE_CHILD_ENV_ALLOW = frozenset()   # --gate allowlist for --child-env keys: none. The pinned env is the whole environment.
SHIPPED_FORBIDDEN_MARKERS = ("bench-loopback", "test-support", "FETCH_MCP_MARKER_")  # any of these in --version => not "shipped"


def utc_now():
    """ISO-8601 UTC wall-clock timestamp (timestamps only; every duration uses time.monotonic())."""
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def ms_since(t0): return round((time.monotonic() - t0) * 1000, 3)


TIMING_KEYS = ("ready_ms", "tools_list_ms", "first_byte_ms", "fetch_ms", "total_ms")


def timing_stats(samples):
    """Per-key {n, median, min, max[, p95 when n >= 20]} over VALID samples only. Recorded, never gated."""
    out = {}
    for k in TIMING_KEYS:
        v = sorted(x[k] for x in samples if x["valid"] and x.get(k) is not None)
        if not v: continue
        d = {"n": len(v), "median": round(statistics.median(v), 3), "min": v[0], "max": v[-1]}
        if len(v) >= 20: d["p95"] = v[math.ceil(0.95 * len(v)) - 1]   # nearest-rank
        out[k] = d
    return out


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
    if cpu == "unknown":   # arm64 /proc/cpuinfo has no model name; lscpu does
        try:
            out = subprocess.run(["lscpu"], capture_output=True, text=True, timeout=5, env={"LC_ALL": "C", "PATH": os.environ.get("PATH", "")}).stdout
            cpu = next((l.split(":", 1)[1].strip() for l in out.splitlines() if l.startswith("Model name")), "unknown")
        except (OSError, subprocess.SubprocessError):
            pass
    return {"kind": "host", "machine": platform.machine(), "kernel": platform.release(), "cpu": cpu,
            "mem_total_kB": mem, "pagesize": os.sysconf("SC_PAGE_SIZE"),
            "thp": read_file("/sys/kernel/mm/transparent_hugepage/enabled"),
            "governor": read_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            "loadavg_before": os.getloadavg(), "cgroup_mem_max": read_file("/sys/fs/cgroup/memory.max"),
            "container": os.path.exists("/.dockerenv") or os.path.exists("/run/.containerenv"),
            "os": read_file("/etc/os-release", "").split("PRETTY_NAME=")[-1].split("\n")[0].strip('"'),
            "runner": os.environ.get("RUNNER_NAME", "not a GitHub-hosted runner (RUNNER_NAME unset)"),
            "runner_image": f"{os.environ.get('ImageOS', 'unknown')} {os.environ.get('ImageVersion', 'unknown')}",
            "runner_arch": os.environ.get("RUNNER_ARCH", "unknown")}


ELF_MACHINE = {"x86_64": 62, "aarch64": 183}   # e_machine per platform.machine()
VERSION_PREFIX = "fetch-mcp "                    # the shipped/bench crate's --version starts with this
MARKER_PREFIX = b"FETCH_MCP_MARKER_"


def check_identity(binary, ver, kind, machine=None):
    """--gate binary identity (QA B2): an ELF of the host architecture whose --version starts `fetch-mcp `, and whose bytes
    agree with the claimed kind (shipped: no FETCH_MCP_MARKER_ token; bench: the bench-loopback marker). Returns None if OK
    else the refusal reason. A stand-in script, a wrong-arch ELF or a stripped-marker bench build is never a gate figure."""
    machine = machine or platform.machine()
    want = ELF_MACHINE.get(machine)
    if want is None: return f"no ELF machine mapping for host {machine}"
    try:
        with open(binary, "rb") as f: data = f.read()
    except OSError as e:
        return f"cannot read binary: {e}"
    if data[:4] != b"\x7fELF" or len(data) < 20: return "not an ELF executable (stand-in script or other file)"
    if data[5] not in (1, 2): return "ELF has an invalid byte-order field"   # EI_DATA: 1 little-endian, 2 big-endian
    got = int.from_bytes(data[18:20], "little" if data[5] == 1 else "big")
    if got != want: return f"ELF e_machine {got} is not the host architecture {machine} ({want})"
    if not ver.startswith(VERSION_PREFIX): return f"--version does not start with {VERSION_PREFIX!r} (stand-in or wrong program): {ver[:60]!r}"
    has_marker = MARKER_PREFIX in data
    if kind == "shipped" and has_marker: return "binary contains a FETCH_MCP_MARKER_ token: not a shipped build"
    if kind == "bench" and b"FETCH_MCP_MARKER_BENCH_LOOPBACK_V1" not in data: return "bench binary lacks the FETCH_MCP_MARKER_BENCH_LOOPBACK_V1 marker"
    return None


IDENTITY_RE = re.compile(r"\bcommit=([0-9a-f]{40}|unknown) cargo-lock=([0-9a-f]{64}|unknown)\b")


def build_identity(ver):
    """(commit, cargo_lock_sha256) from a `--version` line (E-4, re-homed from E-8), or None when the line carries neither."""
    m = IDENTITY_RE.search(ver)
    return (m.group(1), m.group(2)) if m else None


def check_build_identity(ver, peer_ver):
    """--gate rule (E-4): both the measured binary and its peer (the shipped build for a bench run and vice versa) must report a
    real commit and Cargo.lock hash in `--version`, and the two pairs must be equal, so the bench build is provably the
    same commit and lock file as the shipped one. Returns None if OK, else the refusal reason."""
    for v in (ver, peer_ver or ""):
        m = re.search(r"\bcommit=[0-9a-f]{40}-dirty\b", v)
        if m: return f"a build from a modified working tree ({m.group(0)}) never gates: rebuild from a clean checkout"
    mine, other = build_identity(ver), build_identity(peer_ver or "")
    if mine is None: return f"--version carries no commit=/cargo-lock= identity: {ver[:80]!r}"
    if other is None: return f"peer binary --version carries no commit=/cargo-lock= identity: {(peer_ver or '')[:80]!r}"
    if "unknown" in mine + other: return f"commit or Cargo.lock hash is unknown (build outside a git checkout?): {mine} / {other}"
    if mine != other: return f"shipped and bench builds differ: {mine} versus {other}"
    return None


GATE_MACHINES = ("aarch64", "x86_64")   # native gate hosts: ubuntu-24.04-arm and ubuntu-24.04 (ADR-007)


def native_host(machine=None, binfmt_dir="/proc/sys/fs/binfmt_misc", strict=False):
    """Preflight (architecture 11.2, ADR-007): the host is aarch64 or x86_64 and no qemu binfmt handler is registered for
    the host's own architecture (a registered handler means binaries of this arch may be emulated). Returns (ok, reason).
    strict=True (used on the --gate path) fails closed: an unreadable binfmt_misc directory cannot prove the host is native."""
    machine = machine or platform.machine()
    if machine not in GATE_MACHINES: return False, f"machine is {machine}, not one of {'/'.join(GATE_MACHINES)}"
    names = {"aarch64": ("aarch64",), "x86_64": ("x86_64", "x86-64")}[machine]
    try:
        for n in os.listdir(binfmt_dir):
            if any(a in n for a in names) and "qemu" in read_file(f"{binfmt_dir}/{n}", ""): return False, f"qemu {machine} binfmt registered"
    except OSError as e:
        if strict: return False, f"cannot read {binfmt_dir} ({e.__class__.__name__}), so native execution is unproven"
    return True, "ok"


def native_aarch64():
    """Kept for callers that only accept aarch64 (records host["native_aarch64"])."""
    ok, why = native_host()
    return (ok and platform.machine() == "aarch64"), why


class Server:
    """One fresh child process plus a stdout reader thread (so a hung server times out instead of blocking)."""
    def __init__(self, binary, env_extra):
        env = {"PATH": os.environ.get("PATH", ""), **PINNED_ENV, **env_extra}
        self.t_spawn = time.monotonic()
        self.p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env)
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()
        self._id = 0

    def _read(self):
        for l in self.p.stdout: self.q.put(l)
        self.q.put(None)   # EOF sentinel: a crashed server fails the sample immediately, not after the call timeout

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
            try: m = json.loads(line)
            except ValueError: continue   # non-JSON stdout line: ignore, the timeout still bounds the wait
            if isinstance(m, dict) and m.get("id") == want: return m

    def call_many(self, method, params, n, timeout=300, label=""):
        """Send n identical requests back to back, then collect the n replies (any order). Used by the concurrent scenarios
        (g6-concurrent10, hostile-*); `label` names the scenario in a stall report."""
        want = {self.send(method, params) for _ in range(n)}
        got, end = [], time.time() + timeout
        while want:
            try: line = self.q.get(timeout=max(0.1, end - time.time()))
            except queue.Empty: raise RuntimeError(f"{label or method}: stalled, {len(want)} of {n} concurrent replies still outstanding after {timeout}s") from None
            if line is None: raise RuntimeError("server closed stdout")
            try: m = json.loads(line)
            except ValueError: continue
            if isinstance(m, dict) and m.get("id") in want: want.discard(m["id"]); got.append(m)
        return got

    def call_ok(self, method, params=None, timeout=120):
        """call() that requires a JSON-RPC result: an error reply (or no result) is a protocol failure."""
        m = self.call(method, params, timeout)
        if "error" in m or not isinstance(m.get("result"), dict): raise RuntimeError(f"{method} returned a JSON-RPC error or no result: {json.dumps(m)[:200]}")
        return m["result"]

    def handshake(self, t=None):
        """t (optional dict) receives ready_ms (spawn -> valid initialize result) and tools_list_ms."""
        self.call_ok("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "bench", "version": "0"}}, 30)
        if t is not None: t["ready_ms"] = ms_since(self.t_spawn)
        self.send("notifications/initialized", notify=True)
        t_list = time.monotonic()
        tools = self.call_ok("tools/list", timeout=30).get("tools")
        if t is not None: t["tools_list_ms"] = ms_since(t_list)
        if not isinstance(tools, list) or not any(isinstance(t, dict) and t.get("name") == "fetch" for t in tools):
            raise RuntimeError("tools/list has no `fetch` tool")

    def close(self):
        try: self.p.stdin.close(); self.p.wait(timeout=5)
        except Exception: self.p.kill()


def sample(binary, env_extra, scen, srv, base_url, settle):
    """One fresh-process sample -> dict(valid, reason, rss_kB, hwm_kB, start_utc, *_ms timings). Timings never affect validity."""
    t = {"start_utc": utc_now()}
    t0 = time.monotonic()
    r = _sample(binary, env_extra, scen, srv, base_url, settle, t)
    r.update(t); r["total_ms"] = ms_since(t0)   # includes process close; for idle also the settle sleep
    return r


def _sample(binary, env_extra, scen, srv, base_url, settle, t):
    s = None
    try:
        s = Server(binary, env_extra)
        s.handshake(t)
        if scen["kind"] == "idle":
            time.sleep(settle)
            st = proc_status(s.p.pid)
            return {"valid": True, "rss_kB": st["VmRSS"], "hwm_kB": st["VmHWM"]}
        srv.reset()
        t_call = time.monotonic()
        params = {"name": "fetch", "arguments": {"url": base_url + scen["route"], **scen.get("args", {})}}
        n = scen.get("concurrency", 1)
        replies = s.call_many("tools/call", params, n, label=scen["route"]) if n > 1 else [s.call("tools/call", params)]
        t["fetch_ms"] = ms_since(t_call)
        key = scen.get("counter", scen["route"])
        fb = srv.first_byte_at(key)   # fixture server's first body write (same process, same monotonic clock)
        if fb is not None and fb >= t_call: t["first_byte_ms"] = round((fb - t_call) * 1000, 3)
        st = proc_status(s.p.pid)  # VmHWM read after the call returned, before exit
        for r in replies:
            if "error" in r or not isinstance(r.get("result"), dict): raise RuntimeError(f"tools/call returned a JSON-RPC error: {json.dumps(r)[:200]}")
        results = [r["result"] for r in replies]
        # too_large must be a tool error whose content text (not any field) says so; every concurrent call must agree.
        def outcome(res):
            got_err = bool(res.get("isError"))
            err_text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
            if scen["expect"] in ("too_large", "converter_limit"): return got_err and scen["expect"] in err_text
            return (not got_err and all(t in err_text for t in scen.get("text_must", ()))
                    and not any(t in err_text for t in scen.get("text_must_not", ())))
        outcome_ok = all(outcome(res) for res in results)
        sent, need = srv.bytes_sent(key), scen["min_bytes_v"]
        reason = None if outcome_ok else f"unexpected outcome (expected {scen['expect']})"
        reason = reason or (None if sent >= need else f"early stop: server wrote {sent} < expected_min_bytes {need}")
        return {"valid": reason is None, "reason": reason, "rss_kB": st["VmRSS"], "hwm_kB": st["VmHWM"], "bytes_sent": sent}
    except Exception as e:  # any handshake/protocol failure is an invalid sample, never silently dropped
        return {"valid": False, "reason": f"{type(e).__name__}: {e}", "rss_kB": 0, "hwm_kB": 0}
    finally:
        if s: s.close()


TOTAL_RE = re.compile(r"the content is (\d+) characters long")


def probe_total(binary, env_extra, srv, base, route, raw=False):
    """Length in characters of the output the binary produces for `route` (A-5): a fetch with a start_index far past the end reads to the
    end and its reply states the total. Learned from the binary under test because the converted length is the converter's business;
    a probe is never a measured sample. Raises RuntimeError when the probe fails or the reply has no total."""
    s = None
    try:
        s = Server(binary, env_extra)
        s.handshake()
        srv.reset()
        args = {"url": base + route, "start_index": 2 ** 40, "max_length": 1, **({"raw": True} if raw else {})}
        res = s.call_ok("tools/call", {"name": "fetch", "arguments": args}, 300)
    finally:
        if s: s.close()
    text = " ".join(c.get("text", "") for c in res.get("content", []) if isinstance(c, dict))
    m = None if res.get("isError") else TOTAL_RE.search(text)
    if not m: raise RuntimeError(f"length probe of {route} failed: {text[:160]!r}")
    return int(m.group(1))


def resolve_window(scen, totals, manifest):
    """Turn a scenario's `window` into request arguments and validity rules (returns the updated scenario copy). `totals(route, raw)` gives
    the probed output length. "end": the last WINDOW characters (full consumption); "start": start 0 (early stop fires); "in-cap": a window
    ending 3 x WINDOW - WINDOW = 200,000 characters before the end of the 5 MiB page's output, inside the cap of the 50 MiB chunked body;
    "beyond-cap": start_index = the cap in characters (the first 5 MiB of a body cannot make that many characters: too_large).
    Text rules (`text_must`/`text_must_not`) make the reply prove the window sat where intended, so a run that silently returned something
    else is INVALID: an "end" window has no continuation footer (and, past start 0, states the total: the read reached the end)."""
    w = scen.get("window")
    if not w: return scen
    scen = dict(scen); args = dict(scen.get("args", {})); raw = bool(args.get("raw"))
    info = {"window": w}
    if w == "start":
        start = 0; scen["text_must"] = ["More content available"]
    elif w == "end":
        total = totals(scen["route"], raw); start = max(0, total - WINDOW); info["probed_total"] = total
        scen["text_must_not"] = ["More content available"]
        if start > 0: scen["text_must"] = [f"[Total length: {total} characters.]"]
        if "raw_total" in scen:   # raw output is the body text itself: its length is the fixture's size, so the probe is checked independently
            want = manifest["fixtures"][scen["raw_total"]]["size"]
            if total != want: raise RuntimeError(f"raw probe says {total} characters, the ASCII fixture has {want} bytes")
    elif w == "in-cap":
        total = totals("/5mb.html", False); start = max(0, total - 3 * WINDOW); info["probed_total_5mib_page"] = total
        scen["text_must"] = ["More content available"]
    elif w == "beyond-cap":
        start = CAP
    else:
        raise RuntimeError(f"unknown window {w!r}")
    args.update(start_index=start, max_length=WINDOW)
    scen["args"] = args; info.update(start_index=start, max_length=WINDOW); scen["window_info"] = info
    if scen.get("min_bytes") == "window": scen["min_bytes"] = lambda m, n=start + WINDOW: n   # characters never outnumber the bytes they came from
    return scen


def version_of(binary, env_extra):
    try:
        return subprocess.run([binary, "--version"], capture_output=True, text=True, timeout=10,
                              env={"PATH": os.environ.get("PATH", ""), **PINNED_ENV, **env_extra}).stdout.strip()
    except Exception as e:
        return f"unavailable: {e}"


def _main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--binary-kind", required=True, choices=["shipped", "bench"])
    ap.add_argument("--scenario", action="append", required=True)
    ap.add_argument("--peer-binary", help="the other build of the same commit (bench for a shipped run, shipped for a bench run); --gate requires it and asserts equal commit and Cargo.lock hash (E-4)")
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--settle", type=float, default=30.0, help="idle settle seconds after tools/list (default 30)")
    ap.add_argument("--parallel-idle", type=int, default=1, help="idle samples may run in parallel processes")
    ap.add_argument("--fixtures-dir", default=os.path.join(fixtures.HERE, "fixtures"))
    ap.add_argument("--out", help="also append JSONL to this file")
    ap.add_argument("--child-env", action="append", default=[], help="KEY=VAL added to the pinned child env (recorded)")
    ap.add_argument("--gate", action="store_true", help="produce gating numbers: needs a native aarch64 or x86_64 host, forbids overrides and --smoke")
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
        emit({"kind": "summary", "verdict": "REFUSED", "reason": msg}); return finish(code)

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
        ok, why = native_host(strict=True)
        if not ok: return refuse(f"--gate refused: {why} (QEMU/non-native RSS never gates)", 3)
    if a.runs < 10 and not a.smoke: return refuse(f"--runs {a.runs} < 10 valid runs required (use --smoke for a non-gating check)")
    for n in names:
        if not SCENARIOS[n]["implemented"]: return refuse(f"scenario {n} not implemented yet (needs E-2 fixtures/args or A-5/A-6)")
    badenv = [kv for kv in a.child_env if "=" not in kv]
    if badenv: return refuse(f"--child-env entries must be KEY=VAL: {badenv}")
    nofloor = [n for n in names if SCENARIOS[n]["kind"] == "peak" and "min_bytes" not in SCENARIOS[n]]
    if nofloor: return refuse(f"peak scenario(s) {nofloor} define no min_bytes early-stop floor (NB-2): refusing rather than defaulting to 0")
    env_extra = dict(kv.split("=", 1) for kv in a.child_env)

    bad = fixtures.verify(a.fixtures_dir)
    if bad: return refuse("fixture hash mismatch or missing (fresh checkout? run `python3 bench/fixtures.py generate`; the gzip fixture is verified by its decompressed content, so this is a real content problem): " + "; ".join(bad))
    manifest = fixtures.resolve(fixtures.load_manifest(), a.fixtures_dir)

    if not (os.path.isfile(a.binary) and os.access(a.binary, os.X_OK)): return refuse(f"binary not found or not executable: {a.binary}")
    ver = version_of(a.binary, env_extra)
    marker = "bench-loopback" in ver
    if a.binary_kind == "bench" and not marker: return refuse("--binary-kind bench but --version lacks the bench-loopback marker")
    carried = [m for m in SHIPPED_FORBIDDEN_MARKERS if m in ver]
    if a.binary_kind == "shipped" and carried: return refuse(f"--binary-kind shipped but --version carries a forbidden-feature marker: {carried}")
    if a.gate:
        why = check_identity(a.binary, ver, a.binary_kind)
        if why: return refuse(f"--gate binary identity: {why}")
        if not a.peer_binary: return refuse("--gate requires --peer-binary (the other build of the same commit) to assert equal commit and Cargo.lock hash (E-4)")
        if not (os.path.isfile(a.peer_binary) and os.access(a.peer_binary, os.X_OK)): return refuse(f"peer binary not found or not executable: {a.peer_binary}")
        why = check_build_identity(ver, version_of(a.peer_binary, env_extra))
        if why: return refuse(f"--gate build identity: {why}")
    for n in names:
        need = SCENARIOS[n].get("binary", "bench")
        if need != a.binary_kind: return refuse(f"scenario {n} is gated on the {need} binary, got {a.binary_kind}")

    run_start = utc_now()
    host = host_record(); host["native_aarch64"] = native_aarch64()[0]; host["native_gate_host"] = native_host(strict=True)[0]
    emit({**host, "run_start_utc": run_start, "binary": a.binary, "binary_kind": a.binary_kind, "version": ver, "build_identity": build_identity(ver),
          "peer_version": version_of(a.peer_binary, env_extra) if a.peer_binary else None, "binary_sha256": fixtures.sha256_file(a.binary),
          "child_env": {**PINNED_ENV, **env_extra}, "runs": a.runs, "settle_s": a.settle, "gating": a.gate,
          "fixture_sha256": {k: v.get("sha256") or v["decompressed_sha256"] for k, v in manifest["fixtures"].items()}, "unit": "MiB=2^20 bytes"})

    srv = serve.FixtureServer(a.fixtures_dir, manifest).start()
    base = f"http://127.0.0.1:{srv.port}"
    totals = {}
    def total_of(route, raw):   # one probe per (route, raw) per run, from the binary under test
        if (route, raw) not in totals: totals[(route, raw)] = probe_total(a.binary, env_extra, srv, base, route, raw)
        return totals[(route, raw)]
    medians, invalid, missed, incomplete = {}, [], [], []
    try:
        for n in names:
            scen = dict(SCENARIOS[n])
            try:
                scen = resolve_window(scen, total_of, manifest)
            except Exception as e:   # a probe or window that cannot be resolved is an INVALID scenario, never a silent default
                emit({"kind": "scenario", "scenario": n, "gate": scen["gate"], "verdict": "INVALID", "valid_runs": 0, "runs": a.runs,
                      "invalid_reasons": [f"window: {type(e).__name__}: {e}"]}); invalid.append(n); continue
            scen["min_bytes_v"] = scen["min_bytes"](manifest) if "min_bytes" in scen else 0
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
                   "invalid_reasons": sorted({x["reason"] for x in ss if not x["valid"]}), "target_kB": target_kb, "window": scen.get("window_info"),
                   # timings: recorded, NOT gated (ADR-007 restates the readiness figure); labelled advisory unless --gate
                   "timings": {"unit": "ms", "gated": False, "gating_run": a.gate, "standin": not ver.startswith(VERSION_PREFIX),
                               "label": "recorded, not gated" if a.gate else "advisory (not a gating run)",
                               **timing_stats(ss)},
                   "sample_timings": [{"valid": x["valid"], **{k: x[k] for k in ("start_utc", *TIMING_KEYS) if k in x}} for x in ss]}
            if len(good) < 10 and not a.smoke:
                rec["verdict"] = "INVALID"; invalid.append(n)
            elif vals:
                med = statistics.median(vals); medians[n] = med
                rec.update(median_kB=med, min_kB=min(vals), max_kB=max(vals), median_MiB=round(med / MIB_KB, 2),
                           verdict="PASS" if med <= target_kb else "FAIL")
                if scen.get("recorded_only"):   # G6: reported (NFR-08 documentation), never a pass/fail figure
                    rec["verdict"] = "RECORDED"; rec["recorded_only"] = True
                if rec["verdict"] == "FAIL": missed.append(n)
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
    summ["run_start_utc"], summ["run_end_utc"] = run_start, utc_now()
    summ["missed"] = missed; summ["invalid"] = invalid; summ["incomplete"] = incomplete
    # A pass without --gate is advisory and must not read as a gate PASS.
    summ["verdict"] = "INVALID" if invalid else ("FAIL" if missed else ("INCOMPLETE" if incomplete else ("PASS" if a.gate else "ADVISORY_PASS")))
    summ["note"] = "" if a.gate else "advisory: not a gating run (no --gate); never valid for NFR claims"
    emit(summ)
    return finish(2 if invalid else 1 if missed else 2 if incomplete else 0)


def main(argv=None):
    """Any unexpected harness exception is a plumbing failure (exit 2), never the 'target missed' code 1 (NB-1)."""
    try:
        return _main(argv)
    except SystemExit:
        raise
    except Exception as e:  # noqa: BLE001
        print(json.dumps({"kind": "summary", "verdict": "REFUSED", "reason": f"harness error: {type(e).__name__}: {e}"}), flush=True)
        return 2


if __name__ == "__main__":
    sys.exit(main())
