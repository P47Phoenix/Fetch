# Fetch

**What is this?** Fetch is a small program, `fetch-mcp`, that will download a web page and turn it into text for an AI tool. Today it is an early build: it speaks the MCP protocol over standard input and output, offers one tool called `fetch`, checks the input, blocks unsafe addresses, and downloads the page (streamed, size-limited, from public addresses only). It converts HTML pages to markdown as it downloads them, pages through long content with `max_length` and `start_index`, and refuses images, PDFs and other binary types. **Who needs it?** Contributors who want to build, test or measure it. It is not ready for ordinary users. **What to do first:** read the status below, then follow "Build", "Test" and "Run over stdio".

## Status: pre-MVP (early, unfinished)

**Do not register `fetch-mcp` in a real MCP client** (MCP, the Model Context Protocol, is how an AI tool talks to a helper program) before milestone M3. The only planned check with Claude Code is a manual one on the owner's machine, using a throwaway config, not your normal setup.

What works today (stories A-2, A-3a, A-3b, A-5, A-6, E-4 and the implementation of A-4, whose acceptance check is still open, see "What has not been checked yet"; `fetch-mcp --version` prints the git commit (`commit=unknown` in a build made without git, such as a source archive) and the `Cargo.lock` hash it was built from):

- `fetch-mcp` is a real MCP server over stdio (built on the `rmcp` crate, version 3.4.0). It offers exactly one tool, `fetch`, with the inputs `url`, `max_length`, `start_index` and `raw`.
- A valid `fetch` call downloads the URL (HTTP/1.1, https with the built-in trust anchors). An HTML page is converted to markdown while it streams (headings, links, lists, code, tables; scripts, styles and navigation chrome removed; `raw=true` skips this) and other text is returned as fetched, in both cases cut to the requested character window. The body is read as a stream and never held whole: it stops with `too_large` past `FETCH_MAX_BYTES` (5 MiB), after 15 s with `timeout`, and gzip is unpacked in bounded steps. Every request and every redirect hop goes through the SSRF checks, and the client connects only to the addresses those checks approved. **Fetched content is returned as-is with no untrusted-content label (open question OQ-5 was decided "no label" on 2026-09-20).** When a redirect was followed, the returned text begins with a small plain-text header, `URL: <final url>`, `Status: <code>` and a blank line (A-9); with no redirect nothing is added. The header is outside the `max_length` window (it does not count against it), and the URL in it has no userinfo and no fragment. The header is unmarked plain text: a page that was not redirected can begin with identical text, so it is a convenience for citing, not proof of provenance. **Pagination (A-5):** `max_length` (default 5000, at most `FETCH_MAX_LENGTH_CAP`, default 100,000; 0 is refused) and `start_index` count characters (Unicode scalar values) of the returned text, so a window never splits a character. Reading stops as soon as the window is full and one more character has been seen, so a first page of a huge page is cheap. A truncated result ends with a footer, `[More content available. Call fetch again with start_index=N to continue.]`; the last page of a continuation (`start_index` above 0) ends with `[Total length: N characters.]` (the total is only known when the page was read to its end, so it is not stated on a non-final page); a `start_index` at or past the end returns `[No content at start_index=N: the content is M characters long.]` (not an error); a `max_length` above the cap is clamped and a footer line says so. A page that fits in the first window carries no footer. Footers are outside the character count. Each call fetches the page again, so paging a page that changes between calls can misalign. **Content types (A-6):** `text/*`, JSON, XML and their `+json` / `+xml` types, JavaScript and NDJSON are returned as text; HTML is converted unless `raw=true`; an image, PDF, `application/octet-stream` or any other type is refused before the body is read with `error[unsupported_content_type]` naming the type (also with `raw=true`); a response with no `Content-Type` is treated as HTML when the body starts (after whitespace or a byte-order mark) with `<!doctype html`, `<html`, `<head` or `<body`, otherwise as text. Cause-specific errors are in place (A-7). Not yet done: charsets other than UTF-8 (A-8).
- **Sprint 7 status (PR #11, draft):** B-3 (redirect limit, per-hop revalidation) and B-2 (encoded and IPv6 address forms) are covered by new tests. E-5 (benchmark report and release decision) is NOT done: its harness exists (`bench/smoke.py`, `bench/e5_report.py`) but the owner's 10-URL live smoke list has not been supplied, so no live smoke result and no release decision exist.
- **TLS caveat:** certificate validation (the https trust check) has no automated test. It was checked by hand only, against public sites. A regression there would not be caught by CI until B-2/B-5 or E-2 add a test.
- Bad input, and web addresses that point at private or internal machines, are refused with a clear error. See [SSRF range table](docs/SSRF.md) for what is blocked and why.

What has not been checked yet:

- The A-4 conversion-quality check (at least 95% of the 50 offline pages convert, median token reduction at least 50%) has NOT been run: the 50-page snapshot set (E-7) does not exist yet. Conversion is tested only on small local unit-test fixtures. **Story A-4 is therefore NOT Done:** it is implemented, and it stays open until the E-7 check has been run (Sprint 5 at the earliest).
- The automatic checks (CI) run on GitHub and pass on pull request #9 (Sprint 5, head cd99c95: all 14 checks green, including the advisory arm-bench jobs; the same jobs passed on #8 for Sprint 4 and #7 for Sprint 3), but branch protection on `main` is not switched on, and `cargo-audit` has never been run.
- **Memory gate part 1 (G4a, story E-4) passed in CI on all four hosted cells (run 35557702662, PR #8; re-run after fix-pass 1 as 35562347553 and again at head 3e02c70 as 35563535561, all PASS, values within about 1 MiB of each other, no trend claim)** (linux/amd64 and linux/arm64, gnu and musl, native runners, `--gate`, median of 10 valid runs, with HTML conversion): idle 2.35 to 4.62 MiB (target 10) and gating peak 4.56 to 6.16 MiB (target 40), 50 MiB pages within 1.03 of the 5 MiB run; 10 concurrent fetches recorded at 7.4 to 16.0 MiB (recorded, not a pass or fail). After fix-pass 1 (CI run 35562347553, head f9e4c9d; second run 35563535561 at head 3e02c70 also PASS) all four cells still PASS: idle 2.36 to 4.52 MiB, gating peak 4.58 to 6.09 MiB, 50 MiB pages within 1.02 of the 5 MiB run, 10 concurrent 8.1 to 13.2 MiB; two recorded hostile-HTML scenarios (3 concurrent; a 2 MiB attribute bomb is refused with `converter_limit` at 4.6 to 5.8 MiB, one 2 MiB attribute converts at 14.5 to 15.5 MiB) are not gates. Details, hosts and caveats (two or three runs, no trend claim, the shipped-vs-bench record, the public-host check on a 2 MiB page rather than 5 MiB, the allocator record): [BENCHMARK.md section 16](docs/BENCHMARK.md#16-g4a-peak-rss-and-boundedness-with-conversion-on-both-hosted-platforms-e-4-ci-run-35557702662-pr-8). Superseded by G4b below. Earlier read-in-full runs: [section 15](docs/BENCHMARK.md#15-sprint-3-gate-runs-on-both-hosted-platforms-e-2-e-3-ci-runs-35538774565-35539714646-and-35541676701-pr-7). The container image has not been measured yet (the runs use bare binaries; D-2 builds the image). The `bench-gate` job is not a required check. Older advisory numbers: [BENCHMARK.md section 13](docs/BENCHMARK.md#13-real-product-idle-numbers-advisory-sprint-1).
- **Memory gate part 2 (G4b, stories A-5 and A-6) passed in CI on all four hosted cells (first run 35641694726, PR #9, native runners, `--gate`, median of 10 valid runs; the same PASS on the other runs of this PR, see BENCHMARK.md section 17):** across those runs gating peak 4.31 to 6.16 MiB (target 40), shipped-binary idle 2.36 to 4.74 MiB (target 10), both 50 MiB chunked cases within 1.048 of the 5 MiB window-at-end peak (bound 1.10). A handful of runs is not a trend claim. Per-scenario values: [BENCHMARK.md section 17](docs/BENCHMARK.md#17-g4b-window-early-stop-and-raw-on-both-hosted-platforms-a-5-a-6-ci-run-35641694726-pr-9). By the plan this closes the memory gate; it does not cover the E-7-dependent A-4 checks (95% conversion, 50% token reduction, no `<script>`), which are undone. The `bench (gnu)`/`bench (musl)` jobs of `arm-bench.yml` measure the Sprint 0 spike, which has no `start_index`/`max_length`; by owner decision (Sprint 5) they now measure its idle figure only (advisory, not required) and were green at cd99c95.
- Open questions still undecided: OQ-3 (robots.txt on or off by default), OQ-4 (private-host allowlist), and OQ-7 (licence and distribution). An Apache-2.0 `LICENSE` file exists, and the project is marked `publish = false` (Cargo will refuse to publish it to crates.io) until OQ-7 is decided. **OQ-7 is still open with the project owner and is not decided here.** Four dependency crates that arrive with the HTML tokenizer (`lol_html`) are licensed MPL-2.0 (file-level weak copyleft), not Apache-2.0 or MIT: `cssparser`, `cssparser-macros`, `dtoa-short` and `selectors`. `deny.toml` allows exactly those four by per-crate exception; nothing else in the dependency tree is MPL. Using them unmodified does not change the licence of this project's own files, but distributing the binary or image means telling recipients where the source of those four crates can be obtained (their exact versions are in `Cargo.lock`), and modifying or vendoring any of them would oblige publishing the changes under MPL-2.0. Details and the choices this leaves the owner: [licence and distribution](docs/ci-branch-protection.md#licence-oq-7-still-open).
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
2. `tools/list`: one tool, `fetch`, described as "Fetch a URL and return its content. HTML pages are converted to markdown (scripts, styles and navigation dropped); other text, JSON and XML are returned as is; images and other binary types are refused. Set raw=true for the unconverted body. Output is limited to max_length characters from start_index; when truncated, the result ends with the start_index to continue from." `url` is required. `max_length`, `start_index` and `raw` are optional. The tool description string does not mention the A-9 header.
3. A valid URL (id 3) is downloaded, so this needs network access: you get the page text in one text item (`"isError"` false). Offline, you get `"isError":true` and `error[dns_failure]: hostname did not resolve`.
4. A blocked address (id 4) gives `"isError":true` and this text:
   `error[blocked_target]: IP address is not public (loopback)`
5. Bad input (id 5) gives `"isError":true` and this text:
   `error[invalid_argument]: url: must be an absolute http or https URL`

The error convention: a failed `fetch` is still a normal JSON-RPC `result`, with `"isError":true` and one text item that reads `error[<code>]: <message>`. For `invalid_argument` the message starts with the name of the bad field (here `url`). The codes today are `invalid_argument`, `blocked_target`, `dns_failure`, `too_large`, `timeout`, `unsupported_encoding`, `unsupported_content_type`, `http_error`, `too_many_redirects`, `network_error`, `bad_response`, `converter_limit` and `internal`. `converter_limit` means the HTML converter refused the page (for example about 20,000 unclosed elements nested, one tag with more than 1,024 attributes, or a tag bigger than 2 MiB); the message suggests retrying with `raw=true`, which skips the converter. The blocked-target message names a category (such as loopback), never the address, so an error cannot reveal what is on your network. See ADR-006 (tool schema, with its dated amendment) for the design.

Wrong-typed or missing arguments (for example no `url`, or `"max_length":-1`) use the same convention: `error[invalid_argument]: url: is required`, `error[invalid_argument]: max_length: must be a non-negative whole number` (A-7). An unexpected internal failure returns the generic `error[internal]: an unexpected internal error occurred; the server is still running, you may retry`; its detail goes to standard error only. HTTP failures name the class: `error[http_error]: the server refused the request with HTTP status 404 (Not Found); ...` (4xx; 408, 425 and 429 say it may work if retried after a delay) or `the server failed with HTTP status 500 (...)` (5xx).

One shape differs from that convention, because it is produced by the MCP library (rmcp 3.4) before our code runs:

- **Frames the library cannot deserialise** get no reply at all (a client waiting for that `id` would wait forever), and the server keeps running. Examples: a `tools/call` whose `arguments` is nested about 200 levels deep. `"arguments": []` returns JSON-RPC error `-32601` with message `tools/call`, not `-32602`. Invalid UTF-8 or NUL bytes, and `notifications/initialized` sent before `initialize`, end the session (exit code 1, nothing on standard output). None of these can crash or hang the server.

Unknown extra arguments are accepted and ignored. `max_length` and `start_index` pick a character window of the returned text, with the footers described under "What works today": for example `{"url": "...", "max_length": 5000, "start_index": 5000}` asks for the second page. `max_length` 0 returns `error[invalid_argument]: max_length: must be at least 1`. `max_length` is measured on the text after conversion (markdown for HTML), and an early-stopped page does not know the total length.

## Conversion limits (what the HTML to markdown step does and does not do)

The converter is a first version (story A-4, tier 1 of ADR-002); its quality has not been measured (see above). What to expect:

- **Page text is untrusted and passed through as it is** (OQ-5, decided "no label"). Nothing is escaped or neutralised: text such as `[link](http://evil)`, `![x](http://evil/p.png)`, a line starting with `# ` or a code fence, `*` or `_`, all reach the client as written and can look like real markdown. ESC and other control characters (terminal escape sequences) and Unicode direction overrides (U+202E) are not removed either. Treat the output like any web page's text. Only the link destinations the converter itself writes are filtered: just `http`, `https`, `mailto` and `tel` survive, the text of other links is kept without the link.
- **No page title.** `<title>` is dropped with the rest of `<head>`; the output starts with the first content.
- **Boilerplate removal is a small rule set, untuned.** Dropped with their whole content: `script style noscript template svg canvas iframe object embed dialog head title nav footer aside select textarea button`, elements marked `hidden`, `aria-hidden="true"`, `display:none` or `visibility:hidden` (**except** the optional-end and void tags `p li dt dd tr td th thead tbody tfoot option optgroup colgroup html body`: `<p hidden>` and `<td style="display:none">` are kept, because an unmatched end tag would otherwise hide the rest of the page; `<div hidden>` is dropped), the roles navigation, banner, contentinfo, complementary and search, and containers whose class or id is a noise word (cookie, consent, banner, popup, modal, ad, sidebar, share, newsletter, ...). These guesses can remove real content (a class named `share` on an article wrapper) or keep chrome. There is **no link-density rule** (ADR-002 tier 2 is not implemented), so a page whose navigation is plain lists of links keeps it.
- **Landmark holdback can lose content.** Output is held (up to 256 KiB) until a `<main>`, `<article>` or `role="main"` appears; when one does, everything held before it is discarded and only landmark content is kept. On a page where a small `<article>` (a related-links box, a teaser) sits inside or before the real content, the real content outside it is lost. If none appears within the holdback, the whole page is kept.
- **UTF-8 only.** Other charsets are not decoded (A-8); bytes that are not valid UTF-8 become U+FFFD.
- **`<base href>` is ignored.** Relative links and image addresses are resolved against the address the page was fetched from, so a page that sets `<base href>` can get wrong links.
- **Literal `<script>` text can still appear in the output** from sources that are not script elements: an image's `alt` text, `<xmp>` content, or escaped text such as `&lt;script&gt;` (which decodes to `<script>`). Script, style and iframe element content is dropped at every nesting depth. A page with a tag that has more than 1,024 attributes is refused with `converter_limit` (fail closed), not converted. The guard over-approximates (it models the tokenizer and counts candidates conservatively), so it can also refuse a page that has no tag with more than 1,024 attributes, for example `<b` in prose followed by more than 1,024 words with no `>`, or many `<x` comparisons in inline script with no `>` near them; measured on 2,509 real HTML files (and 1,501 real JS files inlined in `<script>`) it refused none. `raw=true` bypasses it.
- **Known gap: a drop tag nested in itself past 256 open elements empties the rest of the page.** With 257 nested `<nav>` (or another dropped tag) the skip never closes, so everything after it is dropped, silently, with no error. Present before fix-pass 1, not fixed yet; not a leak, but content loss.

## Where to go next

| I want to... | Read |
|---|---|
| See which addresses are blocked, and why | [SSRF range table](docs/SSRF.md) |
| Run or understand the memory benchmark | [Benchmark guide: quickstart](docs/BENCHMARK.md#quickstart-contributors) |
| Look up a benchmark word (VmRSS, median, gate, ADVISORY_PASS ...) | [Benchmark glossary](docs/BENCHMARK.md#glossary) |
| Understand the automatic checks (CI) and the branch rules on `main` | [CI and branch protection](docs/ci-branch-protection.md) |

## Notes on pagination and content types (Sprint 5)

- Text types returned without conversion: `text/*`, `application/json`, `application/xml`, `application/javascript`, `application/x-javascript`, `application/ecmascript`, `application/x-ndjson`, and any `+json` or `+xml` type. `text/html` and `application/xhtml+xml` are treated as HTML and converted unless `raw=true`. Anything else, including `application/octet-stream`, is refused with `error[unsupported_content_type]` before a body byte is read; the media type is echoed back, cleaned to printable ASCII and cut to 100 characters.
- A response with no `Content-Type` is sniffed: after whitespace and a byte-order mark, `<!doctype html`, `<html`, `<head` or `<body` means HTML, otherwise text. Binary bodies that arrive without a type are not detected as binary; their bytes are decoded as UTF-8 with invalid bytes shown as U+FFFD.
- Control characters in a page (for example NUL) are passed through as they are, like all fetched content (OQ-5). The declared charset is ignored until A-8; only UTF-8 is decoded.
- A first page that fits carries no `[Total length: ...]` footer; the total is stated on continuation pages and past the end (ADR-006 amendment 2026-09-21). An empty result at `start_index` 0 (an empty page, or one that converts to nothing) reads "No content at start_index=0: the content is 0 characters long."; this is kept as documented behaviour.
- Argument errors in a JSON-object `arguments` (a wrong-typed `max_length`, say) use the uniform `error[invalid_argument]: <field>: <why>` prefix (A-7). `arguments` that is not a JSON object still gets the library's JSON-RPC error (-32601).
- The stdio tests for pagination and content types serve pages from a loopback server, so they need the `bench-loopback` feature: `cargo test --features bench-loopback`. Without it they are compiled out.
