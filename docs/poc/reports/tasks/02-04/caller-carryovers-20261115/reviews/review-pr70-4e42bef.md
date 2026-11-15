# Review of PR #70 at 4e42bef (review 1)

- PR: HeaInSeo/ieo_poc#70, "X-08: apply user decisions 2026-11-15 (refuse on unreadable pending tamper file, Q-3 as contract limit); record 02-05 decisions — snapshot 0043"
- Head: 4e42bef20b5db487a4f5595d9f6b41839c2642b7 (confirmed with `gh pr view 70 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main c5464f5 (c5464f5f9871…), which is also the merge base. Three commits on top: 18d5d9e code, tests and contract text; a979f1e REPORT §4 and test log; 4e42bef decision record, ledger snapshot 0043 and the archived PR #69 review.
- CI at 4e42bef (run 43717025897, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan, d05wt or x08wt worktrees, or the PostgreSQL on 55433. I used my own detached worktree and target directory and removed both at the end. This is review 1 of at most 4.

## Verdict

**REQUEST CHANGES: two Medium, three Low and five Info findings. No Blocking findings.**

- The code change is correct. Every `start` and `reconnect` that has an unreadable or non-object pending file and no fresh detection is refused with 409 `EXTERNAL_RECONCILIATION_REQUIRED`. The file is kept. A missing file still means "no pending". The new test runs over HTTP and gRPC, and it fails when the fix is reverted.
- Refusing `start` goes beyond the literal answer "재개 거절", but the reason is sound and the PR body and REPORT say so. The decision record does not mention it (L-1).
- **M-1:** the decision record says the budget is "계획 §4.3의 표 그대로", but it drops the per-role shares and the budget-check row. The ledger `resume.budget` drops them too.
- **M-2:** the text of runtime contract revision 9 was changed after 02-04 was accepted on it. The revision number was not raised and the header has no note about the change. The new text adds a refusal rule, not only an explanation.
- Ledger otherwise correct: 12/73 and 0/12; X-08 still 진행; 02-04 spare runs unused; X-08 검수 완료 still gates real calls and X-02; F4 written into 04-07 and 05-01..05-03 and not applied to code; S1 recorded; the archived PR #69 review is byte-identical; PR #69 L-1 handled through the 02-05 `result_state`.

## What I ran

- `cargo test --locked --all-targets -j 4` with `IEO_TEST_DB_URL` on the throwaway DB on 55432: rc 0, **350 passed, 0 failed, 0 ignored** across 32 binaries. This matches the PR body and `evidence/test.log`. `an_unreadable_pending_file_stops_until_a_person_reconciles_it::{http,grpc}` both ok.
- `cargo fmt --all -- --check`: rc 0. `cargo clippy --locked --all-targets -- -D warnings`: rc 0.
- Validators in `docs/poc` with `.venv-poc`: `validate_package.py` rc 0 (`schema_errors` 0); `render_progress.py --check` ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl); `test_render_progress.py` PASS.
- Relative-link check over every changed `.md` file: no broken links.
- `cmp` of `reports/tasks/02-04/caller-carryovers-20261115/reviews/review-pr69-d0f72ec.md` against `pending-reviews/review-pr69-d0f72ec.md`: identical.
- Secret grep over the whole diff (sk-ant, ghp_, github_pat, private keys, `password=`, token assignments): the only hit is the archived review's sentence describing its own grep.
- Structural diff of `state.json` against c5464f5 (listed in item 3).
- My own mutation spot-checks. Each was applied alone, the named test run, and the file restored. `git status` was clean afterwards:

| # | Mutation | Result |
| --- | --- | --- |
| M1 | `launch` pending read back to the old `.ok().and_then(parse).filter(is_object)` ("no pending") | KILLED: `an_unreadable_pending_file_stops_until_a_person_reconciles_it` (http and grpc) |
| M2 | refusal skipped when `op == "start"` (non-object case) | KILLED: same test, line 2075 (the `start` assertion), http and grpc |
| M3 | coordinator `latest.tamper_recorded` always false | KILLED: `central_checkpoints_keep_sequence_identity_and_tamper_reports` at the new assertion (coordinator.rs:3485). This closes review 3724bc9 I-1, which survived before. |

## Item 1: the refusal

### Code paths

- `launch` (serve.rs) is the only path that starts a run (HTTP and gRPC both go through it; the other `Job {` is in a unit test). No startup auto-resume exists.
- Order inside `launch`: request checks, then `central::compare` (it may send archived reports; this is reconciliation and idempotent), then the reconnect decision. In the `NewSession | TamperDetected` arm, a fresh detection replaces a pending file that does not read as an object (unchanged since PR #58 Q-2). Then the new `match` reads the file:
  - `NotFound` → `None` (no pending). Correct; most existing tests depend on this.
  - read error of another kind (permission, `EISDIR`) → refused.
  - bytes that do not parse, or JSON that is not an object → refused.
- The refusal comes before the credential check, the budget backstop and `record_attempt`, so a refused request leaves no ledger row and no `runs/<attempt>`. The test asserts `runs/a2` does not exist.
- The resume, `AlreadyRunning`, `StaleGeneration` and unfinished-phase refusals all come earlier and are unchanged.
- The body carries `external_reconciliation_required: true`, the same shape as `central.rs`.

Conclusion: every path that would resume or start with an unreadable or non-object pending file and no fresh detection is now refused, and the file is kept.

### Is refusing `start` an expansion?

- The question asked was about resume ("…‘대기 없음’으로 읽혀 재개됩니다"), and the answer was "재개 거절". The REPORT's earlier option (나) said more broadly "읽히지 않는 대기 파일이 있으면 … 거절한다".
- Refusing `start` is needed for the rule to have any effect. `start` is the path that carries a pending detection: the `(Some(p), false, _)` arm. If `start` read an unreadable file as "no pending", the new session would start with `tamper: null`. The takeover would not clear the file (it clears only when `job.tamper` is some). The detection would then never be reported, and the role would still not resume over it. Refusing is the closed-side reading of the decision.
- The PR body, REPORT §4 and the contract text all disclose it. The decision record §2.2 does not (L-1).

### Way out

- A role with such a file stays blocked until a person acts. Two things clear it:
  - the person removes or repairs `/state/pending-tamper.json` on the role volume;
  - a `reconnect` while the config is still tampered, because the fresh detection overwrites the file.
- Nothing says this. The 409 reason does not name the file. The contract says only "사람이 대조할 때까지". There is no step list: read the file, compare with the coordinator's `latest` and tamper events, then remove the file or replace it with a valid detection (L-2). Other `EXTERNAL_RECONCILIATION_REQUIRED` reasons in this file have no procedure either, so this follows existing practice, but this one has a concrete fix that can be written down.

### Does overwriting with a fresh detection lose anything?

- While a detection is pending, its takeover was not accepted. A takeover is reported before the local write, so the local checkpoint's session does not move. A fresh detection therefore has the same `old_session_id` and `checkpoint_digest`, and only `observed_digest` may differ.
- The corrupt bytes are gone after the overwrite, and nobody learns that the file was corrupt (I-1). The protection goal (no unreported detection) holds.

## Item 2: runtime contract §13 item 10

- The text changes are additive and match the code:
  - (a) adoption counts the staged tamper as reported only when the coordinator answers `tamper_recorded: true`. This is the `carries_tamper && l["tamper_recorded"] == true` check.
  - (b) the refusal rule for unreadable or non-object pending files, with the fresh-detection exception.
  - (c) the Q-3 limit in the completion-condition column: tamper detection happens only on reconnect. A tamper made before a `start` is not applied to that session, but it is not detected or reported.
- Revision handling (M-2):
  - Each revision so far has a header line: revision 9 says "다른 조항은 revision 8과 같다", and earlier revisions say the same of the one before.
  - §13 item 10 was edited without a bump three times before (5b527c6, a1cfe78, 704330f). All of those were during PR #57/#58, before 02-04 was accepted on revision 9.
  - This is the first edit after acceptance. The ledger still cites "런타임 계약 revision 9" (`plan_and_spec`), and §10 makes acceptance of a revision a precondition for implementation work.
  - (b) is a new normative rule. Readers of "revision 9" now see two different texts with no marker between them.

## Item 3: decision record and ledger

- **Quotes.** The four answers match the PR body, event `USER-DECISION-0205-X08-20261115` and `notes[25]`: "권고대로 승인, 바로 착수", "(나) 소량 예산 승인", "(가) 계약의 알려진 한계로 기록", "재개 거절". The kept behaviours (Q-2, L-c3-2, P4) are recorded as confirmed with the last answer, which matches the question text.
- **F4.** Recorded as plan §2.1 (다): numeric ids, author set = PR author plus the role's GitHub identities, App bot included. It is written into 04-07, 05-01, 05-02 and 05-03 `unresolved` (tasks[30], [40], [41], [42]), with "02-05 코드 무변경". EARLY-01 is named as 1b in the record but its row was not updated (I-3).
- **S1.** Policy 0.6.0 kept: recorded in the decision record, 02-05 `unresolved` and the notes.
- **Budget (나).** The record's numbers compared with PLAN.md §4.3:

| Row | PLAN §4.3 | Decision record | Ledger `resume.budget` |
| --- | --- | --- | --- |
| Runs | 3 (dev 2, guardrail 1) + 1 spare; **role shares dev 3, guardrail 1** | 3 + 1 dev-only spare; shares missing | "3회 + 개발 전용 예비 1회"; shares missing |
| Per run | 600 s | 600 s | 600 s |
| Total time | 2,400 s **(dev 1,800, guardrail 600)** | 2,400 s | 2,400 s |
| Billable | 200,000 **(150,000 / 50,000)** | 200,000 | 200,000 |
| cache_read | 2,000,000 **(1,500,000 / 500,000)** | 2,000,000 | 2,000,000 |
| USD | 0 | 0 | 0 |
| R2 detail | "같은 session_id 재개, `branch_head`는 고정 입력" | missing | — |
| Budget check (예산 검사) row | outside the container, with the X-08 fixes | **row missing** | — |
| Block | "X-08 병합 전 실행 금지" | "X-08 **검수 완료** 전" (stricter, correct after PR #69) | "X-08 검수 완료와 예산 코드 PR 뒤" |

  The totals match. The per-role shares and the budget-check row do not appear anywhere in the record or the ledger. That is M-1.
- **Other blocks.** No other block was loosened. The 02-04 spare runs (dev 2, guardrail 3) stay unused. The X-02 budget is still behind X-08 검수 완료 and the remaining X-02 conditions. `resume.blocking` is unchanged. Step 8 needs X-08 검수 완료 and the budget code PR.
- **X-08 status.** Still 진행. The next action is "검수 기록" after merge.
- **`state.json` diff vs c5464f5.** Only these changed:
  - events +1;
  - `extra_tasks[7]` (X-08): evidence +2, `last_observed_at`, `next_action`, `result_state`;
  - `last_actual_work_observed_at`, `snapshot_id` 0043, `snapshot_created_at`;
  - notes +2;
  - `resume`: `branch_sha`, `budget`, `current_work`, `next_action`, `running`, `user_decisions` (+1 at the front);
  - 02-05: `last_observed_at`, `next_action`, `result_state`, `unresolved`;
  - 04-07 and 05-01..05-03 `unresolved`.

  No status or count changed (12/73, 0/12). Stale text was left in place (L-3).
- **PR #69 review L-1.** Handled. The 02-05 `result_state`, which STATUS and TASKS render, now reads "X-08 검수 완료 뒤(PR #69 검토 d0f72ec L-1)". The review allowed this as one of its options. PLAN.md §4.2 and §4.3 still say "X-08 병합", which the review accepted.

## Findings

### M-1 (Medium): the budget record says "§4.3 그대로" but drops the role shares and the budget-check row

- PLAN §4.3 (나) gives role shares as part of the approved quantity: dev 3 runs, guardrail 1; 1,800 s / 600 s; 150,000 / 50,000; 1,500,000 / 500,000.
- PLAN §4.2 says the pre-step-8 code PR adds "이름과 상한·역할 몫".
- The decision record's table, `resume.budget`, the 02-05 `unresolved` entry and the event all give only the totals. The record also leaves out the "예산 검사" row (check outside the container, with the X-08 fixes) and the R2 detail.
- A budget PR written from the record or the ledger would have to guess the shares, or invent different ones, for a user-approved spending limit.
- Fix:
  - add the shares, the budget-check row and the R2 detail to the record's table, or say plainly that the approved table is §4.3 and link the rows;
  - add the shares to `resume.budget`.

### M-2 (Medium): accepted contract revision 9 was changed with no revision mark

- 02-04 was accepted on revision 9. This PR changes §13 item 10 and adds a new refusal rule. The revision number stays 9 and the header has no note.
- The earlier bump-less edits to item 10 all happened before acceptance.
- Fix: either
  - raise the revision to 10, with the usual header line ("§13 항목 10에 사용자 결정 2026-11-15(X-08) 문장 3개 추가, 다른 조항은 revision 9와 같다") and update `plan_and_spec` in the ledger; or
  - keep 9 but add a dated header note saying what was added after acceptance and why.

### L-1 (Low): the decision record does not say that `start` is refused too

§2.2 says "재개하지 않고 사람의 대조를 요구한다". The code and contract also refuse `start`. The reason is good and the PR body and REPORT disclose it. But the user decision record is the document others will cite, and it should state this as the developer's reading of "재개 거절", with the one-line reason: start carries the pending detection, so starting would lose it.

### L-2 (Low): no documented way to reconcile

- The 409 reason does not name the file, and nothing describes what a person should do.
- Suggested additions:
  - put the path (`/state/pending-tamper.json` on the role volume) in the reason;
  - add a short procedure to REPORT §4 or the contract: inspect the file; compare with the coordinator's `latest` and `role_tamper_events` and with the current config digest; then delete the file, or write a valid detection object so the next new session reports it.
- Without this, a blocked role may be fixed by deleting the file with no check, which loses the detection the rule exists to protect.

### L-3 (Low): stale "사용자 결정 대기" left in governing ledger text

- `phase` still says "02-05 진행(계획 병합 fda0f2b, 사용자 결정 대기) · X-08 교정·후속 병합(…, 사용자 결정 대기)". This is the STATUS.md headline.
- X-08 `unresolved` still lists "사용자 결정 대기(보고 §4): Q-3 … 새로 찾은 잔여 …" as open.
- 02-05 `unresolved` still says "사용자 결정 대기 — F4…, S1…, 02-05 실제 회차 예산" and "02-05 구현 단계는 F4·S1 답 뒤에 시작한다". The resolution is appended after these, not put in their place.
- X-08 `next_action` says "검수 완료 시 X-02·실제 호출 차단 해제 조건 충족". For 02-05 step 8 it adds the other preconditions, but for X-02 it does not mention the remaining X-02 conditions. `resume.budget` has them.
- Fix in the next ledger update (the X-08 acceptance record is a natural place).

### I-1 (Info): the corrupt bytes are lost when a fresh detection replaces them

This is safe for reporting (only `observed_digest` can differ, item 1). Consider renaming the file to `pending-tamper.json.corrupt-<time>` before writing the fresh detection, so a person can see the file had been corrupt.

### I-2 (Info): REPORT wording left from the previous state

- §3 still says `evidence/test.log` was replaced by the 9b63ce8 run (349). It is now the 18d5d9e run (350), as §4 says.
- The §6 I-4 row still names the removed test `a_pending_file_that_is_not_an_object_does_not_panic`. §4 says it was replaced.

### I-3 (Info): EARLY-01 not updated with the F4 rule

The record names EARLY-01 among the 1b tasks where G3 changes, but only 04-07 and 05-01..05-03 got the rule. Fine if EARLY-01 inherits it through 05-0x; otherwise add a pointer.

### I-4 (Info): the refusal comes after `central::compare`

A refused request may still send archived reports to the coordinator first. That is reconciliation and harmless, but it means a refused request is not free of side effects.

### I-5 (Info): mutants.txt entry

The new mutant line ("unreadable pending file read as 'no pending' again: KILLED") matches my M1.

## Summary for the author

- M-1: copy the role shares, the budget-check row and the R2 detail of PLAN §4.3 (나) into the decision record's table and into `resume.budget`, or drop "그대로" and link §4.3 as the approved table.
- M-2: mark the post-acceptance change to runtime contract §13 item 10 in the header. Prefer revision 10 with the usual line, and update `plan_and_spec`.
- L-1: decision record §2.2: say `start` is refused too, and why.
- L-2: name the file in the 409 reason (or the docs) and write the reconciliation steps.
- L-3: replace the stale "사용자 결정 대기" text in `phase`, X-08 `unresolved` and 02-05 `unresolved` (can wait for the X-08 acceptance ledger update).
- The code, tests, CI and the rest of the ledger are fine. After M-1 and M-2 I expect to approve.
