# Review of PR #75 at 5dbb3d8 (review 1)

## Verdict

**Changes requested.** One Medium finding (M-1). Everything else is Low or Info. The build, tests, fmt, clippy, the package validator and the OpenAPI validator are all clean. Mutation testing of what the REPORT claims holds up, but one of my own mutations survived.

## What I ran

All runs used a detached worktree at 5dbb3d8, the test DB on port 55432 and `-j 4`. Logs are in the scratchpad as `r75a-*.log`.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy -j 4 --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test -j 4 --workspace` (`RUST_TEST_THREADS=4`, test DB) | exit 0: **376 passed, 0 failed, 0 ignored** across 41 test binaries. Matches the REPORT (367 + 9). All 9 new tests are present by name and pass. |
| `validate_package.py` | exit 0, `schema_errors: 0` |
| `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml` | OK |
| proto diff 375f21f..HEAD | empty, so buf does not apply (confirmed) |
| Evidence integrity | `test.log` header is commit 84c61d9 with a clean tree. 84c61d9..5dbb3d8 changes only REPORT and evidence files. |
| My own mutations (each applied alone, targeted tests run, then reverted) | see the table below |
| Test DB cleanup | Afterwards I listed `ieo_t_%` databases on 55432: none were left. Test `Env` drops its DB even when a test panics. |

| ID | Mutation (by hand) | Result |
| --- | --- | --- |
| A | `check_delivery` also accepts `refused` (`api.rs:4076`) | KILLED: `a_takeover_is_stored_only…` (coordinator.rs:4322) |
| B | chain `first_action_conflict` always false (`chain.rs:285`) | KILLED: `a_lost_first_action…` (coordinator.rs:4539) |
| C | child events (`parent_tool_use_id`) count as actions (`first_action.rs:32`) | KILLED: `first_action::tests::the_first_action_is…` |
| D | `fill_first_action` drops the `taken_over` condition (`api.rs:4177` becomes `if exists {`) | **SURVIVED**: the whole `--test coordinator` passes (see L-2) |

## Section-by-section analysis

### 1. Takeover and delivery: transactions and locks

- **Same transaction.** `check_delivery`, then the checkpoint INSERT, then `role_takeovers` INSERT … ON CONFLICT DO NOTHING. All run inside the single `report_checkpoint` transaction (`api.rs` ~3911–3945).
  - When the insert affects anything other than 1 row, the code returns `Err`, and the checkpoint row is rolled back with it.
  - The test checks this with row counts ("no checkpoint row without its takeover row"). Correct.
- **Lock order.** The takeover transaction takes `checkpoint:{repo}:{role}`, then `delivery:{repo}:{role}` (`api.rs:3779`, `api.rs:4048`).
  - The other holders of the delivery lock are `record_request` and `record_result` (`delivery.rs:208`, `:333`). Neither takes the checkpoint lock.
  - `compare_checkpoint` takes only the checkpoint lock, and in a separate transaction.
  - `record_first_action` and `fill_first_action` take no extra locks.
  - So there is no inverse ordering anywhere and no deadlock. The REPORT's claim is accurate.
- **Duplicate re-report.** The `seq <= last_seq` with the same SHA → 200 branch runs before `check_delivery` and the takeover insert, so no second takeover row is written. Correct.
- **"No result yet" acceptance.** Accepting a delivery with no result is needed for the normal path: the supervisor reports the takeover before the coordinator's transaction 2. It is a disclosed deviation from PLAN ("결과가 기록됐거나 unknown") and is reasonable.
  - However, nothing binds the takeover to the call the delivery actually made. See M-1.
- **One takeover per delivery.** Enforced by the `delivery_id` primary key and tested. It is not in PLAN, and it is disclosed in REPORT §7.

### 2. Behaviour changes to accepted 02-04 operations (REPORT §3)

- All the changes I found in the diff are listed:
  - takeover refused without a delivery;
  - new 400 format checks on the checkpoint;
  - `last_actual_action_at` semantics;
  - supervisor `delivery_id` validation, plus the checkpoint growing two keys;
  - the `source` column and `subject.source` in history;
  - the test-fixture changes.
- **All are fail-closed.** I confirmed the supervisor path. A refused takeover goes to the existing `CentralRefused` branch (`serve.rs:1050–1064`): `spawned: false`, nothing is run, and the outcome is written. `chain_e2e` step 8 covers it.
- **Step 8 is not broken.** PLAN §4.3 (나) routes R1, R2 and G1 through coordinator delivery and `compare`, which now always carry `delivery_id`.
- **The 02-04 host caller (`ieo-role-call`) is affected, and the REPORT says so honestly** (§3 item 1, runtime contract item 16): a direct start against a supervisor whose central record is the coordinator no longer runs.
  - One addition for the user: if the 5 reserved 02-04 runs are ever used later through `ieo-role-call` against a coordinator-central supervisor, they will be refused. Only `local_only` still works.
  - The REPORT implies this but does not tie it to the reserved budget (Info I-5).
- **Weakened assertion check.** The only changed assertion in an accepted test is `last_actual_action_at` `is_string()` → `is_null()` (`tests/serve.rs`). That is justified by the semantic change, and the positive case is covered end to end in `chain_e2e` step 1.
  - The other 02-04 test edits only add fixture deliveries and extra counts.
  - `delivery_e2e` goes from 19 to 20 deliveries and adds an assertion of 6 takeovers. No expected judgment values changed.

### 3. First-action detection

- **Definition.** `first_action.rs` matches the PLAN and contract text:
  - only lines after `init` count;
  - child events, empty, thinking-only and blank-text events are excluded;
  - `tool_use` takes precedence over text;
  - offsets are 0-based line indexes, and non-JSON lines are still counted.
- **Probe placement.** The probe sits in `Sink::line`, after the write, for both the watch path and the post-stop copier. So offsets match the order of lines in `stdout.jsonl`.
- **"none" versus null.**
  - `after_run` with no action gives `"none"`.
  - `error` gives the observation or null.
  - `takeover` gives null.
  - Nothing is ever filled with 0 or the start time. Mutation S4-M7 covers the 0/start-time case.
- **Failure handling.** A failed report never stops the run: the report is sent from a spawned thread, and the callback holds the service state mutex only briefly. I checked every state-lock holder in `serve.rs`; none holds it long.
- **Restore after restart.** `last_actual_action_at` is rebuilt as the maximum `observed_at` over the run outcomes on the volume. That is correct for completed runs. A first action seen in a run that crashed before `outcome.json` is lost (Info I-4).

### 4. getAssignmentChain

- **Access.**
  - Agent credentials get 403.
  - Otherwise `coordinator:assign` or `roles:control` is required.
  - Out-of-repo, unknown assignment and unknown generation all return the same 404.
  - The generation format is checked first and gives 400, but that reveals nothing about existence.
- **Data.** Everything is read from the DB in one transaction, with no supervisor calls. Restart stability is shown by `chain_e2e` step 6.
- **Evidence levels.** Only sprint, assignment and deliveries are `verified`; everything else is `claim`. That is correct.
- **Coverage against PLAN §3.2.** Every element is present: sources, digest, generation, target, limits, issuer, policy digest, deliveries with times, takeovers, first actions, checkpoints, tamper events and agent reports.
- **Weak spots.**
  - Links between first actions and deliveries go only through `attempt_id`.
  - "none" comes from any checkpoint carrying that `delivery_id`; the code does not check that the checkpoint belongs to the takeover's session (Info I-2).

### 5. Claim separation versus the judge

- REPORT §1.6 is accurate and honest. G3 does read the current guardrail `guardrail_review` claim (`api.rs:3319–3336`, "G3 review inputs (N4)").
- The test (`claims_are_kept_apart…`) only shows that development progress reports, takeovers, first actions and session status don't change the outcome. That is correct: a guardrail claim would change it.
- But the **new contract text in this PR still repeats the inaccurate statement** (L-1).

### 6. Tests and mutations

- The names and the count of 9 new tests match.
- I spot-checked mutants A, B and C by hand, and they behave as the REPORT says. All 17 S4 mutants listed in `evidence/mutants.txt` are plausible for their targets.
- My mutation D survived: see L-2.

### 7. Docs and contracts

- **OpenAPI 0.12.0 is additive**, apart from `minLength: 1` on reportSessionStatus `assignment_id`. That is a schema tightening, but it documents what the code already refused (I-6). It is disclosed and acceptable (Info).
- **Internal contract.** §1.2, the ReportCheckpoint order and the new §15 are consistent.
- **Runtime contract.** Revision 12 is a header entry, a reference added to §5-1 item 1, a reference added to the §7 status row, and new §15 items 15–17. All additive.
- The REPORT's links resolve, and the validator passes.

## Findings

### M-1 (Medium): a takeover is not bound to the call its delivery made, so the REPORT's "cannot later become refused/not_accepted" does not hold

- **Where.** `crates/ieo-coord/src/api.rs:4029–4079` (`check_delivery`), together with `crates/ieo-supervisor/src/serve.rs:1209`. The takeover checkpoint carries `delivery_id` but no `attempt_id`, and `check_delivery` compares only repo, role, assignment and generation, plus the latest outcome.
- **The REPORT's argument.** REPORT §7 says a delivery with no result can safely accept a takeover, because "supervisor가 202를 보낸 뒤에만 takeover가 있고". That reasoning assumes the takeover comes from *this delivery's own* call. Nothing checks it. The coordinator test itself shows `D-ok` taken over by session `s3`, and `D-in` by `s1`, whatever session or attempt the delivery holds.
- **Failure scenario.**
  1. Delivery D is recorded (transaction 1) and waits on the budget lock, or has passed the second judgment but not yet called.
  2. Any start that names D reaches the supervisor first. That could be an adapter-credential direct start (the I-4 residual gap the REPORT itself names in §8), or an operator tool retrying with a new `attempt_id` but the same `delivery_id`.
  3. Its takeover is accepted, because D has no result yet, and it consumes D.
  4. D's own call then either:
     - hits `ALREADY_RUNNING`, so D is recorded as `refused`; or
     - after a coordinator crash, a replay gets 404 and records `not_accepted`; or
     - runs, and then has its *own* takeover refused with `DELIVERY_MISMATCH "already has a takeover"`, so the delivered run never executes.
- **Consequence.** In the first two cases the chain shows `taken_over: true` next to `outcome: refused/not_accepted`, and the first action is linked by `attempt_id` to nothing. This breaks the invariant the step exists to establish: "a request alone is never a takeover", and the takeover answers *that* delivery.
- **Suggested fix.**
  1. The supervisor writes `attempt_id` (it already has `job.attempt`) on every checkpoint, or at least the takeover.
  2. `check_delivery` requires `role_deliveries.attempt_id = checkpoint.attempt_id`. Optionally, for `op = start`, also require session equality.
  3. Add a refusal row to `a_takeover_is_stored_only…` for "right delivery, other attempt", and a mutant that drops the check.
  4. Correct the REPORT §7 sentence, and the §8 I-4 description of the residual gap, to match.
- **Contract.** This is an additive checkpoint field under revision 12; no new revision is needed if it is done in this PR.

### L-1 (Low): new contract and code text still says claims are never judgment inputs

- **Where.**
  - `contracts/ieo-api-v0.openapi.yaml:1177–1178`, getAssignmentChain description, new in 0.12.0: "Claims are not inputs of the completion judgment (judgments read GitHub and execution evidence only)."
  - `crates/ieo-coord/src/chain.rs:16–18`, the module doc, says the same.
- **Problem.** REPORT §1.6 correctly states that G3 reads the guardrail `guardrail_review` claim, and that claim (`agent_reports` in this very response) can turn a judgment into a reject. The normative contract text added in this PR contradicts the author's own observation.
- **Failure scenario.** A consumer, for example step 9's acceptance mapping or 04-03, relies on the contract and concludes no claim affects the verdict.
- **Suggested fix.** Reword both places as "claims are not required inputs; the only claim the judgment reads is the current guardrail `guardrail_review` (G3, policy 0.6.0); see the step-4 REPORT §1.6". The same applies to the "never an input of a verdict" wording at line ~904 if it is meant to cover agent reports (it is about first actions, so it is fine as is).

### L-2 (Low): the takeover condition of the after_run fill is untested

- **Where.** `crates/ieo-coord/src/api.rs:4177` (`fill_first_action`).
- **Evidence.** Mutation D (`if exists || !taken_over(..)` → `if exists`) survives the whole coordinator suite.
- **Failure scenario.**
  - `report_checkpoint` accepts an `after_run` that is current even when no takeover of that session is stored: `is_current` alone is enough, and `continues` is not required.
  - With the guard removed, a first action would be recorded for a session that was never taken over. That breaks the precondition `recordFirstAction` enforces (409 `CHECKPOINT_MISMATCH`).
  - The code is right today, but a regression would go unnoticed.
- **Suggested fix.** In `a_lost_first_action…`, report an `after_run` for a session without a stored takeover that carries a `first_action` object. Assert that no `role_first_actions` row appears and the chain shows none.

### I-1 (Info): conflict detection compares only offset and kind

- **Where.** `chain.rs:285–296`.
- **Detail.** A checkpoint `first_action` that differs from the record only in `observed_at` or `attempt_id` is not flagged. Comparing strings directly would be tricky because of the millisecond versus microsecond rendering (REPORT §8). Comparing parsed timestamps, or the stored jsonb `body`, would work.

### I-2 (Info): link "none" and the after_run `delivery_id` are not cross-checked

- **Where.** `chain.rs` links, and `report_checkpoint`.
- **Detail.**
  - The "none" link comes from any checkpoint carrying that `delivery_id`.
  - An `after_run` whose `delivery_id` differs from its session's takeover delivery is accepted silently.
  - `fill_first_action` also takes `attempt_id` from the checkpoint without checking it against the delivery.
- **Assessment.** All of this is claim-level. It would be closed together with M-1 (attempt binding).

### I-3 (Info): first-action kinds the real stream may use

- **Where.** `first_action.rs:31–43`.
- **Detail.** Only `tool_use` and `text` blocks count, so `server_tool_use` (or other block types the real CLI may emit) would not count as an action.
- **Assessment.** This is acceptable until step 8 checks the real stream, as PLAN §4.1 (1) intends. It is worth listing as a step-8 check item.

### I-4 (Info): an in-flight first action is lost from the restored status

- **Where.** `serve.rs` ~313–321.
- **Detail.** `last_actual_action_at` is restored only from `outcome.json`, so a first action seen in a run that crashed before writing its outcome is lost from the restored status. The coordinator record still has it, if it was reported.
- **Assessment.** Worth one sentence in §8.

### I-5 (Info): the reserved 02-04 budget and the direct-start refusal

- **Detail.** REPORT §3 item 1 is honest. I suggest adding, for the user decision record, that the 5 reserved 02-04 runs can no longer be used through `ieo-role-call` against a coordinator-central supervisor. Only `local_only`, or delivery, would work.

### I-6 (Info): OpenAPI `minLength: 1` is a tightening

- **Detail.** It is technically a tightening rather than purely additive. It is disclosed as I-6 and matches code that already refused empty values. No action needed.

## Summary for the author

The implementation is solid:

- Same-transaction takeover rows, rollback on a second takeover, and a lock order that cannot deadlock.
- A clean first-action probe with correct "none"/null semantics, and fail-closed handling of direct starts.
- A well-scoped chain endpoint.
- 376 tests pass, and fmt, clippy and the validators are clean.

Please fix before merge:

- **M-1.** Bind the takeover to the delivery's `attempt_id`. Carry `attempt_id` on the checkpoint and compare it in `check_delivery`. Add a refusal test and a mutant, and correct REPORT §7 and §8 accordingly.
- **L-1.** Correct the "claims are not judgment inputs" sentence in OpenAPI 0.12.0 (getAssignmentChain) and in `chain.rs`. It contradicts your own §1.6 G3 observation.
- **L-2.** Add a test for the takeover condition of the after_run fill. My mutation D survives today.

The Info items are optional.
