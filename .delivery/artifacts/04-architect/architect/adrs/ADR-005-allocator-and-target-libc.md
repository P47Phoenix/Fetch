# ADR-005: Allocator and target libc (glibc vs musl) on aarch64

Status: Proposed; AMENDED by ADR-007 (2026-09-19): measurement moves to GitHub-hosted arm64 runners and the release artifact is a GHCR container image, not standalone binaries. Allocator part: Accepted on x86 evidence; libc choice still pending ARM data, now measured inside the image.
Date: 2026-09-19

## Context
Spike (x86_64, glibc): system allocator idle 3.7-5.2 MB. mimalloc raised idle to ~11.2-11.5 MB (over the 10 MB target) and peak by 6-17 MB (reqwest+htmd: 56.4 -> 63.7 MB; ureq+htmd 56.3 -> 73.2 MB). Cross-builds proven with cargo-zigbuild: `aarch64-unknown-linux-gnu.2.17` (2,957,304 B, dynamic, glibc 2.17 floor) and `aarch64-unknown-linux-musl` (2,904,584 B, static). musl binary ran under qemu-user (handshake + fetch OK); gnu could not run there (no loader). qemu RSS is invalid (13.0/19.5 MB vs 4.7/6.1 MB native x86). No native aarch64 RSS exists. Known risks: musl's malloc is slower and fragments more; ARM kernels may use 16K/64K pages; macOS uses libmalloc (separate).

## Options
1. System allocator, glibc build (`gnu.2.17`), dynamic.
2. System allocator, musl static build.
3. Alternative allocator (mimalloc, jemalloc) on either libc.
4. Ship both 1 and 2 and let the user choose.

## Decision
- Allocator: system allocator. mimalloc/jemalloc not used (mimalloc regressed idle above target on x86). Re-evaluated only under the criteria below.
- Libc: primary release artifact = `aarch64-unknown-linux-gnu.2.17` (dynamic; covers any glibc >= 2.17 ARM Linux). Build `aarch64-unknown-linux-musl` in CI as a candidate from Sprint 1 so ARM data can be gathered early. Final "which is primary, is the other shipped" decision is deferred until the native-runner measurements below exist. macOS arm64 uses the platform allocator (no choice). [SUPERSEDED by ADR-007: there is no standalone gnu/musl/macOS binary release. The image ships exactly one flavour chosen by the criteria below; both candidate images are built and measured until then. The glibc-2.17 floor no longer applies to users, because the host libc is irrelevant inside an image.]
- Release profile: `opt-level`, `lto`, `panic=abort`, `strip`, `codegen-units` are pinned early in D-7 (Sprint 0) so every gate measures what ships; D-1 (Sprint 8) finalises it (including `opt-level` 3 vs `s`) and MUST re-measure idle and peak on the shipped build on aarch64 for both gnu and musl within 10 MB and 40 MB, and a miss blocks MVP tagging. Candidate tuning to test on the native runner, not assumed: glibc `mallopt(M_ARENA_MAX=1)` and `M_TRIM_THRESHOLD`/`M_MMAP_THRESHOLD` (few threads: `current_thread` runtime plus tokio's blocking thread for stdin), and a release `opt-level` 3 vs `s`.

Decision criteria (evaluated on the GitHub-hosted arm64 runner (ADR-007), measured on the process inside the container image, each figure median of 10 fresh processes, both binaries, same fixtures, page size recorded, on the release profile pinned in D-7; idle on the shipped binary, peak on the `bench-loopback` build; the criteria feed gates G4a (end Sprint 4) and G4b (end Sprint 5), architecture.md 11.0):
1. Hard gates for a build to be eligible: idle VmRSS <= 10 MB; 5 MB-page VmHWM <= 40 MB; 50 MB run within 10% of the 5 MB run; 10-concurrent run recorded.
2. Preference when both pass the gates: choose musl (static, no glibc floor, strongest FR-15) if `peak_musl <= 1.10 x peak_gnu` AND `idle_musl <= idle_gnu + 1 MB` AND 1 MB-page conversion overhead p95 <= 500 ms (NFR-02) AND real HTTPS fetch and name resolution (including home-lab `.local`/split-DNS names, see ADR-003) work. Otherwise choose gnu.
3. If only one build passes gates, that build is primary; the other is dropped or documented unsupported.
4. Headroom rule: a passing build with < 20% headroom on idle or peak (idle > 8 MB or peak > 32 MB) triggers an allocator/tuning experiment before release.
5. If both libcs fail gates: try mimalloc and jemalloc on the failing libc; adopt one only if it passes all gates AND does not increase idle by > 0.5 MB vs the system allocator on that libc.

## Consequences
+ Small, predictable decision surface; system allocator keeps idle low and code simple.
+ Both binaries exist from early on, so risk R5/R3 gets data before the M2 gate.
- Two build variants to test until decided (CI cost).
- If musl is chosen: musl DNS stub differences, slower malloc; if gnu is chosen: needs glibc >= 2.17 on target (true on essentially all ARM Linux distributions in use).
- Page-size sensitivity is not controlled by us; documented in benchmark report.

## What ARM data would flip it
- musl becomes primary if it meets criterion 2 (near-parity peak, idle, latency) and resolves names/HTTPS correctly.
- gnu stays primary (and musl is dropped) if musl peak > 1.10 x gnu, or fragmentation appears in the 50 MB or 10-concurrent runs, or 1 MB conversion p95 > 500 ms, or `.local` resolution fails.
- A non-system allocator is adopted only under criterion 5; x86 evidence says it is unlikely.
- If glibc idle on 64K-page kernels alone exceeds 10 MB while musl passes, musl (or smaller initial buffers) wins regardless of the preference order.
