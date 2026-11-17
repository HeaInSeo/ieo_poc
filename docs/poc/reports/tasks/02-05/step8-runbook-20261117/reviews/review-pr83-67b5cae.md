# Review of PR #83 at 67b5cae (review 1)

## Verdict

**Request changes (2 Medium, 5 Low, 6 Info).** The code fixes for L-3 and L-4 are correct. They cannot deadlock, all tests pass, and the behaviour change is listed. The L-2 correction in REPORT §6/§11 matches `caller::spare_gate` and `caller::spare_releasable`. Two RUNBOOK gaps must be fixed before the user follows it:

- **M-1.** The RUNBOOK never says that `session_id` must be a UUID. The real `claude --session-id <uuid>` rejects anything else, and neither the coordinator nor the supervisor checks the format. A natural placeholder such as `s1` would therefore turn R1 into a failed run with no result. That run is most likely unmeasured, which stops the 02-05 budget and cannot be released by the spare.
- **M-2.** The spare request (§6 item 4) is a model call, and the restart (§8) changes the live system. Neither is marked **[실 시스템 변경]**. Neither asks for the user's per-case approval, which decision 2026-11-17 §4.5 requires for every spare.

## What I ran

Fresh detached worktree `.../scratchpad/r83` at `67b5cae374c03c5f4722a1f99c3b2c55190d31df` (base `5d69102`). All commands ran in the foreground with the given environment. Logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Check | Result | Log |
| --- | --- | --- |
| `cargo fmt --all -- --check` | ok | `r83-fmt.log` |
| `cargo clippy -j 4 --workspace --all-targets -- -D warnings` | ok | `r83-clippy.log` |
| `cargo test -j 4 --workspace` (test DB 55432) | **408 passed, 0 failed** (44 result lines). This matches the PR's `evidence/test.log` (408 at 1999494). `git diff 1999494..HEAD -- crates contracts Cargo.lock` is empty, so the evidence applies to this head | `r83-test.log` |
| `validate_package.py` (from `docs/poc`) | exit 0, `schema_errors: 0` | `r83-validate.log` |
| `render_progress.py --check` | `ok: true` | `r83-render.log` |
| Hand mutations A, B, C, E (below) | 1 killed, 3 survived | `r83-mutants.log` |
| Schema of the deployed build vs the head (`git show 778ee47:…/schema.sql`) | Exactly the 11 new tables the RUNBOOK lists. The only drops are the `evidence_kind_check` re-create and the old `role_recovery_uses` unique constraint. Every column is added with `ADD COLUMN IF NOT EXISTS` | `r83-old.txt`, `r83-new.txt` |
| `gh pr view 82`, `gh run view 43995607411` | MERGED, head `4531772`, merge `5d69102`, 2026-11-17T01:26:30Z. Run: push, `ci`, success, headSha `5d69102`. Ledger 0055 (`state.json` snapshot `v1.3.3-EXEC-0055`, event `02-05-PRE8-MERGED-20261117`, CI line) agrees | — |

I made no real model calls and no GitHub writes. I did not touch 55433, 58090, the secrets or the run records. After each mutation `git status` on `crates` was clean.

### Hand mutations

| ID | Mutation | Target | Result |
| --- | --- | --- | --- |
| A | `LATEST` is overwritten unconditionally (`if true`), so a later, shorter bound shrinks the wait | `--lib the_shutdown_waits` | **SURVIVED** |
| C | The run's own window is ignored (`finish_bound(window * 0)`) | `--lib the_shutdown_waits` | **SURVIVED** (the 780 s default hides it for 02-05; it matters for a 02-04 or X-02 window) |
| E | The default has one margin (720) | `--lib the_shutdown_waits` | KILLED |
| B | The second wait in `main` is removed (`let left = 0usize`) | `--test spare_e2e` | **SURVIVED** |

Together with the PR's own R4 (survived), only the arithmetic of L-3 is pinned by tests. The "longest bound wins" rule, the per-run window and both L-4 mechanisms are not. That is acceptable for a fail-closed shutdown path but should be recorded (see L-1).

## Analysis

### 1. Code: L-3 and L-4

- **The 780 s arithmetic is right.** `After::finish` waits window + margin, then after its stop another margin, then fetches the stream and transcripts and writes the record. So `SHUTDOWN_WAIT_DEFAULT_SECS = 600 + 2 × 60 + 60 = 780` (`delivery.rs:60-62`). `finish_bound(window)` adds the same 180 s to each run's own `timeout_ms + grace_ms` (`delivery.rs:910-916`).
- **The longest bound extends the wait.** `Finishing::start` keeps the maximum in `LATEST` (`delivery.rs:76-80`). `shutdown_wait()` takes `max(780 s, LATEST − now)`, and `IEO_COORD_SHUTDOWN_WAIT_SECS` overrides it completely, even downward. That override is documented.
- **L-4 counting order is right.** `_delivery` is counted (`delivery.rs:569`) before `STOPPING` is read (`:574`). The run guard `g` is started (`:916`) while `_delivery` is still alive, so `FINISHING` never drops to 0 between the two. A panic drops both guards.
- **No deadlock.** The signal thread polls the count without holding a lock. `LATEST` is held only for the update or the read. The HTTP workers keep serving during the wait, so the role's after_run report still lands. The second wait in `main` (`bin/ieo-coord.rs:167`) returns at once when nothing is counted.
- **Residual window (L-1).** The wait is computed once, when the signal arrives (`bin/ieo-coord.rs:121`), and is not re-evaluated. `STOPPING` is read only once per delivery. A delivery that passed the check just before SIGTERM can wait up to the 75 s start deadline for the ledger lock and then start a real run after the signal. That run's bound reaches about 780 + 75 s after SIGTERM, but `podman stop -t 780` sends SIGKILL at 780 s. The second wait in `main` cannot help there. It also cannot help the after_run report, because the server is already closed. The pre-stop checks make this unlikely, so it is Low. However, REPORT §11's L-4 sentence ("`main`의 두 번째 기다림이 같은 회차의 기록을 지킨다") overstates the protection under the RUNBOOK's own `-t 780`.
- **Bound on stop time.** If the first wait times out with runs left, the second wait in `main` waits up to another 780 s. Outside podman, a stop can therefore take up to about 1,560 s. The contract says the stop is bounded by `IEO_COORD_SHUTDOWN_WAIT_SECS`, which is only true per wait. Info, included in L-1.
- **Behaviour change.** It is listed in the contract text (`contracts/ieo-api-v0.openapi.yaml:15-18`) and in REPORT §3 item 3, marked "2026-11-17 정정, §11".
- **Unit test state.** The unit test leaves `LATEST` 5,000 s in the future for the rest of the lib test process. Only that test reads `shutdown_wait()`, so this is harmless today.

### 2. The L-2 correction

- **It matches the code.** `spare_gate` (`caller.rs:737-838`) requires all of the following:
  - the marker fields and a person;
  - exactly one raw record of the replaced attempt;
  - an effective record of this role, budget and `model_request`;
  - a failure (`stop_required`, or not a clean `success` / `is_error: false`);
  - `spare_releasable`: usage `measured`, and every `stop_reason` part either `result …` or `no result event`.

  REPORT §6 item 12, the rewritten "예비" bullets and RUNBOOK §6 item 2 say exactly this. The "error/takeover → unmeasured → refused" wording is right for `record_lost`. Once a spare names the failed record, `asks_for_a_person_but` (`caller.rs:419-436`) no longer lets that record block, so "R1 실패 → 예비 → R2" works as the RUNBOOK says.
- **Stale statements remain (L-2 of this review).**
  - REPORT §7, line 125, still describes "예비 + `recovery_id` 종단 … `error` 체크포인트 뒤의 예비(복구 승인 + 표식)" as a path that only lacks an end-to-end test, and proposes adding that test before step 8.
  - REPORT §10, line 148 (M-1 (b) disposition), still says "error·takeover면 복구 승인 + `recovery_id`".
  - Line 155 points to that §7 row.
- **Marking of the post-merge corrections.** The 720 → 780 edits carry an inline "(병합 뒤 780초로 정정, §11)". The in-place rewrites of §6 items 11 and 12 and of the "예비" bullets carry no inline marker; only §11 mentions them. §11 also says "§3·§6은 위 A의 글로 고쳤다", and "위 A" refers to nothing.

### 3. RUNBOOK against the code

The request bodies, field names, endpoints and scopes are correct:

| Request | Allowed keys or path | Code |
| --- | --- | --- |
| Assignment | `repo_id, role, assignment_id, generation, sprint_id, sprint_revision, task_id, target_sha, expected_current, limits, branch_ref` | `api.rs:2702-2716` (only_keys) |
| Evidence `branch_head` | `sha, ref, source_ref` | `api.rs:3295-3300` |
| Delivery | 9 keys; `attempt_id` and `session_id` must be `safe_id` (1-64 characters) | `delivery.rs:138-230` |
| Routes | deliveries, chain, evidence, assignments, health | `api.rs:392-456` |

Scopes:

- `coordinator:assign` for assignments (`api.rs:2692`).
- `evidence:ingest` for evidence (`api.rs:3337`).
- `roles:control` for deliveries (`api.rs:633`).
- `coordinator:assign` or `roles:control` for the chain (`chain.rs:34`).
- `checkpoint:report` for the agent principals (`auth.rs:56`).

Configuration:

- `budget_plan` keys (`budget`, `ledger_dir`, `first_observation.{stream, observed_unix}`, `roles.*.max_window_seconds`, `share` optional and equal to the approval) match `Plan::from_json` (`caller.rs:58-170`).
- The plan's role names must equal the configured roles (`delivery.rs:270-296`).
- `checkpoint_central {url, token_file, transport}` matches `central.rs:401-431`.

The budget numbers in §7 match `BUDGET_0205` (`budget.rs:114-143`), and the 7-day rule matches `budget.rs:148`.

Every SQL statement is a read-only `SELECT`, and the columns exist (`schema.sql:19-32, 198-206, 291-307`). The `notion_imports` and `sprint_revisions` checks are the same as in `check_bindings` (`api.rs:907, 919`).

| RUNBOOK step | Verified | Gap |
| --- | --- | --- |
| Header (approval, notation, secrets) | Yes. `tokens.json` stores only `token_sha256` (`auth.rs:101`), so listing principals cannot leak a token | The header's rule ("[실 시스템 변경] needs approval each time") is not applied to §6.4 or §8 (M-2) |
| §1.1 build | Yes | — |
| §1.2 no run in flight | (1) and (3) yes. (2) `fuser`/`lsof` (I-4 of the previous review) yes | The deployed 778ee47 has no `deliveries`, `chain` or `--roles`, so (4) cannot be run before the redeploy. At 778ee47 the only run path is the host `ieo-role-call` (L-3) |
| §1.3 new tables | Yes. These are exactly the 11 tables added since 778ee47 | — |
| §1.4 bindings | Yes, the same queries as `check_bindings` | — |
| §1.5 central checkpoints | The query is correct after the redeploy | Before the redeploy the table does not exist, so the query errors instead of returning 0 rows (I-5) |
| §1.6 sprint revision and L01 | The query and `spec->'limits'` are correct (`api.rs:2670`). The L01 decision is flagged | No step registers or reviews the revision or binds it (L-4) |
| §1.7 first observation | Yes (`caller.rs:549-575`, `budget.rs:291`) | It is time-bound: if the candidate is the 2026-11-15 stream, R1 must be sent by about 2026-11-22 (decision list) |
| §1.8 credentials | The scopes and kinds are correct | The agent principals with `checkpoint:report` cannot be in the running 778ee47 `tokens.json`, because 778ee47 rejects that scope at load (`git show 778ee47:…/auth.rs:54,122`). A new `tokens.json` (and the agent token files) is a live configuration change that no step describes (L-4) |
| §1.9 / §1.10 volumes and paths | Yes | — |
| §2.1 stop 58090 | Marked | It describes the new build's SIGTERM handling, but it stops 778ee47, which has no signal handler. The full 780 s then passes before SIGKILL; this is harmless and costs only time (L-3). Under the new build, `-t 780` has no slack (L-1) |
| §2.2 – §2.4 | The migrate-on-serve claim is right (`bin/ieo-coord.rs:82-85`), and the list of schema changes is right | — |
| §3.1 migration check | Yes (`evidence_kind_check`, 11 rows) | — |
| §3.2 `roles.json` | Yes (`delivery.rs:249-258` requires `url`, `adapter_token_file`, `image_digest`, `profile_id`, `profile_revision`) | — |
| §3.3 role services | Yes | — |
| §3.4 only the coordinator starts runs | Yes | The field is `capability_gaps`, and `central_checkpoint.mode` is the simpler check (I-1) |
| §4 assignments and `branch_head` | Bodies, `LIMITS_ABOVE_SPEC` and `branch_ref` → `not_observed` are right (`api.rs:2664-2686, 4587-4640`) | `expected_current: null` assumes that no current assignment exists for the repo and role. If one exists, the request gets a fail-closed 409, but no pre-check reads it (Info, in L-4) |
| §5 deliveries | Keys, judgments and ops are right. R2 → `resume_allowed` → reconnect only if the `branch_head` SHA equals `target_sha` (the pushed SHA is null) | **`session_id` must be a UUID (M-1).** Idempotency-Key must be 8-128 bytes. The client timeout must be at least 120 s, and a timed-out request must be resent with the same key (I-3) |
| §6 spare | The preconditions match the gate exactly, and the `new_session` advice is right for a run that failed before its session existed | §6.4 is not marked and does not ask for the user's per-case approval (M-2). Recommending `new_session: true` for a failed R2 gives up R2's purpose, the reconnect check (L-5) |
| §7 stop rules | They match decision 2026-11-15 §2.1 and 2026-11-17 §4.3: no automatic retry, G1 never retried, the spare only under §6 | It does not say that an unmeasured run is reported as unmeasured, never as 0, and it omits `pending_external_requests` (an unresolved external effect, §4.3) (I-4) |
| §8 restart | The four checks are right, and so is checking `local checkpoint … is not yet recorded` afterwards | The step is not marked [실 시스템 변경] and does not ask for approval (M-2). `-t 780` has no slack (L-1) |
| §9 abort and report | Yes | Same as I-4 |

**Can anything start a run by accident?**

- No step starts a run except §5 and §6.4.
- Creating assignments, ingesting evidence and starting the role services start nothing, because central mode refuses a takeover that no delivery called (`serve.rs:1192-1199`).
- The outbox `delivery` operation only marks state (`api.rs:3774-3800`).
- The real risks are a run that is wasted or left unmeasured: M-1 (a non-UUID session id) and L-1 (a stop that races a delivery in flight).

### 4. Secrets

The RUNBOOK prints no secret values:

- It reads `tokens.json` by name, kind and scope only, and that file holds only SHA-256 digests.
- Adapter and agent token files are referred to by path only.
- Role services must not carry `ANTHROPIC_*` variables.

It never shows how to pass a bearer token to `curl`. Recommended: `-H @<file>` or `--oauth2-bearer "$(cat file)"`, never echoed and never placed in shell history (I-3).

### 5. Ledger

- Snapshot 0055 agrees with GitHub (see What I ran).
- `docs/poc/progress/state.json:427-428` lists `review-pr82-63fefc1.md` twice (I-2).

## Findings

| ID | Severity | File:line | Scenario | Fix |
| --- | --- | --- | --- | --- |
| **M-1** | **Medium** | `docs/poc/reports/tasks/02-05/step8-runbook-20261117/RUNBOOK.md:65,71-73,90-92`; `crates/ieo-coord/src/delivery.rs:181-187`; `crates/ieo-supervisor/src/args.rs:48-52` | The delivery's `session_id` becomes `claude --session-id <id>` for a start (R1, G1, a spare with `new_session`). The CLI requires a UUID (`docs/poc/reports/tasks/01-02/runtime-contract-20261112/evidence/claude-help.txt:224`). The coordinator and the supervisor check only `safe_id`, and the fake runtime in the tests accepts anything. An operator who fills `<s1>` with `s1` gets an immediate CLI error, a run without `init` or `result`, and most likely an unmeasured record. That stops the 02-05 budget, and the spare cannot release it (`spare_releasable`: usage unmeasured). Even if the record is measured, it uses up the spare. Either way it wastes the plan's only development retry | In §5 (and §6.4) state that every `session_id` is a **new lowercase UUID** (for example `uuidgen \| tr A-Z a-z`), different for R1, R2, G1 and the spare, and checked before sending. Optionally, in code, make the delivery and the supervisor reject a start `session_id` that is not a UUID (a 400, before anything is recorded or called) |
| **M-2** | **Medium** | `RUNBOOK.md:77-95` (§6, especially item 4), `:108-113` (§8) | The header (`:8`) promises that every [실 시스템 변경] step gets the user's explicit approval each time, and decision 2026-11-17 §4.5 says the decision "does not grant in advance the per-case person's approval of a spare". §6.4 sends a real model call, but it is not marked and only says "사람 자격". An operator holding a `kind: user` credential could send the spare after their own judgment, without the user's per-case approval. §8's `podman stop` / restart is also unmarked | Mark §6 item 4 **[실 시스템 변경 — 모델 호출, 사용자의 건별 예비 승인]**. Require the user's approval of this specific case (the failed attempt, the cause report and the chosen `new_session`) before sending, and record that approval in the report. Mark §8 items 2-3 **[실 시스템 변경 — 사용자 승인]** |
| L-1 | Low | `crates/ieo-coord/src/bin/ieo-coord.rs:121-128,167`; `crates/ieo-coord/src/delivery.rs:569-582,906`; `RUNBOOK.md:40,112`; `docs/poc/reports/tasks/02-05/pre-step8-20261117/REPORT.md` §11 L-4 row | The stop is checked once per delivery and the wait is computed once. A delivery that passed the check just before SIGTERM can start a real run up to the 75 s start deadline later. Its record needs up to 780 s after that, beyond `podman stop -t 780`, so SIGKILL leaves it unmeasured. Without podman, the two waits together allow up to about 1,560 s. Mutations A, B and C, and the PR's R4, survive | Check `STOPPING` again right before `adm.start_with` (drop the admission and answer 503 / `not_called`, no call). Let `wait_finishing` re-read `shutdown_wait()` while it waits. Write `podman stop -t 900` (780 + the 120 s in-flight window) in §2.1 and §8. Soften the REPORT §11 L-4 sentence. Optionally add a unit test that pins `LATEST` as the maximum (mutation A) |
| L-2 | Low | `REPORT.md:125`, `:148`, `:155`, `:159` | §7 and §10 still describe the `error`/`takeover` + `recovery_id` spare path that §11 says cannot work, and §7 proposes an end-to-end test for it before step 8. The in-place §6 rewrites carry no inline post-merge marker. The phrase "위 A의 글" is unclear | Add "(병합 뒤 정정: 불가능, §11)" to lines 125 and 148, and drop or reword the §7 "단계 8 전 필요하면 추가". Mark §6 items 11 and 12 and the "예비" bullets "(2026-11-17 병합 뒤 정정, §11)". Fix "위 A" |
| L-3 | Low | `RUNBOOK.md:17-21,40` | §2.1 explains SIGTERM with the new build's behaviour, but the process being stopped is 778ee47, which has no signal handler (PID 1 ignores SIGTERM). The stop simply waits 780 s and then sends SIGKILL. §1.2 (3) and (4) assume role services and a chain endpoint that 778ee47 does not use, and its only run path is the host `ieo-role-call` | Say that for 778ee47 the stop costs the full `-t` and is safe only when no `ieo-role-call` is running and the 02-04 `caller.lock` is free. Mark §1.2 (4) as "재배포 뒤 적용" |
| L-4 | Low | `RUNBOOK.md:23-32,57-60` | Several live changes that step 8 needs have no step and no marker: a new `tokens.json` (agent principals with `checkpoint:report`, which 778ee47 cannot load) and the agent token files; registering the step-8 sprint revision (executable mode, values) and its semantic review by a second person (G3); the private bindings entry for that sprint; the choice of repo, task, `target_sha` and branch. §1.8, as a check against the running configuration, cannot pass before the redeploy. Nothing here costs budget (it fails closed), but the operator has no procedure for it. `expected_current: null` also assumes there is no current assignment, and nothing checks that | Add a "준비 [실 시스템 변경]" section between §1 and §2 that lists these files and writes and needs approval. Read the current assignment head (`SELECT assignment_id, generation FROM assignments WHERE repo_id=… AND role=…` or the book query) before §4 |
| L-5 | Low (decision) | `RUNBOOK.md:91` | For a failed R2, `new_session: true` starts a new session, so the plan's R2 purpose (a coordinator-mediated reconnect of the R1 session) is not re-tested. A marker alone would reconnect | Let the user choose per case (part of M-2's approval). Note that for R2 the default should be reconnect when the session exists |
| I-1 | Info | `RUNBOOK.md:52` | "능력 목록" is the field `capability_gaps`. `central_checkpoint.mode` shows `local_only` directly (`serve.rs:494,541-560`) | Name both fields |
| I-2 | Info | `docs/poc/progress/state.json:427-428` | `review-pr82-63fefc1.md` is listed twice | Remove the duplicate |
| I-3 | Info | `RUNBOOK.md:65` | Idempotency-Key must be 8-128 bytes (`api.rs:844-852`). A delivery can take up to the 120 s in-flight window before it answers, so a short client timeout followed by a resend with a new key is a second delivery. How to send the bearer token without printing it is not shown | Add these three lines |
| I-4 | Info | `RUNBOOK.md:97-106,126` | The report section does not say that an unmeasured run is reported as unmeasured (never 0). An unresolved external effect (`pending_external_requests` in the role status) is not among the stop and report conditions (decision 2026-11-17 §4.3) | Add both |
| I-5 | Info | `RUNBOOK.md:24` | Before the redeploy, `role_checkpoints` does not exist, so the query errors | Say "표가 없으면 오류 = 정상" or run it only after the redeploy (§3.5) |
| I-6 | Info | Mutations | A, B and C (and the PR's R4) survive | Record them with L-1 |

## Summary

- **Must fix before approval (docs only):**
  - **M-1:** require a fresh lowercase UUID for every `session_id`.
  - **M-2:** mark §6.4 and §8 as live changes, with the user's per-case spare approval per decision 2026-11-17 §4.5.
- **Should fix with them (docs, cheap):**
  - L-2: the remaining stale REPORT lines and the post-merge markers.
  - L-3: the 778ee47 stop.
  - L-4: a preparation section for tokens, sprint and bindings.
  - L-1's RUNBOOK part: `-t 900`.
- **Carry-over (code, optional, fails closed):**
  - L-1's code part: re-check `STOPPING` before the call, and re-evaluate the wait.
  - L-5 as a user choice.
  - I-1 to I-6.
- **Confirmed:**
  - The L-3 and L-4 code is correct, with no deadlock. The behaviour change is listed.
  - The L-2 correction is accurate against `spare_gate` and `spare_releasable`.
  - Every RUNBOOK body, field, endpoint, scope and SQL statement matches the code and the schema, and the SQL is read-only.
  - The stop rules match the decisions.
  - No secret is printed.
  - 408/408 tests pass. fmt, clippy, the validator and `render --check` are clean.
  - Ledger 0055 matches GitHub (PR #82 → `5d69102`, run 43995607411 success).

## User decisions the step-8 run still needs

1. **L01 values** for the step-8 sprint revision (already flagged in the RUNBOOK §1.6): approve 1,800 s / 150,000 for development and 600 s / 50,000 for Guardrail, or choose another way.
2. **Registering that revision:** register it in executable mode, have its semantic review done by a person other than the registrant (policy G3), and bind its `sprint_id` to the repo in the private bindings file. This needs the approval of each write and of the bindings change.
3. **The step-8 inputs:** repo, `task_id`, `target_sha`, `branch_ref`, and the three short prompts (R1, R2, G1). These are within the "읽기·답하기" scope, with no GitHub writes.
4. **Observation deadline:** if the first observation is the 2026-11-15 02-04 stream, R1 must be sent within 7 days of its `observed_unix` (about 2026-11-22). After that, the user must decide how to obtain a fresh no-overage observation, because the only source is a real stream.
5. **The spare's person, and approval per case:** who holds the `kind: user` credential. At a failure, the user's per-case approval of the cause report, the `replaces` attempt and the operation (for R2, reconnect vs `new_session: true`; L-5).
6. **Approval of each [실 시스템 변경] step:**
   - the redeploy window, including the 780 s stop of 778ee47;
   - the 55433 migration, after the backup has been taken and its location chosen (not `/home`);
   - the new `tokens.json`, `roles.json` and role containers and volumes;
   - each of R1, R2 and G1.
