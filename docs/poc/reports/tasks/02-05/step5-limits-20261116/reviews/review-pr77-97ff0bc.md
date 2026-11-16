# Review of PR #77 at 97ff0bc (review 1)

Reviewer: independent reviewer (did not write the PR). Head `97ff0bcf549736b70469680260a799655babcdc7`, base main `7dcb2fb`. Scope: 02-05 step 5 (assignment-level time and token limits, C7), the HTTP `statement_timeout` (review fe924b7 I-5), the chain `"none"`-link test (review debf413 L-3), OpenAPI 0.13.0, ledger snapshot 0049, PREVENTION §1.12, and the review 43f08ed I-1..I-4 fixes.

## Verdict

**Changes requested.** There is one Medium finding, M-1. When a ledger record is missing (because a person removed it, the file is missing or empty for a moment, or the budget plan now points at another ledger), every settled run of an assignment is marked `withdrawn`. Restoring the ledger does not bring the runs back, because of `ON CONFLICT DO NOTHING` and the newest-row-by-`settled_at` rule, and the append-only triggers block any repair in the database. The assignment's consumption then reads 0 for good, and its limits are bypassed. I reproduced this end to end.

Everything else is sound or Low/Info. The definitions match the user decision exactly. Refusals leave the state unchanged. The run limit is the minimum of the assignment, budget and role-share amounts, computed in the coordinator. There are no invented numbers. OpenAPI is additive. The ledger facts match GitHub. 379 tests pass.

## What I ran

All runs were in a detached worktree at 97ff0bc, used test DB 55432 only, and made no model calls. Logs are in the scratchpad (`/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`).

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --all --check` | OK | `r77-fmt.log` |
| `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings` | OK | `r77-clippy.log` |
| `cargo test -j 4 --workspace --locked` (after `cargo clean -p ieo-coord -p ieo-supervisor`, `IEO_TEST_DB_URL` set, `RUST_TEST_THREADS=4`) | **379 passed, 0 failed, 0 ignored, 0 SKIP**; this matches REPORT §4 | `r77-test.log` |
| `openapi_spec_validator contracts/ieo-api-v0.openapi.yaml` | OK | `r77-openapi.log` |
| `validate_package.py` | `schema_errors: 0`, rc 0 | `r77-validate.log` |
| `validate_contracts.py` | rc 0 | `r77-contracts.log` |
| `render_progress.py --check` | `ok: true` (5 files) | `r77-render.log` |
| Relative links in the 9 changed `.md` files | 0 broken | (inline script) |
| `gh pr view 76`, `gh run view 43799695460` | PR #76 MERGED, merge commit `7dcb2fb8…`, head `43f08ed`, merged 2026-11-16T03:41:37Z. The run is `ci`/push on `7dcb2fb8…`, `success`. Both match snapshot 0049 | — |
| `git diff 43fad72..97ff0bc -- crates contracts` | empty, so the evidence commit's test log (first line 43fad72, clean tree) covers the shipped code | — |
| Probe for M-1 (temporary edit to `limits_e2e.rs`, since reverted) | reproduced; see M-1 | `r77-probe-withdrawn.log` |
| My own mutants (one at a time, each reverted with `git checkout`) | 5 killed, 1 **survived** (table below) | `r77-mut-*.log` |

My mutants:

| id | Mutation | Target | Result |
| --- | --- | --- | --- |
| RA | `limits.rs` `room`: `s <= grace` → `s < grace` (the 6 s boundary) | `limits_e2e` (HTTP) | killed (step 8: 201 instead of 409) |
| RB | `delivery.rs:549`: drop the post-lock re-settle (`settle(…, adm.records())`) | `limits_e2e` (HTTP) | **survived** (L-1) |
| RC | `limits.rs` `state`: newest row by `settled_at ASC` (oldest row counts) | `delivery_e2e` (HTTP) | killed |
| RD | `delivery.rs:272`: `state(tx, &aid, 0)` (generation 0's limits instead of the head's) | `limits_e2e` (HTTP) | killed (step 5) |
| RE | `limits.rs` `usage_of`: `ms / 1000` instead of `div_ceil` | `limits_e2e` (HTTP) | killed |
| RF | `limits.rs` `room`: return `(secs, None)`, so the token room is not lowered | `limits_e2e` (HTTP) | killed |

Cleanup: I dropped all 8 `ieo_t_lim_http_*`/`ieo_t_dlv_http_*` databases left by my failing runs. They were identified by the PIDs of my runs, and `ieo_t_%` is 0 afterwards. I removed the matching `/tmp/ieo-*-e2e-http-<pid>` directories. I did not touch 55433, 58090, PR #29 or other worktrees.

## Analysis

### 1. Accounting

- **Definitions.** `limits.rs:171` gives `total = input + output + cache_creation`. `cache_read` is stored and reported (`tokens.cache_read`) but not counted. `elapsed_seconds` is the sum of per-run `duration_ms.div_ceil(1000)` (`limits.rs:110`), the same charge as `budget::spent` (`budget.rs:134-135`), so the time is not wall clock. Sums run over every generation (`assignment_id = $1` only, `limits.rs:156-158`). This matches User_Decisions_2026-11-16 (가)/(i) exactly. S5-M3, S5-M5 and my RE/RF confirm it.
- **Resume = cumulative difference.** This is not recomputed here. The ledger record's `usage.run` is the 02-01 meter's per-run share, written by the unchanged shared library. Step 3 checks a per-run row of 3/7/10/20 when the session totals are double that.
- **Unmeasured.** A record that is not `measured`, or has no duration, gives an `unmeasured` row. `room` refuses on any of these with 409 `ASSIGNMENT_USAGE_UNMEASURED`. It is never counted as 0 (S5-M4; e2e step 9 covers a guard stop).
- **Ledger as authority, "copy at the next delivery".** Settlement is lazy. It happens before tx 1, when the ledger is idle, and again under the budget lock. Consequences:
  - Chain views lag until the next delivery (reported, §8).
  - The withdraw rule. This is a real defect; see M-1.
  - Records are matched by `attempt_id` alone, across the whole ledger; see I-2.
- **Restart.** Step 6 compares the chain JSON before and after a restart, and the refusal is the same. The DB rows are the state.

### 2. Enforcement

- **Pre-check.** It runs inside tx 1 after the assignment, generation and attempt checks (`delivery.rs:272-273`). A refusal rolls tx 1 back, and one event row is written in a separate transaction. The e2e `refused` helper checks that assignments, deliveries and results are unchanged and that the limit-event table grows by at most 1. Repeating the refusal adds no second event (PK `(aid, gen, code, settled_runs)`).
- **Run limits.** Under the ledger lock (`delivery.rs:546-571`), the records the admission read are settled again and `Admission::lower` is applied. `lower` only reduces the window and billable room, so the result is min(assignment remaining, budget/role share). This is computed in the coordinator, outside the role containers. A refusal at this point is recorded as `budget_refused`.
  - The re-settle exists for the race "a run finished between the idle read and the admission". It is untested (RB survives, L-1).
- **Grace.** `room` refuses when `remaining.elapsed_seconds <= GRACE` (6 s, taken from `ieo_supervisor::run::GRACE`). This matches step 3's `lower` rule. The boundary is tested (RA killed).
- **Concurrency.**
  - The ledger lock is held by `Running` until the record is written, so a second delivery during a run cannot be admitted. It waits up to 3 s, then gets `budget_refused`.
  - Two concurrent settles of the same record collide on the PK and are harmless (`DO NOTHING`).
  - `settled_at DEFAULT now()` is the transaction start time, not the time the ledger was read. Together with M-1 this makes the "newest row" rule order-sensitive (see the M-1 fix).
- **Overflow and negatives.**
  - `i64_of` saturates.
  - The columns have `CHECK >= 0`.
  - `state` clamps sums with `max(0)` and uses `saturating_add`/`saturating_sub`.
  - A `sum()` beyond bigint would fail closed (500).
- **422 and spec without limits.** `effective_limits` refuses a request above the revision value, and a request where the revision has no value. Executable revisions cannot have null limits (UNBOUND_DRAFT, `api.rs:2749-2756`), so the "no value" branch is unreachable today.

### 3. Numbers and the approved budget

- No production configuration or policy number was added. The assignment limits come only from the request (≤ revision) or the revision.
- The role share in the tests (8 runs / 120 billable) is a test fixture, and it is labelled as one (`limits_e2e.rs:326-327`).
- REPORT §1.3 and §8 and `next_action` say plainly that budget (나) is not wired and is a separate PR before step 8.
- Nothing enables real runs. `budget_plan` still accepts only `02-04`/`X-02`.
- The only new constant is `HTTP_STATEMENT_TIMEOUT = 10 s`. This is an engineering bound like the existing 3/10/30/120 s values, not a limit value under `numeric_values`.

### 4. statement_timeout

- **Scope.** Only `api::handle` uses `HttpGuard`. gRPC keeps its deadline-derived timeout (`grpc.rs:165`). Migrations and startup do not go through `run_tx` with this guard. `SET LOCAL` limits it to each HTTP transaction.
- **Transaction semantics.** On 57014, `run_tx` rolls back the failing transaction and the operation answers 500.
- **Long statements.** I found no legitimate HTTP statement that should take near 10 s today. The chain and registration queries are indexed lookups. The new test checks the 9–30 s window with a held advisory lock.
- **The "~95 s < 120 s" argument is not established (L-2).**
  - The timeout is per statement, not per transaction.
  - The new post-lock settle transaction is not counted.
  - `caller::admit`'s `services_clear` reads each role's status with the 30 s `caller::Http` timeout, which is 2 × 30 s with two roles.
  - The worst case is therefore well above 120 s. The text and code comment overstate the guarantee.
  - The new 500 message "nothing was stored" is also wrong for deliverAssignment's later transactions, where the request row and possibly a started run already exist.

### 5. Changes to accepted behaviour (REPORT §3)

- The list is complete as far as I can see:
  - the 10 s bound on all HTTP operations;
  - issueAssignment accepting `limits` (an earlier 400 can now be 201 or 422);
  - deliverAssignment's 409 before recording;
  - getOwnAssignment's `limits` source;
  - the chain's `evidence_levels`.
- All of these move to the closed side or are additive.
- One more user-visible change is not listed. The delivery's stored and sent `coordinator_record.limits` still shows the revision's values while getOwnAssignment and the chain show the assignment's (I-1).
- **Modified accepted tests.**
  - `delivery_e2e` step 12 replaces a weaker check (`budget_refused` + reason contains "asks for a person") with a stronger one: the code is 409 `ASSIGNMENT_USAGE_UNMEASURED`, the delivery count is unchanged, and the event count is 1. The totals 20→19 and 22→21 follow from that.
  - `claims_are_kept…` only extends the `verified` list.
  - No assertion was weakened.

### 6. Do the tests back REPORT's claims

- Each row of the REPORT §4 table maps to code in `limits_e2e.rs:489-760`, and the author's 13 mutants are recorded.
- 5 of my 6 mutants are killed.
- The correction path (a correction record giving a newer row) is untested, as the REPORT admits (§8).
- The post-lock re-settle is untested (L-1).
- The `withdrawn` path is covered only in the "removed for good" direction, never "removed then restored" (M-1).

### 7. Ledger and docs

- Snapshot 0049 matches GitHub: PR #76 merged at 7dcb2fb from head 43f08ed, and main CI 43799695460 succeeded on 7dcb2fb.
- `render --check` is clean, the validators pass, and the links resolve.
- Review 43f08ed I-1..I-4 are applied:
  - `unresolved` now says "L-1·I-2·I-6은 단계 4에서 처리";
  - the §11 I-2 cell is annotated;
  - step 8 in `next_action` names the 5dbb3d8 I-3 stream check;
  - the trailing blank line is gone.
- PREVENTION §1.12 separates the confirmed facts (the parent session did not end, the parent had no monitor, the sub-agent did not resume) from the guess, which it marks "추측(확인 불가)" (the completion notice is not delivered as a resume signal). Its times (13:36:35, about 14:00, 14:01) cannot be checked from the repository. That is acceptable for a prevention row.
- One stale sentence in the REPORT header: I-3.

## Findings

### M-1 (Medium): a transient ledger gap permanently erases an assignment's consumption

- **Where.**
  - `crates/ieo-coord/src/limits.rs:54-68`: a record that is absent becomes `withdrawn:{prev}`.
  - `limits.rs:69-92`: a restored record is skipped, because `stored != sha` leads to an INSERT that hits the existing `(delivery_id, record_sha256)` PK and does nothing (`ON CONFLICT DO NOTHING`), so the newest row stays `withdrawn`.
  - `limits.rs:156-158`: the newest row is chosen by `settled_at`.
  - `crates/ieo-supervisor/src/caller.rs:649-652`: `records_when_idle` returns `Some(vec![])` when `ledger_dir` does not exist.
  - `caller.rs:292`: `raw_records` reads a missing `records.jsonl` as empty.
  - `schema.sql`: the append-only trigger on `assignment_run_usage`.
- **Failure scenario.** This is reproduced in `r77-probe-withdrawn.log`, from a temporary edit to `limits_e2e` after step 3, where A1 had two measured runs totalling 40 of 40.
  1. Rename `records.jsonl`. Any delivery naming A1 runs the pre-settle, even one later refused (I used `ATTEMPT_ID_CONFLICT`).
  2. Rename the file back and deliver again.
  3. `getAssignmentChain` now reports `total_tokens 0, runs 0` for good, so the 40-token limit is open again (`PROBE after move back (409): total 0 runs 0`).
- **Real triggers.**
  - A person edits or rewrites the ledger. An editor save or a `>` redirect truncates the file for a moment.
  - The ledger directory is moved, or is not mounted yet.
  - Most likely: the planned "예산 이름·상한 코드" PR points the plan at budget (나)'s own ledger directory. Every existing assignment's runs are then withdrawn, and switching back does not restore them.
- **Why the budget is not affected the same way.** The budget library recovers as soon as the file is back. The assignment rows do not, and the append-only trigger forbids fixing them with UPDATE/DELETE.
- **Impact.** This turns "the ledger is the authority" into "the most recent read of the ledger, even an empty one, is the authority forever." Measured runs are effectively counted as 0. That is the outcome the user decision and the plan rule out for unmeasured runs, and it is worse for measured ones.
- **Fix (any one of these closes it; the first two together are best).**
  1. Fail closed on a missing record. A run that was settled before and is now absent from an idle ledger should block the assignment (for example a new state, or treat it as `unmeasured`, "the ledger no longer has this record: a person decides"), not stop counting. If a person really wants a run to stop counting, that should be an explicit ledger correction or withdrawal entry that names the attempt, not the record's absence.
  2. Make re-appearance count. Give rows a monotonically increasing `seq` (bigserial). Drop `ON CONFLICT DO NOTHING` for content that differs from the newest row, so a record that comes back inserts a new newest row. Pick the newest row by `seq`, not by the transaction-start `settled_at`.
  3. Bind settlement to the ledger. Store the budget name and `ledger_dir` (or a ledger identity) on each row. Refuse to settle, or refuse the delivery, from a different ledger than the one the row was settled from.
  4. Treat an empty or missing `records.jsonl` as "cannot say" (`None`) whenever the DB already holds settled rows for the assignment.
- **Tests.** Add the removed-then-restored scenario to `delivery_e2e` step 10 or to `limits_e2e`. Also test a budget plan that points at a fresh `ledger_dir`.

### L-1 (Low): the post-lock re-settle is not covered by any test

- **Where.** `crates/ieo-coord/src/delivery.rs:546-551`.
- **Evidence.** Mutant RB, which removes the `settle(…, adm.records())` call, passes `assignment_limits_bound_and_stop_deliveries`. Every e2e step waits for `settled(attempt)` before the next delivery, so the pre-settle always sees the finished run.
- **Failure scenario.** At the idle read the ledger is busy, for example with the guardrail's call or a run finishing at that moment, so the pre-settle is skipped and tx 1 checks stale DB totals. Without the post-lock settle, the run would be admitted with a billable room that ignores the just-finished run. The code is correct today, but nothing guards it.
- **Fix.** Add an e2e step that holds `ledger/caller.lock` (flock) from the test for about 1 s while sending the delivery, then releases it before admit's 3 s wait runs out. Do this after a run that took the assignment to its limit, and expect `budget_refused` with "assignment limits: …".

### L-2 (Low): the I-5 bound arithmetic is not right, and the 57014 message overstates "nothing was stored"

- **Where.**
  - `crates/ieo-coord/src/api.rs:546-552`: the `HTTP_STATEMENT_TIMEOUT` comment.
  - REPORT §1.5.
  - `api.rs:52-56`: the message.
- **What is wrong with the argument.**
  1. `statement_timeout` limits each statement, not each transaction. A transaction with several statements that can block (an advisory lock, then a row conflict) can exceed 10 s.
  2. The count after `requested_at` omits the new post-lock settle transaction. Counted, the transactions are the request, judge, judge again, settle and result.
  3. `caller::admit` → `services_clear` reads the status of every role in the plan with `caller::Http`'s 30 s timeout (`caller.rs:184`). With two roles that is up to 60 s, which the "3 s ledger lock wait" omits.
  4. The worst case is therefore roughly 5×10 + 2×10 + 3 + 60 + 30 ≈ 163 s, above `IN_FLIGHT` 120 s, before multi-statement effects.
  5. Past 120 s, a replay re-queries the supervisor while the original call may still be about to start.
  6. Separately, deliverAssignment's later transactions (the post-lock settle, the result) can time out after the request row exists or the run has started, yet the answer says "nothing was stored".
- **Fix.**
  - Restate the bound honestly, or enforce it. One way is a per-delivery deadline guard (as gRPC has) with remaining time checked before each step. Another is `transaction_timeout` if the server is PostgreSQL 17 or later. Another is to raise `IN_FLIGHT` above the true sum.
  - Make the 57014 message say "this step's transaction was rolled back" rather than "nothing was stored".

### I-1 (Info): `coordinator_record.limits` still shows the revision's limits

- **Where.** `crates/ieo-coord/src/delivery.rs:279-281`.
- **What happens.** The verified record stored in `role_deliveries` and sent to the supervisor carries the revision's values. I saw `{"elapsed_seconds":3600,"total_tokens":100000}` on generation 4, whose own limit was the time used plus 7 s. Meanwhile getOwnAssignment and the chain show the assignment's own limits.
- **Effect.** The supervisor ignores the field (`serve.rs:643-665`), so nothing breaks. But two verified records now disagree on "the assignment's limits".
- **Fix.** Put the assignment's effective limits (`st["limits"]`) there, or rename the field `revision_limits`. List it in REPORT §3.

### I-2 (Info): records are matched by `attempt_id` across the whole ledger

- **Where.** `crates/ieo-coord/src/limits.rs:54`.
- **What happens.** Deliveries keep `attempt_id` unique per (repo, role), but the ledger is shared per budget and can include 02-04 host-caller records or another repo's.
- **Effect.** A delivery whose admission was refused with "attempt … already recorded" is charged with the other call's usage. This errs on the closed side (overcount).
- **Fix.** Also match the record's `role` and the `delivery_id`/`target_sha` carried in the sent body. Alternatively, refuse at `record_request` an attempt id the ledger already holds.

### I-3 (Info): the REPORT header contradicts the PR

- **Where.** `docs/poc/reports/tasks/02-05/step5-limits-20261116/REPORT.md:11`.
- **What happens.** It says "독립 검토 전. 원장(`state.json`)은 고치지 않았다", but commit 97ff0bc in this PR adds snapshot 0049 to `state.json`.
- **Fix.** Update the row to name snapshot 0049.

### I-4 (Info): OpenAPI claims a case the code does not produce

- **Where.** `contracts/ieo-api-v0.openapi.yaml:845`.
- **What happens.** It says `ASSIGNMENT_USAGE_UNMEASURED` covers "(a run unmeasured, or started with no record)". The code treats a delivery with no ledger record as never sent and skips it (`limits.rs:66`). A started run that has no record yet leaves an in-flight marker, so `records_when_idle` returns `None` and the budget refuses later, with `budget_refused` and not 409.
- **Fix.** Drop "or started with no record", or implement it.

### I-5 (Info): design notes worth a sentence in the REPORT

- **Re-issuing a generation reopens a stopped assignment.** Issuing a new generation with a larger limit (step 5 of the e2e) reopens an assignment in `awaiting_human_decision`. A `coordinator:assign` service principal can do this with no decision record, up to the revision's limit. The revision value is still a hard cap, because consumption is summed over all generations.
- **The chain for an older generation is not the head's view.** The chain of a non-head generation reports that generation's `limits` and `operational_state`, not the head's, even though the head's limits are what apply.
- **S5-M12 is detected by a hang.** The test's HTTP client has no read timeout, so this mutant is killed only by a hang (already noted in REPORT §8).

## Summary for the author

- **Must fix (M-1).** Do not let a missing ledger record lower an assignment's consumption, and do not make a withdrawal permanent. Fail closed on "settled before, absent now", or require an explicit withdrawal entry. Let a returning record insert a newer row (use a `seq`, not `settled_at`). Bind rows to the ledger they came from. Add a removed-then-restored test and a changed-`ledger_dir` test. This matters before the budget (나) wiring PR changes the ledger.
- **Should fix (Low).**
  - Add a busy-ledger e2e step that kills mutant RB (L-1).
  - Correct the 95 s argument and the "nothing was stored" message, or enforce a real per-delivery deadline (L-2).
- **Optional (Info).**
  - Align `coordinator_record.limits` (I-1).
  - Tighten record matching (I-2).
  - Fix the REPORT header row (I-3).
  - Fix the OpenAPI wording (I-4).
  - Note the design points in the REPORT (I-5).
- **Confirmed correct.**
  - The definitions match the user decision.
  - Refusals change nothing but one event.
  - The run limit is the minimum of the three amounts, computed in the coordinator.
  - No new limit numbers.
  - Budget (나) is honestly not wired.
  - OpenAPI 0.13.0 is additive.
  - Modified accepted tests got stronger.
  - Snapshot 0049 matches GitHub.
  - Review 43f08ed I-1..I-4 are applied.
  - §1.12 separates fact from guess.
  - 379 tests pass.
