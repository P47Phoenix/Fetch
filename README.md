# Fetch

**What is this?** Fetch is a small program, `fetch-mcp`, that will download a web page and turn it into text for an AI tool. Today it is an early build: it speaks the MCP protocol over standard input and output, offers one tool called `fetch`, checks the input and blocks unsafe addresses, but it does not download anything yet. **Who needs it?** Contributors who want to build, test or measure it. It is not ready for ordinary users. **What to do first:** read the status below, then follow "Build", "Test" and "Run over stdio".

## Status: pre-MVP (early, unfinished)

**Do not register `fetch-mcp` in a real MCP client** (MCP, the Model Context Protocol, is how an AI tool talks to a helper program) before milestone M3. The only planned check with Claude Code is a manual one on the owner's machine, using a throwaway config, not your normal setup.

What works today (stories A-2 and A-3a):

- `fetch-mcp` is a real MCP server over stdio (built on the `rmcp` crate, version 3.4.0). It offers exactly one tool, `fetch`, with the inputs `url`, `max_length`, `start_index` and `raw`.
- A valid `fetch` call returns an error result saying `not_implemented`. There is no HTTP client in the program yet, so it never touches the network. The real download arrives in story A-3b.
- Bad input, and web addresses that point at private or internal machines, are refused with a clear error. See [SSRF range table](docs/SSRF.md) for what is blocked and why.

What has not been checked yet:

- The automatic checks (CI) run on GitHub and pass on pull request #5, but branch protection on `main` is not switched on, and `cargo-audit` has never been run.
- The memory gates for the product (10 MiB idle, 40 MiB peak) have not been run. There is no `--gate` run on amd64 or arm64. The only product numbers are advisory (recorded, not enforced): see [BENCHMARK.md section 13](docs/BENCHMARK.md#13-real-product-idle-numbers-advisory-sprint-1).
- Open questions still undecided: OQ-3 (robots.txt on or off by default), OQ-4 (private-host allowlist), OQ-5 (labelling fetched content as untrusted) and OQ-7 (licence and distribution). An Apache-2.0 `LICENSE` file exists, and the project is marked `publish = false` (Cargo will refuse to publish it to crates.io) until OQ-7 is decided.
- The release artifact will be a multi-arch container image (`linux/amd64` and `linux/arm64`) on GHCR, not standalone binaries (ADR-007).

Details: [what has and has not been checked](docs/BENCHMARK.md#read-this-first-what-has-and-has-not-been-checked).

## Prerequisites

- Linux. macOS is not supported yet.
- Rust, installed with `rustup`. The file `rust-toolchain.toml` pins the version (1.94.1); `rustup` installs it on the first build.
- Python 3.8 or newer, only for the benchmark self-test.

## Build

```
cargo build --locked
```

"Locked" means Cargo must use exactly the versions in `Cargo.lock`. Success: the build ends with a `Finished` line and the file `target/debug/fetch-mcp` exists. Check it with `./target/debug/fetch-mcp --version`, which prints `fetch-mcp 0.0.0`.

For a release build (the size and memory numbers come from this one) use `cargo build --release --locked`; the file is then `target/release/fetch-mcp`.

## Test

```
cargo test --locked
python3 bench/selftest.py
```

Success: `cargo test` ends with `test result: ok.` for every test group, and the self-test's last line is `SELFTEST PASSED`. The CI checks and their exact names are listed in [CI and branch protection](docs/ci-branch-protection.md).

## Run over stdio

An MCP server reads one JSON message per line on standard input and answers on standard output. Nothing else is ever written to standard output. Logs go to standard error, and are quiet by default; set `FETCH_LOG=debug` (other levels: `error`, `warn`, `info`) for more detail.

Copy and paste this into a shell, from the repository folder, after building:

```
printf '%s\n' \
'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}' \
'{"jsonrpc":"2.0","method":"notifications/initialized"}' \
'{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
'{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"https://example.com/"}}}' \
'{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"http://127.0.0.1/"}}}' \
'{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"not a url"}}}' \
| ./target/debug/fetch-mcp
```

### What to expect

Five reply lines, one for each request that has an `id` (the `notifications/initialized` line gets no reply):

1. `initialize`: a result with `"protocolVersion":"2025-06-18"`, `"capabilities":{"tools":{}}` and `"serverInfo":{"name":"rmcp","version":"3.4.0"}`. The name shown is the MCP library's, not `fetch-mcp`.
2. `tools/list`: one tool, `fetch`, described as "Fetch a URL and return its content as markdown". `url` is required. `max_length`, `start_index` and `raw` are optional.
3. A valid URL (id 3) gives a tool result with `"isError":true` and this text:
   `error[not_implemented]: fetch is not implemented yet: this build validates input but performs no network requests`
4. A blocked address (id 4) gives `"isError":true` and this text:
   `error[blocked_target]: IP address is not public (loopback)`
5. Bad input (id 5) gives `"isError":true` and this text:
   `error[invalid_argument]: url: must be an absolute http or https URL`

The error convention: a failed `fetch` is still a normal JSON-RPC `result`, with `"isError":true` and one text item that reads `error[<code>]: <message>`. For `invalid_argument` the message starts with the name of the bad field (here `url`). The codes today are `invalid_argument`, `blocked_target` and the temporary `not_implemented`, which goes away when A-3b lands. The blocked-target message names a category (such as loopback), never the address, so an error cannot reveal what is on your network. See ADR-006 (tool schema, with its dated amendment) for the design.

## Where to go next

| I want to... | Read |
|---|---|
| See which addresses are blocked, and why | [SSRF range table](docs/SSRF.md) |
| Run or understand the memory benchmark | [Benchmark guide: quickstart](docs/BENCHMARK.md#quickstart-contributors) |
| Look up a benchmark word (VmRSS, median, gate, ADVISORY_PASS ...) | [Benchmark glossary](docs/BENCHMARK.md#glossary) |
| Understand the automatic checks (CI) and the branch rules on `main` | [CI and branch protection](docs/ci-branch-protection.md) |
