# Review of PR #59, head 62acc7e (review 1 of at most 4)

- **PR:** HeaInSeo/ieo_poc #59, "report: 02-04 accepted (12/73) — acceptance record and hand-overs, ledger snapshot 0032", branch `report/0204-accepted`. Docs and ledger only.
- **Head reviewed:** 62acc7ea2ff09de0f7e13276a5cbc2a99077a213, confirmed with `gh pr view 59 --json headRefOid,statusCheckRollup` before and after the review. Base: main 31cfd5b (merge base = main).
- **CI at 62acc7e** (run 43681446560; I waited for the rust job to finish):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. No podman, no containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge, CI rerun or branch change.
- **Verdict: APPROVE.** Nothing blocks merge.
  - The three conditions of the user decision §6 hold (check 1).
  - Items 1-9 are handled as the decision table says, and the receiving tasks carry their parts (check 2). Two Low gaps remain in where the ledger records item 1b and item 8 (L-1, L-2).
  - Nothing is overclaimed. D3 stays failed, mock and real are kept apart, and the earlier unmet rows are preserved (check 3). One Info wording point (I-1).
  - The ledger is consistent: 12/73, 0/12, budget unchanged, and the render and package checks pass (check 4).

## What I ran

- `gh pr view 57` and `gh pr view 58` (`mergeCommit`, `headRefOid`, `state`), `git log -1 --format='%H %P'` on both merge commits, `gh run view` on 43679480078 and 43681078230.
- A structural diff of `state.json` (head against origin/main 31cfd5b), done with a Python script that walks the JSON.
- A detached worktree of 62acc7e in my scratch directory, removed at the end (`git worktree list` no longer shows it). Using `/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python`:
  - `docs/poc/scripts/render_progress.py --check`: `ok: true` (STATUS, TASKS, RESULTS, RESUME, events.jsonl).
  - `docs/poc/scripts/validate_package.py`: rc 0.
  - `sha256sum state.json` = `6444aaad…12fc10`, the value in the rendered view headers.
- `diff` of the archived `review-pr58-ae65f0c.md` against `pending-reviews/review-pr58-ae65f0c.md`: identical.
- I read:
  - the user decision `User_Decisions_2026-11-15_0204-acceptance.md`;
  - `ACCEPTANCE.md` in full;
  - `REPORT.md` §1, §2 and §7;
  - PROGRESS 9 §4, §5 and §9;
  - the verdicts and findings of all five PR #57 and PR #58 reviews.
- A grep of the added lines for secret patterns.

## Checks

### 1. The acceptance conditions are met

| Condition | Status | Evidence |
| --- | --- | --- |
| PR #57 merged on its approved head | Yes | MERGED. `headRefOid` 802e4a1. Merge commit 12bf34f has parents d337828 and **802e4a1**. Archived review `review-pr57-802e4a1.md`: **Verdict: APPROVE**. The earlier heads 48b1d0e and accd6c8 are REQUEST CHANGES, as the record says (review 3/4, correction 2/3). |
| PR #58 merged on its approved head | Yes | MERGED. `headRefOid` ae65f0c. Merge commit 31cfd5b has parents 12bf34f and **ae65f0c**. `review-pr58-704330f.md` and `review-pr58-ae65f0c.md`: both **APPROVE** (review 2/4, correction 1/3). |
| main CI | Yes | 43679480078: push, main, headSha 12bf34f, success. 43681078230: push, main, headSha 31cfd5b, success. |
| Items 4 and 9 implemented and tested | Yes, as recorded | ACCEPTANCE §1 points to PROGRESS 9 §2 and §3. Both reviews ran the full `ieo-coord` and `ieo-supervisor` suites on the real test DB. |
| Remaining items handed over | Yes, with L-1 and L-2 | Check 2. |
| No further blocking reason | None found | See below. |

**Open items from the PR #57 and PR #58 reviews.**

- 802e4a1: N-1 to N-3 and I-1 to I-4 are dispositioned in PROGRESS 9 §9. They were fixed in PR #58, and review 704330f confirmed the fixes.
- 704330f: P-1 is fixed (ae65f0c confirmed). P-2 is carried as Info, P-3 is covered for 4xx and only nominally for I-2.
- ae65f0c: Q-1 (Low) is answered by the PROGRESS §9 wording fix in this PR. Q-2 and Q-3 are Info. All are listed in ACCEPTANCE §4.
- None was rated blocking by its reviewer, and I agree. The worst one is Q-2: a corrupt `pending-tamper.json` is read as "no pending", so contract §13 item 10 does not hold for that case. It is an edge case that is not new, and the record names it.

**REPORT §1, §2 and §7.**

- §1 "외부 영속 저장소 복원" (external durable store restore) and §2 "§2.1·§5-1·§5-2·§5-6" were partial or unmet because the coordinator side was missing. Item 9 closes that.
- §1 and §2 "§7 status 항목" were partial: provider value mapping and `quota_group_id`. Item 4 closes that. The remaining limit (`last_actual_action_at` is per finished run) is carried in ACCEPTANCE §4.
- §1 "어댑터 연결" (adapter wiring) stays partial, moved to 02-05 by item 2. §1 "02-03 이월" (02-03 carry-overs) stays unmet, moved by item 1. §2 "repo 범위 stop" stays "안전 종료 미확인" (safe shutdown unconfirmed), moved by item 3. §2 "실행 중 실제 정지" (real stop mid-run) is not run, moved by item 6. All four moves are approved in the decision, so none blocks.
- §7: the remaining carries keep their owner and time ("02-04 수용 영향 없음", no effect on acceptance). ACCEPTANCE §4 refers to them.

I found no blocking reason the record overlooks.

### 2. Items 1-9 against the decision table

| Item | Decision | ACCEPTANCE §3 / PROGRESS §4 | Ledger (`unresolved` at 62acc7e) |
| --- | --- | --- | --- |
| 1a | 02-05 | 02-05, required before 02-05 acceptance | 02-05: "배정 수준 시간·토큰 한도 집행 … 02-05 수용 전 필수" (assignment-level limit enforcement, required before 02-05 acceptance). OK. |
| 1b | GH1-GH5, then the connecting tasks and EARLY-01 | 04-07 and 05-01 to 05-03 (GH start conditions) and EARLY-01 | **Not recorded on any receiving task** (L-1). |
| 1c | 02-07 | 02-07 | 02-07: "02-03 F3 판정 머리 — 02-07 수용 전 필수" (F3 judgment head, required before 02-07 acceptance). OK. |
| 2 | 02-05 | 02-05, through `compare` | 02-05: "코디네이터의 start·reconnect 호출(중앙 체크포인트 compare 경유)" (coordinator calls start and reconnect, through the central `compare`). OK. |
| 3 | children 04-05, external requests 02-05; unconfirmed until then | Same; `safe_shutdown_confirmed: false` kept | 04-05: "자식 추적(repo 범위 안전 종료)" (child tracking). 02-05: "외부 요청 추적(repo 범위 안전 종료)" (external request tracking). OK. |
| 4 | Before acceptance | Done | Check 1. |
| 5 | Keep D3 failed, no rerun | Kept: expected 3, answer 4 | 02-04 `unresolved` and `result_state`: "D3 답 불충족 … 유지" (D3 answer failure kept). OK. |
| 6 | 04-05, decision draft before a real run | 04-05, draft in PROGRESS 7 §5, no real stop before approval | 04-05: "실제 실행 전 시험 계획·미측정 사용량 처리·재개 조건 결정안 보고와 승인" (decision draft reported and approved before a real run). OK. |
| 7 | Keep the spares | Developer 2, Guardrail 3, unused | 02-04 `unresolved`. The budget did not change (check 4). OK. |
| 8 | Block the next real call and X-02 | Blocking condition | X-02 already carries "차단 조건(사용자 결정 2026-11-15 항목 8)" (blocking condition, decision item 8), from an earlier snapshot. `resume.next_action` also has it. **No owning task, and the block is not on 02-05** (L-2). |
| 9 | Before acceptance | Done | Check 1. |

### 3. Overclaims

- **Carry-overs.** ACCEPTANCE §3 is headed "이월은 완료가 아님" (carry-over is not completion). PROGRESS §4 says the same, and so does the 02-04 `unresolved`. The 02-04 `result_state` says "나머지 항목 인계" (remaining items handed over), not "done". OK.
- **D3.** ACCEPTANCE §2 keeps expected 3 and answer 4 apart from the contract conditions. It also keeps observation (the four empty Read lines) apart from inference (that the child counted them). No relabel, no rerun. OK.
- **Mock and real.** ACCEPTANCE §2 says no real run went through the item 4 and 9 paths, and that their evidence is mock, real DB and container. OK, apart from I-1.
- **Earlier unmet rows.** ACCEPTANCE §2 says the REPORT §1 and §2 rows keep their verdicts. The diff does not touch REPORT.md. OK.
- **Q-1 fix.** The PROGRESS §9 P-3 row now says the I-2 branch is not pinned, and why. That matches review ae65f0c Q-1. OK.
- **Model calls and budget.** "모델 호출 0" (0 model calls) and 7/12 runs, 40.666 s (counted 45 s), billable 33,948, cache_read 121,189. These match the existing ledger. No budget field changed.

### 4. Ledger

- **Changed `state.json` keys:** `snapshot_id`, `snapshot_created_at`, `last_actual_work_observed_at`, `phase`, `current_task`, `next_task`, `tasks`, `events`, `resume` and `extra_tasks`. `baselines`, `experiments`, `milestones`, `notes`, `delivery` and the global `unresolved` are byte-equal to main. So the budget is unchanged.
- **Task fields:**
  - 02-04: `status` 진행 → **검수 완료**, `verification_level` unverified → **verified**. `latest_report` is ACCEPTANCE.md, and `evidence` gains 2 entries (70 → 72). `result_state` is appended, and `unresolved` and `next_action` are rewritten.
  - Appended `unresolved` on 02-05 and 02-07, new on 04-05.
  - X-03 `next_action`: "02-04 수용 뒤 착수" (start after 02-04 acceptance).
- **Counts:** main tasks with status 검수 완료 = 12 (01-01 to 01-08, 02-01 to 02-04, all `main`, all `verified`). STATUS shows "주계획 12/73 16.4%" and "후속 실험 0/12". Extra tasks: X-01 is the only one 검수 완료, which is not counted.
- **Snapshot:** `v1.3.3-EXEC-0032`, with the SHA-256 in the views equal to the file. One new event, `02-04-ACCEPTED-20261115`, whose evidence lists the acceptance record and both approving reviews.
- **`current_task` / `next_task` = 02-05.** This is the next main task. The ledger has never put an X task in these fields: they stayed 02-03 through X-01, for example. X-03 first is in `resume.next_action` and `phase`, so it is consistent.
- `render_progress.py --check` and `validate_package.py` pass locally and in CI.

### 5. Secrets

None. The only hits on the added lines are the review boilerplate naming the secrets and runs directories, and the word "Secrets".

## Findings

### L-1 (Low; non-blocking): item 1b has no record on any receiving task

- ACCEPTANCE §3 and PROGRESS §4 route item 1b (real GitHub collection, F4 PR author identity, N5 PR-level review state) to "the connecting tasks with GH start conditions (04-07, 05-01 to 05-03) and EARLY-01". It applies after GH1-GH5.
- ACCEPTANCE §3 then says "원장에서 02-05·02-07·04-05·X-02의 `unresolved`에 각자의 몫을 적었다" (each part is written in the `unresolved` of 02-05, 02-07, 04-05 and X-02). 1b's receivers are not on that list.
- Their `unresolved` fields are unchanged: they list only GH start conditions. EARLY-01 is not a task (it shows up as frozen-ledger item L02), so it has no field.
- With 02-04 now closed, 1b is recorded only in the closed task's `unresolved`, ACCEPTANCE and PROGRESS.
- A related point: 02-05 already carries "02-03 이월(02-05 착수 전): … F4 … N5" (02-03 carry-over, before 02-05 starts), which reads as the rule decision. One sentence would keep a later reader from treating the two as the same obligation.
- **Suggestion** (fold into the next ledger PR):
  - add the 1b part to 04-07, or to the first GH-gated task that will collect real GitHub evidence, or to the global `unresolved`;
  - note in 02-05 that its F4 and N5 entry is the rule, and the real check moves with 1b.

### L-2 (Low; non-blocking): the item 8 block has no owner and is not on 02-05

- Decision item 8 blocks "다음 실제 호출" (the next real call) as well as X-02.
- The block now lives in X-02's `unresolved`, the closed 02-04's `unresolved`, and `resume.next_action`. `resume.next_action` is rewritten every snapshot.
- 02-05 is now `current_task`, and its acceptance text includes "실제 인수" (real takeover), which may be where the next real call happens. Its `unresolved` does not mention the block.
- No task owns the item 8 work itself: the P1-P4, N6 and F8 caller fixes from PR #51, L-c3-2 to L-c3-4 from PR #49, and the deadline-stop test.
- **Suggestion:** put the block in the global `unresolved`, or in 02-05's `unresolved` ("no real model call before item 8 is done and reviewed"), and name who does the item 8 work.

### I-1 (Info): "실제 Claude 회차(D1~D6)" omits G1 and R1

- ACCEPTANCE §2, third bullet. The real runs were seven: D1, G1, R1 and D2 to D6 (PROGRESS 6, RUN_PLAN).
- The claim (no real run went through the item 4 and 9 paths) holds for all seven, so this is wording only. Suggested wording: "실제 Claude 회차 7회(D1·G1·R1·D2~D6)" (the seven real Claude runs).

### I-2 (Info): "다음 supervisor 정비" has no task or time

- P-2, Q-2, Q-3, the Q-1 test gap, and the fixing test for the match branch go to "다음 supervisor 정비" (the next supervisor maintenance). REPORT §7 already uses this label for older items, so it is consistent.
- Q-2 is the one that matters most, because it is where contract §13 item 10 does not hold.
- Consider giving this bucket one home, such as a global `unresolved` entry, so it does not depend on a closed task's record.

### I-3 (Info): the 02-04 `unresolved` is shorter, and refers onward instead of listing

- The new text replaces explicit names with "수용 보고 §7의 그 밖의 열린 이월" (other open carries in REPORT §7) and the ACCEPTANCE §3 and §4 references. The dropped names are fa39d86 L-a remainder, L-b, L-c, adf760e L-2, rebuild reproducibility and `last_actual_action_at`.
- They are all still reachable from ACCEPTANCE §4 and PROGRESS §4 and §5, so nothing is lost.
- The global `unresolved` line "f73ac13 P3-C … (02-04 unresolved에 연결)" (linked to the 02-04 unresolved) now reaches the item only through REPORT §7 ("빈 `projects/*/memory` … 다음 supervisor 정비", the empty `projects/*/memory` directory, next supervisor maintenance).
- No action needed. If L-1 and L-2 lead to a global entry, this pointer can be updated at the same time.

## Summary for the author

- Merge as is: CI is 3/3 green, and nothing blocks.
- In the next ledger PR (X-03 or later), optionally:
  - record item 1b on its receiving task, or globally (L-1);
  - put the item 8 block on 02-05 or the global `unresolved`, and name its owner (L-2);
  - list all seven real runs in ACCEPTANCE §2 (I-1);
  - give "다음 supervisor 정비" a home (I-2).
