# Fetch

**What is this?** Fetch is a small program, `fetch-mcp`, that will download a web page and turn it into text for an AI tool. Today it is an early build: it speaks the MCP protocol over standard input and output, offers one tool called `fetch`, checks the input, blocks unsafe addresses, and downloads the page (streamed, size-limited, from public addresses only). It converts HTML pages to markdown as it downloads them. **Who needs it?** Contributors who want to build, test or measure it. It is not ready for ordinary users. **What to do first:** read the status below, then follow "Build", "Test" and "Run over stdio".

## Status: pre-MVP (early, unfinished)

**Do not register `fetch-mcp` in a real MCP client** (MCP, the Model Context Protocol, is how an AI tool talks to a helper program) before milestone M3. The only planned check with Claude Code is a manual one on the owner's machine, using a throwaway config, not your normal setup.

What works today (stories A-2, A-3a, A-3b, A-4 and E-4; `fetch-mcp --version` prints the git commit and the `Cargo.lock` hash it was built from):

- `fetch-mcp` is a real MCP server over stdio (built on the `rmcp` crate, version 3.4.0). It offers exactly one tool, `fetch`, with the inputs `url`, `max_length`, `start_index` and `raw`.
- A valid `fetch` call downloads the URL (HTTP/1.1, https with the built-in trust anchors). An HTML page is converted to markdown while it streams (headings, links, lists, code, tables; scripts, styles and navigation chrome removed; `raw=true` skips this) and other text is returned as fetched, in both cases cut to the requested character window. The body is read as a stream and never held whole: it stops with `too_large` past `FETCH_MAX_BYTES` (5 MiB), after 15 s with `timeout`, and gzip is unpacked in bounded steps. Every request and every redirect hop goes through the SSRF checks, and the client connects only to the addresses those checks approved. **Fetched content is returned as-is with no untrusted-content label (open question OQ-5 was decided "no label" on 2026-09-20).** When a redirect was followed, the returned text begins with a small plain-text header, `URL: <final url>`, `Status: <code>` and a blank line (A-9); with no redirect nothing is added. The header is outside the `max_length` window (it does not count against it), and the URL in it has no userinfo and no fragment. The header is unmarked plain text: a page that was not redirected can begin with identical text, so it is a convenience for citing, not proof of provenance. Not yet done: early stop and continuation messages (A-5), `raw` and content-type handling (A-6), cause-specific errors (A-7), charsets other than UTF-8 (A-8).
- **TLS caveat:** certificate validation (the https trust check) has no automated test. It was checked by hand only, against public sites. A regression there would not be caught by CI until B-2/B-5 or E-2 add a test.
- Bad input, and web addresses that point at private or internal machines, are refused with a clear error. See [SSRF range table](docs/SSRF.md) for what is blocked and why.

What has not been checked yet:

- The A-4 conversion-quality check (at least 95% of the 50 offline pages convert, median token reduction at least 50%) has NOT been run: the 50-page snapshot set (E-7) does not exist yet. Conversion is tested only on small local unit-test fixtures.
- The automatic checks (CI) run on GitHub and pass on pull request #7 (Sprint 3), but branch protection on `main` is not switched on, and `cargo-audit` has never been run.
- **Memory gate part 1 (G4a, story E-4) passed in one CI run (35557702662, PR #8) on all four hosted cells** (linux/amd64 and linux/arm64, gnu and musl, native runners, `--gate`, median of 10 valid runs, with HTML conversion): idle 2.35 to 4.62 MiB (target 10) and gating peak 4.56 to 6.16 MiB (target 40), 50 MiB pages within 1.03 of the 5 MiB run; 10 concurrent fetches recorded at 7.4 to 16.0 MiB (recorded, not a pass or fail). Details, hosts and caveats (single run, the shipped-vs-bench record, the public-host check on a 2 MiB page rather than 5 MiB, the allocator record): [BENCHMARK.md section 16](docs/BENCHMARK.md#16-g4a-peak-rss-and-boundedness-with-conversion-on-both-hosted-platforms-e-4-ci-run-35557702662-pr-8). **A G4a pass does NOT close the memory gate:** G4b (window, early stop, `raw=true`; stories A-5 and A-6) is decided at the end of Sprint 5 and is not run yet. Earlier read-in-full runs: [section 15](docs/BENCHMARK.md#15-sprint-3-gate-runs-on-both-hosted-platforms-e-2-e-3-ci-runs-35538774565-35539714646-and-35541676701-pr-7). The container image has not been measured yet (the runs use bare binaries; D-2 builds the image), and the container image has not been measured yet (the runs use bare binaries; D-2 builds the image). The `bench-gate` job is not a required check. Older advisory numbers: [BENCHMARK.md section 13](docs/BENCHMARK.md#13-real-product-idle-numbers-advisory-sprint-1).
- Open questions still undecided: OQ-3 (robots.txt on or off by default), OQ-4 (private-host allowlist), and OQ-7 (licence and distribution). An Apache-2.0 `LICENSE` file exists, and the project is marked `publish = false` (Cargo will refuse to publish it to crates.io) until OQ-7 is decided.
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

Five reply lines, one for each request that has an `id` (the `notifications/initialized` line gets no reply). Replies can arrive out of order: the network fetch (id 3) is slow, so its reply usually comes last, after ids 4 and 5. Match replies to requests by `id`, not by position. The list below is in request order.

1. `initialize`: a result with `"protocolVersion":"2025-06-18"`, `"capabilities":{"tools":{}}` and `"serverInfo":{"name":"rmcp","version":"3.4.0"}`. The name shown is the MCP library's, not `fetch-mcp`.
2. `tools/list`: one tool, `fetch`, described as "Fetch a URL and return its content as markdown". `url` is required. `max_length`, `start_index` and `raw` are optional. HTML is converted to markdown (A-4). The tool description string does not mention the A-9 header.
3. A valid URL (id 3) is downloaded, so this needs network access: you get the page text in one text item (`"isError"` false). Offline, you get `"isError":true` and `error[dns_failure]: hostname did not resolve`.
4. A blocked address (id 4) gives `"isError":true` and this text:
   `error[blocked_target]: IP address is not public (loopback)`
5. Bad input (id 5) gives `"isError":true` and this text:
   `error[invalid_argument]: url: must be an absolute http or https URL`

The error convention: a failed `fetch` is still a normal JSON-RPC `result`, with `"isError":true` and one text item that reads `error[<code>]: <message>`. For `invalid_argument` the message starts with the name of the bad field (here `url`). The codes today are `invalid_argument`, `blocked_target`, `dns_failure`, `too_large`, `timeout`, `unsupported_encoding`, `http_error`, `too_many_redirects`, `network_error`, `bad_response` and `internal`. The blocked-target message names a category (such as loopback), never the address, so an error cannot reveal what is on your network. See ADR-006 (tool schema, with its dated amendment) for the design.

Two shapes differ from that convention, because they are produced by the MCP library (rmcp 3.4) before our code runs:

- **Wrong-typed or missing arguments** (for example no `url`, or `"max_length":-1`) return `isError: true` with text such as `failed to deserialize parameters: missing field \`url\``, or `failed to deserialize parameters: max_length: invalid value: integer \`-1\`, expected u64`. The field is named, but the text has no `error[invalid_argument]:` prefix. Our own checks (URL scheme, blocked address, and so on) do use the prefix.
- **Frames the library cannot deserialise** get no reply at all (a client waiting for that `id` would wait forever), and the server keeps running. Examples: a `tools/call` whose `arguments` is nested about 200 levels deep. `"arguments": []` returns JSON-RPC error `-32601` with message `tools/call`, not `-32602`. Invalid UTF-8 or NUL bytes, and `notifications/initialized` sent before `initialize`, end the session (exit code 1, nothing on standard output). None of these can crash or hang the server.

Unknown extra arguments are accepted and ignored, and `max_length` (default 5000, capped by `FETCH_MAX_LENGTH_CAP`) and `start_index` pick a character window of the fetched text; there is no continuation footer, no note when `max_length` is clamped and no early stop yet (A-5).

## Where to go next

| I want to... | Read |
|---|---|
| See which addresses are blocked, and why | [SSRF range table](docs/SSRF.md) |
| Run or understand the memory benchmark | [Benchmark guide: quickstart](docs/BENCHMARK.md#quickstart-contributors) |
| Look up a benchmark word (VmRSS, median, gate, ADVISORY_PASS ...) | [Benchmark glossary](docs/BENCHMARK.md#glossary) |
| Understand the automatic checks (CI) and the branch rules on `main` | [CI and branch protection](docs/ci-branch-protection.md) |
