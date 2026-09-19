# Security DoD Review - Stage 4 (Architect)

Reviewer: Security Architect validator. Scope: architecture.md (esp. s13, R6, s12), ADR-001..006 (esp. 003, 004).
Result: NOT_DONE - 1 blocking, 9 non-blocking.

## Gate criteria summary
| Criterion | Verdict |
|---|---|
| SSRF: resolve-once/validate-all/pin | Pass (ADR-003 steps 1-8; custom Resolve, refuse whole answer if any blocked; no second lookup) |
| IPv4-mapped/compat, ULA, link-local, metadata 169.254.169.254, fd00:ec2::254 | Pass |
| IP encodings (decimal/hex/octal/short) | Pass (WHATWG parse of Host enum; tests listed) |
| Redirect re-validation, DNS rebinding, scheme/port, proxy bypass | Pass (manual loop max 5, per-hop steps 1-5, no_proxy, no pool; port policy documented as accepted residual risk) |
| Cookie/auth stripping on cross-origin redirect | Pass by construction (no cookie store, no auth, no forwarded headers); see NB-2 |
| Decompression bombs | Pass (gzip only, single encoding, bounded steps, decompressed cap); see NB-3 |
| Exhaustion / slowloris | Pass (overall deadline covers DNS..body, semaphore, header limits to be set); see NB-4 |
| Prompt injection / OQ-5 switch point | Pass (single render hook, label outside offsets); see NB-6 |
| TLS verification | Pass (rustls, embedded roots, no skip option) |
| stdio log leakage | Pass (stdout never written, lint, no bodies/headers, query stripped); see NB-8 |
| Unsafe interim A-3 before B-1 | FAIL - see B-1 |

## Blocking
### B-1 Interim unsafe window (R6) mitigation is a recommendation, not a design constraint
R6 (High, security) says A-3 "must land" a minimal default-deny table but the PO is only "recommended" to pull B-1 forward or gate registration on M3; "do not register pre-M3 builds" is a process note with no enforcement. Sprint 1 through Sprint 4 builds can fetch 169.254.169.254 and LAN hosts. Required fix: make it binding in the architecture and story map. A-3 acceptance must include the full `ssrf::ranges` table plus IP-literal pre-check plus resolver filter plus per-hop revalidation (the table is small, pure data), OR the binary must refuse to start / return blocked_target for everything non-public until B-1 lands, OR a release-gating rule (no tagged/distributed build before M3) recorded in EPICS. Pick one and state it in section 14/15.

## Non-blocking
- NB-1 Cloud metadata coverage: Azure wire server 168.63.129.16 is public-range and not blocked. Add to the always-blocked list (not allowlistable) alongside 169.254.169.254 and fd00:ec2::254. Also confirm Alibaba 100.100.100.200 stays blocked (CGNAT, yes).
- NB-2 State explicitly that each redirect hop builds a fresh request with no inherited headers, and add a test asserting no Authorization/Cookie/Referer is sent on cross-origin hop (currently justified only by "none are set").
- NB-3 gzip: reqwest auto-decoding may not reject stacked/unknown Content-Encoding the way ADR-004 assumes; add explicit header check before decode and the planned bomb-fixture test (also gzip trailing garbage/multi-member).
- NB-4 Semaphore queue wait: state whether FETCH_TIMEOUT_MS starts before or after the permit acquire (queued calls should not exceed the deadline unbounded). tokio lookup_host uses spawn_blocking; a timed-out getaddrinfo keeps its blocking thread, so cap blocking-pool threads (e.g. max_blocking_threads) to bound slow-DNS abuse. Set explicit header size/count limits in A-3 (currently "verify defaults").
- NB-5 Port policy: any port on public hosts allows cross-protocol probing (SMTP, Redis, etc.) via HTTP. Consider a default deny for the WHATWG Fetch "bad ports" list. Residual risk is documented and accepted, so non-blocking.
- NB-6 Prompt injection: fixed pagination footer and "Final URL" header are spoofable by page text; keep them structurally separate (OQ-5 option a) and, if the human does not answer OQ-5, ship label ON by default. Document that hidden-text stripping is best-effort (already noted).
- NB-7 NAT64 64:ff9b::/96 and 6to4 with public-embedded IPv4 are allowed; on hosts behind a local NAT64 gateway this could reach internal v4 space. Acceptable default but add a config note or test.
- NB-8 Logs record host+path; paths can contain tokens. Consider logging host only at info and path at debug. Error text must not echo full URL with query.
- NB-9 https->http redirect downgrade is permitted; consider blocking it or surfacing it in the Final URL header.
- NB-10 musl resolver differences (ADR-003) can cause fail-closed DNS failures, not security issues; keep the fake-Resolve tests authoritative on both libc targets.

## Positive
Design is correct by construction on the TOCTOU point (validated set is the dial set, SNI intact), tables are data with table-driven tests, test-only loopback policy is cfg-gated and required absent from release, allowlist never relaxes metadata/scheme/port/userinfo and excludes IP literals.
