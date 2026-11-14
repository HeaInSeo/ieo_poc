# Review 2 of PR #40: 02-04 run guard (correction 1)

- Head reviewed: `d2989f75e468b762dc4878747a4f3cf95034fe8d` (confirmed with `gh pr view 40 --json headRefOid`), base `f582403`. Correction diff checked: `git diff 3103933 d2989f7`.
- CI run 43584575005 at this head: all checks pass.
- **Verdict: CHANGES REQUESTED** (2 Medium, 1 Low, 3 Info; no High)

## What I ran

All work was done in a scratch `git worktree` at the head with its own `CARGO_TARGET_DIR`. Both were removed afterwards.

- `cargo clippy -p ieo-supervisor --all-targets -- -D warnings`: clean.
- `cargo test -p ieo-supervisor`: 17 (`run_guard`) + 20 pass.
- `run_guard` binary run 30 times in sequence, then 6 x 10 in parallel (90 runs): 0 failures. No `sleep 300`, fake processes or `/tmp/ieo-run-*` directories were left behind.
- PR `mutants.py` (`.venv-poc` python): 19/19 killed, with the same tests named as in `mutants.txt`.
- My extra mutants: results under finding 2 and finding 4.
- Probes: a throwaway test file, not committed. Results are cited in the findings.
- Fixture check: per-message-id max of `assistant` usage compared with `result.usage` and `modelUsage`, and the position of the first `rate_limit_event`, for every stream in `crates/ieo-evidence/tests/fixtures/`.

## Review 1 findings: status

| # | Status | Notes |
|---|---|---|
| 1 High: error exits leave the group | Resolved | Evidence files are created before `spawn`. Every `watch` result, `Err` included, goes through `stop_group` and the final group KILL. |
| 2 High: unbounded drain | Resolved | The drain is bounded by `min(exit + 2 s, deadline)`. Drained lines go through `on_line`. `try_wait` runs on every loop iteration. Covered by `the_drain_after_exit_is_bounded…` and `lines_after_the_leader_exits…`. |
| 3 High: usage is a lower bound | Resolved | The module doc and PROGRESS §2 now state the lower bound, output at message start, compaction, and final usage only from `result`. Small precision points are in finding 5. |
| 4 Medium: non-UTF-8 or closed stdout blinds the guard | Resolved | `read_until` with lossy decoding. `End`, `Broken` or `Disconnected` while the leader is alive gives `StreamLost` within 2 s. Tested. |
| 5 Medium: no `kill` causes an unbounded wait | Resolved for an unspawnable `kill` | `child.kill()` is used, and `cleanup_failed` is set. A `kill` that runs but fails is still silent on one path (finding 2). |
| 6 Medium: test gaps | Resolved | X1–X4, X7 and X8 now map to R9–R13 and are killed. The collector-backed test exercises `run`, then `ieo-evidence`, then `budget::next`. |
| 7 Medium: `for_next` | Resolved | It wraps `budget::next`, returns `Err(decision)` unless the decision is `Start`, refuses a zero room, and refuses a window of `GRACE` or less. Tested for unmeasured, unclassified, no runs left, extra usage, no observation, and time within the grace. |
| 8 Low: grace overrun | Resolved | `timeout = window − GRACE`, killed by R16. A residual overrun of milliseconds remains (finding 6). |
| 9 Low: id-less lines | Resolved | Each id-less line gets its own key. `incomplete_lines` counts lines with missing fields. |
| 10 Low: stale `budget.rs` doc | Resolved | |
| 11 Info: failing tests leak processes | Partly resolved | See finding 3. |

## New findings

### 1. Medium: after a stop decision, the guard drops lines the process has already written, including `result`. A run whose final usage was reported then becomes unmeasured.

After `watch` returns `Stopped`, nothing more is read from the channel:
- lines already queued behind the stopping line are dropped;
- lines written during the SIGTERM grace are dropped;
- `drop(rx)` discards whatever is left.

The same applies when the stop happens inside `drained`, where the leader has already exited and its `result` is usually in the queue.

Probes (3 out of 3 runs each):

| Probe | Fake | Outcome | Copy |
|---|---|---|---|
| P1 | prints a cap-crossing `assistant` line and `result`, then `exit 0` | `TokenCap`, `exit_code=Some(0)` | 1 line, **no `result`** |
| P2 | same, then `sleep 0.3` | `TokenCap` | 1 line, no `result` |
| P3 | the TERM handler prints a `result` line and exits 0 | `RunTimeout`, `exit_code=Some(0)` | **empty** |

Why it matters:
- `stdout.jsonl` is the evidence the 02-01 collector measures, and PROGRESS §2 says the overrun "can be known only when there is a `result`".
- The guard throws away exactly that `result` for the run that crossed the room, or that ended within the grace. The run is recorded as unmeasured even though the CLI reported its final usage.
- The direction is conservative: the next call is blocked. But the actual usage of the overrunning run is lost, and the user is asked for a decision that the evidence could have avoided.
- The copy is also not a faithful copy of stdout after a stop, and the docs do not say so. The table row "stdout이 그대로 복사된다" covers only normal runs.
- Extra mutant E4 (do not copy the stopping line itself) SURVIVED, so line copying after a stop is untested.

Requested:
- After the stop decision, keep copying every received line to `out` without further decisions. Do this during `stop_group` and after the final KILL, until `End` or a short bound, since a descendant that left the group may hold the pipe.
- Add a test in which a `result` written after the stopping line, or in the TERM handler, appears in the copy. Also feed that copy through the collector: it should be measured.
- Alternatively, if dropping lines is kept, state it in the module doc and PROGRESS §2, along with its consequence: the run becomes unmeasured even when a `result` was emitted.

### 2. Medium: on the normal-exit path, a `kill` that runs but fails is reported as success, so a surviving descendant goes unreported. The fallback code is untested.

`signal_group` treats a non-zero `kill` status as delivered when `/proc/{pgid}` is absent. That path tests whether the **leader** exists, not the group. On the final KILL the leader has always been reaped already, so any non-zero status, such as an unsupported syntax or EPERM, counts as delivered.

Probe P4:
- Setup: `PATH` points at a `kill` that exits 1. The leader backgrounds `/bin/sleep 300` and exits 0.
- Result: `stop=Exited`, `cleanup_failed=false`, and the child is still **alive**. I killed it by hand.
- This contradicts the module doc ("if signalling the group fails … the failure is reported") and the contract on `run`.

Extra mutants that SURVIVED:
- E1: `Ok(_) => true`, always treating a failed group kill as delivered.
- E2: the `child.kill()` fallback removed.

PROGRESS §2 says this path was deliberately not auto-tested.

Requested:
- Decide "group gone" by group membership, not by the leader's pid: scan `/proc/*/stat` field 5 (pgrp) for `pgid`, which needs no `unsafe`. Alternatively, accept only the "No such process" ESRCH case.
- Make `signal_group`, or the `kill` program name, injectable inside the crate, so a unit test can cover the failing and missing `kill` cases without changing the process `PATH`.

### 3. Low: a failing test can still leak processes (review 1 finding 11 is only partly fixed)

`Tmp::drop` sends SIGKILL to the individual pids in `*.pid` files: the leader and the recorded child. The leader's other descendants are orphaned, for example its foreground `sleep 300`, or the `sleep` inside a `( … ) &` subshell.

Reproduced:
- Mutant R12 left `sleep 300`, with PPID 1, in the leader's process group after the run had finished. A full `mutants.py` run left one more.
- Both were from my runs, identified by cwd, and killed.

`Tmp::drop` also kills the recorded pids **unconditionally**, even when a test passed. After a pass those pids are dead and may have been reused (parallel tests, `pid_max` 4194304). The chance of killing an unrelated process is low but not zero.

Requested:
- Kill the group (`kill -s KILL -- -<leader pid>`). The leader is the group leader because of `process_group(0)`.
- Do it only when `std::thread::panicking()`.

### 4. Info: remaining test gaps (extra mutants)

| Mutant | Result |
|---|---|
| E1 failed group kill counted as delivered | SURVIVED (finding 2) |
| E2 no direct `child.kill()` | SURVIVED (finding 2) |
| E3 `StreamLost` wait not capped at the deadline | SURVIVED (overrun of at most 2 s, minor) |
| E4 stopping line not copied | SURVIVED (finding 1) |
| E5 `window <= GRACE` changed to `<` | SURVIVED (boundary only) |
| E6 zero-room check removed | SURVIVED. Equivalent in practice: `next` with reserve never says `Start` with a zero room unless a cap is 0. |
| E7 drain 20 s | killed |
| E8 cache_creation ignored | killed |
| E9 `StreamLost` without the leader check | killed |
| E10 `incomplete_lines` not counted | killed |

### 5. Info: precision of the limits statements against the fixtures

The required limits are stated honestly:
- observe-then-stop, not a provider limit;
- a lower bound with output at message start and compaction absent;
- final usage only from `result`;
- the first `rate_limit_event` only after the first request;
- the pre-run check separate from in-run enforcement.

The fixture numbers check out:
- r1-new: output 5 in the stream, 612 in `result`;
- n4a: 1,567 observed, 1,878 in `result.usage`;
- l3-compact: no `assistant` lines, and `modelUsage` cache_read is 26,400.

Small precision points:
- The figure called "final" for n4a (1,878) is `result.usage`, the parent only. The collector's run share comes from `modelUsage` (n4a session total 2,584 billable / 20,878 cache_read, a resume delta). The gap to the authoritative value can therefore be larger than the example shows.
- l3-compact: the 26,400 quoted is cache_read only. The 8,035 billable tokens of the compaction are also invisible in-run.
- In l3-compact, the first `rate_limit_event` (line 2) comes with **no** `assistant` line at all, after the compaction request. "After the first request" still holds, but the parenthetical "01-04 stream에서 첫 assistant 다음" does not hold for every stream. The first request can be one the guard never sees.
- The `for_next` room is `cap − spent`, without the reserve that `next` uses. That is reasonable for an in-run room, but its doc does not say so.

### 6. Info: the time limit is met within milliseconds, not exactly

The stop completes within `timeout + grace` plus 50 ms polling, the `kill` spawns and `wait`. That is a few tens of ms past the window. `spent` rounds each record up to whole seconds, so the accounting stays conservative. The module doc says "completes within the limit". "within the limit, up to polling slack" would be exact. Optional.

## Is the collector-backed test a fair proof?

Yes, for the path it claims: the guard stops before any `result`, then `ieo_evidence::stream::parse` (the real collector) gives `unmeasured`, then `budget::next` and `Limits::for_next` refuse. The positive control replays a real 01-04 stream through the guard and the collector, and the run is measured and allowed.

Two caveats:
- The record wrapper (`model_request`, `duration_ms`) is built by the test, not by the adapter.
- Unmeasured is guaranteed only because the guard drops everything after the stop (finding 1). Once finding 1 is fixed, a stop after `result` will rightly give a measured run. The test and PROGRESS should then say "a stop before `result`", which PROGRESS already does conditionally.

## Required before approval

- Finding 1: copy lines after a stop, or state the loss honestly.
- Finding 2: fix group-gone detection and test the fallback.
- Finding 3 is a small test fix, recommended in the same correction.
