# QA review: Sprint 4 (A-4 conversion, E-4 G4a), PR #8, head 9d3dc21

Validator: QA (fresh, isolated, read-only on the repo). All builds and probes ran from a clean `git archive HEAD` in /tmp/qa with a target dir under ~/.cache (the /tmp quota filled up mid-run; the failures that produced were quota errors, not code failures, and every step was re-run).

## Decision: DONE for the Sprint 4 exit under the E-7 fallback. A-4 itself stays NOT Done until Sprint 5. E-4 is Done. No blockers.

## Verification matrix (clean archive)
| Check | Result |
|---|---|
| cargo fmt --check | ok |
| clippy -D warnings, default / bench-loopback / test-support | ok x3 |
| cargo test --locked, default / bench-loopback / test-support | 135 / 136 / 135 lib pass (1 ignored timing test); integration 9 / 11 / 9 pass |
| release build | ok, 3,680,488 B; `--version` = `fetch-mcp 0.0.0 commit=unknown cargo-lock=65a5a943...cce1` (archive build, as designed) |
| check-release-features.sh and --self-test | guard OK, self-test OK |
| cargo deny check | advisories, bans, licenses, sources ok |
| bench/selftest.py | SELFTEST PASSED |
| actionlint (in work tree) | clean |
| PR checks on 9d3dc21 | 14/14 success (ci, arm-bench, bench runs 35558898706/773/839) |

## A-4 acceptance criteria
| AC | Verdict | Evidence |
|---|---|---|
| Headings, links, lists, code preserved | PASS | real stdio via bench-loopback build: h1-h3, nested ul, ol start=3, links resolved absolute with entities decoded, fenced rust block, inline code, pipe table with separator, blockquote, image with alt, entities (&amp; &lt; &copy; &#x41;) all correct |
| script/style/hidden nav text absent | PASS on tested cases | script, style, noscript, nav, footer, [hidden], display:none, cookie-banner, template, svg script, select, textarea, comments all absent; upper-case SCRIPT tags, unclosed script (swallows rest, safe), NUL-split tags handled |
| 95% success / 50% median reduction / no script text on E-7 50 pages | NOT RUN (fallback) | E-7 lists not supplied; no corpus fabricated. Docs say so plainly: README line 18, A-4 report gaps 1, sprint plan revision 12 ("A-4 not Done until met in Sprint 5"). Nothing I found claims quality is proven. |
| 1 MiB overhead <= 500 ms p95 on aarch64 | PASS (gnu only) | CI log: arm64 gnu p95 59.9 ms, amd64 gnu 56.7 ms. musl cells skipped the step (no musl figure; stated in E-4 gap 5) |
| Converter behind a trait | PASS | `Converter` trait (push, finish), `Passthrough`, selected in `convert::for_response` |

### Hostile inputs (real stdio, stdout stayed pure JSON-RPC, process stayed alive throughout)
- 200,000-deep div/span/p/blockquote, 20,000-deep li and table nests, a 3 MiB tag name, 3 MiB href: all return `error[converter_limit]: ... retry with raw=true` in ~10 ms, no crash, next call works.
- 8 MiB attribute: `too_large` (cap), correct.
- Unclosed tags, unterminated tag/comment, empty and whitespace-only bodies: clean, no panic.
- Invalid UTF-8 (0xFF, 0xC3 0x28, surrogate, truncated 4-byte): U+FFFD replacement, no error.
- NUL in text and inside a tag name: U+FFFD.
- 200,000 links: output capped at 5000 chars (default length) in 0.13 s.

### Mutation testing (unit tests, `cargo test --lib`)
| Mutant | Result |
|---|---|
| M1 remove "script" from DROP_TAGS | killed (script_style_and_chrome_are_dropped) |
| M2 rewriter memory limit raised to 2 GiB | killed (2 tests, incl. the converter_limit fetch test) |
| M3 heading level off by one | killed (6+ tests) |
| M4 allow `javascript:` scheme in links | killed (relative_links_resolve_and_unsafe_ones_lose_the_destination) |
| M5 `take_failure` swallows the converter error (per-chunk abort) | SURVIVED: `conv.finish` re-reports the failure, so the user-visible error is unchanged; only the early abort (stop reading after a failure) is untested. Low risk (bounded by the 5 MiB cap) |

## E-4 acceptance criteria and G4a numbers
Independently recomputed the medians from the raw `samples_kB` in the CI logs of run 35557702662 (head 8b5b489; 10 valid samples per scenario, no invalid reasons, all four cells). Every figure in BENCHMARK s16, the E-4 report, EPICS and the sprint plan matches:

| cell | idle | gating peak | 50 MiB CL / chunked ratio | g6 (RECORDED) |
|---|---|---|---|---|
| arm64 gnu | 3.91 | 5.38 | 0.924 / 1.006 | 8.18 |
| arm64 musl | 2.59 | 4.56 | 0.930 / 1.026 | 13.33 |
| amd64 gnu | 4.62 | 6.16 | 0.949 / 1.000 | 8.85 |
| amd64 musl | 2.35 | 4.70 | 0.891 / 1.014 | 8.82 |

- Boundedness <= 1.10 and peak <= 40 MiB hold with wide margin. redirect-chain5 (6.80 and 7.25 MiB on musl) is recorded and correctly excluded from the gating max.
- Identity: `build-candidates.sh` logged "identity ok" for gnu and musl in all four cells; every `--version` shows commit acddaaeb... (the PR merge commit) and cargo-lock 65a5a943...cce1, which equals the sha256 of Cargo.lock in the archive. Docs disclose that the measured commit is the merge commit, not the PR head.
- `--version` build identity is deterministic (no timestamps or paths; hand-written SHA-256 with a test against sha256sum). Built the same tree twice: identical bytes/hash line. `git archive` builds print `commit=unknown`.
- Gate false-pass probes (real `measure.py --gate`, real binaries, fixtures generated and verified): stand-in script as bench -> REFUSED (no marker); shipped file claimed as bench -> REFUSED; bench claimed as shipped -> REFUSED; commit mismatch between shipped and bench -> REFUSED (rc 2); `commit=unknown` (git-archive build) on both -> REFUSED; no `--peer-binary` -> REFUSED; incomplete G4a set -> REFUSED; `--runs 5` -> REFUSED; `--gate --smoke` -> REFUSED; G4b scenarios -> REFUSED as not implemented; QEMU binfmt handler registered -> `native_host` returns False (strict); <10 valid samples -> INVALID (code path measure.py 387, selftest covers). Positive control (matching identity, 10 runs, local x86_64): PASS, gating peak 6.40 MiB.
- Honesty: docs consistently say G4a pass is not the memory gate closing (G4b, Sprint 5), single run per cell, g6 noisy and 3-in-flight-plus-7-queued, amd64 run beyond the aarch64-only AC wording, public-host cross-check used a ~2 MiB Wikipedia page (not 5 MiB) and is advisory, bench-gate is not a required check, macOS reader unbuilt. I confirmed the public-host numbers in the logs (ratios 0.997 to 1.02).
- Carry-forward cleanup, 7 of 7 landed: (1) ADR-004 doc lines above `content_encoding_is_gzip`; (2) exact-match zigbuild pin check; (3) `native_gate_host` uses strict probe; (4) lscpu fallback (CI host records show Neoverse-N2); (5) stale docs corrected (grep finds no "two CI runs" / "pull request #5"); (6) `a9_echo_has_no_userinfo_or_fragment_end_to_end` test present and passing; (7) report.py malformed-JSONL selftest passes.

## Non-blocking findings
1. Literal `<script>` can appear in output from non-script sources: `<img alt="<script>x</script>">` yields `![<script>x</script>](...)`; `<xmp>`/`<plaintext>` content passes through verbatim (including `</main>`); legitimate escaped text (`&lt;script&gt;`) decodes to `<script>`. None is script-element content, but the E-7 check "no output contains `<script>` text" must define its rule (element content vs substring) or it will false-fail on tutorial pages, and alt text should perhaps be sanitised.
2. Control characters, ESC (0x1B) and bidi override U+202E pass through to output unfiltered (relevant to B-6 untrusted-content labelling).
3. No markdown escaping of `*`, `_`, `[`, backticks in text (documented gap); text can forge markdown structure.
4. Tokenizer limit: about 5,000 unclosed `<p>`/`<li>`/`<td>` convert, 20,000 give `converter_limit` (documented; ADR-002 rule "raise to 4 MiB if >2% of URL set" cannot be evaluated until E-7).
5. Build identity does not detect a dirty working tree, and `FETCH_MCP_COMMIT` can be set to any 40-hex value; identity proves same commit string and lockfile, not same source.
6. Docs consistency: EPICS A-4 story has no status note (E-4 does); EPICS line 132 ("checks run in A-4 (Sprint 4)") and line 501 (Sprint 4 "Convert (95%/50% checks)") are stale after E-7 moved; README line 9 lists A-4 under "What works today" (line 18 then qualifies it).
7. Mutant M5 survives (no test that a converter failure aborts reading early).
8. Idle-delta bound (0.5 MiB) is checked by reading the record, not enforced by a script (already E-4 gap 4); public-host check is advisory and 2 MiB; bench-gate not a required check.
9. The 1 MiB overhead was measured on gnu cells only; musl skipped.
