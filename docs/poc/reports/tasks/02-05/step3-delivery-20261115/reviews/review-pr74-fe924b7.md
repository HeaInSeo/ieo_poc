# Review of PR #74 at fe924b7 (review 2)

- PR: HeaInSeo/ieo_poc#74, 02-05 step 3: coordinator `deliverAssignment` through `compare` (start/reconnect), the shared budget-check library, supervisor same-attempt replay and current-assignment check, runtime contract revision 11, OpenAPI 0.11.0. This is review 2 of at most 4, after correction 1 of 3 (review c11e734: REQUEST CHANGES).
- Head: fe924b786aae172e0f8cc75fefd467b2417e4232. I confirmed it with `gh pr view 74 --json headRefOid,statusCheckRollup` before and after the review.
- Base: main 3701f6b (merge base). Three commits were added after c11e734:
  - 3f1b0a1: code, contracts and tests (the review c11e734 fixes);
  - 8f8910a: REPORT §5–§7, evidence (test log at 3f1b0a1, mutants);
  - fe924b7: archived review c11e734, ledger.
  - `git diff 3f1b0a1 fe924b7 -- crates contracts` is empty, so the evidence applies to the head.
- CI at fe924b7 (run 43732098811, pull_request). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS, 367 passed and 0 failed in the job log
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan worktree, s3wt, PostgreSQL on 55433 or the coordinator on 58090.
  - I worked in my own detached worktree (r74b) with the shared target directory target-x08.
  - At the end I removed the worktree and ran `cargo clean -p` for all seven workspace packages from an fe924b7 tree, so no workspace test binaries are left behind.
  - Two of my mutants left `ieo_t_dlv_*` databases on 55432. I dropped exactly those two by name and removed their two `/tmp/ieo-dlv-e2e-*-<pid>` directories. No other `ieo_t_dlv_*` database existed before or after.

## Verdict

**APPROVE. No Blocking or Medium findings. One Low and six Info findings.**

- Every review-1 finding is closed:
  - M-1, L-1, L-2, L-3a, L-3b, I-3, I-4 and I-5 are fixed in code or text, with tests where the finding asked for them.
  - I-1 and I-2 are recorded in REPORT §6, as I suggested.
- The M-1 fix changes `judge_resume` itself, and therefore compareCheckpoint, which 02-04 accepted. The change is on the closed side and only relabels one case: an older generation of the same assignment whose last record is `takeover`/`error` is now `external_reconciliation_required` instead of `stale_generation`.
  - Neither label ever started or resumed anything without `new_session`, so the effect is a different decision name and reason, and that `new_session` cannot start over such a run.
  - Every other outcome is unchanged: the new unit test (7 cases), `resume_judgment_compares_local_central_and_assignment` and both `checkpoint_e2e` variants pass unmodified.
  - It is documented consistently in runtime contract revision 11 (header, §13 item 10, §14 item 13), in OpenAPI compareCheckpoint and deliverAssignment, and in REPORT §5 and §7.
  - However, REPORT §5's last bullet still says no existing operation's HTTP behaviour changed (L-1 below). **The user should be told plainly** that this PR amends an accepted 02-04 judgment. The amendment is closed-side and needs no decision, but it is a change to accepted behaviour.
- `DELIVERY_IN_PROGRESS` (120 s) is safe. Run length does not matter, because the result is recorded when the 202 arrives, not when the run ends. A delivery cannot block forever, and a replay can never start a run (details in (c)). The surviving mutant R-L3b2 is acceptable.
- fmt, clippy, buf (lint, build, breaking vs 3701f6b), the OpenAPI validator and the docs validators pass. The full suite passes 367/0 locally with the test DB and in CI.

## What I ran

- `cargo fmt --all -- --check`: ok. `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings`: rc 0.
- `RUST_TEST_THREADS=4 cargo test -j 4 --workspace --locked` with `IEO_TEST_DB_URL` on 55432: rc 0, **367 passed, 0 failed, 0 ignored** over 40 test binaries, with no SKIP lines.
  - That is 365 plus `api::tests::an_unfinished_run_of_an_older_generation_is_reconciliation` and `caller::tests::lower_never_raises_a_limit`.
  - It equals REPORT §7.3 and CI.
  - /home went from 347 MB to 346 MB free.
- `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml`: OK.
- In `contracts/proto`: `buf lint`, `buf build`, and `buf breaking --against <repo>.git#ref=3701f6b…,subdir=contracts/proto`: rc 0.
- In `docs/poc` with `.venv-poc`:
  - `validate_package.py`: `schema_errors` 0;
  - `render_progress.py --check`: ok;
  - `test_render_progress.py`: PASS.
- STATUS.md: main plan 12/73, follow-ups 0/12.
- Relative links in every .md changed since 3701f6b resolve. A secret grep over the added lines (ghp_, github_pat, sk-ant, private keys, `password=`) found nothing.
- `cmp` of the archived `reports/tasks/02-05/step3-delivery-20261115/reviews/review-pr74-c11e734.md` against `pending-reviews/review-pr74-c11e734.md`: identical.
- Mutations. Each was applied alone, run against the named target, and then reverted (`git status` was clean afterwards).

| # | Mutation | Target | Result |
| --- | --- | --- | --- |
| X1 | `choose_op`: `stale_generation` + `new_session` starts whatever the central phase | delivery_e2e | survived (expected: redundant with `judge_resume`, I-1) |
| X2 | `judge_resume`: the new check only for `takeover` (an `error` older generation stays `stale_generation`) | `--lib unfinished` | KILLED |
| X3 | re-query: an answer without a recorded digest counts as `accepted` (`is_none_or`, the c11e734 code) | delivery_e2e | **survived** (I-2) |
| X4 | L-2: plan roles only a subset of the configured roles | delivery_e2e | KILLED (step 9, line 964) |

## (a) Review-1 findings

| Finding | Status | Checked against the code |
| --- | --- | --- |
| M-1 `new_session` over an unfinished older generation | **Fixed** | `judge_resume` (api.rs:4059–4067) returns reconciliation for an older generation of the same assignment with `phase != after_run` before the `stale_generation` branch. `choose_op` (delivery.rs:567–575) also requires `central.phase == after_run` (belt and braces, X1). Tests: unit (7 cases), e2e step 7 (older-generation takeover + `new_session` → `external_reconciliation_required`, `not_called`, reason "did not finish"; then after_run → `stale_generation` / start). |
| L-1 `lower` and re-query digest untested | **Fixed** | `lower` now checks on a copy and assigns at the end, so a refusal changes nothing. `lower_never_raises_a_limit` covers above-room/window → unchanged, below → lowered, `timeout_is_total` set, window ≤ grace and `billable 0` refused, a combined refusal leaves the limits as they were. e2e step 10 stores another accepted request under `a41`, drops the answer, re-queries → `unknown` "another request", `n = 2`, `requeried`. R-L1a/R-L1b killed (my M3/M5). |
| L-2 plan roles ≠ configured roles | **Fixed (closed side)** | `budget_plan` compares the plan's role names (sorted map keys) with `ROLES` filtered by configured objects (ROLES is in sorted order: development, guardrail). A mismatch is `budget_refused`. The e2e now runs a real guardrail service and puts it in the plan, so `services_clear` asks both. Step 9 checks the refusal. X4 is killed. |
| L-3a stale judgment after the lock wait | **Fixed** | See (d). |
| L-3b replay overtaking an in-flight original | **Fixed** | See (c). |
| I-1 worker occupancy | Recorded | REPORT §6 (workers ≥ 2), not enforced, as suggested. The "10 s" there is the supervisor's `CALL_TIMEOUT` (central.rs:31) and the "30 s" is the adapter's timeout (caller.rs:184). Both are correct. |
| I-2 kill mid-run only at library level | Recorded | REPORT §6. |
| I-3 wording | **Fixed** | Internal contract §1.2 ("이 작업을 위한 proto 변경 없음", with the `expected_runtime` pointer). Runtime contract §14 item 13 (stored delivery returned; re-query only for `unknown` or no result after 120 s; digest; re-judgment; role equality). OpenAPI `refused` (any non-202, including 200 `replayed`). |
| I-4 200 `replayed` as refusal | **Addressed** | See (d). |
| I-5 a349dc6 I-5 partial | **Fixed** | OpenAPI reportSessionStatus `session_id` has `minLength: 1`. Step-1 REPORT §5 now names snapshot 0045, which I checked: `089fb95:state.json` is `v1.3.3-EXEC-0045` and its parent is 0044. A sibling gap remains (I-6). |

## (b) M-1: the change to `judge_resume`

- **What changed.** One branch. If `c.assignment_id == head.aid && cgen < generation && c.phase != "after_run"`, the result is `external_reconciliation_required` with the reason "the last run of the older generation g did not finish (phase …)". All earlier checks run before it unchanged: not observed, running, the local/central seq and digest, and no head. So it is reached only when local and central agree byte for byte, and REPORT §7.1's remark that the local phase equals the central phase holds.
- **Closed side.**
  - In 02-04, `stale_generation` never caused an action. compareCheckpoint is read-only. The supervisor refuses reconnect with `STALE_GENERATION` on its own check. The documented recovery (02-04 PROGRESS §7, e2e 11) is a new session in the new generation, and that recovery was always an `after_run` case.
  - For the current generation, an unfinished run was already reconciliation, and for reconnect the supervisor refuses `takeover`/`error` (serve.rs:762–771). The new branch only makes the older generation consistent with that rule and with runtime contract §6 / §13 item 10 ("마지막 기록이 `after_run`이 아니면 재개하지 않는다").
  - The 02-04 decision item 9 says "낡은 세대에는 계약에 정한 조치". The contract is where this is fixed, and revision 11 records it.
- **Other outcomes unchanged.**
  - The unit test pins: older/after_run → stale, older/takeover|error → reconcile, same/after_run → resume, same/takeover → reconcile, newer → reconcile, other assignment → reconcile.
  - The pre-existing `resume_judgment_compares_local_central_and_assignment` (coordinator.rs:3576) and both `checkpoint_e2e` variants, with their `stale_generation` assertions at steps 8 and 11, pass without modification. Both of those steps have an `after_run` central record.
  - 02-04 PROGRESS rows 128/136/194 remain true as written.
- **Documentation.**
  - Runtime contract revision 11: the header sentence, the §13 item 10 parenthesis and the §14 item 13 text agree.
  - OpenAPI: the compareCheckpoint description defines `stale_generation` as "whose last run finished (`after_run`)"; deliverAssignment repeats it.
  - REPORT: §5 has a bullet "재개 판단 하나를 바꿨다", and §7.1 has the M-1 row.
  - `state.json` 02-05 `result_state`: "compare가 끝나지 않은(takeover·error) 옛 세대를 external_reconciliation_required로 판단(02-04 판단의 닫힌 쪽 변경)".
  - Inconsistent: REPORT §5's last bullet (L-1).
- **Report to the user: yes.** No decision is needed, because the change is closed-side and inside the contract's own scope. But compareCheckpoint is an accepted 02-04 operation, and its answer changes for one input class. The merge report should say so in one sentence, for example: "02-04에서 수용한 compareCheckpoint 판단을 하나 바꿨다: 같은 배정의 낮은 세대라도 마지막 기록이 takeover·error면 stale_generation 대신 external_reconciliation_required. 닫는 쪽 변경이며 다른 판단은 그대로다."
- **Not covered by M-1 (pre-existing, I-4).** The supervisor's `start` path still checks no checkpoint phase, in any generation. A start sent directly to the supervisor, for example by the 02-04 host caller, can still begin a new session over an unfinished run. The delivery path no longer can.

## (c) L-3b: `DELIVERY_IN_PROGRESS` and the 120 s window

- **What the window measures.** It is `now() - requested_at`, where `requested_at` is set in transaction 1, up to the recording of a result. The result is recorded right after the supervisor's answer to `start`/`reconnect` (202 or refusal). The run itself is finished, measured and charged in a background thread. So the 600 s run windows play no part: a run in progress has a result (`accepted`) long before it ends.
- **Upper bound of the original's in-flight time.** Two `GET /status` calls (10 s each, roles.rs:26), the ledger lock wait (`LOCK_WAIT` 3 s), the start call (adapter timeout 30 s, caller.rs:184) and short transactions. That is about 55 s, well below 120 s.
  - The HTTP path has no `statement_timeout` (`NoGuard`), so an advisory-lock wait is unbounded in principle.
  - In practice the `checkpoint:` and `delivery:` locks are held only by short transactions. A crash between transaction 1 and transaction 2 leaves no result, which is the case the 120 s timeout exists for.
- **Cannot block forever.** After 120 s a replay re-queries (`GET /v0/role/attempts/{id}`) and records the answer. A delivery whose first or second judgment returned a non-200 also has no result. Its replay gets 409 for 120 s and then a re-query (`not_accepted`), which is accurate.
- **Cannot let a duplicate through.** A replay never calls `start`/`reconnect`; it only reads. The budget is unaffected either way: only the original goes through `admit`/`start_with`, and `finish` charges it. If the original somehow exceeded 120 s:
  - The re-query could record `not_accepted` (n), and the original's `accepted` (n+1) would then be the newest, which is correct.
  - If the original's result lands between the re-query's read and its insert, the new `expect_n` check keeps the original's row newest (record_result, delivery.rs:333–345).
- **The re-query without a digest** (the coordinator stopped before transaction 2): a 200 is now `unknown` "no recorded request digest", not `accepted`. That is closed-side but untested (X3, I-2).
- **R-L3b2 (the `expect_n` check removed) survives. Acceptable.** It needs the original to stay in flight beyond 120 s and to write in a sub-millisecond window. No deterministic test can produce that without a 120 s test. The 409 that closes the reviewed window is tested (R-L3b1 killed, e2e 6b). The check itself is three lines and plainly correct by reading: it compares `max(n)` under the same `delivery:` lock that every result insert takes. The worst case without it is a wrong newest label, never a run or a charge.

## (d) L-2, L-3a, I-4

- **L-2 closed side.** A plan with fewer roles than configured is `budget_refused` before any supervisor call (e2e step 9, launches unchanged). A plan with a role that is not configured is refused too (`names != configured`). Earlier it failed through a null URL in `Plan::from_json`. Both directions fail closed.
- **L-3a second judgment under the lock.**
  - After `admit` succeeds (the ledger lock is held, no in-flight marker), `judge` runs again. That means a fresh `GET /status` plus `compare_checkpoint` in its own transaction.
  - If the status is not 200, or the decision or the chosen op differs, the admission is dropped (the lock is released and no marker was set, since `mark_inflight` happens only in `start_with`) and `not_called` is recorded. The first reason says "the judgment changed while the budget check waited (a → b)".
  - Every budgeted run (a delivery or the host caller on the same ledger) holds that lock from admission until its record is written. So no run on this ledger can start or finish between the second judgment and the call, which closes the review-1 window.
  - The comparison is by decision and op, not by central seq. With the lock held, a seq change can only come from outside the budget (a person), and the supervisor's own compare still guards the call.
  - e2e 6b holds `caller.lock`, changes the local checkpoint, releases the lock, and gets `external_reconciliation_required` / `not_called` / "judgment changed" with no launch. R-L3a is killed.
- **I-4 replayed-as-refusal.**
  - The semantics are unchanged: any non-202 is `Sent::Refused`, the marker is cleared, and nothing is recorded in the ledger.
  - A 200 with `replayed: true` now carries its own reason: "accepted earlier, not by this call". The OpenAPI says the same.
  - Clearing the marker is correct, because this call started nothing.
  - Not recording a ledger entry is correct for this ledger. With one ledger, `admit` refuses a known attempt first, so the 200 can only come from a caller on another ledger, and that ledger's own `finish` charges the run.
  - No test covers the new reason. It is text only.

## (e) Regressions

- The 32 `caller` tests, `checkpoint_e2e` (2) and the coordinator judgment test pass unchanged.
- `lower`'s refactor keeps the old results for accepted inputs (the same `min` operations, now on a copy).
- The e2e totals: 19 deliveries, 22 results, 5 `accepted`, 5 runs, 12 checkpoint rows. They match REPORT §7.3 and the assertions at delivery_e2e.rs:1136–1148.
- The guardrail service in the e2e is `local_only` and stays idle (`share` max_runs 2, never called). Step 6's `not_observed` case now uses a `fail_status` switch (503) instead of an unreachable port, so the guardrail is observable for the budget check and unobservable only for that one judgment.

## (f) Docs and ledger

- REPORT §5–§7 match the code, with the exception of L-1.
  - §7.1 (one row per finding), §7.2 (changed contracts) and §7.3 (367/0, 23/24 mutants with R-L3b2 explained, the stale `ieo-evidence` binaries note) are accurate.
  - The `mutants.txt` header names 3f1b0a1 and the targets, and the evidence `test.log` header names 3f1b0a1 with only REPORT.md dirty.
- The ledger note "단계 3 교정 중 시험 DB(55432)에서 자기 변이 실행이 남긴 ieo_t_dlv_* 데이터베이스 26개를 지움(…그 이름은 이 시험만 씀)" is plausible and consistent with the code:
  - only `delivery_e2e.rs:285` creates `ieo_t_dlv_{transport}_{pid}`;
  - the test drops its database only on success, so each killed e2e mutant leaves two;
  - 13 e2e-killed runs give 26.
  - I cannot verify the count after the fact. None remained when I started.
- `git diff 3701f6b fe924b7 -- docs/poc/progress/state.json`:
  - top-level changes only in `snapshot_id` (0046 → 0047), the two timestamps, `tasks`, `notes`, `events` and `resume`;
  - since c11e734, only the 02-05 `result_state` (correction 1 summary, 367, 23/24), +1 evidence (the archived review) and +1 `notes` line (the 26 databases).
  - Status stays 진행 and unverified, with 12/73 and 0/12 in STATUS.md. Nothing unintended.
- The archive is byte-identical, the links resolve and there are no secrets (see "What I ran").

## Findings

### L-1 (Low): REPORT §5 still says no existing operation's HTTP behaviour changed

REPORT §5, last bullet: "사용자 결정이 필요한 것은 없다고 본다. 정책 파일·판정 규칙·기존 작업의 HTTP 동작은 바꾸지 않았다."

The bullet just above it says compareCheckpoint's judgment changed. compareCheckpoint is an existing, accepted 02-04 operation, and its HTTP answer (`decision`, `reasons`) differs now for an older generation at `takeover`/`error`.

Suggested text: "…기존 작업의 HTTP 동작은 compareCheckpoint 판단 하나(위, 닫는 쪽) 말고는 바꾸지 않았다".

In the merge report to the user, state the amendment explicitly. No decision is needed, but it must not read as "nothing accepted changed". It can be fixed in this PR if another correction happens, or in the step-4 PR's REPORT. It does not block.

### I-1 (Info): the `choose_op` phase guard is redundant (X1 survives)

`judge_resume` already turns an unfinished older generation into reconciliation, so `("stale_generation", true) if central.phase == "after_run"` can never see another phase. This is the intended double check (REPORT §7.1 "이중 확인") and no test can kill it. Not a gap.

### I-2 (Info): the no-digest re-query branch has no test (X3 survives)

Restoring the c11e734 `is_none_or` (a stored answer counts as `accepted` when the delivery recorded no digest) passes `delivery_e2e`. That situation needs the coordinator to stop between the supervisor's answer and transaction 2. The new behaviour is the closed side and has no budget effect: the in-flight marker or `stop_required` already holds the budget. A test would need a coordinator kill at that point, so I suggest tracking it with review-1 I-2 rather than adding it now.

### I-3 (Info): R-L3b2 accepted as untestable

See (c). No action.

### I-4 (Info): the supervisor's own start path does not refuse an unfinished checkpoint

This is pre-existing (02-04), not introduced here. `serve.rs` refuses `takeover`/`error` only for `reconnect`. After this PR the delivery path never starts over an unfinished run, in either generation. A direct `start` to the supervisor (the 02-04 host caller, or anyone holding the adapter credential) still can. If deliveries are meant to be the only path for future real runs, consider the same refusal on `start` (unless `new_session` semantics are wanted there), or a note in the step-4 plan.

### I-5 (Info): the 120 s bound assumes short database transactions

The HTTP path sets no `statement_timeout`, so the original delivery's in-flight time is bounded by its HTTP timeouts (about 55 s) only while the `checkpoint:`/`delivery:` advisory locks are held briefly, as they are today. If a later step adds a long transaction under those locks, revisit `IN_FLIGHT`, or bound the delivery's transactions.

### I-6 (Info): reportSessionStatus `assignment_id` lacks `minLength: 1`

Like `session_id` before I-5, the code (`nullable_str` → `str_field`) refuses an empty `assignment_id`, but the OpenAPI schema (line 963) allows it. Add `minLength: 1` at the next OpenAPI change.

## Summary for the author

- Approve at fe924b7. M-1 and every Low from review 1 are fixed and tested. The Info items are recorded or addressed.
- Before or at merge: fix the REPORT §5 sentence (L-1), and tell the user in plain words that compareCheckpoint's accepted 02-04 judgment changed on the closed side (an older generation's unfinished run → `external_reconciliation_required`).
- Track for step 4: I-2 (a test for a coordinator stop before transaction 2), I-4 (refusing `start` over an unfinished checkpoint at the supervisor), and I-6.
