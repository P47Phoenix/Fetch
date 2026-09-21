# PR #11 code + security review — sprint-7/redirects-encodings-report

Reviewer: independent code review (read-only). Base `f4f2c1b` (main) → head `ba8a3c9`.
Date: 2026-09-21.

## Scope reviewed

| File | Change |
|---|---|
| `src/fetch/tests.rs` | +169 / −2 — B-3 and B-2 tests, `get()` made generic over `Resolver` |
| `bench/smoke.py` | new, 121 lines — E-5 live smoke runner |
| `bench/e5_report.py` | new, 117 lines — E-5 report and release decision |
| `bench/selftest.py` | +51 — self-tests for both new scripts |
| `.delivery/artifacts/05-plan/po/sprint-plan.md` | +6 — Sprint 6 record, Sprint 7 start |
| `.delivery/artifacts/06-dev/sprint-7/dev-report.md` | new — dev report |

**No production source change.** `git diff f4f2c1b..ba8a3c9 -- src/` touches only `src/fetch/tests.rs`;
`src/ssrf/**`, `src/fetch/mod.rs`, `src/policy.rs` and `src/server.rs` are byte-identical to main.
No `.github/workflows/**` change. This matches the dev report's claim that B-3/B-2 are delivered as test
depth over existing behaviour.

## Verification performed

- `cargo test --lib` — **177 passed, 0 failed, 1 ignored** (3.30 s), including all four new tests.
- `cargo fmt --check` — clean. `cargo clippy --locked --all-targets -- -D warnings` — clean.
- `python3 bench/selftest.py` — **SELFTEST PASSED** (includes the 11 new E-5 checks).
- Targeted adversarial probes of `smoke.py` list validation and `e5_report.py` decision branches
  (results quoted inline below).

---

## Are the new tests genuine, or vacuous?

Assessed specifically for vacuous-pass, `FakeResolver` hiding real behaviour, and ARM timing flakiness.
**They hold up.** Detail, because this is the load-bearing question for B-3/B-2:

**`b3_a_chain_of_five_redirects_succeeds_and_a_chain_of_six_fails`** (tests.rs:496–512) asserts
`(f.redirects, text) == (5, "ok")` *and* `ok.accepted() == 6`, then `Err(TooManyRedirects)`, empty body and
`bad.accepted() == 6`. Moving `MAX_REDIRECTS` in either direction, or following the 6th hop, fails a
connection-count assertion, not just an error string. It also pins `assert_eq!(MAX_REDIRECTS, 5)`.

**`b3_redirect_to_every_encoded_blocked_form_is_refused_without_a_second_connection`** (tests.rs:514–535)
is the strongest of the four. The server runs under the `permit_loopback_for_tests()` policy, but every
redirect target is *non*-loopback (`167772161`, `0xa000001`, `012.0.0.1`, `10.1`, `::ffff:10.0.0.1`,
`fc00::1`, `0.0.0.0`, `::`). If the per-hop guard regressed, the client would attempt a real dial and
produce a transport error, so `code(&res) == "blocked_target"` fails; `srv.accepted() == 1` independently
catches a second dial. Not vacuous.

**`b2_encoded_and_ipv6_forms_are_refused_as_the_request_url`** (tests.rs:559–591) is well constructed: the
fixture server listens on `127.0.0.1:port` and *every* spelling is aimed at that exact port
(`http://2130706433:{port}/`, `http://0x7f000001:{port}/`, …). A guard regression that let any loopback
spelling through would connect to the live fixture, so `srv.accepted() == 0` is a real assertion rather
than a tautology. `r.count() == 0` on the `FakeResolver` proves no lookup was issued. `FakeResolver` is not
hiding anything here — the refusals happen before DNS by construction, which is the property under test.

**`b3_a_hung_resolver_is_bounded_by_the_deadline_on_every_hop_and_frees_the_slot`** (tests.rs:611–643)
uses a real `std::future::pending` resolver, a **400 ms** fetch deadline and a **5 s** assertion — 12×
headroom, on top of the 60 s test-level guard inside `get()` (tests.rs:130–140). Not flaky on a slow ARM
runner. The concurrency-slot check (concurrency 1, three sequential retries, `srv.accepted() == 3`) is a
real leak detector.

Making `get()` generic over `R: Resolver` (tests.rs:124) is the minimal change needed to drive
`HangResolver` and is correct.

---

## Blocking

### B1 — `e5_report.py` prints `Decision: release` (exit 0) for a smoke run in which every URL failed, and `smoke.py` accepts loopback/private IP literals into the "real" list (confidence 93)

`bench/e5_report.py:54–60` (`smoke_result`) validates the *shape* of the smoke summary — `verdict == "DONE"`,
not `dry_run`, `list_count == 10` — and then returns `"done"` unconditionally. `main` (line 82) maps
`"done"` to `"met"`. `summ["ok"]` is only interpolated into prose; it never influences the decision.

`bench/smoke.py:40–56` (`validate`) checks scheme, userinfo, duplicates, count and category labels, but
never that a host is a public name. A list of ten `https://` IP-literal URLs passes.

Chained, verified locally:

```
list: 3x https://127.0.0.1/pN tls, 3x https://10.0.0.5/pN redirect,
      2x https://[::1]/pN json, https://169.254.169.254/latest text, https://example.org/ html
smoke.py  -> rc=0, verdict DONE, list_count 10, ok 0, failed 10
e5_report -> rc=0, "**Decision: release**"
```

Same result with any 10-URL list where all ten fetches fail for any reason (network down, wrong binary,
expired TLS). The report section is literally headed *"10-URL live smoke (Goal 2 evidence; network, TLS,
redirects, JSON, plain text)"*, and the dev report names "Goals 2 and 3 have no live evidence" as the
reason E-5 stays open — yet the harness will emit `release` on evidence that establishes the opposite.
"Non-gating" should mean an individual URL failure does not block, not that zero successes is a pass.

Suggested fix, both halves:
- `smoke_result`: require at least one success in each of `tls`, `redirect`, `json`, `text` (the four
  `REQUIRED` categories the list validator already enforces), and require `redirected >= 1`. Anything less
  returns `"missing"` → "not decided" (exit 2), which is the harness's existing fail-safe.
- `validate`: reject URLs whose host is an IP literal (`ipaddress.ip_address()` parses `u.hostname`, and
  `hostname.startswith("[")` / `":" in hostname` for v6) with "smoke URLs must be public names, not IP
  literals".

### B2 — `smoke.py` list validation dies with an uncaught `ValueError` on a malformed URL: exit 1, no summary record, contradicting its documented contract (confidence 90)

`bench/smoke.py:46`, `u = urllib.parse.urlparse(e["url"])` inside `validate`. `urlparse` raises
`ValueError: Invalid IPv6 URL` on an unbalanced bracket. Verified:

```
list line: http://[::1/ json
smoke.py --dry-run -> rc=1, stdout empty, traceback on stderr
```

The module docstring (line 14–15) promises "Exit 0 when the run completed …, 2 when the list is invalid or
the harness failed", and `main` already has an `OSError` path that emits a
`{"kind":"smoke-summary","verdict":"INVALID"}` record. An unbalanced bracket *is* an invalid list; it should
take that path. As written the operator gets a bare traceback, no JSONL record, no `--out` file, and an exit
code the documented contract does not define. This is the first thing the owner's real list will hit on a
typo. Fail-closed (exit 1 ≠ 0), so not a security hole — but a contract violation in the one function whose
job is to reject bad lists cleanly.

Fix: wrap the `urlparse` call in `try/except ValueError` and append
`f"line {e['line']}: not a parseable URL ({exc})"` to `bad`.

---

## Non-blocking

### N1 — the `a3b-merge-gate` CI job does not cover the new `b2_`/`b3_` tests (confidence 85)

`.github/workflows/ci.yml:98` filters on `a3b_merge_gate refusal_ dial_once differential b1_every_blocked_class`
and lines 99–105 grep a name allowlist to prove each test actually ran, on the **release** profile. Sprint 6
added `b1_every_blocked_class…` to both. Sprint 7's four SSRF tests match neither the filter nor the
allowlist, and this PR changes no workflow. They do run in the `test` job (`cargo test --locked`, dev
profile), so a regression is still caught — but B-3/B-2 get no release-profile gate and no
"it actually ran" assertion, inconsistent with the established precedent for the same class of story.
One-line fix: add `b2_ b3_` to the filter and the four names to the grep list.

### N2 — `e5_report.py` raises `KeyError` on a truncated `DONE` smoke summary, exiting 1 instead of 2 (confidence 88)

`bench/e5_report.py:60` (`summ['ok']`) and line 100 (`sm['by_category']`, `sm['redirected']`,
`sm["failed_urls"]`) index directly. A smoke JSONL whose summary line is present but incomplete —
`{"kind":"smoke-summary","verdict":"DONE","list_count":10}` — crashes: verified `rc=1`,
`KeyError: 'ok'`, no report written. Exit 1 is the script's own "do not release" code, so a crash is
indistinguishable from a legitimate gap decision in CI. Every *other* malformed-input path in this file
is deliberately fail-safe (`load` swallows `OSError` and `ValueError`, `overhead` returns `None`), so this
looks like an oversight rather than a choice. Use `.get()` with a sentinel and return `"missing"`.

### N3 — `--child-env` without `=` crashes (confidence 88)

`bench/smoke.py:103`, `dict(kv.split("=", 1) for kv in a.child_env)` → uncaught `ValueError`, exit 1
(verified). `measure.py`'s equivalent should be reused, or validate with an explicit message.

### N4 — `#` comment stripping silently truncates URL fragments (confidence 82)

`bench/smoke.py:30`, `line = raw.split("#", 1)[0].strip()`. `#` is legal in a URL, so
`https://example.org/doc#section json` is silently fetched as `https://example.org/doc`, and two entries
differing only by fragment collapse into a duplicate-looking pair. Low impact for a smoke list, but it
changes the URL under test without telling anyone. Strip only a `#` that begins the line or follows
whitespace.

### N5 — `b2_redirect_hop_refuses_every_loopback_spelling` tests a wrapper the redirect loop does not call (confidence 84)

`src/fetch/tests.rs:550` calls `crate::ssrf::resolver::revalidate_hop`. That function
(`src/ssrf/resolver.rs:95`) is a one-line wrapper around `validate_target(.., Origin::Redirect)` and has
**no production callers** — the real loop calls `validate_target` directly at `src/fetch/mod.rs:170`. The
behaviour is identical today, so the test is not wrong, but its name promises redirect-hop wiring coverage
it does not provide: gutting the loop's per-hop revalidation would leave this test green. Genuinely awkward
to test end-to-end (the fixture server is on loopback, so a default-policy client cannot reach hop 1), and
`b3_redirect_to_every_encoded_blocked_form…` does cover the real loop for non-loopback forms. Worth either
renaming to say it is a unit test of `validate_target(Origin::Redirect)`, or deleting the unused wrapper and
calling `validate_target` directly.

### N6 — accepting `invalid_argument` weakens two rows of the B-2 table (confidence 80)

`src/fetch/tests.rs:585`, `matches!(e.code(), "blocked_target" | "invalid_argument")`. For the twelve
loopback spellings this is harmless — `srv.accepted() == 0` carries the proof, because those URLs target the
live fixture's port. But `[fc00::1]`, `[fd00::1]` and `[::]` can never reach the fixture, so for those rows
the error code is the *only* assertion, and `invalid_argument` would satisfy it even if the ULA range check
were removed and the URL merely failed to parse. Consider asserting `"blocked_target"` exactly for the rows
where the spelling is known to parse.

### N7 — smaller notes

- **Known, documented, unfixed:** the dev report records that a real `getaddrinfo` hung in the tokio
  blocking pool cannot be cancelled when the future is dropped, so a persistently hung resolver leaks one
  blocking thread per lookup until the OS resolver gives up. Accepted with owner/architect escalation and
  bounded in practice (pool default 512 vs 3 concurrent fetches × 6 lookups). Should be a tracked issue
  rather than a dev-report paragraph, but it is not a PR blocker.
- `smoke.py` category labels are never verified against behaviour: a URL tagged `tls` is not checked to have
  used TLS, and a URL tagged `redirect` is not checked to have redirected (`rec["redirected"]` is recorded
  but `by_category` ignores it). The E-5 AC's "covers redirects" rests on an operator-supplied label.
- `smoke.py:76` infers a redirect from `text.startswith("URL: ")`, matching `src/server.rs:157`. Correct,
  but a plain-text page whose first line begins `URL: ` is misreported, and `bench/standin_mcp.py` never
  emits that header, so the selftest exercises the false branch only.
- `smoke.py:106` `ok = len(text.strip()) > 0` counts an HTML error page as a success.
- `chain_server`'s `.unwrap_or(0)` (tests.rs:504) silently restarts the chain on an unparseable path rather
  than failing loudly.
- `Box::leak` per loop iteration (tests.rs:525–526) leaks nine small allocations; test-only and idiomatic
  enough for a `&'static` header fixture.
- Markdown/shell injection into the report was checked: `failed_urls` and `--binary-path` are interpolated
  into `e5_report.py:100` and `:102`, but both are operator-supplied, URLs cannot contain whitespace after
  `split()`, and the `claude mcp add` line sits in a fenced block and is never executed. No finding.
- `smoke.py` making live requests to list entries is acceptable: every fetch goes through the product's own
  SSRF-guarded client, so an attacker-controlled entry gains nothing the product does not already defend.

---

## Summary

The Rust side is the strong half of this PR: four well-built, non-vacuous tests, no production change, fmt
and clippy clean, full suite green. The E-5 harness is the weak half — the decision function's shape checks
are thorough for the cases the docstring enumerates (missing, advisory, dry-run, wrong size, all covered by
selftest) but stop short of checking whether the smoke actually *succeeded*, and the list validator has a
gap (IP literals) that makes the resulting false `release` reachable from a plausible list. Both blocking
items are small, local fixes in `bench/`.

Recommendation: **REQUEST_CHANGES** on B1 and B2, then re-review the two `bench/` diffs only.
