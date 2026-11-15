# Review of PR #64 at c6c0656 (review 1)

- PR: HeaInSeo/ieo_poc#64, "X-04: gRPC path (operation table, tonic adapter, supervisor CoordinatorClient) and HTTP·gRPC common regression — snapshot 0037"
- Head: c6c06568e3c9cac8f7f160e733533171787b9c85 (confirmed with `gh pr view 64 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 493b712 (493b712ee459…). Six commits on top of the base (0e1c0b5 step 1, e176ebb step 2, 72a37ac step 3, 7a926d3 step 4, 3887382 step 5, c6c0656 report).
- CI at c6c0656 (run 43701595104), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: **FAILURE**. `ieo-supervisor --test serve`: `a_live_session_is_reported_running_before_its_checkpoint_phase_is_judged` (serve.rs:855) got 409 `ALREADY_RUNNING` (pid 4243) where it expected `EXTERNAL_RECONCILIATION_REQUIRED`. Every other test binary passed, including `tests/grpc.rs` with 9 passed on the CI PostgreSQL. See M-1.
  - proto buf lint and breaking (new, not required): SUCCESS
- Required checks on main (branch protection): the three names above. The new job is not required, as the PR says.
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories and did not touch the main checkout, the df02-plan or x04wt worktrees, or the PostgreSQL on 55433. This is review 1 of at most 4.

## Verdict

**REQUEST CHANGES: one Medium finding (M-1), no Blocking.** The code meets the X-04 acceptance criteria and the contract. Step 1 does not change HTTP behaviour. The adapter adds a transport and no business rules. The supervisor client keeps the 02-04 staged-copy rule. The common regression is a real two-database comparison, and 10 of 11 targeted mutations were caught by the end-to-end tests. The eleventh was caught only by the unit status table (I-1).

The only reason for REQUEST CHANGES is M-1. A required check is red at this head. The cause is a known fork/exec flock race in the supervisor `serve` tests. This PR does not create the race, but it adds three spawning tests to that binary, so the race now fires more often (I reproduced it 1 time in 20 runs locally). The fix is small and already exists in the repo as a pattern. After the fix and a green CI run at the new head, I would approve. The Low and Info items can stay as notes.

## What I ran

- Detached worktree of c6c0656 in my scratch directory (removed at the end). Reused the existing target cache. No stale-path problems.
- `cargo fmt --check`: rc 0. `cargo clippy --all-targets --locked -- -D warnings`: rc 0.
- `cargo test --locked --all-targets` with `IEO_TEST_DB_URL` pointing at the throwaway DB on 55432: rc 0, **332 passed, 0 failed, 0 ignored**. This equals `evidence/test.log`. Per binary: coordinator 68, grpc 9, operations 1, checkpoint_e2e 2 (http + grpc), supervisor serve 28, central 4, ieo_proto unit 4, and the others unchanged.
- `ieo-supervisor --test serve` 20 times in a loop: 19 passes and 1 failure (`reconnect_refuses_stale_or_mismatched_records_and_missing_credentials`, serve.rs:400). Both this failure and the CI one are the M-1 race.
- `buf lint`, `buf build -o /dev/null`: rc 0. `buf breaking` against an extracted copy of 493b712 `contracts/proto`: rc 0. The proto diff touches comments only; no field, number or type changed.
- The CI buf sha256 (`3a0c4da8…b5d8`) matches the official `sha256.txt` of the v1.47.2 release for `buf-Linux-x86_64`.
- Validators from `docs/poc` with `.venv-poc`: `validate_package.py` rc 0 (`schema_errors` 0), `render_progress.py --check` ok, `test_render_progress.py` PASS. `openapi-spec-validator` on `ieo-api-v0.openapi.yaml`: OK.
- Mutation tests. Each was applied to the worktree, run against its target test binary, and reverted (`git status` was clean afterwards):

| # | Mutation | Caught by |
| --- | --- | --- |
| M1 | `CallGuard::before_commit` always `Ok` (no pre-commit guard) | `a_call_cancelled_before_commit_rolls_back` |
| M2 | `query_canceled` returns `None` (57014 stays 500) | `a_statement_past_the_deadline_rolls_back_as_deadline_exceeded` |
| M3 | `GrpcClient::report_checkpoint` resends once on no answer | `a_status_without_a_trailer_is_unknown_and_is_not_resent` |
| M4 | adapter authenticates before the 1 MiB check | `both_transports_give_the_same_answers_and_the_same_database` |
| M5 | 409 split: `EVENT_CONTENT_CONFLICT` → `FAILED_PRECONDITION` | unit `status_table_follows_section_4_1` only (I-1) |
| M6 | `CancelOnDrop` never sets the flag | `a_call_cancelled_before_commit_rolls_back` |
| M7 | a status without a trailer read as a 400 | `a_status_without_a_trailer_is_unknown_and_is_not_resent` |
| M8 | unset `timeout_seconds` sent as `null` | `a_record_stored_over_one_transport_replays_over_the_other` |
| M9 | no `SET LOCAL statement_timeout` | `a_statement_past_the_deadline_…` |
| M10 | `dispatch` asks for a credential before health | both `checkpoint_e2e` variants |

- Diffed `tests/coordinator.rs` at 493b712 against the new `tests/common/mod.rs` and `tests/coordinator.rs`. The move is mechanical: `pub` added, `include_str!` paths get one more `../`, `Env.app` added, `call_raw` added, and `report()` reformatted. No test body changed.
- `state.json`, `events.jsonl` and the generated views diffed against 493b712. `cmp` of the archived `review-pr63-d1c86c5.md` against `pending-reviews/`: identical. Grepped the added lines for secrets: none, apart from the archived review's own sentences saying there are none. `evidence/test.log` holds no URL or password; paths are masked as `<scratchpad>`.
- `Cargo.lock`: every new package comes from crates.io, with no git sources. The new packages are the tonic 0.14.9 / prost 0.14.6 / hyper 1 / axum 0.8 / tower stack, plus protox 0.10 and its parser (logos, miette, prost-reflect) for build-time generation. Nothing surprising.

## Step 1: HTTP behaviour preserved

Old `handle` (493b712) against new `route` + `dispatch`:

- Order is the same: (1) `GET /v0/health` is answered before authentication, (2) 401, (3) role control without DB, (4) one transaction per operation, (5) 404 for no match. A non-health request without a credential gets 401 even on an unknown path, as before. Because `dispatch` checks 401 before `None → 404`, the old order holds.
- Every one of the 22 old match arms maps to one `Operation`, with the same method, the same segments and the same handler call. Segment splitting is unchanged (`trim_matches('/')` then `split('/')`): a trailing slash is ignored, an inner empty segment is kept, and `/v0/agent/repro-requests/` matches nothing. `tests/operations.rs` pins these cases.
- `compare` still authorizes first, calls the supervisor (`role_status`) outside the transaction, and then runs the transaction.
- `run_tx` with `NoGuard` matches the old `run` closure: `before_begin` and `before_commit` pass, there is no `SET`, and `query_canceled` returns `None`, so a 57014 stays 500. The new `QUERY_CANCELED` thread-local is reset before each operation and is read only through the guard.
- `http.rs`: `body_too_large()` and `db_unavailable()` return byte-identical bodies. The DB-reconnect check still comes before the body-size check.

## Adapter and contract

- No business rule is duplicated. The adapter builds the HTTP-equivalent body and calls `api::dispatch` with the named `Operation`. The status table, trailer and JSON conversions live once, in `ieo-proto`.
- §4.1: 400/401/403/404/422/503 map as in the table. 409 is split by "every code in group", with a mixed group → `FAILED_PRECONDITION`. A 500 and any other status → `INTERNAL`. DB unavailable → `UNAVAILABLE` with the HTTP 500 body in the trailer. Deadline and cancel answers carry no trailer. `Outcome`: 200 → REPLAYED, 201/202 → STORED.
- §7.0 order: DB → size (before auth) → auth → path values (404) → UTF-8 (400) → `before_begin`. M4 shows the e2e test enforces size before auth.
- Path values: `/`, `?`, `#` anywhere, and an empty `request_id`, give 404 after authentication. An empty `repo_id` is passed through, matching contract §3 and the HTTP route.
- JSON equivalence: in `repro_request_json`, every non-optional field is always present and an unset `timeout_seconds` is absent (M8 is caught). The tests compare the HTTP body to the gRPC answer read back through `*_to_json`. This is asymmetric, so if a field were dropped on both sides, the raw HTTP body would expose it.
- Deadline and cancel:
  - `work_deadline` is deadline − 50 ms. `before_begin` and `before_commit` check the cancel flag and the work deadline. `statement_timeout` is set once, at least 1 ms. 57014 maps to `DEADLINE_EXCEEDED` or `CANCELLED`.
  - A commit cannot follow a guard abort: `Err` from `before_commit` always rolls back. A commit can still land just after the guard passes, while the client sees `CANCELLED` or a lost answer. The contract treats that as "unknown", and the relay test shows the same-key re-query finds it.
  - A cancelled call does not hold a connection beyond its own statement (at most the 15 s budget). The worker reconnects through `is_closed`.
- Shutdown: HTTP drains first, then gRPC graceful shutdown, then the pool sender is dropped. Queued jobs still run, and their guards abort the ones whose futures were dropped.

## Supervisor client

- One classification for both transports: `Reply::Answered(status, body)` / `NoAnswer`, decided only in `Central::latest` and `Central::report`.
  - On gRPC, `reply_of` reads the trailer's `http_status`. A status without a trailer is `NoAnswer`, which gives `NOT_OBSERVED`, keeps the staged copy and is not resent (M3 and M7 are caught).
  - The HTTP client is the 02-04 code moved without change (same `ureq` call, 10 s, lossy UTF-8 body).
- `serve.rs` is unchanged, so the staged-copy rule (`coordinator_status` 400..499 → drop, else keep) is the same for HTTP. The three staged-copy tests now run over both transports.
- The default transport is HTTP. An unknown `transport` is a config error. Old configs without `transport` behave as before, and `Central` equality now includes the transport.

## Ledger and docs

- `state.json`: snapshot 0037, phase gains "X-04 구현 PR", X-04 status 미착수 → 진행 with report and evidence, the X-03 `unresolved` gains the Q-1/Q-2 note, the archived d1c86c5 review is in X-03's evidence, and `resume` is updated (branch_sha 493b712, running, next_action, current_work with Q-2 wording fixed).
  - Baselines 73/12/31/5 are unchanged, and STATUS shows 12/73 and 0/12.
  - `blocking` and `budget` (X-08 gate, 02-04 7/12) are unchanged.
- The X-03 REPORT header now records the 8e3e817 approval, the merge and 검수 완료 (Q-1). The archived review is next to the REPORT, so its example link `reviews/review-pr62-8e3e817.md` resolves.
- Contract §12 is faithful to the code:
  - Several changes are stricter or more conservative than X-03: auto re-read 0 instead of "up to 3", transport-limit overflow treated as "unknown" instead of "client defect, not unknown", and empty `request_id`, `?` and `#` → 404.
  - The one loosening, a single `statement_timeout` per transaction instead of one per statement, is explained, and the pre-commit check covers the total.
  - No 02-05 sentence changed, and the PR adds no prerequisite to 02-05.
- REPORT claims match: 94 `Do` steps in the scenario, 332 tests, RPC 5, `--grpc-listen`, protox, and the stated limits in §3.

## Findings

### M-1 (Medium): a required check is red at c6c0656, from the session-lock fork/exec race in `tests/serve.rs`, which this PR makes more likely

**Where:**
- `crates/ieo-supervisor/tests/serve.rs`. In CI: `a_live_session_is_reported_running_before_its_checkpoint_phase_is_judged` (line ~855). Locally (1/20): `reconnect_refuses_stale_or_mismatched_records_and_missing_credentials` (line ~400).
- CI run 43701595104, job "rust fmt clippy test".

**Problem:**
- The session lock is an `flock` on an `O_CLOEXEC` file. A child spawned on another test thread shares that descriptor between fork and exec, so a lock that was just released can still look held. The test then gets `ALREADY_RUNNING` with the test's own pid 4243.
- The repo already handles this race elsewhere. `tests/supervisor.rs` uses the `SPAWN` mutex (review 119ab95 P1), `tests/caller.rs` serializes its tests, and `caller.rs` waits up to 3 s (review f90f5b5 N1). `tests/serve.rs` has no such guard.
- These tests are unchanged and use `local_only`, so the X-04 logic is not at fault. But X-04 turned three central-record tests into six (`over_both_transports!`), and each one spawns sessions, so more processes are spawned while lock tests run.
- Merging needs the three required checks green at the approved head. A plain rerun might pass, but the flake would remain in a file this PR edits.

**Failure scenario:** CI on this PR or on the merge commit on main fails at random (about 5% locally), which blocks the merge-SHA CI step of the acceptance record.

**Fix:**
- Apply the `tests/supervisor.rs` pattern in `tests/serve.rs`: a `static SPAWN: Mutex<()>`, held around every session start (`/v0/role/start`, `reconnect` that spawns) and held throughout the lock-probing tests. Alternatively, run the two lock-sensitive tests (`a_live_session_…`, `reconnect_refuses_stale_…`) and any other test that asserts on a just-released lock under one shared guard.
- Then confirm with a loop of about 20 local runs and a green CI run at the new head.
- Note the flake and its fix in REPORT §3.

### L-1 (Low): `a_statement_past_the_deadline_…` has a 50 ms timing margin

**Where:** `crates/ieo-coord/tests/grpc.rs`, `assert!(took < 600ms)` with a 600 ms client deadline.

**Problem:** `statement_timeout` fires at about 550 ms. The answer has to come back within the next 50 ms, or tonic's own layer answers `CANCELLED` and the `DEADLINE_EXCEEDED` assertion fails. CI and my runs passed, but a loaded runner could break this, like M-1.

**Fix:** use a longer client deadline in the test (e.g. 2 s, lock held longer). The margin is a property of the server, so the test can make the budget large without changing the 50 ms constant. Or accept `CANCELLED` with a note.

### L-2 (Low): a panicking worker shrinks the gRPC pool for good, and callers see "coordinator is stopping"

**Where:** `crates/ieo-coord/src/grpc.rs` `Pool::start` / `Pool::run`.

**Problem:**
- If a handler panics, the worker thread dies and its oneshot sender is dropped. The caller gets `UNAVAILABLE "coordinator is stopping"` with no trailer. That is correctly "unknown" for the client, but the message is wrong.
- The worker is never replaced. Once all workers are gone, every call and the health check fail until restart.
- The HTTP workers behave the same (also no respawn), so this is parity, not a regression.

**Fix:** (optional) wrap `job(db.as_mut())` in `catch_unwind` (drop the connection on panic), or log it and use a distinct message such as "worker failed". Record the limit in contract §11 if it is not fixed.

### I-1 (Info): the e2e gRPC code check is circular

`answer()` in `tests/grpc.rs` asserts `st.code() == ieo_proto::grpc_code(http, body)`, the same function the server uses. M5 (swapping the 409 group) passed every e2e test and was caught only by the unit table in `ieo-proto`. That is acceptable because the unit table is a fixed list. To make the e2e independent, record the expected `Code` per `Do` step, or compare against a literal table for the `(status, code)` pairs in `seen`.

### I-2 (Info): a 57014 during COMMIT gives `INTERNAL`, not `DEADLINE_EXCEEDED`

`run_tx` maps a `tx.commit()` error straight to 500 without consulting `query_canceled`. Both are "unknown" for the client (§5), so nothing goes wrong. Contract §6.1 could say that the 57014 mapping covers statements before COMMIT.

### I-3 (Info): `Shape` is documentation, not enforcement

`dispatch` does not read `Operation::shape()`. Only `tests/operations.rs` checks it. That is fine, but a new 02-05 operation could get a shape that disagrees with its `dispatch` arm. Either have `dispatch` choose `run`/no-DB/call-then-tx from `shape()`, or say in the doc comment that the table is descriptive.

### I-4 (Info): the client's `repo_id` pre-check answer is not the HTTP answer

For a `repo_id` over 256 bytes, `GrpcClient` records 400 `SCHEMA_VIOLATION` field `repo_id`. HTTP would send the request, and the handler would answer 403 or 409/400, depending on the credential. Both are 4xx, so the staged-copy result is the same (dropped). Configured repo ids are short, so this cannot happen in practice. Contract §7.1 already describes the pre-check. No change needed.

### I-5 (Info): ledger note without a run id

`state.json` notes: "main CI 기록: 0c243ee 뒤 PR #63 병합 493b712(X-03 수용 기록) main CI success" has no run id. Earlier notes have one. It is run **43697454428** (push, success).

### I-6 (Info): the pool's worker threads are not joined on stop

`GrpcServer::stop` returns after the tonic server and the runtime thread end. A worker may still be running a job whose future was dropped earlier. In the binary the process then exits, and PostgreSQL rolls the open transaction back. No change needed. One line in §6.2 or §11 would make this explicit.

## Summary of requested change

Fix M-1 (spawn guard in `tests/serve.rs`), get the three required checks green at the new head, and optionally L-1 in the same commit. Everything else can be merged as it is.
