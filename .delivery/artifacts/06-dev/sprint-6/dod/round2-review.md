# PR #10 round-2 validation (head 0b4f1e0, previous ec9dab0)

Decision: NOT_DONE (2 small doc blockers; code, tests and CI are clean).

## 1. Round-1 blockers
- 408/425/429 wording: FIXED. Over real stdio (bench-loopback builds of b633e60 and 0b4f1e0): 408, 425, 429 -> "...; it may work if retried after a delay"; 404 -> "unlikely to help"; 500 -> "may work if retried later".
- Log line: FIXED in code (`error internal {detail}`, matches lowercase "warn" style). Not observable end to end (no input triggers Internal); disclosed in EPICS.
- Schema: tools/list from main b633e60 and head 0b4f1e0 is byte-identical (cmp, whole tools array). The "advertised schema unchanged" claim is now true; `default: null` removed via drop_default, pinned by a stdio test.

## 2. Diff ec9dab0..0b4f1e0
Correct, no regressions. error.rs 408|425|429 arm precedes 400..=499. Test timeout split (short client only for the timeout case) is sound. b1 test additions (PUBLIC const, literal-message check) fine. ci.yml: only the cargo test filter and one required-name were added; the job set is unchanged and nothing removed. Verified locally on release: filter matches the b1 test and the regex `^test .*b1_... \.\.\. ok$` matches its output line. Gate strengthened, not weakened.

## 3. Gates (local + CI)
fmt ok; clippy -D warnings x3 clean; cargo test --locked x3 pass (172/173/172 lib, stdio 10/16/10); cargo deny ok; check-release-features.sh and --self-test ok; bench/selftest.py PASSED. `gh pr checks 10`: 14/14 pass on 0b4f1e0.

## 4. Docs
Accurate and disclosed in EPICS A-7: panic=abort deviation, Internal unit-tested only, validation moved into FetchParams::parse (fields now serde_json::Value), non-object arguments keep rmcp -32601.
Blocking (stale, now false):
- README.md line 12 ends "Not yet done: cause-specific errors (A-7)". A-7 is implemented.
- README.md line 119 says parameter-type errors use the library's own prefix and uniformity is planned under A-7. Now false for JSON-object arguments (README line 83 says the opposite).
Non-blocking:
- No sprint-6 stage summary exists yet (only a sprint-5 note edit); expected at sprint close.
- README line 83 could mention the 408/425/429 "retried after a delay" wording.
