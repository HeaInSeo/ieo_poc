# Review of PR #63 at d1c86c5 (review 1)

- PR: HeaInSeo/ieo_poc#63, "report: X-03 accepted (contract 0c243ee) — snapshot 0036"
- Head: d1c86c55b448f1fb5fd0dff23c939f14596f425b (confirmed with `gh pr view 63 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 0c243ee (PR #62 merge). One commit on top of the base: d1c86c5. Branch `report/x03-accepted`.
- CI at d1c86c5 (run 43697126357), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking and no Medium findings.** The acceptance of X-03 is justified, the hand-over of review 8e3e817's findings to X-04 is complete and faithful, and the `state.json` diff contains only the intended changes. Counts stay at 12/73 and 0/12, the budget and blocking text are unchanged, and X-08 is unchanged.

- **Q-1 (Low):** the REPORT header "상태" row still ends at "교정 2(§6)". It does not mention the 8e3e817 approval or the new §7 acceptance.
- **Q-2 (Info):** `resume.current_work` lists "supervisor 클라이언트 전환 포함" under "장부 §7 X-04 수용 기준", while X-04 `unresolved` says this is not in the ledger acceptance criteria.

Neither needs to block the merge. Both are one-line text edits that can go into this PR or the next snapshot.

## What I ran

- A detached worktree of d1c86c5 from `ieo_poc-0204` in my scratch directory, removed at the end.
- `gh pr view 62 --json mergeCommit,headRefOid`: head 8e3e817, merge commit 0c243ee. `git log` of 0c243ee: parents 0779850 (main) and 8e3e817 (the approved head). origin/main is 0c243ee.
- `gh run view 43696890495`: push on main, headSha 0c243ee, conclusion success, all three jobs success.
- Read the verdicts of the archived reviews: b8506fd "REQUEST CHANGES" (review 1 of 4), 0de371c "APPROVE" (review 2), and the pending 8e3e817 "APPROVE" (review 3, corrections 2 of 3), no Blocking or Medium.
- `git diff 0c243ee d1c86c5` in full (8 files, +241/−34). `git diff --quiet 0c243ee d1c86c5 -- crates contracts docs/poc/Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md`: no difference.
- `cmp` of the archived `reviews/review-pr62-8e3e817.md` against `.git/ieo-dev-workspace/pending-reviews/review-pr62-8e3e817.md`: byte-identical (sha256 dfe0b7d2…).
- A JSON structural walk of `state.json` against 0c243ee.
- The new `events.jsonl` line compared with the last `state.json` event: equal. Its two evidence paths exist.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: `ok: true`, rc 0.
  - `scripts/validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` and `generated_views_match` true, accepted main tasks 12 of 73, followup denominator 12, 532 internal links checked.
- A manual check of the 10 relative links in REPORT.md: all resolve.
- `git diff --check 0c243ee d1c86c5`: clean. Grepped the added lines for secrets: the only hits are the archived review's sentences saying no secrets were found.
- Spot-checked `contracts/ieo-internal-v0.md` against the ledger §7 X-03 row and `Plan_gRPC_Internal_API_v0.md` §6.
- `gh pr checks 63 --watch`, then `gh pr view`.

## (1) Is the acceptance justified?

| Condition | Result |
| --- | --- |
| PR #62 merged on the approved head | Yes. `headRefOid` 8e3e817; merge 0c243ee has parents 0779850 and 8e3e817 |
| Last review approved that head | Yes. review-pr62-8e3e817.md: "APPROVE: no Blocking and no Medium findings", Lows P-1, P-2 and Info I-7..I-10, recommends merging 8e3e817 as it is and recording these as X-04 inputs |
| Required CI on the approved head | Yes. Run 43696529248, three checks SUCCESS (recorded in the review) |
| Main CI on the merge SHA | Yes. 43696890495, 0c243ee, success |
| Review count 3/4 | Right: b8506fd, 0de371c, 8e3e817 |
| Correction count 2/3 | Right: 0de371c (correction 1, for b8506fd) and 8e3e817 (correction 2, for 0de371c) |

Ledger §7 X-03 coverage (spot check, not a re-review), in `contracts/ieo-internal-v0.md`:
- Services and messages: §1.1, proto `SupervisorService` with five RPCs.
- Version compatibility: §2, no field number reuse, `reserved`, incompatible changes go to `ieo.internal.v1`, `buf breaking` `WIRE_JSON`.
- Authentication and per-role permissions with the same credentials and scopes as HTTP: §3.
- HTTP↔gRPC status mapping and detail reasons: §4.1–§4.3 (`ieo-error-bin`).
- Retry and idempotency, no re-execution on connection failure, confirmation by re-read: §5.
- Deadline and cancellation: §6.1, pre-commit expiry or cancellation rolls back; a disconnect after commit is resolved by re-read.
- The HTTP/JSON contract is kept and business rules are not redefined: §1.4 and §9 (`api::handle` reused).
- Split impact: §9.

The REPORT §7 claim "장부 §7 X-03 수용 기준의 요소가 모두 있다(§1)" holds.

## (2) Hand-over to X-04

REPORT §7 and X-04 `unresolved` against review 8e3e817:

| ID | Review | REPORT §7 / X-04 `unresolved` | Faithful |
| --- | --- | --- | --- |
| P-1 | Empty `request_id` and values with `?` (optionally `#`): answer `NOT_FOUND` after authentication; add to §10 item 8 | Same, with "설명 §3·§10 항목 8". Empty `repo_id` is correctly left out (the review says it already matches HTTP) | Yes |
| P-2 | One rule for `RESOURCE_EXHAUSTED` (simplest: outcome unknown, per §4.3); pre-check bounds path values (256/128 bytes); align §11 and REPORT §4; state the staged-copy handling | All four parts are listed | Yes |
| I-7 | Name the `format` and `seq` 400 checks before `REPO_MISMATCH` in §3 | Same | Yes |
| I-8 | Name the serialization (compact serde_json); note the `from_utf8_lossy` vs raw-bytes difference | Same | Yes |
| I-9 | Make REPORT §2 item 4's 02-05 wording conditional | Same | Yes |
| I-10 | Add DB `UNAVAILABLE` to the trailer list in §4.3 and the proto comment | Same | Yes |

Nothing is dropped. The optional note in P-1 (HTTP cannot address a stored `request_id` containing `/` or `?`) is not carried, but the review marked it optional.

02-05 and X-04 scope:
- `tasks[]` is unchanged, so no 02-05 prerequisite, acceptance or `unresolved` field changed. The ledger file is unchanged.
- X-04 `prerequisites` stay `["X-03"]` and its status stays 미착수. Its `unresolved` keeps "02-05의 선행·수용 기준은 바꾸지 않음". The appended text adds review inputs, not acceptance criteria.
- The new X-04 `next_action` sequence (작업 표 분리 → tonic 어댑터·metadata 인증·상태 대응·커밋 전 확인 → supervisor CoordinatorClient → 공통 회귀 §10) matches `Plan_gRPC_Internal_API_v0.md` §6 and contract §6.2, §9 and §10. It adds no scope.
- See Q-2 for one wording point in `resume.current_work`.

## (3) state.json structural diff against 0c243ee

Changed paths (all others identical):
- `snapshot_id` (0036), `snapshot_created_at`, `last_actual_work_observed_at` (18:00:27+09:00), `phase` ("X-03 계약 초안 PR" → "X-03 검수 완료(계약 0c243ee)").
- `notes` +1: the main CI record for 43696890495 (0c243ee).
- `events` +1: `X-03-ACCEPTED-20261115`, equal to the new `events.jsonl` line.
- `extra_tasks[2]` (X-03): `status` 검수 완료, `verification_level` verified, `last_observed_at`, `result_state` (appends 8e3e817 approve, merge, main CI, "73·12 미산입"), `evidence` +1 (the archived 8e3e817 review), `next_action` "없음(X-04로)", `unresolved` (appends the hand-over pointer to REPORT §7).
- `extra_tasks[3]` (X-04): `next_action` and `unresolved` only.
- `resume`: `last_verified_stage`, `branch_sha`, `running`, `next_action`, `current_work`.

Not changed: `baselines`, every `tasks[]` entry, `resume.budget`, `resume.blocking`, the whole X-08 entry, and the other `resume` fields. The generated views changed only in the snapshot and SHA lines, the STATUS "현재" line and timestamps, and the X-03 and X-04 rows.

Resume fields:
- `branch_sha`: main 0c243ee, PR #62 merge, main CI 43696890495 success, "이 수용 기록 PR 병합 전". Correct.
- `running`: "X-03 수용 기록 PR(report/x03-accepted)". Matches the PR branch.
- `next_action`: "X-03 수용 기록 PR 병합 확인 → X-04 → 02-05", X-08 before the first real model call. Correct and not stale.
- `current_work`: points at X-04 after this PR. Correct, apart from Q-2.
- `blocking` and `budget`: byte-identical to 0c243ee. X-08 blocking is kept.

## (4) Archive, links, validators, secrets, CI

- The archived review-pr62-8e3e817.md is byte-identical to the pending copy.
- The REPORT links (10) and the event evidence paths (2) resolve. The validator checked 532 internal links.
- Validators pass (above). No secrets in the added lines. `git diff --check` is clean.
- All three required checks are SUCCESS at d1c86c5 (run 43697126357). The head was still d1c86c5 after the checks finished.

## Findings

### Q-1 (Low): the REPORT header "상태" row does not record the acceptance

**Where:** `docs/poc/reports/extra/X-03/grpc-contract-20261115/REPORT.md`, header table, row "상태":
"계약 초안. 구현 없음(X-04). 독립 검토 b8506fd 변경 요청 … → 재검토 0de371c 승인(Low N-1~N-5) → 교정 2(§6)".

**Problem:** this PR adds §7 (acceptance, 8e3e817 approval, merge 0c243ee) but leaves the header row at the previous round. The earlier rounds updated this row each time (review 8e3e817 noted that the row was updated for correction 2). A reader who looks only at the header sees an unaccepted draft that is still in review.

**Effect:** none on the ledger. `state.json`, the views and §7 all say 검수 완료.

**Fix:** extend the row, for example: "… → 교정 2(§6) → [8e3e817 승인](reviews/review-pr62-8e3e817.md) → 병합 0c243ee, **검수 완료**(§7). 남은 Low·Info는 X-04 입력(§7)".

### Q-2 (Info): `resume.current_work` puts the supervisor client switch under the ledger X-04 acceptance criteria

**Where:** `resume.current_work`: "X-04 완료 조건: 장부 §7 X-04 수용 기준 — X-03 계약대로 gRPC 경로 추가, …, supervisor 클라이언트 전환 포함".

**Problem:** the ledger §7 X-04 row does not contain the supervisor client switch. X-04's own `unresolved` says so: "X-04 PR 설명·보고에 supervisor 클라이언트 전환을 명시(장부 수용 기준에는 없음)". The switch is X-04 scope through `Plan_gRPC_Internal_API_v0.md` §6 and the user decision, so the work is right, but the label attributes it to the ledger.

**Effect:** none. This field is a resume aid. The ledger and X-04's acceptance criteria are unchanged.

**Fix (optional):** "… 재전송 중복 반영 방지). 계획 §6에 따라 supervisor 클라이언트 전환도 X-04 범위(장부 수용 기준에는 없음). …".
