# Technical Writer review: plain-language rewrite (HEAD cf108d4)

Scope: README.md, docs/BENCHMARK.md, docs/ci-branch-protection.md. Read-only; no edits.
Method: `git archive HEAD` extracted under $CLAUDE_JOB_DIR/tmp/x (x86_64 Linux host, Python 3.14, cargo present); every documented command run.

## Verdict
No blocking findings. Every quickstart command behaved as documented. Facts, targets and honesty caveats are preserved.

## Blocking
None.

## Non-blocking (ranked)

1. docs/BENCHMARK.md:51 (glossary INVALID). Says INVALID is "usually fewer than 10 valid samples, or the fixtures did not match". A fixture hash mismatch is REFUSED (exit 2, measure.py:232), not INVALID. The doc's own table at :163 says "Refusal". Fix: drop "or the fixtures did not match" from INVALID.
2. docs/BENCHMARK.md:147-154, :165 (binary identity). The identity check runs after the native-aarch64 check (measure.py:225 vs :242). On any non-aarch64 host a `--gate` run exits 3 first, so a beginner cannot reach "exit 2, reason `binary identity`" off-runner. :145 covers only the override and incomplete refusals. Add: "the identity refusal is reached only on aarch64 (elsewhere you get exit 3)".
3. docs/BENCHMARK.md:99-102 (step 3). Both builds write the same file, `target/release/fetch-mcp`. Building the bench kind then the shipped kind overwrites the first. A beginner following "Later steps assume you did this" could hand step 5 the wrong kind. Add "each build overwrites the last; rebuild for the kind you measure".
4. docs/BENCHMARK.md:116. "verdict is ADVISORY_PASS or FAIL". Advisory runs can also give INVALID or INCOMPLETE (verified: skeleton gives INVALID; a lone 50 MB scenario gives INCOMPLETE). "Never PASS" remains true.
5. docs/BENCHMARK.md:122. The stated blocking stories "A-2 and A-3a" are unverified. A-3a is the SSRF core in EPICS; the `fetch` tool that the handshake requires arrives with the fetch client (A-3b). Confirm against EPICS or say "later stories".
6. docs/ci-branch-protection.md:26. "four HTTP-backend feature sets" matches ci.yml:41 (default plus four sets), but reqwest appears twice, so there are three backends and four sets. Suggest "four feature sets across three HTTP backends".
7. README.md:1-17. The README states the skeleton, OQ-7 and publish=false caveats. It does not mention that hosted Actions, cargo-deny, cargo-audit and native aarch64 never ran, or that the 10 MB idle / 40 MB peak targets are unverified. The two linked docs carry those caveats in "Read this first", so nothing is lost. Optional: one sentence and link in README. `publish = false` is unexplained for beginners.
8. docs/BENCHMARK.md:213-233, :206-208, :253-262. Jargon is used without definition: "pinned interim script", `.2.17`, sha256, chunked, Content-Length, gzip, cgroup (the last is defined inline at :194). These sections are reference material after the quickstart, so this is acceptable. A short glossary addition would finish the job.
9. docs/BENCHMARK.md:174. "Gate names versus scenario IDs" is a bare paragraph, not a heading. The glossary link "(see ... below)" works but is not anchorable.

## Check results

(1) Beginner walkthrough, from a clean extract:
- Step 1: `python3 bench/selftest.py` gave `SELFTEST PASSED`, rc 0, 33 s wall ("about 1 minute" is true).
- Step 2: printed `generated in <path>/bench/fixtures`.
- Step 3: the shipped build finished in under 2 s (warm cache). The binary exists.
- Step 4: the stand-in smoke run printed 3 JSONL lines (host, scenario, summary) and exited 0, in under 1 s ("a few seconds" is fine). The summary has ADVISORY_PASS and the scenario line says PASS, as the doc warns.
- Step 5 on x86_64: exit 3 with a REFUSED summary. Refusals confirmed: `--gate --smoke` gives exit 2; a partial bench gate set gives exit 2 with "--gate incomplete"; a lone `g4a-50mib-cl` smoke run gives INCOMPLETE, exit 2; `--runs 3` without smoke is refused, exit 2.
- The skeleton run (`--smoke`, shipped kind) gave INVALID, exit 2, with "server closed stdout" in the invalid reasons, matching :164.
- Not timed: a shipped idle run against the stand-in, because 10 runs x 30 s would take about 5 minutes.
- Also run: `cargo fmt --check`, clippy on the default features, `cargo test --locked`, `scripts/check-release-features.sh` and its `--self-test` all pass. actionlint is installed here; cargo-deny and cargo-audit are not.

(2) Glossary against code:
- Exit codes 0/1/2/3, ADVISORY_PASS, the INCOMPLETE vs `--gate` incomplete-set distinction, the `--gate` refusal list (smoke, target overrides, settle != 30, parallel-idle > 1, any child-env because the allow-list is empty) and the four identity rules all match measure.py. The G4a/G4b required-set rule matches `required_gate_set`, including the default to G4a. Exceptions are findings 1 and 2. The scenario table matches scenarios.py (implemented flags, `g4a-50mib-cl` min_bytes = 0, bounded_vs).

(3) Fact preservation. Preserved and consistent with EPICS (E-1, D-7, E-7 deferral, OQ-7) and the A-1 spike report (3.7-5.2 MB idle, 6.1/8.7 MB peaks, 56 MB buffered DOM, QEMU 13.0 MB, 0.5 s settle, 16 MiB cap):
- 10 MB idle / 40 MB peak, MiB units, and the 1.10x boundedness rule.
- Never-run items: hosted Actions, cargo-deny, cargo-audit, native aarch64.
- OQ-7 undecided with the Apache LICENSE present and publish=false.
- The 50-URL list deferred.
- The skeleton not for real MCP clients.
- Smoke exit 0 is not a result.

(4) Consistency and links:
- CI check names fmt, clippy, test, deny, release-guard match ci.yml job ids. The test row's "four commands" is correct. dependabot.yml exists and is weekly for cargo and github-actions. deny.toml has yanked = deny. Script options match.
- The README anchors `#quickstart-contributors` and `#glossary` resolve. Internal "section N" references resolve. Cargo.toml, measure.py, fixtures.py, serve.py and scenarios.py references (BENCHMARK sections 1, 5, 6, sec 8) are still valid. Architecture 9.2, 11.0, 11.1 and 11.2 exist.

(5) No contradictions with EPICS or the architecture found.
