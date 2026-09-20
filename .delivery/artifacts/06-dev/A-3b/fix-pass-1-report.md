# A-3b fix-pass 1 report (PR #6)

Base head c053d8c; fix commit 5c9a0dd plus a figures commit.

## Blocking
1. map_transport: `is_connect` captured, `without_url()` applied before the Debug text is matched. Tests `url_text_containing_header_never_changes_the_transport_class` (path `/api/headers`, query, and a redirect Location containing "header"); confirmed to fail on the old code.
2. Native aarch64 RSS smoke: new advisory job `bench-product-peak` (ubuntu-24.04-arm, SHA-pinned actions, bench-loopback build, bench/serve.py fixture, harness `g4a-5mib-full`, no --gate). CI run 35523134933: VmHWM median 4.77 MiB (4636-4896 kB, 10/10 valid). Idle VmRSS refreshed from `bench-product` same run: 3.58 MiB (was 2.59, pre-client). Recorded in docs/BENCHMARK.md sections 13-14, labelled advisory, single run, not a gate. Not waived.
3. Docs: TLS-untested caveat in README and docs/ci-branch-protection.md; "proves" -> "checks", source scan stated as heuristic.

## Non-blocking done
README reply-order note, memory smoke provenance, unconverted-markdown note; OQ-5 marked resolved in architecture.md (two places); BlockedTarget doc (port policy deferred to C-1/B-3); one shared `ssrf::canonical_name` (exactly one root dot) used by parse_host, Pinned and cross_check; tests for cross_check backstop, wire-byte cap (gzip under cap on content, over on wire, chunked), truncated gzip (bad_response), short identity body (network_error), empty gzip body (decided: empty page, documented in test); test-level 60 s guard in `get()`; E-8 `--version` hash AC and handshake/delta/cross-check items re-homed to E-2/E-4 in docs/EPICS.md, src/lib.rs comment fixed, E-8 dev report correction appended; ADR-001 dependency note (rustls-platform-verifier/openssl-probe) and D-1 note; redirect-chain memory check (E-2) and DNS blocking-pool note (B-3) in EPICS.

## Verification (clean git archive of 5c9a0dd)
fmt OK; clippy -D warnings x3 clean; cargo test --locked default 91+9, bench-loopback 92+10, test-support 91+9, all pass; test-support suite repeated 5 more times, 0 failures; release build OK; guard OK and --self-test OK; bench/selftest.py passed on CI arm and locally earlier; cargo-deny not installed locally (CI `deny` passed); actionlint clean.

## CI on PR #6 (head 5c9a0dd)
All pass: fmt, clippy, test, deny, release-guard, a3b-merge-gate, bench (gnu), bench (musl), bench-product, bench-product-peak.
