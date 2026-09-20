# E-1 dev report

## Done
- `docs/BENCHMARK.md`: unit (MiB), targets, host placeholders, binaries/E-8 bounds, method, scenarios, valid-run rule, determinism, TLS decision, Goal 2/3/5 methods, G0 recommendation.
- `bench/`: `fixtures.py` (deterministic gen, manifest, verify), `serve.py` (fixture server with byte counter, 5 MB, gzip, 50 MB CL/chunked, slow), `scenarios.py`, `measure.py` (JSONL, median/min/max, pass/fail, valid-run, kind/preflight refusals), `standin_mcp.py`, `selftest.py`, `manifest.json`.

## Commands and results
- `python3 bench/fixtures.py generate/write-manifest/verify` -> fixtures OK (5,241,856 / 776,917 gz / 52,428,800 bytes).
- `python3 bench/selftest.py` -> 15 checks ok, `SELFTEST PASSED` (x86_64 dev host; known +20 MiB alloc measured in VmRSS and VmHWM within 1.5 MiB; exit codes 0/1/2/3; early stop INVALID; tamper refused). Stand-in figures are not product measurements.
- No real MCP server run; no aarch64 measurement.

## AC checklist
- Targets + one unit definition: pass (doc sec 1-2).
- Host recorded as native aarch64 runner, OS/RAM to fill: pass (placeholders).
- Protocol (handshake, 30 s idle, fixtures, client script, /proc): pass (5/50 MB CL/chunked, gzip, slow-drip generated or served; client script is measure.py).
- TLS decision: pass (manual real-host run, caveat kept).
- Loopback path and which binary each gate measures: pass.
- Token/baseline/success/overhead definitions: pass.
- 50-URL list and 10-URL smoke list: NOT DONE (criteria only; no URLs invented; owner/E-7).
- A-1 re-run under protocol: NOT RUN, deviation recorded (no ARM host).
- Go/no-go recommendation: pass (preliminary GO, conditional).

## Deviations
- 5 MB fixture is 5 MiB - 1 KiB so it passes a 5 MiB cap. Gzip fixture hash depends on zlib build. Late-landmark fixture, G6, G4b args, macOS path, CI workflow left to E-2. `bench/fixtures/` gitignored (regenerate).
