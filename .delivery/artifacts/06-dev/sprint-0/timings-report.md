# E-1 harness timings (Sprint 0) - developer report

Timings are RECORDED, NOT GATED. No pass/fail rule, exit code, verdict or valid-run rule was added or changed; memory gate logic is untouched (selftest proves verdict and exit code are identical for a 300 ms slower server).

## Fields (all milliseconds, monotonic clock `time.monotonic()`; wall clock only for `*_utc`)
Per sample (`sample_timings[]` in each `scenario` JSONL record, all samples incl. invalid, with `valid`):
- `start_utc`: ISO-8601 UTC (ms precision, `Z`) at sample start.
- `ready_ms`: process spawn (just before Popen) to the first valid `initialize` result.
- `tools_list_ms`: `tools/list` request sent to valid result (includes the `fetch`-tool presence check's reply only; the check itself is after the timer).
- `first_byte_ms` (fetch scenarios): `tools/call` sent to the fixture server's first body write for that sample. Server-side stamp in the same process, so it is an upper-bound-ish proxy for "first byte reached the client"; absent when the server never writes a body (50 MiB Content-Length: client aborts on headers).
- `fetch_ms` (fetch scenarios): `tools/call` sent to result received.
- `total_ms`: sample start to after the child is closed. For idle it includes the settle sleep (30 s default), so it is not comparable to fetch scenarios.
Per scenario (`timings` object): `unit`, `gated: false`, `gating_run` (= --gate), `standin` (true when `--version` does not start `fetch-mcp `), `label` ("advisory (not a gating run)" or "recorded, not gated"), and per key `{n, median, min, max}` plus `p95` (nearest-rank) only when n >= 20. Statistics use VALID samples only. Run level: `run_start_utc` on the host record, `run_start_utc`/`run_end_utc` on the summary.

## Not measured
Client-observed first byte (only server write time), TLS/DNS (loopback HTTP), CPU time, container start (`docker run -i` daemon/namespace overhead), page-cache/cold-start effects, time per idle settle phase.

## Files changed
bench/measure.py, bench/serve.py (first-body-write stamp, timing only), bench/standin_mcp.py (`STANDIN_DELAY_MS` for the selftest), bench/selftest.py (6 new checks: fields present, sane ordering/non-negative, labels and UTC stamps, p95 at 20 only, invalid excluded, verdict unaffected), .github/workflows/arm-bench.yml (job summary adds a Timings table; job id, permissions, pins, triggers unchanged). Not pushed.

## Docs needed (docs/BENCHMARK.md, owned by others)
- Add a Timings section: the field list above, units, "recorded, not gated", p95 rule, valid-samples-only, idle `total_ms` includes settle.
- State that the PRD 250 ms readiness figure is NOT checked by the harness; `ready_ms` is evidence only, restated under ADR-007 for the container case.
- Note the advisory/stand-in labelling and that the JSONL schema gained `timings`, `sample_timings`, `run_start_utc`, `run_end_utc`.

## Caveats
- Hosted cloud VMs are noisy neighbours: expect wide min-max; use medians, never one run. 10 samples give no p95.
- Once a container image exists, `docker run -i` adds daemon, namespace and image-layer startup to `ready_ms`; measure it as a separate binary-kind/wrapper and label it, do not compare with native spawn. Memory of the container process would also differ (harness reads /proc of the spawned pid, i.e. the docker client, not the container) - a design decision for the container-measurement story.
- Spawn timer starts before Popen so interpreter/exec cost of wrappers (e.g. the `sh` exec wrapper in the workflow) is included.
