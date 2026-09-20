# Independent Developer review: Sprint 0 (D-7, E-1), branch sprint-0/spikes

Scope: code quality, correctness, AC conformance. Prior DoD reviews not read.

## Commands run (HEAD)
- cargo fmt --check: pass
- cargo clippy --locked --all-targets -D warnings (default, bench-loopback): pass
- cargo test --locked (default, bench-loopback, test-support): pass (bench-loopback runs 3 unit tests; others 2)
- scripts/check-release-features.sh: "guard OK"; --self-test: "self-test OK"
- python3 bench/selftest.py: SELFTEST PASSED
- cargo deny: NOT runnable here (cargo-deny not installed). deny.toml unverified locally.
- Action SHAs verified via gh api: checkout 11bd719... and cargo-deny-action 3c63498... exist (v2.1.1 tag is an annotated tag object; the pinned SHA is the peeled commit, plausible).

## Blocking

B1. D-7 AC "A-1 spike code passes clippy -D warnings" is not met and not checked in CI.
- spikes/a1 is a separate crate, outside the root package, so root clippy never covers it and ci.yml has no job for it.
- `cd spikes/a1 && cargo clippy --locked --all-targets -- -D warnings` fails to compile with default features (src/main.rs:38 `get_body` cfg'd out). With `--features http-reqwest,conv-htmd` it fails on clippy::needless_return at src/main.rs:93.
- The D-7 dev report never mentions this AC; its "clippy: pass" line covers only the root crate. Failure scenario: DoD/QA reads D-7 as complete while an explicit AC is silently unmet. Fix: fix or `#![allow]`-justify the spike, add a CI step for a defined feature set, or have the PO record a waiver.

B2. E-1 AC "50-URL curated set ... lists the URLs" (and the 10-URL smoke list) is NOT DONE (E-1 dev report; docs/BENCHMARK.md:108). The sprint-plan Sprint 0 exit line (35) requires "50-URL list" in the E-1 result, and A-3b/E-7 list E-1 (URL list) as a dependency. Failure scenario: E-7 (Sprint 2) starts with no list. The report is honest, but the story cannot be called Done against its AC; needs the list, or an explicit PO deferral recorded in the plan.

## Non-blocking

N1. bench/measure.py:104 (reader thread) `[self.q.put(l) for l in self.p.stdout] or self.q.put(None)`: the list of Nones is truthy whenever the server emitted at least one line, so the EOF sentinel is never queued. A server that crashes mid-fetch (after handshake) leaves `call()` blocked in `q.get` for the full 120 s timeout (queue.Empty, caught as an invalid sample). The "server closed stdout" branch (line 119) is unreachable after the first line. Effect: 10 crashes = 20 minutes of hang, and the failure reason is a bare queue.Empty. Use a plain for loop then put(None).

N2. bench/serve.py:44-55 + scenarios.py g4a-50mib-cl (`min_bytes=lambda m: 1`): the byte counter increments only after a successful write of the first 64 KiB block. If the client aborts on the Content-Length header before the server thread writes, the write raises BrokenPipe/ConnectionReset, sent stays 0, and a correct too_large run is classed "early stop" and invalid. Racy against a fast real server (Rust aborts at headers by design). With <10 valid samples the whole report is INVALID (flaky gate, not a false pass). Count header bytes or set the floor to 0 for this route.

N3. bench/measure.py:139-141 `text = json.dumps(res)` is dead; `outcome_ok` `"result" in r` is always true for non-error replies (harmless, misleading). measure.py:238-239 the `scen.get("kind") == "peak"` and idle branches are two identical appends and could be one.

N4. CI does not run `python3 bench/selftest.py` (the harness's only proof that it can fail) nor `cargo test --features bench-loopback`, although D-7 AC says E-8 unit tests run with that feature in hosted CI. The ci.yml comment and docs/ci-branch-protection.md defer it to E-8, yet the test `bench_build_version_carries_marker` (lib.rs:47) already exists and is unexercised in CI. Add one line to the `test` job now.

N5. scripts/check-release-features.sh: `set -e` is inert inside functions used in `||`/`if` contexts, so in self-test a failed `cargo build` is not fatal at the call site. It stays safe today because a missing binary makes check_markers return 2 and the wrong-reason grep then flags it, but this depends on incidental behaviour. Also `--features CSV` prints no "guard OK" on success (inconsistent with the default mode). Default mode uses `target` when CARGO_TARGET_DIR is unset, so a stale binary is possible only if cargo build silently fails, which set -e prevents in that path (verified by reading).

N6. Guard scope is by-design narrow: markers exist only for the two known features, so a future forbidden feature is caught by the tree allowlist only; a `--cfg 'feature="x"'` RUSTFLAGS bypass of the tree check is caught only if x has a marker. Documented, but the lib.rs marker table has no compile-time link to FORBIDDEN_FEATURES in the script.

N7. E-1 dev-report and bench/fixtures.py: gzip fixture sha256 depends on the zlib build; selftest's "regenerate byte-identical" check and `measure.py`'s hash gate will fail on a runner whose zlib differs (e.g. the aarch64 runner) even though the data is fine. Also bench/fixtures/ is gitignored, so measure.py refuses to run on a fresh checkout until `fixtures.py generate` is run (documented in the module docstring, not in measure.py's error text).

N8. Scope creep (minor, mostly documented): E-1 delivered a full working harness (measure/serve/scenarios/selftest, ~550 lines) which is E-2 scope (5 pts). Not harmful and tested, but E-2 estimates and the Sprint 0 capacity claim should be adjusted. `.gitignore` edit and `docs/ci-branch-protection.md` are D-7-adjacent and acceptable.

N9. Unverifiable claims: `cargo deny` pass and all hosted-CI results (D-7 report says "NOT RUN"); required-status-check configuration is "NOT YET CONFIGURED" (docs/ci-branch-protection.md), so the D-7 AC "are required status checks" is an owner action, not done. aarch64 idle/peak numbers: none exist; the G0 recommendation is preliminary from x86_64 spike data (E-1 dev report says so).

N10. Cargo.toml/src: no defects found. `publish = false`, profile pinned (opt-level "s", lto, codegen-units 1, panic abort, strip), lockfile committed, toolchain pinned to 1.94.1, features gated correctly (cfg on array literals). `--version` marker contract matches the guard and harness substring checks.
