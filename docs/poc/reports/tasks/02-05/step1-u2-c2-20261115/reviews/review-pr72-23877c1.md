# Review of PR #72 at 23877c1 (review 1)

- PR: HeaInSeo/ieo_poc#72, "02-05 step 1: U2 registration check order and C2 binding sprint_id reservation". It also carries the PLAN.md update note, the X-08 REPORT wording fixes and ledger snapshot 0045.
- Head: 23877c1b92c2f5a61d93a0544b03df4a34a67b38. I confirmed it with `gh pr view 72 --json headRefOid,statusCheckRollup` before and after the review.
- Base: main 386a223, which is also the merge base. Three commits sit on top:
  - 1ad0a29: code and tests;
  - 24b2f9e: REPORT and evidence;
  - 23877c1: ledger 0045, the PLAN note, the X-08 REPORT fixes and the archived review b92a398.
- CI at 23877c1 (run 43719719712, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS, 357 passed and 0 failed in the job log
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan, d05wt, x08wt or s1wt worktrees, target-x08, PostgreSQL on 55433, or the coordinator on 58090. I used my own detached worktree (r72) and target directory (target-r72) and removed both at the end. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking or Medium findings. Two Low and three Info findings.**

- The U2 invariant holds. Compared with base 386a223, the set of refused `registerSprint`/`convertHumanInput`/`importNotionTask` requests is unchanged except for the planned C2 refusals. No request goes from 409 to 200. Only the first code changes, and only for requests that fail more than one check.
- All checks run inside the registry lock and before any write. No new race is introduced.
- The C2 rule is exactly the planned rule. It does not block the binding's own repo, drafts of that repo, or the conversion path. Imports were already refused for reserved ids.
- OpenAPI 0.9.0 matches the code, and openapi-spec-validator reports OK.
- fmt and clippy pass. The full suite passes: 357/0 locally and in CI. My own mutations were caught 3 of 4; the survivor is artificial (I-2).
- The docs and ledger are accurate apart from a stale step-1 REPORT header (L-1).

## What I ran

- `cargo fmt --all -- --check`: ok. `cargo clippy --workspace --all-targets -- -D warnings`: rc 0.
- `cargo test --workspace --all-targets --locked` with `IEO_TEST_DB_URL` on the throwaway DB on 55432:
  - First run: 1 failure, `checkpoint_e2e::checkpoints_are_reported_compared_and_judged_across_restarts_over_grpc`. This is a timing flake (I-1).
  - Rerun with `--no-fail-fast`: rc 0, **357 passed, 0 failed, 0 ignored**. coordinator has 75 tests (68 + 7 new) and shutdown has 2. This equals the trailer of `evidence/test.log` ("full suite after rebase: passed 357") and the CI job.
- `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml`: OK.
- Validators in `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` true, `generated_views_match` true, 578 internal links checked.
  - `render_progress.py --check`: ok.
  - `test_render_progress.py`: PASS.
- Relative links in every changed .md resolve. A secret grep over the diff (password=, ghp_, sk-ant, private keys, token values) found nothing.
- `cmp` of the archived `reports/tasks/02-04/caller-carryovers-20261115/reviews/review-pr71-b92a398.md` against `pending-reviews/review-pr71-b92a398.md`: identical.
- `gh run view 43719278914`: 386a223, push, success. This matches the new `notes` line in state.json.
- Rebase check: the evidence was produced at c61708f on c5464f5. Two comparisons show it still applies:
  - The patch c5464f5→c61708f and the patch 386a223→1ad0a29 differ only in index hashes and one hunk offset.
  - `crates/ieo-coord/src` and `crates/ieo-spec` are byte-identical between c5464f5 and 386a223.
  - So `old-codes.txt` (base answers) and the c61708f test log apply unchanged to this head.
- My own mutation spot-checks. Each mutation was applied alone to `api.rs`, followed by `cargo test -p ieo-coord --test coordinator -- u2_ n2_ c2_ an_imported_draft`, and the file was then restored. `git status` was clean afterwards.

| # | Mutation | Result |
| --- | --- | --- |
| MA | import gate only for a new revision (skip `AlreadyRegistered`/`Superseded`) | KILLED: `n2_a_re_gated_page_keeps_an_unchanged_executable_copy_refused` |
| MB | C2 reservation skipped when the spec's `registration_state` is `draft` | survived (I-2) |
| MC | startup check: "two bindings name one sprint_id" disabled | KILLED: `c2_the_startup_check_refuses_a_binding_on_another_repos_sprint` |
| MD | ownership guard back before `check_spec` | KILLED: `u2_an_older_revision_…`, `u2_an_imported_sprint_with_a_01_07_error_…` |

## (1) Invariant: refused set unchanged except C2

Old order (386a223) for `registerSprint`:
1. idempotent replay;
2. `guard_import_owned`, which skipped other repos and malformed bodies;
3. `notion_gate` (executable, well-formed bodies only);
4. `register_spec`, which ran RG2 → `check_registration` → store.

New order:
1. idempotent replay;
2. `check_spec`, which runs `guard_sprint_repo` (RG2 + C2) → `check_registration`, and returns every `Rejected` outcome as its error;
3. `guard_import_owned`;
4. `notion_gate` (executable);
5. `save_spec`.

Why the refused set is unchanged:
- **Order.** Every guard is a pure read of the same state, and nothing is written before `save_spec`. A request is therefore refused under the new order exactly when some guard refuses it under the old order.
- **Guard conditions.** The new guards apply under the same conditions as the old ones. Ownership and the gate used to be skipped for malformed bodies, and those bodies are still refused, with the same 400. The format branch removed from `guard_import_owned` is dead code now, because `check_spec` has already refused malformed bodies.
- **Outcomes covered.** `check_spec` returns `AlreadyRegistered` and `Superseded` as non-errors. So ownership and the gate still apply to the 200 outcomes, which is review 3c017a4 N-2 problem 1. Mutation MA confirms the tests pin this.
- **No new acceptance.** The `Rejected` arm in `save_spec` is now an unreachable 500. It used to be the rejection path, and every rejection now leaves through `check_spec` first, so no acceptance can come from there.

Cases I checked beyond the REPORT table:
- **Replay with the same key.** It is answered first, as before. A refused request is never stored in `idempotency`, because `idempotent_store` runs only after `save_spec`. So no 409 can later become a 200 through a replay.
- **Same body, different key.** It goes through the checks again: `AlreadyRegistered` → ownership → gate. The answer is the same as before.
- **Draft vs executable.** The gate is executable-only in both versions. Ownership applies to both modes in both versions. Table rows 1–4 include both modes, and row 5 (binding) is executable-only by nature.
- **`Superseded` with the gate.** Tested as n2-copy-05: 409 `IMPORT_GATED`, not 200.
- **Ownership combined with RG2/C2.** This cannot happen. Ownership fires only when `target.repo_id` equals the import sprint's repo, which is the RG2 owner. An import-owned id cannot be reserved, because the startup check (1) forbids it and the import path refuses reserved ids.
- **Gate combined with RG2/C2.** These are rows 9 and 10, which the REPORT §2 adds and which are not in the PLAN §2.4 table. The REPORT asks this review to confirm them. I confirm both are refusals in old and new code (`old-codes.txt`: u2-g-0005/0006 were 409 `IMPORT_GATED`, now 409 `SPRINT_REPO_CONFLICT`). The OpenAPI lists them.
- **Gate combined with other 01-07 codes.** Examples are `REVISION_*`, references, and `UNBOUND_DRAFT` for a stored imported draft. These are covered generically: OpenAPI says "now the 01-07 code", with digest as the tested instance.
- **`convertHumanInput` and imports.** Both still call `register_spec` = `check_spec` + `save_spec`, which is the old RG2 → 01-07 → store order. Neither calls ownership or the gate.
  - Convert: the spec's sprint_id is the caller repo's own binding id, and `reserved_for` returns that same repo. A duplicate across two bindings is refused at startup, so nothing changes for convert.
  - Import: it refuses a reserved `test_id` before `register_spec` unless the id was imported before, and the startup check excludes that case. So C2 adds no new refusal there.
- **Evidence.** `old-codes.txt` was produced against the base `api.rs` (identical to 386a223's) plus a no-op `check_bindings` stub. Every changed row is a refusal in both versions. The only accepted old answers are the C2 cases c2n2-0001/0002/0006 (201). c2n2-0003 was 400 and is now 409, which is the documented RG2 position.

## (2) Transaction and locking

- `register` takes `lock(tx, "registry")` before the idempotent lookup. `check_spec`, `guard_import_owned`, `notion_gate` and `save_spec` all run in the same transaction after that.
- No guard writes. The first write is the `UPDATE`/`INSERT` in `save_spec`, followed by `history` and `idempotent_store`.
- Imports and conversion take the same registry lock (convert at line ~988; the import path uses the same lock as before and is unchanged here). So nothing can change ownership between the check and the save.
- `reserved_for` reads the immutable `ctx.bindings`, which are loaded once at start.

## (3) C2

- `reserved_for` returns the first binding whose `sprint_id` equals the id. With duplicates refused at startup, the result is unique.
- `guard_sprint_repo`:
  - The owner is the DB repo if one exists (RG2). Otherwise it is the reserving repo.
  - A request is refused when the owner is not `target.repo_id`.
  - The owning repo, its drafts and its conversion pass (c2n2-0004 `/v0/sprints`, c2n2-0005 `/v0/sprint-inputs`).
  - The guard runs before format checks, like RG2. That is documented in OpenAPI and the REPORT.
  - The rule is exactly PLAN §3 step 1 ("아직 등록되지 않은 예약 ID를 다른 repo가 등록·변환하면 409 `SPRINT_REPO_CONFLICT`") and no broader.
- `check_bindings` refuses start in three cases:
  - (1) an imported test id (moved unchanged from the binary);
  - (2) a binding id registered for another repo (PLAN §2 C2 row);
  - (3) two bindings naming one id. This is not literally in the plan, but it is the necessary consistency condition for (2) and is documented. See L-2.
- `main` maps `Err` to exit 2. `tests/shutdown.rs` checks the real binary's exit 2 and message for (2).
- The check runs only on `serve` start, after `migrate`, on a non-transactional connection. That is fine, because nothing else is serving yet.
- Persistent coordinator: its bindings and DB live in the private directory and are not in the repo (X-01 REPORT). I could not and did not inspect them. Before C2, another repo could register a bound id; that is exactly what (2) now refuses at start. A valid configuration, meaning one where no other repo holds a bound id and no id is bound twice, starts as before. See L-2 for the rebuild note.

## (4) OpenAPI 0.9.0

- The info description and the `registerSprint` description match the code:
  - the steps are numbered (1)–(6);
  - (4) and (5) apply to every non-rejected outcome;
  - RG2/C2 come before the format check;
  - the list of changed first codes equals REPORT §2 rows 1–10.
- The `convertHumanInput` sentence ("steps (2), (3) and (6)") is accurate.
- The `importNotionTask` text was corrected: format 400, repo 409, 01-07 codes kept, and an earlier import's revision re-sent is 409, not 200.
- No new code or field.
- The only change to existing X-01 test expectations is `an_imported_draft_with_unresolved_reasons_cannot_execute`: 409 `IMPORT_GATED` → 422 `UNBOUND_DRAFT`. This is the change pre-announced in the user decision record §2.1 and in review N-1. The test also gained a row-count check. No other existing coordinator/02-03 test expectation changed; the diff touches only that test and appends new tests.

## (5) Tests

- The 7 new coordinator tests send every refusal through `refused()`, which asserts that the 14-table row counts are unchanged.
- They cover:
  - old and new first codes for rows 1–10;
  - the unchanged cases (u2-o, u2-f, u2-r, u2-g-0001/0003);
  - `Superseded` and `AlreadyRegistered` under ownership (u2-m1) and under the gate (n2-copy-04/05);
  - C2 for draft mode, executable mode and malformed bodies;
  - the owner's own registration and conversion;
  - RG2 after registration;
  - all three startup refusals, plus the owner's own binding accepted.
- The author's M1–M4 and my MA/MC/MD were caught.

## (6) Docs and ledger

- **PLAN.md update note.** It adds a dated block above the body and leaves the body untouched. It says:
  - the user decisions: F4 (다), S1 kept, start now, budget (나);
  - X-08 is accepted (PRs #67/#68/#70, record PR #71), and the plan's "X-08 병합" is to be read as "검수 완료";
  - the other step-8 preconditions remain: budget-code PR, steps 3/4, rebuild after 1c21177;
  - "revision 10" in the plan is to be read as revision 11.
- All of these match `User_Decisions_2026-11-15_0205-x08.md` and the ledger. None weakens a block: real calls still wait for the stricter "검수 완료" plus the other preconditions. This settles b92a398 I-4 by an overriding note rather than line edits, which is acceptable.
- **X-08 REPORT edits.** They match review b92a398:
  - L-1: the header "상태" row now says 검수 완료 and lists the PRs, merges and the review link.
  - I-2: §4.1 step 2 now checks `/state/checkpoints/` before rewriting, which is the review's optional suggestion.
  - I-1: step 3 now says "설정이 아직 변조된 채로 남아 있으면", which is the review's exact wording, and adds the cleanup caveat.
  - §8 gains one line recording the review.
- **state.json diff vs 386a223.** The changes are:
  - snapshot 0044 → 0045, with timestamps;
  - the 02-05 task: `last_observed_at`, `latest_report`, an appended `result_state` sentence, +2 evidence entries and a new `next_action`. The status stays 진행 and `verification_level` stays unverified;
  - one `notes` line for main CI 43719278914;
  - one event, 02-05-S1-PR-20261115, mirrored in `events.jsonl`;
  - resume: `last_verified_stage`, `branch_sha`, `running`, `saved_reports` (+1), `next_action` and `current_work`;
  - X-08 evidence +1 (the archived review).
  - Nothing else changes. Counts stay at 12/73 and 0/12 (STATUS.md). `blocking` and `budget` are unchanged.

## Findings

### L-1 (Low): the step-1 REPORT header and status describe the pre-rebase branch

`docs/poc/reports/tasks/02-05/step1-u2-c2-20261115/REPORT.md` has four stale statements:
- **Base and commit.** The header says "기준 커밋 main c5464f5. 구현 커밋 c61708f". The PR's implementation commit is 1ad0a29 on 386a223, and c61708f is not in the PR's history. The test.log trailer names a third head, d9f81ba, which is also not in the PR.
- **Decision record.** The header says the decision record "이 브랜치에는 아직 없다". It is on the branch through 386a223, and PLAN's note links it.
- **Ledger.** "원장은 바꾸지 않았다" and §5 "이 보고는 원장을 바꾸지 않는다" are contradicted by 23877c1 in the same PR, which writes snapshot 0045.
- **Test count.** §4 gives 356 at c61708f. The head runs 357, as the test.log trailer and the ledger say.

The evidence is still valid: the code patch is identical and the base `api.rs` is byte-identical (see "What I ran"). Only the record is stale.

Suggested fix: state the base as 386a223 and the implementation as 1ad0a29 (written at c61708f on c5464f5, code identical). Say the decision record is on main, give the 357 count after the rebase, and say the ledger update is in 23877c1. This can be done in a correction or in the acceptance record.

### L-2 (Low): the rebuild of the persistent coordinator meets two new startup refusals

The 58090 coordinator must be rebuilt before step 8. That rebuild now runs `check_bindings`, which refuses start (exit 2) in two new cases:
- a private binding names a sprint_id that another repo has already registered in the 55433 DB, which C2 previously allowed;
- two bindings name one id. This case (3) is a sound consequence of C2 but goes beyond the plan's literal C2 row.

Neither REPORT §5 nor the ledger's rebuild precondition says this. A valid configuration is unaffected. If the private configuration has either condition, the rebuild fails and the binding must change, which can affect what step 8 runs.

Suggested fix: add one line to REPORT §5, or to the step-8/rebuild precondition. It should say that before the rebuild the operator checks the private bindings against `sprint_revisions` and `notion_imports` on 55433 (read-only), and that a refusal is resolved by changing the binding, never by editing the stored sprint.

### I-1 (Info): the gRPC checkpoint e2e test is timing-sensitive

On my first full run, `checkpoints_are_reported_compared_and_judged_across_restarts_over_grpc` failed at `checkpoint_e2e.rs:564`. It got seq 14 and current true where it expected 16 and false. Other `cargo test` processes were running on the host at the time. The step depends on `sleep(500 ms)` before the reassignment. The rerun and CI passed. This PR does not change that path: registration happens only once in its setup. It is a pre-existing flake to keep in mind for later steps that touch checkpoints.

### I-2 (Info): no C2 case with a draft-state spec

Every C2 case uses `common::spec`, whose `registration_state` is `ready`. So my mutation MB, which skips the reservation for `registration_state: draft`, survived. The mutation is artificial, because the code never branches on that field. Mode draft and mode executable are both covered (c2n2-0001/0002). Optional: one c2 case with a draft-state body.

### I-3 (Info): small ledger wording

- The 02-05 `result_state` keeps "구현 전" in its earlier, historical sentence, before the appended step-1 sentence. A reader can see the order, but "구현 전" could be marked as history.
- "시험 357 통과(1c21177 위)": the 357 run was on the rebased head d9f81ba over 1c21177. 386a223 is docs-only, and I reproduced 357 at 23877c1, so the number is right.

## Summary for the author

- Approve at 23877c1.
- Optional, in this PR or the acceptance record:
  - L-1: refresh the step-1 REPORT header (base 386a223 and commit 1ad0a29, decision record on main, 357, ledger in 23877c1).
  - L-2: add the pre-rebuild binding check for the 58090 coordinator to REPORT §5 or the step-8 preconditions.
- I-1 to I-3 are notes only.
