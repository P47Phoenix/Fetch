# QA review: Sprint 5 (A-5, A-6, G4b), PR #9 head cd99c95

Validator: QA (fresh, isolated, read-only on the repo; builds under ~/.cache/qa-s5). E-7 is not in this PR; A-4 stays NOT Done.

DECISION: DONE (A-5 and A-6 DONE; G4b PASS independently verified; A-4 unchanged, NOT Done)

## 1. Gates from a clean `git archive cd99c95`
| Gate | Result |
|---|---|
| cargo fmt --check | pass |
| clippy -D warnings (default, bench-loopback, test-support) | pass x3 |
| cargo test --locked (default / bench-loopback / test-support) | pass x3 (167 / 168 / 167 unit; stdio 9 / 15 / 9) |
| release build | pass |
| check-release-features.sh and --self-test | guard OK, self-test OK |
| cargo deny check | advisories, bans, licenses, sources ok |
| bench/selftest.py | SELFTEST PASSED |
| actionlint (in repo) | clean |
| CI at cd99c95 (runs 35651351749 ci, 35651351728 bench, 35651351874 arm-bench) | all success |

Note: the pagination and content-type stdio tests live under `bench-loopback` (loopback fixture), so plain `cargo test` does not run them; CI runs all three feature sets.

## 2. Black-box over real stdio (bench-loopback binary, local HTTP server)
- FR-04: 20,000-char page, max_length 5000, four sequential calls following the returned start_index: concatenation equals the page exactly, no gap or overlap. Last page carries `[Total length: 20000 characters.]`.
- Window at start on 3 MiB body: 10 chars plus `start_index=10` footer (early stop). Window at end: correct chars plus total length. Beyond end / exactly at end / u64::MAX / i64::MAX start_index: `[No content at start_index=N: the content is 20000 characters long.]`, isError false. Empty body: same message with 0.
- Clamp: max_length 1e9 returns 100,000 chars, states the clamp and the next start_index.
- Chunked 4 MiB body (inside cap): window near the end succeeds. Chunked 6 MiB body (beyond 5 MiB cap): start_index near the end gives `error[too_large]`. Same body at start 0 returns early (early stop, by design).
- Multi-byte (é, CJK, emoji): window boundary on character boundary, verified equal to the reference slice.
- Invalid inputs: max_length -1, 5.5, "5", 1e30; start_index -1, 1.5, 1e30; raw "yes": all isError with the field named (rmcp text `failed to deserialize parameters: <field>: ...`). max_length 0: `error[invalid_argument]: max_length: must be at least 1`. No crash, no protocol error.
- FR-08 / A-6: text/html converted (script dropped); text/plain, application/json, application/xml, text/xml, application/ld+json, application/atom+xml returned as text; image/png, application/pdf, application/octet-stream: isError, `error[unsupported_content_type]` naming the type, also with raw=true. raw=true on HTML returns the unconverted body (script kept). Missing Content-Type: `<!DOCTYPE html>` body converted, plain text and JSON body returned as text; raw on untyped HTML returns unconverted.
- A-9 interaction: after a redirect the `URL:/Status:` header (Sprint 3 format, EPICS A-9) precedes the content and stays outside offsets; window, continuation footer and beyond-end message all work behind it. No header without a redirect.
- Charset: `text/html; charset=iso-8859-1` body decodes with U+FFFD (non-UTF-8 charsets are A-8, Sprint 11; out of scope here).

## 3. Mutation testing (patched copies of the archive; `cargo test --features test-support`, survivors re-run with bench-loopback)
| Mutant | Result |
|---|---|
| M1 next start_index off by one (server.rs) | killed (2 tests) |
| M2 clamp note dropped (server.rs) | survived test-support run; killed by `pagination_end_to_end` under bench-loopback (CI runs it) |
| M3 `+json` suffix removed (convert/mod.rs) | killed (2 tests) |
| M4 `taken < take` -> `<=` (window.rs) | survived: equivalent mutant (zero room path is a no-op) |
| M5 skip `n <= skip` -> `<` (window.rs) | survived: equivalent mutant (same result via the else path) |
| M6 unsupported types allowed as text | killed (3 tests) |
| M7 `text/html` no longer classed HTML | killed (3 tests) |
No real survivors.

## 4. G4b independent verification
Source: gh logs of bench-gate jobs, run 35641694726 (head b182c4d, the run BENCHMARK s17 cites) and the cd99c95 run 35651351728; also run 35643951687 (head 2efc286) exists and is green. Parsed the JSONL records.
- Every per-scenario median and both ratios in BENCHMARK s17 match run 35641694726 for all four cells (spot-checked all four cells: gating peak 6.06 / 4.46 / 5.41 / 4.38; chunked-in-cap ratios 1.025 / 1.006 / 1.000 / 1.048; beyond-cap 0.986 / 1.000 / 0.981 / 0.987; idle 4.62 / 2.36 / 3.91 / 2.59). Every scenario 10/10 valid, verdict PASS (RECORDED for g6/hostile), summaries `missed/invalid/incomplete` empty, gating true, binary_kind bench for peak and shipped for idle, idle at most 10 (max 4.62).
- Final head cd99c95 re-run (35651351728): also PASS in all four cells, 10/10 valid, ratios at most 1.048 (in-cap 0.999 to 1.021, beyond-cap 0.969 to 0.987), idle 2.37 to 4.74, gating peak 4.38 to 6.16. Slightly different from s17 (amd64 gnu gating peak 6.16 vs 6.06, idle 4.74 vs 4.62); s17 correctly cites the run and head it used, but the README/EPICS range "4.38 to 6.06" and "idle 2.36 to 4.62" is not the final-head range (non-blocking).
- No target changed (40 / 10 / 1.10). Windows are resolved from the binary's own reported length and the min_bytes check makes an early-stopping "read to end" scenario INVALID (selftest proves it).
- False-pass probes (selftest plus manual): wrong/mismatched build identity refused; fewer than 10 runs refused (`--runs 3` -> REFUSED); peak scenario on the shipped binary refused; bench without marker / shipped with marker refused; QEMU binfmt (x86_64 and aarch64) refused, unreadable binfmt fails closed under --gate; stand-in scripts refused; partial set (G4b scenario missing idle or its 5 MiB reference) refused/INCOMPLETE; a G4b scenario run under `--gate` on the shipped binary was REFUSED. Fixture-hash mismatch refused. The gate cannot false-pass on what I could construct.
- Statement of the memory gate: BENCHMARK s17, README, EPICS and plan Rev 14 all say "By the plan this closes the memory gate (G4a plus G4b, all four cells); Sprint 6 may start", and exclude E-7-dependent A-4 checks, image, macOS reader, branch protection. Matches the plan definition (Rev 13: G4b closes the memory gate, E-7-dependent checks excepted). No document claims A-4 Done or E-7 done (README, EPICS, BENCHMARK, plan Rev 13/14, dev report all say NOT Done / not in PR).

## 5. arm-bench.yml change
Diff is 14 lines: the two `bench (gnu|musl)` spike steps now run idle only (peak measurement and its exit-code term removed) and the two summary printers use `.get()` for metric/valid_runs/runs/verdict. No job, action pin, permission or trigger changed; actionlint clean; run 35651351874 is green. This is what the owner authorised ("Drop spike peak, keep idle"). Side effect worth knowing: the spike's peak is no longer measured on that advisory job (evidence remains in s11/s15).

## 6. Judgement of the five recorded deviations
1. Total-length footer only on continuation or beyond-end (vs ADR-006 item 4): ACCEPTABLE. No AC requires it on a first page that fits; the beyond-end AC (states total length) is met; OQ-5 ("no label, returned as fetched") supports it. ADR-006 item 4 permits "stated only when known" but does not require it on every last page. Recommend an ADR-006 dated amendment (owner acknowledgement already requested).
2. G5 via the 50 MiB chunked variant: ACCEPTABLE. The 5 MiB fixture is under the 5 MiB cap and cannot yield `too_large`; the AC intent (window beyond the cap gives `too_large`) is satisfied and verified over stdio (6 MiB chunked, start near the end).
3. G1 duplicates g4a-5mib-full: ACCEPTABLE (cosmetic; separate name lets G4b run alone; selftest covers it).
4. text/* extras (javascript, ecmascript, x-ndjson, +json, +xml): ACCEPTABLE. Superset of the AC list, none image or binary; +json/+xml follow from "JSON/XML". Owner may trim.
5. G4a scenarios redefined as window-at-end: ACCEPTABLE. Necessary since early stop makes a start-0 read stop early; window-at-end still consumes the whole body and the harness min_bytes check enforces it. G4a comparability with Sprint 4 numbers is preserved in kind (values are within about 0.1 MiB).
None is an AC miss.

## 7. AC map
A-5: sequential paging (verified), clamp stated (verified), next start_index (verified), beyond-end message (verified), char boundary (verified), G4b window scenarios run in CI (verified) -> DONE. A-6: raw, text/JSON/XML as text, PNG/PDF isError naming type, missing Content-Type sniffing, raw scenario G3, idle re-check, combined G4b record -> DONE. Sprint 5 exit: A-5/A-6 AC pass, G4b run and recorded, idle re-checked -> met (E-7 not part of this PR; A-4 not Done).

## BLOCKING
None.

## NON_BLOCKING
1. Parameter deserialisation errors read `failed to deserialize parameters: ...` while max_length 0 uses `error[invalid_argument]: ...`; ADR-006 amendment wants the uniform prefix (fold into A-7, Sprint 6).
2. README/EPICS state G4b ranges from run 35641694726 (b182c4d); the final head cd99c95 re-run is also PASS but ranges are 4.38 to 6.16 (peak) and 2.37 to 4.74 (idle); consider citing both or refreshing.
3. Pagination and content-type stdio tests need `--features bench-loopback`; a plain `cargo test` does not exercise them (CI does).
4. ADR-006 item 4 deviation should be recorded as a dated ADR amendment, not only in EPICS.
5. Non-UTF-8 charsets decode with U+FFFD until A-8 (Sprint 11); untyped binary bodies are treated as text per AC.
6. arm-bench spike peak is no longer measured (authorised).
7. Mutants M4 and M5 in window.rs are equivalent (no test gap).
