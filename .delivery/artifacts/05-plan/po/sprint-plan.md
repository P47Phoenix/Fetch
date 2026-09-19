# Sprint Plan (Stage 5): Fetch MCP Server

Inputs: `docs/PRD.md` v0.4, `docs/EPICS.md` (31 stories after the A-3 split), architecture sections 11, 14.1, 15 and ADR-001..006. Roles: Product Owner + Scrum Bag. Date: 2026-09-19.

## Top of plan: flags

1. **OQ-5 is due before Sprint 2 starts** (owner Michael). Labelling can change the `fetch` result envelope produced by A-3b (Sprint 2) and A-4 (Sprint 4). B-6 itself is Sprint 12. OQ-3, OQ-4, OQ-7 stay open (OQ-3 needed before B-4 in Sprint 11, OQ-4 before C-2 in Sprint 10, OQ-7 before D-6 in Sprint 12).
2. **A-3 split (user decision 2026-09-19).** Points rose from 5 to 5 + 5, so total scope is 92 points (was 87) and MVP is 68 points (was 63). MVP is still reached at the end of Sprint 9; v1.0 moves from Sprint 11 to Sprint 12.
3. **ARM runner access** is a hard dependency for the Sprint 0 handshake check, the Sprint 3 and Sprint 4 measurements, and the Sprint 4 memory gate.

## The A-3 split and why this cut

| Story | Scope | Pts |
|---|---|---|
| A-3a SSRF core | `ssrf::ranges` full table, IP-literal check, resolver filter (resolve once, refuse on any blocked answer, return validated set), per-hop revalidation function, fail-closed default `Policy`, test-only constructor, table-driven unit tests with an injectable resolver | 5 |
| A-3b fetch client | reqwest client, manual redirect loop wired to A-3a, deadline, flate2 gzip, header limits, byte caps, semaphore, UTF-8 decoder, integration tests (four refusals) | 5 |

Justification for the cut: it follows the module seam in architecture 15 (`ssrf::*` versus `fetch::*`). A-3a has no dependency on the HTTP client and needs no network, so it can be finished and reviewed on its own. It is first, and Sprint 1 (A-2 + A-3a = 8 pts) stays at the ceiling. A-3b is Sprint 2, so no fetch-capable build exists in Sprint 1. Alternatives rejected: (a) SSRF second: creates a window with an unguarded client. (b) One 10-pt story: exceeds the 8-pt story maximum. (c) Three-way split with semaphore/decoder separate: adds overhead for two small items.

**Merge gate (A-3b):** the A-3b PR is not merged unless (1) A-3a is already on the sprint branch, (2) every request and redirect hop goes through `check_url` + resolver filter, (3) the integration tests prove `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused, and (4) the default `Policy` is fail-closed and the test constructor is absent from release builds. The four refusal AC moved from A-3 to A-3b because they need the real client; A-3a covers the same cases at unit level.

## Capacity and ceiling

Solo part-time, ~10 pts per 2-week sprint, commitment at most 8 (80%). Planned commitments: 6, 8, 6, 7, 8, 6, 8, 8, 6, 7, 7, 8, 7 = 92. No sprint exceeds 8. Stories moved by the split: A-4 (Sprint 2 to 4), A-5 (2 to 5), A-6 (3 to 5), A-7 (4 to 6), and downstream B/D/C/A-8 stories shifted; E-2, E-3 (Sprint 3) and E-4 (Sprint 4) keep their sprints or close to them so the memory gate stays at the end of Sprint 4.

## Sprints 0-4 (detailed)

### Sprint 0: De-risk (6 pts)
**Goal:** Fix the memory targets and measurement method, and confirm the crate stack builds and runs on ARM, so a go/no-go can be made.
Stories: E-1 (3, spike 2 days), A-1 (3, spike 3 days; already delivered on this branch, close out formally).
Dependencies: none. ARM runner for A-1 handshake and E-1 host record.
Entry: PRD v0.4, epics accepted, benchmark host identified.
Exit: E-1 one-page result committed (targets, MB definition, fixtures, TLS approach, 50-URL set, host OS/RAM); A-1 result lists chosen crates and dependency count vs NFR-05 (<= 15); PRD assumption changes recorded.
**Gate G0 (go/no-go):** targets (idle <= 10, peak <= 40 for 5 MB) look achievable per A-1 measurements. No-go: re-scope PRD Goal 1 before any feature work.

### Sprint 1: Skeleton and SSRF core (8 pts)
**Goal:** A registered `fetch` tool that validates input, and a tested address-blocking core, with no network-capable code yet.
Stories: A-2 (3), A-3a (5).
Dependencies: A-1. A-3a depends on A-2 (`config`, `error`, `Policy` skeleton).
Entry: G0 = go; branch `sprint-1/skeleton-ssrf` and draft PR opened.
Exit: `tools/list` shows exactly `fetch`; stdout-purity test passes; ready within 250 ms on ARM; A-3a table-driven tests pass for every range; default policy fail-closed; `cargo test`, clippy, fmt green.
Gate: no HTTP client dependency is present in the crate at end of Sprint 1 (or it is unreachable from `fetch`).

### Sprint 2: Guarded streaming fetch (6 pts)
**Goal:** Claude Code can fetch a public page through a size-bounded stream that refuses internal addresses.
Stories: A-3b (5), A-9 (1).
Dependencies: A-3a (merge gate above), A-2. **OQ-5 decided before start.**
Entry: A-3a merged; OQ-5 decision recorded; flate2 version chosen for pinning.
Exit: all A-3b AC pass, including four-refusal integration test, gzip/bomb/header-bomb fixtures, semaphore test; flate2 pinned exact; verify hyper buffer defaults (architecture row a); UTF-8 decoder in place.
Gate: A-3b merge gate. Sprint is deliberately 6 pts (highest-risk story, re-estimated) and the slack absorbs overrun.
Note: pre-M3 builds are not registered in a real MCP client (release rule); Claude Code check uses a throwaway config on the author's machine only against public URLs.

### Sprint 3: Harness and idle RSS (7 pts)
**Goal:** A one-command harness on native ARM that reports idle RSS against the 10 MB target.
Stories: E-2 (5), E-3 (2).
Dependencies: E-1, A-3b, A-2. ARM runner required (native preflight refuses QEMU).
Exit: fixtures with committed sha256 manifest; G1-G7 scenarios defined; idle RSS median of 10 recorded; harness exits non-zero when over target.
Gate: idle RSS <= 10 MB (strict). Miss triggers an investigation item before Sprint 4 starts.

### Sprint 4: Convert and memory gate (8 pts)
**Goal:** HTML converts to markdown within budget and peak memory is proven bounded.
Stories: A-4 (5), E-4 (3).
Dependencies: A-3b, E-2. A-4 lands first in the sprint; E-4 needs it (`lol_html` limits, architecture rows f/g).
Exit: A-4 AC (token reduction on curated set, 1 MB in <= 500 ms p95 on aarch64, converter behind a trait); E-4 fixtures: 5 MB peak <= 40 MB; 50 MB with Content-Length -> `too_large` within 10% of 5 MB peak; chunked in-cap -> success; chunked beyond cap -> `too_large`; 10 concurrent recorded; allocator recorded.
**Gate G4 (memory gate, end of Sprint 4):** idle RSS <= 10 MB and 5 MB peak (VmHWM, max of per-scenario medians) <= 40 MB on native aarch64, each median of 10 valid runs. Pass: continue to safety and packaging. Fail: stop feature work and run a memory-reduction sprint (allocator, buffer sizes, converter swap via trait) before Sprint 5. Requires ARM runner access; without it the gate cannot be evaluated and Sprint 5 does not start.

## Sprints 5-11 summary (12 for v1.0)

| Sprint | Goal | Stories | Pts | Notes |
|---|---|---|---|---|
| 5 | Paginate and content types | A-5, A-6 | 6 | |
| 6 | Clear errors, private-IP test depth | A-7, B-1 | 8 | B-1 hardens A-3a code |
| 7 | Redirect limit, encoded forms, report | B-3, B-2, E-5 | 8 | E-5 needs E-3, E-4, A-7 |
| 8 | SSRF suite, coverage gate, release profile | B-5, D-1, D-5 | 6 | M3 (Safety complete) reached when B-5 done |
| 9 | ARM release pipeline and docs | D-2, D-4 | 7 | MVP complete (68 pts) |
| 10 | Config and allowlist | C-1, C-2, C-3 | 7 | C-2 needs OQ-4; drop = 5 pts |
| 11 | robots, charset, ARM tests | B-4, A-8, D-3 | 8 | B-4 needs OQ-3 |
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 | needs OQ-5, OQ-7, ARM CI runner |

MVP = 68 pts, end of Sprint 9. Tagging/distribution only after M3 (Sprint 8) and E-5 release decision.

## Definition of Done (per story)

- All acceptance criteria have a passing test that asserts them.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` pass.
- No new direct dependency without recording it against the NFR-05 count; high-churn deps use exact `=` pins (architecture 8).
- stdout carries only MCP messages; logs to stderr.
- Stories touching fetch, decode, convert or concurrency (A-3b, A-4, A-5, A-6, C-3, E-*): benchmark run on native aarch64 with results recorded; no regression against the absolute targets.
- SSRF-touching stories (A-3a, A-3b, B-*, C-2): table-driven cases; release build asserted free of `test-support`.
- Docs touched where behaviour is user-visible; PR reviewed (self-review checklist for a solo project) and CI green.

## Branch and PR convention

One branch per sprint, `sprint-N/<slug>` (Sprint 0: `sprint-0/spikes`), created from main. One draft PR per sprint targeting `main`, opened at sprint start, titled `Sprint N: <goal>`, body listing stories and gates. Commits reference the story ID (`A-3a: ...`). PR marked ready and merged only when the sprint exit criteria and gates hold. No pushes without the owner's instruction. Tagged builds are prohibited before M3.

## Traceability (Sprints 0-4 stories)

| Story | FR / NFR | Architecture module / ADR |
|---|---|---|
| E-1 | NFR-10, 11, 14 | bench design, sec 11 |
| A-1 | FR-01, FR-15, NFR-05, 15 | spike; ADR-001, 002, 005 |
| A-2 | FR-01, FR-02, FR-13, NFR-01 | `main`, `server`, `config`, `error`, `obs`; ADR-006 |
| A-3a | FR-06, NFR-04, Risk 2 | `ssrf::ranges`, `ssrf::resolver`, `ssrf::check_url`; ADR-003, sec 14.1 |
| A-3b | FR-07, FR-16, NFR-07, NFR-08 | `fetch` (client, redirect loop, body, deadline), `config`, flate2; ADR-001, 003, 004 |
| A-9 | FR-14 | `server::render` |
| E-2 | NFR-14 | `bench/`, sec 11 |
| E-3 | NFR-10 | `bench/` |
| A-4 | FR-03, NFR-02 | `convert::html`, `boilerplate`; ADR-002 |
| E-4 | NFR-11, 12, FR-16 | `bench/`, ADR-004 |

Later stories follow `docs/EPICS.md` (traceability table) and architecture section 15 unchanged; FR-06 maps to A-3a, B-1, B-2, C-2; FR-07 to A-3b, C-3.

## Risks and open dependencies

| Item | Impact | Mitigation / owner |
|---|---|---|
| ARM runner access (author's aarch64 cluster) | Blocks G0 handshake, Sprint 3-4 measurements, G4 gate, E-6, D-3 | Confirm access and record OS/RAM before Sprint 0 exit; no native runner means G4 not evaluated and no release; QEMU never used for RSS (Michael) |
| OQ-5 (labelling) due before Sprint 2 | Result envelope rework in A-3b/A-4 | Decide before Sprint 2; default assumption "label", fixed prefix not shifting `start_index` (Michael) |
| OQ-3, OQ-4, OQ-7 open | B-4, C-2, D-6 blocked at their sprints | Decide before Sprints 11, 10, 12 |
| A-3b estimate (5 pts, high scope) | Sprint 2 overrun | Sprint 2 committed at 6; overflow takes A-9 out first |
| Interim unsafe window | LAN probing by pre-M3 builds | A-3a first, merge gate, no tags or client registration before M3 |
| Memory gate miss at G4 | Re-plan | Memory-reduction sprint inserted, later sprints shift |
| Pinned dependency drift (flate2 etc.) | Benchmark validity | Pin on add; bump PR re-runs benchmark |
