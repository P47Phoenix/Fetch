# E-4 dev report: peak RSS and boundedness checks, G4a (Sprint 4, part 2)

Branch `sprint-4/convert-memory-gate`, draft PR #8. E-4 commits are prefixed `E-4:`. Measured CI run: **35557702662** (bench, `pull_request`, PR head 8b5b489; binaries report the merge commit acddaaeb3a9e5db467a936dcce87bb59b89597dc that GitHub checks out). Full tables, hosts and caveats: `docs/BENCHMARK.md` section 16.

## Result

**G4a: PASS on all four hosted cells in that run (arm64 gnu and musl, required; amd64 gnu and musl, also run).** Single run per cell. This is NOT the memory gate closing: G4b (A-5, A-6) is Sprint 5.

| cell | host CPU | idle shipped (10) | gating peak (40) | 50 MiB CL / chunked ratio (1.10) | g6-concurrent10 (recorded) |
|---|---|---|---|---|---|
| arm64 gnu | Neoverse-N2 | 3.91 | 5.38 | 0.923 / 1.006 | 8.18 |
| arm64 musl | Neoverse-N2 | 2.59 | 4.56 | 0.930 / 1.027 | 13.33 |
| amd64 gnu | Xeon Platinum 8573C | 4.62 | 6.16 | 0.949 / 1.000 | 8.85 |
| amd64 musl | EPYC 9V74 | 2.35 | 4.70 | 0.891 / 1.014 | 8.82 |

MiB, medians of 10 valid `--gate` runs, summary verdict PASS. Per-scenario peaks are in BENCHMARK.md section 16 (5 MiB page 4.32 to 5.81, gzipped 4.56 to 6.16, late-landmark 4.43 to 6.03). No target was changed or tuned. Local advisory run on the dev host before CI (Fedora x86_64, not evidence): gating peak 6.46 MiB.

## What was built

- `build.rs` (no dependency, no timestamps): embeds git commit and SHA-256 of Cargo.lock (SHA-256 written out; a unit test compares with `sha256sum`). `--version` = `fetch-mcp <ver> commit=<40 hex> cargo-lock=<64 hex> [marker]`. `git archive` builds print `commit=unknown`.
- `scripts/build-candidates.sh` fails unless shipped and bench `--version` identity equals `git rev-parse HEAD` and `sha256sum Cargo.lock` (passed in all four cells: "identity ok").
- `bench/measure.py`: `--gate` requires `--peer-binary` and refuses on missing, unknown or differing identity; `g6-concurrent10` implemented (10 concurrent calls in one process, verdict `RECORDED`, valid only if all 10 ok and at least 10 x 5 MiB served); JSONL header records identity and peer version.
- G4b scenarios stay defined and are refused as not implemented (self-tested); implementation is A-5/A-6.
- `bench/public_check.py` + advisory CI step: shipped vs bench fetch of one public HTTPS page.
- `bench.yml`: G4a + redirect-chain5 + g6 run with peer identity; `build.rs` added to trigger paths; job ids unchanged so `ci-branch-protection.md` required list is unchanged (text updated).
- Self-test extended (identity parse/mismatch/unknown, peer required, g6 RECORDED and non-gating, G4b refusal). Rust tests: `--version` identity in unit and integration tests.
- Docs: BENCHMARK read-first table, section 5 table, glossary, bounds text, new section 16; README status; EPICS E-4 status; sprint plan G4a result; ci-branch-protection.

## Shipped-vs-bench and allocator records (details in BENCHMARK section 16)

Idle delta (bench minus shipped): +0.13 (amd64 gnu), -0.06 (amd64 musl), 0.00 and 0.00 (arm64), bound 0.5 MiB: within. Size delta: +80 B (three cells), +144 B (amd64 musl). Public-host check: ratios 0.997 to 1.02, all under 40 MiB. Allocator: system allocator (glibc/musl malloc); no swap needed (ADR-005 criterion 5 not triggered; headroom rule not triggered); libc choice is not made here.

## Verification (clean `git archive HEAD` of the code commit)

fmt ok; clippy -D warnings x3 ok; `cargo test --locked` x3 ok (135/136/135 lib tests, integration 9/11/9); release build ok; release guard ok and `--self-test` OK; `cargo deny check`: advisories, bans, licenses, sources ok; `bench/selftest.py` PASSED; actionlint ok (run in the work tree, the archive has no `.git`). CI: ci and arm-bench success, bench-gate 4/4 success (see the PR).

## Honest gaps

1. Public-host cross-check used a ~2 MiB page (Wikipedia list), not 5 MiB: no ~5 MiB public HTML page is known. It is advisory in CI (network). The AC text asks for 5 MiB.
2. G4a AC text says "native aarch64 only"; ADR-007 adds amd64, which was also run and passed.
3. Single CI run per cell; spread seen (musl amd64 idle outliers 4.3 vs 2.35; g6 on arm64 musl bimodal 8 and 14 MiB). The recorded g6 range over all samples is 7.4 to 16.0 MiB. Default concurrency limit is 3, so g6 measures 3 in flight plus 7 queued.
4. The idle-delta bound is checked by reading the record, no script fails on it.
5. 1 MiB conversion p95 measured on gnu only (56.7 ms amd64, 59.9 ms arm64); no musl figure. `.local`/split-DNS resolution untested. So ADR-005 criterion 2 (libc choice) is not decided here.
6. Late-landmark holdback and conversion quality are untuned and the E-7 snapshot quality check has not run (A-4 report).
7. macOS `/usr/bin/time -l` reader still not built. Image not measured (D-2). TLS covered only by the public-host check.
8. `bench-gate` is still not a required check (owner decision).
