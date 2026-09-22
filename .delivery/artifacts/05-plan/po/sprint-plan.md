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
**G4a result (E-4, Sprint 4 part 2, recorded in docs/BENCHMARK.md section 16):** PASS in CI run 35557702662 on all four hosted cells (arm64 gnu 5.38 / musl 4.56 MiB gating peak; amd64 gnu 6.16 / musl 4.70), the first run of the gate, re-run as 35562347553, 35563535561 and 35633732179 (all PASS), with the caveats stated there. Not the memory gate closing.
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
| 12 | Labelling, CI gate, v1.0 | B-6, E-6, D-6 | 7 | OQ-5 decided; hosted arm64 runner available | Labelling per OQ-5; memory-gate CI required (NOT MET — see Revision 22/23: a CI regression-tripwire job was added but branch protection was not enabled, no repo-settings access); v1.0 tagged by D-6 (NOT MET — see Revision 22/23: tag mechanism exists but was deliberately not triggered, pending OQ-7) |

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

Decisions needing owner acknowledgement: (1) Total-length footer only on continuation/beyond-end (deviates from ADR-006 item 4); (2) G5 too_large via the 50 MiB chunked variant; (3) G1 duplicates g4a-5mib-full; (4) `text/*` extras (javascript, ndjson, +json/+xml) chosen by the developer; (5) G4a read-in-full scenarios redefined as window-at-end. Resolved: the two arm-bench `bench (gnu)`/`(musl)` jobs (Sprint 0 spike lacks start_index/max_length) were red at first; by owner decision ("drop spike peak, keep idle") they now measure idle only (commit cd99c95). All 14 checks were green at cd99c95. Sprint 5 fix pass (tech-writer findings) documented in the dev report.

## Revision 15 (2026-09-21): Sprint 5 done and merged, Sprint 6 started

Sprint 5: A-5 (3) and A-6 (3) DONE; E-7 (2) NOT DONE (owner lists missing). PR #9 merged as b633e60 after four DoD reviews and a round-2 approval. G4b PASS on all four hosted cells (run 35641694726, BENCHMARK s17); the memory gate is closed. A-4 stays not Done. Deviations pending owner acknowledgement and deferred follow-ups: `.delivery/artifacts/06-dev/sprint-5/stage-summary.md`. Stale reference fixed: the Sprint 4 G4a result line named only two re-runs; it now lists all three (35562347553, 35563535561, 35633732179) after the first run 35557702662 (which remains the run behind the quoted figures).

**Sprint 6 (A-7 3, B-1 5 = 8 pts) started 2026-09-21** on branch `sprint-6/a7-error-taxonomy`. The Sprint 5 record is the first commit. E-7 is still unsupplied: Sprint 7's E-5 needs the 10-URL smoke result and A-4's 95%/50% AC needs the 50-URL set, so neither can close until the owner supplies the lists; no fallback is defined for E-5.

## Revision 16 (2026-09-21): Sprint 6 done and merged, Sprint 7 started

Sprint 6: A-7 (3) and B-1 (5) DONE (8 pts). PR #10 merged as f4f2c1b after QA/docs, code review and a round-2 approval. E-7 (2) is still NOT DONE: the owner has not supplied the 50-URL and 10-URL lists, so A-4 stays not Done.

**Sprint 7 (B-3 3, B-2 3, E-5 2 = 8 pts) started 2026-09-21** on branch `sprint-7/redirects-encodings-report`. B-3 and B-2 are implemented (tests). E-5 is split: the harness (bench/smoke.py, bench/e5_report.py, self-tested against local fixtures) is done, but E-5 stays OPEN and cannot close without the owner's 10-URL smoke list (its AC needs the live smoke result and a "release" decision). The plan still has no fallback for this: if the list is not supplied, E-5 does not close, Goals 2 and 3 have no live evidence, and no release decision can be published. No points change; Room for the 2 pts: Sprint 8 has 2 pts of room and is the only sprint that fits E-5 whole; Sprints 9 and 10 have 1 pt each; Sprint 12 is not claimed.

## Revision 17 (2026-09-21): Sprint 7 done and merged, Sprint 8 started

Sprint 7: B-3 (3) and B-2 (3) DONE (6 of 8 pts). PR #11 merged as `0926ba8` after QA/docs, code review and a round-2 approval. **E-5 (2) NOT DONE**: the owner's 10-URL smoke list has still not been supplied, so E-5 stays open exactly as recorded when Sprint 7 started; no fallback exists in the plan.

**Sprint 8 (B-5 3, D-1 2, D-5 1 = 6 pts) started 2026-09-21** on branch `sprint-8/ssrf-suite-release-profile`, matching the Sprints 6-12 table row 8 exactly ("SSRF suite, coverage gate, release profile"). Entry satisfied: B-1, B-2, B-3 done. E-5 (2 pts) is **not** listed in that table row, so it is not scheduled into Sprint 8 and is not completed here; the Sprint 7 harness (`bench/smoke.py`, `bench/e5_report.py`) is left ready to run once the owner's list arrives. Note: this narrative log's Revision 16 entry called Sprint 8 "the only sprint that fits E-5 whole" by points, but the formal Sprints 6-12 table row for Sprint 8 was never edited to add E-5 — that inconsistency is flagged here, not resolved unilaterally; E-5 currently has no sprint in the table scheduled to close it. B-5's SSRF/redirect/rebinding/DNS-to-private/encoded-form table-driven cases were already in place from A-3a/A-3b/B-1/B-2/B-3; Sprint 8 adds the required >=90% coverage gate (measured 97.7% combined locally) and fills no further test gaps. D-1 finalises the D-7-pinned release profile unchanged (already passing G4a/G4b with margin) and adds the re-measure plus an `ldd` no-OpenSSL CI guard. D-5's tool description was already 59 words with every parameter and the continuation pattern named; only a regression test was added. Details: `.delivery/artifacts/06-dev/sprint-8/dev-report.md`.

## Revision 18 (2026-09-21): Sprint 8 closed and merged, Sprint 9 started

Sprint 8: B-5 (3), D-1 (2), D-5 (1) DONE (6 of 6 pts, matching the Sprints 6-12 table row 8). PR #12 merged to `main` as `e15872e` after code review (fix pass `5465b92`), the D-1 bench-gate re-measure (`eda0408`), and a round-2 approval. E-5 (2) remains NOT DONE, unchanged from Sprint 7 (owner's 10-URL smoke list still not supplied, no plan fallback exists).

**Sprint 9 (D-2, 8 pts) STARTED 2026-09-21** on branch `sprint-9/multi-arch-release-pipeline`, matching the Sprints 6-12 table row 9 exactly. Entry criteria checked: M3 reached (end of Sprint 8, B-5/D-1/D-5 done); D-1 re-measure passed (Sprint 8 dev report); hosted-runner availability assumed per ADR-007 (both `ubuntu-24.04` and `ubuntu-24.04-arm` used by `bench.yml`/`arm-bench.yml` already). **OQ-7 (image distribution/licence) is STILL OPEN** and is not decided in this sprint; per the D-2 AC, only private, untagged candidates may exist in GHCR and the package is never made public until OQ-7 is answered.

D-2 delivered this sprint: a multi-stage, multi-platform `Dockerfile` (non-root, base images pinned by digest, `shipped`/`bench` targets) and a new `.github/workflows/release.yml` (native per-platform build on `ubuntu-24.04`/`ubuntu-24.04-arm`, push-by-digest private candidates to GHCR, a manifest-list merge job, native per-platform test jobs reusing the existing D-7 guard and `bench/measure.py` harness, and a `publish` job implemented but hard-gated `if: false && ...` so it cannot run this sprint under any trigger). `deny.toml` reviewed, unchanged (already had the required sections from D-7). D-3 (separate story) untouched; the existing D-7 ARM CI job is noted as already present.

Deviations recorded (owner/architect acceptance needed): (1) `cargo-zigbuild`/`ziglang` NOT adopted — D-2 as rewritten by ADR-007/Revision 7 builds natively per platform, so the EPICS.md D-2 bullet inherited from the earlier cross-build approach does not apply; (2) the merge job's manifest list is pushed to a private `candidate-<sha>` reference rather than a bare digest (no tag at all), because the stock `docker buildx imagetools` tooling needs a reference to push a manifest list to; every downstream consumer uses only the resulting digest, and the reference is never a version tag or `latest`. Full details, local-gate results and the exact CI evidence gathered: `.delivery/artifacts/06-dev/sprint-9/dev-report.md`.

## Revision 19 (2026-09-22): Sprint 9 merged (c902b3e), Sprint 10 started

Sprint 9 CLOSED: PR #13 merged to `main` as `c902b3e` (merge of `sprint-9/multi-arch-release-pipeline`, prior commit `dfa63df` fixed release.yml blocking review findings from `8dd717b`). D-2 (8 pts) matches the Sprints 6-12 table row 9. OQ-7 (image distribution/licence) is still OPEN and unchanged: only private, untagged GHCR candidates exist; the `publish` job stays hard-gated `if: false` and is not enabled by this merge.

**Sprint 10 (C-1, C-2, C-3, D-4 = 9 pts) STARTED 2026-09-22** on branch `sprint-10/config-allowlist-install-docs`, matching the Sprints 6-12 table row 10 (C-2 included per the 2026-09-19 user acceptance; not deferred). **Entry criteria per the table row are NOT fully met**: the row lists "OQ-3 default decided (C-1 needs it), OQ-4 decided for C-2" as entry criteria, and **both remain OPEN and undecided** at Sprint 10 start (see the open-questions row at the top of this file and Revision 18's carried owner items). Sprint 10 proceeds anyway, by explicit instruction, on the following basis, which keeps both open questions genuinely open rather than deciding them by default:

- **C-1** parses and validates `FETCH_ROBOTS_TXT` (`ignore`/`enforce`, default `ignore`) but implements it as an inert placeholder only: neither value fetches or enforces `robots.txt` today (unchanged behavior). This does **not** answer OQ-3; B-4 (Sprint 11) still needs a decided default before it can implement enforcement.
- **C-2** wires `ssrf::Policy`'s allowlist mechanism through `FETCH_ALLOW_PRIVATE_HOSTS`, defaulting to an empty (inert) list, which is identical to today's fail-closed behavior for anyone who does not set it. This does **not** answer OQ-4 either: whether the mechanism should ever be enabled, and under what governance, is left to the product owner.
- C-3 (`max_length_cap` hard cap, already substantially implemented in `src/server.rs` from a prior sprint) was reviewed, confirmed correctly wired end to end (per-call `max_length` clamped to `FETCH_MAX_LENGTH_CAP`, footer on clamp), and given additional config-level tests; no behavior change was needed.
- D-4 install/config docs (README "Configuration" section) list every `FETCH_*` variable, its default and its validation-error behavior, and explicitly flag OQ-3 and OQ-4 as still-open product decisions with the mechanism described as "available but unendorsed" pending those decisions.

Full implementation, test evidence and CI status: `.delivery/artifacts/06-dev/sprint-10/dev-report.md`. **Owner decisions still needed, carried forward unchanged in substance: OQ-3 (robots.txt default policy) and OQ-4 (whether/how to use the private-host allowlist mechanism now shipped inert).**

## Revision 20 (2026-09-22): Sprint 10 PR #14 fix-pass 1 (after two review rounds)

D-4 was closed out fully: the original PR only added the config-variable table (a partial reading of the AC).
This fix-pass adds the remaining D-4 AC content to README.md -- a "Registering in Claude Code" section with a
`docker run` snippet worded as not-yet-generally-available (consistent with the pre-M3 banner and OQ-7 still
being open), a "Limitations" section (no JS rendering, prompt-injection/untrusted-content note), and an
"Other operational notes" section (config-error-before-handshake, fixed no-proxy client, NAT64/6to4 gateway
behavior, musl `.local`/split-DNS resolution gap). No AC bullet needed to be deferred or disclosed as a gap;
all were writable from already-decided facts. `docs/SSRF.md` was also corrected: it previously said no
private-host allowlist existed, which C-2 had already made false. See the dev-report's "Fix-pass 1" section
for the full list of review findings addressed (`Kind::Private` replacing a string-matched category check,
tightened `FETCH_ALLOW_PRIVATE_HOSTS` IP-literal validation, the empty-value startup-failure fix, corrected
stale security comments, and new regression tests). OQ-3 and OQ-4 remain open and unchanged in substance.

## Revision 21 (2026-09-22): Sprint 10 merged (0d0ff2f), Sprint 11 started

Sprint 10 (PR #14, including fix-pass 1 above) merged to `main` as `0d0ff2f`. Sprint 11 started on branch
`sprint-11/robots-charset-arm-tests`, scope per the Sprints 6-12 table row 11: **B-4, A-8, D-3 (8 pts)**.

Entry criteria for Sprint 11 as written in the table ("OQ-3 decided; runner checked") were **not fully met**:
OQ-3 (robots.txt enforce-by-default policy) is still OPEN at Sprint 11 start, exactly as it was at Sprint 10
start for the same reason. Proceeded anyway, following the same discipline Sprint 10 used for OQ-3/OQ-4: B-4 was
delivered as a fully implemented and tested robots.txt fetch/parse/matcher mechanism, strictly gated behind the
existing `FETCH_ROBOTS_TXT` flag (added in Sprint 10's C-1), whose default (`ignore`) is unchanged, so today's
behavior does not change until the owner decides OQ-3 and someone flips the default. A-8 (charset decoding via
`Content-Type`/meta-tag sniffing, `encoding_rs`) and D-3 (aarch64 leg added to the PR-level `test` job matrix,
per ADR-007) were delivered in full with no OQ blocking either. See `.delivery/artifacts/06-dev/sprint-11/dev-report.md`
for full detail, deviations, and CI results.

**Owner decisions still needed, carried forward unchanged in substance: OQ-3 (robots.txt default policy) and
OQ-4 (private-host allowlist activation/governance, no new Sprint 11 action).**

## Revision 22 (2026-09-22): Sprint 11 merged (152e2ac), Sprint 12 started

Sprint 11 (PR #15, B-4 3 / A-8 3 / D-3 2 = 8 pts) DONE. Code review round 1 found a real functional
regression introduced by A-8: `decode_whole_body` and the gzip-HTML branch of `read_body` never checked
`stop()`, silently defeating the A-5 `max_length` early-stop guarantee and the converter-failure early-abort
guarantee on every new charset-decoding path — verified live (2 MiB body, `stop` set to fire at 100 chars,
full 2 MiB still pulled/converted). Also found: a latent gzip `finish()`-on-truncated-stream bug that the
stop() fix would have activated; a `charset::sniff_meta` false-positive matching `charset=` text inside any
quoted attribute value, not just a real charset attribute; stale "robots.txt is an inert placeholder"
language in README/config docs/SSRF.md left over from Sprint 10's C-1 phrasing (QA round 1, 1 blocking,
flagged as a repeat of the Sprint 10 stale-docs pattern); an undisclosed memory-footprint change (gzip-HTML
with no charset header went from streaming to ~4x-cap buffered). Fix-pass (commit `da38309`) addressed all
13 blocking/non-blocking items in one pass: a new incremental `encoding_rs` decoder (`Pipeline::with_encoding`
in `src/fetch/body.rs`) restores genuine streaming/early-stop on the charset paths; the gzip-HTML branch now
returns before calling `finish()` on early stop; `charset::attr_value` was rewritten to do real
attribute-position scanning instead of a substring `find`; README/config/SSRF.md robots.txt language rewritten
to state the mechanism is real and gated behind `FETCH_ROBOTS_TXT=enforce`; the one remaining non-O(1) case
(gzip-HTML, no charset header, encoding_rs can't reliably flush partial gzip output for meta-charset peeking)
is explicitly disclosed in `docs/BENCHMARK.md` section 19 with a measured bound (~2-4x `max_bytes`, well under
the 40 MiB peak gate). Round-2 validator independently re-verified every item (re-read the rewritten code,
ran `cargo test --locked` itself — 236 unit + 1 integration + 11 stdio tests, all passed — and re-confirmed
all 17 CI checks green) and returned DONE, 0 blocking. PR #15 merged to `main` as `152e2ac` (head `da38309`).

D-3's CI matrix split renames the required check from `test` to `test (amd64)`/`test (arm64)` — a **new
disclosed operational risk**: branch protection on `main` still lists the old `test` name as a required check,
so this is a repo-owner action item, not something any agent here has permission to fix (needs a GitHub
settings change to the branch protection rule).

**Sprint 12 (B-6, E-6, D-6 = 7 pts) STARTED** on branch `sprint-12/labelling-ci-gate-v1`, matching the
Sprints 6-12 table row 12 exactly (final sprint; v1.0 tag). Entry criterion per the table ("OQ-5 decided;
hosted arm64 runner available") is met: OQ-5 was resolved in Sprint 2 (Revision 10 — "no label"). Runner
availability assumed per ADR-007 precedent (used without incident in Sprints 3-11).

**Owner decisions still needed at Sprint 12 start, unchanged in substance, to be surfaced again in full at
final wrap-up: OQ-3 (robots.txt default policy), OQ-4 (private-host allowlist activation/governance), OQ-7
(image distribution/licence, blocks D-2's `publish` job and any public GHCR image), E-5/E-7 owner URL lists
(10-URL smoke / 50-URL snapshot set, still not supplied), the new D-3 branch-protection required-check-rename
follow-up above, plus previously-carried items: Sprint 5 deviations acknowledgement, the panic=abort
deviation, `coverage`/`release-ldd-guard` not yet in required checks, cargo-audit never run, A-2's
throwaway-config check.**

## Revision 23 (2026-09-22): Sprint 12 implemented (PR opened), v1.0 tag deliberately NOT cut

Sprint 12 (B-6, E-6, D-6 = 7 pts): **B-6 CLOSED, re-confirmed** (no code change; OQ-5's "no label" decision from
Sprint 2 already fully implemented and tested since Sprint 5). **E-6 implemented**: `bench.yml`'s existing
`bench-gate` job gained a 10%-vs-last-main-baseline regression tripwire (`scripts/regression_gate.py`,
`bench/baseline.json`, self-tested), additive to the pre-existing strict 10 MiB/40 MiB absolute gate; a new
`update-baseline` job refreshes the baseline on push-to-main only. Two AC items are disclosed as not done: the
job cannot be added to branch-protection required checks (no agent here has that access — same constraint as
D-3/D-7), and `release.yml`'s `publish` job was not wired to depend on it because that job is already hard-gated
`if: false` on OQ-7 and touching its `needs:` now would be speculative ahead of the real unblocking edit.
**D-6 implemented (mechanism), NOT Done (by its own AC)**: new CI jobs `audit` (installs and runs `cargo-audit`
0.22.2, closing the long-carried "cargo-audit never run" item, 0 vulnerabilities against 209 crates) and
`dependency-count` (machine-checks NFR-05, currently 11 of <= 15); a release-notes step was added inside the
still-blocked `publish` job linking `docs/BENCHMARK.md`. **The `v1.0` git tag was deliberately NOT created**:
D-6's AC couples tagging to "a licence selected per OQ-7", which remains open, and the sprint plan's own risk
table already states that publishing before OQ-7 is answered would decide licence/distribution by default;
tag creation is reserved for the coordinator after full review of this sprint's PR, per this sprint's explicit
instructions. Full detail, deviations and test evidence:
`.delivery/artifacts/06-dev/sprint-12/dev-report.md`.

**Owner decisions still needed at Sprint 12 close, unchanged in substance: OQ-3, OQ-4, OQ-7 (now also blocking
the new D-6 release-notes step, in addition to D-2's `publish` job and the `v1.0` tag itself), branch protection
on `main` (still not configured — the largest single carried item across the whole engagement, now with two
more checks, `audit` and `dependency-count`, plus the E-6 `bench-gate` regression condition, added to the list
of things it can eventually require), E-5/E-7 owner URL lists (still not supplied), and whether/when to remove
the `false &&` guard on `release.yml`'s `publish` job once OQ-7 is decided (its own comment specifies what must
accompany that edit).**

## Revision 23 (2026-09-22): Sprint 12 merged (56f5a79) — all 12 sprints code-complete

CI on PR #16 was fully green (18/18 checks) at head `900e145`, but the new E-6 `bench-gate (amd64, musl)`
regression-tripwire leg then FAILED: root cause was a stale `bench/baseline.json` seed (Sprint 5 G4b figures)
that predated the legitimately disclosed Sprint 11 A-8 charset-decoding memory growth (BENCHMARK.md section
19) — not a real regression; the absolute 40 MiB memory gate passed cleanly throughout. Fix-pass 1 (commit
`10c4965`) reseeded `bench/baseline.json` from this PR's own bench-gate run, legitimate because `src/` and
`Cargo.lock` were byte-identical to `main` at the time. Full CI re-ran green on all 18 checks at the new head.

Two fresh, independent reviewers (code + QA) both returned 0 blocking findings. The code reviewer traced
`scripts/regression_gate.py`'s threshold math and JSON parsing by hand (not just read it), confirmed the
`bench/baseline.json` reseed values are plausible against `docs/BENCHMARK.md`, verified `contents: write` is
correctly scoped to only the `update-baseline` job (gated to pushes on `main`, gated on `bench-gate` succeeding
via `needs:`), confirmed `dependency_count_gate.py`'s count (11 of <=15) is correct by independently re-running
it, and confirmed `release.yml`'s `publish` job stays fully unreachable (`if: false && ...` short-circuits)
so the new D-6 release-notes step cannot run under any trigger. The QA reviewer verified every B-6/E-6/D-6 AC
bullet against the actual diff (not just the dev-report's claims) and found none of the "dev-report claims
more completion than the diff delivers" pattern seen in Sprints 10/11, with one minor exception: the Sprints
6-12 table's row 12 (line 89) still read "memory-gate CI required; v1.0 tagged by D-6" without flagging that
neither was actually met this sprint (both are disclosed honestly in prose elsewhere in this file and in the
dev-report, so no reader was misled, but the table cell itself was stale) — fixed directly in that row before
merge, same edit as this revision.

PR #16 merged to `main` as `56f5a79` (head `10c4965`). Per this sprint's explicit instruction and D-6's own
AC, **no v1.0 git tag was created** — the mechanism exists (CI jobs for audit/dependency-count, a release-notes
step) but tagging is coupled to a licence decision under OQ-7, which remains open; cutting the tag now would
decide image distribution/licence by default, exactly the risk this plan's own risk table warns against.

**This closes all 12 sprints' code delivery (all 34 stories' code either DONE or, for A-4/E-5/E-7, blocked
purely on the owner's still-unsupplied URL lists — no further coding work is scheduled by this plan).** The
full set of owner decisions still needed before v1.0 can be tagged and any image published is unchanged from
Revision 22's list above, plus the historically carried items: E-5/E-7 URL lists, OQ-3 (robots.txt default),
OQ-4 (private-host allowlist governance), OQ-7 (image distribution/licence — blocks the v1.0 tag and D-2's
`publish` job), branch protection on `main` never configured (for the Sprint 11 `test`->`test (amd64)`/
`test (arm64)` rename nor the new Sprint 12 `audit`/`dependency-count`/regression-tripwire checks), Sprint 5's
deviations (never formally acknowledged), the `panic = "abort"` release-profile deviation (never formally
acknowledged), and `coverage`/`release-ldd-guard` never added to required checks. None of these are decided
here; all are the project owner's calls per the standing instruction never to decide OQ-3/OQ-4/OQ-7 unilaterally.

## Revision 24 (2026-09-22): Sprint 13 (OQ-3/OQ-4/OQ-7 resolution, E-5/E-7 rework, sign-offs) — all seven carried owner items decided

**The project owner made all seven remaining decisions on 2026-09-22.** Sprint 13 (branch
`sprint-13/oq-resolution-finalization`, off `main` at `56f5a79`, the Sprint 12 merge) implements every one.
This closes out the last open items from Revisions 22/23 above except the `v1.0` tag itself, which stays a
coordinator action per every sprint's standing instruction.

1. **OQ-7 (image distribution/licence): RESOLVED — open source, `MIT OR Apache-2.0`.** `LICENSE-APACHE` (the
   file that lived at `LICENSE` since the initial commit) plus a new `LICENSE-MIT`; `Cargo.toml`'s `license`
   field set. `release.yml`'s `publish` job had its `false &&` short-circuit removed — the job is now
   reachable in principle — but the `inputs.confirm_publish == 'true'` manual-dispatch requirement is
   UNCHANGED and remains the actual safety net: a bare `v*` tag push still cannot publish anything by itself.
   `docs/ci-branch-protection.md`, `docs/EPICS.md` (D-2/D-6), README all updated from "still open" to
   resolved. No `v1.0` tag created or pushed by this sprint (reserved for the coordinator, per instruction).
2. **OQ-4 (private-host allowlist governance): RESOLVED — mechanism ships, gated behind a new master
   switch.** `FETCH_ALLOW_PRIVATE_HOSTS_ENABLED` (default `false`), a config var parsed the same way as
   `FETCH_ROBOTS_TXT` (case-insensitive, error-on-invalid). Even a populated `FETCH_ALLOW_PRIVATE_HOSTS` list
   has no effect unless this switch is explicitly `true` — an independent gate from list contents, defense in
   depth. `Policy::with_allow_private_hosts_gated` / `Policy::for_build` thread it through; `check_ip_for_host`
   checks it before consulting the list. Table-driven regression matrix added
   (`src/policy.rs::master_switch_regression_matrix`): switch off + list populated -> fail closed; switch on +
   list populated -> relaxes only listed hostnames; switch on + empty list -> nothing to relax; every row also
   reasserts IP-literal rejection, metadata/loopback/CGNAT blocking are untouched. Docker/README exposure: a
   plain `-e FETCH_ALLOW_PRIVATE_HOSTS_ENABLED=true`, no new plumbing. README/SSRF.md updated.
3. **OQ-3 (robots.txt default): RESOLVED — stays `ignore`, no code change.** Owner rationale: network-level
   ACLs elsewhere in the operator's infrastructure are the intended control point, not this server.
   `FETCH_ROBOTS_TXT=enforce` remains available for operators who want it. `README.md`, `docs/ci-branch-protection.md`,
   `docs/SSRF.md`, `src/config.rs` doc comments and `docs/EPICS.md` (B-4/C-1) updated from "still open" to
   resolved-as-`ignore`-by-design.
4. **E-5/E-7 (owner test-data lists): REWORKED — local-fixture harness replaces the live-URL dependency.**
   No E-7 harness code existed before this sprint (verified: nothing in `bench/` referenced a 50-URL list or
   manifest format). New `bench/corpus_check.py`: reads `.html` files from `bench/corpus/`, serves
   them over a loopback HTTP server, runs each through the real `fetch-mcp` `fetch` tool (via the existing
   `measure.py` stdio driver), and checks the same A-4 criteria (>= 95% success, >= 50% median token
   reduction, no `<script` leak) plus regenerates a plain `sha256sums.txt` manifest. Three clearly-marked
   placeholder fixtures (`example-01-article.html`, `example-02-table-heavy.html`,
   `example-03-inline-script.html`) prove the tooling works end to end; they are NOT a real corpus and do not
   pass the reduction target on their own (too few, too small — expected and documented). `bench/smoke.py` and
   `bench/e5_report.py` (E-5's live-URL harness, built Sprint 7) are left unmodified and still usable if a real
   live smoke is ever wanted later; they are simply no longer what A-4/E-5/E-7 are blocked on. New
   `docs/TEST-FIXTURES.md` documents exactly what to generate (count, suggested content diversity: prose
   article, nav/boilerplate-heavy, table-heavy, code-documentation-heavy, inline-script/style, edge cases),
   naming convention, drop location, and the exact commands to regenerate the manifest and report. **A-4/E-5/E-7
   remain NOT DONE** — this is expected: they are no longer blocked on a URL list that never arrived, only on
   the owner dropping real fixture files into `bench/corpus/`, a materially lower-friction ask.
5. **Branch protection: RESOLVED — deliberately not configured.** Owner decision: solo-developer repo, not
   worth the GitHub-settings friction. This is an accepted decision, not a gap, dated 2026-09-22.
   `docs/ci-branch-protection.md` rewritten (new "Branch protection: RESOLVED" section; the old "Owner
   quickstart" content kept only as clearly-marked historical reference, not an action item). No GitHub
   settings touched (no agent here has that access regardless, unchanged fact).
6. **Sprint 5 deviations and `panic = "abort"`: RESOLVED — both formally accepted as-is.** The project owner
   formally acknowledges, as of 2026-09-22: (a) the Sprint 5 deviations recorded in
   `.delivery/artifacts/06-dev/sprint-5/stage-summary.md` (re-homed E-2 AC items, the container-image
   measurement gap, the macOS reader gap, the `ziglang`/`cargo-zigbuild` pin gap) — accepted as-is, no code
   change; (b) the `panic = "abort"` release-profile choice (`Cargo.toml`, decided D-7/Sprint 0) — accepted
   as-is, no code change. Both were carried as "pending owner acknowledgement" since their respective sprints;
   both are now closed items.
7. **`coverage`/`release-ldd-guard`: RESOLVED — treated as already-effectively-required.** These CI jobs
   (from D-1/B-5, Sprint 8) run on every PR today. Per item 5 above, GitHub-native branch protection will not
   be configured, so they will never appear in a native "required checks" list — but the delivery process
   itself (this coordinator's own merge checklist) already requires all CI checks green, these two included,
   before any merge. Documented in `docs/ci-branch-protection.md`'s new "De facto required checks" subsection.
   No code change needed.

**Test evidence and full detail:** `.delivery/artifacts/06-dev/sprint-13/dev-report.md`.

**Owner decisions still needed going forward: none of the seven carried items above remain open.** The only
remaining action before v1.0 is the coordinator's own tag-creation step after reviewing and merging this
sprint's PR, and the owner generating real fixture files per `docs/TEST-FIXTURES.md` to actually close
A-4/E-5/E-7 (not required for v1.0 tagging, which was never coupled to those stories' completion).
