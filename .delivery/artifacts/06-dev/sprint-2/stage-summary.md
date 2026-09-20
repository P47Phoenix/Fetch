# Stage 6 Development, Sprint 2: summary (routing metadata)
Status: PARTIALLY DONE. E-8 (1 pt) and A-3b (5 pts) DONE. E-7 (2 pts) NOT DONE. Branch `sprint-2/guarded-fetch`, PR #6 merged to main as 35a3450 (2026-09-20). CI 10/10 green.

## What shipped
- A-3b: streaming, size-bounded, SSRF-guarded HTTP fetch client (reqwest/rustls(ring)/webpki-roots/flate2). Reports: A-3b/dev-report.md, fix-pass-1-report.md.
- E-8: `bench-loopback` feature (absent from release builds, guarded by D-7).
- CI: `a3b-merge-gate` job added; advisory `bench-product-peak` arm64 smoke job added.

## Reviewer verdicts (from A-3b/dod*, pr-review/)
- Architect: DONE. PR review: APPROVE (round 2). QA and tech-writer: DONE (round 3).

## Sprint 2 exit criterion NOT MET: E-7
E-7 (offline snapshot capture of the 50 URLs, committed sha256 manifest) was not started. It needs the owner's 50-URL list and the 10-URL smoke list; neither has been supplied. This is recorded as an exit criterion not met, not as done. Per the plan's own overflow rule, E-7 moves to Sprint 5 (6 + 2 = 8 pts) unless the lists arrive; A-4's 95%/50% AC waits for it (A-4 not Done until then; the Sprint 4 exit excludes that check; Goals 2 and 3 evidence one sprint later; G4a unaffected). Sprint 3 stays at 8. Note that the overflow rule was written for an A-3b/E-8 overrun; here both were delivered and E-7 slips only because of the missing owner input. If the lists arrive before Sprint 4 entry, E-7 can be pulled back.

## Owner decision
- OQ-5 RESOLVED (Michael, 2026-09-20): no untrusted-content label; fetched content returned as-is; B-6 won't-do.

## Evidence
- aarch64 advisory 5 MiB smoke: VmHWM peak 4.77 MiB (single CI run 35523134933, median of 10 samples; advisory, not a gate); idle 3.58 MiB. Not the memory gate.
- G4a and G4b have not been run.

## Deferred, non-blocking
- E-8 `--version` commit/Cargo.lock hash AC re-homed to E-4.
- TLS has no automated test.
- DNS resolution blocking-thread note for B-3/B-5.
- Redirect-chain memory check (5 hops) for E-2.
- Unused rustls-platform-verifier trim in D-1.
- architecture.md line 4 stale "No open question is decided" header: fixed in this commit.

## Owner items
- Add `a3b-merge-gate` to branch protection on main; configure branch protection generally (docs/ci-branch-protection.md).
- A-2 Claude Code throwaway-config check not confirmed.
- cargo-audit never run (nightly audit is D-3).
- amd64 gate mode in bench/measure.py not done.
- 50-URL and 10-URL lists (E-7).
