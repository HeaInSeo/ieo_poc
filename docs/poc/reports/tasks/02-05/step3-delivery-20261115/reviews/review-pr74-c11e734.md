# Review of PR #74 at c11e734 (review 1)

- PR: HeaInSeo/ieo_poc#74, 02-05 step 3: coordinator `deliverAssignment` through `compare` (start/reconnect), the shared budget-check library split out of the 02-04 host caller, supervisor same-attempt replay and current-assignment check, runtime contract revision 11, OpenAPI 0.11.0. It also carries the step-3 REPORT and evidence, step-1/step-2 REPORT fixes (review a349dc6 L-2, L-3, I-5), the archived review a349dc6 and ledger snapshot 0047.
- Head: c11e7346cd398abf916c7ddd6254471c7b7869c6. I confirmed it with `gh pr view 74 --json headRefOid,statusCheckRollup` before and after the review.
- Base: main 3701f6b, which is also the merge base. Three commits sit on top:
  - 2c8fe8d: code, contracts and tests;
  - 87956da: evidence (test log at 2c8fe8d, mutants);
  - c11e734: step-3 REPORT, ledger 0047, step-1/step-2 REPORT fixes, archived review.
  - The code under test is the same at 2c8fe8d and c11e734 (only docs change in c11e734), so the evidence applies to the head.
- CI at c11e734 (run 43728715005, pull_request, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS, 365 passed and 0 failed in the job log
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan worktree, s3wt, PostgreSQL on 55433 or the coordinator on 58090. I used my own detached worktree (r74) and the shared target directory target-x08, and removed the worktree at the end. This is review 1 of at most 4.

## Verdict

**REQUEST CHANGES: one Medium finding (M-1). Three Low and five Info findings.**

- The budget library split is faithful. `call_with` is now exactly `admit` → `Admission::start` → `Running::finish`, with every 02-04/X-08 check in the same order and with the same messages. The 32 caller tests pass unchanged. The delivery path can reach the supervisor's start/reconnect only through `Admission::start_with`. The only other supervisor calls it makes are `GET /v0/role/status` (compare) and `GET /v0/role/attempts/{id}` (re-query), and neither starts a run. A lost answer is recorded as unmeasured with `stop_required`. The background `finish` holds the ledger lock until the record is appended. A coordinator crash mid-run leaves the in-flight marker, which blocks the next admission. No automatic retry was added.
- M-1: `new_session` is a PLAN deviation that the REPORT asks the reviewer to confirm. For `stale_generation`, it lets a start go ahead where compare would otherwise have answered `external_reconciliation_required`. `judge_resume` returns `stale_generation` before it checks `phase != after_run`, so an unfinished run (`takeover`/`error`) in an older generation is never reported as one. With `new_session: true`, the delivery then starts a new session over it. The supervisor's `start` path does not check the phase. Restrict it as described below and add a test. The `resume_allowed` + `new_session` half is fine.
- Supervisor: the replay record is durable before the run starts. Every crash window fails closed (stored "accepted" with no run → the coordinator's finish times out → unmeasured `stop_required`). The current-assignment check sits before the central compare and the tamper handling. A fresh tamper is recomputed on the next reconnect, so it is not lost. The STALE_GENERATION / EXTERNAL_RECONCILIATION_REQUIRED / NOT_OBSERVED mapping matches contract §14 item 12.
- Contracts: revision 11 §14 matches the code. OpenAPI 0.11.0 is additive and validates. The proto change is additive (buf breaking vs 3701f6b passes). One wording slip is in the internal contract (I-3).
- fmt, clippy and the full suite pass: 365/0 locally and in CI. Two of my budget-library mutations survive both test targets (L-1).
- Docs and ledger are accurate. The 114-database cleanup on 55432 is disclosed in the REPORT §6 and in `state.json` notes. The archive is byte-identical. The a349dc6 I-5 fix is partial (I-5).

## What I ran

- `cargo fmt --all -- --check`: ok. `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings`: rc 0.
- `RUST_TEST_THREADS=4 cargo test -j 4 --workspace --locked` with `IEO_TEST_DB_URL` on the throwaway DB on 55432: rc 0, **365 passed, 0 failed, 0 ignored** over 40 test binaries. This equals the REPORT, test.log and CI. /home stayed at 359 MB free, so the tests left no databases behind.
- `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml`: OK.
- `buf lint`, `buf build`, `buf breaking --against <repo>.git#ref=3701f6b,subdir=contracts/proto`: rc 0.
- Validators in `docs/poc` with `.venv-poc`: `validate_package.py` (`schema_errors` 0), `render_progress.py --check` (ok), `test_render_progress.py` and `test_summarize.py` (PASS).
- STATUS.md: main plan 12/73, follow-ups 0/12.
- Relative links in every changed .md resolve. The 02-05 `evidence` and `latest_report` paths exist. A secret grep over the changed files and the evidence (password, ghp_, github_pat, sk-ant, private keys) found only the archived review's own sentence about such a grep.
- `cmp` of the archived `reports/tasks/02-05/step2-supervisor-ops-20261115/reviews/review-pr73-a349dc6.md` against `pending-reviews/review-pr73-a349dc6.md`: identical.
- `git diff origin/main -- docs/poc/progress/state.json`: snapshot 0046 → 0047 and timestamps. 02-05 `latest_report`, `result_state`, +3 evidence, `next_action` and `unresolved` (a349dc6 L-2, the step-3 decisions to confirm). Two `notes` lines: main CI 43724114126 and the 114-database cleanup. One event (`02-05-S2-MERGED-20261116`). `resume` fields. Nothing else changed. Status stays 진행 and unverified.
- Mutations. Each was applied alone, followed by `cargo test -p ieo-supervisor --test caller` and `cargo test -p ieo-coord --test delivery_e2e`, and then reverted. `git status` was clean afterwards.

| # | Mutation | caller | delivery_e2e |
| --- | --- | --- | --- |
| M1 | `admit`: a `services_clear` refusal ignored (L-c3-3) | KILLED | survived (compare's `already_running` comes first) |
| M2 | `admit`: an attempt already in the ledger not refused | KILLED | survived |
| M3 | `Admission::lower`: `billable_room = b` (raises instead of only lowering) | **survived** | **survived** (L-1) |
| M4 | delivery: `new_session` also starts on `external_reconciliation_required` | n/a (passes) | KILLED (step 6, line 693) |
| M5 | delivery re-query: a stored answer accepted whatever its `request_sha256` | n/a (passes) | **survived** (L-1) |

M1 and M2 are killed by the library's own tests, which is where those checks live. That is enough, because the delivery path calls the same function.

## (1) Budget safety

- **Same checks, same code.** I compared the old `call_with` to the new `admit` + `start_with` + `finish`. The checks are, in order: role in the plan; op is start or reconnect (`call_with`); ledger lock with the `LOCK_WAIT` retry; in-flight marker; fail-closed `records()` (re-measured corrections); `asks_for_a_person`; attempt already recorded; `limits_for` (unmeasured → stop, the overage triple check, budget and role-share limits); `services_clear` (every plan role idle, its last attempt known, its checkpoint). Then `mark_inflight` before the send. A 202 becomes `Running`, which carries the lock. Any other answer clears the marker. A transport error goes to `record_lost` (unmeasured, `stop_required`, append, then clear). In `finish`, an append failure keeps the marker. All of it is unchanged in substance, the 32 caller tests pass unchanged, and M1/M2 are killed there.
- **No path around admission.** In `ieo-coord`, the only `POST` to a role service is the existing repo stop (`roles.rs`). `delivery.rs` reaches start/reconnect only through `Admission::start_with`. `roles::call` is used only for `GET /v0/role/status` and `GET /v0/role/attempts/{id}`. The latter takes the supervisor's `launch_gate` and only reads a file.
- **Limits.** `Admission::lower` only lowers the window and `billable_room`. A window not above the grace is refused, and `billable 0` is refused. The lowered limits go into both the body and `base_rec`. The "never raise" property has no test (L-1).
- **Background finish.** `Running` owns the lock file, and `finish(self)` drops it only after `append` + `clear_inflight`, or after `record_lost`. Double charging cannot happen: the re-query path never calls `finish` or `append`, and the same `attempt_id` is refused by `admit` ("already recorded") and by the DB (`ATTEMPT_ID_CONFLICT`). A charge cannot be lost either: a thread panic or a process exit drops the flock, but the marker stays, so the next `admit` refuses until a person reconciles. The marker gives the same protection even inside the process, so the lock being held for the whole run is belt and braces.
- **Lost answer.** `Sent::Lost` → `unknown` with the stop reason. The ledger has the unmeasured `stop_required` record, so the next delivery is `budget_refused` ("asks for a person"), as in e2e step 11. A later re-query that finds the attempt `accepted` does not change the ledger. That is correct under the user decision ① (no unmeasured run counted as 0, no auto retry).
- **Coordinator restart mid-run.** The marker blocks, as REPORT §5 says. Only the library test (`a_call_left_in_flight_blocks_until_reconciled`) covers this; there is no e2e kill mid-run (I-2).
- **Config two-source.** `budget_plan` takes the role URLs from the role configuration. A role missing there gives a null URL, `Plan::from_json` fails, and the delivery is `budget_refused` (fail closed). But `services_clear` covers only `budget_plan.roles`. A repo whose role configuration has both roles while `budget_plan.roles` names one is never checked for the other role being idle (L-2).

## (2) Delivery semantics vs PLAN

- **Request record.** Transaction 1 runs under the advisory lock `delivery:{repo}:{role}`. Same key and same body → replay. Same key and another body → 409. No head → 404. Another generation → 409 `STALE_ASSIGNMENT`. A reused attempt → 409 `ATTEMPT_ID_CONFLICT`, with a UNIQUE backstop. The coordinator record (including `task_id` and the revision limits) and `target_sha` come from the DB, and image and profile come from the role configuration. The body's keys are a closed set, so nothing in the request reaches the record. `evidence_level` is fixed to `verified` by a CHECK, and both tables are append-only by trigger.
- **Judgment.** `compare_checkpoint` is the same function as compareCheckpoint, in its own short transaction under the checkpoint lock. Every supervisor call is outside any transaction, and the result goes into transaction 2.
- **Decision → call.** `no_checkpoint` → start; `resume_allowed` → reconnect; `new_session` also → start on `resume_allowed`/`stale_generation`; anything else → `not_called`. `already_running`, `external_reconciliation_required` and `not_observed` can never be overridden (e2e step 6 sends `new_session: true` for these). The exception is the `stale_generation` masking in M-1.
- **Concurrency.**
  - Two deliveries with different keys for one budget: the second waits up to `LOCK_WAIT` (3 s) and then gets `budget_refused` ("another caller holds the ledger"), or the in-flight marker. That is sound for the budget.
  - Compare runs before admission, so a delivery that obtains the lock after another run finished within those 3 s acts on a stale judgment (L-3).
  - A same-key replay can overtake an original that is still in flight (L-3).
- **Re-query.** It is read-only and records the next `n` with `requeried`. The digest comparison relies on both sides serialising a `serde_json::Value` the same way: the workspace has no `preserve_order` feature, so keys are sorted. e2e step 11 shows that the `sent_sha256` matches. The "other request under this attempt" branch is not tested (see M5).
- **The worker pool.** A delivery holds an HTTP worker and its DB connection for the whole supervisor call. Over the HTTP transport, the supervisor's `getOwnAssignment`/`latest` callbacks need another free worker (I-1).

## (3) Supervisor

- **Replay storage.** The order is: replay check (before validation, the running check and the assignment check, so a resend never starts anything); then all refusals; then `budget_backstop`; then a `durable_write` of `attempts/<id>.json`; then `record_attempt`; then the spawn, all under `launch_gate` and the state lock.
  - Crash after the stored answer and before the spawn: `GET` says accepted although nothing ran. The coordinator's `finish` then times out to unmeasured `stop_required`, which is conservative.
  - `record_attempt` failure removes the stored answer. If that removal fails, the answer stays, which is also conservative.
  - A same-content resend answers 200 `replayed`. The shared library treats any non-202 as `Refused` and clears its marker, so if a caller ever resent a stored attempt, the delivery would record `refused` for an attempt that ran. That cannot happen with one ledger (`admit` refuses known attempts); see I-4.
- **ID_CONTENT_CONFLICT** covers another digest, another op, an unreadable stored answer, and the older `runs/<id>` check.
- **Current-assignment check order.** The order is: ALREADY_RUNNING, then `own_assignment`, then `central::compare`, then the phase and tamper logic, then pending-tamper. Refusing before tamper detection loses nothing, because the detection is recomputed from the config dir on the next reconnect. The earlier central-compare refusal already behaved that way, and the X-08 pending-file rules are untouched.
- **Mapping.** Same assignment and an older generation → `STALE_GENERATION`. A newer generation, another assignment, no head or any refusal → reconcile. No answer → `NOT_OBSERVED`. `local_only` skips the check. Image and profile are compared only when a delivery exists (`expected_runtime`).
- **No `target_sha` check.** This is acceptable: takeover storage condition (b) refuses a mismatched SHA before anything is stored as a takeover, and step 7 makes GitHub the final ground truth.

## (4) Contracts

- Runtime contract revision 11: the header says what changed (§7 table row, new §14 items 11–14). Item 11 matches `serve.rs`, including "refusals not stored". Item 12 matches `check_assignment` and its position. Item 13 matches `delivery.rs`, except that the sentence "같은 전달 요청을 다시 보내면 … 항목 11의 조회만 한다" holds only when the stored result is `unknown` or missing (I-3). Item 14 matches the tests.
- OpenAPI 0.11.0: additive (the only removed lines are the version, one description and two `required`/enum lines that were extended). The request schema, order sentence, outcomes and `ATTEMPT_ID_CONFLICT` match the code.
- Proto: field 16 and `RuntimeExpectation`. The two converters round-trip it (unit test), and buf breaking passes.
- Internal contract: §1.2 says deliverAssignment needs "proto 변경 없음", while §14 records the `expected_runtime` addition (I-3).

## (5) Tests

- `delivery_e2e` runs once per supervisor transport and covers every REPORT §3 row I checked against the code: auth and format, stale, start with DB-built record and limits, replay and conflicts, reconnect and C1, supervisor-side mismatches, compare refusals, stale vs `new_session`, the two budget refusals, coordinator restart, refused, lost refusal, lost acceptance and re-query. It ends by counting runs, rows, `accepted` and checkpoints.
- C12 tests (`an_empty_memory_directory_is_not_a_tamper`, `an_unreadable_file_outside_the_directive_scope_is_never_opened`) match f73ac13 P3 and bc55edf R5.
- a349dc6 L-1: the three direct assertions (equal-time stale, impossible dates, another session's takeover) are in `grpc.rs`.
- The REPORT's 16 mutants are plausible, and my M1, M2 and M4 agree with its claims. M3 and M5 expose gaps (L-1).

## (6) Docs and ledger

- REPORT is accurate about behavior, deviations (new_session, read-only re-query, budget stop after a lost answer, background finish, no target_sha), limits and counts (365 = 358 + 7).
- The environment note (114 `ieo_t_*` databases dropped on 55432, including other sessions' finished ones; 55433 and 58090 untouched) is in REPORT §6 and `state.json` notes.
- The ledger keeps 12/73 and 0/12, status 진행 and unverified. The new `unresolved` clause lists exactly the step-3 decisions that review should confirm.

## Findings

### M-1 (Medium): `new_session` on `stale_generation` skips the "previous run did not finish" reconciliation

PLAN §3 step 3 says `stale_generation` calls nothing. The PR adds `new_session: true`, which starts on `resume_allowed` and `stale_generation` (REPORT §5 asks for this to be confirmed).

The `resume_allowed` half is safe: that state already implies `phase == after_run` and matching digests.

The `stale_generation` half is not:
- `judge_resume` (api.rs ~4058–4078) returns `stale_generation` as soon as the central checkpoint is an older generation of the same assignment. That check comes **before** `if c["phase"] != "after_run" → external_reconciliation_required`.
- So a central/local checkpoint at `takeover` or `error` (a run that never reached its end: children, external requests and the session record not observed, runtime contract §6 and §13 item 10) in generation g−1 is reported as `stale_generation`, never as reconciliation.
- `delivery.rs:396` then maps it to `start`.
- The supervisor's start path checks the phase only for reconnect (serve.rs ~762). `central::compare` checks only seq and digest. The start therefore runs.

The budget gate usually also holds such a state: a run whose finish failed leaves `stop_required` or the marker. But a person's budget correction reconciles usage, not the unfinished run's external effects. The request text for this review says `new_session` should not bypass compare's refusals, and in this one masked case it does.

Suggested fix (small, no API change): in `deliver`, allow `new_session` on `stale_generation` only when `judged.body["central"]["phase"] == "after_run"`. Otherwise record `not_called` with an `external_reconciliation_required`-style reason. Alternatively, have `judge_resume` add a reason that the delivery checks. Add an e2e or unit case: older-generation central checkpoint at `takeover` + `new_session: true` → not called. Also note the deviation in PLAN-facing text (REPORT §5 already does) and in contract §14 item 13.

### L-1 (Low): two budget-relevant properties have no test (surviving mutations M3, M5)

- M3: `Admission::lower` with `billable_room = b` (raising instead of lowering) passes both `caller` and `delivery_e2e`. The e2e revision limit (100,000) is below the plan room, so `min` and `=` agree. This is the property that keeps a revision's `total_tokens` from lifting a run above the remaining budget or role share. Add a unit test in `caller.rs` for `lower`: revision above the room → unchanged; below → lowered; window above the full window → unchanged; window ≤ grace → refused; `billable 0` → refused.
- M5: the re-query's guard `v["request_sha256"] == sent_sha256` (delivery.rs:531) can be replaced by `true` and `delivery_e2e` still passes: no case has the supervisor hold another request under the delivery's `attempt_id`. That guard is what stops a re-query from recording `accepted` (and a `verified` delivery) for a run the coordinator did not send. Add a case: store a different accepted answer for the attempt on the role volume (or start it directly with other content), lose the delivery's answer, re-query → `unknown` with "another request", not `accepted`.

### L-2 (Low): `budget_plan.roles` and the coordinator's role configuration can disagree

The delivery path's plan takes URLs from `ctx.roles[repo]` only for the roles that `budget_plan.roles` names. `services_clear` checks only those roles. If an operator configures both roles but `budget_plan.roles` names only `development`, then:
- a `guardrail` delivery is refused (fail closed);
- a `development` delivery is admitted without checking that the guardrail service is idle and its last attempt known (X-08 L-c3-3's "every service").

Suggested fix: at coordinator start (or in `budget_plan()`), require `budget_plan.roles` keys to equal the repo's configured role entries, and refuse otherwise.

### L-3 (Low): two concurrency windows in the delivery record (no budget effect)

- **Stale judgment after the lock wait.** Compare runs before `admit`. Delivery B can wait up to `LOCK_WAIT` (3 s) for A's lock, then get it after A's run finished and was recorded. B then sends the op it judged earlier. If B judged `no_checkpoint`, it starts a new session although a resumable checkpoint now exists, which amounts to an implicit `new_session`. With real runs (> 3 s) this is unlikely. Fix: re-run `compare_checkpoint` after `admit` succeeds and before `start_with`, or make LOCK_WAIT 0 for the delivery path.
- **Replay overtaking an in-flight original.** A client that retries with the same key while the original is still between transaction 1 and its supervisor call sees `result: null`, so it re-queries. The supervisor has no stored answer yet, so B records and returns `not_accepted` (a final-looking outcome), and then A's run is accepted. Usually A's `accepted` row comes later and wins. If A's accept and record fall between B's GET and B's insert, `not_accepted` stays newest for good, and later replays return it as final. The budget is unaffected (A's finish charges; B starts nothing). Fix: in `record_result` for re-query rows, re-read the newest result under the delivery lock and keep it if it changed since `load`. Also, answer a `result: null` replay that is younger than the call timeout with an "in progress" answer instead of re-querying.

### I-1 (Info): a delivery occupies an HTTP worker for the whole supervisor call

`--workers` defaults to 4. Over the HTTP transport, the supervisor's start needs the coordinator to answer `getOwnAssignment` and `latest` while the delivery's worker waits. With `--workers 1`, or four concurrent deliveries, the callbacks queue until the supervisor's client times out. That gives `NOT_OBSERVED` (refused, nothing runs), or a lost answer if it exceeds the adapter's 30 s, which fails closed but stops the budget. Suggest a note in the operations doc (workers ≥ 2), or reject `--workers 1` when `--roles` has `budget_plan`.

### I-2 (Info): coordinator kill mid-run is covered only at library level

REPORT §5 says that a restart during a run leaves the marker and blocks the next delivery. `delivery_e2e` restarts the coordinator only between runs. The library test covers the marker semantics, so this is a coverage note, not a gap in behavior.

### I-3 (Info): wording

- `contracts/ieo-internal-v0.md` §1.2: "RPC를 두지 않는다(proto 변경 없음, …)". This PR does change the proto (`expected_runtime`), as §14 states. Suggest "RPC는 없다(이 작업의 proto 변경 없음)".
- Runtime contract §14 item 13 "같은 전달 요청을 다시 보내면 코디네이터는 항목 11의 조회만 한다": this applies only when the stored result is `unknown` or missing; otherwise the stored delivery is returned.
- OpenAPI `refused` is described as "the supervisor's 4xx/5xx", but any non-202 (including a 200 `replayed`) is recorded as `refused` (I-4).

### I-4 (Info): a 200 `replayed` answer is treated as a refusal by the shared library

`Admission::start_with` treats every non-202 answer as `Refused` and clears the in-flight marker. With one ledger this cannot happen (`admit` refuses an attempt already recorded, and every accepted attempt is recorded or leaves the marker). With two callers on different ledger directories reusing an `attempt_id`, the second would record `refused` for an attempt that ran under the other ledger. That is harmless for the budget, but misleading. Optionally map 200 + `replayed: true` to "already accepted elsewhere" with its own reason.

### I-5 (Info): a349dc6 I-5 is only partly addressed

The commit message says I-5 is addressed. Of its three bullets, only the `sprints` → `sprint_revisions`·`notion_imports` wording was changed:
- OpenAPI `reportSessionStatus.session_id` still lacks `minLength: 1` (line ~949);
- step-1 REPORT §5 still says "이 보고는 원장(`docs/poc/progress/*`)을 바꾸지 않는다" (line 83).

These are Info items, but the commit message should not claim them.

## Summary for the author

- Request changes at c11e734 for M-1 only: gate `new_session` on `stale_generation` by the central phase (`after_run`), and add the test.
- Recommended in the same round: L-1 (unit test for `lower`; an e2e case for the re-query digest guard), L-2 (plan roles = configured roles).
- L-3 and I-1 to I-5 can go in this PR or be tracked for step 4.
- Everything else, including the budget library split, the supervisor replay and assignment check, contracts, tests and ledger, I found correct.
