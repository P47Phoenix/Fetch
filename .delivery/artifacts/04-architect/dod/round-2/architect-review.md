# Round 2 DoD review - Solution Architect perspective

SKILL_LOADED: architect
Verdict: DONE (0 blocking, 6 non-blocking)

## Gate checks
| Criterion | Result |
|---|---|
| FR/NFR coverage | Pass. FR-01..16 and NFR-01..15 each land in a module, ADR or test/bench section. NFR-08 met only by queueing (see NB-3). |
| Module boundaries | Pass. Dependency rule is acyclic (server -> fetch -> ssrf/convert/config/error); convert and ssrf::ranges are pure. |
| Story-to-module mapping | Pass. All 30 stories mapped (A-1..9, B-1..6, C-1..3, D-1..6 with D-4/D-5 merged, E-1..6). Story count 30 confirmed against EPICS. |
| ADR structure | Pass. ADR-001..006 each have Context, Options, Decision, Consequences, plus a flip-criteria section. |
| Spike numbers | Pass. Checked against the report: 3926/15060/2799, 3754/13792/2436, 3660/13976/2440 kB; htmd/html2md 56.4/56.5 MB, html2text 184.6 MB; streaming 6144/8726 kB, raw 11122 kB, idle 4684/4594; mimalloc idle 11.2-11.5 MB, peak deltas 5.9-16.9 MB; qemu 13.0/19.5 vs 4.7/6.1; binary sizes 2,957,304 / 2,904,584 B. Pins in 9.1 match spikes/a1/Cargo.lock (encoding_rs, webpki-roots, url, ring, rustls, tokio, reqwest, lol_html, rmcp). rustc 1.94.1 (e408947bf 2026-03-25) matches the local toolchain. No invented measurements: budgets are labelled as budgets. |
| OQ-3/4/5/7 | Pass. Section 12 and Prior Art mark all four open; the design keeps switch points (robots.rs optional, empty allowlist, one render hook, channel-agnostic release). |

## Budget arithmetic (recomputed)
- Items a..h = 1.0 + 0.0625 + 0.094 + 0.004 + 0.016 + 2.0 + 1.0 + 0.25 = 4.4265 (4.43 stated).
- i: 100,000 x 4 B = 0.4 MB = 0.381 MiB. k: 100,000 x 12 B = 1.2 MB = 1.144 MiB. j = 0.38.
- S1 = 4.43 + 0.02 + 0.02 + 0.06 = 4.53 (stated 4.5). S2 = 4.43 + 0.38 + 0.38 + 1.14 = 6.33 (stated 6.3). S3 = 1.1725 + 1.9 = 3.07 (stated 3.1).
- Concurrency 3: 10 + 3 x 6.3 = 28.9 (28.99 using 6.33), headroom about 11 of 40. n=4: 35.2 (35.3), n=5: 41.5 (41.7), so 5 fails, and 3 is the largest n with at least 8 MiB headroom under the ADR-004 flip rule (idle 10 + 3 x 6.33 = 29 <= 32; n=4 gives 35.3 > 32). Closes.
- Idle: budget uses the 10 MB ceiling, spike measured 4.6 MB (x86). Peak: 10 MiB = 10.5 MB, 29 MiB = 30.4 MB, still under 40 MB decimal. Closes against 10 idle / 40 peak, as a budget. No ARM measurement exists, which the doc states.
- Pipeline-freed-before-serialise is not assumed (additive sum), so the total is conservative.

## Non-blocking findings
1. ADR-006 "What ARM data would flip it" (b) still says "large max_length (200,000 chars)... consider lowering to 100,000". The cap is already 100,000 per the schema table and architecture 5.1. Stale wording; update to the current cap.
2. Dependency count: 9.1 pins `ring = "=0.17.14"` as a high-churn direct dep, but section 3 does not count ring as direct (13 listed + flate2 = 14). Pinning ring in Cargo.toml makes it direct (15/15), or serde_json (conditional) would break the ceiling. Either pin ring only via Cargo.lock/rustls feature, or recount and update R13.
3. NFR-08 ("10 in flight without errors") is met by semaphore 3 plus queue. A queued call errors with `timeout` if waits exceed FETCH_TIMEOUT_MS, so with slow real targets 10 concurrent calls can produce errors. Fine on localhost fixtures. Recommend the PO explicitly accepts "10 issued, 3 in flight" as satisfying NFR-08, and G6 asserts zero errors on the fixture.
4. ADR-001 consequences say "~1 MB more peak" for reqwest vs hyper, while its Context and the spike give ~1.3 MB (1268 kB). Harmless but make it consistent.
5. Cosmetic: ADR-002 drop-rule sentence has a stray backtick and review IDs ("NB-4") inside the ADR text; architecture 13.1 items numbered 8, 10, 11, 9 out of order; ADR-002 Option A "~4-5 MB for a 512 KB cap" is a derived estimate, labelled with ~ but not a measurement.
6. Rows f (2 MiB lol_html limit), g (1 MiB emitter), a (1 MiB network) are allocations, not measurements. That is disclosed, and the 11 MiB headroom is the guard. R1/R14 track them; A-2/A-3 must verify. Not a gap in the design.

## Notes for downstream (not findings)
- The E-4 size-error vs early-stop rule and the Required doc changes are correctly marked PROPOSED and need PO confirmation before Stage 5.
- Section 13.2 "ship the label ON if OQ-5 unanswered" is recorded as a recommendation only, not a decision. Acceptable.
