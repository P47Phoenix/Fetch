# Fetch

**What is this?** Fetch is a small program, `fetch-mcp`, that will download a web page and turn it into text for an AI tool. **Who needs it?** Contributors who want to test or measure it. It is not ready for ordinary users. **What to do first:** read the status below, then follow the Quickstart in the benchmark guide.

## Status: pre-MVP (early, unfinished)

The `fetch-mcp` binary is a skeleton. It has no MCP server yet, so it does nothing useful. **Do not register it in a real MCP client.** (MCP, the Model Context Protocol, is how an AI tool talks to a helper program.)

The licence is not decided yet (open question OQ-7). An Apache-2.0 `LICENSE` file exists, and the project is marked `publish = false` (Cargo will refuse to publish it to the public crates.io registry) until the decision is made.

What has not been checked yet: the automatic checks (CI) ran once on GitHub and passed, but branch protection is not switched on, `cargo-audit` and native aarch64 measurement have not run, and the 10 MB idle and 40 MB peak targets are unverified. Details: [what has and has not been checked](docs/BENCHMARK.md#read-this-first-what-has-and-has-not-been-checked).

## Where to go next

| I want to... | Read |
|---|---|
| Run or understand the memory benchmark (start here) | [Benchmark guide: quickstart](docs/BENCHMARK.md#quickstart-contributors) |
| Look up a benchmark word (VmRSS, median, gate, ADVISORY_PASS ...) | [Benchmark glossary](docs/BENCHMARK.md#glossary) |
| Understand the automatic checks (CI) and the branch rules on `main` | [CI and branch protection](docs/ci-branch-protection.md) |
