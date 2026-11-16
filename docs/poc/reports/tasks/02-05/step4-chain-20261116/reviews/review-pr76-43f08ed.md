# Review of PR #76 at 43f08ed (review 1)

- PR: HeaInSeo/ieo_poc#76 (`report/0205-s4-ledger` → main). It is a docs and ledger-only PR. It records the merges of PR #74 (02-05 step 3) and PR #75 (02-05 step 4) as ledger snapshot 0048, archives three reviews, and adds REPORT §7 qualification and §12 to the step-4 REPORT.
- Head: 43f08edd2a4d4c4a938f53be427fd630ac5dda7d (confirmed with `gh pr view 76`). Base: main 1b5bc1f.
- Reviewer: independent review agent, read-only. I used a fresh detached worktree (scratchpad `r76`, removed afterwards). I made no model calls and started no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch PostgreSQL on 55433, the coordinator on 58090 or any other worktree. I did not push or comment on GitHub.

## Verdict

**Approve.** There are no High, Medium or Low findings. Every SHA, CI id, count and version in the diff matches the primary sources. The archived reviews are byte-identical to the originals. The renderer `--check` and the package validator pass. I raise four Info items, all optional.

## What I ran

| Check | Result |
| --- | --- |
| `git diff --name-status 1b5bc1f 43f08ed` | 10 files, all under `docs/poc/` (6 progress files, 3 added reviews, 1 REPORT). No code or contract changes |
| `gh pr view 74 --json mergeCommit,headRefOid,reviews,…` | MERGED. Merge commit 375f21f63084…, head fe924b786aae… (matches review fe924b7). Merged at 2026-11-15T16:00:09Z, which is 01:00:09 KST |
| `gh pr view 75 --json …` | MERGED. Merge commit 1b5bc1f18e37…, head debf413ba8cd… (matches review debf413). Merged at 2026-11-16T00:30:42Z, which is 09:30:41 KST, before the snapshot time 09:35:42 |
| `gh run view 43732975756` | ci, push, main, headSha 375f21f63084…, success |
| `gh run view 43780803494` | ci, push, main, headSha 1b5bc1f18e37…, success |
| `gh pr view 76 statusCheckRollup` | docs/poc validation, buf, and toolchain pin: SUCCESS. Rust fmt/clippy/test was still running; it is irrelevant to a docs-only diff |
| `cmp` of the 3 archived reviews against `.git/ieo-dev-workspace/pending-reviews/` | All three are byte-identical (review-pr74-fe924b7.md, review-pr75-5dbb3d8.md, review-pr75-debf413.md) |
| Relative-link scan of every changed `.md` | No broken links. The archived reviews contain no relative links |
| Existence of every `evidence` path in the 02-05 task and the two new events | All 17 plus 3 exist |
| `.venv-poc/bin/python docs/poc/scripts/render_progress.py --check` | `{"ok": true, "mode": "check", …}` for all 5 views |
| `.venv-poc/bin/python docs/poc/scripts/validate_package.py` | rc 0 |
| `git diff --check 1b5bc1f 43f08ed` | One hit: a new blank line at the end of REPORT.md (I-4) |
| Secret grep over the diff (sk-ant, ghp_, token=, password, private keys, api key, raw session ids, secrets/runs paths) | Only matches inside the quoted review texts that *describe* not reading them. No secrets |

## Analysis

### 1. Factual claims against primary sources (at 1b5bc1f)

- **Merge SHAs.** 375f21f (#74) and 1b5bc1f (#75) are correct, including the full SHA in `resume.branch_sha`.
- **CI ids.** 43732975756 → 375f21f, success. 43780803494 → 1b5bc1f, success. Both are recorded in `notes` (state.json:2055–2056) and in the events.
- **Approved heads.** The step-3 approval fe924b7 equals PR #74's merged head. The step-4 approval debf413 equals PR #75's merged head.
- **Review/correction counts.**
  - fe924b7's header says "review 2 of at most 4, after correction 1 of 3".
  - debf413 is review 2 after one correction commit series (1b7f51c, 40e2c98, 8717b88, debf413).
  - The ledger's "검토 2/4·교정 1/3" is correct for both.
- **Tests 376.** The sum of `test result: ok. N passed` in `step4-chain-20261116/evidence/test.log` is 376. The first line names commit 40e2c98, and `git diff` shows no code change between that commit and the head.
- **Mutants 21/21.** `evidence/mutants.txt` has 21 `S4-*` rows, all KILLED. The one historical survivor (S4-M18 at 1b7f51c) is disclosed and was re-run at 40e2c98.
- **OpenAPI 0.12.0.** `contracts/ieo-api-v0.openapi.yaml:4` at 1b5bc1f.
- **Runtime contract revision 12.** `Runtime_Contract_v0.md` head record and §15 at 1b5bc1f.
- **Step-4 behaviour summary in `result_state`.** Checked against REPORT §1–§3 and §11:
  - takeover bound to delivery and attempt;
  - 409 DELIVERY_REQUIRED / DELIVERY_MISMATCH;
  - first-action detection and fill;
  - `last_actual_action_at` semantics;
  - getAssignmentChain;
  - C1 closed;
  - accepted 02-04 behaviour changes.

  I found no overstatement.

### 2. Archives

- All three files are byte-identical to the pending originals and are placed in the step's `reviews/` directory, next to the earlier reviews.
- They contain no relative links, so they cannot break.

### 3. REPORT §12 against review debf413, and Info carry-through

Each debf413 finding has a row:

| Finding | Disposition in §12 | Reflected in the ledger |
| --- | --- | --- |
| L-3 | Test plus mutant F in the step-5 PR | `next_action` step 5 and `unresolved` |
| I-7 | Fixed in §7. The parenthesis cites §8 and debf413 I-7. It matches the reviewer's suggested wording | — |
| I-8 | Step 6 or 8 | `unresolved` |
| I-9 | Step 6, together with the remaining fe924b7 I-4 gap | `next_action` step 6 and `unresolved` |

- **Review 5dbb3d8's Info items.** I-1 to I-6 are disposed in REPORT §11. Their owners are in §8:
  - I-1: optional, no owner;
  - I-3: step 8 check;
  - I-4: 02-08;
  - I-5: user report;
  - I-6: no action.
- The ledger carries I-3 and I-4 in `unresolved`, and I-5's substance in `result_state` and `blocking` (02-04 spare runs can no longer go through `ieo-role-call`; real runs go through `deliverAssignment`). I-1 and I-6 need no carry. This is consistent.
- **Review fe924b7.** REPORT §9 disposes L-1, I-2, I-4, I-5 and I-6. I-1 (redundant guard) and I-3 (an accepted untestable item) need no action. The ledger lists I-5 for step 5 and I-4's residual for step 6. This matches PLAN §3 step 5 (C7 limits) and step 6 (C9 external requests). See I-1 below on wording.

Nothing is silently dropped.

### 4. Resume fields

- `current_task` and `next_task` are both 02-05, a main task.
- The STATUS rows `12/73` and `0/12` are unchanged. The rendered diff touches only the snapshot header, the times and the 02-05 row.
- `last_verified_stage` adds steps 3 and 4. `branch_sha` is 1b5bc1f with CI 43780803494.
- `plan_and_spec` is revision 12 with OpenAPI 0.12.0. `running` names this PR's branch `report/0205-s4-ledger`, which matches PR #76's headRefName.
- `current_work` and `next_action` both have the ledger PR next, then step 5. `blocking` marks the step 3–4 merges as done and adds the delivery-path requirement for step 8.
- `budget` and `user_decisions` are unchanged and still correct: no runs were used, and there is no new decision.
- `restart_procedure` is unchanged and generic.
- The task-level `next_action` ordering (5 → 6 → 7 → budget code PR → 8 → 9) matches PLAN §3.
- `saved_reports`, `latest_report` and `evidence` add the step-4 REPORT, test log and reviews.
- None of these fields is stale.

### 5. Rendered views

`render_progress.py --check` reports all five views (STATUS, TASKS, RESULTS, RESUME, events.jsonl) as exactly what the renderer produces. The two new `events.jsonl` lines equal the two new `state.json` events.

### 6. Confidentiality

- No secrets and no raw stream or session bodies appear in the diff.
- The archived reviews mention the secrets and runs paths only to say they were not read.
- The scratchpad log names (`r75a-*.log`, `r75b-*.log`) are not published.

## Findings

### I-1 (Info): resolved fe924b7 items still read as pending in the ledger's `unresolved`

- **Where.** `docs/poc/progress/state.json`, 02-05 `unresolved` ("PR #74 검토 fe924b7: L-1 단계 3 보고 §5 정정(단계 4 PR), I-2 … (단계 4 PR), I-6 minLength 1(단계 4 PR) …").
- **Scenario.** Step 4 has been merged, and REPORT §9 shows L-1, I-2 and I-6 done. A reader resuming from `unresolved` may think they are still open.
- **Fix (optional).** Write "(단계 4에서 처리, 보고 §9)", or drop them from `unresolved` and keep only I-4's residual (step 6) and I-5 (step 5).

### I-2 (Info): REPORT §11 row I-2 still names the L-2 tests as covering the `"none"` link

- **Where.** `docs/poc/reports/tasks/02-05/step4-chain-20261116/REPORT.md:274` (the §11 I-2 row: "…`"none"`도 … | L-2 시험 (b)(c)").
- **Scenario.** Review debf413 L-3 shows that this test reference is inaccurate. §12 acknowledges this. However, §11 itself was left unmarked, while §7 received an inline note for I-7. A reader of §11 alone sees the stale claim.
- **Fix (optional).** Add "(`"none"`은 시험 없음, §12 L-3)" to that cell.

### I-3 (Info): step-8 `next_action` does not name the 5dbb3d8 I-3 stream check

- **Where.** `state.json`, 02-05 `next_action`.
- **Scenario.** Step 5 and step 6 name their carried review items, but step 8 does not mention confirming first-action block kinds (`server_tool_use` and others) on the real stream. The item is in `unresolved` and REPORT §8, so nothing is lost.
- **Fix (optional).** Add "5dbb3d8 I-3 실제 stream 블록 확인" to step 8.

### I-4 (Info): trailing blank line at the end of REPORT.md

- **Where.** `docs/poc/reports/tasks/02-05/step4-chain-20261116/REPORT.md:292`.
- **Scenario.** `git diff --check` reports "new blank line at EOF". This is cosmetic, and CI does not check it.
- **Fix (optional).** Remove the final empty line.

## Summary for the author

The ledger PR is accurate:

- merge SHAs, CI runs, approved heads and review counts;
- the test (376) and mutant (21/21) counts;
- OpenAPI 0.12.0 and runtime contract revision 12.

The three archived reviews are byte-identical to the originals, and the rendered views and validator are clean. REPORT §12 disposes every debf413 finding, and the 5dbb3d8 and fe924b7 Info items are carried consistently between the REPORT and the ledger. The resume fields point correctly at "merge this PR, then step 5". The completion counts are unchanged.

**Approve.** The four Info items are wording and cosmetic only. They can be folded into the step-5 PR or ignored.
