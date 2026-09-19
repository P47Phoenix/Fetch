# Architecture: Fetch MCP Server (Rust, stdio, low-memory ARM)

Stage 4 (Architect). Roles: Solution Architect + Security Architect (SSRF). Inputs: `docs/PRD.md` v0.3, `docs/EPICS.md`, A-1 spike report, `spikes/a1/src/main.rs`.
Status: Revision 2 (post-plan, 2026-09-19) on top of Revision 1 (self-correction round 1 of 3); see Revision log at the end. Revision 2 aligns this document with the user-approved Sprint Plan (`.delivery/artifacts/05-plan/po/sprint-plan.md`, 34 stories): bench-loopback build, G4a/G4b split of the memory gate, release profile pin. No open question is decided. Scope note: EPICS has 30 stories (A-1..A-9, B-1..B-6, C-1..C-3, D-1..D-6, E-1..E-6), not 32.

Measurement labels: "spike" = x86_64 measurement from the A-1 report (Intel, glibc, plain HTTP, one synthetic 5,243,433 B fixture). "budget" = design allocation, NOT a measurement. No aarch64 RSS exists yet. Every ARM-dependent choice is listed in section 14 and in the ADRs.

## 1. Prior Art Analysis

Read in full: PRD v0.3, EPICS, A-1 report, spike source.

| Spec element | Classification | Rationale |
|---|---|---|
| New tool, not a replacement; no incumbent baseline (OQ-8) | Decision already made | PRD 1 |
| Rust, official `rmcp`, stdio (OQ-2) | Decision already made | spike pinned rmcp 3.4.0 |
| Absolute targets: idle <= 10 MB VmRSS, peak <= 40 MB VmHWM on 5 MB page, median of 10; 50 MB body bounded within 10% (NFR-10..12) | Decision already made | PRD 6 |
| Params `url`, `max_length`, `start_index`, `raw`; defaults 5000/0/false | Decision already made (default design, not compat contract) | FR-02, OQ-8 |
| Benchmarks on author's native aarch64 runner (OQ-9) | Decision already made | PRD 1 |
| Streaming/bounded buffering, FR-16 | Decision already made | PRD 5 |
| DOM converters unusable (htmd 56 MB, html2md 56 MB, html2text 185 MB) | Spike-proven | A-1 3a |
| Streaming reqwest + lol_html `send` API: 6.1 / 8.7 / 11.1 MB | Spike-proven (x86, crude emitter) | A-1 3b |
| mimalloc regresses idle to ~11 MB | Spike-proven | A-1 3a |
| ureq needs spawn_blocking | Spike-proven | A-1 8 |
| cargo-zigbuild for aarch64 gnu.2.17 + musl | Spike-proven (build only) | A-1 4 |
| HTTP client (reqwest vs hyper), converter internals, readability approach, SSRF mechanics, allocator/libc, pagination semantics over stream | Open (architect fills) | ADR-001..006 |
| OQ-3 robots.txt default | Open, human decision | design keeps a switch point, does not decide |
| OQ-4 private-host allowlist | Open, human decision | design supports both outcomes |
| OQ-5 untrusted-content labelling | Open, human decision | design supports both outcomes |
| OQ-7 distribution/licence | Open, human decision | design lists impact only |
| gnu vs musl on ARM | Open, needs ARM data | ADR-005 |

Deviation protocol: no "Decision already made" element is contradicted. One PRD/EPICS consistency note is raised in section 6.4 (size error vs early stop) for the PO, not decided here.

Domain Discovery: the PRD/EPICS give sufficient domain detail (single-user local tool, no data store). Stated assumptions: single user, one MCP client, trusted local operator, untrusted network content. Risk: assumptions unapproved by a human beyond PRD wording.

## 2. Architecture Decision Summary

| ADR | Decision | Status |
|---|---|---|
| 001 | reqwest 0.13, rustls+ring, HTTP/1.1 only, gzip only, no proxy, manual redirects, no pool | Accepted (ARM check pending) |
| 002 | lol_html streaming tokenizer + own markdown emitter, bounded hold-back for main-content selection, behind `Converter` trait | Proposed (quality unproven) |
| 003 | Resolve once, validate all IPs, pin via custom resolver, per-hop revalidation, own IP tables | Accepted |
| 004 | Push pipeline with fixed byte/char budgets; manual gzip (flate2); early stop; raw path streamed too | Accepted for pipeline; the E-4 size-error vs early-stop rule is ACCEPTED (Product Owner, 2026-09-19; 6.4) |
| 005 | System allocator; ship gnu.2.17 primary + musl candidate; choice pending ARM | Proposed |
| 006 | Schema, character-based stateless pagination over streamed output | Accepted |

Style: modular monolith, single crate (workspace deferred; solo dev, `architecture.style: auto`). Paradigm: async I/O shell around a synchronous push-based pure core (converter, window, policy).

## 3. Module Layout

Single binary crate `fetch-mcp` (internal modules; extract crates only if compile time hurts).

```
src/
  main.rs        runtime bootstrap (tokio current_thread), config load, rustls provider install, serve stdio
  config.rs      env parsing -> immutable Config (validated at startup; exit non-zero on error)
  error.rs       FetchError enum, stable error codes, mapping to tool result text
  obs.rs         stderr logger, per-call log record, optional /proc/self/status memory sample
  server/
    mod.rs       rmcp #[tool_router] handler, FetchParams schema, concurrency semaphore, result rendering
    render.rs    result assembly: optional header (final URL/status, FR-14), content, pagination footer, optional untrusted label (OQ-5 hook)
  fetch/
    mod.rs       orchestrator: deadline, redirect loop, per-hop policy check, body pump
    client.rs    reqwest client construction (rustls, http1, gzip, no proxy, no cookies, no pool)
    redirect.rs  manual redirect state machine (max 5, scheme re-check, Location resolution)
    body.rs      byte-capped stream adapter (wire cap, decompressed cap, Content-Length precheck)
    charset.rs   BOM/header/meta prescan, streaming encoding_rs decoder to UTF-8
    ctype.rs     content-type classification (html / text / json / xml / binary / sniff)
    robots.rs    OPTIONAL (OQ-3): streamed robots.txt parse, only our UA group retained
  convert/
    mod.rs       Converter trait, Window sink, Progress/ControlFlow
    html.rs      lol_html send-API rewriter + markdown emitter state machine
    boilerplate.rs  element/attr/class-id drop rules, link-density rule, landmark holdback
    text.rs      passthrough converter (text/json/xml/raw)
    window.rs    start_index/max_length char windowing, UTF-8 safe
  ssrf/
    mod.rs       Policy (allowlist, port policy), check_url(), check_ip()
    ranges.rs    static blocked-range tables (v4, v6, embedded-v4 unwrap)
    resolver.rs  reqwest dns Resolve impl: resolve once, filter, return only validated addrs
```

Dependency rule: `server -> fetch -> {ssrf, convert, config, error}`. `convert` and `ssrf::ranges` are pure (no I/O, no tokio) and unit-testable without a runtime. `convert` never imports `fetch`. `error` is a leaf.

Direct dependency budget (NFR-05 <= 15, provisional): rmcp, tokio, reqwest, rustls (ring), lol_html, encoding_rs, serde, schemars, url, futures-util, an HTML entity decoder (crate TBD at A-4), webpki-roots (if ADR-001 root choice holds), one error helper (thiserror or hand-rolled). plus `flate2` (pure-Rust backend, push-style bounded gzip decode; reqwest's own `gzip` feature is OFF, see ADR-004). That is 14, leaving 1 spare. `ring` and `webpki-roots` pins in 9.1: `ring` is a transitive dependency of `rustls` (not a direct dependency; it is pinned through the committed `Cargo.lock` and `--locked` builds, not as a Cargo.toml direct requirement), so it does not change the count; `rustls` is the counted entry. If a direct `ring` line is ever added, the count becomes 15 of 15 and needs an ADR note. serde_json only if rmcp requires it directly. No direct `tracing`, no `anyhow`, no `regex`, no `ipnet` (own tables). rmcp may pull `tracing` transitively: not counted as direct, but A-2 must confirm with `cargo tree -i tracing` and record its idle-RSS and binary-size effect; if it is pulled in, no subscriber is installed so it is inert. Dev-deps are not counted.

## 4. Data Flow

```
MCP client --stdio--> rmcp --> server::fetch(params)
   validate params (schema) ; acquire semaphore permit
   |
   v
fetch::run (single deadline = FETCH_TIMEOUT_MS covers everything below)
   1. ssrf::check_url(url): scheme in {http,https}; url-crate host parse (normalises decimal/hex/octal/short IPv4);
      IP-literal hosts checked here (connectors skip DNS for literals); port policy
   2. [optional OQ-3] robots check (same client, same policy, <= 512 KB)
   3. reqwest GET (redirects OFF) --> ssrf::resolver: resolve ONCE, validate ALL IPs, return validated set only
        Connect goes to that set; no second lookup
   4. response headers: Content-Length precheck vs cap; status; Location -> back to step 1 (hop <= 5)
   5. ctype classify; unsupported -> error before reading body
   6. body pump (per chunk, no accumulation):
        wire chunk -> [gzip inflate, bounded step] -> cap counter -> charset decoder (UTF-8 out)
        -> Converter.push(&str) (html.rs | text.rs) -> Window.push(&str)
        Window.done() (start+max chars produced AND one more char seen) -> stop, drop body (connection closes)
        cap exceeded / deadline -> typed error
   7. Converter.finish(); Window yields (text, next_start | none, total_if_known)
   |
   v
server::render -> CallToolResult(text) ; errors -> isError:true text "error[code]: message"
```

Everything after step 5 is synchronous code called from the async loop; per-chunk work is bounded so a chunk never blocks the single-threaded runtime for long (chunk <= 64 KB budget).

## 5. Streaming and Bounded-Buffer Design (details in ADR-004)

Runtime: `tokio` `current_thread`, features `rt,macros,io-std,io-util,net,time` (as in spike). Tool futures must be `Send` (spike finding): lol_html must use `lol_html::send`; converter state lives in one owned struct, shared via `Arc<Mutex<..>>` only where the API forces it.

### 5.1 Memory budget (design allocations tied to the 40 MB peak)

Budget, not measurement. Unit is MiB (1 MiB = 1,048,576 B); the 40 MB gate is read as 40 MiB-equivalent VmHWM in the benchmark (E-1 defines MB once (10^6 or 2^20, stated in one place) and that single definition is used for every target, fixture and cap; using MB = 10^6 only makes the gate stricter by 4.6%, and the headroom below covers that. All gate figures are the median of 10 valid runs). Idle budget uses the NFR ceiling (10) even though spike idle is 4.6 MB, so a bad ARM idle result still leaves the peak budget intact.

Per-fetch items (all budgets, worst case per item):

| # | Item | Budget MiB | How enforced |
|---|---|---|---|
| a | Network read buffers + TLS records | 1.0 | HTTP/1.1 only, no pooling; verify hyper buffer defaults in A-3b (Sprint 2) |
| b | Wire chunk in flight | 0.0625 (64 KiB) | re-slice larger chunks; never `collect()` |
| c | gzip inflate state + step buffer (flate2) | 0.094 (32 KiB + 64 KiB) | bounded output steps |
| d | Charset prescan hold | 0.004 | prefix hold until decoder chosen |
| e | Decoder working buffer | 0.016 | encoding_rs streaming decoder |
| f | lol_html rewriter | 2.0 | `MemorySettings.max_allowed_memory_usage = 2 MiB`; exceed -> `converter_limit` |
| g | Markdown emitter state | 1.0 | depth 256; href <= 2 KiB; pending block <= 64 KiB; table row <= 64 KiB |
| h | Landmark holdback (ADR-002) | 0.25 | overflow -> flush, whole-body mode |
| i | Output window String | 0.38 (100,000 chars x 4 B = 0.4 MB) | `FETCH_MAX_LENGTH_CAP` default lowered from 200,000 to 100,000 (see Required doc changes); default `max_length` 5000 -> 0.02 |
| j | Result copy handed to rmcp (`CallToolResult` text clone) | 0.38 | ASSUMPTION: one clone; rmcp clone count is NOT verified. A-2 must measure by allocation count (counting allocator in a test) and this row is revised to the measured number |
| k | JSON-RPC serialised frame, worst-case escaping | 1.14 | worst case every char escapes to a 12-byte surrogate-pair `\uXXXX\uXXXX` sequence: 100,000 x 12 B = 1.2 MB. Typical text is ~1 B/char |

Peak arithmetic is done per scenario. The pipeline is dropped (network, decoder, lol_html, emitter freed) before serialisation, but the budget DOES NOT assume the allocator returns or reuses that memory; phases are summed additively (conservative).

Worked worst cases, per fetch, delta over idle:

| Scenario | Items | Sum MiB |
|---|---|---|
| S1 HTML, default `max_length` 5000, identity or gzip | a+b+c+d+e+f+g+h = 4.43; + window 0.02 + copy 0.02 + frame 0.06 | 4.5 |
| S2 HTML, `max_length` at cap 100,000, gzip, holdback full | 4.43 + i 0.38 + j 0.38 + k 1.14 | 6.3 |
| S3 `raw=true` or text/json/xml at cap (no lol_html, emitter, holdback, prescan) | a+b+c+e = 1.17; + 0.38 + 0.38 + 1.14 | 3.1 |
| S4 50 MB body (any encoding) | same as S1 or S2: no stage retains more than budgeted regardless of body size | 4.5 to 6.3 |
| S5 gzip bomb | as S2 (bounded inflate step, cap on decompressed bytes) | 6.3 |

Worst per-fetch delta W = 6.3 MiB (S2). Earlier draft said "6 MiB design ceiling" but summed items were about 8.2 MiB because it counted a 200,000-char cap (window 0.76 MiB, 3 MiB JSON copy) at the same time as the largest converter state; the honest figure at the old cap was ~8.5 MiB. Two changes close the arithmetic: cap 100,000 chars, and concurrency default 3.

Concurrency: PRD NFR-08 wants 10 in flight without errors. Uncapped: 10 + 10 x 6.3 = 73 MiB, over the gate. Decision (design default, tunable): `FETCH_MAX_CONCURRENCY` = 3. Worst case (all three permits in S2): 10 + 3 x 6.3 = 28.9 MiB budget, leaving 11.1 MiB headroom against 40 for allocator fragmentation, ARM page-size effects, TLS state and anything the spike did not exercise. Concurrency 4 would give 10 + 25.2 = 35.2 (4.8 headroom), judged too thin for an unmeasured ARM target; 5 gives 41.5 which fails. With 10 calls issued, 3 run and 7 queue and all complete (no errors). Peak with 10 issued is bounded by the same 28.9 budget; E-4 records the measured value.

Queue wait: to keep deadlines meaningful (Security NB-4) a queued call waits at most `FETCH_TIMEOUT_MS` for a permit, then returns `timeout` (message says "queued too long"); the fetch deadline starts at permit acquisition. Worst caller-observed latency is therefore 2 x FETCH_TIMEOUT_MS. Also cap the tokio blocking pool (`max_blocking_threads` = 4) so timed-out `getaddrinfo` calls cannot accumulate threads.

Spike reference (x86, plain HTTP, crude emitter): 6.1 MB first page, 8.7 MB deep page peak absolute (idle 4.6 => delta 1.5 and 4.1 MB), 11.1 MB raw buffered (removed). The budget is deliberately larger than the spike deltas because the spike emitter is incomplete and TLS/gzip were not exercised.

Body cap: `FETCH_MAX_BYTES` default 5 MiB (5,242,880) applies to wire bytes AND to decompressed bytes fed downstream. Because decoding is done by us (flate2 on the raw wire stream, ADR-004), both counters are observable. A 50 MB body therefore cannot cost more than the 5 MB body plus one chunk.

### 5.2 Early stop

Consequence for measurement: early stop makes the default call cheap, so a benchmark that early-stops cannot demonstrate the NFR-11 peak. Section 11 defines the gating scenarios as ones that force (near) full consumption. The `Window` sink reports `done` as soon as it holds `max_length` chars after `start_index` plus one confirming char. The pump then drops the response (connection closed, no reuse). Deep pages (start_index large) must consume and discard up to `start_index` chars, so cost is O(start_index) time but O(max_length) memory. Spike deep page (start 2,000,000): 8.7 MB peak.

### 5.3 Raw path

`raw=true` and non-HTML text (json/xml/plain) use `convert::text` through the same decoder and `Window`. No full-body buffering (the spike's 11.1 MB raw path was buffered; that path is removed).

## 6. Error Model

### 6.1 Types

`FetchError` (error.rs), each variant has a stable `code` string, a message safe for the model, and a `retryable` hint.

| code | Cause | Notes |
|---|---|---|
| `invalid_argument` | schema/param violation, INCLUDING non-http(s) scheme or userinfo in the initial `url` (single rule, matches ADR-006) | protocol-level validation error (JSON-RPC invalid params) naming the field; not an `isError` result |
| `blocked_target` | port policy, private/loopback/link-local/metadata address, redirect to blocked, redirect `Location` with non-http(s) scheme | message gives category only ("resolves to a non-public address"), never the resolved IP or internal detail |
| `robots_disallowed` | robots.txt (only if OQ-3 enables) | |
| `dns_failure` | NXDOMAIN, resolver error | |
| `connect_failed` / `tls_failure` | TCP or handshake failure | |
| `timeout` | overall deadline | |
| `http_status` | non-2xx after redirects | includes status |
| `too_many_redirects` | > 5 hops | |
| `too_large` | Content-Length > cap, or wire/decompressed cap hit before window complete | |
| `unsupported_content_type` | binary types | names the type |
| `unsupported_encoding` | Content-Encoding other than identity or single gzip; checked from the header BEFORE decode | |
| `converter_limit` | lol_html/emitter memory or depth limit | suggests `raw=true` |
| `internal` | anything unexpected (panics are abort; this is for Err paths) | generic message, detail to stderr only |

### 6.2 Mapping

Runtime failures become `CallToolResult { isError: true, content: [text] }` with text `error[<code>]: <message>. <hint>`. Only parameter validation uses the protocol error path. Stdout is never written by application code; a clippy `print_stdout`/`print_stderr`-aware deny keeps `println!` out (FR-13). `panic = "abort"` (spike profile) means a panic terminates the process after a stderr message; the MCP client restarts it. Accepted trade-off for size/RSS; reviewed at D-1 if crash frequency matters.

### 6.3 Oracle avoidance

Blocked-target errors occur before any connection, so error text cannot reveal LAN topology. Blocked messages are identical for "resolves private" vs "IP literal private" except category words.

### 6.4 Size-error vs early-stop rule (ACCEPTED)

Status: ACCEPTED by the Product Owner on 2026-09-19 as proposed below; ADR-004 carries the same status. PRD FR-07 and EPICS A-3, E-2, E-4 are amended to the three fixtures.

EPICS E-4 expects a 50 MB body with a 5 MB cap to yield a size error (and A-3 expects a 10 MB body to yield "too large"; FR-07 acceptance is the same). With early stop, a chunked (no Content-Length) 50 MB HTML whose first window fits inside the first 5 MB legitimately succeeds. Proposed rule: Content-Length above cap -> `too_large` immediately; no Content-Length -> stream, succeed if the window completes before the cap, `too_large` if the cap is reached first. Proposed fixture set for E-4 (and E-2, A-3): (1) 50 MB with Content-Length -> `too_large`; (2) 50 MB chunked, window inside cap -> success; (3) 50 MB chunked, requested window beyond cap -> `too_large`. If the PO instead requires a size error for every over-cap body, early stop must be disabled for over-cap chunked bodies (read to cap), which is memory-neutral but costs time. Please confirm before Stage 5 planning.

## 7. Configuration Surface

All env vars, parsed once in `config.rs` into an immutable struct. Invalid value -> stderr message naming the variable, exit non-zero (C-1). No config file, no per-call override of security policy.

| Variable | Default | Notes |
|---|---|---|
| `FETCH_TIMEOUT_MS` | 15000 | overall deadline (DNS through last body byte, all redirects) |
| `FETCH_MAX_BYTES` | 5242880 | wire and decompressed cap |
| `FETCH_USER_AGENT` | `fetch-mcp/<version> (+repo url TBD by OQ-7)` | descriptive |
| `FETCH_ALLOW_PRIVATE_HOSTS` | empty | comma-separated hostnames; OQ-4 decides whether feature ships (see 12) |
| `FETCH_IGNORE_ROBOTS` | per OQ-3 | only meaningful if robots.rs ships |
| `FETCH_MAX_CONCURRENCY` | 3 | derived from section 5.1 (10 + 3 x 6.3 = 28.9 MiB budget) |
| `FETCH_MAX_LENGTH_CAP` | 100000 | ceiling for `max_length` (was 200000; see 5.1) |
| `FETCH_LOG` | `warn` | `error|warn|info|debug` to stderr |
| `FETCH_ALLOWED_PORTS` | unset (any port) | optional restriction, ADR-003 |

Invalid `FETCH_LOG` values follow the same exit-non-zero rule. Config errors exit before the MCP handshake, so the client only shows a generic spawn failure; the stderr text naming the variable is the diagnostic (document in D-4). CLI: `--version` prints crate version, commit, target triple and libc; startup writes one `info` line to stderr (feeds the benchmark report). Known limit: `no_proxy()` is fixed, so proxy-only networks are unsupported (document in D-4).

Fixed constants (not configurable): max redirects 5, robots cap 512 KB, HTTP/1.1, no cookies/credentials (NFR-07), no proxy.

## 8. Observability

- Stderr only. Key=value lines: `level ts call_id event ...`. Hand-rolled minimal logger (avoids `tracing-subscriber` weight, keeping idle low and crates <= 15).
- Per-call record at `info`: call_id, scheme+host only (path, query and fragment logged only at `debug`; paths can carry tokens, Security NB-8); error text never echoes a full URL with query, hop count, final status, wire bytes, decompressed bytes, chars emitted, elapsed ms, outcome code.
- At `debug` on Linux: read `/proc/self/status` VmRSS/VmHWM at call end and log them. Cheap file read, gives field evidence for the memory claim between benchmark runs.
- No metrics endpoint, no telemetry, no network egress other than fetch targets.
- Never log response bodies or request headers.

## 9. Build and Release (aarch64)

- Profile: `opt-level="s"` (spike), `lto=true`, `codegen-units=1`, `panic="abort"`, `strip=true`. **Pinned early:** D-7 (Sprint 0) fixes these five settings in `Cargo.toml` so G0, G4a, G4b and E-5 measure what ships. D-1 (Sprint 8) finalises the profile (`opt-level` `3` vs `s` against NFR-02) and MUST re-measure idle and peak on the shipped build on the native aarch64 runner (gnu and musl) against 10 MB idle and 40 MB peak; a miss blocks MVP tagging (a failure re-plans Sprint 9). E-5 (Sprint 7) reports on the pinned profile and states that the D-1 re-measure still governs the tag. Install a panic hook that writes to stderr before abort; confirm rmcp itself never logs to stdout (A-2).
- Targets: `aarch64-unknown-linux-gnu.2.17` (glibc floor 2.17, dynamic), `aarch64-unknown-linux-musl` (static), `aarch64-apple-darwin` (native macOS runner, no zig needed), x86_64-linux best-effort. Release artifact names contain the target triple (`fetch-mcp-<version>-<triple>`), fixed now so D-4 docs do not churn. macOS arm64 binaries get ad-hoc codesign; notarization is out of scope (decide with OQ-7).
- Cross-build method proven in spike: `cargo-zigbuild` with `ziglang` pip package, ~35 s per target on x86 host, both exit 0. ring provider avoids the aws-lc C build; zig cc supplies the C compiler for ring.
- Runtime QEMU caveat: the gnu.2.17 binary cannot run under qemu-user without a loader (spike). QEMU functional tests therefore cover the musl binary only, or use `QEMU_LD_PREFIX` with an aarch64 sysroot. Spike showed qemu RSS is invalid (13.0/19.5 MB vs 4.7/6.1 MB native x86), so NEVER gate or publish memory from QEMU.
- FR-15 check in CI: `ldd`/`otool -L` (no OpenSSL; musl shows "statically linked") AND `objdump -T` asserting every required GLIBC symbol version is <= 2.17 (ldd alone does not show this).

### 9.1 Pinning and reproducibility (resolves DevOps B1)

| Item | Pin |
|---|---|
| Rust toolchain | `rust-toolchain.toml` with an exact `channel = "1.94.1"` (the version this design was reviewed on; `rustc 1.94.1 (e408947bf 2026-03-25)`), profile minimal, components `clippy`, `rustfmt`, plus the aarch64/macOS targets. `rust-version` (MSRV) in `Cargo.toml` set in D-2. |
| Toolchain bump procedure | bump is its own PR; it must re-run the full benchmark (section 11) on the native runner and attach the report; binary size and RSS deltas recorded; merge only if gates hold. |
| Cargo.lock | committed (binary crate); every CI/release command uses `--locked`. |
| High-churn direct deps | exact requirements (`=`): `rmcp = "=3.4.0"`, `reqwest = "=0.13.5"`, `lol_html = "=2.9.0"`, `rustls = "=0.23.45"`, `ring = "=0.17.14"`, `tokio = "=1.53.1"`, `encoding_rs = "=0.8.41"`, `webpki-roots = "=1.0.9"`, `url = "=2.5.8"`. Values are the versions in `spikes/a1/Cargo.lock`; changing any of them is a bump PR that re-runs the benchmark. `flate2` is pinned when first added in A-3. |
| Cross-build tools | `cargo-zigbuild` 0.23.4 and `ziglang` 0.16.0 pinned in the CI workflow (`cargo install --locked cargo-zigbuild --version 0.23.4`; `pip install ziglang==0.16.0`). |
| GitHub Actions | every `uses:` pinned by full commit SHA with a version comment; Dependabot (github-actions and cargo ecosystems) proposes bumps. |
| Bit-for-bit reproducibility | NOT a goal for v1. Builds use `--remap-path-prefix` and `SOURCE_DATE_EPOCH` to reduce noise, but SHA256 checksums attest only "what CI built", stated in the release notes. The benchmark reproducibility requirement (NFR-14) is met by the pinned toolchain plus the committed lockfile plus the report recording the binary sha. |

### 9.2 Runner topology and trust (resolves DevOps B2)

| Trigger | Hosted (github-hosted) jobs | Self-hosted aarch64 jobs |
|---|---|---|
| Pull request from same repo | fmt, clippy, unit/integration tests (x86_64), cross-build aarch64 gnu + musl, `cargo deny`, `cargo audit`, feature-guard checks, QEMU musl functional smoke | none by default. Reduced memory smoke may be run only after a maintainer applies a label (manual approval) |
| Pull request from a fork | same hosted jobs, no secrets | NEVER. The self-hosted label is unreachable from `pull_request` events of forks; the workflow contains no `pull_request_target` trigger for these jobs |
| Push to `main` | as above | full aarch64 suite (D-3) and full memory benchmark (E-2..E-4); stores the baseline |
| Tag (release) | build all release targets | full aarch64 suite and full memory benchmark; REQUIRED for release |
| Nightly schedule | `cargo audit` (advisories arrive without code changes) | full matrix benchmark (idle 30 s runs run in parallel processes; RSS is per-process) |
| `workflow_dispatch` | any | any (maintainer only) |

Runner rules: registered as ephemeral (one job per registration) or as a container/VM that is destroyed after the job; runs as an unprivileged user with no cluster credentials, no kubeconfig and no secrets mounted; network egress restricted to the fixture server on loopback plus the package registries needed for the build (build artifacts are produced on hosted runners and downloaded, so the runner does not need a build toolchain at all); no route to the LAN or to the TrueNAS/Home Assistant hosts named in the PRD as protected assets. The runner is dedicated during a benchmark (no other workload; load average recorded, section 11).

Availability: if the self-hosted runner is offline, jobs queue and time out, and the release is BLOCKED. Skipped is failure: branch protection marks the aarch64 test and memory-gate jobs as REQUIRED status checks on `main` and the release workflow `needs:` them, so a skipped or absent job cannot read as green. QEMU must never substitute for the memory gate. A documented manual fallback exists: the maintainer may run the same bench entry command on the same runner by hand and attach the report, which the release checklist accepts as equivalent.

- Supply chain: `cargo audit` per-PR and nightly. `deny.toml` sections: `advisories`; `bans` (deny `openssl`, `openssl-sys`, `native-tls`, `aws-lc-sys`, `aws-lc-rs` to enforce FR-15 and the ring decision); `sources` (crates.io only, no git deps); `licenses` (permissive allow-list now; the project's own licence waits on OQ-7). webpki-roots staleness (R11) is owned by Dependabot plus a release-checklist item.
- Release integrity: SHA256 checksums per tag; provenance attestations and an SBOM (`cargo cyclonedx`) recommended, decide with OQ-7. Distribution channel is OQ-7: `cargo install` builds from source on the user's machine (no zig; the RSS claim covers released binaries only); crates.io needs metadata and a licence. UA string embeds a repo URL that must be a build-time constant, not a TBD in code.
- Roots: see ADR-001 (webpki-roots embedded). Unmeasured on ARM.
- Feature-guard CI (R12): D-7 (Sprint 0, skeleton with a self-test positive control, `-p <crate>` release build) and D-2 (every release artifact) assert that `test-support`, `bench-loopback` and the bench-only fixture-CA feature (section 11 item 7) are all absent: `cargo tree -e features` on the release build PLUS a marker-string grep on the actual artifact (the `bench-loopback` build embeds a fixed marker string, so the grep is reliable). Cargo features are additive, so the guard is also a required CI check from Sprint 1. E-8 unit tests run WITH the feature in hosted CI.
- Bench build (`bench-loopback`, E-8, user-confirmed 2026-09-19): a second binary built from the same commit and Cargo.lock by the same pinned pipeline as the shipped binary; never a release artifact. Both binaries report the same commit and Cargo.lock hash (`--version` and startup line). It is off by default and permits only 127.0.0.0/8 and ::1 (ADR-003).

## 10. Testing Strategy

| Layer | Approach |
|---|---|
| Unit, pure | table-driven `ssrf::ranges` (IPv4, IPv6, IPv4-mapped, 6to4, NAT64, encodings); `window` UTF-8 boundary and concatenation; config parsing; content-type classification |
| Property | pagination: for random text and random `max_length`, concatenating sequential windows equals full text (FR-04); converter chunk-boundary invariance: splitting the body at every offset yields identical output (required by ADR-006 determinism) |
| Converter goldens | fixture pages for headings/links/lists/code/tables/entities/script/style/nav; snapshot files reviewed in PR; plus quality set (below) |
| Integration | in-process test HTTP server(s); fake `Resolve` implementation injected to simulate DNS-to-private, mixed public/private, rebinding (first answer public, second private; assert connection used the first) |
| Loopback fixtures | production policy blocks loopback, so tests use a test-only policy constructor behind `#[cfg(any(test, feature = "test-support"))]`; benchmarks use the `bench-loopback` build (11 item 7). It must NOT be reachable from env vars or default features (Security review item) |
| MCP e2e | stdio client script (rmcp client or Python): handshake, `tools/list` has exactly one tool, call, stdout contains only JSON-RPC while stderr logs (FR-13) |
| Security additions | each redirect hop builds a fresh request with no inherited headers; test asserts no Authorization/Cookie/Referer is sent on a cross-origin hop; gzip fixtures: stacked/unknown Content-Encoding rejected before decode, multi-member and trailing garbage; `168.63.129.16` blocked; https->http redirect surfaced in the Final URL header; error/log text contains no full URL query |
| Resource abuse | 50 MB with and without Content-Length; slow-drip; gzip bomb (small wire, huge output); 100k-deep nesting; single 10 MB attribute; header bomb; 6-hop redirect; redirect to `127.0.0.1` and to `file:` |
| Coverage | `cargo llvm-cov`, gate >= 90% lines on `ssrf`, `fetch::redirect`, `convert::window` (NFR-04, B-5) |
| Quality set | offline snapshots of the 50-URL curated set (E-1) measure CONVERSION success and token reduction (Goals 2, 3), not live fetch success: snapshot success != live success. Add a small non-gating live smoke run (10 URLs, documented) for network/TLS/redirect behaviour. Thresholds (95% success, 50% token reduction) are unproven until A-4 (ADR-002 Proposed); fallback if failing: whole-body mode, `raw=true` escape hatch, and PRD threshold review |
| ARM | full suite on native aarch64 runner (D-3); QEMU fallback for functional only |
| Fuzz (optional, post-MVP) | cargo-fuzz on converter with a memory-limit assertion |

## 11. Memory-Benchmark Approach

Promote the spike harness (`bench/gen_fixture.py`, `serve.py`, `measure.py`) into `bench/` with one entry command (E-2). Method per PRD/NFR-14:

1. Start fixture server; spawn server binary; MCP `initialize` + `tools/list`.
2. Idle: wait 30 s (spike used 0.5 s), read `VmRSS` from `/proc/<pid>/status`. Median of >= 10 fresh processes, print min/max. Idle samples may run in parallel processes (RSS is per-process) to keep the gate short.
3. Peak: one `fetch` per fresh process, read `VmHWM` after the call. Median of >= 10.
4. Scenarios, split into gating and non-gating (below).
5. Record per report: host CPU, kernel, `getconf PAGESIZE`, RAM, cgroup limits / container flag, load average before and after, CPU governor, libc (gnu/musl), allocator, binary sha, commit, cargo profile, toolchain version, fixture sha256s. macOS uses `/usr/bin/time -l` (not comparable to VmHWM; reported separately).
6. Gate decision uses the MEDIAN exactly as the PRD defines it (idle <= 10 MB, peak <= 40 MB, 50 MB run <= 1.10 x the 5 MB run). Exit non-zero on failure. E-3/E-5 release gate is the strict absolute target. E-6 CI is a regression tripwire with two conditions: fail if the median exceeds the absolute target at all, AND fail if the median regresses more than 10% versus the stored last-main baseline (baseline stored as a CI artifact / dedicated branch by the main-push job). The 10% is relative to the stored baseline, never a licence to exceed 40 MB; the two gates are not contradictory.
7. Bench harness reach and binaries (E-8, CONFIRMED by the user 2026-09-19): the shipped binary's fail-closed policy blocks the loopback fixture server, so the harness uses a second binary built with the compile-time Cargo feature `bench-loopback` (ADR-003; off by default; permits only 127.0.0.0/8 and ::1; every other blocked range still refuses; no runtime switch). Which binary each gate measures: IDLE RSS is gated on the SHIPPED binary (also recorded on the bench build); PEAK RSS (VmHWM) is gated on the BENCH build. To bound fidelity loss the record carries numeric bounds: idle delta bench vs shipped <= 0.5 MB; binary size delta recorded and explained; one public-host 5 MB fetch on the shipped binary within 10% of the bench peak and <= 40 MB; both binaries report the same commit and Cargo.lock hash. Exceeding a bound fails G4a. Figures in reports are labelled by binary. This does not decide OQ-4 (the shipped allowlist question is separate).
8. HTTPS coverage: plain-HTTP fixtures under-measure TLS. Recommendation for E-1: a bench-only build feature that trusts a fixture CA from an env var, never enabled in release (CI asserts absence, 9.2), with one comparison run showing the feature costs no measurable RSS. Alternative: measure TLS against real hosts once, manually. Decision left to E-1. Until decided, the E-5 report template carries the caveat "NFR-11 measured over plain HTTP only".
9. Release profile: the profile of section 9 (opt-level, lto, panic=abort, strip, codegen-units) is pinned in Sprint 0 (D-7) and every gate and benchmark uses it (with MB defined once in E-1 and the median of 10 valid runs; the harness rejects fewer). D-1 (Sprint 8) re-measures the finalised profile on the shipped build on aarch64 (gnu and musl) within 10 MB idle and 40 MB peak; a miss blocks MVP tagging.
10. Baseline note: PRD has no incumbent (OQ-8); the spike report's "E-1 incumbent baseline" item is obsolete.

### 11.0 Memory gate split: G0, G4a, G4b

Naming: the memory GATES are G0, G4a and G4b; the scenario IDs G1..G7 in 11.1 are a separate list (the plan's "G4" gate is not the G4 scenario below). Targets are unchanged and not lowered: idle <= 10 MB, peak <= 40 MB, gnu and musl, native aarch64 only, median of 10 valid runs, on the D-7 profile.

| Gate | When | Content |
|---|---|---|
| G0 | end Sprint 0 | A-1 idle <= 10 MB and 5 MB-fetch peak <= 40 MB on native aarch64 (glibc), or a written gap analysis; A-1 re-run under the E-1 protocol or the deviation recorded |
| G4a | end Sprint 4 | idle (shipped binary) plus the scenarios that need only A-3b and A-4 (full consumption, no window or early stop): (1) 5 MB HTML fully read and converted (G1/G2 read-in-full form), (2) same gzipped, (3) late-landmark holdback-full HTML (G4), (4) 50 MB with Content-Length -> `too_large` (G7a), (5) 50 MB chunked read beyond the cap without a window -> `too_large`; (4) and (5) within 10% of the 5 MB peak. 10-concurrent (G6) recorded, not gating. Includes the shipped-vs-bench record of item 7. Pass: continue to Sprint 5. Fail: stop feature work, memory-reduction sprint (allocator, buffer sizes, converter swap via trait) |
| G4b | end Sprint 5 (owned by A-5 and A-6; A-6 records the combined result) | scenarios that need A-5 (window at start with early stop, window at end, chunked window inside the cap succeeding (G7b), window beyond cap -> `too_large` (G5, G7c)) and A-6 (`raw=true`, G3), same 40 MB peak, idle re-checked at 10 MB on the shipped binary, the two 50 MB chunked window cases within 10% of the 5 MB peak. Fail: stop before Sprint 6 |

A G4a pass does not close the memory gate: the complete gate closes at the end of Sprint 5 (one sprint later than first told). Both need ARM runner access; without it they cannot be evaluated and the next sprint does not start. The G1..G7 definitions in 11.1 are the scenario definitions for both parts; where a scenario's window-at-end form needs A-5, its full-read form is used in G4a.

### 11.1 Gating scenarios (resolves QA B1)

Early stop makes a default call read only the first chunks, so the NFR-11 peak MUST come from scenarios that force near-full consumption. Which scenarios are evaluated at G4a versus G4b is set in 11.0. The gating peak is the MAXIMUM of the per-scenario medians over the scenarios below (measured on the bench build, 11 item 7) (each the median of >= 10 fresh processes); the 40 MB check applies to that maximum.

| ID | Scenario | Forces consumption because |
|---|---|---|
| G1 | 5 MB HTML identity, `max_length` = cap, `start_index` set so the window sits at the end of the converted output | early stop cannot fire until the tail is reached |
| G2 | same as G1, gzip | adds inflate state |
| G3 | `raw=true` 5 MB, window at end | raw path, no lol_html |
| G4 | HTML fixture whose main-content landmark appears late so the ADR-002 holdback fills to its 256 KiB limit, `max_length` = cap | worst emitter/holdback state (S2) |
| G5 | 5 MB body with requested window beyond the 5 MiB cap (expects `too_large`) | reads to the cap |
| G6 | 10 concurrent calls (each a G1/G2 mix) | worst concurrent state with semaphore 3 |
| G7 | 50 MB body: (a) with Content-Length (header abort), (b) chunked, window inside cap, (c) chunked, window beyond cap | boundedness check vs the 5 MB run (rule 6.4, ACCEPTED); G7b/G7c compare with the G1/G5 medians; G7a is trivially small |

Non-gating (reported, not part of the peak gate): default-parameter call (`max_length` 5000, start 0), slow-drip (deadline behaviour; timing tolerance +-20% of FETCH_TIMEOUT_MS), TLS comparison run.

Validity rule: a run that early-stops before reading the expected amount of the body is NOT a valid NFR-11 sample and invalidates the scenario. Self-check: the fixture server keeps a server-side counter of bytes actually written to the socket per request; the fixture manifest records `expected_min_bytes` per scenario (for G1..G4 at least the byte offset of the requested window's end; for G5 the cap; for G7a zero); the harness asserts counter >= expected_min_bytes and marks the sample invalid otherwise. Fewer than 10 valid samples for a scenario -> the whole report is INVALID (not partial).

### 11.2 Determinism (resolves QA B2)

- Fixtures: generated by a script with a fixed seed (committed in `bench/`); each fixture has a committed sha256 and byte size in a manifest; the harness verifies the hashes before any run and refuses to run on mismatch. The spike's 5,243,433 B synthetic fixture is the starting point; regeneration must be byte-identical.
- Warm-up: none inside a process (every sample is a fresh process, cold). One discarded dry-run of the whole matrix per session to warm the page cache for binary and fixtures, recorded as "warm-up run 1 discarded"; nothing else is dropped silently.
- Pinned environment: explicit env allow-list for the child (`FETCH_*` values, `MALLOC_ARENA_MAX` unset unless the scenario column says otherwise, `RUST_LOG` unset, `LD_PRELOAD` unset, `FETCH_LOG=warn`); same for every sample. ASLR left at system default (RSS is not ASLR-sensitive; recorded); THP setting (`/sys/kernel/mm/transparent_hugepage/enabled`) recorded, and comparisons are only between runs with the same THP and page size.
- Runner state: CPU governor recorded (performance preferred), no concurrent load (load average recorded before and after; > 0.5 above idle baseline marks the run suspect and it is repeated once), fixture server and harness on the same host over loopback.
- Run-count rule: a scenario needs >= 10 valid runs; if fewer succeed, the report is invalid; no run is discarded as an outlier; report min/median/max, the decision uses the median.
- Native-ARM preflight (refuse to emit gating numbers otherwise): `uname -m` = aarch64; `file <binary>` shows ARM aarch64; no qemu binfmt handler registered for aarch64 in `/proc/sys/fs/binfmt_misc`; `getconf PAGESIZE` recorded and only same-page-size runs compared (16K/64K hosts inflate RSS).
- Binaries: idle uses the shipped release binary, peak uses the `bench-loopback` build from the same commit and Cargo.lock hash (11 item 7); both are built by the same pinned pipeline with the D-7 profile.
- Matrix: the harness runs BOTH the gnu.2.17 and musl binaries (and any allocator candidates per ADR-005); report rows carry libc and allocator columns so ADR-005 and E-4 AC 4 can be decided from one report.
- Duration: full matrix nightly and on main/tag; PR-label smoke runs 3 scenarios x 3 runs (non-gating, never valid for NFR claims).

## 12. Open PRD Decision Points (not decided here)

| OQ | Question | What the design needs from the answer | Design already supports |
|---|---|---|---|
| OQ-3 | robots.txt enforced by default? | default value of `FETCH_IGNORE_ROBOTS`; whether `fetch/robots.rs` ships in v1 | robots fetch reuses client + SSRF policy + 512 KB cap; parser is streaming and keeps only our UA group; costs one extra request and small bounded state; module is isolated so removal is cheap |
| OQ-4 | private-host allowlist or blanket block? | whether `FETCH_ALLOW_PRIVATE_HOSTS` and C-2 ship | `ssrf::Policy` takes an allowlist that is empty by default. If used, allowlisting applies to the exact hostname of the original request only; a redirect hop to any other private host is still blocked; IP-literal hosts are not allowlistable; the allowlist relaxes only the private-range check, never scheme/port/metadata (169.254.169.254) rules. Blanket block = pass an empty list |
| OQ-5 | label output as untrusted content? | whether B-6 ships and its wording | `server::render` has one hook. Options for the human: (a) separate leading text content block (does not touch offsets); (b) in-band prefix on first page only. Either way the label is outside `start_index` accounting (ADR-006). Label is a mitigation, not prevention |
| OQ-7 | public distribution and licence | licence file, `cargo deny` licence policy, release channel, UA string contact URL | release job and artifact layout are channel-agnostic; only UA and docs depend on it |

## 13. Threat Model (STRIDE-lite)

Assets: the developer's LAN and cloud-metadata endpoints; the host's memory; the LLM's context integrity; the operator's stdout protocol stream. Trust boundaries: (1) MCP client <-> server over stdio (client trusted, but parameters LLM-driven, so treated as attacker-influenced); (2) server <-> network (untrusted); (3) server <-> DNS (untrusted answers).
Actors: prompt-injected LLM supplying malicious URLs; malicious web server; malicious DNS/rebinding host; a page author targeting the LLM.

| STRIDE | Threat | Mitigation |
|---|---|---|
| Spoofing | Malicious server impersonates target | TLS via rustls with embedded roots; no custom verifier, no cert-skip option |
| Tampering | Redirect or DNS swaps target to internal address after check | ADR-003: resolve once, connect only to validated set, manual redirects revalidated |
| Repudiation | No trace of what was fetched | stderr per-call log (host+path), local only |
| Information disclosure | SSRF reads internal services/metadata and returns content to LLM; error text reveals topology; credentials leaked | ADR-003; blocked errors are categorical; no cookies/auth/proxy (NFR-07); no headers forwarded across redirects because none are set; query stripped in logs |
| Denial of service | Huge/slow/compressed/deeply nested bodies exhaust memory or CPU | ADR-004: caps, deadline, semaphore (queue wait bounded), blocking-pool cap, lol_html memory limit, depth caps |
| Elevation of privilege | Fetched content makes the LLM take actions | Out of server control; OQ-5 labelling; no code execution, no JS, no file scheme, output is text only |

### 13.1 SSRF (Security Architect view)

Attack paths and controls:
1. Direct private/loopback/link-local/metadata host, name or literal: `check_url` + resolver validation (ADR-003).
2. IP encodings (decimal, hex, octal, short forms, IPv4-mapped/compatible IPv6, IPv6 zone ids, trailing-dot hosts, userinfo tricks `http://public@127.0.0.1`): the `url` crate applies WHATWG host parsing, normalising numeric IPv4 forms to a real address; we check the parsed `Host` enum, never the raw string. Zone-id and userinfo hosts rejected by policy. Userinfo in URLs is rejected outright (credentials are out of scope, NFR-07).
3. DNS pointing to private: resolver validates every returned address; ANY blocked address in the answer set -> refuse (mixed public/private, B-1).
4. DNS rebinding (TOCTOU): resolver is the connector's only lookup path; the validated addresses are what gets dialed. No `IP-literal after validation` rewriting of the URL is needed, so SNI/Host header remain correct for TLS.
5. Redirect to internal/`file:`/`ftp:`: reqwest redirect policy is `none`; our loop re-runs steps 1-3 per hop, max 5.
6. Proxy environment variables (`HTTP_PROXY`) would bypass IP validation: `no_proxy()` explicit.
7. Connection reuse: pooling disabled (`pool_max_idle_per_host(0)`), so no connection validated for one call is reused under a different policy context.
8. Port scanning of public hosts: not blocked by default (see ADR-003; residual risk accepted; Security NB-5 suggests defaulting to deny the WHATWG Fetch 'bad ports' list, left for the human/Security to decide), optionally restricted by `FETCH_ALLOWED_PORTS`.
9. IPv6 transition addresses embedding IPv4 (6to4 2002::/16, NAT64 64:ff9b::/96, Teredo): embedded address extracted and checked; ranges blocked where they cannot be safely interpreted.
10. Cloud metadata: always blocked and never allowlistable: 169.254.169.254, fd00:ec2::254, Azure wire server 168.63.129.16 (public-range, so listed explicitly); Alibaba 100.100.100.200 is covered by CGNAT 100.64.0.0/10. NAT64 64:ff9b::/96 and 6to4 with public embedded IPv4 are allowed by default; hosts behind a local NAT64 gateway should note this (config note in D-4, test in B-2).
11. Redirect hygiene: every hop builds a fresh request; no headers, cookies or Referer are inherited. https->http downgrade is permitted but shown in the Final URL header (A-9).

### 13.2 Prompt injection

Cannot be eliminated. Server-side measures: text-only output, no active content, script/style/hidden-element stripping (also removes some hidden-text injection vectors), fixed pagination footer separated from content, optional untrusted label (OQ-5, human decision). The fixed pagination footer and 'Final URL' header are spoofable by page text; keep them structurally separate from content (OQ-5 option a) and, if OQ-5 stays unanswered, ship the label ON by default (Security recommendation, not decided here). Documentation (D-4) states the residual risk. Note the stripping of `[hidden]`, `aria-hidden` and CSS-hidden text is best-effort: inline `style="display:none"` can be matched, external CSS cannot.

### 13.3 Resource exhaustion

| Vector | Control |
|---|---|
| Large body | wire cap 5 MiB default, Content-Length precheck |
| Decompression bomb | gzip only advertised; decompressed byte cap = same cap; bounded inflate step; ADR-004 |
| Slow-drip / hung connection | single overall deadline incl. DNS/connect/TLS/body |
| Redirect loops | max 5 hops |
| Many concurrent calls | semaphore (default 3), queue wait <= FETCH_TIMEOUT_MS, blocking pool capped |
| HTML pathologies (deep nesting, huge attributes/text nodes, tag soup) | lol_html memory limit 2 MiB, emitter depth 256, buffer caps; failure -> `converter_limit` |
| Huge `max_length` | hard cap 100,000 chars (default) |
| Header bomb | A-3b sets explicit header size/count limits (acceptance: a header bomb fixture fails cleanly), not merely 'verify defaults' |
| Log flooding | one line per call; no body logging |

## 14. Risks (architecture level)

| # | Risk | Impact | Mitigation / owner story |
|---|---|---|---|
| R1 | ARM RSS unmeasured; page size (4K vs 16K/64K), kernel and libc can shift idle/peak | High | budget headroom (worst case 28.9 MiB planned vs 40); early native run in E-1/A-2; ADR-005 flip rules |
| R2 | Streaming converter quality unproven (spike emitter drops links/emphasis/code/pre, no entity decode); readability without DOM is heuristic | High | ADR-002: golden tests, quality set, whole-body fallback, `raw=true` escape hatch, trait swap |
| R3 | musl malloc fragmentation/speed | Medium | ADR-005 measure both; ship gnu if musl fails criteria |
| R4 | TLS/HTTPS, real pages, gzip not exercised by the spike; peak will rise | Medium | benchmark scenarios in section 11; budget headroom |
| R5 | rmcp 3.x churn, `Send` future requirement | Medium | pin exact version; thin `server` module; lol_html `send` API |
| R6 | Interim unsafe window: A-3 (Sprint 1) precedes B-1 (Sprint 5) | High (security) | BINDING mitigation, see 14.1 |
| R7 | Early stop means total length unknown on non-final pages | Low | ADR-006: report `more content available`, total only when scanned |
| R8 | Stateless pagination re-fetches and re-converts on each call; page can change between calls | Low-Medium | documented; determinism tests; caching is out of scope |
| R9 | Brotli window up to 16 MiB would break the budget | Medium | brotli not advertised (ADR-004) |
| R10 | HTTP/1.1-only fails for the rare h2-only origin | Low | ADR-001 revisit trigger |
| R11 | webpki-roots embeds roots in binary; staleness and RSS effect unmeasured | Low | ARM measure; release cadence |
| R12 | Test-only or bench-only policy hooks (`test-support`, `bench-loopback`) could leak into release | High (security) | cfg/feature guards; D-7 and D-2 guard asserts both features and the `bench-loopback` marker string absent (cargo tree plus marker grep on every release artifact, self-tested); default `Policy` fail-closed; E-8 |
| R13 | Dependency count near NFR-05 ceiling | Low | 14 of 15 (flate2 added); add nothing without an ADR note |
| R14 | rmcp clone count for result copies unverified; transitive `tracing` | Low-Medium | A-2 allocation-count test and `cargo tree -i tracing`; revise 5.1 row j |

### 14.1 Binding interim mitigation for R6 (resolves Security B-1)

Decision (binding on architecture and story map): the pure `ssrf::ranges` table and the checks that enforce it are pulled forward into A-3 (since split by the plan into A-3a SSRF core, Sprint 1, and A-3b fetch client, Sprint 2). A-3 acceptance is extended so that, from the first build that can fetch, the binary applies the FULL blocked-range table (IPv4 and IPv6 incl. IPv4-mapped/compatible, ULA, link-local, loopback, CGNAT, metadata addresses 169.254.169.254, fd00:ec2::254, 168.63.129.16), the IP-literal pre-check, the resolver filter (resolve once, refuse if any answer is blocked, dial only the validated set) and per-hop revalidation in the redirect loop. The default `Policy` is fail-closed: it blocks everything non-public, and the only way to reach loopback is the cfg/feature-gated test constructor (R12). B-1/B-2/B-3 then own encodings, mixed-answer and rebinding test depth, coverage gate and hardening, not the first implementation.

Backstop rules (both recorded in EPICS, see Required doc changes): (1) release-gating: no tagged or distributed build before M3, and pre-M3 builds are not registered in a real MCP client; (2) A-3b (which carries the four-refusal integration test; A-3a covers the same cases at unit level) is not Done until an integration test proves `127.0.0.1`, `169.254.169.254`, a private-resolving name and a redirect to a private address are refused. The table is small, pure data, so the A-3 cost is modest; if the PO judges A-3 too large it may split, but the split may not leave any fetch-capable build without the table.

## 15. Story-to-Module Mapping

| Story | Modules / ADR | Notes |
|---|---|---|
| E-1 | bench design, section 11 | uses this doc's budgets and scenario list; defines MB once |
| A-1 | done (spike) | inputs to ADR-001, 002, 005 |
| A-2 | `main`, `server`, `config` (skeleton), `error`, `obs` | schema per ADR-006; stdout purity test |
| A-3a (SSRF core, 5 pts, Sprint 1) | `ssrf::ranges` full table, `ssrf::resolver` (resolve once, refuse on any blocked answer, return validated set), `ssrf::check_url` and per-hop revalidation function, `ssrf::Policy` fail-closed default plus test-only constructor (14.1) | ADR-003; no HTTP client dependency yet; unit tests with injectable resolver |
| A-3b (fetch client, 5 pts, Sprint 2) | `fetch` (client, redirect loop wired to A-3a, body, deadline), `config`, flate2 gzip, UTF-8 decoder, semaphore, header limits | ADR-001, 003, 004; merge gate: non-defaulted `Policy` in client constructor, required check `a3b_merge_gate` plus four-refusal integration tests |
| A-4 | `convert::html`, `boilerplate`, `mod` | ADR-002; entity decoder crate chosen here |
| A-5 | `convert::window`, `server::render` | ADR-006; owns G4b window scenarios |
| A-6 | `fetch::ctype`, `convert::text` | raw path streamed (ADR-004); records combined G4b result |
| A-7 | `error`, `server::render` | error table section 6 |
| A-8 | `fetch::charset` | prescan + streaming decode (ADR-004); the UTF-8 decoder with replacement already lands in A-3b (plan), A-8 adds the prescan and other charsets |
| A-9 | `server::render` | header only when a redirect occurred |
| B-1 | `ssrf::ranges`, `ssrf::resolver`, `ssrf::check_url` | ADR-003; first implementation lands in A-3 (14.1), B-1 owns test depth and hardening |
| B-2 | `ssrf::mod` (URL host handling), `ranges` | encodings, mapped IPv6 |
| B-3 | `fetch::redirect` | per-hop revalidation |
| B-4 | `fetch::robots` | blocked by OQ-3 |
| B-5 | tests, coverage gate | section 10 |
| B-6 | `server::render` hook | blocked by OQ-5 |
| C-1 | `config` | |
| C-2 | `ssrf::Policy` allowlist | blocked by OQ-4 |
| C-3 | `config`, `fetch::body` | |
| D-1 | Cargo profile (finalise, re-measure on shipped build, blocks MVP tag on miss) | ADR-005, sec 9 |
| D-7 | hosted PR CI, release profile pin, release-feature guard (`test-support`, `bench-loopback`), `publish = false` | sec 9, 9.1 |
| E-7 | 50-URL offline snapshots with sha256 manifest | sec 10 quality set |
| E-8 | `ssrf::policy` cfg feature `bench-loopback`, bench build | sec 11 item 7, sec 9 R12, ADR-003 |
| D-2 | CI workflows | section 9 |
| D-3 | CI, tests | |
| D-4, D-5 | docs, tool description (ADR-006 text budget <= 150 words) | |
| D-6 | supply chain, licence | OQ-7 |
| E-2..E-6 | `bench/`, CI | section 11 (E-4: G4a; G4b owned by A-5/A-6) |

Story count: 34 (A-1..A-9 with A-3 split into A-3a/A-3b, B-1..B-6, C-1..C-3, D-1..D-7, E-1..E-8). D-4 and D-5 share a row; A-3 rows above replace the former single A-3.

Ordering (as adopted by the plan): A-3a lands first with no HTTP client, A-3b follows in Sprint 2, so no fetch-capable build exists without the table. Original suggestion: A-3 needs the resolver seam and manual redirect loop from day one (cheap now, costly to retrofit); the UTF-8 streaming decoder is part of the window contract, so land it with A-3/A-4 rather than A-8.

## 16. Next Steps / Assumptions

Assumptions: single user, trusted local operator; one client; no persistence; HTTP/1.1 acceptable; gzip-only acceptable; character-based pagination acceptable.
Next: (1) run the ARM measurements in ADR-005/ADR-001 on the native runner as part of E-1; (2) 6.4 is ACCEPTED (PO, 2026-09-19); the Required doc changes below are historical (applied via the plan); (3) human answers OQ-3/4/5/7 before B-4/C-2/B-6/D-6; (4) A-4 spike-in-story: converter quality on 10 real pages before committing to landmark thresholds.


## Required doc changes (for the orchestrator/PO; docs/ not edited here)

1. EPICS A-3 acceptance: add the interim-safety criteria of 14.1 (full range table, IP-literal check, resolver filter, per-hop revalidation, integration test refusing 127.0.0.1 / 169.254.169.254 / private-resolving name / redirect to private). Adjust A-3 estimate (5 pts) if the PO judges it necessary. Note B-1 becomes "test depth and hardening" for what A-3 lands.
2. EPICS release rule: no tagged or distributed build before M3; pre-M3 builds not registered in a real MCP client (PRD Risk 2 mitigation).
3. EPICS E-4 AC 2, A-3 AC 1 (10 MB body -> "too large") and PRD FR-07 acceptance: amend to the three fixtures of 6.4 IF the PO confirms the proposed rule. Also E-2 fixtures gain header/no-header variants. Must be resolved before Stage 5.
4. PRD/EPICS FR-02 / C-3: `max_length` hard-cap default 100,000 chars (this design) instead of the 200,000 of the earlier draft; PRD does not currently state a cap, so this is a new documented config default.
5. EPICS E-3 vs E-6: state that E-3/E-5 release gate is strict absolute targets, E-6 CI is a regression tripwire (fail above absolute target OR >10% regression vs stored main baseline).
6. EPICS E-1: fix the MB vs MiB unit for the 40 MB gate, decide the TLS benchmark approach (11 item 7), and define the 50-URL success metric as conversion success with a non-gating live smoke run.
7. EPICS E-2/E-6: record the gating scenarios G1-G7, validity rule and determinism list of section 11; idle samples run in parallel; full matrix nightly.
8. EPICS D-2/D-3: pinning list (9.1), runner topology and trust (9.2), required status checks, `deny.toml` contents, `--version` flag.
9. EPICS C-1 / D-4: config-error diagnostics, fixed no-proxy limit, NAT64 note, musl DNS limitation.
10. PRD OQ-9 wording: note the self-hosted runner trust model (9.2). OQ-3/4/5/7 remain open; nothing here decides them.
11. EPICS story count is 30; confirm the orchestrator's "32" was a miscount.

## Revision log (round 1 of 3)

| Finding | Resolution |
|---|---|
| Architect B-1 memory arithmetic | 5.1 rewritten: 11 labelled items, worked worst case per scenario (S1 4.5, S2 6.3, S3 3.1 MiB), honest note that the old items summed ~8.2-8.5 MiB; cap lowered to 100,000 chars, `FETCH_MAX_CONCURRENCY` 3 so 10 + 3 x 6.3 = 28.9 MiB vs 40; ADR-004 flip formula updated. rmcp clone count flagged UNVERIFIED with A-2 measurement task (R14). No measurement invented. |
| Architect NB-1 | ADR-001 Options now says ~0.36 MB. |
| Architect NB-2 | Single rule: initial-URL non-http(s)/userinfo = invalid params; redirect Location non-http(s) = `blocked_target` (6.1, ADR-003, ADR-006). |
| Architect NB-3 | Decoding done manually with `flate2` on the raw wire stream; reqwest `gzip` feature off; both counters observable; dependency count now 14 (ADR-001, ADR-004). |
| Architect NB-4 | ADR-002 drop list rewritten; `form` no longer dropped wholesale; only form controls/chrome dropped (form contents kept). Tuned at A-4. |
| Architect NB-5 | ADR-004 rule and 6.4 marked PROPOSED pending PO. |
| Architect NB-6 | Transitive `tracing` noted; A-2 check (section 3, R14). |
| Architect NB-7 | 30 stories noted; Required doc change 11. |
| QA B1 | 11.1 gating scenarios G1-G7, max-of-medians rule, invalid-sample rule, server-side byte counter self-check. |
| QA B2 | 11.2 determinism list (seeded fixtures + sha256, env allow-list, run-count invalid-if <10, governor/load recorded, no silent drops, median decision). |
| QA N1 | Kept as PROPOSED, doc change 3. |
| QA N2 | Native-ARM preflight in 11.2. |
| QA N3 | Strict vs tripwire gate clarified (11 item 6, doc change 5). |
| QA N4 | Both libc/allocator binaries in matrix (11.2). |
| QA N5 | TLS caveat in E-5 template (11 item 7). |
| QA N6 | Quality-set caveat and live smoke (section 10). |
| DevOps B1 | 9.1 pinning table: exact toolchain 1.94.1, `=` dep pins from spike lockfile, locked builds, zigbuild 0.23.4 / ziglang 0.16.0, SHA-pinned Actions, reproducibility explicitly not a goal. Exact flate2 version deferred to A-3 (not yet in a lockfile). |
| DevOps B2 | 9.2 runner topology, trigger matrix, fork-PR policy, ephemeral unprivileged no-credential runner, offline = release blocked, skipped = failure via required checks, manual fallback. |
| DevOps NB 1-9 | Baseline definition (11 item 6), parallel idle runs, cgroup fields (11 item 5), deny.toml and nightly audit, checksums/provenance/SBOM/codesign, distribution implications, QEMU gnu caveat and objdump check, feature-guard for fixture-CA, `--version` and startup line. All in sections 7, 9, 11. |
| Security B-1 | 14.1 binding interim mitigation: full range table pulled into A-3, fail-closed default policy, release gate before M3, story change list (doc changes 1-2). |
| Security NB-1..NB-10 | Azure wire server always-blocked (13.1 item 10); fresh-request/no-header test (section 10); flate2 header check before decode; queue wait bound, blocking-pool cap, explicit header limits; bad-ports deferred as a human/Security decision (accepted residual); OQ-5 label-ON-if-unanswered recorded as recommendation; NAT64 note; host-only logging; https->http shown in Final URL header; fake-Resolve tests authoritative on both libcs (already in ADR-003). |
| Deferred | OQ-3/4/5/7 left open. E-4 vs early-stop rule left PROPOSED. Exact flate2 version and ARM-dependent choices await A-3 / native runs. |

## Revision 2 (post-plan, 2026-09-19)

Applies the user-approved Sprint Plan and the plan's "Required architecture change" note. No decision not approved by the plan or user was changed; OQ-3, OQ-4, OQ-5 and OQ-7 remain OPEN.

| Change | Where |
|---|---|
| `bench-loopback` Cargo feature (E-8, user-confirmed): off by default, only 127.0.0.0/8 and ::1, second binary from same commit and pinned pipeline, no runtime switch; idle gated on shipped binary, peak on bench build, numeric bounds (idle delta <= 0.5 MB, public-host peak within 10% and <= 40 MB, same commit and Cargo.lock hash) | sec 9, sec 11 item 7, sec 11.2, R12, ADR-003 |
| Release guard (D-7, D-2) forbids `test-support`, `bench-loopback` and fixture-CA feature and marker string in release artifacts | sec 9, R12, ADR-003 |
| Memory gate split G0 / G4a (end S4) / G4b (end S5); complete gate closes end of Sprint 5; scenario IDs G1..G7 kept, gate and scenario naming clarified | sec 11.0, 11.1, ADR-004, ADR-005 |
| Release profile pinned in D-7 (Sprint 0), D-1 (Sprint 8) re-measures on shipped build, miss blocks MVP tag | sec 9, sec 11 item 9, ADR-005 |
| Stale wording: ADR-006 200,000 -> 100,000 char cap; MB defined once in E-1, median of 10 valid runs; `ring` is transitive (count 14 unchanged); ADR-001 "~1 MB" -> ~1.3 MB; G7 pairing stated; 13.1 item order | sec 3, 5.1, 11.1, 13.1, ADR-001, ADR-006 |
| Story map: 34 stories, A-3a/A-3b split, D-7, E-7, E-8 added; A-8 decoder note aligned | sec 15, 14.1 |
