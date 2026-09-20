# arm-bench report: A-1 spike on native aarch64 (GitHub-hosted arm64)

Status: ADVISORY spike evidence for gate G0. Not a `--gate` run, not the product.

## What ran
- Workflow `.github/workflows/arm-bench.yml` (job id `bench`, not a required check), runner `ubuntu-24.04-arm`, read-only permissions, pinned actions, no secrets.
- Build: `spikes/a1`, `--release --locked --features http-reqwest,conv-lolhtml` (reqwest + lol_html streaming), gnu (native) and musl (`aarch64-unknown-linux-musl` via apt `musl-tools`/`musl-gcc`; it worked cleanly, no zigbuild).
- Harness: `bench/measure.py` (E-1), no `--gate`, 10 runs, default 30 s idle settle, fresh process per sample, harness starts its own loopback fixture server. Scenarios: `idle` (kind shipped), `g4a-5mib-full`, `g4a-5mib-gz` (kind bench). The harness self-test ran first.
- Identity handling: the spike has no `--version`, and the harness requires one (bench needs the `bench-loopback` marker). Two tiny `exec` wrapper scripts answer `--version` ("a1-spike 0.0.0 (A-1 stand-in, NOT fetch-mcp)", plus `bench-loopback` for the bench one) and otherwise `exec` the spike, so the measured pid is the spike. No harness rule was weakened; `--gate` would (correctly) refuse this stand-in.
- Runs: PR run https://github.com/P47Phoenix/Fetch/actions/runs/35478468746 (success, both legs) and dispatch run https://github.com/P47Phoenix/Fetch/actions/runs/35478804725 (success, both legs). Results (JSONL, platform facts, exit codes) are in `.delivery/artifacts/06-dev/sprint-0/arm-bench/{pr-run,dispatch-run}/`.

## Platform facts (both runs)
Azure VM, Linux 6.17.0-1022-azure aarch64, Neoverse-class core (`CPU part 0xd49`), 4 vCPU, MemTotal 16330124 kB, PAGESIZE 4096, glibc 2.39, Ubuntu 24.04.5, image ubuntu24-arm64 20260907.118.1, THP `madvise`, no qemu aarch64 binfmt. Different runner instances per run (names 1000003301 / 1000003317). gnu spike sha256 48aa14c6...fe3c7 (musl hash in the platform file).

## Medians (MiB = 2^20; valid/runs = 10/10 for every scenario, 0 invalid)
| scenario | metric | gnu PR | gnu dispatch | musl PR | musl dispatch | target |
|---|---|---|---|---|---|---|
| idle | VmRSS | 3.66 | 3.66 | 2.09 | 2.09 | <= 10 |
| g4a-5mib-full | VmHWM | 16.29 | 16.31 | 10.49 | 10.49 | <= 40 |
| g4a-5mib-gz | VmHWM | 7.93 | 7.93 | 5.46 | 5.46 | <= 40 |

Per-run spread is tiny (idle 3740-3744 kB gnu; 5 MiB full 16664-16752 kB gnu). Run-to-run difference across the two runners is about 0.02 MiB.

## Answer
On this hosted ARM VM, for the spike: idle <= 10 MiB YES (3.66 gnu / 2.09 musl), peak <= 40 MiB YES (worst scenario 16.31 gnu / 10.49 musl). Summary verdict is `ADVISORY_PASS`, which by BENCHMARK.md is not a result and never satisfies a target.

## Caveats
- The spike is not the product: no SSRF layer, no real pagination or `too_large` path, and its `fetch` is a minimal implementation. The product will use more memory. Its own gate run (fetch-mcp binaries, `--gate`) is still required.
- The 50 MiB boundedness scenarios were not run (only the requested 5 MiB scenarios).
- `g4a-5mib-gz` peaks lower than `full`; the spike probably does not decompress (no gzip feature), so it is not evidence about decompression cost.
- Cloud VM (Azure, Neoverse, 4 vCPU, 16 GB) vs the Pi 5 target: different core, memory system and kernel. Page size is 4 KiB here; Pi OS and some ARM kernels use 16 KiB, which can raise RSS. Only two runner instances were sampled, so cross-runner variance is barely characterised.
- Advisory path uses wrapper scripts for `--version`; the ELF identity check is a `--gate`-only rule and was not exercised.
