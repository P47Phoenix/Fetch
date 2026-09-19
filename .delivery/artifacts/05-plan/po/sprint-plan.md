# Sprint Plan (Stage 5): Fetch MCP Server

Inputs: `docs/PRD.md` v0.4, `docs/EPICS.md` (34 stories after the A-3 split and plan revisions 1 and 2), architecture sections 11, 14.1, 15 and ADR-001..006. Roles: Product Owner + Scrum Bag. Date: 2026-09-19.

## Top of plan: flags

1. **Open questions and due sprints (all still OPEN, undecided).** OQ-5 due before Sprint 2 starts (envelope of A-3b/A-4; B-6 is Sprint 12; any default assumption is not a decision). OQ-4 before Sprint 10 (C-2). OQ-3 **now due before Sprint 10 starts** (was Sprint 11): C-1 (Sprint 10) parses the robots toggle and needs the default; C-1 uses a placeholder only and does not decide it; B-4 (Sprint 11) still uses it. OQ-3 stays OPEN. **OQ-7 now due before Sprint 9 starts** (was Sprint 12): D-2 artifact naming and publication, D-4 install guide and the LICENSE need it; D-6 (Sprint 12) still uses it.
2. **Scope and dates (after revision 2).** Total scope is 97 points (was 96 in revision 1; 92 before; 87 before the A-3 split), 34 stories (was 33). MVP is 73 points (was 72), still reached at the end of Sprint 9. v1.0 is still Sprint 12. Revision 2 added E-8 (1 pt, Sprint 2), now CONFIRMED by the user. **Round 3 (user decision, 2026-09-19):** the memory gate is split with corrected scenario lists. G4a (end of Sprint 4) is idle plus only full-consumption scenarios that need A-3b and A-4 (nothing that needs the A-5 window or early stop, or A-6 raw). G4b (end of Sprint 5, owned by A-5 and A-6) gates the window, early-stop and `raw=true` scenarios at the same targets. **Plainly: the complete memory gate is at the end of Sprint 5, one sprint later than originally told;** a G4a pass means 'go on to Sprint 5', not 'gate closed'. MVP (Sprint 9), v1.0 (Sprint 12) and the 8-pt sprint ceiling do not move. The 10 MB idle and 40 MB peak targets are not lowered. Counts are unchanged in round 3: 97 pts, 34 stories, MVP 73 pts in Sprint 9.
3. **ARM runner access** is a hard dependency for the Sprint 0 handshake check, the Sprint 1 readiness check (250 ms), the Sprint 2 RSS smoke, the Sprint 3 and Sprint 4 measurements, the Sprint 4 gate G4a and the Sprint 5 gate G4b. Runner availability is checked at the start of Sprints 3, 4, 5, 7 (E-5 report), 9, 10 (C-3), 11 (D-3, A-8) and 12. Sprints 6 and 8 need no runner.
4. **CONFIRMED by the user (2026-09-19): bench loopback path (E-8).** The shipped binary's fail-closed policy blocks the loopback fixture server. Recommended: a compile-time Cargo feature `bench-loopback` (off by default, permits only 127.0.0.0/8 and ::1, every other blocked range still refuses), built as a second binary from the same commit by the same pinned pipeline; D-7 asserts it and `test-support` are absent from release builds. Idle RSS is gated on the shipped binary; peak RSS on the bench build. Details and the fidelity trade-off are in E-8 (EPICS). OQ-4 is not decided by this.
5. **Release profile pinned early (user decision, 2026-09-19).** D-7 (Sprint 0) fixes the release profile (opt-level, lto, panic=abort, strip, codegen-units) so G0, G4a, G4b and E-5 measure what ships. D-1 (Sprint 8) finalises it and must re-measure idle and peak on the shipped build, including the aarch64 runner (gnu and musl); a miss blocks MVP tagging. No points change.

## The A-3 split and why this cut

| Story | Scope | Pts |
|---|---|---|
| A-3a SSRF core | `ssrf::ranges` full table, IP-literal check, resolver filter (resolve once, refuse on any blocked answer, return validated set), per-hop revalidation function, fail-closed default `Policy`, test-only constructor, table-driven unit tests with an injectable resolver | 5 |
| A-3b fetch client | reqwest client, manual redirect loop wired to A-3a, deadline, flate2 gzip, header limits, byte caps, semaphore, UTF-8 decoder, integration tests (four refusals) | 5 |

Justification for the cut: it follows the module seam in architecture 15 (`ssrf::*` versus `fetch::*`). A-3a has no dependency on the HTTP client and needs no network, so it can be finished and reviewed on its own. It is first, and Sprint 1 (A-2 + A-3a = 8 pts) stays at the ceiling. A-3b is Sprint 2, so no fetch-capable build exists in Sprint 1. Alternatives rejected: (a) SSRF second: creates a window with an unguarded client. (b) One 10-pt story: exceeds the 8-pt story maximum. (c) Three-way split with semaphore/decoder separate: adds overhead for two small items.

**Merge gate (A-3b), machine-checked:** the client constructor takes a `Policy` with no default on the client path, and a required CI check runs the named test `a3b_merge_gate` plus the four-refusal integration tests; the A-3b PR is not merged unless (1) A-3a is already on the sprint branch, (2) every request and redirect hop goes through `check_url` + resolver filter, (3) the integration tests prove `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused, and (4) the default `Policy` is fail-closed and the test constructor is absent from release builds. The four refusal AC moved from A-3 to A-3b because they need the real client; A-3a covers the same cases at unit level.

## Capacity and ceiling

Solo part-time, ~10 pts per 2-week sprint, commitment at most 8 (80%). Planned commitments: 8, 8, 8, 8, 8, 6, 8, 8, 6, 7, 7, 8, 7 = 97 (13 sprints; sprint 5 is a thin 6-pt block with 2 pts of slack, which G4b evaluation and any G4a remediation can use). No sprint exceeds 8. Stories moved by the split: A-4 (Sprint 2 to 4), A-5 (2 to 5), A-6 (3 to 5), A-7 (4 to 6), and downstream B/D/C/A-8 stories shifted; E-2, E-3 (Sprint 3) and E-4 (Sprint 4) keep exactly Sprints 3, 3 and 4; G4a stays at the end of Sprint 4 and G4b is at the end of Sprint 5 (revision 2). Revision 1: D-7 added to Sprint 0, E-7 to Sprint 2, A-9 moved from Sprint 2 to Sprint 3. Revision 2: E-8 (1 pt) added to Sprint 2, now at the 8-pt ceiling with no in-plan slack. Sprint 0 has only 5 pts of remaining work because A-1 is already delivered.

## Sprints 0-4 (detailed)

### Sprint 0: De-risk and CI baseline (8 pts)
**Goal:** Fix the memory targets and measurement method, and confirm the crate stack builds and runs on ARM, so a go/no-go can be made.
Stories: E-1 (3, spike 2 days), A-1 (3, spike 3 days; already delivered on this branch, close out formally), D-7 (2, hosted PR CI: fmt, clippy, test, deny, SHA-pinned actions, toolchain pin, release-feature-guard skeleton with self-test, release profile pinned in `Cargo.toml`, `publish = false`; branch protection requires these checks).
Dependencies: none. ARM runner for A-1 handshake and E-1 host record. Owner's sprint-start instruction covers pushing the branch and opening the PR (needed for CI). E-1 also records the confirmed loopback path (E-8).
Entry: PRD v0.4, epics accepted, benchmark host identified.
Exit: release profile pinned (D-7); native aarch64 host access confirmed and OS/RAM recorded; hosted CI green on the Sprint 0 PR (D-7); E-1 one-page result committed (targets, MB definition, fixtures, TLS approach, host OS/RAM; the 50-URL list is DEFERRED to the project owner by user decision in Sprint 0 revision 4 and is a prerequisite for E-7 in Sprint 2); A-1 result lists chosen crates and dependency count vs NFR-05 (<= 15); PRD assumption changes recorded.
**Gate G0 (go/no-go):** measurable rule: A-1 measured idle RSS <= 10 MB and 5 MB-fetch peak <= 40 MB (MB defined once in E-1 and used for every target, fixture and cap, each the median of 10 valid runs under the E-1 protocol on the D-7 profile; A-1 predates the protocol, so its binary is re-run under it or the deviation is recorded) on the native aarch64 host (glibc), or a written gap analysis with a credible path; ARM host recorded. No-go: re-scope PRD Goal 1 before any feature work.

### Sprint 1: Skeleton and SSRF core (8 pts)
**Goal:** A registered `fetch` tool that validates input, and a tested address-blocking core, with no network-capable code yet.
Stories: A-2 (3), A-3a (5).
Dependencies: A-1, D-7 (CI). ARM runner for the 250 ms readiness check. A-3a depends on A-2 (`config`, `error`, `Policy` skeleton).
Entry: G0 = go; branch `sprint-1/skeleton-ssrf` and draft PR opened.
Exit: `tools/list` shows exactly `fetch`; stdout-purity test passes; ready within 250 ms on ARM; A-3a table-driven tests pass for every range; default policy fail-closed; the CI release-build check (no `test-support`, via `cargo tree -e features` and symbol grep) is a required check and passes; hosted CI green. Claude Code check for A-2 uses a throwaway config only.
Gate: no HTTP client dependency is present in the crate at end of Sprint 1 (or it is unreachable from `fetch`).

### Sprint 2: Guarded streaming fetch, snapshots and bench build (8 pts)
**Goal:** Claude Code can fetch a public page through a size-bounded stream that refuses internal addresses, and a bench-only build can reach the loopback fixture server.
Stories: A-3b (5), E-7 (2: capture the 50-URL offline snapshots with committed sha256 manifest), E-8 (1, CONFIRMED: `bench-loopback` feature).
Dependencies: A-3a (merge gate above), A-2, D-7, E-1 (URL list; loopback path confirmed by the owner 2026-09-19). ARM runner for the RSS smoke. **OQ-5 decided before start.**
Entry: A-3a merged; OQ-5 decision recorded; flate2 version chosen for pinning; E-8 loopback path confirmed (done).
Exit: all A-3b AC pass, including the four-refusal integration test (run on the shipped-profile build), the dial-once test, gzip/bomb/header-bomb fixtures, semaphore test; flate2 pinned exact; verify hyper buffer defaults (architecture row a); UTF-8 decoder in place; non-gating manual RSS smoke (one 5 MB fetch, VmHWM) on native aarch64 recorded; E-7 snapshot set and manifest committed; E-8 unit tests and the D-7 guard prove the feature is absent from release builds.
DoD scoping: the per-story native-aarch64 benchmark rule does not apply to A-3b beyond this smoke, because the harness lands in Sprint 3; the full memory gate is E-3/E-4 (Sprints 3-4). E-1, A-1, E-7 and E-8 are exempt.
Gate: A-3b merge gate. Sprint is exactly 8 pts (ceiling). Overflow rule: A-3b and E-8 are protected (E-2 needs E-8). If either overruns, E-7 slips to Sprint 5 (6 + 2 = 8 pts, no other sprint changes) and A-4's 95%/50% AC waits for it (A-4 not Done until then; the Sprint 4 exit then excludes that check; Goals 2 and 3 evidence one sprint later; G4a unaffected). This is flagged to the owner at the time; nothing moves into Sprint 3, which stays at 8.
Note: pre-M3 builds are not registered in a real MCP client (release rule); Claude Code check uses a throwaway config on the author's machine only against public URLs.

### Sprint 3: Harness and idle RSS (8 pts)
**Goal:** A one-command harness on native ARM that reports idle RSS against the 10 MB target.
Stories: E-2 (5), E-3 (2), A-9 (1, moved from Sprint 2; first to drop if E-2 overruns).
Dependencies: E-1, A-3b, E-8, A-2. ARM runner required (native preflight refuses QEMU); runner provisioned and isolated per architecture 9.2 as part of E-2; interim gnu+musl build script with pinned `cargo-zigbuild`/`ziglang` versions (D-2 adopts the same pins).
Exit: fixtures with committed sha256 manifest; G1-G7 scenarios defined; harness targets the bench build for peak and the shipped binary for idle; minimal self-hosted nightly/dispatch workflow (or documented manual runs until D-2); idle RSS median of 10 recorded; harness exits non-zero when over target.
Gate: idle RSS <= 10 MB (strict) on the shipped release binary (also recorded on the bench build), native aarch64 only; QEMU or non-native figures never count. Miss triggers an investigation item before Sprint 4 starts.

### Sprint 4: Convert and memory gate (8 pts)
**Goal:** HTML converts to markdown within budget and peak memory is proven bounded.
Stories: A-4 (5), E-4 (3).
Dependencies: A-3b, E-2, E-8, E-7. Entry: E-7 merged (else the Sprint 2 fallback applies: the 95%/50% check is removed from the Sprint 4 exit, done in Sprint 5, and this is flagged); ARM runner checked. A-4 lands first in the sprint (E-4 is first priority if A-4 slips); E-4 needs it (`lol_html` limits, architecture rows f/g).
Exit: A-4 AC (on the E-7 snapshot set, unless the fallback applies: >= 95% conversion success and median token reduction >= 50%, no `<script>` text; 1 MB in <= 500 ms p95 on aarch64, converter behind a trait); E-4 fixtures: 5 MB peak <= 40 MB; 50 MB with Content-Length -> `too_large` within 10% of 5 MB peak; 50 MB chunked, full read without a window, beyond the cap -> `too_large` within 10% of 5 MB peak; 10 concurrent recorded; allocator recorded. Windowed variants (chunked in-cap window, window beyond cap) need A-5 and are G4b, not Sprint 4.
**Gate G4a (memory gate part 1, end of Sprint 4):** idle RSS <= 10 MB (shipped binary) and 5 MB peak (VmHWM, max of per-scenario medians) <= 40 MB (bench-loopback build, confirmed) on native aarch64, each the median of 10 valid runs (harness rejects fewer), for both gnu and musl, on the release profile pinned in D-7 (MB as defined once in E-1). Scenarios are only those that need A-3b and A-4 and no window or early stop: (1) the 5 MB HTML page fully read and converted, (2) the same page gzipped, (3) late-landmark holdback-full HTML, (4) 50 MB with Content-Length -> `too_large`, (5) 50 MB chunked read beyond the cap without a window -> `too_large`; (4) and (5) within 10% of the 5 MB peak. The 10-concurrent scenario is recorded, not gating. The shipped-vs-bench record is part of the pass, with numeric bounds (E-8): idle delta <= 0.5 MB, binary size delta recorded and explained, one public-host 5 MB fetch on the shipped binary within 10% of the bench peak and <= 40 MB; both binaries report the same commit and Cargo.lock hash. QEMU or non-native figures never count. Pass: continue to Sprint 5. Fail: stop feature work and run a memory-reduction sprint (allocator, buffer sizes, converter swap via trait) before Sprint 5. **A G4a pass does not close the memory gate.**
**Gate G4b (memory gate part 2, end of Sprint 5; owning stories A-5 and A-6, A-6 records the combined result; not part of E-4's Done):** the scenarios that need A-5 (window at start with early stop, window at end, chunked window inside the cap succeeding, window beyond the cap -> `too_large`) and A-6 (`raw=true`) meet the same 40 MB peak (idle re-checked at 10 MB on the shipped binary) under the same rules, and the two 50 MB chunked window cases stay within 10% of the 5 MB peak. Fail: stop before Sprint 6. The 10 MB idle and 40 MB peak targets are not lowered. **The complete memory gate is therefore closed at the end of Sprint 5.** Requires ARM runner access; without it G4a cannot be evaluated and Sprint 5 does not start, and G4b likewise blocks Sprint 6.

## Sprints 5-12 summary (12 for v1.0)

### Sprint 5: Paginate, content types and memory gate G4b (6 pts, thin block)
**Goal:** Pagination and content types work and the memory gate closes. Stories: A-5 (3), A-6 (3). Dependencies: A-4, E-2, E-4, E-8; ARM runner. Entry: G4a passed (or its remediation sprint done); runner checked. Exit: A-5 and A-6 AC pass; G4b run and recorded (A-5 window scenarios, A-6 raw and combined write-up); idle re-checked. The block is 6 of 8 pts on purpose: 2 pts of slack absorb G4b work or the E-7 overflow (Sprint 2 fallback: 6 + 2 = 8).

### Sprints 6-12

| Sprint | Goal | Stories | Pts | Entry | Exit |
|---|---|---|---|---|---|
| 6 | Clear errors, private-IP test depth | A-7, B-1 | 8 | G4b passed | Every error cause has a flag+message test; B-1 encodings and mixed-answer cases pass on A-3a code |
| 7 | Redirect limit, encoded forms, report | B-3, B-2, E-5 | 8 | E-3, E-4, A-7 done; runner checked | Redirect limit and rebinding tests pass; E-5 report published on the D-7-pinned profile with figures labelled by binary, and states the D-1 re-measure is still required |
| 8 | SSRF suite, coverage gate, release profile | B-5, D-1, D-5 | 6 | B-1, B-2, B-3 done; runner available (D-1 re-measure) | B-5 SSRF suite 100% and 90% coverage gate (M3 reached); D-1 profile finalised and idle and peak **re-measured on the shipped build on aarch64 (gnu and musl) within 10 MB and 40 MB, or MVP tagging is blocked** |
| 9 | ARM release pipeline and docs | D-2, D-4 | 7 | M3 reached; OQ-7 decided; D-1 re-measure passed; runner checked | D-2 artifacts built, D-7 guard run on each artifact (cargo tree and marker grep for `test-support` and `bench-loopback`); D-4 install guide; MVP complete (73 pts); manual bench run on the D-2 artifact plus E-5 stand in for the CI gate |
| 10 | Config and allowlist | C-1, C-2, C-3 | 7 | OQ-3 default decided (C-1 needs it), OQ-4 decided for C-2 (drop = 5 pts) | Env parsing and validation, allowlist (if OQ-4 yes), timeout and size config; D-4 config text re-checked |
| 11 | robots, charset, ARM tests | B-4, A-8, D-3 | 8 | OQ-3 decided; runner checked | robots enforced per OQ-3; charset decoding; aarch64 test job required check |
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 | OQ-5 decided; ARM CI runner | Labelling per OQ-5; memory-gate CI required; v1.0 tagged by D-6 |

MVP = 73 pts, end of Sprint 9 (sprints 0-9 sum to 75, less non-MVP A-9 and D-5). Tagging/distribution only after M3 (Sprint 8), the E-5 release decision and the D-1 shipped-build re-measure (Sprint 8) passing; the MVP release relies on the documented manual bench run until E-6 (Sprint 12), and D-6 is the v1.0 tagger.

Milestones M0-M5 map to sprints in PRD section 9: M0 S0, M1 S1-2, M2 S3-6 (G4a at end of S4; memory gate complete at end of S5 with G4b), M3 S6-8 (reached end of S8), M4 S9-11, M5 S11-12.

## Definition of Done (per story)

- All acceptance criteria have a passing test that asserts them.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` pass on the hosted CI from D-7 (required checks from Sprint 0).
- No new direct dependency without recording it against the NFR-05 count; high-churn deps use exact `=` pins (architecture 8).
- stdout carries only MCP messages; logs to stderr.
- Stories touching fetch, decode, convert or concurrency (A-4, A-5, A-6, C-3, E-2 to E-6): benchmark run on native aarch64 with results recorded (manual on PRs); no regression against the absolute targets. A-3b: non-gating manual RSS smoke only (harness arrives in Sprint 3). Spikes E-1 and A-1, E-7 and E-8 are exempt. All benchmark figures use the release profile pinned in D-7, MB as defined once in E-1, and the median of 10 valid runs.
- SSRF-touching stories (A-3a, A-3b, E-8, B-*, C-2): table-driven cases; release build asserted free of `test-support` and `bench-loopback` (cargo tree plus marker grep on the actual artifact, guard self-tested).
- Docs touched where behaviour is user-visible; PR reviewed (self-review checklist for a solo project) and CI green.

## Branch and PR convention

One branch per sprint, `sprint-N/<slug>` (Sprint 0: `sprint-0/spikes`), created from main after the previous sprint PR is merged (Sprint 0's branch already carries A-1). If a gate (G0, G4a, G4b) blocks the merge, the next sprint does not start; a memory-reduction sprint branches from the unmerged tip only with the owner's instruction. One draft PR per sprint targeting `main`, opened at sprint start, titled `Sprint N: <goal>`, body listing stories and gates. Commits reference the story ID (`A-3a: ...`). PR marked ready and merged only when the sprint exit criteria and gates hold. No pushes without the owner's instruction. Tagged builds are prohibited before M3.

## Traceability (Sprints 0-4 stories)

| Story | FR / NFR | Architecture module / ADR |
|---|---|---|
| D-7 | NFR-15, DoD | hosted CI, sec 9 |
| E-1 | NFR-10, 11, 14 | bench design, sec 11 |
| A-1 | FR-01, FR-15, NFR-05, 15 | spike; ADR-001, 002, 005 |
| A-2 | FR-01, FR-02, FR-13, NFR-01 | `main`, `server`, `config`, `error`, `obs`; ADR-006 |
| A-3a | FR-06, NFR-04, Risk 2 | `ssrf::ranges`, `ssrf::resolver`, `ssrf::check_url`; ADR-003, sec 14.1 |
| A-3b | FR-07, FR-16, NFR-07, NFR-08 | `fetch` (client, redirect loop, body, deadline), `config`, flate2; ADR-001, 003, 004 |
| A-9 | FR-14 | `server::render` |
| E-7 | Goals 2, 3 | test fixtures |
| E-8 (confirmed) | NFR-14, NFR-11, Risk 2 | `ssrf::policy` (cfg feature), sec 11 item 7, sec 9 R12; ADR-003 |
| E-2 | NFR-14 | `bench/`, sec 11 |
| E-3 | NFR-10 | `bench/` |
| A-4 | FR-03, NFR-02 | `convert::html`, `boilerplate`; ADR-002 |
| E-4 | NFR-11, 12, FR-16 | `bench/`, ADR-004 |

**Required architecture change (not made here; architecture and ADR files are not edited in Stage 5):** section 11 item 7 and section 11.2 must add the compile-time `bench-loopback` policy route and state which binary each gate measures (idle: shipped; peak: bench build), and that G4 is split into G4a (end of Sprint 4, idle plus scenarios needing only A-3b and A-4) and G4b (end of Sprint 5, window, early-stop and `raw=true` scenarios); section 11 must also state that the release profile (opt-level, lto, panic=abort, strip, codegen-units) is pinned in Sprint 0 (D-7), that all gates measure it, and that D-1 (Sprint 8) re-measures the finalised profile and blocks MVP tagging on a miss; section 9 R12 and ADR-003 must name `bench-loopback` beside `test-support` and require the D-7 guard to assert both absent. E-8 is confirmed by the user (2026-09-19).

Later stories follow `docs/EPICS.md` (traceability table) and architecture section 15 unchanged; FR-06 maps to A-3a, B-1, B-2, C-2; FR-07 to A-3b, C-3.

## Risks and open dependencies

| Item | Impact | Mitigation / owner |
|---|---|---|
| ARM runner access (author's aarch64 cluster) | Blocks G0 handshake, Sprint 1 readiness, Sprint 2 smoke, Sprint 3-5 measurements, G4a/G4b gates, E-5, C-3, A-8, E-6, D-3; a runner outage in Sprint 12 leaves no slack | Confirm access and record OS/RAM before Sprint 0 exit; no native runner means G4a/G4b not evaluated and no release; QEMU never used for RSS (Michael) |
| OQ-5 (labelling) due before Sprint 2 | Result envelope rework in A-3b/A-4 | Decide before Sprint 2 (Michael); a "label, prefix not shifting `start_index`" working assumption is NOT a decision |
| OQ-3, OQ-4, OQ-7 open | C-1 default, B-4, C-2, D-2/D-4/D-6 blocked at their sprints | Decide OQ-3 before Sprint 10 (C-1), OQ-4 before Sprint 10, OQ-7 before Sprint 9 |
| A-3b estimate (5 pts, high scope) | Sprint 2 overrun | Sprint 2 is committed at 8 (ceiling); if A-3b or E-8 overruns, E-7 slips to Sprint 5 and A-4's 95%/50% check waits for it (A-9 is in Sprint 3 and unaffected) |
| Bench build diverges from shipped (E-8 confirmed) | Peak RSS not measured on the shipped binary; report could mislead | Numeric bounds (idle delta <= 0.5 MB, public-host peak within 10% of bench and <= 40 MB), same commit and Cargo.lock hash, marker checks; report labels figures by binary; OQ-4 untouched |
| Release profile changes late (D-1, Sprint 8) | Idle or peak could exceed targets on the shipped build after all gates passed | Profile pinned in D-7 so gates measure it; D-1 requires a re-measure on aarch64 that blocks MVP tagging; no slack in S8 is needed (6 pts), but a failure re-plans S9 |
| G4b after G4a | Full gate closes one sprint later than first told (end of Sprint 5) | Stated in top flags, PRD M2 and EPICS; G4a lists only scenarios needing A-3b and A-4; G4b is owned by A-5/A-6; fail stops Sprint 6 |
| Interim unsafe window | LAN probing by pre-M3 builds | A-3a first, merge gate, no tags or client registration before M3 |
| Memory gate miss at G4a or G4b | Re-plan | Memory-reduction sprint inserted, later sprints shift |
| Pinned dependency drift (flate2 etc.) | Benchmark validity | Pin on add; bump PR re-runs benchmark |

## Revision log (round 1)

| Finding | Resolution |
|---|---|
| DevOps B1: no CI before Sprint 9 | New story D-7 (2 pts, Sprint 0): hosted PR checks (fmt, clippy, test, deny, SHA-pinned actions, toolchain pin, release-feature guard skeleton). D-2 keeps only the release pipeline. |
| DevOps B2: D-2 (S9) needs E-6/D-3 (S11-12) | Not moved: the required-check AC leaves D-2 and lands in D-3 and E-6, which add themselves as required checks. MVP release uses the documented manual bench run (E-6's own accepted equivalent) plus E-5; D-6 is the v1.0 tagger. D-2 no longer claims gates that do not exist. |
| DevOps B3: OQ-7 due S12 but D-2/D-4 in S9 need it | OQ-7 due before Sprint 9 (was 12); still OPEN and undecided. D-2 and D-4 list OQ-7 as a dependency. EPICS and PRD OQ tables updated. |
| DevOps N1, N2 | Runner provisioning and runner checks at Sprints 3, 4, 9, 12 added; interim pinned zigbuild script in E-2 (Sprint 3). N3 clarified in Sprint 0 and D-7. N4 noted in risks. |
| QA B1: no sprint captures 50-URL snapshots or runs 95%/50% checks | New story E-7 (2 pts, Sprint 2) captures snapshots with sha256 manifest; A-4 AC and Sprint 4 exit run the 95% success and 50% median reduction checks. |
| QA B2: A-3b benchmark DoD impossible in S2 | DoD scoped: A-3b needs only a non-gating manual RSS smoke; full gate at E-3/E-4 (S3-4). Spikes and E-7 exempt. |
| QA NB: G0, G4 text, G6 gating, ARM in S1/S2, S4 slack, test-support check | G0 made measurable; G4 states MiB definition, gnu and musl, QEMU never counts, boundedness in pass condition; G6 reported not gating (EPICS E-2 edited); ARM added to S0/S1/S2 dependencies; test-support absence is a required CI check (A-3a AC, D-7); E-4 is first priority in S4. |
| PO B1: PRD s9 milestones and s10 OQ due stale | M0-M5 re-mapped to sprints (M0 S0, M1 S1-2, M2 S3-6, M3 S6-8, M4 S9-11, M5 S11-12); OQ-5 before S2, OQ-4 before S10, OQ-3 before S11, OQ-7 before S9; PRD decision rule now cites G0/G4. |
| PO B2: D-2 required checks created later | As DevOps B2. |
| PO NB | D-4 scoped to defaults with a re-check in C-1 (S10); Goal 2 has an AC in A-4 (and E-7); C-3 wording aligned to the size rule; A-2 throwaway-config note; OQ-5 default marked not a decision; line about E-2/E-3/E-4 sprints made exact. |
| SM NB | D-2 CI job clarification as above; ARM gaps in S1/S2 fixed; memory re-run at S5 end added; per-sprint entry/exit for Sprints 5-12 remains to refine before Sprint 5. |

Told numbers changed: total 92 to 96 points; MVP 68 to 72 points. Unchanged: MVP in Sprint 9, v1.0 in Sprint 12, memory gate at end of Sprint 4. OQ-3, OQ-4, OQ-5 and OQ-7 remain open; only OQ-7's due sprint moved (12 to 9).

## Revision log (round 2)

| Finding | Resolution |
|---|---|
| QA B1: gating binary cannot reach loopback fixture; D-7 forbids `test-support` | New story E-8 (1 pt, Sprint 2, PROPOSED, needs user confirmation): compile-time `bench-loopback` feature (loopback ranges only, off by default, second binary from the same commit and pinned pipeline). Runtime switch rejected (ships a bypass, overlaps OQ-4); in-process, netns or non-loopback fixture rejected as primary (RSS not of the server process, or private addresses still blocked). D-7 guard extended to `bench-loopback` and marker string; E-1 records and E-2/E-4 use it; idle gated on the shipped binary, peak on the bench build, delta record and a shipped-binary public-host fetch. "Required architecture change" note added; architecture and ADR files not edited. OQ-4 undecided. |
| PO B1: G4 at S4 needs A-5/A-6 (S5) | Not moved (S3 and S4 are full). G4a (end S4) covers scenarios needing only A-3b/A-4; G4b (end S5) gates window-at-end and `raw=true` at the same targets. Complete gate lands one sprint later than told; flagged in top flags; PRD M2, decision rule and EPICS decision gates updated. Targets unchanged. |
| PO B2, QA NB1, SM 1 (stale Sprint 2 risk) | Risk row rewritten: Sprint 2 is 8 pts; overflow moves E-7 to Sprint 5 (not 9 pts in Sprint 3); Sprint 2 gate text matches. |
| QA NB2 | Sprint 4 entry requires E-7 merged; fallback defined. |
| QA NB3, DevOps 1 | Runner checks added for Sprints 5, 7, 10, 11 (with 3, 4, 9, 12). |
| QA NB4 | G0 states median of 10 valid runs and the E-1 MB definition; A-1 re-run under the protocol or deviation recorded. |
| QA NB5 | A-3b dial-once test with a resolver that changes its answer on a second lookup. |
| PO N1-N3 | PRD Goal 2 and 5 reworded; E-1 defines tokenizer, baseline, success and overhead; E-5 owns the 10-URL live smoke and overhead figure. |
| PO N4, N5, N6 | FR-11 default per OQ-3 (open); A-3b concurrency/timeout are compiled defaults until C-1; stale Priority columns dropped from EPICS story maps. |
| SM 3, 2 | Sprint 9 order D-2 then D-4; Sprint 5 entry/exit added; Sprints 6-12 detail still to refine before each. |
| DevOps 2-4 | Branch protection AC in D-7; E-2 creates the minimal nightly/dispatch workflow (main/tag triggers in D-2) or runs are manual; sprint branch sequencing rule added. |

Told numbers changed: total 96 to 97 points; MVP 72 to 73; stories 33 to 34; complete memory gate end of Sprint 4 to end of Sprint 5 (G4a stays end of Sprint 4). Unchanged: MVP Sprint 9, v1.0 Sprint 12, every sprint <= 8. OQ-3, OQ-4, OQ-5 and OQ-7 remain open.

## Decisions log

| Date | Decision | By | Effect |
|---|---|---|---|
| 2026-09-19 | A-3 split into A-3a and A-3b | User | Sprint 1 and 2 as above |
| 2026-09-19 | E-8 (`bench-loopback` compile-time feature) CONFIRMED (recorded as confirmed, not proposed) | User | E-2 and E-4 peak on the bench build; does not decide OQ-4 |
| 2026-09-19 | G4a scenario lists fixed: G4a = idle plus scenarios needing only A-3b and A-4; G4b (end of Sprint 5) gates window, early-stop and raw | User | Complete memory gate at end of Sprint 5; targets unchanged |
| 2026-09-19 | Release profile pinned early in D-7, finalised and re-measured in D-1 (blocks MVP tagging) | User | All gates measure the pinned profile; no point change |

## Round 3 / escalation resolution (2026-09-19)

| Finding | Resolution |
|---|---|
| PO B1 (G4a needs A-5 window) | User decision: scenario lists fixed, not reinterpreted. G4a keeps only full-consumption scenarios needing A-3b and A-4 (5 MB page, gzipped, late-landmark holdback-full, 50 MB Content-Length, 50 MB chunked full read beyond cap). Window at start (early stop), window at end, chunked in-cap window, window beyond cap and `raw=true` move to G4b, end of Sprint 5. E-4 ACs, A-3b chunked AC, Sprint 4 exit, PRD M2 and decision rule, EPICS decision gates and this plan made consistent. Complete gate is at the end of Sprint 5; stated as such. |
| QA N4 | G4b owned by A-5 (window scenarios, runs) and A-6 (raw and combined record), ACs added; harness maintenance for those scenarios in scope; not part of E-4's Done. |
| QA B1, PO N1, DevOps N5 | User decision: release profile pinned in D-7 (Sprint 0) so every gate measures it; D-1 (Sprint 8) finalises and must re-measure idle and peak on the shipped build and the aarch64 runner (gnu and musl), blocking MVP tagging on a miss; E-5 (Sprint 7) reports on the pinned profile and says the D-1 re-measure governs the tag. No points change. |
| PO N3, QA SM 3 | E-8 recorded as CONFIRMED in plan, EPICS and PRD, and in the decisions log and EPICS Open Items; not a decision on OQ-4. |
| PO N4, DevOps N1, N2 | D-2 guard asserts `test-support`, `bench-loopback` and the fixture-CA feature, and runs cargo tree plus a marker grep on every release artifact; D-7 guard has a self-test (positive control), `-p <crate>` release build, and E-8 tests run with the feature in hosted CI. DevOps N3: shipped and bench binaries report the same commit and Cargo.lock hash (E-2, E-8). DevOps N6: `publish = false` and license handling and clippy on A-1 in D-7. DevOps N4 (musl and macOS as shipped artifacts) remains for D-2 detail before Sprint 9. |
| QA N2 | Numeric bounds: idle delta <= 0.5 MB (kept as told), public-host peak within 10% of bench and <= 40 MB, size delta explained; exceeding fails G4a. |
| QA N3 | A-3b merge gate is machine-checked: non-defaulted `Policy` in the client constructor and a required CI check running `a3b_merge_gate` plus the four-refusal tests. |
| QA N1 | E-7 fallback: Sprint 4 exit and A-4 AC state the 95%/50% check is excluded when E-7 slips, A-4 not Done until met in Sprint 5. |
| QA N5 | One MB definition in E-1 (10^6 or 2^20, stated once) covering targets, fixtures and the 5 MB cap; median of 10 valid runs everywhere; the harness rejects fewer than 10. |
| PO N2 | OQ-3 due before Sprint 10 starts (was 11) so C-1's default is not a hidden decision; C-1 uses a placeholder; OQ-3 stays OPEN. |
| SM 1, 2 | Sprint 5 has its own block and is flagged as a thin 6-pt sprint (2 pts slack); entry and exit criteria added for Sprints 6-12. Sprint 3 zero slack noted (A-9 drops first). |

Told numbers changed: none in points or counts (97 pts, 34 stories, MVP 73 pts in Sprint 9, v1.0 Sprint 12, every sprint <= 8). Changed dates: OQ-3 due before Sprint 10 (was 11). Restated plainly: complete memory gate at end of Sprint 5, G4a at end of Sprint 4 covers fewer scenarios than round 2 listed. No point change was unavoidable, so none is flagged. OQ-3, OQ-4, OQ-5 and OQ-7 remain OPEN and undecided.
