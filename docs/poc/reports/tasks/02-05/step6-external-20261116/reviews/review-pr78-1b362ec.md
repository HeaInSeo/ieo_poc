# Review of PR #78 at 1b362ec (review 1)

- PR: HeaInSeo/ieo_poc #78, 02-05 step 6 (external-request tracking, carry-over C9)
- Head reviewed: `1b362ec05e38421e41d6ce595762269a8625b9da`. Base: main `de1336c`.
- Reviewer: independent (I did not write this code). Reviewed adversarially in a detached worktree.
- Model calls: 0. No real `claude`. PostgreSQL 55433, port 58090, PR #29 and other worktrees were not touched. Only the one-off test DB on 55432 was used.

## Verdict

**Changes requested.** There is one Medium finding (M-1). Everything else is Low or Info.

The core tracking design is sound. Entries are added before the send. A crash on either side of the send is settled by the next re-query. A list that cannot be read fails closed for reconnect. `not_observable` is never reported as "none". C-1 and C-2 from review af68c93 are fixed, and C-2 is really exercised: reviewer mutation H5 is now killed.

The blocking problem is in stopRepo, an accepted 02-04 operation. It now sends the stops to both roles before it reads the DB. If that read fails, it answers 500 and drops the whole stop report. I reproduced this.

## What I ran

All logs are in the scratchpad, `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --check` | OK | `r78-lint.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (after `cargo clean -p` of the workspace crates) | OK | `r78-lint.log` |
| `cargo test --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`, `-j 4`) | **384 passed, 0 failed, 0 ignored**. This matches REPORT §4 (380 + 4) | `r78-test.log` |
| `openapi_spec_validator contracts/ieo-api-v0.openapi.yaml` | OK. The 0.14.0 change is additive: one optional response property, plus description text | `r78-docs.log` |
| `render_progress.py --check` | `ok: true` | `r78-docs.log` |
| `validate_package.py` | `schema_errors: 0` | `r78-docs.log` |
| Relative links in the step-6 REPORT, the runtime contract, the step-5 REPORT, RESUME.md and STATUS.md | all resolve | (inline) |
| `gh pr view 77`, `gh run view 43821085760`, `gh pr view 78` | #77 MERGED as `de1336c`, head `af68c93`, 07:03:27Z (= 16:03 KST). Run 43821085760: `ci`, push on main, `de1336c`, success. #78 head is `1b362ec` | `r78-gh.log` |
| `git diff e7e4e4b 1b362ec -- crates contracts Cargo.lock` | empty, so the evidence commit is the code under review | (inline) |
| Probe: stopRepo with the DB connection killed after the warm-up (temporary test file, removed afterwards) | **500 `{"code":"INTERNAL","message":"database error: db error"}`**, although the stop calls had already been sent (M-1) | `r78-probe-stopdb.log` |
| Re-run of `limits_e2e` (unmodified) | 2/2 passed (see I-4 for one earlier flake) | `r78-limits-rerun.log` |

### Hand mutations (mine)

Each mutation was applied alone with `r78-mut.sh` and the target test was run. The file was restored afterwards with `git checkout`, and `git status` was clean after each one.

| ID | Mutation | Target | Result |
| --- | --- | --- | --- |
| R1 (= af68c93 H5) | `limits.rs:120` `(may_have_run \|\| sent)` → `(may_have_run)` | `delivery_e2e` | **KILLED**, both transports, at `delivery_e2e.rs:1078`, the new C-2 step (`r78-mut-R1.log`) |
| R2 | `serve.rs` `first_action_tracked`: the 4xx arm is removed (`answered = r["ok"] == true`) | `ieo-supervisor` (all) | **SURVIVED** (`r78-mut-R2.log`) |
| R3 | `api.rs:729` `Some("unknown") => "result unknown"` is removed, so `unknown` deliveries are not reported | `limits_e2e`, `roles` | **SURVIVED** (`r78-mut-R3.log`) |
| R4 | `api.rs` `safe_shutdown_confirmed` → `json!(true)` | `roles`, `limits_e2e` | KILLED (`limits_e2e.rs:972`) |
| R5 | `serve.rs` `report_tracked`: a 4xx refusal no longer clears the entry | `serve` | SURVIVED. Close to equivalent, because the next compare clears it (I-3) |
| R6 | `serve.rs:1636`: the `LocalOnly` early return is removed, so local-only first actions are tracked and never cleared | `serve` | **SURVIVED** (`r78-mut-R6.log`) |
| R9 | `roles.rs:180`: `pending_external_requests` is dropped from the reason keys | `roles` | KILLED (2 tests) |

The author's 13 mutants (`evidence/mutants.txt`) cover the main supervisor paths. R2, R3 and R6 are the gaps (L-3).

Test DBs: five `ieo_t_*` databases were left behind by my failing mutation runs (PIDs 539359, 540840 and 553166, all of them my processes and no longer running). I dropped them by name. `ieo_t_%` is 0 afterwards.

## Analysis

### 1. Tracking correctness (supervisor)

- **What is tracked.** Two kinds of request are tracked: checkpoint reports (`report_tracked`, a staged takeover included) and first-action reports (`first_action_tracked`). I grepped `crates/ieo-supervisor/src` and found no other coordinator-bound send (no progress report, repro request or session-status report). So the "nothing else can be pending today" claim in REPORT §1 holds.
- **Before the send.** `serve.rs:1613` and `:1648` add the entry before `central.report` / `record_first_action`. A crash after the add and before the send leaves a stale entry. A checkpoint entry is then cleared by the next compare. A first-action entry is re-sent, and the coordinator is idempotent on the same body (`api.rs` `record_first_action`: same row and body → 200). A crash after the answer and before the remove is settled the same way. There is no window in which a request is sent and has no entry, **except when the add itself fails** (L-1).
- **Clearing on 4xx.** For checkpoints, `central::report` only puts `coordinator_status` on an *answered* non-2xx, and only 4xx counts. That is the 02-04 rule. For first actions, the coordinator's 4xx answers are 409 `EVENT_CONTENT_CONFLICT` (another first action is already recorded), 409 `CHECKPOINT_MISMATCH` (no takeover of the session), 400 / 401 / 403 and 404. Each one means "not applied, and the coordinator decided". So clearing on 4xx is right for both kinds: the outcome is known. The 4xx branch for first actions is untested (R2, L-3).
- **What proves the re-query.** `central::compare` returns `compared: true` only in two cases. Either the coordinator's latest record equals the local checkpoint, or every missing archive entry up to the local one has just been accepted (`central.rs` compare). In both cases each checkpoint report on the list is either applied or confirmed superseded. A lost staged takeover is either adopted (`cseq == lseq` after adoption) or confirmed not held (`cseq < staged seq`). So clearing all `checkpoint_report` entries on `compared` is justified. `compared` is `false` only for `LocalOnly`, which never adds checkpoint entries. A failed compare returns before the re-query, so the entries survive. This is correct.
- **Persistence.** `durable_write` does tmp + fsync + rename + dir fsync. A corrupt or non-list file gives `Err` → `report()` = `"unreadable"` → reconnect 409 (`serve.rs:750-765`) and a stop reason (`roles.rs:180`). This was verified by the author's step 5 and by mutation S6-P8.
- **Same UID.** The file sits on the role volume next to the development cwd (`/state/work/<repo>`), and the model runs as the same UID. Runtime contract §2.1 already says the development checkpoint "can be changed by development code". Neither revision 13 §16 item 18 nor the REPORT says this about `pending-requests.json`. A `rm` empties the list, and a planted directory at `pending-requests.tmp` makes every add fail silently (L-1, L-2). The practical impact is bounded: the only entries that can really stay pending are first-action claims, and the model already holds the claim credential per §2.1. But the limit should be written down.
- **Restart.** A restarted service reads the same file. The author's test step 1 confirms this.

### 2. Reconnect refusal and start

- The reconnect check runs after the re-query and before `checkpoint::decide`. Both `Ok(non-empty)` and `Err` give 409 `EXTERNAL_RECONCILIATION_REQUIRED` with the list, and nothing runs. I found no bypass through the HTTP / gRPC adapter. `cp_live` keeps the decision from reading the checkpoint's historical list (S6-P6). That is the right call, or a reconnect could never resume after any tracked request.
- **Start** (a new session) proceeds over a non-empty list. This is documented in REPORT §7 / §8 and in contract item 18. It also proceeds over an **unreadable** list: `live` is `Err`, `cp_live` uses `unwrap_or_default()`, and every later `add` fails silently. The new session then writes checkpoints with `pending_external_requests: "unreadable"`, and every later reconnect stops until a person repairs the file. That is fail-closed for reconnect, but the start case is not written down (L-1).

### 3. `not_observable`

- Status (`safe_shutdown.direct_network_requests`), checkpoint (`direct_network_requests`) and the capability gaps all say `not_observable`. It is never "none" or `[]`.
- `roles::stop` does not treat it as a reason. This is documented, and the decision is deferred to 04-05 with an owner (REPORT §2.2, §7, §8; contract item 19). Today the stop is unconfirmed anyway because of `active_children: "not_tracked"` (I-1).

### 4. stopRepo

- `api.rs:625-656`: `roles::stop` runs first and sends the stops. Then `run(db, …pending_deliveries…)?` runs, and `if pending.status != 200 { return Ok(pending) }` follows. A DB failure at that point replaces the whole stop report with a generic 500. This is M-1, and I reproduced it.
- `safe_shutdown_confirmed` is recomputed from `unconfirmed`. It stays `false` while the roles report `active_children: "not_tracked"` (R4 killed). `pending_deliveries` reads every delivery of the repo whose newest result is missing or `unknown`. The author's S6-P11 shows that only the two lost-result deliveries are reported. The `unknown` arm is untested (R3).

### 5. Behaviour changes to accepted 02-04 work (REPORT §3)

- The list in §3 is complete as far as I can see: the status field, the checkpoint fields, the reconnect refusal, and the stopRepo shape plus the possible 500. All of them except the 500 hide nothing and fail closed.
- I checked the accepted tests that changed:
  - The `status_reports_the_section_7_fields…` assertion was replaced by stricter ones: `[]`, `not_observable`, and the gap text plus the count.
  - `operations.rs` swaps one shape expectation and adds one for `RoleStatus`.
  - `roles.rs` keeps the old `fake` behaviour: the `checkpoint_reported` key was absent before and is `Null` now.
  - `delivery_e2e.rs` adds steps and only rewords one comment.
- No accepted assertion was weakened.
- OpenAPI does not list a 500 for stopRepo (M-1).

### 6. C-1 and C-2

- **C-1.** `checked_bounds` now also requires `f <= 120 && d <= 75`, and three unit cases above the defaults were added (S6-C1). `bounds()` is `pub`. It is called once after `listen` (`bin/ieo-coord.rs:90`) and logs the effective values whenever they differ from the defaults, and an ignored setting too. Fixed.
- **C-2.** My R1 (the reviewer's H5) is killed on both transports at the new assertion `delivery_e2e.rs:1078` (`409 ASSIGNMENT_USAGE_UNMEASURED`, a10 `unmeasured`). Fixed.

### 7. Ledger and docs

- Snapshot 0050 records the PR #77 merge `de1336c` and main CI 43821085760 success, and the review counts 3/4 and 2/3 (97ff0bc, 1548061, af68c93). All of this matches GitHub. The new event's evidence paths exist. `render --check` and the validator pass.
- Step-5 REPORT §13 gives C-1, C-2 and C-3 a disposition and an owner.
- REPORT §8: two rows have no real owner (L-4).

## Findings

### M-1 (Medium): stopRepo answers 500 after it has stopped the roles when the DB read fails, so the stop report is lost

- **Where:** `crates/ieo-coord/src/api.rs:625-640` (`roles::stop` …, then `let pending = run(db, …)?; if pending.status != 200 { return Ok(pending); }`). Also `contracts/ieo-api-v0.openapi.yaml`: the stopRepo responses list no 500.
- **Failure scenario:** the operator stops a repo while the coordinator's DB connection is broken. The DB may be restarting, or a statement may hit the 10 s bound under load. Both roles receive `POST /v0/role/stop` and stop, but the operator gets `500 {"code":"INTERNAL","message":"database error: db error"}`. The answer has no `roles`, no `unconfirmed` and no `safe_shutdown`, and nothing says the stops were sent. Reproduced: `r78-probe-stopdb.log`. A warm stop answered 200; after `pg_terminate_backend` on the test DB, the same stop answered 500 once `roles::stop` had run.
- **Why it matters:** before this PR, stopRepo was `NoDatabase`, so it worked and reported honestly without the DB. That is the degraded condition in which a stop is most likely to be used. A 500 on a side-effecting safety operation reads as "nothing happened". The caller loses the per-role safe-shutdown report, and it is not clear whether the roles stopped. REPORT §3 item 4 says only "500이 될 수 있다". It does not say that the stops have already happened.
- **Fix:** keep the stop answer, and fail closed only on the new part. On a non-200 from the read (or an `Err`), return `status` and `body` from `roles::stop` with:
  - `coordinator_pending_external_requests: "unreadable"` (or `"not_observed"`), and the DB error in a reason such as "coordinator: pending deliveries could not be read (…)";
  - `safe_shutdown_confirmed: false`.

  Add a test like my probe: kill the backend, stop, then assert 200, both roles reported, the reason present and `confirmed: false`. If you keep the 500 instead, the problem body must at least carry the roles report, and the OpenAPI and REPORT §3 must say that the stops were sent.

### L-1 (Low): a failed `add` is ignored, so the request is sent untracked, and start over an unreadable list is not documented

- **Where:** `crates/ieo-supervisor/src/serve.rs:1613` and `:1648` (`let _ = self.pending.add(...)`). The start path is at `serve.rs:750` and `:772` (`unwrap_or_default()` on `Err`).
- **Failure scenario:**
  1. The add fails while the file is still readable. This happens on ENOSPC or EACCES, or when something creates a directory named `/state/pending-requests.tmp`, which development code can do (same UID).
  2. The first-action report is then sent with no entry. If its answer is lost, the next reconnect sees `[]` and resumes over an unknown outcome. This is the case the step is meant to stop.
  3. Separately, a start over an unreadable list runs, and none of its requests can be tracked.
- REPORT §2.1 and contract item 18 say unconditionally that requests are added "보내기 전에".
- **Fix:** either refuse to send when the add fails (for a first action: keep it as a pending report in the status, so that a later run or the re-query sends it), or set a sticky in-memory "tracking failed" flag that `list()` / `report()` turn into `"unreadable"`, so that reconnect fails closed. Document in item 18 / §8 that start proceeds over an unreadable list.

### L-2 (Low): the same-UID limit is not stated for `pending-requests.json`

- **Where:** runtime contract revision 13 §16 item 18. REPORT §1 / §2.1.
- **Scenario:** development code (same UID, cwd on the same volume) deletes or edits the list. A reconnect then resumes over a pending first-action report. Alternatively, it plants a first-action `detail.body`, which the supervisor re-sends with its claim credential during the re-query.
- **Impact:** limited. §2.1 already assumes that the model can read that credential, and first actions are claims. But the step's guarantee ("reconnect never resumes over a request whose outcome is not known") holds only against the supervisor itself, not against the role's code. This is the same N5 caveat that §2.1 states for the checkpoint.
- **Fix:** add one sentence to item 18 and REPORT §8: the list is on the role volume, development code can change it, and it is not an integrity guarantee. The final resume basis is the coordinator record and step 7's GitHub comparison.

### L-3 (Low): three claimed behaviours have no test (R2, R3 and R6 survive)

- **R2** (`serve.rs` `first_action_tracked`, the 4xx arm). Without it, a 4xx-refused first action stays pending, and the re-query gets 4xx again forever. Every reconnect of the role would then be blocked permanently. Test: make the fake answer 409 `EVENT_CONTENT_CONFLICT` (or `CHECKPOINT_MISMATCH`), then assert that the list is `[]` and that a reconnect resumes.
- **R3** (`api.rs:729`, `Some("unknown")`). Contract item 20 and REPORT §1 say that `unknown` deliveries are reported, but only "no result recorded" is exercised. Test: in `limits_e2e` or `delivery_e2e`, stop the repo while a delivery's newest result is `unknown` (the lost-answer path already exists in `delivery_e2e`), and assert `state: "result unknown"`.
- **R6** (`serve.rs:1636`, the `LocalOnly` early return). REPORT §2.1 says "local_only … 목록이 생기지 않는다", but nothing checks it. Without the guard, every local-only reconnect after a run that showed a first action would be a 409. Test: a local-only run with a tool use, then assert the status list is `[]` and that a reconnect resumes.

### L-4 (Low): two REPORT §8 carry-overs have no owner

- **Where:** `docs/poc/reports/tasks/02-05/step6-external-20261116/REPORT.md` §8. The "진행 보고·재현 요청" row says "그 경로를 만드는 작업". The "start의 목록" row says "관찰".
- **Fix:**
  - Name the task. Per runtime contract §2.1, the repro path's API is 01-08 and its executor is 02-06. Name the task that adds supervisor-mediated progress reports. If none is planned, say so and give 02-05 step 9 (the acceptance report) as the owner of the decision.
  - For "start의 목록", give 02-05 step 9 (or the user decision for the acceptance) as the owner instead of "관찰".

### I-1 (Info): `not_observable` is not a stop reason

`roles.rs:180` checks only `active_children` and `pending_external_requests`. When 04-05 tracks children, a stop could become "confirmed" even though the model's own network requests cannot be seen. This is documented and owned by 04-05 (REPORT §7 / §8, contract item 19). Keep it on 04-05's entry list.

### I-2 (Info): stale comment

`crates/ieo-coord/src/api.rs:610` still says "repo-scope role control (§7): no database". That is only true for RoleStatus now.

### I-3 (Info): R5 is near-equivalent

The 4xx arm of `report_tracked` only affects how the status looks until the next compare, which clears checkpoint entries anyway. It needs no test, but REPORT §1 might say that the compare is the real settling step for checkpoint entries.

### I-4 (Info): one flake of a step-5 assertion

During one targeted run (`-p ieo-coord --test limits_e2e --test roles`, unmodified code; the mutation was not applied because I passed a wrong path), `assignment_limits_bound_and_stop_deliveries` (HTTP) failed at `limits_e2e.rs:948`: `settled("aL")["usage"]["run"]["state"]` was `"unmeasured"`, but `"measured"` was expected. That code is from step 5, not this PR. The full workspace run, the R3, R4 and mutation runs, and a dedicated re-run all passed. It looks timing-dependent: the ledger record for aL is read right after `role.wait("aL")`. Worth watching in CI. It does not block this PR.

## Summary

Step 6 does what PLAN §3.2 step 6 and user decision item 3 ask for:

- The supervisor's own coordinator requests are listed on the role volume, added before the send and cleared on a known answer or by the re-query.
- Reconnect stops on a non-empty or unreadable list.
- `not_observable` is reported separately.
- stopRepo lists the coordinator's deliveries whose outcome is not known, and the stop stays unconfirmed while children are untracked.
- C-1 and C-2 are fixed, and H5 is now killed.
- The tests pass (384), the docs and ledger match GitHub, and OpenAPI 0.14.0 is additive.

One change blocks approval. **M-1:** the new DB read in stopRepo turns a stop that was already carried out into a bare 500, so the safety report is lost exactly when the DB is degraded. Return the stop report with a fail-closed reason instead.

The Low items are:

- L-1: a silent add failure, and start over an unreadable list is undocumented.
- L-2: the same-UID caveat for the list file.
- L-3: tests for the first-action 4xx path, `result unknown`, and local-only.
- L-4: REPORT §8 owners.

They should be handled in the same correction, or recorded with owners.
