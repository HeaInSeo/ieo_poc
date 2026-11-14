# Review 1 · PR #47 (02-04 role supervisor service, PID 1, fake claude)

- Head reviewed: `88a45350466cedcca5e637adf14f0a62e164c539` (confirmed with `gh pr view 47`: headRefOid = 88a4535…, mergeStateStatus BLOCKED, CI 3/3 pass). Base: main `d3e28df`.
- Reviewer: independent reviewer (Claude). No model calls, no real `claude`, secrets directory not touched. Fake claude only.

## Verdict: CHANGES REQUESTED

The run-guard changes (stdin writer, stop flag, kill form) are sound and cause no regressions. Most of the serve.rs mapping is right, and the container evidence reproduces. Three contract gaps should be fixed, or explicitly recorded as remaining, before merge: F1, F2 and F3. F4 must be a gate before any real call.

## What I ran

| Check | Result |
| --- | --- |
| `cargo test -p ieo-supervisor` (own target dir) | all pass (run_guard 25, serve 6, supervisor 20, unit) |
| `cargo clippy -p ieo-supervisor --all-targets -- -D warnings` | clean |
| `cargo tree --locked --offline -p ieo-supervisor` | lock consistent; new: nix 0.29.0, signal-hook 0.3.18 (+ registry, errno, cfg_aliases); tiny_http 0.12.0 already in the lock (ieo-coord, ieo-notion) |
| run-guard mutants.py (unchanged patterns all still match; run with python3.11 because the host python3 is 3.6, which has no `capture_output`) | 27/27 killed, tree clean |
| Own mutants on serve.rs/run.rs (19) | 12 killed, 6 survived (S3, S7, S8, S9, S14, S15, S18; S8 accepted), 1 pattern miss (S13, covered by S5) |
| Kill form in `golang:1.26.9-bookworm` (procps-ng) | `kill -s TERM -- -PGID` rc=1 (prints usage); `kill -TERM -- -PGID` rc=0; absent group rc=1 |
| Kill form on host util-linux 2.32.1 | `kill -TERM -- -PGID` rc=0; absent group rc=1. Ubuntu CI passes the run_guard group-kill tests with the new form |
| `run-fake.sh` (scratch copy, containers `r47-ct-*`, ports 58247/58248, plus extra cases) | all original checks reproduce; extra results below; containers and workdir removed |

Extra container cases (scratch copy only):
- SIGTERM to PID 1 **while a run is active**: `podman stop` took 0.4 s with exit 0. The outcome was `Requested`, `cleanup_failed:false`, and the checkpoint was written (`session_cumulative` null, not 0). This works, but the PR's own evidence does not show it.
- Checkpoint overwritten with `not json`, then `reconnect`: **202, mode `new`, `tamper:null`**, new session started (F1).
- Checkpoint `session_id` set to `../../tmp/r47evil`, then `reconnect`: **202, mode `resume`, session `../../tmp/r47evil`**, and the file `/tmp/r47evil.lock` was created outside `/state/locks` (F2).
- A 2 MiB body was truncated at 1 MiB and refused with 400 (bounded).

## Findings

### 1. [Medium] `reconnect` with a missing or unreadable checkpoint silently starts a new session (§5-4)
`launch` reads the checkpoint with `.ok().and_then(from_slice).ok()`, so "absent", "unreadable" and "corrupt JSON" all become `None`. `checkpoint::decide(None, …)` returns `NewSession`, and `reconnect` then starts `new_session_id` with `tamper:null` and no flag (reproduced in the container above). §5-4 says a failed or unobservable resume is reported as `NOT_OBSERVED` and is **not silently replaced by a new session**. §5-6's "first boot without a checkpoint starts new" is what `start` does. On `reconnect` there is supposed to be a previous session. Development code can corrupt `/state/checkpoint.json` (§2.1, N5), so this is a reachable path. Fix: on `reconnect`, map an absent checkpoint to `NOT_OBSERVED`, and an unreadable or invalid one to `EXTERNAL_RECONCILIATION_REQUIRED`. Add a test for each.

### 2. [Medium] The checkpoint `session_id` is not validated before it is used as a path and as an argv value
Request ids go through `safe_id`, but the session id read from the checkpoint does not:
- the `reconnect` lock probe and `execute_inner` both call `SessionLock::acquire(locks, sid, …)`, which opens `locks/<sid>.lock` with create+truncate. With `../../tmp/r47evil` this created `/tmp/r47evil.lock` (shown above). The probe runs **before** `decide`, so it creates or truncates the file even when the decision is a refusal;
- the same value is passed as `--resume <sid>`. A value beginning with `-` (for example `--settings=/state/x.json` or `--dangerously-skip-permissions`) would be parsed by the CLI as an option, not as a session id. Note that `safe_id` also allows a leading `-` for request ids.

Within the development container this does not cross the §2.1 boundary (same UID). It is still the supervisor building argv and paths from tamperable data. Fix: validate the checkpoint session id (`safe_id` and no leading `-`, or the UUID form the CLI uses) before the probe. If it fails, return `EXTERNAL_RECONCILIATION_REQUIRED`. Also forbid a leading `-` in `safe_id`, and add a test.

### 3. [Medium] Checkpoint timing does not meet §5-1, and the gap is not listed as remaining
The checkpoint is written only after `run_with` returns `Ok`. §5-1 requires a checkpoint at takeover/first action, before termination or a limit, and on an exception. Consequences:
- a `start` (new session) that crashes or is killed mid-run leaves the **previous** session's checkpoint, so a later `reconnect` may resume the old session, and the new session has no record;
- the `execute_inner` error paths (lock, cleanup, `run` error) write no checkpoint and no `outcome.json`;
- `active_children` / `pending_external_requests` are always `[]`. `worktree_head` / `diff_sha256` are `"none"` (carried over from PR #31 finding 8).

PROGRESS §1 says "실행 뒤 §5-1 체크포인트를 원자적으로 쓴다" and lists nothing of this in §3. Either write an in-flight checkpoint (session id, mode, attempt) before spawning, or record these gaps explicitly in "아직 하지 않은 것".

### 4. [Medium, gate before real calls] The service enforces no budget ceiling and no overage precondition
`limits_from` accepts any `timeout_ms`, `billable_room` and `cache_read_room` from the caller, and `launch` calls neither `budget::next` nor `observation_check`. With fake claude this costs nothing. Once a real token is mounted, though, any holder of the adapter token (or a manual `curl` like `run-fake.sh`) can start runs outside the 02-04 budget (12 runs) with arbitrary rooms. That is a budget bypass in the user's terms. PROGRESS §3 lists the caller as remaining, which is honest. Before the first real call, do at least one of these: (a) a configured hard ceiling in `/etc/ieo/role` that the service clamps limits to, and a run counter in `/state`; or (b) a recorded rule, with a check, that real-credential containers are started only by the budget-enforcing executor. Do not merge real-call scripts that `curl` the service directly.

### 5. [Low] Tests do not exercise several claims (mutants S3, S7, S9, S14, S15, S18 survive)
- **S3/S14**: removing the lock probe, or the session lock in `execute_inner`, breaks no unit test. `one_running_process_per_session_and_stop` runs with no checkpoint, so the 409 comes from the in-memory `st.running` check, which returns `{"code","running"}` and **no status**. The §10 item 4 / §5-5 claim "답은 status" holds only on the lock-probe path, which only the container log exercises, and that log prints only the code. Add a test with a checkpoint present that asserts the `status` body, and make the in-memory 409 return `status()` too. PROGRESS §1 "409 `ALREADY_RUNNING`과 상태" overstates the tested behaviour.
- **S7**: `safe_id` allowing `.` and `/` is not caught. Add a traversal test for `attempt_id` and `session_id`, and for GET `runs/<id>/…`.
- **S9**: the "reap only while idle" safety is not tested. That is acceptable, since it is hard to test in-process, but say so.
- **S15**: an empty `oauth_token` file is not tested (only a missing file is).
- **S18**: reverting to `kill -s SIG` passes on the host (util-linux accepts both forms). Only the container run catches it. Note in PROGRESS that the procps regression check is the container run, or add a test that runs with a `kill` shim rejecting `-s`.

### 6. [Low] §7 adapter shape deviates and is not listed as remaining
- Error codes: `UNAUTHENTICATED` vs `UNAUTHORIZED`, and `EVENT_CONTENT_CONFLICT` vs `ID_CONTENT_CONFLICT`. `ALREADY_RUNNING`, `CREDENTIAL_UNAVAILABLE` and `EXTERNAL_RECONCILIATION_REQUIRED` are not among the §7 codes; §7 says reconnect returns `external_reconciliation_required` and `restored_checkpoint_ref` fields.
- `status` lacks `quota_group_id`, `provider_status_or_unknown`, `last_actual_action_at` and the session's process liveness. `start` returns no `capability_gaps`.
- Stop has no "safe shutdown conditions" report (unconfirmed external effects, active children, unsaved checkpoint).

Align these, or list them in PROGRESS §3.

### 7. [Low] SIGTERM and stop timing
- `serve` waits a fixed 30 s for the run after SIGTERM, but the run's stop takes up to `grace_ms` (caller-chosen, unbounded) plus the drain. `podman stop` defaults to 10 s and Kubernetes to 30 s. If `grace_ms` is near or above the runtime's stop timeout, PID 1 is SIGKILLed before the checkpoint and outcome are written. Cap `grace_ms` (or validate it against a configured stop budget) and document the required `terminationGracePeriodSeconds`.
- A `stop` that arrives between `launch` and the first `watch` iteration still spawns `claude` (with the prompt on stdin) before killing it. Check the flag before `spawn` in `run_with`.
- PROGRESS §1 "SIGTERM/SIGINT를 받으면 실행 중 회차를 guard로 멈추고 끝낸다" was not shown in the PR's evidence (both `podman stop` calls ran with no active run). I reproduced it (0.4 s, `Requested`, checkpoint written). Add that case to `run-fake.sh` and its log.

### 8. [Low] Checkpoint write durability
tmp+rename is atomic with respect to readers, but there is no `fsync` of the file or the directory. After a host crash the new name can point to an empty or partial file, which then hits F1. Add `sync_all` on the tmp file and on `/state`. Mutant S8 (non-atomic write) survives. That is acceptable, but the claim "원자적" is untested.

### 9. [Info] Items checked and found OK
- **Auth**: fails closed when `adapter.sha256` is missing or empty (hex never matches an empty string), and when the bearer is absent or wrong. The comparison is on SHA-256 hex, not constant-time; that is negligible for a hash of a random token. Transport is plain HTTP on `0.0.0.0:8080` inside the pod; acceptable for the PoC, record it for the cluster stage.
- **Decision mapping**: Resume / Tamper→new session plus `config_tamper_detected` (both ids and digests) / Stale→409 / mismatch→409 reconcile / running→409. Mutants S5 and S11 are killed. The second `decide` call is redundant (the first already returns the Tamper fields) but harmless.
- **Races**: the HTTP loop is single-threaded, so start, reconnect and stop are serialized. The only concurrency is with the worker thread, and that is guarded by `state` plus the per-session flock (same-process flock on a separate open file description does conflict, so the probe works while the worker holds the lock). The windows after `run_with` returns and before `running=None` cause only benign 202/409 answers.
- **`reap_orphans`**: in the binary, the run's child (and the `kill` helpers) are spawned and waited only while `running` is `Some`, and `running` is set under the same mutex that `reap_orphans` holds while it checks. So `waitpid(-1)` cannot take a run child's status and cannot interleave with `launch`. Caveat: it is `pub`, and calling it in a multi-threaded process that spawns children elsewhere (e.g. a test binary) would steal statuses. Document that it is PID-1-only. Zombies from group kills accumulate during a run and are reaped afterwards. The container showed 0 left.
- **`run_with`**: the stdin writer thread cannot block the guard. EPIPE is ignored (Rust ignores SIGPIPE), and the writer is released when the group is killed. The stop flag is checked every iteration. `Stop::Requested` goes through `stop_group` (TERM, then grace, then KILL), and `charge` treats it as unmeasured (not `Exited`), so unmeasured usage is never 0. `run_with_kill` and `run` keep their earlier behaviour (27/27 earlier mutants killed).
- **Kill form** `-SIG -- -PGID`: valid for util-linux 2.32.1 (host), procps-ng in the bookworm image, and Ubuntu CI procps (CI green). Exit status for an absent group is 1 on both implementations, so the `group_alive` fallback is unchanged.
- **Credentials**: the token is read into memory and passed only through the env allow-list. The outcome records env **names** only. Error strings carry no values. The tests and the container log find 0 occurrences. `GET runs/<id>/stderr|stdout` returns the raw process output to the authenticated coordinator; that is outside the supervisor's control and acceptable.
- **Bodies**: capped at 1 MiB (`take(1<<20)`); a truncated body gives 400. Run-file GETs are unbounded reads of the run's own output (Info).
- **Dependencies**: nix (features `signal`, `process`; only `wait` is used, so `signal` could be dropped), signal-hook and tiny_http are reasonable safe wrappers given `unsafe` is forbidden. Cargo.lock is consistent (`--locked` resolves).

### 10. [Info] PROGRESS.md scope
It correctly states fake claude only, 0 model calls, 02-04 not accepted, 11/73 unchanged, and lists real-call tests, the caller/executor, the coordinator container and the role image as remaining. Corrections needed: §1 claims "409와 상태" (F5), "실행 뒤 §5-1 체크포인트" without the §5-1 gaps (F3), and SIGTERM-during-run (F7) without evidence. Also add F1, F4 and F6 to §3.
