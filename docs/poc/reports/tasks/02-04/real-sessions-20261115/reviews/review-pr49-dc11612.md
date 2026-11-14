# Review of PR #49 at dc11612

- **PR**: #49 "02-04: budget-enforcing caller (ieo-role-call), real-session run plan and container dry run"
- **Head reviewed**: `dc1161213836700830fb3f9649b8c7c124534dc3`, confirmed with `gh pr view 49 --json headRefOid,statusCheckRollup`.
  - Base: main `d9123a1`. `origin/main` is still `d9123a1`; the PR is MERGEABLE / CLEAN.
  - Correction 2 is `f90f5b5..dc11612`: code in `7ff27d9`, and `dc11612` changes only `evidence/cargo-test.log`.
- **CI at this head**: 3/3 SUCCESS:
  - `rust fmt clippy test`
  - `rust toolchain pin resolves`
  - `docs/poc package and progress validation`
- **Reviewer**: independent review agent (Claude). This is review 3 of at most 4, after correction 2.
- **Verdict: APPROVE**

N1 and N2 are resolved, and the review-2 Lows are resolved or recorded. I found no new defect that lets a run escape the budget or that blocks D1 or G1. The new findings below are all Low. They can go into a later PR or be recorded, and none needs a correction 3.

## What I ran

I worked in a worktree of the head (`<scratchpad>/rv49c`), with `CARGO_TARGET_DIR=<scratchpad>/target-review49c`. Constraints I kept to:
- no model call and no real claude;
- I did not read `/opt/rust/ieo_poc_secrets`;
- I did not run `run-real.sh` with real secrets.

Evidence labels: **mock** means a fake claude with fake OAuth-form strings. **Stored-log** means the 01-04 streams in the repo. There is no **real** evidence in this review.

- **Lint and tests**:
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
  - `cargo test -p ieo-supervisor --locked`: all passed. Per binary: 1, 0, 0, caller 17, run_guard 26, serve 18, supervisor 20.
  - `evidence/cargo-test.log` sums to 273 passed, which matches the PR body. It was taken at `7ff27d9`, the code commit of this head.
- **Flakiness (N1)**:
  - Caller test binary at the head, default thread count, `timeout 300` per run: **15/15 passed**, about 16 s each.
  - Experiment: I turned `serial()` into a no-op so the tests run in parallel, keeping the caller's 3 s lock retry. Result: **13/15 passed. The lock error ("another caller holds the ledger") never appeared**. Both failures were `the_caller_talks_to_the_service_over_http` with "transport: Connection Failed". See L-c3-1.
  - I restored the source afterwards.
- **My mutations of the correction-2 code** (`--test caller`, source restored after each):
  - Killed:
    - the checkpoint-attempt check disabled;
    - the "usage unmeasured → stop_required" rule disabled;
    - the lock give-up changed to "proceed".
  - Survived:
    - the lock retry window set to 0 s. This is expected, because the tests are now serialised. See L-c3-2.
- **Stored-log measurement**. A temporary test, not committed and since deleted, passed 13 stored 01-04 streams through `ieo_evidence::stream::parse` and then `run::charge_reported`. It used the guard's lower-bound rule and resume bases chained the way the caller chains them. Every run came out **measured**:
  - **New sessions**: 05-G1 (Guardrail, with denied tools), 11-N1, 07-G3a, 24-R1, 01-U, and 23-T (one child).
  - **Resumes**: the chain N1→N2→N3→N4a→L2, plus G3a→G3b and R1→R2.
  - **18-L3 `/compact` resumed on L2**: measured. Attribution is `mismatch`, which is tolerated because `compact_boundaries = 1`. There are no assistant lines, so the guard lower bound is 0.

  So the new rule does not stop a run shaped like G1, like D4 (`/compact`), like the new session of D6, or like a plain resume.
- **`dry-run.sh`** from the worktree (mock). Before starting I checked that ports 58221/58222 were free and that no `ieo-dry-*` containers existed.
  - **All 8 checks PASS**.
  - R1 restored the checkpoint (`after_run D1`, then `last None`).
  - A call in flight was refused (exit 2).
  - **New line**: "restart with a call in flight: restart refused: a call is in flight without a record".
  - Setup over the used workdir was refused.
  - Secret strings: 0 / 0.
  - Stop timeout: 15 s.
- **Review-2 N2 scenario** (mock: fresh workdir, `run-real.sh`, a fake claude that sleeps 15 s):
  1. `call.sh D1`. At 4 s the status was `running D1`. I sent `kill -KILL` to `ieo-role-call` (found with `pgrep -x`): exit 137.
  2. The marker named D1, and there was no `records.jsonl`. After the run ended, the status was `last D1`, checkpoint `D1`.
  3. `call.sh R1` with the marker present was **refused (exit 1)**. It gave three reasons: the in-flight marker, last D1 unknown, and checkpoint D1 unknown.
  4. I removed the marker without writing a record (a wrong reconciliation). `R1` was still **refused** (last and checkpoint unknown). `D2` was **refused (exit 2)**.
  5. I bypassed `call.sh` and ran `podman restart` directly. The status became `last None`, checkpoint `D1`. `R1` was **refused**: "the checkpoint names attempt D1, which has no caller record". `D2` was **refused (exit 2) by the caller**, with reason "the service's checkpoint names attempt D1, which has no caller record". At f90f5b5 only the role-service backstop refused this.
  6. `G1` (Guardrail) **started and was recorded as measured**. This is the cross-role residual that review 2 already noted. See L-c3-3.
- **Cleanup**:
  - I removed both `ieo-dry-*` containers and ran `podman unshare rm -rf` on both workdirs.
  - I removed the worktree, the target directory and my temporary files.
  - The leftover `/tmp/ieo-caller-before-…` from review 2 is gone. During my loops `ieo-caller-lost-*` directories appeared, but only while a test was running, and none remained.

## Status of review-2 findings

| # | Status | Evidence |
| --- | --- | --- |
| N1 caller.lock flakiness / red CI | **Resolved** | See "N1 in detail" below the table. |
| N2 unguarded R1 restart | **Resolved** (for the same role) | (a) `call.sh` R1 now refuses on any of these: the marker; a `stop_required` record; `running`; last unknown; checkpoint attempt unknown; status unavailable. (b) The caller refuses when the persisted checkpoint's `session_cumulative_attempt` is unknown, and this survives a restart. Both are reproduced above, and the mutation is killed. The cross-role residual is L-c3-3. |
| L-new-1 unmeasured but exit 0 | **Resolved** | Any `usage.run.state != "measured"` adds "usage unmeasured: …" → `stop_required` → exit 3. `call.sh` prints the usage state. Tested; the mutation is killed. |
| L-new-2 misleading lock message | **Resolved** | `TryLockError::WouldBlock` → "another caller" (after 3 s); `TryLockError::Error` → `Err("caller lock: …")` → exit 1, before any marker. |
| L-new-3 surviving mutations | **Mostly resolved** | New tests cover: a refused start clears the marker; no result event; stream-save failure; a second caller. "Append, then clear" is covered through the stream-save path. There is still no test that a stop is sent at the deadline (correct by inspection, not blocking). |
| L-new-4 PR description stale | **Resolved**, minor residue | The body now describes the marker, lock, result check, R1 guard and per-source timing. Residue: "Caller mutations: 11/11 killed (`evidence/mutants.txt`)" refers to the correction-1 code and the 14-test suite. It does not cover the correction-2 checks (my 3 kills above do). Worth one clause when the body is next edited. |
| L-new-5 leftover temp dir | **Resolved** | Gone. |
| L2 (serve ledger) | **Resolved** | `append_ledger` writes `format!("{line}\n")` in one `write_all`, then `sync_all`. |

### N1 in detail

**The explanation is plausible.**
- `File::try_lock` is `flock(2)`. A flock belongs to the open file description, so a process forked by another test thread holds a reference to it until `exec` closes the `O_CLOEXEC` descriptor. A `try_lock` in that window gets `EWOULDBLOCK` even though the owner has dropped its `File`.
- My parallel experiment supports this: with the retry in place and the mutex removed, the lock error never occurred in 15 runs.
- The HTTP test fails the same way in parallel (L-c3-1). That is consistent with the same mechanism.

**The 3 s retry is safe.**
- A flock is exclusive, so two real callers can never both hold it.
- A second caller that still sees `WouldBlock` after 3 s returns `NotStarted` before it reads the records, writes the marker or sends a POST, so a refused caller never runs.
- A second caller that gets the lock within 3 s, because the first ended that fast, is an ordinary sequential call: it re-reads the records, marker and status under the lock.
- The test `a_second_caller_is_refused` holds the lock on a separate description and checks refusal and that no run directory was created. Disabling the give-up is caught by that test.

**Serialisation does not hide a production defect.**
- `ieo-role-call` is single-threaded and uses `ureq`; it starts no process.
- The role service runs in its own container process.
- The race exists only because the test process hosts the services and the caller together.

**PROGRESS §6** now says "로컬에서 6회 연속 통과" and states that the CI result is verified by this PR's check. CI is green at this head.

## New findings (all Low; none blocking)

**L-c3-1 (Low, tests only): with the tests in parallel, the HTTP test's readiness probe can be fooled by the same fork window.**
- **Location**: `tests/caller.rs` `the_caller_talks_to_the_service_over_http`, the port probe.
- **Scenario**: 2 of 15 runs failed with the mutex disabled. The test binds port 0 and drops the listener. If another test thread forks while that listener exists, the child keeps a listening copy until `exec`. The probe's `TcpStream::connect` then succeeds against that copy, and `serve`'s own bind fails with `EADDRINUSE`. The loop checks `rx` only after a failed connect, so it never reports the bind error. All requests are then refused for 10 s.
- **Status**: the mutex prevents it, so this is not a defect at this head. It does mean the correction-1 "port-retry fix" was not the fix; the mutex is.
- **Fix (optional)**: after a successful connect, also check `rx.try_recv()`, or use a status GET as the probe. Alternatively, keep the mutex and say in the test comment that it covers this test too.

**L-c3-2 (Low, informational): the 3 s lock retry is untested and has no production purpose.**
- With the tests serialised, the "retry window 0 s" mutation survives.
- Production never needs the retry, because the caller starts no process. It is harmless either way.
- Either keep it and note that it only serves the in-process test harness, or drop it now that the mutex exists. No action is needed for the real runs.

**L-c3-3 (Low, residual of N2, cross-role): the checkpoint and last-attempt checks only look at the target role's service.**
- **Location**: `caller.rs::call_with`, the status checks (L436–467).
- **Scenario** (reproduced, mock): D1's caller is killed and the marker is then removed without a record. That is against RUN_PLAN §4, which says to remove the marker only after a record is written. G1 then starts and is recorded as measured, because the Guardrail service never saw D1.
- **Effect**: the caller's shared ledger undercounts D1 by one run, up to 600 s, with unknown tokens. The development service's own `attempts.jsonl` still counts D1, but only for the development share. The guards hold as long as RUN_PLAN §4 is followed.
- **Fix (cheap, optional)**: before any call, GET the status of **every** role in the plan. Refuse if any role is running, or if any role's `last` or checkpoint attempt has no caller record.
- **Not blocking**: D1 has no predecessor, and G1 follows a D1 that either exited cleanly or left a marker that blocks G1 by itself.

**L-c3-4 (Low): weak points in `call.sh` R1's guard.**
- (a) `curl -s` has no `-f`. A 401 or 5xx JSON body parses as a status with no `running`, `last` or `checkpoint`, so those three checks pass silently. The marker and record checks, which read local files, still apply. The `before=` line then fails, but the restart still runs because the script uses `set -u` without `-e`.
- (b) The guard does not run under `caller.lock`. A call started between the guard and `podman restart` would be killed and recorded fail-closed (unmeasured, `stop_required`), so a run would be wasted, not lost.
- **Fix**: use `curl -sf`, and exit when `before` is empty. For (b), have `call.sh` take `flock "$W/ledger/caller.lock"` around the guard and the restart (`flock -n` with a short wait). R1 involves no model call, so the effect is limited to restart hygiene.

**L-c3-5 (Low, plan risk, not a code defect): D5, the resume after `/compact`, has no stored-log analogue.**
- In 01-04 the `/compact` run (18-L3) was the last run of its session.
- If CLI 2.1.322 does not keep `modelUsage` cumulative across a resume after compaction, D5's session total falls below D4's. D5 is then unmeasured. With this correction that means `stop_required` and exit 3, and the budget stops before D6, so D6 needs a user decision.
- This is the designed, honest outcome: unmeasured is never 0, and there is no retry. D6 cannot move earlier, because its new session ends S.
- **Suggestion**: state this possible stop in RUN_PLAN §2/§4 ahead of time, so that D5 → stop is reported as a §11 item 7 finding and not as a harness failure.

## Is anything blocking the first real call (D1, G1)?

**No.** Checked for D1 and G1 specifically:
- **Clean ledger**: D1 starts on an empty ledger, an empty checkpoint and no `last`. The checkpoint check passes when the checkpoint is null, and the dry run's D1 and G1 show this.
- **Observation window**: the first observation (01-04 27-AM2, one `rate_limit_event`, `rejected`/`false`) is valid until 2026-11-19T08:05Z. **D1 must run before then**, or the call is held, as designed.
- **New result and usage rules**: they do not misfire on G1-shaped runs with denied tools (stored 05-G1, 21/22-C-guardrail: `success`, measured).
- **Exit codes**: exit 0 means recorded, 2 means not started, anything else means stop. `call.sh` prints the subtype, `is_error`, the usage state and `stop_required`.
- **Secrets**: no new output path prints the credential or the adapter token. In my runs: 0 fake credential strings in summaries and container logs.

Before **R1**, run the steps in order: R1 only after D1 and G1 both exited 0. The `call.sh` guard now enforces this, apart from the L-c3-3 and L-c3-4 cases.

## Summary for the author

- Approve as is.
- Optional follow-ups, in a later PR or recorded in the report:
  - L-c3-3: check every role's status before a call.
  - L-c3-4: `curl -sf` and `flock` in R1.
  - L-c3-1 and L-c3-2: test comments and the retry.
  - L-c3-5: a note in the plan.
  - The `mutants.txt` clause in the PR body.
