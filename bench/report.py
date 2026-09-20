#!/usr/bin/env python3
"""Markdown job summary from harness JSONL files (stdlib only). Prints measured values against targets in MiB.

  report.py --platform out/platform.txt --idle out/idle.jsonl --idle-bench out/idle-bench.jsonl --peak out/peak.jsonl
            [--shipped BIN --bench BIN]

Prints: per-scenario median/min/max in MiB and verdict, the summary verdict, timings (recorded, not gated), and the E-8
shipped-vs-bench deltas (idle bound 0.5 MiB; binary size recorded). Exit 0 always: pass/fail is the harness's exit code.
"""
import argparse, json, os

MIB = 1024.0


def load(path):
    try:
        with open(path) as f: return [json.loads(l) for l in f if l.startswith("{")]
    except OSError:
        return []


def scenarios(recs): return [r for r in recs if r.get("kind") == "scenario"]
def summary(recs): return next((r for r in reversed(recs) if r.get("kind") == "summary"), {})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--platform"); ap.add_argument("--idle"); ap.add_argument("--idle-bench"); ap.add_argument("--peak")
    ap.add_argument("--shipped"); ap.add_argument("--bench")
    a = ap.parse_args()
    if a.platform and os.path.exists(a.platform):
        print("```\n" + open(a.platform).read() + "```\n")
    print("| scenario | binary | metric | valid/runs | median MiB | min-max MiB | target MiB | verdict |\n|---|---|---|---|---|---|---|---|")
    med = {}
    for label, path in (("idle", a.idle), ("idle-bench", a.idle_bench), ("peak", a.peak)):
        recs = load(path) if path else []
        for o in scenarios(recs):
            v = o.get("samples_kB") or [0]
            med[o["scenario"]] = o.get("median_kB")
            print(f"| {o['scenario']} | {o['binary_kind']} | {o['metric']} | {o['valid_runs']}/{o['runs']} | {o.get('median_MiB', 'n/a')} | "
                  f"{min(v)/MIB:.2f}-{max(v)/MIB:.2f} | {o['target_kB']/MIB:g} | {o['verdict']} {o.get('invalid_reasons') or ''} |")
        s = summary(recs)
        if s: print(f"\n{label}: summary **{s.get('verdict')}** {s.get('reason', '')} (gating={s.get('gating')}) missed={s.get('missed')} "
                    f"invalid={s.get('invalid')} incomplete={s.get('incomplete')} boundedness={s.get('boundedness')} gating_peak_MiB={s.get('gating_peak_MiB')}\n")
    i, ib = med.get("idle"), med.get("idle-bench")
    if i and ib:
        d = (ib - i) / MIB
        print(f"E-8 idle delta (bench minus shipped): {d:+.2f} MiB, bound 0.5 MiB: {'within' if abs(d) <= 0.5 else 'OVER'} (recorded)")
    if a.shipped and a.bench and os.path.exists(a.shipped) and os.path.exists(a.bench):
        s1, s2 = os.path.getsize(a.shipped), os.path.getsize(a.bench)
        print(f"E-8 binary size: shipped {s1} B, bench {s2} B, delta {s2 - s1:+d} B (recorded; explanation is E-4's)")
    tim = [o for p in (a.idle, a.peak) if p for o in scenarios(load(p))]
    if tim:
        print("\n### Timings (ms, median (min-max) over valid samples; recorded, not gated)\n\n| scenario | ready | tools/list | first byte | fetch | total |\n|---|---|---|---|---|---|")
        for o in tim:
            t = o.get("timings", {})
            cell = lambda k: f"{t[k]['median']} ({t[k]['min']}-{t[k]['max']})" if k in t else "n/a"
            print(f"| {o['scenario']} | " + " | ".join(cell(k) for k in ("ready_ms", "tools_list_ms", "first_byte_ms", "fetch_ms", "total_ms")) + " |")


if __name__ == "__main__":
    main()
