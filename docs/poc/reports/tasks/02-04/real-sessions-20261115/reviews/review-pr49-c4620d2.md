# Review of PR #49 at c4620d2

- **PR**: #49 "02-04: budget-enforcing caller (ieo-role-call), real-session run plan and container dry run"
- **Head reviewed**: `c4620d29a334020b2aefb8526dba018bdb580ead`, confirmed with `gh pr view 49 --json headRefOid,statusCheckRollup`. Base: main `d9123a1`.
- **CI at this head**: all 3 required checks SUCCESS.
- **Reviewer**: independent review agent (Claude). This is review 1 of at most 4.
- **Verdict: CHANGES REQUESTED**

The caller is mostly sound and fail-closed:
- the stop_required gate;
- the duplicate-attempt guard;
- shares that cannot exceed the approval;
- the resume base taken from the caller's own record of the run actually resumed;
- guard-stopped runs are unmeasured, and a missing observed value maps to `u64::MAX`, which makes the run unmeasured;
- a lost POST answer is recorded as unmeasured with stop_required;
- no retry.

However, a model run can still end up **without any caller record**. I reproduced this in containers with a fake claude: the next call then started on a ledger that did not count the lost run. The setup script can also wipe the role-side evidence of that run. Both are budget-authority defects and must be fixed before the first real call.

## What I ran

All runs were in a worktree of the head under the scratchpad (`rv49a`), with `CARGO_TARGET_DIR=<scratchpad>/target-review49a`. There were no model calls and no real claude. I did not read `/opt/rust/ieo_poc_secrets`.

- **Lint and tests**:
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
  - `cargo test -p ieo-supervisor --locked`: all passed. Counts per binary: 1, 7 (caller), 26, 18, 20; 72 tests in total.
- **Caller mutations** (13 in total; `cargo test -p ieo-supervisor --test caller`, script `mut.sh`):
  - Caught (7):
    - no stop_required gate
    - no base-attempt match
    - no duplicate-attempt check
    - no post-run signal check
    - no share-sum check
    - no cleanup_failed flag
    - role token room ignored
  - **Survived** (6):
    - M2: environment check removed (`CLAUDE_CODE_OAUTH_TOKEN` / `ANTHROPIC_*`)
    - M4: `charge_reported(true, …)`, so guard-stopped runs are charged as measured
    - M8: per-role `max_runs` share check removed
    - M10: the `before.running` check removed (the service also refuses)
    - M11: a missing guard-observed value treated as 0 instead of `u64::MAX`
    - M13: the per-role time left no longer cuts the window
- **`dry-run.sh` rerun from my worktree** (workdir under the scratchpad, ports 58221/58222 checked free first): all 8 checks PASS, 0/0 secret strings, stop timeout 15 s. The result is the same as `evidence/dry-run.log`.
- **Lost-run reproduction**. Same scripts, but the fake claude sleeps 15 s, with a fresh workdir:
  1. I started `ieo-role-call … --role development --op start --attempt D1` and killed the caller after 4 s.
  2. After the run ended, the service status showed `last = D1, stop Exited`, and **`ledger/records.jsonl` did not exist**.
  3. The next call (`G1`, guardrail start) **started and was recorded as measured**. The caller ledger then held only `['G1']`.
  4. A further development start was refused only by the role service backstop (`BUDGET_EXCEEDED`), because the caller asked for the full share room.
- **Cleanup**: I removed both `ieo-dry-*` containers and ran `podman unshare rm -rf` on both workdirs. I also remove the worktree and target dir at the end of this review.

## Findings

### Must fix before the first real call

**H1 (High): a model run can end without a caller record.** Location: `crates/ieo-supervisor/src/caller.rs::call`, after the `202`.

Every path below leaves the caller ledger without the run, while the run happened or may have happened:

- **Transport error on the stdout fetch** (L438–443). `adapter.send(…stdout…)?` returns `Err`, and the binary exits 1 ("usage error") with no record.
- **Filesystem error saving the stream** (L449 `create_dir_all(...)?`, L451 `std::fs::write(...)?`). Same result.
- **The caller process dies while polling.** The poll can last up to window + grace + 90 s, about 11 min at the plan's 600 s window. A Ctrl-C, SSH drop, OOM or reboot all qualify. Nothing is written, because the record is only appended at the end. I reproduced this; see "What I ran".
- **The final `append` fails.** Unavoidable as such, but it currently has the same effect.

Consequences:
- The next call computes `budget::next` and `Limits::for_next` without the run. This breaks "all runs count" and the 12-run count, and undercounts tokens and seconds.
- For the other role, or a fresh session, the call simply starts. I observed this.
- For a resume, the missing record only happens to yield "unmeasured" through the base mismatch.
- The role-service backstop is a per-role defence in depth, not the authority.

Fix:
1. **Write ahead before the POST.** For example, write `ledger/inflight/<attempt>.json` (fsynced) before sending start/reconnect, and remove it only after the final record is appended. `call()` refuses (`NotStarted`) while any in-flight file exists, and a person reconciles it as for `stop_required`. Alternatively, append a "started" record and make `spent` pair start and end records, so nothing is double counted.
2. **Map every error after the POST to `fail(...)`** (recorded, unmeasured, stop_required) instead of `?`. Wrap the post-202 part so that any `Err` becomes `fail`.
3. **Cheap extra guard:** before starting, refuse if `before.last.attempt_id` (or any attempt directory or attempts-ledger start on the service) is not in the caller's records.
4. **RUN_PLAN §4 and the binary doc:** any exit other than 0 or 2 means stop. Reconcile against the role service (`/v0/role/status`, `/state/budget/attempts.jsonl`, `/state/runs/`) before any further call. Today exit 1 is documented as "usage error" and has no row in the stop table.

PROGRESS §1 claims that a lost answer ("응답 분실") becomes `stop_required`. That holds only for the POST answer and for a poll timeout, not for these paths.

**H2 (High): `run-real.sh` can destroy the only evidence of an unrecorded run.** Location: `run-real.sh` L23–24 and L71.

The reset guard checks only `$W/ledger/records.jsonl`. If the first call left no caller record (H1) and the setup is re-run, the script deletes the role state:
- `podman unshare rm -rf "$W/development" "$W/guardrail" …` removes the service attempts ledger, `runs/<attempt>/stdout.jsonl`, the checkpoint and the session files;
- `podman rm -f` kills a container that may still be running the model.

Fix: also refuse when any of these exist:
- `$W/*/state/budget/attempts.jsonl`
- `$W/*/state/runs/*`
- `$W/ledger/`
- a running `$NAME_PREFIX-$REPO-*` container

Never `rm -f` a container whose status reports `running`.

**M1 (Medium): a non-success result is not surfaced, so D1/G1 credential failures look like success.** Location: `caller.rs` end of `call`, `ieo-role-call.rs` summary, `call.sh`.

An authentication failure, or any `result.is_error`/non-`success` subtype on an `Exited` run, produces:
- `stop_required: false`;
- exit 0;
- a printed summary without the result subtype or `is_error`.

RUN_PLAN §4 says to stop on an authentication failure, but the operator only sees "exit 0 · summary …". D1 and G1 are exactly the credential checks.

Fix (one of):
- treat `result.is_error != false` or `subtype != "success"` as `stop_required`, which gives exit 3;
- or at minimum print `result.subtype` and `is_error` in the summary, and make RUN_PLAN require checking them before the next step.

### Should fix (in this PR, but not a correctness blocker for the budget)

**M2 (Medium): the no-overage description overstates what happens before the call.** Location: RUN_PLAN.md §3 L57–59, and the PR body "Before a call".

The plan lists all three sources under "호출 전". In the code:
- before the call, only the past stream is checked (`Limits::for_next` → `observation_check`);
- the credential form is checked once at setup, in `run-real.sh`;
- the environment is checked after the run, from `env_names`;
- this run's signal is checked during the run (guard) and after it.

The environment is in fact fixed by `env::build`'s allowlist, so the design is defensible. User decision 2026-11-14 pr30 §3 asks for exact statements, so state this precisely. Also state the limit plainly: the first `rate_limit_event` arrives only after the first API request, so "guard stops on the first breaking event" is a stop after observation, not a provider-side block. Do not describe it as one.

Optionally, re-check the credential form (prefix only) per call in `call.sh`. Also record a flag when a run has **no** `rate_limit_event`. Today such a run gets no reason; the next call is held, but the record does not say why.

**M3 (Medium): tests do not cover several authority checks; 6 of 13 mutations survive.** Location: `tests/caller.rs`.

Add tests at least for:
- the environment check (M2 mutation), which is one of the three no-overage sources;
- guard stop → `usage.run` unmeasured in the caller record (M4);
- the per-role `max_runs` share (M8);
- a missing `observed` value → unmeasured (M11).

The share-time cut of the window (M13) and the `running` check (M10) are lower priority.

Also add transport-failure tests with a fake `Adapter` that fails:
- the POST;
- the status polls until the deadline;
- the stdout fetch (this one will expose H1).

**M4 (Medium): plan coverage against user decision 2026-11-14 pr30 §4.** Location: RUN_PLAN.md §2.

The 7 runs are faithful to §10 items 1–3 and §11 item 7:
- item 1: G1;
- item 2: D1, with the supplied marker, the repo CLAUDE.md marker and the hook;
- item 3 (a): D1;
- item 3 (b): D6, without the plugins/store path;
- item 3 (c): D2–D5, which cover each prior kind (no tool, Read, Agent child, `/compact`);
- §11 item 7: D1..D5, with the D4 `compact_boundary` required, so compression is not passed unobserved.

User decision §4 also lists 기동·종료·재연결·체크포인트 복원 for the real tests. Neither of the following is in the plan:
- a real-process stop (adapter stop or timeout of a real run);
- checkpoint restoration across a supervisor restart.

Suggested additions:
- restart the development container between D1 and D2, which costs no run;
- either add a stop run from the spares, or state explicitly that 종료 and items 4–5 are covered only by mock tests (progress record 3) and label them as mock in the report. §4 requires separating stored-log replay, mock and real.

### Low / informational

- **L1 — clock.** `unix_now()` falls back to 0, and `saturating_sub` turns a past-dated or future-dated observation into age 0, so a clock set backwards makes a stale observation look fresh. Hold the call when `now < observed_unix`, or when the clock is before the 02-04 start.
- **L2 — no ledger lock and multi-syscall appends.** There is no lock on the ledger. `writeln!(f, "{rec}")` on an unbuffered `File` issues many `write` syscalls per record. Concurrent callers can interleave lines, and a crash can leave a partial line. A partial line is fail-closed: `records()` rejects it, which blocks everything. Serialise to one buffer with `write_all`, and `flock` the ledger for the whole `call()`. The same pattern is in `serve::append_ledger`. With per-role shares summing to the approval, concurrent use cannot exceed the approval, but the reserve logic is bypassed.
- **L3 — workdir guard.** `run-real.sh` only pattern-matches `/opt/rust/src/*`. A relative `W` from the repo root passes. Resolve with `realpath -m` first.
- **L4 — `dry-run.sh` deletes without a guard.** `podman unshare rm -rf "$W"` runs on whatever is given. Pointed at the real workdir by mistake, it would delete the real ledger. Require a scratch prefix, or refuse when `$W/run/ledger/records.jsonl` exists from a non-dry plan.
- **L5 — dry-run label.** The section "next call after the plan: refused or held?" in `dry-run.sh` only counts lines and checks nothing. Make it a real check or drop the label.
- **L6 — image not pinned.** The image is used by tag (`golang:1.26.9-bookworm`), while `coordinator.json` asserts `image_digest golang@sha256:d9c68c…`. Run by digest, or check `podman image inspect` against it.
- **L7 — adapter token in `ps`.** `curl -H "Authorization: Bearer $(cat adapter.token)"` puts the adapter token, not the model credential, in argv, where `ps` shows it. Use `-H @file` or a config file.
- **L8 — lossy stream copy.** The stream is fetched via `String::from_utf8_lossy`, so the saved file and its SHA-256 are of the lossy text, not the raw bytes. Note this in the report, or return base64.
- **L9 — R3 procedure gap.** An attempt with an `{"unknown": true}` end line can never be unblocked by appending a further end line, because any unreadable end makes the whole attempt `None`. Say in RUN_PLAN §4 what a person does then. It is rare today, since an `Ok` outcome always carries observed values.
- **L10 — PID-1 reaping depends on no `--init`.** Reaping is correct now that serve runs as PID 1 (no `--init` in `run-real.sh`). If `--init` or catatonit is ever added, orphans would no longer be reaped. Leave a comment in `run-real.sh`.

## Role service changes (R1, R2, R4, R6, PID 1): no regression found

- **R1**: the lock probe now precedes the `takeover`/`error` phase rule. The `flock` probe conflicts with the live holder thread, and a restart releases the lock. The new early `ALREADY_RUNNING` makes the later `Reconnect::AlreadyRunning` arm effectively dead but harmless.
- **R2**: `own` is used only when `Exited`, and the error checkpoint passes `false`.
- **R4**: max over all end lines; any unreadable line blocks.
- **R6**: tests present.
- **Reaping**: limited to PID 1, still under the state lock with `running` checked, so there is no race with `run_with`.

Containers match the contract §2 values:
- UID 20001/20002;
- `--cap-drop ALL` and `no-new-privileges`;
- supervisor as PID 1;
- RO `/etc/ieo/role` and `/run/ieo/cred`;
- labels;
- loopback-only port;
- `--stop-timeout 15`.

State is in bind-mounted directories rather than named volumes. That is acceptable for the same-host stage. The fixture repository instead of Sori is acceptable for 02-04: the decisions reserve Sori for X-02.

## Secrets and publication

- No path prints the credential:
  - the form check is `head -c 10 | grep -q`;
  - caller errors use `e.kind()` only;
  - summaries and records hold variable names, paths, sizes and hashes.
- In my dry run, 0 fake credential strings appeared in the summaries and container logs.
- Raw streams stay in the workdir. RUN_PLAN §5 publishes summaries and a records copy after a credential-pattern scan. Keep that scan mandatory, because the summaries include up to 400 characters of the model's answer and 200 characters of each tool error.
