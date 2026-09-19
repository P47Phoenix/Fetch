# Product Requirements Document

| Field | Value |
|---|---|
| Product/Feature | Fetch MCP Server (`fetch` tool), Rust, low-memory MCP server for ARM |
| Version | 0.4 (Draft) |
| Author | Michael Connelly |
| Status | Draft - OQ-1, OQ-2, OQ-8 and OQ-9 resolved; Stage 4 architecture changes applied (v0.4), E-4 size-error rule accepted; OQ-3, OQ-4, OQ-5 and OQ-7 still open |
| Last Updated | 2026-09-19 |

## 1. Problem Statement

LLM clients cannot read live web pages on their own. They need a tool that takes a URL and returns content the model can use: clean text, bounded in size, and safe to run from the developer's machine or network.

The author runs MCP servers on ARM hardware (aarch64 Linux and Apple Silicon). On small ARM machines the memory cost of an always-resident MCP server matters. **The purpose of this project is a new fetch MCP server that stays within absolute memory targets on ARM** (idle <= 10 MB RSS; peak <= 40 MB while fetching a 5 MB page), with reliable functional behavior.

Secondary concerns carry over from the original draft. Raw HTTP responses are a poor fit for LLMs: HTML is token-heavy, large pages overflow context, and a naive fetcher can be steered to internal network addresses (SSRF) by prompt-injected content.

The product is a small, local MCP server exposing one `fetch` tool. It is written in Rust with the official MCP Rust SDK (`rmcp`), ships as a single binary, streams and bounds all buffering so memory is capped by the configured max download size, converts HTML to markdown, paginates large responses, and blocks unsafe targets by default.

**Decisions recorded**
- OQ-1 (resolved): the goal is a new fetch MCP server with a small memory footprint on ARM. SSRF protection, determinism and code ownership are secondary benefits.
- OQ-2 (resolved): Rust with the official SDK, https://github.com/modelcontextprotocol/rust-sdk (crate `rmcp`), stdio transport.
- OQ-8 (resolved 2026-09-19): this is a brand-new server, not a replacement for `mcp__fetch__fetch`; there is no incumbent to baseline. The parameter schema (`url`, `max_length`, `start_index`, `raw`) is the default design, not a compatibility contract.
- OQ-9 (resolved 2026-09-19): benchmarks run on the author's native aarch64 runner on their cluster. RAM and OS are to be recorded in the benchmark report. Trust model: the runner is ephemeral or destroyed after each job, unprivileged, holds no cluster credentials or secrets, is unreachable from fork pull requests, and has no route to the LAN or the protected home-lab hosts; a skipped or absent benchmark job blocks release (architecture 9.2).
- Size rule (accepted 2026-09-19, architecture 6.4): a `Content-Length` above the max download size returns `too_large` immediately. With no `Content-Length` the body is streamed; the call succeeds if the requested window completes under the cap, and returns `too_large` if the cap is reached first.

**Assumptions (adjustable)**
- Async runtime is `tokio`; HTTP client is `reqwest` or `hyper` with `rustls` (no OpenSSL); HTML-to-markdown crate is TBD. All crate choices are unvalidated and are decided in the spike (Epic A, story A-1).
- Memory targets are absolute (idle <= 10 MB RSS; peak <= 40 MB VmHWM fetching a 5 MB page; median of 10 runs). The benchmark harness and targets are defined in the Sprint 0 story E-1.
- Transport is stdio; single user; runs locally.
- Target clients are Claude Code and Claude Desktop.
- Primary platforms: aarch64-linux and macOS arm64. x86_64-linux is best-effort.

## 2. Goals & Success Metrics

Goal 1 is the primary goal. Goals 2-6 are guardrails that the server must meet to be usable.

| # | Goal | Metric | Target | Baseline |
|---|---|---|---|---|
| 1a | **Lower idle memory on ARM** | Idle RSS (after MCP `initialize` + `tools/list`, 30 s settle) on aarch64-linux | <= 10 MB RSS (VmRSS), median of 10 runs | none - new |
| 1b | **Lower peak memory while fetching** | Peak RSS (VmHWM) fetching a 5 MB HTML page on aarch64-linux | <= 40 MB VmHWM, median of 10 runs | none - new |
| 1c | **Memory capped by max size** | Peak RSS growth when the server returns a 50 MB body with max size 5 MB | Within 10% of the 5 MB-page peak (no unbounded buffering) | none - new |
| 1d | **Claim proven, not asserted** | Reproducible benchmark against the absolute targets, published in repo | Report exists; run in CI on aarch64 | none - new |
| 2 | Reliable page retrieval | Success rate on a 50-URL curated test set (static HTML, JSON, plain text, redirects) | >= 95% | none - new |
| 3 | Token-efficient output | Median token reduction, HTML to markdown, on the test set | >= 50% | none - new |
| 4 | Safe by default | Private/loopback/link-local targets blocked in SSRF test suite | 100% of cases | none - new |
| 5 | Responsive | p95 overhead for pages under 1 MB, excluding remote server time | <= 500 ms | none - new |
| 6 | Works in real clients | Server registers and tool call succeeds in Claude Code on aarch64-linux and macOS arm64, by v1.0 | Yes | none - new |

Decision rule: if the spikes show the Rust server cannot reach 1a and 1b, the project is stopped or re-scoped at the go/no-go gate G0 (end of Sprint 0) or the memory gate G4 (end of Sprint 4), before safety and packaging work (see Section 9).

## 3. User Personas

**Primary: Solo developer on ARM hardware (Michael Connelly).** Runs Claude Code on aarch64 Linux and Apple Silicon, including memory-constrained machines. Wants a resident MCP server with a small footprint and a single binary to install. Cares about correctness, safety on a home network, and control over the code.

**Secondary: MCP client LLM agent.** The programmatic consumer of the tool. Needs a clear schema, predictable output, actionable error messages and pagination hints.

**Secondary: Home-lab operator.** The same person, with internal services (e.g. TrueNAS, Home Assistant) that the agent must not reach by accident.

## 4. User Stories (summary titles)

Detailed, sized stories with full acceptance criteria are in `docs/EPICS.md`. Summary:

- US-1: Retrieve a page as markdown
- US-2: Page through large content
- US-3: Get raw content when needed
- US-4: Understand failures
- US-5: Blocked from internal network
- US-6: Configure limits and policy
- US-7: Run a small resident server on ARM (idle and peak memory targets)
- US-8: Install as a single ARM binary
- US-9: See proof that the memory targets are met

### US-1: Retrieve a page as markdown
As a developer running an LLM agent, I want the agent to fetch a URL and receive markdown so that it can read the page cheaply.
- Given a public HTML page, when `fetch` is called with its URL, then the result is markdown with the main content and headings, links and code blocks preserved.
- Given the page has `<script>` and `<style>` content, when it is converted, then that content is absent from the output.

### US-2: Page through large content
As an LLM agent, I want to request content in chunks so that I can read long pages without overflowing my context.
- Given a page longer than `max_length`, when `fetch` is called, then the result contains the first `max_length` characters and a message with the next `start_index`.
- Given `start_index` from a prior result, when `fetch` is called again, then the result continues exactly where the prior one ended.

### US-3: Get raw content when needed
As a developer, I want a `raw` option so that I can inspect the original HTML, JSON or text without conversion.
- Given `raw: true`, when `fetch` is called, then the body is returned unconverted, with the same truncation rules.
- Given a JSON response, when `fetch` is called, then it is returned as text without HTML conversion.

### US-4: Understand failures
As an LLM agent, I want structured, actionable errors so that I can retry, change the URL, or tell the user.
- Given a 404, timeout, DNS failure, or blocked robots.txt, when `fetch` is called, then the result has `isError: true` and a message naming the cause.

### US-5: Blocked from internal network
As a home-lab operator, I want requests to private addresses refused so that a prompt-injected page cannot make the agent probe my LAN.
- Given a URL resolving to a private, loopback or link-local address, when `fetch` is called, then it is refused before any connection is made.
- Given a public URL that redirects to a private address, when `fetch` is called, then the redirect is refused.

### US-6: Configure limits and policy
As a developer, I want to set limits and an allowlist via environment variables so that I can tune behavior without editing code.
- Given `FETCH_ALLOW_PRIVATE_HOSTS=nas.local`, when `fetch` targets that host, then it is permitted.
- Given `FETCH_TIMEOUT_MS=5000`, when a server takes longer, then the call fails with a timeout error.

### US-7: Run a small resident server on ARM
As a solo developer on ARM hardware, I want the server to use little memory when idle and when fetching so that it does not compete with my other workloads.
- Given the server has completed the MCP handshake on aarch64-linux, when it is idle for 30 s, then its RSS is within the NFR-10 target.
- Given a 5 MB page is fetched, when peak RSS is measured, then it is within the NFR-11 target.
- Given a server response larger than the max size, when `fetch` is called, then memory stays bounded and the call returns a size error, unless the response has no `Content-Length` and the requested window completes under the max size (then it succeeds, still bounded).

### US-8: Install as a single ARM binary
As a developer, I want one self-contained binary for aarch64-linux and macOS arm64 so that I can register it in Claude Code without installing a runtime.
- Given a release artifact for my platform, when I copy it onto my PATH and register it, then Claude Code lists the `fetch` tool with no other installs.

### US-9: See proof that the memory targets are met
As the project owner, I want a reproducible benchmark so that I can release based on evidence.
- Given the benchmark harness, when it is run on aarch64-linux, then it prints idle and peak RSS and whether each meets its absolute target.

## 5. Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|---|---|---|---|
| FR-01 | The server must expose an MCP tool named `fetch` over stdio, implemented with the `rmcp` crate. | Must | `tools/list` returns exactly one tool `fetch` with a JSON schema; passes an MCP client handshake. |
| FR-02 | `fetch` must accept `url` (required, http/https only), `max_length` (default 5000, hard cap default 100,000 characters via `FETCH_MAX_LENGTH_CAP`; larger values are clamped and the result says so), `start_index` (default 0), and `raw` (default false). | Must | Schema validation rejects missing/invalid `url`, non-http(s) schemes (`file:`, `ftp:`), and negative or non-integer numbers; a `max_length` above the cap is clamped and the result states the clamp. |
| FR-03 | The server must convert HTML responses to markdown, stripping scripts, styles and navigation chrome where detectable. | Must | On the test set, output contains no `<script>` text; headings, links, lists and code blocks are preserved. |
| FR-04 | The server must truncate output at `max_length` characters from `start_index` and state the next `start_index` when truncated. | Must | For a 20,000-char page with `max_length=5000`, four sequential calls reproduce the full text with no overlap or gap. |
| FR-05 | The server must follow up to 5 redirects and re-validate every hop against the SSRF policy. | Must | A redirect chain of 6 fails with a clear error; a redirect to `127.0.0.1` is refused. |
| FR-06 | The server must block requests to loopback, private (RFC 1918), link-local (incl. 169.254.169.254), and unique-local IPv6 addresses by default, checked on the resolved IP. | Must | SSRF test suite passes for IPv4, IPv6, decimal/hex-encoded IPs, and a DNS name resolving to a private IP. |
| FR-07 | The server must apply a request timeout (default 15 s) and a maximum download size (default 5 MB, counting wire and decompressed bytes). | Must | A slow server returns a timeout error. Size behaviour, three cases: (1) a response with `Content-Length` above the limit returns a size error (`too_large`) before the body is read; (2) a chunked response with no `Content-Length` whose requested window completes under the limit succeeds; (3) a chunked response whose requested window would extend beyond the limit returns a size error at the limit. |
| FR-08 | The server must handle content types: convert `text/html`; return `text/*`, `application/json` and `application/xml` as text; reject other binary types with an error naming the type. | Must | A PNG and a PDF return `isError: true` with the content type; JSON returns as text. |
| FR-09 | The server must send a descriptive `User-Agent` and decode responses by charset from headers or meta tags, defaulting to UTF-8. | Should | A test page in ISO-8859-1 renders correctly; the request carries the configured UA. |
| FR-10 | The server must return errors as tool results with `isError: true` and a cause-specific message (HTTP status, DNS, timeout, blocked, too large, unsupported type). | Must | Each cause in the list has a test asserting the message and flag. |
| FR-11 | The server must support an optional robots.txt check, enabled by default, that refuses disallowed URLs with an explanatory error. | Should | With a robots.txt disallowing `/private`, a fetch of `/private` is refused; `FETCH_IGNORE_ROBOTS=1` allows it. |
| FR-12 | The server must read settings from environment variables: timeout, max size, `max_length` cap, concurrency, user agent, allowed private hosts, robots toggle. | Should | Each variable changes behavior in a test; invalid values fail startup with a clear message naming the variable. |
| FR-13 | The server must write logs to stderr only and must never write non-protocol output to stdout. | Must | A stdio test client receives no malformed messages while requests are logged. |
| FR-14 | The server must include the final URL (after redirects) and HTTP status in the result header. | Could | Result text begins with the final URL and status when redirects occurred. |
| FR-15 | The server must build as a single self-contained binary with no runtime (no Node, Python or system OpenSSL) required. | Must | `ldd`/`otool -L` on the release binary shows only libc/system libraries; the binary runs on a clean aarch64-linux container. |
| FR-16 | The server must read response bodies as a stream and stop reading at the max download size; it must not buffer more than max size plus a documented conversion overhead. | Must | A test server sending an unbounded/50 MB body causes an abort at max size and peak RSS stays within NFR-12. |

## 6. Non-Functional Requirements

Memory NFRs (NFR-10 to NFR-14) are the primary acceptance gates. Targets are absolute.

| ID | Requirement | Type | Target |
|---|---|---|---|
| NFR-01 | Server startup time | Performance | <= 250 ms to ready on aarch64 (tighter than the original 1 s, since no runtime boot) |
| NFR-02 | Conversion overhead for a 1 MB HTML page | Performance | <= 500 ms p95 |
| NFR-03 | Peak memory during a max-size fetch | Resource | Superseded by NFR-11 |
| NFR-04 | Unit and integration test coverage of SSRF, redirect, and pagination logic | Quality | >= 90% line coverage |
| NFR-05 | Direct dependencies | Maintainability | <= 15 crates (provisional, from spike); `cargo audit` and `cargo deny` clean at release |
| NFR-06 | Supported platforms | Compatibility | Primary: aarch64-linux (glibc, musl optional) and macOS arm64. Best-effort: x86_64-linux. Stable Rust, MSRV pinned |
| NFR-07 | Cookies, credentials and auth headers | Security | Not sent or stored; no persistent state |
| NFR-08 | Concurrent fetch calls | Reliability | 10 in flight without errors or cross-contamination; peak RSS with 10 in flight documented. Design default `FETCH_MAX_CONCURRENCY` = 3: excess calls queue (at most one timeout for a permit) and all complete |
| NFR-09 | Tool description | Usability | States purpose, parameters, and pagination usage in <= 150 words |
| NFR-10 | Idle RSS on aarch64-linux | Resource (primary) | <= 10 MB RSS (VmRSS), median of 10 runs |
| NFR-11 | Peak RSS fetching a 5 MB HTML page on aarch64-linux | Resource (primary) | <= 40 MB VmHWM, median of 10 runs |
| NFR-12 | Memory boundedness | Resource (primary) | Peak RSS with a 50 MB response and 5 MB max size within 10% of NFR-11 measurement |
| NFR-13 | Release binary size | Resource | <= 10 MB stripped, aarch64-linux (provisional) |
| NFR-14 | Benchmark reproducibility | Quality | Harness runs from one command; reports median of >= 10 runs; method: `/proc/<pid>/status` VmHWM/VmRSS on Linux, `/usr/bin/time -l` on macOS; run in CI on an aarch64 runner |
| NFR-15 | ARM build and test | Compatibility | CI builds release binaries for aarch64-linux and macOS arm64 and runs the full test suite on aarch64 (native runner or QEMU documented as fallback) |

Measurement protocol (applies to NFR-10 to NFR-12): same host, fixture served by a local HTTP server, one MCP client script; benchmark host (native aarch64 runner), OS, RAM, server version and commit recorded in the report.

## 7. Out of Scope

- JavaScript rendering or headless browser (SPA support). May be a later phase.
- Web search.
- Authenticated fetching: cookies, OAuth, custom headers.
- PDF, image, or other binary extraction (OCR, transcription).
- HTTP methods other than GET; form submission.
- Remote (HTTP/SSE) transport and multi-user hosting.
- Response caching.
- Multi-URL batch fetching and crawling.
- Publishing to an MCP registry (revisit after v1.0).
- Windows and 32-bit ARM builds.

## 8. Dependencies & Risks

| # | Dependency / Risk | Impact | Likelihood | Owner | Mitigation |
|---|---|---|---|---|---|
| 1 | `rmcp` API changes or missing features (pre-1.0 SDK) | Medium | Medium | Michael | Spike A-1; pin version; keep the transport layer thin. |
| 2 | DNS rebinding bypasses SSRF check | High | Low | Michael | Resolve once, connect to the validated IP (custom resolver/connector), re-check each redirect. Interim window: the full blocked-range table and checks land in A-3a (SSRF core, Sprint 1, before the first fetch-capable build A-3b), not B-1. Release rule: no tagged or distributed build before M3, and pre-M3 builds are not registered in a real MCP client. |
| 3 | Rust HTML-to-markdown crate quality or memory use (DOM-based crates may hold several times the page size) | High | Medium | Michael | Spike A-1 evaluates crates on quality and RSS; consider streaming rewriter (e.g. `lol_html`) or a size-capped DOM; keep converter behind a trait. |
| 4 | Absolute memory targets (10 MB idle, 40 MB peak) not achievable with the chosen crates | High | Low-Medium | Michael | Define harness and targets first (E-1) with go/no-go gate before feature work; stop or re-scope if 1a/1b cannot be met. |
| 5 | Prompt injection in fetched content | High | High | Michael | Cannot be eliminated by the server; document it and label output as untrusted content (see OQ-5). |
| 6 | Sites block bots or need JS | Low | High | Michael | Accept for v1; document limitation. |
| 7 | Solo developer time and no stated deadline | Low | Medium | Michael | Spike-first ordering; keep scope to the Must items first. |
| 8 | No aarch64 CI runner available or QEMU RSS numbers unrepresentative | Medium | Medium | Michael | Use the author's native aarch64 runner for benchmark numbers; QEMU only for functional tests. |
| 9 | Allocator choice affects RSS (glibc vs musl vs mimalloc/jemalloc) | Medium | Medium | Michael | Include allocator comparison in spike; fix choice in E-4. |
| 10 | Rust learning curve / build times for solo part-time dev | Low | Medium | Michael | Small crate set; incremental stories. |

## 9. Timeline & Milestones

No deadline stated; durations assume part-time solo work (about 10 story points per 2-week sprint, committing at most 8) and are estimates. See `docs/EPICS.md` for the sprint plan.

| Milestone | Target | Exit Criteria |
|---|---|---|
| M0: Decisions and spikes | Sprint 0 (Weeks 1-2) | OQ-1, OQ-2, OQ-8 and OQ-9 resolved (done). Benchmark harness and absolute memory targets defined (E-1). `rmcp` and crate choices validated, aarch64 cross-build proven (A-1). Hosted PR CI baseline live (D-7). Go/no-go on memory target recorded (G0). |
| M1: Walking skeleton | Sprints 1-2 (Weeks 3-6) | FR-01, FR-02, FR-13 pass (Sprint 1); SSRF core tested (A-3a); streaming bounded, SSRF-guarded fetch (FR-16, A-3b, Sprint 2); text returned in Claude Code via a throwaway config; 50-URL snapshots captured (E-7). |
| M2: Core fetch + memory gate | Sprints 3-6 (Weeks 7-14) | Harness live and idle RSS checked (Sprint 3); FR-03 and the memory gate G4, NFR-10 to NFR-12, at the end of Sprint 4 (decision gate: if not met, stop or re-scope); FR-04, FR-08 (Sprint 5); FR-10 (Sprint 6). |
| M3: Safety | Sprints 6-8 (Weeks 13-18) | FR-05 (B-3, Sprint 7), FR-06 (B-1/B-2, Sprints 6-7) pass; SSRF suite 100% and coverage gate (B-5) at the end of Sprint 8; NFR-04 met. Reached at the end of Sprint 8; no tagged build before it. |
| M4: Config, packaging, ARM | Sprints 9-11 (Weeks 19-24) | FR-15 (D-2, Sprint 9, MVP complete), FR-12 (Sprint 10), FR-09 (Sprint 11) pass; hosted CI green since Sprint 0, aarch64 test job (Sprint 11); README with ARM install steps (Sprint 9). |
| M5: v1.0 | Sprints 11-12 (Weeks 23-26) | FR-11 (B-4, Sprint 11), labelling per OQ-5 (Sprint 12), memory-gate CI (E-6), all Goals in Section 2 met; benchmark report published (E-5); NFR targets verified; tagged release (D-6, Sprint 12). |

## 10. Open Questions

| # | Question | Owner | Due | Status |
|---|---|---|---|---|
| 1 | Why build this instead of using the existing `mcp__fetch__fetch`? | Michael | - | **Resolved 2026-09-19:** build a new fetch MCP server with a small memory footprint on ARM. |
| 2 | TypeScript or Python SDK? | Michael | - | **Resolved 2026-09-19:** Rust with the official `rmcp` SDK, stdio transport. |
| 3 | Should robots.txt be enforced by default for agent-initiated fetches? | Michael | Before Sprint 11 (B-4; needed by C-1 default in Sprint 10 only as a placeholder) | Open |
| 4 | Is a private-host allowlist needed for home-lab use, or is blanket blocking acceptable? | Michael | Before Sprint 10 (C-2) | Open |
| 5 | Should output be wrapped or labelled as untrusted external content to mitigate prompt injection? | Michael | Before Sprint 2 (envelope of A-3b/A-4) | Open |
| 6 | Is a v1.1 headless-browser mode wanted, and if so as a separate tool? | Michael | After v1.0 | Open |
| 7 | Will this be distributed publicly (crates.io, GitHub releases, registry), which affects licensing and docs? | Michael | Before Sprint 9 (D-2, D-4; then D-6 in Sprint 12) | Open |
| 8 | (New) Which incumbent is being replaced, and is its parameter schema the compatibility target? | Michael | Before E-1 | **Resolved 2026-09-19:** not a replacement; no incumbent. Schema `url`, `max_length`, `start_index`, `raw` stays as the default design, not a compatibility contract. |
| 9 | (New) Is a native aarch64-linux host or runner available for benchmarking, and what is its RAM? | Michael | Before E-1 | **Resolved 2026-09-19:** the author's native aarch64 runner on their cluster; RAM and OS to be recorded later. |
