# Review of PR #65 at 62b8fa4 (review 1)

- PR: HeaInSeo/ieo_poc#65, "report: X-04 accepted (gRPC path 84c9668) — snapshot 0038"
- Head: 62b8fa4b5d00a1cf94d52412816f8c0f0df80f79 (confirmed with `gh pr view 65 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 84c9668 (PR #64 merge). One commit on top of the base: 62b8fa4. Branch `report/x04-accepted`.
- CI at 62b8fa4 (run 43707800585), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - proto buf lint and breaking (not required): SUCCESS
- Required checks on main (branch protection, re-read): the three names above.
- Reviewer: independent review agent, read-only, no model calls, no containers, no cargo builds. I did not read the secrets or runs directories and did not touch the main checkout, the df02-plan or x04wt worktrees. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking and no Medium findings.** The acceptance of X-04 is justified, REPORT §5 records review 835f583's L-3 and I-7 faithfully, the L-1 row now says what L-3 asked for, and the `state.json` diff contains only the intended changes. Counts stay at 12/73 and 0/12; budget, blocking and X-08 are unchanged; `tasks[]` is unchanged.

- **Q-1 (Low):** `resume.current_work` lists the 02-03·X-01 carry-over rule decisions among 02-05's *completion* conditions, while the 02-05 ledger entry marks them "02-05 착수 전". Together with `next_action` "02-05 착수(모델 호출 없는 부분부터)", a resuming agent could start 02-05 code before those decisions.
- **Q-2 (Info):** `resume.current_work` does not mention the non-blocking PR #61 N-2 note (handle f73ac13 P3 and bc55edf R5 in 02-05's first supervisor change PR).

Neither needs to block the merge. Both are wording in a resume aid and can go into this PR or the next snapshot.

## What I ran

- A detached worktree of 62b8fa4 from `ieo_poc-0204` in my scratch directory, removed at the end.
- `gh pr view 64 --json mergeCommit,headRefOid,state`: MERGED, head 835f583, merge commit 84c9668. `git log` of 84c9668: parents 493b712 (main) and 835f583 (the approved head). origin/main is 84c9668.
- `gh run view 43706567018`: pull_request on `feat/x04-grpc`, headSha 835f583, success, all four jobs success (three required + proto).
- `gh run view 43707470907`: push on main, headSha 84c9668, success, all four jobs success.
- Read the verdicts: review-pr64-c6c0656.md "REQUEST CHANGES: one Medium finding (M-1), no Blocking" (review 1 of 4); review-pr64-835f583.md "APPROVE. There are no Blocking or Medium findings" (review 2, correction 1 of 3), new findings L-3 (Low) and I-7 (Info).
- `git diff origin/main HEAD` in full (8 files, +176/−33). `git diff --quiet origin/main HEAD -- crates contracts .github docs/poc/Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md`: no difference.
- `cmp` of the archived `reviews/review-pr64-835f583.md` against `.git/ieo-dev-workspace/pending-reviews/review-pr64-835f583.md`: byte-identical.
- A JSON structural walk of `state.json` against 84c9668.
- The new `events.jsonl` line compared with the last `state.json` event: equal. Its two evidence paths exist.
- `sha256sum state.json` = `6ccac384…3962`, the hash printed in the four views.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: `ok: true`, rc 0.
  - `scripts/validate_package.py`: rc 0, `schema_errors` 0.
  - `scripts/test_render_progress.py`: RESULT PASS.
- The 6 distinct relative links in REPORT.md: all resolve.
- `git diff --check`: clean. Grepped REPORT, the archived review, `state.json` and `events.jsonl` for tokens, keys, passwords, home paths and the secrets directory: the only hit is the archived review's sentence saying none were found.
- `crates/ieo-coord/src/grpc.rs`: `ANSWER_MARGIN` = 50 ms (line 65), subtracted from the call deadline to make the work deadline (line 352), to check the L-1/L-3 wording.
- `gh pr checks` until the rust job finished, then `gh pr view`.

## (1) Is the acceptance justified?

| Condition | Result |
| --- | --- |
| PR #64 merged on the approved head | Yes. `headRefOid` 835f583; merge 84c9668 has parents 493b712 and 835f583 |
| Last review approved that head | Yes. review-pr64-835f583.md: APPROVE, no Blocking or Medium; L-3 Low and I-7 Info, "can stay as notes, or L-3's REPORT wording can be fixed in the acceptance-record PR" |
| Required CI on the approved head | Yes. Run 43706567018 at 835f583, three required checks and the proto job SUCCESS |
| Main CI on the merge SHA | Yes. 43707470907, 84c9668, success |
| Review count 2/4 | Right: c6c0656 (review 1), 835f583 (review 2) |
| Correction count 1/3 | Right: 835f583 is correction 1, for c6c0656 (M-1). No other correction commit after c6c0656 |

The REPORT §5 sentence "장부 §7 X-04 수용 기준과 계획 §6의 supervisor 클라이언트 전환이 §2대로 충족됐다" rests on the two reviews; review c6c0656 said "The code meets the X-04 acceptance criteria and the contract", and 835f583 confirmed the fix. It does not claim anything those reviews did not verify. "추가 작업이므로 73·12 완료 수에 넣지 않는다" is consistent with the views.

## (2) REPORT §4 L-1 row and §5

| Item | Review 835f583 | This PR | Faithful |
| --- | --- | --- | --- |
| L-1 row (§4) | L-3: the wording "시간 경계 대신 서버가 만든 답인지 확인" overstates; say the test proves the source, but passing still depends on the server answering within `ANSWER_MARGIN` | "서버가 만든 답(“rolled back” 메시지)인지 확인하고, 상한은 5초로 넓혔다. 다만 서버가 문장 시간 제한 뒤 50ms(`ANSWER_MARGIN`) 안에 답해야 하는 조건은 남는다(검토 835f583 L-3, §5)" | Yes. "시간 경계 대신" is removed, and the residual is named. The code matches: the work deadline is the call deadline minus `ANSWER_MARGIN` |
| L-3 (§5) | On a loaded runner the server may miss the 50 ms margin and the test fails with tonic `CANCELLED`; a longer deadline does not help; if it flakes, a test-only larger margin through `GrpcConfig` | "느린 기계에서 서버가 시간 제한 뒤 50ms 안에 답하지 못하면 tonic의 `CANCELLED`로 실패할 수 있다. 시간 여유는 deadline 길이와 무관하다. 흔들리면 시험용 `GrpcConfig`로 여유를 넓힌다" | Yes |
| I-7 (§5) | Evidence log taken on "c6c0656 + working tree"; re-run and CI at 835f583 give the same 332 | "evidence/test.log는 “c6c0656 + 작업 트리”에서 기록했다. 835f583의 검토 실행과 CI가 같은 결과(332 통과)다" | Yes |
| Other recorded items | L-2, I-1–I-6 recorded faithfully | "교정 1 표의 L-2·I-1~I-6(기록만 함)" | Yes |

The header "상태" row now ends with "→ 835f583 승인 → 병합 84c9668, **검수 완료**(§5)", so the header and §5 agree. §5's CI sentence ("필수 CI 3개와 비필수 proto 작업 성공(run 43706567018)") matches the run. "gRPC는 선택(`--grpc-listen`, `checkpoint_central.transport`)이고 HTTP가 기본" matches review c6c0656's description and X-04 `result_state`. Nothing claims more than was verified.

## (3) state.json structural diff against 84c9668

Changed paths (all others identical):
- `snapshot_id` (0037 → 0038), `snapshot_created_at`, `last_actual_work_observed_at` (20:12:23+09:00, after the PR #64 merge at 11:08:34Z = 20:08:34+09:00), `phase` ("X-04 구현 PR" → "X-04 검수 완료(84c9668)").
- `notes` +1: the main CI record for 43707470907 (84c9668).
- `events` +1: `X-04-ACCEPTED-20261115`, equal to the new `events.jsonl` line.
- `extra_tasks[3]` (X-04): `status` 진행 → 검수 완료, `verification_level` unverified → verified, `last_observed_at`, `result_state` (appends 835f583 approve, merge, main CI, "73·12 미산입"), `evidence` +1 (the archived 835f583 review), `next_action` "없음(02-05로). X-05·X-06은 각자의 선행대로", `unresolved` (appends the §5 non-blocking items).
- `resume`: `last_verified_stage`, `branch_sha`, `running`, `next_action`, `current_work`.

Not changed: `baselines`, `current_task`/`next_task` (02-05), every `tasks[]` entry, `experiments`, `milestones`, `resume.budget`, `resume.blocking`, the whole X-08 entry, and the other `resume` fields. The generated views changed only in the snapshot and SHA lines, the STATUS "현재" line, timestamps and X-04 row, the TASKS X-04 row, and the RESUME fields above. STATUS still shows 12/73 and 0/12.

Resume fields against the 02-05 ledger entry (`tasks[]` 02-05 `unresolved`, unchanged):
- `branch_sha`: main 84c9668, PR #64 merge, main CI 43707470907 success, "이 수용 기록 PR 병합 전". Correct.
- `running`: "X-04 수용 기록 PR(report/x04-accepted)". Matches the PR branch.
- `next_action`: "X-04 수용 기록 PR 병합 확인 → 02-05 착수(모델 호출 없는 부분부터) · X-08 … 첫 실제 모델 호출 전에 병합". Matches the ledger: "02-05의 모델 호출 없는 부분 … 은 X-08을 기다리지 않는다" and "X-08 병합 전에는 02-05의 실제 모델 호출을 하지 않는다". No prerequisite is added.
- `current_work`: the four 02-04 carry-overs (배정 수준 한도 집행, start·reconnect 호출과 중앙 compare, 외부 요청 추적, GitHub SHA·CI 대조) are listed as "02-05 수용 전 필수", matching the ledger. "실제 모델 호출이 필요한 부분은 X-08 병합 뒤" matches. See Q-1 and Q-2 for two wording points.
- `blocking` and `budget`: unchanged from 84c9668. X-08 blocking is kept.

## (4) Archive, links, validators, secrets, PR description, CI

- The archived review-pr64-835f583.md is byte-identical to the pending copy.
- The REPORT links (6 distinct) and the event evidence paths (2) resolve. Validators pass (above). No secrets in the added lines. `git diff --check` is clean.
- PR description: every statement matches the diff and the runs (X-04 → 검수 완료/verified; #64 merged on 835f583 as 84c9668; main CI 43707470907; reviews 2/4 with c6c0656 M-1 and 835f583 approve; corrections 1/3; L-3 and I-7 in §5; 12/73 and 0/12; budget unchanged; resume at 02-05 starting with model-free parts; X-08 before any real model call).
- CI at 62b8fa4 (run 43707800585): all three required checks and the proto job SUCCESS. The head was still 62b8fa4 after the checks finished.

## Findings

### Q-1 (Low): `resume.current_work` puts the 02-03·X-01 rule decisions under 02-05 completion, but the ledger says "착수 전"

**Where:** `resume.current_work`: "02-05 완료 조건: 장부의 02-05 수용 기준과 02-05 unresolved의 02-04 이월(배정 수준 한도 집행, 코디네이터 start·reconnect 호출과 중앙 compare, 외부 요청 추적, GitHub SHA·CI 대조, 02-03·X-01 이월 규칙 결정)", together with `resume.next_action` "02-05 착수(모델 호출 없는 부분부터)".

**Problem:** the 02-05 entry's `unresolved` labels these items "02-03 이월(02-05 착수 전): … F4, … N5, … S1" and "X-01 이월(02-05 착수 전): … U2". They are not 02-04 carry-overs and they are due before 02-05 starts, not at its acceptance. The resume text files them inside the 02-04 parenthesis as completion conditions, so a resuming agent could begin 02-05 implementation without first settling F4, N5, S1 and U2.

**Effect:** none on the ledger. `tasks[]` is unchanged and the 02-05 entry still states the order. This is not a new prerequisite; it only restates one the ledger already has.

**Fix (optional):** for example, "02-05 착수 전: 02-03 이월 규칙 결정(F4·N5·S1)과 X-01 이월 U2(02-05 unresolved). 02-05 수용 전 필수: 02-04 이월(배정 수준 한도 집행, …, GitHub SHA·CI 대조)."

### Q-2 (Info): the N-2 follow-up is not in `resume.current_work`

**Where:** 02-05 `unresolved`, "비차단 참고(PR #61 검토 79b40da N-2): … f73ac13 P3 … bc55edf R5 … 02-05의 첫 supervisor 변경 PR에서 함께 처리한다. 02-05의 선행·수용 조건이 아니다".

**Problem:** `resume.current_work` does not point at it. It is explicitly not a prerequisite or acceptance condition, so leaving it out is correct for "완료 조건"; it is only easy to miss when the first supervisor PR is planned.

**Effect:** none. The ledger entry carries it.

**Fix (optional):** none required. A short "첫 supervisor 변경 PR에 N-2(P3·R5) 포함(비차단)" could be added.

## Summary

X-04's acceptance is justified: approved head 835f583 merged as 84c9668, required CI green on 835f583, main CI green on 84c9668, reviews 2/4 and corrections 1/3 correct. REPORT §5 and the L-1 row record review 835f583's L-3 and I-7 faithfully and without overclaiming. The ledger diff is limited to X-04, the snapshot, phase, note, event and resume; 12/73, 0/12, budget and X-08 blocking are unchanged. Validators pass and the archive is byte-identical.

**APPROVE 62b8fa4.** Q-1 and Q-2 are optional wording fixes for this PR or the next snapshot.
