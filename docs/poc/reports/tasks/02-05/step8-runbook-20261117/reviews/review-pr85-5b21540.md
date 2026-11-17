# Review of PR #85 at 5b21540 (review 1)

Reviewer: independent (did not write the PR). Head `5b21540f8b2e3d29758adc0385b48723830696a1`, base `main` `83bc84d`. The diff is exactly 3 commits: `149f82f` (tests and contract), `62c50c5` (evidence) and `5b21540` (REPORT, RUNBOOK, ledger 0057, archived review).

## Verdict

**Approve.** There are no High or Medium findings. Every finding of review b291d7c is addressed, and the text matches the code:

- **L-1.** The cause is now stated as unconfirmed, with two honest candidates. No text anywhere still claims the outgoing-connection mechanism as the cause.
- **L-2.** One `free_port` in `tests/common` is used by all 7 files that spawn a coordinator child.
- **L-3.** The L-A window wording is corrected.
- **I-1 to I-5.** Handled.

The suite passes: 411/411, and every e2e file passed in all 21 of my repeat runs. The new gRPC stop test passed 10/10 alone and 3/3 in its full file.

I raise two Lows and a few Infos:

- **L-1 (new).** The new contract section §7.3a says the gRPC side stops accepting at SIGTERM. In the code, gRPC keeps serving through the whole finishing wait (up to 780 s), and only then gets its 16 s grace.
- **L-2 (new).** The new test's timing bound is too loose to detect a graceful stop that never ends early. My mutation M1 survives every test.

## What I ran

All runs used the worktree at `5b21540` and the scratch target dir, with `-j 4` and `RUST_TEST_THREADS=4`. The test DB was 55432. Logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| What | Result | Log |
| --- | --- | --- |
| `cargo test --workspace --locked --no-run` | ok | `r85-build.log` |
| `cargo test --workspace --locked --exclude ieo-coord` | 288 passed, 0 failed | `r85-test-rest.log` |
| `cargo test -p ieo-coord --locked` | 123 passed, 0 failed (5 min). **Total 411/411**, which matches `test-pre8d.log` | `r85-test-coord.log` |
| `grpc a_call_in_progress_is_answered_across_the_stop` ×10 | 10/10 ok, each 3.75–3.77 s (so the stop returns about 3.2 s after it is called, before the 5 s grace) | `r85-grpcstop-x10.log` |
| full `--test grpc` ×3 (the stop test runs with 10 other tests) | 33/33 ok | `r85-grpc-full-x3.log` |
| `recovery_e2e`, `checkpoint_e2e`, `shutdown`, `chain_e2e`, `delivery_e2e`, `spare_e2e`, `limits_e2e` ×3 each | 21/21 runs ok, no flakes | `r85-e2e-x3.log` |
| `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -D warnings` | ok; ok | `r85-checks.log` |
| `validate_package.py`; `render_progress.py --check`; `test_render_progress.py` | rc 0, `schema_errors: 0`; rc 0; `RESULT PASS 0` | `r85-validate.log` |
| Leftover test DBs (`pg_database LIKE 'ieo_t_%'`, read with a throwaway `postgres` client) | none | inline |
| `/tmp/ieo-rcv-coord-*.log` after all runs | 0 files (I-1 confirmed) | inline |
| Ledger vs GitHub | see §6 | — |

Own hand mutations (each reverted with `git checkout`, `git status` clean afterwards):

| ID | Mutation | Result | Log |
| --- | --- | --- | --- |
| M1 | `grpc.rs:653` graceful shutdown future never resolves (`std::future::pending`), so every stop waits the full grace | **survived** everything I ran. `--test grpc` passes (12 s → 46 s), and `recovery_e2e` passes (25 s → 40 s). The new test's stop took about 5 s, which is still under its bound of 7 s (L-2) | `r85-mut-M1.log` |
| M2 | `shut()` drops the sender without sending `true` | survived. This mutant is **equivalent**: `wait_for` returns `Err`, which starts the graceful stop, and the bounded arm still sleeps the grace | `r85-mut-M2.log` |
| M3 | `grpc.rs:659` bounded arm sleeps `grace / 4` (1.25 s with the test's 4 s deadline) | **killed**: the new test fails at `grpc.rs:2384` with `answered across the stop: (0, {"grpc": "Unknown"})` | `r85-mut-M3.log` |

## Analysis

### 1. L-1: root-cause wording

- **REPORT.** `REPORT.md:195` now says "원인은 확인되지 않았다" (the cause was not confirmed).
  - Candidate (가): another `bind(0)` in the same process, namely the role `tiny_http` servers, the second port, or the parallel variant. It is rated most plausible.
  - Candidate (나): the coordinator's start itself takes more than 20 s on CI.
  - It explicitly says the outgoing-connection explanation is almost impossible, because of the kernel's parity split.
  - It still says "확정은 못 했다" (could not confirm it).
  - This is accurate and honest.
- **Small imprecision.** "실행당 약 1%" (about 1% per run) is ambiguous. The review estimated about 10⁻³ per gRPC-variant run and roughly 1% per CI run across the e2e files (I-1 below).
- **No leftover old mechanism.** I grepped `outgoing`, `나가는 연결`, `ephemeral`, `임시 포트/범위` and the CI run id over `crates`, `contracts`, `docs/poc/progress` and `docs/poc/reports/tasks/02-05`, excluding the reviews.
  - The only remaining mentions say the outgoing connection is *unlikely*: `tests/common/mod.rs:660-666`, `REPORT.md:195,208-209` and the `state.json` 02-05 `result_state`.
  - The `state.json` `unresolved` entry says "유력 추정(포트 충돌)이며 확정 못 함" (a likely guess, port collision, not confirmed). That is generic and still true.
  - `RUNBOOK.md:166` never stated a mechanism.
- **`free_port` doc comment** (`tests/common/mod.rs:660-666`). It now names the `bind(0)` mechanism and scopes the range claim to the default `ip_local_port_range` (I-2).

### 2. L-2: shared `free_port`

- **Code.** `tests/common/mod.rs:667-678` is byte-for-byte the previously reviewed function, so the range, the 131/12 768 cycle, the pid spread and the bind check are unchanged.
- **Coverage: all 7 coordinator-child files use it.** I grepped for `CARGO_BIN_EXE_ieo-coord`: chain, checkpoint, delivery, limits, recovery, spare and shutdown (2 sites). The five copies are gone.
  - The remaining `127.0.0.1:0` uses in `ieo-coord/tests` are listeners that stay held (`tiny_http`, the `grpc.rs:2120` relay, `GrpcServer::start("127.0.0.1:0")`). They are not bind-and-release patterns.
  - Two bind-and-release patterns remain outside the scope: `ieo-supervisor/tests/caller.rs:605` (with a 5× retry) and `:2789`. Both serve an in-process role service, not a coordinator child (I-3).
- **Parallel collisions.** Within one process, the atomic `NEXT` gives distinct offsets, so two calls cannot return the same port before 12 768 picks. Across processes, the bind check covers ports that are held. Only the pick-to-child-bind window remains, and the recovery diagnostics make it visible. 21/21 e2e runs and the full suite passed.
- **Mixing `mod common;` into the e2e files.** This adds the shared helpers, and `#![allow(dead_code)]` keeps clippy clean (verified).

### 3. L-3: the L-A window

`REPORT.md:188` now says that a signal after the re-check proceeds like any delivery past the check. The approval is used by a real run, and the stop waits for it (L-4). This matches `delivery.rs` (no further `STOPPING` check after the re-check) and the bin's two waits (`ieo-coord.rs:121-129,164-167`).

### 4. I-3: new gRPC stop test and contract §7.3a

- **The test** (`tests/grpc.rs:2354-2390`) is meaningful.
  - The call waits on the checkpoint advisory lock. `stop()` is called 0.5 s after the call starts, and the lock is released 2.5 s after the stop.
  - With `max_deadline` 4 s the grace is 5 s, so the call is answered (201, one checkpoint) across the stop.
  - M3, a bounded arm shorter than about 2.5 s, is killed, and so is the PR's own G2.
  - It is not flaky in my runs (13/13). Timing: 3.76 s total; the stop returned about 3.2 s after it was called.
- **The test has two weaknesses.**
  - The 500 ms sleep is the only thing that orders "the call is waiting on the lock" before the stop. In that window the call thread runs `prepare()`, which opens a new PG connection and inserts a row, then builds a channel and connects. Under heavy load it could still be connecting when `stop()` runs. Then it fails loudly with `(0, Unavailable)`, not silently (I-2).
  - The timing assertion `stopped < stop_grace + 2 s` (7 s) does not separate "graceful stop finished when the call ended" (about 3.2 s) from "cut at the grace" (5 s). M1 therefore survives (L-2).
- **Contract §7.3a** (`contracts/ieo-internal-v0.md:206`).
  - Correct: 15 + 1 = 16 s matches `GrpcConfig::default().max_deadline` (15 s) + `TRANSPORT_SLACK` (1 s) and `stop_grace`. "No longer call" matches `.timeout(max_deadline + TRANSPORT_SLACK)` at `grpc.rs:649`. "Streams such as `Health/Watch` are dropped" is correct.
  - Wrong: the first clause, "코디네이터가 멈출 때(SIGTERM/SIGINT) gRPC 쪽은 새 연결을 받지 않고" (when the coordinator stops, the gRPC side accepts no new connections). In `ieo-coord.rs:113-170` the signal thread first runs `begin_stop` and `wait_finishing_bounded()` (up to 780 s) while **both HTTP and gRPC keep serving**. This is deliberate, so that a role's after_run checkpoint over gRPC is still received (review 63fefc1 M-1). Only after HTTP is unblocked and `serve` returns does `g.stop()` stop accepting and start the 16 s grace.
  - This is a normative document. A supervisor implementer who reads it could conclude that reporting over gRPC after a SIGTERM is pointless (L-1).
  - The parenthetical cites PR `feat/0205-pre8c` for the test, which was added in pre8d. The doc comment on the test (`grpc.rs:2351`) also says "pre-step-8c" (I-4).

### 5. I-1, I-2, I-4, I-5 and the "no behaviour change" claim

- **I-1.** `Coord.log` = `dir.join("coordinator-stderr.log")` (`recovery_e2e.rs:455`). It is removed with `dir` at `:1642` and kept on a panic, where it is useful. No files were left in `/tmp`.
- **I-2.** Done (`common/mod.rs:665`).
- **I-4.** `RUNBOOK.md:37` now recommends registering after the redeploy. It also claims that a revision registered on 778ee47 "배정 때 새 빌드가 다시 검사한다" (is checked again by the new build at assignment).
  - `assign` (`api.rs:2691`, `:2850-2880`) checks the mode, `review_state`, `notion_gate`, `find_task` and `effective_limits`. It does **not** re-run `check_registration`/`check_spec`.
  - The practical risk is nil: `crates/ieo-spec` is unchanged between `778ee47` and HEAD (`git diff --stat 778ee47..HEAD -- crates/ieo-spec` is empty). The only new registration guard, `guard_sprint_repo`, applies only with bindings, and step 8 uses none.
  - The sentence overstates what is checked (I-5). The wording came from my earlier review's suggestion, which was itself imprecise.
- **I-5.** Recorded correctly in REPORT §13 and RUNBOOK §11. 780 + 16 ≤ 900 holds while the latest run bound is ≤ 884 s.
- **"동작 변경 없음" (no behaviour change; `REPORT.md:217`).** Confirmed. `git diff 83bc84d..5b21540 -- crates/*/src` is empty. Only tests, the contract text, docs and evidence change.

### 6. Evidence and ledger 0057 against GitHub

- **`test-pre8d.log`.** Commit `149f82f`, clean tree, fmt and clippy ok, 411 passed / 0 failed, no secrets in it (grep for `password`: 0).
  - REPORT §13 says the suite was run "세 번" (three times), all 411. The evidence file records only one run (I-6).
- **`mutants-pre8d.txt`.** G1 and G2 are killed, consistent with my M3. It does not include a "graceful never ends" mutant (see L-2).
- **Archived review.** `reviews/review-pr84-b291d7c.md` is identical to the reviewer's original (`diff`: same).
- **Snapshot.** `snapshot_id` is `v1.3.3-EXEC-0057`, created at `2026-11-17T12:51:41+09:00`.
  - The `sha256` of `state.json` is `b1845c3c…6870`, the same as in all four rendered views.
  - Counts 12/73 and 0/12 are unchanged, and no model calls were made.
- **`gh pr view 84`.** `MERGED` at 2026-11-17T03:28:38Z, merge commit `83bc84d36877…`, head `b291d7c`, branch `feat/0205-pre8c`.
  - This matches event `02-05-PRE8C-MERGED-20261117`. Its `observed_at` 12:51:41+09:00 = 03:51:41Z is later than the merge.
- **`gh run view 44010481541`.** Push at head `83bc84d`, attempt 1, conclusion `success`. All 4 jobs (docs validation, proto lint, toolchain pin, rust fmt/clippy/test) succeeded. This matches the event and the `result_state`.
- **`gh pr view 85`.** OPEN, head `5b21540`, base `main`.

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| L-1 | Low (contract text) | `contracts/ieo-internal-v0.md:206` | §7.3a says that when the coordinator stops (SIGTERM/SIGINT), the gRPC side accepts no new connections and gives calls in progress up to 16 s. In the code (`crates/ieo-coord/src/bin/ieo-coord.rs:113-170`), a signal first runs `begin_stop` and the finishing wait (default up to 780 s, or the latest run's bound). During that wait **gRPC keeps serving and accepting**, so a role's after_run checkpoint over gRPC is recorded (review 63fefc1 M-1). Only after HTTP's `serve` returns does `GrpcServer::stop` stop accepting and start the 16 s grace. An implementer who trusts the contract could stop reporting over gRPC once a stop is signalled, which would lose exactly the checkpoints that the wait exists to keep | Reword: "멈춤 신호 뒤에도 진행 중 전달·회차가 끝날 때까지(02-04 멈춤 대기, 기본 최대 780초) gRPC는 HTTP와 함께 계속 받는다. 그 뒤 새 연결을 받지 않고, 진행 중 호출에 … 16초 …" (after a stop signal, gRPC keeps accepting together with HTTP until the deliveries and runs in progress finish; then it accepts no new connections and gives calls in progress up to 16 s). Also cite pre8c for the behaviour and pre8d for the test |
| L-2 | Low (test strength) | `crates/ieo-coord/tests/grpc.rs:2386-2389` | The stop bound `stop_grace(4 s) + 2 s` = 7 s is above the 5 s grace. A server whose graceful stop never finishes early (M1: `grpc.rs:653` shutdown future → `pending`) cuts every stop at the full grace, and it **passes every test**: the new test (about 5 s < 7 s), the whole `--test grpc` (12 s → 46 s) and `recovery_e2e` (25 s → 40 s). In production that would be a silent extra 16 s on every SIGTERM. The test measures about 3.2 s today | Assert `stopped < stop_grace(Duration::from_secs(4))` (5 s), which leaves about 1.8 s of margin today, or ` < Duration::from_millis(4_500)`. That kills M1. Add M1 to the mutants list |
| I-1 | Info | `docs/poc/reports/tasks/02-05/pre-step8-20261117/REPORT.md:195` | "실행당 약 1%" (about 1% per run) reads as per test run. The estimate was about 10⁻³ per gRPC-variant run and about 1% per CI run across the e2e files | Say "CI 실행당 약 1%" (about 1% per CI run) |
| I-2 | Info | `crates/ieo-coord/tests/grpc.rs:2370-2373` | Only a 500 ms sleep orders "the call is blocked on the lock" before `stop()`. The call thread first runs `prepare()` (a new PG connection + INSERT), then connects the channel. On a heavily loaded runner the stop could come first. It would fail loudly with `(0, Unavailable)`, not silently. 13/13 passed here | Optionally, poll `pg_locks`/`pg_stat_activity` until a backend waits on the advisory lock (`wait_event_type = 'Lock'`) before calling `stop()`. Then the order is deterministic |
| I-3 | Info | `crates/ieo-supervisor/tests/caller.rs:605,2789` | Two remaining bind-(0)-and-release patterns serve an in-process role service, not a coordinator child, so they are outside L-2. `:605` retries 5×; `:2789` does not | None needed; use `free_port` there if they ever flake |
| I-4 | Info | `crates/ieo-coord/tests/grpc.rs:2351`; `contracts/ieo-internal-v0.md:206` | The doc comment says "02-05 pre-step-8c", and the contract cites `feat/0205-pre8c` for the test. The test was added in pre-step-8d (`149f82f`). The test comment's "past a 1 s grace" means a hypothetical grace of `TRANSPORT_SLACK` alone, but the actual grace in the test is 5 s | Say "pre-step-8d" and "past a grace of 1–2.5 s (G2), inside the 5 s grace" |
| I-5 | Info | `docs/poc/reports/tasks/02-05/step8-runbook-20261117/RUNBOOK.md:37`; `REPORT.md:214` | "778ee47에서 했다면 그 revision은 배정 때 새 빌드가 다시 검사" (a revision registered on 778ee47 is checked again by the new build at assignment). `assign` (`crates/ieo-coord/src/api.rs:2850-2880`) checks the mode, the semantic-review state, the Notion gate, the task and the limits, but does not re-run the registration checks (`check_registration`). There is no practical gap: `crates/ieo-spec` is unchanged since 778ee47, and `guard_sprint_repo` needs bindings, which step 8 does not use. (My earlier review suggested this wording) | Say "배정 때 새 빌드가 상태·Notion 게이트·task·한도를 다시 본다; 등록 검사(ieo-spec)는 778ee47과 같다" (at assignment the new build checks the state, the Notion gate, the task and the limits again; the registration checks in ieo-spec are the same as 778ee47's) |
| I-6 | Info | `REPORT.md:219`; `docs/poc/reports/tasks/02-05/step8-runbook-20261117/evidence/test-pre8d.log` | The REPORT says the full suite was run three times at `149f82f`, all 411. The evidence file holds one run | Append the other two runs' `test result` lines, or say "한 번 기록(세 번 실행)" (recorded once, run three times) |

## Summary

- **Must fix before merge:** none (no High or Medium).
- **Should fix (Low), can be a carry-over:**
  - L-1: the contract §7.3a first clause contradicts the stop order. gRPC keeps accepting during the finishing wait.
  - L-2: tighten the new test's stop bound to the grace, so a graceful stop that never ends early (M1) is caught.
- **Info:** I-1 to I-6 (wording, test ordering, out-of-scope bind patterns, attribution, the I-4 re-check claim, evidence for three runs).
- **Verified:**
  - All b291d7c findings are addressed. L-1 is honestly unconfirmed, and no old mechanism text remains. L-2 covers all 7 coordinator-child files. L-3 matches the code. I-1 leaves no logs in `/tmp`.
  - No source change; the "no behaviour change" claim holds.
  - 411/411 tests. 21/21 e2e repeats and 13/13 runs of the new stop test, no flakes.
  - fmt, clippy `-D warnings`, the validator and `render --check` pass.
  - Own mutations: M3 killed, M1 survived (L-2), M2 is equivalent.
  - Ledger 0057, PR #84 and run 44010481541 match GitHub.
  - No test DBs left behind.
