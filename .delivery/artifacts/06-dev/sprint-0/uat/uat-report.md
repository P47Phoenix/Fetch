# Sprint 0 User Acceptance Report (A-1, D-7, E-1)

Role: Product Owner. Branch `sprint-0/spikes` at HEAD f5d0b3c. Sources: docs/EPICS.md, docs/PRD.md, `.delivery/artifacts/05-plan/po/sprint-plan.md`, A-1 report, dev reports.

## 1. Verdict

**CONDITIONAL GO** (preliminary, x86_64 evidence only). The Sprint 0 exit criteria and Gate G0 are NOT formally met: G0 as defined requires measurement on the native aarch64 host (or a gap analysis with a credible path), and the ARM host has not been recorded. What holds is a strong x86_64 signal plus a credible-path argument. Nothing in the evidence points to NO-GO. The sprint PR must not be merged as "G0 passed" until the conditions in section 4 are closed or the owner explicitly accepts them.

## 2. Verification I ran myself
Clean extract: `git archive HEAD | tar -x -C $CLAUDE_JOB_DIR/tmp/ex`; separate target dir. Results (all on x86_64 Linux, rustc 1.94.1):

| Command | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` (root) | pass, no warnings |
| `cargo clippy --locked --all-targets -- -D warnings` (`spikes/a1`, lockfile committed) | pass |
| `cargo test --locked` | pass (2 unit tests; 0 doc/other) |
| `scripts/check-release-features.sh` | "guard OK" |
| `scripts/check-release-features.sh --self-test` | "self-test OK" (bad tree, unknown marker, allowlisted features all behave) |
| `python3 bench/selftest.py` | "SELFTEST PASSED" (stand-in server only) |
| `cargo build --release --locked -p fetch-mcp` then `fetch-mcp --version` | builds; prints `fetch-mcp 0.0.0`, rc 0 |
| `actionlint .github/workflows/ci.yml` | pass |
| `grep uses:` in ci.yml | all 6 actions pinned by 40-char SHA with version comment |

Not runnable here (tools absent or no network/remote): hosted GitHub Actions, `cargo-deny`, `cargo-audit` (`which` finds neither), any aarch64 hardware, branch protection settings. I did not run the A-1 x86_64 measurements again; A-1 numbers are taken from its report and committed JSONL.

## 3. Acceptance criteria

Legend: MET / NOT MET / DEFERRED (owner) / UNVERIFIABLE-HERE.

### A-1 (spike)
| # | AC | Status | Evidence |
|---|---|---|---|
| A1.1 | rmcp stdio server, one tool; client completes `initialize` and `tools/list`; rmcp version and features recorded | MET | A-1 report sec 1 (rmcp 3.4.0; features `server`, `transport-io`, `macros`, `schemars`); handshake and `tools/call` verified by `spikes/a1/bench/measure.py`. Not re-executed by me (see sec 2). |
| A1.2 | reqwest vs hyper (rustls) streaming a 5 MB body: RSS and binary size recorded, one selected with rationale | MET | Report sec 3a (reqwest 3926 kB idle/15060 peak/2799 kB bin; hyper 3754/13792/2436) and sec 8 (reqwest chosen, hyper noted as smaller alternative). Caveat: measured on one synthetic plain-HTTP fixture. |
| A1.3 | >= 2 HTML-to-markdown approaches on a 5 MB page and 10 sample pages: quality notes, peak RSS, time; one selected | PARTIAL, treat as NOT MET on the "10 sample pages, quality, time" parts | 5 MB page peak RSS recorded for htmd, html2md, html2text, lol_html (sec 3a/3b); lol_html selected. The report has no 10-sample-page run, no time figures and no real quality assessment; it states streaming emitter quality is "NOT proven" (R2). Selection rests on the memory result, which is decisive (DOM converters 56/185 MB vs 6-9 MB), but the AC text is not fully satisfied. Owner: developer; close in A-4 (Sprint 4) or accept. |
| A1.4 | glibc vs one alternative allocator RSS difference | MET | mimalloc vs system: idle about 11 MB vs about 5 MB (sec 3a); system allocator chosen. |
| A1.5 | Cross-build `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin` compile; aarch64-linux binary runs handshake on ARM hardware | NOT MET | Linux gnu and musl cross-builds proven (sec 4). `aarch64-apple-darwin` was not attempted or reported. Handshake ran only under qemu-user (musl); the gnu binary was not run; no native ARM hardware. Owner: project owner (ARM access); developer (darwin target, D-2 or earlier). |
| A1.6 | Result lists chosen crates, direct-dependency count vs NFR-05, PRD assumption changes | PARTIAL | Chosen crates listed (sec 8). No explicit direct-dependency count vs the NFR-05 limit (<= 15) is stated in the report, and no explicit "PRD assumption changes" section exists (streaming-converter requirement and the R1-R6 risks are the de facto changes). The sprint exit lists both as required. Owner: developer, small doc fix. |

### D-7 (hosted CI, pinned profile, release guard)
| # | AC | Status | Evidence |
|---|---|---|---|
| D7.1 | PR CI runs fmt, clippy `-D warnings`, test with `--locked`; required checks | UNVERIFIABLE-HERE | Workflow exists and passes actionlint; the same three commands pass locally. Actions have never run; required status is a repo setting. |
| D7.2 | `rust-toolchain.toml` exact channel, `Cargo.lock` committed, `--locked` everywhere, SHA-pinned actions | MET | `rust-toolchain.toml` (1.94.1); `Cargo.lock` and `spikes/a1/Cargo.lock` tracked in `git ls-files`; SHA pins verified by grep. |
| D7.3 | `deny.toml` (advisories, bans x5, sources, licenses) and `cargo deny` passes | UNVERIFIABLE-HERE | `deny.toml` present; `cargo-deny` never run (not installed). Do not count as passing. |
| D7.4 | Release-guard job asserts `test-support` and `bench-loopback` absent (tree and marker), with self-test, `-p` release build; E-8 tests with `--features bench-loopback` in CI | MET for guard and self-test locally; the E-8 test sub-clause DEFERRED to E-8 | Guard and self-test pass (sec 2). No E-8 tests exist yet; this is expected and noted in ci.yml. CI execution of the job is unverified. |
| D7.5 | Release profile pinned in `Cargo.toml` (opt-level, lto, panic=abort, strip, codegen-units); `publish = false`; `licenses.private.ignore`; spike passes clippy `-D warnings` | MET (profile, publish, spike clippy); `licenses.private.ignore` not confirmed by a `cargo deny` run | Cargo.toml: opt-level "s", lto true, cgu 1, panic abort, strip true; spike clippy passes (sec 2). Note: opt-level "s" vs 3 is left to D-1; the pin is A-1's build profile, not re-measured on aarch64. |
| D7.6 | Branch protection on `main` requires fmt, clippy, test, deny, guard checks, recorded in the PR | NOT MET (owner action) | Only documented in `docs/ci-branch-protection.md`. Owner: project owner; needs the branch pushed and PR/CI run first so the check names exist. |
| D7.7 | Fork PRs: hosted jobs only, no self-hosted | MET (by inspection) | ci.yml uses hosted runners only; not exercised by a real fork PR. |

Sprint exit criterion "hosted CI green on the Sprint 0 PR": NOT MET (branch not pushed, no PR; requires an owner instruction).

### E-1 (benchmark harness and targets)
| # | AC | Status | Evidence |
|---|---|---|---|
| E1.1 | One-page result: idle <= 10 MB, peak <= 40 MB on 5 MB page, median of 10 valid runs; one MB unit stated | MET | `docs/BENCHMARK.md` sec 1-2 (MiB, stated once). Note the doc is longer than one page; acceptable. |
| E1.2 | Host recorded as author's native aarch64 runner with OS and RAM | PARTIAL: MET as written (placeholders allowed by the AC), NOT MET against the sprint exit ("OS/RAM recorded") | BENCHMARK sec 3 placeholders. Owner: project owner. |
| E1.3 | Protocol: handshake, 30 s idle, 5 MB / 50 MB (with and without Content-Length) / slow-drip fixtures, client script, `/proc` read | MET | BENCHMARK sec 4-7; `bench/fixtures.py`, `serve.py`, `measure.py`, `scenarios.py`; fixtures verified by the dev; selftest passes (I ran it). Harness has never run against a real MCP server. |
| E1.4 | TLS benchmark approach decided, caveat kept until then | MET | BENCHMARK sec 8: no fixture-CA build; one-off manual real-host run; caveat "NFR-11 measured over plain HTTP only" retained. Labelled author recommendation for owner review; owner should confirm. |
| E1.5 | 50-URL set (and 10-URL smoke list) | DEFERRED, owner: project owner | User decision in Sprint 0 revision 4; must exist before E-7 starts in Sprint 2. Criteria are defined; no URLs invented. |
| E1.6 | Loopback path recorded (`bench-loopback`) and which binary each gate measures; does not decide OQ-4 | MET | BENCHMARK sec 5; features present in Cargo.toml and guarded. |
| E1.7 | Tokenizer/count method, baseline, "converts successfully", Goal 5 overhead method | MET | BENCHMARK sec 9. |
| E1.8 | A-1 binary re-run under the protocol, or deviation recorded | MET via recorded deviation | BENCHMARK sec 10 states A-1 was not re-run (no ARM host). |
| E1.9 | Go/no-go recommendation on whether targets look achievable | MET | BENCHMARK sec 10 and A-1 sec 9: preliminary GO, conditional on streaming design and native ARM confirmation. |

## 4. Sprint 0 exit criteria and Gate G0 (as the plan defines them)

| Exit criterion | Status |
|---|---|
| Release profile pinned (D-7) | MET |
| Native aarch64 host access confirmed, OS/RAM recorded | NOT MET (owner) |
| Hosted CI green on the Sprint 0 PR | NOT MET / UNVERIFIABLE-HERE (no push, no PR; deny/audit never run) |
| E-1 one-page result committed (targets, MB definition, fixtures, TLS approach, host OS/RAM) | MET except host OS/RAM |
| 50-URL list | DEFERRED to project owner (prerequisite of E-7, Sprint 2) |
| A-1 lists chosen crates and dependency count vs NFR-05 (<= 15) | PARTIAL: crates yes, count not stated |
| PRD assumption changes recorded | PARTIAL: implicit in A-1 risks, no explicit list |

**G0 rule:** idle <= 10 MB and 5 MB peak <= 40 MB, median of 10 valid runs under the E-1 protocol on the D-7 profile, on the native aarch64 host (glibc), OR a written gap analysis with a credible path; ARM host recorded.
- Native aarch64 measurement: NOT DONE. Measurements are x86_64 only, taken before the protocol existed (0.5 s settle, 16 MiB cap, kB) on a spike build, not the D-7 skeleton.
- x86_64 signal: idle 3.7-5.2 MB, streaming peak 6.1 MB (first page), 8.7 MB (deep page), 11.1 MB (raw), against 10 / 40 MB. Wide margin.
- Gap analysis: present in substance (A-1 sec 5 and 7, BENCHMARK sec 10) naming the unmeasured items (page size, musl vs glibc allocator, TLS, compression, real pages, real converter quality). The credible path is real but rests on an unproven streaming converter (R2) and a crude emitter.
- ARM host recorded: NOT MET.
- No-go condition ("Goal 1 not achievable") is not triggered by anything measured; the only failing design (buffered DOM converter, 56 MB) is already excluded by the chosen architecture.

Reading strictly: the "or gap analysis" branch is satisfied only if the ARM host is recorded, which it is not. Hence CONDITIONAL GO rather than GO.

## 5. Conditions (must close, owner named)
1. C1 (project owner): confirm native aarch64 runner access and record OS, RAM, page size in BENCHMARK sec 3. Hard dependency for the real G0, Sprint 1 readiness and later gates.
2. C2 (project owner, then developer): on the ARM host, re-run the built spike (or the D-7 skeleton plus streaming path) under the E-1 protocol (idle and 5 MB peak, gnu and musl, median of 10) and append to G0. If the ARM run exceeds a target, the verdict reverts to NO-GO pending re-scope of Goal 1. Must precede any Sprint 4 gate; ideally before Sprint 1 ends.
3. C3 (project owner): push branch and open the draft Sprint 0 PR on instruction; get hosted CI green (including `cargo-deny`, which has never run); configure branch protection per `docs/ci-branch-protection.md` and record it in the PR.
4. C4 (developer): close A-1 doc gaps: direct-dependency count vs NFR-05, explicit PRD-assumption-changes list, `aarch64-apple-darwin` build result (or move it explicitly to D-2 with owner approval).
5. C5 (project owner): produce the 50-URL and 10-URL lists before E-7 starts (Sprint 2).
6. C6 (developer, Sprint 4 A-4): 10-sample-page quality and time comparison for the chosen converter; carry risk R2 (streaming conversion quality) as an open risk until then.
7. C7 (project owner): confirm the TLS decision (manual real-host run, no fixture-CA feature).

## 6. Open items carried, not decided here
OQ-3, OQ-4, OQ-5 and OQ-7 remain open (PRD status row). OQ-7 affects D-2 and `licenses`/publishing; OQ-4 is not decided by E-8; OQ-3 and OQ-5 affect later stories. `cargo-audit` has never been run and is not a Sprint 0 AC. No claim in this report extends to ARM, hosted CI, cargo-deny, TLS, compression, redirects or non-UTF-8 charsets.

## 7. Decisions needed from the user
1. Accept a preliminary CONDITIONAL GO on x86_64 evidence (proceed to Sprint 1 planning) or hold until the ARM measurement (C2)? Recommendation: proceed with Sprint 1 only for work that needs no ARM (A-3a table-driven SSRF core), and treat C1/C2 as due before Sprint 1's ARM readiness check; do not merge the PR as "G0 passed" until then.
2. Provide ARM runner access and record OS/RAM (C1), and say who runs the ARM measurement.
3. Authorise pushing `sprint-0/spikes` and opening the draft PR (C3), and configure branch protection after the first CI run.
4. Provide the 50-URL and 10-URL lists before Sprint 2 (C5).
5. Confirm the TLS approach (C7) and whether A-1's missing darwin build and 10-sample-page comparison are accepted as deferred (C4, C6).
