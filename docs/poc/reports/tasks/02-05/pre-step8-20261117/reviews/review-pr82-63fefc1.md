# Review of PR #82 at 63fefc1 (review 1)

Reviewer: independent (did not write the PR). Head `63fefc1a5fb21494a9c9a39c7f662b8815dad828`, base main `0a3d2a3`. Sources read: `docs/poc/reports/tasks/02-05/pre-step8-20261117/REPORT.md`, `docs/poc/reports/tasks/02-05/budget-0205-20261117/reviews/review-pr81-291a9b0.md` (N-1..N-3), `docs/poc/reports/tasks/02-05/step4-chain-20261116/reviews/review-pr75-debf413.md` (I-8), `docs/poc/User_Decisions_2026-11-17_spare.md` (§2, §4), `docs/poc/reports/tasks/02-05/plan-20261115/PLAN.md` §4, the code diff, runtime contract revision 18, OpenAPI 0.17.1, and `docs/poc/progress/state.json` snapshot 0054.

## Verdict

**Request changes (one Medium, documentation only).** The code is correct. N-1, N-2, N-3 and I-8 are fixed as described, the tests are real, and the full suite, fmt, clippy, the validator and `render --check` all pass. Ledger 0054 matches GitHub. The Medium is the step-8 readiness checklist (REPORT §6), which this PR offers as the last gate before real, budgeted calls. It is accurate as far as it goes. But it leaves out several conditions that, if missed, would burn the one spare, leave a run unmeasured (which the spare cannot release), or let something other than the coordinator start a 02-05 run. These include one behavior the new e2e itself shows: without `new_session: true`, a spare is a **reconnect into the failed R1 session**, not a fresh R1. Every fix is a few added lines in REPORT §6. No code change is needed.

## What I ran

All runs were in the detached worktree `.../scratchpad/r82` with `CARGO_TARGET_DIR=.../target-x08`, `RUST_TEST_THREADS=4`, `-j 4`. I ran `cargo clean -p` on the 7 workspace crates first. Logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --all --check` | OK | `r82-fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | OK | `r82-clippy.log` |
| `cargo test --workspace --locked` (test DB 55432) | **407 passed, 0 failed, 0 ignored** (first line of log = 63fefc1) | `r82-build.log`, `r82-test.log` |
| `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml` | OK. The 0.17.0 → 0.17.1 diff changes only `info.version` and description text: additive, no schema change | `r82-openapi.log` |
| `docs/poc/scripts/render_progress.py --check` | `ok: true` (5 files) | `r82-render.log` |
| GitHub: `gh pr view 81`, `gh run view 43985814742`, `gh pr view 82` | PR #81 MERGED, head 291a9b0 → merge `0a3d2a39`; run 43985814742 `ci`/push/main at `0a3d2a39`, **success**; PR #82 OPEN at 63fefc1 | `r82-gh.log` |
| 2 own hand mutations (+1 probe) | 1 killed, 1 survived (→ L-1); the probe confirmed the spare op (→ M-1 b) | `r82-mutants.log`, `r82-probe.log` |
| `git diff 778ee47 HEAD -- crates/ieo-coord/src/schema.sql`, per-merge `-` lines | see §4 | — |

Each mutation was applied with `sed`/`perl`, tested, and reverted with `git checkout`. `git status` was clean after each. One mutation run panicked before its own cleanup and left `ieo_t_spare_2065331`. I dropped it, and `ieo_t_*` count is now 0. Two `/tmp/ieo-spare-e2e-{2002474,2004550}` directories from earlier, non-mine runs remain; I did not touch them. I made no model calls and did not touch 55433, 58090, the secrets/runs directories, PR #29 or other worktrees.

## Analysis

### 1. N-1: `spare_e2e` (person → deliverAssignment → 02-05 → role supervisor)

`crates/ieo-coord/tests/spare_e2e.rs` spawns the real coordinator binary against a real PostgreSQL database and puts the real `Service` behind HTTP with a fake `claude`. The role config is `budget: "02-05"` with no share, and the coordinator's `budget_plan` is `02-05`. The test steps:

- **R1 FAIL.** R1 runs with `FAIL`, which gives an `is_error` result. The record has `stop_required: true`.
- **No marker.** A delivery without a marker gets `budget_refused` "asks for a person".
- **Service spare.** A service (`operator`) spare gets 403 `HUMAN_APPROVAL_REQUIRED`, and the delivery row count is unchanged.
- **Person's spare.** The person's (`human-op`, `kind: user`) spare is accepted. The record's `spare` has reason, `replaces: r1`, `evidence_ref`, `by {name, kind: user, via: deliverAssignment}` and `approved_at` ending in `Z`. The spare record has `stop_required: false`. R1's record is still exactly one record and byte-identical (`r1s == vec![r1]`).
- **Replay.** The same key and body again gets 200 with an equal stored answer. Delivery count and ledger line count are unchanged.
- **Second spare.** Another key gets `budget_refused` "used already".
- **R2 and after.** R2 is accepted as the third development call. The next call gets "planned runs are used". The ledger holds `[r1, sp, r2]`.

This kills H5. If the raw body `spare` were passed, it would have no `by`/`approved_at`, so `spare_gate` would refuse and step 4 would fail. My own mutation R82-A removed the `kind != User` 403 check in `deliver` (`crates/ieo-coord/src/delivery.rs:470`). Step 3 then failed (`spare_e2e.rs:474`): **killed**. This matters because `spare_extra` writes `"kind": "user"` unconditionally (`delivery.rs:141`). The 403 is therefore the only guard on that path, and the e2e now pins it (I-1).

Probe (`r82-probe.log`): the spare delivery was `decision: resume_allowed, op: reconnect`. Its record has `session {mode: resume, resume_base_attempt: r1, session_id: s1}`. The body's `session_id: s3` is not used. The e2e is valid for the budget path, but it does not test the "redo R1 as a fresh run" reading (see M-1 b).

### 2. N-2: stop-reason guard; P7; N-3

- I enumerated every place that writes `stop_required: true` / `stop_reason` in the workspace. In `caller.rs` these are `record_lost` (`:1163`) and `After::finish` (`:1446`). `effective` only clears. `roles.rs` `reasons` belong to another structure (stop/status of the services) and never reach a ledger record. In `After::finish`, every non-result reason goes through `other_stop_reason`: cleanup, env, unmeasured, both "this run's signal" branches, and `record_lost`. The two result reasons are built from the shared constants, and `"; "` is flattened.
- Soundness. `replace("; ", ", ")` cannot create a new `"; "`, because the replacement starts with `,`. A reason ending in `;` joins as `x;; y`, which splits into `x;` and `y`, so the non-result fragment `x;` still blocks. After the guard, a non-result reason can no longer start with `result ` or equal `no result event`. `split("; ")` therefore cannot produce a releasable fragment from a non-result reason. **No non-result stop reason can be released by a spare.** The guard is opt-in by convention rather than by type. A future reason pushed without `other_stop_reason` would bypass it. The doc comment says so (I-2).
- P7 (cleanup reason not routed through the guard) is truly equivalent. The constant `"process-group cleanup failed"` neither starts like a result reason nor contains `"; "`, so the output is identical.
- Behaviour change, REPORT §3 item 2. `remeasure` requires `starts_with("usage unmeasured") && !contains("; ")`. An unmeasured reason whose external text held `"; "` used to be rejected. It is now flattened and accepted as "unmeasured only". That is the intended effect, and REPORT §3 lists it.
- N-3 wording in the contract §19 item 30 and OpenAPI 0.17.1 matches the code (`approved_at = ieo_evidence::time::now()` in `spare_extra`, the coordinator's request-handling time).

### 3. I-8: `recordFirstAction` requires the takeover linked to the delivery that called the attempt

- `takeover_linked` (`crates/ieo-coord/src/api.rs:4424`) is the previous fill query moved into a function. It joins `role_takeovers` → `role_deliveries` on `delivery_id` and matches repo, role, session, assignment, generation and `d.attempt_id`. Both `record_first_action` and `fill_first_action` now call it. Fail-closed: a false result gives 409 `CHECKPOINT_MISMATCH` before any insert, and the internal contract order matches.
- The supervisor sends exactly the delivered `attempt_id` and `session_id` (`serve.rs:1108-1110` → probe body). A 409 is a 4xx, so it is removed from pending (`serve.rs:1792-1797`), and "a failed report never stops the run". The after_run fill uses the same condition. The behaviour change therefore cannot stop or cost a real run. REPORT §3 item 1 lists it.
- Fixture changes in `crates/ieo-coord/tests/grpc.rs`. The `att(seq)` helper is the fixture delivery's attempt. A new assertion checks that the undelivered `att-1` gets 409 after the takeover. R4 moved: it now uses the delivered `att(3)` with another session `s2` and still expects 409, which is stricter because only the session differs. The "second run of the same session" case moved before the generation change and now uses `att(3)`, still 201 with `current_assignment: false`. I found no weakened assertion.
- My mutation R82-B dropped `t.generation = $5` from `takeover_linked`. **It survived** the whole `ieo-coord` suite. The predicate is not redundant. After generation 1 is issued, a report naming `(A1, 1)` for an attempt delivered under generation 0 passes `issued_here` and would be stored under the wrong generation. See L-1.

### 4. Step-8 checklist (REPORT §6)

Schema 778ee47..HEAD (`+319/−1`). The per-merge `-` lines show only one in-place change, the `evidence.kind` CHECK (PR #79). Every other change is a new `CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, `CREATE OR REPLACE TRIGGER` or `DROP CONSTRAINT IF EXISTS`. The REPORT's list is **complete**: 11 tables, 4 columns, 2 constraint changes, triggers.

The migration is **non-destructive and safe on existing data**:
- `serve` does run `migrate` (one `batch_execute`, i.e. one implicit transaction) before `check_bindings` (`crates/ieo-coord/src/bin/ieo-coord.rs:80-85`).
- The only new constraint on a pre-existing table is the re-added `evidence_kind_check`. It is a superset of the old list, so existing rows validate.
- `assignments.limits` is nullable.
- The NOT NULL DEFAULT columns are on tables that did not exist at 778ee47.
- No UNIQUE constraint or FK is added to an existing table.
- `CREATE OR REPLACE TRIGGER` was already in the 778ee47 schema, so the PG ≥ 14 requirement is already met.

One assumption is implicit: the persistent DB was only ever migrated by main builds. A PR-branch build pointed at 55433 could have created an older shape that `IF NOT EXISTS` would keep. A read-only table list before redeploy settles this (part of M-1).

| # | Item (REPORT §6) | Verified against | Gap |
| --- | --- | --- | --- |
| 1 | Redeploy `ieo-role-call`/`ieo-coord` (58090) and images at the merge SHA | PLAN §4.2; RESUME says 58090 runs 778ee47 | Correct. **Missing**: no coordinator restart or redeploy while a run is in flight (M-1 a) |
| 2 | Schema migration list, auto-migrate, non-destructive | `schema.sql` diff, `bin/ieo-coord.rs:82` | Correct and complete. Add a read-only pre-check that none of the new tables already exists in 55433, or that any that exists matches |
| 3 | Bindings / sprint ownership read-only comparison (`check_bindings`) | `api.rs` `check_bindings`, startup order | Correct (a failure stops the start, no cost) |
| 4 | No other assignment's central checkpoint for the repo/role | budget REPORT §4/§8 | Correct |
| 5 | Fresh role volumes (supervisor backstop counts the volume's attempts) | `serve.rs:1579-1592` `budget_backstop` | Correct. Note that the backstop also counts **granted windows**: 3 × 600 s = 1,800 s exactly, so `max_window_seconds` must not be raised above 600 |
| 6 | Config names budget `02-05`, new ledger dir, `first_observation` | `budget::named`, `Plan::from_json` | **Incomplete** (M-1 c): `first_observation` must be ≤ 7 days old at R1 (`OBSERVATION_MAX_AGE_SECONDS`), else the call is held. The stream and `ledger_dir` must be readable and writable by the **coordinator process** (container mount) and persist across redeploys |
| 7 | Human credential (`kind: user`, `roles:control`) | `delivery.rs:470`, contract §19-30 | Correct. **Missing**: the supervisors' own `agent` credentials (`checkpoint:report`, role-bound) in the coordinator's `tokens.json`, and who else holds `roles:control` (any holder can start R1/R2/G1) |
| 8 | `branch_head` evidence = `target_sha` for R2 | `api.rs` `github_judgment`/`github_head_check` | Correct. Add that the development assignment **must carry `branch_ref`**, else R2 is `not_observed` |
| 9 | G1 only while its target is the current development `target_sha` | `api.rs` `github_gate` (`stale_target`) | Correct |
| 10 | USD 0; 02-04 / X-02 ledgers unused | decisions 2026-11-15 / 11-17 | Correct. Add "no `ieo-role-call` against the step-8 role services" (M-1 d) |
| — | What R1/R2/G1 send | `delivery.rs` `choose_op`, `serve.rs` | Correct for R1/R2/G1. **Missing: what the spare sends** (M-1 b) |

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| M-1 | Medium | `docs/poc/reports/tasks/02-05/pre-step8-20261117/REPORT.md:70-90` (§6) | The checklist is the stated last gate before budgeted calls, but it omits conditions that would fail a run, burn the one spare, or leave an unmeasured stop that the spare cannot release (`spare_releasable`), or that leave a start path other than the coordinator. See (a)–(e) below the table | Add (a)–(e) to §6: items, evidence, read-only checks. Documentation only |
| L-1 | Low | `crates/ieo-coord/src/api.rs:4438`; `crates/ieo-coord/tests/grpc.rs:1901-1923` | Mutation R82-B (drop `t.generation = $5`) survives all `ieo-coord` tests. After generation 1, a direct report `(A1, 1, att(1), s1)` for an attempt delivered under generation 0 would be stored under generation 1. The code is correct; the gap is in the tests | Add one assertion after the new generation: `write(&fa("A1", 1, &att(1), "s1"))` → 409 `CHECKPOINT_MISMATCH`. Carry-over acceptable |
| I-1 | Info | `crates/ieo-coord/src/delivery.rs:141` | `spare_extra` writes `by.kind: "user"` regardless of the caller. On the coordinator path, `spare_gate`'s `by.kind == "user"` check is therefore not an independent second check. The 403 at `:470` is the only guard. It is tested now (R82-A killed) | Optional: write `who.kind` there so the gate checks it too |
| I-2 | Info | `crates/ieo-supervisor/src/caller.rs:1368-1427` | The N-2 guard is a convention: `reasons` is a `Vec<String>`. A future reason pushed without `other_stop_reason` bypasses it. It is fail-closed only if it happens not to start with `result ` | Optional: push through a small enum or helper so a raw `String` cannot be added |
| I-3 | Info | `crates/ieo-coord/tests/spare_e2e.rs` | The e2e covers R1-fails → spare (as a reconnect) → R2. It does not cover a spare with `new_session: true` (start), or R2-fails → spare, through the coordinator. Both are covered at caller level (`tests/caller.rs:2416`, `:2559`, `:2704`), which meets decision §4.4 | None required |

### M-1 gaps

- **(a) No coordinator restart during a run.** An accepted run is finished, measured and written to the 02-05 ledger by a thread inside the coordinator process (`delivery.rs:797-811`). The SIGTERM handler only unblocks HTTP workers (`bin/ieo-coord.rs:100-118`) and does not wait for that thread. Restarting 58090 (redeploy, `podman restart`) mid-run leaves `inflight.json` and no record. That stops the budget, and the assignment refuses the next delivery with `ASSIGNMENT_USAGE_UNMEASURED`. The model call has happened but has no record, and the spare cannot release it.
- **(b) The spare's shape.** After a failed R1 whose last checkpoint is `after_run`, a spare without `new_session: true` is a **reconnect** that resumes R1's session. The probe shows `op: reconnect`, `session.mode: resume`, `session_id: s1`, and the body's `session_id` is ignored. If R1 failed before a session existed (e.g. an authentication or environment defect), `--resume` fails and the spare is spent. After an `error` checkpoint the judgment is not `resume_allowed`, so the spare is `not_called` and needs `new_session`/`recovery_id`. §6 "what R1/R2/G1 send" has no spare line. It should state when to send `new_session: true`, `recovery_id`, or a plain reconnect, and that the person checks the R1 failure phase first.
- **(c) Paths, freshness and limits.**
  - `first_observation` must be ≤ 7 days old at R1, otherwise the call is held.
  - The `ledger_dir` and observation stream must be reachable from the coordinator process and persist across redeploys.
  - The development assignment needs `branch_ref`.
  - The sprint revision's / assignment's `limits` must leave room for the 600 s / 150k plan. Otherwise the run window or token room is cut, and a guard stop gives an unmeasured or limit stop that the spare cannot release.
  - `max_window_seconds` ≤ 600 (see item 5).
  - Keep the R1/R2 prompts small. A guard stop is not spare-releasable.
- **(d) The coordinator as the only start path.** Both role services must use `checkpoint_central` → the coordinator, not `local_only`. With central checkpoints, a start without a delivery has its takeover refused and nothing runs (`serve.rs:1192-1199`). With `local_only`, anyone holding the adapter token, including `ieo-role-call` with its own plan or ledger, could start 02-05 runs on a second ledger, bypassing the spare rule. Only the volume backstop would then limit them. Add: the supervisors' `agent` credentials exist in `tokens.json`; list the `roles:control` holders; no `ieo-role-call` against these services.
- **(e) A pre-existing schema shape.** Before redeploy, check read-only that none of the 11 new tables already exists in 55433. A table created by a non-main build would be kept by `IF NOT EXISTS`.

## Summary

- **Must fix before approval:** M-1, the step-8 checklist additions (a)–(e), in REPORT §6 (documentation only; the state.json `next_action` already routes the checklist to the user before step 8).
- **Carry-over acceptable:** L-1 (one generation-mismatch assertion for `recordFirstAction`).
- **Informational:** I-1 (`spare_extra` hard-codes `kind: user`), I-2 (the N-2 guard is a convention, not a type), I-3 (spare start/R2-failure paths are e2e-uncovered but caller-tested).
- **Confirmed fixed:**
  - N-1: H5 is killed, and my R82-A is killed.
  - N-2: sound. P7 is truly equivalent. The `remeasure` effect is listed.
  - N-3: wording is correct.
  - I-8: fail-closed, the fixtures are stricter, and the behaviour change is listed. It cannot stop a real run.
- **Also confirmed:**
  - OpenAPI 0.17.1 is additive.
  - Runtime contract revision 18 matches the code.
  - Ledger 0054 matches GitHub: PR #81 merged at `0a3d2a3` and CI 43985814742 succeeded.
  - `render --check` is OK; 407/407 tests pass; fmt, clippy and the validator are clean.
