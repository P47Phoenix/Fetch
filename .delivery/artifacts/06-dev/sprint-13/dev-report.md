# Sprint 13 dev report: OQ-3/OQ-4/OQ-7 resolution, E-5/E-7 rework, branch-protection/deviations sign-off

Branch: `sprint-13/oq-resolution-finalization`, off `main` at `ef1e52f` (4 commits after the `56f5a79` Sprint 12 merge, all automated E-6 bench-baseline-refresh commits). Date: 2026-09-22.

The project owner made seven previously-open decisions. This sprint implements all seven; see
`.delivery/artifacts/05-plan/po/sprint-plan.md` Revision 24 for the plan-side record.

## 1. OQ-7 (image distribution/licence): RESOLVED — open source, MIT OR Apache-2.0

- `LICENSE` (Apache-2.0, present since the initial commit) renamed to `LICENSE-APACHE`; new `LICENSE-MIT`
  added (standard MIT boilerplate, copyright holder "the fetch-mcp project contributors" — `Cargo.toml` had
  no `authors` field to draw a name from).
- `Cargo.toml`: `license = "MIT OR Apache-2.0"` added.
- `README.md`: new "License" section linking both files; the OQ-7 summary line in "What has not been checked
  yet" updated from "still open" to resolved, with a dated decision statement.
- `.github/workflows/release.yml`: the `publish` job's `if: false && startsWith(github.ref, 'refs/tags/v') &&
  inputs.confirm_publish == 'true'` condition had the `false &&` prefix removed, leaving
  `if: startsWith(github.ref, 'refs/tags/v') && inputs.confirm_publish == 'true'`. **The
  `inputs.confirm_publish == 'true'` conjunct is unchanged and is still mandatory** — a bare tag push cannot
  reach the job; a human must separately run `workflow_dispatch` with `confirm_publish=true` after the tag
  exists. Comments throughout the file (top-of-file block comment, the job's own comments, the
  `confirm_publish` input description) rewritten to state OQ-7 is resolved while explicitly restating that the
  manual-confirmation gate is unweakened.
- `docs/ci-branch-protection.md`'s "Licence" section, `docs/EPICS.md`'s D-2 and D-6 sections, `docs/SSRF.md`
  (no direct OQ-7 mention, unaffected) updated from "still open"/"OPEN" to resolved, dated 2026-09-22.
- **No `v1.0` git tag was created or pushed.** Per instruction, tag creation is reserved for the coordinator
  after this PR is reviewed and merged.

## 2. OQ-4 (private-host allowlist governance): RESOLVED — mechanism kept, new master disable switch added

- New config var `FETCH_ALLOW_PRIVATE_HOSTS_ENABLED` (`src/config.rs`), parsed with the same convention as
  `FETCH_ROBOTS_TXT`: case-insensitive, error-on-invalid-value naming the variable. Default `false`.
- `Config::allow_private_hosts_enabled: bool` field added; threaded through `main.rs` into
  `Policy::for_build(allow_private_hosts, allow_private_hosts_enabled)` (signature changed, both call sites
  and all tests updated).
- `src/policy.rs`: new `Policy::allow_private_hosts_enabled: bool` field, `Policy::with_allow_private_hosts_gated`
  constructor, `Policy::allow_private_hosts_enabled()` accessor. `check_ip_for_host` now requires
  `self.allow_private_hosts_enabled` to be true, IN ADDITION to the existing exact-hostname match, before it
  relaxes anything. The IP-literal path (`check_ip`) never consulted the allowlist at all and is untouched.
- **This is a separate, independent gate from list contents**, exactly as specified: a populated
  `FETCH_ALLOW_PRIVATE_HOSTS` list with the switch left at its default (`false`) still fails closed.
- Table-driven regression tests: `src/policy.rs::master_switch_regression_matrix` covers all four
  switch×list combinations (off+populated, on+populated+match, on+populated+no-match, on+empty,
  off+empty) and, for every row, reasserts that metadata addresses, loopback, CGNAT and IP-literal rejection
  are unaffected by any combination. Existing `src/ssrf/resolver.rs` and `src/ssrf/mod.rs` C-2 tests updated
  to use `with_allow_private_hosts_gated(..., true)` where relaxation was expected, keeping the same coverage.
- Docker/README: a new `docker run -e FETCH_ALLOW_PRIVATE_HOSTS_ENABLED=true` example added alongside the
  existing config table and OQ-4 narrative — no new Docker-specific plumbing, just the same `-e` mechanism
  every other `FETCH_*` variable already uses.
- `docs/SSRF.md`, `README.md`, `docs/ci-branch-protection.md`, `docs/EPICS.md` (C-2 story) updated to describe
  the mechanism as available-and-enabled-by-operator-choice, with the master switch as the recommended way to
  keep it off by default.

## 3. OQ-3 (robots.txt default): RESOLVED — stays `ignore`, no code change

No code change: the default was already `ignore` and stays `ignore`. Doc/comment updates only:
`README.md` (config table row + the dedicated OQ-3/OQ-4 section, retitled), `docs/ci-branch-protection.md`,
`docs/SSRF.md` (no direct mention needed), `src/config.rs` module doc comments and `RobotsMode` doc comment,
`src/robots.rs` module doc, `src/fetch/tests.rs` section comment, `docs/EPICS.md` (B-4, C-1 sections). All
now state: OQ-3 is resolved, `ignore` stays the default by design (network-level ACLs elsewhere handle this
concern, per the owner), not by omission; `FETCH_ROBOTS_TXT=enforce` remains available but is not, and will
not become, the default.

## 4. E-5/E-7 rework: local-fixture harness replaces the live-URL dependency

- Verified before building anything: no E-7 harness code existed anywhere in `bench/` (grepped for
  "conversion success", "token reduction", "95%" across `.py`/`.rs` — nothing outside
  `src/convert/markdown/tests.rs`'s own unit tests). E-5's harness (`bench/smoke.py`, `bench/e5_report.py`,
  built Sprint 7) does exist and is left unmodified — it still works for a real live smoke if ever wanted.
- New `bench/corpus_check.py`: discovers `*.html` files in `bench/corpus/`, serves them from a
  loopback `http.server.ThreadingHTTPServer`, fetches each through the real `fetch-mcp` binary via the
  existing `measure.py` stdio driver (reused, not reimplemented), and checks: >= 95% conversion success (no
  `error`/`isError` in the MCP result), median token reduction (whitespace-token count, raw vs converted) >=
  50%, and no literal `<script` text survives in the converted output of any fixture that actually contained
  a `<script` tag. Also regenerates a plain `sha256sums.txt` manifest (`corpus_check.py manifest`).
- Three placeholder fixtures added, clearly marked as tooling-proof examples, not a real corpus:
  `bench/corpus/example-01-article.html` (prose/nav-boilerplate), `example-02-table-heavy.html`
  (table conversion), `example-03-inline-script.html` (inline `<script>`/`<style>` stripping).
  `sha256sums.txt` generated for all three.
- Verified end to end locally: `cargo build --locked --release --features bench-loopback`, then
  `python3 bench/corpus_check.py check --binary target/release/fetch-mcp` runs successfully, correctly
  reports `success_pass: true`, `script_pass: true` (after a fixture wording fix that had tripped the known
  escaped-`<script>`-text limitation on its own placeholder prose — not a converter bug), and correctly
  reports `reduction_pass: false` on the 3 tiny placeholders (expected and documented: not a real corpus).
- New `docs/TEST-FIXTURES.md`: documents exactly what to generate (suggested count ~15-50, content diversity
  — prose article, nav/boilerplate-heavy, table-heavy, code-documentation-heavy, inline-script/style, edge
  cases like empty body/malformed HTML), naming convention (`NN-slug.html`), drop location
  (`bench/corpus/`), and the exact commands to regenerate the manifest and produce the pass/fail
  report.
- `docs/EPICS.md` and the sprint plan (Revision 24) updated: A-4/E-5/E-7 are **NOT DONE**, unchanged status,
  but the blocker is now "owner drops local fixture files per `docs/TEST-FIXTURES.md`" instead of "owner
  supplies a live 50-URL/10-URL list" — no live URLs, no external-website dependency, a materially
  lower-friction ask.

## 5. Branch protection: RESOLVED — deliberately not configured

`docs/ci-branch-protection.md` rewritten: a new "Branch protection: RESOLVED, deliberately not configured
(2026-09-22)" section states the owner's decision (solo-developer repo, not worth the settings friction) as
accepted, not a gap. The old "Owner quickstart" content is kept only under a clearly-marked "Historical: what
'turn on the rule' would have looked like (not pursued)" heading, explicitly stated as not an open action
item. The "Read this first" table's branch-protection row updated to match. Sprint-plan open-items list
(Revision 24, item 5) records the same. No GitHub settings were touched (no access, unchanged fact).

## 6. Sprint 5 deviations and panic=abort: RESOLVED — formally accepted as-is

Recorded in sprint-plan.md Revision 24, item 6: the project owner formally acknowledges, dated 2026-09-22,
both (a) the Sprint 5 deviations from `.delivery/artifacts/06-dev/sprint-5/stage-summary.md` and (b) the
`panic = "abort"` release-profile choice (`Cargo.toml`, D-7/Sprint 0), both accepted as-is with no code
change. No `Cargo.toml` release-profile edit was made.

## 7. coverage/release-ldd-guard: RESOLVED — de facto required by process

`docs/ci-branch-protection.md`: new "De facto required checks (process-enforced, not GitHub-native)"
subsection states that `coverage` and `release-ldd-guard` run on every PR and are already required in
practice by the delivery coordinator's own merge checklist, independent of GitHub's native required-checks
list (which, per item 5, will not be configured). No code change.

## Test evidence

Run locally on this branch, in order:

```
cargo fmt --check                                              # clean
cargo clippy --locked --all-targets -- -D warnings              # clean
cargo clippy --locked --all-targets --features bench-loopback -- -D warnings   # clean
cargo clippy --locked --all-targets --features test-support -- -D warnings     # clean
cargo test --locked                                             # 240 passed, 1 ignored, 0 failed (unit)
                                                                  # + 1 hostile_rss + 11 stdio integration tests, all ok
cargo test --locked --features bench-loopback                   # 17 stdio tests (incl. new pagination/content-type
                                                                  # loopback tests), all ok
cargo test --locked --features test-support                     # 11 stdio tests, all ok
bash scripts/check-release-features.sh --self-test               # self-test OK (release build clean of
                                                                  # test-support/bench-loopback markers)
python3 scripts/dependency_count_gate.py Cargo.toml --max 15     # 11 of <= 15, within NFR-05
cargo build --locked --release --features bench-loopback         # builds
python3 bench/corpus_check.py manifest                           # 3 fixtures, sha256sums.txt regenerated
python3 bench/corpus_check.py check --binary target/release/fetch-mcp
                                                                  # success_pass: true, script_pass: true,
                                                                  # reduction_pass: false (expected: 3 tiny
                                                                  # placeholders, not a real corpus)
```

New/changed `src/policy.rs` regression tests specific to OQ-4:
`master_switch_regression_matrix`, `allowlisted_hostname_relaxes_private_range_only_for_exact_match_when_enabled`,
`allowlist_never_relaxes_metadata_link_local_or_cgnat_even_when_enabled`. New `src/config.rs` tests:
`allow_private_hosts_enabled_default_is_false`,
`allow_private_hosts_enabled_accepts_true_and_false_case_insensitively`,
`allow_private_hosts_enabled_is_independent_of_list_contents`.

## Deviations

None beyond what each section above already discloses (the placeholder-fixture wording fix for the
`<script>`-text known limitation, and using a generic "fetch-mcp project contributors" copyright holder in
`LICENSE-MIT` since `Cargo.toml` had no `authors`/`repository` field to draw a real name from).

## Files touched (non-exhaustive, by area)

- Licensing: `LICENSE-APACHE` (renamed from `LICENSE`), `LICENSE-MIT` (new), `Cargo.toml`,
  `.github/workflows/release.yml`, `README.md`, `docs/ci-branch-protection.md`, `docs/EPICS.md`.
- OQ-4: `src/config.rs`, `src/policy.rs`, `src/main.rs`, `src/ssrf/resolver.rs`, `src/ssrf/mod.rs`,
  `src/ssrf/ranges.rs`, `README.md`, `docs/SSRF.md`, `docs/EPICS.md`.
- OQ-3: `src/config.rs`, `src/robots.rs`, `src/fetch/tests.rs`, `README.md`, `docs/ci-branch-protection.md`,
  `docs/EPICS.md`.
- E-5/E-7 rework: `bench/corpus_check.py` (new), `bench/corpus/example-*.html` (new, placeholders),
  `bench/corpus/sha256sums.txt` (new), `docs/TEST-FIXTURES.md` (new).
- Branch protection / de facto required checks: `docs/ci-branch-protection.md`.
- Sign-offs: `.delivery/artifacts/05-plan/po/sprint-plan.md` (Revision 24).
