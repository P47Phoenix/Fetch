# Sprint 0 independent QA review (D-7, E-1)

Scope: verification trustworthiness only. HEAD of sprint-0/spikes. Dod artifacts not read.

## Executed
- `python3 bench/selftest.py`: SELFTEST PASSED (all checks ok).
- `scripts/check-release-features.sh --self-test`: self-test OK (clean passes; test-support and bench-loopback each fail for tree AND marker reasons; renamed feature, unknown marker, garbage input all fail).
- `cargo test --locked` and `--features bench-loopback`: pass (2 and 3 tests). Tests assert real things (empty marker list, version prefix, marker literal), though they are thin and `default_build_has_no_markers` is skipped under the feature.
- Adversarial probes on measure.py (below).

## Blocking

B1. Handshake accepts JSON-RPC errors, so a broken server yields "valid" samples. bench/measure.py:90-102 (`call` returns any message with matching id; `handshake` never inspects `error`/`result`). Repro: /tmp-style server that answers every request with `{"error":...}` and prints `fetch-mcp 0.0.0` for `--version`:
`python3 bench/measure.py --binary errsrv.py --binary-kind shipped --scenario idle --settle 0.2 --smoke` -> 10/10 valid_runs, verdict computed on its RSS. On aarch64 with --gate a server that fails initialize/tools/list (lower RSS than a working one) would count as a valid idle PASS. Fix: require `result` on initialize and tools/list, and require a `fetch` tool in tools/list.

B2. No binary identity check; stand-in or wrong-arch binary can be a "shipped" gate figure. measure.py:195-203 trusts only the `--version` text (substring markers). Repro above: a Python script printing "fetch-mcp 0.0.0" is accepted as `shipped`; bench/standin_mcp.py itself is accepted as `shipped` (its version says "standin-mcp", not checked). BENCHMARK.md sec 5 step "file <binary> = ARM aarch64" is a manual step, not enforced. On the aarch64 runner, `--gate` would emit gating=true PASS for a stand-in. Also kind is derived from --version, not from scanning the binary for FETCH_MCP_MARKER_ (the guard's method), so a bench build whose --version omits the marker is refused but a bench build reporting a clean --version passes as shipped. Fix: in --gate require ELF e_machine aarch64, `--version` starting `fetch-mcp `, and grep the file for FETCH_MCP_MARKER_ / bench-loopback for shipped kind.

## Non-blocking

N1. Advisory exit code 0. measure.py:255-258: without --gate a pass returns exit 0 (`ADVISORY_PASS`), including with `--idle-target-mib 1000`; a naive CI step checking only rc would read it as success. Suggest non-zero (e.g. 4) for non-gating passes or refusing overrides unless --smoke.
N2. `--runs` above 10 tolerates invalid runs (measure.py:228): the median is over the valid subset only; invalid reasons are recorded but there is no cap on the invalid fraction. E-2 AC "fewer than 10 valid" is met literally.
N3. The gate PASS path (a.gate true, native aarch64, exit 0, verdict PASS) is never exercised: selftest only shows refusals (rc 2/3) off aarch64. Untested code path that produces the headline result.
N4. bench-kind gate is unreachable by design: required set includes unimplemented `g4a-late-landmark` (scenarios.py:276) so it always refuses (exit 2). Fail-safe and good; docs should say so (BENCHMARK.md does list it unimplemented).
N5. CI test job does not run `cargo test --features bench-loopback` (ci.yml:144 comment), yet D-7 AC says E-8 unit tests run with that feature in hosted CI; `bench_build_version_carries_marker` never runs in CI. Clippy covers the feature but not `test-support` (ci.yml:133-134).
N6. `deny` uses `arguments: --locked` (ci.yml:155). Whether cargo-deny accepts `--locked` after `check` is unverified (cargo-deny not installed here). If rejected, the required `deny` check fails on first run.
N7. Guard scope: builds host target only, gnu/musl/aarch64 artifacts are not guarded (check-release-features.sh:53-60). Acceptable for Sprint 0; D-2/D-3 must call it on real artifacts. `--features` mode guard_build under `set -e`/`||` is ok (build failure still yields rc via missing binary).
N8. Guard tree check covers only root-package features; transitive enablement is safe today (no dependencies) and backstopped by the marker grep, but a future feature without a marker relies solely on the allowlist (script comment states this). Positive controls are real (they build the actual features and require both failure reasons).
N9. SHA pinning: both actions are full 40-char SHAs with comments; the checkout SHA matches v4.2.2 by my recollection, the cargo-deny-action SHA/version cannot be verified offline. All cargo commands use `--locked` (fmt has no resolution). Harness env: `PATH` is inherited unpinned (measure.py:77); `LC_ALL` etc. pinned; `--child-env` allowed without --gate and recorded.

## Checked and sound
- Units: 1 MiB = 1024 kB of /proc kB; targets 10/40 MiB; fixtures and CAP are MiB (measure.py:18, scenarios.py:266).
- Fields: idle = VmRSS after settle (30 s default, --gate forces 30, parallel 1); peak = VmHWM read after the call returns. Median via statistics.median of valid samples; <10 valid -> INVALID, exit 2; strict <= target.
- Early stop: server-side byte counter vs manifest min bytes; 50 MiB CL uses floor 1 (header abort, per architecture 11.2). too_large must be a tool error with "too_large" text.
- Boundedness: 50 MiB medians vs 5 MiB median, ratio <= 1.10; missing reference -> INCOMPLETE (exit 2), never PASS. Selftest covers both.
- Nonexistent binary refused; kind/scenario mismatch refused; fixture hash mismatch refused; gate overrides refused before the aarch64 check (exit 2 vs 3 exercisable anywhere).

## Unverified (honesty of docs)
Not runnable here: GitHub Actions execution (no run of ci.yml), required-status-check branch protection (docs/ci-branch-protection.md correctly says NOT YET CONFIGURED), cargo-deny (not installed, licence allow-list and `--locked` unproven), cargo-audit (not installed; docs correctly say deny covers advisories and that a scheduled audit is D-3), native aarch64 measurement (none; docs state x86_64-only A-1 data, G0 re-run outstanding, runner PLACEHOLDER, 50-URL list NOT DONE). Docs are honest on these points; the residual risk is that `--gate` cannot yet be trusted (B1, B2) before the first aarch64 run.
