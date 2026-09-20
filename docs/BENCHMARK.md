# Benchmark protocol and absolute targets (story E-1)

**What is this?** This file explains how we measure how much memory the `fetch-mcp` program uses, and the two limits it must stay under: 10 MiB when idle and 40 MiB at its peak. **Who needs it?** Anyone who runs the benchmark scripts in `bench/`, or who reads a memory report and wants to know what the numbers mean. **What to do first:** read the Glossary below, then follow the Quickstart (step 1 takes about 1 minute and needs no Rust).

Status: E-1 result, Sprint 0. Memory targets: 10 MiB idle (VmRSS), 40 MiB peak (VmHWM), 5 MiB fetch cap. Other documents (PRD, EPICS, architecture, config defaults, reports) refer to this file for the unit, targets, scenarios and method. The test tools live in `bench/` (self-test: `python3 bench/selftest.py`). Story E-2 builds the full fixture set and CI on top of them.

## Read this first: what has and has not been checked

Be careful not to read more into this document than it says.

| Item | Status |
|---|---|
| Hosted GitHub Actions running this repository's workflow | Run once, on pull request #2. All five required checks (`fmt`, `clippy`, `test`, `deny`, `release-guard`) passed on commit 740c0fb (run 35470977286). One green run is not a trend. |
| `cargo-deny` (the `deny` job) | Ran once, in that same hosted run, and passed. |
| `cargo-audit` | Never run (not installed on the dev host). |
| The benchmark scripts | Only run against the stand-in and the skeleton binary. Never against a real MCP server. |
| The `fetch-mcp` binary | A skeleton. It has no MCP server yet. |
| Native aarch64 measurement | Advisory only: the A-1 spike (not the product) was measured on a GitHub-hosted arm64 runner, no `--gate` (section 11). No product measurement, and no `--gate` run, exists yet. |
| Native amd64 measurement | NOT YET MEASURED on a hosted amd64 runner (the earlier A-1 x86_64 figures predate this protocol). |
| Branch protection on `main` | Not configured yet (see `docs/ci-branch-protection.md`). |

## Glossary

Each term is explained here once. Later sections use the short form.

| Term | Plain meaning |
|---|---|
| MCP | Model Context Protocol. A way for an AI tool (a "client") to talk to a helper program (a "server") such as `fetch-mcp`. |
| Skeleton | A program with the right name and shape but almost nothing inside. `fetch-mcp` today only prints its version. It has no MCP server yet. Do not register it in a real MCP client. |
| Stand-in | `bench/standin_mcp.py`. A fake server written in Python, used only to test the benchmark tools. It is NOT the product. Its memory figures say nothing about the product. |
| MiB | Mebibyte, the unit for every memory figure in this project: 1 MiB = 2^20 = 1,048,576 bytes. |
| kB | Kilobyte as Linux reports it in `/proc`: 1 kB = 1,024 bytes (strictly a KiB). So 10 MiB = 10,240 kB and 40 MiB = 40,960 kB. |
| VmRSS | "Resident set size". How much RAM the process holds right now. Linux shows it as the `VmRSS` line in `/proc/<pid>/status`. We use it for the idle target. |
| VmHWM | "High water mark". The most RAM the process ever held so far. Same file, `VmHWM` line. We use it for the peak target. |
| Idle | The server has started, said hello, listed its tools, and then waited 30 seconds doing nothing. |
| Peak | The highest memory use while the server fetches one page. |
| Settle | The 30 second wait before the idle reading, so start-up noise dies down. Flag: `--settle`. |
| Median | Sort the results and take the middle one. With 10 runs it is the average of the 5th and 6th. One odd run cannot change it much. |
| Fixture | A test file with fixed content, so every run sees the same input. Made by `bench/fixtures.py`. |
| Cap | The largest page the server may read: 5 MiB (5,242,880 bytes). Bigger pages give a `too_large` error. |
| Loopback | Talking to your own machine (`127.0.0.1` or `::1`) instead of the internet. The benchmark's test web server runs this way. |
| Fetch | What the server does: download a web page and turn it into text. |
| SSRF | Server-side request forgery. Tricking a server into fetching addresses it should not, such as internal ones. The shipped binary blocks these, which is why it cannot reach the loopback test server (see section 4). |
| Shipped binary | The real release build. Blocks loopback. Used for the idle measurement. |
| Bench binary (`bench-loopback`) | A special build that is allowed to reach loopback so the benchmark can fetch from the local test server. Never released. |
| Marker | A fixed piece of text (`FETCH_MCP_MARKER_...`) that a special build prints in `--version` and carries inside the binary. Checks look for it to tell builds apart. |
| Scenario | One named test case, for example `idle` or `g4a-5mib-full`. |
| Gate | A point in the project plan where we must take a memory verdict: G0 (end of Sprint 0), G4a (end of Sprint 4), G4b (end of Sprint 5). Not the same as the scenario IDs G1 to G7 (see "Gate names versus scenario IDs" below). |
| Gating run | A run made with `--gate`. Only a gating run can prove a target is met. |
| Advisory run | Any run without `--gate`, including `--smoke` runs. Useful for testing the tools. Proves nothing about a target. |
| Smoke run (`--smoke`) | A quick run that may use fewer than 10 samples. Never valid for a memory claim. |
| ADVISORY_PASS | The summary word for a passing advisory run. It is NOT a result. Only a `--gate` run with summary `PASS` counts. |
| PASS / FAIL | PASS: the numbers are within the target. FAIL: a target was missed. |
| INVALID | The run does not count. Usually fewer than 10 valid samples, or a failed handshake. (A fixture hash mismatch is not INVALID. It is a refusal, exit 2.) |
| INCOMPLETE | A 50 MiB scenario has no valid 5 MiB reference run in the same run (see "Boundedness" in section 5). Exit 2. Never a pass. A `--gate` run that leaves out required scenarios is different: it is refused (exit 2, reason `incomplete`) before any measuring. |
| REFUSED | The script declined to start, for example because you asked `--gate` on the wrong machine. |
| Handshake | The opening exchange: the client sends `initialize`, then `tools/list`, and the server must answer with real results, including a tool named `fetch`. |
| JSON-RPC, stdio | The message format (JSON-RPC) and the pipe (the program's standard input and output) the client and server use to talk. |
| JSONL | "JSON lines": one JSON record per line of output. |
| aarch64 / arm64 (ARM) | The 64-bit ARM CPU type. Two names for the same thing (`linux/arm64` is the container platform name). A gating run for this platform must be on a real (native) aarch64 machine. |
| amd64 (x86_64) | The 64-bit Intel/AMD CPU type (`linux/amd64`). Gated on its own native runner, never averaged with arm64. |
| Platform | One CPU and OS pair the release image is built for: `linux/amd64` or `linux/arm64`. Each is measured on its own and never compared with the other. |
| Image, digest | The release artifact is a container image (ADR-007). Its digest (`sha256:...`) names exact bytes. A result is keyed by platform and digest. |
| Hosted runner | A short-lived GitHub Actions virtual machine: `ubuntu-24.04` (amd64) or `ubuntu-24.04-arm` (arm64). There is no self-hosted runner. |
| QEMU | Software that pretends to be another CPU. Its memory figures are not trusted, so QEMU never gates. |
| ELF | The Linux program file format. The harness reads its header to see the CPU type. |
| gnu, musl | Two versions of the C library the binary can be built with. Both are measured. |
| p95 | 95% of results are at or below this value. Timings report it only with 20 or more valid samples (nearest-rank). |
| Timings | Durations the harness records next to memory (section 12). Recorded, not gated. |
| ready_ms | Time from starting the process to its first valid `initialize` answer. Evidence only; the PRD 250 ms readiness figure is not checked by the harness. |
| tools_list_ms, first_byte_ms, fetch_ms, total_ms | Other recorded durations, all in milliseconds (section 12). |
| OQ-n | An open question in the project plan, for example OQ-7 (licence). |
| E-1, E-2, A-1 ... | Story IDs in `docs/EPICS.md`. |

## Quickstart (contributors)

Do these steps in order. Steps 1 to 4 work on any Linux machine and need no Rust for step 1 and step 4. Step 5 only works on a native host of the platform under test (a GitHub-hosted runner in CI).

**Before you start you need:**

- Linux. macOS is not supported yet (E-2).
- Python 3.8 or newer. The standard library is enough.
- A Rust toolchain, only if you build the real binary (step 3).

### Step 1. Check that the test tools work (about 1 minute)

```
python3 bench/selftest.py
```

Success: the last line reads `SELFTEST PASSED`.

Why: this proves the benchmark tools can pass and can fail. It makes its own fixtures. It runs against `bench/standin_mcp.py`, a stand-in that is NOT the product, so its memory figures are not product measurements.

### Step 2. Generate the fixtures (once, before any real run)

```
python3 bench/fixtures.py generate
```

Success: it prints `generated in <your path>/bench/fixtures`.

Why: `measure.py` refuses to run if `bench/manifest.json` is missing or its hashes do not match the fixture files.

### Step 3. Build the binary you want to measure (skip for the stand-in)

| Kind | Command |
|---|---|
| Shipped (the real release build) | `cargo build --release --locked -p fetch-mcp` |
| Bench (allowed to reach loopback) | `cargo build --release --locked -p fetch-mcp --features bench-loopback` |

Success: the build ends with a `Finished` line and the file `target/release/fetch-mcp` exists. Later steps assume you did this.

**Both builds write the same file**, `target/release/fetch-mcp`. Each build overwrites the last. Rebuild for the kind you are about to measure, or copy or rename the file after each build (for example to `target/release/fetch-mcp-shipped` and `target/release/fetch-mcp-bench`) and pass that path to `--binary`.

### Step 4. Try a worked example (advisory smoke run on the stand-in)

```
python3 bench/measure.py --binary bench/standin_mcp.py --binary-kind bench --child-env STANDIN_BENCH=1 --scenario g4a-5mib-full --smoke
```

Why `STANDIN_BENCH=1`: a bench-kind binary must show the marker, and this variable makes the stand-in show it.

Success: it prints three JSON lines (a `host` line, one `scenario` line, then a final `summary` line) and exits with code 0. It takes a few seconds. The `summary` line has `"verdict": "ADVISORY_PASS"`.

**Important: this success is not a result.**

- Without `--gate`, the summary verdict is `ADVISORY_PASS` or `FAIL` (or `INVALID` or `INCOMPLETE`, as in the glossary). It is never `PASS`.
- Exit 0 with `ADVISORY_PASS` (any run without `--gate`, including every `--smoke` run) never satisfies a target.
- Only a `--gate` run with summary verdict `PASS` counts.
- A per-scenario line may say `"verdict": "PASS"` in an advisory run. Ignore it. Only the `summary` verdict is authoritative.
- The exit codes are in the table in section 6.

**Current limitation.** The `fetch-mcp` binary is a skeleton until story A-2 (the stdio server with the `fetch` tool schema) lands. A real `measure.py` run against it fails the handshake and exits with code 2. Only the self-test and the stand-in are runnable now. Also, story E-8 says a bench build must print its marker to stderr at start-up. That is not built yet. Today the marker is only printed by `--version`.

### Step 5. Gating run (native host only)

```
python3 bench/measure.py --binary target/release/fetch-mcp --binary-kind shipped --scenario idle --gate
```

Why it is strict: this is the run that counts, so the script refuses anything that could bend the result.

`--gate` refuses all of these:

- `--smoke`
- target overrides (`--idle-target-mib`, `--peak-target-mib`)
- `--settle` other than 30
- `--parallel-idle` above 1
- any `--child-env`. The allow-list is empty, so `GLIBC_TUNABLES`, `FETCH_*`, `LC_ALL`, `PATH` and the rest are all refused.

`--gate` also needs the complete set of scenarios for the binary kind:

- A shipped binary needs `idle`.
- A bench binary needs every scenario of the G4a group (and/or the G4b group) it touches. A partial run is refused: exit 2, reason `incomplete`.

Those two refusals (override and incomplete set) come before the native-aarch64 check (exit 3). So they behave the same on any host. The binary identity refusal below comes after it, so it is reached only on aarch64. Anywhere else a `--gate` run stops at exit 3 first. A bench binary under `--gate` needs a real `bench-loopback` build (its `--version` marker). The stand-in with the environment variable trick does not work.

**Binary identity check.** With `--gate` only, after the override rules and before any sample, the script checks that the file really is what you say it is:

1. It must be an ELF file whose CPU type (`e_machine`) matches the host (aarch64 for a gating run).
2. `--version` must start with `fetch-mcp `.
3. A `shipped` binary must contain no `FETCH_MCP_MARKER_` text.
4. A `bench` binary must contain `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1`.

On the aarch64 runner, a stand-in script, a wrong-architecture ELF or a binary with its marker stripped is refused (exit 2, reason `binary identity`). Without `--gate` (advisory and self-test runs) only the `--version` marker rules apply, so the stand-in still works.

**Handshake check.** `initialize` and `tools/list` must both return a JSON-RPC `result`. An `error` reply is an invalid sample, so a server that only sends errors gives INVALID (exit 2). `tools/list` must contain a tool named `fetch`. A server that closes its output part-way through fails that sample at once.

### What can go wrong

| Symptom | What it means | Fix |
|---|---|---|
| Self-test does not end with `SELFTEST PASSED` | A benchmark tool is broken, or Python is older than 3.8. | Read the last failing line above it. Check `python3 --version`. |
| Refusal about `bench/manifest.json` or a fixture hash (exit 2) | The fixtures are missing or changed. | Run `python3 bench/fixtures.py generate`, then try again. |
| Exit 2, `INVALID`, `RuntimeError: server closed stdout` when you point it at `target/release/fetch-mcp` | The skeleton has no MCP server yet, so the handshake fails. This is expected today. | Nothing to fix. Use the self-test and the stand-in until A-2 lands. |
| Exit 2, refused, reason `binary identity` (only on aarch64; elsewhere you get exit 3 first) | The file is not the right kind: a script, a wrong-CPU ELF, or the wrong marker. | Rebuild with the command for the kind you passed (step 3). |
| Exit 2, refused, reason `incomplete` | A `--gate` bench run left out scenarios of its group. | Run the whole group (for example every G4a scenario). |
| Exit 2, `INCOMPLETE` in a 50 MiB scenario | Its 5 MiB reference scenario has no valid result in the same run. | Include `g4a-5mib-full` in the same run. |
| Exit 2, refused, `--gate` override | You used `--smoke`, a target override, another `--settle`, `--parallel-idle` above 1, or `--child-env`. | Remove the flag. |
| Exit 3, `--gate` off native aarch64 | The script today only accepts a native aarch64 host for `--gate` (an amd64 gate mode is not built yet). | Run on the hosted arm64 runner. Other machines can only do advisory runs. |
| Exit 0 but the verdict says `ADVISORY_PASS` | The run was not a `--gate` run. It is not a result. | Do the gating run in step 5 on a native host. |
| Exit 1 | A target was missed. | The numbers are real. Investigate the memory use; do not change the target. |
| `--runs` below 10 is refused | Fewer than 10 samples are only allowed with `--smoke`. | Use the default 10, or add `--smoke` for a non-counting test. |

### Gate names versus scenario IDs

G0, G4a and G4b are gates (points in the plan where a memory verdict is taken). G1 to G7 are the scenario IDs from architecture 11.1. The harness IDs in the table in section 5 (for example `g4a-5mib-full`) name the gate and the scenario together.

## 1. Unit definition (stated once)

The unit is MiB (mebibyte) everywhere: 1 MiB = 2^20 = 1,048,576 bytes. This holds for the targets, the fixtures and the fetch cap alike. Linux `/proc` reports kB, which is really KiB (1,024 bytes), so 10 MiB = 10,240 kB and 40 MiB = 40,960 kB. The 5 MiB fetch cap = 5,242,880 bytes.

## 2. Absolute targets

| Target | Metric | Limit | Rule |
|---|---|---|---|
| Idle | VmRSS after `initialize` + `tools/list` + 30 s settle | <= 10 MiB (10,240 kB) | median of 10 valid runs, shipped binary |
| Peak | VmHWM after the fetch returned | <= 40 MiB (40,960 kB) | max over gating scenarios of per-scenario medians, each of 10 valid runs, `bench-loopback` binary |
| Boundedness | 50 MiB scenarios peak | <= 1.10 x the 5 MiB full-read peak | medians |

"Boundedness" means memory must not grow with page size: a 50 MiB page may use at most 10% more than a 5 MiB page.

There is no tolerance on the release gate (E-3 and E-5). E-6 CI is a tripwire: the absolute target still applies, plus a 10% regression bound against the stored last-main baseline. Gates: G0 (end of Sprint 0), G4a (end of Sprint 4), G4b (end of Sprint 5); definitions in architecture 11.0. Targets are not lowered.

## 3. Benchmark hosts (per platform)

Per ADR-007 the memory gates run on GitHub-hosted native runners, one per platform: `ubuntu-24.04` for `linux/amd64` and `ubuntu-24.04-arm` for `linux/arm64`. There is no self-hosted runner. A hosted runner is a cloud VM, not the author's Raspberry Pi 5 (see the caveats in section 11). The harness records these in a `kind: host` JSONL line: machine, kernel, CPU, MemTotal (total RAM), page size, THP (transparent huge pages), load average, cgroup limit (a memory cap set by the system), container flag, OS. The CPU speed policy (governor) is not controlled on a hosted VM and is not a gating rule.

Host facts per platform. Compare figures only within one platform and only for the same page size.

| Field | linux/arm64 (`ubuntu-24.04-arm`) | linux/amd64 (`ubuntu-24.04`) |
|---|---|---|
| Measured? | Yes, advisory spike only (2 runs, section 11) | NOT YET MEASURED |
| VM / OS | Azure VM, Ubuntu 24.04.5, image `ubuntu24-arm64` 20260907.118.1 | NOT YET MEASURED |
| Kernel | Linux 6.17.0-1022-azure aarch64 | NOT YET MEASURED |
| CPU | Neoverse-class core (`CPU part 0xd49`), 4 vCPU | NOT YET MEASURED |
| RAM (MemTotal) | 16,330,124 kB | NOT YET MEASURED |
| Page size (`getconf PAGESIZE`) | 4096 | NOT YET MEASURED |
| THP | `madvise` | NOT YET MEASURED |
| glibc on the host | 2.39 | NOT YET MEASURED |
| qemu binfmt handler | none for aarch64 | NOT YET MEASURED |

Only two arm64 runner instances have been sampled, so how much the hosted VMs differ from one another is barely known.

Native procedure (per platform):

1. Preflight, run twice. On the host: `uname -m` prints the platform's CPU (`aarch64` or `x86_64`) and there is no qemu handler for it in `/proc/sys/fs/binfmt_misc`. Inside the container: `uname -m` must equal the platform under test (`aarch64` for `linux/arm64`, `x86_64` for `linux/amd64`), otherwise the run is invalid. The measurement job must not use `docker/setup-qemu-action`. The `--gate` mode of `bench/measure.py` checks the host part today and refuses with exit 3 otherwise.
2. Build the per-platform image on its native runner, then pull the tested manifest by digest, select the platform, and check that the resolved platform digest is the one that was built.
3. Run `python3 bench/measure.py --gate --binary <bin> --binary-kind shipped|bench --scenario ...` against the server process (see the next subsection).
4. Commit the JSONL output under the report, keyed by platform and image digest.

**How memory is read for an image.** Read `VmRSS` and `VmHWM` from `/proc/<pid>/status` of the server process itself, inside the container (the server is PID 1 with `docker run -i`; the harness reads the host-visible pid from `docker inspect -f '{{.State.Pid}}'` and the report says which). Do NOT use `docker stats` or cgroup `memory.current`: they include page cache and are not comparable to the targets. The container runtime's own processes (dockerd, containerd-shim, runc) are not part of the server's memory and are excluded. The harness, the MCP client script and the fixture server run outside the container. The gate runs with no memory limit. A second, recorded-only run with `--memory=64m` must finish without an out-of-memory kill, to show headroom.

**Results are keyed by platform and digest.** Every reported figure names its platform (`linux/amd64` or `linux/arm64`) and the image digest it was measured on. The two platforms are never averaged. A figure without both is not a result.

QEMU or any non-native figure is never gating. (The A-1 QEMU figure of 13.0 MiB idle was translator overhead.) If the harness cannot yet do something the procedure above needs (for example an amd64 `--gate` mode or in-container pid reading), that is a build item, not a reason to weaken a rule.

## 4. Binaries (E-8 loopback path, CONFIRMED by the user 2026-09-19; OQ-4 not decided)

**Why two binaries?** The shipped release binary is fail-closed: it blocks unsafe addresses, so it cannot reach the loopback test server. So idle is measured on the shipped binary. Peak and 50 MiB scenarios run on the `bench-loopback` build instead.

The `bench-loopback` build:

- is a compile-time Cargo feature, off by default;
- permits only `127.0.0.0/8` and `::1`;
- is never in a release, tag or distributed artifact (the D-7 guard asserts it is absent).

Both binaries come from one commit, one `Cargo.lock` hash and the D-7 release profile, through the same pinned pipeline. Both report commit and `Cargo.lock` hash in `--version`. (E-8 and D-3 add the commit and hash; today `--version` prints the crate version only.)

**Marker contract (one definition).**

- A build with a forbidden feature makes `--version` print `FETCH_MCP_MARKER_<FEATURE>_V1:<feature-name>`. A bench build prints `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback`. A release build prints no marker.
- `--binary-kind shipped` is refused if `--version` contains `bench-loopback`, `test-support` or any `FETCH_MCP_MARKER_` text. So a binary carrying only the test-support marker is not accepted as shipped.
- The harness looks for the literal `bench-loopback` in `--version`. It refuses peak runs without it. It refuses a marked binary for a shipped-binary idle run.
- The D-7 guard searches the binary for the `FETCH_MCP_MARKER_` prefix. Any marker (including a renamed feature) fails a release build.
- Every figure is labelled with its binary (`binary_kind`).

**Bench-versus-shipped bounds** (E-8, part of the G4a pass). No script enforces them yet. E-8 (a story ID) checks the idle delta, size record, public-host cross-check and commit/lock-hash equality when `--version` gains commit and lock hash. Until then the manual E-8/G4a report checks them.

| Bound | Rule |
|---|---|
| Idle delta | <= 0.5 MiB |
| Binary size delta | recorded and explained (no bound) |
| Public-host cross-check | one manual 5 MiB fetch of a public host on the shipped binary: within 10% of the bench peak and <= 40 MiB |
| Build identity | same commit and `Cargo.lock` hash |

Exceeding a bound fails G4a until it is explained and re-measured.

## 5. Measurement method

Each sample uses a fresh child process (cold: no in-process warm-up). It is started with a pinned environment. The allow-list is `PATH`, `FETCH_LOG=warn`, `LC_ALL=C`, plus any recorded `--child-env`. `RUST_LOG`, `LD_PRELOAD` and `MALLOC_*` are unset. The client script is `bench/measure.py` (JSON-RPC over stdio).

1. Start the process. Send `initialize`, then `notifications/initialized`, then `tools/list`.
2. Idle: sleep 30 s (`--settle`), then read `VmRSS` (and `VmHWM`) from `/proc/<pid>/status`. Idle samples may run in parallel processes (`--parallel-idle`).
3. Peak: make one `tools/call fetch` per fresh process. When it returns, before the process exits, read `VmHWM`.
4. The fixture server and the harness run on the same host over loopback (`bench/serve.py`).

**Fixtures** (`bench/fixtures.py`). They use a fixed seed (1). Their sha256 and size are committed in `bench/manifest.json`, and the harness refuses to run on a mismatch. The gzip file is the one exception: gzip output differs between zlib builds (a laptop and the GitHub runner produce different bytes), so the manifest pins what the file unpacks to (its size and sha256, which must match the plain 5 MiB page) and only requires the compressed size to sit between 256 KiB and 2 MiB. The check unpacks the file and compares, so a corrupt or altered gzip is still refused. The scenarios use the real compressed size on disk as the wire bytes, and the 5 MiB limit is still applied to the unpacked bytes. `bench/fixtures/` is not committed: regenerate it with `fixtures.py generate` before the first real run (the self-test does this itself).

| Fixture | Detail |
|---|---|
| 5 MiB HTML | 5,241,856 B, 1 KiB under the cap so a correct server does not answer `too_large` |
| Same page gzipped | sent with `Content-Encoding: gzip`. The compressed bytes depend on the zlib build, so they are not pinned; the unpacked content is |
| 50 MiB HTML | served two ways: with `Content-Length`, and chunked (no `Content-Length`) |
| Slow-drip route | `/slow`, 64 B/s |

The late-landmark HTML and the remaining E-2 fixtures are not in the skeleton.

**Scenarios** (`bench/scenarios.py`). G0, G4a and G4b are gates. G1 to G7 are scenario IDs (architecture 11.1).

| Harness ID | Scenario | Gate | Status in skeleton |
|---|---|---|---|
| `idle` | handshake + 30 s | G0, G4a, G4b (shipped) | implemented |
| `g4a-5mib-full` | 5 MiB HTML read in full and converted (G1/G2 full form) | G4a | implemented |
| `g4a-5mib-gz` | same, gzip (G2) | G4a | implemented |
| `g4a-late-landmark` | late-landmark holdback-full HTML (G4) | G4a | defined, fixture is E-2 |
| `g4a-50mib-cl` | 50 MiB with `Content-Length` -> `too_large` (G7a), <= 1.10x 5 MiB | G4a | implemented |
| `g4a-50mib-chunked` | 50 MiB chunked, no window, `too_large` at cap, <= 1.10x | G4a | implemented |
| `g6-concurrent10` | 10 concurrent (G1/G2 mix), recorded not gating | none | defined, E-4 |
| `g4b-window-start`, `g4b-window-end` (G1), `g4b-raw` (G3), `g4b-chunked-window-in-cap` (G7b), `g4b-window-beyond-cap` (G5, G7c) | need A-5 / A-6; same 40 MiB target; 50 MiB cases <= 1.10x | G4b | defined, args/windows are E-2 |

**The `g4a-50mib-cl` early-stop floor is ZERO bytes** (`min_bytes` = 0). Architecture 11.2 sets `expected_min_bytes` for G7a to zero, because a correct client stops on the `Content-Length` header, before any body is written. This has a consequence. Nothing per-scenario confirms the request reached the server. A fetch-less or error-only stub that returns `too_large` for every call passes this scenario line (reproduced). The gate is still not falsely passed, because that same server fails `g4a-5mib-full` and `g4a-5mib-gz` (their byte floors are above zero), and the handshake requires a real `fetch` tool. Treat the `g4a-50mib-cl` line as meaningful only alongside the passing 5 MiB scenarios.

**Boundedness.** A scenario with a `bounded_vs` reference (the 50 MiB ones compare against `g4a-5mib-full`) whose reference has no valid result in the same run gets verdict `INCOMPLETE` (exit 2). It never gets `PASS` or `ADVISORY_PASS`.

Non-gating, reported: a default-parameter call, slow-drip (timing +-20% of `FETCH_TIMEOUT_MS`), and a TLS run. The harness refuses unimplemented scenarios (exit 2) rather than skipping them.

## 6. Valid-run rule and verdicts

A sample is valid only if all of these hold:

- the handshake succeeded;
- the outcome matches the scenario (`ok` or `too_large`);
- the fixture server's byte counter for the route is >= the scenario `min_bytes` (the early-stop check; the counter is an upper bound on what the client read).

A scenario needs at least 10 valid samples, otherwise the whole report is INVALID (exit 2). `--runs` below 10 is refused unless `--smoke`, which is never valid for memory claims (NFR claims). No outlier is discarded. Min, median and max are reported, and the decision uses the median.

Exit codes:

| Code | Meaning |
|---|---|
| 0 | `--gate` run: summary `PASS`. Non-gate or `--smoke` run: `ADVISORY_PASS`, which is NOT a result and never satisfies a target |
| 1 | target missed |
| 2 | INVALID, INCOMPLETE or refused (fixture hash, wrong binary kind, missing binary, unimplemented scenario, fewer than 10 runs, `--gate` override or incomplete gate set, failed handshake) |
| 3 | `--gate` off native aarch64 |

Output is JSONL: one `host` line, one line per scenario, and one `summary` line (gating peak = max of medians, boundedness ratios, verdict).

## 7. Determinism list

"Determinism" means: the same setup gives the same numbers. These are the rules that make it so.

- Fresh process per sample.
- Pinned child environment (section 5).
- Fixtures from a fixed seed with committed hashes.
- These are recorded: page size, THP, load average before and after, cgroup limit, libc (gnu/musl), allocator, binary sha256, commit, profile and toolchain.
- Load average more than 0.5 above the idle baseline marks the run suspect: repeat once.
- ASLR (address randomisation) is left at its default and recorded.
- One discarded dry run of the matrix per session, recorded as such.
- Native-platform preflight (host and inside the container, section 3).
- Both gnu and musl binaries are run.
- No other load on the runner that the job itself starts. The hosted VM has a runner agent and other neighbours, so the load-average rule is recorded and applied, with the baseline taken in the same job.

Skeleton status: the host record, pinned environment, hash check, preflight and binary sha are implemented. The load-average repeat rule, the dry-run discard, the libc/allocator/profile/commit fields (from `--version`) and macOS `/usr/bin/time -l` are E-2.

## 8. TLS approach (decision)

NFR-11 (the memory requirement) is measured over plain HTTP on the fixture server. TLS (encrypted HTTPS) is covered by a one-off manual run against real public hosts on the shipped binary. That is the same run that serves as the E-8 shipped-binary cross-check, and it is reported separately.

A bench-only fixture-CA build feature is not adopted. It would add a second compile-time route that affects trust and would need guarding beyond `bench-loopback`, for a small RSS question the manual run answers. Until the manual run exists, reports carry the caveat "NFR-11 measured over plain HTTP only". (Author recommendation for owner review.)

## 9. Goals 2, 3 and 5 method definitions (for A-4 and E-5)

- **Token count:** `tiktoken` encoding `cl100k_base` (pin the package version), `len(encode(text, disallowed_special=()))`. A token is a small chunk of text that AI models count. This is a reproducible proxy, not Claude's tokenizer. It is used only in the offline A-4 check script, not in the memory harness.
- **Baseline:** the raw HTML body as captured in the snapshot (no conversion, no stripping). Reduction per page = 1 - tokens(markdown) / tokens(raw HTML). Goal 3 = median over the set (>= 50%).
- **"Converts successfully":** the converter returns no error and the markdown is non-empty (after whitespace trim). Goal 2 = successes / snapshots (>= 95%), on offline snapshots, not live fetches.
- **Goal 5 overhead:** conversion time of the 1 MiB (1,048,576 B) fixture page, excluding network. Call the converter in-process on the page held in memory, discard 5 warm-up calls, time 100 calls, report p95. Use the native aarch64 runner and the release profile. Target <= 500 ms.
- **50-URL set:** E-7 captures offline snapshots (HTML plus a manifest with URL, date, size, sha256). The 10-URL live smoke list (network, TLS, redirects, JSON, plain text) is separate and non-gating.
  - **DEFERRED** (owner: project owner; user decision, Sprint 0 revision 4). The concrete 50-URL list and the 10-URL live smoke list are not defined in Sprint 0. The project owner must supply them before E-7 (Sprint 2) starts. They must exclude pages that need cookies or authentication. The Sprint 0 exit for E-1 records the deferral in place of the list.

## 10. G0 and go/no-go

A-1 was measured on x86_64 only, before this protocol existed. Its figures: idle 3.7-5.2 MiB; streaming-build peak 6.1 MiB on the first page, 8.7 MiB on a deep page, 11.1 MiB raw; a buffered DOM (whole page held as a tree in memory) at 56 MiB fails. It used a 0.5 s settle, a 16 MiB cap and kB medians of 10.

Deviation recorded: A-1 has not been re-run under this protocol with the product. Its G0 evidence is the advisory native aarch64 run of the A-1 spike in section 11 (the spike went through the E-1 harness, without `--gate`).

Sprint 0 verdict: GO, on the native aarch64 spike evidence in section 11. The conditions still stand: the streaming design (a buffered DOM converter is NO-GO), and the product's own gate runs (G4a, G4b) on both platforms, which are still to do.

## 11. Hosted arm64 spike results (advisory, Sprint 0)

**Read this as evidence about the spike, not the product, and not a gate result.** The run was not a `--gate` run, so its summary verdict is `ADVISORY_PASS`, which by this document never satisfies a target. Full report: `.delivery/artifacts/06-dev/sprint-0/arm-bench-report.md`; raw JSONL is beside it in `arm-bench/`.

What ran: the A-1 spike (reqwest and `lol_html` streaming, `spikes/a1`), gnu and musl builds, on `ubuntu-24.04-arm`, through `bench/measure.py` with 10 valid runs per scenario and the default 30 s settle. Two runs on different runner instances (a pull-request run and a manual dispatch run). Harness self-test ran first. The spike has no `--version`, so tiny wrapper scripts answered `--version` and then `exec`ed the spike (the measured process is the spike). `--gate` would correctly refuse this stand-in.

Medians in MiB (10 of 10 runs valid, none invalid). Platform `linux/arm64`, host facts in section 3.

| Scenario | Metric | Target | gnu (run 1 / run 2) | musl (run 1 / run 2) |
|---|---|---|---|---|
| `idle` | VmRSS | <= 10 | 3.66 / 3.66 | 2.09 / 2.09 |
| `g4a-5mib-full` | VmHWM | <= 40 | 16.29 / 16.31 | 10.49 / 10.49 |
| `g4a-5mib-gz` | VmHWM | <= 40 | 7.93 / 7.93 | 5.46 / 5.46 |

On this hosted ARM VM, the spike is under both targets by a wide margin. The two runner instances differed by about 0.02 MiB.

Caveats, all still open:

- The spike is not the product. It has no SSRF layer and no real pagination or `too_large` path. The product will use more memory. The product's own `--gate` runs are still required.
- The 50 MiB boundedness scenarios were NOT run (only the 5 MiB scenarios), so boundedness has no evidence yet.
- `g4a-5mib-gz` peaks lower than `full` because the spike probably does not decompress (no gzip feature). It says nothing about decompression cost.
- The hosted VM is an Azure Neoverse-class machine with 4 vCPUs and 16 GB, not the Raspberry Pi 5. The core, memory system and kernel differ. Its page size is 4 KiB, while Pi OS uses 16 KiB pages, which can raise RSS. A pass at 4 KiB does not prove a pass on a Pi. Targets are unchanged.
- Only two runner instances were sampled.
- This is arm64 only. amd64 is NOT YET MEASURED on a hosted runner.

## 12. Timings (recorded, not gated)

Alongside memory, the harness records how long each step takes. **These timings are RECORDED, NOT GATED.** No timing changes a verdict, an exit code or whether a sample is valid. The self-test proves this: a server made 300 ms slower gets the same verdict and exit code.

**The PRD 250 ms readiness figure is NOT checked by the harness.** `ready_ms` is evidence only. ADR-007 restates the readiness figure for the container case.

All values are milliseconds from a monotonic clock. Wall-clock time appears only in the `*_utc` stamps.

| Field | Meaning |
|---|---|
| `start_utc` | UTC time (ISO-8601, ending `Z`) when the sample started. |
| `ready_ms` | From just before the process is spawned to the first valid `initialize` result. |
| `tools_list_ms` | From sending `tools/list` to receiving a valid result. |
| `first_byte_ms` | Fetch scenarios only. From sending `tools/call` to the fixture server first writing a body byte. It is the server's own stamp, so it approximates when the first byte reached the client. Absent when the server never writes a body (the 50 MiB `Content-Length` case, where the client stops at the headers). |
| `fetch_ms` | Fetch scenarios only. From sending `tools/call` to receiving its result. |
| `total_ms` | From sample start until the child process is closed. For the idle scenario this includes the settle wait (30 s by default), so do not compare it with the fetch scenarios. |

How the numbers are summarised:

- Statistics use valid samples only. Invalid samples are still listed in `sample_timings` (with `valid: false`) but do not count.
- Per key the harness reports `n`, `median`, `min` and `max`. It adds `p95` (nearest-rank) only when there are 20 or more valid samples. Ten samples give no p95.
- Each scenario carries a `timings` object with `unit`, `gated` (always false), `gating_run` (true only with `--gate`), `standin` (true when `--version` does not start with `fetch-mcp `) and a `label`: "advisory (not a gating run)" or "recorded, not gated". A stand-in run is not evidence about the product.

New JSONL keys: `timings` and `sample_timings` on each scenario record, `run_start_utc` on the `host` record, and `run_start_utc` plus `run_end_utc` on the `summary` record. The hosted arm64 job summary shows a Timings table.

Caveats:

- Hosted cloud VMs are noisy neighbours. Expect a wide min to max range. Use medians and never trust one run.
- Once a container image exists, `docker run -i` adds daemon, namespace and image-layer startup to `ready_ms`. Measure it as a separate binary kind or wrapper, label it, and do not compare it with a native spawn.
- The harness reads `/proc` of the spawned process. For `docker run -i` that is the docker client, not the container, so memory would differ too. How to measure the container is a design decision for the container-measurement story.
- The spawn timer starts before the process is started, so wrapper cost (such as the `sh` exec wrapper in the workflow) is included.
- Not measured: client-observed first byte, TLS and DNS (loopback HTTP), CPU time, container start, page-cache and cold-start effects, time per idle settle phase.
