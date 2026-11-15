# Review of PR #57, head accd6c8 (review 2 of at most 4; correction 1 of at most 3 reviewed)

- **PR:** HeaInSeo/ieo_poc #57, "02-04: provider status and shared quota group (item 4), central checkpoint report/store/compare (item 9)", branch `task/0204-provider-checkpoint`, ledger snapshot 0030.
- **Head reviewed:** accd6c870760cc478c45c7c09a308182c4d4b2b5, confirmed with `gh pr view 57 --json headRefOid,statusCheckRollup`. Base: main d337828.
  - Correction 1 for review 48b1d0e: ec64d19 (code), 5b527c6 (contract §13 item 10, PROGRESS §3 and §7), accd6c8 (images rebuilt at 5b527c6, container log rerun, ledger references).
  - `git diff ec64d19 accd6c8 -- crates contracts Cargo.*` is empty, so the images built at 5b527c6 carry the ec64d19 code, as PROGRESS says.
- **CI at accd6c8** (run 43677663947, waited for it to finish):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - **rust fmt clippy test: FAILURE.** `tests/serve.rs::provider_status_maps_raw_signals_and_never_reuses_an_old_value` panicked at `serve.rs:1113` ("observed during the run"). See L-1.
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. I did not run the real `claude` binary, podman or containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge, CI rerun or branch change.
- **Verdict: REQUEST CHANGES.**
  - The fix for the M-1 path review 1 reproduced works. A refused takeover writes nothing, the ledger end line is 0/0, and the next start runs. A reassignment during a run is stored as `current_assignment=false` and the role recovers with a new session. The `continues` exception is tight.
  - The fix has two gaps of the same kind:
    - M-1r: when the takeover's outcome is ambiguous (the answer is lost after the coordinator stored it, or the local write fails or crashes after acceptance), the role is still blocked for good, and the docs claim the "unreachable" case is not blocked.
    - M-2: a refused takeover now erases a detected config tamper, so the next reconnect resumes the old session and no tamper event is recorded.
  - Both are reproduced against the real DB.
  - A required check is red (L-1).

## What I ran

- Detached worktree of accd6c8 at `scratchpad/wt57`, with its own `CARGO_TARGET_DIR` and `CARGO_PROFILE_DEV_DEBUG=0`. Both were removed at the end.
- `cargo test -p ieo-coord -p ieo-supervisor` against the test PostgreSQL: exit 0, 0 failures.
  - ieo-coord: unit 1, checkpoint_e2e 1, coordinator 68, roles 7, shutdown 2.
  - ieo-supervisor: unit 3, caller 23, run_guard 27, serve 21, supervisor 20.
  - These match PROGRESS §3.
- The CI failure: `provider_status_maps…` passed 30 of 30 runs alone and 15 of 15 runs of the whole serve binary pinned to one CPU. It did not fail locally; it is a timing race (L-1).
- **Probes** (scratch edits of `checkpoint_e2e.rs`, real DB, fake runtime; all reverted, `git status` clean):
  1. **After e2e step 10 (a refused takeover):**
     - The ledger tail is `{"attempt_id":"a8","end":{"observed_billable":0,"observed_cache_read":0}}`. Good.
     - The status says `checkpoint_seq=12 reported=false last_report=13`, although checkpoint 12 is the coordinator's latest. See L-2.
  2. **After step 11, a takeover the coordinator stored but the supervisor never wrote** (I posted seq 19 directly with the supervisor's report credential, which simulates a lost answer or a crash or write failure after acceptance):
     - The next `start` gives 409 `EXTERNAL_RECONCILIATION_REQUIRED`: "the local checkpoint 18 is older than the coordinator's 19 (rolled back or replaced)".
     - `compare` gives `external_reconciliation_required`. See M-1r.
  3. **After step 6:**
     - I tampered with `claude/CLAUDE.md` and sent a reconnect naming a wrong `target_sha`: 202 with tamper=true, then `CentralRefused` with spawned=false. The tampered `CLAUDE.md` is gone afterwards.
     - A correct reconnect then gives 202 `mode="resume"` of the **old session s1** with tamper=null.
     - The central record is seq 10, session s1, `tamper_events=0`. See M-2.
- From `docs/poc`, using `.venv-poc/bin/python`:
  - `render_progress.py --check`: ok.
  - `validate_package.py`: rc 0, schema_errors 0.
- **Read:**
  - the ec64d19 diff in full (serve.rs, central.rs, provider.rs, api.rs, schema.sql, roles.rs, OpenAPI, tests);
  - `serve.rs` launch, execute and status;
  - `central.rs::compare`;
  - `api.rs::report_checkpoint` and `judge_resume`;
  - contract §13 item 10 and PROGRESS §1, §3, §4, §5 and §7;
  - both evidence files;
  - the `state.json` and `events.jsonl` diffs.
  - A field-by-field `state.json` diff against main shows only snapshot, phase, 02-04 next/unresolved/evidence, resume text, and one X-02 blocking condition added from the user decision. No count or budget field changed.
- **Secrets scan of the PR diff:** no real tokens or passwords.
  - It contains only the fake `sk-ant-oat01-fake-not-a-credential` and the container script's random `$PGPW`.
  - The evidence logs have no tokens, prompts or session text.

## Review 1 findings

| Finding | Status at accd6c8 | Evidence |
| --- | --- | --- |
| M-1 refused report blocks the role and ledger | **Fixed for a definite refusal; still open for an ambiguous outcome (M-1r)** | `serve.rs:1003-1011` reports before writing. A refusal returns `Ok(Some(refusal))` and `execute_inner` closes the attempt as `CentralRefused`/`spawned:false`. `execute` writes the 0/0 end (`serve.rs:745-746`; ledger confirmed by probe 1). e2e 10 shows the next start runs (seq 14). Reassignment during a run: `api.rs:3424-3431` and e2e 11 (`current_assignment=false`, `stale_generation`, new session under generation 2 → seq 18, `resume_allowed`). |
| `continues` cannot store arbitrary non-current checkpoints | OK | It requires phase ≠ takeover and the **latest** stored row to be a `takeover` with the same session, assignment_id and generation. It then still checks that the named assignment was issued for this repo and role, and that sprint, revision, target SHA and spec digest match it (`api.rs:3447-3481`). After one continuation, the latest row is `after_run`/`error`, so a second one is refused. The generation floor still applies. A takeover must be current. |
| L-1 missing `checkpoint_reported` | Fixed | `roles.rs:171` `!= true`. No test covers the missing or `local_only` field (Info I-3). |
| L-2 report state lost on restart | Fixed, with a new inaccuracy (L-2 below) | `checkpoint-report.json` is read at start (`serve.rs:289`). Asserted in e2e 4. |
| L-3 seq from archive only / orphan | Fixed, with a side effect (L-3 below) | `serve.rs:979-985` uses `max(archive, local) + 1`. The repair is in `local_checkpoint` (`serve.rs:307-325`). |
| L-4 refusal code | Fixed | `central.rs:136-142`. |
| L-5 OpenAPI | Fixed | `quota_groups` schema, stop reason, 404s, `current_assignment` and the exception are documented. |
| I-1, I-4 | Fixed | `provider.rs` handles year 0 and future start times, with tests. |
| I-2 | Fixed | `need_compare`, then `role_status` outside the transaction, then DB reads under the report lock (`api.rs:342-349`, `3601`). The supervisor read is still not atomic with the DB reads; a transient result is possible and harmless. |
| I-3 | Fixed | The status copies the state and reads files after the lock is released (`serve.rs:336-345`). |

## Findings

### M-1r (Medium): an ambiguous takeover outcome still blocks the role permanently, and the docs claim it does not

- **Location:**
  - `crates/ieo-supervisor/src/serve.rs:1003-1015` (report, then archive and `checkpoint.json`);
  - `central.rs:121-124` (any transport error, including a 10 s timeout after the request was sent, is `NOT_OBSERVED`);
  - `central.rs:222-226` (central ahead → reconcile);
  - `serve.rs:793` (`?`: a local write error after acceptance is an `Err`, and `execute` writes no ledger end for an `Err`, `serve.rs:738`).
- **What:** the takeover is reported first, but the supervisor treats "no answer" as "not stored". There are three cases:
  1. The coordinator stores seq N+1, but the answer is lost: a timeout, a connection reset, or the coordinator dying after commit. The supervisor writes nothing locally and closes the attempt as 0/0.
  2. The coordinator accepts, but the archive or `checkpoint.json` write fails (disk full; this host is at 90 %).
  3. The supervisor crashes between the accepted report and the archive write.

  In all three, the central record is N+1 and the local one is N. `compare` refuses every later `start` and `reconnect` with "local checkpoint N is older than the coordinator's N+1 (rolled back or replaced)". The table is append-only, and there is no adoption path. That is the permanent block M-1 was about. In case 2 the budget ledger is also left open, because the `Err` path writes no end line, although nothing was spawned.
- **Reproduced:** probe 2. A takeover stored centrally but never written locally gives 409 `EXTERNAL_RECONCILIATION_REQUIRED` on the next start, and `compare` gives `external_reconciliation_required`.
- **Overclaim:** these sentences say the case is not blocked:
  - PROGRESS §3 item 3 (`PROGRESS.md:103`): "takeover 보고가 거부되거나 **닿지 않으면** … 다음 기동이 막히지 않는다" (if the takeover report is refused or does not reach the coordinator, the next start is not blocked).
  - Contract §13 item 10 (`Runtime_Contract_v0.md:243`), the same sentence.
  - PROGRESS §7 M-1 row.

  They hold for a refused report or a connection refused before sending, not for a lost answer.
- **Fix (any of the following, plus a test):**
  - (a) Before reporting, stage the bytes, for example as `checkpoints/pending-<seq>.json`.
    - On a definite refusal (4xx answer), delete the staged file.
    - On a transport error, keep it.
    - In `compare`, when `cseq == lseq + 1` and the staged file's SHA-256 equals the central digest, adopt it: write the archive copy and `checkpoint.json`, then compare again. The adopted checkpoint is a `takeover` that never ran, so `reconnect` reconciles on it and a new-session `start` works. That is the same recovery as for a reassignment.
  - (b) On a transport error, ask `latest` once. If it holds seq N+1 with this digest, treat the report as accepted and write locally.
  - In either case, a local write error after acceptance should close the attempt as not spawned (0/0) rather than return `Err`. Nothing has been spawned at that point.
  - Correct the two doc sentences to say what is guaranteed.
  - Add an e2e step: "takeover stored but the answer is lost → the next start recovers".

### M-2 (Medium): a refused takeover erases a detected config tamper; the next reconnect resumes the old session

- **Location:**
  - `serve.rs:776` (`configdir::clean` runs before the takeover);
  - `serve.rs:792-805` (refused → return with nothing written);
  - `serve.rs:662-676` (`launch` detects the tamper only by comparing the local checkpoint's settings digest with the config dir).
- **What:**
  - Before correction 1, the takeover checkpoint, carrying `config_tamper_detected`, was written locally before the report. A refusal therefore left it on disk, and the tamper stayed visible.
  - Now a refused or unreachable takeover writes nothing. But the attempt has already cleaned the config dir back to the template, which removes the evidence of the tamper.
  - The next `reconnect` sees a config dir that matches the old checkpoint, decides `Resume` of the old session, and the tamper event is never recorded. Contract §5-6 and §13 item 10 ("변조 탐지는 새 세션 하나에 한 번 기록한다", a tamper detection is recorded once, with a new session) are violated.
  - Any coordinator refusal of that attempt is enough to trigger it: a stale target SHA, a racing reassignment, or an unreachable coordinator. The stop-before-spawn path (`serve.rs:778-788`) has the same gap, which already existed before this PR. It now has a second, more likely trigger.
- **Reproduced:** probe 3.
  - Tamper, then reconnect with a wrong `target_sha`: 202 with `tamper.config_tamper_detected=true`, then `CentralRefused`, and `CLAUDE.md` is removed.
  - Correct reconnect: 202 `mode="resume"`, session s1, `tamper=null`.
  - Central latest: session s1, `tamper_events=0`.
- **Fix:** keep the detection until a takeover that carries it is accepted. Either of these works:
  - Persist `job.tamper` to the volume (for example `state/pending-tamper.json`) before cleaning. `launch` then treats it as `TamperDetected`: a new session is required and the tamper is carried. Clear it only when the takeover carrying it is accepted.
  - Or report the takeover before `configdir::clean`, which needs the post-clean digest computed without changing the directory.

  Apply the same rule to the stop-before-spawn return. Add an e2e step: "tamper + refused takeover → the next reconnect still requires a new session and records the tamper once".

### L-1 (Low; blocks merge): required check `rust fmt clippy test` is red at the head, from a racy test

- **Location:** `crates/ieo-supervisor/tests/serve.rs:1089-1113`.
- **What:**
  - The fake prints a `rejected` window and then sleeps 3 s. The run guard stops the run on that event at once: CI shows `stop: Overage(...)` and `elapsed_ms: 7`.
  - The loop polls `status()` every 50 ms and asserts that it caught the attempt while `running`. That only works if a poll lands inside the few milliseconds between the event write and `running = None`. The CI poll missed it.
  - This is not caused by the I-3 change; the race came in with 5f38ab9. But the required check is red at accd6c8, and the PR cannot merge on it.
- **Fix:** make the live window deterministic. Any of these works:
  - Use a live signal the guard does not stop on, for example an `allowed` window after an earlier attempt's `usage_limit`, then assert the running status is not the old value.
  - Keep the attempt alive with a fake that emits the event only after the test signals it, for example through a file the script waits for.
  - Assert on the run's own recorded observation instead of on `running`.

  Then rerun CI.

### L-2 (Low): a refused takeover makes `checkpoint_reported` false for a checkpoint the coordinator holds, and this is persisted

- **Location:** `serve.rs:1006` (`record_report` for the refused takeover), `serve.rs:1026-1045` and `serve.rs:364-373`.
- **What:**
  - The refused takeover's seq N+1 is recorded as the last report with `ok:false`, and written to `checkpoint-report.json`. The status compares the local seq (N) with the last report's seq. So it says `checkpoint_reported: false` for checkpoint N, which the coordinator does hold (probe 1: `checkpoint_seq=12 reported=false last_report=13`).
  - This survives restarts and lasts until the next accepted report. The reports `compare` resends from the archive are never recorded either.
  - The record keeps no digest. So a `checkpoint.json` replaced with other content under the same seq still shows `reported: true` after a restart. `compare` at launch catches that, but the repo stop does not.
  - It fails closed, but the repo-stop reason points at reporting rather than at the real state. That is the same kind of wrong status L-2 in review 1 was about.
- **Fix:**
  - Keep the record for the last **accepted** checkpoint (seq and SHA-256) apart from the last failed attempt.
  - Compute `checkpoint_reported` as seq and digest equal to the local `checkpoint.json`.
  - Update the record after a successful resend in `compare`.
  - Write `checkpoint-report.json` atomically (temp file and rename), as for the checkpoint.

### L-3 (Low): the one-behind archive repair also hides a one-step rollback of `checkpoint.json`, without a trace; the rollback test was weakened

- **Location:**
  - `serve.rs:307-325` (`local_checkpoint`; the repair uses a plain `std::fs::write` at `serve.rs:320`);
  - `crates/ieo-coord/tests/checkpoint_e2e.rs:392-393` (the rollback case changed from "archived 3" to "archived 2");
  - PROGRESS §3 table, row "로컬 불일치·누락·되돌림" (local mismatch, missing, rollback).
- **What:**
  - The repair cannot tell an interrupted write from `checkpoint.json` being rolled back by exactly one step. Both are restored from the archive silently. That is safe in the sense that the restored bytes are then checked against the central seq and digest. The archive is on the same volume, so a full-volume rollback is still caught (central ahead).
  - But a one-step rollback is no longer **detected or reported**. The e2e test was moved to a two-step rollback to keep passing, and PROGRESS still lists "되돌림 → 409" (rollback → 409) without the qualifier.
  - The repair write is not atomic or durable. A crash during it leaves a torn `checkpoint.json`, which then needs reconciliation ("the local checkpoint cannot be read").
- **Fix:**
  - Record the repair (`restored_from_archive: {from, to}`) in the launch answer, the status and the run outcome, so that it is visible.
  - Restore with the same durable temp-file, fsync and rename helper.
  - In PROGRESS §3, say that a one-step difference is repaired, verified against the central record and reported, and that rollback detection is tested for two steps or more.
  - Optionally, restore only when the archived N+1 is a plausible successor of N: the same session for `after_run`/`error` after a `takeover`, or a `takeover` after `after_run`.

### I-1 (Info): the new NOT NULL column has no upgrade path for a database that applied the 48b1d0e schema

`schema.sql:306` adds `current_assignment boolean NOT NULL` inside `CREATE TABLE IF NOT EXISTS`. A database that already created `role_checkpoints` at 48b1d0e (any local run of the earlier head) keeps the old table, and every report then fails on the INSERT.

- The table never reached main.
- The tests drop and recreate their database.
- The persistent PoC coordinator runs main 778ee47.

So the impact is nil today. The file already follows the pattern `ALTER TABLE … ADD COLUMN IF NOT EXISTS` (lines 224, 259). Adding `ALTER TABLE role_checkpoints ADD COLUMN IF NOT EXISTS current_assignment boolean NOT NULL DEFAULT true` would keep the migration idempotent. The other choice is to state in the PR that a database used at 48b1d0e must be recreated.

### I-2 (Info): `judge_resume` gives `stale_generation` only for the same assignment_id

A reassignment to a **different** assignment during a run gives `external_reconciliation_required` (`api.rs:3679-3683`), not `stale_generation`. The recovery is the same: a new session. PROGRESS §7 and contract §13 item 10 say only "판단은 `stale_generation`" (the judgment is `stale_generation`). It is tested for a generation bump only. It would help to state the other case, or to judge it as stale too.

### I-3 (Info): L-1's `!= true` has no test

No test covers a role status without `checkpoint_reported`, or with `"local_only"`. A small `tests/roles.rs` case with a stub status would pin it.

## Docs and ledger (fresh pass)

- **Mock and real are labelled:**
  - per row in PROGRESS §1 ("모의" = fake `claude`, never a real Claude run);
  - §5 repeats that no real session passed items 4 or 9;
  - the provider's rejected and auth-error output shapes are marked as an assumption;
  - the container evidence is labelled as image 5b527c6 with the fake runtime. It does not exercise the correction-1 paths (refused takeover, reassignment); those rows cite e2e only, which is accurate.
- **Carry-overs:** §4 opens with "이월은 완료가 아니다" (a carry-over is not completion). Prior states are preserved, and each row has an owner, an acceptance condition and a blocking condition. The item 8 blocking condition is also added to X-02's `unresolved`.
- **Not accepted:** status is "진행" (in progress); "이 기록은 02-04 수용이 아니다" (this record is not the 02-04 acceptance). 11/73 and 0/12 are unchanged. D3's failure is kept (expected 3, answer 4).
- **Budget:** 7/12, 40.666 s (45 s counted), 33,948 / 121,189. No budget field changed against main. No spare runs were used.
- **Overclaims:** the "닿지 않으면" (if it does not reach) sentences in M-1r, and the rollback row in L-3. Nothing else found.

## Summary for the author

Close the two gaps the report-first ordering opened, each with an e2e step:

- **M-1r:** adopt a takeover the coordinator stored when its answer was lost or the local write failed, for example through a staged file plus adoption in `compare`, or a `latest` check. Close a post-acceptance write error as not spawned. Correct the doc sentence.
- **M-2:** keep a detected tamper across a refused or stopped takeover.

Fix the racy test so the required check is green (L-1). Preferably also:

- **L-2:** base `checkpoint_reported` on the last accepted seq and digest, and write the record atomically.
- **L-3:** make the archive repair visible and durable, and qualify the rollback row.

The rest of correction 1 is sound:

- report-first for a definite refusal, with the 0/0 ledger end;
- `current_assignment` and the bounded `continues` rule;
- L-1, L-4 and L-5;
- I-1 to I-4;
- the docs and ledger (no acceptance, counts and budget unchanged, mock and real labelled).
