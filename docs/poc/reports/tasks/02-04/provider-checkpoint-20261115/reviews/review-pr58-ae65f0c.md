# Review of PR #58, head ae65f0c (review 2 of at most 4)

- **PR:** HeaInSeo/ieo_poc #58, follow-up to PR #57 (merged 12bf34f). Correction 1 (commit ae65f0c), answering review 704330f (P-1 Low, P-2 Info, P-3 Info). Corrections used: 1 of 3.
- **Head reviewed:** ae65f0c8435c9609b00b5eb4310a9b65e8567473, confirmed with `gh pr view 58 --json headRefOid,statusCheckRollup` before and after the review. Base: main 12bf34f.
- **CI at ae65f0c** (run 43680588590, waited for it to finish):
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- **Reviewer:** independent review agent (Claude Code).
  - No model calls. I did not run the real `claude` binary, podman or containers.
  - I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`.
  - No push, comment, merge, CI rerun or branch change.
- **Verdict: APPROVE.** Nothing blocks merge.
  - P-1 is fixed on every refusal path in `launch` that can follow a fresh detection. A test pins it, and the test fails on the 704330f code.
  - No regression: the full `ieo-coord` and `ieo-supervisor` suites pass, and the 4xx branch is now pinned by a test.
  - Three new non-blocking items: Q-1 (Low, test strength), Q-2 (Info), Q-3 (Info).

## What I ran

- Detached worktree of ae65f0c in my scratch directory, with its own `CARGO_TARGET_DIR` and `CARGO_PROFILE_DEV_DEBUG=0`. Both were removed at the end, and `git worktree list` no longer shows the worktree.
- `cargo test -p ieo-coord -p ieo-supervisor` against the test PostgreSQL: exit 0, 0 failures.
  - ieo-coord: unit 1, checkpoint_e2e 1, coordinator 68, roles 7, shutdown 2.
  - ieo-supervisor: unit 3, caller 23, run_guard 27, serve 25 (24 + 1 new), supervisor 20.
- **Determinism:**
  - The new test, run alone 30 times: 30/30 passed.
  - The whole `serve` suite, run 10 times: 10/10 passed.
- **Probes** (scratch tests appended to `tests/serve.rs`, all with the fake coordinator). In each one, s1 runs first, then the config is tampered with.
  - **A. Credential refusal.**
    1. The credential file is removed, and `reconnect` is sent with s2: 503 `CREDENTIAL_UNAVAILABLE`, and `pending-tamper.json` is written.
    2. The config and the credential are restored, and `reconnect` is sent with s3: 202, `mode: "new"`, `tamper = {old s1, new s3}`.
    3. The pending file is removed after the run.
  - **B. Budget refusal.** `reconnect` is sent with s2 and `timeout_ms` 9,000,000.
    1. The answer is 409 `BUDGET_EXCEEDED` ("timeout + grace 9001 s above the per-run limit 1800 s").
    2. The config is restored, and `reconnect` is sent with s3: 202, `mode: "new"`, and the tamper is carried.
  - **D. A fresh detection while an older one is pending.**
    1. The config is tampered with as "first", and `reconnect` is sent without an id: 400. The detection is saved.
    2. The config is tampered with again as "second", and `reconnect` is sent with s3. The reported `observed_digest` is the "first" digest (Q-2).
    3. One new session carries the tamper (s3's takeover and after_run).
  - **E. `start` on a tampered config:** 202, `tamper: null`, and no pending file. `start` never calls `decide` (Q-3).
  - **F. A corrupt `pending-tamper.json` plus a fresh detection plus a refusal** (Q-2).
    1. The refusal leaves the corrupt file as it was, so the fresh detection is not saved.
    2. After the config is restored, `reconnect` with s3 answers `mode: "resume"`, `tamper: null`.
- **Mutations** (the new test, or the whole serve suite, against a changed `serve.rs`; each change was reverted):

  | Change | Result |
  | --- | --- |
  | M1: `serve.rs` from 704330f | The new test **fails** at `tests/serve.rs:1560` (pending file not written). |
  | M2: delete the I-2 arm (`serve.rs:344-346`) | The new test **passes** (Q-1). |
  | M3: delete the 4xx staged drop (`serve.rs:1153-1155`) | The new test **fails** at `tests/serve.rs:1577`. |
  | M4: put the arm `(Some(p), false, _)` back to `(Some(p), false, None)` | All 30 serve tests pass. The arm only matters in probe D's case, which has no test. |
  | M5: drop the `!exists()` condition (`serve.rs:746`) | All 30 serve tests pass. Same case. |
  | Timing: 300 ms sleep after the staged write in `write_checkpoint` | The new test still passes. Its `!checkpoint-staged.json` check after k3 is not hit by k3's own staged copy in practice. |

  After the reverts, `git status` was clean.
- From `docs/poc`, using `.venv-poc/bin/python`:
  - `render_progress.py --check`: ok.
  - `validate_package.py`: rc 0.
- **Read:**
  - the full ae65f0c diff against 704330f;
  - `serve.rs` `launch` (547-862), `local_checkpoint` (328-380), and the takeover branch of `write_checkpoint` (1130-1185);
  - `central::durable_write` (tmp file, then rename, then directory fsync);
  - the PROGRESS §9 addition, and contract §13 item 10.
- **Archived review:** `reviews/review-pr58-704330f.md` is byte-identical (`cmp`) to `pending-reviews/review-pr58-704330f.md`.
- **Secrets scan of the diff:** none. The fake coordinator answers `ASSIGNMENT_SHA_MISMATCH`, which is a code, not a secret.

## Review 704330f findings

| Finding | Status at ae65f0c | Evidence |
| --- | --- | --- |
| P-1 fresh detection lost on refusal | **Fixed** | `serve.rs:732-754`: the detection is built with `new_session_id: null` and durably written before the "new_session_id required" 400 (`serve.rs:755`). The same-id guard (`serve.rs:791-799`) now runs after the file exists. The second write (`serve.rs:800-807`, carrying the real new id) comes before the credential check (`serve.rs:808`), the locked `ALREADY_RUNNING` check, `BUDGET_EXCEEDED` (`serve.rs:830`) and `record_attempt`. So every refusal after a detection leaves it on the volume (probes A and B, and the test for both 400s). The refusals earlier in `reconnect` (`NOT_OBSERVED`, unreadable checkpoint, unsafe id, live holder, `takeover`/`error` phase, `StaleGeneration`, `ExternalReconciliation`, the comparison) happen before `decide` reports a tamper, so there is no detection to lose. The `is_running` check at the top also comes before any detection. M1 confirms that the test catches the old order. |
| P-2 cleared on `tamper_recorded: false` | Carried as Info (PROGRESS §9) | The reason given (only a caller reusing an earlier new-session id) is accurate. |
| P-3 tests for the 4xx drop and I-2 | **4xx: covered. I-2: only nominally** (Q-1) | M3 fails the new test. M2 does not. |

### Checks asked for in the brief

- **Never reported twice.**
  - The fresh write is skipped when a pending file exists, and the carried tamper is the file's content with the new id. So there is one pending record per detection.
  - It is cleared only on an accepted takeover that carries it (`serve.rs:1160`, `1179`), or on adoption of a copy that carries it (`serve.rs:363`).
  - Probe D and the new test each show one new session carrying the tamper.
- **Never lost.**
  - Holds on all refusal paths, and after a 4xx takeover (test).
  - The one exception is a pending file that exists but does not parse (probe F, Q-2). It is not new: at 704330f a corrupt file was also read as "no pending".
- **The `None` → `_` arm.**
  - Correct, and needed: after the fresh save, `pending` is always `Some` when `tamper` is `Some`. With `None`, the fresh `tamper` would have fallen to `(_, r, t)` and been reported. That gives the same result unless an older detection was pending.
  - With `_`, the older detection wins. Its `old_session_id` is the same (the checkpoint only moves on an accepted takeover, which clears the file). Only `observed_digest` can differ (Q-2).
  - **`op == start`:** `tamper` is always `None` for `start`, which never calls `decide` (probe E). So for `start` the arm is the old `(Some(p), false, None)`, and the change has no effect there.
- **Regression.** None found. All 25 serve tests and the other suites pass, and the earlier tests (`a_pending_tamper_needs_a_session_other_than_the_tampered_one`, `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once`, `reconnect_resumes_an_untouched_session_and_starts_anew_after_tamper`) are unchanged.
- **`reject` flag.**
  - It is one-shot (`swap(false)`). It answers 409 with a `rejections` body and stores nothing.
  - It is checked before the seq logic, so it fires on the next POST, which in the test is k2's takeover.
  - `report` maps it to a refusal with `coordinator_status` 409, so the 4xx drop is exercised. The attempt ends `CentralRefused` (asserted).

## Findings

### Q-1 (Low; non-blocking): the new test does not pin the I-2 guard, but PROGRESS §9 says it does

- **Location:** `tests/serve.rs:1582-1592`; `serve.rs:344-346`; PROGRESS.md §9 table after line 232, row P-3.
- **Scenario:**
  1. The test copies `checkpoint.json` as the staged copy. That copy has the same seq and the same bytes as the coordinator's latest record, which is s1's accepted after_run.
  2. Without the `n <= local_seq` arm (M2), the adoption arm matches instead. It rewrites the same bytes, removes the staged copy and marks it accepted.
  3. Both asserts still hold, so a regression that adopts a staged copy over a newer local checkpoint would pass this test.
- PROGRESS row P-3 says the test confirms "로컬보다 새롭지 않은 대기 사본의 삭제" (the removal of a staged copy not newer than local). The removal is observed, but not that the I-2 guard caused it.
- **Fix (optional):**
  - Stage a copy at a seq ≤ local whose bytes differ from the coordinator's record, for example the local checkpoint with one field changed. Without the guard it would be kept (`_ => {}`). With the guard it is removed.
  - Or reword the PROGRESS row.

### Q-2 (Info): an older pending record wins over a fresh detection, and a corrupt pending file blocks the fresh save

- **Location:** `serve.rs:746` (`!exists()`), `serve.rs:767-769` (an unparsable file reads as `None`), `serve.rs:784`.
- **Scenario 1** (probe D): tamper → refused → tamper differently → new session. The event carries the first `observed_digest`, not the current one. This is consistent with "an earlier unreported detection is kept as it is" (the code comment and PROGRESS), and the takeover checkpoint records the current config digest anyway.
- **Scenario 2** (probe F): `pending-tamper.json` exists but does not parse → a fresh detection is refused before the second write → the config is restored → resume with no report.
  - `durable_write` is atomic (tmp file, then rename, then directory fsync), so this needs outside corruption of the role volume.
  - It is not a regression.
- **Fix (optional):** test "pending parses" instead of `exists()`, or overwrite an unparsable file with the fresh detection.

### Q-3 (Info): `start` does not detect a tamper

- **Location:** `serve.rs:635-639`.
- **Scenario:** `start` with a tampered config and no pending file: the run starts with `tamper: null` (probe E).
- `start` is an explicit new session and gets a fresh config cleanup before each run. The contract places detection on `reconnect`.
- This behaviour comes from before this PR, and the PR does not change it. It is recorded only to answer the brief's question: `start` cannot make a fresh detection, so the arm change does not affect `start`.

## Docs and ledger

- **PROGRESS §9** (new table after line 232):
  - P-1: the description matches the code, including "이미 대기 중인 탐지가 있으면 그것을 유지한다" (an already pending detection is kept), and the named test exists.
  - P-2: carried as Info, with an accurate reason.
  - P-3: overclaims the I-2 part (Q-1).
  - "PR #58에서 생긴 회귀" (a regression introduced in PR #58) is accurate for the same-id path. The no-id path predates the PR, and the fix covers both.
- **Contract §13 item 10:** unchanged. Its "a detected tamper stays pending until a takeover carrying it is accepted" now holds on the refusal paths, except the corrupt-file case in Q-2.
- **Ledger:**
  - `state.json` changes only one line: `evidence` gains `reviews/review-pr58-704330f.md`.
  - The four generated views change only the ledger SHA-256, and the snapshot stays `v1.3.3-EXEC-0031`. `render_progress.py --check` is ok.
  - 02-04 is still "진행" (in progress). Counts are 11/73 and 0/12.
  - Budget: 7/12 runs, 40.666 s (45 s counted), billable 33,948, cache_read 121,189. No budget field changed.
- **Model calls:** none claimed, none made.
- **Secrets:** none.

## Summary for the author

- **Verdict: APPROVE.** Nothing blocks merge, and CI is green (3/3).
- **P-1 is fixed.** A fresh detection is saved before every refusal that can follow it. The new test catches the old order, and the 4xx drop is now pinned too.
- **Optional, may be carried without a correction:**
  - **Q-1:** make the I-2 part of the test use a staged copy whose bytes differ from the coordinator's record, or soften the PROGRESS P-3 row.
  - **Q-2 and Q-3:** Info only.
