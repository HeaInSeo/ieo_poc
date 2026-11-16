# Review of PR #80 at 185d12b (review 1)

Reviewer: independent (did not write the PR). Head `185d12b8cd010c98161d766b827055a911ce0404`, base main `78d7fe5`. Binding spec: `docs/poc/User_Decisions_2026-11-16_recovery.md` §2.

## Verdict

**Request changes.** One Medium finding (M-1): a single use of an approval that never produced a run blocks every later approval of the same checkpoint, so the role cannot be recovered through the official path. The report and runtime contract item 27 say that "a new approval" fixes this case. That is not true; my probe shows it. Safety holds: no path I tried started two runs, and the human-only gate, the holds, the re-check at use and the budget checks behave as stated. Four Low findings and four Info notes follow.

## What I ran

All logs are in the scratchpad directory `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Log | Content | Result |
| --- | --- | --- |
| `r80-lint.log` | `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -D warnings` | pass |
| `r80-test.log` | `cargo test --workspace --locked` at 185d12b (test DB 55432, `RUST_TEST_THREADS=4`) | **395 passed, 0 failed, 0 ignored** (same as the report) |
| `r80-probe-stuck.log` | Own probe (`r80-probe_r80.rs.txt`, a copy of `recovery_e2e` with a branch after step 8): the supervisor refuses the start after the use is committed. A second approval of the same checkpoint is then delivered | **M-1 confirmed** (`PROBE-CONFIRMED`) |
| `r80-probe-race.log` | Own concurrency probe: 4 deliveries with the same `recovery_id` at once, run 3 times | One run each time: 1 accepted, 1 use. The other three are held ("delivery(ies) … no known outcome") |
| `r80-mutants.log` | Own hand mutations (below), reverted after each run; `git status` clean apart from the probe file | M0 (sanity) and M4 killed; M1, M2, M3 and M5 **survive** |
| `r80-docs.log` | `render_progress.py --check`, `validate_package.py`, `validate_contracts.py`, `openapi-spec-validator`, relative links in the changed `.md` files | all OK, 0 broken links |
| GitHub (read only) | `gh pr view 79`, `gh run view 43895157928`, `gh run view 43898119170` | PR #79 merged at `78d7fe5` from head `47e4e3f`. Run 43895157928 (pull_request, attempt 2) succeeded at `47e4e3f`. Run 43898119170 (push) succeeded at `78d7fe5`. Ledger 0052 and the rendered views agree |

Hand mutations:

| id | Mutation | Tests | Result |
| --- | --- | --- | --- |
| M0 | Sanity: rename the `RECOVERY_RUNNING` code | `recovery_e2e` | killed |
| M1 (B12) | `record_use` treats a conflict (`n == 0`) as success | `recovery_e2e` plus the race probe | **survives**. The use check in `usable`, the pending-delivery hold and the budget lock already serialise this (Info I-1) |
| M2 | Drop the guardrail "review target = development target" check (`recovery.rs:156`) | `recovery_e2e` | **survives** (L-3) |
| M3 | Run the recovery re-check only in the first judgment and reuse its result in the judgment under the budget lock (`delivery.rs:518`) | `recovery_e2e` | **survives** (L-3) |
| M4 | Supervisor asks for `recovery_id` only over `takeover`, not `error` (`serve.rs:833`) | `serve a_start_over…` | killed |
| M5 | Drop the digest half of the "local checkpoint ≠ central" hold (`recovery.rs:197-198`) | `recovery_e2e` | **survives** (L-3) |

No model calls and no real `claude`. I did not touch 55433, 58090, PR #29, the secrets or the runs directories, and I pushed and commented nothing. Every test database I created was dropped on its own exit path: the probe drops its database before its assert, and the mutant runs all passed. `psql` is not installed here, so I could not list leftover databases.

## Per-condition table

| # | Condition (decision §2) | Verdict | Evidence |
| --- | --- | --- | --- |
| 1 | Human-only approval: `roles:control` and principal `kind == user`; service and agent (the role's own included) refused; no self-approval | **Met**, limit stated honestly | `recovery.rs:46-64` (`need_client_scope` refuses agents first, then the repo check, then `kind != User` → 403 `HUMAN_APPROVAL_REQUIRED`). The DB enforces `approver_kind = 'user'` (`schema.sql`). `kind` comes from the operator's tokens file (`auth.rs:109`); a token holder cannot choose it. E2E step 3 covers service, supervisor, dev-agent, no scope and other repo. REPORT §7 and §8 state that `kind` is a declaration in the credential file (single token, no MFA). Self-approval is impossible because the role's credentials are `agent` |
| 2 | Central record (approver, reason, time, assignment, generation, checkpoint seq/digest/phase, GitHub comparison), append-only | **Met** | `role_recoveries` and `role_recovery_uses` with `forbid_change` triggers (`schema.sql:552-596`). E2E step 6 checks every field. The use row keeps the GitHub check at use and `head_at_approval` |
| 3 | The next delivery starts a NEW session only for the approved target; never a resume; distinguishable | **Met** | `choose_op`: `recovery_refused` → None and `recovery_allowed` → `start` (`delivery.rs:784-792`). Records show the decision `recovery_allowed`, the delivery's `recovery`, the chain's `recoveries` and the checkpoint's `recovery_id`. E2E step 9 shows `mode: new` |
| 4 | Reuse of `github_head_check`; trusted-input statement | **Met** | `recovery.rs:244` calls the step 7 function. `source` says "evidence:ingest (trusted service input; not a live GitHub read)", asserted in step 5. Same statement in REPORT §8 and contract §18 |
| 5 | Running → refuse; unconfirmable end, external requests or unknown deliveries → held with evidence, never usable | **Met** (one hold is untested, L-3) | `RECOVERY_RUNNING` (`recovery.rs:187-196`, step 1). Holds: not observed, local ≠ central, pending list non-empty or unreadable (`is_none_or`), unknown deliveries, GitHub ≠ match. Step 5 covers four of them; local ≠ central is untested (M5 survives). Step 5 also shows a hold is never used |
| 6 | Re-check at use if the checkpoint, generation or head changed (incl. under the budget lock) | **Met in code; the budget-lock part is untested** | `usable` re-runs `assess` in both judgments (`delivery.rs:518-555`, the second at `:633`). Step 7 shows a head change after approval leads to `not_called` and the approval stays unused. M3 (no re-check under the lock) survives (L-3) |
| 7 | One approval → at most one execution | **Met for safety; liveness broken (M-1)** | The use is committed before the start (`delivery.rs:714-735`), with PK per approval and UNIQUE per checkpoint. Step 10 (duplicate and other delivery), step 11 (lost answer, re-query, coordinator and supervisor restart) and my race probe (3×4 concurrent: one run) all hold. But a use that ran nothing consumes the checkpoint for good (M-1) |
| 8 | Usage and unmeasured kept; no budget reset; budget checks apply | **Met** | The approval touches no ledger or usage (step 6 usage before = after). An unmeasured record → `budget_refused` and unused (step 8). After a lost answer → 409 `ASSIGNMENT_USAGE_UNMEASURED` (step 11) |
| 9 | Supervisor start over an unfinished checkpoint needs `recovery_id` (incl. `local_only`); can the supervisor verify it in coordinator mode? Accepted-test changes complete and not weakened | **Partly met** | `serve.rs:830-844` requires the id, and the `local_only` test passes. The supervisor **cannot** verify the id in coordinator mode, and the coordinator does not check a takeover's `recovery_id` against a recorded use either (L-1). The accepted supervisor tests were strengthened, not weakened: the 409 is asserted first and `BUDGET_EXCEEDED` still holds with an id. But the shipped 02-04 host caller `ieo-role-call` cannot send `recovery_id`, which REPORT §3 does not say (L-2) |
| 10 | gRPC transport slack (cap + 1 s) | **Correct** | `grpc.rs:643`. Without a client deadline the work deadline is cap − 50 ms (`grpc.rs:396-405`) and the layer is cap + 1 s, so the server's `DEADLINE_EXCEEDED` always wins. With a client deadline the layer is still `min(grpc-timeout, cap+1s)`, so client deadlines still cut. A hung handler without a client deadline is cut 1 s later than before: bounded, not masked. The test now also asserts "rolled back". The internal contract §6 is updated (I-4) |
| 11 | Tests: normal, permission, running, mismatch, head changed, duplicate, restart/lost answer; ≥3 own mutations incl. B12-like | **Present and meaningful, with gaps** | All seven scenarios are in `recovery_e2e` steps 1-11. Six own mutations, one of them B12-like (M1) with a concurrency probe. Gaps: guardrail, re-check under the budget lock, local ≠ central (L-3); a supervisor refusal or lost connection after the use (M-1) |
| 12 | Ledger and docs vs GitHub; render `--check`; validator; links | **Met** | See "What I ran" |

## Findings

### M-1 (Medium): a use that ran nothing makes the checkpoint unrecoverable for good; the docs claim a new approval works

- **Where:**
  - `crates/ieo-coord/src/schema.sql:589`: `UNIQUE (repo_id, role, checkpoint_seq)` on `role_recovery_uses`.
  - `crates/ieo-coord/src/recovery.rs:521-558`: `record_use`.
  - `crates/ieo-coord/src/recovery.rs:86-267`: `assess` never looks at earlier uses.
  - `crates/ieo-coord/src/delivery.rs:714-735`: the use is committed before the start.
  - Docs: `Runtime_Contract_v0.md:295` (item 27, "닫는 쪽, 새 승인 필요"); REPORT §7 ("다시 하려면 새 승인이 필요하다") and §8 ("새 승인 필요").
- **Failure scenario:** the use row is committed, then nothing runs. This happens when:
  - the supervisor refuses the start (409 `ID_CONTENT_CONFLICT`, `EXTERNAL_RECONCILIATION_REQUIRED` from its pending-list probe or central compare, `ALREADY_RUNNING`, …);
  - the connection fails because the supervisor container is restarting, which is likely right after a crash;
  - the coordinator stops between the use commit and the send.

  The latest checkpoint is then unchanged, at seq N. A person approves again. `assess` passes and the approval is recorded as **`approved`**. But its use always fails on the per-checkpoint unique key, so the delivery is `not_called` with "this approval (or another approval of its checkpoint) was used already". The checkpoint never changes again, so no approval of it can ever be used. A new assignment does not help: the crashed checkpoint belongs to the old assignment (`assess` refuses), and compare keeps answering external reconciliation. The supervisor still requires a recovery for a start. Both tables are append-only, so the only way out is DB surgery past the triggers.
- **Reproduced** (`r80-probe-stuck.log`): an `a6` attempt directory existed, so the start was refused with `ID_CONTENT_CONFLICT`, after the use was recorded. The re-approval returned 201 `approved` with `used=null`. The second delivery got `not_called` with "…was used already".
- **Fix:**
  - Treat a use whose delivery's known outcome ran nothing (`refused` or `not_called`, or a re-query that confirms the attempt is unknown to the supervisor) as no longer consuming the checkpoint.
  - For example, drop the per-checkpoint UNIQUE. Under the recovery lock, refuse or hold a new approval or use when another use of the same checkpoint has a delivery outcome of `accepted`, `unknown` or no result yet (`needs_confirmation`, with that delivery as evidence). Allow it when that outcome is `refused` or `not_called`.
  - Also make `approve` report an already-consumed checkpoint instead of recording a dead `approved` row.
  - Add an E2E step: use, then supervisor refusal, then a new approval, then exactly one run.
  - Correct item 27 and REPORT §7 and §8 to say what actually happens.

### L-1 (Low): in coordinator mode nothing verifies the `recovery_id` a start carries; contract item 28 overstates

- **Where:**
  - `crates/ieo-supervisor/src/serve.rs:712-722` and `:830-844`: any `safe_id` string is accepted, and the comment says "the coordinator checked it".
  - `crates/ieo-coord/src/api.rs:4141-4152`: a takeover's checkpoint report is checked against its delivery but not against `role_recovery_uses`.
  - `Runtime_Contract_v0.md:296` (item 28: "코디네이터 모드에서는 코디네이터가 승인을 대조한 뒤에만 그 id가 온다").
- **Scenario:** a holder of the adapter credential sends a direct start over a takeover or error checkpoint with `recovery_id: "R-x"`. To get the takeover accepted, the start also names a delivery of the current generation whose outcome is `accepted`, `unknown` or not yet recorded, with that delivery's attempt id (`check_delivery`). An example is a delivery whose answer was lost before the supervisor ever saw it. The new session is accepted centrally without any approval. The adapter credential is inside the trust boundary and this window existed before the PR, so the severity is Low. Still, the contract sentence describes a guarantee that only the delivery path gives.
- **Fix:** when a `takeover` report follows a central checkpoint at `takeover` or `error`, require `checkpoint.recovery_id` to equal the `role_recovery_uses.recovery_id` recorded for that `delivery_id`; otherwise refuse with 409. Alternatively, reword item 28 and the `serve.rs` comment to say this is the delivery path's guarantee only.

### L-2 (Low): the 02-04 host caller `ieo-role-call` cannot send `recovery_id`; the behaviour-change list is incomplete

- **Where:** `crates/ieo-supervisor/src/bin/ieo-role-call.rs:82-92` (`caller::call` with no extra keys); REPORT §3 item 1.
- **Scenario:** the accepted 02-04 recovery after a crash (a person starts a new session) now gets 409 `EXTERNAL_RECONCILIATION_REQUIRED`. The shipped CLI has no way to add the id, so a `local_only` operator has to hand-craft the HTTP body, which also bypasses the CLI's budget path. REPORT §3 says the path "now has to send `recovery_id`" without saying the tool cannot.
- **Fix:** add `--recovery-id` to `ieo-role-call` (passed through `start_with` extras) with a test, or state in REPORT §3 and contract item 28 that the 02-04 CLI recovery is unavailable until then.

### L-3 (Low): three stated guarantees have no test (surviving mutants M2, M3, M5)

- **Where:** `recovery.rs:155-175` (guardrail target), `delivery.rs:633` (re-check under the budget lock, which contract item 27 says happens "예산 잠금 뒤 포함"), `recovery.rs:197-205` (local ≠ central hold, listed in contract item 26).
- **Scenario:** any of these checks can regress unnoticed. The guardrail gap is admitted in REPORT §8; the other two are not.
- **Fix:**
  - Change the head between the two judgments. For example, hold the budget lock (as `limits_e2e` does) and ingest a new head meanwhile, then assert `not_called` and that the approval stays unused.
  - Approve with a local checkpoint whose digest differs from the central one, and assert a hold.
  - Add a guardrail recovery step, or accept the gap explicitly.

### I-1 (Info): B12 is redundant, not unguarded

M1 survives both the E2E test and the race probe. In 3 runs of 4 concurrent deliveries, one ran and the others held each other through the "no known outcome" rule, and the budget lock serialises the rest. The DB unique constraint is a third layer. Nothing to do, but REPORT §5 could cite this probe instead of leaving B12 "not run".

### I-2 (Info): a concurrent retry with the same `Idempotency-Key` can get a 500

`recovery.rs:366-382` and `:387-431`: the replay check and the insert run in separate transactions. Two simultaneous identical requests both see "no row", so the second insert hits the primary key and the transaction error is answered as a DB error, not the stored 200. No double approval results. Fix: repeat the key lookup inside the insert transaction after `lock`, or use `ON CONFLICT DO NOTHING` and then load.

### I-3 (Info): step 11 does not re-try the used approval after the restarts

The post-restart delivery `a9` is refused by `ASSIGNMENT_USAGE_UNMEASURED` before the recovery check runs. "Used stays used after restart" is shown by the DB row, not by a refused second use. That is acceptable because the use is a DB row, but the step's comment suggests more than it shows.

### I-4 (Info): the gRPC fix removes the race only when there is no client deadline

With a client deadline, the 50 ms `ANSWER_MARGIN` race against tonic's layer remains by design, and the internal contract §6 says so. Under heavy CI load a client-deadline test could still flake the same way.

## Summary

- **Must fix before approval:** M-1, the permanent wedge after a use that ran nothing, plus the false "new approval" statements in contract item 27 and REPORT §7 and §8.
- **Should fix in this PR (Low):** L-1 (verify or reword the coordinator-mode `recovery_id` guarantee), L-2 (`ieo-role-call` or the documentation), L-3 (three missing tests).
- **Carry-over or optional:** I-1 to I-4; the guardrail end-to-end coverage already listed in REPORT §8; the identity limit of `kind` (1b/F4); I-3 from review 47e4e3f, handled as a hold with the expectation-changing approval left as a §6 follow-up; I-8 untouched as stated.

What is sound: the human-only gate, the record and append-only design, never resuming, reuse of `github_head_check` with an honest trusted-input statement, the running refusal and holds, budget neutrality, at most one run under duplicates, lost answers, restarts and concurrency (probed), the gRPC slack fix, the ledger matching GitHub, and render, validator and links all clean.
