# Review of PR #61 at 79b40da (review 2)

- PR: HeaInSeo/ieo_poc#61, "report: resume record and item 8 owner X-08 (amendment A6), PR #60 review N-1..N-3 — snapshot 0034"
- Head: 79b40da591d3f33454b1699869a73f11b424e683 (confirmed with `gh pr view 61 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 3157c89. Two commits on top of 3157c89: 86e966d (reviewed in review 1) and 79b40da (correction 1).
- CI at 79b40da (run 43694639502), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories. This is review 2 of at most 4. Correction 1 of 3 was applied after review 1 (86e966d).

## Verdict

**Approve, no blocking or medium findings.** The correction fixes L-2, L-3, I-1, I-3 and I-4 fully, and fixes the pinned part of L-1. I-2 is partly fixed. One Low and two Info notes remain.

- **N-1 (Low):** one leftover "§7 항목 8" reference that review 1 L-1 named. It is in state, not in pinned text.
- **N-2 (Info):** where the two REPORT §7 items are recorded.
- **N-3 (Info):** a leftover "this PR" phrase in `resume.current_work`.

None needs another amendment. N-1 is a one-word state edit. It can go into this PR or the next snapshot.

## What I ran

- A detached worktree of 79b40da in my scratch directory, removed at the end. Using `.venv-poc/bin/python` from `docs/poc`:
  - `scripts/render_progress.py --check`: `ok: true`, rc 0.
  - `scripts/validate_package.py`: rc 0, `schema_errors: 0`, `ledger_spec_match` and `generated_views_match` true, accepted main tasks 12/73, followup 0/12, 520 internal links, amendments A1 to A6.
- Diffs:
  - `git diff 86e966d 79b40da` (10 files, +26/−21).
  - `git diff --check` for both 86e966d..79b40da and 3157c89..79b40da (clean).
- Python checks:
  - X-08 acceptance text, compared across A6 json, ledger §7 row and state X-08 `unresolved`.
  - A6 `files` sha256 compared with the files at head. `original_sha256` compared with `SHA256SUMS.txt`.
- A JSON structural walk of `state.json` against 3157c89 and against 86e966d.
- Validator mutation tests: 17 cases on a full `git archive` copy of the tree. The views were regenerated after each mutation.
- Greps:
  - leftover old wording ("교정하지 않는 근거", "독립 검토의 동의", "수용 보고 §7 항목 8");
  - secrets in the added lines of 3157c89..79b40da;
  - a manual link check of `User_Decisions_2026-11-15_resume.md` and `Amendment_v1.3.3-A6.md` (7 links, all resolve).
- `gh pr checks 61 --watch`, then `gh pr view`.

## Status of the review 1 findings

| ID | Status | Evidence |
| --- | --- | --- |
| L-1 wrong section for item 8 | **Fixed in the pinned text, one residue** | A6 json acceptance, the ledger §7 X-08 row and state X-08 `unresolved` now read "02-04 수용 보고 §4·수용 기록 §3 항목 8". The ledger sha256 in A6 was updated. The trailing "· 근거: 02-04 수용 보고 §7 항목 8" in state X-08 `unresolved` was not changed, although review 1 named it. See N-1. |
| L-2 "다음 supervisor 정비" lost its anchor | **Fixed** | ACCEPTANCE §4 I-2 and the 02-04 `unresolved` now say the following. The two REPORT §7 items (f73ac13 P3 empty memory digest, bc55edf R5 cleanup test) are outside X-08. They are handled in 02-05's first supervisor change PR, are not a prerequisite of any task, and are non-blocking. See the scope check below and N-2. |
| L-3a reviewer-only no-fix path | **Fixed** | The text now requires a regression test for each item. If an item is not fixed, the reason is reported to the user and a decision is obtained. Reviewer agreement alone does not exempt it. This is consistent in A6 json, A6 md, the ledger row and state X-08. No other copy of the old wording remains outside the archived reviews. |
| L-3b validator does not require X-08 to exist | **Fixed** | New checks: state `extra_tasks` set == amendment-defined set; `extra_prerequisite_changes` must target a defined task; every extra-task prerequisite must be a known main/followup/extra task. Mutation results below. |
| I-1 current task 02-05 vs X-03 | **Fixed** | `resume.current_work` now says `current_task`/`next_task` can only hold main tasks, so they show 02-05. It also says that the work actually in progress is X-03, which is not a prerequisite of 02-05. |
| I-2 `resume.next_action` starts with "this PR" | **Fixed in `next_action`; residue in `current_work`** | `next_action` now starts "PR #61(재개 기록·개정 A6) 병합 여부 확인 →". That still reads correctly after the merge. `current_work` still contains "(이 재개 기록 PR 다음)". See N-3. |
| I-3 X-02 "남은 조건" stale | **Fixed** | Now "남은 조건: X-08 병합, executable 등록·의미 검토(02-04 수용은 2026-11-15 충족)". |
| I-4 bare KeyError | **Fixed** | Now `AssertionError: ('prerequisite change for an undefined extra task', 'X-99')`. |

## Regression checks on the correction

### X-08 acceptance identity and sha256

- A6 `extra_tasks[0].acceptance` is byte-identical to the ledger §7 X-08 cell (column 4), and to the part of state X-08 `unresolved` between "수용 기준(장부 §7, 개정 A6): " and " · 근거:". Both comparisons are True.
- Ledger: A6 `sha256` 50d379d9… equals the file at head, and `original_sha256` 23f6b324… equals `SHA256SUMS.txt`.
- Validator: A6 `sha256` 9deda2f3… equals the file at head, and `original_sha256` bd43577b… equals `SHA256SUMS.txt`.
- In the ledger, only the X-08 row changed in this correction. The A6 md summary and `reason` strings describe the new rule and the new validator checks accurately.

### The "no-fix needs a user decision" wording against the user's instructions

- `User_Decisions_2026-11-15_0204-acceptance.md` item 8 says: "다음 실제 호출과 X-02 착수 전에 반드시 완료하도록 차단 조건으로 연결한다". Its line 77 says: "범위·예산 변경이 필요하면 근거와 대안을 묶어 보고하세요".
- `User_Decisions_2026-11-15_resume.md` §2 says: "교정·시험·독립 검토·병합을 마친 뒤에만 기존 승인 범위의 호출을 진행해줘".
- The new text keeps fix + regression test as the default. It makes any no-fix a scope change that goes to the user, which is exactly the reporting route the decision prescribes. It does not create a path that lets X-08 count as done, or lets calls start, without that decision.
- It does not conflict with resume §5, "기존 승인 범위 안의 작업은 매 단계마다 재승인을 기다리지 않아도 돼". A no-fix would narrow the approved scope, so it is not a step inside that scope.
- **No contradiction.**

### REPORT §7 items placed in "02-05's first supervisor change PR, non-blocking"

- The 02-05 `unresolved`, `next_action`, and any prerequisite field are unchanged in this correction. The structural diff against 86e966d shows no 02-05 path.
- No extra task's prerequisites changed. A6 still has the single change X-02 += X-08.
- 02-04 status, `verification_level` and evidence are unchanged.
- The text explicitly says "어느 작업의 선행 조건도 아니다(비차단)" and "X-08 범위가 아니다".
- The two items were already open carry-overs in REPORT §7 with the label "다음 supervisor 정비". The correction only gives that label a concrete time. It does not add a 02-05 acceptance item or expand 02-04.
- **No new prerequisite and no scope expansion.** See N-2 on where it is recorded.

### Validator mutation tests (full tree copy, views regenerated after each)

| Mutant | Result |
| --- | --- |
| control | rc 0 |
| delete X-08 from `state.extra_tasks` | caught: an amendment-defined extra task is missing from the ledger |
| delete X-01 from `state.extra_tasks` | caught (same message) |
| delete the X-08 definition from A6 and from state, keeping A6's X-02 += X-08 | caught: extra task prerequisite is not a known task (X-02) |
| `extra_prerequisite_changes` targets X-99 | caught: prerequisite change for an undefined extra task (X-99) |
| X-08 prerequisite 99-99 (A6 and state) | caught: not a known task (X-08) |
| X-02 added X-99 (A6 and state) | caught: not a known task (X-02) |
| X-02 added EARLY-01 (A6 and state) | caught: not a known task. EARLY-01 is not a ledger task, so this is correct. |
| drop X-08 from X-02 prerequisites (state only) | caught: prerequisites differ from the amendments (X-02) |
| duplicate X-08 in state | caught by `render_progress.py` (duplicate extra task ID) |
| valid: X-08 prerequisite = main task 02-05 | rc 0 |
| valid: X-08 prerequisite = follow-up task | rc 0 |
| valid: X-08 prerequisite = extra task X-03 | rc 0 |
| valid: A6 without `extra_prerequisite_changes`, state X-02 without X-08 | rc 0 |
| valid: A6 without `extra_tasks` and changes, state without X-08, X-02 without X-08 | rc 0 |
| valid: A1 with explicit empty `extra_tasks` / `extra_prerequisite_changes` | rc 0 |
| valid: reversed order of `state.extra_tasks` | rc 0 |

All bad mutants are caught with a clear message, and **no false failures** occurred. A1, A2 and A5 have no `extra_tasks` key and still pass.

One observation that is not a finding: if two amendments define the same extra task ID, the later definition silently replaces the earlier one in `want`. This code is older than this PR, and no amendment does that today.

### General checks

- **Structural diff of `state.json` against 3157c89:** the same path set as review 1 §5, with no new paths. Against 86e966d, only five text fields changed:
  - X-02 `unresolved`
  - X-08 `unresolved`
  - `resume.current_work`
  - `resume.next_action`
  - 02-04 `unresolved`

  No status, `verification_level`, budget, baseline, experiment, milestone, prerequisite or evidence field changed. `snapshot_id` stays 0034, the same snapshot within one open PR. The generated views differ only in the ledger SHA header and the RESUME rows. `events.jsonl` has one added line (X-08-DEFINED), with no lines removed. **Only intended changes.**
- **Secrets:** no hits in the added lines of 3157c89..79b40da for API keys, GitHub tokens, private keys, `password=`/`token=`/`api_key` values or the secrets and runs paths.
- **Links:** the validator checks 520 internal links and all resolve. The manual check of the two `User_Decisions*`/A6 documents also resolves.
- **CI:** all 3 required checks SUCCESS at 79b40da (run 43694639502).

## New findings

### N-1 (Low): X-08 `unresolved` still cites "수용 보고 §7 항목 8"

State X-08 `unresolved` ends with "· 근거: 02-04 수용 보고 §7 항목 8, 수용 기록 §3·§4, PR #60 검토 65022bc N-1". Review 1 L-1 named this exact clause. Item 8 is REPORT §4 row 8, and REPORT §7 ("이전 보고 문구의 정정") has no item 8.

This clause is outside the pinned acceptance text, so no amendment is needed. A grep finds no other "§7 항목 8" in the package outside the archived reviews.

- **Fix:** change it to "§4 항목 8" in `state.json` and regenerate the views. Do it in this PR, or in the next snapshot.

### N-2 (Info): the 02-05 placement is recorded only on the 02-04 side

The rule that the two REPORT §7 items go with "02-05's first supervisor change PR" is written in ACCEPTANCE §4 I-2 and in 02-04 `unresolved`. The 02-05 `unresolved` does not mention it, so whoever opens 02-05 has to find it from the 02-04 side. Nothing says what happens if 02-05 makes no supervisor change.

This is not wrong, and putting it in 02-05 must not turn it into a prerequisite. A later snapshot could add a short non-blocking pointer to 02-05 `unresolved`, for example "02-04 이월 f73ac13 P3·bc55edf R5: 첫 supervisor 변경 PR에서, 비차단".

### N-3 (Info): `resume.current_work` still says "이 재개 기록 PR 다음"

`next_action` was fixed for review 1 I-2, but `current_work` still says "X-03 착수 전(이 재개 기록 PR 다음)". This goes stale at merge in the same way. Restart procedure step (1) catches it. Refresh it in the next snapshot.
