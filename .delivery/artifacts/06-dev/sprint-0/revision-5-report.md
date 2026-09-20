# Sprint 0 revision 5 report

1. BLOCKER B1 fixed: removed `Cargo.lock` from spikes/a1/.gitignore and committed spikes/a1/Cargo.lock (2292 lines, acceptable; makes spike results reproducible). Proof: `git archive HEAD | tar -x` into a temp dir, then the exact CI loop (`cargo clippy --locked --all-targets --features "$f" -- -D warnings` for all 5 feature sets) passed there.
2. G7a min_bytes: kept at 0 (architecture 11.2). No code change. BENCHMARK.md now says the floor is ZERO, that a fetch-less/error-only stub passes the 50mib-cl line alone (reproduced by QA), and why the gate is still not falsely passed (5 MiB scenarios have byte floors, handshake requires a real `fetch` tool). No selftest added since code is unchanged.
3. docs/ci-branch-protection.md test-check line corrected (3 feature configs + selftest). Added plain statements in BENCHMARK.md and ci-branch-protection.md that hosted Actions, cargo-deny and cargo-audit have never been run.
4. Removed dead `refuse` FAIL branch (measure.py). Fixed counter race (Dev N4): serve.py counters now carry a generation token bumped by reset(); handlers capture the generation at request start and stragglers from a prior sample no longer count. Removed duplicate `import platform` in selftest.py (Dev N7 part).

Skipped (reason): Dev N2/QA7 exit 0 on smoke (documented, advisory); N5 stderr capture, N6 runner placeholder (E-2 scope); N7 macOS KeyError (Linux only documented); N8 <1 s assertion looseness; N10 spike quality; QA3 early-stop upper bound, QA4 binfmt fail-open (needs aarch64 host to test), QA5 marker tests, QA6 ok-scenario content check, QA8 deny/dependabot/spike feature sets, QA9 (all E-2 or later, not cheap).

Verification: fmt, clippy -D warnings (default, bench-loopback, spikes/a1 from clean archive), cargo test --locked (default 2, bench-loopback 3, test-support 2), guard and --self-test, bench/selftest.py, actionlint all pass.
