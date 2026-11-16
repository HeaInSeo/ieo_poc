# Review of PR #77 at af68c93 (review 3)

Reviewer: independent reviewer (did not write the PR). Head `af68c930395cc080ccb4b9534a7d3f24656adfc0`; previous review 1548061 (changes requested). Correction 2 commits: 9f0cb93 (code, contracts, tests), ba63f70 (evidence), af68c93 (REPORT §12, archived review 2, ledger). This is the last correction allowed, so findings below are split into **must-fix** (none) and **carry-over**.

## Verdict

**Approve.** There are no High or Medium findings.

- **N-M1 is fixed.** I re-ran my result-lost probe from review 2 unchanged. The lost-result run is now charged: the next delivery gets `billable_room 20` and the chain shows total 20 and 1 run. While the run was unmeasured, the assignment stopped (409 `ASSIGNMENT_USAGE_UNMEASURED`). The new e2e step 8b covers the same case, plus a refused call whose result was lost and is then closed.
- **I-1, I-2 and I-3 from review 2 are fixed.**
- **Two Low carry-over items remain.**
  - The test-override environment variables are accepted in production without an upper cap, and nothing is logged when they take effect (C-1).
  - One matching arm is untested (C-2).

## What I ran

All runs were in a detached worktree at af68c93, used test DB 55432 only, and made no model calls. Logs are in the scratchpad (`/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`).

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --all --check` | OK | `r77c-fmt.log` |
| `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings` (after `cargo clean -p ieo-coord -p ieo-supervisor`) | OK | `r77c-clippy.log` |
| `cargo test -j 4 --workspace --locked` (`IEO_TEST_DB_URL` set, `RUST_TEST_THREADS=4`) | **380 passed, 0 failed, 0 ignored, 0 SKIP**. The new test is the unit test `delivery_bounds_keep_the_call_inside_the_in_flight_window` | `r77c-test.log` |
| `openapi_spec_validator` | OK; the 1548061→af68c93 changes are wording only (75 s, the "may have run" classes) | `r77c-openapi.log` |
| `validate_package.py` | `schema_errors: 0`, rc 0 | `r77c-validate.log` |
| `validate_contracts.py` | rc 0 | `r77c-contracts.log` |
| `render_progress.py --check` | `ok: true` | `r77c-render.log` |
| Relative links in the 11 changed `.md` files | 0 broken | (inline script) |
| `git diff --check 7dcb2fb..HEAD` | clean (the trailing spaces from review 2 are gone) | — |
| `git diff 9f0cb93..af68c93 -- crates contracts` | empty, so evidence ba63f70 (test log at 9f0cb93) covers the shipped code | — |
| Review-2 result-lost probe, re-run unchanged (temporary edit to `limits_e2e.rs`, since reverted) | fixed; see below | `r77c-probe-result-lost.log` |
| My hand mutants (one at a time, each reverted) | 1 killed, 1 survived | `r77c-mut-H*.log` |

### Probe: what review 2's N-M1 scenario does now

The test coordinator runs with in-flight 47 s and a 2 s start deadline.

1. The delivery of a1 gets 500 (57014, "this step's transaction was rolled back").
2. The ledger record for a1 is unmeasured.
3. While the delivery is younger than the in-flight bound, the chain shows 0 runs. Settlement leaves it alone, which is correct.
4. After the bound, the replay records `unknown` with no digest.
5. The next delivery is **409 `ASSIGNMENT_USAGE_UNMEASURED`**. Before this fix it was let through.
6. After a person reconciles a1 to measured (20 billable), the next delivery is accepted with **`billable_room 20`** (it was 40 before). The chain shows **total 20, runs 1**.

### Hand mutants

| id | Mutation | Target | Result |
| --- | --- | --- | --- |
| H1 | `limits.rs:118`: add `unknown` to `nothing_ran`, so a run that may have happened can be closed as "sent nothing" | `limits_e2e` (HTTP) | killed |
| H5 | `limits.rs:120`: match records only when `may_have_run`, dropping the `|| sent` arm | `delivery_e2e` (HTTP) and `limits_e2e` (HTTP) | **survived both** (C-2) |

The author's 28 mutants, all killed, include role matching (N1), the no-result age (N2), `unknown` (N3), closing (N4), the chain ledger (N5) and the deadline (C10).

Cleanup: I dropped the 2 `ieo_t_lim_http_*` databases left by my failing runs (identified by PID and time; `ieo_t_%` is 0 afterwards) and removed their `/tmp` directories. I did not touch 55433, 58090, PR #29 or other worktrees.

## Analysis

### The new matching (`limits.rs:96-160`)

- **Classes.** A delivery *may have run* in three cases:
  - its newest result is `accepted`;
  - its newest result is `unknown`;
  - it has no result and is older than the in-flight bound.

  A delivery *sent its call* when it has any result with a digest. A ledger record is looked up only when one of these holds, and it must match on attempt **and** role (N1 kills the role check; review 2's survivor is now covered by the other-role entry in step 8b).
- **The age rule is sound.**
  - The original thread either calls before the start deadline (75 s) or records `not_called`.
  - The call is bounded at 30 s, and the result transaction's only blocking statement is bounded at 10 s, so a delivery with no result older than 120 s has either lost its result transaction or its thread died.
  - Treating it as "may have run" fails closed. It becomes `missing` and stops the assignment.
  - It recovers through its own replay: the replay records `unknown` (the supervisor holds the attempt) or `not_accepted` (it does not).
  - During the first 120 s the delivery is invisible. In that window, though, a call it sent still holds the ledger lock (so there is no idle pre-settle and no admission), or its record is written under that lock.
  - The bound is exactly the replay's `young` bound. Both use `bounds().0`.
- **Can a real run be closed wrongly?** Closing (`withdrawn`, "sent nothing") needs all three of the following:
  1. no ledger record for that attempt and role. A record always wins: `usage_of`, as H1 shows;
  2. the newest row is `missing`;
  3. the newest result is one that proves nothing was sent.

  `refused`, `budget_refused` and `not_called` come only from the original flow, before or instead of a call. `not_accepted` comes only from a replay, when the supervisor answers 404 for the attempt. Results are append-only, and a replay adds a row only when the newest result is `unknown` or absent, so an `accepted` delivery can never later get a "nothing ran" result.

  A wrong close would need two failures together: the ledger lost the record of a call that really ran (for example the coordinator died mid-run before the record was written), **and** the supervisor lost its stored attempt (its state volume was reset) and answered 404. That double failure is outside what this step can detect, and the replay path already trusts the supervisor's 404 in the same way. Acceptable.

  A closed row stays closed only while the result still says nothing ran and no record exists (`stored == "sent-nothing"` → `continue`). A record that appears later reopens it as measured or unmeasured.
- **The `|| sent` arm.** This covers a delivery whose call was sent (it has a digest) but whose newest result is `not_accepted`. Example: `unknown`, where the library wrote a lost-answer record, followed by a replay that gets 404. The library's record is then still matched, and its unmeasured state stops the assignment. No test reaches this arm (H5 survives). The budget's own "asks for a person" check would stop the next call anyway, so this is defence in depth. Carry-over C-2.

### Review 2's Info items

- **I-1: fixed.** N1 (role matching) and N3 (`unknown`) are killed by step 8b (S5-N1, S5-N3).
- **I-2: fixed.** The chain reads usage against the configured ledger (`chain.rs:100-103`). Step 4b(d) asserts `ledger_mismatch: true` and `awaiting_human_decision` (S5-N5).
- **I-3: fixed.**
  - The `missing` reason string uses a `\` continuation, so the run of spaces is gone.
  - The start deadline is 75 s, giving 75 + 30 + 10 + 5 = 120 (5 s of slack).
  - The trailing spaces in `mutants.txt` are gone.
  - S5-C10 (the deadline) is now killed by step 8b(i): with the ledger busy for 2.5 s, the delivery records `not_called`, and no ledger record is created for `ad`.

### The test-override environment variables (`delivery.rs:713-746`)

- **What they do.** `IEO_COORD_DELIVERY_IN_FLIGHT_SECS` and `IEO_COORD_DELIVERY_START_DEADLINE_SECS` are read once (`OnceLock`) from the coordinator process environment.
- **Validation.**
  - Both must be set and parse as `u64`.
  - The deadline must be at least 1.
  - `deadline + 45 ≤ in_flight` must hold, which keeps the call and result inside the replay window.
  - Otherwise the defaults (120/75) are used, and a message is printed to stderr.
- **Who can set them.** Only whoever starts the coordinator process. That is the same trust level as `--roles`, `--tokens` and `IEO_DB_URL`. No API caller can set them.
- **Shorter values** (the test's 47/2) only fail closed: more `not_called` results, and earlier `missing`.
- **Larger values are accepted without any cap.** For example in-flight 10^9 with deadline 10^9 − 45 passes the check.
  - The start deadline is then effectively disabled.
  - A delivery with no result never becomes "old", so N-M1's hole reopens: a lost-result run is not charged or stopped at the assignment level.
  - Its replay answers `DELIVERY_IN_PROGRESS` indefinitely.
  - The budget library still bounds the run.
- **Nothing is logged when an override *takes effect*.** Only a rejected override is logged.
- **Documentation.** REPORT §1.5 says the variables are for tests and that "운영 설정에서는 쓰지 않는다". That is stated, not enforced.
- **Judgement.** This is acceptable for this PR, because it needs an operator to misconfigure the process and the defaults are safe. It should be tightened as carry-over C-1. It is not a must-fix.

### Regressions in 1548061..af68c93

- `replay`'s `young` check now uses `make_interval(secs => $2)` with the same bound as settlement. It is equivalent to the old `interval '120 seconds'` at the defaults.
- `assignment_chain` gained a `roles` parameter. Its only caller is `api::dispatch`.
- `limits_e2e` raised the test role share to 10 runs and 1,000 billable to fit the new steps. Step 7's expectation changed from 60 to 940 for that reason only (it still checks that the budget is the binding limit). That is not a weakened assertion.
- Step 9 moved to generation 6, and the final totals were updated with an explanation (11 deliveries, 13 usage rows, 6 events), which I checked against the steps.
- No accepted operation changed behaviour in this correction. REPORT §3 is unchanged except for the 75 s value.

### Ledger and docs

- The ledger update adds review 1548061 to the evidence.
- `render --check` is clean, the validators pass, and the links resolve.
- REPORT §12 maps review 2's N-M1 and I-1..I-3 to commit 9f0cb93, step 8b and mutants S5-N1..N5/C10.

## Findings

### Must-fix

None.

### Carry-over

#### C-1 (Low): the delivery-bound overrides have no upper cap and are not logged when used

- **Where.** `crates/ieo-coord/src/delivery.rs:720-746` (`bounds`, `checked_bounds`).
- **Failure scenario.** A deployment inherits test environment values, or someone sets `IEO_COORD_DELIVERY_IN_FLIGHT_SECS=86400` with a deadline of 86355. Both pass the check, nothing is logged, and then:
  - lost-result deliveries are not charged or stopped for a day (N-M1's window);
  - replays answer `DELIVERY_IN_PROGRESS` for that long;
  - the start deadline no longer limits anything.
- **Fix (next step or a small follow-up).**
  - Accept only values at or below the defaults (`in_flight ≤ 120`, `deadline ≤ 75`). Shorter values only fail closed.
  - Always log the effective bounds at startup when they differ from the defaults.
  - Optionally require an explicit `--test-delivery-bounds` flag instead of the environment.

#### C-2 (Low): the "sent but nothing ran" matching arm is untested

- **Where.** `crates/ieo-coord/src/limits.rs:120` (`may_have_run || sent`).
- **Evidence.** H5 drops `|| sent` and survives both `limits_e2e` and `delivery_e2e`.
- **Case.** A delivery whose answer was lost (`unknown`, digest kept, a lost-answer record in the ledger) and whose replay then records `not_accepted`. Without the arm, its unmeasured record would no longer stop the assignment. Only the budget library's "asks for a person" check would stop the next call.
- **Fix.** Extend `delivery_e2e` step 10 so that, before the reconciliation, the a10 lost-answer record is still present after the `not_accepted` replay. Then assert 409 `ASSIGNMENT_USAGE_UNMEASURED` and an `unmeasured` usage row for a10.

#### C-3 (Info): known limits, recorded

- **Double failure.** A wrong close needs both a lost ledger record and a supervisor 404 for an attempt that ran (see Analysis). Worth a line in REPORT §8.
- **No margin left with test bounds.** The test bounds (47/2) are exactly `deadline + 45 = in_flight`, the same margin the defaults have by design.

## Summary for the author

- **Approve at af68c93.**
- N-M1 is fixed and confirmed with my probe: a lost-result run is now charged, and it stops the assignment while it is unmeasured. Closing a `missing` row requires a result that proves nothing ran and no ledger record, so a run with a record is never closed.
- Review 2's I-1..I-3 are fixed.
- Fmt, clippy, the validators and render pass; 380 tests pass; 28/28 author mutants are killed, and 1 of my 2 is killed.
- **Carry-over, to record in the ledger or the next step's plan; no correction needed in this PR:**
  - C-1: cap the override variables at the defaults and log them when used.
  - C-2: test the `|| sent` arm.
  - C-3: the double-failure note.
