# Review 3 · PR #47 (02-04 role supervisor service, PID 1, fake claude) · correction 2

- Head reviewed: `475b409739a2bc8b08d43d98fe4fa62739fc87b3`. Confirmed with `gh pr view 47`: headRefOid = 475b409…, mergeStateStatus BLOCKED. CI at review time: docs validation and toolchain checks passed, `rust fmt clippy test` was in progress. Base: main `d3e28df`. Correction diff: `git diff 6975d5f 475b409` (commits 6fb7c0d and 475b409).
- Reviewer: independent reviewer (Claude). No model calls and no real `claude`; only the fake was used. The secrets directory was not touched. I worked in a separate worktree with my own target dir, and removed the worktree, the target dir and the container workdir afterwards.

## Verdict: APPROVE

Correction 2 resolves N1–N6 and the F6/F7 remainders, or records them honestly. Every new path I probed fails closed: an unreadable ledger, a missing ledger, an attempt without an end line, an unfinished checkpoint, and a SIGTERM wait that runs out. I found **no blocking defect**. The findings below (R1–R6) are Low or Info. They can be recorded as remaining and handled in the next PR, the host-side caller that must exist before any real call. R1 and R2 are the ones worth fixing there.

## What I ran

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test -p ieo-supervisor` (own target dir) | all pass: lib unit 1, run_guard 26, serve 14, supervisor 20 |
| Reviewer probes (scratch test file, deleted afterwards) | Q1–Q4 below |
| Own mutants on serve.rs (19, run against lib + `serve` + `run_guard` tests) | 13 killed. 6 survived: C5 (directory count), C10 (redundant session filter, acceptable), C12 (end line of a stop-before-spawn), C14 (share bounds), C15 (drain in the SIGTERM wait; container only), C19 (end-line lookup) |
| `run-fake.sh` scratch copy (containers `r47c-ct-*`, ports 58267/58268, venv python) | every PR check reproduces: NOT_OBSERVED, cleanup, resume, restart, tamper, ALREADY_RUNNING, stop with 0 processes left, guardrail argv, 0 credential occurrences, ledger 5/5 + 1/1 started/ended, BUDGET_EXCEEDED, SIGTERM during a run (0.6 s, `after_run`), slowest stop (TRAPSETSID: outcome `Requested`, checkpoint `after_run`, exit 0). Containers and workdir removed |

Reviewer probes:
- **Q1**: the checkpoint is at `phase: takeover` (the normal state while a run is live) and another process holds the session lock. `reconnect` returns **409 `EXTERNAL_RECONCILIATION_REQUIRED` "did not finish"**, not `ALREADY_RUNNING` with pid and status (R1).
- **Q2**: the ledger has a start line for `a1`, then a forged end line `{0, 0}`, then the real end line `{999,990, 0}`. A start with room 100,000 gets **202**: the first end line wins (R4).
- **Q3**: a spawn error leaves the checkpoint at `error` and the ledger without an end line. Before reconciliation, `reconnect` gets 409 reconcile and `start` gets 409 `BUDGET_EXCEEDED`. After I appended an end line by hand, `start` of a new session got 202 and ended at `after_run`. `reconnect` stays refused until that new start. No legitimate flow is stuck, but the manual step is not documented (R3).
- **Q4**: `a1` finishes with base 0.01 from `a1`. A resumed `a2` is stopped (`Requested`), and its TERM handler writes an `error_during_execution` result with `modelUsage: {}` and cost 0. The checkpoint base becomes **`{modelUsage: {}, total_cost_usd: 0}` from `a2`**. The known base from `a1` is replaced by the untrusted figure (R2).

`podman stop` of the TRAPSETSID case took 11.0 s of wall time in my run (8.4 s in the PR log). The run itself ended about 8.2 s after SIGTERM: stdout at 00:58:28.6, outcome at 00:58:37.8, stop sent about 1 s after the start. The exit code was 0, so PID 1 was not SIGKILLed. The extra time is podman teardown on a loaded disk (root fs 95% full). See R5.

## Findings from earlier reviews

| # | Status | Evidence |
| --- | --- | --- |
| N1 grace incompatible with `for_next` | **Resolved** | There is one constant: `run::GRACE` = 6 s, and `MAX_GRACE = run::GRACE`. `limits_from_the_budget_path_are_accepted` shows `for_next` gives timeout + grace = 1,800 s and gets 202. 6,001 ms gives 400. The only other user of `GRACE` is `for_next` (`window <= GRACE`). PROGRESS §2 records the 10 → 6 s change against 진행 기록 2. |
| N2 backstop above the approval, fail-open ledger | **Resolved / recorded** | Per-role `share` is mandatory in config. Each field must be in 1..=cap. The run count, the granted time and the cumulative observed tokens + room are checked against the share (C7, C16–C18 killed). The ledger reads fail-closed: a bad line, a missing ledger while attempt dirs exist, a non-numeric window counted as a full window, and an attempt without an end line (C3, C4, C6, C8, C13 killed). The cross-role sum is documented as the config author's job. The real-call precondition is recorded in §1 and §3: no real credential until the caller exists and passes an end-to-end test, and no manual `curl` with a real token. Same-UID forgery is recorded as a limitation. Untested: the directory count in `max(starts, dirs)` (C5) and the share bounds (C14), see R6. |
| N3 resume base lost | **Resolved, with a gap** | Carry-forward on a resume with no result of its own, plus `session_cumulative_attempt`. Tested (C9 killed). Gap: a stopped run's own late `result` still overrides the carried base (Q4, R2). |
| N4 takeover checkpoint resumed silently | **Resolved** | `reconnect` refuses `phase` `takeover` or `error` with `EXTERNAL_RECONCILIATION_REQUIRED` (C1 and C2 killed). Side effect: R1. |
| N5 SIGTERM wait ends before the outcome | **Resolved** | Grace 6 s; the wait is grace + DRAIN + 1 s = 9 s. The TRAPSETSID case is in run-fake.sh and reproduces (outcome `Requested`, `after_run`). The required stop timeout of at least 10 s is documented. Margin, see R5. C15 survives in unit tests; this is acceptable because the container run covers it. |
| N6 untested claims | **Resolved** | Unit test `a_stop_before_the_spawn_starts_nothing`: nothing runs, no takeover checkpoint, outcome written (C11 killed). `a_run_error_leaves_an_error_checkpoint_and_stops_further_starts` covers the `error` phase and the backstop refusal. Not covered: the `end` line of a stop-before-spawn (C12), see R6. |
| F6 remainder (extension codes) | **Recorded** | PROGRESS §1 lists `ALREADY_RUNNING`, `CREDENTIAL_UNAVAILABLE`, `BUDGET_EXCEEDED`, `EXTERNAL_RECONCILIATION_REQUIRED`, `NOT_OBSERVED`, `STALE_GENERATION`, `SCHEMA_VIOLATION` and `INTERNAL` as extensions for the contract revision, plus the missing §7 status fields. |
| F7 remainder (stop before spawn, timing) | **Resolved** | Stop-before-spawn is tested, and the takeover checkpoint is now written after the stop check. The SIGTERM timing is as in N5. |

## New findings (none blocking)

### R1. [Low, not blocking] The phase rule hides the §5-5 "already running" answer for a live holder
- **Location**: `serve.rs` `launch`: the `takeover`/`error` refusal comes before the lock probe and `checkpoint::decide`.
- **Scenario**: while a run is live, its checkpoint is always at `takeover`. If a process still holds the session lock (the case `a_session_held_by_another_process_is_reported_not_restarted` describes as "a process left from before a supervisor restart"), `reconnect` answers 409 "did not finish" without the pid or status (Q1). The existing test passes only because its checkpoint is at `after_run`. This is safe, because nothing starts. But §5-5 says to answer with the status, and the coordinator cannot tell "a process is still alive" from "a process died".
- **Fix (next PR)**: run the lock probe before the phase rule, so that a live holder yields `ALREADY_RUNNING` with pid and status. Add the probe case to the test.

### R2. [Low, not blocking] A guard-stopped run's late `result` replaces the carried resume base
- **Location**: `write_checkpoint`: `own = session_cumulative(stdout)` is preferred whenever a `result` line exists.
- **Scenario**: run.rs and 진행 기록 2 say a stopped run's `result` is never trusted (the CLI writes zero-usage `error_during_execution`). Q4 shows that this figure becomes the base, attributed to the stopped attempt. The caller's check "the base comes from the run actually resumed" then passes on a wrong base. The effect is conservative: a lower base over-charges the next resumed run, and `budget::next` already blocks after an unmeasured run. Still, it undoes N3's intent in exactly the stopped case.
- **Fix (next PR)**: use `own` only when `outcome.stop == Exited`. Otherwise carry the previous base forward, or record `session_cumulative_trusted: false`.

### R3. [Low, not blocking] Reconciliation after an error or an interrupted run is not described
- **Location**: PROGRESS §1 ("사람이 대조할 때까지 거부한다"), `budget_backstop`, the `reconnect` phase rule.
- **Scenario**: any run error, including a spawn failure with no model call, or a PID 1 death mid-run, blocks every further start in the role until someone edits `/state/budget/attempts.jsonl`. `reconnect` stays refused until a new session is started (Q3). This is correct fail-closed behaviour, but the only way out is an undocumented hand edit of a role volume, from the host with `podman unshare`.
- **Fix (next PR / caller)**: document the procedure: which end line to append, with figures taken from the 02-01 record, and that only `start` (a new session) follows. Better, let the caller do it through the evidence record.

### R4. [Info] The end-line lookup takes the first match
- `budget_backstop` uses `.find(...)` on end lines, so a line the run itself appended before the supervisor's line wins (Q2, C19 survives). This falls under the recorded limitation (same UID, "줄을 정교하게 고치는 것은 막지 못한다"). Taking the last end line, or the maximum, costs nothing and removes the cheapest forgery. Optional.

### R5. [Info] The SIGTERM margin is thin
- The stop path takes up to about 0.2 s (serve poll) + 0.2 s (watch poll) + 6 s grace + 2 s drain + the end-line and checkpoint fsyncs, against the service's own 9 s cut-off. The measured run needed about 8.2 s, leaving a margin of about 0.8 s. When the cut-off is missed, the result fails closed: the checkpoint stays at `takeover`, there is no end line, and the role needs reconciliation. `podman stop` wall time can exceed 10 s (11.0 s here) without a SIGKILL. Recommend `--stop-timeout`/`terminationGracePeriodSeconds` of about 15 s for real role containers, and note it where 10 s is given as the minimum.

### R6. [Low, not blocking] Claimed behaviours without a test
- C5: the run count via `/state/runs` directories (`max(starts, dirs)`), for example a ledger edited to fewer start lines while directories remain.
- C12: a stop-before-spawn writes an `end` line with 0/0. Without it the role would block for reconciliation. The unit test calls `execute_inner`, not `execute`.
- C14: config refusal of a share above the budget cap, or of 0.

## Info
- The ledger split is consistent. Start lines are those without `end` and carry the window. End lines with an object `end` carry tokens. A malformed `end` (non-object, non-null) is neither, which is safe.
- `ledger()` treats an unreadable `runs/` as 0 directories. The ledger itself still must parse, so this is fine.
- `env::apply` can fail after the takeover checkpoint (`?`, no `error` phase). The checkpoint then stays at `takeover`, which is refused just the same. This is consistent.
- The carry-forward session filter is redundant: a resume always uses the checkpoint's session. C10 survives, which is acceptable.
- Evidence `cargo-test.log` was taken at 6fb7c0d (252 tests). 475b409 changes only docs/evidence.
