# Review of PR #57, head 802e4a1 (review 3 of at most 4; correction 2 of at most 3 reviewed)

- **PR:** HeaInSeo/ieo_poc #57, "02-04: provider status and shared quota group (item 4), central checkpoint report/store/compare (item 9)", branch `task/0204-provider-checkpoint`, ledger snapshot 0030.
- **Head reviewed:** 802e4a156a86fcd8012e184c83969793f2c273a5, confirmed with `gh pr view 57 --json headRefOid,statusCheckRollup`. Base: main d337828.
  - Correction 2 for review accd6c8: a1cfe78 (code, tests, contract §13 item 10, PROGRESS §3 and §8), 802e4a1 (images rebuilt at a1cfe78, container log, ledger references).
  - `git diff a1cfe78 802e4a1 -- crates contracts Cargo.*` is empty, so the images built at a1cfe78 carry the reviewed code.
- **CI at 802e4a1** (run 43678888888, waited for it to finish):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. I did not run the real `claude` binary, podman or containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge, CI rerun or branch change.
- **Verdict: APPROVE**, with the findings below carried or fixed. Nothing blocks merge.
  - M-1r is fixed: a takeover whose answer is lost, or whose local write fails after acceptance, is staged and adopted on the next start when the coordinator holds exactly it. The role recovers with a new-session `start` (probe 1).
  - M-2 is fixed: a detected tamper survives a refused or stopped takeover in `pending-tamper.json`, and a reconnect no longer resumes over it (e2e 12).
  - L-1, L-2, L-3 and the three Info items are fixed.
  - Two new edge cases where the two fixes meet (N-1, N-2) and stale write-order sentences in the docs (N-3) are Low. They can be fixed in the last correction or carried with a record.

## What I ran

- Detached worktree of 802e4a1 at `scratchpad/rv57c`, with its own `CARGO_TARGET_DIR` and `CARGO_PROFILE_DEV_DEBUG=0`. Both were removed at the end.
- `cargo test -p ieo-coord -p ieo-supervisor` against the test PostgreSQL: exit 0, 0 failures.
  - ieo-coord: unit 1, checkpoint_e2e 1, coordinator 68, roles 7, shutdown 2.
  - ieo-supervisor: unit 3, caller 23, run_guard 27, serve 21, supervisor 20.
- **Probes** (one scratch edit appended after e2e step 13 in `checkpoint_e2e.rs`; real DB, fake runtime; reverted, `git status` clean). To get past the 12-run share, the probe cleared the role's budget ledger and run directories first.
  1. **Recovery after adoption.** After step 13 (the adopted takeover s7, seq 21), `start` with session s8 gives 202 (`compared`, central 21 = local 21). The run ends `Exited`, and central latest is seq 23 `after_run` s8. So the adopted takeover does not block the role.
  2. **Lost answer for a takeover that carried a tamper.**
     - Setup: a takeover seq 24, session s9, carrying a tamper (old s8 → new s9), posted with the supervisor's credential. The answer is 201 with `tamper_recorded=true` (tamper_events 3). The test also wrote `checkpoint-staged.json` (that takeover) and `pending-tamper.json` (the same detection), which is what the 503 path leaves on disk.
     - `start` with session s10 gives 202 with `tamper={old s8, new s10, …}`. Central latest is then s10 with **tamper_events 4**. The one detection is recorded twice (N-1).
  3. **Pending tamper and an explicit `start` that reuses the tampered session id.** I wrote `pending-tamper.json` with old_session_id s10, then sent `start` with session_id s10.
     - The answer is 202 with mode "new" and `tamper={old s10, new s10}`.
     - The coordinator stores the takeover. Its `ON CONFLICT (new_session_id) DO NOTHING` drops the event: tamper_events stays 4.
     - `pending-tamper.json` is removed anyway. The detection is lost, and the "new" session has the old id (N-2).
- From `docs/poc`, using `.venv-poc/bin/python`:
  - `render_progress.py --check`: ok.
  - `validate_package.py`: rc 0, schema_errors 0.
- **Read:**
  - the a1cfe78 diff in full (serve.rs, central.rs, schema.sql, the tests in serve.rs, checkpoint_e2e.rs and roles.rs);
  - `serve.rs` launch, execute, `write_checkpoint`, `local_checkpoint`, status, `mark_accepted` and `record_report`;
  - `central.rs` `report`, `compare` and `durable_write`;
  - `api.rs` tamper insert (3491-3525);
  - contract §13 item 10, PROGRESS §1, §3 and §8;
  - the evidence diff;
  - a field-by-field `state.json` diff against accd6c8 and main.
- **Secrets scan of the correction-2 diff:** no tokens, passwords or keys. The evidence holds only image and binary digests and podman timings, with no prompts or session text.

## Review 2 findings

| Finding | Status at 802e4a1 | Evidence |
| --- | --- | --- |
| M-1r ambiguous takeover outcome blocks the role | **Fixed** | `serve.rs:1090-1110`: staged durably before the report. A 503 (transport, `central.rs:121`) keeps it. A post-acceptance write failure returns `LOCAL_WRITE_FAILED`, so the attempt is not spawned and the ledger end is 0/0. Adoption is at `serve.rs:328-353`: same seq and digest as the coordinator's latest, then both files are written durably and `mark_accepted` runs. A staged copy the coordinator does not hold (latest < n) is dropped. e2e 13 covers adoption, and probe 1 shows a new-session `start` recovers. Docs corrected: §8 says the earlier sentence was an overclaim. |
| M-2 refused takeover erases tamper | **Fixed**, with N-1 and N-2 at the edges | `serve.rs:728-761`: the pending detection is written durably before the credential check. A resume decision with pending present requires `new_session_id`. A new session without a fresh detection carries the pending one. It is cleared only after an accepted and written takeover (`serve.rs:1119-1122`), so the stop-before-spawn path keeps it. e2e 12: 400 without a new session, then 202 with the tamper carried, then tamper_events 2 and the pending file removed. |
| L-1 racy CI test | Fixed | `tests/serve.rs:1090-1140`: the live window is an auth error that does not stop the run, plus a 3 s sleep. `running` is set before the 202, so a 20 ms poll cannot miss it. The rejected case is now asserted after the run. CI is green. |
| L-2 `checkpoint_reported` after a refusal | Fixed | `central_accepted` holds `{seq, sha256}` from an accepted report (`record_report`→`mark_accepted`), an adoption, or a successful compare (`serve.rs:600-606`). The status requires both seq and digest to equal `checkpoint.json` (`serve.rs:407-415`). The record file is written with `durable_write`. |
| L-3 archive repair hides a rollback | Fixed | Write order is now `checkpoint.json`, then the archive (`write_local`). The repair only writes a missing archive copy, durably. A `checkpoint.json` behind the archive is left alone, and compare refuses it. e2e 5 again uses a one-step rollback (archived 3 with archive max 4). The serve test asserts the file is unchanged. |
| I-1 column upgrade | Fixed | `schema.sql:328` `ADD COLUMN IF NOT EXISTS … DEFAULT true`. |
| I-2 different-assignment judgment | Fixed (docs) | Contract item 10 and PROGRESS §8 state both cases. |
| I-3 `!= true` test | Fixed | `tests/roles.rs`: a role without the field is reported as "checkpoint not reported". |

### Checks asked for in the brief (no finding unless noted)

- **Stale staged file adopted wrongly.** Adoption requires the coordinator's latest to be exactly the staged seq and SHA-256. A staged copy from a takeover the coordinator never stored is dropped as soon as latest < n. If latest ≠ n, the copy stays and is overwritten by the next takeover. Two edge cases remain, in I-2 and I-3.
- **Adoption overwriting a newer local checkpoint.** This needs a staged file that survived a successful local write, which happens only if `remove_file` fails silently at `serve.rs:1109` (I-2).
- **`durable_write` temp names.** No collision between files:
  - `checkpoint.json` → `checkpoint.tmp`
  - archive `<seq>.json` → `<seq>.tmp`
  - `write_checkpoint`'s own closure uses `<name>.json.tmp`
  - `archived_max` only parses `*.json`

  The only shared temp name is `checkpoint-report.tmp`, written by both `mark_accepted` and `record_report`. Two writers can only meet in concurrent launches (I-3).
- **Lock ordering.** `record_report` and `mark_accepted` each take the state mutex, release it, and then write the file. No nesting. Neither is called with the lock held: `launch` takes it only after the compare and the pending handling.
- **`checkpoint_reported` wrongly true.** I found no path. Every source of `central_accepted` is a coordinator acknowledgement or an equal-digest compare, and the status compares against the current file bytes.
- **Reconnect decide vs pending.** If `TamperDetected` fires again, the fresh detection replaces the pending one (`(_, r, t)` arm) and is written as the new pending file. `ExternalReconciliation`, `StaleGeneration` and an unfinished run all refuse before the pending handling, and the pending file stays. Both are fine.

## Findings

### N-1 (Low; can be carried): a tamper whose takeover was stored but whose answer was lost is recorded twice

- **Location:**
  - `serve.rs:328-347` (adoption does not look at `pending-tamper.json`);
  - `serve.rs:1104-1108` (`LOCAL_WRITE_FAILED` returns before the pending file is cleared);
  - `serve.rs:1119-1122` (cleared only on the full success path);
  - `api.rs:3518-3523` (the event key is `new_session_id`).
- **Scenario:**
  1. A tamper is detected, and the takeover carrying it (new session s9) is stored by the coordinator, so the event is recorded.
  2. The answer is lost (503), or the local write fails. The staged copy and `pending-tamper.json` both stay.
  3. The next start adopts the staged takeover, but the pending file is still there. The following new session s10 carries the same detection again, and the coordinator records a second event under s10.
- **Reproduced:** probe 2 (tamper_events 3 → 4 for one detection). This contradicts contract §13 item 10 / §5-6, "변조 탐지는 새 세션 하나에 한 번 기록한다" (a tamper detection is recorded once, with a new session). It fails safe: there is no resume and nothing is lost, but the event count overstates the tampering.
- **Fix:**
  - In `write_checkpoint`, clear the pending file as soon as the takeover is **accepted**, before `write_local`. Acceptance is the recording event.
  - In the adoption branch, clear it when the adopted bytes have a non-null `config_tamper_detected`.
  - Extend e2e 13 with a staged takeover that carries a tamper, and assert `tamper_events` does not grow on the next start.

### N-2 (Low; can be carried): a `start` carrying a pending tamper is not checked to be a new session, and its event can be dropped silently

- **Location:**
  - `serve.rs:746-750` (the `(Some(p), false, None)` arm takes the caller's `session_id` as `new_session_id` without comparing it to `p.old_session_id` or to the checkpoint's session);
  - `serve.rs:1119-1122` (the pending file is cleared whether or not the coordinator answered `tamper_recorded: true`).
- **Scenario:**
  1. After a refused tamper takeover, the caller sends `start` with the **old** session id.
  2. The supervisor runs it as mode "new" under the tampered session's id, and the event is stored with old == new.
  3. If that id already has a tamper event, `ON CONFLICT DO NOTHING` drops the new one (`tamper_recorded: false`), and the pending file is still removed. The detection is then lost.
- **Reproduced:** probe 3. It needs a caller error (the coordinator's own flow supplies a fresh id), so this is Low. But M-2's guarantee, "the tamper is reported with the next new session", rests on it.
- **Fix:**
  - In `launch`, when a tamper is carried, refuse a `session_id` (or `new_session_id`) equal to the tamper's `old_session_id` or to the checkpoint's `session_id` (400 `SCHEMA_VIOLATION`, or 409).
  - Optionally, have the coordinator refuse a tamper with `new_session_id == old_session_id`.
  - Optionally, clear the pending file only when the answer says `tamper_recorded: true`, or when an existing event for that session has the same digests.

### N-3 (Low; doc only; fix or carry): stale write-order sentences and one "어느 경우든" (in every case) overclaim

- **Location:**
  - `Runtime_Contract_v0.md:243` (item 10) still opens with "사본을 `/state/checkpoints/<seq>`에 보관한 뒤 `checkpoint.json`을 바꾼다" (archive first, then `checkpoint.json`). A few sentences later the same cell says the order is `checkpoint.json` first, then the archive.
  - `PROGRESS.md:86` (§3 item 1, third bullet) has the same stale sentence, directly above the corrected one at line 87.
  - `Runtime_Contract_v0.md:243`: "어느 경우든 시도는 실행되지 않은 것으로(사용량 0) 닫힌다" (in every case the attempt is closed as not run, usage 0) also covers "그 사이에 멈춘 경우" (a stop in between).
- **What:**
  - The normative contract contradicts itself on the write order, and the code (`write_local`) follows the second sentence.
  - After a supervisor crash, nothing writes the ledger end. A failed write of the staged file itself (`serve.rs:1094-1095`, `?`) returns `Err`, and `execute` writes no end line for an `Err`. That is conservative, because the backstop counts an open attempt at its full window, but it is not "0/0".
- **Fix:**
  - Delete or replace the two stale sentences.
  - Qualify the 0/0 sentence: a refusal, a lost answer or a failed local write is closed as 0/0; after a crash, or if the staged write fails, the attempt stays open and the backstop counts its full window.

### I-1 (Info): any non-503 answer deletes the staged copy

`serve.rs:1099-1101` deletes it for every coordinator answer that is not 200 or 201, including 5xx answers (`central.rs:126-131` maps them to a 409 refusal). A 5xx after the coordinator committed would then bring M-1r back. That is unlikely with the coordinator answering directly, and possible behind a proxy. Adoption already drops a staged copy the coordinator does not hold, so the early delete is not needed. Keep the copy on every error, or delete it only on a 4xx.

### I-2 (Info): adoption does not check that the local checkpoint is older than the staged one

`serve.rs:336-347` overwrites `checkpoint.json` whenever the coordinator's latest equals the staged copy. Suppose the `remove_file` at `serve.rs:1109` failed silently after a successful write, the run then wrote `after_run` N+2 locally, and its report failed. The next start would replace N+2 with the takeover N+1, and the N+2 archive copy would then be stranded above it.

Adopt only when the local seq is missing or lower than the staged seq. Otherwise drop the staged copy.

### I-3 (Info): concurrent launches

`launch` runs `local_checkpoint` (adoption and drop) and the pending write before it takes the state lock (`serve.rs:582` is a lock-free pre-check). This can cause two problems:

- A second launch that overlaps a first launch's takeover in flight can see latest < n and **drop the staged file** while the report is still unanswered. If that answer is then lost, M-1r is back.
- Two concurrent writers can race on `checkpoint-report.tmp`. Errors are ignored there.

The caller normally serializes launches per role. A launch mutex held across compare and decide, released after `st.running` is set, would close this.

### I-4 (Info): labels and coverage

- A post-acceptance local write failure ends as `stop: "CentralRefused"` (`serve.rs:880`) with `central_refusal.code = LOCAL_WRITE_FAILED`, although the coordinator accepted. A distinct stop value would be clearer.
- No test drives the real `execute` path for the 503-keep branch or for `LOCAL_WRITE_FAILED`. e2e 13 writes the staged file itself.
- The new-session recovery after adoption is not in the e2e. Probe 1 shows it works, and a short step 14 would pin it.

## Docs and ledger (fresh pass)

- **Mock and real are labelled.** PROGRESS §1 marks every row 모의 (mock; fake `claude`), 실제 DB (real DB) or 컨테이너 (container). The images are labelled a1cfe78 with the fake runtime. The new §3 rows (e2e 12 and 13) cite only e2e. No real Claude run is claimed.
- **Carry-overs:** §4 is unchanged and still opens with "이월은 완료가 아니다" (a carry-over is not completion).
- **Not accepted:** status is "진행" (in progress). events and state still say "상태 “진행” 유지, 11/73 불변" (status stays "in progress", 11/73 unchanged).
  - Against accd6c8, `state.json` changes only the image and evidence references (5b527c6 → a1cfe78) and "교정 2 반영" (correction 2 applied).
  - Against main, no count, acceptance or budget field changed.
- **Budget:** 7/12, 40.666 s (45 s counted), 33,948 / 121,189, unchanged. No spare runs used. No model calls.
- **Overclaims:** only N-3. The M-1r and M-2 descriptions match the code, apart from the N-1 and N-2 edges.

## Summary for the author

Correction 2 closes M-1r, M-2, L-1, L-2, L-3 and the Info items, and CI is green. **Approve.** Nothing must block merge.

If the last correction is used, these are small and worth doing:

- **N-1:** clear `pending-tamper.json` when the takeover is accepted, and on adopting a takeover that carries a tamper.
- **N-2:** refuse a carried tamper whose session id is not new.
- **N-3:** remove the two stale "archive first" sentences and qualify the "어느 경우든 0/0" (in every case 0/0) sentence.
- **I-2:** add an adoption guard (local seq lower than the staged seq).

Otherwise, record N-1 to N-3 in PROGRESS §4 or §8 as carried, with an owner (02-04 follow-up or 02-05) and an acceptance condition. I-1, I-3 and I-4 can be carried without a record.
