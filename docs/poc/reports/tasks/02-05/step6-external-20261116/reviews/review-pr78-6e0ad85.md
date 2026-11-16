# Review of PR #78 at 6e0ad85 (review 2)

- PR: HeaInSeo/ieo_poc #78, 02-05 step 6 (external-request tracking, C9)
- Head reviewed: `6e0ad8575b03a4082a7e60b9acb6c9609f7364c9`. Previous review: `1b362ec` (review 1, changes requested; M-1).
- Correction commits: `9ebf329` (code and contract), `81bfe7a` (evidence), `6e0ad85` (REPORT §10, archived review, ledger).
- Reviewer: independent. I worked in a fresh detached worktree, `scratchpad/r78b`, which has since been removed.
- Model calls: 0. I did not touch 55433, 58090, PR #29 or other worktrees. Only the one-off test DB on 55432 was used.

## Verdict

**Approve.** M-1 is fixed, and I verified the fix with my own probe. Every review 1 Low item is handled. I found no High or Medium in the correction and no regression in `1b362ec..6e0ad85`.

The remaining items are Low or Info carry-overs (§ "Carry-over"). None of them blocks the merge.

## What I ran

Logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` (after `cargo clean -p` of the workspace crates) | both OK | `r78b-lint.log` |
| `cargo test --workspace --locked` (55432, `RUST_TEST_THREADS=4`, `-j 4`) | **388 passed, 0 failed, 0 ignored**. This matches REPORT (384 + 4) | `r78b-test.log` |
| `openapi_spec_validator` | OK. `coordinator_pending_external_requests` is now `oneOf [array, string]`. It is still 0.14.0 and unmerged, so this is no break against 0.13.0 | `r78b-docs.log` |
| `render_progress.py --check` / `validate_package.py` | `ok: true` / `schema_errors: 0` | `r78b-docs.log` |
| Links in the step-6 REPORT and runtime contract | all resolve | `r78b-docs.log` |
| `git diff 9ebf329 6e0ad85 -- crates contracts Cargo.lock` | empty. The evidence commit (`test.log` line 1: 9ebf329, clean tree) is the code under review | `r78b-docs.log` |
| **My M-1 probe** (review 1's stopdb probe, temporary test file, removed afterwards) | warm stop: 200. After `pg_terminate_backend` on the test DB: **200**, with both roles reported, `coordinator_pending_external_requests: "unreadable"`, the reason `coordinator: pending deliveries could not be read (500 database error: db error)`, and `safe_shutdown_confirmed: false`. Before the fix this was a bare 500 | `r78b-probe-stopdb.log` |
| `limits_e2e` × 2 alone | 2/2 passed each time (123 s) | `r78b-limits-1.log`, `r78b-limits-2.log` |
| `limits_e2e` under load: run at the same time as the `coordinator` (81 tests) and supervisor `serve` (44 tests) binaries, each with 8 test threads | all passed | `r78b-limits-3-load.log`, `r78b-load-coord.log`, `r78b-load-serve.log` |

### New hand mutations (mine)

Each was applied alone and restored with `git checkout`. The tree was clean after each.

| ID | Mutation | Target | Result |
| --- | --- | --- | --- |
| N1 | `pending.rs` `add`: `self.read()?` → `self.read().unwrap_or_default()`, so an add over an unreadable file overwrites it with a fresh list and reconnect would pass again | `ieo-supervisor` | KILLED (`pending::tests::entries_are_kept…`) |
| N2 | `serve.rs` `report_tracked`: the `record_report` on the not-sent path is dropped, so the status no longer says why the report was not sent | `serve` | KILLED (both transports, step 7) |
| N3 | `serve.rs:1666` `first_action_tracked`: the not-sent answer says `"ok": true` | `serve` | **SURVIVED**. The status `first_action_report.result` would claim success for a report that was never sent. Nothing asserts that field on the not-sent path (I-b) |

The author's 7 correction mutants (C1-M1, M2, L1a, L1b, R2, R3, R6) are in `evidence/mutants.txt`, and all are killed. They cover the review 1 survivors R2, R3 and R6.

Test DBs: `ieo_t_%` is 0 after my runs. No mutation run left a DB.

## Review 1 items

| Item | Status | Check |
| --- | --- | --- |
| M-1 stopRepo 500 after the stops | **Fixed** | `api.rs:633-674`: `run(...)` without `?`. Both a non-200 `Resp` and an `Abort` go to the `failed` arm. The arm keeps the `roles::stop` status and body, sets `"unreadable"`, adds a reason with the cause and recomputes `confirmed`. My probe shows this, and so does the author's `a_repo_stop_keeps_its_report_when_the_pending_deliveries_cannot_be_read`, which loops until a cut connection is hit (no timing assumption). OpenAPI, contract item 20 and REPORT §3 are updated |
| L-1 a failed add is ignored | **Fixed** (design reviewed below) | |
| L-2 same-UID limit | **Fixed** | `pending.rs` module doc, contract item 18 and REPORT §2.1 / §8 say that the list is not an integrity guarantee, and that the final basis is the coordinator record and step 7 |
| L-3 R2, R3, R6 | **Fixed** | Serve step 6 (409 `EVENT_CONTENT_CONFLICT` clears the entry and the reconnect resumes), `limits_e2e` second stop (`result unknown`, exact pair), `a_local_only_role_has_no_pending_external_requests`. The author's C1-R2, C1-R3 and C1-R6 are killed |
| L-4 §8 owners | **Fixed** | Repro: 01-08 / 02-06. Progress reports and "start의 목록": 02-05 step 9 |
| I-1 `not_observable` is not a reason | Kept, owner 04-05 | fine |
| I-2 stale comment | **Fixed** | `api.rs:610-612` |
| I-3 R5 near-equivalent | Sentence added to §1 | fine |
| I-4 `limits_e2e` flake | **Fixed for the failing path**, explanation confirmed | see below |

### L-1: "not on the list → not sent" on every send path

I traced each send path:

- **Takeover report** (`serve.rs:1325-1347` → `report_tracked`). If the add fails, the method returns `Refusal{503, EXTERNAL_REQUEST_NOT_TRACKED}` with no `coordinator_status`. The staged copy is therefore kept, and the run ends `CentralRefused` before the model starts. The next start's staged check (`serve.rs:365-399`) drops a staged copy that the coordinator does not hold. So nothing is lost or left dangling. **Fail-closed.**
- **after_run / error report** (`write_local` first, then `report_tracked`). If the add fails, nothing is sent and `last_report` records why. The local checkpoint is now ahead of the coordinator. The next start's compare sends it from the archive, or stops for reconciliation. **Fail-closed.**
- **First-action report** (the run's thread). If the add fails, it returns `{"ok": false, "not_sent": true}` and nothing is sent. The run goes on, and the after_run checkpoint carries the same first action. That checkpoint's own report is also not sent while the list is broken, and reaches the coordinator later through compare. This is consistent with contract item 18.
- **Re-query re-send** (`requery_pending` → `first_action_tracked`). The `add` replaces the existing entry. If that fails, nothing is sent and the old entry stays. With a readable list, reconnect gets 409 because the list is non-empty. With an unreadable one, `list()` fails and reconnect also gets 409. Serve step 7 asserts both that the fake received 0 reports and the 409. **Fail-closed.**
- **Compare's own archive sends** (`central::compare` → `central.report`). These are *not* on the list, before or after this correction. That is acceptable: compare re-reads the coordinator's latest record first and again on every later start, so its outcome never needs the list. But REPORT §1 and contract item 18 do not mention this path (I-c).

**Accepted-start behaviour change** (REPORT §3 item 5, contract item 18, §7, §8). With an unreadable or unwritable list, a start is answered 202. Its takeover is not sent, the run ends `CentralRefused`, the model is never called, and the coordinator holds nothing new. This fails closed and is documented. It does wedge the role until a person repairs the file: every start and reconnect is refused while the file is broken. Recovery is "fix or remove the file", and serve step 7 shows that the role works again after the blocker is removed and that the re-query then sends the held report. Two rough edges remain (L-a):

1. The reason does not name the file. It reads `… could not be put on the pending external requests list (pending requests: Is a directory (os error 21))`, with no path. In the unwritable case the status list still looks normal.
2. The start is *accepted* (202) and then runs nothing. That consumes the attempt id. On the delivery path it also leaves the author's side finding: a never-spawned run recorded `unmeasured`.

### I-4: the explanation and the fix

- **Explanation confirmed.** The takeover check takes `delivery:{repo}:{role}` (`api.rs:4223`, `check_delivery`), the same advisory lock that the test holds for 15 s. With a fixed 1.5 s delay, a slow pre-call phase lets the test take the lock before the takeover. The takeover then hits the 10 s statement bound and is refused, so aL never spawns and is `unmeasured`. That is exactly the value I saw in review 1. The coordinator behaved correctly, so this was a test race, not a code race.
- **The fix is condition-based.** The role server sets `handled` and holds its answer until `release`. The lock thread waits for `handled`, plus (for aL) for the supervisor status to show the run ended (`running: null`, `last.attempt_id == aL`). Only then does it take the lock and set `release`. The takeover therefore always comes before the lock.
- **Remaining sleeps in 8b.** The `20 ms` polls are fine. The 15 s hold is deliberately longer than the 10 s statement bound, so it is relative, not a race. The 48 s wait is the in-flight age bound, which is intended.
- **One fixed-time start remains in step (i), not the I-4 path.** `limits_e2e.rs:904-914` takes the budget ledger lock in a thread and then the main thread sleeps 200 ms before delivering. If that thread has not yet taken the lock within 200 ms under load, the delivery is not `not_called` and step (i) fails (I-a). This is pre-existing step-5 code. It passed in all three of my runs, including the one under load.

### The author's side finding: a takeover-refused, never-spawned run stays `unmeasured`

- **Severity: Low.** Carry-over with an owner (02-05 step 9, REPORT §8 last row).
- It fails closed: the assignment's next delivery stops with `ASSIGNMENT_USAGE_UNMEASURED` until a person reconciles. Nothing is under-counted.
- It is not new to step 6: any takeover refusal (5xx, lost answer, lock timeout) already led here in step 5. Step 6 adds one more trigger, a broken pending list.
- The cost is availability: a person must check a run that provably never started (`spawned: false`). Changing this touches the accepted 02-04 rule that a stopped run is always unmeasured (`run::charge`). That decision belongs with the acceptance (step 9), as the author proposes. I agree with the severity and the owner.

## Regressions in 1b362ec..6e0ad85

None found.

- The stopRepo change only adds a branch. Its success path is the same as at 1b362ec: the `limits_e2e` stops, `roles.rs` and the `coordinator.rs` stop tests all pass.
- `report_tracked` and `first_action_tracked` are unchanged after a successful add.
- The test changes tighten assertions; none is weakened:
  - The `limits_e2e` lock helper now takes conditions.
  - The fake's `first_action` returns a status pair.
- Ledger: snapshot 0050 adds the review 1 archive and the correction summary. Its earlier sentence "stopRepo … (500 가능)" is now superseded by the correction text that follows it (I-d).

## Findings

### Must fix before merge

None.

### Carry-over (Low / Info)

- **L-a (Low): recovery after a broken pending list is not self-explanatory, and the start is accepted before it fails.**
  - **Where:** `serve.rs:1613-1629` (the reason text) and the launch path in `serve.rs` (202 before the takeover).
  - **Scenario:** development code (same UID) creates `/state/pending-requests.tmp/`. Every later start is answered 202, ends `CentralRefused`, and on the delivery path leaves an `unmeasured` ledger record that stops the assignment. The reason names an OS error but not the file. In the unwritable case the status list looks healthy.
  - **Fix (suggested, no blocker):**
    1. Name the file (`/state/pending-requests.json` / `.tmp`) in the `EXTERNAL_REQUEST_NOT_TRACKED` reason and in the status.
    2. Before answering 202, check under the launch gate that the list can be read and written (for example `list()` plus a no-op `add` / `remove` of a probe id). If not, refuse at once with 409 `EXTERNAL_RECONCILIATION_REQUIRED`, as reconnect does. The attempt id is then not consumed, and no never-spawned run reaches the ledger.
  - **Owner:** 02-05 step 9, together with the unmeasured-ledger carry-over, or the next supervisor PR.
- **I-a (Info): one fixed 200 ms start-up race remains in `limits_e2e` step (i)** (`limits_e2e.rs:911-914`, pre-existing step 5). Signal after `f.lock()` instead of sleeping 200 ms.
- **I-b (Info): N3 survives.** The `not_sent` first-action answer's `ok` value is not asserted. Add `assert_eq!(s["first_action_report"]["result"]["not_sent"], true)` and `ok == false` to serve step 7.
- **I-c (Info): compare's archive sends are outside the list.** They need no list, because compare re-reads the coordinator's record, but REPORT §1 and contract item 18 list "checkpoint reports" without this exception. Add one sentence.
- **I-d (Info): ledger wording.** Snapshot 0050's 02-05 `result_state` still says "stopRepo … (500 가능)" before the correction text. A reader may take the 500 as current. Reword it at the next ledger update.
- **Side finding (Low, owner 02-05 step 9):** a never-spawned run is recorded `unmeasured` (above). The severity and owner are agreed.

## Summary

The correction closes M-1 properly. A failed DB read after the stops now returns the full stop report, with `"unreadable"`, a reason and `confirmed: false`, and my review 1 probe confirms this.

L-1 is handled with a coherent "not on the list → not sent" rule on every tracked send path. The new accepted-start change (a broken list runs nothing) fails closed and is documented. It does need a person to repair the file, and its reason and its 202-then-refuse shape could be clearer (L-a, carry-over).

L-2 to L-4, I-2 and I-3 are done. The I-4 explanation is right, and the fixed test is condition-based and stable: 3/3 runs passed, one of them under load.

Tests (388), fmt, clippy, OpenAPI, render `--check` and the validator all pass. **Approve.** L-a, I-a to I-d and the unmeasured-ledger side finding are carry-overs, not merge blockers.
