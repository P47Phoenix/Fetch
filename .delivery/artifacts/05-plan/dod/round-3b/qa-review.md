# QA review, Stage 5 plan, round 3b (final re-check)

STATUS: DONE (0 blocking, 5 non-blocking)

## Gate criteria check
- Memory gate on shipped profile: PASS. Profile pinned in D-7 (S0); G0, G4a, G4b, E-5 measure it; D-1 (S8) re-measures gnu+musl on aarch64 and blocks MVP tagging (D-1 AC, S8 exit, S9 entry).
- Bench-vs-shipped tolerance: PASS. Idle delta <= 0.5 MB; public-host 5 MB peak within 10% of bench and <= 40 MB; size delta recorded; same commit and Cargo.lock hash; failure fails G4a.
- Loopback without weakening shipped default: PASS. Compile-time bench-loopback (127.0.0.0/8, ::1 only), guard with self-test, `-p` release build, artifact marker grep in D-2.
- DoD achievable per sprint: PASS. A-3b scoped to non-gating smoke; G4a lists only A-3b/A-4 scenarios; window/raw scenarios moved to G4b (A-5/A-6). E-7 fallback defined.
- SSRF suite, snapshots, 95/50, G4a/G4b: PASS. A-3a S1 unit tests; A-3b S2 four refusals; B-1/B-5 S6/S8; E-7 S2 with sha256 manifest; 95/50 in A-4 S4; G4a owned by E-4, G4b by A-5/A-6; ARM runner dependency listed per sprint.
- A-3b merge gate: PASS. Non-defaulted Policy in constructor plus required check `a3b_merge_gate` and four-refusal tests.
- Thresholds: PASS. 10 MB idle / 40 MB peak, median of 10 (harness rejects fewer), one MB definition in E-1.
- QEMU never gating: PASS (plan, E-2 preflight, architecture).

## Non-blocking
1. Public-host cross-check is one fetch over TLS against a plain-HTTP bench run over loopback, on a variable host. The 10% bound may flake or hide TLS cost. Specify a fixed host/URL, repeat count (median), and record TLS separately.
2. Delta bounds (0.5 MB, 10%) are enforced at G4a only. G4b and the D-1 re-measure do not restate them, though D-1 may change the profile. Add "delta bounds re-checked" to D-1.
3. Plan top flag 5 and S8 exit say D-1 re-measures peak "on the shipped build"; D-1 AC says peak on the matching bench build. Align wording.
4. `a3b_merge_gate` is a test inside `cargo test`; D-7 branch-protection list names fmt/clippy/test/deny/release-guard only. State whether it is its own required check. Condition (1) (A-3a on branch) is reviewer-checked, not machine-checked.
5. Architecture doc (sec 9 R12, sec 11 item 7 line ~280, sec 11.2 G4) still says test-support is the only loopback route and a single G4. The plan lists this as a required change but names no owner or date. Apply before S2 (E-8) starts.
