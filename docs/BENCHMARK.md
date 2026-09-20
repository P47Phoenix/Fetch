# Benchmark protocol and absolute targets (story E-1)

**What is this?** This file explains how we measure how much memory the `fetch-mcp` program uses, and the two limits it must stay under: 10 MiB when idle and 40 MiB at its peak. **Who needs it?** Anyone who runs the benchmark scripts in `bench/`, or who reads a memory report and wants to know what the numbers mean. **What to do first:** read the Glossary below, then follow the Quickstart (step 1 takes about 1 minute and needs no Rust).

Status: E-1 result, Sprint 0. Memory targets: 10 MiB idle (VmRSS), 40 MiB peak (VmHWM), 5 MiB fetch cap. Other documents (PRD, EPICS, architecture, config defaults, reports) refer to this file for the unit, targets, scenarios and method. The test tools live in `bench/` (self-test: `python3 bench/selftest.py`). Story E-2 builds the full fixture set and CI on top of them.

## Read this first: what has and has not been checked

Be careful not to read more into this document than it says.

| Item | Status |
|---|---|
| Hosted GitHub Actions running this repository's workflow | Has run on several pull requests (first #2, most recently #5); all five required checks (`fmt`, `clippy`, `test`, `deny`, `release-guard`) have passed each time. That is still not a trend claim, and branch protection is not configured yet. |
| `cargo-deny` (the `deny` job) | Has run in those hosted runs and passed. |
| `cargo-audit` | Never run (not installed on the dev host). |
| The benchmark scripts | Run against the stand-in, against the real `fetch-mcp` idle scenario (advisory, section 13), and in `--gate` runs on both hosted platforms (section 15, CI job `bench-gate`). |
| The `fetch-mcp` binary | A real stdio MCP server with one `fetch` tool (A-2) and, since A-3b, a guarded streaming download. The idle figure with the client compiled in is 3.58 MiB on hosted arm64 (section 13, advisory); the older 2.59 and 3.15 figures predate the client. |
| Native aarch64 measurement | Advisory runs earlier (A-1 spike, section 11; early product idle, section 13). Since Sprint 3, `--gate` runs on the hosted arm64 runner (gnu and musl): section 15, two CI runs, idle 2.16 to 3.55 MiB, read-in-full peak 4.14 to 4.66 MiB. |
| Native amd64 measurement | `--gate` runs on the hosted amd64 runner (gnu and musl): section 15, two CI runs, idle 2.27 to 4.06 MiB, read-in-full peak 4.25 to 5.27 MiB. Runner CPUs vary between runs (Xeon 8573C, 8370C, 6973P-C, AMD EPYC 7763). |
| Product memory gates (G4a, G4b on amd64 and arm64) | Not decided. The E-3 idle gate (strict 10 MiB) passed on all four cells, and the peak gate ran in its read-in-full form (before A-4 conversion exists), which is NOT a G4a pass. G4b scenarios and `g6-concurrent10` are not run. The container image has not been measured (D-2 builds it). |
| Branch protection on `main` | Not configured yet (see `docs/ci-branch-protection.md`). |

## Glossary

Each term is explained here once. Later sections use the short form.

| Term | Plain meaning |
|---|---|
| MCP | Model Context Protocol. A way for an AI tool (a "client") to talk to a helper program (a "server") such as `fetch-mcp`. |
| Skeleton | A program with the right name and shape but almost nothing inside. `fetch-mcp` used to be one (it only printed its version). It is now a real stdio server with a guarded download, but no HTML conversion yet. Do not register it in a real MCP client. |
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
| gnu, musl | Two versions of the C library the binary can be built with. Both are built and gated for the product by `bench.yml` on each native platform (section 15). |
| p95 | 95% of results are at or below this value. Timings report it only with 20 or more valid samples (nearest-rank). |
| Timings | Durations the harness records next to memory (section 12). Recorded, not gated. |
| ready_ms | Time from starting the process to its first valid `initialize` answer. Evidence only; the PRD 250 ms readiness figure is not checked by the harness. |
| tools_list_ms, first_byte_ms, fetch_ms, total_ms | Other recorded durations, all in milliseconds (section 12). |
| OQ-n | An open question in the project plan, for example OQ-7 (licence). |
| E-1, E-2, A-1 ... | Story IDs in `docs/EPICS.md`. |

## Quickstart (contributors)

Do these steps in order. Steps 1 to 4 work on any Linux machine and need no Rust for step 1 and step 4. Step 5 only works on a native host of the platform under test (a GitHub-hosted runner in CI).

**Before you start you need:**

- Linux. macOS (`/usr/bin/time -l`) is NOT implemented: an open gap of E-2, recorded in the Sprint 3 dev report.
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

**What you can run on the real binary.** Story A-2 has landed, so the handshake works and `measure.py` can measure the real `fetch-mcp` in advisory mode (no `--gate`). The `idle` scenario works on the shipped build. The fetch scenarios (5 MiB and 50 MiB) need the E-2 fixtures and the harness wiring (Sprint 3); the peak scenarios need the `bench-loopback` build. (A-3b has landed, so `fetch` no longer returns `not_implemented`.) A `--gate` run needs a native aarch64 or x86_64 host (see "What can go wrong"). Example, after the release build of step 3:

```
python3 bench/measure.py --binary target/release/fetch-mcp --binary-kind shipped --scenario idle --runs 10
```

Success: a `summary` line with `"verdict": "ADVISORY_PASS"` and exit 0 (this takes about 5 minutes: 10 runs with a 30 s settle each). It is not a result; see section 13 for what has been recorded. Also, story E-8 says a bench build must print its marker to stderr at start-up. That is not built yet. Today the marker is only printed by `--version`.

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

Those two refusals (override and incomplete set) come before the native-host check (exit 3). So they behave the same on any host. The binary identity refusal below comes after it, so it is reached only on a native aarch64 or x86_64 host. On a host that is not aarch64 or x86_64, or where QEMU handlers or an unreadable `/proc/sys/fs/binfmt_misc` make native execution unproven, a `--gate` run stops at exit 3 first. A bench binary under `--gate` needs a real `bench-loopback` build (its `--version` marker). The stand-in with the environment variable trick does not work.

**Binary identity check.** With `--gate` only, after the override rules and before any sample, the script checks that the file really is what you say it is:

1. It must be an ELF file whose CPU type (`e_machine`) matches the host (aarch64 or x86_64, matching the host, for a gating run).
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
| Exit 2, `INVALID`, `RuntimeError: server closed stdout` when you point it at `target/release/fetch-mcp` | The server exited or crashed before the handshake finished, or the path is not a `fetch-mcp` build. Since A-2 the handshake should work, so this is a real problem. | Run the binary by hand with the session in the README ("Run over stdio") and read stderr, using `FETCH_LOG=debug`. Rebuild with step 3. Check the path and that the file is `fetch-mcp`, not the stand-in. |
| Exit 2, refused, reason `binary identity` (only reached on a native aarch64 or x86_64 host; elsewhere you get exit 3 first) | The file is not the right kind: a script, a wrong-CPU ELF, or the wrong marker. | Rebuild with the command for the kind you passed (step 3). |
| Exit 2, refused, reason `incomplete` | A `--gate` bench run left out scenarios of its group. | Run the whole group (for example every G4a scenario). |
| Exit 2, `INCOMPLETE` in a 50 MiB scenario | Its 5 MiB reference scenario has no valid result in the same run. | Include `g4a-5mib-full` in the same run. |
| Exit 2, refused, `--gate` override | You used `--smoke`, a target override, another `--settle`, `--parallel-idle` above 1, or `--child-env`. | Remove the flag. |
| Exit 3, `--gate` off a native gate host | `--gate` needs a native aarch64 or x86_64 host with no QEMU handler registered for that architecture (hosted `ubuntu-24.04-arm` and `ubuntu-24.04`, E-2). Same rigor on both: median of 10, INVALID/INCOMPLETE exit 2, ADVISORY is never a result. | Run on a hosted runner. Other machines (and QEMU) can only do advisory runs. |
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
| Measured? | Yes: advisory spike (2 runs, section 11) and gate runs (section 15) | Yes: gate runs, 2 CI runs (section 15) |
| VM / OS | Azure VM, Ubuntu 24.04.5, image `ubuntu24-arm64` 20260907.118.1 | Azure VM, Ubuntu 24.04.5 LTS, image `ubuntu24` 20260907.300.1 |
| Kernel | Linux 6.17.0-1022-azure aarch64 | Linux 6.17.0-1022-azure x86_64 |
| CPU | Neoverse-class core (`CPU part 0xd49`), 4 vCPU; the model name is recorded from `lscpu` from the fix-pass run onward | Varies per run: Intel Xeon Platinum 8573C, 8370C, Xeon 6973P-C, AMD EPYC 7763 (from the job logs); 4 vCPU |
| RAM (MemTotal) | 16,330,124 kB | 16,373,452 kB |
| Page size (`getconf PAGESIZE`) | 4096 | 4096 |
| THP | `madvise` | `always` on the run logged (differs from arm64) |
| glibc on the host | 2.39 | not recorded by the run (Ubuntu 24.04 ships 2.39) |
| qemu binfmt handler | none for aarch64 | none for x86_64 (gate refuses otherwise) |

Only a handful of runner instances have been sampled, so how much the hosted VMs differ from one another is barely known. The amd64 runner CPU model changed between runs, so compare only within a platform and note the CPU.

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

E-2 fixtures (Sprint 3): `late_landmark` (about 5 MiB of navigation markup with the only `<main>` in the last KiB; seed 2, committed sha256 in `bench/manifest.json`, generated by `python3 bench/fixtures.py generate`), the `/redir/5` chain (5 redirect hops, each with a 32 KiB body, then a 4 KiB 200; generated by `bench/serve.py` from constants, so no file and no hash; scenario `redirect-chain5`, recorded outside the gating peak) and the `/slow` drip. The harness refuses to run on a hash mismatch. Scenario `idle-bench` records the idle RSS of the bench build for the E-8 delta (never a gate figure; `idle` still refuses a bench binary).

**Scenarios** (`bench/scenarios.py`). G0, G4a and G4b are gates. G1 to G7 are scenario IDs (architecture 11.1).

| Harness ID | Scenario | Gate | Status in skeleton |
|---|---|---|---|
| `idle` | handshake + 30 s | G0, G4a, G4b (shipped) | implemented |
| `g4a-5mib-full` | 5 MiB HTML read in full and converted (G1/G2 full form) | G4a | implemented |
| `g4a-5mib-gz` | same, gzip (G2) | G4a | implemented |
| `g4a-late-landmark` | late-landmark holdback-full HTML (G4), read-in-full form until A-4 adds the holdback | G4a | implemented (E-2) |
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
| 3 | `--gate` off a native aarch64 or x86_64 host (QEMU never gates) |

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
- Both gnu and musl product binaries are built (`scripts/build-candidates.sh`) and gated by `bench.yml` on amd64 and arm64 (section 15). The older advisory figures in sections 11 and 13 were gnu-only or spike-only.
- No other load on the runner that the job itself starts. The hosted VM has a runner agent and other neighbours, so the load-average rule is recorded and applied, with the baseline taken in the same job.

Skeleton status: the host record, pinned environment, hash check, preflight and binary sha are implemented. The load-average repeat rule, the dry-run discard, the libc/allocator/profile/commit fields (from `--version`) are E-2 work still open; macOS `/usr/bin/time -l` is NOT implemented.

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
- The hosted VM is an Azure Neoverse-class machine with 4 vCPUs and 16 GiB (as reported by the machine), not the Raspberry Pi 5. The core, memory system and kernel differ. Its page size is 4 KiB, while Pi OS uses 16 KiB pages, which can raise RSS. A pass at 4 KiB does not prove a pass on a Pi. Targets are unchanged.
- Only two runner instances were sampled.
- This is arm64 only. (Historical, Sprint 0. amd64 was measured on hosted runners in Sprint 3: section 15.)

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

## 13. Real-product idle numbers (advisory, Sprint 1)

**Read this as an early, advisory record, not a gate result and not evidence that the targets are met.** These runs were not `--gate` runs, so the summary verdict is `ADVISORY_PASS`, which by this document never satisfies a target. The measured program is the A-2 build: a stdio MCP server with a `fetch` tool that returns `not_implemented`. It has no HTTP client, TLS or HTML converter, so the real number will be higher. No fetch scenario (5 MiB or 50 MiB) has been run on the product. (This describes the A-2 build; superseded by A-3b and section 14.)

Idle VmRSS median, 10 of 10 valid runs each, 30 s settle, shipped (release) binary, target 10 MiB:

| Where | Platform | VmRSS median (MiB) | `ready_ms` median | Notes |
|---|---|---|---|---|
| Developer's own machine, A-2 (`.delivery/artifacts/06-dev/A-2/`) | x86_64, NOT a hosted runner | 3.15 | about 1 ms | Used the advisory-only `--parallel-idle 5` option. Release binary 1,276,440 bytes. |
| Hosted `ubuntu-24.04-arm`, A-3a (`bench-product` job; `.delivery/artifacts/06-dev/A-3a/arm/`) | linux/arm64, gnu build only | 2.59 | about 1 ms | 4 KiB pages, kernel 6.17 Azure VM. One run on one runner instance. |
| Hosted `ubuntu-24.04-arm`, A-3b fix-pass 1 (`bench-product` job, run 35523134933, PR #6 head 5c9a0dd), the CURRENT figure with the client compiled in | linux/arm64, gnu build only | 3.58 (3,664-3,668 kB, 10 of 10 valid) | about 1.2 ms | Replaces the stale 2.59 above (A-2/A-3a build, no HTTP client). Advisory, single CI run, not a gate. |

What these do NOT show:

- They are recorded, not enforced. `ready_ms` is not compared with the 250 ms figure (section 12).
- Only the gnu build was measured, and the amd64 figure is from a developer machine, not a hosted amd64 runner. (Historical, Sprint 1. Both libcs on both hosted platforms: section 15.)
- The product memory gates G4a and G4b have not been run on amd64 or arm64. The peak (40 MiB) is completely unmeasured for the product. (This describes the A-2 build; superseded by A-3b and section 14, which has one advisory arm64 5 MiB peak run.)
- The hosted arm64 job (`bench-product` in `.github/workflows/arm-bench.yml`) is advisory and is not a required check.

## 14. A-3b RSS smoke on native aarch64 (advisory, single run, not a gate)

The A-3b acceptance criterion asks for one non-gating manual 5 MiB fetch on native aarch64 with `VmHWM` read from `/proc/<pid>/status`. It was run on the GitHub-hosted arm64 runner by the advisory job `bench-product-peak` in `.github/workflows/arm-bench.yml` (CI run 35523134933, PR #6 head 5c9a0dd; job is not a required check). Binary: `fetch-mcp` release profile built with `--features bench-loopback` (`--version` shows `FETCH_MCP_MARKER_BENCH_LOOPBACK_V1:bench-loopback`), aarch64 gnu, dynamically linked, stripped; fixture served by `bench/serve.py` on loopback; scenario `g4a-5mib-full` (`max_length` 5 MiB), 10 fresh processes, harness run without `--gate` (`gating=false`, advisory verdict).

| Figure | Value |
|---|---|
| VmHWM after one 5 MiB fetch, median of 10 valid | 4.77 MiB (samples kB: 4892, 4752, 4752, 4896, 4768, 4896, 4892, 4896, 4880, 4636) |
| Idle VmRSS, shipped build, client compiled in (`bench-product`, same run) | 3.58 MiB (section 13) |
| Targets for context | 40 MiB peak, 10 MiB idle |
| Platform | aarch64, Ubuntu 24.04.5, 4 KiB pages, hosted runner (CPU part 0xd49) |

Read this as one advisory CI run on one runner instance. It is not a `--gate` run, is not evidence for NFR claims, and does not replace G4a (E-4). Idle 3.58 MiB is the shipped binary; the peak is the bench build. The earlier x86_64 figure (idle 3.8 MiB, VmHWM 5.6 MiB) was a substitute recorded in the A-3b dev report.

## 15. Sprint 3 gate runs on both hosted platforms (E-2, E-3; CI runs 35538774565 and 35539714646, PR #7)

Workflow `.github/workflows/bench.yml`, job `bench-gate`, four cells: amd64 and arm64 (ubuntu-24.04 and ubuntu-24.04-arm), gnu (glibc 2.17 floor) and musl. Binaries built on the matching native runner by `scripts/build-candidates.sh` (cargo-zigbuild 0.23.4, ziglang 0.16.0, release profile of D-7), shipped and bench-loopback, release guard run on the shipped ones. All runs are `--gate` runs (native host, no QEMU, 10/10 valid samples, summary verdict PASS, not ADVISORY). Two CI runs: **run 1 = 35538774565, measured commit d635a67**; **run 2 = 35539714646, measured the PR head 39379c1** (its bench.yml differs from d635a67 only by a trailing space; source and harness identical). Hosts (from the job logs): run 1 amd64 gnu Intel Xeon Platinum 8573C, amd64 musl Xeon Platinum 8370C @ 2.80GHz; run 2 amd64 gnu AMD EPYC 7763, amd64 musl Xeon 6973P-C, so the amd64 CPU varies between runs and vendors. arm64 CPU model was NOT recorded in either run (the arm64 `/proc/cpuinfo` has no `model name` line, the `cpu:` line was empty in all eight arm64 logs; earlier text claiming the workflow "now also records CPU part" was wrong, the change was only a trailing space). Fixed in fix-pass 1 (`lscpu` Model name, `CPU implementer`/`CPU part` fallback); see the correction below. Both about 16 GB RAM, 4 KiB pages, Ubuntu 24.04.5. Figures are MiB, median (min-max), from run 1; run 2 is in the next table. Two runs are still not a trend.

| cell | idle shipped (target 10) | idle bench | gating peak (target 40) | redirect-chain5 (recorded) |
|---|---|---|---|---|
| amd64 gnu | 4.06 (3.96-4.07) | 4.06 | 5.27 | 4.96 |
| amd64 musl | 2.27 (2.27-4.25) | 2.27 | 4.37 | 6.97 |
| arm64 gnu | 3.55 (3.54-3.55) | 3.54 | 4.66 | 4.32 |
| arm64 musl | 2.16 (2.16-2.16) | 2.16 | 4.14 | 6.44 |

Run 2 (39379c1), same layout: amd64 gnu idle 3.92, gating peak 5.17 (5294 kB); amd64 musl idle 2.27, peak 4.25 (4348 kB); arm64 gnu idle 3.55, peak 4.65 (4766 kB); arm64 musl idle 2.16, peak 4.20 (4300 kB). All four cells PASS with `--gate`, 10/10 valid; boundedness ratios at most 1.015. The two runs agree within about 0.15 MiB per cell, except the amd64 gnu idle (4.06 against 3.92, different CPUs).

Per-scenario peaks (arm64 gnu): 5 MiB 4.66, gz 4.46, late-landmark 4.66, 50 MiB with Content-Length 4.12 (ratio 0.883), 50 MiB chunked 4.61 (ratio 0.988). Every boundedness ratio in all four cells is at most 1.03 (bound 1.10). E-8 idle delta (bench minus shipped) is within +-0.01 MiB in every cell (bound 0.5 MiB). Binary size deltas and timings are in the job summaries and artifacts (`bench-gate-<arch>-<libc>`).

What these numbers are not: the peak gate ran before A-4 (conversion) exists, so it is the read-in-full form of G4a and NOT a G4a pass (G4a is decided at the end of Sprint 4); G4b scenarios and `g6-concurrent10` are not run; the shipped-binary public-host 5 MiB cross-check (E-8) is still open (E-4); the runs are against bare binaries, not the container image (D-2 builds the image). The idle gate (E-3) is a strict check, no tolerance.

Design notes recorded in fix-pass 1: `redirect-chain5` has gate `none`, meaning it is excluded from the reported gating peak figure, but it is intentionally still fail-closed (an INVALID run or a median above the peak target fails the gating invocation; tested). The `idle-bench` step is advisory (exit 1 is recorded, exit 2/3 fail the job). The native-host preflight fails closed under `--gate` if `/proc/sys/fs/binfmt_misc` cannot be read.

**Correction (fix-pass 1, appended, not a rewrite of the above).** The sentence above in the first version of this section, that the workflow "now also records CPU part", was false: the CPU model was empty on arm64 in every run so far. The workflow now records `cpu:` from `lscpu` (`Model name`), with `/proc/cpuinfo` and `CPU implementer`/`CPU part` as fallbacks, and a `cpu_id:` line. Values from the first run that has the fix are listed in `.delivery/artifacts/06-dev/sprint-3/fix-pass-1-report.md`.
