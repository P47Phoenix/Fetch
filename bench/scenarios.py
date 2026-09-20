"""Scenario definitions (data only). IDs and gate assignment follow docs/BENCHMARK.md section 5.
`route` is a serve.py path; `expect` is "ok" or "too_large"; `min_bytes` is a callable(manifest)->int, the
valid-run floor for the server-side byte counter. implemented=False scenarios need fixtures/args E-2 supplies."""
from fixtures import REDIR_BODY, REDIR_FINAL
MIB = 1024 * 1024
CAP = 5 * MIB
_size = lambda name: (lambda m: m["fixtures"][name]["size"])

SCENARIOS = {
    "idle":            dict(kind="idle", gate="G0,G4a,G4b", binary="shipped", implemented=True),
    # Idle RSS of the BENCH build (E-8: the idle delta versus the shipped build is bounded at 0.5 MiB). Recorded next to the
    # shipped figure; never a gate figure itself (gate "none"), and `idle` still refuses a bench-marked binary.
    "idle-bench":      dict(kind="idle", gate="none", binary="bench", implemented=True, ref="E-8 idle delta"),
    "g4a-5mib-full":   dict(kind="peak", gate="G4a", route="/5mb.html", args={"max_length": 5 * MIB}, expect="ok",
                            min_bytes=_size("html_5mib"), implemented=True, ref="G1/G2 read-in-full form"),
    "g4a-5mib-gz":     dict(kind="peak", gate="G4a", route="/5mb.html.gz", args={"max_length": 5 * MIB}, expect="ok",
                            min_bytes=_size("html_5mib_gz"), implemented=True, ref="G2 read-in-full form"),
    "g4a-late-landmark": dict(kind="peak", gate="G4a", route="/late-landmark.html", args={"max_length": 5 * MIB}, expect="ok",
                            min_bytes=_size("late_landmark"), implemented=True, ref="G4 read-in-full form (the holdback itself needs A-4)"),
    "g4a-50mib-cl":    dict(kind="peak", gate="G4a", route="/50mb-cl.html", args={"max_length": 5 * MIB}, expect="too_large",
                            min_bytes=lambda m: 0,  # architecture 11.2: expected_min_bytes for G7a is zero (a correct client aborts on the Content-Length header, before any body write)
                            bounded_vs="g4a-5mib-full", implemented=True, ref="G7a"),
    "g4a-50mib-chunked": dict(kind="peak", gate="G4a", route="/50mb-chunked.html", args={"max_length": 5 * MIB}, expect="too_large",
                            min_bytes=lambda m: CAP, bounded_vs="g4a-5mib-full", implemented=True, ref="chunked, no window"),
    # 5 redirect hops, each with a body and (in the product) a fresh client build (A-3b fix-pass 1 note for E-2/E-4).
    # Recorded, outside the gating peak. `counter` is the serve.py counter key; floor = 5 hop bodies + the final body.
    "redirect-chain5": dict(kind="peak", gate="none", route="/redir/5", counter="/redir", expect="ok",
                            min_bytes=lambda m: 5 * REDIR_BODY + REDIR_FINAL, implemented=True, ref="redirect-chain memory check"),
    "g6-concurrent10": dict(kind="peak", gate="none", route="/5mb.html", expect="ok", implemented=False,
                            ref="G6: 10 concurrent, recorded not gating; E-4"),
    # G4b: need A-5 / A-6 (window, early stop, raw); windows computed by E-2 from the converted-output length.
    "g4b-window-start": dict(kind="peak", gate="G4b", route="/5mb.html", expect="ok", implemented=False, ref="window at start, early stop"),
    "g4b-window-end":  dict(kind="peak", gate="G4b", route="/5mb.html", expect="ok", implemented=False, ref="G1"),
    "g4b-raw":         dict(kind="peak", gate="G4b", route="/5mb.html", args={"raw": True}, expect="ok", implemented=False, ref="G3"),
    "g4b-chunked-window-in-cap": dict(kind="peak", gate="G4b", route="/50mb-chunked.html", expect="ok", bounded_vs="g4a-5mib-full",
                            implemented=False, ref="G7b"),
    "g4b-window-beyond-cap": dict(kind="peak", gate="G4b", route="/50mb-chunked.html", expect="too_large", bounded_vs="g4a-5mib-full",
                            implemented=False, ref="G5, G7c"),
}
GROUPS = {"g4a": [k for k, v in SCENARIOS.items() if v["gate"] == "G4a"],
          "g4b": [k for k, v in SCENARIOS.items() if v["gate"] == "G4b"]}
