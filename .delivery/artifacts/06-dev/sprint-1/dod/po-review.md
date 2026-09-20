# Sprint 1 PO acceptance (A-2 3 pts, A-3a 5 pts), PR #5, branch sprint-1/skeleton-ssrf @ 0bcae4e

Verdict: ACCEPT WITH CONDITIONS. Blocking findings: none. STATUS: DONE.
Verified by me: `cargo test --locked` (40 unit + 7 stdio integration, all pass), release build (aarch64 ELF, `--locked`), stdio session against the release binary, `cargo tree --locked -e normal` grep for HTTP/TLS crates (none), `gh pr checks 5` (bench gnu/musl, bench-product, clippy, deny, fmt, release-guard, test: all pass), the ARM evidence files, and git diff main...HEAD. I did not read other validators' output.

## A-2 acceptance criteria
| AC | Status | Evidence |
|---|---|---|
| tools/list is exactly `fetch` with url, max_length, start_index, raw | MET | Ran the binary: one tool `fetch`, schema has the four params; a stdio test also covers it |
| Validation errors name the field (missing url, file:/ftp:, negative/non-integer numerics) | MET | `file:///etc/passwd` gives `error[invalid_argument]: url: scheme must be http or https`; `max_length:-1` is rejected naming `max_length`. Missing url and ftp are covered in tests/stdio.rs and ssrf tests (not each re-run by me) |
| Logs to stderr, stdout only MCP | MET | Session stdout held only JSON-RPC lines; stderr was empty; purity integration test passes |
| Ready within 250 ms on aarch64 | MET (advisory evidence) | .delivery/artifacts/06-dev/A-3a/arm/results-product.jsonl: ready_ms median 1.06, max 1.07 (n=10), native aarch64 Azure VM, Ubuntu 24.04, 4 KiB pages, recorded not gated, matching the plan ("recorded evidence"). Idle RSS 2.6 MB |
| Claude Code lists `fetch` via throwaway config on author's machine | UNVERIFIABLE-HERE / DEFERRED, owner = project owner (Michael) | Manual, needs the owner's machine and Claude Code. The protocol-level equivalent (initialize + tools/list over stdio) passes |

Judgement on the manual item: it does not block Sprint 1. The sprint exit list does not include it; the plan only says the check uses a throwaway config. It is a tracked manual item. Condition C1: the owner runs it and records the date and result in the PR before merge, or carries it as an explicit open item in the sprint review. A-2 should be called Done pending that record.

## A-3a acceptance criteria
| AC | Status | Evidence |
|---|---|---|
| Table-driven range tests cover the full set (v4, v6, mapped/compatible, ULA, link-local, loopback, CGNAT, unspecified, three metadata addresses; public passes) | MET | src/ssrf/ranges.rs tests (445 lines) include fd00:ec2::254, 168.63.129.16, 169.254.169.254; all pass |
| IP-literal blocked before resolution | MET | check_url tests in ssrf/mod.rs pass |
| Resolver filter: resolve once, refuse if any answer blocked, return validated set; injectable resolver | MET | ssrf/resolver.rs tests incl. mixed answer and DNS-failure cases pass |
| Per-hop revalidation, non-http(s) refused | MET | ssrf/mod.rs tests (redirect error mapping to blocked_target) pass |
| Default Policy fail-closed; loopback only via gated constructor; release guard is a required CI check | MET for code and check, NOT YET CONFIGURED for "required" | Policy Default exists and is fail-closed (tests pass); release-guard job passes on PR. But `gh api .../branches/main/protection` returns 404 (Branch not protected), and docs/ci-branch-protection.md says the rule is NOT YET CONFIGURED. See condition C2 |
| No dependency on HTTP client | MET | cargo tree: no reqwest, hyper, ureq, h2, rustls, native-tls, openssl. The guard also asserts this. tokio has no `net` feature |
| cargo test, clippy, fmt pass; each range and mixed answer covered | MET | Local tests and CI green |

## Sprint 1 exit and gate
- tools/list exactly `fetch`: MET. Stdout-purity test: MET. Ready on ARM: MET (advisory). Range tests: MET. Fail-closed default: MET. Hosted CI green: MET (all 8 checks pass on the PR head).
- CI release-build check is a required check: NOT MET as configured. The job exists and passes, but branch protection is not set on main (API 404). This is an owner-only GitHub setting, so I record it as DEFERRED, owner = repository owner. Condition C2: configure the rule (fmt, clippy, test, deny, release-guard) and record it per the template in docs/ci-branch-protection.md before or at merge. Sprint 1 cannot honestly claim this exit line until then.
- Gate "no HTTP client dependency": MET, machine-checked by release-guard.

## Scope creep, points, open questions
- Scope: diff is 26 files, +3170 lines, of which Cargo.lock is 748. Extras beyond A-2/A-3a: arm-bench.yml `bench-product` job, bench/measure.py and selftest changes, and check-release-features.sh extension (HTTP-client check). All serve the recorded ARM evidence and the Sprint 1 gate and add no product features; I count them as in-scope enablers, non-blocking. Note: the dev-report claims A-3a adds 0 direct dependencies (4 of 15 for NFR-05); that is consistent with Cargo.toml (rmcp, tokio, serde, schemars). No fetch-capable code exists.
- Points: 3 + 5 = 8, at the ceiling and unchanged. Honest. No re-estimate needed. The bench-product/harness work was absorbed without a point change; fine, but the owner should know it was outside the stated stories.
- Open questions: no silent decision found. A grep for OQ-3/4/5/7 in src and Cargo.toml finds none; `publish = false` stays and docs explicitly say OQ-7 is not decided. Non-blocking observation: the LICENSE is Apache-2.0 from the initial commit, pre-existing and not a Sprint 1 change.
- Requirement trace: FR-01, FR-02, FR-06, FR-13, NFR-01, NFR-04, NFR-05 are satisfied at the Sprint 1 level (schema, validation, stderr-only logging, SSRF core, low RSS, dependency count <= 15). FR-06 in full (wired through a client) is A-3b.

## Conditions
C1. Owner runs the Claude Code throwaway-config check and records the result (owner: Michael; before merge or carried in the sprint review).
C2. Owner configures required checks on main (fmt, clippy, test, deny, release-guard) and records it in the PR (owner: repository owner).
C3 (non-blocking). The 250 ms figure is recorded, not asserted in CI; keep it advisory and revisit at D-3, and restate the figure as process start, not container start, as the plan asks.
C4 (A-3b entry). OQ-5 must be decided before Sprint 2 starts; A-3b may not merge without A-3a's checks on the branch.
