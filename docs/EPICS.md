# Epics and Stories: Fetch MCP Server (Rust, low-memory, ARM)

Source: `docs/PRD.md` v0.4 (34 stories in total; A-3 was split into A-3a and A-3b on 2026-09-19; D-7 and E-7 added in plan revision 1; E-8 added in plan revision 2 and CONFIRMED by the user on 2026-09-19). Story IDs use the epic letter. Sizes are Fibonacci points (1,2,3,5,8); no story exceeds 8. "Spike" stories are time-boxed and produce a decision, not shipped features. **Revision (ADR-007, user decisions 2026-09-19):** memory gates run natively on GitHub-hosted runners for BOTH linux/amd64 (`ubuntu-24.04`) and linux/arm64 (`ubuntu-24.04-arm`), no self-hosted runner; the release artifact is a multi-arch GHCR image published only after both platform gates pass on the exact digests; standalone binary releases are dropped. D-2 re-estimated 5 to 8 (unvalidated); total 97 to 100 points, MVP 73 to 76, MVP now end of Sprint 10 (D-4 moves), v1.0 still Sprint 12; D-3 flagged for downward re-estimate; E-2, E-3, E-4, D-1 keep their points with the amd64 matrix noted as a risk (E-2 has zero slack in Sprint 3: A-9 moves to Sprint 5 first if needed).

Assumed capacity: solo part-time, about 10 points per 2-week sprint; commitment capped at 80% (8 points).

## Ordering Rationale

1. **Risk first.** The whole project is justified by a memory claim (PRD Goal 1). The benchmark harness and absolute targets (E-1) and the `rmcp`/crate spike (A-1) run first, because a failed memory case or an unworkable crate stack invalidates everything later. Cost of delay on these is negative: building features first risks wasted work.
2. **Measure early, not last.** The benchmark harness (E-2 to E-4) lands right after the first streaming fetch, so memory regressions are caught while the code is small. Trade-off: some harness effort is spent before all features exist; accepted.
3. **Value density.** Core fetch (A) delivers the core value. Network safety (B) is next because an unsafe fetcher is not acceptable. Config (C) and polish stories follow. robots.txt and content labelling are lowest value density and depend on open questions, so they go last.
4. **Packaging (D) split.** The aarch64 cross-build is proven in A-1 (risk); D-2 (Sprint 9) builds, tests by digest and publishes the container image (ADR-007); docs/release finish at the end.
5. **Interim safety.** Because the first fetch-capable build (A-3b, Sprint 2) precedes B-1 (Sprint 6), the full SSRF range table and checks land first in A-3a (Sprint 1), and A-3b cannot merge without them (architecture 14.1). B-1/B-2/B-3 own test depth and hardening.

## Release Rules (binding, from Stage 4 architecture)

- No tagged or distributed build before M3 (Safety complete). Pre-M3 builds are not registered in a real MCP client (PRD Risk 2 mitigation).
- A-3a (SSRF core) lands before A-3b (fetch client) and A-3b merges only if A-3a is on the branch (merge gate). A-3b is not Done until an integration test through the real client proves `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused. No fetch-capable build may exist without the range table and fail-closed default policy.
- Bench loopback (CONFIRMED by the user 2026-09-19; plan revision 2, confirmed in round 3): the shipped release binary keeps the fail-closed policy and cannot reach the loopback fixture server. Peak-RSS scenarios (E-2, E-4, G4a, G4b) therefore run on a separate `bench-loopback` build of the same commit, made by the same pinned pipeline; idle RSS is gated on the shipped release binary. The feature is off by default, is never enabled in a release, tag or distributed artifact, and is asserted absent by the D-7 guard. See E-8. This does not decide OQ-4.
- Required architecture change (not made here; architecture and ADR files are not edited in Stage 5): architecture section 11 item 7 (bench-only fixture-CA feature) and section 9 R12 / ADR-003 (`test-support` as the only loopback route) must be extended to name a second compile-time-only route, `bench-loopback` (loopback ranges only), and the D-7 guard and section 11.2 must state which binary each gate measures. It must also record that the release profile (opt-level, lto, panic=abort, strip, codegen-units) is fixed in D-7 (Sprint 0), that every gate measures it, and that G4 is split into G4a (end of Sprint 4, idle plus scenarios needing only A-3b and A-4) and G4b (end of Sprint 5, window and `raw=true`).
- Release profile pinned early (user decision 2026-09-19): D-7 (Sprint 0) fixes the release profile (opt-level, lto, panic=abort, strip, codegen-units) in `Cargo.toml` so that G0, G4a, G4b and E-5 all measure the profile that ships. D-1 (Sprint 8) finalises it and must re-measure idle and peak on the shipped build, including on the aarch64 runner; exceeding a target blocks MVP tagging. No points change.
- Memory gate: E-3/E-5 use the strict absolute targets. E-6 is a regression tripwire (see E-6).

## Epic-to-Requirement Map

| Epic | PRD IDs |
|---|---|
| A Core fetch and conversion | FR-01, FR-02, FR-03, FR-04, FR-07 (size/timeout enforcement), FR-08, FR-09, FR-10, FR-13, FR-14, FR-16, NFR-01, NFR-02, NFR-08 |
| B Network safety | FR-05, FR-06, FR-11, NFR-04, NFR-07, Risk 2, Risk 5 |
| C Configuration and policy | FR-07, FR-12, FR-06 (allowlist) |
| D Packaging, ARM builds, docs | FR-15, NFR-05, NFR-06, NFR-09, NFR-13, NFR-15, US-8 |
| E Memory benchmarking and validation | NFR-10, NFR-11, NFR-12, NFR-14, Goals 1a-1d, US-7, US-9 |

---

# Epic E: Memory benchmarking and validation

**Epic Goal:** Prove, with reproducible measurements on ARM, that the Rust server meets its absolute memory targets (idle <= 10 MiB RSS; peak <= 40 MiB while fetching a 5 MiB page), and keep it that way.

**Success Metric:** Report shows idle RSS at or below 10 MiB and 5 MiB-fetch peak RSS (VmHWM) at or below 40 MiB on aarch64-linux, each the median of 10 runs; 50 MiB-response test stays within 10% of the 5 MiB peak; CI gate active.

**Out of Scope:** CPU or latency benchmarking beyond NFR-02; non-ARM benchmark tuning.

### Story Map

| # | Story | Value | Effort | Dependencies |
|---|---|---|---|---|
| E-1 | Spike: define benchmark harness and absolute targets | Critical (go/no-go) | 3 | None (OQ-9 resolved) |
| E-2 | Benchmark harness and fixtures | High | 5 | E-1, A-3b, E-8 |
| E-3 | Idle RSS measurement and target check | High | 2 | E-1, E-2, A-2 |
| E-4 | Peak RSS and boundedness checks | High | 3 | E-1, E-2, E-8, A-3b, A-4 |
| E-5 | Benchmark report and release decision | High | 2 | E-3, E-4, A-7 |
| E-6 | CI regression gate on aarch64 | Medium | 3 | E-2, D-2 |
| E-7 | Offline 50-URL snapshot set | High | 2 | E-1, A-3b |
| E-8 | Bench-only loopback policy build (CONFIRMED) | High | 1 | A-3a, D-7 |

**MVP Slice:** E-1, E-2, E-3, E-4, E-5, E-7, E-8. Rationale: these prove or disprove the primary claim. E-6 protects it after release and can follow.

### E-1: Spike - define benchmark harness and absolute targets (3 pts) [SPIKE, time-box 2 days]
Maps to: NFR-10, NFR-11, NFR-14, Goals 1a, 1b, 2, 3.
As a solo developer, I want the measurement method and absolute memory targets fixed up front so that the go/no-go decision rests on agreed numbers.
- Given the targets, when the spike ends, then a one-page result states idle RSS (VmRSS) <= 10 MiB and peak RSS (VmHWM) <= 40 MiB while fetching a 5 MiB page, each as the median of 10 valid runs, and states that the unit is MiB (2^20 bytes) for the 10 MiB, 40 MiB and 5 MiB figures and the 5 MiB fetch cap (10 MiB = 10,240 kB and 40 MiB = 40,960 kB as Linux `/proc` reports kB = KiB; applied consistently in the harness, fixtures, config default and report; other documents refer to this definition).
- Given the benchmark host, when the spike ends, then the result records a per-platform host-facts table for the GitHub-hosted arm64 runner (`ubuntu-24.04-arm`) and amd64 runner (`ubuntu-24.04`) (public repo; ADR-007; cloud VMs, not the Pi 5 cluster), each with OS, kernel, CPU model, RAM and `getconf PAGESIZE` captured from the actual run. The A-1 spike is re-run on both under the protocol; the earlier x86 figure is preliminary.
- Given the measurement protocol, when written, then it defines the handshake and 30 s idle procedure, the 5 MiB, 50 MiB (with and without `Content-Length`) and slow-drip fixtures, the MCP client script, and the `/proc/<pid>/status` read method.
- Given plain-HTTP fixtures under-measure TLS, when the spike ends, then it decides the TLS benchmark approach (bench-only fixture-CA build feature absent from release builds, or a one-off manual run against real hosts); until decided, reports carry the caveat "NFR-11 measured over plain HTTP only".
- [DEFERRED, owner: project owner, user decision Sprint 0 revision 4: the 50-URL list and the 10-URL smoke list are not produced in Sprint 0 and must exist before E-7 starts in Sprint 2] Given the 50-URL curated set, when defined, then it lists the URLs as offline snapshots used for conversion success rate and token reduction (Goals 2 and 3); the success metric is conversion success, not live fetch success, and a small non-gating live smoke run (10 URLs) covers network, TLS and redirect behaviour.
- Given the shipped binary blocks loopback (A-3a fail-closed default) and the fixture server is on 127.0.0.1, when the spike ends, then it records the confirmed loopback path (E-8 `bench-loopback` compile-time feature, confirmed by the user 2026-09-19, see E-8), and states which binary each gate measures. It does not decide OQ-4.
- Given Goals 2, 3 and 5, when the spike ends, then it defines the tokenizer and count method for token reduction, the baseline (raw HTML body, no conversion), what "converts successfully" means (no error and non-empty markdown), and the overhead measurement for Goal 5 (conversion time of a 1 MiB page, excluding network), so A-4 checks are reproducible.
- Given G0 uses A-1 measurements taken before this protocol existed, when the spike ends, then it either re-runs the A-1 binary under this protocol (median of 10 valid runs, MiB unit above, release profile as pinned in D-7 or the deviation noted) or records the deviation.
- Given the protocol and A-1 results, when the spike ends, then it states a go/no-go recommendation on whether the targets look achievable.

### E-2: Benchmark harness and fixtures (5 pts)
Maps to: NFR-14, Goal 1d.
As the project owner, I want a one-command harness so that memory measurements are repeatable.
- Given the repo, when I run the single harness command, then it starts a local fixture HTTP server, drives a target MCP server over stdio, and prints idle and peak RSS.
- Given the harness is pointed at a server binary, when it runs, then it uses the client script and fixtures defined in E-1.
- Given a run, when results are produced, then each figure is the median of 10 valid runs with min and max shown, and the harness rejects (non-zero exit) a result set with fewer than 10 valid runs. The unit is MiB (E-1), used for targets, fixtures and caps alike.
- Given Linux and macOS hosts, when the harness runs, then it reads `/proc/<pid>/status` on Linux and `/usr/bin/time -l` on macOS.
- Given fixtures, when the harness starts, then it provides a 5 MiB HTML page, a 50 MiB body served both with and without `Content-Length` (chunked), a gzip variant, and a slow-drip response; each fixture is generated from a fixed seed with a committed sha256 and size in a manifest, and the harness refuses to run on a hash mismatch.
- Given the gating scenarios G1-G7 (5 MiB HTML window at end; same gzip; `raw=true`; late-landmark holdback-full HTML; window beyond the 5 MiB cap expecting `too_large`; 10 concurrent calls; 50 MiB with Content-Length, chunked window inside cap, chunked window beyond cap), when the harness runs, then the gating peak is the maximum of the per-scenario medians, excluding the 10-concurrent scenario, which is recorded and reported (see E-4) and does not gate. Assignment: G4a (end of Sprint 4) uses only full-consumption scenarios that need A-3b and A-4 (5 MiB HTML fully read and converted, same page gzipped, late-landmark holdback-full HTML, 50 MiB with Content-Length, 50 MiB chunked read without a window beyond the cap); G4b (end of Sprint 5) adds the scenarios that need A-5 or A-6 (window at start, window at end, `raw=true`, chunked window inside the cap, window beyond the cap). This story keeps the scenario definitions and harness runnable for all of them; the G4b runs are owned by A-5 and A-6.
- Given the validity rule, when a run early-stops before reading the expected amount (fixture server byte counter below the manifest `expected_min_bytes`), then that sample is invalid, and fewer than 10 valid samples for any scenario makes the whole report INVALID.
- Given Sprint 3 precedes the D-2 pipeline, when the harness builds the gnu and musl candidate images for BOTH linux/amd64 and linux/arm64 (natively on the matching hosted runner, ADR-007), then it uses a documented interim build script derived from the A-1 spike with `cargo-zigbuild` and `ziglang` versions pinned from the start (D-2 later adopts the same pins), and the hosted amd64 and arm64 runner jobs follow architecture 9.2 (hosted, least-privilege token, no `pull_request_target`, no secrets on fork PRs) with OS, RAM, CPU model and page size recorded.
- Given the shipped binary blocks loopback, when the harness runs peak-RSS scenarios, then it targets the `bench-loopback` build from E-8 (built from the same commit, Cargo.lock hash and release profile as pinned in D-7 by the same interim pinned script, and both binaries report commit and Cargo.lock hash in `--version` so the harness can assert they match), refuses a peak run against a binary lacking the bench marker, refuses a bench-marked binary for the shipped-binary idle check, and labels every figure with the binary used; idle RSS is also run on the shipped release binary (no fetch needed).
- Given Sprints 3-8 have no D-2 pipeline, when this story closes, then it creates the hosted benchmark workflow as a two-platform matrix (`ubuntu-24.04` and `ubuntu-24.04-arm`; nightly, manual dispatch and same-repo PRs; the tag trigger with the digest gate is added by D-2) per architecture 9.2, running the harness against the container image (process VmRSS/VmHWM read from `/proc/<pid>/status`, ADR-007 and 9.3), not a bare binary.
- Given the determinism list (fresh process per sample, pinned child environment, recorded page size, THP and load average (the CPU governor is not controllable on hosted runners and is not recorded as a rule), native-ARM preflight refusing QEMU, both gnu and musl binaries), when the harness runs, then it applies and records each item; idle samples may run in parallel processes; the full matrix runs nightly and on main and tag builds.

### E-3: Idle RSS measurement and target check (2 pts)
Maps to: NFR-10, US-7.
As a developer on ARM, I want idle memory verified against target so that a resident server stays small.
- Given the release binary on aarch64-linux, when idle 30 s after `initialize` and `tools/list`, then RSS is at or below 10 MiB.
- Given the result exceeds the target, when the harness finishes, then it exits non-zero and prints the measured value and the target. This E-3/E-5 gate is the strict absolute target (no tolerance).

### E-4: Peak RSS and boundedness checks (3 pts)
Maps to: NFR-11, NFR-12, FR-16, US-7.
As a developer on ARM, I want peak memory verified during fetches so that large pages cannot exhaust RAM.
- Given the 5 MiB HTML fixture, when `fetch` runs on the `bench-loopback` build, then peak RSS is at or below 40 MiB; the shipped-binary cross-check (idle delta versus bench build, binary size delta, and one manual 5 MiB fetch of a public host on the shipped binary) is recorded and checked against the E-8 bounds.
- Given Sprint 4 precedes A-5 (window and early stop) and A-6 (`raw=true`), when G4a is evaluated, then it covers idle plus only scenarios that need A-3b and A-4: 5 MiB HTML fully read and converted, the same page gzipped, late-landmark holdback-full HTML, 50 MiB with Content-Length, and 50 MiB chunked read without a window beyond the cap; 10 concurrent recorded, not gating. Nothing that needs the window, early stop or `raw` is in G4a. Those scenarios (window at start, window at end, chunked window inside the cap, window beyond the cap, `raw=true`) are gated as G4b at the end of Sprint 5 against the same targets, owned by A-5 and A-6 (non-blocking for E-4's own Done). The complete memory gate is therefore closed at the end of Sprint 5, not Sprint 4.
- Given G4a, when it is evaluated, then it runs on the release profile pinned in D-7 for both gnu and musl, on native aarch64 only, each figure the median of 10 valid runs; the shipped-vs-bench delta bounds in E-8 are part of the pass.
- Given the 50 MiB fixture served with `Content-Length` and max size 5 MiB, when `fetch` runs, then the call returns a `too_large` error and peak RSS is within 10% of the 5 MiB-page peak.
- Given the 50 MiB fixture served chunked (no `Content-Length`) and a full read without a window, when `fetch` runs, then reading stops at the cap, the call returns a `too_large` error and peak RSS is within 10% of the 5 MiB-page peak. (The windowed variants, chunked window inside the cap succeeding and window beyond the cap, need A-5 and are G4b scenarios.)
- Given 10 concurrent fetches of the 5 MiB page, when they run, then peak RSS is recorded and reported (NFR-08 documentation).
- Given allocator candidates from A-1, when compared here, then the chosen allocator and its RSS effect are recorded.

### E-5: Benchmark report and release decision (2 pts)
Maps to: Goals 1a-1d, US-9.
As the project owner, I want a written report so that I can decide to release and register the server in my Claude Code config.
- Given E-3, E-4 and the G4b results, when the report is generated, then it states the release profile measured (pinned in D-7, finalised and re-measured in D-1, Sprint 8; the D-1 re-measure is the figure that governs the MVP tag) and it lists idle RSS, peak RSS, and 50 MiB boundedness against their absolute targets.
- Given the Goal 2 wording (offline conversion success), when the report is generated, then it includes the 10-URL live smoke result (network, TLS, redirects, JSON and plain text; non-gating) and the Goal 5 overhead figure defined in E-1.
- Given the report shows every target met, when it is published to `docs/`, then it states "release" and lists the config change needed.
- Given any target missed, when it is published, then it states the gap and the follow-up actions.

### E-6: CI regression gate on aarch64 (3 pts)
Maps to: NFR-14, NFR-15.
As the project owner, I want CI to fail on memory regressions so that the saving persists.
- Given a pull request, when CI runs on the hosted amd64 and arm64 runners, then it executes the E-3 and E-4 checks against the built binary.
- Given the median idle or peak RSS exceeds the absolute target, or regresses more than 10% against the stored last-main baseline, when CI runs, then the job fails and prints the figures. The 10% is relative to the baseline and never a licence to exceed the absolute target; E-6 is a regression tripwire, while the release gate (E-3/E-5) is the strict absolute target.
- Given E-6 lands (this story creates the memory-gate job), when it is merged, then the memory-gate job is added as a required status check and the release workflow from D-2 is updated to depend on it (moved here from D-2, plan revision 1).
- Given a hosted runner (either platform) is unavailable or its job is skipped, when CI runs, then the job fails or blocks (a skipped job never reads as green: it is a required status check and the release workflow depends on it); the only accepted fallback is a `workflow_dispatch` re-run against the same image digest; a local or Pi run is informational and is not accepted as release evidence (ADR-007).

### E-7: Offline 50-URL snapshot set (2 pts)
Maps to: Goals 2 and 3 (inputs for A-4).
As the project owner, I want the 50 curated pages captured once as offline snapshots so that conversion success and token reduction are measured against a fixed set.
- [Prerequisite: the URL list is DEFERRED from E-1 to the project owner; E-7 cannot start without it] Given the URL list from E-1, when the snapshots are captured (a script using the A-3b client or `curl`, run once by the author), then each page is stored as an HTML file with a manifest carrying its URL, capture date, size and sha256, and the harness or test refuses a hash mismatch.
- Given the set, when committed, then the total size and licensing of the stored pages are recorded and the set contains no page that needs cookies or authentication.
- Given the set, when the 10-URL live smoke list is defined, then it is stored as a separate non-gating list.
- The 95% conversion success and 50% median token reduction checks run in A-4 (Sprint 4); this story only supplies the set.

### E-8: Bench-only loopback policy build (1 pt) [CONFIRMED by the user 2026-09-19; added in plan revision 2]
Maps to: NFR-14, NFR-11, Goal 1d, Risk 2.
As the project owner, I want the memory benchmark to reach its loopback fixture server without weakening the shipped binary's SSRF policy, so that peak RSS is measured on code identical to what ships except the address policy.
Options evaluated: (A, chosen and confirmed) compile-time Cargo feature `bench-loopback`, off by default, same pinned pipeline, a second binary; (B) runtime env/config switch, rejected because it ships a bypass in the release binary and overlaps OQ-4 (private-host allowlist, OPEN); (C) in-process harness (does not measure the server process), network namespace or non-loopback fixture address (RFC 1918 addresses are also blocked, so a test resolver or public host is still needed), or a public-IP fixture (not reproducible), rejected as the primary path (one public-host fetch is kept as the non-gating cross-check).
- Given the feature `bench-loopback` is enabled, when the policy is constructed, then only 127.0.0.0/8 and ::1 additionally pass; every other blocked range (private, link-local, metadata, CGNAT, ULA, unspecified) still refuses, resolver filter, per-hop revalidation and dial-once behaviour are unchanged, and a unit test proves each still refuses under the feature.
- Given the feature is absent (default and every release, tag or distributed build), when the binary is built, then loopback is refused; the D-7 release guard fails the build if `bench-loopback` or `test-support` appears in `cargo tree -e features` for the release profile, or if the marker string `bench-loopback` appears in the binary; the A-3b four-refusal integration test runs against the shipped-profile build.
- Given a `bench-loopback` build, when it starts, then it logs a marker to stderr and reports it in the harness handshake, and the harness and E-5 report label every figure taken with it as "bench build".
- Given both binaries built from one commit, when compared, then the only source difference is the cfg'd policy constructor; the size delta and idle RSS delta are recorded, and the bounds are: idle delta at most 0.5 MiB, binary size delta recorded (no bound, must be explained), and one manual 5 MiB fetch of a public host on the shipped binary with peak within 10% of the bench-build peak and at or below 40 MiB. Exceeding any bound fails G4a until explained and re-measured, so a diverging bench build cannot pass. Both binaries must report the same commit and Cargo.lock hash in `--version`.
- Given the release workflow (D-2), when it runs, then it never sets the feature and never uploads the bench binary.
Effect on "what is measured is what ships": idle RSS is gated on the shipped binary; peak RSS is gated on the bench build, identical code except the address-range check, backed by the delta record and one shipped-binary public-host fetch. This is a stated approximation; the owner confirmed this path on 2026-09-19.

---

# Epic A: Core fetch and conversion

**Epic Goal:** A working `fetch` MCP tool in Rust (`rmcp`, stdio) that retrieves a URL through a streaming, size-bounded pipeline, converts HTML to markdown, paginates, and reports errors clearly.

**Success Metric:** 50-URL test set >= 95% success; median token reduction >= 50%; all Must FRs in this epic pass; parameters `url`, `max_length`, `start_index`, `raw` as specified.

**Out of Scope:** JS rendering, PDF/binary extraction, caching, batch fetch, authenticated fetch.

### Story Map

| # | Story | Value | Effort | Dependencies |
|---|---|---|---|---|
| A-1 | Spike: `rmcp`, HTTP, and HTML-to-markdown crate choice | Critical (risk) | 3 | E-1 in parallel |
| A-2 | Walking skeleton: stdio server with `fetch` schema | High | 3 | A-1 |
| A-3a | SSRF core: range table, resolver filter, fail-closed policy | Critical | 5 | A-2 |
| A-3b | Streaming, size-bounded HTTP fetch client | High | 5 | A-2, A-3a |
| A-4 | HTML to markdown conversion | High | 5 | A-3b |
| A-5 | Pagination with `max_length` and `start_index` | High | 3 | A-4 |
| A-6 | Content-type handling and `raw` mode | High | 3 | A-3b |
| A-7 | Cause-specific structured errors | High | 3 | A-3b |
| A-8 | Charset decoding and User-Agent | Medium | 2 | A-3b |
| A-9 | Final URL and status header | Low | 1 | A-3b |

**MVP Slice:** A-1 to A-7 (A-3 as A-3a and A-3b). Rationale: this is the minimum core behavior (fetch, convert, paginate, raw, errors). A-8 (charset, UA) and A-9 (header) are refinements; UTF-8 default covers most pages.

### A-1: Spike - `rmcp`, HTTP client, and HTML-to-markdown choice (3 pts) [SPIKE, time-box 3 days]
Maps to: FR-01, FR-15, NFR-05, NFR-15, Risks 1, 3, 9.
As a solo developer, I want to validate the Rust crate stack early so that later stories rest on known-good choices.
- Given the `rmcp` crate, when a minimal stdio server exposing one tool is built, then a client completes `initialize` and `tools/list` and the chosen `rmcp` version and feature flags are recorded.
- Given `reqwest` and `hyper` both with `rustls`, when each streams a 5 MiB body into a capped buffer, then RSS and binary size are recorded and one is selected with rationale.
- Given at least two HTML-to-markdown approaches (for example a DOM-based converter and a streaming rewriter such as `lol_html`), when each converts a 5 MiB page and 10 sample pages, then output quality notes, peak RSS, and time are recorded and one is selected.
- Given glibc and one alternative allocator, when the spike binary runs under load, then the RSS difference is recorded.
- Given the chosen stack, when cross-built for `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, then both targets compile and the aarch64-linux binary runs the handshake on ARM hardware.
- Given the spike ends, when the result is written, then it lists chosen crates, direct-dependency count against NFR-05, and any changes proposed to PRD assumptions.

### A-2: Walking skeleton - stdio server with `fetch` schema (3 pts)
Maps to: FR-01, FR-02, FR-13, NFR-01.
As an MCP client developer, I want a registered `fetch` tool with a validated schema so that Claude Code can connect.
- Given the server starts, when a client sends `tools/list`, then exactly one tool `fetch` is returned with `url`, `max_length`, `start_index`, and `raw` in its JSON schema.
- Given `url` is missing, uses `file:` or `ftp:`, or numeric params are negative or non-integer, when `fetch` is called, then the call is rejected with a validation error naming the field.
- Given any request is logged, when the server runs, then all logs go to stderr and stdout carries only MCP protocol messages.
- Given an aarch64 host, when the server starts, then it reaches ready within 250 ms.
- Given Claude Code is configured with the binary path in a throwaway config on the author's machine (no registration in a permanent config before M3; no fetch-capable code exists yet in Sprint 1), when it lists tools, then `fetch` appears.

### A-3a: SSRF core - range table, resolver filter, fail-closed policy (5 pts)
Maps to: FR-06, NFR-04, Risk 2. Split from A-3 on 2026-09-19 (user decision). Architecture modules: `ssrf::ranges`, `ssrf::resolver`, `ssrf::check_url` (14.1).
As a home-lab operator, I want the address-blocking core to exist and be tested before any code can make a network request, so that no fetch-capable build ever lacks it.
- Given the `ssrf::ranges` table, when unit-tested table-driven, then it blocks the FULL set: IPv4 and IPv6 including IPv4-mapped/compatible, ULA, link-local, loopback, CGNAT, unspecified, and metadata addresses 169.254.169.254, fd00:ec2::254, 168.63.129.16; public addresses pass.
- Given a URL whose host is an IP literal, when `check_url` runs, then blocked literals are refused before any resolution or connection.
- Given a name, when the resolver filter runs (with an injectable test resolver), then it resolves once, refuses if any answer is blocked, and returns only the validated set for dialling (no second lookup).
- Given a redirect target, when the per-hop revalidation function runs, then it applies the same scheme, IP-literal and resolver checks to the new URL and refuses non-http(s) schemes.
- Given the default `Policy`, when constructed, then it is fail-closed (blocks everything non-public); the only way to permit loopback is the cfg/feature-gated test constructor (R12), asserted absent from release builds by a CI job (release build, `cargo tree -e features` and a symbol grep for `test-support`) that is a required check (job skeleton from D-7).
- Given the module, when reviewed, then it has no dependency on the HTTP client, so it is testable without network access.
- Given the module, when complete, then `cargo test`, clippy and fmt pass and unit tests cover each range and the mixed-answer case; deeper encoding, rebinding and coverage-gate work stays in B-1, B-2, B-5.

### A-3b: Streaming, size-bounded HTTP fetch client (5 pts)
Maps to: FR-07, FR-16, NFR-07, NFR-08, Risk 2. Split from A-3 on 2026-09-19. Architecture modules: `fetch` (client, redirect loop, body, deadline), `config`, flate2 gzip (ADR-001, 003, 004). Depends on A-3a; merge gate: no client code merges without A-3a's checks wired in. The gate is machine-checked: the client constructor takes a `Policy` value with no `Default` impl on the client path, and a required CI check runs the named test `a3b_merge_gate` (every dial goes through the validated-IP-set path; a build with any other dial route fails it) together with the four-refusal integration tests.
As a developer on ARM, I want the body read as a stream and capped so that memory cannot grow past the size limit, and I want every request to pass through the SSRF core.
- Given a URL returning a 10 MiB body with a `Content-Length` header and the default 5 MiB limit, when `fetch` is called, then it returns a "too large" error without reading the body.
- Given a URL returning a 10 MiB chunked body (no `Content-Length`) when `fetch` reads it in full, then reading stops at the limit and the call returns a "too large" error. (Window-based behaviour, a chunked body whose requested window completes under the limit succeeding, arrives with A-5 and is tested there.)
- Given a server that never finishes sending, when 15 s elapse, then the call returns a timeout error and the connection is closed.
- Given a response with a `Content-Length` above the limit, when `fetch` is called, then it aborts before reading the body.
- Given 10 concurrent calls to different URLs, when they complete, then each result matches its own URL with no cross-contamination.
- Given a request, when it is sent, then no cookies, credentials or auth headers are included (NFR-07).
- Given more calls than the concurrency default of 3 (compiled defaults until C-1 adds `FETCH_MAX_CONCURRENCY` parsing), when 10 calls are issued, then 3 run, 7 queue for at most the timeout default of 15 s (`FETCH_TIMEOUT_MS` parsing arrives in C-1) and all complete without errors; the fetch deadline starts at permit acquisition.
- Given the manual redirect loop, when a request and each redirect hop are made, then every URL passes A-3a `check_url`, the resolver filter and per-hop revalidation, and the client dials only the validated IP set (the redirect limit and its tests are B-3).
- Given an injectable resolver that returns a public answer on the first lookup and a private answer on a second lookup, when `fetch` runs, then the client dials only the validated IP set from the first lookup and never re-resolves (dial-once test).
- Given the parser differential risk (A-3a hand-rolls host parsing; the client re-parses with the `url` crate for Host header and SNI), when the merge gate runs, then a differential test compares `check_url` host, port and scheme classification against `url::Url::parse` over the encodings corpus (`url` as a dev-dependency only) with no disagreement that lets a blocked host through, and a test proves the dial route accepts only `Validated.addrs` (custom connector/resolver, no fallback to the client's own resolver). A-3b also deletes the Sprint 1 guard checks (HTTP client ban and tokio `net` ban) in `scripts/check-release-features.sh`.
- Given the integration tests through the real client, when run, then they prove `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are each refused; A-3b is not Done until they pass.
- Given a gzip response, when decoded with `flate2` (pinned when added), then only identity or a single gzip is accepted (checked from the header before decode), stacked, unknown, multi-member and trailing-garbage fixtures behave per ADR-004, and the decompressed-byte cap applies with output steps of at most 64 KiB (bomb fixture asserts it).
- Given explicit header size and count limits, when a header-bomb fixture is served, then the call fails cleanly.
- Given the DoD benchmark rule, when A-3b is closed, then a non-gating manual RSS smoke (one 5 MiB fetch on native aarch64, VmHWM read from `/proc/<pid>/status`, result noted in the PR) is recorded; the full memory gate is E-3/E-4 in Sprints 3-4 (harness lands in Sprint 3).
- Given a byte stream, when decoded, then the UTF-8 streaming decoder with replacement is used (non-UTF-8 charsets complete in A-8).

Note: B-1, B-2 and B-3 own test depth (encodings, mixed answers, rebinding simulation), the coverage gate and hardening for what A-3a and A-3b land. Split rationale and cut: see `.delivery/artifacts/05-plan/po/sprint-plan.md`.

### A-4: HTML to markdown conversion (5 pts)
Maps to: FR-03, NFR-02.
As an LLM agent, I want clean markdown so that I spend fewer tokens.
- Given an HTML page with headings, links, lists and code blocks, when `fetch` is called, then those elements are preserved in markdown.
- Given `<script>`, `<style>` and hidden navigation chrome, when converted, then their text is absent.
- Given the E-7 offline snapshot set (E-7 merged is a Sprint 4 entry condition) and the E-1 definitions of tokenizer, baseline and successful conversion, when converted, then at least 95% of the 50 pages convert successfully, median token reduction is at least 50% and no output contains `<script>` text (Goals 2 and 3; both checks are part of the Sprint 4 exit unless the E-7 fallback applies: if E-7 slipped to Sprint 5, this AC is not part of the Sprint 4 exit, A-4 is not Done until it is met in Sprint 5, and G4a is unaffected).
- Given a 1 MiB HTML page, when converted, then overhead is at most 500 ms p95 on aarch64.
- Given the converter, when used, then it sits behind a trait so it can be swapped.

### A-5: Pagination with `max_length` and `start_index` (3 pts)
Maps to: FR-02, FR-04.
As an LLM agent, I want to page through long content so that I never overflow my context.
- Given a 20,000-character page and `max_length=5000`, when four sequential calls use the returned `start_index`, then concatenated output equals the full text with no overlap or gap.
- Given `max_length` above the hard cap (default 100,000 characters), when `fetch` is called, then the value is clamped and the result states the clamp.
- Given content is truncated, when the result is returned, then it states the next `start_index`.
- Given `start_index` at or beyond the content length, when `fetch` is called, then the result is an empty-content message stating the total length, not a crash.
- Given multi-byte UTF-8 text, when a page boundary falls inside a character, then the split is on a character boundary.
- Given the window and early-stop path (`Window` sink) is in place, when G4b is prepared, then the harness scenarios that need it (window at start, window at end G1, chunked window inside the 5 MiB cap succeeds, window beyond the cap returns `too_large`) run on native aarch64 on the D-7 release profile (gnu and musl, median of 10 valid runs, peak on the bench build), each at or below 40 MiB and the 50 MiB chunked cases within 10% of the 5 MiB peak. A-5 owns these runs and keeps the harness scenarios for them working (in scope, no extra points).

### A-6: Content-type handling and `raw` mode (3 pts)
Maps to: FR-08, US-3.
As a developer, I want raw output and correct type handling so that JSON and text are usable and binaries are refused clearly.
- Given `raw: true` on an HTML page, when `fetch` is called, then the unconverted body is returned with the same truncation rules.
- Given a `text/*`, `application/json` or `application/xml` response, when `fetch` is called, then it is returned as text without HTML conversion.
- Given a PNG or PDF response, when `fetch` is called, then the result has `isError: true` and names the content type.
- Given a missing `Content-Type`, when the body begins with HTML markers, then it is treated as HTML; otherwise it is treated as text.
- Given G4b, when the end of Sprint 5 is reached, then A-6 owns the gate record: the `raw=true` scenario (G3) is run on the same terms as A-5's scenarios, idle is re-checked at 10 MiB on the shipped binary, and the combined G4b result (A-5 and A-6 scenarios) is written up. Pass: Sprint 6 starts. Fail: stop feature work before Sprint 6. G4b is the closing half of the memory gate and is not part of E-4's Done.

### A-7: Cause-specific structured errors (3 pts)
Maps to: FR-10, US-4.
As an LLM agent, I want distinct error messages so that I can choose the right recovery.
- Given HTTP 404, 500, DNS failure, timeout, blocked target, too-large response, or unsupported type, when `fetch` is called, then the result has `isError: true` and a message naming that cause.
- Given each cause, when tested, then a test asserts both the flag and the message text.
- Given an unexpected internal error, when it occurs, then the server returns a generic error result and keeps running without writing to stdout.

### A-8: Charset decoding and User-Agent (2 pts)
Maps to: FR-09.
As a developer, I want correct text decoding so that non-UTF-8 pages read properly.
- Given an ISO-8859-1 page declaring its charset in the header or a meta tag, when `fetch` is called, then output renders correctly.
- Given no charset, when `fetch` is called, then UTF-8 is assumed and invalid bytes are replaced, not fatal.
- Given a request, when sent, then it carries the configured descriptive `User-Agent`.

### A-9: Final URL and status header (1 pt)
Maps to: FR-14.
As an LLM agent, I want to know the final URL so that I can cite it.
- Given a redirect occurred, when `fetch` returns, then the text begins with the final URL and HTTP status.
- Given no redirect occurred, when `fetch` returns, then no header line is added.

---

# Epic B: Network safety

**Epic Goal:** The server cannot be steered into reaching loopback, private, link-local or metadata addresses, including by redirects or DNS tricks, and respects site crawl policy.

**Success Metric:** SSRF test suite 100% pass; >= 90% line coverage on SSRF, redirect and pagination logic; no connection attempted to a blocked address.

**Out of Scope:** Prompt-injection prevention (only labelling); WAF/bot-evasion; TLS pinning.

### Story Map

| # | Story | Value | Effort | Dependencies |
|---|---|---|---|---|
| B-1 | Block private, loopback, link-local on resolved IP | Critical | 5 | A-3a |
| B-2 | Encoded and IPv6 address forms | High | 3 | B-1 |
| B-3 | Redirect limit and per-hop revalidation | High | 3 | A-3b, B-1 |
| B-4 | robots.txt enforcement | Low | 3 | A-3b, OQ-3 |
| B-5 | SSRF suite and coverage gate | High | 3 | B-1, B-2, B-3 |
| B-6 | Untrusted-content labelling | Medium | 2 | A-4, OQ-5 |

**MVP Slice:** B-1, B-2, B-3, B-5. Rationale: an unguarded fetcher is not acceptable. B-4 and B-6 are policy items awaiting OQ-3 and OQ-5, so they follow.

### B-1: Block private, loopback and link-local on resolved IP (5 pts)
Maps to: FR-06, Risk 2.
As a home-lab operator, I want internal addresses refused so that a poisoned page cannot make the agent probe my LAN.
Note: the first implementation of the range table, resolver filter and per-hop revalidation lands in A-3a (wired into the client in A-3b); B-1 is test depth and hardening of that code (mixed answers, rebinding simulation, range-table completeness).
- Given a URL whose host resolves to loopback, RFC 1918, link-local (including 169.254.169.254) or IPv6 unique-local, when `fetch` is called, then it is refused with a "blocked" error before any connection is made.
- Given a DNS name that resolves to a private IP, when `fetch` is called, then it is refused.
- Given a name resolves to a public IP, when connecting, then the connection uses that validated IP so a second lookup cannot change it (rebinding defense).
- Given a name returns mixed public and private addresses, when `fetch` is called, then it is refused.

### B-2: Encoded and IPv6 address forms (3 pts)
Maps to: FR-06.
As a home-lab operator, I want obfuscated addresses caught so that encodings cannot bypass the block.
- Given hosts written as decimal (`2130706433`), hex (`0x7f000001`), octal, or short forms (`127.1`), when `fetch` is called, then they are refused.
- Given IPv6 loopback `::1`, IPv4-mapped `::ffff:127.0.0.1`, and unique-local `fc00::/7`, when `fetch` is called, then each is refused.
- Given the unspecified addresses `0.0.0.0` and `::`, when `fetch` is called, then they are refused.

### B-3: Redirect limit and per-hop revalidation (3 pts)
Maps to: FR-05, US-5.
As a home-lab operator, I want each redirect hop validated so that a public URL cannot bounce to an internal one.
- Given a public URL that redirects to `127.0.0.1`, when `fetch` is called, then the redirect is refused with a "blocked" error.
- Given a chain of 6 redirects, when `fetch` is called, then it fails with a "too many redirects" error; a chain of 5 succeeds.
- Given a redirect to a non-http(s) scheme, when `fetch` is called, then it is refused.

### B-4: robots.txt enforcement (3 pts)
Maps to: FR-11, OQ-3.
As a site-respecting developer, I want disallowed URLs refused so that the agent follows crawl rules.
- Given a robots.txt that disallows `/private`, when `fetch` targets `/private`, then it is refused with an explanatory error.
- Given `FETCH_IGNORE_ROBOTS=1`, when the same URL is fetched, then it is allowed.
- Given robots.txt is missing or returns 404, when `fetch` runs, then it proceeds.
- Given the robots.txt fetch itself, when made, then it passes the same SSRF checks and size cap (small, at most 512 KB).
- Blocked by OQ-3: default on or off is confirmed before this story starts.

### B-5: SSRF suite and coverage gate (3 pts)
Maps to: NFR-04, Goal 4.
As the project owner, I want an automated SSRF suite so that safety is proven and cannot regress.
- Given the test suite, when run, then it covers IPv4, IPv6, encoded forms, DNS-to-private, rebinding simulation and redirect cases and all pass.
- Given CI, when it runs, then line coverage of SSRF, redirect and pagination modules is at least 90% or the build fails.
- Given a new blocked-range case is added, when tests run, then it needs no harness change (table-driven cases).

### B-6: Untrusted-content labelling (2 pts)
**CLOSED as won't-do (2026-09-20): OQ-5 was decided "no label" by the owner. Fetched content is returned as-is, so the result envelope carries no notice. The 2 points leave the plan (recorded by the Product Owner at the next planning update; not re-baselined here).**
Maps to: Risk 5, OQ-5.
As a developer, I want fetched content marked as untrusted so that the model treats embedded instructions with suspicion.
- Given OQ-5 is decided as "label", when `fetch` returns content, then it is wrapped or prefixed with a fixed untrusted-content notice.
- Given the notice, when pagination is used, then it does not shift `start_index` offsets.
- Given OQ-5 is decided as "no label", when the story is reviewed, then it is closed as won't-do.
- Blocked by OQ-5 (now resolved: no label, so this story is closed as won't-do).

---

# Epic C: Configuration and policy

**Epic Goal:** Behavior limits and network policy are adjustable via environment variables with safe defaults and clear startup errors.

**Success Metric:** Every variable changes behavior in a test; invalid values fail startup with a clear message; defaults match the PRD.

**Out of Scope:** Config files, runtime reconfiguration, per-call overrides of security policy.

### Story Map

| # | Story | Value | Effort | Dependencies |
|---|---|---|---|---|
| C-1 | Environment variable parsing and validation | Medium | 3 | A-3b |
| C-2 | Private host allowlist | Medium | 2 | B-1, C-1, OQ-4 |
| C-3 | Configurable timeout and max size | Medium | 2 | C-1 |

**MVP Slice:** None required; defaults (15 s, 5 MiB, block all private) suffice for the first release. C-2 is promoted if OQ-4 says home-lab access is needed at v1.0.

### C-1: Environment variable parsing and validation (3 pts)
Maps to: FR-12.
As a developer, I want a single validated config so that misconfiguration is caught at startup.
- Given valid values for timeout, max size, user agent, allowed hosts and robots toggle, when the server starts, then it applies them.
- Given an invalid value (non-numeric timeout, negative size), when the server starts, then it exits non-zero with a message naming the variable and writing only to stderr.
- Given no variables, when the server starts, then it uses defaults: 15 s, 5 MiB, `max_length` cap 100,000, concurrency 3, block private, and the robots toggle parsed with a placeholder default only (C-1 does not decide OQ-3; the default is confirmed when OQ-3 is decided, due before Sprint 10 starts).
- Given a config error, when the server exits before the MCP handshake, then the stderr text names the variable (the client shows only a generic spawn failure, so this text is the diagnostic; documented in D-4).

### C-2: Private host allowlist (2 pts)
Maps to: FR-06, FR-12, US-6, OQ-4.
As a home-lab operator, I want to allow named internal hosts so that the agent can use my own services deliberately.
- Given `FETCH_ALLOW_PRIVATE_HOSTS=nas.local`, when `fetch` targets `nas.local`, then it is permitted.
- Given the same setting, when `fetch` targets another private host or a redirect leads to one, then it is still refused.
- Given the allowlist is unset, when any private host is targeted, then it is refused.
- Blocked by OQ-4.

### C-3: Configurable timeout and max size (2 pts)
Maps to: FR-07, FR-12, US-6.
As a developer, I want to tune limits so that I can trade completeness against memory.
- Given `FETCH_TIMEOUT_MS=5000`, when a server takes longer, then the call fails with a timeout error.
- Given `FETCH_MAX_BYTES=1048576`, when a larger body is served, then a `Content-Length` above 1 MiB or a chunked body whose requested window extends beyond 1 MiB gives a size error, and a chunked body whose window completes inside 1 MiB succeeds (same rule as FR-07).
- Given a configured max size, when peak RSS is measured, then it scales with the configured limit rather than the response size.
- Given `FETCH_MAX_LENGTH_CAP` is unset, when the server starts, then the `max_length` hard cap is 100,000 characters (was 200,000 in an earlier draft; lowered to fit the memory budget); a set value is validated and applied.

---

# Epic D: Packaging, ARM builds and docs

**Epic Goal:** A dependency-light, ARM-first release delivered as a tested container image on GHCR (ADR-007), with automated builds, tests on native arm64, and documentation to pull and register it.

**Success Metric:** A multi-arch (`linux/amd64` + `linux/arm64`) image published to GHCR only after the memory gates pass natively on BOTH platforms for that exact manifest and per-platform digests; CI green on hosted amd64 and arm64; a new user registers the server in Claude Code (`docker run -i --rm`) from the README in under 5 minutes.

**Out of Scope:** Windows, 32-bit ARM, standalone binary releases (gnu, musl, macOS; dropped by ADR-007), Windows containers, arm/v7, 386, riscv64, ppc64le, s390x (considered and DEFERRED, not rejected; a future platform that cannot be natively memory-verified may ship only with the label "memory targets not verified on this platform", never gated on QEMU), package-manager distribution (brew, apt), crates.io publishing, a self-hosted runner. Publishing the image to GHCR is IN scope of D-2 but is a distribution act gated by OQ-7.

### Story Map

| # | Story | Value | Effort | Dependencies |
|---|---|---|---|---|
| D-1 | Size- and memory-optimized release profile | High | 2 | A-3b |
| D-2 | Multi-arch image build and publish to GHCR (tested-digest gate, both platforms) | High | 8 | A-1, D-1, D-7, E-2, E-3, E-4, OQ-7 |
| D-3 | Test suite on aarch64 | High | 3 | D-2 |
| D-4 | README, install and migration guide (moves to Sprint 10 after the D-2 re-estimate) | High | 2 | D-2, OQ-7 |
| D-5 | Tool description within 150 words | Low | 1 | A-5 |
| D-6 | Licensing, dependency audit and release tag | Medium | 2 | D-3, OQ-7 |
| D-7 | Hosted PR CI baseline (x86_64) | High | 2 | None |

**MVP Slice:** D-7, D-1, D-2, D-4. Rationale: a release needs a buildable, gated ARM image and install steps; D-3 can be manual until then, D-5 and D-6 are v1.0 hygiene.

### D-1: Size- and memory-optimized release profile (2 pts)
Maps to: FR-15, NFR-13.
As a developer on ARM, I want a lean release build so that the binary and footprint stay small.
- Given `cargo build --release`, when it completes, then the profile uses LTO, `opt-level` chosen in A-1, `panic=abort` if compatible, and stripped symbols.
- Given the aarch64-linux release binary, when measured, then it is at most 10 MiB (provisional).
- Given the profile pinned in D-7 (Sprint 0) is finalised here, when D-1 closes, then idle RSS and 5 MiB peak RSS are re-measured on the shipped build (gnu and musl, hosted native runners for BOTH amd64 and arm64, measured on the process inside the image, median of 10 valid runs per platform, peak on the matching bench image; a miss on either platform blocks tagging, idle on the shipped binary) at the same 10 MiB and 40 MiB targets. This is a required precondition for MVP tagging: if either exceeds its target, no tag is created until it is fixed. Any change from the D-7 pin is recorded with the before and after figures. If either hosted runner is unavailable, D-1 does not close.
- Given the binary, when inspected with `ldd`, then it links no OpenSSL and no runtime beyond libc.

### D-2: Multi-arch image build and publish to GHCR with tested-digest gate (8 pts, re-estimated from 5) [REWRITTEN per ADR-007, user decision 2026-09-19]
Maps to: NFR-15, NFR-06, FR-15.
As a developer, I want CI to build, test and publish a container image so that I can run the server without compiling and the published artifact is exactly the tested one.
- Given a tag push, when the release workflow runs on `ubuntu-24.04-arm`, then per-platform jobs on `ubuntu-24.04` and `ubuntu-24.04-arm` each build the `linux/amd64` / `linux/arm64` image natively from a multi-stage Dockerfile (`--locked`, D-7 profile, pinned toolchain), with a minimal non-root base pinned by digest (architecture 9.3), and push it by digest, untagged, as a private candidate to GHCR; a merge job then creates the manifest list M from the per-platform digests (untagged, private).
- Given the candidate manifest digest M, when the per-platform test jobs run on their native runner, then each pulls `@sha256:M` (never a mutable tag), asserts the resolved platform digest equals the one built, and passes: MCP handshake in a clean container, `--version` (crate version, commit, libc, Cargo.lock hash), non-root user, FR-15 (no OpenSSL; static for musl or symbol floor check for gnu), the D-7 guard plus marker grep for `test-support` and `bench-loopback` on the binary extracted from that digest, and the platform's memory gates (idle on the shipped image, peak on that platform's never-pushed bench image built from the same commit, Cargo.lock hash and base digest).
- Given BOTH platform gates are green on the exact per-platform digests and M, when the publish job runs, then it adds the version tag (and `latest` only if OQ-7 says so) to that SAME manifest digest M without rebuilding, attaches provenance and SBOM attestations (recommended, not blocking for MVP), and writes M and both per-platform digests into the release notes, plus any deferred-platform statement. No tag or public visibility exists before both gates pass on those digests; the release is blocked if either platform gate fails or is skipped.
- Given OQ-7 is answered before Sprint 9 (still OPEN; not decided by this story), when the first image is to be published, then the channel, licence and package visibility follow that decision; until then only private candidates exist and the package is not made public.
- Given the workflow, when written, then it follows architecture 9.2: default `contents: read`, `packages: write`/`id-token`/`attestations` only in the publish job, SHA-pinned actions, no `pull_request_target`, fork PRs never push. Skipped or absent gate jobs block the release (required checks; `needs:`).
- Given `deny.toml` (committed in D-7), when the release pipeline runs, then it still has `advisories`, `bans` (deny `openssl`, `openssl-sys`, `native-tls`, `aws-lc-sys`, `aws-lc-rs`), `sources` (crates.io only) and `licenses` (permissive allow-list) sections.
- Given the base image digest or the toolchain changes, when the pin is bumped, then it is its own PR and re-runs the ARM gates.
- Given the libc choice is made by ADR-005 on data from the image, when D-2 lands, then exactly one libc flavour is published; the other candidate image is built for measurement only.
- Note (points): the original 5 pts covered gnu, macOS, checksums, codesign and a best-effort x86 build. Those are removed; image build, digest passing, the gate wiring and GHCR permissions are added. RE-ESTIMATED 5 to 8 pts (unvalidated, a planning judgment): the second platform, the manifest-list merge, per-platform digest assertions and two required gate jobs are real added work, only partly offset by removing macOS, checksums, codesign and the zig cross-build (both platforms build natively). Effect: Sprint 9 is D-2 alone at 8 pts; D-4 (2 pts) moves to Sprint 10 (9 pts with C-2, 7 without; over the 8-pt ceiling by 1 if OQ-4 is yes, PO/user to accept or defer C-3); MVP completes at the end of Sprint 10, not Sprint 9. Re-estimate again at Sprint 8 planning.
- Given the aarch64 test job (D-3) and memory-gate job (E-6) may not exist yet, when the MVP is released, then the release workflow's own in-workflow ARM gate run (above) is the evidence; the manual-bench-run fallback is removed (ADR-007). D-3 and E-6 add their PR-level jobs as required checks when they land. Tagged builds before M3 remain prohibited; D-6 is the v1.0 tagger.


### D-3: Test suite on aarch64 (3 pts)
Maps to: NFR-15, NFR-04.
As a developer, I want tests to run on real ARM so that ARM-specific issues are caught.
- Given a pull request, when CI runs, then unit and integration tests execute on the hosted native arm64 runner (`ubuntu-24.04-arm`) as well as x86_64 (`ubuntu-24.04`). Note: the D-7 developer work already adds an ARM CI job; at Sprint 11 planning, re-estimate D-3 downward for what already exists (not re-estimated here).
- Given the hosted arm64 runner is unavailable, when CI runs, then the job fails (no QEMU substitute for required checks); QEMU may be used only for optional functional smoke and states that RSS figures come only from native runs.
- Given a test failure on aarch64, when CI finishes, then the pipeline fails.

### D-4: README and install guide (2 pts)
Maps to: US-8, Goal 6.
As a developer, I want clear install steps so that I can register the server quickly.
- Given the README, when followed on a machine with Docker or Podman (Linux amd64 or arm64, macOS via Docker Desktop, Windows via WSL2), then the server is registered in Claude Code with `docker run -i --rm ghcr.io/<owner>/<repo>@sha256:...` (or version tag) and `fetch` works; the README states that a container runtime is required and that the image is a linux/amd64 + linux/arm64 manifest (other platforms not published). Publication-dependent wording follows OQ-7.
- Given the README, when read, then it lists environment variables (defaults only at Sprint 9; the env-var and config-error sections are re-checked when C-1 lands in Sprint 10), defaults, limitations (no JS, prompt-injection note), and a section "Registering in Claude Code" with the config snippet.
- Given the benchmark report exists, when linked, then the README states the measured idle and peak memory.
- Given the README, when read, then it documents: config errors exit before the handshake with the variable named on stderr; proxies are unsupported (fixed no-proxy); hosts behind a local NAT64 gateway should note that NAT64/6to4 addresses with public embedded IPv4 are allowed; and the musl static build may not resolve `.local` or split-DNS names that glibc resolves.

### D-5: Tool description within 150 words (1 pt)
Maps to: NFR-09.
As an LLM agent, I want a concise tool description so that I use pagination correctly.
- Given the tool description, when counted, then it is at most 150 words.
- Given the description, when read, then it names purpose, all parameters, and the `start_index` continuation pattern.

### D-6: Licensing, dependency audit and release tag (2 pts)
Maps to: NFR-05, OQ-7.
As the project owner, I want a clean v1.0 tag so that the release is auditable.
- Given the repo, when `cargo audit` and `cargo deny` run, then neither reports high or critical issues.
- Given a licence is selected per OQ-7, when the release is tagged, then LICENSE is present and direct dependencies are at most 15.
- Given all PRD Goals are met, when v1.0 is tagged, then the release notes link the benchmark report.

### D-7: Hosted PR CI baseline (2 pts) [added in plan revision 1]
Maps to: NFR-15, NFR-04 (release-build guard), Definition of Done.
As a solo developer, I want hosted PR checks from Sprint 0 so that "CI green" in the Definition of Done is real from the first story.
- Given a pull request, when hosted CI runs on x86_64, then `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked` run and are required status checks.
- Given the repo, when committed, then `rust-toolchain.toml` pins the exact channel, `Cargo.lock` is committed, all CI commands use `--locked`, and every GitHub Action is pinned by full commit SHA.
- Given `deny.toml` (sections `advisories`, `bans` denying `openssl`, `openssl-sys`, `native-tls`, `aws-lc-sys`, `aws-lc-rs`, `sources` crates.io only, `licenses` permissive allow-list), when CI runs, then `cargo deny` passes.
- Given the crate, when CI runs, then a job skeleton builds the release profile and asserts absence of the `test-support` and `bench-loopback` (E-8) features and of their marker strings in the binary (fails if present; becomes meaningful once A-3a adds `test-support`). The guard has a self-test: a deliberately built binary with `bench-loopback` (and one with `test-support`) must fail it, so it cannot pass vacuously. The release build uses `-p <crate>` so a workspace bench member cannot unify `bench-loopback` into it; E-8 unit tests run with `--features bench-loopback` in hosted CI.
- Given the release profile, when this story closes, then `Cargo.toml` fixes it (opt-level, lto, panic=abort where compatible, strip, codegen-units) with the values chosen from the A-1 spike, and G0, G4a, G4b and E-5 all measure it. D-1 (Sprint 8) finalises it. Also `publish = false` and `licenses.private.ignore` so `cargo deny` passes before OQ-7, and A-1 spike code passes clippy `-D warnings`.
- Given the repository settings, when this story closes, then branch protection on `main` requires the fmt, clippy, test, deny and release-guard checks (configured by the owner and recorded in the PR), so 'required check' is real from Sprint 0.
- Given fork pull requests, when CI runs, then jobs run with no secrets and a read-only token, no image is pushed, and there is no `pull_request_target` trigger. All runners are GitHub-hosted (ADR-007); there is no self-hosted runner in this project.
- Note: pushing the branch and opening the sprint PR is covered by the owner's sprint-start instruction; per-story benchmarks are manual on PRs until E-2 (Sprint 3) lands; from then the hosted arm64 workflow can run on same-repo PRs (ADR-007 removed the self-hosted restriction).

---

# Overall MVP Slice (cross-epic)

**Definition:** "Safe to run on my ARM machines, with the memory targets proven."

Included (73 points; was 72 before plan revision 2 added E-8 (1), 68 before plan revision 1 added D-7 (2) and E-7 (2), and 63 before the A-3 split): E-1, A-1, D-7, A-2, A-3a, A-3b, E-7, E-8, E-2, A-4, A-5, A-6, E-3, E-4, A-7, B-1, B-3, B-2, B-5, D-1, D-2, D-4, E-5.

Deferred to post-MVP (v1.0 completion, 24 points): A-8, A-9, C-1, C-2, C-3, B-4, B-6, D-3, D-5, D-6, E-6.

Rationale: the MVP contains every story needed to (a) prove the memory case, (b) deliver the core fetch behavior, (c) refuse internal addresses, and (d) install on ARM as a tested container image. Charset handling, config knobs, robots.txt and labelling do not affect the release decision. Trade-off: MVP ships with fixed defaults and UTF-8-only decoding; acceptable for personal use. Cost of adding A-8 early is low (2 pts) and it can be pulled forward if the test set shows encoding failures.

**Decision gates:** (1) End of Sprint 0 (G0): go/no-go on the absolute memory targets from E-1 and A-1. (2) End of Sprint 4 (G4a): idle RSS (shipped binary) and the peak/boundedness scenarios that need only A-3b and A-4 (full-consumption 5 MiB page, gzipped, late-landmark holdback-full, 50 MiB Content-Length, 50 MiB chunked read beyond the cap) at 10 MiB idle and 40 MiB peak; a pass means go on to Sprint 5, not that the memory gate is closed. (3) End of Sprint 5 (G4b, owned by A-5 and A-6): the scenarios that need the window or early stop (A-5) and `raw=true` (A-6) meet the same targets; the complete memory gate closes here. A fail at G4a stops before Sprint 5, a fail at G4b stops feature work before Sprint 6. Idle is gated on the shipped binary; peak on the `bench-loopback` build (E-8, confirmed). All gates measure the release profile pinned in D-7; D-1 (Sprint 8) re-measures the finalised profile and blocks MVP tagging on a miss.

# Suggested Sprint Order

Assumes 2-week sprints, 10 points capacity, at most 8 committed.

| Sprint | Goal | Stories | Points |
|---|---|---|---|
| 0 | De-risk and CI baseline: targets, crate stack, hosted CI; go/no-go | E-1, A-1, D-7 | 8 |
| 1 | Walking skeleton and SSRF core (no network code yet) | A-2, A-3a | 8 |
| 2 | Bounded streaming fetch, SSRF-guarded; 50-URL snapshots captured; bench loopback build | A-3b, E-7, E-8 | 8 |
| 3 | Benchmark harness and idle RSS | E-2, E-3, A-9 | 8 |
| 4 | Convert (95%/50% checks), and memory gate G4a | A-4, E-4 | 8 |
| 5 | Paginate and content types; memory gate G4b | A-5, A-6 | 6 |
| 6 | Clear errors and private-IP test depth | A-7, B-1 | 8 |
| 7 | Redirect limit, encoded forms, benchmark report | B-3, B-2, E-5 | 8 |
| 8 | SSRF suite, coverage gate, release profile | B-5, D-1, D-5 | 6 |
| 9 | ARM release pipeline and docs (MVP complete) | D-2, D-4 | 7 |
| 10 | Config and allowlist (C-2 if OQ-4 yes) | C-1, C-2, C-3 | 7 |
| 11 | robots.txt, charset, ARM tests | B-4, A-8, D-3 | 8 |
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 |

ADR-007 revision: total is now 100 points (D-2 5 to 8), MVP 76 points reached at the end of Sprint 10 (D-2 alone fills Sprint 9 at 8 pts; D-4 moves to Sprint 10, 9 pts with C-2, 7 without: 1 over the ceiling if OQ-4 is yes, PO/user decision), v1.0 still Sprint 12. The figures below are the pre-ADR-007 record.

Total 97 points over 13 sprints (0-12) (was 96; plan revision 2 added E-8, 1 pt; 92 before revision 1). Overall MVP (73 points) is reached at the end of Sprint 9 (D-2, D-4 land there, D-2 first; E-5 landed in Sprint 7), still Sprint 9. Non-MVP A-9 (Sprint 3) and D-5 (Sprint 8) fill slack; C-2 (Sprint 10) needs C-1. v1.0 moves from Sprint 11 to Sprint 12 because the re-estimate added 5 points. To reach MVP sooner, move D-1 into Sprint 7 and D-2 into Sprint 8, at the cost of delaying B-5.

**Split decision (2026-09-19, user):** A-3 is split into A-3a (SSRF core, Sprint 1, 5 pts) and A-3b (fetch client, Sprint 2, 5 pts). The original 5 pts under-estimated the absorbed scope; the two halves are re-estimated at 5 each (+5 total). Sprint 1 stays at the 8-point ceiling (A-2 + A-3a) and A-3b moves to Sprint 2, so no fetch-capable build exists before A-3a is in place, and A-3b has a merge gate requiring A-3a's checks. The following stories moved one to two sprints later as a result: A-4 (Sprint 2 to 4), A-5 (2 to 5), A-6 (3 to 5), A-7 (4 to 6); E-2, E-3 and E-4 keep Sprints 3, 3 and 4. The memory gate decision point stays at the end of Sprint 4 (E-3 in Sprint 3, E-4 in Sprint 4) for the scenarios available then (G4a, limited to scenarios that need only A-3b and A-4); the scenarios that need A-5 (window, early stop) and A-6 (`raw`) are gated at the end of Sprint 5 (G4b), so the complete gate lands one sprint after the previously told date (plan revision 2). MVP (Sprint 9) and v1.0 (Sprint 12) do not move.

**OQ-5 deadline:** OQ-5 (untrusted-content labelling) must be decided before Sprint 2 starts, because it can change the shape of the `fetch` result envelope that A-3b and A-4 produce. OQ-3, OQ-4 and OQ-7 stay open.

Trade-off: Sprint 10 is 7 points with C-2 and 5 if OQ-4 is "no" (C-2 dropped).

# Story-to-Requirement Traceability

| Requirement | Stories |
|---|---|
| FR-01 | A-1, A-2 |
| FR-02 | A-2, A-5 |
| FR-03 | A-4 |
| FR-04 | A-5 |
| FR-05 | B-3 |
| FR-06 | A-3a, B-1, B-2, C-2 |
| FR-07 | A-3b, C-3 |
| FR-08 | A-6 |
| FR-09 | A-8 |
| FR-10 | A-7 |
| FR-11 | B-4 |
| FR-12 | C-1, C-2, C-3 |
| FR-13 | A-2 |
| FR-14 | A-9 |
| FR-15 | A-1, D-1, D-2 |
| FR-16 | A-3b, E-4 |
| NFR-01 | A-2 |
| NFR-02 | A-4 |
| NFR-04 | A-3a, B-5, D-3 |
| NFR-05 | A-1, D-6 |
| NFR-06, NFR-15 | D-2, D-3, E-6 |
| NFR-07 | A-3b |
| NFR-08 | A-3b, E-4 |
| NFR-09 | D-5 |
| NFR-10 | E-1, E-3 |
| NFR-11, NFR-12 | E-1, E-4 |
| NFR-13 | D-1 |
| NFR-14 | E-2, E-6, E-8 |

# Open Items Affecting This Plan

| # | Item | Owner | Blocks |
|---|---|---|---|
| OQ-3 | robots.txt default. DUE before Sprint 10 starts (C-1 parses the toggle with a placeholder; the default is decided by OQ-3, still OPEN) | Michael | C-1 default (Sprint 10), B-4 (Sprint 11) |
| OQ-4 | Private-host allowlist needed. DUE before Sprint 10 | Michael | C-2 (Sprint 10) |
| OQ-5 | Untrusted-content labelling. **RESOLVED 2026-09-20: no label** (result envelope unchanged; B-6 won't-do) | Michael | None |
| OQ-7 | Distribution and licence. DUE before Sprint 9 starts, and in any case BEFORE the first image is published to GHCR (D-2 naming/publication/visibility, D-4 install guide, LICENSE). Still OPEN; not decided by ADR-007 | Michael | D-2, D-4, D-6 |
| E-8 | Bench-only loopback build. CONFIRMED by the user 2026-09-19 (does not decide OQ-4) | Michael | None (recorded; Sprint 2 E-8, E-2) |
| OQ-8 | Resolved 2026-09-19: not a replacement; no incumbent; schema is default design | Michael | None |
| OQ-9 | Resolved 2026-09-19; AMENDED 2026-09-19 by the user (ADR-007): native aarch64 measurement runs on GitHub-hosted arm64 runners (`ubuntu-24.04-arm`, cloud VM), no self-hosted runner; OS/RAM/page size recorded per run | Michael | None |
