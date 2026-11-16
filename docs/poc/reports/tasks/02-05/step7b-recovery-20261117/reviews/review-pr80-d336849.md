# Review of PR #80 at d336849 (review 2)

Reviewer: independent; I did not write the code. Head `d33684949e459e3d9b82462b1847794671890f06`, base main `78d7fe5`. Review 1 (at `185d12b`) requested changes. Correction commits: `5d61d1b` (code, contracts, tests), `e3c47db` (evidence), `d336849` (REPORT, ledger, archive). Binding spec: `docs/poc/User_Decisions_2026-11-16_recovery.md` §2.

## Verdict

**Approve.** I found no High or Medium issue.

- Review 1's M-1 is fixed. A use that ran nothing (`refused`, `not_called`, `budget_refused`, `not_accepted`) no longer consumes its checkpoint. I reproduced review 1's stuck scenario: it now recovers with exactly one run, and in some runs the new approval is the one that runs.
- L-1, L-2 and L-3 are fixed. The I-2 fix and the I-3 rewording are correct.
- I tried to make two runs start for one checkpoint: concurrent deliveries with two different approved approvals, over HTTP and gRPC, 4 runs. I also reasoned through every case of a lost answer, a re-query, and a restart before or after the send. Every time, at most one run started.
- I raise three new Low findings and one Info note:
  - L-a: the coordinator's `RECOVERY_REQUIRED` check does not tie the use to the checkpoint being taken over. Reproduced.
  - L-b: the `error` half of the L-1 check is untested. My mutant survives.
  - L-c: a use whose start was accepted but never spawned leaves its checkpoint consumed for good. REPORT §8 discloses this for `accepted` but not for the `unknown` variant.
  - None of these starts a second run.

## What I ran

All logs are in the scratchpad directory `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`. The worktree `r80b` was detached at `d336849` and removed at the end.

| Log | Content | Result |
| --- | --- | --- |
| `r80b-lint.log` | `cargo fmt --all --check`; `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings` | pass, pass |
| `r80b-test.log` | `cargo test -j 4 --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`) | **395 passed, 0 failed, 0 ignored**, the same as `evidence/test.log` |
| `r80b-docs.log` | `render_progress.py --check`; `validate_package.py`; `validate_contracts.py`; `openapi_spec_validator`; relative links of every `.md` changed since `78d7fe5`; the OpenAPI lines removed vs 0.15.0 | all OK. 0 broken links. The removed lines are only the version, the extended `decision` enum, the extended problem-code enum and the reworded prose, so 0.16.0 is additive |
| `r80b-probe.log` | My probe `r80b-probe_r80b.rs.txt` (patch script `r80b-probe-patch.py`): a copy of `recovery_e2e` that branches after step 8c. Details below | race: 1 launch each time (3× HTTP, 1× gRPC). Bind gap confirmed (L-a) |
| `r80b-mutants.log` | 4 hand mutations, restored after each (`git checkout`, then `git status` clean apart from the probe file) | 2 killed, 2 survived (below) |

Probe (`r80b-probe.log`):

1. **Review 1's scenario, then a race.** Step 8b already gives a use refused by the supervisor and a new approval `ry` that is `approved`. I then delivered `ry` and the older `rid` at the same moment.
   - In all 4 runs, exactly 1 delivery was `accepted`, with 1 launch and 1 new run directory.
   - The other delivery got `not_called` from the "delivery(ies) … no known outcome" hold.
   - Over gRPC the winner was `ry`, the new approval. So the review 1 wedge is gone, and a second approval of a freed checkpoint runs exactly once.
   - Re-using the refused approval `ridx` gave `not_called` "already used by delivery …".
2. **Bind test (L-a).**
   - I approved a recovery of checkpoint seq 3 and gave it a use row whose delivery `D-forge` has no takeover yet. This is a fixture, made the same way the PR's own step 12 makes one.
   - After another crash (latest seq 6, `takeover`), I reported a takeover by `D-forge` at seq 7 with that `recovery_id`.
   - The coordinator answered **201 `stored`**. An approval for seq 3 was accepted as the recovery of seq 6.

Hand mutations (`r80b-mutants.log`):

| id | Mutation | Target | Result |
| --- | --- | --- | --- |
| H1 | `record_use` skips the consumed-checkpoint check under the recovery lock (`recovery.rs:606`) | `recovery_e2e` | **survived**. This layer is redundant: `usable` already holds on the other delivery's missing or unknown result (Info I-a) |
| H2 | An `accepted` use no longer consumes its checkpoint (`recovery.rs:98`) | `recovery_e2e` | killed (`recovery_e2e.rs:1375`, step 12) |
| H3 | The L-1 check covers only a previous `takeover`, not `error` (`api.rs:4163`) | `recovery_e2e`, then all of `ieo-coord` | **survived both** (L-b) |
| H4 | A takeover by a delivery without a use is accepted (`api.rs:4171`) | `recovery_e2e` | killed (`recovery_e2e.rs:800`, step 2) |

The H2 and H4 runs failed by design and left 4 test databases (`ieo_t_rcv_{http,grpc}_{1577008,1579254}`, my processes) and their `/tmp/ieo-rcv-e2e-*` directories. I dropped all of them by exact name through a probe helper; the listing afterwards showed none of mine. My probe runs dropped their own databases.

No model calls and no real `claude`. I did not read the secrets or runs directories. I did not touch 55433, 58090, PR #29 or other worktrees, and I pushed and commented nothing.

## Review 1 findings

| Finding | Fixed? | Evidence |
| --- | --- | --- |
| **M-1** a use that ran nothing wedges the checkpoint | **Yes** | `checkpoint_uses` and `consumed_by` (`recovery.rs:76-110`): a use consumes only when its delivery's newest result is `accepted`, `unknown` or missing. The per-checkpoint UNIQUE is dropped, with a migration for old databases (`schema.sql:591-593`). `assess` holds when another use consumes (`recovery.rs:213-224`), so `approve` reports a consumed checkpoint as a `needs_confirmation` hold that names the delivery, not a dead `approved` row (step 12, H2 killed). `record_use` re-checks under `recovery:{repo}:{role}` (`:605-616`). E2E step 8b: refused, then a new approval `approved` (`earlier_uses[0].consumes == false`), then one run (step 9). My probe: the new approval itself runs exactly once, and a concurrent second approval does not. Contract item 27 and REPORT §7 and §8 now describe this. The safety of freeing on `not_accepted`: a late-arriving start of that attempt cannot spawn, because `check_delivery` refuses the takeover of a delivery whose newest result is `not_accepted`, and the supervisor reports the takeover before anything runs (`serve.rs:1157-1172`, `:1403-1420`). A non-202 answer means not spawned: the accepted file and the ledger entry come before `spawn`, and a failure removes them (`serve.rs:1052-1093`) |
| **L-1** coordinator mode does not verify `recovery_id` | **Yes, with a gap (L-a)** | `api.rs:4153-4185`: a takeover over a latest `takeover` or `error` needs `checkpoint.recovery_id` equal to the use recorded for its `delivery_id`, else 409 `RECOVERY_REQUIRED`. The supervisor reports the takeover before spawning, so a refusal means nothing runs. Step 2 tests a null and a forged id (H4 killed). Item 28 and the `serve.rs:830-834` comment are now accurate. Gaps: the use's checkpoint is not compared (L-a), and the `error` branch is untested (L-b) |
| **L-2** `ieo-role-call` cannot send `recovery_id` | **Yes** | `--recovery-id`, validated as 1-64 of `[A-Za-z0-9_-]`, is sent through `caller::call_with_extra`. `tests/caller.rs:667-719`: without it, exit 2 mentioning `recovery_id`; with `../x`, exit 1; with `R-cli-1`, exit 0 and the checkpoint carries the id. REPORT §3 1b, item 28 |
| **L-3** M2, M3 and M5 survive | **Yes** | Step 13: guardrail approval, then a stale review target gives 409 (B29). Step 8c: the head changes while the budget lock is held, giving `not_called` with "judgment changed" and the approval unused (B30). Step 5: local ≠ central gives a hold (B31). The author's mutants file shows all three killed |
| I-1 B12 redundant | Recorded | REPORT §5 and §8 cite the race. My H1 confirms the same redundancy for the new consumed check (I-a) |
| I-2 concurrent same-key 500 | **Yes** | The key is looked up again under the lock inside the insert transaction (`recovery.rs:437-454`). Step 6 sends two identical requests at once and gets 201 and 200 with the same id (B27) |
| I-3 step 11 comment | Yes | Reworded |
| I-4 gRPC client-deadline race | Recorded | REPORT §8 |

## Per-condition table (decision §2)

| # | Condition | Verdict | Evidence |
| --- | --- | --- | --- |
| 2.1a | Approval by a confirmed person only: `roles:control` plus the caller identity; no self-approval by development or guardrail credentials | **Met** | `need_approver` (`recovery.rs:46-64`): agents are refused first, then repo scope, then `kind != User` → 403 `HUMAN_APPROVAL_REQUIRED`. The DB has `CHECK (approver_kind = 'user')`. Role credentials are `agent`. The identity limit of `kind` is stated (REPORT §8, 1b/F4) |
| 2.1b | Central record: approver, reason, time, assignment, generation, unfinished checkpoint | **Met** | `role_recoveries` (approver, kind, reason, time, assignment, generation, seq, digest, phase, facts with `earlier_uses`, GitHub). `role_recovery_uses` holds the GitHub check at use and `head_at_approval`. Both are append-only (`forbid_change` triggers). The PR adds no UPDATE or DELETE. The only DDL is the UNIQUE drop, which touches no history |
| 2.1c | New session only for the approved target; distinct from a resume | **Met** | `choose_op`: `recovery_refused` → None, `recovery_allowed` → `start` (`delivery.rs:784-792`). The supervisor starts with `mode: new` and writes `recovery_id` on the checkpoint. Step 9 |
| 2.2a | Reuse the step 7 GitHub head check; state whether its input is trusted or collected | **Met** | `github_head_check` is called in `assess`. The source is "evidence:ingest (trusted service input; not a live GitHub read)" in REPORT §8 and contract §18 |
| 2.2b | Running → refuse; unconfirmable → hold with evidence | **Met** | `RECOVERY_RUNNING`. Holds: supervisor not observed, local ≠ central, pending external requests, unknown deliveries, GitHub not `match`, and now an earlier consuming use. Each hold is recorded with its reasons and is never usable (steps 1, 5, 12) |
| 2.2c | Re-check at use if the checkpoint, generation or head changed | **Met** | `usable` re-runs `assess` in both judgments. Steps 7 and 8c (B10, B30) |
| 2.2d | One approval → at most one run; no double run from duplicates or lost answers | **Met** | PK per approval. A checkpoint is consumed by any use that may have run, enforced under the recovery lock. The use is committed before the send. Steps 10 and 11, review 1's race and my probe all show at most one run. No path I found (late start after `not_accepted`, coordinator or supervisor restart before or after the send, a refused start) starts a second run. L-a and L-c are binding and liveness gaps, not double runs |
| 2.3a | Earlier checkpoints and evidence kept; recovery history added | **Met** | The chain shows `recoveries`, each delivery's `recovery`, and the crashed checkpoint (step 9) |
| 2.3b | Usage and unmeasured state kept; no budget reset or extra call approval | **Met** | Usage before = after the approval (step 6). Unmeasured → `budget_refused` and unused (step 8). After a lost answer → `ASSIGNMENT_USAGE_UNMEASURED` (step 11). The approval writes no ledger |
| 2.3c | USD 0; 02-04 reserve unused | **Met** | No model calls in tests or probes (fake runtime) |
| 2.4 | Real DB plus fake runtime for normal, permission, running, mismatch, duplicate, restart and lost-answer cases | **Met** | `recovery_e2e` steps 1-13 over HTTP and gRPC. 27/27 author mutants. Gaps: L-b, I-a |
| — | Behaviour changes to accepted work (REPORT §3) complete and not weakened | **Met** | Item 1: supervisor start needs `recovery_id`; 4 accepted tests now pass the id and assert the 409 first. Item 1a: the L-1 takeover rule. The accepted `coordinator.rs` test's first checkpoint changes to `after_run`, stated, other checks unchanged. Item 1b: `ieo-role-call`. Item 2: gRPC slack. Item 3: pre-acceptance 02-05 changes. I found no other changed accepted behaviour in `185d12b..d336849`. A replayed identical takeover is answered as a duplicate before the new check (`api.rs:4016-4040`), so supervisor retries are unaffected |
| — | Ledger snapshot 0052 | **Consistent** | `state.json` adds the correction text and the archived review. The rendered views carry the new ledger SHA-256. `render_progress.py --check` is OK |

## New findings

### L-a (Low): the `RECOVERY_REQUIRED` check does not tie the use to the checkpoint being taken over

- **Where:** `crates/ieo-coord/src/api.rs:4156-4171`. `prev` reads only the phase, and the use is looked up by `delivery_id` alone.
- **Scenario:** a delivery whose use was recorded for checkpoint N, with no takeover yet (result `accepted`, `unknown` or missing), takes over a different unfinished checkpoint M. The coordinator accepts it as the recovery of M, although no person approved M.
  - Reproduced: approval of seq 3, takeover at seq 7 over the seq 6 takeover → 201 (`r80b-probe.log`, "PROBE bind").
  - Reaching it through the API needs a forged start with the adapter credential, or M appearing in the milliseconds between a use and its takeover. The consumed-checkpoint hold blocks the obvious paths. So this is hardening of L-1's guarantee, not a reachable double run.
- **Fix:** read `seq` with `phase`, and look up `role_recovery_uses WHERE delivery_id = $1 AND checkpoint_seq = <prev seq>`. Optionally also compare the approval's assignment and generation. Add a step like my probe that expects 409.

### L-b (Low): the `error` half of the L-1 check is untested

- **Where:** `api.rs:4163` (`Some("takeover" | "error")`).
- **Scenario:** mutant H3 narrows the check to `takeover` only and survives both `recovery_e2e` and the whole `ieo-coord` suite. The coordinator-mode guarantee for a run that ended in an `error` checkpoint (item 28: "takeover·error") could regress unnoticed.
- **Fix:** one E2E step. Report an `error` checkpoint after a takeover, then a takeover over it without the used `recovery_id`, and expect 409 `RECOVERY_REQUIRED`.

### L-c (Low, partly disclosed): a start that was accepted but never spawned keeps its checkpoint consumed for good; the `unknown` variant is not described

- **Where:** `recovery.rs:98` decides consumption only from the delivery's result. `delivery.rs:910-951`: a replay without a recorded `sent_sha256` stays `unknown` while the supervisor holds the attempt. REPORT §8, row "`accepted`인데 takeover가 없는 사용". Contract item 27.
- **Scenario:**
  1. The coordinator stops just after the start reaches the supervisor, before transaction 2.
  2. The supervisor answers 202. Its takeover report then fails because the coordinator is down, so the run is not spawned (`CentralRefused`, `spawned: false`), and the central latest checkpoint stays N.
  3. The coordinator comes back. Every replay of the delivery finds the supervisor's stored attempt but no digest to compare, so the result stays `unknown`.
  4. Checkpoint N stays consumed. Every new approval is a `needs_confirmation` hold, and no path releases it.

  The same happens with `accepted` when the takeover report is refused (for example a reassignment in the window), which §8 does disclose. Item 27 and REPORT §7 say a stop "after the use, before the call" is released by a `not_accepted` re-query. That is true before the send, but this variant is never released.

  This analysis is from the code; I did not reproduce it. It fails closed, never with a double run, and it is rare.
- **Fix (carry-over to step 9 is acceptable):** add the `unknown` / no-digest variant to §8 and item 27. Then give the release path a definite signal: for example, the supervisor's attempt outcome `spawned: false` together with no `role_takeovers` row for the delivery, or a recorded person's confirmation.

### I-a (Info): the consumed check in `record_use` is a redundant layer

Mutant H1, which removes the check, survives the E2E test. In my race probe the losing delivery was always stopped earlier, by `usable`'s "no known outcome" hold on the winner's pending delivery. That is the same pattern as review 1's I-1. The DB-side check under the lock is still correct to keep. A unit test of `record_use` with a consuming earlier use would pin it down.

## Summary

- **Must fix before merge:** none. There are no High or Medium findings, and review 1's M-1 and its documentation errors are fixed.
- **Should fix (Low, in this PR or the next):**
  - L-a: tie the `RECOVERY_REQUIRED` lookup to the previous checkpoint's seq, with a test.
  - L-b: add an `error`-phase takeover step.
  - L-c: document the `unknown` / no-digest variant of the never-spawned wedge in REPORT §8 and contract item 27.
- **Carry-over:** L-c's release path (step 9, next to the disclosed `accepted` case); I-a; review 1's I-4 (gRPC client-deadline race); guardrail recovery delivery not tested end to end (REPORT §8); the identity limit of `kind` (1b/F4); the persistent PoC DB still to be migrated after merge (REPORT §8).
