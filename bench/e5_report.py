#!/usr/bin/env python3
"""E-5 benchmark report and release decision (stdlib only). Assembles harness output into a Markdown report.

  e5_report.py --idle out/idle.jsonl --peak out/peak.jsonl --overhead out/convert-overhead.txt --smoke out/smoke.jsonl
               [--platform out/platform.txt] [--out docs/E5-REPORT.md]

Decision rule (E-5 ACs):
  release       every input present AND every target met AND the smoke is a REAL run of the owner's 10-URL list.
  do not release  an input is present and a target is missed (the gap and follow-ups are listed).
  not decided   an input is missing or is not evidence (advisory, dry-run, invalid, wrong list size). The report says exactly what is missing.
It never defaults a missing input to a pass, and a dry-run smoke (local fixtures) is never accepted as the smoke result. Whatever the
decision, the report states that the D-1 (Sprint 8) shipped-build re-measure governs the MVP tag, and labels each figure by binary.
Exit 0 = release, 1 = do not release, 2 = not decided.
"""
import argparse, json, re, sys

IDLE_TARGET_MIB, PEAK_TARGET_MIB, OVERHEAD_TARGET_MS = 10, 40, 500


def load(path):
    out = []
    if not path: return out
    try:
        with open(path, encoding="utf-8") as f:
            for l in f:
                if l.startswith("{"):
                    try: out.append(json.loads(l))
                    except ValueError: pass
    except OSError: pass
    return out


def gate_result(recs, label):
    """(status, detail, rows): status is met | missed | missing. Only a --gate PASS summary is evidence."""
    if not recs: return "missing", f"{label}: no results file or no records", []
    summ = next((r for r in reversed(recs) if r.get("kind") == "summary"), None)
    rows = [r for r in recs if r.get("kind") == "scenario"]
    if summ is None: return "missing", f"{label}: no summary record (truncated run)", rows
    if not summ.get("gating"): return "missing", f"{label}: the run was advisory (no --gate), which proves nothing about a target", rows
    v = summ.get("verdict")
    if v == "PASS": return "met", f"{label}: gate PASS", rows
    if v == "FAIL": return "missed", f"{label}: gate FAIL ({', '.join(summ.get('missed', [])) or 'see scenarios'})", rows
    return "missing", f"{label}: gate verdict {v} is not a result", rows


def overhead(path):
    if not path: return None
    try: text = open(path, encoding="utf-8").read()
    except OSError: return None
    m = re.search(r"CONVERT_1MIB bytes=(\d+) median_ms=([\d.]+) p95_ms=([\d.]+) max_ms=([\d.]+) arch=(\w+)", text)
    if not m or re.search(r"qemu", text, re.I): return None   # no line, or an emulated run: never evidence
    o = {"bytes": int(m[1]), "median_ms": float(m[2]), "p95_ms": float(m[3]), "max_ms": float(m[4]), "arch": m[5]}
    # Goal 5 is defined on the 1 MiB page on a native host (BENCHMARK.md section 9); anything else is not the figure.
    return o if o["bytes"] == 1048576 and o["arch"] in ("aarch64", "x86_64") else None


def smoke_result(recs):
    summ = next((r for r in reversed(recs) if r.get("kind") == "smoke-summary"), None)
    if summ is None: return "missing", "10-URL live smoke: no results (the owner's list has not been supplied and run)", None
    if summ.get("verdict") != "DONE": return "missing", f"10-URL live smoke: run INVALID ({summ.get('reason', '')})", summ
    if summ.get("dry_run"): return "missing", "10-URL live smoke: only a dry run against local fixtures exists; it is not the smoke result", summ
    if summ.get("list_count") != 10: return "missing", f"10-URL live smoke: list has {summ.get('list_count')} URLs, not 10", summ
    bc = summ.get("by_category")
    if not isinstance(bc, dict) or not isinstance(summ.get("ok"), int) or not isinstance(summ.get("redirected"), int) or not isinstance(summ.get("failed_urls"), list):
        return "missing", "10-URL live smoke: summary is truncated or malformed", summ
    dead = [c for c in ("tls", "redirect", "json", "text") if not (isinstance(bc.get(c), list) and len(bc[c]) == 2 and bc[c][0] >= 1)]
    if dead: return "missing", f"10-URL live smoke: no successful fetch in category {', '.join(dead)} (a failed smoke is not evidence)", summ
    if summ["redirected"] < 1: return "missing", "10-URL live smoke: no fetch followed a redirect", summ
    return "done", f"10-URL live smoke: {summ['ok']} of 10 fetched (non-gating)", summ


def table(rows):
    lines = ["| scenario | binary | metric | valid/runs | median MiB | min-max MiB | verdict |", "|---|---|---|---|---|---|---|"]
    for r in rows:
        f = lambda k: round(r[k] / 1024, 2) if k in r else "n/a"
        lines.append(f"| {r.get('scenario')} | {r.get('binary_kind', '?')} | {r.get('metric', '?')} | {r.get('valid_runs')}/{r.get('runs')} | {f('median_kB')} | {f('min_kB')}-{f('max_kB')} | {r.get('verdict')} |")
    return "\n".join(lines)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for k in ("idle", "peak", "overhead", "smoke", "platform", "out"): ap.add_argument("--" + k)
    ap.add_argument("--binary-path", default="<path to fetch-mcp>")
    a = ap.parse_args(argv)
    idle_s, idle_d, idle_rows = gate_result(load(a.idle), "idle RSS (E-3)")
    peak_s, peak_d, peak_rows = gate_result(load(a.peak), "peak RSS and 50 MiB boundedness (E-4, G4b)")
    ov = overhead(a.overhead)
    ov_s = "missing" if ov is None else ("met" if ov["p95_ms"] <= OVERHEAD_TARGET_MS else "missed")
    ov_d = "Goal 5 overhead: no measurement line" if ov is None else f"Goal 5 overhead: p95 {ov['p95_ms']} ms on {ov['arch']} (target <= {OVERHEAD_TARGET_MS} ms)"
    sm_s, sm_d, sm = smoke_result(load(a.smoke))
    checks = [(idle_s, idle_d), (peak_s, peak_d), (ov_s, ov_d), ("met" if sm_s == "done" else "missing", sm_d)]
    missed = [d for s, d in checks if s == "missed"]
    missing = [d for s, d in checks if s == "missing"]
    if missed: decision, code = "do not release", 1
    elif missing: decision, code = "not decided", 2
    else: decision, code = "release", 0

    o = ["# E-5 benchmark report and release decision", "", f"**Decision: {decision}**", ""]
    o += ["Release profile: the D-7 pinned profile (see Cargo.toml). Every figure below is labelled by binary (shipped or bench). "
          "The D-1 (Sprint 8) re-measure of idle and peak on the shipped build, on the hosted arm64 runner, governs the MVP tag; this report does not replace it.", ""]
    if a.platform:
        try: o += ["```", open(a.platform, encoding="utf-8").read().rstrip(), "```", ""]
        except OSError: o += ["(platform file unreadable)", ""]
    o += [f"## Memory (targets: idle <= {IDLE_TARGET_MIB} MiB, peak <= {PEAK_TARGET_MIB} MiB, 50 MiB boundedness <= 1.10x the 5 MiB peak)", "", f"- {idle_d}", f"- {peak_d}", ""]
    if idle_rows or peak_rows: o += [table(idle_rows + peak_rows), ""]
    o += ["## Conversion overhead (Goal 5)", "", f"- {ov_d}", ""]
    o += ["## 10-URL live smoke (Goal 2 evidence; network, TLS, redirects, JSON, plain text; non-gating)", "", f"- {sm_d}", ""]
    if sm_s == "done":
        o += [f"- by category (ok/total): {json.dumps(sm['by_category'])}; redirected: {sm['redirected']}"] + [f"- FAILED: {u}" for u in sm["failed_urls"]] + [""]
    if decision == "release":
        o += ["## Config change to register the server", "", "```", f"claude mcp add fetch -- {a.binary_path}", "```", "",
              "Tagging and distribution still wait for M3, the D-1 re-measure and OQ-7 (registry and image naming).", ""]
    elif decision == "do not release":
        o += ["## Gap and follow-up actions", ""] + [f"- Missed: {m}" for m in missed] + ["- Follow-up: fix the miss, re-run the gate, regenerate this report.", ""]
        if missing: o += ["Also not yet evidenced:"] + [f"- {m}" for m in missing] + [""]
    else:
        o += ["## Not decided: inputs missing", ""] + [f"- {m}" for m in missing] + [""]
    text = "\n".join(o)
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f: f.write(text)
    print(text)
    return code


if __name__ == "__main__":
    sys.exit(main())
