# ADR-007 report: hosted arm64 runners and GHCR image release

## Summary
Recorded the two user decisions (2026-09-19): (1) the native aarch64 memory measurement (G0, G4a, G4b) runs on GitHub-hosted arm64 runners (`ubuntu-24.04-arm`), no self-hosted runner; (2) the release artifact is a tested container image on GHCR published from GitHub Actions only, standalone binaries dropped, tag and publish only after the ARM gates pass on the same digest. Targets unchanged (idle <= 10 MB VmRSS, peak <= 40 MB VmHWM, 5 MiB, median of 10 valid runs). Docs only; no code, scripts, .github, bench/*.py, Cargo files or docs/BENCHMARK.md touched. OQ-3/4/5/7 remain OPEN.

Libc and allocator recommendation: system allocator unchanged; publish exactly one libc flavour inside the image, chosen by the unchanged ADR-005 criteria measured inside the image. The gnu.2.17 glibc floor loses its purpose (host libc is irrelevant in an image), so the expected outcome is musl static on distroless/static if parity holds, else gnu on distroless/cc. No user decision needed unless the data is a tie or both fail.

## Edits per file
- `.delivery/artifacts/04-architect/architect/adrs/ADR-007-hosted-arm-runners-and-ghcr-image-release.md` (new): context, decision, alternatives (self-hosted Pi 5, both, binaries, image), consequences, image design, libc/allocator, in-container measurement notes, ADR-005 amendment, risks.
- `adrs/ADR-005-...md`: status amended by ADR-007; gnu.2.17-primary/standalone-binary text marked SUPERSEDED; runner wording updated.
- `architecture.md`: ADR table (005 amended, 007 added), decisions table, sec 9 profile/targets text, sec 9.2 rewritten (hosted-only trigger matrix, fork-PR policy, kept least-privilege token, no pull_request_target, SHA pins; removed ephemeral registration, LAN isolation, label gating; availability now hosted-runner/quota, QEMU never, only workflow_dispatch re-run as fallback), new 9.3 (image build: minimal non-root digest-pinned base, arm64 only, bench image never pushed, digest gate, attestations, GHCR visibility, in-container measurement, libc), release-integrity bullet, gate/preflight wording, required-doc-changes list.
- `docs/EPICS.md`: E-1, E-2 (hosted workflow against the image), D-1, E-6, D-7 lines; Epic D goal/metric/scope; D-2 rewritten (image build, digest gate, publish, OQ-7 ordering); D-3; D-4; MVP wording; Open Items OQ-7/OQ-9; header revision note.
- `.delivery/artifacts/05-plan/po/sprint-plan.md`: flag 3 rewritten, new flag 6, Sprint 0 dependencies/exit and G0 (closable by hosted-ARM A-1 run, ARM-runner-access dependency removed), Sprints 1-5, 9, 11, 12 dependencies, D-2 exit, risks table (runner risk replaced; new GHCR/OQ-7 risk), decisions log, new Revision 7 log. Decisions and older revision logs kept.
- `docs/PRD.md`: OQ-9 text, new release-artifact decision line, US-8 (container image), measurement protocol, risk 8, OQ-7 row (must precede first publish), OQ-9 row.

Points: unchanged (97 total, MVP 73). D-2 kept at 5 pts and flagged as an unvalidated estimate (macOS/checksum/codesign/x86 work removed; image, digest gate and GHCR permission work added). D-3 probably shrinks because the developer is adding an ARM CI job now; not re-estimated, flagged for Sprint 11 planning. E-7/E-8 needed no runner edits. OQ-7 ordering: still due before Sprint 9 and now also before the first image publish; still correct, since candidates can stay private and untagged.

## What docs/BENCHMARK.md must change (follow-up pass)
- Host: replace author's native aarch64 runner/cluster with GitHub-hosted `ubuntu-24.04-arm` cloud VM; record OS, kernel, CPU model, RAM, `getconf PAGESIZE`, THP, container runtime and version, runner image version.
- Subject: measure the process inside the container image (PID 1 of `docker run -i --rm`; host pid via `docker inspect`), by digest; report the image digest and bench-image build info; exclude runtime daemons; never use docker stats or cgroup counters for gating; add a `--memory=64m` confirmation run (recorded only).
- Preflight: add `uname -m` inside the container, forbid `setup-qemu-action`/binfmt in measurement jobs.
- Load-average rule: baseline measured in the same job on a non-dedicated VM; record CPU model per run; latency recorded, not gated.
- Remove wording about dedicated runner, governor control, self-hosted provisioning and the manual-run-on-same-runner fallback; only workflow_dispatch re-run counts.
- Add caveat: 4 KiB hosted pages vs Pi 5 16 KiB; same-page-size comparisons only.
- Libc columns: keep gnu/musl candidate images until ADR-005 decides.

## What docs/ci-branch-protection.md must change
- Remove self-hosted runner sections and rules (labels, ephemeral registration, LAN isolation, maintainer-only self-hosted dispatch).
- Required checks: add arm64 test job, image build/handshake/FR-15 job, memory-gate job (when they exist); publish job must `needs:` them; skipped-is-failure stays.
- Keep: default `contents: read`, per-job `packages: write`/`id-token`/`attestations` only on the tag publish path, no `pull_request_target`, SHA-pinned actions, no secrets for fork PRs, forks never push.
- Add: GHCR package settings (private until OQ-7, linked to repo, visibility change is manual), optional `release` environment with required reviewer.

## Risks for the user
1. GHCR packages are private by default when first pushed; making it public is manual and is the OQ-7 distribution act (verify whether repository-linked visibility inheritance applies).
2. Hosted VM CPU/host varies between runs; VmRSS/VmHWM are largely insensitive but latency is not; median of 10 is within one job on one VM.
3. Arm hosted CPU differs from Pi 5; page size expected 4 KiB vs Pi 5 default 16 KiB (RSS can be higher on a Pi). Targets left unchanged as instructed; flagged, not decided.
4. Runner background load makes the load-suspect rule noisier.
5. Free arm64 minutes on public repos are a GitHub policy, not a contract; availability blocks the release.
6. Users need a container runtime; `docker run -i` adds startup latency (the Sprint 1 250 ms readiness figure needs restating); macOS users run arm64 image in a VM.
7. Dropping macOS/standalone binaries means FR-15 and NFR-06/13/15 wording (PRD requirements table) still refers to binaries; I edited US-8 and the protocol/risk rows but did not rewrite FR-15, NFR-06, NFR-13, NFR-15. The PO should reword them.

## Open questions for the user
- OQ-3, OQ-4, OQ-5, OQ-7 remain open (OQ-7 must be answered before the first image is published).
- Whether to run the image once on the Pi 5 for information, and whether targets should later be restated per page size.
- Libc flavour is data-driven; user input only if the data is a tie or both fail.

## ADDENDUM: multi-arch decision (user, later the same day) - supersedes earlier "arm64 only" / "points unchanged" statements above
- Decisions folded in: image is a `linux/amd64` + `linux/arm64` manifest list built from per-platform images; both platforms are hard-gated on native hosted-runner measurements (`ubuntu-24.04`, `ubuntu-24.04-arm`); release blocked unless BOTH pass; the digest tested is the per-platform digest published and the manifest digest M that references them is the one tagged. Deferred (considered, not rejected): Windows containers, arm/v7, 386, riscv64, ppc64le, s390x. Unverified-platform policy recorded in ADR-007 decision 5 and architecture 9.3 (label "memory targets not verified on this platform"; QEMU never gates). The amd64 baseline is no longer the preliminary x86 spike figure.
- Re-estimate (honest, unvalidated): D-2 5 to 8 pts. Total 97 to 100, MVP 73 to 76. Sprint 9 = D-2 alone (8). D-4 moves to Sprint 10, which becomes 9 pts with C-2 (7 without): 1 over the 8-pt ceiling if OQ-4 is yes. MVP now completes at the end of Sprint 10 (was 9); v1.0 stays Sprint 12. E-2 (5), E-3, E-4, D-1 keep their points; E-2 has zero slack in Sprint 3 and A-9 moves to Sprint 5 first if needed. D-3 probably shrinks (not re-estimated). This PROPOSED rearrangement needs PO/user acceptance.
- Extra edits: ADR-007 (decisions 4-5, digest flow, libc one-flavour-for-both rule, risks 6-7), architecture 9 (targets, 9.2 matrix with amd64 column, 9.3 digest gate, gate wording), EPICS (E-1 per-platform host table, E-2 matrix, D-1, E-6, D-2 8 pts, D-3, D-4, scope, totals note), sprint plan (flag 6, Sprint 9/10 rows, decisions and revision logs), PRD (US-8, platforms, protocol).
- Units: the coordinator wrote MiB for the targets; docs keep "MB as defined once in E-1" (10^6 vs 2^20 still to be stated there); confirm the unit.
- Still not reworded: PRD FR-15, NFR-06, NFR-13, NFR-15 (binary/ARM-only wording), PO to do.
- BENCHMARK.md additional follow-up: a per-platform host-facts table (amd64 `ubuntu-24.04` and arm64 `ubuntu-24.04-arm`: OS, kernel, CPU model, RAM, page size, THP, runtime and version, runner image version); every result table keyed by platform and by per-platform image digest plus manifest digest; the preflight `uname -m` must equal the platform under test; the amd64 spike numbers are labelled preliminary; platforms are never averaged; release requires both platform reports valid.
- Additional risks: both gates must pass (double the exposure to runner outages and flakiness); amd64 hosted CPUs vary (Intel/AMD); macOS/Windows users run in a VM (claim is the process RSS on native Linux only); libc flavour must satisfy both platforms and if they disagree it is a user decision.
