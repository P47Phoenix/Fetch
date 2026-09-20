# A-3a cleanup report (pre-merge, PR #5)

Scope: only the cheap items from developer, QA, tech-writer round-2 reviews and the PR review. No other changes.

## Changes
1. **RFC 9780 dummy prefix `100:0:0:1::/64` blocked** (ranges.rs row "reserved (dummy prefix)"). Tests: first/last, the address past it, bracketed URL `http://[100:0:0:1::1]/`; the earlier test that listed `100:0:0:1::1` as passing was wrong and was fixed. Row added to docs/SSRF.md and the ADR-003 amendment; the "everything else covered" claim in fix-pass-1-report.md is corrected at its end (history not rewritten). IANA IPv6 special-purpose registry re-checked from memory of the registry (no network lookup in this session): no other non-globally-reachable row is missing. ISATAP and non-well-known NAT64 prefixes are documented in docs/SSRF.md as accepted residuals (not blocked).
2. **`Resolver: Send + Sync`** plus compile-time test `resolver_and_validation_futures_are_send`. Note: the trait returns `impl Future`, so it is not dyn-compatible; it is shared via generics (or `Arc<R>`), and the doc comment says so.
3. **FR-13 lint**: `#![deny(clippy::print_stdout)]` added to src/main.rs. Probe: a temporary extra `println!` (with the `#[allow]` removed) failed `cargo clippy -- -D warnings` with "use of `println!`" pointing at that deny; probe reverted.
4. **CI**: clippy job also runs `--features test-support` (job ids unchanged); docs/ci-branch-protection.md command list updated.
5. **QA F1-F4**: F1 not done in code: making rmcp's deserialisation failures use `error[invalid_argument]:` would need the handler to take raw JSON and hand-write the input schema, which is not clean with rmcp 3.4, so the exact behaviour (missing/wrong-typed args, undeserialisable frames get no reply, `[]` arguments give -32601, invalid UTF-8/NUL and early `initialized` end the session) is documented in README. The existing tests are kept. F3 (loopback relaxation covers embedded forms) and F4 (URL-level rules: localhost names, IPv4 folding, zone ids, userinfo, non-ASCII refused, port 0, trailing dot) documented in docs/SSRF.md.
6. **Docs wording**: stale "run once on PR #2" replaced in BENCHMARK.md and ci-branch-protection.md (CI has run green on several PRs; not a trend claim; branch protection not configured); BENCHMARK section 7 and glossary musl wording says plan vs today (gnu only for the product); "skeleton" removed from rustdoc in lib.rs and error.rs. Correction notes appended (history kept) to A-3a and A-2 dev reports and the fix-pass-1 report; architecture.md IPv6 line mentions the new row.

## Test counts
default 43 unit + 10 integration; bench-loopback 44 + 10; test-support 43 + 10.

## Verification
See the commit; clean `git archive HEAD` extract results are listed in the hand-back message.
