# Review of PR #69 at d0f72ec (review 1)

- PR: HeaInSeo/ieo_poc#69, "report: ledger snapshot 0042 — X-08 follow-up merged (e0a7f8a); every remaining gate reads "X-08 검수 완료" (review 3724bc9 L-1); archive review; REPORT §3 test-count note (I-2); waiting for user decisions"
- Head: d0f72ecc64ee3467b11a0a8629b11ccab356e0b4 (confirmed with `gh pr view 69 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main e0a7f8a (PR #68 merge), which is also origin/main. One commit on top: d0f72ec.
- CI at d0f72ec (run 43715228119), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - proto buf lint and breaking (not required): SUCCESS
  - The required contexts on main are exactly the first three.
- Reviewer: independent review agent, read-only, no model calls, no containers, no builds. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan worktree, or d05wt/x08wt. I used my own detached worktree and removed it at the end. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking and no Medium findings. One Low and three Info findings.**

- PR #68 was merged on its approved head, and the merge is recorded correctly.
- All five fields named in review 3724bc9 L-1 now say "X-08 검수 완료". `resume.blocking`, `resume.current_work`, `resume.budget`, X-02, 02-05 `unresolved` and EARLY-01 now agree. A restarted agent that reads the `resume` fields or the X-02/02-05 rows will not take the X-08 merge as the unblock point.
- The `state.json` diff contains only the intended changes. X-08 stays 진행. Counts stay 12/73 and 0/12, and the budget numbers are unchanged.
- **L-1 (Low):** the live 02-05 plan text still names "X-08 병합" as the gate for step 8. This is PLAN.md §4.2, the §4.3 decision table and the closing line, plus the 02-05 `result_state` summary shown in STATUS/TASKS. X-08 is already merged, so that text now reads as a met condition. The governing fields override it, so this does not block the merge. The PR title's "every remaining gate" is slightly too broad.

## What I ran

- A detached worktree of d0f72ec from `ieo_poc-0204` in my scratch directory, removed at the end.
- `gh pr view 68 --json mergeCommit,headRefOid,state`: MERGED, head 3724bc9, merge commit e0a7f8a. `git log -1 --format='%H %P' e0a7f8a`: parents 6e5e502 (main) and 3724bc9 (the approved head). origin/main is e0a7f8a.
- `gh run view 43714821313`: ci, headSha e0a7f8a, conclusion success.
- Pending `review-pr68-3724bc9.md`: "APPROVE: no Blocking or Medium findings. One Low and five Info findings."
- `git diff e0a7f8a d0f72ec` in full (8 files, +230/−37). `git diff --quiet e0a7f8a d0f72ec -- crates contracts docs/poc/Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md docs/poc/Amendment_v1.3.3-A6.md docs/poc/reports/tasks/02-05`: no difference. `git diff --check`: clean.
- `cmp` of the archived `reports/tasks/02-04/caller-carryovers-20261115/review-pr68-3724bc9.md` against `pending-reviews/review-pr68-3724bc9.md`: byte-identical (sha256 0507f65c…).
- A JSON structural walk of `state.json` against e0a7f8a (below).
- The new `events.jsonl` line compared with the last `state.json` event: equal. Its evidence path exists.
- A regex sweep over `docs/poc/**/*.md|json|jsonl` (archived reviews excluded) for "X-08 / 이 작업 / 항목 8 … 병합 … 전/뒤/까지/되기" (results in (2)).
- Relative links in REPORT.md, the archived review, RESUME/STATUS/TASKS/RESULTS: none broken. All X-08 `evidence` paths exist.
- `evidence/test.log`: header commit 9b63ce8, per-binary "passed" sums to 349. The PR #67 version (e2d420c, an ancestor of 6e5e502): header 1feebb1, sum 346.
- From `docs/poc`, with `.venv-poc/bin/python`: `validate_package.py`, `render_progress.py --check`, `test_render_progress.py`, `test_summarize.py`: all rc 0. `git status` was clean afterwards.
- Grepped the diff for secret patterns (sk-ant, ghp_, github_pat, private keys, `password`/`token` assignments). The only hit is the archived review's sentence that describes its own secret grep.
- `gh pr view 69 --json headRefOid,statusCheckRollup`, polled until every check completed, and `gh api …/branches/main/protection/required_status_checks`.

## (1) PR #68 merge

| Condition | Result |
| --- | --- |
| Merged on the approved head | Yes. `headRefOid` 3724bc9; e0a7f8a has parents 6e5e502 and 3724bc9 |
| Review of that head approved | Yes. review-pr68-3724bc9.md: APPROVE, L-1 plus I-1..I-5 |
| Main CI on the merge SHA | Yes. 43714821313, e0a7f8a, success |
| Ledger record | event `X-08-PR68-MERGED-20261115`, a note, `resume.branch_sha`, `last_verified_stage`, `phase`, and the X-08 `result_state` ("PR #68 검토 3724bc9 approve(검토 1/4), 병합 e0a7f8a, main CI 43714821313 success"). All correct |

## (2) Review 3724bc9 L-1: is "X-08 병합" still read as the unblock point?

**Fixed (the five fields L-1 listed):** X-02 `next_action` and `unresolved` (both places), 02-05 `unresolved`, `resume.budget`, and `unresolved[26]` (EARLY-01). Each now says "X-08 검수 완료". The new `resume.current_work` says "실제 모델 호출·X-02는 X-08 검수 완료와 예산 결정 뒤". `resume.blocking` already said "X-08 검수 완료 전 … 병합만으로 풀지 않음". The events for 0041 and 0042 say the same.

**Still says 병합, by place:**

| Place | Text | Judgement |
| --- | --- | --- |
| Amendment A6 (.md "차단 조건", .json `blocking` and X-08 criterion); ledger §7 X-08 row; X-08 `unresolved` (which quotes the criterion) | "X-08 병합 전에는 … 하지 않는다" / "이 작업이 병합되기 전에는 …" | Pinned text. As a necessary condition ("not before merge") it stays true. `notes[22]` explains the stricter rule: A6 says merge, but real calls wait for 검수 완료, and why. The same `unresolved` field and `next_action` add "X-08 검수 완료 전에는 X-02와 모든 실제 모델 호출 금지". Clear enough |
| 02-04 ACCEPTANCE.md §3 table, §4 N-1, §6 ("X-08이 병합되기 전에는 실제 호출과 X-02를 시작하지 않는다") | Same wording | A historical acceptance record. It is also phrased as a necessary condition. Acceptable |
| 02-04 `unresolved` ("8 … 병합 전 X-02·모든 후속 실제 호출 금지"), events 41/42 | Same | History of an accepted task and append-only events. Acceptable |
| User_Decisions_2026-11-15_resume.md (line 40, "첫 실제 모델 호출 전에 병합돼야 한다") | A necessary condition | Acceptable. §2's own words ("교정·시험·독립 검토·병합을 마친 뒤에만") are the source of the stricter rule |
| **02-05 PLAN.md** §4.2 ("**X-08 병합.** 병합 전에는 어떤 실제 호출도 하지 않는다"), C11, the §4.3 decision table row "차단 · X-08 병합 전 실행 금지", and the closing line "단계 8은 3번의 (나) 승인, X-08 병합, 공유 예산 검사 뒤에만 한다" | Lists "X-08 병합" as the step-8 precondition | **L-1 below.** This is the live plan of the current task. `resume.next_action` sends the reader to PLAN.md, and the closing line reads as a sufficient list |
| 02-05 `result_state` (rendered in STATUS.md/TASKS.md): "실제 호출 8은 사용자 결정·X-08 병합 뒤" | Same | Part of L-1. It summarizes the plan |

The X-08 REPORT has no such wording. §6 says the decisions are open, and that `ieo-coord` must be rebuilt before the next real call.

## (3) `state.json` diff vs e0a7f8a

Changed paths (structural walk), all intended:

- `snapshot_id` 0041→0042, `snapshot_created_at` and `last_actual_work_observed_at` set to 21:41:59.
- `phase`: "X-08 교정·후속 병합(6e5e502·e0a7f8a, 사용자 결정 대기)".
- `events` +1 and `notes` +1 (main CI 43714821313).
- `resume.branch_sha`, `last_verified_stage`, `running`, `current_work`, `next_action` and `budget` (only the gate wording changed).
- X-02 `next_action` and `unresolved`, 02-05 `unresolved` and `unresolved[26]`: "병합" → "검수 완료" only. I compared the old and new strings; nothing else changed.
- X-08: `last_observed_at` bumped (3724bc9 I-3), +1 evidence (the archived review), `result_state` adds the PR #68 merge and the coordinator detail "(58090, 778ee47 빌드)" (I-5), and `unresolved` appends I-1/I-2/I-4/I-5 as non-blocking.
- No `status` changed. X-08 stays 진행, X-02 stays 조건 대기 and 02-05 stays 진행. `validate_package.py` reports 12/73 accepted and a follow-up denominator of 12, unchanged. The budget numbers in `resume.budget` (7/12, 40.666초, 33,948, 121,189, X-02 6회·3,600·14,400·3M·30M, USD 0) are unchanged.

**Resume fields against the open decision requests:**

| Source | Decisions asked | `resume.current_work` |
| --- | --- | --- |
| PLAN.md §4.3 | 1 F4, 2 S1, 3 real runs and budget ((가)/(나)), the optional early start, and a report of developer decisions N5·U2·T1·C2 | "F4·S1(착수 조건)·실제 회차 예산·이른 착수 여부·U2 보고". Matches (see I-2) |
| X-08 REPORT §4/§6 | Q-3 (가)/(나), the corrupt or non-object pending file (가)/(나), and the three "확인" items (Q-2 first part, P4, L-c3-2) | "Q-3·손상/객체가 아닌 대기 파일·유지한 동작 확인". Matches |

`resume.next_action` ("이 기록 PR 병합 → 사용자 답 대기 → X-08 결정 반영 PR과 검수 기록, 02-05 단계 1~7, 예산 결정에 따라 단계 8") agrees with PLAN §2.6 (steps 1–7 only after F4·S1 or an early-start permission). `current_work` says the same.

## (4) Archive, REPORT note, links, validators, CI

- The archived review sits beside REPORT.md, not in `reviews/`, and it is byte-identical to the pending copy. Its relative links (e.g. `evidence/test.log`) resolve from that folder. That is the reason REPORT §3 gives for the placement.
- REPORT §3 note: "346 통과 … (커밋 1feebb1, PR #67). 그 기록은 후속(§6)의 9b63ce8 실행(349 통과)으로 evidence/test.log를 바꿨다. 이전 기록은 git 이력(PR #67의 이 파일)에 있다". This is accurate: the current log is 9b63ce8/349, and e2d420c (in PR #67) holds 1feebb1/346.
- Links: none broken. Validators: all pass. Secrets: none. Required CI: 3/3 SUCCESS at d0f72ec.

## Findings

### L-1 (Low): the 02-05 plan still names "X-08 병합" as the step-8 gate

- PLAN.md §4.2 lists "**X-08 병합.**" as the precondition. Its §4.3 table row reads "차단 · X-08 병합 전 실행 금지". Its closing line reads "단계 8은 3번의 (나) 승인, X-08 병합, 공유 예산 검사 뒤에만 한다". The 02-05 `result_state`, rendered in STATUS.md and TASKS.md, says "실제 호출 8은 사용자 결정·X-08 병합 뒤".
- **Why it matters:** X-08 is merged. An agent that resumes 02-05 by PLAN.md (as `resume.next_action` directs), and gets a (나) answer, could treat the X-08 condition as met. The §4.3 text is also the decision request in front of the user. The ledger's rule is stricter than the words the user is answering. The error points the safe way, but it is still a mismatch.
- **Mitigation:** `resume.blocking`, `resume.current_work`, 02-05 `unresolved` ("X-08 검수 완료 전에는 02-05의 실제 모델 호출을 하지 않는다") and `notes[22]` all state the stricter gate. X-08 is still 진행. Review 3724bc9 called the PLAN and `result_state` wording acceptable while X-08 was being fixed. Now that X-08 is merged, the PLAN wording is the one place left that reads as satisfied.
- **Ask (no need to block):** in the next ledger or 02-05 PR, at the latest with the budget-answer PR or the step-8 PR, add one line. Either append to 02-05 `result_state` "(단계 8 관문은 X-08 검수 완료로 강화, notes[22])", or add a short note under PLAN §4.2 that the gate is now 검수 완료. When the decision request is restated to the user, say "X-08 검수 완료".

### I-1 (Info): the commit and PR title overstate the scope

"every remaining gate reads 'X-08 검수 완료'" is true for the five L-1 fields and for `resume`. It is not true for PLAN.md, the 02-05 `result_state`, ACCEPTANCE.md or A6. The PR body is precise ("five fields"). A wording note only.

### I-2 (Info): "U2 보고" names only one of the four reported developer decisions

PLAN §4.3 reports N5·U2·T1·C2 together. `current_work` names only U2. That is reasonable, because U2 is the one with a user-visible effect (409→422, review 3c017a4 N-1). "N5·U2·T1·C2 보고" would match the plan exactly.

### I-3 (Info): A6's pinned wording relies on `notes[22]`

A6 and ledger §7 keep "병합" as the condition. The explanation of the stricter reading is in a ledger note and in `resume.blocking`, not next to A6. That is acceptable for a pinned amendment. If A6 is ever revised, or X-08 is accepted, the acceptance record should say once that X-08 was gated on acceptance, not merge.

## Summary for the author

- Approve d0f72ec as is.
- L-1: add a one-line pointer that the 02-05 step-8 gate is "X-08 검수 완료". Put it in the 02-05 `result_state` or in PLAN §4.2, at the latest with the budget-answer or step-8 PR. Use that wording when you restate the decision request.
- I-1 to I-3 are optional.
