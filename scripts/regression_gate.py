#!/usr/bin/env python3
"""E-6: CI memory-regression tripwire.

Compares this run's median idle (VmRSS) and gating peak (VmHWM) against the stored last-main
baseline (bench/baseline.json) for one platform cell, and fails (exit 1) if either regressed by
more than --max-regression-pct (default 10%) relative to the baseline.

This is a TRIPWIRE on top of, never a replacement for, the absolute-target gate already enforced
by `bench/measure.py --gate` (10 MiB idle / 40 MiB peak, strict). A run that is comfortably under
the absolute target can still trip this gate if it regressed materially from last main; a run that
misses the absolute target has already failed via measure.py's own exit code regardless of this
script. The 10% is always relative to the stored baseline, never a licence to exceed the absolute
target (sprint-plan.md E-6 AC).

Usage:
  regression_gate.py --idle out/idle.jsonl --peak out/peak.jsonl --baseline bench/baseline.json \
      --platform amd64-gnu [--max-regression-pct 10]

  regression_gate.py --update --idle out/idle.jsonl --peak out/peak.jsonl --baseline bench/baseline.json \
      --platform amd64-gnu --run-id 12345

Exit codes: 0 no regression (or --update wrote a new baseline); 1 a metric regressed beyond the
threshold; 2 plumbing error (missing/unparseable input, no baseline and not --update).
"""
import argparse
import datetime
import json
import sys


def load_jsonl(path):
    recs = []
    try:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line.startswith("{"):
                    continue
                try:
                    recs.append(json.loads(line))
                except ValueError:
                    continue
    except OSError as e:
        print(f"regression_gate: cannot read {path}: {e}", file=sys.stderr)
        return None
    return recs


def idle_median_kb(idle_recs):
    """The 'idle' scenario record's median_kB (shipped binary), from idle.jsonl."""
    for r in idle_recs:
        if r.get("kind") == "scenario" and r.get("scenario") == "idle" and "median_kB" in r:
            return r["median_kB"]
    return None


def peak_gating_kb(peak_recs):
    """The summary record's gating_peak_kB (max of per-scenario gating medians), from peak.jsonl."""
    for r in reversed(peak_recs):
        if r.get("kind") == "summary" and "gating_peak_kB" in r:
            return r["gating_peak_kB"]
    return None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--idle", required=True, help="idle.jsonl produced by measure.py")
    ap.add_argument("--peak", required=True, help="peak.jsonl produced by measure.py")
    ap.add_argument("--baseline", required=True, help="bench/baseline.json")
    ap.add_argument("--platform", required=True, help="baseline key, e.g. amd64-gnu")
    ap.add_argument("--max-regression-pct", type=float, default=10.0)
    ap.add_argument("--update", action="store_true", help="write this run's medians as the new baseline instead of gating")
    ap.add_argument("--run-id", default="", help="recorded alongside an --update (e.g. the GitHub Actions run id)")
    a = ap.parse_args(argv)

    idle_recs = load_jsonl(a.idle)
    peak_recs = load_jsonl(a.peak)
    if idle_recs is None or peak_recs is None:
        return 2
    idle_kb = idle_median_kb(idle_recs)
    peak_kb = peak_gating_kb(peak_recs)
    if idle_kb is None or peak_kb is None:
        print(f"regression_gate: could not find idle median_kB and/or peak gating_peak_kB "
              f"(idle={idle_kb} peak={peak_kb}) - treating as plumbing failure, not a regression verdict",
              file=sys.stderr)
        return 2

    try:
        with open(a.baseline) as f:
            baseline = json.load(f)
    except (OSError, ValueError) as e:
        if a.update:
            baseline = {}
        else:
            print(f"regression_gate: cannot read baseline {a.baseline}: {e} "
                  f"(first run for this platform: run with --update to seed it, never treated as a silent pass)",
                  file=sys.stderr)
            return 2

    if a.update:
        baseline[a.platform] = {
            "idle_kB": idle_kb,
            "peak_kB": peak_kb,
            "updated_from_run": a.run_id or "unknown",
            "updated_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        }
        with open(a.baseline, "w") as f:
            json.dump(baseline, f, indent=2, sort_keys=True)
            f.write("\n")
        print(f"regression_gate: baseline[{a.platform}] updated: idle={idle_kb} kB peak={peak_kb} kB (run {a.run_id or 'unknown'})")
        return 0

    entry = baseline.get(a.platform)
    if not entry:
        print(f"regression_gate: no baseline entry for platform {a.platform!r} yet - "
              f"nothing to compare against, not gating this run (seed it with --update on the next push to main); "
              f"current: idle={idle_kb} kB peak={peak_kb} kB", file=sys.stderr)
        return 0

    threshold = 1.0 + a.max_regression_pct / 100.0
    failures = []
    for label, current, base_key in (("idle (VmRSS)", idle_kb, "idle_kB"), ("peak (VmHWM)", peak_kb, "peak_kB")):
        base = entry.get(base_key)
        if not base:
            continue
        limit = base * threshold
        pct = (current / base - 1.0) * 100.0
        verdict = "REGRESSION" if current > limit else "ok"
        print(f"regression_gate[{a.platform}] {label}: current={current} kB baseline={base} kB "
              f"({pct:+.1f}%, limit {a.max_regression_pct:.0f}% -> {limit:.0f} kB) {verdict}")
        if current > limit:
            failures.append(f"{label}: {current} kB > {limit:.0f} kB ({pct:+.1f}% vs baseline {base} kB from run {entry.get('updated_from_run', '?')})")

    if failures:
        print(f"regression_gate[{a.platform}]: FAIL - regressed more than {a.max_regression_pct:.0f}% vs the stored last-main baseline:",
              file=sys.stderr)
        for f_ in failures:
            print(f"  - {f_}", file=sys.stderr)
        print("regression_gate: this is a regression tripwire on top of the absolute 10 MiB/40 MiB gate, "
              "not a substitute for it; the absolute gate's own verdict (measure.py --gate) is unaffected.",
              file=sys.stderr)
        return 1

    print(f"regression_gate[{a.platform}]: PASS - within {a.max_regression_pct:.0f}% of the stored last-main baseline")
    return 0


if __name__ == "__main__":
    sys.exit(main())
