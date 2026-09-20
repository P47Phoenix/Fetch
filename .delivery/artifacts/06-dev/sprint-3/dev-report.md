# Sprint 3 dev report (E-2, E-3, A-9)

Branch `sprint-3/harness-idle-rss`, PR #7 (draft). Order done: A-9, E-2, E-3. A-9 was not dropped; E-2 did not overrun.

## Hosted arm64 runner availability (plan requirement, recorded at sprint start)
Public repo P47Phoenix/Fetch; `ubuntu-24.04-arm` jobs (arm-bench.yml) ran and passed on this and earlier PRs, so the standard hosted arm64 runner is available and free for this public repo today. Billing and quota pages were not inspected (no such API access here), so "free for public repos" is the plan's stated assumption, confirmed only by jobs running. Recheck at Sprint 4 start.

## A-9 (final URL and status header)
`Fetched` now carries `final_url`; `server::with_header` prepends `URL: <final>\nStatus: <code>\n\n` only when a redirect was followed; nothing otherwise; the header sits outside the `max_length` window. Tests: unit (`header_is_added_only_after_a_redirect`), `final_url` asserted in the redirect test, and an end-to-end stdio test on the bench build. Header format is my choice (the AC fixes only that the text begins with URL and status).

## E-2 (harness)
Mostly present from E-1 and A-3b; added this sprint: native amd64 gate mode (`native_host()`: aarch64 or x86_64, refuses if a QEMU handler is registered for the host arch; exit 3 only off a native gate host; same median-of-10, INVALID/INCOMPLETE exit 2, ADVISORY never a result; identity check already covered both ELF machines), `late_landmark` fixture (committed sha256), `redirect-chain5` scenario (5 hops each with a body, recorded, outside the gating peak, invalid if the chain was not followed), `idle-bench` scenario (E-8 idle delta), runner name/image in the host record, `bench/report.py`, `scripts/build-candidates.sh` (cargo-zigbuild 0.23.4, ziglang 0.16.0 pinned; gnu 2.17 + musl; shipped + bench; native arch), workflow `.github/workflows/bench.yml` (2 OS x 2 libc matrix, nightly/dispatch/main push/same-repo PR, read-only token, no pull_request_target, fork PRs skipped). Self-test extended (native_host cases; in-process gate PASS / FAIL / INVALID paths with patched host and identity; redirect chain validity). No new Cargo dependencies; the cross tools are CI tooling, not crates.

## E-3 (idle gate)
The `bench-gate` job runs `measure.py --gate` idle on the shipped binary, strict 10 MiB, and fails the job on exit 1/2/3. Numbers below.

## Measured (CI run 1 35538774565 at d635a67 shown; run 2 35539714646 at 39379c1 in gap 9; `--gate`, native, 10/10 valid, summary PASS; MiB median)
| cell | idle shipped | idle bench | gating peak (read-in-full G4a set) | redirect-chain5 |
|---|---|---|---|---|
| amd64 gnu | 4.06 | 4.06 | 5.27 | 4.96 |
| amd64 musl | 2.27 | 2.27 | 4.37 | 6.97 |
| arm64 gnu | 3.55 | 3.54 | 4.66 | 4.32 |
| arm64 musl | 2.16 | 2.16 | 4.14 | 6.44 |
Idle target 10 MiB, peak target 40 MiB: both met with wide margin in all four cells; no target loosened. Boundedness ratios <= 1.03 (bound 1.10). Idle delta bench vs shipped within 0.01 MiB (bound 0.5). Timings recorded in job artifacts, not gated.

## Honest gaps
1. The peak run happens before A-4 (no conversion); it is the read-in-full form of G4a and NOT a G4a pass. G4b scenarios and g6-concurrent10 are still unimplemented (A-5/A-6/E-4).
2. Harness runs bare binaries, not the container image (AC wants the image; D-2 builds it). The tag trigger with digest gate is D-2.
3. macOS `/usr/bin/time -l` reader not implemented.
4. E-8 items: bench marker in the handshake not checked by E-2 (marker is checked via `--version`); binary size delta recorded in job summaries only, no explanation (E-4); public-host 5 MiB cross-check on the shipped binary still open (E-4); same-commit/Cargo.lock hash in `--version` is E-4's item.
5. Load-average repeat rule and dry-run discard still not implemented (loadavg is recorded).
6. arm64 CPU model was NOT captured in either CI run (the `cpu:` line was empty in all arm64 logs). The earlier claim here that the workflow "now also records CPU part" was false (the commit only added a trailing space). Corrected in fix-pass 1 (`lscpu` Model name plus CPU implementer/part fallback); see fix-pass-1-report.md.
7. `bench-gate` is not a required check; owner decides after a few runs (about 16 min per cell).
8. Header format for A-9 is a design choice.
9. Two CI runs exist (run 1 35538774565 measured d635a67, amd64 hosts Xeon 8573C and 8370C; run 2 35539714646 measured the head 39379c1, amd64 hosts AMD EPYC 7763 and Xeon 6973P-C). They agree within about 0.15 MiB per cell; variance across more runs is still unknown. Run 2: amd64 gnu 3.92/5.17, amd64 musl 2.27/4.25, arm64 gnu 3.55/4.65, arm64 musl 2.16/4.20 (idle/peak MiB), all PASS.
