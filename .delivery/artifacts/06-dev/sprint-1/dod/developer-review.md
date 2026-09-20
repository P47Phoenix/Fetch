# Developer DoD review: Sprint 1 (A-2, A-3a), HEAD 0bcae4e, branch sprint-1/skeleton-ssrf

STATUS: DONE (0 blocking, 4 non-blocking)

## Commands (clean `git archive HEAD` extract, all exit 0)
- cargo fmt --check: pass
- clippy --locked --all-targets -D warnings: pass in default, bench-loopback, test-support
- cargo test --locked: pass in all 3 configs (40 / 41 / 40 unit + 7 integration each)
- cargo build --release --locked: pass
- scripts/check-release-features.sh: "guard OK"; --self-test: "self-test OK"
- python3 bench/selftest.py: SELFTEST PASSED
- Extra: 3M-input random fuzz of check_url (mixed schemes, brackets, hex/octal, %, @, \, unicode, tabs): no panic; no Ok(Name) for a numeric-ending host (apparent hits were oracle false positives such as `0X7f_`, not numbers per WHATWG).

## Critical read
- Range tables (ranges.rs:30-94) match ADR-003 lines 29-31 row for row, including 168.63.129.16/32, fd00:ec2::254, 2001::/23, 64:ff9b:1::/48. Embedded v4 (mapped, compatible, NAT64 /96, 6to4 bits 16..48; ranges.rs:138-151) is correct; the 6to4 shift `(n>>80)&0xffffffff` verified. Whole-table-first ordering keeps ::1 loopback and :: unspecified.
- check_url (ssrf/mod.rs:60-109): authority delimiters match WHATWG special scheme (/ ? # \); '@' anywhere in authority refused; control/space/non-ASCII/% refused; zone ids refused; decimal/octal/hex/short IPv4 folded (mod.rs:187-226, no overflow or panic path: from_str_radix on u64 returns Err, shifts bounded); numeric-ending host is IPv4 or refused, never a name; extra slashes (`http:///127.0.0.1/`) refused (fail closed). No blocked literal found that passes.
- Resolver (resolver.rs:31-60): one lookup, any blocked answer refuses whole answer, returns dedup'd validated set; literals never reach resolver; error text carries category only.
- revalidate_hop uses Origin::Redirect so every failure is blocked_target; relative Location deferred to A-3b/B-3 (documented, refused today).
- Policy: Default fail-closed (policy.rs:14-21); loopback relaxed only by constructor under cfg(any(test, test-support, bench-loopback)) (policy.rs:45); only Kind::Loopback relaxable, metadata always Other. Guard proves absence in release.
- Stdout purity: lib `#![deny(clippy::print_stdout)]`; only print is --version (main.rs:8-11); integration test checks every stdout line is JSON-RPC.
- Panic audit (non-test src): no unwrap/expect/panic/indexing on untrusted input; slices (mod.rs:77,115,120,184,192) are on ASCII-validated boundaries; `as` casts in ranges.rs are const/bounded. Only reachable abort: eprintln (non-blocking N2).
- Dependencies: 4 direct (rmcp =3.4.0, tokio =1.53.1, serde 1, schemars 1) vs NFR-05 <= 15; no HTTP/TLS/net crate (guard asserts); Cargo.lock committed and --locked builds pass. serde/schemars are caret but locked.
- Dev-report claims A-2 and A-3a checked: test counts, tables, dependency ledger, deviations, guard behaviour all TRUE. A-2 report's "11 default unit tests" is stale (now 40); cosmetic.

## Findings (all non-blocking)
N1. A-2 AC "Claude Code lists fetch" not evidenced (manual, external; dev report admits). Needs a UAT note/manual confirmation before Sprint 1 exit sign-off. The 250 ms aarch64 AC is evidenced (1.06 ms, bench-product run 35482584789).
N2. src/obs.rs:37, src/main.rs:22,34: `eprintln!` panics if stderr is a closed pipe (EPIPE); under panic=abort that aborts the process. Repro: FETCH_LOG=info with stderr closed-reader pipe -> abort at startup log (main.rs:26). Not reachable from untrusted MCP input. Suggest `writeln!(io::stderr(), ..).ok()`.
N3. ranges.rs:141-143 and V6_BLOCKED: IPv4-translated `::ffff:0:0:0/96` (RFC 2765) is not judged by the v4 table: `http://[::ffff:0:a00:1]/` (embeds 10.0.0.1) passes check_url. Also `3fff::/20` (RFC 9637 documentation) not blocked. Neither is in ADR-003 (spec-conformant); not routable without a SIIT translator. Suggest B-2 adds both. Related, already flagged by dev: `::8.8.8.8` allowed by ADR.
N4. mod.rs:164 host label length (>63) not checked; harmless (resolver rejects), noting for A-3b resolver impl.
