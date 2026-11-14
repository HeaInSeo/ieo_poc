# Review 3 of PR #40: 02-04 run guard (correction 2)

- Head reviewed: `5136d66c6cba9115efdbdd893b6dbbf6ed8ce785` (confirmed with `gh pr view 40 --json headRefOid`), base `f582403`. Correction diff checked: `git diff d2989f7 5136d66`.
- **Verdict: CHANGES REQUESTED** (1 Medium, 3 Low, 2 Info). The required change is small: mostly docs, plus one rule the adapter must follow.

## What I ran

All work was done in a scratch `git worktree` at the head, with its own `CARGO_TARGET_DIR`. Both were removed afterwards.

- `cargo clippy -p ieo-supervisor --all-targets -- -D warnings`: clean.
- `cargo test -p ieo-supervisor`: 20 (`run_guard`) + 20 pass.
- The `run_guard` binary ran 30 times in sequence, then 6 x 10 in parallel (90 runs): 0 failures. No `sleep 300`, fake processes or `/tmp/ieo-run-*` directories were left behind.
- PR `mutants.py`, run from a scratch copy with the `.venv-poc` python: 23/23 killed, with the same tests named as in `mutants.txt`. `run.rs` and the tests are identical between the evidence commits (`f777399`, `3b6f483`) and the head.
- My extra mutants: results under finding 4.
- Probes: a throwaway test file, not committed. Results are cited in the findings.

## Status of the d2989f7 findings

| # | Status | Notes |
|---|---|---|
| 1 Medium: lines after a stop are dropped | Resolved | `Copier` copies everything queued during the grace, plus up to 2 s after the final KILL. Probes: a `result` behind the stopping line is kept; a stop inside `drained` keeps the `result`; 800,001 lines written during the grace are all copied, including the final `result`. Killed by R22. This fix has a side effect, see finding 1. |
| 2 Medium: a failed `kill` counted as success | Resolved | `group_alive` scans `/proc/*/stat` for pgrp, skipping zombies, and parses safely after the last `)`. `run_with_kill` makes the program injectable. The broken-`kill` test covers both the reported survivor and the direct kill of the leader. R20 and R21 are killed. |
| 3 Low: failing tests leak processes | Resolved | `Tmp::drop` kills the leader's group, only while panicking. |
| 4 Info: test gaps | Resolved as stated | E1, E2, E4 and E5 are now R20 to R24. E3 and E6 were left, with reasons given. |
| 5, 6 Info: precision of the docs | Resolved | n4a now quotes `modelUsage` 2,584; l3-compact quotes both 8,035 and 26,400; "tens of ms" is stated. |

The 3103933 findings did not regress. Mutants R1 to R19 are still killed, the drain is still bounded, the closed-stream stop still works, and evidence files are still created before spawn.

## Findings

### 1. Medium: a stopped run can now be measured below what the guard itself observed, and the next call is then allowed. The docs still say a stopped run is unmeasured.

Copying lines after a stop means a `result` written on the way out reaches the collector. `ieo-evidence::stream::parse` accepts any `result`, whatever its `subtype` or `is_error`. In a new session with an empty `modelUsage`, it measures `result.usage` plus the children.

The real CLI does write such results. 01-04 attempt 05 shows `subtype: "error_during_execution"`, `is_error: true`, zero `usage` and an empty `modelUsage`. Whether the CLI writes one on SIGTERM in the middle of a request is not established either way.

Probe P1: the fake emits an `assistant` line with 5,001 billable tokens, then hits the time limit. Its TERM handler writes an `error_during_execution` result with zero usage.

| Step | Result |
|---|---|
| guard | `stop=RunTimeout`, `observed.billable=5001` |
| collector | `state: measured`, tokens 0 |
| `budget::spent` | billable 0, `unmeasured_runs: []` |
| `budget::next(BUDGET_0204)` | **`Start`** |

Before correction 2, the same run was unmeasured and the next call was blocked. The conservative property "a guard stop leaves the run unmeasured, so the next call is blocked" now holds only for a stop before any `result`.

The docs still state the property without that condition:
- PROGRESS §2, second bullet: "guard가 회차를 멈추면 `result`가 없고, 그 회차는 미계측이 되어 …". This is now false in general, and it contradicts the new last bullet of §2.
- The module doc says "A run stopped before its `result` is unmeasured". That is correct but incomplete: it does not say that a `result` written after the stop is trusted as final.

This matters for budgeted calls. The tokens of the request that the stop interrupted were spent, and the guard's lower bound shows it. A `result` written on the way out may not include them, and the budget then counts less than the guard observed.

Requested, the minimum for the last correction:
- Fix PROGRESS §2 bullet 2 and §1 row 4, and the module doc. State that a `result` written after a stop is copied and measured by the collector, and that it can be lower than the observed lower bound when the CLI reports an interrupted request.
- State a rule for the adapter step: a run with `Outcome.stop != Exited` must not count less than `Outcome.observed`. For example, take the larger of the two per field, or record the run as unmeasured unless the `result` has `subtype: "success"`.
- Better: implement that rule now as a small, tested helper in `run.rs`, and add P1 as a test. That pins the gap.

I asked for this copy behaviour in review 2 without spelling out this consequence. The fix itself is right, and only the trust rule is missing.

### 2. Low: stale or incomplete PROGRESS statements

- §2: "이 경로는 PATH를 바꾸는 시험이 다른 시험에 영향을 주므로 자동 시험하지 않았다". This path is now tested through `run_with_kill`, by `a_failed_group_kill_is_reported_and_the_leader_is_still_stopped`.
- The §1 table does not list the new tests: `a_failed_group_kill_…` belongs under cleanup, and `output_written_on_the_way_out_is_kept` and `the_stop_grace_must_fit_…` are not cited anywhere.
- §1 row 5 says "stdout이 그대로 복사된다". With lossy UTF-8 decoding and the `\r` trim, the copy is not byte-for-byte (finding 6).

### 3. Low: an error return loses the cleanup report

The contract on `run` says that when cleanup cannot be confirmed, `Outcome::cleanup_failed` is set. On every `Err` path (`watched?`, `copy_error`, `out.flush()?`) no `Outcome` is returned, so the caller cannot learn that cleanup failed.

Probe P4:
- With a broken `kill`, `stdout_path = /dev/full` and a background `sleep 300`, the result is `Err(StorageFull)` and the child is **alive**. Nothing reports it.
- With the real `kill` (P4b), the result is `Err` and the child is gone. The normal case is fine.

This needs two failures at once, so it is Low. Fix: put the cleanup state in the error message, for example with `io::Error::other(format!("…; cleanup_failed"))`, or return the `Outcome` together with the error.

### 4. Low: the 2 s bound on the post-stop copy is untested. Without it, a descendant could hang the guard.

Extra mutants against `run_guard` at the head:

| Mutant | Result |
|---|---|
| X8: `Copier::pump` waits for `End` with no bound | **SURVIVED**. A descendant that left the group (`setsid`) and holds stdout would hang `run` forever. Probe P3 at the head: bounded, with `elapsed` 2.58 s for a 0.5 s limit. |
| X9: copy window of 50 ms instead of 2 s | SURVIVED (the timing margin is not pinned; minor) |
| X1: `group_alive` counts zombies | SURVIVED (equivalent in practice) |
| X3: unreadable `/proc` counts as "group gone" (fail open) | SURVIVED |
| X4: a TERM failure is not flagged | SURVIVED (the KILL failure flags it anyway) |
| X6: copy errors after a stop are ignored | SURVIVED |
| X10: copy also after an `Err` | SURVIVED (harmless) |
| X12: `kill` exit status ignored, `/proc` scan only | SURVIVED (equivalent) |
| X2: ppid instead of pgrp | killed |
| X5: final KILL failure not flagged | killed |
| X7: the copier drops lines | killed |
| X11: a stop inside `drained` not taken | killed |

Requested: add a test in which a `setsid` descendant keeps stdout open after a stop and `elapsed` stays below `timeout + grace + DRAIN + margin`, then add X8 to `mutants.py`. The others are optional.

### 5. Info: a descendant that left the group is not reported

P3: the `setsid` child survives and `cleanup_failed=false`. This is documented as a limit ("best effort", adapter step). I note it only because the field name might suggest more than it covers.

### 6. Info: minor accuracy points

- The module doc says "faithful copy". Lines are decoded lossily, a trailing `\r` is removed, and a final line without a newline gets one. Faithful at the line level, not byte-for-byte.
- On the `Exited` path, the final group KILL targets a pgid whose leader has already been reaped. If the group is empty, the pid could in theory be reused as a new group leader. This is negligible (`pid_max` 4,194,304, a window of microseconds).

## Are the §3 claims proven?

| §3 requirement | Proven by |
|---|---|
| time limits with child cleanup | R1, R2, R7, R10, R11 and R19 to R21 are killed; the broken-`kill` path is now tested |
| stop on observed usage at the cap | R4, R5 and R6, including the boundary |
| stop on extra-usage or limit signals | R3 |
| block the next call when measurement fails | yes for a stop **before** `result`, through the real collector; not for a stop followed by an error `result` (finding 1) |
| positive control | a real 01-04 stream is measured and allowed |

## Required before approval

- Finding 1: the docs correction and the adapter trust rule. A tested helper and the P1 test are preferred.
- Finding 2: refresh the stale PROGRESS statements.
- Findings 3 and 4 are recommended in the same correction: an error message carrying the cleanup state, and a `setsid` pipe-holder test with mutant X8.
