# Review of PR #58, head 704330f (review 1 of at most 4)

- **PR:** HeaInSeo/ieo_poc #58, follow-up to PR #57 (merged 12bf34f) for the non-blocking findings of review 802e4a1 (N-1, N-2, N-3, I-1 to I-4). Ledger snapshot 0031.
- **Head reviewed:** 704330f9e93539202e519cd2f756aa08f73f2e87, confirmed with `gh pr view 58 --json headRefOid,statusCheckRollup`. Base: main 12bf34f (merge base = main).
- **CI at 704330f** (run 43679988408, waited for it to finish):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
  - The main CI run the ledger cites for 12bf34f (43679480078) is `success` at head 12bf34f.
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. I did not run the real `claude` binary, podman or containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge, CI rerun or branch change.
- **Verdict: APPROVE.** Nothing blocks merge.
  - N-1, N-2, N-3, I-1, I-2, I-3 and I-4 are fixed as described. No regression in the existing tests.
  - One new Low edge (P-1): the N-2 guard refuses a reconnect before a **fresh** detection is written to `pending-tamper.json`, so the detection can be lost if the config is then restored. The same loss already exists on the older "new_session_id required" refusal. It can be fixed in a later PR or carried with a record.
  - Two Info items (P-2, P-3).

## What I ran

- Detached worktree of 704330f in my scratch directory, with its own `CARGO_TARGET_DIR` and `CARGO_PROFILE_DEV_DEBUG=0`. Both were removed at the end.
- `cargo test -p ieo-coord -p ieo-supervisor` against the test PostgreSQL: exit 0, 0 failures.
  - ieo-coord: unit 1, checkpoint_e2e 1, coordinator 68, roles 7, shutdown 2.
  - ieo-supervisor: unit 3, caller 23, run_guard 27, serve 24 (21 + 3 new), supervisor 20.
- **Determinism:** the three new tests, run 6 more times in a row: 3/3 passed each time (0.38-0.43 s).
- **Probes** (scratch tests appended to `tests/serve.rs`, then reverted with `git checkout -- crates`; `git status` clean):
  1. **4xx on a takeover that carries a tamper.** A fake coordinator answers 409 `NOT_CURRENT_ASSIGNMENT` to the takeover and does not store it.
     - The attempt ends `CentralRefused`, with `central_refusal.coordinator_status = 409`.
     - `checkpoint-staged.json` is removed and `pending-tamper.json` is kept.
     - Then I restored the config and sent `start` with s3: 202 with `tamper={old s1, new s3}`. The pending file is removed after the run. The only checkpoints carrying a tamper are s3's takeover and after_run, which is one event under the coordinator's `(repo, role, new_session_id)` key.
  2. **Launch gate and status.** The fake coordinator's `GET latest` sleeps 1.5 s. Launch g1 is in its comparison, and a second launch g2 is started 300 ms later.
     - `GET /v0/role/status` during g1 answered 200 in 0.16 ms. It does not wait for the gate.
     - g2 waited about 1.27 s for the gate, then answered 409 `ALREADY_RUNNING` (from `is_running`). The coordinator saw 1 GET, so g2 never read or dropped the staged files.
     - No deadlock: the gate is taken first and only once, and the state lock is never held while the gate is requested.
  3. **Fresh detection with the tampered session id (P-1).** After s1's run, I tampered with the config and sent `reconnect` with `new_session_id: s1`: 400 `SCHEMA_VIOLATION`, and `pending-tamper.json` was **not** written. After restoring the config, `reconnect` with s9 answered 202 `mode: "resume"`, `tamper: null`.
- From `docs/poc`, using `.venv-poc/bin/python`:
  - `render_progress.py --check`: ok.
  - `validate_package.py`: rc 0.
- **Read:**
  - the full code diff against 12bf34f (`serve.rs`, `central.rs`, the new tests);
  - `serve.rs` `local_checkpoint`, `status`, `launch`, `execute`, `execute_inner` (takeover refusal), `write_checkpoint`, `mark_accepted`, `budget_backstop`, and the `serve` loop;
  - `central.rs` `call`, `report`, `compare`;
  - `api.rs` 3488-3530 (tamper insert);
  - contract §13 item 10, PROGRESS §3 and §9;
  - a field-by-field `state.json` diff against 12bf34f, plus `events.jsonl` and the generated views.
- **Archived reviews:** the three `reviews/review-pr57-*.md` files are byte-identical to the copies in `pending-reviews/`.
- **Secrets scan of the diff:** no tokens, passwords or keys. The tests use the literals `sup-token`, `fake-oauth-token` and `adapter-test-token`, which are fixtures.

## Review 802e4a1 findings

| Finding | Status at 704330f | Evidence |
| --- | --- | --- |
| N-1 tamper recorded twice | **Fixed** | `serve.rs:1142-1144`: the pending file is removed as soon as the takeover is accepted, before `write_local`, so the `LOCAL_WRITE_FAILED` path also clears it. `serve.rs:362-364`: adoption clears it when the staged bytes carry `config_tamper_detected`, and only after both durable writes succeed. Test `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once`: one tamper-carrying checkpoint after a 502 and adoption. With the old code, s4 would have carried the pending tamper again (checked by reading the test against the 12bf34f code). |
| N-2 tampered session id reused | **Fixed** (see P-1) | `serve.rs:775-783`: a carried or fresh tamper whose `new_session_id` equals `old_session_id` is refused with 400. It is checked before the pending write and the spawn. Test `a_pending_tamper_needs_a_session_other_than_the_tampered_one` covers `start` and `reconnect`. The pending tamper's `old_session_id` is always the checkpoint's session in practice, because the checkpoint only moves on an accepted takeover, which clears the pending file. So the one comparison is enough. |
| N-3 stale write order, "어느 경우든 0/0" | **Fixed** | Contract item 10 and PROGRESS §3 now both say `checkpoint.json` first, then the archive. The 0/0 sentence lists only a clear refusal, no answer, and a failed local write after acceptance. It adds that a stop before the staged write or during the report leaves no end line, and that the backstop then blocks the next start until reconciled. This matches `budget_backstop` (`serve.rs`, "has no recorded end with observed usage; reconcile first"). |
| I-1 non-4xx deletes staged | **Fixed** | `central.rs:130` adds `coordinator_status` only on the coordinator-answered branch. `serve.rs:1133-1138` drops the staged copy only for 400-499. A transport error, or a credential-file read error, goes through `not_observed` (503, no `coordinator_status`) and keeps the copy. A 5xx (mapped to a 409 refusal by `report`) keeps it too. The comparison's NOT_OBSERVED (`compare`) never reaches this code. It is returned from `launch` before anything is staged, and adoption treats an unreachable `latest` as "keep" (`_ => {}`). Covered by the 502 test and probe 1 (409). |
| I-2 adoption over a newer local | **Fixed** | `serve.rs:344-346`: a staged seq ≤ the local `checkpoint.json` seq is dropped before the coordinator's answer is looked at. Since `write_checkpoint` numbers a takeover as max(archive, local) + 1, `n == local_seq` means the local write already happened. No dedicated test (P-3). |
| I-3 concurrent launches | **Fixed** | `serve.rs:550`: `launch_gate` is held for the whole of `launch`. `running` is set before it is released, so the takeover in `execute` (which touches the staged and pending files outside the gate) cannot overlap a later launch: that launch stops at `is_running` (`serve.rs:603`) before `local_checkpoint`. `status()` takes only the state lock (briefly, then copies). `launch` drops the state lock before calling `status()`. Probe 2. |
| I-4 label and coverage | **Fixed** | `serve.rs:911` `LocalWriteFailed`. Nothing else in the code, schemas or the caller matches on `CentralRefused` (grep). New tests drive the real `execute` path for the 5xx keep (502) and for `LOCAL_WRITE_FAILED` (a directory in place of `checkpoint.json`), including the 0/0 ledger end and the adoption on the next start. |

### Checks asked for in the brief

- **Adoption conditions.** In order:
  1. A staged seq ≤ local is dropped.
  2. The coordinator's latest has the same seq and SHA-256 → adopt: both files are written durably, then the staged copy is removed, `mark_accepted` runs, and the pending file is cleared if the copy carries a tamper.
  3. The coordinator's latest is below n, or it has none → drop.
  4. Otherwise (unreachable, or a different digest at n) → keep. The comparison that follows then refuses.

  A corrupt `checkpoint.json` reads as local seq 0 and can still be replaced by an adopted copy. This is unchanged from 12bf34f, and the adopted bytes are exactly the coordinator's record. No finding.
- **Pending-tamper lifetime.** Each path:
  - **Written:** in `launch`, before the credential check.
  - **Cleared:** on an accepted takeover, on adoption of a copy that carries a tamper, and in LocalOnly mode after the local write.
  - **Kept:** after a 4xx (the next new session carries it once), after a 5xx or no answer (adoption clears it if the coordinator stored it, otherwise the copy is dropped and the next session carries it), after a stop before the spawn, and after a credential or budget refusal.

  I found no path that reports it twice, other than a silent `remove_file` failure. I found one path that loses a fresh detection (P-1), and one that clears it although the coordinator stored no event (P-2).
- **4xx test.** Covered in the I-1 row above.
- **Launch gate deadlock.** None. The gate is never taken while the state lock is held. `status()`, `stop` and `is_running` never take the gate. `serve` handles requests one at a time on one thread (`recv_timeout` loop), so in production the gate is a guard for library callers and tests. It adds no new blocking: a launch already blocked the loop while it called the coordinator.

## Findings

### P-1 (Low; non-blocking; fix later or carry): a fresh detection refused by the N-2 guard is not persisted

- **Location:**
  - `serve.rs:775-783` (the guard returns before the pending write at `serve.rs:784-791`);
  - the same pattern before this PR: `serve.rs:733` ("new_session_id required" returns before the detection is written).
- **Scenario** (probe 3):
  1. The config is tampered with, and the caller sends `reconnect` with `new_session_id` equal to the checkpoint's session (a caller error).
  2. The answer is 400, and `pending-tamper.json` is not written.
  3. The config is restored before the next reconnect, either by the tamperer or by an operator.
  4. The next reconnect finds no difference and no pending file. It answers `mode: "resume"` on the tampered session, and the detection is never reported.

  At 12bf34f the same request wrote the pending file first, so the detection survived the refusal. The contract sentence "탐지된 설정 변조는 그것을 담은 takeover가 받아들여질 때까지 볼륨에 대기로 남는다" (a detected tamper stays pending on the volume until a takeover carrying it is accepted) does not hold on these two refusal paths.
- **Fix:**
  - In the `Reconnect::TamperDetected` arm, durably write the detection to `pending-tamper.json` as soon as `decide` returns it, before either 400. Its `new_session_id` is overwritten when it is carried.
  - Run the same-id guard after that write.
  - Add a test: tamper, reconnect with the same id (400), restore the config, reconnect with a new id. Expect a new session that carries the tamper, not a resume.

### P-2 (Info): the pending file is cleared even when the coordinator stored no tamper event

- **Location:** `serve.rs:1142-1144`; `api.rs:3518-3523` (`ON CONFLICT (repo_id, role, new_session_id) DO NOTHING` answers `tamper_recorded: false`).
- **Scenario:** a `start` carrying a pending tamper reuses a session id that was already the new session of an earlier tamper event (not the tampered one, so the N-2 guard passes). The takeover is stored, the event is dropped (`tamper_recorded: false`), and the pending file is removed.
- This needs a caller to reuse an old session id. It is the optional third part of the N-2 fix, which was not taken.
- **Fix (optional):** clear the pending file only when the answer has `tamper_recorded: true`. Otherwise keep it, or have the coordinator refuse a takeover whose tamper would be dropped.

### P-3 (Info): no test for the I-2 guard or the 4xx drop

- The `n <= local_seq` drop and the 4xx branch have no test of their own. My probe 1 shows the 4xx branch behaves as documented.
- **Fix (optional):**
  - a serve test with a staged copy at or below the local seq (expect it removed and `checkpoint.json` unchanged);
  - the 4xx case from probe 1.

## Docs and ledger

- **Docs match the code.**
  - Contract §13 item 10 adds: only a 4xx drops the staged copy; adoption also counts the tamper report; a new session after a tamper must have a different session_id; the missing end line and the backstop block.
  - PROGRESS §9 maps each finding to its change and its tests. It claims tests only where they exist: none for I-2 or I-3, and it does not claim one.
  - Overclaim: only the "stays pending until accepted" sentence on the P-1 paths. This is not new wording, but the new refusal adds a path where it is not true.
- **Mock and real are labelled.** No real Claude run is claimed. "모델 호출 0" (zero model calls) appears in the event.
- **Ledger** (snapshot 0030 → 0031):
  - 02-04 `status` stays "진행" (in progress). `verification_level` stays `unverified`.
  - Counts: 11/73 and 0/12, from STATUS and `resume.last_verified_stage`.
  - Budget: 7/12, 40.666 s (45 s counted), billable 33,948, cache_read 121,189. The text is unchanged, and no budget field changed in `state.json`.
  - Changed: snapshot id and times, the phase text, 02-04 `result_state` (one appended sentence on the PR #57 history and the follow-up), `evidence` (+3 archived reviews), `next_action`, `resume.branch_sha` (main 12bf34f), `resume.running` (branch name), and one new event `02-04-PR57-MERGED-20261115`.
  - No count, acceptance or budget field changed. The generated views match (`--check` ok).
- **Secrets:** none.

## Summary for the author

The follow-up closes N-1, N-2, N-3 and I-1 to I-4, the three new fake-coordinator tests are deterministic, and CI is green. **Approve.** Nothing blocks merge.

Worth a small later change, or a carried record in PROGRESS §9:

- **P-1:** persist a fresh detection before the "new_session_id required" and same-id refusals.

P-2 and P-3 can be carried without a record.
