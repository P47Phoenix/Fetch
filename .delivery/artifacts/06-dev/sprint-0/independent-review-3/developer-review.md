# Developer independent review 3: Sprint 0 (D-7, E-1) at 5ccc4a6

Prior review folders were not read.

## Fresh-checkout proof
`git archive HEAD` extracted to $CLAUDE_JOB_DIR/tmp/fresh, CARGO_TARGET_DIR outside the repo. All exit 0:
- cargo fmt --check
- clippy --locked --all-targets -D warnings: default, bench-loopback
- spikes/a1 clippy, 5 feature sets from ci.yml
- cargo test --locked: default (2 tests), bench-loopback (3), test-support (2)
- python3 bench/selftest.py: SELFTEST PASSED
- scripts/check-release-features.sh: guard OK
- scripts/check-release-features.sh --self-test: self-test OK
Same tests and selftest also pass in the working repo. Verdict: a fresh checkout is green locally.

## Blocking
None.

## Non-blocking
1. cargo-deny was not run (not installed here; docs admit it). Deny job unproven: deny.toml has `[graph] all-features=false` and an empty dependency tree, so it is likely trivial, but the licences/sources config is untested. Hosted Actions have never run either, and the docs say so honestly.
2. `cargo fmt --check` covers only the root crate. spikes/a1 is not formatted (`cargo fmt --check` exits 1 there). Not a D-7 AC, but the AC covers spike clippy only.
3. Spike clippy covers 5 feature sets. mimalloc, conv-none, and http-hyper with other converters are not linted, though the Cargo.toml defines them. The AC says "passes clippy", so this is a partial cover.
4. The spike release profile lacks panic=abort and strip. It is a spike, so this is irrelevant to D-7, which pins the root profile only.
5. Guard `--features` path: tree check plus marker check are both exercised by the self-test with each forbidden feature. The marker check depends on main.rs printing `--version` markers, which is by design. Unit test coverage of the markers is thin (default 2 tests). The guard itself is well tested.
6. D-7 "required status checks" and the branch protection record are owner actions, not verifiable here. The docs correctly mark them as pending.
7. The exact `cargo build -p` release-build claim in D-7 is met via `-p $PKG` in the guard script.

## Report claims
revision-5-report.md claims verified true: spikes/a1/Cargo.lock committed and the archive loop passes, `--locked` present, tests 2/3/2, guard and self-test pass. Not checkable: actionlint (not re-run) and the deny job.

## E-1 / D-7 AC conformance
D-7: toolchain pinned to 1.94.1, Cargo.lock committed, action SHAs pinned, `publish=false`, licenses.private.ignore, release profile fixed, guard with self-test, hosted-only jobs. Conforms, except the deny run is unverified. E-1: the 50-URL and 10-URL lists are deferred per the owner and not flagged. Other E-1 items were not deeply audited beyond the harness self-test.
