# A-3b DoD round 3 validation (docs + evidence), head ad77e57

Decision: DONE

- B1 closed: docs/BENCHMARK.md section 14 exists (native aarch64, advisory, single run, gating=false, 10/10 valid, median 4.77 MiB VmHWM). Samples match the run 35523134933 artifact results-peak.jsonl exactly (median of 4892,4752,...,4636 = 4886 kB = 4.77 MiB).
- Run at ad77e57 (35524830107) log now prints the measure result: 10/10 valid, samples 4880..4896, median 4.78 MiB, summary gating=false ADVISORY_PASS. Consistent with section 14 within 0.01 MiB (different run; section 14 cites the earlier run id).
- Tech-writer blocking closed: README lines 19 and 93 point to BENCHMARK.md section 14 (plain text on line 19, no anchor link; fine). PR body has the aarch64 smoke paragraph (4.77 MiB, run id, section 14).
- ci-branch-protection.md line 7 lists bench, bench-product, bench-product-peak as advisory.
- architecture.md: OQ-5 RESOLVED no label (lines 28, 369, 403, 552), B-6 won't-do (464); ADR-006, PRD, EPICS consistent.
- gh pr checks 6: all 10 pass at ad77e57.
- git diff 5c9a0dd..ad77e57: only src change is one comment line in src/fetch/mod.rs.
- Clean git-archive: cargo fmt --check OK; cargo test --locked: 91 + 9 passed, 0 failed.

Non-blocking: architecture.md line 4 header still says "No open question is decided" (stale Rev 2 text; contradicted by line 28/531). Section 14 cites run 35523134933 (5c9a0dd) not the head run; log at ad77e57 shows 4.78 MiB.
