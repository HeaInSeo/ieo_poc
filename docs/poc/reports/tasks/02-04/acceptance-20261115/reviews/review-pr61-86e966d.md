# Review of PR #61 at 86e966d (review 1)

- PR: HeaInSeo/ieo_poc#61, "report: resume record and item 8 owner X-08 (amendment A6), PR #60 review N-1..N-3 — snapshot 0034"
- Head: 86e966db7506429ad935e1ca8e0ad3eae30c1a91 (confirmed with `gh pr view 61 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 3157c89 (one commit on top of 3157c89)
- CI at 86e966d (run 43694087680):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories.

## Verdict

**Approve, no blocking or medium findings.** The PR does what the user instruction asks, inside the existing scope. Counts, budget, the D3 failure and the 02-04 acceptance are unchanged. The blocking rule is consistent in every place I checked. There are three Low findings and four Info notes.

L-1 is a wrong section reference inside the amendment's acceptance text. That text is pinned by sha256 (A6 `files` and ledger §7), so a later fix would need another amendment. It is cheapest to fix in this PR before merge.

## What I ran

- A detached worktree of 86e966d in my scratch directory, removed at the end. Using `.venv-poc/bin/python` from `docs/poc`:
  - `scripts/render_progress.py --check`: `ok: true`, rc 0.
  - `scripts/validate_package.py`: rc 0, `schema_errors: 0`, accepted main tasks 12, 520 internal links, amendments A1 to A6.
- `git diff 3157c89 86e966d` (14 files) and `git diff --check` (clean).
- A Python script that walks both `state.json` versions as JSON, plus a sentence-level diff of the changed long strings.
- `sha256sum` of the two amended files at head and at 3157c89, compared with A6 `files` and `SHA256SUMS.txt`.
- Mutation tests of the new validator code: 13 mutants on a full copy of the tree, each followed by regenerating the views.
- `cmp` of the archived review-pr60 against the pending-reviews copy.
- A secrets grep of the added lines.
- Manual link check of `User_Decisions_2026-11-15_resume.md`, which the validator's link check skips (`User_Decisions*`), and of `Amendment_v1.3.3-A6.md`.
- `gh pr checks 61 --watch`, then `gh pr view`.

## Checks

### 1. User instruction, point by point

| Instruction | Where | Result |
| --- | --- | --- |
| Keep 02-04 budget: 7/12, spare dev 2 / Guardrail 3 unused, no extra billing, no reset or reuse | `resume.budget`; no budget field changed | Values match ACCEPTANCE §1 and REPORT §5: 7/12, 40.666 s, billable 33,948, cache_read 121,189, spare 2 + 3, USD 0, no reset or reuse. **OK** |
| Item 8 gets a named owner task with completion criteria that blocks every later real call and X-02 | A6 X-08, ledger §7, X-02 prerequisites, `resume.blocking`, 02-05 `unresolved` | **OK**, see §3 below (L-1, L-3) |
| Call-free work continues | 02-05 `unresolved`, A6 `blocking`, `resume.blocking` | Stated explicitly. **OK** |
| Order X-03 → X-04 → 02-05; gRPC not finished does not block 02-05 | `resume.next_action`, `resume.blocking`, ACCEPTANCE §6, A6 md | "X-04가 늦으면 02-05는 HTTP" is backed by Plan_gRPC_Internal_API_v0.md lines 44 and 76. **OK** |
| PR #60 N-1..N-3 | ACCEPTANCE §3 and §4 block, `resume.next_action` | N-1: X-08. N-2: §3 closing sentence now lists 02-05, 02-07, 04-05, 04-07, 05-01..05-03, X-02 and X-08, with EARLY-01 in the global `unresolved`. N-3: `next_action` refreshed. **OK** (I-2 on the recurring N-3 pattern) |
| 1b linked to 04-07, 05-01..05-03, EARLY-01 scope and start conditions | 04-07 and 05-01..05-03 already had the 1b sentence from PR #60. New global `unresolved` entry for EARLY-01: GH1~GH5, frozen-ledger L02, and X-08 if real calls are used | **OK** |
| Persist work, completion conditions, blocks, budget, decisions, next action, restart procedure | `resume.current_work/blocking/budget/user_decisions/restart_procedure/next_action`, RESUME.md | **OK** (I-3) |
| Link to PREVENTION | PREVENTION §1.11 and §2 | It keeps the table format and separates "now" from what goes to the formal design. It states that only operational rules enforce the block. **OK** |
| No new product scope | X-08 covers existing carry-overs only; no new budget | **OK** |

No overclaim. 12/73 and 0/12 are unchanged in state and views. D3 and the 02-04 status, `verification_level` and evidence (one archived review appended) are unchanged.

### 2. Amendment conventions and package integrity

- A6 has the same shape as A3 and A4: `baselines_unchanged` 73/31/12/5; empty `profile_revisions`, `sprint_revisions` and `changed_acceptance`; one `extra_tasks` entry. The new field `extra_prerequisite_changes` is additive. No main-task acceptance changes.
- `original_sha256` equals the `SHA256SUMS.txt` entry for both files (ledger 23f6b324…, validator bd43577b…). `sha256` equals the file at head (ledger bfa4b816…, validator 13332bad…). The previous `sha256` values in A5 equal the files at 3157c89 (0c462034…, f7397cc1…), so the chain is continuous. The validator's last-wins `changed` map picks A6. **OK**
- In the ledger diff, only the §7 heading, the X-02 prerequisite cell, the new X-08 row and the closing paragraph change. X-02's acceptance text is unchanged. **OK**

### 3. X-08 acceptance against the item 8 sources, and blocking rules

The sources are REPORT §4 row 8 and PROGRESS 9 §4 row 8: PR #51 71e408a P1~P4, 12c9a21 N6, 608b367 F8, PR #49 dc11612 L-c3-2/3/4, and the deadline-stop test. All are present.

The ACCEPTANCE §4 PR #58 items are also present: P-2, Q-2, Q-3, the match-branch / keep-pending fixing test, and Q-1 (a pending copy with different bytes). Before this PR they were already bound to "the item 8 caller carry-over PR". None is dropped, and nothing outside those two sources is added. See L-2 for the two REPORT §7 items that used the same label.

The blocking rule is the same in all places I checked: A6 json `blocking`, A6 md, ledger §7 row and closing paragraph, state 02-04 `unresolved` and `next_action`, 02-05 `unresolved`, X-02 `prerequisites`, `unresolved` and `next_action`, X-08 `next_action` and `unresolved`, `resume.blocking`, `next_action` and `budget`, the global EARLY-01 entry, ACCEPTANCE §3, §4 and §6, and PREVENTION §1.11.

- Nowhere does a text make 02-05's call-free work wait for X-08. 02-05 says the opposite explicitly.
- Nowhere does a text allow a real call before X-08 is merged.
- The X-02 budget is stated as usable only after X-08 and X-02's remaining conditions.

### 4. Validator code

The new code reads amendments with `.get('extra_tasks',[])` and `.get('extra_prerequisite_changes',[])`, so A1, A2 and A5 (no extra tasks) still pass. Mutation results (full tree copy, views regenerated after each mutation):

| Mutant | Result |
| --- | --- |
| none (control) | rc 0 |
| drop X-08 from X-02 prerequisites | caught: prerequisites differ (X-02) |
| reorder X-02 prerequisites | caught |
| add an extra prerequisite to X-08 | caught |
| remove `extra_prerequisite_changes` from A6 | caught (X-02) |
| remove `extra_tasks` from A6 | caught: extra task not defined by an amendment |
| empty `resume.blocking` / delete `resume.budget` | caught: resume misses a restart field |
| `user_decisions` names a missing .md | caught |
| `extra_prerequisite_changes` names an undefined task | fails with a bare `KeyError`, not an assertion message (Info I-4) |
| delete X-08 from `state.extra_tasks` | **not caught**, rc 0 (L-3) |
| `user_decisions` = `["anything"]` (no .md) | not caught (by design) |
| `resume.blocking` = "없음" | not caught (by design; PREVENTION §1.11 says only field presence is checked) |

### 5. Structural diff of state.json vs 3157c89

Only these paths change:
- `snapshot_id` (0033 → 0034), `snapshot_created_at` and `last_actual_work_observed_at` (both 17:19:50, the time of the new event).
- `phase`: A6 appended.
- `notes`: two added (main CI 43682243501, the instruction).
- The global `unresolved`: the EARLY-01 entry added.
- `events`: X-08-DEFINED added. `events.jsonl` is append-only, with no lines removed.
- `tasks[02-04]`: `unresolved` (item 8 → X-08, the PR #58 items → X-08, the N-1..N-3 line), `next_action` and `evidence` (+ review-pr60).
- `tasks[02-05]`: `unresolved` (the X-08 sentences).
- `extra_tasks[X-02]`: `prerequisites` (+X-08), `next_action` and `unresolved` (the blocking clause names X-08).
- `extra_tasks[X-08]`: added (status 미착수, unverified, no evidence).
- `resume`: `last_verified_stage`, `branch_sha`, `plan_and_spec` (+A6), `running` and `next_action` changed; `current_work`, `blocking`, `budget`, `user_decisions` and `restart_procedure` added.

No status, `verification_level`, budget, baseline, experiment or milestone field changed. **Only intended changes.**

### 6. Secrets

No hits in the added lines for API keys, GitHub tokens, private keys, `password=`/`token=` values or the secrets and runs paths. **OK.**

### 7. Links and archive

- The validator checks 520 internal links and all resolve. The manual check of the resume decision document (4 links) and A6 md (3 links) also resolves.
- The archived `review-pr60-65022bc.md` is byte-identical to the pending-reviews copy.

## Findings

### L-1 (Low): X-08 cites the wrong section for item 8

A6 json `extra_tasks[0].acceptance`, the ledger §7 X-08 row and state X-08 `unresolved` read "02-04 수용 보고 §7·수용 기록 §3 항목 8" (acceptance report §7). Item 8 is row 8 of REPORT **§4** ("사용자 결정이 필요한 항목"). PROGRESS 9 §4 row 8 also says "수용 보고 §4 항목 8". REPORT §7 is "이전 보고 문구의 정정" and has no item 8. The `unresolved` "근거: 02-04 수용 보고 §7 항목 8" has the same error.

- **Fix:** change §7 to §4 in the A6 json, the ledger row and state X-08 `unresolved`, then update the ledger `sha256` in A6. The text is sha-pinned, so fix it before merge rather than in a later amendment.

### L-2 (Low): the "다음 supervisor 정비" label lost its anchor

PR #59 review I-2 and PR #60 fixed the label as "다음 supervisor 정비, 곧 항목 8 호출자 이월 PR과 함께". This PR replaces that sentence in ACCEPTANCE §4 and in 02-04 `unresolved` with "X-08". As a result:

- ACCEPTANCE §4 still says "I-2: “다음 supervisor 정비”의 시점을 정했다(위)", but the phrase no longer appears above.
- REPORT §7 still assigns two open carry-overs to "다음 supervisor 정비": the f73ac13 P3 empty `projects/*/memory` digest, and the bc55edf R5 test for unreadable out-of-scope files. ACCEPTANCE §4 says those keep "표의 담당·시점". Before this PR that label meant the item 8 PR, which is now X-08. X-08's acceptance does not include them, which is correct under "nothing beyond item 8". But they now have no defined time.
- **Fix:** add one sentence to ACCEPTANCE §4 or 02-04 `unresolved`. State that these two REPORT §7 items are not part of X-08 and keep the open "다음 supervisor 정비" timing, or name a time. Do not fold them into X-08 without a user decision.

### L-3 (Low): X-08 adds a no-fix path, and the validator does not require X-08 to exist

- X-08's acceptance allows "교정하지 않는 근거를 기록해 독립 검토의 동의를 받는다" (record a reason not to fix, with the reviewer's agreement). Decision (나) item 8 says "반드시 완료", and the resume instruction says "교정·시험·독립 검토·병합을 마친 뒤에만". A reviewer-only no-fix path is broader than that wording.
  - **Fix:** either remove it, or say that a no-fix disposition is reported to the user in the X-08 report and does not count as done without the user's agreement.
- The validator checks that `state.extra_tasks` is a subset of the amendment-defined tasks, and that each listed task's prerequisites match. Deleting X-08 from `state.extra_tasks` still passes (mutant above). X-02 then names a prerequisite that does not exist. The check that blocks X-02 can therefore be removed in one edit without a validator error.
  - **Fix:** assert that every amendment-defined extra task is in state, or at least that every extra-task prerequisite resolves to a known main or extra task.

### I-1 (Info): "current task 02-05" vs `resume.current_work` X-03

STATUS shows "현재 작업 02-05 · 다음 작업 02-05" (main track), while `resume.current_work` says X-03 comes next. The restart procedure (2) reads both. Not wrong, since `current_task` is the main-track pointer, but a restarted reader may stumble. Consider a short note in `current_work` such as "current_task는 주계획 기준".

### I-2 (Info): `resume.next_action` again starts with "this PR"

"이 재개 기록 PR 검토·병합 →" goes stale at merge, the same pattern as PR #60 N-3, which PREVENTION §1.11 now lists as a cause. Restart procedure step (1), which compares with origin/main, catches it. Refresh it in the next snapshot.

### I-3 (Info): X-02 `unresolved` "남은 조건" is stale

X-02 `unresolved` still says "남은 조건: 02-04 수용, executable 등록·의미 검토". 02-04 is accepted, and X-08 appears only in the later blocking clause. `next_action` and `prerequisites` are correct. Wording only.

### I-4 (Info): unknown task in `extra_prerequisite_changes` gives a bare KeyError

The validator still fails, so this is not a bypass. An assertion with a message would match the rest of the script.
