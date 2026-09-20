# Docs pass 3 report (technical writer)

Scope: unit standardised to MiB; hosted per-platform benchmark docs; branch-protection end state; PRD wording per ADR-007. Verdict GO (native aarch64 spike evidence) recorded in BENCHMARK section 10.

## Changes
- MiB: `MB` replaced by `MiB` in README, BENCHMARK, ci-branch-protection, PRD, EPICS, architecture and ADR-001..007. Definitions rewritten (BENCHMARK glossary and section 1, architecture 5.1, EPICS E-1 AC, architecture doc-change list). One-line note kept: `/proc` kB = KiB, 10 MiB = 10,240 kB, 40 MiB = 40,960 kB. No numeric value changed. Left as decimal MB: architecture 5.1 rows i and k (0.4 MB, 1.2 MB of bytes) and ADR-004 line 40 (0.4 MB).
- BENCHMARK: per-platform host table (arm64 facts from the arm-bench report; amd64 NOT YET MEASURED), results keyed by platform and digest, in-container `/proc/<pid>/status` method (not docker stats), host and in-container `uname -m` preflight, dedicated-runner/governor/manual-fallback wording removed, runner PLACEHOLDER replaced, new section 11 (advisory hosted-ARM spike results, 4K vs 16K pages, 50 MiB boundedness not run), section 10 verdict GO. Glossary, exit codes and caveats kept.
- ci-branch-protection: no self-hosted; arm-bench job `bench` advisory, not required; planned two platform gate jobs marked NOT YET EXISTING; today's required set unchanged (fmt, clippy, test, deny, release-guard); publish job permissions, no fork/PR trigger; GHCR private by default, public = OQ-7 act.
- PRD: FR-15, NFR-06, NFR-13, NFR-15 reworded to image/platform; OQ-3/4/5/7 not decided.
- README status line updated.

## Verification
- `python3 bench/selftest.py` from clean `git archive HEAD` plus my docs: SELFTEST PASSED.
- Relative links and anchors in README, BENCHMARK, ci-branch-protection resolve.
- Job ids fmt, clippy, test, deny, release-guard match ci.yml; `bench` matches arm-bench.yml.
- Commands in BENCHMARK unchanged except prose; step 1 command run.

## Notes
- Code/doc conflicts: `bench/measure.py` `--gate` accepts only native aarch64 (exit 3 elsewhere), so an amd64 gate and in-container pid reading are not implemented; BENCHMARK says so.
- Script strings still saying MB: `bench/measure.py` line 12 docstring ("MB = MiB (2**20)"). bench/selftest.py line 91/94 ("50 MB" check labels).
- Residual "CPU governor recorded" wording remains in architecture 9.x/11.2 and EPICS E-2 (not edited, harmless).
- Working tree also shows modifications to .github/workflows/arm-bench.yml and bench/*.py that are not mine; not committed.
