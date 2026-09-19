# Sprint 0 revision 3 report

Docs only, no code change.
- docs/BENCHMARK.md: ADVISORY_PASS exit 0 is not a result; per-scenario PASS vs summary; exit code table (sec 6); skeleton binary handshake fails (exit 2); E-8 stderr marker not implemented.
- docs/ci-branch-protection.md: neutral licence wording (Apache-2.0 LICENSE exists, OQ-7 open, publish = false). No other docs contradicted (grep).
- README.md: pre-MVP notice, pointers to both docs.
Verified: selftest PASSED, cargo fmt --check, release guard --self-test OK.
