# Sprint 0 independent QA review 2 (D-7, E-1) at 84ec595

Verdict: DONE, 0 blocking, 9 non-blocking. The 50-URL list deferral is not flagged.

## Executed
- `python3 bench/selftest.py`: SELFTEST PASSED (incl. identity, EOF, error-only, fetch-less, early stop).
- `scripts/check-release-features.sh --self-test`: self-test OK (real cargo builds of both forbidden features).
- `cargo fmt --check`, clippy (default, bench-loopback), spikes/a1 clippy (default set), `cargo test` with default/bench-loopback/test-support: all green locally.
- Real shipped and bench-loopback release binaries pass `check_identity` (positive control is real, not only synthetic). `--gate` on this x86_64 host: REFUSED (exit 3 path).
- Guard attack copies in /tmp: alias feature (`other=["bench-loopback"]`), `default=["bench-loopback"]`, RUSTFLAGS `--cfg feature=...`: all FAIL the guard (tree and/or marker). No defeat found.
- actions/checkout SHA 11bd719... = v4.2.2 and cargo-deny-action 3c63498... = v2.1.1 peeled tag (git ls-remote). All 7 cargo invocations use --locked. Job ids fmt/clippy/test/deny/release-guard match docs/ci-branch-protection.md. selftest and bench-loopback tests are in ci.yml:56-59.

## Blocking
None. Could not make a full `--gate` pass falsely: error-only, fetch-less, no-`fetch`-tool, stand-in script, wrong-arch, wrong-kind, marker-stripped bench, partial scenario set, <10 valid, overrides, --child-env, --smoke are all refused or INVALID. Unit MiB/kB and VmRSS/VmHWM usage is correct; boundedness with missing reference is INCOMPLETE.

## Non-blocking (ranked)
1. Doc/code mismatch, vacuous G7a: bench/scenarios.py:21 `min_bytes=lambda m: 0`, but docs/BENCHMARK.md section 5 says the floor is "1 byte ... confirms the request reached the server". Repro: a script printing `fetch-mcp 0 bench-loopback` that answers every tools/call with `isError:true,"too_large"` gets `g4a-50mib-cl valid_runs 3, PASS` (/tmp/fl.py). Whole gate still INVALID because 5mib-full fails, but the per-scenario line is meaningless. Fix the doc or assert >=1 byte / request seen.
2. docs/ci-branch-protection.md:7 says `test` "runs only cargo test --locked (default features); bench-loopback tests ... do not run yet". Contradicts ci.yml:55-57 and the table in the same file. Stale.
3. Early-stop check is an upper bound (bench/serve.py:_send, measure.py:169): a client reading only ~4.2 MB of 5.24 MB was counted valid in 1 of 6 runs (repro: `--child-env STANDIN_EARLY_STOP=4200000 --smoke --runs 6`). Slack scales with host socket buffers. Peak effect small but the valid-run rule is weaker than documented; consider a floor of size minus slack recorded, or compare client-side bytes.
4. measure.py:94-97 `native_aarch64` fails open if /proc/sys/fs/binfmt_misc is unreadable, and qemu-user reports `aarch64` via uname. A container under qemu-user without binfmt visibility could pass preflight. Add a cpuinfo/`/proc/self/maps` cross-check. Not reproduced (no aarch64 host).
5. Weak marker tests: src/lib.rs has a bench marker test but none for test-support; `default_build_has_no_markers` asserts nothing when a feature is on. CI `--features test-support` therefore runs 2 near-trivial tests. Guard self-test covers the markers, so risk is low. main.rs `--version` output is untested.
6. Ok-scenarios only check `isError` false and bytes served (measure.py:167); an empty-content success passes. Acceptable for a memory harness, note for E-2.
7. measure.py:129 matches any message with the awaited `id`, including a server-initiated request; fails closed (invalid sample) but could spuriously invalidate a valid rmcp run. Also exit 0 on ADVISORY_PASS is documented but a script checking only rc will misread it.
8. deny.toml is vacuous with zero dependencies (bans never exercised); dependabot.yml covers `/` only, not spikes/a1; spike clippy covers 5 feature sets, not `mimalloc` or `conv-none`.
9. Guard marker check depends on the marker being emitted only via version_line; a future cfg(feature) code path without a marker is caught only by the tree check (which is sound). Keep the tree check mandatory when A-3a lands.

## Unverified (and disclosure)
- GitHub Actions: workflow has never run (no hosted run, no required-check config). deny job's `arguments: --locked` after `check` and the action's behaviour are untested.
- cargo-deny and cargo-audit are not installed here; `deny` and advisories are unrun. docs/ci-branch-protection.md is honest that branch protection is NOT YET CONFIGURED and that cargo audit is not installed, but it does NOT say the workflow and cargo-deny have never been executed; add one line.
- Native aarch64: no gate, idle or peak figure exists; the real binary is a skeleton with no MCP server so measure.py has never driven the product. BENCHMARK.md sections 3 and 10 honestly mark the host as PLACEHOLDER and G0 as conditional.
