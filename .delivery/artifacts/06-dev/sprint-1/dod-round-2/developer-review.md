# Developer DoD validation, round 2 (Sprint 1: A-2, A-3a) - HEAD of sprint-1/skeleton-ssrf

STATUS: DONE (no blocking findings; 4 non-blocking)

## Gates (clean `git archive HEAD` extract)
- cargo fmt --check: pass
- clippy --locked --all-targets -D warnings: pass in default, bench-loopback, test-support
- cargo test --locked: pass in all 3 (42 / 43 / 42 unit + 10 integration)
- cargo build --release --locked: pass
- scripts/check-release-features.sh (plain, includes no-HTTP-client and tokio-net ban), --no-http-client, --self-test: pass
- python3 bench/selftest.py: SELFTEST PASSED

## Critical source review (src/)
- check_url/parse_host: userinfo (any '@'), zone ids ('%' anywhere), whitespace/control/non-ASCII, backslash authority end, trailing dot (one stripped; two refused), all WHATWG IPv4 spellings (decimal/octal/hex/1-4 parts, 5 parts and non-parsing numeric last label refused, never a name), bracketed IPv6 (mapped, SIIT, compat, NAT64, 6to4, Teredo), port 0/overflow/junk. Probed ~30 extra spellings by hand plus 3,000,000 random-grammar inputs in a scratch example: no panic, and no accepted IP host that Policy::check_ip refuses. No unwrap/expect/index/slice that can panic on untrusted input in non-test src (slices are on ASCII-checked boundaries; `5 - nums.len()` bounded by 4).
- Policy: Default fail-closed; permit_loopback_for_tests only under cfg(test)/test-support/bench-loopback; only Kind::Loopback relaxable, metadata never. Release binary marker guard passes.
- Resolver filter: one lookup, every answer checked, whole answer refused on any blocked, returns deduplicated validated set only. Error text carries category only.
- Dependencies: 4 direct (rmcp =3.4.0, tokio =1.53.1, serde 1, schemars 1) of 15; no HTTP/socket crate, tokio `net` off.
- stderr: no eprintln in src; broken pipe cannot abort.

## Findings (all non-blocking)
N1. src/ssrf/ranges.rs:52-118 (V6_BLOCKED) - IANA IPv6 special-purpose registry entry 100:0:0:1::/64 (Dummy IPv6 Prefix, RFC 9780; forwardable=No) is not blocked. Failing input: `http://[100:0:0:1::1]/` returns Ok(Host::Ip) (probed). Only 100::/64 is listed. Not practically routable, so non-blocking, but the fix-pass-1 report's claim "everything else listed in the IANA IPv6 registry is covered" and docs/SSRF.md "follows the IANA registries" are untrue on this row. Fix: add `(v6([0x100,0,0,1,0,0,0,0]), 64, Other, "reserved (dummy prefix)")` plus SSRF.md row and test, or reword the claim.
N2. src/ssrf/resolver.rs:14-17 - `Resolver` trait has no `Sync` bound, so the future of `validate_target`/`resolve_validated` is Send only when the concrete R: Sync. rmcp needs Send tool futures; no test asserts Send of these futures (grep: none). A-3b will hit a compile error or add the bound ad hoc. Suggest `pub trait Resolver: Sync` and a compile-time `assert_send` test for validate_target now.
N3. .delivery/artifacts/06-dev/A-3a/dev-report.md ("Ranges table" row `::/96 compatible` under embedded forms and deviation 2 "::8.8.8.8 is ALLOWED", "40 unit + 7 integration") and A-2/dev-report.md (`validate_url`, "11 default unit") describe pre-fix-pass behaviour that is now false (`http://[::8.8.8.8]/` is refused; counts are 42+10). fix-pass-1-report supersedes it but the originals were not annotated. Add a "superseded by fix-pass 1" line.
N4. ISATAP-style addresses with a public prefix and embedded private IPv4 (`2606:4700::5efe:10.0.0.1`, i.e. `http://[2606:4700::5efe:a00:1]/`) pass; only relevant if the host has an ISATAP interface, which is not a container norm. Record as accepted residual risk in SSRF.md "Deliberately not blocked" (also non-WKP NAT64 prefixes). Also: config max_* fields and FetchParams max_length/start_index/raw are parsed but unused until A-3b/A-5 (expected, not dead by lint).

## Claims checked
- Verification list in reports (fmt, clippy x3, tests x3, guard, self-test, selftest, release build): reproduced true.
- "Direct deps 4 of 15", "no `url` crate", "tokio net absent": true.
- Not verified by me: cargo deny, actionlint, aarch64 numbers, Claude Code manual listing (documented as manual/CI).
- A-3b merge-gate text (differential test vs url crate, Validated.addrs-only dial, guard-check removal) is documented in EPICS A-3b; nothing in HEAD violates it.
