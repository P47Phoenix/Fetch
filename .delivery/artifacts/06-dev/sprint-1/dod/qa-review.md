# QA review: Sprint 1 (A-2, A-3a), branch sprint-1/skeleton-ssrf @ 0bcae4e

Reviewer role: QA validator (independent). Scratch code lives in `$CLAUDE_JOB_DIR/tmp` (drive.py, qa/, neg/), not committed.

STATUS: DONE (no blocking findings)

## What I ran and observed
| Check | Result |
|---|---|
| `cargo fmt --check`, clippy `-D warnings` (default, bench-loopback), `cargo test` (default, test-support) | pass (40 unit + 7 stdio tests each) |
| Release build; `tools/list` | exactly one tool `fetch`; schema has `url` (required string), `max_length`, `start_index` (uint64, min 0), `raw` (bool) |
| Validation drive over stdio (release binary) | missing url, file:, ftp:, data:, javascript:, -1, 1.5, "5", 1e30, 2^64, null url, non-string url, non-bool raw all rejected with a message naming the field (`missing field \`url\``, `url: scheme must be http or https`, `max_length: invalid value ...`). Loopback and metadata URLs give `error[blocked_target]` with a category only. A valid public URL gives `error[not_implemented]` and no network I/O. |
| Stdout purity | Every stdout line in every scenario parsed as JSON-RPC 2.0. 400 random-fuzz sessions (random methods, ids, params, byte-mutated frames, 1 to 6 frames each) plus ~20 hand cases (bad JSON, `[]`, `null`, invalid UTF-8, batch, dup ids, 20 MiB url, 200 concurrent calls, no trailing newline, init twice, bad protocolVersion): 0 non-protocol stdout lines, 0 signals/aborts (panic=abort would show as rc<0), 0 hangs. Logs and errors went to stderr only (`FETCH_LOG=debug`, bad config exit 2). |
| stdin close / early | exit 1 with `error serve connection closed: initialize request` on stderr (no stdout). Clean exit 0 when closed after init. |
| Startup | ready in 0.7 to 1.4 ms on this x86 host, VmRSS ~3.0 MB. |
| Release binary contents | `strings` finds no `FETCH_MCP_MARKER`, `bench-loopback`, `test-support` or `permit_loopback`. `--version` prints `fetch-mcp 0.0.0`. |
| Dependencies | `cargo tree -e normal` has no reqwest/hyper/ureq/h2/tls/curl/socket2/mio. tokio features are rt, macros, io-std, io-util (no `net`). |
| Guard `scripts/check-release-features.sh` | plain run "guard OK"; `--self-test` "self-test OK" (builds test-support and bench-loopback, both rejected for tree AND marker reasons). |
| My own negative control (not the dev's) | Copy of the repo with `ureq = "=3.4.2"` added: `--no-http-client` printed `guard FAIL ... ureq v3.4.2`, rc=1. So the assertion is real and not vacuous. |
| CI `.github/workflows/ci.yml` | job ids `fmt`, `clippy`, `test`, `deny`, `release-guard` match docs/ci-branch-protection.md exactly; `release-guard` runs both plain guard and `--self-test`; test job runs all three feature sets. `arm-bench.yml` is advisory and correctly not in the required list. |

## SSRF: my own table (qa/src/main.rs, ~330 assertions, built from ADR-003 text, not the dev table)
- Every IPv4 blocked range, first and last address, in dotted, decimal, hex, octal, `[::ffff:a.b.c.d]`, `[::a.b.c.d]`, `[64:ff9b::a.b.c.d]` and `[2002:hi:lo::]`/`[2002:hi:lo::1]` forms: all `blocked_target`. Address one below and one above each range: passes exactly when it is outside all other ranges (168.63.129.15/17 pass, 168.63.129.16 blocked).
- IPv6 edges (::, ::1 in long form, fc00::/7 first/last and one below, fe80::/10, fec0::/10, ff00::/8, 100::/64, 2001:db8::/32, 2001::/23 incl. Teredo, 64:ff9b:1::/48, fd00:ec2::254, mapped uppercase, 6to4 and NAT64 public-embedded pass): all as ADR-003 specifies.
- URL tricks: 0x7f.1, 2130706433, 017700000001, 127.1, 256, 4294967295 (blocked); 4294967296, 0x100.1, 09, 1.2.3.4.5, `example.0x7f` (refused, never a name); zone ids, userinfo variants, `127.0.0.1\@8.8.8.8` (blocked, WHATWG backslash), `8.8.8.8#@127.0.0.1` (host is 8.8.8.8, fine), tab/newline/space, fullwidth digits and ideographic dots, %-escapes, trailing dot, `..`, empty labels, ports 0/65536/+80/space, punycode host (name, lower-cased), raw IDN (refused), `127.0.0.1.nip.io` (a name, left to the resolver filter): all as expected.
- Resolver (injected): [pub,priv] and [priv,pub] for 10/8, ::1, mapped 127, fd00:ec2::254, 64:ff9b::a00:1, 2002:c0a8:1::1, Teredo, 168.63.129.16, 0.0.0.0, `::`: whole answer refused, one lookup, no address in the message, port carried, duplicates removed. Rebinding script [public then 127.0.0.1]: one lookup, only public returned. Literals and `localhost` never reach the resolver. Empty answer is `dns_failure`.
- Redirect revalidation: file:, ftp:, data:, javascript:, gopher:, userinfo, relative, scheme-relative, empty, zone id, decimal loopback, metadata, [::1], name resolving private: all `blocked_target`; public name, uppercase HTTPS, public literal: ok.
- My 3 "failures" were errors in my expectations (::2 is IPv4-compatible 0.0.0.2 so blocked; 2001:db7:: is outside 2001::/23), except F-3 below.

## Findings (all non-blocking)
| ID | Sev | Finding | Repro |
|---|---|---|---|
| F-1 | non-blocking | A request whose JSON nests too deeply gets NO reply (id never answered), so a client would wait to timeout. Server does not die. | `{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fetch","arguments":` + `[`*100000 + `]`*100000 + `}}` after initialize; only the initialize frame is emitted. Rmcp behaviour; note for A-7/hardening. |
| F-2 | non-blocking | Any message other than `initialize` first (notification or request) terminates the server with exit 1 and a stderr line that echoes a Debug dump of the message. Stdout stays clean. For a request first, a -32602 error frame is sent then it exits. | `printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n' \| target/release/fetch-mcp; echo $?` prints 1. Acceptable for stdio but the stderr dump can contain client-supplied content. |
| F-3 | non-blocking | IPv6 SIIT form `::ffff:0:a.b.c.d` (0:0:0:0:ffff:0:x:x, RFC 2765, deprecated) is not judged by its embedded IPv4, so `[::ffff:0:127.0.0.1]` passes `check_url`. ADR-003 does not list it; needs a SIIT translator on the path, so risk is low. Similarly 3fff::/20 (RFC 9637 documentation) and 5f00::/16 are not in the table. | qa harness: `check_url("http://[::ffff:0:127.0.0.1]/")` returns `ok-ip ::ffff:0:7f00:1`. Suggest B-5 adds them. |
| F-4 | non-blocking | `max_length: 5.0` is rejected as non-integer (a JSON float). Strict reading of the AC is fine; some clients serialise ints as 5.0. Unknown extra fields are silently accepted (schema has no `additionalProperties:false`). Non-object `arguments` (`[1]`) yields -32601 "tools/call" (misleading code) instead of invalid-params. | see drive.py cases `float_int`, `extra`, `args_list`. |
| F-5 | non-blocking | Guard's HTTP client ban list is name-based; a client built directly on tokio `net` or `socket2`/`mio` would pass. Fine for the Sprint 1 gate (crate has no tokio `net`), but the gate does not assert the tokio feature set. | Add `tokio` `net` to the tree in a copy: `--no-http-client` still passes. |

## Unverified (cannot be verified here)
- 250 ms readiness on aarch64: only recorded evidence from the arm-bench `bench-product` job (advisory, not gated); I measured x86 only (~1 ms).
- Claude Code listing `fetch` from a throwaway config: manual step, not run.
- `deny` job: `cargo-deny` not installed here; not run by me. CI is the only evidence.
- Hosted CI green on HEAD: I read the workflow, I did not see a run.
- Real DNS resolution and redirect following do not exist yet (A-3b); the resolver and per-hop code were exercised only with injected resolvers.
- Branch protection on `main` is documented as not yet configured (owner action).
