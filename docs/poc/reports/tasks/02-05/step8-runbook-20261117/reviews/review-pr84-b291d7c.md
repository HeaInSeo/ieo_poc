# Review of PR #84 at b291d7c (review 1)

Reviewer: independent (did not write the PR). Head `b291d7ce188d54ea0ba98c503114968f1f05d8f7`, base `main` `ad96116` (merge-base = `origin/main` = `ad96116`, so the diff is exactly the 5 PR commits `79bf6a3`, `88b65d9`, `7519738`, `40fdfea`, `b291d7c`).

## Verdict

**Approve.** No High or Medium findings. The gRPC stop fix is correct and bounded, the L-A move works, I-A/I-B/I-C/I-E/I-F are addressed, and the RUNBOOK §1A order and the semantic-review body match the code. Three Low findings remain. The most important one, L-1, is about the root-cause analysis. The test fix (ports 20000–32767) is sound. But the mechanism written in the REPORT and `state.json` ("an outgoing connection takes the ephemeral port") is very unlikely on Linux. On this kernel, `bind(0)` always takes odd ports and `connect()` always takes even ones (measured below). A more likely mechanism is that another `bind(0)` reuses the port the test just released. The same fix covers that case. The other Lows are the same port pattern left unfixed in `checkpoint_e2e.rs`, and a wrong description of the remaining L-A window.

## What I ran

All runs used the worktree at `b291d7c` and the scratch target dir, with `-j 4` and `RUST_TEST_THREADS=4`. The test DB was 55432. Logs are in the scratchpad (`/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`).

| What | Result | Log |
| --- | --- | --- |
| `cargo test --workspace --no-run` | ok | `r84-build.log` |
| `cargo test --workspace` | **410 passed, 0 failed** (6 min) | `r84-test.log` |
| `recovery_e2e` ×4 | 4/4 ok (24–26 s each; the gRPC restart now ends by itself) | `r84-recovery-x4.log` |
| `chain_e2e`, `delivery_e2e`, `spare_e2e`, `limits_e2e` ×2 each | 8/8 ok, no flakes | `r84-e2e-x2.log` |
| `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -D warnings` | ok; ok | `r84-checks.log` |
| `validate_package.py`; `render_progress.py --check`; `test_render_progress.py`; `openapi-spec-validator` | rc 0, `schema_errors: 0`; ok; PASS; OK | `r84-validate.log`, `r84-checks.log` |
| Kernel port parity (python, 127.0.0.1) | `bind(0)`: 400/400 odd. `connect()` to 55432: 200/200 even. `ip_local_port_range` 32768–60999, kernel 4.18 | inline (below) |
| Immediate `bind(0)` after releasing one: same port again | 1 / 20 000 | inline |
| Ledger vs GitHub | see §6 | — |

Own hand mutations (each reverted with `git checkout`):

| ID | Mutation | Result | Log |
| --- | --- | --- | --- |
| MD | `begin_stop`: `get_or_insert_with(now)` → `*at = Some(now)` (a second signal resets the stop time) | **killed**: `delivery::tests::the_shutdown_waits_count_from_the_stop` | `r84-mut-MD.log` |
| MA | `grpc.rs:621` `grace = stop_grace(..) * 100` (in effect, no bound) | **killed**: `recovery_e2e` gRPC, `recovery_e2e.rs:1381` "the coordinator exits by itself on SIGTERM" (36.9 s) | `r84-mut-MA.log` |
| MF (diagnostic check) | `recovery_e2e.rs:401` `grpc_port = port` (forces a bind failure at the first start) | Fails in **0.56 s** with `exit Some(.. 512)`, plus the stderr text `ieo-coord: grpc listen 127.0.0.1:25291: Address already in use (os error 98)`. Before this PR it would have been a silent 20 s "did not start". The printed text is the listen address and the policy digest only, so no secrets | `r84-mut-MF.log` |

MA and MF left two test DBs behind because they panicked before the cleanup at `:1660` (`ieo_t_rcv_grpc_2398563` and `ieo_t_rcv_grpc_2399877`). I dropped both, and `pg_database LIKE 'ieo_t_%'` now returns 0. I also deleted the `ieo-rcv-coord-*.log` files that my runs left in `/tmp` (see I-1).

## Analysis

### 1. Root cause of main CI 44003424073 attempt 1

**"First start, not restart": supported.** In the saved log (`ci-44003424073-attempt1-failed.log:364-383`), `recovery_e2e` starts at 02:35:30.99. The non-gRPC variant passes at 02:35:36.29. The gRPC variant fails at 02:35:51.12, with "finished in 20.13s" and the panic at `recovery_e2e.rs:100`, which is the start-wait assert. The start wait is 20 s, so at most about 0.13 s passed before the failing `start()`. A restart only happens after the whole first half of the test plus a stop. So the failure is the first start, and the PR #82/#83 stop changes are not involved. The REPORT's argument (`REPORT.md:195`) is correct.

**The ephemeral-port explanation: the fix is right, but the stated mechanism is not the plausible one (L-1).** The REPORT and `state.json:383` say the most plausible cause is an *outgoing connection* (to PostgreSQL or a role) taking the released port as its local port. Since Linux 4.6, `__inet_hash_connect` scans ports of one parity first, and `inet_csk_find_open_port` (`bind(0)`) deliberately takes the other parity ("do the opposite to not pollute connect() users"). I measured this on this host: `bind(0)` gave odd ports 400/400 times, and `connect()` gave even ports 200/200 times. `connect()` falls back to the other parity only when the even half of the range is exhausted, which does not happen on a CI runner. GitHub's Ubuntu runners use 6.x kernels with the same logic and the same range. So an outgoing connection is very unlikely to take the port the test picked.

A mechanism that does fit:
- The test releases `port`, then calls `free_port()` again for `grpc_port` (`recovery_e2e.rs:400-401`).
- It then starts two role services with `tiny_http::Server::http("127.0.0.1:0")` (`:168`, `Role::new`) before `coord.start()` (`:493`).
- The non-gRPC variant runs at the same time in the same process and makes its own `bind(0)` calls.

All of these are `bind(0)` calls drawing randomly from the same odd half (about 14 100 ports). Each later call has about a 1/14 000 chance to hit a released port. I measured 1/20 000 for an immediate re-bind. That gives on the order of 10⁻³ per gRPC-variant run, and roughly 1% per CI run across the e2e files. If a role server or the second free-port pick took `port` or `grpc_port`, the coordinator exits 2 on its bind error, and the old test waits silently for 20 s. Exactly that was observed. The new `free_port` (20000–32767) is outside both the `bind(0)` range and the `connect()` range, so it removes **both** mechanisms. The fix is sound; only the explanation needs correcting. "Unconfirmed" is stated honestly in the REPORT ("확정은 못 했다") and in `state.json` `unresolved`.

**The new `free_port`: sound.**
- `131` is coprime with `12 768` (2⁵·3·7·19), so one process cycles through all 12 768 ports.
- The pid spreading (`pid*7919`) separates concurrent processes (other worktrees on this host).
- The bind check rejects ports held by other services. Rust's std sets `SO_REUSEADDR`, but on Linux that still conflicts with a listener on 127.0.0.1, 0.0.0.0 or dual-stack `[::]`.
- The remaining race is between the check and the coordinator's bind. It needs another process to pick the same port in that window, which is negligible. If it happens, the new diagnostics show it at once.
- `n * 131` overflows only after about 3·10⁷ calls.
- It cannot collide with 55432, 55433 or 58090 (all above 32767).

The comment "Ports from 20000-32767 are never given to outgoing connections" holds only for the default `ip_local_port_range` (I-2). The same pattern is still in `checkpoint_e2e.rs` (L-2).

**Diagnostics: correct and secret-safe.**
- The stderr file is opened in append mode, so a restart keeps the first start's lines.
- `try_wait()` makes the test fail as soon as the process exits, with its status and the file contents (MF: 0.56 s).
- The child is stored only after it answers. On a panic, the `Coord` drop therefore has nothing to stop, and the panicked child has already exited.
- What the coordinator writes to stderr: listen addresses, the policy digest, `db connect: {e}` (tokio-postgres errors do not contain the password), and stop messages. No tokens and no DB URL.
- One leftover: the log files are written to `temp_dir()` and never removed (I-1).

### 2. gRPC stop fix (`grpc.rs:617-667,700-707`)

- **Mechanism.** One `watch::Sender<bool>` and two receivers. `graceful` drives tonic's `serve_with_incoming_shutdown`. `bounded` drives `wait_for(stop) → sleep(grace)`. `tokio::select!` returns whichever finishes first. `wait_for` on a watch sees a `true` that was sent before it was polled, so there is no lost wake-up. If the sender is dropped without sending (`Drop` → `shut` always sends `true` first), `wait_for` returns `Err` and both arms end at once, the same as the old oneshot behaviour. No hang.
- **After the grace.** Dropping the serve future does not cancel the connection tasks that tonic spawned, but the thread then leaves `block_on` and drops the runtime, which cancels them. The DB pool uses its own detached threads, not `spawn_blocking`, so dropping the runtime does not block on them. MA shows that without the bound the process is SIGKILLed, and the tests show that with it the process exits 0.
- **Can a valid call be cut off? (G2 reasoning)** A unary call's handling time is `min(grpc-timeout, max_deadline = 15 s)`. Its DB work stops at `deadline − ANSWER_MARGIN` (50 ms) through the statement timeout and the pre-commit check. tonic's timeout layer cuts the call at 15 s + `TRANSPORT_SLACK` (1 s) after it started.
  - A call that started before the stop is therefore finished or already cut off by its own layer when the grace (`stop_grace` = 16 s after the stop) ends.
  - A stream that hyper accepted just before the GOAWAY starts a few milliseconds after the stop. Its work deadline is 15 s − 50 ms from its start, which still leaves almost 1 s of margin.
  - The only case of "answer after 16 s" is a commit that passed the pre-commit check and then took more than about 1 s. The timeout layer would cut that call off at the same moment, so nothing is lost that the old code would have kept. The commit is either done or rolled back by PostgreSQL when the connection closes.
  - Long-lived streams exist only in `grpc.health.v1.Health/Watch` (all `SupervisorService` RPCs in `supervisor.proto` are unary). Cutting those streams is the intended change.
  - G2 (a call that crosses the stop) is still untested, as the REPORT says.
- **Order.** The signal leads to `begin_stop` and the finishing wait while **both** HTTP and gRPC still serve, so a role's after_run report over gRPC is still received. Then HTTP is unblocked, `serve` returns, `g.stop()` runs (≤ 16 s), and the second wait follows. That order is unchanged (`ieo-coord.rs:118-170`), so records are as safe as before. The default total is at most 780 + 16 s, which is under `podman stop -t 900` (I-5).
- **Behaviour change.** Listed under "동작 변경(수용된 작업)" in REPORT §12 (02-04 stop, X-04 gRPC) and in RUNBOOK §11. The internal contract (`contracts/ieo-internal-v0.md`) has no stop section, so it needs no change (I-3 suggests one line).
- **Test.** `recovery_e2e.rs:1377-1386` asserts that the process exits by itself with status 0 within `stop_grace(15 s) + 5 s`. `stop()` now reports `killed` and success instead of hiding a SIGKILL. MA kills this test.

### 3. L-A and I-A to I-F

- **L-A** (`delivery.rs:952-963` before `:966` `record_use`). The re-check is now before the approval is used, so a stop before that point records `not_called` and leaves the approval unused. The one stop check moved earlier for all deliveries. For deliveries without a recovery, the extra code between the check and the call is only `sent_sha256` and building `o`, which is negligible. The remaining window, a signal between the check and the `record_use` commit, is described wrongly in `REPORT.md:188` (L-3). That delivery does not stop. `record_use` commits, `start_with` sends the start, and the run is accepted, charged and waited for (L-4 path). It is a normal run, not "approval only". The behaviour is acceptable; only the text is wrong. W1 survives, as the REPORT says (the timing cannot be made deterministic).
- **I-A** (`o.op = None` at `:809`, `:959`). Done. `spare_e2e.rs:482` asserts `op` is null for the UUID case.
- **I-B** (`delivery_e2e.rs:785-790`). Done. `dbody` maps the session through `sid()`, and the override sets the raw string `"s9"`, so a reconnect with a non-UUID body session is `accepted`.
- **I-C, I-E.** Done in the RUNBOOK (`:78`, the blank line in §9 removed).
- **I-D.** Handled by the REPORT text.
- **I-F** (`STOP_AT`, `wait_start`). Done. Both waits count from the first signal. MD kills the unit test. `begin_stop` sets `STOPPING` before `STOP_AT`. A reader in between falls back to `now`, which is harmless. The unit test sets the global `STOPPING`/`STOP_AT` in the lib test binary. No other lib unit test delivers or calls `wait_finishing_bounded`, so they do not interfere today (keep it that way).

### 4. RUNBOOK §1A and the semantic review

- **Order** (`RUNBOOK.md:37`, §2.3 at `:53`). Items 4 and 5 come after the redeploy and the §3.1 check, and §1 item 6 after them. That is consistent with §1's note (`:26`).
- **The semantic-review body** `{"decision": "passed", "reason": …, "digest": …}` matches `api.rs:2131-2152`: `only_keys(["decision","reason","digest"])`, decision `passed|rejected` (≤16), reason ≤4000, digest must equal the stored `sprint_revisions.digest` or the review gets 409 `DIGEST_MISMATCH`. The reviewer must differ from the registrant (`:2116-2129`, `SELF_REVIEW_DENIED`). No `Idempotency-Key` is needed (only registration and conversion use one). The digest query `SELECT digest FROM sprint_revisions WHERE sprint_id = … AND revision = …` matches the schema.
- **Bindings optional, and the C2 text.** Correct. `ctx.bindings` is read only by `reserved_for`/`guard_sprint_repo` (409 `SPRINT_REPO_CONFLICT`, `api.rs:942-963`), `import_notion` and `convert_and_register`. Step 8 registers with `POST /v0/sprints`, which needs no binding.
- **The option "on 778ee47 if it already has the principals"** is allowed by the text. A registration there runs 778ee47's checks, not the new build's (I-4).

### 5. Evidence files

- `ci-44003424073-attempt1-failed.log` matches the run (attempt 1 `failure`, head `ad96116`).
- `test-pre8c.log` (410 at 7519738) agrees with my 410 at b291d7c. The later commits change only docs.

### 6. Ledger 0056 against GitHub

- `snapshot_id` `v1.3.3-EXEC-0056`. The `sha256` of `state.json` is `43e85eb8…f9e8`, the same as the four rendered views.
- `gh pr view 83`: `MERGED` at 2026-11-17T02:29:57Z, merge commit `ad96116`, head `8336044`. This matches event `02-05-RUNBOOK-MERGED-20261117` (its `observed_at` 12:04:05+09:00 is the observation time, later than the merge).
- `gh run view 44003424073`: head `ad96116`, push. Attempt 1 is `failure` (`/attempts/1`), and the latest attempt (2) is `success`. This matches "첫 시도 실패 → 재실행 성공".
- PR #84 CI run `44007538045` at `b291d7c`: all 4 jobs pass.
- Counts 12/73 and 0/12 are unchanged, and no model calls were made.

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| L-1 | Low (analysis/docs) | `docs/poc/reports/tasks/02-05/pre-step8-20261117/REPORT.md:195`, `docs/poc/progress/state.json:383` (and the identical comment on the 5 `free_port` copies, e.g. `crates/ieo-coord/tests/recovery_e2e.rs:327-332`) | The stated "most plausible" mechanism, an outgoing connection taking the released ephemeral port, is very unlikely on Linux ≥ 4.6. `connect()` uses one parity (even here, 200/200) and `bind(0)` the other (odd, 400/400), and `connect()` falls back only when the even half is exhausted. A likely mechanism is another `bind(0)` reusing the released port: the second `free_port()` (`:401`), the two role `tiny_http` servers (`:168`), or the parallel variant's binds. Each has about a 1/14 000 chance (measured 1/20 000), roughly 1% per CI run. A future reader trusting the written mechanism may draw wrong conclusions | Rewrite the cause as "most likely another `bind(0)` in the same test or process (role services, the second port, the parallel variant) took the released port; an outgoing connection is unlikely because of the kernel's parity split". Keep "unconfirmed". The fix stays: 20000–32767 is outside both ranges |
| L-2 | Low (tests) | `crates/ieo-coord/tests/checkpoint_e2e.rs:246-253`; also `crates/ieo-coord/tests/shutdown.rs:51,167` | `checkpoint_e2e` has the same structure as `recovery_e2e`: two `bind(0)`-and-release ports, then role `tiny_http` `bind(0)` (`:111`), then a coordinator child. It is exposed to the same flake, which was not fixed ("다섯 종단 시험"). `shutdown.rs` is a smaller instance | Move `free_port` into `tests/common/mod.rs` and use it in `checkpoint_e2e.rs` and `shutdown.rs`. Also remove the 5 identical copies |
| L-3 | Low (docs) | `REPORT.md:188` (L-A row, "닫는 쪽: 승인만 쓰이고 예산·회차는 없음") | A signal between the new re-check (`delivery.rs:955`) and the `record_use` commit (`:966-969`) does not end with "approval used, no run". Nothing checks `STOPPING` again: the start is sent (`:995`), and the run is accepted, charged and waited for (L-4). This is acceptable behaviour, but described wrongly | Say: "a signal after the re-check: the delivery proceeds like any delivery past the check (the approval is used by a real run, which the stop waits for)" |
| I-1 | Info | `crates/ieo-coord/tests/recovery_e2e.rs:93-102` | The coordinator log `ieo-rcv-coord-{port}-{pid}.log` is written to `temp_dir()` and never removed. Two files per run of the test file pile up on developer hosts (13 after my runs; I removed them) | Write it inside the test's `dir` (removed at `:1659`), or remove it after a successful run |
| I-2 | Info | the 5 `free_port` doc comments | "Ports from 20000-32767 are never given to outgoing connections" holds only for the default `ip_local_port_range` (32768–60999) | Say "with the default `ip_local_port_range`" |
| I-3 | Info | `crates/ieo-coord/src/grpc.rs:700-707`; `contracts/ieo-internal-v0.md` | The gRPC stop semantics (calls in progress get up to `max_deadline + TRANSPORT_SLACK`, then open connections and streams such as `Health/Watch` are dropped) are documented in code and reports but not in the internal contract. G2 (a call crossing the stop) is still untested | Optional: one line in the contract, and a `grpc.rs` test with a slow call that crosses `stop()` |
| I-4 | Info | `docs/poc/reports/tasks/02-05/step8-runbook-20261117/RUNBOOK.md:37` | The option to register and review on 778ee47 when it already has those principals runs 778ee47's registration checks, not the merged build's | Prefer after the redeploy; or note that a revision registered on 778ee47 is checked again by the new build at assignment |
| I-5 | Info | `crates/ieo-coord/src/bin/ieo-coord.rs:118-170` | Worst-case stop is the finishing wait (780 s by default, or a longer run bound) + gRPC grace 16 s. That fits inside `podman stop -t 900` only while the latest run bound is ≤ about 884 s. This existed before the PR; the grace adds 16 s | None needed for step 8 (600 s window); keep in mind if windows grow |

## Summary

- **Must fix before merge:** none (no High or Medium).
- **Should fix (Low), can be a carry-over:**
  - L-1: correct the root-cause mechanism in the REPORT and `state.json`. The fix itself stays.
  - L-2: apply the same `free_port` to `checkpoint_e2e.rs` and `shutdown.rs`.
  - L-3: correct the L-A remaining-window sentence.
- **Info:** I-1 to I-5.
- **Verified:**
  - 410/410 tests; e2e repeats 12/12 with no flakes.
  - fmt, clippy `-D warnings`, the validator, `render --check` and the OpenAPI check all pass.
  - Own mutations MD and MA killed; MF confirms the new fail-fast diagnostics.
  - Ledger 0056 and the CI facts match GitHub.
