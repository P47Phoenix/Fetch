# Stage 4 DoD review: Solution Architect validator

Artifact: architecture.md + ADR-001..006. Verdict: NOT_DONE (1 blocking, 7 non-blocking).

## Checks passed
- All ADRs have context / options / decision / consequences (plus "what ARM data would flip it").
- Spike numbers cited match a1-spike-report.md: reqwest 3926/15060/2799 kB, hyper 3754/13792/2436, ureq 3660/13976/2440, htmd 56422, html2md 56462, html2text 184612, mimalloc idle ~11.2-11.5 MB, mimalloc peaks 62.1-73.2 MB, streaming 6144 / 8726 / 11122 kB raw, binary 3193 kB, qemu 13.0/19.5 vs 4.7/6.1. Delta claims (+41 MB, 6-17 MB) check out. No invented measurements; budgets are labelled as budgets.
- Story mapping: EPICS.md contains 30 story IDs (A-1..A-9, B-1..B-6, C-1..C-3, D-1..D-6, E-1..E-6). All 30 appear in architecture.md s15. The task brief said 32; EPICS has 30 (NB-7).
- FR-01..16 and NFR-01..15 have a design home. Open questions OQ-3/4/5/7 are explicitly not decided; design keeps switch points. Sec 6.4 (size error vs early stop) is flagged to PO.
- Dependency budget 13 of 15 counted correctly.

## Blocking
B-1. Memory budget arithmetic does not close (architecture.md 5.1). The per-fetch items sum to about 8.2 MiB, not the stated "6 MiB design ceiling":
1 (net) + 0.06 (chunk) + 0.09 (gzip 32K+64K) + 0.004 + 0.016 (decoder) + 2 (lol_html) + 1 (emitter) + 0.25 (holdback) + 0.76 (window, 0.8 MB) + 3 (JSON copy) = ~8.2 MiB.
With that, the stated concurrency bound 10 + 4x6 = 34 MB becomes 10 + 4x8.2 = ~42.8 MB, over the 40 MB gate. The tables' own rounding claim is wrong, and the "24 MB headroom" and the ADR-004 flip formula (10 + n x delta <= 34) inherit it. Fix: either recompute the ceiling (about 8.5 MiB) and reduce FETCH_MAX_CONCURRENCY or the individual items, or state that not all worst cases coincide (raw path has no lol_html; JSON copy only at max_length cap) and give a worked worst-case for each scenario. Also the JSON-RPC copy is assumed to be a single copy; rmcp clone counts are not verified.

## Non-blocking
NB-1. ADR-001 Options says hyper is "~1.1 MB smaller binary"; spike is 2799 vs 2436 kB (~0.36 MB), and the same ADR's Consequences say ~0.4 MB. Fix Options text.
NB-2. Error routing inconsistency: architecture 6.1 lists scheme violations under `blocked_target`, ADR-003 maps non-http(s) redirect Location to `blocked_target`, but ADR-006 says non-http(s) schemes (and userinfo) in the `url` param are JSON-RPC invalid params. Pick one for the initial URL.
NB-3. Byte-cap design counts wire and decompressed bytes separately, but reqwest's `gzip` feature auto-decompresses and hides wire bytes; ADR-004 needs to say whether decoding is done manually (async-compression on the raw stream) or the wire counter is dropped. Also affects the dependency count.
NB-4. ADR-002 drop list contains a garbled entry ("form (unless it contains main text is NOT evaluated: forms dropped)") and drops all `form` elements. ASP.NET-style pages wrap the whole body in one `<form>`, which would erase content and contradicts the stated "conservative bias". Decide and rewrite.
NB-5. FR-07 acceptance ("10 MB response aborts at the limit with error") conflicts with early-stop success on chunked bodies; ADR-004 is marked Accepted while the PO confirmation (6.4) is pending. Mark that rule Proposed until confirmed.
NB-6. Architecture asserts "no tracing" dependency; rmcp may pull `tracing` transitively. Confirm at A-2 (transitive only, not counted, but affects RSS/binary).
NB-7. Brief said 32 stories; EPICS has 30. Confirm nothing is missing on the orchestrator side.
