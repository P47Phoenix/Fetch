# QA DoD review: Sprint 0 (D-7, E-1)
STATUS: DONE (no blocking findings)

## Executed
- `python3 bench/selftest.py`: 15/15 ok, SELFTEST PASSED (x86_64). Known +20 MiB alloc is measured in both VmRSS (idle) and VmHWM (peak) within 1.5 MiB.
- `scripts/check-release-features.sh --self-test`: OK (clean passes; test-support, bench-loopback, fixture-ca each fail on BOTH tree and marker detection, reason-checked, so not vacuous).
- Probes: `--runs 3` without `--smoke` refused (exit 2); wrong kind (shipped on peak, bench without marker, bench-marked on idle) refused; early stop -> INVALID exit 2; tamper -> refused; `--gate` off aarch64 -> exit 3; target override flips verdict PASS/FAIL at the boundary (16.29 MiB vs 16.3 / 10).

## Gate criteria
- Validity rule: <10 valid samples gives INVALID and exit 2 (9 of 10 also INVALID); handshake/protocol failure counts as invalid, never dropped; early stop caught by server-side byte counter vs manifest size. Pass.
- Median of 10, min/max recorded, `median <= target` (10 MiB idle, 40 MiB peak); gating peak = max of per-scenario medians; boundedness <= 1.10. Pass.
- /proc fields: idle VmRSS after handshake + 30 s settle; peak VmHWM read after fetch returned. Matches AC. Pass.
- Unit: single MiB (2^20) definition, applied to targets, kB/1024. Pass.
- Cannot pass falsely: `--gate` forbids `--smoke` and target overrides and requires native aarch64; non-gate runs are labelled `gating:false`.
- Release guard: working positive-control self-test. Pass.
- CI covers D-7 ACs: fmt, clippy, test, deny, release-guard, all `--locked` (fmt does not need it), SHA-pinned actions, hosted only, read-only permissions, `-p` release build. actionlint and shellcheck available and reportedly clean.

## Findings (all non-blocking)
1. Selftest gaps: no case for peak-target FAIL exit 1, boundedness > 1.10 FAIL, the aggregate summary FAIL, or `--gate` override refusal (unreachable off aarch64). Logic read as correct but untested.
2. `--smoke` (or any non-gate run) can exit 0 with verdict PASS; only the `note`/`gating:false` field flags it as advisory. Risk of misreading in scripts; suggest a distinct verdict/exit for non-gating runs.
3. Bench-vs-shipped identity relies on the `--version` string marker; a mislabelled binary would be trusted. E-8 bounds (same commit/lock hash) are documented but not enforced by the harness yet.
4. `g4a-50mib-cl` has min_bytes 0, so no early-stop check for that scenario (acceptable: server may refuse on headers), but it is weaker validation.
5. Stand-in based only; no product measurement and no aarch64 run. Gzip fixture hash depends on the zlib build (dev-report admits it).

## Unverified, stated honestly by the dev reports (confirmed)
- GitHub Actions never ran; required-status-check names and branch protection are owner-configured (docs only).
- cargo-deny not installed locally (confirmed), so deny.toml has never been executed.
- E-8 `--features bench-loopback` test job deferred; guard markers only become meaningful after A-3a.
- E-1: 50-URL/10-URL lists NOT DONE (owner/E-7), A-1 re-run not done (deviation recorded), host fields are placeholders. `spikes/a1/Cargo.lock` remains untracked.
