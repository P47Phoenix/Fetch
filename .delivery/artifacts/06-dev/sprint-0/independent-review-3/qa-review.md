# Sprint 0 independent QA review 3 (D-7, E-1) at 5ccc4a6

Scope: can verification be trusted? Reviewer did not read prior review folders. The 50-URL list deferral is not flagged.

Verdict: no blocking findings. 5 non-blocking findings. Nothing that ran was found to give a false PASS for a wrong-kind binary or an error-only server; one partial-fetch false pass exists (F1).

## Executed
- `python3 bench/selftest.py`: SELFTEST PASSED (33 s, all checks ok).
- `scripts/check-release-features.sh --self-test`: self-test OK (clean passes; test-support and bench-loopback each fail for both tree and marker; renamed feature, garbage tree, unknown marker fail).
- Extra guard probe: set `default = ["bench-loopback"]` (aliasing) in Cargo.toml, ran the guard: fails on tree AND marker. Cargo.toml restored, tree clean, release binary rebuilt.
- `actionlint`: clean.
- Probe of `measure.py --gate` with a scratch server (/tmp/qa3, not committed) and `native_aarch64`/`check_identity` monkeypatched, since x86_64 host exits 3 first.

## Findings

### F1 (non-blocking, worth fixing before E-2): partially fetch-less server passes the whole G4a gate
`g4a-50mib-cl` has `min_bytes = 0` and nothing proves the request reached the fixture server. A server that really fetches the 5 MiB, gz and chunked routes but returns an error for `/50mb-cl.html` without connecting (URL-substring shortcut) got 10/10 valid, boundedness 0.974, summary `PASS`, rc 0 (with late-landmark forced implemented). The outcome check is also an unanchored substring: error text "not too_large" counts as too_large. BENCHMARK.md line 79 says the gate is not falsely passed "because the same server fails g4a-5mib-full"; that holds only for an error-only server, not a partial one. Fix: record request arrival per route in serve.py (request counter >= 1 for G7a) and require exact error code. Also `g4a-late-landmark` will have floor 0 when E-2 implements it (same hole).

### F2 (non-blocking): gate path after the aarch64 check is never exercised end to end
On x86 the native check returns exit 3 before binary identity, fixture, unimplemented-scenario and runs checks. Identity is tested only by direct `check_identity` unit calls; no test drives `--gate` through identity to a refusal or PASS. Also a G4a `--gate` run cannot currently succeed anywhere: `g4a-late-landmark` is unimplemented and the group requires it (honest but worth stating in the docs quickstart). Docs table row "3 = off native aarch64" is accurate; the doc statement that override/incomplete refusals precede exit 3 is accurate for those rules only.

### F3 (non-blocking): guard tree check inspects the root package only
Transitive dependency features are not examined; protection there rests on the marker grep, which only detects features that emit a `FETCH_MCP_MARKER_` string. Today there are no dependencies, so it is moot; A-3a must keep the marker convention or extend the tree check. Guard also checks only the host-target binary (not musl/aarch64 artifacts) except via `--binary`, which CI never invokes.

### F4 (non-blocking): CI unverifiable until first hosted run; docs are honest about it
ci.yml matches D-7 ACs on paper: fmt/clippy/test (with --locked), SHA-pinned actions, toolchain pinned, deny job, release-guard with self-test, spike clippy across 5 feature sets, hosted runners only, no pull_request_target. Unknowns never run: Actions, `cargo-deny-action` argument order (`arguments: --locked` with `command: check`), `rustup show active-toolchain` auto-install of a toolchain with 3 extra targets, cargo-deny, cargo-audit, native aarch64. Both BENCHMARK.md and ci-branch-protection.md state this plainly. Minor: `concurrency.cancel-in-progress` also cancels main-branch runs on rapid pushes; branch protection is "NOT YET CONFIGURED" so the D-7 AC for required checks is open until the owner acts (docs say so).

### F5 (non-blocking): doc nits
ci-branch-protection.md mentions `cargo audit` "not installed", while no job uses it; fine but D-7 does not require it. The stand-in worked example needs `STANDIN_BENCH=1`, correctly documented, yet gate mode refuses that trick (also documented).

## Probe results (--gate false-pass matrix)
| Probe | Result |
|---|---|
| error-only server | INVALID, exit 2 (handshake `call_ok`) |
| no `fetch` tool | INVALID |
| crash/EOF mid-fetch | fails fast (0.6 s), invalid sample |
| stand-in script / wrong-arch ELF / foreign version / marker mismatch | refused by `check_identity` (unit-tested only, see F2) |
| <10 valid runs, --smoke, overrides, child env, partial scenario set | refused/INVALID exit 2 (tested) |
| units | kB from /proc treated as KiB, MiB=1024 kB; consistent |
| missing 5 MiB reference | INCOMPLETE exit 2 |
| non-gate pass | ADVISORY_PASS, never PASS |
| fetch-less 50mib-cl only | PASS (F1) |
| release guard aliasing (`default` -> bench-loopback) | fails |
