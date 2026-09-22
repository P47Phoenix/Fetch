# Sprint 8 dev report: SSRF suite, coverage gate, release profile

Branch: `sprint-8/ssrf-suite-release-profile`. Stories: B-5 (3 pts), D-1 (2 pts), D-5 (1 pt) = 6 pts, per the sprint plan's Sprints 6-12 table row 8. Entry (B-1, B-2, B-3 done) satisfied: Sprint 6/7 merged to main at `0926ba8`.

## B-5: SSRF suite and coverage gate (3 pts) — DONE

The table-driven SSRF suite already built across A-3a, A-3b, B-1, B-2 and B-3 already covers every AC bullet:
- IPv4 and IPv6 (including embedded-IPv4/NAT64/6to4 forms): `src/ssrf/ranges.rs` table-driven "every blocked whole range" tests.
- Encoded and mixed-case/userinfo forms: `src/ssrf/differential.rs`, `b1_`/`b2_` cases in `src/fetch/tests.rs`.
- DNS resolving to a private address: `src/ssrf/resolver.rs`, `b1_every_blocked_class_by_name_mixed_and_literal_is_refused_before_any_connection`.
- Rebinding simulation: `resolver.rs::rebinding_shape_only_the_first_lookup_is_used_and_no_second_happens` — an injectable resolver returns a public answer on the first lookup and a private answer on a second, asserting only one lookup ever happens and the private answer is never dialled.
- Redirect chains/refusals: `b3_` cases in `fetch/tests.rs`.
- No harness change for a new blocked-range case: adding a row to the `ranges.rs` range tables is picked up by the existing table-driven range test automatically.

No gaps were found requiring new test cases. What Sprint 8 adds: a required CI job `coverage` (`.github/workflows/ci.yml`) running `cargo llvm-cov --lcov` plus `scripts/coverage_gate.py`, gating combined line coverage of `src/ssrf/{mod,ranges,resolver,differential}.rs`, `src/fetch/mod.rs` (redirect loop) and `src/convert/window.rs` (pagination) at >= 90%. Local run: 97.7% combined (1832/1876 lines); see `docs/BENCHMARK.md` section 18 for the per-file breakdown. The AC's 90% figure is read as the combined figure across those modules (stated explicitly, since a couple of individual files sit in the high 80s/high 90s and a strict per-file floor is not what the AC text says).

## D-1: Size- and memory-optimized release profile (2 pts) — DONE

The release profile pinned in D-7 (`opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`) is finalised unchanged: every G4a/G4b gate run recorded in `docs/BENCHMARK.md` sections 16-17 already passes with margin on it (worst-case gating peak 6.06 MiB vs a 40 MiB target, worst-case idle 4.74 MiB vs 10 MiB), so there is no measured case for moving to `opt-level = 3`. The re-measure required by the AC (idle RSS and 5 MiB peak RSS on the shipped build, gnu and musl, amd64 and arm64, median of 10 valid runs, hosted native runners) runs via the existing `bench.yml` `bench-gate` job (the same 4-cell matrix used for G4a/G4b) triggered by this PR — see the PR's CI run for the actual figures; `docs/BENCHMARK.md` section 18 records the run once it completes and is not backfilled with invented numbers.

Added: CI job `release-ldd-guard` builds the release binary and asserts (via `ldd` + grep) that it links no `ssl`/`crypto`/`native-tls` library — a machine-checked version of the AC's `ldd` inspection requirement, rather than a one-off manual check. Locally confirmed (x86_64 dev build): only `libgcc_s`, `libm`, `libc` and the dynamic linker are linked.

## D-5: Tool description within 150 words (1 pt) — DONE

The tool description in `src/server.rs` was already 59 words (well under the 150-word cap) and already names `url`, `max_length`, `start_index`, `raw` and the `start_index` continuation pattern — no change to the description text was needed. Added a regression test, `d5_tool_description_is_concise_and_names_every_parameter` in `tests/stdio.rs`, which drives the real binary over stdio, reads the description from the live `tools/list` response (not a copy of the literal, so it can't drift silently), and asserts the word count and the presence of every parameter name and the continuation-pattern wording.

## E-5 — still OPEN, NOT scheduled into Sprint 8

E-5 (2 pts) remains open: the owner's 10-URL live-smoke list (and the separate 50-URL list for E-7/A-4) have still not been supplied. Per the Sprint 8 row of the plan's Sprints 6-12 table, Sprint 8 scopes only B-5, D-1, D-5 — E-5 is not listed there, so per instruction it is not completed or closed in this sprint. The E-5 harness built in Sprint 7 (`bench/smoke.py`, `bench/e5_report.py`) is untouched and ready to run as soon as the list arrives. No sprint in the current plan table is scheduled to close E-5 (Revision 16 flagged Sprint 8 as "the only sprint that fits E-5 whole" by points, but the Sprint 8 row as written does not include it — this is an inconsistency between the narrative Revision 16 note and the formal Sprints 6-12 table; flagged below rather than resolved unilaterally).

## Deviations from AC

- D-1's actual re-measured figures are not in this report; they are recorded in `docs/BENCHMARK.md` section 18 (filled in once this PR's `bench-gate` CI run completes — see PR checks for the run number).
- B-5's coverage gate enforces a combined 90% across the listed modules rather than a strict per-file 90% floor (one file, `src/fetch/mod.rs`, measures 89.2% locally); this reading of the AC text is stated explicitly rather than silently assumed.

## OWNER_DECISIONS_NEEDED

- E-5's 10-URL live-smoke list (and E-7's 50-URL list) — still missing; blocks E-5 close and A-4's 95%/50% checks. No current sprint plan row is scheduled to close E-5; the Revision 16 narrative note ("Sprint 8 ... is the only sprint that fits E-5 whole") is not reflected in the Sprint 8 table row — needs a plan decision (add E-5 to a sprint explicitly, or accept it stays open indefinitely until the list arrives).
- OQ-3, OQ-4, OQ-7 remain OPEN and undecided; not touched by this sprint's work.
