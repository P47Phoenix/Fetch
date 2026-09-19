# QA / Testability Review, Stage 4 (Architect)

Reviewer: QA Engineer validator. Artifacts: architecture.md (sections 5, 6.4, 10, 11), ADR-001..006, EPICS E-1..E-6, PRD NFR-10..14.
Status: NOT_DONE (2 blocking, 6 non-blocking)

## What passes
- Design is testable: `ssrf::ranges` and `convert` are pure; injected fake `Resolve`; in-process HTTP servers; test-only loopback policy guarded by cfg/feature, with a CI check that release has no `test-support` (R12).
- Strategy covers unit, property (pagination concatenation, chunk-boundary invariance), goldens, integration, SSRF (ranges, rebinding, redirect to 127.0.0.1/file:), MCP e2e stdout purity, resource abuse, coverage gate 90% on ssrf/redirect/window, 50-URL quality set, ARM.
- Benchmark: median of >=10 fresh processes, min/max, 30 s idle, VmRSS idle / VmHWM peak, absolute caps with non-zero exit, host metadata recorded (incl. PAGESIZE, libc, allocator).
- QEMU never gates memory (section 9 states it explicitly, backed by spike data). Confirmed.
- E-4 vs early stop conflict is correctly detected (6.4) with a sound three-fixture rule.

## Blocking

B1. NFR-11 headline peak can be under-measured because of early stop (sec 5.2 vs sec 11).
Default params (max_length 5000) plus early stop mean a plain 5 MB fetch may stop after the first few chunks and report a tiny VmHWM, so the 40 MB gate passes without exercising the worst case. Section 11 lists scenarios but does not say which one is the gating "5 MB page" figure. Required: define the gating peak as the maximum VmHWM across scenarios that force full consumption (5 MB identity and gzip with the window at end / deep start_index, raw=true, hold-back worst case for ADR-002 main-content selection, 10-concurrent), and state that a run which early-stops before reading the body is not a valid NFR-11 sample. Add a benchmark self-check asserting bytes actually read from the fixture server (server-side counter) >= expected, so the harness cannot silently measure a short-circuit.

B2. Benchmark determinism is not specified.
Nothing fixes: fixture generation seed and checksum (spike fixture was synthetic; E-2 needs reproducible bytes), warm-up policy, fresh-process rule (stated) plus page-cache/THP/ASLR-independent environment, CPU governor/frequency and no-concurrent-load on the runner, env var set (allocator tunables such as MALLOC_ARENA_MAX, RUST_LOG), outlier rule (median only, but what if fewer than 10 runs succeed), and slow-drip timing tolerance. Required: add a "determinism" list to section 11 (seeded fixtures with committed sha256, pinned env, run-count invalid-if <10, record governor and load average, drop nothing silently) and state that the pass/fail decision uses the median exactly as PRD defines it.

## Non-blocking

N1. EPICS E-4 AC 2 ("50 MB fixture returns a size error") conflicts with early stop for chunked bodies. Design's rule (6.4) is right but awaiting PO. Action: PO amends E-4 to the three fixtures (Content-Length 50 MB -> too_large; chunked 50 MB with window inside cap -> success; chunked with window beyond cap -> too_large), and E-2 fixtures ("50 MB streaming body") gain the header/no-header variants. Must be resolved before Stage 5 planning; treat as a tracked handoff.

N2. ARM runner verification plan is thin. Sec 9/11 record CPU and PAGESIZE but nothing proves the runner is native and not emulated. Add a preflight in the harness: `uname -m` = aarch64, no qemu binfmt handler registered for the binary (`/proc/sys/fs/binfmt_misc`), `file` on binary shows ARM aarch64, and refuse to emit gating numbers otherwise. Also note that 16K/64K page size hosts inflate RSS; record and compare only same-page-size runs.

N3. E-3 says "at or below 10 MB" while E-6/section 11 says CI fails only above cap + 10%. Document that E-3/E-5 release gate is strict, E-6 CI gate is cap x 1.10 (regression tripwire), so the two are not read as contradicting.

N4. E-4 AC 4 (allocator comparison) and ADR-005 gnu-vs-musl need ARM numbers; ensure the harness runs both binaries and the report format has a column for libc/allocator. Sec 11 records these but no scenario matrix ties them.

N5. HTTPS coverage left to E-1. TLS cost can move peak; the decision should be recorded before E-4 gates, or the NFR-11 claim is only for plain HTTP. Mark the caveat in the E-5 report template.

N6. 50-URL quality set uses offline snapshots: this measures conversion, not live fetch success (Goal 2 success rate includes network/TLS/redirect behaviour). State that snapshot success != live success, and add a small live smoke run (non-gating, documented) or define success as conversion-only. Also state that quality thresholds (95% success, 50% token reduction) are unproven until A-4 (ADR-002 status Proposed); a failing threshold has a defined fallback (whole-body, raw).

## Verdict
Two blocking gaps (B1, B2) are cheap to fix in section 11 without redesign. After they are addressed, DONE.
