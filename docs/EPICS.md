# Epics and Stories: Fetch MCP Server (Rust, low-memory ARM replacement)

Source: `docs/PRD.md` v0.2. Story IDs use the epic letter. Sizes are Fibonacci points (1,2,3,5,8); no story exceeds 8. "Spike" stories are time-boxed and produce a decision, not shipped features.

Assumed capacity: solo part-time, about 10 points per 2-week sprint; commitment capped at 80% (8 points).

## Ordering Rationale

1. **Risk first.** The whole project is justified by a memory claim (PRD Goal 1). The incumbent baseline (E-1) and the `rmcp`/crate spike (A-1) run first, because a failed memory case or an unworkable crate stack invalidates everything later. Cost of delay on these is negative: building features first risks wasted work.
2. **Measure early, not last.** The benchmark harness (E-2 to E-4) lands right after the first streaming fetch, so memory regressions are caught while the code is small. Trade-off: some harness effort is spent before all features exist; accepted.
3. **Value density.** Core fetch (A) delivers the replacement value. Network safety (B) is next because an unsafe fetcher cannot replace the incumbent. Config (C) and polish stories follow. robots.txt and content labelling are lowest value density and depend on open questions, so they go last.
4. **Packaging (D) split.** The aarch64 cross-build is proven in A-1 (risk), automated in D-2 mid-project, and docs/release finish at the end.

## Epic-to-Requirement Map

| Epic | PRD IDs |
|---|---|
| A Core fetch and conversion | FR-01, FR-02, FR-03, FR-04, FR-07 (size/timeout enforcement), FR-08, FR-09, FR-10, FR-13, FR-14, FR-16, NFR-01, NFR-02, NFR-08 |
| B Network safety | FR-05, FR-06, FR-11, NFR-04, NFR-07, Risk 2, Risk 5 |
| C Configuration and policy | FR-07, FR-12, FR-06 (allowlist) |
| D Packaging, ARM builds, docs | FR-15, NFR-05, NFR-06, NFR-09, NFR-13, NFR-15, US-8 |
| E Memory benchmarking and validation | NFR-10, NFR-11, NFR-12, NFR-14, Goals 1a-1d, US-7, US-9 |

---

# Epic E: Memory benchmarking and validation

**Epic Goal:** Prove, with reproducible measurements on ARM, that the Rust server uses less memory than the incumbent `mcp__fetch__fetch`, and keep it that way.

**Success Metric:** Report shows idle RSS and 5 MB-fetch peak RSS each at or below 50% of the incumbent on aarch64-linux; 50 MB-response test stays within 10% of the 5 MB peak; CI gate active.

**Out of Scope:** Optimizing the incumbent; CPU or latency benchmarking beyond NFR-02; non-ARM benchmark tuning.

### Story Map

| # | Story | Value | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| E-1 | Spike: baseline incumbent memory | Critical (go/no-go) | 3 | 1 | OQ-8, OQ-9 |
| E-2 | Benchmark harness and fixtures | High | 5 | 5 | A-3 |
| E-3 | Idle RSS measurement and target check | High | 2 | 6 | E-2, A-2 |
| E-4 | Peak RSS and boundedness checks | High | 3 | 6 | E-2, A-3, A-4 |
| E-5 | Comparative report and replace decision | High | 2 | 10 | E-3, E-4, A-7 |
| E-6 | CI regression gate on aarch64 | Medium | 3 | 11 | E-2, D-2 |

**MVP Slice:** E-1, E-2, E-3, E-4, E-5. Rationale: these prove or disprove the primary claim. E-6 protects it after release and can follow.

### E-1: Spike - baseline the incumbent's memory (3 pts) [SPIKE, time-box 2 days]
Maps to: NFR-10, NFR-11, Goals 1a, 1b, 2, 3.
As a solo developer replacing `mcp__fetch__fetch`, I want to measure the existing server's memory on ARM so that the new server's targets and the go/no-go decision rest on real numbers.
- Given the incumbent is installed on an aarch64-linux host, when it completes the MCP handshake and idles 30 s, then idle RSS (VmRSS) is recorded as the median of 10 runs.
- Given a local HTTP server serving a 5 MB HTML fixture, when the incumbent fetches it, then peak RSS (VmHWM) is recorded as the median of 10 runs.
- Given the same 50-URL curated set, when the incumbent runs it, then success rate, median token reduction, default `max_length`, robots.txt behavior and error message forms are recorded for parity.
- Given the numbers, when the spike ends, then a one-page result states incumbent version, host, RAM, the two baselines, proposed absolute caps for NFR-10/NFR-11, and a go/no-go recommendation.

### E-2: Benchmark harness and fixtures (5 pts)
Maps to: NFR-14, Goal 1d.
As the project owner, I want a one-command harness so that memory comparisons are repeatable.
- Given the repo, when I run the single harness command, then it starts a local fixture HTTP server, drives a target MCP server over stdio, and prints idle and peak RSS.
- Given the harness is pointed at either the incumbent or the new binary, when it runs, then it uses an identical client script and fixtures for both.
- Given a run, when results are produced, then each figure is the median of at least 10 runs with min and max shown.
- Given Linux and macOS hosts, when the harness runs, then it reads `/proc/<pid>/status` on Linux and `/usr/bin/time -l` on macOS.
- Given fixtures, when the harness starts, then it provides a 5 MB HTML page, a 50 MB streaming body, and a slow-drip response.

### E-3: Idle RSS measurement and target check (2 pts)
Maps to: NFR-10, US-7.
As a developer on ARM, I want idle memory verified against target so that a resident server stays small.
- Given the release binary on aarch64-linux, when idle 30 s after `initialize` and `tools/list`, then RSS is at or below 50% of the E-1 baseline and at or below the agreed absolute cap.
- Given the result exceeds the target, when the harness finishes, then it exits non-zero and prints the ratio.

### E-4: Peak RSS and boundedness checks (3 pts)
Maps to: NFR-11, NFR-12, FR-16, US-7.
As a developer on ARM, I want peak memory verified during fetches so that large pages cannot exhaust RAM.
- Given the 5 MB HTML fixture, when `fetch` runs, then peak RSS is at or below 50% of the E-1 baseline and at or below the agreed absolute cap.
- Given the 50 MB fixture with max size 5 MB, when `fetch` runs, then the call returns a size error and peak RSS is within 10% of the 5 MB-page peak.
- Given 10 concurrent fetches of the 5 MB page, when they run, then peak RSS is recorded and reported (NFR-08 documentation).
- Given allocator candidates from A-1, when compared here, then the chosen allocator and its RSS effect are recorded.

### E-5: Comparative report and replace decision (2 pts)
Maps to: Goals 1a-1d, US-9.
As the project owner, I want a written comparison so that I can decide to swap the incumbent in my Claude Code config.
- Given E-3 and E-4 results, when the report is generated, then it lists incumbent versus new for idle RSS, peak RSS, and 50 MB boundedness with ratios.
- Given the report shows every target met, when it is published to `docs/`, then it states "replace" and lists the config change needed.
- Given any target missed, when it is published, then it states the gap and the follow-up actions.

### E-6: CI regression gate on aarch64 (3 pts)
Maps to: NFR-14, NFR-15.
As the project owner, I want CI to fail on memory regressions so that the saving persists.
- Given a pull request, when CI runs on an aarch64 runner, then it executes the E-3 and E-4 checks against the built binary.
- Given idle or peak RSS exceeds the cap by more than 10%, when CI runs, then the job fails and prints the figures.
- Given no aarch64 runner is available, when CI runs, then the job is marked skipped with a reason, and the release checklist requires a manual run.

---

# Epic A: Core fetch and conversion

**Epic Goal:** A working `fetch` MCP tool in Rust (`rmcp`, stdio) that retrieves a URL through a streaming, size-bounded pipeline, converts HTML to markdown, paginates, and reports errors clearly.

**Success Metric:** 50-URL test set >= 95% success; median token reduction >= 50%; all Must FRs in this epic pass; parity with the incumbent's parameters.

**Out of Scope:** JS rendering, PDF/binary extraction, caching, batch fetch, authenticated fetch.

### Story Map

| # | Story | Value | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| A-1 | Spike: `rmcp`, HTTP, and HTML-to-markdown crate choice | Critical (risk) | 3 | 2 | E-1 in parallel |
| A-2 | Walking skeleton: stdio server with `fetch` schema | High | 3 | 3 | A-1 |
| A-3 | Streaming, size-bounded HTTP fetch | High | 5 | 4 | A-2 |
| A-4 | HTML to markdown conversion | High | 5 | 7 | A-3 |
| A-5 | Pagination with `max_length` and `start_index` | High | 3 | 7 | A-4 |
| A-6 | Content-type handling and `raw` mode | High | 3 | 8 | A-3 |
| A-7 | Cause-specific structured errors | High | 3 | 9 | A-3 |
| A-8 | Charset decoding and User-Agent | Medium | 2 | 12 | A-3 |
| A-9 | Final URL and status header | Low | 1 | 12 | A-3 |

**MVP Slice:** A-1 to A-7. Rationale: this is the minimum that reproduces the incumbent's core behavior (fetch, convert, paginate, raw, errors). A-8 (charset, UA) and A-9 (header) are refinements; UTF-8 default covers most pages.

### A-1: Spike - `rmcp`, HTTP client, and HTML-to-markdown choice (3 pts) [SPIKE, time-box 3 days]
Maps to: FR-01, FR-15, NFR-05, NFR-15, Risks 1, 3, 9.
As a solo developer, I want to validate the Rust crate stack early so that later stories rest on known-good choices.
- Given the `rmcp` crate, when a minimal stdio server exposing one tool is built, then a client completes `initialize` and `tools/list` and the chosen `rmcp` version and feature flags are recorded.
- Given `reqwest` and `hyper` both with `rustls`, when each streams a 5 MB body into a capped buffer, then RSS and binary size are recorded and one is selected with rationale.
- Given at least two HTML-to-markdown approaches (for example a DOM-based converter and a streaming rewriter such as `lol_html`), when each converts a 5 MB page and 10 sample pages, then output quality notes, peak RSS, and time are recorded and one is selected.
- Given glibc and one alternative allocator, when the spike binary runs under load, then the RSS difference is recorded.
- Given the chosen stack, when cross-built for `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, then both targets compile and the aarch64-linux binary runs the handshake on ARM hardware.
- Given the spike ends, when the result is written, then it lists chosen crates, direct-dependency count against NFR-05, and any changes proposed to PRD assumptions.

### A-2: Walking skeleton - stdio server with `fetch` schema (3 pts)
Maps to: FR-01, FR-02, FR-13, NFR-01.
As an MCP client developer, I want a registered `fetch` tool with a validated schema so that Claude Code can connect.
- Given the server starts, when a client sends `tools/list`, then exactly one tool `fetch` is returned with `url`, `max_length`, `start_index`, and `raw` in its JSON schema.
- Given `url` is missing, uses `file:` or `ftp:`, or numeric params are negative or non-integer, when `fetch` is called, then the call is rejected with a validation error naming the field.
- Given any request is logged, when the server runs, then all logs go to stderr and stdout carries only MCP protocol messages.
- Given an aarch64 host, when the server starts, then it reaches ready within 250 ms.
- Given Claude Code is configured with the binary path, when it lists tools, then `fetch` appears.

### A-3: Streaming, size-bounded HTTP fetch (5 pts)
Maps to: FR-07, FR-16, NFR-08.
As a developer on ARM, I want the body read as a stream and capped so that memory cannot grow past the size limit.
- Given a URL returning a 10 MB body and the default 5 MB limit, when `fetch` is called, then reading stops at the limit and the call returns a "too large" error.
- Given a server that never finishes sending, when 15 s elapse, then the call returns a timeout error and the connection is closed.
- Given a response with a `Content-Length` above the limit, when `fetch` is called, then it aborts before reading the body.
- Given 10 concurrent calls to different URLs, when they complete, then each result matches its own URL with no cross-contamination.
- Given a request, when it is sent, then no cookies, credentials or auth headers are included (NFR-07).

### A-4: HTML to markdown conversion (5 pts)
Maps to: FR-03, NFR-02.
As an LLM agent, I want clean markdown so that I spend fewer tokens.
- Given an HTML page with headings, links, lists and code blocks, when `fetch` is called, then those elements are preserved in markdown.
- Given `<script>`, `<style>` and hidden navigation chrome, when converted, then their text is absent.
- Given the 50-URL test set, when run, then median token reduction is at least 50% and no output contains `<script>` text.
- Given a 1 MB HTML page, when converted, then overhead is at most 500 ms p95 on aarch64.
- Given the converter, when used, then it sits behind a trait so it can be swapped.

### A-5: Pagination with `max_length` and `start_index` (3 pts)
Maps to: FR-02, FR-04.
As an LLM agent, I want to page through long content so that I never overflow my context.
- Given a 20,000-character page and `max_length=5000`, when four sequential calls use the returned `start_index`, then concatenated output equals the full text with no overlap or gap.
- Given content is truncated, when the result is returned, then it states the next `start_index`.
- Given `start_index` at or beyond the content length, when `fetch` is called, then the result is an empty-content message stating the total length, not a crash.
- Given multi-byte UTF-8 text, when a page boundary falls inside a character, then the split is on a character boundary.

### A-6: Content-type handling and `raw` mode (3 pts)
Maps to: FR-08, US-3.
As a developer, I want raw output and correct type handling so that JSON and text are usable and binaries are refused clearly.
- Given `raw: true` on an HTML page, when `fetch` is called, then the unconverted body is returned with the same truncation rules.
- Given a `text/*`, `application/json` or `application/xml` response, when `fetch` is called, then it is returned as text without HTML conversion.
- Given a PNG or PDF response, when `fetch` is called, then the result has `isError: true` and names the content type.
- Given a missing `Content-Type`, when the body begins with HTML markers, then it is treated as HTML; otherwise it is treated as text.

### A-7: Cause-specific structured errors (3 pts)
Maps to: FR-10, US-4.
As an LLM agent, I want distinct error messages so that I can choose the right recovery.
- Given HTTP 404, 500, DNS failure, timeout, blocked target, too-large response, or unsupported type, when `fetch` is called, then the result has `isError: true` and a message naming that cause.
- Given each cause, when tested, then a test asserts both the flag and the message text.
- Given an unexpected internal error, when it occurs, then the server returns a generic error result and keeps running without writing to stdout.

### A-8: Charset decoding and User-Agent (2 pts)
Maps to: FR-09.
As a developer, I want correct text decoding so that non-UTF-8 pages read properly.
- Given an ISO-8859-1 page declaring its charset in the header or a meta tag, when `fetch` is called, then output renders correctly.
- Given no charset, when `fetch` is called, then UTF-8 is assumed and invalid bytes are replaced, not fatal.
- Given a request, when sent, then it carries the configured descriptive `User-Agent`.

### A-9: Final URL and status header (1 pt)
Maps to: FR-14.
As an LLM agent, I want to know the final URL so that I can cite it.
- Given a redirect occurred, when `fetch` returns, then the text begins with the final URL and HTTP status.
- Given no redirect occurred, when `fetch` returns, then no header line is added.

---

# Epic B: Network safety

**Epic Goal:** The server cannot be steered into reaching loopback, private, link-local or metadata addresses, including by redirects or DNS tricks, and respects site crawl policy.

**Success Metric:** SSRF test suite 100% pass; >= 90% line coverage on SSRF, redirect and pagination logic; no connection attempted to a blocked address.

**Out of Scope:** Prompt-injection prevention (only labelling); WAF/bot-evasion; TLS pinning.

### Story Map

| # | Story | Value | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| B-1 | Block private, loopback, link-local on resolved IP | Critical | 5 | 8 | A-3 |
| B-2 | Encoded and IPv6 address forms | High | 3 | 9 | B-1 |
| B-3 | Redirect limit and per-hop revalidation | High | 3 | 8 | A-3, B-1 |
| B-4 | robots.txt enforcement | Low | 3 | 13 | A-3, OQ-3 |
| B-5 | SSRF suite and coverage gate | High | 3 | 9 | B-1, B-2, B-3 |
| B-6 | Untrusted-content labelling | Medium | 2 | 13 | A-4, OQ-5 |

**MVP Slice:** B-1, B-2, B-3, B-5. Rationale: an unguarded fetcher is not an acceptable replacement. B-4 and B-6 are policy items awaiting OQ-3 and OQ-5, so they follow.

### B-1: Block private, loopback and link-local on resolved IP (5 pts)
Maps to: FR-06, Risk 2.
As a home-lab operator, I want internal addresses refused so that a poisoned page cannot make the agent probe my LAN.
- Given a URL whose host resolves to loopback, RFC 1918, link-local (including 169.254.169.254) or IPv6 unique-local, when `fetch` is called, then it is refused with a "blocked" error before any connection is made.
- Given a DNS name that resolves to a private IP, when `fetch` is called, then it is refused.
- Given a name resolves to a public IP, when connecting, then the connection uses that validated IP so a second lookup cannot change it (rebinding defense).
- Given a name returns mixed public and private addresses, when `fetch` is called, then it is refused.

### B-2: Encoded and IPv6 address forms (3 pts)
Maps to: FR-06.
As a home-lab operator, I want obfuscated addresses caught so that encodings cannot bypass the block.
- Given hosts written as decimal (`2130706433`), hex (`0x7f000001`), octal, or short forms (`127.1`), when `fetch` is called, then they are refused.
- Given IPv6 loopback `::1`, IPv4-mapped `::ffff:127.0.0.1`, and unique-local `fc00::/7`, when `fetch` is called, then each is refused.
- Given the unspecified addresses `0.0.0.0` and `::`, when `fetch` is called, then they are refused.

### B-3: Redirect limit and per-hop revalidation (3 pts)
Maps to: FR-05, US-5.
As a home-lab operator, I want each redirect hop validated so that a public URL cannot bounce to an internal one.
- Given a public URL that redirects to `127.0.0.1`, when `fetch` is called, then the redirect is refused with a "blocked" error.
- Given a chain of 6 redirects, when `fetch` is called, then it fails with a "too many redirects" error; a chain of 5 succeeds.
- Given a redirect to a non-http(s) scheme, when `fetch` is called, then it is refused.

### B-4: robots.txt enforcement (3 pts)
Maps to: FR-11, OQ-3.
As a site-respecting developer, I want disallowed URLs refused so that the agent follows crawl rules.
- Given a robots.txt that disallows `/private`, when `fetch` targets `/private`, then it is refused with an explanatory error.
- Given `FETCH_IGNORE_ROBOTS=1`, when the same URL is fetched, then it is allowed.
- Given robots.txt is missing or returns 404, when `fetch` runs, then it proceeds.
- Given the robots.txt fetch itself, when made, then it passes the same SSRF checks and size cap (small, at most 512 KB).
- Blocked by OQ-3: default on or off is confirmed before this story starts.

### B-5: SSRF suite and coverage gate (3 pts)
Maps to: NFR-04, Goal 4.
As the project owner, I want an automated SSRF suite so that safety is proven and cannot regress.
- Given the test suite, when run, then it covers IPv4, IPv6, encoded forms, DNS-to-private, rebinding simulation and redirect cases and all pass.
- Given CI, when it runs, then line coverage of SSRF, redirect and pagination modules is at least 90% or the build fails.
- Given a new blocked-range case is added, when tests run, then it needs no harness change (table-driven cases).

### B-6: Untrusted-content labelling (2 pts)
Maps to: Risk 5, OQ-5.
As a developer, I want fetched content marked as untrusted so that the model treats embedded instructions with suspicion.
- Given OQ-5 is decided as "label", when `fetch` returns content, then it is wrapped or prefixed with a fixed untrusted-content notice.
- Given the notice, when pagination is used, then it does not shift `start_index` offsets.
- Given OQ-5 is decided as "no label", when the story is reviewed, then it is closed as won't-do.
- Blocked by OQ-5.

---

# Epic C: Configuration and policy

**Epic Goal:** Behavior limits and network policy are adjustable via environment variables with safe defaults and clear startup errors.

**Success Metric:** Every variable changes behavior in a test; invalid values fail startup with a clear message; defaults match the PRD.

**Out of Scope:** Config files, runtime reconfiguration, per-call overrides of security policy.

### Story Map

| # | Story | Value | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| C-1 | Environment variable parsing and validation | Medium | 3 | 11 | A-3 |
| C-2 | Private host allowlist | Medium | 2 | 10 | B-1, C-1, OQ-4 |
| C-3 | Configurable timeout and max size | Medium | 2 | 11 | C-1 |

**MVP Slice:** None required; defaults (15 s, 5 MB, block all private) suffice for the first swap. C-2 is promoted if OQ-4 says home-lab access is needed at v1.0.

### C-1: Environment variable parsing and validation (3 pts)
Maps to: FR-12.
As a developer, I want a single validated config so that misconfiguration is caught at startup.
- Given valid values for timeout, max size, user agent, allowed hosts and robots toggle, when the server starts, then it applies them.
- Given an invalid value (non-numeric timeout, negative size), when the server starts, then it exits non-zero with a message naming the variable and writing only to stderr.
- Given no variables, when the server starts, then it uses defaults: 15 s, 5 MB, block private, robots per OQ-3.

### C-2: Private host allowlist (2 pts)
Maps to: FR-06, FR-12, US-6, OQ-4.
As a home-lab operator, I want to allow named internal hosts so that the agent can use my own services deliberately.
- Given `FETCH_ALLOW_PRIVATE_HOSTS=nas.local`, when `fetch` targets `nas.local`, then it is permitted.
- Given the same setting, when `fetch` targets another private host or a redirect leads to one, then it is still refused.
- Given the allowlist is unset, when any private host is targeted, then it is refused.
- Blocked by OQ-4.

### C-3: Configurable timeout and max size (2 pts)
Maps to: FR-07, FR-12, US-6.
As a developer, I want to tune limits so that I can trade completeness against memory.
- Given `FETCH_TIMEOUT_MS=5000`, when a server takes longer, then the call fails with a timeout error.
- Given `FETCH_MAX_BYTES=1048576`, when a larger body is served, then reading stops at 1 MB with a size error.
- Given a configured max size, when peak RSS is measured, then it scales with the configured limit rather than the response size.

---

# Epic D: Packaging, ARM builds and docs

**Epic Goal:** A single-binary, dependency-light, ARM-first release with automated builds, tests on ARM, and documentation to swap it in for the incumbent.

**Success Metric:** Release artifacts for aarch64-linux and macOS arm64; CI green on aarch64; a new user registers the server in Claude Code from the README in under 5 minutes.

**Out of Scope:** Windows, 32-bit ARM, package-manager distribution (brew, apt), registry publishing.

### Story Map

| # | Story | Value | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| D-1 | Size- and memory-optimized release profile | High | 2 | 12 | A-3 |
| D-2 | ARM build pipeline (aarch64-linux, macOS arm64) | High | 5 | 12 | A-1, D-1 |
| D-3 | Test suite on aarch64 | High | 3 | 14 | D-2 |
| D-4 | README, install and migration guide | High | 2 | 14 | D-2 |
| D-5 | Tool description within 150 words | Low | 1 | 14 | A-5 |
| D-6 | Licensing, dependency audit and release tag | Medium | 2 | 15 | D-3, OQ-7 |

**MVP Slice:** D-1, D-2, D-4. Rationale: a swap needs a buildable ARM binary and install steps; D-3 can be manual until then, D-5 and D-6 are v1.0 hygiene.

### D-1: Size- and memory-optimized release profile (2 pts)
Maps to: FR-15, NFR-13.
As a developer on ARM, I want a lean release build so that the binary and footprint stay small.
- Given `cargo build --release`, when it completes, then the profile uses LTO, `opt-level` chosen in A-1, `panic=abort` if compatible, and stripped symbols.
- Given the aarch64-linux release binary, when measured, then it is at most 10 MB (provisional).
- Given the binary, when inspected with `ldd`, then it links no OpenSSL and no runtime beyond libc.

### D-2: ARM build pipeline (5 pts)
Maps to: NFR-15, NFR-06, FR-15.
As a developer, I want CI to build ARM release binaries so that I can install without compiling.
- Given a tag push, when CI runs, then it produces stripped binaries for `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, with checksums.
- Given the aarch64-linux artifact, when run on a clean aarch64 container, then it completes the MCP handshake.
- Given an x86_64-linux build, when requested, then it is produced as best-effort and its failure does not block release.
- Given the Rust toolchain, when the MSRV is set, then it is pinned in `rust-toolchain.toml`.

### D-3: Test suite on aarch64 (3 pts)
Maps to: NFR-15, NFR-04.
As a developer, I want tests to run on real ARM so that ARM-specific issues are caught.
- Given a pull request, when CI runs, then unit and integration tests execute on an aarch64-linux runner.
- Given no native runner, when CI runs, then tests run under QEMU and the job states that RSS figures come only from native runs.
- Given a test failure on aarch64, when CI finishes, then the pipeline fails.

### D-4: README, install and migration guide (2 pts)
Maps to: US-8, Goal 6.
As a developer, I want clear install steps so that I can replace the incumbent quickly.
- Given the README, when followed on aarch64-linux and macOS arm64, then the server is registered in Claude Code and `fetch` works.
- Given the README, when read, then it lists environment variables, defaults, limitations (no JS, prompt-injection note), and a section "Replacing mcp__fetch__fetch" with the config diff and rollback.
- Given the benchmark report exists, when linked, then the README states the measured memory ratio.

### D-5: Tool description within 150 words (1 pt)
Maps to: NFR-09.
As an LLM agent, I want a concise tool description so that I use pagination correctly.
- Given the tool description, when counted, then it is at most 150 words.
- Given the description, when read, then it names purpose, all parameters, and the `start_index` continuation pattern.

### D-6: Licensing, dependency audit and release tag (2 pts)
Maps to: NFR-05, OQ-7.
As the project owner, I want a clean v1.0 tag so that the release is auditable.
- Given the repo, when `cargo audit` and `cargo deny` run, then neither reports high or critical issues.
- Given a licence is selected per OQ-7, when the release is tagged, then LICENSE is present and direct dependencies are at most 15.
- Given all PRD Goals are met, when v1.0 is tagged, then the release notes link the benchmark report.

---

# Overall MVP Slice (cross-epic)

**Definition:** "Safe to swap in for `mcp__fetch__fetch` on my ARM machines, with the memory claim proven."

Included (63 points): E-1, A-1, A-2, A-3, E-2, A-4, A-5, A-6, E-3, E-4, A-7, B-1, B-3, B-2, B-5, D-1, D-2, D-4, E-5.

Deferred to post-MVP (v1.0 completion): A-8, A-9, C-1, C-2, C-3, B-4, B-6, D-3, D-5, D-6, E-6.

Rationale: the MVP contains every story needed to (a) prove the memory case, (b) match the incumbent's core behavior, (c) refuse internal addresses, and (d) install on ARM. Charset handling, config knobs, robots.txt and labelling do not affect the swap decision. Trade-off: MVP ships with fixed defaults and UTF-8-only decoding; acceptable for personal use. Cost of adding A-8 early is low (2 pts) and it can be pulled forward if the test set shows encoding failures.

**Decision gates:** (1) End of Sprint 0: go/no-go on the memory target from E-1 and A-1. (2) End of Sprint 4: E-3 and E-4 confirm targets before investing in safety and packaging.

# Suggested Sprint Order

Assumes 2-week sprints, 10 points capacity, at most 8 committed.

| Sprint | Goal | Stories | Points |
|---|---|---|---|
| 0 | De-risk: incumbent baseline and crate stack; go/no-go | E-1, A-1 | 6 |
| 1 | Walking skeleton and bounded streaming fetch in Claude Code | A-2, A-3 | 8 |
| 2 | Convert and paginate | A-4, A-5 | 8 |
| 3 | Harness and content types | E-2, A-6 | 8 |
| 4 | Memory gate: idle/peak verified, errors clear | E-3, E-4, A-7 | 8 |
| 5 | Redirect and private-IP safety | B-1, B-3 | 8 |
| 6 | Hardening and allowlist (if OQ-4 yes) | B-2, B-5, C-2 | 8 |
| 7 | Config and text polish | C-1, C-3, A-8, A-9 | 8 |
| 8 | ARM release pipeline | D-1, D-2 | 7 |
| 9 | ARM tests, docs, report | D-3, D-4, D-5, E-5 | 8 |
| 10 | Policy items and CI gate | B-4, B-6, E-6 | 8 |
| 11 | v1.0 | D-6 (plus buffer, rework) | 2 |

Overall MVP is reached at the end of Sprint 9 in this ordering (E-5 lands there). To reach MVP sooner, move D-1, D-2 into Sprint 7 (ahead of C-1/C-3/A-8/A-9) and D-4, E-5 into Sprint 8, giving MVP at the end of Sprint 8 at the cost of delaying config.

Trade-off: Sprint 6 sits below the 80% ceiling only if C-2 is dropped (OQ-4 "no"); it is exactly 8 with it.

# Story-to-Requirement Traceability

| Requirement | Stories |
|---|---|
| FR-01 | A-1, A-2 |
| FR-02 | A-2, A-5 |
| FR-03 | A-4 |
| FR-04 | A-5 |
| FR-05 | B-3 |
| FR-06 | B-1, B-2, C-2 |
| FR-07 | A-3, C-3 |
| FR-08 | A-6 |
| FR-09 | A-8 |
| FR-10 | A-7 |
| FR-11 | B-4 |
| FR-12 | C-1, C-2, C-3 |
| FR-13 | A-2 |
| FR-14 | A-9 |
| FR-15 | A-1, D-1, D-2 |
| FR-16 | A-3, E-4 |
| NFR-01 | A-2 |
| NFR-02 | A-4 |
| NFR-04 | B-5, D-3 |
| NFR-05 | A-1, D-6 |
| NFR-06, NFR-15 | D-2, D-3, E-6 |
| NFR-07 | A-3 |
| NFR-08 | A-3, E-4 |
| NFR-09 | D-5 |
| NFR-10 | E-1, E-3 |
| NFR-11, NFR-12 | E-1, E-4 |
| NFR-13 | D-1 |
| NFR-14 | E-2, E-6 |

# Open Items Affecting This Plan

| # | Item | Owner | Blocks |
|---|---|---|---|
| OQ-3 | robots.txt default | Michael | B-4, C-1 default |
| OQ-4 | Private-host allowlist needed | Michael | C-2 (Sprint 6) |
| OQ-5 | Untrusted-content labelling | Michael | B-6 |
| OQ-7 | Distribution and licence | Michael | D-6 |
| OQ-8 | Exact incumbent and schema compatibility target | Michael | E-1 |
| OQ-9 | Native aarch64 host/runner for benchmarks | Michael | E-1, E-6, D-3 |
