# Sprint 0 revision 4 report (Developer)

Source: independent developer and QA reviews (.delivery/artifacts/06-dev/sprint-0/independent-review/). User decisions: fix blockers and cheap items; E-1 50-URL list DEFERRED, owner project owner.

## Done
1. spikes/a1 clippy (Dev B1): decision = fix and gate in CI (D-7 AC says A-1 spike code passes clippy -D warnings). Fixed needless_return (documented allow on `convert`), added a no-backend `get_body` fallback and a scoped dead_code allow so default features compile. CI `clippy` job now lints spikes/a1 for default plus four backend feature sets (job id unchanged). All five sets pass locally.
2. Handshake (QA B1): initialize and tools/list must return a JSON-RPC result; tools/list must contain `fetch`; tools/call error replies are protocol failures. Selftests: error-only server and no-fetch-tool server give INVALID, exit 2.
3. Binary identity (QA B2): `check_identity` under `--gate` (after override rules, so exit 3 ordering is unchanged): ELF e_machine == host arch, `--version` starts `fetch-mcp `, shipped has no FETCH_MCP_MARKER_ bytes, bench has the BENCH_LOOPBACK marker bytes. Advisory/selftest flows unchanged (stand-in still works). Selftests (function level, since off-aarch64 --gate exits 3 first): stand-in, script-with-good-version, wrong arch, foreign version, marker present/absent, big-endian. Documented in docs/BENCHMARK.md.
4. EOF sentinel (Dev N1): plain reader loop then put(None); selftest proves a mid-fetch crash fails in <1 s with "server closed stdout". Racy counter (Dev N2): serve.py counts before write; g4a-50mib-cl floor set to 0 per architecture 11.2.
5. CI (Dev N4, QA N5): `test` job runs `cargo test --locked` default, bench-loopback, test-support, and `python3 bench/selftest.py`. No new actions. docs/ci-branch-protection.md updated; job ids unchanged.
6. 50-URL list recorded DEFERRED (owner: project owner) in docs/EPICS.md (E-1 AC, E-7 prerequisite), docs/BENCHMARK.md section 9, and sprint-plan Sprint 0 exit line. Stale fixture-CA mentions removed from docs/EPICS.md D-7 ACs (former L419, L457). Remaining fixture-CA mentions (L20, L65, BENCHMARK L101) are the historical decision record and were left.
7. Extras: dead code removed in measure.py (Dev N3); fixture-missing refusal now hints `fixtures.py generate` and the zlib caveat (Dev N7).

## Skipped
- E-1 50-URL list (deferred by user; Dev B2).
- QA N1 advisory exit 0 (changing exit codes alters the documented contract; ADVISORY_PASS verdict is explicit); QA N2 invalid-fraction cap; QA N3 gate PASS path needs native aarch64; QA N4 bench gate unreachable is by design.
- Dev N5/N6, QA N7/N8 guard script hardening: by design or Sprint D-2/D-3 scope; Dev N7 gzip hash portability needs an aarch64 run; Dev N8 scope note (E-2 estimate adjustment is a PO action); Dev/QA N9 and QA N6 unverifiable here (cargo-deny not installed, hosted CI not run, branch protection is an owner action).

## Verification (all pass)
cargo fmt --check; clippy -D warnings default and bench-loopback; spikes/a1 clippy x5; cargo test --locked default (2), bench-loopback (3), test-support (2); guard OK and --self-test OK; bench/selftest.py PASSED; actionlint clean.
