# Sprint 10 dev report -- C-1, C-2, C-3, D-4

Branch: `sprint-10/config-allowlist-install-docs`. Sprints 6-12 table row 10: "Config and allowlist, plus
install docs (MVP complete)", 9 pts (C-2 included per the 2026-09-19 user acceptance).

## CRITICAL CONSTRAINT carried through this sprint

OQ-3 (robots.txt default policy) and OQ-4 (private-host allowlist policy) are **still open** -- undecided by
the product owner. Nothing in this sprint decides either. See "Owner decisions still needed" below and the new
Revision 19 entry in `.delivery/artifacts/05-plan/po/sprint-plan.md`.

## What shipped

### C-1: env parsing and validation

`src/config.rs` already had `FETCH_TIMEOUT_MS`, `FETCH_MAX_BYTES`, `FETCH_MAX_LENGTH_CAP` and
`FETCH_MAX_CONCURRENCY` (all positive-integer, startup-error-naming-the-variable). This sprint adds:

- `FETCH_ROBOTS_TXT` (`ignore` | `enforce`, case-insensitive, default `ignore`). Parsed and validated
  (`RobotsMode::parse`), but a **placeholder only**: neither value fetches or enforces `robots.txt` -- that is
  entirely unimplemented until B-4 (Sprint 11). The default preserves today's behavior and is not a decision on
  OQ-3.
- `FETCH_ALLOW_PRIVATE_HOSTS` (comma-separated hostnames, trimmed, canonicalized via `ssrf::canonical_name`,
  default empty). Rejects an IP-literal entry or an empty entry (stray comma) as a startup error naming the
  variable, since IP literals can never be allowlisted (see C-2).

### C-2: `ssrf::Policy` allowlist mechanism

`src/policy.rs`:
- `Policy` gained an `allow_private_hosts: Vec<String>` field (empty by default -- `Policy::default()` is
  unchanged and still fail-closed for everyone).
- `Policy::with_allow_private_hosts(hosts)` builds a policy with the mechanism; `Policy::for_build(hosts)`
  (was `Policy::for_build()`) threads the configured list into the binary's runtime policy alongside the
  existing (unrelated) `bench-loopback` compile-time loopback relaxation.
- `Policy::check_ip_for_host(ip, hostname)`: like `check_ip`, but relaxes the block **only** when (a) the
  block's category is exactly `"private"` (RFC 1918 / private-use, i.e. 10/8, 172.16/12, 192.168/16) and (b)
  `hostname` exactly matches an allowlist entry. Every other category -- cloud metadata (169.254.169.254,
  168.63.129.16), link-local, CGNAT, loopback, multicast, reserved, documentation -- stays blocked regardless
  of the allowlist.

`src/ssrf/resolver.rs`: `resolve_validated` now takes `origin: Origin` and calls `check_ip_for_host` only for
`Origin::Initial`; a redirect hop (`Origin::Redirect`, via `revalidate_hop`) always uses plain `check_ip`, so an
allowlisted hostname's own redirect to a *different* private host, or even a *repeated* redirect to itself, is
still blocked -- matching the architecture's OQ-4 answer text: "a redirect hop to any other private host is
still blocked" and "applies to the exact hostname of the original request only". IP-literal hosts never reach
this path (`check_url` judges them by `check_ip` alone, with no hostname to match against the allowlist), so
they are structurally unallowlistable.

`src/main.rs`: `Policy::for_build(cfg.allow_private_hosts.clone())`.

### C-3: `max_length_cap` hard cap

Found already implemented end to end in `src/server.rs` (a prior sprint): `Fetch::new` takes
`max_length_cap` from `Config`, the per-call `max_length` argument is clamped
(`asked.min(self.max_length_cap)`), and a footer line (`[max_length was reduced to N characters, the
maximum.]`) is emitted when clamped. Verified this wiring is correct and complete; no code change was needed.
Added `src/config.rs` tests exercising `FETCH_MAX_LENGTH_CAP` alongside the new variables (existing
`server.rs` tests already covered the clamp/footer behavior). No `fetch/body.rs` change was needed: that
module enforces `FETCH_MAX_BYTES` (wire/decompressed byte cap), a separate limit from the post-conversion
character-window `max_length`, which server.rs already owns.

### D-4: install/config docs

`README.md` gained a "## Configuration (environment variables)" section: a table of all seven `FETCH_*`
variables (`FETCH_LOG`, `FETCH_TIMEOUT_MS`, `FETCH_MAX_BYTES`, `FETCH_MAX_LENGTH_CAP`,
`FETCH_MAX_CONCURRENCY`, `FETCH_ROBOTS_TXT`, `FETCH_ALLOW_PRIVATE_HOSTS`) with defaults and validation-error
behavior, followed by an explicit subsection ("Two variables are mechanisms, not finished product policy")
documenting that OQ-3 and OQ-4 are still open, that neither variable changes behavior when unset, and stating
the exact C-2 allowlist semantics (exact-hostname-only, redirect always blocked, no IP literals, metadata
never relaxed).

## Test evidence

All commands run from the repo root with `CARGO_TARGET_DIR=~/.cache/fetch-target` (per repo convention,
`/tmp` on this host is a 4 GiB tmpfs but the cache dir keeps builds off it regardless).

- `cargo build --all-targets`: clean.
- `cargo test`: **189 passed, 0 failed, 1 ignored** (lib) + 1 (hostile_rss) + 11 (stdio) + 0 doctests, run
  **twice** with identical results (no flakiness observed). New tests added this sprint: `config.rs` (8 new:
  robots defaults/parsing, allowlist defaults/parsing/canonicalization, extended
  `invalid_values_name_the_variable`), `policy.rs` (5 new: empty-allowlist-is-inert, exact-hostname match,
  metadata/link-local/CGNAT/loopback never relaxed, public addresses untouched by an allowlist),
  `ssrf/resolver.rs` (4 new: allowlisted hostname resolves to a private address, allowlist does not relax a
  metadata answer, allowlist does not cover a different hostname, allowlist never applies to a redirect hop
  even to the same hostname).
- `cargo fmt --check`: clean after `cargo fmt` (one pre-format diff in `config.rs`, fixed).
- `cargo clippy --all-targets --all-features -- -D warnings`: clean, run **twice**.
- `cargo clippy --all-targets --features bench-loopback -- -D warnings`: clean (the compile-time
  loopback-relaxation feature still builds and lints clean with the new `Policy::for_build` signature).
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok` (two pre-existing
  `license-not-encountered` informational warnings for BSD-2-Clause/CC0-1.0 allowances that no current
  dependency uses; not errors, unrelated to this sprint).
- `scripts/check-release-features.sh`: `guard OK` (release profile still has neither `test-support` nor
  `bench-loopback` compiled in).
- `cargo test` was not repeated a third time beyond the two full runs above plus the individual clippy/fmt
  passes, given the size of this change relative to the sprint's time budget; no flakiness was observed in any
  run.

CI (GitHub Actions) status is reported separately after the PR is opened and checks run; see the PR
description / handback for `gh pr checks` output.

## Deviations from AC

1. **Entry criteria not met.** The Sprints 6-12 table row 10 lists "OQ-3 default decided (C-1 needs it), OQ-4
   decided for C-2" as entry criteria for this sprint. Neither was decided. Per explicit instruction, the
   sprint proceeded anyway with C-1/C-2 implemented as inert, decision-free mechanisms (see Revision 19 in the
   sprint plan and the "CRITICAL CONSTRAINT" section above). This is a deviation from the plan's literal entry
   gate, recorded here and in the plan's revision log rather than silently ignored.
2. **`FETCH_ROBOTS_TXT` accepts two values, not a single placeholder token.** The task description mentions
   the variable as "a placeholder only", which could be read as accepting a single trivial value. Instead it
   accepts `ignore` and `enforce` as syntax (both currently no-ops), so the env var's name and accepted syntax
   are stable ahead of B-4, without the *default* deciding OQ-3 (default stays `ignore`, i.e. today's
   behavior). Flagging this interpretation explicitly in case the owner wanted a stricter single-value
   placeholder.
3. **C-3 required no code change.** The architecture line about wiring `max_length_cap` into
   `fetch/body.rs`/the tool schema turned out to already be fully implemented in `src/server.rs` by a prior
   sprint (A-5). Only additional config-level tests were added; this is noted rather than fabricating a
   change that wasn't needed.

## Owner decisions still needed

- **OQ-3 (robots.txt default policy):** still open. `FETCH_ROBOTS_TXT` is parsed/validated but is a no-op;
  B-4 (Sprint 11) needs a decided default before implementing actual robots.txt fetching/enforcement.
- **OQ-4 (private-host allowlist policy):** still open. `ssrf::Policy`'s allowlist mechanism now exists,
  is wired end to end, defaults to empty/inert, and is documented with exact semantics (exact hostname only,
  never relaxes redirects/metadata/IP-literals, relaxes only the RFC 1918 private-range check). The owner
  needs to confirm these semantics as implemented, or reject the mechanism entirely (e.g. remove
  `FETCH_ALLOW_PRIVATE_HOSTS` and the `Policy` allowlist field) before any real deployment sets it.

## Files changed

- `src/config.rs` -- `RobotsMode`, `FETCH_ROBOTS_TXT` and `FETCH_ALLOW_PRIVATE_HOSTS` parsing/validation, tests.
- `src/policy.rs` -- `allow_private_hosts` field, `with_allow_private_hosts`, `check_ip_for_host`,
  `for_build(allow_private_hosts)`, tests.
- `src/ssrf/resolver.rs` -- `resolve_validated`/`validate_target` origin-aware allowlist check, tests.
- `src/fetch/dns.rs` -- updated `resolve_validated` call site for the new `origin` parameter.
- `src/main.rs` -- `Policy::for_build(cfg.allow_private_hosts.clone())`.
- `README.md` -- new "Configuration (environment variables)" section documenting all `FETCH_*` vars and the
  OQ-3/OQ-4 gap.
- `.delivery/artifacts/05-plan/po/sprint-plan.md` -- Revision 19.
- `.delivery/state.md` -- Sprint 9 merged / Sprint 10 started note.
- `.delivery/artifacts/06-dev/sprint-10/dev-report.md` -- this file.
