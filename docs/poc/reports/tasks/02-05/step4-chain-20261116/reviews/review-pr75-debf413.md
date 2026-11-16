# Review of PR #75 at debf413 (review 2)

## Verdict

**Approve.** M-1, L-1 and L-2 from review 1 (5dbb3d8) are fixed and covered by tests that kill the matching mutants. No High or Medium findings remain. I raise one new Low (a test gap behind a REPORT claim) and some Info items. They can be fixed in this PR or carried forward.

## What I ran

All runs used a fresh detached worktree at debf413 (scratchpad `r75b`, now removed), the test DB on port 55432 and `-j 4`. Logs are in the scratchpad as `r75b-*.log`.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy -j 4 --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test -j 4 --workspace` (`RUST_TEST_THREADS=4`, test DB) | exit 0: **376 passed, 0 failed, 0 ignored** across 41 binaries. The new cases were added inside existing tests, so the count is unchanged, as the REPORT says. |
| `validate_package.py` | exit 0, `schema_errors: 0` |
| `openapi-spec-validator` | OK |
| proto diff 375f21f..debf413 | empty, so buf does not apply |
| Evidence | `test.log` header is commit 40e2c98 with a clean tree. 40e2c98..debf413 touches only the REPORT and evidence files. `mutants.txt` lists 21 mutants, all killed, and records the history of S4-M18. |
| Test DB cleanup | Afterwards I listed `ieo_t_%` databases on 55432: none were left. |

Hand mutations (each applied alone to debf413, targeted tests run, then reverted):

| ID | Mutation | Result |
| --- | --- | --- |
| D2 (review 1 D, rebased) | fill without the takeover linkage: `api.rs:4227` `if exists \|\| !linked` → `if exists` | KILLED: `a_lost_first_action…` (coordinator.rs:4635) |
| E | takeover with a **missing** `attempt_id` accepted: `api.rs:4100` → `if attempt_id.is_some() && attempt_id != Some(..)` | KILLED: `a_takeover_is_stored_only…` (coordinator.rs:4355) |
| G | fill ignores the checkpoint's own attempt: `api.rs:3968` `is_none_or(..)` → `true` | KILLED: `a_lost_first_action…` |
| F | chain `"none"` link ignores the attempt: `chain.rs:285` `c.attempt_id == d.attempt_id` → `true` | **SURVIVED** the whole `--test coordinator` and `--test chain_e2e` (see L-3) |

## Analysis

### M-1: takeover bound to the delivery's call

- **Supervisor.** It writes `attempt_id = job.attempt` on every checkpoint (`serve.rs:1210`). `checkpoints_carry…` asserts it on the takeover and on the after_run, and S4-M19 drops it.
- **Coordinator.** `check_delivery` now requires `role_deliveries.attempt_id == checkpoint.attempt_id` (`api.rs:4094–4105`). A different attempt or a null attempt gives 409 `DELIVERY_MISMATCH`. A non-string attempt gives 400.
  - Tests: `D-wait` cases with attempts `"other"`, `null` and `7`, followed by unchanged row counts.
  - My mutant E, which accepts a missing attempt, is killed.
- **Holes I checked.**
  - **Re-report or duplicate.** The same-bytes path (`seq <= last_seq`, same SHA → 200) still runs before `check_delivery`. No second row is written, and the original takeover row stays. Correct.
  - **after_run and error.** These are not checked against the delivery, and they don't need to be: they create no takeover row.
    - The fill now requires two things: (i) the checkpoint's `attempt_id`, when present, equals the first action's; and (ii) a `role_takeovers` row exists for the checkpoint's session whose delivery called that attempt (`api.rs:4209–4229`).
    - The test covers three cases: a session never taken over, an attempt the session's delivery did not call, and a checkpoint attempt different from the first action's attempt. All three are killed: D2, G and S4-M18.
  - **Reconnect takeover with a different session.** The author deliberately does not require the session to match (REPORT §11, M-1 row). That is correct: a reconnect resumes the checkpoint's session, and a reconnect after a tamper uses `new_session_id`, so the delivery's `session_id` legitimately differs. The attempt is the right key, because the supervisor accepts an attempt only once (see the next item).
  - **Residual I-4 gap and the `ID_CONTENT_CONFLICT` claim.** I verified this by reading the code.
    - The supervisor stores each accepted attempt. A request with the same `attempt_id` and a different `request_sha256` gets 409 `ID_CONTENT_CONFLICT` (`serve.rs` ~599–620). `caller::start_with` maps any non-202 answer to `Sent::Refused`, and the delivery records `refused`.
    - So a start that copies **both** ids does consume the delivery, but it is visible: the chain shows a takeover on a `refused` delivery. That matches REPORT §8.
    - This needs an adapter credential plus knowledge of the delivery and attempt ids, which is inside the §2.1 trust boundary. Acceptable as a recorded residual. The REPORT §7 wording is slightly too absolute (Info I-7).
- **Contracts.** `attempt_id` is documented in OpenAPI 0.12.0 (reportCheckpoint), the internal contract §1.2 order and §15, and runtime contract revision 12 §15 item 16. All are additive and the same revision. A checkpoint from a 02-04 supervisor without `attempt_id` (null) still parses. As before, its takeover is refused for lack of a delivery.

### L-1: claims are not judgment inputs

Fixed in both places, the OpenAPI getAssignmentChain description and the `chain.rs` module doc. Both now say the only claim the judgment reads is the current guardrail's `guardrail_review` (G3, policy 0.6.0), and that supervisor claims are never judgment inputs. Accurate against `api.rs` `judge`.

### L-2: test for the fill's takeover condition

Fixed, and the condition was also tightened, as described above. Review 1's mutant D (now D2) is killed. The REPORT honestly records that S4-M18 survived at 1b7f51c and was killed after the 40e2c98 test change.

### I-items from review 1

- **I-2 (after_run attempt and delivery cross-check).** Fixed in the fill. In the chain's `"none"` link the code is fixed but the test is missing: see L-3.
- **I-1, I-3, I-4 and I-5** are recorded in REPORT §8 with an owner. That is appropriate.
- **I-6** needs no action.

### Regressions in 5dbb3d8..debf413

None found.

- The fixtures were updated consistently. `common::checkpoint` sets `attempt_id = delivery_id`, the fixture deliveries use their id as their attempt, and `checkpoint_e2e` and `delivery_e2e` pass explicit attempts.
- No accepted assertion was weakened. The diff only adds cases and renumbers sequences (9 → 12) after inserting three checkpoints.
- The chain now exposes each checkpoint's `attempt_id`. That is additive.

## Findings

### L-3 (Low): the attempt condition on the chain's `"none"` link is untested, but REPORT §11 says it is tested

- **Where.** `crates/ieo-coord/src/chain.rs:281–288`.
- **The claim.** REPORT §11, row I-2: "연결 조회의 `"none"`도 그 전달의 시도와 같은 `attempt_id`의 체크포인트에서만 | L-2 시험 (b)(c)".
- **Evidence.** Mutant F, which drops `c["attempt_id"] == d["attempt_id"]`, survives both `--test coordinator` and `--test chain_e2e`. Cases (b) and (c) carry first-action *objects*, never `"none"`.
- **Failure scenario.**
  - A later `after_run` from another attempt names delivery D and says `first_action: "none"`, for example a stray or copied `delivery_id`.
  - The chain then reports D's first action as "none" ("the run had none") when it is actually unknown.
  - A regression that drops the condition would pass CI.
- **Suggested fix.**
  1. In `a_lost_first_action…` (or the chain test), report an `after_run` with `delivery_id = D`, `attempt_id = "other"` and `first_action: "none"` for a D that has no recorded first action.
  2. Assert that the `links[D].first_action` is null.
  3. Add a mutant matching F to `mutants.txt`, or correct the §11 test reference.

### I-7 (Info): REPORT §7 is more absolute than §8

- **Detail.** §7 says "그래서 결과가 나중에 `refused`·`not_accepted`가 될 수는 없다". §8 correctly describes the case of a start that copies both ids, where the delivery ends `refused` *with* a takeover.
- **Suggestion.** Qualify §7, for example: "…없다(같은 시도 id를 베낀 직접 start는 예외, §8)".

### I-8 (Info): the direct first-action report and the fill use different conditions

- **Where.** `record_first_action`, `api.rs` (uses `taken_over`).
- **Detail.** The direct report still accepts any `attempt_id` for a session that has any takeover checkpoint. The fill now requires a takeover linked to the delivery that called that attempt. Both are claims, and the chain links first actions to deliveries by `attempt_id`, so a mismatched direct report simply links to nothing.
- **Suggestion.** Consider using the same linkage condition in step 6 or 8.

### I-9 (Info): a byte-identical copy of the delivery's request is also recorded as `refused`

- **Detail.** This goes beyond REPORT §8. If a copied start is byte-identical to what the coordinator then sends, the supervisor answers 200 (replayed). `caller::start_with` maps every non-202 answer to `Sent::Refused`, so the delivery is still recorded as `refused` even though the run is effectively its own.
- **Assessment.** This is pre-existing step-3 behaviour, inside the trust boundary, and needs no action in this PR. It is worth one clause in §8.

## Summary for the author

All three review-1 findings are fixed properly:

- **Attempt binding** is in place on the supervisor and the coordinator, the contracts are updated, and refusals leave no rows.
- **The judgment-input wording** is accurate.
- **The fill condition** is tightened and covered; my old mutant D and two new mutants (E and G) are killed.

Full tests (376), fmt, clippy and the validators are clean, and I found no regressions.

**Approve.** Optionally, before merge:

- **L-3.** Add a test for the attempt condition on the chain's `"none"` link (mutant F survives today), or correct the §11 test reference.
- **I-7.** Align the wording of §7 with §8.
