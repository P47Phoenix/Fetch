# Benchmark protocol and absolute targets (story E-1)

Status: E-1 result, Sprint 0. Other documents (PRD, EPICS, architecture, config defaults, reports) refer to this file for the unit, targets, scenarios and method. Harness skeleton: `bench/` (self-test: `python3 bench/selftest.py`). E-2 builds the full fixture set and CI on top of it.

## Quickstart (contributors)

Prerequisites: Linux, Python 3.8+ (standard library only), a Rust toolchain only if you build the real binary. macOS is not supported yet (E-2).

1. Self-test the harness (about 1 minute, no Rust needed): `python3 bench/selftest.py`. Expected last line: `SELFTEST PASSED`. It regenerates fixtures itself and runs against `bench/standin_mcp.py`, a stand-in that is NOT the product, so its RSS figures are not product measurements.
2. Before a real run, generate the fixtures once: `python3 bench/fixtures.py generate` (measure.py refuses on a missing or mismatched `bench/manifest.json` hash).
3. Worked example, advisory smoke on the stand-in (bench kind needs the marker, so set `STANDIN_BENCH=1`): `python3 bench/measure.py --binary bench/standin_mcp.py --binary-kind bench --child-env STANDIN_BENCH=1 --scenario g4a-5mib-full --smoke`. Output is JSON lines, one per scenario and a final `summary`; without `--gate` the summary verdict is `ADVISORY_PASS` or `FAIL`, never `PASS`. Exit codes: 0 pass, 1 target missed, 2 invalid or refused, 3 `--gate` off native aarch64.
4. Gating run (aarch64 runner only): `python3 bench/measure.py --binary target/release/fetch-mcp --binary-kind shipped --scenario idle --gate`. `--gate` refuses `--smoke`, target overrides, `--settle` other than 30, `--parallel-idle` above 1, and `LD_PRELOAD`, `MALLOC_*` or `RUST_LOG` child env.

Gate names versus scenario IDs: G0, G4a and G4b are gates (points in the plan where a memory verdict is taken). G1 to G7 are the scenario IDs from architecture 11.1. The harness IDs in the table in section 5 (for example `g4a-5mib-full`) name the gate and the scenario together.

## 1. Unit definition (stated once)

MB in this project means MiB: 1 MB = 2^20 = 1,048,576 bytes, for the targets, the fixtures and the fetch cap alike. The harness reads `/proc` in kB (KiB), so 10 MB = 10,240 kB and 40 MB = 40,960 kB. The 5 MB fetch cap = 5,242,880 bytes. Reports say "MB (MiB)".

## 2. Absolute targets

| Target | Metric | Limit | Rule |
|---|---|---|---|
| Idle | VmRSS after `initialize` + `tools/list` + 30 s settle | <= 10 MB (10,240 kB) | median of 10 valid runs, shipped binary |
| Peak | VmHWM after the fetch returned | <= 40 MB (40,960 kB) | max over gating scenarios of per-scenario medians, each of 10 valid runs, `bench-loopback` binary |
| Boundedness | 50 MB scenarios peak | <= 1.10 x the 5 MB full-read peak | medians |

No tolerance on the release gate (E-3/E-5). E-6 CI is a tripwire: the absolute target still applies, plus a 10% regression bound versus the stored last-main baseline. Gates: G0 (end Sprint 0), G4a (end Sprint 4), G4b (end Sprint 5); definitions in architecture 11.0. Targets are not lowered.

## 3. Benchmark host (placeholders, fill when known)

Per OQ-9 the host is the author's native aarch64 runner on their cluster. Recorded by the harness (`kind: host` JSONL line): machine, kernel, CPU, MemTotal, page size, THP, governor, load average, cgroup limit, container flag, OS.

| Field | Value |
|---|---|
| Runner name / access | PLACEHOLDER: not yet known (do not guess) |
| OS and kernel | PLACEHOLDER |
| RAM | PLACEHOLDER |
| CPU / page size | PLACEHOLDER (record `getconf PAGESIZE`; only same-page-size runs are compared) |
| Isolation | must satisfy architecture 9.2 (isolated, no fork PRs, no secrets) |

Native aarch64 procedure: (1) preflight: `uname -m` = aarch64, `file <binary>` = ARM aarch64, no qemu aarch64 handler in `/proc/sys/fs/binfmt_misc` (the harness `--gate` mode checks and refuses with exit 3 otherwise); (2) build gnu (`.2.17`) and musl with the pinned interim script, shipped and `bench-loopback` from one commit; (3) `python3 bench/measure.py --gate --binary <bin> --binary-kind shipped|bench --scenario ...`; (4) commit the JSONL under the report. QEMU or any non-aarch64 figure is never gating (the A-1 qemu figure of 13.0 MB idle was translator overhead).

## 4. Binaries (E-8 loopback path, CONFIRMED by the user 2026-09-19; OQ-4 not decided)

The shipped release binary is fail-closed and cannot reach the loopback fixture server. Idle is gated on the shipped binary. Peak and 50 MB scenarios run on the `bench-loopback` build (compile-time Cargo feature, off by default, permits only 127.0.0.0/8 and `::1`, never in a release, tag or distributed artifact, asserted absent by the D-7 guard). Both come from one commit, one Cargo.lock hash and the D-7 release profile, via the same pinned pipeline, and both report commit and Cargo.lock hash in `--version` (commit and hash are added by E-8/D-3; today `--version` prints the crate version only). Marker contract, one definition: a build with a forbidden feature makes `--version` print a marker `FETCH_MCP_MARKER_<FEATURE>_V1:<feature-name>`, so a bench build prints `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback` and a release build prints no marker. The harness checks for the literal `bench-loopback` in `--version` (it refuses peak runs without it, and refuses a marked binary for shipped-binary idle); the D-7 guard greps the binary for the `FETCH_MCP_MARKER_` prefix, so any marker (including a renamed feature) fails a release build. Every figure is labelled with its binary (`binary_kind`). Bench-vs-shipped bounds (E-8, part of the G4a pass): idle delta <= 0.5 MB; binary size delta recorded and explained (no bound); one manual 5 MB fetch of a public host on the shipped binary within 10% of the bench peak and <= 40 MB; same commit and Cargo.lock hash. Exceeding a bound fails G4a until explained and re-measured.

## 5. Measurement method

Per sample a fresh child process (cold; no in-process warm-up), spawned with a pinned environment (allow-list: `PATH`, `FETCH_LOG=warn`, `LC_ALL=C`, plus recorded `--child-env`; `RUST_LOG`, `LD_PRELOAD`, `MALLOC_*` unset). Client script = `bench/measure.py` (JSON-RPC over stdio):

1. Spawn; `initialize`; `notifications/initialized`; `tools/list`.
2. Idle: sleep 30 s (`--settle`), read `VmRSS` (and `VmHWM`) from `/proc/<pid>/status`. Idle samples may run in parallel processes (`--parallel-idle`).
3. Peak: one `tools/call fetch` per fresh process; on return, before exit, read `VmHWM`.
4. Fixture server and harness on the same host over loopback (`bench/serve.py`).

Fixtures (`bench/fixtures.py`, fixed seed 1, sha256 and size committed in `bench/manifest.json`, harness refuses to run on mismatch; `bench/fixtures/` is not committed, regenerate with `fixtures.py generate` before the first real run; selftest does this itself): 5 MB HTML (5,241,856 B, 1 KiB under the cap so a correct server does not answer `too_large`), the same page gzipped (`Content-Encoding: gzip`; compressed bytes depend on the zlib build, re-commit the manifest deliberately if it changes), 50 MB HTML served with `Content-Length` and served chunked (no `Content-Length`), and a slow-drip route (`/slow`, 64 B/s). Late-landmark HTML and the remaining E-2 fixtures are not in the skeleton.

Scenarios (`bench/scenarios.py`). G0/G4a/G4b are gates; G1..G7 are scenario IDs (architecture 11.1):

| Harness ID | Scenario | Gate | Status in skeleton |
|---|---|---|---|
| `idle` | handshake + 30 s | G0, G4a, G4b (shipped) | implemented |
| `g4a-5mib-full` | 5 MB HTML read in full and converted (G1/G2 full form) | G4a | implemented |
| `g4a-5mib-gz` | same, gzip (G2) | G4a | implemented |
| `g4a-late-landmark` | late-landmark holdback-full HTML (G4) | G4a | defined, fixture is E-2 |
| `g4a-50mib-cl` | 50 MB with `Content-Length` -> `too_large` (G7a), <= 1.10x 5 MB | G4a | implemented |
| `g4a-50mib-chunked` | 50 MB chunked, no window, `too_large` at cap, <= 1.10x | G4a | implemented |
| `g6-concurrent10` | 10 concurrent (G1/G2 mix), recorded not gating | none | defined, E-4 |
| `g4b-window-start`, `g4b-window-end` (G1), `g4b-raw` (G3), `g4b-chunked-window-in-cap` (G7b), `g4b-window-beyond-cap` (G5, G7c) | need A-5 / A-6; same 40 MB target; 50 MB cases <= 1.10x | G4b | defined, args/windows are E-2 |

Non-gating, reported: default-parameter call, slow-drip (timing +-20% of `FETCH_TIMEOUT_MS`), TLS run. The harness refuses unimplemented scenarios (exit 2) rather than skipping them.

## 6. Valid-run rule and verdicts

A sample is valid only if the handshake succeeded, the outcome matches the scenario (`ok` or `too_large`), and the fixture server's byte counter for the route is >= the scenario `min_bytes` (early-stop check; the counter is an upper bound on client consumption). A scenario needs >= 10 valid samples, else the whole report is INVALID (exit 2; `--runs < 10` is refused unless `--smoke`, which is never valid for NFR claims). No outlier is discarded; min/median/max are reported and the decision uses the median. Exit codes: 0 pass, 1 target missed, 2 INVALID or refused (fixture hash, wrong binary kind, unimplemented scenario, fewer than 10 runs), 3 `--gate` off native aarch64. Output is JSONL: one `host` line, one line per scenario, one `summary` line (gating peak = max of medians, boundedness ratios, verdict).

## 7. Determinism list

Fresh process per sample; pinned child env (above); fixtures from fixed seed with committed hashes; page size, THP, CPU governor (performance preferred), load average before and after (> 0.5 above idle baseline marks the run suspect, repeat once), cgroup limit, libc (gnu/musl), allocator, binary sha256, commit, profile and toolchain recorded; ASLR left at default (recorded); one discarded dry-run of the matrix per session, recorded as such; native-ARM preflight; both gnu and musl binaries run; no concurrent load on the runner. Skeleton status: the host record, pinned env, hash check, preflight and binary sha are implemented; load-average repeat rule, dry-run discard, libc/allocator/profile/commit fields (from `--version`) and macOS `/usr/bin/time -l` are E-2.

## 8. TLS approach (decision)

NFR-11 is measured over plain HTTP on the fixture server. TLS is covered by a one-off manual run against real public hosts on the shipped binary (the same run that serves as the E-8 shipped-binary cross-check), reported separately. A bench-only fixture-CA build feature is not adopted: it would add a second trust-affecting compile-time route to guard beyond `bench-loopback`, for a small RSS question the manual run answers. Until the manual run exists, reports carry the caveat "NFR-11 measured over plain HTTP only". (Author recommendation for owner review.)

## 9. Goals 2, 3 and 5 method definitions (for A-4 and E-5)

- Token count: `tiktoken` encoding `cl100k_base` (pin the package version), `len(encode(text, disallowed_special=()))`. A reproducible proxy, not Claude's tokenizer. Used only in the offline A-4 check script, not in the memory harness.
- Baseline: the raw HTML body as captured in the snapshot (no conversion, no stripping). Reduction per page = 1 - tokens(markdown) / tokens(raw HTML); Goal 3 = median over the set (>= 50%).
- "Converts successfully": the converter returns no error and the markdown is non-empty (after whitespace trim). Goal 2 = successes / snapshots (>= 95%), on offline snapshots, not live fetches.
- Goal 5 overhead: conversion time of the 1 MB (1,048,576 B) fixture page, excluding network: call the converter in-process on the page held in memory, 5 warm-up calls discarded, 100 timed calls, report p95, on the native aarch64 runner, release profile; target <= 500 ms.
- 50-URL set: E-7 captures offline snapshots (HTML plus manifest with URL, date, size, sha256); the 10-URL live smoke list (network, TLS, redirects, JSON, plain text) is separate and non-gating. NOT DONE in E-1: the concrete URL list is not defined here (see dev report); it must be curated by the owner or E-7 and must exclude pages needing cookies or authentication.

## 10. G0 and go/no-go

A-1 was measured on x86_64 only (idle 3.7-5.2 MB, streaming-build peak 6.1 MB first page, 8.7 MB deep, 11.1 MB raw; buffered DOM 56 MB fails), before this protocol existed: 0.5 s settle, cap 16 MiB, kB medians of 10. Deviation recorded: A-1 has not been re-run under this protocol (no aarch64 access and no A-1 binary rebuilt in E-1); G0 requires that re-run or a gap analysis on the native aarch64 host. Preliminary recommendation: GO, conditional on the streaming design (a buffered DOM converter is NO-GO) and on native aarch64 confirmation, since x86_64 figures sit well under both targets but page size and allocator behaviour on ARM are unmeasured.
