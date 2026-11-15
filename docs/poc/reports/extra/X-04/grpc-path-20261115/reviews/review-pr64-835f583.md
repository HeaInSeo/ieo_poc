# Review of PR #64 at 835f583 (review 2)

- PR: HeaInSeo/ieo_poc#64, "X-04: gRPC path (operation table, tonic adapter, supervisor CoordinatorClient) and HTTP·gRPC common regression — snapshot 0037"
- Head: 835f583f3b03e8940ce7ed01b007b93beb339237 (confirmed with `gh pr view 64 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 493b712. Seven commits on top of the base. The new one is 835f583 (correction 1 of 3, for review c6c0656).
- CI at 835f583 (run 43706567018). I waited for every job to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: **SUCCESS**. The log shows 332 passed in total, and `tests/grpc.rs` passed 9 on the CI PostgreSQL.
  - proto buf lint and breaking (not required): SUCCESS
- Required checks on main (branch protection, re-read): the three names above.
- Reviewer: independent review agent, read-only. No model calls, no containers, no push, no comments. I did not read the secrets or runs directories. I did not touch the main checkout, the df02-plan or x04wt worktrees, or the PostgreSQL on 55433. This is review 2 of at most 4.

## Verdict

**APPROVE.** There are no Blocking or Medium findings.

- M-1 is fixed. Every test in `tests/serve.rs` holds one serial guard for its whole body. That includes the macro-generated http/grpc variants. The required rust job is green at this head, and 20 local `serve` runs had 0 failures.
- L-1 is addressed for its evidence half: the test now proves the answer came from the server. Its timing half is a property of the design and is not removed. That residual is recorded as L-3 below. It is Low and does not block.
- The other recorded items are faithful to review 1, and none of them is a correctness problem.

## What I ran

- I made a detached worktree of 835f583 in my scratch directory, with a **separate** `CARGO_TARGET_DIR` (`target-r64b`). I removed both at the end. The shared target cache was not used.
- `cargo fmt --check`: rc 0. `cargo clippy --all-targets --locked -- -D warnings`: rc 0.
- `cargo test --locked --all-targets` with `IEO_TEST_DB_URL` set to the throwaway DB on 55432: rc 0, **332 passed, 0 failed, 0 ignored**. This equals `evidence/test.log` and the CI count.
- `cargo test -p ieo-supervisor --test serve` 20 times in a loop: **0 failures** (28 passed each time, about 12–15 s per run, against 5.6 s before serialization).
- `a_statement_past_the_deadline_rolls_back_as_deadline_exceeded` 5 times in a row: all passed. Two mutations of `crates/ieo-coord/src/grpc.rs`, each reverted afterwards (`git status` was clean):
  - A: `ANSWER_MARGIN` set to 0 ms, so tonic's timeout and the statement timeout fire together. The test fails at the code assertion, with `Cancelled, "Timeout expired"` and source `tonic::transport::Error(Transport, TimeoutExpired)`. This is the answer from the client and transport side.
  - B: the server's deadline message changed to `"Timeout expired"`. The test fails at the new `answered by the server` assertion (grpc.rs:1213).
- Validators from `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0, `schema_errors` 0.
  - `render_progress.py --check`: ok.
  - `test_render_progress.py`: PASS.
- `state.json` compared structurally against c6c0656.
- `cmp` of the archived `reviews/review-pr64-c6c0656.md` against `pending-reviews/review-pr64-c6c0656.md`: identical.
- `evidence/test.log` grepped for URLs, passwords, tokens and home paths: none. Paths are masked as `<scratchpad>`.

## Status of earlier findings (review c6c0656)

### M-1: fixed

- `static SERIAL: Mutex<()>` and `fn serial()` use `lock().unwrap_or_else(|e| e.into_inner())`. A poisoned mutex, left by an earlier failed test, does not cascade into later tests.
- There are 24 `#[test]` functions: 22 plain ones and the two in the `over_both_transports!` macro. Each takes `let _serial = serial();` as its first statement, so the guard is held until the test ends.
  - In the macro, the guard is taken in `http()` and `grpc()`, and the shared body `super::$name(transport)` does not take it again.
  - No helper calls `serial()`, so the guard is never taken twice and cannot deadlock.
  - Nothing is spawned before the guard is taken: `setup()` runs after it in every test.
- Residual (Info only): the service starts each session's child in a background `execute` thread. A test that returned while a job was still running could fork after the guard is released. All lock-probing tests (`r1`, `refuse`, `lockprobe`) `wait()` until the service is idle before they probe. A late fork from an earlier test would hold only that test's own lock file, not the current test's lock. The 20-run loop and CI found nothing.
- The REPORT §4 entry describes the cause and fix correctly.

### L-1: partly addressed (evidence fixed; the timing residual is L-3)

- The new assertion `st.message().contains("rolled back")` matches the server's string in `crates/ieo-coord/src/grpc.rs:373`, `"deadline reached before commit: rolled back"`, returned with `Code::DeadlineExceeded`.
- An answer from tonic's timeout layer or from the client transport is `Cancelled, "Timeout expired"`. It fails the code assertion and, with that code, would also fail the message assertion (mutations A and B).
- The test still checks `DEADLINE_EXCEEDED`, that there is no trailer (`error_from_status(&st).is_none()`), and the rollback (`checkpoints(&e) == 0` after the lock is released).
- The `took < 5 s` bound now only guards against hangs, which is fine.
- See L-3 for what was not removed.

### L-2, I-1 to I-6: recorded faithfully, none blocking

REPORT §4 matches review 1 for every item:

- **L-2** (no worker respawn after a panic): recorded as the same behaviour as the HTTP workers. The client treats the answer as unknown. Review 1 suggested a line in contract §11 if the item was not fixed; that line was not added, and REPORT §4 is the only record. This is acceptable, because the client side stays correct (no trailer means NOT_OBSERVED, the staged copy is kept, and nothing is resent).
- **I-1** (circular e2e code check): recorded. The `ieo-proto` unit table pins the 409 split.
- **I-2** (57014 during COMMIT gives INTERNAL): recorded. Both answers are "unknown" to the client.
- **I-3** (`Shape` is descriptive): recorded.
- **I-4** (repo_id pre-check answer): recorded. Both answers are 4xx, so the staged copy is handled the same way.
- **I-5** (ledger note without a run id): fixed. The note and `resume.branch_sha` now say 43697454428.
- **I-6** (pool workers not joined on stop): recorded together with L-2/I-2. PostgreSQL rolls back an open transaction when the process exits.

None of them changes a stored row or what the client decides.

## Other checks

- **Code changes since c6c0656** are only `crates/ieo-coord/tests/grpc.rs` (+5/-2) and `crates/ieo-supervisor/tests/serve.rs` (+33). No `src` file changed.
- **`evidence/test.log`**: plausible.
  - The header says "head c6c0656 + working tree (fix)". The per-binary counts are unchanged, and the total is 332.
  - `serve` time goes from 5.58 s to 11.58 s, which fits serialization.
  - The log adds the line "serve.rs repeated 20 times … failures=0". The stray `Finished` line and the `test done` line were dropped.
- **REPORT note on 18 `ieo-evidence` failures from a stale cache**: accurate. `crates/ieo-evidence/tests/collector.rs` builds its fixture paths at compile time with `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/…")` (T23, R1, and others; 6 uses, the T23 fixture is read by many tests). A binary built in a worktree that was later deleted fails on those reads. My clean build in a separate target dir passed them all.
- **Ledger**: a structural diff of `state.json` against c6c0656 shows exactly 4 changes:
  - `notes[17]`: the run id 43697454428.
  - `extra_tasks[3].result_state` (X-04): the review/correction sentence ("검토 1/4·교정 1/3").
  - `extra_tasks[3].evidence`: + `reviews/review-pr64-c6c0656.md`.
  - `resume.branch_sha`: the CI run id.

  Everything else is unchanged:
  - The X-04 status stays 진행, unverified.
  - The snapshot is still 0037. STATUS shows 12/73 and 0/12.
  - Blocking (X-08) and budget did not change.
  - The generated views changed only in their ledger hash and in the TASKS row.

## New findings

### L-3 (Low): the 50 ms answer margin still decides the deadline test; the REPORT wording overstates the L-1 fix

**Where:**
- `crates/ieo-coord/tests/grpc.rs`, `a_statement_past_the_deadline_…`, with the 600 ms client deadline.
- REPORT §4, L-1 row ("시간 경계 대신 서버가 만든 답인지 확인").

**Problem:**
- The `took < 600 ms` bound that was removed was redundant. If the server's answer is later than the deadline, tonic's timeout layer or the client transport answers `Cancelled "Timeout expired"` (mutation A), and `assert_eq!(code, DeadlineExceeded)` fails just as before.
- The real margin is still `ANSWER_MARGIN` (50 ms). In that window the server has to finish the PostgreSQL cancel, the rollback, the oneshot and the encoding.
- A longer client deadline would not help, because the margin is the same at any deadline. So review 1's first suggestion would not have fixed it either.
- The flake risk on a loaded runner is therefore about the same as before. It is small: 5 of 5 local runs and CI passed.

**Fix (optional, non-blocking):**
- Change the REPORT §4 L-1 row (or §3) to say that the test now proves the source, and that whether it passes still depends on the server answering within `ANSWER_MARGIN`. A loss shows up as `CANCELLED "Timeout expired"`, which the contract treats as unknown.
- If the test ever flakes, the next step is a test-only, larger `ANSWER_MARGIN` passed through `GrpcConfig`, not a longer deadline.

### I-7 (Info): the evidence log was taken from c6c0656 plus the working tree

The header of `evidence/test.log` says the run was on "c6c0656 + working tree", not on the 835f583 commit. I re-ran everything at 835f583 and got the same 332 passed. CI at 835f583 also shows 332. No change needed.

## Summary

M-1 is fixed and confirmed: 20/20 local runs pass and the required CI is green at 835f583. All three required checks and the proto job are SUCCESS. L-1 is fixed as far as the evidence goes. Its remaining timing margin (L-3) belongs to the design and is Low. The recorded items and the ledger are faithful.

**APPROVE 835f583.** L-3 and I-7 can stay as notes, or L-3's REPORT wording can be fixed in the acceptance-record PR.
