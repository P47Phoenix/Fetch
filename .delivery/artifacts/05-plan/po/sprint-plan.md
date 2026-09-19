# Sprint Plan (Stage 5): Fetch MCP Server

Inputs: `docs/PRD.md` v0.4, `docs/EPICS.md` (33 stories after the A-3 split and plan revision 1), architecture sections 11, 14.1, 15 and ADR-001..006. Roles: Product Owner + Scrum Bag. Date: 2026-09-19.

## Top of plan: flags

1. **Open questions and due sprints (all still OPEN, undecided).** OQ-5 due before Sprint 2 starts (envelope of A-3b/A-4; B-6 is Sprint 12; any default assumption is not a decision). OQ-4 before Sprint 10 (C-2). OQ-3 before Sprint 11 (B-4). **OQ-7 now due before Sprint 9 starts** (was Sprint 12): D-2 artifact naming and publication, D-4 install guide and the LICENSE need it; D-6 (Sprint 12) still uses it.
2. **Scope and dates (after revision 1).** Total scope is 96 points (was 92; 87 before the A-3 split). MVP is 72 points (was 68; 63 before the split), still reached at the end of Sprint 9. v1.0 is still Sprint 12. Plan revision 1 added D-7 (2 pts) and E-7 (2 pts) and moved A-9 to Sprint 3. Told numbers that moved: total 92 to 96, MVP 68 to 72 points; told sprints did not move.
3. **ARM runner access** is a hard dependency for the Sprint 0 handshake check, the Sprint 1 readiness check (250 ms), the Sprint 2 RSS smoke, the Sprint 3 and Sprint 4 measurements, and the Sprint 4 memory gate. Runner availability is checked at the start of Sprints 3, 4, 9 and 12.

## The A-3 split and why this cut

| Story | Scope | Pts |
|---|---|---|
| A-3a SSRF core | `ssrf::ranges` full table, IP-literal check, resolver filter (resolve once, refuse on any blocked answer, return validated set), per-hop revalidation function, fail-closed default `Policy`, test-only constructor, table-driven unit tests with an injectable resolver | 5 |
| A-3b fetch client | reqwest client, manual redirect loop wired to A-3a, deadline, flate2 gzip, header limits, byte caps, semaphore, UTF-8 decoder, integration tests (four refusals) | 5 |

Justification for the cut: it follows the module seam in architecture 15 (`ssrf::*` versus `fetch::*`). A-3a has no dependency on the HTTP client and needs no network, so it can be finished and reviewed on its own. It is first, and Sprint 1 (A-2 + A-3a = 8 pts) stays at the ceiling. A-3b is Sprint 2, so no fetch-capable build exists in Sprint 1. Alternatives rejected: (a) SSRF second: creates a window with an unguarded client. (b) One 10-pt story: exceeds the 8-pt story maximum. (c) Three-way split with semaphore/decoder separate: adds overhead for two small items.

**Merge gate (A-3b):** the A-3b PR is not merged unless (1) A-3a is already on the sprint branch, (2) every request and redirect hop goes through `check_url` + resolver filter, (3) the integration tests prove `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused, and (4) the default `Policy` is fail-closed and the test constructor is absent from release builds. The four refusal AC moved from A-3 to A-3b because they need the real client; A-3a covers the same cases at unit level.

## Capacity and ceiling

Solo part-time, ~10 pts per 2-week sprint, commitment at most 8 (80%). Planned commitments: 8, 8, 7, 8, 8, 6, 8, 8, 6, 7, 7, 8, 7 = 96. No sprint exceeds 8. Stories moved by the split: A-4 (Sprint 2 to 4), A-5 (2 to 5), A-6 (3 to 5), A-7 (4 to 6), and downstream B/D/C/A-8 stories shifted; E-2, E-3 (Sprint 3) and E-4 (Sprint 4) keep exactly Sprints 3, 3 and 4 so the memory gate stays at the end of Sprint 4. Revision 1: D-7 added to Sprint 0, E-7 to Sprint 2, A-9 moved from Sprint 2 to Sprint 3. Sprint 0 has only 5 pts of remaining work because A-1 is already delivered.

## Sprints 0-4 (detailed)

### Sprint 0: De-risk and CI baseline (8 pts)
**Goal:** Fix the memory targets and measurement method, and confirm the crate stack builds and runs on ARM, so a go/no-go can be made.
Stories: E-1 (3, spike 2 days), A-1 (3, spike 3 days; already delivered on this branch, close out formally), D-7 (2, hosted PR CI: fmt, clippy, test, deny, SHA-pinned actions, toolchain pin, release-feature-guard skeleton).
Dependencies: none. ARM runner for A-1 handshake and E-1 host record. Owner's sprint-start instruction covers pushing the branch and opening the PR (needed for CI).
Entry: PRD v0.4, epics accepted, benchmark host identified.
Exit: native aarch64 host access confirmed and OS/RAM recorded; hosted CI green on the Sprint 0 PR (D-7); E-1 one-page result committed (targets, MB definition, fixtures, TLS approach, 50-URL list, host OS/RAM); A-1 result lists chosen crates and dependency count vs NFR-05 (<= 15); PRD assumption changes recorded.
**Gate G0 (go/no-go):** measurable rule: A-1 measured idle RSS <= 10 MB and 5 MB-fetch peak <= 40 MB on the native aarch64 host (glibc), or a written gap analysis with a credible path; ARM host recorded. No-go: re-scope PRD Goal 1 before any feature work.

### Sprint 1: Skeleton and SSRF core (8 pts)
**Goal:** A registered `fetch` tool that validates input, and a tested address-blocking core, with no network-capable code yet.
Stories: A-2 (3), A-3a (5).
Dependencies: A-1, D-7 (CI). ARM runner for the 250 ms readiness check. A-3a depends on A-2 (`config`, `error`, `Policy` skeleton).
Entry: G0 = go; branch `sprint-1/skeleton-ssrf` and draft PR opened.
Exit: `tools/list` shows exactly `fetch`; stdout-purity test passes; ready within 250 ms on ARM; A-3a table-driven tests pass for every range; default policy fail-closed; the CI release-build check (no `test-support`, via `cargo tree -e features` and symbol grep) is a required check and passes; hosted CI green. Claude Code check for A-2 uses a throwaway config only.
Gate: no HTTP client dependency is present in the crate at end of Sprint 1 (or it is unreachable from `fetch`).

### Sprint 2: Guarded streaming fetch and snapshots (7 pts)
**Goal:** Claude Code can fetch a public page through a size-bounded stream that refuses internal addresses.
Stories: A-3b (5), E-7 (2: capture the 50-URL offline snapshots with committed sha256 manifest).
Dependencies: A-3a (merge gate above), A-2, D-7, E-1 (URL list). ARM runner for the RSS smoke. **OQ-5 decided before start.**
Entry: A-3a merged; OQ-5 decision recorded; flate2 version chosen for pinning.
Exit: all A-3b AC pass, including four-refusal integration test, gzip/bomb/header-bomb fixtures, semaphore test; flate2 pinned exact; verify hyper buffer defaults (architecture row a); UTF-8 decoder in place; non-gating manual RSS smoke (one 5 MB fetch, VmHWM) on native aarch64 recorded; E-7 snapshot set and manifest committed.
DoD scoping: the per-story native-aarch64 benchmark rule does not apply to A-3b beyond this smoke, because the harness lands in Sprint 3; the full memory gate is E-3/E-4 (Sprints 3-4). E-1 and A-1 (spikes) are exempt.
Gate: A-3b merge gate. Sprint is 7 pts; if A-3b overruns, E-7 slips first (to Sprint 3 in place of A-9, which moves to Sprint 5).
Note: pre-M3 builds are not registered in a real MCP client (release rule); Claude Code check uses a throwaway config on the author's machine only against public URLs.

### Sprint 3: Harness and idle RSS (8 pts)
**Goal:** A one-command harness on native ARM that reports idle RSS against the 10 MB target.
Stories: E-2 (5), E-3 (2), A-9 (1, moved from Sprint 2; first to drop if E-2 overruns).
Dependencies: E-1, A-3b, A-2. ARM runner required (native preflight refuses QEMU); runner provisioned and isolated per architecture 9.2 as part of E-2; interim gnu+musl build script with pinned `cargo-zigbuild`/`ziglang` versions (D-2 adopts the same pins).
Exit: fixtures with committed sha256 manifest; G1-G7 scenarios defined; idle RSS median of 10 recorded; harness exits non-zero when over target.
Gate: idle RSS <= 10 MB (strict), native aarch64 only; QEMU or non-native figures never count. Miss triggers an investigation item before Sprint 4 starts.

### Sprint 4: Convert and memory gate (8 pts)
**Goal:** HTML converts to markdown within budget and peak memory is proven bounded.
Stories: A-4 (5), E-4 (3).
Dependencies: A-3b, E-2, E-7. A-4 lands first in the sprint (E-4 is first priority if A-4 slips); E-4 needs it (`lol_html` limits, architecture rows f/g).
Exit: A-4 AC (on the E-7 snapshot set: >= 95% conversion success and median token reduction >= 50%, no `<script>` text; 1 MB in <= 500 ms p95 on aarch64, converter behind a trait); E-4 fixtures: 5 MB peak <= 40 MB; 50 MB with Content-Length -> `too_large` within 10% of 5 MB peak; chunked in-cap -> success; chunked beyond cap -> `too_large`; 10 concurrent recorded; allocator recorded.
**Gate G4 (memory gate, end of Sprint 4):** idle RSS <= 10 MB and 5 MB peak (VmHWM, max of per-scenario medians, MB as defined in E-1) <= 40 MB on native aarch64, each median of 10 valid runs, for both gnu and musl binaries, plus the E-4 boundedness checks (50 MB Content-Length and chunked cases within 10% of the 5 MB peak). The 10-concurrent scenario is recorded and reported, not gating. QEMU or non-native figures never count. Pass: continue to safety and packaging. Fail: stop feature work and run a memory-reduction sprint (allocator, buffer sizes, converter swap via trait) before Sprint 5. Requires ARM runner access; without it the gate cannot be evaluated and Sprint 5 does not start.

## Sprints 5-12 summary (12 for v1.0)

| Sprint | Goal | Stories | Pts | Notes |
|---|---|---|---|---|
| 5 | Paginate and content types | A-5, A-6 | 6 | Re-run idle/peak at sprint end (cheap, non-gating) |
| 6 | Clear errors, private-IP test depth | A-7, B-1 | 8 | B-1 hardens A-3a code |
| 7 | Redirect limit, encoded forms, report | B-3, B-2, E-5 | 8 | E-5 needs E-3, E-4, A-7 |
| 8 | SSRF suite, coverage gate, release profile | B-5, D-1, D-5 | 6 | M3 (Safety complete) reached when B-5 done |
| 9 | ARM release pipeline and docs | D-2, D-4 | 7 | MVP complete (72 pts); needs OQ-7 decided before start; no aarch64 test or memory-gate required check yet (manual bench run + E-5 stand in) |
| 10 | Config and allowlist | C-1, C-2, C-3 | 7 | C-2 needs OQ-4; drop = 5 pts; re-check D-4 config text |
| 11 | robots, charset, ARM tests | B-4, A-8, D-3 | 8 | B-4 needs OQ-3 |
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 | needs OQ-5, ARM CI runner; E-6/D-3 become required checks for the release workflow |

MVP = 72 pts, end of Sprint 9. Tagging/distribution only after M3 (Sprint 8) and the E-5 release decision; the MVP release relies on the documented manual bench run until E-6 (Sprint 12), and D-6 is the v1.0 tagger.

Milestones M0-M5 map to sprints in PRD section 9: M0 S0, M1 S1-2, M2 S3-6, M3 S6-8 (reached end of S8), M4 S9-11, M5 S11-12.

## Definition of Done (per story)

- All acceptance criteria have a passing test that asserts them.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` pass on the hosted CI from D-7 (required checks from Sprint 0).
- No new direct dependency without recording it against the NFR-05 count; high-churn deps use exact `=` pins (architecture 8).
- stdout carries only MCP messages; logs to stderr.
- Stories touching fetch, decode, convert or concurrency (A-4, A-5, A-6, C-3, E-2 to E-6): benchmark run on native aarch64 with results recorded (manual on PRs); no regression against the absolute targets. A-3b: non-gating manual RSS smoke only (harness arrives in Sprint 3). Spikes E-1 and A-1 and E-7 are exempt.
- SSRF-touching stories (A-3a, A-3b, B-*, C-2): table-driven cases; release build asserted free of `test-support`.
- Docs touched where behaviour is user-visible; PR reviewed (self-review checklist for a solo project) and CI green.

## Branch and PR convention

One branch per sprint, `sprint-N/<slug>` (Sprint 0: `sprint-0/spikes`), created from main. One draft PR per sprint targeting `main`, opened at sprint start, titled `Sprint N: <goal>`, body listing stories and gates. Commits reference the story ID (`A-3a: ...`). PR marked ready and merged only when the sprint exit criteria and gates hold. No pushes without the owner's instruction. Tagged builds are prohibited before M3.

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
| E-2 | NFR-14 | `bench/`, sec 11 |
| E-3 | NFR-10 | `bench/` |
| A-4 | FR-03, NFR-02 | `convert::html`, `boilerplate`; ADR-002 |
| E-4 | NFR-11, 12, FR-16 | `bench/`, ADR-004 |

Later stories follow `docs/EPICS.md` (traceability table) and architecture section 15 unchanged; FR-06 maps to A-3a, B-1, B-2, C-2; FR-07 to A-3b, C-3.

## Risks and open dependencies

| Item | Impact | Mitigation / owner |
|---|---|---|
| ARM runner access (author's aarch64 cluster) | Blocks G0 handshake, Sprint 1 readiness, Sprint 2 smoke, Sprint 3-4 measurements, G4 gate, E-6, D-3; a runner outage in Sprint 12 leaves no slack | Confirm access and record OS/RAM before Sprint 0 exit; no native runner means G4 not evaluated and no release; QEMU never used for RSS (Michael) |
| OQ-5 (labelling) due before Sprint 2 | Result envelope rework in A-3b/A-4 | Decide before Sprint 2 (Michael); a "label, prefix not shifting `start_index`" working assumption is NOT a decision |
| OQ-3, OQ-4, OQ-7 open | B-4, C-2, D-2/D-4/D-6 blocked at their sprints | Decide OQ-4 before Sprint 10, OQ-3 before Sprint 11, OQ-7 before Sprint 9 |
| A-3b estimate (5 pts, high scope) | Sprint 2 overrun | Sprint 2 committed at 6; overflow takes A-9 out first |
| Interim unsafe window | LAN probing by pre-M3 builds | A-3a first, merge gate, no tags or client registration before M3 |
| Memory gate miss at G4 | Re-plan | Memory-reduction sprint inserted, later sprints shift |
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
