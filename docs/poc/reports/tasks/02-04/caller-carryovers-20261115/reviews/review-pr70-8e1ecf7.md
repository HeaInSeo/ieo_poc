# Review of PR #70 at 8e1ecf7 (review 2)

- PR: HeaInSeo/ieo_poc#70, "X-08: apply user decisions 2026-11-15 (refuse on unreadable pending tamper file, Q-3 as contract limit); record 02-05 decisions — snapshot 0043"
- Head: 8e1ecf74707c9165c10fac54b9c6e63dbbd1c33a (confirmed with `gh pr view 70 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main c5464f5 (c5464f5f9871…), which is also the merge base. Four commits on top: 18d5d9e, a979f1e, 4e42bef (reviewed in review 1), and 8e1ecf7, correction 1 of 3 for review 4e42bef.
- CI at 8e1ecf7 (run 43717943212, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS. The log shows 350 passed, 0 failed, 0 ignored, and `an_unreadable_pending_file_stops_until_a_person_reconciles_it::{http,grpc}` ok.
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls, no containers and no local build: disk was tight, and CI ran the full suite on this head. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan, d05wt, x08wt or s1wt worktrees, `target-x08`, or the PostgreSQL on 55433. I used my own detached worktree and removed it at the end. This is review 2 of at most 4.

## Verdict

**APPROVE: two Low and four Info findings. No Blocking or Medium findings.**

- **M-1 fixed.** The decision record's budget table matches PLAN.md §4.3 row by row, except for two rows:
  - the header (제안 → 승인);
  - the 차단 row, which is stricter ("X-08 **검수 완료** 전" instead of "병합 전") and gives the reason.

  `resume.budget` and the 02-05 `unresolved` entry carry the same role shares.
- **M-2 fixed.** Runtime contract revision 10 has a header line in the same form as revisions 5–9. `plan_and_spec` now says revision 10.
- **L-1 and L-3 fixed. L-2 partly fixed.**
  - The new reconciliation procedure (REPORT §4.1) keeps the evidence: it copies the file first and never says to delete without a copy.
  - It is imprecise in three places that could block a role again or invite an unchecked deletion (new L-1).
- **New inconsistency.** The merged 02-05 PLAN.md had already reserved "runtime contract revision 10" for 02-05's changes. That number is now taken (new L-2).

## What I ran

- `gh pr view 70` before and after: head 8e1ecf7 both times, all four checks success after waiting.
- `git diff 4e42bef 8e1ecf7`: 11 files. The only code change is the reason string of `pending_unreadable` in `serve.rs`.
- Validators in `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0 (`schema_errors` 0);
  - `render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl);
  - `test_render_progress.py`: PASS.
- Relative-link check over all 9 `.md` files changed since c5464f5: no broken links.
- `cmp` of `reports/tasks/02-04/caller-carryovers-20261115/reviews/review-pr70-4e42bef.md` against `pending-reviews/review-pr70-4e42bef.md`: identical.
- Secret grep over `git diff c5464f5 8e1ecf7`: the only hits are the archived reviews' sentences describing their own grep.
- Exact comparison of the decision record's table rows with PLAN.md lines 283–296 (`diff` after stripping the `> ` quote prefix): only the header and 차단 lines differ.
- Structural diff of `state.json`: 4e42bef → 8e1ecf7, and c5464f5 → 8e1ecf7.
- Code reading of `launch`, `write_checkpoint`/takeover reporting (serve.rs), coordinator `report_checkpoint` and `latest_row` (ieo-coord api.rs), to check the reconciliation procedure.

## Item (a): review-1 findings

| Finding | Status | Notes |
| --- | --- | --- |
| M-1 budget shares | **Fixed** | The record's table equals PLAN §4.3 (나) apart from the header and the stricter 차단 row, which cites PR #67 review 681c1d3 L-2. The rows now include the shares (dev 3 runs / 1,800 s / 150,000 / 1,500,000; Guardrail 1 / 600 / 50,000 / 500,000), the R2 detail and the 예산 검사 row. `resume.budget` and `tasks[12].unresolved` carry the same shares. The 차단 row's "02-04(남은 5회 유지)" is the same as the former "예비 회차(개발 2·Guardrail 3) 미사용". The event `USER-DECISION-0205-X08-20261115` still gives totals only (I-2). |
| M-2 contract revision | **Fixed** | Header: "문서 **revision 10**(2026-11-15): … §13 항목 10에 반영했다. … 다른 조항은 revision 9와 같다." Same pattern as revisions 6–9. Revision 9's line is kept unchanged after it. The §13 heading says "(revision 9, 항목 10은 revision 10에서 보완)". The word diff since c5464f5 shows no contract change outside the header, the §13 heading and item 10. `resume.plan_and_spec` says "런타임 계약 revision 10". |
| L-1 start refusal | **Fixed** | Record §2.2 sub-bullet: start is refused too, with the reason (start carries the pending detection), marked as an implementation reading of the decision. |
| L-2 reconciliation | **Partly** | The reason now names `/state/pending-tamper.json` and "X-08 report §4". It compiles (CI clippy and tests pass); no test asserts the text. REPORT §4.1 gives a 4-step procedure. It is safe, but imprecise: see new L-1. |
| L-3 stale wording | **Fixed** | `phase`: "02-05 진행(…, 사용자 결정 2026-11-15로 착수) · X-08 …, 사용자 결정 반영 PR". X-08 `unresolved`: "사용자 결정 2026-11-15로 해소 … 당시 질문:". 02-05 `unresolved`: "사용자 결정 2026-11-15로 해소" and "“02-05 착수 전” 조건 … 충족됐다". Left in place: past events, the dated history inside X-08 `result_state` (it ends with "남은 사용자 결정 없음"), and the historical "N-3 … 착수 조건은 F4·S1뿐". Acceptable. |
| L-3 X-08 `next_action` | **Fixed** | "검수 완료는 X-02의 선행 하나와 실제 호출 차단 해제 조건 하나를 채울 뿐이다: X-02는 executable 등록·의미 검토 등 남은 조건, 02-05 단계 8은 예산 코드 PR·단계 3·4 병합 등 그 단계의 전제도 필요". This matches X-02 `unresolved` ("남은 조건: X-08 검수 완료, executable 등록·의미 검토") and the record's step-8 conditions. It does not overstate. |
| I-1 corrupt bytes lost | **Recorded** as a limit in §4.1, with no code change. Fine. |
| I-2 REPORT wording | **Fixed, one rendering slip** | §3 now explains the test.log history (346 → 349 → 350). §6 I-4 row: see I-1 below. |
| I-3 EARLY-01 F4 | **Fixed** | `unresolved[26]` (EARLY-01 bullet in STATUS) carries the F4 rule. |
| I-4, I-5 | **Recorded** in REPORT §7. No change needed. |

### Reconciliation procedure (REPORT §4.1) against the code

- **Step 1** copies the file without changing its bytes. Nothing is lost.
- **Step 3, "write a valid object back; the next new session reports it":** true in the code, if the object has the right fields.
  - In `launch`, any JSON object is taken as pending.
  - On `start`, or on a reconnect that becomes a new session, the `(Some(p), false, _)` arm sets `new_session_id` and makes it `job.tamper`.
  - Every checkpoint of that session carries `config_tamper_detected`.
  - The coordinator inserts `role_tamper_events` and returns `tamper_recorded: true`, and the supervisor then deletes the pending file.
- **Required fields.** The coordinator requires `old_session_id`, `new_session_id`, `checkpoint_digest` and `observed_digest` as non-empty strings (`api.rs` around line 3710). Otherwise it answers 400.
  - A 4xx drops the staged takeover, and the run returns the refusal. The pending file stays.
  - So a hand-written object without those fields does not lose evidence, but it blocks the role again on every start.
  - §4.1 does not list the fields.
- **"Reconnect with a pending detection requires a new session id":** true.
  - Arm `(Some(p), true, _)`: a resume with a pending detection returns 400 `SCHEMA_VIOLATION` "new_session_id required: a detected config tamper is not yet reported".
  - Arm `NewSession | TamperDetected`: `new_session_id` is required anyway.
  - The check `new_session_id != old_session_id` also applies.
- **Step 2, "`GET …/checkpoints/latest`의 `tamper_events`와 … 대조":**
  - `tamper_events` in `latest` is a **count** of the role's tamper events (`SELECT count(*) …`), not a list. It cannot show whether one particular detection was recorded.
  - What can show it:
    - an archived checkpoint under `/state/checkpoints/` that carries this detection in `config_tamper_detected`. Local checkpoints of a takeover are written only after the coordinator accepted it;
    - `latest.tamper_recorded` for that seq, if it is the latest;
    - or comparing the count with the number of distinct archived detections.
- **"탐지 내용이 없음을 확인했으면 파일을 지운다":**
  - If the bytes do not parse, nobody can confirm from them that there was no detection.
  - The safer order is to try a **reconnect** (not a start) first. If the config is still tampered, the fresh detection replaces the file and is reported. Because of Q-3, a start would not detect it.
  - Or compare the current config digest with the last checkpoint's digest before deleting.

## Item (b): regressions and new inconsistencies

- **Revision-number collision with the 02-05 plan (new L-2).** PLAN.md (merged fda0f2b) says:
  - line 153: "런타임 계약에서 어댑터 필드의 뜻이 바뀌면 revision 10으로 적고";
  - line 204: "뜻 변경이 아니며 revision 10에 그렇게 적는다";
  - line 313: "런타임 계약 revision 10(체크포인트 `delivery_id`·`worktree_head`·첫 행동·외부 요청 목록, 어댑터 멱등 재조회)".

  This PR now uses revision 10 for X-08. Nothing tells the 02-05 implementer that those changes become revision 11.
- **Markdown table (I-1).** The REPORT §6 I-4 (가) row now has four cells under a three-column header. GitHub drops the extra cell, so the note "이 시험은 … 로 대체됐다" does not render.
- **Code.** Only the reason string changed. Its format arguments are unchanged (`{why}`), and CI passes. The path in the text is the contract path `/state/...`. `cfg.state` maps to it in the container; in tests it is a temporary directory (I-3).
- **No other regression found.** Statuses, counts, blocking text and budgets other than the 02-05 shares are unchanged.

## Item (c): general

- **`state.json` vs c5464f5.** Changes:
  - events +1;
  - X-08 (`extra_tasks[7]`): evidence +3 (the third is the archived review-pr70-4e42bef.md), `last_observed_at`, `next_action`, `result_state`, `unresolved`;
  - `last_actual_work_observed_at`, `snapshot_id` (0043), `snapshot_created_at`;
  - notes +2;
  - `phase`;
  - `resume`: `branch_sha`, `budget`, `current_work`, `next_action`, `plan_and_spec`, `running`, `user_decisions`;
  - 02-05 (`tasks[12]`): `last_observed_at`, `next_action`, `result_state`, `unresolved`;
  - 04-07 and 05-01..05-03 `unresolved`;
  - `unresolved[26]` (EARLY-01).

  All are intended. No status changed.
- **12/73 and 0/12** in STATUS. X-08 status is still **진행**. `resume.next_action`: "X-08 결정 반영 PR 검토·병합 → X-08 검수 기록 → 02-05 단계 1…".
- **Archived review-pr70-4e42bef.md:** byte-identical to the pending copy.
- **Validators** pass and **links** resolve (see What I ran).
- **No secrets** in the diff.
- **CI:** the 3 required checks pass at 8e1ecf7, and the full suite runs in the rust job (350 passed).
- **test.log:** the appended note says that only the reason text changed, and that `-p ieo-supervisor` passed 122 tests. It is a comment, not a new run. The full count comes from CI. That is honest.

## Findings

### L-1 (Low): the reconciliation procedure (REPORT §4.1) is imprecise in three places

- Step 2 compares against "`latest`의 `tamper_events`", which is a count. Name instead:
  - the archived checkpoint in `/state/checkpoints/` that carries this detection in `config_tamper_detected` (local takeover checkpoints exist only after the coordinator accepted them);
  - `latest.tamper_recorded` when that seq is the latest;
  - or the count compared with the number of distinct archived detections.
- Step 3, "올바른 JSON 객체": say what the object needs:
  - `config_tamper_detected: true`;
  - non-empty `old_session_id`, `checkpoint_digest` and `observed_digest` (the supervisor fills `new_session_id`).

  The coordinator refuses the takeover with 400 if any are missing, and the role stays blocked.
- Step 3, "탐지 내용이 없음을 확인했으면 지운다": unparsable bytes cannot show this. Before deleting, either:
  - try a reconnect (not a start; Q-3), which replaces the file with a fresh detection if the config is still tampered; or
  - compare the current config digest with the last checkpoint's digest.

No step loses evidence (step 1 keeps a copy), so this is not blocking. Can be fixed in the X-08 acceptance record.

### L-2 (Low): "runtime contract revision 10" was already reserved by the 02-05 plan

PLAN.md lines 153, 204 and 313 name revision 10 for 02-05's contract changes. Add one line to the 02-05 `unresolved` entry, or a PLAN note, saying that those changes become revision 11 because X-08 took revision 10. This can wait for the X-08 acceptance record or the 02-05 step-1 PR.

### I-1 (Info): REPORT §6 I-4 (가) row has an extra cell

The row has four cells under a three-column header, so "— 이 시험은 §4의 … 로 대체됐다" is not rendered on GitHub. Fold it into the third cell.

### I-2 (Info): event USER-DECISION-0205-X08-20261115 gives budget totals only

Events are a dated record. The record, `resume.budget` and 02-05 `unresolved` now carry the shares, so this is fine as it is.

### I-3 (Info): the reason names the contract path

`/state/pending-tamper.json` is written as text, not from `cfg.pending_tamper_path()`. It is correct in the container, where the contract path is `/state`. In tests the real path differs. Fine.

### I-4 (Info): small stale numbers in X-08 `unresolved`

"I-2 보고 §3의 346은 기록 당시 값(현재 시험 기록 349)": the current test log is 350. X-08 `result_state` keeps the dated "→ 사용자 결정 대기" history, followed by "남은 사용자 결정 없음". Both can be tidied in the acceptance record.

## Summary for the author

- M-1, M-2, L-1 and L-3 are fixed, and I-1, I-3, I-4 and I-5 are handled. L-2 is partly fixed: the file path is in the reason and a procedure exists.
- Not blocking, can go in the X-08 acceptance record:
  - L-1: make REPORT §4.1 precise (`tamper_events` is a count; the fields a valid object needs; reconnect before deleting an unparsable file);
  - L-2: note that 02-05's planned contract changes become revision 11.
- Info: fix the four-cell table row in REPORT §6.
- Approve at 8e1ecf7.
