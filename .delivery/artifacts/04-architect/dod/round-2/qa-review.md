# QA Review, Stage 4 Round 2

STATUS: DONE (0 blocking, 4 non-blocking)

Verified: layered strategy (sec 10) covers unit, property, integration with fake resolver, SSRF table tests, resource abuse, MCP e2e, 90% coverage gate, 50-URL offline quality set, native ARM.
Benchmark (sec 11): median of >=10 fresh processes; VmRSS idle (30 s) and VmHWM peak; absolute caps; gating peak = max of per-scenario medians over G1-G7 that force near-full consumption (window at end, late landmark, gzip, raw, cap-reach, concurrency). Server-side byte counter with expected_min_bytes invalidates early-stopped samples. Determinism: seeded fixtures with sha256 manifest, env allow-list, governor/THP/pagesize/load recorded, no outlier drops, <10 valid runs invalidates the report. Native-ARM preflight rejects qemu binfmt; QEMU is functional-only and never gating; ARM jobs are required checks (skipped is failure).

## Non-blocking
1. E-4 vs early-stop (6.4) rule is still PROPOSED. EPICS E-4 AC2, A-3 AC1 and FR-07 conflict with it until the PO confirms and amends. Must be closed before Stage 5.
2. G7 "50 MB <= 1.10 x 5 MB" does not name the pairing. State that G7b/G7c are compared with G1/G5 medians; G7a (header abort) is trivially small.
3. Live-smoke 10 URLs is non-gating, and the 95% success metric is offline conversion success. EPICS E-1 wording should be amended (arch sec 14 item 6 already lists it).
4. G6 must run under the default semaphore of 3, and NFR-08 (10 in flight, no errors) needs an explicit assertion that all 10 succeed. Also state the idle baseline for the load-average threshold.
