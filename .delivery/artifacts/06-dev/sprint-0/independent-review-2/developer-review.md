# Sprint 0 Developer review (independent, round 2) - HEAD 84ec595

Scope: D-7, E-1 as Developer. E-1 50-URL list deferral not flagged. Did not read dod/ or independent-review/.

## Commands run (all local, HEAD)
- cargo fmt --check: clean
- clippy --locked --all-targets -D warnings: default, bench-loopback: clean; spikes/a1 x5 feature sets: clean (only because a local, gitignored Cargo.lock exists, see B1)
- cargo test --locked: default 2, bench-loopback 3, test-support 2 pass
- scripts/check-release-features.sh and --self-test: OK
- python3 bench/selftest.py: PASSED (also PASSED from a clean `git archive HEAD` copy)
- actionlint: clean

## Blocking

### B1. CI spike clippy step fails on a fresh checkout (spikes/a1/.gitignore:4, .github/workflows/ci.yml:39-46)
spikes/a1/.gitignore ignores `Cargo.lock`, and `git ls-files spikes/a1` shows no lock file. The CI step runs `cargo clippy --locked` in spikes/a1. Reproduced: `git archive HEAD | tar -x -C /tmp/fx; cd /tmp/fx/spikes/a1; cargo metadata --locked --offline` -> "error: cannot create the lock file ... because --locked was passed". So the `clippy` job (a required check per docs/ci-branch-protection.md) is red on every PR from the first run, or, if the flag is dropped, resolves unpinned spike deps (contradicts EPICS.md:455 "Cargo.lock is committed, all CI commands use --locked"). Revision-4 report item 1 ("All five sets pass locally") is true only because of the ignored local lock; the D-7 AC "spike passes clippy -D warnings" is therefore not demonstrated in CI. Fix: remove `Cargo.lock` from spikes/a1/.gitignore and commit the lock (also makes spike results reproducible).

## Non-blocking

N1. bench/measure.py:230 `refuse()`: `"REFUSED" if code != 1 else "FAIL"` is dead (refuse is only called with 2 or 3). Revision-4 "dead code removed (Dev N3)" is not fully true. Trivial cleanup.

N2. bench/measure.py:274 (`elif vals` branch) with `--smoke` and <10 runs: scenario verdict "PASS" and process exit 0 even though the docstring (line 8-9) says exit 0 means "all targets met and report VALID". Summary verdict is ADVISORY_PASS and record gating=false, so not a gate false-pass, but scripts keyed on exit code can misread it (report skipped this as QA N1).

N3. bench/scenarios.py:20 (g4a-50mib-cl `min_bytes=0`): a server that returns a canned `too_large` error for every call, with no network activity, satisfies G7a and (with a small stub RSS) boundedness. This is mandated by architecture 11.2 (abort on Content-Length), so not a defect of the harness; the shipped G4a still has the 5 MiB scenarios' byte floors as a partial guard. Worth a note in BENCHMARK.md if not there.

N4. bench/serve.py:75 `_send` counter plus measure.py `srv.reset()`: a straggler handler thread from the previous sample (client aborted, server still writing into kernel buffers) can add bytes to the next sample's counter after reset, inflating `bytes_sent` and masking an early-stop for chunked too_large (floor = CAP). Only matters for a broken client, low probability; per-sample route token or per-connection counters would fix it.

N5. bench/measure.py `sample()` idle path: no check that the server still responds after the settle sleep; only a crash (zombie, no VmRSS -> KeyError -> invalid) is caught. stderr is DEVNULL (line ~114), so failure diagnosis for a real server is hard. Consider capturing a stderr tail into invalid_reasons.

N6. bench/measure.py:62 `host_record()` emits `"runner": "PLACEHOLDER: ..."` into every JSONL header, including gating runs. Provenance field is a literal placeholder; fill from an env/arg before gate evidence is collected (E-2).

N7. bench/selftest.py: `import platform` duplicated (line ~85 and top); `measure.ELF_MACHINE[host]` raises KeyError on non-x86_64/aarch64 hosts (e.g. macOS arm64 reports "arm64"), so the self-test crashes there rather than skipping. Docs say Linux only, so low.

N8. Revision-4 report claims the EOF fail-fast is "<1 s"; selftest only asserts <30 s (selftest.py, crash check). Behaviour is right (sentinel), assertion is looser than the claim.

N9. scripts/check-release-features.sh: guard scans only the host-target binary and the tree of the root package; the tree check trusts `cargo tree` output format (parse failure is handled fail-closed, good). `--features` mode uses relative `target` dir; fine. No defect found; self-test has real positive controls (each forbidden feature fails for both tree and marker reasons).

N10. spikes/a1/src/main.rs (scope note, spike quality, not shipping): `skip` counter uses usize with `-= 1` in the end-tag handler (underflow panic in debug if an end tag fires without a start increment); `out.len()` (bytes) compared to `start+max` (chars). Acceptable for a throwaway spike, but "passes clippy" is the only stated bar.

## Verified true
- Guard and self-test genuinely fail on forbidden features, unknown markers and unparseable tree output.
- Handshake requires JSON-RPC results and a `fetch` tool; error/no-tool servers give INVALID (self-tested).
- --gate override, incomplete-set and identity refusals work; ordering (exit 3 last among gate rules) as documented.
- actionlint clean; actions SHA-pinned; permissions contents: read; persist-credentials false.
- src/ is minimal skeleton, no scope creep; Cargo.toml release profile matches spike.
