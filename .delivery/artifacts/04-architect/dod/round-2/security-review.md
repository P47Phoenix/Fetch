# Security DoD Review - Stage 4 (Architect), Round 2

Reviewer: Security Architect validator. Scope: architecture.md (s5, s6, s8, s12-14), ADR-001, 003, 004 (others skimmed).
Result: DONE - 0 blocking, 6 non-blocking.

## Gate criteria
| Criterion | Verdict |
|---|---|
| Resolve-and-pin | Pass (ADR-003 step 5: single lookup, all answers validated, only validated set dialed; pooling off; SNI intact) |
| IPv4-mapped/compat, IPv6, ULA, link-local, metadata (169.254.169.254, fd00:ec2::254, 168.63.129.16, 100.100.100.200 via CGNAT) | Pass |
| IP encodings | Pass (url-crate WHATWG parse, check on Host enum; zone ids and userinfo rejected; encodings table test) |
| Redirect re-validation, rebinding, scheme/port, proxy env | Pass (manual loop, max 5, steps 1-5 per hop, non-http(s) Location -> blocked_target, no_proxy, any-port residual documented) |
| Credential stripping on cross-origin redirect | Pass (no cookie store/auth/default headers; fresh request per hop; test specified in s10) |
| Decompression bombs | Pass (gzip only, header checked before decode, first member only, decompressed cap = wire cap, bounded 64 KiB steps, fixtures required) |
| Timeouts/slowloris | Pass (one deadline DNS..last byte incl. redirects; queue wait bounded; blocking pool capped; header limits are an A-3 acceptance) |
| Prompt injection / OQ-5 | Pass (single render hook, label outside start_index accounting, both options supported; label-ON-if-unanswered recorded) |
| TLS verification | Pass (rustls+webpki-roots, no skip option; bench fixture-CA feature guarded by CI absence check) |
| A-3-before-B-1 window | Pass (see below) |
| stdio log leakage | Pass (stdout never written, lint; stderr host-only at info; no bodies/headers; errors don't echo query) |

## R6 / s14.1 assessment
Round-1 B-1 is resolved by the first of the three options offered: the architecture now makes the full range table, IP-literal check, resolver filter and per-hop revalidation part of A-3 scope, with a fail-closed default Policy (only cfg/feature-gated test constructor reaches loopback, CI-asserted absent from release) and an A-3 done-condition integration test (127.0.0.1, 169.254.169.254, private-resolving name, redirect to private). That is a technical, test-enforced constraint, not advice. The release-gate and "no registration pre-M3" rules are process backstops only, and are not relied upon.

## Non-blocking
- NB-1 docs/EPICS.md is not yet amended (A-3 AC, release rule; Required doc changes 1-2). The binding mechanism is only enforceable once the PO applies them. Stage 5 must verify A-3 AC contains the 14.1 criteria and the split rule (no fetch-capable build without the table).
- NB-2 Range table gaps, minor: 192.31.196.0/24, 192.52.193.0/24, 192.175.48.0/24 (AS112/AMT), 3fff::/20 not listed. Not exploitable for LAN/metadata; add in B-1/B-5.
- NB-3 Any-port default remains (cross-protocol probing of public hosts); bad-ports default still deferred to human. Accepted residual.
- NB-4 tokio lookup_host has no cancel; a timed-out getaddrinfo holds a blocking thread. Cap of 4 bounds it but four hung lookups delay later DNS (self-DoS only). Consider documenting.
- NB-5 https->http downgrade allowed, only surfaced in Final URL header; consider blocking cross-scheme downgrade by default.
- NB-6 stderr may be captured by MCP clients; host at info is fine, but ensure the panic hook and rmcp/transitive tracing output never include URLs or bodies (A-2 test).
