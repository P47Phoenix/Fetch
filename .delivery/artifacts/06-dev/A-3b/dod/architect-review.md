# Architect review: A-3b guarded streaming fetch client (PR #6, head c053d8c)

DECISION: DONE. Blocking: 0. Non-blocking: 5.

## Method
Clean `git archive` extract, built and tested there. `cargo test --locked` (85 lib + 9 stdio tests) passes; the named gate tests
(`a3b_merge_gate`, `refusal_*`, `dial_once_*`, `differential::*`) pass on the release profile; clippy `-D warnings` is clean; `cargo fmt --check` is clean;
`scripts/check-release-features.sh` prints "guard OK". cargo-deny and cargo-audit are not installed here, so I did not run them (see NB1).
I also wrote 9 adversarial tests against local raw-TCP servers (scratch only, deleted, not committed).

## Adversarial results (all held)
- Rebinding/TOCTOU: one resolver lookup per hop, every answer checked, mixed answers refused whole. The client is built per hop with `dns::Pinned`, which serves only the validated set for only the validated host name. IP literals never reach a resolver. `cross_check` re-parses with the `url` crate and compares scheme, port, host and userinfo. Host header and SNI come from that URL. Host mirrors the URL, so a trailing dot is preserved; it is not the IP.
- Redirects: 20 hostile Locations were refused with zero body bytes read. They included `::ffff:169.254.169.254`, `::ffff:a9fe:a9fe`, decimal and hex IPv4, a trailing-dot IP, `fd00:ec2::254`, userinfo forms, `//` scheme-relative, CGNAT and 192.0.0.192, `0`, `[::]`, `file:` and `gopher:`. The `\@` form is parsed by the `url` crate as host=pub.test, path=`/@...`, so the dial goes to the validated host (consistent, no bypass). A port switch is revalidated per hop. The bound of 5 is enforced (6th redirect gives `too_many_redirects`). A missing Location gives `http_error`. A 3xx body is never read. http->https and https->http are permitted per ADR-003 (http stays allowed); the credentials/cookies/referer test covers cross-hop leakage.
- Bombs: a 2 GiB gzip of zeros (2 MB on the wire) stopped at exactly 5 MiB decoded with `too_large` in 31 ms. An endless chunked body stopped at the cap. Content-Length above the cap aborts before the body. Corrupt gzip gives `bad_response`. Stacked or repeated Content-Encoding is refused before decode. The multi-member and trailing-garbage handling matches ADR-004. reqwest's gzip feature is off, so no hidden decode.
- Headers: a 300 KB single header, 100 headers and an endless header stream with no terminator all failed cleanly (`bad_response`, 18 ms). Count is enforced by hyper (64), bytes by `check_head` (32 KiB) after parse; the hyper parse buffer bounds the interim read (documented in the code).
- Slowloris: a trickled header block and a byte-per-200ms status line both timed out at the overall deadline (1.00 s and 0.80 s for 1.0 s and 0.8 s limits). The deadline covers connect, headers, all hops and the body. Queue wait is separately bounded by the timeout.
- Panics under panic=abort: non-test code in `src/fetch` has no unwrap, expect or panic; slice indexing sits behind length guards; errors are categories with no address text.
- Stdout: only `--version` prints (pre-existing); `deny(clippy::print_stdout)` is in force; the stdio purity tests pass; build markers go to stderr.
- Release guard: the removed checks (HTTP-client crate ban, raw-net ban, tokio `net` ban) are exactly the ones the A-3b AC and the script's own header told A-3b to delete, because the client legitimately depends on them. The feature allowlist, the marker grep and all self-test positive controls remain unchanged; the added CI job `a3b-merge-gate` asserts each named test actually ran (grep of the log, so a vacuous pass is not possible). Not weakened.
- `Policy::for_build`: fail-closed unless compiled with `bench-loopback`; no runtime switch; `bench-loopback` and `test-support` remain in the guard's forbidden list.
- N1/N2 closure: ADR-003 amendment 2026-09-20 records N2a (runtime cross-check plus differential corpus, incl. a 150k fuzz corpus and a positive control) and N2b (`a3b_merge_gate`). I verified each in code and by running the tests. Closed.
- Deps: no aws-lc, openssl or native-tls in Cargo.lock; deny.toml ban list intact; the only deny.toml change is the CDLA-Permissive-2.0 licence for webpki-roots (legitimate, permissive data licence); exact pins; direct-dependency count 8 of 15; `url` is a dev-dependency only.

## Non-blocking
1. cargo-deny and cargo-audit were not run by me; confirm the CI `deny` job is green on this head (licence and advisory checks for the new tree of about 1000 lock lines).
2. `reqwest` `rustls-no-provider` still compiles in `rustls-platform-verifier` and `openssl-probe`, although `tls_backend_preconfigured` never uses them. This is dead weight against binary size and NFR-05; check whether feature trimming is possible in D-1.
3. `map_transport` classifies any error whose chain contains "header" as a header-limit `bad_response`, so unrelated errors can be mislabelled. Cosmetic; A-7 refines the messages.
4. `tokio::net::lookup_host` runs on the blocking pool and keeps running after a timeout drops the future. A slow DNS attacker could hold blocking threads for the OS resolver timeout; the pool is bounded (512) and concurrency is 3, so not exploitable now, but note it for B-3 or B-5.
5. A response's header bytes are checked after hyper parses the whole head, so the effective pre-check bound is hyper's buffer (about 400 KiB) rather than 32 KiB. This is documented and harmless at the deadline and cap.
