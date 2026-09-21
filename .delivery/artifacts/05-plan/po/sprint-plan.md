# Sprint Plan (Stage 5): Fetch MCP Server

Inputs: `docs/PRD.md` v0.4, `docs/EPICS.md` (34 stories after the A-3 split and plan revisions 1 and 2), architecture sections 11, 14.1, 15 and ADR-001..006. Roles: Product Owner + Scrum Bag. Date: 2026-09-19.

## Top of plan: flags

1. **Open questions and due sprints (all still OPEN, undecided).** OQ-5 due before Sprint 2 starts (envelope of A-3b/A-4; B-6 is Sprint 12; any default assumption is not a decision). OQ-4 before Sprint 10 (C-2). OQ-3 **now due before Sprint 10 starts** (was Sprint 11): C-1 (Sprint 10) parses the robots toggle and needs the default; C-1 uses a placeholder only and does not decide it; B-4 (Sprint 11) still uses it. OQ-3 stays OPEN. **OQ-7 now due before Sprint 9 starts** (was Sprint 12): D-2 artifact naming and publication, D-4 install guide and the LICENSE need it; D-6 (Sprint 12) still uses it.
2. **Scope and dates (after revision 2).** **Current (user acceptance, 2026-09-19, see decisions log): total 100 points, 34 stories, MVP 76 points finishing at the end of Sprint 10; v1.0 is still Sprint 12.** (History: 97 points and MVP 73 in Sprint 9 after revision 2; 96 in revision 1; 92 before; 87 before the A-3 split.) Revision 2 added E-8 (1 pt, Sprint 2), now CONFIRMED by the user. **Round 3 (user decision, 2026-09-19):** the memory gate is split with corrected scenario lists. G4a (end of Sprint 4) is idle plus only full-consumption scenarios that need A-3b and A-4 (nothing that needs the A-5 window or early stop, or A-6 raw). G4b (end of Sprint 5, owned by A-5 and A-6) gates the window, early-stop and `raw=true` scenarios at the same targets. **Plainly: the complete memory gate is at the end of Sprint 5, one sprint later than originally told;** a G4a pass means 'go on to Sprint 5', not 'gate closed'. MVP (Sprint 9), v1.0 (Sprint 12) and the 8-pt sprint ceiling do not move. The 10 MiB idle and 40 MiB peak targets are not lowered. Counts were unchanged in round 3 (97 pts, 34 stories, MVP 73 pts in Sprint 9); superseded by the accepted ADR-007 re-estimate above.
3. **GitHub-hosted arm64 runner (ADR-007; replaces "ARM runner access", no self-hosted runner)** is the environment for the Sprint 0 handshake check, the Sprint 1 readiness check (250 ms), the Sprint 2 RSS smoke, the Sprint 3 and Sprint 4 measurements, the Sprint 4 gate G4a and the Sprint 5 gate G4b. This is no longer a person-dependent access item: it needs only the public repository and GitHub Actions. Hosted-runner availability and quota (free for standard arm64 runners on public repos at the time of writing; verify at Sprint 3) are checked at the start of Sprints 3, 4, 5, 7 (E-5 report), 9, 10 (C-3), 11 (D-3, A-8) and 12. The user's decision (2026-09-19): measurement environment = hosted cloud VM, not the Pi 5 cluster; targets unchanged; page-size and CPU caveats are in ADR-007.
4. **CONFIRMED by the user (2026-09-19): bench loopback path (E-8).** The shipped binary's fail-closed policy blocks the loopback fixture server. Recommended: a compile-time Cargo feature `bench-loopback` (off by default, permits only 127.0.0.0/8 and ::1, every other blocked range still refuses), built as a second binary from the same commit by the same pinned pipeline; D-7 asserts it and `test-support` are absent from release builds. Idle RSS is gated on the shipped binary; peak RSS on the bench build. Details and the fidelity trade-off are in E-8 (EPICS). OQ-4 is not decided by this.
5. **Release profile pinned early (user decision, 2026-09-19).** D-7 (Sprint 0) fixes the release profile (opt-level, lto, panic=abort, strip, codegen-units) so G0, G4a, G4b and E-5 measure what ships. D-1 (Sprint 8) finalises it and must re-measure idle and peak on the shipped build, on the hosted arm64 runner, measured inside the image (gnu and musl candidates until ADR-005 decides); a miss blocks MVP tagging. No points change.
6. **User decisions 2026-09-19 (ADR-007).** (a) The native memory measurement (G0 evidence, G4a, G4b, E-5, D-1, E-6) runs on GitHub-hosted runners, BOTH `linux/amd64` (`ubuntu-24.04`) and `linux/arm64` (`ubuntu-24.04-arm`) hard-gated; no self-hosted runner; the OQ-9 self-hosted trust items are resolved by removal (least-privilege token, no `pull_request_target`, SHA-pinned actions kept). (b) The release artifact is a tested multi-arch container image on GHCR published only from GitHub Actions after BOTH platform gates pass on the exact per-platform and manifest digests; standalone binary releases dropped. Windows containers, arm/v7, 386, riscv64, ppc64le, s390x considered and DEFERRED, not rejected; a platform that cannot be natively verified may ship only labelled "memory targets not verified on this platform"; QEMU never gates. **OQ-7 must be answered before the first image is published** (already due before Sprint 9; ordering confirmed). **Re-estimate (honest, unvalidated) ACCEPTED by the user 2026-09-19: D-2 8 pts; total 100, MVP 76; D-2 alone fills Sprint 9 (8); D-4 (2) moves to Sprint 10 (9 pts with C-2, 7 without). The user accepted Sprint 10 at 9 pts, one over the 8-pt ceiling; C-3 is NOT deferred; MVP ends at the end of Sprint 10; v1.0 stays Sprint 12. E-2 stays 5 pts with a stated zero-slack risk in Sprint 3 (A-9 moves to Sprint 5 first). D-3 probably shrinks (ARM CI job exists from D-7 work), not re-estimated here.**

## The A-3 split and why this cut

| Story | Scope | Pts |
|---|---|---|
| A-3a SSRF core | `ssrf::ranges` full table, IP-literal check, resolver filter (resolve once, refuse on any blocked answer, return validated set), per-hop revalidation function, fail-closed default `Policy`, test-only constructor, table-driven unit tests with an injectable resolver | 5 |
| A-3b fetch client | reqwest client, manual redirect loop wired to A-3a, deadline, flate2 gzip, header limits, byte caps, semaphore, UTF-8 decoder, integration tests (four refusals) | 5 |

Justification for the cut: it follows the module seam in architecture 15 (`ssrf::*` versus `fetch::*`). A-3a has no dependency on the HTTP client and needs no network, so it can be finished and reviewed on its own. It is first, and Sprint 1 (A-2 + A-3a = 8 pts) stays at the ceiling. A-3b is Sprint 2, so no fetch-capable build exists in Sprint 1. Alternatives rejected: (a) SSRF second: creates a window with an unguarded client. (b) One 10-pt story: exceeds the 8-pt story maximum. (c) Three-way split with semaphore/decoder separate: adds overhead for two small items.

**Merge gate (A-3b), machine-checked:** the client constructor takes a `Policy` with no default on the client path, and a required CI check runs the named test `a3b_merge_gate` plus the four-refusal integration tests; the A-3b PR is not merged unless (1) A-3a is already on the sprint branch, (2) every request and redirect hop goes through `check_url` + resolver filter, (3) the integration tests prove `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused, and (4) the default `Policy` is fail-closed and the test constructor is absent from release builds. The four refusal AC moved from A-3 to A-3b because they need the real client; A-3a covers the same cases at unit level.

## Capacity and ceiling

Solo part-time, ~10 pts per 2-week sprint, commitment at most 8 (80%). Planned commitments after the accepted ADR-007 re-estimate: 8, 8, 8, 8, 8, 6, 8, 8, 6, 8, 9, 8, 7 = 100 (13 sprints; Sprint 10 is 9, one over the ceiling by user acceptance; the earlier 97-pt sequence was 8, 8, 8, 8, 8, 6, 8, 8, 6, 7, 7, 8, 7; sprint 5 is a thin 6-pt block with 2 pts of slack, which G4b evaluation and any G4a remediation can use). No other sprint exceeds 8. Stories moved by the split: A-4 (Sprint 2 to 4), A-5 (2 to 5), A-6 (3 to 5), A-7 (4 to 6), and downstream B/D/C/A-8 stories shifted; E-2, E-3 (Sprint 3) and E-4 (Sprint 4) keep exactly Sprints 3, 3 and 4; G4a stays at the end of Sprint 4 and G4b is at the end of Sprint 5 (revision 2). Revision 1: D-7 added to Sprint 0, E-7 to Sprint 2, A-9 moved from Sprint 2 to Sprint 3. Revision 2: E-8 (1 pt) added to Sprint 2, now at the 8-pt ceiling with no in-plan slack. Sprint 0 has only 5 pts of remaining work because A-1 is already delivered.

## Sprints 0-4 (detailed)

### Sprint 0: De-risk and CI baseline (8 pts)
**Goal:** Fix the memory targets and measurement method, and confirm the crate stack builds and runs on ARM, so a go/no-go can be made.
Stories: E-1 (3, spike 2 days), A-1 (3, spike 3 days; already delivered on this branch, close out formally), D-7 (2, hosted PR CI: fmt, clippy, test, deny, SHA-pinned actions, toolchain pin, release-feature-guard skeleton with self-test, release profile pinned in `Cargo.toml`, `publish = false`; branch protection requires these checks).
Dependencies: none. Hosted arm64 runner (`ubuntu-24.04-arm`) for the A-1 handshake and the E-1 host record (a hosted-ARM CI run of the A-1 spike closes G0; the dev workflow for it is a separate change). Owner's sprint-start instruction covers pushing the branch and opening the PR (needed for CI). E-1 also records the confirmed loopback path (E-8).
Entry: PRD v0.4, epics accepted, benchmark host identified.
Exit: release profile pinned (D-7); hosted arm64 runner run recorded with OS, kernel, CPU model, RAM and page size; hosted CI green on the Sprint 0 PR (D-7); E-1 one-page result committed (targets, MiB definition, fixtures, TLS approach, host OS/RAM; the 50-URL list is DEFERRED to the project owner by user decision in Sprint 0 revision 4 and is a prerequisite for E-7 in Sprint 2); A-1 result lists chosen crates and dependency count vs NFR-05 (<= 15); PRD assumption changes recorded.
**Gate G0 STATUS: CLOSED AS GO on 2026-09-19 (user decision; see UAT report).** Evidence: hosted native aarch64 A-1 spike run (`.delivery/artifacts/06-dev/sprint-0/arm-bench-report.md`): idle 3.66 MiB gnu / 2.09 musl; 5 MiB page peak 16.3 / 10.49 MiB; 10/10 valid runs; advisory (not `--gate`), Azure aarch64 VM, 4 KiB pages, spike not the product; 50 MiB boundedness not run. Conditions carried forward: see UAT report section 5.
**Gate G0 (go/no-go), rule as defined:** measurable rule: A-1 measured idle RSS <= 10 MiB and 5 MiB-fetch peak <= 40 MiB (MiB defined once in E-1 and used for every target, fixture and cap, each the median of 10 valid runs under the E-1 protocol on the D-7 profile; A-1 predates the protocol, so its binary is re-run under it or the deviation is recorded) on the GitHub-hosted arm64 runner (glibc, A-1 spike binary; the image build is not needed for G0), or a written gap analysis with a credible path; runner OS/RAM/page size recorded. G0 is now closable by a hosted-ARM run of the A-1 spike; no ARM-runner-access dependency remains. No-go: re-scope PRD Goal 1 before any feature work.

### Sprint 1: Skeleton and SSRF core (8 pts) - STATUS: DONE (PR #5 merged as 090e86c, 2026-09-20)
**Goal:** A registered `fetch` tool that validates input, and a tested address-blocking core, with no network-capable code yet.
Stories: A-2 (3), A-3a (5).
Dependencies: A-1, D-7 (CI). Hosted arm64 runner for the 250 ms readiness check (restate the figure for process start; container start adds runtime latency, decide at Sprint 1). A-3a depends on A-2 (`config`, `error`, `Policy` skeleton).
Entry: G0 = go; branch `sprint-1/skeleton-ssrf` and draft PR opened.
Exit: `tools/list` shows exactly `fetch`; stdout-purity test passes; ready within 250 ms on ARM; A-3a table-driven tests pass for every range; default policy fail-closed; the CI release-build check (no `test-support`, via `cargo tree -e features` and symbol grep) is a required check and passes; hosted CI green. Claude Code check for A-2 uses a throwaway config only.
Gate: no HTTP client dependency is present in the crate at end of Sprint 1 (or it is unreachable from `fetch`).

### Sprint 2: Guarded streaming fetch, snapshots and bench build (8 pts)
**Goal:** Claude Code can fetch a public page through a size-bounded stream that refuses internal addresses, and a bench-only build can reach the loopback fixture server.
Stories: A-3b (5), E-7 (2: capture the 50-URL offline snapshots with committed sha256 manifest), E-8 (1, CONFIRMED: `bench-loopback` feature).
Dependencies: A-3a (merge gate above), A-2, D-7, E-1 (URL list; loopback path confirmed by the owner 2026-09-19). Hosted arm64 runner for the RSS smoke. **OQ-5 decided before start.**
Entry: A-3a merged; OQ-5 decision recorded; flate2 version chosen for pinning; E-8 loopback path confirmed (done).
Exit: all A-3b AC pass, including the four-refusal integration test (run on the shipped-profile build), the dial-once test, gzip/bomb/header-bomb fixtures, semaphore test; flate2 pinned exact; verify hyper buffer defaults (architecture row a); UTF-8 decoder in place; non-gating manual RSS smoke (one 5 MiB fetch, VmHWM) on native aarch64 recorded; E-7 snapshot set and manifest committed; E-8 unit tests and the D-7 guard prove the feature is absent from release builds.
DoD scoping: the per-story native-aarch64 benchmark rule does not apply to A-3b beyond this smoke, because the harness lands in Sprint 3; the full memory gate is E-3/E-4 (Sprints 3-4). E-1, A-1, E-7 and E-8 are exempt.
Gate: A-3b merge gate. Sprint is exactly 8 pts (ceiling). Overflow rule: A-3b and E-8 are protected (E-2 needs E-8). If either overruns, E-7 slips to Sprint 5 (6 + 2 = 8 pts, no other sprint changes) and A-4's 95%/50% AC waits for it (A-4 not Done until then; the Sprint 4 exit then excludes that check; Goals 2 and 3 evidence one sprint later; G4a unaffected). This is flagged to the owner at the time; nothing moves into Sprint 3, which stays at 8.
Note: pre-M3 builds are not registered in a real MCP client (release rule); Claude Code check uses a throwaway config on the author's machine only against public URLs.

### Sprint 3: Harness and idle RSS (8 pts)
**Goal:** A one-command harness on native ARM that reports idle RSS against the 10 MiB target.
Stories: E-2 (5), E-3 (2), A-9 (1, moved from Sprint 2; first to drop if E-2 overruns).
Dependencies: E-1, A-3b, E-8, A-2. Hosted arm64 runner required (native preflight refuses QEMU, also inside the container); no runner provisioning (ADR-007), workflow per architecture 9.2 as part of E-2; interim gnu+musl build script with pinned `cargo-zigbuild`/`ziglang` versions (D-2 adopts the same pins).
Exit: fixtures with committed sha256 manifest; G1-G7 scenarios defined; harness targets the bench build for peak and the shipped binary for idle; hosted arm64 nightly/dispatch/PR benchmark workflow against the image; idle RSS median of 10 recorded; harness exits non-zero when over target.
Gate: idle RSS <= 10 MiB (strict) on the shipped release binary (also recorded on the bench build), native aarch64 only; QEMU or non-native figures never count. Miss triggers an investigation item before Sprint 4 starts.

### Sprint 4: Convert and memory gate (8 pts)
**Goal:** HTML converts to markdown within budget and peak memory is proven bounded.
Stories: A-4 (5), E-4 (3).
Dependencies: A-3b, E-2, E-8, E-7. Entry: E-7 merged (else the Sprint 2 fallback applies: the 95%/50% check is removed from the Sprint 4 exit, done in Sprint 5, and this is flagged); hosted arm64 runner availability checked. A-4 lands first in the sprint (E-4 is first priority if A-4 slips); E-4 needs it (`lol_html` limits, architecture rows f/g).
Exit: A-4 AC (on the E-7 snapshot set, unless the fallback applies: >= 95% conversion success and median token reduction >= 50%, no `<script>` text; 1 MiB in <= 500 ms p95 on aarch64, converter behind a trait); E-4 fixtures: 5 MiB peak <= 40 MiB; 50 MiB with Content-Length -> `too_large` within 10% of 5 MiB peak; 50 MiB chunked, full read without a window, beyond the cap -> `too_large` within 10% of 5 MiB peak; 10 concurrent recorded; allocator recorded. Windowed variants (chunked in-cap window, window beyond cap) need A-5 and are G4b, not Sprint 4.
**Gate G4a (memory gate part 1, end of Sprint 4):** idle RSS <= 10 MiB (shipped binary) and 5 MiB peak (VmHWM, max of per-scenario medians) <= 40 MiB (bench-loopback build, confirmed) on native aarch64, each the median of 10 valid runs (harness rejects fewer), for both gnu and musl, on the release profile pinned in D-7 (MiB as defined once in E-1). Scenarios are only those that need A-3b and A-4 and no window or early stop: (1) the 5 MiB HTML page fully read and converted, (2) the same page gzipped, (3) late-landmark holdback-full HTML, (4) 50 MiB with Content-Length -> `too_large`, (5) 50 MiB chunked read beyond the cap without a window -> `too_large`; (4) and (5) within 10% of the 5 MiB peak. The 10-concurrent scenario is recorded, not gating. The shipped-vs-bench record is part of the pass, with numeric bounds (E-8): idle delta <= 0.5 MiB, binary size delta recorded and explained, one public-host 5 MiB fetch on the shipped binary within 10% of the bench peak and <= 40 MiB; both binaries report the same commit and Cargo.lock hash. QEMU or non-native figures never count. Pass: continue to Sprint 5. Fail: stop feature work and run a memory-reduction sprint (allocator, buffer sizes, converter swap via trait) before Sprint 5. **A G4a pass does not close the memory gate.**
**G4a result (E-4, Sprint 4 part 2, recorded in docs/BENCHMARK.md section 16):** PASS in CI run 35557702662 on all four hosted cells (arm64 gnu 5.38 / musl 4.56 MiB gating peak; amd64 gnu 6.16 / musl 4.70), single run, with the caveats stated there. Not the memory gate closing.
**Gate G4b (memory gate part 2, end of Sprint 5; owning stories A-5 and A-6, A-6 records the combined result; not part of E-4's Done):** the scenarios that need A-5 (window at start with early stop, window at end, chunked window inside the cap succeeding, window beyond the cap -> `too_large`) and A-6 (`raw=true`) meet the same 40 MiB peak (idle re-checked at 10 MiB on the shipped binary) under the same rules, and the two 50 MiB chunked window cases stay within 10% of the 5 MiB peak. Fail: stop before Sprint 6. The 10 MiB idle and 40 MiB peak targets are not lowered. **The complete memory gate is therefore closed at the end of Sprint 5.** Requires the hosted arm64 runner; if it is unavailable G4a cannot be evaluated and Sprint 5 does not start, and G4b likewise blocks Sprint 6.

## Sprints 5-12 summary (12 for v1.0)

### Sprint 5: Paginate, content types and memory gate G4b (6 pts, thin block)
**Goal:** Pagination and content types work and the memory gate closes. Stories: A-5 (3), A-6 (3). Dependencies: A-4, E-2, E-4, E-8; hosted arm64 runner. Entry: G4a passed (or its remediation sprint done); runner availability checked. Exit: A-5 and A-6 AC pass; G4b run and recorded (A-5 window scenarios, A-6 raw and combined write-up); idle re-checked. The block is 6 of 8 pts on purpose: 2 pts of slack absorb G4b work or the E-7 overflow (Sprint 2 fallback: 6 + 2 = 8).

### Sprints 6-12

| Sprint | Goal | Stories | Pts | Entry | Exit |
|---|---|---|---|---|---|
| 6 | Clear errors, private-IP test depth | A-7, B-1 | 8 | G4b passed | Every error cause has a flag+message test; B-1 encodings and mixed-answer cases pass on A-3a code |
| 7 | Redirect limit, encoded forms, report | B-3, B-2, E-5 | 8 | E-3, E-4, A-7 done; runner checked | Redirect limit and rebinding tests pass; E-5 report published on the D-7-pinned profile with figures labelled by binary, and states the D-1 re-measure is still required |
| 8 | SSRF suite, coverage gate, release profile | B-5, D-1, D-5 | 6 | B-1, B-2, B-3 done; runner available (D-1 re-measure) | B-5 SSRF suite 100% and 90% coverage gate (M3 reached); D-1 profile finalised and idle and peak **re-measured on the shipped build on aarch64 (gnu and musl) within 10 MiB and 40 MiB, or MVP tagging is blocked** |
| 9 | Multi-arch image release pipeline (D-4 moved to Sprint 10, ADR-007 re-estimate) | D-2 | 8 | M3 reached; OQ-7 decided (before the first image is published); D-1 re-measure passed; runner availability checked | D-2 per-platform images built, manifest list merged, both platforms tested natively by digest (handshake, FR-15, D-7 guard on the extracted binary, ARM memory gates on that digest) and published to GHCR only if BOTH platform gates pass; D-7 guard run (cargo tree and marker grep for `test-support` and `bench-loopback`); D-4 install guide lands in Sprint 10 and MVP completes in Sprint 10 (76 pts), not here; SUPERSEDED (history, ADR-007 removed the manual fallback): "manual bench run on the D-2 artifact plus E-5 stand in for the CI gate" |
| 10 | Config and allowlist, plus install docs (MVP complete) | C-1, C-2, C-3, D-4 | 9 (7 if C-2 dropped; one over the 8-pt ceiling, ACCEPTED by the user 2026-09-19; C-3 not deferred) | OQ-3 default decided (C-1 needs it), OQ-4 decided for C-2 (drop = 5 pts) | Env parsing and validation, allowlist (if OQ-4 yes), timeout and size config; D-4 config text re-checked |
| 11 | robots, charset, ARM tests | B-4, A-8, D-3 | 8 | OQ-3 decided; runner checked | robots enforced per OQ-3; charset decoding; aarch64 test job required check |
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 | OQ-5 decided; hosted arm64 runner available | Labelling per OQ-5; memory-gate CI required; v1.0 tagged by D-6 |

MVP = 76 pts after ADR-007 (was 73 at end of Sprint 9; now end of Sprint 10 because D-2 is 8 pts and D-4 moves), SUPERSEDED HISTORY, kept for the record only (original text, no longer current): "MVP = 73 pts, end of Sprint 9 (sprints 0-9 sum to 75, less non-MVP A-9 and D-5)". Tagging/distribution only after M3 (Sprint 8), the E-5 release decision and the D-1 shipped-build re-measure (Sprint 8) passing; the MVP release evidence is the release workflow's own ARM gate run on the published digest (the manual bench run fallback is removed, ADR-007); D-3 and E-6 add PR-level required checks in Sprints 11-12; D-6 is the v1.0 tagger.

Milestones M0-M5 map to sprints in PRD section 9: M0 S0, M1 S1-2, M2 S3-6 (G4a at end of S4; memory gate complete at end of S5 with G4b), M3 S6-8 (reached end of S8), M4 S9-11, M5 S11-12.

## Definition of Done (per story)

- All acceptance criteria have a passing test that asserts them.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` pass on the hosted CI from D-7 (required checks from Sprint 0).
- No new direct dependency without recording it against the NFR-05 count; high-churn deps use exact `=` pins (architecture 8).
- stdout carries only MCP messages; logs to stderr.
- Stories touching fetch, decode, convert or concurrency (A-4, A-5, A-6, C-3, E-2 to E-6): benchmark run on native aarch64 with results recorded (manual on PRs); no regression against the absolute targets. A-3b: non-gating manual RSS smoke only (harness arrives in Sprint 3). Spikes E-1 and A-1, E-7 and E-8 are exempt. All benchmark figures use the release profile pinned in D-7, MiB as defined once in E-1, and the median of 10 valid runs.
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
| RESOLVED/REPLACED by ADR-007: ARM runner access (was the author's aarch64 cluster) becomes hosted-runner availability and quota risk | Blocks (if GitHub arm64 runners are unavailable or quota-limited) G0 handshake, Sprint 1 readiness, Sprint 2 smoke, Sprint 3-5 measurements, G4a/G4b gates, E-5, C-3, A-8, E-6, D-3; a runner outage in Sprint 12 leaves no slack | Record OS/RAM/CPU/page size from the first hosted run before Sprint 0 exit; unavailable runner means G4a/G4b not evaluated and no release; QEMU never used for RSS; a re-run via `workflow_dispatch` is the only fallback (Michael). Additional risks: hosted VMs vary in CPU/host between runs (memory counters robust, latency not gated), 4 KiB hosted page size vs the Pi 5 default of 16 KiB, GHCR packages private by default |
| OQ-5 (labelling) due before Sprint 2 | Result envelope rework in A-3b/A-4 | Decide before Sprint 2 (Michael); a "label, prefix not shifting `start_index`" working assumption is NOT a decision |
| OQ-3, OQ-4, OQ-7 open | C-1 default, B-4, C-2, D-2/D-4/D-6 blocked at their sprints | Decide OQ-3 before Sprint 10 (C-1), OQ-4 before Sprint 10, OQ-7 before Sprint 9 |
| Publishing a public image (GHCR) is a distribution act (ADR-007) | Publishing before OQ-7 is answered would decide licence/distribution by default | OQ-7 due before Sprint 9 and before the first image publish; candidates stay private and untagged until then; the publish job is gated on the ARM gates for the same digest (Michael) |
| A-3b estimate (5 pts, high scope) | Sprint 2 overrun | Sprint 2 is committed at 8 (ceiling); if A-3b or E-8 overruns, E-7 slips to Sprint 5 and A-4's 95%/50% check waits for it (A-9 is in Sprint 3 and unaffected) |
| Bench build diverges from shipped (E-8 confirmed) | Peak RSS not measured on the shipped binary; report could mislead | Numeric bounds (idle delta <= 0.5 MiB, public-host peak within 10% of bench and <= 40 MiB), same commit and Cargo.lock hash, marker checks; report labels figures by binary; OQ-4 untouched |
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
| QA NB4 | G0 states median of 10 valid runs and the E-1 MiB definition; A-1 re-run under the protocol or deviation recorded. |
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
| 2026-09-19 | Accept the schedule overage from the ADR-007 multi-arch re-estimate: D-2 8 pts, total 100 pts, MVP 76 pts finishing at the end of Sprint 10 (not 9), Sprint 10 at 9 pts (one over the 8-pt ceiling), C-3 NOT deferred, v1.0 stays Sprint 12 | User | Sprint 9 = D-2 (8); Sprint 10 = C-1, C-2, C-3, D-4 (9); plan counts updated |
| 2026-09-19 | Use MiB (2^20 bytes) as the unit everywhere: 10 MiB idle VmRSS, 40 MiB peak VmHWM, 5 MiB body | User | Targets and wording unified; numeric targets unchanged |
| 2026-09-19 | Sprint 0 verdict recorded as GO (upgraded from CONDITIONAL GO) on native aarch64 evidence (hosted-ARM A-1 spike runs, advisory, spike not the product); UAT checkpoint passed ("record the GO") | User | Gate G0 closed as GO; conditions carried forward (UAT report sec 5); Sprint 1 (A-2 + A-3a) starts on a new branch after PR #2 merges |
| 2026-09-19 | (1) Native memory measurement runs on GitHub-hosted runners for BOTH amd64 and arm64 (public repo), no self-hosted runner; (2) release artifact is a tested multi-arch GHCR image published only from GitHub Actions after both platform gates pass on the exact digests; standalone binary releases dropped; Windows containers, arm/v7, 386, riscv64, ppc64le, s390x deferred; unverified platforms only with the "memory targets not verified on this platform" label (ADR-007) | User | OQ-9 self-hosted trust items resolved by removal; G0 evidence closable by hosted A-1 runs; D-2 5 to 8 pts (unvalidated); total 100, MVP 76, MVP at end of Sprint 10; D-3 flagged for downward re-estimate; OQ-7 must precede first publish; targets unchanged |

## Round 3 / escalation resolution (2026-09-19)

| Finding | Resolution |
|---|---|
| PO B1 (G4a needs A-5 window) | User decision: scenario lists fixed, not reinterpreted. G4a keeps only full-consumption scenarios needing A-3b and A-4 (5 MiB page, gzipped, late-landmark holdback-full, 50 MiB Content-Length, 50 MiB chunked full read beyond cap). Window at start (early stop), window at end, chunked in-cap window, window beyond cap and `raw=true` move to G4b, end of Sprint 5. E-4 ACs, A-3b chunked AC, Sprint 4 exit, PRD M2 and decision rule, EPICS decision gates and this plan made consistent. Complete gate is at the end of Sprint 5; stated as such. |
| QA N4 | G4b owned by A-5 (window scenarios, runs) and A-6 (raw and combined record), ACs added; harness maintenance for those scenarios in scope; not part of E-4's Done. |
| QA B1, PO N1, DevOps N5 | User decision: release profile pinned in D-7 (Sprint 0) so every gate measures it; D-1 (Sprint 8) finalises and must re-measure idle and peak on the shipped build and the aarch64 runner (gnu and musl), blocking MVP tagging on a miss; E-5 (Sprint 7) reports on the pinned profile and says the D-1 re-measure governs the tag. No points change. |
| PO N3, QA SM 3 | E-8 recorded as CONFIRMED in plan, EPICS and PRD, and in the decisions log and EPICS Open Items; not a decision on OQ-4. |
| PO N4, DevOps N1, N2 | D-2 guard asserts `test-support`, `bench-loopback` and the fixture-CA feature, and runs cargo tree plus a marker grep on every release artifact; D-7 guard has a self-test (positive control), `-p <crate>` release build, and E-8 tests run with the feature in hosted CI. DevOps N3: shipped and bench binaries report the same commit and Cargo.lock hash (E-2, E-8). DevOps N6: `publish = false` and license handling and clippy on A-1 in D-7. DevOps N4 (musl and macOS as shipped artifacts) remains for D-2 detail before Sprint 9. |
| QA N2 | Numeric bounds: idle delta <= 0.5 MiB (kept as told), public-host peak within 10% of bench and <= 40 MiB, size delta explained; exceeding fails G4a. |
| QA N3 | A-3b merge gate is machine-checked: non-defaulted `Policy` in the client constructor and a required CI check running `a3b_merge_gate` plus the four-refusal tests. |
| QA N1 | E-7 fallback: Sprint 4 exit and A-4 AC state the 95%/50% check is excluded when E-7 slips, A-4 not Done until met in Sprint 5. |
| QA N5 | One MiB definition in E-1 (10^6 or 2^20, stated once) covering targets, fixtures and the 5 MiB cap; median of 10 valid runs everywhere; the harness rejects fewer than 10. |
| PO N2 | OQ-3 due before Sprint 10 starts (was 11) so C-1's default is not a hidden decision; C-1 uses a placeholder; OQ-3 stays OPEN. |
| SM 1, 2 | Sprint 5 has its own block and is flagged as a thin 6-pt sprint (2 pts slack); entry and exit criteria added for Sprints 6-12. Sprint 3 zero slack noted (A-9 drops first). |

Told numbers changed: none in points or counts (97 pts, 34 stories, MVP 73 pts in Sprint 9, v1.0 Sprint 12, every sprint <= 8). Changed dates: OQ-3 due before Sprint 10 (was 11). Restated plainly: complete memory gate at end of Sprint 5, G4a at end of Sprint 4 covers fewer scenarios than round 2 listed. No point change was unavoidable, so none is flagged. OQ-3, OQ-4, OQ-5 and OQ-7 remain OPEN and undecided.


## Revision 7 (2026-09-19): hosted ARM runners and GHCR image release (ADR-007)

| Change | Detail |
|---|---|
| Runner | Self-hosted Pi 5 runner and OQ-9 trust items removed; hosted `ubuntu-24.04-arm` is the measurement environment; a cloud VM, not the cluster. |
| Release artifact and platforms | Standalone binaries dropped; D-2 is multi-arch (amd64 + arm64) image build, per-platform digest-gated native tests and GHCR publish; both platform memory gates hard; D-4 install guide uses `docker run -i`. |
| Points and counts | CHANGED: 97 to 100 total, MVP 73 to 76, 34 stories; D-2 5 to 8 pts (unvalidated); D-4 moves to Sprint 10; MVP end of Sprint 10; v1.0 still Sprint 12; Sprint 10 at 9 pts if C-2 stays (1 over ceiling, PO/user decision). D-3 probably shrinks (ARM CI job exists from D-7 work), not re-estimated here. |
| Still OPEN | OQ-3, OQ-4, OQ-5, OQ-7 (OQ-7 now also gates the first image publish). |
| G0 evidence | A-1 spike re-run natively on both hosted runners under the E-1 protocol; the earlier Intel/glibc x86 figure is preliminary. |

## Revision 8 (2026-09-19): user acceptance of overage, MiB unit, Sprint 0 GO

| Change | Detail |
|---|---|
| Schedule | User accepted the ADR-007 re-estimate: D-2 8 pts, total 100, MVP 76 at end of Sprint 10, Sprint 10 at 9 pts (one over the ceiling), C-3 not deferred, v1.0 Sprint 12. The "needs PO/user acceptance" flag is closed. |
| Unit | "MB" replaced by "MiB" throughout this plan (10 MiB idle, 40 MiB peak, 5 MiB body). Historical log rows were converted mechanically; numeric values unchanged. |
| Gate G0 | Closed as GO on hosted native aarch64 spike evidence; conditions carried forward in the UAT report (G4a/G4b product gates on BOTH linux/amd64 and linux/arm64; amd64 baseline still from the original x86_64 spike; Pi 5 16K-page pass optional and not done). |
| Still OPEN | OQ-3 (before Sprint 10), OQ-4 (before Sprint 10), OQ-5 (before Sprint 2), OQ-7 (before Sprint 9 and before the first published image). |

## Revision 9 (2026-09-20): Sprint 1 DONE, Sprint 2 started

Sprint 1 (A-2 + A-3a, 8 pts) is DONE. PR #5 merged to main as 090e86c; hosted CI 8/8 green (as reported by the coordinator at merge). Exit criteria and evidence:

| Exit criterion | Evidence |
|---|---|
| `tools/list` shows exactly `fetch` | `tests/stdio.rs` integration test spawns the real binary: one tool `fetch`, four schema properties, `required=["url"]` (A-2 dev-report) |
| Stdout-purity test passes | A-2 stdout-purity test; `#![deny(clippy::print_stdout)]` in `src/main.rs` proven by probe (cleanup-report item 3) |
| Ready within 250 ms on ARM | ready_ms median 1.058 ms (0.977 to 1.067) on hosted aarch64 (Azure, 4 KiB pages), A-3a dev-report; A-2 x86_64 median 0.99 ms |
| A-3a tests pass for every range | table-driven tests per range incl. RFC 9780 `100:0:0:1::/64` added in cleanup (cleanup-report item 1) |
| Default policy fail-closed | default `Policy` fail-closed; test constructor behind `test-support` |
| Release guard required and passes | D-7 guard (no `test-support` via `cargo tree -e features` and symbol grep) is a required CI check; clippy also runs `--features test-support` |
| Gate: no HTTP client crate | none in the dependency tree at end of Sprint 1 |

Stage summary: `.delivery/artifacts/06-dev/sprint-1/stage-summary.md`.

**Sprint 2 (A-3b 5, E-7 2, E-8 1 = 8 pts) started 2026-09-20** on branch `sprint-2/guarded-fetch`. Entry blockers: OQ-5 decision (plan entry criterion; owner: Michael), the 50-URL list and 10-URL smoke list (E-7 prerequisite; owner: project owner). E-8 has no blocker. No implementation has begun.

## Revision 10 (2026-09-20): Sprint 2 closed, Sprint 3 started

Sprint 2: A-3b (5) and E-8 (1) DONE (PR #6, merge 35a3450, CI 10/10; architect DONE, PR review APPROVE round 2, QA and tech-writer DONE round 3). **E-7 (2) NOT DONE**: the owner's 50-URL and 10-URL lists have not arrived, so the Sprint 2 exit criterion "E-7 snapshot set and manifest committed" is not met. Overflow rule applied: E-7 moves to Sprint 5 (6 + 2 = 8 pts) unless the lists arrive first; A-4's 95%/50% AC waits for E-7; Sprint 4 exit excludes that check; G4a unaffected; Sprint 3 unchanged at 8. OQ-5 resolved: no label. Details: `.delivery/artifacts/06-dev/sprint-2/stage-summary.md`. **Sprint 3 (E-2 5, E-3 2, A-9 1) started** on branch `sprint-3/harness-idle-rss`; no implementation begun.


## Revision 11 (2026-09-20): Sprint 3 fix-pass 1 (after DoD reviews and PR review of PR #7)

Recorded re-homing of E-2 items that Sprint 3 did not deliver, **pending owner acknowledgement (the owner has not acknowledged these)**:
- Container-image measurement and the tag trigger with the digest gate: E-2 acceptance text says the harness runs against the image; Sprint 3 measured bare binaries on both native hosted platforms. Re-homed to D-2 (Sprint 9).
- `g6-concurrent10` and the G4b scenario implementations: defined but not implemented in the harness. Re-homed to E-4, and to A-5 and A-6 for the G4b runs.
- macOS `/usr/bin/time -l` reader: not built, no macOS gate host; known deviation from the E-2 acceptance text.
- Architect NB1 (no hash pin for `ziglang`, cargo-zigbuild not lock-checked): recorded in the D-2 acceptance criteria.
A-9 is done (Sprint 3). Sprint 3 gate runs in CI passed on all four cells in the read-in-full form (not a G4a pass).

## Revision 12 (2026-09-20): Sprint 3 closed, Sprint 4 started

Sprint 3: A-9 (1), E-3 (2) DONE; E-2 (5) DONE CONDITIONAL (four AC items re-homed per Revision 11, PENDING OWNER ACKNOWLEDGEMENT, not acknowledged). PR #7 merge c13159e, CI 14/14 at head 37b2823; architect DONE, QA round 2 DONE, PR review round 2 APPROVE. Idle/peak MiB (run 35541676701): amd64 gnu 3.90/5.17, amd64 musl 2.27/4.25, arm64 gnu 3.55/4.61, arm64 musl 2.16/4.20; read-in-full peak, NOT a G4a pass, bare binaries not image, single run. Details: `.delivery/artifacts/06-dev/sprint-3/stage-summary.md`.

**Sprint 4 (A-4 5, E-4 3 = 8 pts) started 2026-09-20** on branch `sprint-4/convert-memory-gate`. First commit is Sprint 3 carry-forward cleanup (listed in the Sprint 3 stage summary). E-7 lists were NOT supplied before Sprint 4 entry: E-7 moves to Sprint 5 and the A-4 95%/50% check is excluded from the Sprint 4 exit (fallback applies; A-4 not Done until met in Sprint 5). E-4 also implements `g6-concurrent10` and the G4b-scenario stubs re-homed from E-2. Open owner items: E-7 lists, branch protection, E-2 re-homing acknowledgement, A-2 throwaway check, cargo-audit.

## Revision 13 (2026-09-21): Sprint 4 closed, Sprint 5 started

Sprint 4: E-4 (3) DONE; G4a PASS in all four hosted native cells (docs/BENCHMARK.md section 16; runs 35562347553, 35563535561, 35633732179); this is not the memory gate closing. A-4 (5) implemented, NOT DONE: the 95%/50%/no-`<script>` checks need E-7, which needs the owner's 50-URL and 10-URL lists (not supplied). PR #8 merge c86f445, CI 14/14; architect DONE, QA DONE, PR review APPROVE (round 2). Details: `.delivery/artifacts/06-dev/sprint-4/stage-summary.md`.

**Sprint 5 (A-5 3, A-6 3, E-7 2 = 8 pts) started 2026-09-21** on branch `sprint-5/paginate-content-types-g4b`. With E-7 in, the sprint is at the 8-pt ceiling and the 2 pts of slack noted in the Sprint 5 block are used. G4b (owned by A-5 and A-6, A-6 records the combined result) closes the memory gate at the end of Sprint 5. E-7 cannot start until the owner supplies the lists; A-5 and A-6 do not depend on them.

Overflow if the E-7 lists are still missing at Sprint 5 exit: the plan defines no further fallback. Consequences: E-7 stays undone; A-4 stays not Done (its 95%/50% AC unmet); Sprint 6 is already 8 pts, Sprint 7 is 8, so neither can absorb 2 pts; Sprint 8 (6 pts) and Sprint 12 (7 pts) are the only sprints with room; E-5 (Sprint 7) needs the 10-URL smoke result and Goals 2 and 3 have no evidence. This needs an owner decision at Sprint 5 exit (see stage summary); no change is made here.

Open owner items (unchanged): E-7 lists; OQ-7 (now with the MPL-2.0 fact recorded in the Sprint 4 stage summary); acknowledgement of re-homed E-2 items; branch protection (`a3b-merge-gate` required, `bench-gate` optional); A-2 throwaway-config check; cargo-audit; OQ-3 and OQ-4 before Sprint 10.

## Revision 14 (2026-09-21): Sprint 5 A-5 and A-6 implemented, G4b PASS (PR #9, draft)

A-5 and A-6 implemented; E-7 is NOT in the PR (owner URL lists still missing), so A-4 stays not Done. G4b PASS on all four hosted cells (run 35641694726; docs/BENCHMARK.md section 17): gating peak 4.38 to 6.06 MiB (target 40), shipped idle 2.36 to 4.62 MiB (target 10), 50 MiB chunked ratios at most 1.048 (bound 1.10). By the plan this closes the memory gate; Sprint 6 may start. Not covered: E-7-dependent A-4 checks.

AC map: A-5 (window, clamp, continuation, beyond-end, char boundaries, chunked in/beyond cap, G4b window scenarios) - src/convert/window.rs, src/server.rs, src/fetch/tests.rs, tests/stdio.rs, bench/. A-6 (raw, content types, sniffing, `raw=true` scenario, idle re-check, combined G4b record) - src/convert/mod.rs, src/fetch/mod.rs, tests/stdio.rs, BENCHMARK s17. E-4 G4b scenarios implemented (bench/scenarios.py, measure.py).

Decisions needing owner acknowledgement: (1) Total-length footer only on continuation/beyond-end (deviates from ADR-006 item 4); (2) G5 too_large via the 50 MiB chunked variant; (3) G1 duplicates g4a-5mib-full; (4) `text/*` extras (javascript, ndjson, +json/+xml) chosen by the developer; (5) G4a read-in-full scenarios redefined as window-at-end. Also pending with the owner: the two red arm-bench `bench (gnu)`/`(musl)` jobs (Sprint 0 spike lacks start_index/max_length; advisory, not required; fix in .github/workflows/arm-bench.yml not made).

