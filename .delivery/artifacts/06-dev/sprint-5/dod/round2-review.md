# Technical Writer + light QA, round 2 (PR #9, head 01cffe1)

Decision: DONE (0 blocking, 5 non-blocking).

## 1. Round-1 blockers
1. README stale PR #8 / red arm-bench text: fixed (now PR #9, arm-bench described as idle-only, green). 
2. Plan Rev 14 and dev report CI section: fixed (red jobs shown as resolved by owner decision; final-head results recorded at cd99c95).
3. BENCHMARK read-first row and s5 table: fixed (PR #9, "stubs only" removed, G4b rows "implemented and run", Sprint 0 line points to s16/s17).
4. Run counts and ranges: fixed in substance (s17 now lists five bench runs, plan G4a lists three runs).
5. docs/ci-branch-protection.md: fixed (six checks, #9, idle-only note).

Numbers re-checked against CI logs (runs 35639776795, 35641694726, 35643750672, 35651351728), all match the BENCHMARK s17 ranges:
- gating peak MiB: amd64 gnu 6.07/6.06/5.90/6.16 (5.90-6.16), amd64 musl 4.65/4.46/4.52/4.46 (4.46-4.65), arm64 gnu 5.44/5.41/5.44/5.45 (5.41-5.45), arm64 musl 4.38/4.38/4.31/4.38 (4.31-4.38). README overall 4.31-6.16 correct.
- idle MiB: amd64 gnu 4.45/4.62/4.50/4.74, amd64 musl 2.36/2.36/2.36/2.37, arm64 gnu 3.91 all, arm64 musl 2.59 all. README 2.36-4.74 correct.
- G4b chunked ratios: max 1.048, min 0.969 across the four runs; correct.
- "Memory gate closed" is stated as the plan defines it, with A-4 E-7 checks, container image, macOS reader and branch protection excluded; A-4 not Done; MiB used; no new overclaim found.

## 2. Code diff cd99c95..01cffe1
- src/server.rs: `clamped_to = p.max_length.filter(|m| *m > max_length).map(|_| max_length)`. Correct: note only when caller asked above the cap; default above a configured cap no longer produces a note. No regression seen. No test covers the new case (default > cap gives no note).
- bench/report.py: `.get()` with defaults; correct, tolerant of records without metric/runs/verdict. `target_kB` default 0 prints 0, acceptable.
- Local: cargo fmt --check OK; clippy -D warnings OK (default and bench-loopback); cargo test --locked 167+1+9 passed; with bench-loopback 168+1+15 passed; bench/selftest.py PASSED 5 of 6 attempts (see NB-1).

## 3. CI at 01cffe1
`gh pr checks 9`: all 14 green (a3b-merge-gate, fmt, clippy, test, deny, release-guard, bench-gate x4, bench-product, bench-product-peak, bench (gnu), bench (musl)); runs ci 35654660175, bench 35654660112, arm-bench 35654660106 (both arm-bench musl/gnu jobs pass).

## 4. Deferrals
Dev report "Fix pass" section records NB-4 (max_bytes ceiling on g4b-window-start), NB-6/7, NB-3 (binary-magic refusal, "A-6 follow-up", also stated in README), NB-5 (documented). Recorded with reasons, but not as a tracked follow-up list with an owner/story (NB-2).

## Non-blocking
1. bench/selftest.py failed once ("target missed -> exit 1 with FAIL verdict") on the first local run, then passed 5 times; a possible timing flake under load. CI is green.
2. Deferrals are in prose under "Skipped"; add them to EPICS/backlog as follow-ups.
3. README and BENCHMARK s17 call 35641694726 the "first run"; 35639776795 (head e828267, 18:39) ran earlier. Also s17 has an unbalanced parenthesis ("(PR #9 at head b182c4d (GitHub checks out ..."). 
4. Ranges and README "green at cd99c95" do not include the 01cffe1 runs (arm64 gnu peak 5.39 and beyond-cap ratio 0.955 fall just outside the stated ranges; idle and other cells inside). State 01cffe1 or say the ranges cover the listed runs only (the text does say so).
5. No unit test for the new clamp-note behaviour (default max_length above a configured cap).
