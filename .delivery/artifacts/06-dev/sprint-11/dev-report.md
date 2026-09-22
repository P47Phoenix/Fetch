# Sprint 11 dev report

Branch: `sprint-11/robots-charset-arm-tests`, off `main` at `0d0ff2f` (PR #14 merged: Sprint 10 C-1/C-2/C-3/D-4).

Scope taken from the live Sprints 6-12 table in `.delivery/artifacts/05-plan/po/sprint-plan.md` (row 11, not guessed):
**B-4, A-8, D-3** (8 pts), exit criteria "OQ-3 decided; runner checked".

## Entry-criteria gap, disclosed up front

Per the sprint-plan row, Sprint 11 formally expects **OQ-3 to be decided** before it starts. OQ-3 (robots.txt
enforce-by-default policy) is still OPEN — it was not decided in Sprint 10 and is not decided here either; this
is the product owner's call, not the delivery agent's, per repo convention. Proceeding anyway (as Sprint 10 did
for OQ-3/OQ-4 under explicit instruction) by delivering B-4 as a **fully implemented, fully tested, but inert by
default** mechanism, exactly mirroring how C-1 (robots placeholder) and C-2 (private-host allowlist) were
delivered in Sprint 10. Today's behavior (`FETCH_ROBOTS_TXT` default `ignore`, robots.txt never fetched or
enforced) is unchanged.

## Stories delivered

### A-8: Charset decoding and User-Agent (2 pts) — DONE
- User-Agent: already correct from A-3b (`fetch-mcp/<version>`, sent on every request); verified unchanged, no
  more specific "descriptive" format is specified anywhere in `docs/PRD.md` or `docs/EPICS.md`, so the existing
  value satisfies the AC as written.
- Charset decoding (new): `src/fetch/charset.rs` detects the response charset from, in priority order, (1) the
  `Content-Type` header's `charset=` parameter, (2) for HTML bodies, a `<meta charset=...>` /
  `<meta http-equiv="Content-Type" content="...charset=...">` tag in the first bytes of the body, (3) UTF-8
  default. Non-UTF-8 charsets are decoded with `encoding_rs` (exact-pinned `=0.8.41`, `default-features = false`,
  added to `Cargo.toml`/`Cargo.lock`). The existing streaming UTF-8 decoder (U+FFFD replacement on invalid bytes)
  is preserved unchanged for the common case (identity transfer, no header charset, ASCII/UTF-8-looking start):
  a small (~1-8 KB) lookahead buffer is used to sniff a meta tag without breaking the existing
  early-abort-on-converter-failure test. Full-body buffering (bounded by the existing byte cap) is used only for
  the two AC-required but rarer cases: a non-UTF-8 header charset, or gzip-compressed HTML with no header charset
  (see Deviations below for why gzip needed its own path).
- Tests added in `src/fetch/tests.rs`: header-charset ISO-8859-1 decodes correctly; meta-only charset (including
  under gzip) decodes correctly; no charset anywhere still defaults to UTF-8 with replacement (regression); an
  unrecognized charset label falls back to UTF-8 rather than erroring or panicking; a dedicated regression proves
  the plain-UTF-8/no-markers path still streams and aborts early on converter failure (no accidental full-buffer
  regression for the common case).

### B-4: robots.txt enforcement — mechanism only, DONE; enforcement default left to the owner (OQ-3)
- New module `src/robots.rs`: a robots.txt parser and `is_allowed(robots_txt, user_agent, path) -> bool`
  matcher, RFC 9309 semantics (most-specific matching `User-agent` group wins, else `*`; longest matching
  path-prefix wins between `Allow`/`Disallow` within the applicable group, `Allow` wins ties). 13 unit tests
  covering precedence, empty file, malformed lines, `*` vs specific UA.
- Wired into `src/fetch/mod.rs`: when (and only when) `Config.robots_txt == RobotsMode::Enforce`, the target
  origin's `robots.txt` is fetched through the *same* guarded `FetchClient`/SSRF/redirect machinery used for the
  real fetch (so the robots.txt fetch itself is SSRF-checked and cannot be redirected to a private address),
  capped at 512 KB, and checked against the requested path before the real fetch proceeds. A disallowed path
  returns the new `FetchError::RobotsDisallowed` (`error[robots_disallowed]: ...`), an explanatory error per AC.
  A missing, 404, timed-out, or otherwise-failing robots.txt fetch is treated as "no restrictions" (fetch
  proceeds), per AC. `RobotsMode::Ignore` (the unchanged default) skips this path entirely — zero behavior
  change from Sprint 10.
- No new environment variable was added: `FETCH_ROBOTS_TXT` (added in Sprint 10's C-1 as a validated placeholder)
  is the only toggle, and its default (`Ignore`) was **not changed**.
- Integration tests added in `src/fetch/tests.rs`: disallowed path refused with `Enforce`; allowed path proceeds;
  missing/404 robots.txt proceeds; `Ignore` mode proves the mechanism is inert (disallowed path still succeeds);
  the robots.txt fetch itself is size-capped at 512 KB; the robots.txt fetch is SSRF-checked; no robots check is
  re-triggered on redirect hops or on the robots.txt sub-fetch itself (see Deviations).

### D-3: Test suite on aarch64 (3 pts) — DONE
- `.github/workflows/ci.yml`: the `test` job is now a matrix (`amd64`/`ubuntu-latest`,
  `arm64`/`ubuntu-24.04-arm`), each leg running the same three `cargo test` invocations (default,
  `bench-loopback`, `test-support`) plus the bench-harness self-test, per ADR-007 (native hosted arm64, no QEMU
  substitute for a required check). D-7's existing arm64 CI presence (noted in the EPICS AC as already partially
  covering this) was confirmed to be only the `arm-bench`/`release.yml` per-platform jobs, not the PR-level
  `test` job — so this was still needed, not a re-estimate-to-zero.
- `actionlint` run against the changed workflow file: clean, no findings.
- **Disclosed risk, not a story deviation:** this changes the required-check name from `test` to
  `test (amd64)` / `test (arm64)`. If GitHub branch protection has `test` configured by exact name as a required
  status check, the repo owner will need to update that configuration to the two new names (or GitHub may treat
  them as new, currently-non-required checks until then). This is an operations/administration action outside
  what this dev agent can do (no access to repo branch-protection settings) — flagged here rather than silently
  left for someone to discover.

## Deviations from AC, with justification

1. **A-8 streaming vs. full-buffer decode split** (see above): the architecture's early-abort-on-converter-error
   guarantee (existing regression test) rules out unconditionally buffering the whole body just to sniff a meta
   tag. Resolved with a small bounded lookahead for the common identity/UTF-8 case and full buffering (already
   bounded by the existing byte cap) only for the two cases that need it. Not a scope reduction — all AC bullets
   are met — but noted because it is an implementation choice beyond what the AC states literally.
2. **Gzip + meta-charset interaction**: `flate2`'s streaming `GzDecoder` does not reliably flush partial output
   on the code paths available here, so mid-stream meta-tag sniffing on gzip content is unsafe; gzip+HTML+no-
   header-charset is handled by fully decompressing (`.finish()`) then sniffing, rather than the streaming
   lookahead used for identity bodies. Behaviorally equivalent, implementation detail only.
3. **B-4 recursion guard**: `run()` (the shared internal fetch path) gained a `check_robots: bool` argument so
   the robots.txt sub-fetch does not re-trigger a robots check on itself (this caused infinite recursion before
   the fix) and does not re-acquire a concurrency slot (which would self-deadlock at
   `FETCH_MAX_CONCURRENCY=1`). The public entry point is unaffected; this is an internal-only signature change,
   not a behavior or AC deviation.

No AC bullet from B-4, A-8, or D-3 was silently dropped or partially delivered.

## Quality gates — all run locally, all green

(`TMPDIR=~/.cache/tmp`, `CARGO_TARGET_DIR=~/.cache/cargo-target-fetch`, since `/tmp` is constrained)

- `cargo fmt --check` — clean.
- `cargo clippy --locked --all-targets -- -D warnings` — clean.
- `cargo clippy --locked --all-targets --features bench-loopback -- -D warnings` — clean.
- `cargo clippy --locked --all-targets --features test-support -- -D warnings` — clean.
- `cargo test --locked` — 229 passed, 0 failed, 1 pre-existing ignored, no regressions.
- `cargo test --locked --features bench-loopback` — all pass.
- `cargo test --locked --features test-support` — 229 passed, 0 failed, 1 pre-existing ignored.
- `cargo build --release --locked` + `ldd` — no OpenSSL/native-tls/crypto linkage (release-ldd-guard).
- `scripts/check-release-features.sh` and `--self-test` — guard OK, self-test OK (release-guard).
- `cargo test --locked --release --lib -- a3b_merge_gate refusal_ dial_once differential b1_every_blocked_class
  b2_ b3_` — 16 passed (a3b-merge-gate).
- `cargo deny check` — advisories ok, bans ok, licenses ok, sources ok (two harmless
  license-not-encountered warnings for allow-listed-but-unused license identifiers, not failures).
- `cargo llvm-cov` + `scripts/coverage_gate.py ... --min 90` — 96.6% combined
  (`src/ssrf/mod.rs` 99.8%, `ranges.rs` 97.9%, `resolver.rs` 100%, `src/fetch/mod.rs` 89.6%,
  `src/convert/window.rs` 100%) — PASSED. (`src/fetch/mod.rs`'s 89.6% single-file figure is the same
  pre-existing, previously-disclosed gap noted in Sprint 8's B-5 dev notes, not a new regression.)
- `actionlint .github/workflows/ci.yml` — clean (workflow file touched for D-3).

CI results from the actual GitHub Actions run (not just local): see the PR — `gh pr checks` output is recorded
in the handback report for this sprint; poll results there for the authoritative CI status, since GitHub-hosted
runner behavior (especially the new arm64 `test` matrix leg) can only be confirmed by an actual run.

## Owner decisions still needed

- **OQ-3** (robots.txt enforce-by-default policy): unchanged, still OPEN. B-4's mechanism is complete and
  tested but ships inert (`FETCH_ROBOTS_TXT` default `ignore`) until the owner decides the default.
- **OQ-4** (private-host allowlist activation/governance): not a Sprint 11 story; unchanged from Sprint 10,
  still OPEN, no new action here.

Full diff: branch `sprint-11/robots-charset-arm-tests`, files changed listed in the PR.
