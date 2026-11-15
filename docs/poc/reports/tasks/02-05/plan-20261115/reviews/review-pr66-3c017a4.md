# Review of PR #66 at 3c017a4 (review 2)

- PR: HeaInSeo/ieo_poc#66, "design: 02-05 plan — pre-start rule decisions, model-free steps, real-call budget request draft; snapshot 0039"
- Head: 3c017a4f7b43be5543d4fcb0c54577b40686d313 (confirmed with `gh pr view 66 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 433ec4c (433ec4c531168e26…). Four commits on top of the base: 07dba6f, 3f9909f (reviewed in review 1), then correction 1: bebcc19 (PLAN.md) and 3c017a4 (ledger, archived review 3f9909f).
- CI at 3c017a4 (run 43710424940), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - (not required) proto buf lint and breaking: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers, no builds. I did not read the secrets or runs directories. This is review 2 of at most 4. Correction 1 of 3 was applied after review 1.

## Verdict

**APPROVE 3c017a4, with Low findings. No Blocking or Medium findings.**

All three Medium findings of review 1 are fixed:
- M-1: the ownership guard now covers every non-rejected 01-07 outcome, so the rejected set stays the same.
- M-2: implementation waits for F4/S1 unless the user chooses otherwise.
- M-3: X-08 gates only step 8 in PLAN and in the 02-05 ledger entry.

All Low and Info items are addressed. The correction introduces no new scope or prerequisite that is not already in the 02-05 sources. The one added step-8 precondition, the shared budget-check library, is the plan's own work.

What remains is one wrong factual claim, some wording gaps, and one plan-versus-ledger inconsistency:
- The plan says no X-01 test expectation changes. One test does change (N-1).
- `notion_gate` coverage of `AlreadyRegistered` is not stated, and the new order sentence leaves out the repo-ownership 409s (N-2).
- The heading and closing line of §4.3 make the budget answer look like a pre-start condition (N-3).
- `resume.next_action` still reads X-08 as a step before 1–7 (N-4).
- §2.2 says the N5 hand-over is not recorded in this PR, but the ledger in this PR records it (N-5).

None of these changes a decision or a number. They can go in a final text touch-up, or N-1 and N-2 can be carried as explicit inputs to the step-1 PR and N-4 and N-5 to the next snapshot.

## What I ran

- Made a detached worktree of 3c017a4 at `scratchpad/r66b` and removed it at the end.
- Read review 1 (`pending-reviews/review-pr66-3f9909f.md`) and `git diff 3f9909f 3c017a4`, then the whole of PLAN.md at 3c017a4.
- From `docs/poc`, with `.venv-poc/bin/python -I`:
  - `scripts/render_progress.py --check`: ok, rc 0.
  - `scripts/validate_package.py`: rc 0.
  - `scripts/validate_contracts.py`: rc 0.
  - `scripts/test_render_progress.py`: RESULT PASS 0.
- `cmp` of the archived `reviews/review-pr66-3f9909f.md` against the pending copy: identical.
- `sha256sum progress/state.json` = 69538c90…, which matches the hash in STATUS, TASKS, RESULTS and RESUME.
- `git diff --check origin/main 3c017a4`: clean. Secret-pattern grep over the added lines: none.
- Resolved all 10 distinct relative links in PLAN.md: all exist.
- Read the code at 3c017a4:
  - `crates/ieo-coord/src/api.rs`: `register`, `register_spec` (RG2 check, state match), `idempotent_lookup`, `guard_import_owned`, `notion_gate`, `convert`.
  - `crates/ieo-spec/src/check.rs`: `check_registration` steps 1–8, `AlreadyRegistered`/`Superseded`, `UnboundDraft`.
  - `crates/ieo-coord/tests/coordinator.rs`: every test that expects `EVENT_CONTENT_CONFLICT`, `IMPORT_GATED` or `IMPORT_INCOMPLETE` on a `/v0/sprints` registration, and the helpers `derived_executable`, `ready_from` and `promote_review_assign`.
- Read the documents:
  - The 02-04 RUN_PLAN role shares (line 32).
  - User decisions 2026-11-13 x02 ① (budget table, stop rules) and 2026-11-15 resume §3 and §6.
  - The X-08 ledger entry.
- `gh pr view` / checks until all four completed. The head was unchanged.

## (a) Review 1 findings

| ID | Status | Verification |
| --- | --- | --- |
| M-1 | **Fixed** (with N-1, N-2) | §2.4 "위치(중요)" places the guard right after `check_registration` and applies it to all non-rejected outcomes (new 201, `AlreadyRegistered`, `Superseded`). It includes the stale-identical-revision example and adds that case to the regression list. This placement leaves the rejected set unchanged. I checked this against the code. All checks run inside one transaction under the registry lock, and nothing is written before the state match. `guard_import_owned`'s predicate does not depend on the `check_registration` result. So a request is rejected iff some check rejects it, in any order. The 409 → 200 case of review 1 no longer arises. The claim "no X-01 test expectation changes" is false for one test (N-1). |
| M-2 | **Fixed** | §2.6 says "이 조건을 완화하지 않는다", and also: "사용자가 F4와 S1에 답하기 전에는 구현 단계 1~7을 시작하지 않는다". The §3 common rule repeats it. §4.3 offers the early start as a separate "선택 사항 · 착수 시점": the reason is given, no permission is assumed, and the default is to wait. In state.json, 02-05 `next_action` says "답을 받은 뒤 구현 단계 1~7", and `unresolved` keeps the original "02-05 착수 전" text and adds "F4·S1 답 뒤에 시작 … 사용자가 원하면 그 전 착수를 허용할 수 있다". `resume.current_work` and `resume.next_action` agree. Nothing in PLAN or the ledger lets implementation start before the answers without the user's choice. The status stays 진행 (계획 단계). That is review 1's second fix option (planning only, no implementation PR). |
| M-3 | **Fixed** (with N-4) | Step table text: "X-08은 단계 3~7의 선행이 아니다 … X-08 병합이 필요한 것은 단계 8뿐". Step 3 splits out the budget-check library (after X-08, before step 8) and labels the order as "선호" ("X-08이 늦으면 기다리지 않고 진행"). §4.2 and §5 match. 02-05 `next_action` says "X-08은 관문이 아님 … 선호". `unresolved` keeps "X-08을 기다리지 않는다". `resume.blocking` is unchanged. `resume.next_action` still reads sequentially (N-4). |
| L-1 | Fixed | The header row says that the snapshot in the same PR records 진행 (계획 단계) and that 12/73 and 0/12 are unchanged. |
| L-2 | Fixed (with N-5) | §2.2 "효과의 한계": the claim is limited to coordinator-requested merges. The new input changes neither `outcome` nor the G3 inputs (a)/(b). The hand-over is recorded in the 02-06 and 02-07 `unresolved`. |
| L-3 | Fixed | C2 is in the §2 table. The startup check refuses a binding whose sprint_id another repo already owns, as in X-01 U3, so RG2 is never contradicted at runtime. The exposure limit for unbound IDs is stated. |
| L-4 | Fixed | §3 common rule: "gRPC는 02-05를 막지 않는다". Steps 3 onward continue on HTTP, and the RPC follows in a separate PR. Step 2 may merge with HTTP tests only. Matches resume §3. |
| L-5 | Fixed | The spares are now "사용하지 않고 유지 … 전용하지 않는다". The spare is "개발 역할에만", and only to re-run R1/R2 after an environment fault, following a cause report. G1 is not repeated. (가) records the two checks as unverified carry-overs in the 04-03 and EARLY-01 `unresolved`. |
| L-6 | Fixed | The authoritative budget check runs outside the role container (coordinator or host caller). The supervisor's `config.budget`/`share` is a backstop. "역할 몫" is defined, with the 02-04 source. The RUN_PLAN numbers (8 runs, 9,600 s, 1,000,000, 10,000,000 / 4 runs, 4,800 s, 500,000, 5,000,000) match RUN_PLAN line 32. |
| I-1 | Addressed | §2.5 "범위 문장". The OpenAPI scope text and the `agent:*` sentence are to be fixed in the step-2 PR. |
| I-2 | Addressed | Step 3: the replay is a correction toward runtime contract §7, recorded in rev 10. |
| I-3 | Addressed | Step 7 "남는 것" and §5: the acceptance report repeats the limit next to C10. |

## (b) New findings

### N-1 (Low): one X-01 test expectation does change under the new order

**Where:** PLAN §2.4 "X-01 시험의 기대 코드": "main 433ec4c의 `crates/ieo-coord/tests/coordinator.rs`를 읽은 범위에서는 바뀌는 기대가 없다".

**Problem:** `an_imported_draft_with_unresolved_reasons_cannot_execute` (coordinator.rs about line 1735–1742) re-posts the stored imported draft rev 1 with `mode=executable`. That spec has `registration_state: draft` and non-empty `unresolved_bindings`. It expects `409 IMPORT_GATED`, and its comment states the current order: "refused by the import gate … (C2), before the binding check".
- The stored row is draft-mode, so `covers` is false and the outcome is not `AlreadyRegistered`.
- `check_registration` step 8 then rejects with `UnboundDraft`.
- Under the planned order (01-07 including the binding check → ownership → `notion_gate`), the answer becomes `422 UNBOUND_DRAFT`.

This is exactly the class in the §2.4 table row "출처 대조에 걸리고 … 01-07 … 오류도 있음 → 01-07의 그 코드". The rejected set is unchanged, so the developer-authority classification and the §4.3 report wording ("거부 집합 동일, 겹치는 거부의 첫 코드만 바뀜") stay correct. Two things are wrong:
- The sentence "no expectation changes" is wrong.
- The change also reverses a gate-before-binding order that X-01 C2 deliberately fixed in this test. That goes beyond U2's own subject, which is ownership versus the 01-07 revision checks.

I checked the other `IMPORT_GATED` and `EVENT_CONTENT_CONFLICT` registration tests. They send ready, new-revision, correctly digested specs (`derived_executable`, `ready_from`, `spec(...)`), so their expected codes stay the same. That includes h2's "stale before gate", because ownership stays ahead of the gate.

**Fix:** replace the sentence with the known change: test name, 409 `IMPORT_GATED` → 422 `UNBOUND_DRAFT`, and the comment to update. In the U2 report line in §4.3, mention that the import gate now also follows the 01-07 binding check. Alternatively, keep `notion_gate` before the binding check and explain why. Then check the effect on the h2 order. Either way, list it in the step-1 PR description, as §2.4 already promises.

### N-2 (Low): `notion_gate` coverage and the full order sentence are not stated

**Where:** PLAN §2.4 "위치", "순서(새 문장)", "닿는 곳".

**Problem 1: `notion_gate` coverage.** The plan states explicitly that the ownership guard covers `AlreadyRegistered` and `Superseded`. For `notion_gate` it says only "소유 검사 뒤에 그대로 둔다" and "검사와 저장 사이".

Today, `notion_gate` runs on every format-valid executable request before `register_spec`, so it also covers same-revision resubmissions. It also covers copies under other sprint ids, which are not import-owned.

Example: a copy registered executable while its page was released, then a re-gating import arrives (`c2_a_re_gating_import_stops_a_running_assignment` shows that this happens). Re-posting the copy unchanged gives `AlreadyRegistered`.
- Today: 409 `IMPORT_GATED`.
- If the gate is implemented "before the INSERT/UPDATE" only: 200.

That is the M-1 pattern again, on the second check. It is harmless for state, because there is no write and `assign` checks the gate again, but it changes the rejected set.

**Problem 2: missing 409s in the order sentence.** The new order sentence, which goes into OpenAPI, does not place two checks:
- the RG2 check `SPRINT_REPO_CONFLICT`, which sits at the top of `register_spec`, before `check_registration` and so before its format check;
- the C2 reservation 409 from step 1.

**Fix:**
- State that `notion_gate` sits at the same point as the ownership guard and applies to all three non-rejected outcomes.
- Add "re-gated page, unchanged executable copy re-posted → 409 `IMPORT_GATED`, not 200" to the regression list.
- Place RG2 and the reservation check in the order sentence. If they stay first, say so, because then a malformed body that names another repo's sprint keeps getting 409 before 400, as it does today.

### N-3 (Low): §4.3 makes the budget answer look like a pre-start condition

**Where:** PLAN §4.3.

**Problem:** The quoted request is titled "02-05 착수 전 결정 세 가지", and item 3 is the real-call budget. The closing line says "사용자 답을 받기 전에는 이 계획 PR과 X-08만 진행한다". §2.6 and the ledger make only F4 and S1 the start condition, and the budget gates only step 8.

Read literally, a user who answers F4 and S1 but defers the budget would hold back steps 1–7. That is a stricter condition than the plan intends, and the user would set it unawares.

The rest of the request is fair and consistent:
- (가) is a full option ("추가 사용량 0", with the hand-over named).
- The recommendation is labelled.
- The numbers add up: 4 × 600 = 2,400 s (1,800 + 600); 150,000 + 50,000 = 200,000; 1.5M + 0.5M = 2M.
- The stop rules match decision 2026-11-13 ①: stop on caps or a disallowed limit signal, unmeasured usage is never 0, no automatic retry, hold the call if `overageStatus` cannot be confirmed, USD 0. The spare re-run follows a cause report and is not automatic.
- The early-start option is the user's choice, with the default stated.

**Fix:**
- Title the request, for example, "02-05 결정 요청: 착수 전 규칙 2건(F4·S1)과 실제 회차 예산".
- Mark item 3 as "단계 8에만 필요, 답이 늦어도 단계 1~7은 F4·S1 답 뒤 진행".
- Change the closing line to "F4·S1 답(또는 착수 시점 선택) 전에는 …".
- Optional: the early-start option gives only the reason for; one clause on what waiting costs would make it fully neutral.

### N-4 (Low): `resume.next_action` still reads X-08 as a step before 1–7

**Where:** state.json `resume.next_action`: "… → (답을 기다리는 동안) X-08 PR → 답 뒤 02-05 단계 1~7 → 9. PR은 하나씩".

**Problem:** The 02-05 entry says "X-08은 관문이 아님", but the resume line does not. With "PR은 하나씩", an X-08 PR that stays open is enough to hold the step-1 PR. The X-08 row explicitly allows X-08 to stop for a user decision on an item it does not fix. In that case a resuming agent reading only `resume` would stall 02-05 on X-08. That is the M-3 situation again, this time through the one-PR rule.

**Fix:** add "X-08은 단계 1~7의 관문이 아님. X-08이 사용자 결정으로 멈추면 그 PR을 두고 02-05 단계 PR을 진행". Or say how the one-PR rule applies in that case.

### N-5 (Low): §2.2 and the ledger disagree on where the N5 hand-over is recorded

**Where:** PLAN §2.2: "02-06·02-07로 넘기는 몫은 그 작업들의 원장 `unresolved`에 다음 원장 갱신에서 적는다(이 계획 PR의 원장 몫이 아님)".

**Problem:** commit 3c017a4 in this PR already adds the N5 text to the `unresolved` of both 02-06 and 02-07. Recording the hand-over now is the better outcome, and it closes review 1 L-2. The PLAN sentence is now false.

**Fix:** change it to "이 PR의 snapshot에서 02-06·02-07 `unresolved`에 적었다". A related point: the 02-06 text could repeat the scope limit "코디네이터가 요청하는 병합만". The 02-07 text already says that the outcome and the G3 inputs are unchanged.

### I-1 (Info): ledger details

- 02-05 `result_state` lists "U2·T1 개발자 권한" but not C2. `unresolved` does mention C2.
- The correction rewrote state.json but kept `snapshot_id` 0039 and the timestamps 20:32:15. The ledger hash changed, from 7f2c29f9… to 69538c90… That is acceptable within one unmerged PR, but the times no longer mark the last ledger change.
- The event `02-05-PLAN-20261115` says "02-05 착수(계획)". It is append-only and is accurate in the "planning stage" sense used by the header.

## (c) General

- **Ledger diff.** The `state.json` diff against origin/main 433ec4c contains only the intended changes:
  - Snapshot metadata and `phase` (+ "02-05 진행(계획)").
  - `tasks[02-05]`: status 진행, report and evidence (PLAN.md plus the archived review), `result_state`, `next_action`. `unresolved` has its old text verbatim plus the plan paragraph.
  - The 02-06 and 02-07 `unresolved` gain the N5 hand-over (N-5).
  - One main-CI note (43708153941, 433ec4c).
  - The event, also present in events.jsonl.
  - `resume`: `last_verified_stage`, `branch_sha` 433ec4c, `running`, `saved_reports`, `next_action`, `current_work`.
  - X-04 evidence + `review-pr65-62b8fa4.md`.
- **Unchanged:**
  - `baselines` (73/12), 12/73 and 0/12 in STATUS, `current_task`/`next_task` 02-05.
  - `resume.budget` and `resume.blocking`, which is the X-08 block.
  - The X-08 entry (미착수, same `next_action` and `unresolved`).
- **Archive.** The archived `review-pr66-3f9909f.md` is byte-identical to the pending copy.
- **Checks.** Links resolve, the validators pass, the generated views match, and there are no secrets. The 3 required checks are green at 3c017a4.

## Summary

| ID | Severity | Topic |
| --- | --- | --- |
| M-1 (r1) | fixed | guard covers all non-rejected outcomes; rejected set identical |
| M-2 (r1) | fixed | implementation waits for F4/S1; early start is the user's option |
| M-3 (r1) | fixed | X-08 gates only step 8 (PLAN, 02-05 entry) |
| L-1..L-6, I-1..I-3 (r1) | fixed / addressed | — |
| N-1 | Low | `an_imported_draft_with_unresolved_reasons_cannot_execute` changes 409 `IMPORT_GATED` → 422 `UNBOUND_DRAFT`; "no expectation changes" is false; gate now follows the binding check |
| N-2 | Low | `notion_gate` coverage of `AlreadyRegistered`/`Superseded` unstated (409 → 200 risk); order sentence omits RG2/reservation 409 |
| N-3 | Low | §4.3 title and closing line make the budget answer look like a start condition |
| N-4 | Low | `resume.next_action` + "PR은 하나씩" can still make X-08 gate steps 1–7 |
| N-5 | Low | §2.2 says the N5 hand-over is not in this PR's ledger; it is |
| I-1 | Info | `result_state` omits C2; snapshot times unchanged after the correction; event wording |

**APPROVE 3c017a4.** The Low items are text fixes. They can go in a final touch-up, or be carried explicitly: N-1 and N-2 into the step-1 PR description and tests, N-4 and N-5 into the next snapshot.
