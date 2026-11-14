# Review 2 · PR #47 (02-04 role supervisor service, PID 1, fake claude) · correction 1

- Head reviewed: `6975d5f638a7d483d6c67381fe890d15518de9c5`. Confirmed with `gh pr view 47`: headRefOid = 6975d5f…, mergeStateStatus BLOCKED. CI at review time: docs validation and toolchain checks passed, `rust fmt clippy test` was in progress. Base: main `d3e28df`. Correction diff: `git diff 88a4535 6975d5f` (commits cdc0f83 and 6975d5f).
- Reviewer: independent reviewer (Claude). No model calls and no real `claude`; only the fake was used. The secrets directory was not touched. I worked in a separate worktree with my own target dir, and removed both afterwards.

## Verdict: CHANGES REQUESTED

Correction 1 resolves F1, F2, F3 (as far as it was recorded), F5, F8 and F9, and resolves or honestly records most of F6 and F7. The new code is careful, and the reconnect refusals hold up under mutation. Two problems remain before merge:

- **N1**: the service rejects the limits that the documented caller path produces. `Limits::for_next` always sets `grace` to 10 s, but the new `MAX_GRACE` is 8 s, so every caller-computed request gets 400.
- **N2**: the F4 backstop is per role, so the two roles together may start twice the approved 02-04 budget. It also has no cumulative token cap, and its ledger fails open. That is acceptable only if the limitation and the gate are written down as a hard precondition for real calls, and the ledger fails closed.

N3–N6 are Low. Fix them or record them.

## What I ran

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test -p ieo-supervisor` (own target dir) | all pass: run_guard 26, serve 11, supervisor 20 |
| Reviewer probes (scratch test file, not committed; copy at `scratchpad/r47b-probes.rs`) | P1–P4 below |
| Own mutants on serve.rs (20) | 15 killed, 5 survived: M5/M19 (redundant pair, acceptable), M8 stop-before-spawn, M9 error checkpoint, M10/M11 fsync (acceptable) |
| `run-fake.sh` scratch copy (containers `r47b-ct-*`, ports 58257/58258, venv python) plus 2 extra SIGTERM cases | every PR check reproduces (NOT_OBSERVED, cleanup, resume, restart, tamper, ALREADY_RUNNING, stop, guardrail argv, 0 credential occurrences, BUDGET_EXCEEDED, SIGTERM during run 0.6 s → `after_run`). Extra cases below. Containers and workdir were removed |

Reviewer probes:
- **P1**: `Limits::for_next(&BUDGET_0204, &[], fresh observation)` returns `grace = 10s`, `timeout = 1790s`. Sending those limits to `start` gives **400 `SCHEMA_VIOLATION` "limits.grace_ms above 8000 ms"** (N1).
- **P2**: a ledger with 12 unparseable lines lets `start` through with 202. So do 11 lines whose `window_ms` is a string, and a deleted ledger also gives 202 (N2).
- **P3**: after `a1`, `session_cumulative.total_cost_usd` is 0.01. A resumed `a2` is stopped before any `result` event. During `a2` (`takeover`) the value is `null`, and after `a2` (`after_run`) it is still `null`. The resume base is lost (N3).
- **P4**: I snapshot the `takeover` checkpoint mid-run, as if PID 1 died there. A new `Service` then answers `reconnect` with **202 mode `resume`** of the interrupted session. No interrupted or unknown flag is set, and `active_children` / `pending_external_requests` read `[]` (N4).

Extra container cases (`podman stop -t 10`, `grace_ms` 8000):
- `TRAP` (the leader ignores TERM): stop took 8.5 s, exit 0, outcome `Requested`, elapsed 9,830 ms, checkpoint `after_run`. This is OK.
- `TRAPSETSID` (the leader ignores TERM, and a `setsid` descendant keeps stdout open): stop took 9.3 s, exit 0, **outcome.json missing, checkpoint left at `takeover`**. PID 1 exits after its MAX_GRACE + 1 s wait, while the worker is still in the 2 s DRAIN (N5).

## Findings from review 1

| # | Status | Evidence |
| --- | --- | --- |
| F1 reconnect without a usable checkpoint | **Resolved** | absent → 409 `NOT_OBSERVED`; read error or corrupt → 409 `EXTERNAL_RECONCILIATION_REQUIRED`. Tested (`a_reconnect_needs_a_readable_checkpoint`), mutants M1 and M2 killed, container log shows `409 NOT_OBSERVED`. A checkpoint without `session_id` goes to `decide` → reconcile. |
| F2 checkpoint session_id unvalidated | **Resolved** | `safe_id` now requires a first alphanumeric character, and the checkpoint sid is checked **before** the lock probe. Tested with `--dangerously-skip-permissions`, `../x`, `""`, `a/b`. M3 and M4 killed. |
| F3 checkpoint timing (§5-1) | **Resolved / recorded** | A `takeover` checkpoint is written after lock and cleanup and before spawn; `error` and `after_run` follow. Empty children/external requests and `worktree_head: none` are recorded in §1 as limits. Tested (M7 killed). The `error` path is untested (M9). Errors before takeover (lock, cleanup) still write neither a checkpoint nor outcome.json; `status.last` carries the error. That is acceptable. New semantics issues: N3, N4. |
| F4 no budget ceiling | **Partially resolved** | The backstop (run count, per-run window, total granted window, per-run token rooms) and an fsynced ledger are added and tested (M12–M18, M20 killed). Remaining: N1 (incompatible with `for_next`) and N2 (per role, so 2× combined; no cumulative tokens; fail-open ledger). |
| F5 test gaps | **Resolved** (S9 accepted) | Lock-probe path test with `pid` and `status` (M6 killed). The in-memory 409 now returns `status`, and the test asserts `status.running.attempt_id`. Id traversal and leading `-` are tested. The empty credential is tested. A strict-`kill` shim test covers the `-SIG -- -PGID` form. Reap-while-idle is still untested; that is acceptable and now private. |
| F6 §7 shape | **Mostly resolved / recorded** | `UNAUTHORIZED` and `ID_CONTENT_CONFLICT` are renamed. Missing status fields, `restored_checkpoint_ref`, `capability_gaps` and the safe-shutdown report are listed in PROGRESS §1. Not recorded: `ALREADY_RUNNING`, `CREDENTIAL_UNAVAILABLE`, `BUDGET_EXCEEDED` and `EXTERNAL_RECONCILIATION_REQUIRED` are used as codes but are not among the six §7 codes, and §7 makes `external_reconciliation_required` a reconnect field. Add one line to the list. |
| F7 SIGTERM and stop timing | **Mostly resolved** | `grace_ms` ≤ 8 s (400 otherwise, M16 killed). The serve wait is MAX_GRACE + 1 s. Stop is checked before spawn (`spawned: false`), but this is untested (M8 survives; the test was moved to stop after spawn). SIGTERM during a run is in run-fake.sh and its log. The worst case still overruns: N5. |
| F8 fsync | **Resolved** | `sync_all` on the tmp file, then rename, then directory `sync_all`. The ledger is also `sync_all`ed. Untested (M10/M11), which is acceptable. |
| F9 info items | **Resolved** | `reap_orphans` is private with a PID-1-only note. The unused `nix` `signal` feature is dropped. |

## New findings

### N1. [Medium] The service refuses every limit computed by the documented caller path
- **Location**: `serve.rs` `MAX_GRACE = 8 s` and the `limits.grace > MAX_GRACE` check; `run.rs` `pub const GRACE: Duration = 10 s`, used by `Limits::for_next` (`grace: GRACE`).
- **Scenario**: PROGRESS §1 says "호출자가 회차마다 한도(`Limits::for_next`)를 넘기고". `for_next` always returns `grace = 10 s`, and `start`/`reconnect` answer 400 `limits.grace_ms above 8000 ms` (probe P1). The first real caller either fails, or someone hand-edits the grace, which bypasses `for_next`, the budget-checked path.
- **Fix**: use one constant. Set `run::GRACE` to 8 s (and keep `MAX_GRACE = run::GRACE`), or make `for_next` produce a grace ≤ `MAX_GRACE`. Add a test that `for_next`'s output is accepted by `Service::handle` (202) and that `timeout + grace` equals the per-run window the backstop allows.

### N2. [Medium, gate before real calls] The backstop does not bound the approved 02-04 budget, and its ledger fails open
- **Location**: `serve.rs` `budget_backstop`, `attempts`, `record_attempt`; PROGRESS §1 "한계: 이 기록은 역할마다 따로다".
- **Scenario**:
  - Each role checks the **full** 02-04 caps against its own ledger. Development and Guardrail together can start 24 runs and receive 28,800 s of granted windows, twice the approved 12 runs and 14,400 s.
  - Tokens are checked only per run (`room ≤ 1,500,000`). There is no cumulative sum, so one role alone may be granted up to 12 × 1.5 M billable. PROGRESS says "토큰 여유가 예산 상한을 넘는다", which a reader can take for a cumulative cap.
  - `attempts()` silently drops unparseable lines, and `window_ms` values that are not numbers are ignored (probe P2). A deleted ledger resets the count. The development role's `claude` runs with the same UID and a writable `/state` (§2.1), so it can delete or corrupt `/state/budget/attempts.jsonl`. This matters only together with a caller that over-requests, but that is exactly the case a backstop exists for.
  - The caller that enforces the combined total does not exist yet. Nothing besides PROGRESS §3 says real-credential containers must not be started before it exists. The PR's own `run-fake.sh` shows the pattern of a manual `curl` to the service.
- **Fix** (any one of these, plus the ledger change):
  1. Give each role a **share** in `/etc/ieo/role` (for example `max_runs`, `total_seconds` and cumulative token caps per role, where the sum over roles ≤ the approved budget). The service checks its share. Then the backstop alone never exceeds the approval.
  2. Write a recorded gate in PROGRESS §3 and in the user-decision trail: no real credential is mounted in a role container until the budget-enforcing caller exists and passes an end-to-end test, and no script may `curl` start/reconnect with a real token. Also correct the wording: "per role, ≤ 2× combined; per-run token room only".
  
  In both cases, make the ledger fail closed. Count every non-empty line. Treat an unparseable line or a missing `window_ms` as a full per-run window. Treat a missing ledger as an error when the attempt-id directories under `/state/runs` exist (or count those directories as a second source).

### N3. [Low] Takeover and stopped-run checkpoints drop the resume base `session_cumulative`
- **Location**: `serve.rs` `write_checkpoint(..., None, "takeover")` → `session_cumulative: Null`. The `after_run` value comes from the run's own stdout only.
- **Scenario**: start `s1` (cumulative 0.01). A resumed run is stopped, crashes, or hits a limit before emitting `result`. The checkpoint's `session_cumulative` becomes `null` and the previous base is gone (probe P3). Per §6 / §11 item 7, the next resumed run's share is then unmeasured, even though the base was known. This is safe, because it fails as unmeasured, not as zero, but it wastes a run of the 12-run budget.
- **Fix**: on a resume, carry the previous checkpoint's `session_cumulative` forward when the current stdout has no `result`. Record which run it belongs to (for example a `session_cumulative_attempt`), so the caller can check "실제로 재개한 직전 회차".

### N4. [Low, record before real calls] A `takeover`-phase checkpoint is resumed like a clean one
- **Location**: `checkpoint::decide` ignores `phase`. The takeover checkpoint writes `active_children: []` and `pending_external_requests: []`.
- **Scenario**: PID 1 dies mid-run (OOM, node loss, or the N5 overrun). On `reconnect`, `decide` sees an unchanged digest and empty lists, and resumes with no flag (probe P4). §6 says to compare active children and unconfirmed external requests before resuming. Here the lists are not "empty", they are **unobserved**, and writing `[]` asserts zero (the P35 principle: unmeasured is not zero). PROGRESS records that the lists are always empty, but not that an interrupted run resumes without reconciliation.
- **Fix**: on reconnect, when `phase == "takeover"`, return `EXTERNAL_RECONCILIATION_REQUIRED` (reason "previous run interrupted"), or at least set an `interrupted: true` field in the 202 answer and in the status. Alternatively, write `null` for the lists at takeover, so that `decide` reconciles. Add a test.

### N5. [Low] The SIGTERM wait can still end before the run's outcome and checkpoint are written
- **Location**: `serve()` waits `MAX_GRACE + 1 s` = 9 s. `run_with` can take up to about 0.2 s (stop poll) + grace (8 s) + `DRAIN` (2 s) + the checkpoint write.
- **Scenario**: reproduced in the container with `grace_ms` 8000, a TERM-ignoring leader and a `setsid` descendant holding stdout. `podman stop` took 9.3 s, PID 1 exited 0, **outcome.json was missing, and the checkpoint stayed at `takeover`**. PROGRESS §1 says "컨테이너 기본 정지 제한 10초 안에서 정지가 끝나도록", which does not hold in this case. Even waiting longer would exceed podman's 10 s. The consequences are bounded: the takeover checkpoint is consistent, and a missing outcome is unmeasured. Combined with N4, though, the next reconnect resumes silently.
- **Fix**: set `MAX_GRACE` to `stop timeout − DRAIN − margin` (for example 6 s with a 10 s stop), or shorten the drain when a stop is requested. Document the required `--stop-timeout` / `terminationGracePeriodSeconds`. Optionally add the `TRAPSETSID` case to run-fake.sh.

### N6. [Low] Claimed behaviours without a test
- `spawned: false` (stop before spawn): M8 survives. The test was changed to stop only after the fake started, so the PROGRESS line "시작 직전에 정지 요청이 있으면 아무것도 띄우지 않는다" is unverified. A test can set the stop flag between `launch` and the worker. For example, fill the template dir so that cleanup is slow, or add a test hook. Alternatively, mark the claim as untested.
- The `error` checkpoint (M9): a missing `claude` binary makes `run_with` fail at spawn. Test that the checkpoint phase is `error` and that `status.last.error` is set.

## Info
- Races: `launch` runs on the single HTTP thread. The early `is_running` check and the re-check under the lock are redundant with each other (M5 and M19 each survive alone), which is fine. A `reconnect` during the starting window (running set, lock not yet held by the worker) gets 409 with status. I found no deadlock: `status()` is called only after the state guard is dropped.
- The lock probe now runs only for a `safe_id` session, inside `locks/`.
- The ledger is written before the worker starts. Attempts that later fail at lock, cleanup or spawn count as used, which is conservative and matches "실패·정지 회차도 센다".
- The granted window does not include `DRAIN`, so actual elapsed time can exceed the window by about 2 s. The caller charges measured elapsed time, so this is Info only.
- The `budget` config key is now mandatory; an unknown value refuses start-up. Good.
