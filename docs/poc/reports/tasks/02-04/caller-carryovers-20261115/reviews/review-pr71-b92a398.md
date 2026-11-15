# Review of PR #71 at b92a398 (review 1)

- PR: HeaInSeo/ieo_poc#71, "report: X-08 accepted (1c21177) — snapshot 0044"
- Head: b92a398632caf94b50afca8df3e9c3970e25c38e (confirmed with `gh pr view 71 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 1c21177 (PR #70 merge). One commit on top of the base: b92a398.
- CI at b92a398, waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - proto buf lint and breaking (not required): SUCCESS
- Required checks on main (branch protection, re-read): the three names above.
- Reviewer: independent review agent, read-only. No model calls, no containers, no cargo builds. I did not read the secrets or runs directories. I did not touch the main checkout, the df02-plan worktree or the d05wt/x08wt/s1wt worktrees. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking or Medium findings. One Low and five Info findings.**

- The X-08 acceptance is justified. Every item in the ledger §7 row has a fix and a regression test, or a user decision. PRs #67, #68 and #70 were merged on their approved heads. Main CI passed on all three merge SHAs. Review and correction counts are within limits.
- REPORT §4.1 now matches the code:
  - the required fields are right;
  - reconnect compares digests before cleanup;
  - start does not detect (Q-3);
  - no step loses evidence.
- The §6 row now renders with 3 cells. §8 is accurate.
- `resume.blocking` does not loosen anything.

Findings:

- **L-1 (Low):** the REPORT header row "상태" still says the §4 changes are "독립 검토 전" and "이 기록은 X-08 완료가 아니다". This contradicts §8 and the ledger.
- **I-1 (Info):** §4.1 step 3 re-detects a tamper only if the config is still tampered on disk. A tamper that a later attempt's cleanup already removed is not re-detected.
- **I-2 (Info):** §4.1 step 2 no longer checks whether a readable detection was already recorded. Re-reporting it can create a second tamper event. This over-reports and loses nothing.
- **I-3 to I-5 (Info):** the wording of `resume.budget`, PLAN.md's "revision 10" and "X-08 병합", and an older event's evidence path. None of these loosens anything.

## What I ran

- Created a detached worktree of b92a398 from `ieo_poc-0204` in my scratch directory, and removed it at the end.
- `gh pr view 67/68/70 --json state,headRefOid,mergeCommit`:
  - #67 MERGED, head 681c1d3, merge 6e5e502;
  - #68 MERGED, head 3724bc9, merge e0a7f8a;
  - #70 MERGED, head 8e1ecf7, merge 1c21177.
- `git log -1` parents of each merge:
  - 6e5e502 = fda0f2b + 681c1d3;
  - e0a7f8a = 6e5e502 + 3724bc9;
  - 1c21177 = c5464f5 + 8e1ecf7.
- `gh run list`:
  - pull_request CI success on the approved heads (681c1d3 run 43711240051, 3724bc9 43713811893, 8e1ecf7 43717943212);
  - main CI success on the merge SHAs (6e5e502 43712109183, e0a7f8a 43714821313, 1c21177 43718393840).
- Read the verdict lines of the pending reviews:
  - review-pr67-681c1d3.md: APPROVE (review 1);
  - review-pr68-3724bc9.md: APPROVE (review 1);
  - review-pr70-4e42bef.md: REQUEST CHANGES (review 1);
  - review-pr70-8e1ecf7.md: APPROVE (review 2).
- Commit history:
  - `git log c5464f5..8e1ecf7`: one correction commit (8e1ecf7) after 4e42bef;
  - `fda0f2b..681c1d3` and `6e5e502..3724bc9` contain no correction after a review.
- `git diff origin/main HEAD` in full: 8 files, +227/−41. Only `docs/poc/progress/*` and the X-08 report folder changed. No code, contract or ledger change.
- `cmp` of the archived `reviews/review-pr70-8e1ecf7.md` against the pending copy: byte-identical (sha256 `a5775354…1771763`).
- `sha256sum state.json` = `f157f1bb…f8b19`, the hash printed in the four views.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: `ok: true`;
  - `scripts/validate_package.py`: rc 0;
  - `scripts/test_render_progress.py`: RESULT PASS.
- Relative links in REPORT.md, the four views and the archived review: all resolve.
- Grepped the added lines for tokens, keys, passwords and oauth. The only hits are the archived review's own sentences about its grep.
- Code read for §4.1:
  - `crates/ieo-supervisor/src/serve.rs`: `launch` (lines 549–877), `execute_inner` (lines 907–950), `pending_unreadable` (line 1528);
  - `crates/ieo-supervisor/src/checkpoint.rs`: `decide`;
  - `crates/ieo-coord/src/api.rs`: tamper validation (lines 3701–3734), `latest` `tamper_events` (line 3462).

## (1) Is the acceptance justified?

| Condition | Result |
| --- | --- |
| Ledger §7 X-08 items | REPORT §1 table: P1, P2 (C4/C6), P3, P4, N6, F8, L-c3-2, L-c3-3, L-c3-4, the deadline-stop test, P-2 (all three paths after 9b63ce8), Q-1, Q-2, and the kept-pending and matching-branch fixing tests. Each row has a fix (or "code already did it; test added") and a named regression test with a mutant caught. |
| Items not fixed | Q-3 got user decision (가) (User_Decisions_2026-11-15_0205-x08.md §2.2). The kept behaviours Q-2 (older pending detection wins), L-c3-2 and P4 were confirmed by the user. The corrupt pending file is now refused (decision "재개 거절", code at 18d5d9e, test `an_unreadable_pending_file_stops_until_a_person_reconciles_it`). The ledger rule "decisions come from the user, not from reviewer agreement" holds. |
| No model calls, spare runs, real stops or payments | REPORT header row "모델 호출 0회"; the budget is unchanged in `state.json` |
| PRs merged on approved heads | Yes, for all three (parents above) |
| Last reviews approved | 681c1d3 APPROVE, 3724bc9 APPROVE, 8e1ecf7 APPROVE |
| Main CI on the merge SHAs | success for 6e5e502, e0a7f8a and 1c21177 (run 43718393840) |
| Review and correction counts | #67 1/4, 0/3. #68 1/4, 0/3. #70 2/4, 1/3. Within 4/3. |
| Amendment A6 | A6 says "X-08 병합 전" and the ledger used the stricter "검수 완료" (681c1d3 L-2). §8 says so correctly. |
| Counted as extra | Yes. STATUS still shows 12/73 and 0/12, and `phase` and the views list X-08 as 검수 완료 outside the counts. |

## (2) REPORT §4.1, §6 and §8 against the code

| Claim | Code | Correct |
| --- | --- | --- |
| Start and reconnect stop with 409 `EXTERNAL_RECONCILIATION_REQUIRED` and the quoted reason | `launch` reads the pending file after the op split and returns `pending_unreadable(...)` for both ops. Reason: "the pending tamper file /state/pending-tamper.json cannot be read …" | Yes |
| Step 1: copy without changing the bytes | The procedure itself does not depend on code. Nothing deletes the file before step 3. | Yes |
| Step 2: required fields are non-empty `old_session_id`, `checkpoint_digest` and `observed_digest`; otherwise the next takeover gets 400 and the role stops again | api.rs lines 3705–3720 require these three plus `new_session_id` as non-empty strings. The supervisor fills `new_session_id` (`(Some(p), false, _)` and `(Some(p), true, _)` arms). A failure returns `bad(...)` (400). The takeover becomes `CentralRefused`, and the pending file stays because `tamper_recorded` is not true. | Yes. Review 8e1ecf7 also suggested `config_tamper_detected: true`. Neither the coordinator (non-null check only) nor the supervisor's `carries_tamper` (`!is_null()`) needs it, so leaving it out is right. |
| Step 2: the pending file is deleted when the coordinator answers `tamper_recorded: true` | `write_checkpoint` (lines 1181–1182) and `local_checkpoint` (line 364) delete only on `== true` | Yes |
| Step 3: `tamper_events` is a count | api.rs line 3462: `SELECT count(*)` | Yes |
| Step 3: reconnect compares the checkpoint's settings digest with the current config before cleanup | In `launch` (reconnect branch), `checkpoint::decide(cp, record, observed, …)` compares `cp.settings_digest` with `configdir::observed(config_dir)`. `configdir::clean` runs later, in `execute_inner` (line 921), after `launch` returned 202. | Yes |
| Step 3: the comparison creates a fresh detection that is reported with the new session | `TamperDetected` → `detected`. With the corrupt file deleted, `pending_readable` is false, so the detection is written durably before any refusal. `new_session_id` is required. Then `tamper` = detection plus `new_session_id` → `Job.tamper` → the takeover checkpoint's `config_tamper_detected` → coordinator insert. | Yes. Scope: see I-1. |
| Step 3: start does not detect (Q-3) | `op == "start"` → `(sid, false, None)`, with no `decide` | Yes |
| Step 4: with a pending detection, reconnect needs a new session id (400 otherwise) | Arm `(Some(p), true, _)` → 400 `SCHEMA_VIOLATION`. The `NewSession`/`TamperDetected` arm also requires it. | Yes |
| No step loses evidence | Step 1 keeps the bytes. Step 2 re-reports (closed side). Step 3 deletes only after the copy and records the unknown in the progress record. | Yes |
| §6 I-4 (가) row | Now `\| … \| … \| serve (당시) … ·— 이 시험은 … 대체됐다 \|`: 3 cells under the 3-column header | Renders |
| §8 | PR, review and merge SHAs, CI run, counts, the A6/L-2 release wording, the remaining preconditions, revision 11, and the rebuild of `ieo-role-call`/`ieo-coord` (58090) after 1c21177 | Accurate |

## (3) Ledger

- `state.json` diff against 1c21177:
  - snapshot 0044 and timestamps;
  - `phase`;
  - 02-05 `unresolved`: a revision-11 sentence appended;
  - one `notes` line (main CI 43718393840);
  - event `X-08-ACCEPTED-20261115`, equal to the new `events.jsonl` line, with evidence paths that exist;
  - `resume.last_verified_stage`, `branch_sha`, `running`, `next_action`, `current_work`, `blocking`;
  - X-08: `status` 검수 완료, `verification_level` verified, `result_state`, `evidence` (+ review-pr70-8e1ecf7.md), `next_action`, `unresolved` (349→350, post-acceptance items).

  Nothing else changed: `tasks[]` other than 02-05 `unresolved`, the budget, X-02, the baselines and the counts are untouched.
- `resume.blocking` keeps every existing real-call condition:
  - 02-05 step 8 needs the approved 02-05 budget, the budget-name/limits code PR, steps 3 and 4 merged, and a redeploy built after 1c21177. This matches PLAN.md §4.2.
  - X-02 needs executable registration, semantic review and its other conditions.
  - The 04-05 rule for real stops and the 1b GitHub collection rule stay.
- `resume.budget` is unchanged:
  - the 02-04 spares (dev 2, Guardrail 3) stay unused;
  - no budget is reset or reused, 01-04 included;
  - the X-02 budget is used only after X-08 acceptance and X-02's remaining conditions;
  - 02-05 runs only in step 8.
- Other places:
  - X-02 `next_action` starts with "X-08 검수 완료 →" and then lists registration, review and development.
  - 02-05 `next_action` keeps "예산 이름·상한 코드 PR → 단계 8".
  - X-08 `next_action` points rebuilds to step 8 and X-02 preparation.
  - The A6 notes stay historical. PLAN.md is unchanged.

  None of these reads as "real calls may start now".
- The 02-05 `unresolved` now ends with "X-08이 revision 10을 썼으므로 02-05의 계약 변경(PLAN.md의 "revision 10")은 revision 11부터 쓴다".

## Findings

### L-1 (Low): the REPORT header row "상태" is stale

The header table of REPORT.md still says: "사용자 결정 반영(§4, 18d5d9e)은 **독립 검토 전**. 이 기록은 X-08 완료가 아니다." §8 of the same file, and the ledger, now record X-08 as 검수 완료 after review 8e1ecf7. A reader who stops at the header gets the opposite status.

Suggested text: "PR #67·#68·#70 병합(6e5e502·e0a7f8a·1c21177). §8에서 검수 완료. 남은 사용자 결정 없음."

Not blocking. It can be fixed in this PR or the next one.

### I-1 (Info): §4.1 step 3 re-detection covers only a tamper that is still on disk

`configdir::clean` runs in `execute_inner` before the takeover checkpoint is written. Consider this case:

1. An earlier attempt passed `launch`, so cleanup ran.
2. Its takeover was then refused centrally, so the local checkpoint keeps the old digest.
3. The pending file was corrupted after that.

The config now matches the checkpoint again, so step 3's reconnect finds nothing. §4.1's last sentence covers this in general terms ("손상 전의 탐지가 이미 기록됐는지는 알 수 없는 채로 남으며"). The words "그 사이 변조가 있었으면 새 탐지를 만들고" could still be read more broadly than the code supports. "설정이 아직 변조된 채로 남아 있으면" would be exact.

### I-2 (Info): §4.1 step 2 re-reports without checking whether the detection was already recorded

The earlier procedure asked the person to check this first; the new one drops the check. `role_tamper_events` is unique only on `(repo_id, role, new_session_id)`. If a readable detection had in fact been recorded, a rewritten file is recorded again under the next session. That could happen, for example, if `remove_file` failed after `tamper_recorded: true`. The result is a second event for one tamper, which is over-reporting on the closed side, and no evidence is lost.

Optional: before step 2, look for an archived checkpoint under `/state/checkpoints/` whose `config_tamper_detected` has the same `old_session_id` and digests.

### I-3 (Info): `resume.budget` lists fewer step-8 preconditions than `resume.blocking`

The 02-05 budget line says "단계 8에만, X-08 검수 완료와 예산 코드 PR 뒤". It does not mention steps 3 and 4 or the rebuild. `blocking` and PLAN.md §4.2 carry them, so nothing is loosened. A resuming agent should read `blocking` together with the budget line.

### I-4 (Info): PLAN.md still says "revision 10" and "X-08 병합"

PLAN.md lines 53, 168, 259, 296, 304 and 308 say "X-08 병합". Lines 153, 204 and 313 say "revision 10". The ledger overrides both:

- revision 11, in 02-05 `unresolved`;
- "검수 완료" as the release point, in the notes, the decision record and REPORT §8.

The 02-05 step-1 PR can update PLAN.md so that it does not contradict the ledger.

### I-5 (Info, not from this PR): evidence path of event X-08-PR68-MERGED-20261115

The path `reports/tasks/02-04/caller-carryovers-20261115/review-pr68-3724bc9.md` is outside `reviews/`. It resolves, because the file was deliberately kept next to REPORT (REPORT §3). Noted only for consistency.

## Summary for the author

- Approve at b92a398.
- Optional in this PR: fix the stale REPORT header "상태" row (L-1). The two §4.1 wording refinements (I-1, I-2) are optional too.
- Update PLAN.md's "revision 10" and "X-08 병합" wording in the 02-05 step-1 PR (I-4).
