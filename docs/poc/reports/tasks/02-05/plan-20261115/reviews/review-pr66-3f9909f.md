# Review of PR #66 at 3f9909f (review 1)

- PR: HeaInSeo/ieo_poc#66, "design: 02-05 plan — pre-start rule decisions, model-free steps, real-call budget request draft; snapshot 0039"
- Head: 3f9909fcd247905fa9b8e3173a7659a5ec562d6c (confirmed with `gh pr view 66 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 433ec4c (433ec4c531168e26…). Two commits on top of the base: 07dba6f (PLAN.md) and 3f9909f (snapshot 0039, archived PR #65 review).
- CI at 3f9909f (run 43709422324), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - (not required) proto buf lint and breaking: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers, no builds. I did not read the secrets or runs directories. This is review 1 of at most 4.

## Verdict

**REQUEST CHANGES: three Medium findings, no Blocking.** The plan is complete against its sources. Every 02-05 carry-over appears in §1.2 (C1–C14) and is closed by a step or assigned elsewhere with a source. None of the four "02-05 수용 전 필수" hand-overs (1a, 2, 3 external requests, GitHub SHA·CI) is deferred out of 02-05. The budget section does not assume approval, keeps the 02-04 spares unused, does not reuse the 01-04/02-04/X-02 budgets, and keeps "no real call before X-08 merges". The ledger snapshot is clean. The Medium findings matter because the plan is the basis for the next PRs and for the user decision request:

- **M-1:** U2 option (나) is justified as "the set of rejected requests stays the same; only code priority changes". That is false for the placement the plan specifies ("저장 직전"). Some requests that get 409 today would get 200.
- **M-2:** the ledger marks F4, N5 and S1 as "02-05 착수 전". The snapshot moves 02-05 to 진행 and plans to implement steps 1–7 while F4 and S1 are still waiting for the user. The plan relaxes a recorded pre-start condition on its own authority. The user decision request does not say so.
- **M-3:** step 3 and `next_action` put the model-free steps 3–7 behind the X-08 merge. The recorded decision (A6, resume §6, 02-05 `unresolved`) says the model-free parts of 02-05, naming the coordinator's start·reconnect call and the limit enforcement, do not wait for X-08.

All three are text fixes in PLAN.md and in the resume and `next_action` fields. None needs code, a new task or a new prerequisite.

## What I ran

- Made a detached worktree of 3f9909f in my scratch directory and removed it at the end.
- From `docs/poc`, with `.venv-poc/bin/python -I`:
  - `scripts/render_progress.py --check`: ok, rc 0.
  - `scripts/validate_package.py`: rc 0.
  - `scripts/validate_contracts.py`: rc 0.
  - `scripts/test_render_progress.py`: RESULT PASS 0.
- `cmp` of the archived `reviews/review-pr65-62b8fa4.md` against `.git/ieo-dev-workspace/pending-reviews/review-pr65-62b8fa4.md`: identical.
- `sha256sum progress/state.json` = 7f2c29f9…, which matches the hash printed in STATUS, TASKS, RESULTS and RESUME.
- `git diff origin/main` on state.json, events.jsonl and the generated views. `git diff --check` is clean. Grepped the added lines for secrets: none.
- Resolved every relative link in PLAN.md (9 distinct): all exist. The event evidence path exists.
- Read the code at 3f9909f:
  - `crates/ieo-coord/src/api.rs`: `register`, `register_spec`, `guard_import_owned`, `convert`, `compare_checkpoint`, `judge_resume`, the GitHub review query, merge evidence.
  - `crates/ieo-spec/src/check.rs`: `check_registration` revision handling.
  - `crates/ieo-supervisor/src/{budget.rs,caller.rs,serve.rs}`.
- Read the documents:
  - `contracts/judgment-policy-v0.json` 0.6.0 (G1, G3, G5, G7, RG2) and `contracts/ieo-internal-v0.md` §1–§3, §8, §11.
  - OpenAPI 0.8.0 (order sentence at line 41, `checkpoint:report` text).
  - Runtime contract §4.3, §7 and §13 item 10. `SprintSpec_v0.schema.json` `limits`.
  - Sprints task 02-05, the ledger rows 02-05..02-07 and X-08, and the 02-04 ACCEPTANCE §2–§4, REPORT §4 and PREVENTION §1.3.
  - Validation plan §1.2, §1.9 and P21, and review 0fa839d N2 and N5, and review 74bd796 U2.
  - User decisions 2026-11-13 x02, 2026-11-14 grpc, 2026-11-15 0204-acceptance and 2026-11-15 resume.
- `gh pr checks` until all four completed, then `gh pr view` again (head unchanged).

## Completeness (state.json `tasks[02-05]`, ledger row, Sprints task 02-05)

| Source item | PLAN §1.2 | Closed by | Note |
| --- | --- | --- | --- |
| Acceptance / exit_condition text (source·digest·revision → delivery → actual takeover → first action → checkpoint; agent records stay claims, not required by the verdict) | header, §1.1 | steps 3, 4 (+ the claim-separation test in step 4) | Matches Sprints `02-05.accept` profile rev 4 verbatim. |
| Runtime contract rev 6 §10 item 5 (session_id only) | C1 | steps 3, 4 | |
| 02-03 N2 sprint_id binding | C2 | step 1 | See L-3. |
| F4, N5, S1 (02-05 착수 전) | C3–C5 | §2.1–§2.3 | See M-2. |
| X-01 U2 (착수 전) | C6 | §2.4, step 1 | See M-1. |
| 1a assignment limits (수용 전 필수) | C7 | step 5 | Not deferred. |
| 2 coordinator start·reconnect via `compare` (수용 전 필수) | C8 | step 3 | Not deferred. |
| 3 external-request tracking (수용 전 필수) | C9 | step 6 | Not deferred. Children stay with 04-05, `safe_shutdown_confirmed` stays false. This matches decision item 3. |
| GitHub SHA·CI comparison as final resume basis (수용 전 필수) | C10 | step 7 | Not deferred. The input is ingested evidence, because real collection is 1b by decision item 1. §5 states this limit (I-3). |
| Item 8 / X-08 block | C11 | §4.2, §5 | Kept. |
| f73ac13 P3, bc55edf R5 (non-blocking) | C12 | step 3 | |
| Token-holding process, HTTP+gRPC shape of the new ops | C13 | §2.5, step 2 | |
| 02-04 remaining limits | C14 | steps 4, 6, 7, §4 | |
| Sprints `outputs` (02-05-output, required) | — | steps 1–7, 9 | |

Nothing from the 02-05 entry, the ledger row or the Sprints task is missing.

### Facts I verified as correct

- `register` calls `guard_import_owned` before `register_spec`/`check_registration`, and the notion gate sits between them. `guard_import_owned` lets format errors and other repos fall through, so "its 409 precedes the 01-07 revision codes and the digest/reference 400s" is accurate.
- The OpenAPI order sentence is "format → reference → binding" (line 41) and OpenAPI is 0.8.0. The judgment policy is 0.6.0. The G1 text says a verdict under another policy digest never satisfies a prerequisite, so the S1 consequence is stated correctly. The G3 author-string sentence and `body->>'reviewer' <> body->>'author'` exist as quoted. G7 reads only `required=true`.
- `BUDGET_0204` is 12 runs, 1,800 s per run, 14,400 s total, 1.5M tokens and 15M cache_read. `BUDGET_X02` is 6 runs, 3,600 s, 14,400 s, 3M and 30M. `caller.rs` and `serve.rs` accept only `"02-04"` and `"X-02"`, so §4.2's "a code PR is needed before a 02-05 real run" is right.
- `judge_resume` returns exactly the six values that step 3 dispatches on. The supervisor answers 409 `ID_CONTENT_CONFLICT` to any reused `attempt_id` (serve.rs:599). Runtime contract §7 reserves that code for "same request_id, different content", so the planned replay change brings the code in line with the contract (I-2).
- `ieo-internal-v0.md` §3 says "새 범위는 만들지 않는다" and leaves the token-holding process to 02-05. §1.2 says the supervisor gets its own assignment through 02-05's self-assignment lookup. §8.2 says that 02-05 adds the RPCs because X-04 merged first. The plan follows all three.
- The validation plan §1.2 row "역할 Pod 수신" and the P21 row ("요청만으로 인수 인정 0건") exist as cited.
- §4.3 numbers are consistent: 4 × 600 s = 2,400 s, and 02-04 usage is billable 33,948 and cache_read 121,189, the same as `resume.budget`. The stop rules match decision 2026-11-13: stop on caps or a disallowed limit signal, unmeasured usage is never 0, no automatic retry, USD 0 extra. Option (나) explicitly forbids X-08-less runs and any use of 02-04, X-02 or 01-04 budgets.
- §4.1 is reasonable. With the fake runtime and real DB everything in the chain is testable except the real stream's first-assistant shape and one real pass through the coordinator path. ACCEPTANCE §2 confirms that the 7 real 02-04 runs did not pass the item 4·9 path.

## Findings

### M-1 (Medium): U2 (나) does not keep the rejected set the same at the planned position

**Where:** PLAN §2.4, "(나) 소유 검사를 01-07 검사(형식 → 참조 → revision·digest → 바인딩) 뒤, 저장 직전으로 옮긴다" and "거부되는 요청의 집합은 같고 바뀌는 것은 거부 코드의 우선순위뿐이다. 수용 결과·상태 변화는 바뀌지 않는다". The developer-authority classification rests on that sentence.

**Problem:** `check_registration` has two non-rejecting outcomes that do not write. `AlreadyRegistered` and `Superseded` (check.rs:240–255) both return 200 when the same (sprint_id, revision) is stored with the same digest in a covering mode. `register_spec` writes only on `AcceptedForSemanticReview`. Take an imported sprint whose latest import is rev 2, and re-post the exact stored rev 1 (from the earlier import) in draft mode:

- Today, `guard_import_owned` compares against the latest import. The `notion_page` digest and the tasks differ, so the answer is 409 `EVENT_CONTENT_CONFLICT`.
- With the guard "저장 직전" (before the INSERT/UPDATE), `check_registration` returns `Superseded` and the answer is 200 with no write.

So a request rejected today becomes accepted. The change is harmless for state (no write), but it changes an accepted X-01 response class (409 → 200). That is not "priority only", and the user would not learn of it from the plan.

**Fix:** place the guard after `check_registration` and before the state match, so that it also covers `AlreadyRegistered` and `Superseded`. Then the rejected set really is unchanged. Add the stale-identical-revision case to the (a)–(d) regression list. Alternatively, keep "저장 직전" but list the 409 → 200 case in §2.4 and the PR description as a behaviour change of X-01, and report it to the user with the F4 request.

### M-2 (Medium): 02-05 moves to 진행 with the "착수 전" rule decisions F4 and S1 still open

**Where:**
- state.json `tasks[02-05].unresolved`: "02-03 이월(02-05 착수 전): … F4 … N5 … S1" and "X-01 이월(02-05 착수 전): … U2".
- The snapshot sets `status: 진행`.
- `resume.current_work`: "F4·S1은 사용자 결정·확인 대기이지만 02-05 구현 단계와 무관해 막지 않는다".
- PLAN §2.3 says "확인 전에도 02-05는 진행할 수 있다". PLAN §4.3 says "결정을 받기 전까지 단계 1~7을 진행한다".

**Problem:** PR #65 review 62b8fa4 Q-1 asked for the order to be fixed so that these rules are settled before 02-05 starts. This PR settles N5, U2 and T1 by developer authority. It then classifies F4 as a user decision and S1 as a user confirmation, and starts 02-05 anyway. The argument may well be right: no step in 1–7 reads the G3 author rule or the G1 `outputs` comparison, and the policy file is not touched. But it changes the timing condition recorded in the ledger from "before start" to "before 1b" (F4) and "whenever" (S1). That is a sequencing change of a recorded pre-start condition, made on the developer's own authority. User decision 2026-11-15 §6 asks for such changes to be reported with reasons and alternatives. The PLAN header also says "이 문서는 02-05를 '진행'으로 올리지 않는다", so the PR contradicts itself (L-1).

**Fix:** one of these two.
- Put the relaxation into the user decision request, for example "F4·S1 결정 전에 모델 호출 없는 단계 1~7을 진행해도 되는지", with the reason (no step reads G3 author or G1 outputs). Keep the ledger wording "착수 전" until the user answers.
- Or keep 02-05 at planning only (no implementation PR) until F4 and S1 are answered.

In both cases, say in `unresolved` which of F4, N5, S1 and U2 is settled and by whom. The current text already does that.

### M-3 (Medium): steps 3–7 are queued behind the X-08 merge, against the recorded "does not wait for X-08"

**Where:**
- PLAN step 3, last bullet: "이 부분은 X-08이 고치는 코드와 겹치므로 X-08 병합 뒤 그 위에서 한다".
- PLAN §5: "단계 3은 X-08 병합 뒤 그 위에서 한다".
- `tasks[02-05].next_action` and `resume.next_action`: "단계 1·2 → X-08 PR/병합 → 단계 3~7".

**Problem:** A6, User_Decisions_2026-11-15_resume §6 and the 02-05 `unresolved` all say: "02-05의 모델 호출 없는 부분(코디네이터 start·reconnect 호출, 배정 한도 집행 등의 가짜 런타임 시험)은 X-08을 기다리지 않는다". The plan quotes that rule in §5 and then makes it conditional ("X-08이 먼저 열려 있으면"). The two `next_action` fields drop the condition. A resuming agent would read X-08 as a gate for 02-05 steps 3–7, and therefore for 02-05 acceptance. The X-08 ledger row lets X-08 stop for a user decision on an item it does not fix. If that happens, 02-05 would stall on a task that the user said it does not depend on.

Only the shared budget-check library has to sit on top of X-08's `caller.rs` changes, and that library matters only before step 8 (real runs). Delivery, compare dispatch and fake-runtime tests do not need it.

**Fix:**
- In step 3 and §5, split the budget-check library sharing (after X-08, required before step 8) from the rest of step 3 (no X-08 dependency).
- Write `next_action` as a preference, for example "X-08을 단계 3 전에 병합하는 것을 선호, X-08이 늦으면 단계 3~7의 모델 호출 없는 부분은 먼저 진행".
- Keep "단계 8은 X-08 병합 뒤" as it is.

### L-1 (Low): PLAN header contradicts the PR

**Where:** PLAN header row "상태": "이 문서는 02-05를 '진행'으로 올리지 않는다 … 원장(`progress/*`)은 이 PR에서 바꾸지 않는다".

**Problem:** commit 3f9909f in the same PR sets 02-05 to 진행 and rewrites `progress/*` (snapshot 0039). The sentence was true of commit 07dba6f alone.

**Fix:** say that the snapshot in the same PR records 02-05 as 진행 (plan stage) and that 12/73 and 0/12 are unchanged.

### L-2 (Low): N5 (다) overstates its effect and hands work to 02-06/02-07 without recording it there

**Where:** PLAN §2.2: "병합이 없으면 판정은 `accepted`가 될 수 없으므로, 판정 규칙을 바꾸지 않고 결과가 GitHub와 어긋나지 않는다", and "닿는 곳: … 병합 요청 조건(02-06), 판정 문구(02-07)".

**Problem:**
- The verdict accepts `merge` evidence from `evidence:ingest` (api.rs:3182). That evidence does not come from a coordinator merge request. A merge done outside the coordinator, for example an admin merge while `reviewDecision` is CHANGES_REQUESTED, still yields `accepted`. That is exactly the probe in 0fa839d N5. So (다) prevents the divergence only for merges the coordinator requests.
- The "외부 상태 불일치" mark adds an input that shows up in verdict output. G3 enumerates the review inputs, (a) and (b). The plan should say that the new input never changes `outcome` and is not a G3 review input. Otherwise it is the policy change that (나) is said to be.
- The hand-over to 02-06 and 02-07 exists only in 02-05's `unresolved`. The F4 section says "승인되면 1b 작업의 `unresolved`에 적는다", but N5 has no such step.

**Fix:**
- Narrow the claim to "for merges the coordinator requests".
- State that the PR-level input affects neither `outcome` nor the G3 input set.
- Record the 02-06/02-07 hand-over in those tasks' `unresolved` in a later snapshot, or say in §2.2 when it will be recorded.

Keeping N5 as developer authority is acceptable with these limits. It is reported to the user together with F4.

### L-3 (Low): C2 (sprint_id reservation) is not in the §2 authority table and leaves the "already owned by another repo" case open

**Where:** PLAN step 1, C2: "다른 repo의 등록·변환은 첫 등록 여부와 상관없이 409 `SPRINT_REPO_CONFLICT`다".

**Problem:** policy RG2 says "A sprint_id belongs to the repo of its first registered revision". Suppose a sprint_id is already registered by repo B and is then named in repo A's binding. Under the planned rule, B's next revision is refused ("첫 등록 여부와 상관없이"), and under RG2 A's revision is refused too. That leaves a sprint that nobody can register, and it contradicts RG2's ownership sentence. The direction itself (N2 → 02-05 binding) comes from the accepted 02-03 report. Its interaction with RG2, however, is a policy-meaning question, like F4 and S1, and it is missing from the §2 table.

**Fix:** add C2 to §2. Handle the conflict as X-01 U3 did: a startup check that refuses a binding whose sprint_id is already owned by another repo, so RG2 is never contradicted at runtime. Otherwise mark it for the user.

### L-4 (Low): the plan does not restate that gRPC must not block 02-05

**Where:** PLAN §3 common rules and step 2. Step 2 includes "proto에 RPC·메시지 추가, `buf breaking` 통과" and "두 전송의 같은 답", and step 3 depends on step 2.

**Problem:** contract §8.2 makes the RPC addition 02-05's job, because X-04 merged first. That part is correct. The user decision of 2026-11-14 and the resume instruction, together with X-04 `unresolved` ("02-05의 선행·수용 기준은 바꾸지 않음"), still say that gRPC must not block 02-05. As written, a gRPC adapter problem in step 2 would hold steps 3–9.

**Fix:** add one sentence. The HTTP form and its tests are what 02-05 depends on. If the gRPC form of the new operations slips, steps 3 onward continue on HTTP, and the RPC follows in a separate PR without changing 02-05's acceptance.

### L-5 (Low): budget request draft wording

**Where:** PLAN §4.2, §4.3.

- §4.2 says "02-04 예산은 7/12회 사용으로 닫혔다". Decision item 7 and ACCEPTANCE §3 say the spares are **kept** unused until a new decision ("새 사용 결정 전 사용 금지"). They are not closed. Write "7/12회 사용, 남은 5회는 사용하지 않고 유지(전용 금지)".
- In (나), "3회 + 예비 1회(개발 3·Guardrail 1의 몫)" leaves implicit that the spare is development-only. The planned runs are R1 and R2 (development) and G1 (guardrail). Say "예비 1회는 개발 역할만". Alternatively, let the spare re-run any of the three after an environment fault.
- In (가), "04-03·EARLY-01의 실제 회차에서 확인합니다" gives those tasks new verification content. Neither the ledger row of 04-03 nor EARLY-01's entry says so. Write "(가)를 고르면 이 두 확인을 04-03·EARLY-01의 `unresolved`에 미확인 이월로 적는다", so the user sees the hand-over.
- The draft is otherwise fair. (가) is presented as a full option ("02-05의 모델 호출 없는 단계는 그대로 진행"), the recommendation is labelled as such, and the numbers are consistent.

### L-6 (Low): step 3 and step 5 leave two items undefined

- Step 3 puts the budget check into a library "shared by the delivery path and the caller", but does not say where it runs. §5 says the current authority sits outside the container. The supervisor runs as the same UID as the model (runtime contract §2.1), so a check that runs only inside the supervisor would be weaker. State that the authoritative check runs in the coordinator or the host caller, outside the role container. The supervisor's `config.budget` stays a backstop.
- Step 5's `limits` = min(assignment remainder, approved budget remainder, "역할 몫"). "역할 몫" has no defined source. The plan also says no new numbers are set (policy `numeric_values`). Name the source (for example a per-role cap in the approved budget) or drop the term.

### I-1 (Info): T1 widens what `checkpoint:report` covers, inside the contract

The new self-assignment lookup returns the sources, digest, limits and image expectations. With `recordFirstAction` and `reportSessionStatus`, the scope grows from "report and read own checkpoints" to "read own assignment and report session facts". This is anticipated by `ieo-internal-v0.md` §1.2 (self-assignment lookup is 02-05's) and §3 (no new scope), so developer authority is correct. In the step-2 PR:
- Update the OpenAPI scope text for the new operations.
- Note that a same-UID model process can read the token and forge these claims. The plan already treats them as claims.
- OpenAPI line 23 ("Agent credentials carry only `agent:*` scopes") is already inconsistent with `checkpoint:report`. It could be corrected in the same PR.

### I-2 (Info): attempt_id replay aligns code with the runtime contract

Today the supervisor answers 409 `ID_CONTENT_CONFLICT` for any reused `attempt_id`. Runtime contract §7 uses that code only for "same request_id, different content". The planned replay of the stored result for the same content is therefore a correction toward the contract, not a meaning change. Record it in revision 10 as such.

### I-3 (Info): step 7 compares against ingested evidence only

This is consistent with decision item 1 (real collection, identity and PR-level state go to 1b). §5 states the limit. The acceptance report should repeat it next to C10, so that "GitHub 대조" is not read as real collection.

## Ledger snapshot 0039

- The `state.json` diff against 433ec4c contains only the intended changes:
  - snapshot id and times, and `phase` (+ "02-05 진행(계획)").
  - `tasks[02-05]`: status 진행, report and evidence = PLAN.md, `result_state`, `next_action`. `unresolved` is the old text unchanged, with the plan's pending decisions appended.
  - One main-CI note (43708153941, 433ec4c).
  - Event `02-05-PLAN-20261115`, also in events.jsonl.
  - `resume`: `last_verified_stage`, `branch_sha` 433ec4c, `running`, `saved_reports` + PLAN.md, `next_action`, and `current_work` with the 62b8fa4 Q-1 order.
  - X-04 evidence + `review-pr65-62b8fa4.md`, which is byte-identical to the pending copy.
- Unchanged:
  - `baselines` (73/12), 12/73 accepted, 0/12 follow-ups.
  - `current_task` and `next_task` (02-05).
  - `resume.budget` and `resume.blocking` (X-08 block).
  - X-08 entry (미착수).
- Validators pass. Generated views match. Links resolve. There are no secrets. The 3 required checks are green at 3f9909f.
- M-2 and M-3 apply to the `current_work` and `next_action` wording in this snapshot.

## Summary

| ID | Severity | Topic |
| --- | --- | --- |
| M-1 | Medium | U2 (나) at "저장 직전" turns some 409s into 200 (Superseded/AlreadyRegistered); "same rejected set" is false |
| M-2 | Medium | 02-05 to 진행 with "착수 전" F4/S1 open; relaxation not put to the user |
| M-3 | Medium | steps 3–7 queued behind X-08 against "model-free parts do not wait for X-08" |
| L-1 | Low | PLAN header says the ledger is not changed and 02-05 is not raised |
| L-2 | Low | N5 (다) claim holds only for coordinator-requested merges; 02-06/02-07 hand-over unrecorded |
| L-3 | Low | C2 reservation vs RG2 for already-owned IDs; not in §2 table |
| L-4 | Low | gRPC non-blocking for 02-05 not restated; step 2 bundles RPCs |
| L-5 | Low | budget draft: "닫혔다" vs "유지", spare role, (가) hand-over to 04-03/EARLY-01 |
| L-6 | Low | budget-check location; undefined "역할 몫" |
| I-1 | Info | T1 widens `checkpoint:report`, within contract §1.2/§3 |
| I-2 | Info | attempt_id replay = alignment with runtime contract §7 |
| I-3 | Info | step 7 compares ingested evidence only |

**REQUEST CHANGES 3f9909f.** M-1 to M-3 are text fixes. The Low items can go in the same revision.
