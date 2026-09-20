# PR #5 independent code review — Sprint 1: Skeleton and SSRF core

- **PR**: https://github.com/P47Phoenix/Fetch/pull/5 (`sprint-1/skeleton-ssrf` → `main`), +3916 / −40
- **Reviewed at**: `1ce36a0`, worktree clean
- **Reviewer**: independent pass; prior review folders under `.delivery/artifacts/06-dev/` (dod*, independent-review*, pr-review) deliberately not read
- **Scope**: `Cargo.toml` / `Cargo.lock`, `src/` (server, config, error, obs, policy, ssrf), `tests/`, `scripts/check-release-features.sh`, `.github/workflows/*.yml`, `bench/*.py`
- **Nothing was edited, pushed or merged.**

## Decision

**APPROVE** — 0 blocking, 5 non-blocking findings.

The SSRF core is the strongest part of this change. I attacked the URL/host parser with an independently written WHATWG reference implementation and found **no address-level divergence** and **no panics**; the range tables match the IANA special-purpose registries plus the documented cloud-metadata additions. The findings below are hardening and guard-coverage items, none of which is exploitable in this build (there is no network code yet).

## Verification performed

All commands run locally at HEAD against a scratch `CARGO_TARGET_DIR`.

| Check | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo clippy --locked --all-targets --features bench-loopback -- -D warnings` | pass |
| `cargo clippy --locked --all-targets --features test-support -- -D warnings` | pass |
| `cargo test --locked` | pass (42 lib + 10 stdio) |
| `cargo test --locked --features bench-loopback` | pass |
| `cargo test --locked --features test-support` | pass |
| `cargo build --release --locked` | pass (1,289,016 bytes) |
| `scripts/check-release-features.sh` | `guard OK` |
| `scripts/check-release-features.sh --self-test` | `self-test OK` (every positive control fails the guard as required) |
| `python3 bench/selftest.py` | `SELFTEST PASSED` (40 checks) |
| `gh pr checks 5` | 8/8 pass: fmt, clippy, test, deny, release-guard, bench (gnu), bench (musl), bench-product |

Release-binary spot checks: `strings` finds **0** `FETCH_MCP_MARKER_` tokens and **0** occurrences of `permit_loopback` / `bench-loopback` / `test-support` in the default release build.

### Adversarial testing I added (outside the repo, in `/tmp`)

I built a throwaway crate depending on `fetch-mcp` by path and drove `ssrf::check_url` against an **independently written WHATWG URL/host reference** in Python (C0/tab/newline stripping, authority termination on `/?#\`, last-`@` userinfo, percent-decoding, `endsInNumber`, and the full ipv4-number parser with decimal/octal/hex radices).

- **232,640 differential cases** (structured corpus of IP spellings, ports, tails and trailing dots, plus random mutation, plus 120,000 randomly generated 1–5-part mixed-radix numeric hosts).
  - **Zero divergences on IP results.** Every host the Rust parser folded to an `IpAddr` matched the reference byte-for-byte.
  - The only divergence class (571 cases) is the trailing-dot normalisation in finding NB-4 below — a name-vs-name difference, never an address difference.
  - **Zero cases where Rust accepted an input the reference rejected** in a way that produced a different host.
- **86,003 raw-byte panic cases** (random byte strings, a 100 KB all-digits host, a 5,000-colon bracketed host, 4 KB numeric hosts, 200-label names): **no panic, no abort, no non-global address ever accepted.** This matters because the release profile is `panic = abort`.

I manually audited every slice/index operation reachable from untrusted input (`&after[..end]`, `&authority[..=close]`, `&authority[close+1..]`, `&part[1..]`, `last[2..]`, `mask32`/`mask128` shifts). All are on ASCII-validated data or on `str::find` byte offsets, so all are char-boundary-safe; the shift amounts are bounded by the prefix-length checks. `obs::stderr_line` correctly avoids `eprintln!`'s broken-pipe panic, and `tests/stdio.rs::closed_stderr_pipe_does_not_abort_or_panic` pins that.

### What I specifically looked for and did not find

- **Range-table gaps**: IPv4 covers every non-globally-reachable IANA row (`0/8`, `10/8`, `100.64/10`, `127/8`, `169.254/16`, `172.16/12`, `192.0.0/24`, `192.0.2/24`, `192.88.99/24`, `192.168/16`, `198.18/15`, `198.51.100/24`, `203.0.113/24`, `224/4`, `240/4` incl. `255.255.255.255`) plus `169.254.169.254/32` and `168.63.129.16/32` for categorisation. IPv6 covers `::/128`, `::1/128`, `::/96`, `fc00::/7`, `fe80::/10`, `fec0::/10`, `ff00::/8`, `100::/64`, `2001::/23`, `2001:db8::/32`, `64:ff9b:1::/48`, `3fff::/20`, `5f00::/16`, `fd00:ec2::254/128`. Embedded-IPv4 forms (mapped, SIIT, NAT64, 6to4) are re-judged by the IPv4 table. Row ordering is correct: the `/32` metadata rows precede their containing ranges, and `::`/`::1` precede `::/96`, so kinds and categories stay right.
- **Parsing bypasses**: every WHATWG IPv4 spelling (decimal, octal, hex, 1–4 parts, `0x`, `00`, trailing dot) folds to the same address the reference produces, so no `http://2130706433/` or `http://0251.0376.0251.0376/` style evasion. Userinfo, `%`-escapes, non-ASCII, control characters, zone identifiers, unbracketed IPv6, junk after `]`, multi-colon authorities and port `0`/overflow are all refused. Numeric-looking hosts can never fall through to the DNS-name path.
- **Rebinding**: `resolve_validated` performs exactly one lookup, refuses the whole answer set if any address is blocked, and returns only the validated `SocketAddr`s. Tests assert the lookup count.
- **Information leakage**: blocked-target messages carry a category word only; tests assert the address never appears.
- **stdout pollution**: `tests/stdio.rs` asserts every stdout line is a JSON-RPC 2.0 frame across five scenarios, and that stdout stays empty on a config error and on a bad first frame. (But see NB-1 for the lint that is supposed to back this up.)
- **Feature/cfg leaks**: `permit_loopback_for_tests` is the only relaxation and is gated `#[cfg(any(test, feature = "test-support", feature = "bench-loopback"))]`; `resolver::testing::FakeResolver` is `#[cfg(test)]`. Confirmed absent from the release artifact.
- **Guard false-passes**: `--self-test` is genuinely non-vacuous — it builds each forbidden feature and requires **both** the tree check and the marker grep to fire, rejects garbage/unparseable tree output, and asserts the real dependency tree is large enough that the ban could not be silently empty. The tokio-`net` check requires `tokio feature "rt"` to be visible before trusting the result.
- **Workflows**: `permissions: contents: read` on both; `pull_request` (never `pull_request_target`); no `secrets.*`, no `github.event.*` interpolation anywhere (so no script-injection surface); `persist-credentials: false` on every checkout; all five action uses pinned to full commit SHAs. `arm-bench` is correctly documented as advisory and is not a required check.
- **Dependency risk**: 4 direct runtime deps (`rmcp` `=3.4.0`, `tokio` `=1.53.1`, `serde`, `schemars`) against an NFR-05 budget of 15; the two high-churn crates are exact-pinned. 80 lockfile packages. `serde_json`/`zmij` are dev-only. `deny.toml` denies openssl/native-tls/aws-lc and uses a permissive licence allowlist. `tokio` deliberately excludes `net`, enforced by the guard.

## Non-blocking findings

### NB-1 — `#![deny(clippy::print_stdout)]` does not cover the binary crate (confidence 92)

`src/lib.rs:6` carries `#![deny(clippy::print_stdout)]`, and `src/main.rs:8` carries a matching `#[allow(clippy::print_stdout)]` on `print_version`, which reads as if the deny applied to both. It does not: `src/main.rs` is a separate crate root, and an inner attribute at one crate root never reaches another. `clippy::print_stdout` is a `restriction` lint (allow-by-default), so `cargo clippy --all-targets -- -D warnings` will not flag a `println!` added anywhere in `main.rs`.

I confirmed this empirically with a minimal two-crate probe: a `lib.rs` with the same `deny` plus a `main.rs` containing `println!` passes `cargo clippy --all-targets -- -D warnings` cleanly.

FR-13 (stdout carries only MCP frames) is a core invariant, and `main.rs` is exactly where future signal handling, shutdown and diagnostics will land. The runtime assertions in `tests/stdio.rs` mitigate today, but the compile-time guard is the one that scales.

**Fix**: add `#![deny(clippy::print_stdout)]` as the first line of `src/main.rs` (the existing `#[allow]` on `print_version` then becomes meaningful rather than decorative).

### NB-2 — `bench-loopback` is currently inert: a bench build still blocks loopback (confidence 90)

`Policy::permit_loopback_for_tests()` is compiled under `test-support`/`bench-loopback`, but its only call sites are `#[cfg(test)]` (`src/policy.rs:83,107`, `src/ssrf/mod.rs:494`, `src/ssrf/resolver.rs:319`). `src/main.rs:50` unconditionally constructs `Policy::default()`. So `cargo build --features bench-loopback` produces a binary that carries the marker, satisfies `measure.py`'s `--binary-kind bench` check, and **still refuses `127.0.0.1`**.

Nothing breaks today because `fetch` returns `not_implemented`, but `docs/BENCHMARK.md` already tells contributors that "the peak scenarios need the `bench-loopback` build", and the fixture server in `bench/serve.py` binds loopback. The wiring gap will surface as a confusing benchmark failure the moment A-3b lands.

**Fix**: in `main.rs`, select the policy under `#[cfg(feature = "bench-loopback")]` (with a loud stderr line at startup), or note explicitly in the A-3b story that the feature still needs wiring.

### NB-3 — CI runs clippy on two feature configurations but tests on three (confidence 88)

`.github/workflows/ci.yml:36-37` runs clippy for default and `bench-loopback`; `:55-57` runs tests for default, `bench-loopback` **and** `test-support`. The `test-support` configuration is never linted. It compiles (the test job proves that), so only lint coverage is missing, and `test-support` gates a single function today — but the asymmetry is unintentional and will not stay cheap.

**Fix**: add `- run: cargo clippy --locked --all-targets --features test-support -- -D warnings` to the `clippy` job. I ran it locally; it passes.

### NB-4 — trailing-dot host normalisation diverges from WHATWG (confidence 85)

`src/ssrf/mod.rs:163` strips one trailing dot before classifying, so `http://example.com./` yields `Host::Name("example.com")`. WHATWG keeps the root label, giving `example.com.`. This was the *only* divergence class in 232,640 differential cases (571 hits), and it never changes an address — but it does soften the module-header claim at `src/ssrf/mod.rs:6-8` that "a client that later re-parses the URL with a WHATWG parser dials the address that was validated".

Concretely: a fully-qualified `host.` bypasses the resolver search list, while the stripped `host` may have search domains appended (relevant for short/single-label names under the default `ndots:1`). The validated address set and the set a re-parsing client would look up can therefore differ.

This is **not an SSRF bypass** — `resolve_validated` checks every address it returns and A-3b is specified to dial only those — so the impact is confined to target identity, not to the block decision.

**Fix (pick one)**: keep the trailing dot in `Host::Name` so the resolved name matches what a WHATWG client would look up, or narrow the module-header claim to IP-literal hosts and note the normalisation explicitly.

### NB-5 — no port policy yet, though the error type already advertises one (confidence 82)

`split_host_port` accepts any non-zero `u16`, so `http://public.example:22/`, `:25`, `:6379`, `:11211` all pass `check_url`. Meanwhile `src/error.rs:13` documents `BlockedTarget` as covering "Port policy, non-public address, blocked redirect", and `src/config.rs:3` lists allowed ports among the variables that "arrive with the stories that use them".

Not exploitable in this build (no socket is ever opened), and the deferral looks deliberate — but a port allowlist is a standard second line of SSRF defence and needs to land **with or before** A-3b, not after. Worth an explicit acceptance criterion on that story rather than relying on the config comment.

## Notes (below the reporting threshold, recorded for completeness)

- 6to4 `2002::/16` with a *public* embedded IPv4 is allowed. This is the documented choice in `docs/SSRF.md` and is defensible (the encapsulated destination is public); flagging only so it stays a conscious decision.
- `ci.yml` jobs have no `timeout-minutes` (default 360); `arm-bench.yml` sets 45/30. Hygiene only.
- `Config::from_lookup` imposes no upper bound on `FETCH_MAX_BYTES` / `FETCH_MAX_CONCURRENCY`. These are operator-supplied, not attacker-supplied, so this is not a security issue — it may become a resource-limit question in a later story.
- `deny.toml` sets `[graph] all-features = false`, so cargo-deny audits only the default graph. Safe here because neither forbidden feature adds a dependency, and the release guard's own tree check uses `--all-features`.
- `docs/SSRF.md` matches `src/ssrf/ranges.rs` row for row; the README's quoted error strings match the code exactly.

## Summary

The SSRF layer is well constructed and, more importantly, well *tested*: the property-style tests that walk every table row in every embedded form are the right shape for this problem, and they held up against independent differential and fuzz testing. The release guard's self-test is a genuine positive control rather than theatre. CI is correctly locked down. The five findings are hardening items; none blocks the merge.
