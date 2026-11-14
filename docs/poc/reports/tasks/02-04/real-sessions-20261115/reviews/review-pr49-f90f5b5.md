# Review of PR #49 at f90f5b5

- **PR**: #49 "02-04: budget-enforcing caller (ieo-role-call), real-session run plan and container dry run"
- **Head reviewed**: `f90f5b51daa134ae56843204fa502622e07cd214`, confirmed with `gh pr view 49 --json headRefOid,statusCheckRollup`. Base: main `d9123a1`. Correction 1 is `c4620d2..f90f5b5`.
- **CI at this head**: **`rust fmt clippy test` FAILURE**. The other two checks passed. The failure is `a_running_role_or_an_unknown_last_attempt_starts_nothing` in `tests/caller.rs`, with the message "not started: another caller holds the ledger". See N1.
- **Reviewer**: independent review agent (Claude). This is review 2 of at most 4, after correction 1.
- **Verdict: CHANGES REQUESTED**

Correction 1 fixes the budget-authority defects:
- **H1**: I killed the caller process mid-run in containers. The in-flight marker named the attempt, no record existed, and every following call was refused. The setup re-run was refused too.
- **H2**: resolved.
- **M1–M4**: resolved.
- **L1–L10**: resolved, or recorded in the plan.

I found no new defect that lets a run escape the budget. The verdict is "changes requested" for two reasons:
1. The required CI check is red at this head because of a test race that the correction introduced (N1). The PR cannot merge in this state.
2. The R1 restart step weakens the second H1 guard (N2). The fix is small, so it should go into correction 2.

## What I ran

I worked in a worktree of the head (`<scratchpad>/rv49b`), with `CARGO_TARGET_DIR=<scratchpad>/target-review49b`. There were no model calls and no real claude. I did not read `/opt/rust/ieo_poc_secrets`, and I did not run `run-real.sh` with real secrets. All evidence below is **mock evidence** (fake claude, fake OAuth-form strings), except where I name **stored-log** evidence (01-04 streams in the repo).

- **Lint and tests**:
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
  - `cargo test -p ieo-supervisor --locked`: **the first run failed** (`a_new_session_then_a_resume…`: "not started: another caller holds the ledger").
- **Flake measurement** (`tests/caller.rs` binary):
  - Default parallel threads: **4 failures in 18 runs**.
    - 3 × "another caller holds the ledger", each in a different test.
    - 1 × `the_caller_talks_to_the_service_over_http`: "transport: Connection Failed".
  - `--test-threads=1`: 10 of 10 runs passed.
- **My own mutations of `caller.rs`** (11 in total; `--test caller --test-threads=1`, `timeout 400` each, source restored afterwards):
  - Killed (5):
    - the marker written only after the 202;
    - `is_error` dropped from the result check;
    - the clock check removed;
    - `fail()` without `stop_required`;
    - the run-error branch removed (killed through another path).
  - Survived (6), all on paths that are correct by inspection but untested. See L-new-3:
    - the marker not cleared after a refused start;
    - the marker cleared *before* the record is appended;
    - `caller.lock` removed;
    - "no result event" accepted;
    - a stream-save error ignored;
    - no stop sent at the deadline.

  The author's `evidence/mutants.txt` covers a different set (11 + 2 redone). It is consistent with mine.
- **`dry-run.sh` from my worktree** (workdir under the scratchpad; ports 58221/58222 and the `ieo-dry-*` names were checked free first):
  - all 8 checks PASS;
  - R1 restored the checkpoint;
  - in-flight refusal (exit 2);
  - setup re-run refused;
  - 0 / 0 secret strings;
  - stop timeout 15 s.

  Same result as `evidence/dry-run.log`.
- **Kill-the-caller reproduction** (mock). This was the review-1 H1 scenario, using `run-real.sh` with a fake claude that sleeps 15 s and a fresh workdir:
  1. `ieo-role-call … --role development --op start --attempt D1`. At 4 s the status was `running D1`, checkpoint `takeover`. I sent `kill -KILL` to the `ieo-role-call` PID; it exited with 137.
  2. `ledger/inflight.json` = `{"attempt_id":"D1","op":"start","role":"development","started_at":…}`, and `records.jsonl` did not exist.
  3. After the run ended, the status was `last D1 Exited`, checkpoint `after_run`.
  4. `G1` (guardrail start) and `D2` (development reconnect) were both refused with exit 2: "a call is in flight without a record ({…D1…}): a person reconciles it". No run directory was created on the guardrail volume.
  5. The `run-real.sh` re-run over the workdir was refused, and nothing was deleted.
  6. `call.sh … R1` **ran despite the marker** and restarted the container. Afterwards the status was `last None`. See N2.
  7. I then removed the marker without writing a record, which simulates a wrong reconciliation. `D2` was refused **only by the role-service backstop** (`BUDGET_EXCEEDED`). The caller's unknown-last-attempt check could no longer see D1. See N2.

  Note: my first attempt killed the shell wrapper, not `ieo-role-call`. The caller then finished D1 normally, recorded it as measured and cleared the marker. That is correct behaviour, not a defect, and it is not counted above.
- **Result check against stored logs** (stored-log evidence): I checked all 26 01-04 streams under `remaining-probes-20261112/evidence/attempts/`:
  - every stream ends with `result` `success` / `is_error: false`;
  - every stream has exactly 1 `rate_limit_event`;
  - this includes the `/compact` run (18-L3: `compact_boundary`, then `result success`, `num_turns 0`) and the Guardrail denial-rule runs (05-G1, 21/22-C-guardrail).

  So the new result check (M1) and the "no rate_limit_event" note do not misfire on the planned D4 (`/compact`) or G1 (denials).
- **Cleanup**:
  - I removed both `ieo-dry-*` containers and ran `podman unshare rm -rf` on both workdirs.
  - I removed the worktree and the target directory.
  - One leftover directory, `/tmp/ieo-caller-before-1397666-…`, dates from 01:41, before this review. It is probably from the author's hung `running` mutation (see the `mutants.txt` note). I left it in place.

## Status of review-1 findings

| # | Status | Evidence |
| --- | --- | --- |
| H1 caller record can be lost | **Resolved** | See "In-flight marker lifecycle" below. Specifically: `mark_inflight` (create_new + fsync of file and dir) runs under `caller.lock` before the POST. Any `Err` after the 202 → `fail()` (unmeasured, `stop_required`). The marker is cleared only after a successful `append`, or after a non-202 refusal. The new `before.last` check exists. Exit 1 is documented as stop. Reproduced with a real process kill (mock). |
| H2 setup re-run wipes evidence | **Resolved** | `run-real.sh` refuses when `$W/development`, `$W/guardrail` or `$W/ledger` exists, or when either container exists in any state. It has no `rm` at all now. Refusal reproduced. |
| M1 failed result looks like success | **Resolved** | Anything other than `success` / `is_error false`, or no result at all, → `stop_required`. The binary and `call.sh` print the subtype and `is_error`. The mutation is caught. Stored logs show no false positive. |
| M2 overstated timing of the no-overage sources | **Resolved in RUN_PLAN §3**; **PR body stale** | RUN_PLAN §3 now gives the timing per source and says "관측 뒤 정지이며 공급자 차단이 아님". `call.sh` re-checks the credential prefix on every call. A run without a `rate_limit_event` is noted, and the next call is held (verified in `budget::observation_check`). The PR description still lists the environment as a "before a call" source and says nothing about the marker. See L-new-4. |
| M3 test gaps | **Resolved** | 14 caller tests. The env, guard-stop, missing-observed, max_runs, time-cut and running mutations are now caught (author's evidence, consistent with mine). Residual gaps: L-new-3. |
| M4 plan coverage / real vs mock | **Resolved** | R1 (container restart, no model call) covers checkpoint restoration for real. RUN_PLAN §2 labels the running-run stop and §10 items 4–5 as mock-only, and gives the reason a real stop is not planned. |
| L1 clock | Resolved | `now < observed` → hold. The `unix_now()` fallback of 0 also holds. The mutation is caught. |
| L2 ledger lock / single write | Resolved for the caller; serve residual | The caller writes one `write_all` under `flock` and fsyncs, and `caller.lock` covers the whole call. `serve::append_ledger` still uses `writeln!`. Its records are rare and fail closed, so this can be recorded. |
| L3 workdir guard | Resolved | `realpath -m` runs before the `/opt/rust/src/*` check. |
| L4 dry-run deletes unguarded | Resolved | It deletes only a workdir that contains `fake/claude`, and removes only `ieo-dry-*` containers. |
| L5 dry-run label | Resolved | Replaced by real refusal checks. |
| L6 image by digest | Resolved | `run-real.sh` runs `golang@sha256:d9c68c…`, the same digest that `coordinator.json` asserts. The image exists locally. |
| L7 token in argv | Resolved | The header file is written by the `printf` builtin under `umask 077` and passed as `curl -H @file` (curl 7.61.1 supports it). No `Bearer $(cat …)` remains. |
| L8 lossy stream | Recorded | Code comment and RUN_PLAN §3. |
| L9 `unknown` end line | Recorded | The procedure is in RUN_PLAN §4. |
| L10 PID 1 / `--init` | Resolved | Comment in `run-real.sh`. |

### In-flight marker lifecycle (checked path by path)

- **Before `mark_inflight`**: lock busy, marker present, unreadable records, `stop_required`, duplicate attempt, limits, status `Err` or non-200, `running`, unknown last attempt. Every one of these returns without a marker and without a POST.
  - A marker that exists but is unreadable passes the `read_to_string` check. `create_new` then fails, so the call ends with exit 1 before the POST. This still fails closed.
- **Non-202 answer** → marker cleared, `NotStarted`. This is correct: every non-202 in `serve::launch` is returned before the job thread is spawned. A 500 "attempt ledger" may leave a start line on the service, which over-counts on the service side. That is safe.
- **Transport `Err` on the POST**, including an unreadable response body → `fail()` → unmeasured, `stop_required`, then the marker is cleared. If the append fails, the marker stays.
- **`finish` `Err`**: run error, poll deadline, stdout fetch, non-200 stdout, stream save, serialisation. Each goes to `fail()` as above.
- **Success** → `append`, then `clear`. If `clear` fails, a record and a marker both exist. The next call is refused and a person deletes the marker. This fails closed.
- **Panic or kill anywhere after the mark**: the marker stays. Reproduced.

## New findings

### Must fix before merge (and therefore before the first real call)

**N1 (High for the merge gate, not a budget defect): `caller.lock` makes the caller tests flaky, and the required CI check is red at this head.**
- **Location**: `caller.rs::call_with` (L391–401) together with `tests/caller.rs`, whose tests run in parallel threads of one process.
- **Scenario**:
  - Each test has its own ledger directory, yet `try_lock` reports "another caller holds the ledger".
  - The likely cause: a test thread's role service spawns the fake claude, and while it does, the forked child shares a copy of every open file description, including another test's `caller.lock`, until it `exec`s and `O_CLOEXEC` closes it.
  - When that test's call returns and its next call re-opens and `try_lock`s the file during that window, the old description is still locked.
  - Measured: 3 of 18 parallel runs, 0 of 10 serial runs. The same message failed CI on this head.
  - The HTTP test also failed once with "Connection Failed" despite the port-retry fix (1 of 18). PROGRESS §5 says "이후 8회 연속 통과", but CI contradicts it.
- **Production impact**: none for the budget. `ieo-role-call` is single-threaded and spawns no process. A spurious busy lock would only give `NotStarted` (exit 2), which fails closed.
- **Fix**:
  - Serialise the caller tests. For example, a `static M: Mutex<()>` taken by every test that calls `caller::call`, or a single test binary with `--test-threads=1` set in `.cargo/config` or by the test harness. A retry inside the caller is not the right fix.
  - Make the HTTP test's readiness wait longer, or make it diagnose its failure: report the elapsed time and whether `serve` returned.
  - Then show green CI on the new head. Replace "8회 연속 통과" with a measured repetition count that includes parallel runs.

### Should fix in correction 2 (before step R1; not blocking D1/G1)

**N2 (Medium): R1 restarts the container without checking the caller's state, and the restart erases the service's `last`, which is the caller's second H1 guard.**
- **Location**: `call.sh` L10–22 (the `restart` branch) and `caller.rs` L440–444.
- **Scenario** (reproduced, mock):
  1. D1's caller is killed; the marker names D1.
  2. The operator still runs `call.sh … R1`. Nothing stops it, and `podman restart` succeeds. `status.last` is in memory, so it becomes `None` after the restart.
  3. If the marker is then removed without a record (a wrong reconciliation, against RUN_PLAN §4), the caller's "service's last attempt has no caller record" check sees nothing.
  4. In my run, D2 was refused only by the role-service backstop. That happened by chance: the caller asked for the full share room. A Guardrail call would have started, because its `last` was never set.
  5. R1 run while a run is in flight would also kill that run. That case is recorded fail-closed (unmeasured, `stop_required`) because the polling caller never sees `last` again, but it wastes a run.
- **Fix** (both are cheap):
  - (a) In `call.sh` R1: refuse when `ledger/inflight.json` exists, when any record has `stop_required` or an unmeasured run, when `status.running` is not null, or when `status.last.attempt_id` is not in `records.jsonl`.
  - (b) In the caller: also refuse when the persisted checkpoint's `session_cumulative_attempt` is not in the caller's records. The checkpoint survives restarts, so this check is not blinded by R1.

  In RUN_PLAN §4, also state that R1 runs only after D1 and G1 both exited 0.

### Recordable (Low)

- **L-new-1 — unmeasured but exit 0.**
  - An `Exited` run that is unmeasured with a success result gives `stop_required: false` and exit 0. Examples: a resume base mismatch, or a collector that cannot measure. RUN_PLAN §2 calls that "recorded, nothing to look at", and `call.sh` does not print `usage.run.state`.
  - The next call is still stopped by `budget::next`, so the budget is safe. Only the operator signal is wrong.
  - Fix: return exit 3 when `usage.run.state != "measured"`, or print the state in `call.sh`.
- **L-new-2 — misleading lock message.** `try_lock().is_err()` also reports an I/O error (for example `ENOLCK`) as "another caller holds the ledger". It fails closed. Distinguish `TryLockError::WouldBlock` from other errors in the message.
- **L-new-3 — surviving mutations** (code correct by inspection):
  - no test that a refused start (non-202) clears the marker so that the next call proceeds;
  - no test of the order "append, then clear" (it needs an append-failure hook);
  - no test of `caller.lock`;
  - no test of "no result event" → `stop_required`;
  - no test of a stream-save failure (PROGRESS §5 lists 저장 실패 under H1 tests, but only the code path exists);
  - no test that a stop is sent at the deadline.
- **L-new-4 — PR description stale.** The description still lists the environment as a pre-call source, says nothing about the in-flight marker, `caller.lock`, the result check or R1, and quotes "caller 7" tests and "263 passed". Update it with correction 2. User decision 2026-11-14 pr30 §3 asks for exact statements.
- **L-new-5 — leftover temp directory.** `/tmp/ieo-caller-before-1397666-…` is left over from the author's hung mutation run. Remove it.

## Secrets and publication

- No new path prints the credential or the adapter token:
  - the header file is written by a builtin;
  - the marker holds the attempt, role, op and time only;
  - the summaries now add only the result subtype and `is_error`.
- In my runs: 0 fake credential strings in the dry-run summaries and container logs, and 0 in my reproduction output.
- Raw streams stay in the workdir. The RUN_PLAN §5 pre-publication scan remains mandatory.

## Summary for the author

1. **N1**: fix the test race, and get CI green with a repetition count measured under parallel threads.
2. **N2**: R1 guards in `call.sh`, plus the persisted-checkpoint attempt check in the caller.
3. Update the PR description (L-new-4). L-new-1 to L-new-3 are optional in this PR.

With N1 and N2 done, I see nothing else that blocks the first real call.
