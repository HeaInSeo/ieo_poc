# Review of PR #57, head 48b1d0e (review 1 of at most 4; no corrections yet, at most 3)

- **PR:** HeaInSeo/ieo_poc #57, "02-04: provider status and shared quota group (item 4), central checkpoint report/store/compare (item 9)", branch `task/0204-provider-checkpoint`, ledger snapshot 0030.
- **Head reviewed:** 48b1d0e3fd4a25984b700a27cb8e59846f79759f, confirmed with `gh pr view 57 --json headRefOid,statusCheckRollup`. Base: main d337828. Two commits: 5f38ab9 (code) and 48b1d0e (report and ledger).
- **CI at 48b1d0e:** all three required checks finished COMPLETED SUCCESS:
  - docs/poc package and progress validation
  - rust toolchain pin resolves
  - rust fmt clippy test (still running when I started, so I waited for it)
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. I did not run the real `claude` binary, podman or containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge or branch change.
- **Verdict: REQUEST CHANGES.**
  - Item 4 is met as the decision describes it.
  - Item 9's path exists and its happy-path and restart tests are real (real DB, fake runtime).
  - One Medium defect needs fixing before 02-04 is accepted on this code (M-1). A single coordinator refusal of a supervisor report permanently blocks the role, with no recovery path, and it also locks the role's budget ledger. I reproduced this with a scratch test against the real DB; it is not tested in the PR.
  - The other findings are Low or Info.

## What I ran

- Detached worktree of 48b1d0e at `scratchpad/wt57`, with its own `CARGO_TARGET_DIR` and `CARGO_PROFILE_DEV_DEBUG=0`. Both were removed at the end.
- `cargo test -p ieo-coord -p ieo-supervisor` with `IEO_TEST_DB_URL` set to the test PostgreSQL: exit 0, 0 failures.
  - ieo-coord: unit 1, checkpoint_e2e 1, coordinator 68, roles 7, shutdown 2.
  - ieo-supervisor: unit 3, caller 23, run_guard 27, serve 20, supervisor 20.
  - These match PROGRESS §3. The e2e test ran against the DB; it was not skipped.
- **Probe (scratch edit, reverted, not committed).** I appended to `checkpoint_e2e.rs` after step 8:
  - a `start` whose request names a wrong `target_sha`;
  - then a correct `start`.
  - Output in M-1.
- From `docs/poc`, using `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: ok.
  - `scripts/validate_package.py`: rc 0, schema_errors 0.
  - `scripts/test_render_progress.py`: RESULT PASS.
- **Read:**
  - the user decision 2026-11-15;
  - the runtime contract §2.1, §5, §7 and the new §13 items 9 and 10 (revision 9 diff);
  - in full: `provider.rs`, `central.rs`, the `serve.rs` diff and the launch, execute and write_checkpoint paths, the `api.rs` checkpoint section and routes, the `schema.sql`, `auth.rs` and `roles.rs` diffs, the OpenAPI diff, `checkpoint_e2e.rs`, and the new test names in `coordinator.rs`, `roles.rs` and `serve.rs`;
  - PROGRESS.md, PREVENTION.md, `images-fake.sh`, both evidence files, and the `state.json` and `events.jsonl` diffs.
- **Every test name cited in PREVENTION.md exists**, as do the cited files: `test_summarize.py`, the `d3-child-usage` fixture and `build-images.sh`.
- **Secrets scan of the diff:** no real tokens or passwords.
  - The container script uses a random DB password and random tokens kept in umask-077 files removed at exit, and the literal `sk-ant-oat01-fake-not-a-credential`.
  - The evidence log has no tokens, prompts or session text.

## Checks of the user requirements

| Requirement | Status | Evidence at 48b1d0e |
| --- | --- | --- |
| 4: raw signals → §7 values, with source | Met (mock and type definitions) | `provider.rs:40-96`. Each value carries `source` (line and raw fields). The unit test covers all 6 auth values, 3 transient values, rejected, `isUsingOverage`, an unknown status value, a missing flag and a truncated line. |
| 4: observation time and freshness | Met | `observed_not_before` is the attempt's `started_at` (`ieo_evidence::time::now()`, format `YYYY-MM-DDTHH:MM:SS.mmmZ`). `unix_of` parses that format; I checked it against `time.rs:6-19`. Age is counted from the start, and anything older than 600 s is `unknown`, with the observation kept beside it. |
| 4: a running attempt or a restarted service never reuses an old "available" | Met | `serve.rs:342-360`: while running, only that attempt's own `stdout.jsonl` is read. After a restart, `last` is None, so the status is `unknown`, and the old value appears only under `provider_status_before_start` with `current: false`. Covered by `serve.rs` tests 2 and 5 and by the container log. |
| 4: "not observed" is never available or zero | Met | `current(None, …)` returns `unknown`. A group is `available` only when some member observed it (`roles.rs`, `quota_groups`). |
| 4: `quota_group_id` explicit; a shared group spreads `usage_limit` | Met | Required and validated in `Config::from_json`. `quota_groups` spreads `usage_limit` to `affected_roles`, does not spread `auth_or_billing`, and flags a mismatch with the expected group. |
| 9: identifiers linked | Met | `role_checkpoints` stores repo, role, seq, sha, phase, session, assignment, generation, sprint/revision, spec digest, target SHA, image, profile, settings digest and the raw jsonb. It has a FK to `assignments` and an append-only trigger. The report is checked against `assignment_heads` (`api.rs:3396-3447`). |
| 9: local vs central compare; mismatch, missing, stale, gap | Met for the compare; recovery is missing (M-1) | `central.rs:169-249` handles all cases fail-closed, and an unreachable coordinator gives `NOT_OBSERVED` before anything is recorded. Gap → `CHECKPOINT_SEQUENCE_GAP`; stale → `STALE_ASSIGNMENT` / `stale_generation`. |
| 9: not just "session_id stored centrally" | Met | See the identifiers row. The coordinator computes the digest itself over the exact bytes. |
| 9: tests (real DB, fake runtime): normal, duplicate, out of order, mismatch, tamper, restarts | Mostly met | All are present in `coordinator.rs` and `checkpoint_e2e.rs` (steps 2-8). The gap. Identifier mismatch is tested only by posting directly to the coordinator. No test sends a mismatching identity through the supervisor, which is exactly the M-1 path. |
| Transactions and rollback | OK | `run` rolls back on any `Err`. That covers the late tamper-validation error after the `role_checkpoints` INSERT (`api.rs:3473-3483`), so a refused report leaves no row, as the docs claim. A per-(repo, role) `pg_advisory_xact_lock` serializes reports. |
| Authorization | OK | Report: kind agent + `checkpoint:report` + repo + bound role. Latest: the same reporter, or a `roles:control` client of the repo. Compare: `roles:control` only. The model's `agent:report` credential is refused (tested). |
| Race between the report and the local write | OK | One run per role. The report happens on the run thread before `st.running` is cleared, and `launch` refuses while running. A concurrent launch that compares during a takeover at worst resends the same bytes, which the coordinator answers as a duplicate (200). |
| Docs do not overclaim | OK | Mock vs real is labelled per row. Carry-overs say "이월은 완료가 아니다" (a carry-over is not completion). D3's failure is kept (expected 3, answer 4). Status stays "진행" (in progress), 11/73 and 0/12 are unchanged, and nothing is marked accepted. The budget (7/12, 45 s counted, 33,948 / 121,189) is unchanged. |

## Findings

### M-1 (Medium): a refused supervisor report permanently blocks the role and locks its budget ledger

- **Location:**
  - `crates/ieo-supervisor/src/serve.rs:916-959` (`write_checkpoint`: the seq is assigned, the archive and `checkpoint.json` are written durably, and only then is the checkpoint reported);
  - `serve.rs:660` (`record_attempt` runs before the thread starts);
  - `serve.rs:~690-708` (`execute` writes no ledger `end` when the result is `Err`);
  - `crates/ieo-supervisor/src/central.rs:236-247` (the resend loop).
- **What:** the request's `coordinator` record and `target_sha` are checked against the coordinator only when the takeover checkpoint is reported. By then that checkpoint is already the local `checkpoint.json` and is in the archive. If the coordinator refuses it with `ASSIGNMENT_SHA_MISMATCH`, `CHECKPOINT_MISMATCH`, `DIGEST_MISMATCH`, `STALE_ASSIGNMENT` or `NO_CURRENT_ASSIGNMENT`, two things happen:
  1. The run correctly does not spawn. But the local sequence is now one ahead with a checkpoint the coordinator will never accept. Every later `start` or `reconnect` resends it from the archive, is refused again, and stops with `EXTERNAL_RECONCILIATION_REQUIRED`. The central table is append-only, and deleting the local file gives "central has N, local none", which also reconciles. No API or documented procedure gets the role out of this state.
  2. The attempt was recorded in `budget/attempts.jsonl` with no end line. `budget_backstop` then refuses every later run of the role ("attempt … has no recorded end with observed usage; reconcile first"), even though no process was spawned and no model call was made.
- **Same trap, normal operation:** a run whose assignment generation is raised while it runs gets `STALE_ASSIGNMENT` on its `after_run` report. That is the contract action, but the leftover unreportable local checkpoint then blocks every later start, including a `start` of a new session under the new generation.
- **Reproduced** (scratch edit of `checkpoint_e2e.rs`, real DB, fake runtime):
  - `start` with a wrong `target_sha` → 202 accepted (`central.compared true, local_seq 10 = central_seq 10`).
  - `last.error` → "central checkpoint report 11 failed … ASSIGNMENT_SHA_MISMATCH".
  - The next `start` with the correct identity → 409 `EXTERNAL_RECONCILIATION_REQUIRED` ("the coordinator refused checkpoint 11").
- **Fix (any of the following, plus a test):**
  - (a) For `takeover`, report first and write the archive and `checkpoint.json` only after the coordinator accepts. Nothing has run yet, so nothing is lost. On refusal, close the attempt with `spawned: false` and a zero end, like the stop-before-spawn path at `serve.rs:~733`, so the ledger is not left open.
  - (b) Validate the identity before `record_attempt`, for example with a read of the assignment head through `latest` or a dry-run, and still handle the refusal as in (a).
  - (c) Mark a refused local checkpoint void in the archive so that the next write reuses its seq.
  - Also define how a stale `after_run` leftover is handled, for example by superseding it with the next generation's takeover, or by an explicit operator reconciliation record. Document this in contract §13 item 10 and PROGRESS §5.
  - Add an e2e step: "start with a mismatching identity → refused, nothing spawned, the next correct start runs, the budget backstop is not blocked".

### L-1 (Low): repo-scope stop treats a missing `checkpoint_reported` as reported

- **Location:** `crates/ieo-coord/src/roles.rs:169` (`if safe["checkpoint_reported"] == false`).
- **What:** only an explicit `false` adds a reason. A status without the field, for example from an older supervisor image, or with the string `"local_only"`, passes this check. That is the "not observed treated as fine" pattern the decision forbids. Today the impact is nil, because untracked children and external requests always leave safe shutdown unconfirmed. The check becomes load-bearing once 02-05 and 04-05 track those.
- **Fix:** `if safe["checkpoint_reported"] != true`, with a distinct reason for `local_only`.

### L-2 (Low): `checkpoint_reported` is false after every service restart

- **Location:** `serve.rs:322-330`.
- **What:** `central_last` lives only in memory. After the role process restarts, a checkpoint that the coordinator holds (same seq and digest) is reported as `checkpoint_reported: false` until the next run. A successful compare at launch does not update it either. This fails closed, but the status is wrong, and the repo-stop reason would point at reporting rather than at the restart.
- **Fix:** persist the last report result on the volume next to the archive, or compute the field at status time from `latest` (bounded timeout, and `not_observed` if the coordinator is unreachable, never `true`).

### L-3 (Low): the seq comes from the archive alone, so an orphaned archive entry or a lost archive causes a refused takeover

- **Location:** `serve.rs:916` (`archived_max(&archive) + 1`) and `central.rs:213-247`.
- **What:**
  - **Crash between the two writes.** A crash after the archive copy N+1 is written but before `checkpoint.json` is renamed leaves an orphan. The next compare sees local N = central N and passes. The next takeover takes seq N+2, which gives `CHECKPOINT_SEQUENCE_GAP` → the takeover is refused → the M-1 consequences (budget ledger open).
  - **Lost archive.** If the archive directory is lost while `checkpoint.json` (seq 5) survives, the next seq is 1. `checkpoint.json` is overwritten with seq 1, and the report gets `EVENT_CONTENT_CONFLICT`.
  - Both fail closed, but they burn an attempt.
- **Fix:**
  - Use `seq = max(archived_max, local checkpoint seq) + 1`.
  - In `compare`, treat archive entries above `lseq` as an orphan: report them in order, or stop for reconciliation before anything is recorded.
  - Refuse to write when `checkpoint.json` has a seq but its archive copy is missing.

### L-4 (Low): the refusal reason loses the coordinator's code

- **Location:** `central.rs:126-129` (also line 116).
- **What:** coordinator refusals are `Rejected` bodies (`{"rejections":[{"code":…}]}`), not `{"code":…}`. The reason therefore reads "the coordinator refused checkpoint 11 (409 null)", as seen in the probe. The full answer is attached under `coordinator_answer`, but the human-readable reason and the status line lose it.
- **Fix:** use `v["code"]`, falling back to `v["rejections"][0]["code"]`.

### L-5 (Low): OpenAPI gaps

- **Location:** `contracts/ieo-api-v0.openapi.yaml`.
- **What:**
  - (a) `GET /v0/repos/{repo_id}/roles` now returns `quota_groups` (`groups`, `affected_roles`, `auth_or_billing_roles`, `member_status`, `mismatches`), but its response is not described.
  - (b) The new repo-stop reason "checkpoint not reported to the coordinator" is not mentioned.
  - (c) An unknown `{role}` gets 404 `NOT_FOUND` from `need_role` on all three routes. The `reportCheckpoint` and `compareCheckpoint` operations list no 404, and the `latestCheckpoint` 404 is described only in prose.
- **Fix:** add the response schemas and the 404 entries (the enums already contain the codes).

### I-1 (Info): `unix_of` can panic on a pre-1970 or year-0 time in debug builds

`provider.rs:163-172` uses unchecked `u64` subtraction (`y - 1`, `… - 719_468`). The input is the supervisor's own `started_at`, so this is not reachable today. Use `checked_sub` and return `None`.

### I-2 (Info): `compare_checkpoint` calls the supervisor inside an open DB transaction, and the two reads are not atomic

At `api.rs:3543-3551`, the central row is read first and the supervisor status second, inside the transaction. A report landing between the two reads gives a transient `external_reconciliation_required` ("not yet recorded"), and the transaction is held for up to the role call's timeout. Reading the status first, outside the transaction, avoids both.

### I-3 (Info): `status()` reads the live stdout while holding the state mutex

At `serve.rs:342-347`, the status reads the running attempt's stdout under the lock. The run thread's `central_last` update waits on that lock. This is harmless at PoC stream sizes, but the read could be done after cloning `running` and dropping the lock.

### I-4 (Info): freshness edge cases

- A run longer than `provider_status_max_age_seconds` reports `unknown` for its own fresh signals, because age is counted from the start. This is documented as the conservative choice.
- A `started_at` in the future after a clock step gives age 0 (`saturating_sub`). Consider treating a negative age as unreadable.

## Summary for the author

Fix M-1 and add the e2e step for a mismatching identity through the supervisor:

- report or validate before committing the local takeover;
- close the attempt as not spawned;
- define recovery for a stale `after_run` leftover.

Fold in L-3, because it feeds the same path, and preferably L-1, L-2, L-4 and L-5 too. The provider-status work (item 4), the coordinator storage and judgment, the authorization, the transaction handling, and the docs and ledger (no overclaim, D3 kept, not accepted, counts and budget unchanged) need no change.
