# Review of PR #73 at a349dc6 (review 1)

- PR: HeaInSeo/ieo_poc#73, "02-05 step 2: supervisor operations getOwnAssignment, recordFirstAction, reportSessionStatus (HTTP and gRPC)". It also carries the step-2 REPORT and evidence, the step-1 REPORT header and post-review notes (review 23877c1 L-1, L-2, I-1..I-3), the archived review 23877c1 and ledger snapshot 0046.
- Head: a349dc6448bdc0df256778548500b12b3a9cf7fe. I confirmed it with `gh pr view 73 --json headRefOid,statusCheckRollup` before and after the review.
- Base: main 089fb95, which is also the merge base. Three commits sit on top:
  - 15b2f03: code, contracts and tests;
  - 0574abc: evidence (test log at 15b2f03, mutants);
  - a349dc6: step-2 REPORT, ledger 0046, step-1 REPORT fixes, archived review.
  - `git diff 15b2f03 a349dc6 -- crates contracts Cargo.lock` is empty, so the evidence applies to the head.
- CI at a349dc6 (run 43723155431, pull_request, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS, 358 passed and 0 failed in the job log
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan worktree, s2wt, PostgreSQL on 55433 or the coordinator on 58090. I used my own detached worktree (r73) and target directory (target-r73) and removed both at the end. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking or Medium findings. Three Low and six Info findings.**

- The three operations match PLAN §3.1 and step 2 with the documented deviations. None of the deviations breaks a later step's assumption or the acceptance sentence. Supervisor facts are claims that no judgment, sprint-state or event path reads.
- Scope is exactly the (repo, role) supervisor credential. Reusing `checkpoint:report` adds only claim writes and a read of the role's own assignment. The model process does not get the credential (`env.rs` allow-list).
- Idempotency, locking, rollback and the schema are sound. The migration is additive only.
- OpenAPI 0.10.0 and the proto are additive and match the code. `buf breaking` against origin/main passes, and the internal contract tables are accurate. Leaving the runtime contract unchanged is correct for this step.
- fmt, clippy and the full suite pass: 358/0 locally and in CI. The author's three original survivors are now killed. Three more of my mutations survive, for the same reason the REPORT gives: the common regression compares the two transports, not expected values (L-1).
- Docs and ledger are accurate apart from small points (L-2, L-3, I-5).

## What I ran

- `cargo fmt --all -- --check`: ok. `cargo clippy --workspace --all-targets -- -D warnings`: rc 0.
- `cargo test --workspace` with `IEO_TEST_DB_URL` on the throwaway DB on 55432: rc 0, **358 passed, 0 failed, 0 ignored**. grpc has 10 tests and coordinator has 75. This equals the REPORT, the test.log and CI. `checkpoint_e2e` passed on the first run this time.
- `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml`: OK.
- `buf lint`, `buf build`, and `buf breaking --against origin/main` (subdir contracts/proto): rc 0.
- Validators in `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` true, `generated_views_match` true, 583 internal links checked.
  - `render_progress.py --check`: ok.
  - `test_render_progress.py` and `test_summarize.py`: PASS.
- Relative links in the 8 changed .md files all resolve. A secret grep over the diff (password=, ghp_, github_pat, sk-ant, private keys, bearer values) found only the archived review's own sentence about that grep. test.log records that the DB URL value was not written.
- `cmp` of the archived `reports/tasks/02-05/step1-u2-c2-20261115/reviews/review-pr72-23877c1.md` against `pending-reviews/review-pr72-23877c1.md`: identical.
- `gh run view 43720550428`: 089fb95, push, success. This matches the new `notes` line and event.
- Probes: a temporary test appended to my copy of `tests/grpc.rs`, then removed. Results:

| # | Probe | Result |
| --- | --- | --- |
| P1 | session status with `[{"x":1e2}]` and with `-0` in a list object, identical bytes replayed over HTTP and gRPC | 201, then 200 and 200. The JSON-value replay holds for number spellings |
| P2 | session status with a NUL in `session_id` | 500 `INTERNAL` "database error" (I-1) |
| P3 | first action with `event_offset` = 2^64−1 over gRPC | 201. The column stores 2^63−1 while the body keeps 18446744073709551615 (I-2) |
| P4 | status at `05:00:00Z`, then a second one at `05:00:00.0000004Z` | the second is `stale_on_arrival: true`: PostgreSQL rounds to microseconds (I-3) |
| P5 | gRPC `idempotency_key` = `"  키키키  "` | 201. HTTP cannot carry this key in a header (I-4) |
| P6 | `provider_status_or_unknown: "whatever"`, `session_id: null` with `lifecycle: running` | 201 (I-6) |

- Mutations. Each was applied alone to `api.rs`, followed by `cargo test -p ieo-coord --test grpc`, and the file was then restored. `git status` was clean afterwards.

| # | Mutation | Result |
| --- | --- | --- |
| S2-M1 | takeover requirement removed (author's original survivor) | KILLED: `supervisor_operations_…` |
| S2-M3 | own assignment without `need_reporter` (author's original survivor) | KILLED: `supervisor_operations_…` |
| S2-M4 | first-action replay ignores content (author's original survivor) | KILLED: `supervisor_operations_…` |
| R1 | `need_reporter` without the role check | KILLED: both tests |
| R2 | `stale_on_arrival` with `>` instead of `>=` (equal time not stale) | **survived** (L-1) |
| R3 | `issued_here` always reports current | KILLED: `supervisor_operations_…` |
| R4 | takeover check ignores `session_id` (any takeover of the assignment and generation) | **survived** (L-1) |
| R5 | calendar check weakened to `1..=31` days (2026-02-30 passes to PostgreSQL) | **survived** (L-1) |

## (1) Semantics, deviations, claims and scope

**getOwnAssignment.**
- It reads `assignment_heads` ⋈ `assignments` ⋈ `sprint_revisions` for (repo, role). It returns:
  - the head;
  - sprint revision, digest and `sources[]`, restricted to the five SprintSpec v0 source keys;
  - the revision's `limits` (three keys; `unresolved_reason` is dropped);
  - `attempts_issued` from `task_attempts`, the policy digest, `issued_at`, and `last_delivery_id: null`.
- No head → 404 `NO_CURRENT_ASSIGNMENT`. The order is 401 → 404 role → 403 → 404.

**recordFirstAction.**
- The key is (repo, role, assignment_id, generation, attempt_id), and it is the PRIMARY KEY.
- Replay compares the stored jsonb with the parsed body: 200 when equal, 409 `EVENT_CONTENT_CONFLICT` when not. The replay runs before the assignment and takeover checks, so a stored record replays after the assignment moves, as the scenario shows.
- A new record needs `issued_here` (409 `STALE_ASSIGNMENT`) and a stored `takeover` checkpoint of that session under that assignment and generation (409 `CHECKPOINT_MISMATCH`).
- `current_assignment` is evaluated against the head at insert time.

**reportSessionStatus.**
- The PLAN item list is carried exactly: lifecycle, session_id, quota_group_id, provider status, last_actual_action_at, checkpoint seq and digest, pending external requests and children. `observed_at` and the assignment pair are added. Every key is required, and the pair rules hold.
- `stale_on_arrival` is true when any stored status of (repo, role) has `observed_at >=` the new one. The check runs under the lock.

**Documented deviations.** None blocks a later step:
- *First action requires a stored takeover.* This matches step 4's order: delivery, then takeover, then first action. Step 4's fallback ("실행 중 보고가 실패해도 … after_run 체크포인트에 같은 값을 넣어 다음 보고로 채운다") still works. The takeover row stays stored, and a late first action after the assignment moves is accepted with `current_assignment: false`. The only lost case is a `local_only` supervisor, which is a test configuration. The acceptance sentence wants the action linked to a takeover, so the precondition helps it.
- *getOwnAssignment omits the remaining amount, image/profile and `last_delivery_id`.*
  - Remaining amount cannot exist before step 5's metering.
  - Image and profile are coordinator configuration, not DB state. Step 3 will record them with the delivery, and adding them to the answer is additive in both JSON and proto.
  - Step 3's supervisor-side check reads assignment, generation, revision and digest, all of which are present now.
  - These items are tracked only in REPORT §5, not in the ledger (L-2).
- *Session status stored but not readable.* PLAN step 2 says "조회 때 … 마지막 보고와 그 시각을 보인다", and that half is deferred. Nothing reads the table yet, so the freshness rule has no consumer to violate. Deferring the read to step 4's chain read is reasonable, but it is a step-2 PLAN item moving to step 4 and should be tracked (L-2).

**Claims never feed judgment.**
- `role_first_actions` and `role_session_status` are referenced only by the three handlers (grep over `crates/*/src`).
- `judge`, the sprint state, the events and the outbox do not read them.
- The only other write is `history`, and `history` is read only by the `sprint.registered` check.
- Both tables have `evidence_level` with a `CHECK (= 'claim')`.
- The new test checks that the sprint state is unchanged and that only `history`, `role_checkpoints`, `role_first_actions` and `role_session_status` changed. The acceptance sentence ("에이전트 기록은 … claim으로 보존하고 완료 판정의 선행 신호로 요구하지 않는다") holds.

**Scope.**
- `need_reporter` requires kind `agent`, `checkpoint:report`, the repo in scope and the bound role equal to the path role.
- 403 for the model credential (`agent:report`), clients (`roles:control`), the assigner, the other role and the other repo, all asserted by expected code.
- The model process does not receive coordinator credentials. `ieo-supervisor/src/env.rs` clears the environment and passes only `HOME`, `PATH`, `LANG`, `CLAUDE_CONFIG_DIR` and the OAuth token (runtime contract §4.3).
- The same-UID file-read limit (runtime contract §2.1) remains and is stated. What it adds for a hostile development process that steals the supervisor token:
  - claim writes nobody reads;
  - a read of its own assignment, including `sources[]` and `limits`. `latestCheckpoint` already exposed the sprint, revision, digest and target SHA to that credential.
  - That is no meaningful widening (I-6 has a small note).

## (2) Idempotency, locking, rollback, schema, migration

- **Locks.** Both writes take `checkpoint:{repo}:{role}`, the checkpoint report's lock, before the key lookup. A takeover and a first action are therefore serialized, and the "stale" comparison is race-free.
- **Rollback.** Every refusal comes before the first INSERT. `run` rolls back on `Err`, and the equivalence test's DB snapshot confirms that refusals leave nothing. A PostgreSQL error after the INSERT (P2) also rolls back.
- **Session-status replay.** It is keyed by (repo, role, key) and compares the jsonb with the parsed body, so a different byte spelling of the same value replays (scenario row "replay as other bytes"; P1 for number spellings). gRPC carries `status_json` unchanged and maps an empty `idempotency_key` to no header. Cross-transport replay is tested both ways (`a_record_stored_over_one_transport_replays_over_the_other`).
- **`stale_on_arrival`.** "At or before" is the right reading of 02-04 item 4: a second status with the same time is not newer. The comparison is per (repo, role) across sessions and supervisor restarts, which the REPORT §6 states as a limit (clock comparisons only within one supervisor). Sub-microsecond differences collapse (I-3).
- **UTC timestamps.** `is_utc_timestamp` is careful:
  - fixed positions, digits checked before slicing, so it is UTF-8 safe;
  - a fraction of 1–9 digits, `Z` only;
  - year 1970–9999 and real calendar days, with leap years;
  - hour < 24 and seconds < 60 (no leap second).
  - Everything it accepts, PostgreSQL accepts.
- **Schema.**
  - Both tables use `CREATE TABLE IF NOT EXISTS`, with CHECKs (role, kind, `event_offset >= 0`, claim), FKs to `assignments`, and append-only triggers via the existing `forbid_change()` (`CREATE OR REPLACE TRIGGER`).
  - The migration for the persistent DB is additive only.
  - Nothing existing is altered, and `schema_meta` is unchanged, as with earlier additions.
  - The 58090 coordinator gets the tables at its next `migrate`, which the REPORT states.

## (3) Contracts

- **OpenAPI 0.10.0.**
  - Additive: three paths, `FirstActionReceipt`, `SessionStatusReceipt`, and `NO_CURRENT_ASSIGNMENT` in the Problem enum.
  - The scope sentence is corrected per 3f9909f I-1.
  - Bodies, required keys, order sentences and receipt shapes match the code. Minor gap: `session_id` in the status allows `""` per schema (no `minLength`) but the code refuses it (I-5).
- **Proto.**
  - Additive: three RPCs, new messages and `FirstActionKind`. Existing numbers are untouched, and `buf breaking` passes.
  - The `ieo-proto` conversions round-trip (unit test).
  - The gRPC adapter:
    - maps `idempotency_key` to the HTTP header (empty means none);
    - applies the `/ ? #` path check to `repo_id` for all three RPCs;
    - builds the first-action JSON with every field (unspecified kind becomes `""`, which gives 400 as on HTTP).
- **Internal contract.**
  - §1.1 rows, the §3 T1 text and check orders, §4.2 code rows, §5 retry rows, §8.2 and §11 are accurate against the code.
  - §13 lists them.
  - §2's statement that `status_json` is a byte document matches the adapter.
- **Runtime contract unchanged.** This is correct. The step adds coordinator endpoints that the supervisor does not call yet, and it changes no adapter field or supervisor behavior. The supervisor's own status JSON (`serve.rs`) has a different shape: `running`, `checkpoint`, `provider_status_detail`, `capability_gaps`, no `lifecycle`, and no list fields. So step 3/4 needs a mapping, and that belongs in revision 11 together with `delivery_id`.

## (4) Tests

- The equivalence scenario adds **63** rows for the new operations. I counted the `Do(` additions and the step-2 rows printed at the end of test.log. The REPORT says 72 (L-3). They cover:
  - auth and scope;
  - format;
  - replay and conflicts;
  - stale and old generations;
  - the old run after the move;
  - over-1-MiB, non-UTF-8 and non-JSON bodies;
  - the slash path.
- The `seen` list adds `(404, NO_CURRENT_ASSIGNMENT)` and `(409, IDEMPOTENCY_KEY_CONFLICT)`.
- I read every printed step-2 row in test.log, and each answer is the right one. Examples:
  - "session without takeover" gives 409 `CHECKPOINT_MISMATCH`;
  - "same time as the stored one" gives `stale_on_arrival: true`;
  - "bad time" gives 400.
  - But those rows assert equality across transports only, which is why R2, R4 and R5 survive (L-1).
- The new dedicated test asserts expected values for the main paths, and it kills the author's three original survivors.

## (5) Docs and ledger

- **Step-2 REPORT.** Accurate apart from the 72/63 count (L-3).
  - It states the deviations, the limits and the "no user decision needed" conclusion, which I agree with: no policy, judgment rule or existing HTTP behavior changed.
  - Its mutants.txt matches what I reproduced for M1, M3 and M4.
- **Step-1 REPORT fixes.**
  - L-1: the header now names c5464f5/c61708f → 386a223/1ad0a29, the merge 089fb95, the decision link and 356 → 357.
  - L-1 (one statement left): §5 still says "이 보고는 원장(`docs/poc/progress/*`)을 바꾸지 않는다" (I-5).
  - The L-2 note is sensible: a read-only comparison of the private bindings against DB ownership before redeploying 58090, and a report without changes if they conflict. It is scoped to "단계 8 준비(재배치)". The ledger's `next_action` says "단계 8(재배치 전 바인딩 읽기 전용 대조 포함)". So it is not a new prerequisite beyond step-8 preparation. Wording nit: there is no `sprints` table; ownership is `sprint_revisions.repo_id` plus `notion_imports` (I-5).
- **state.json diff vs 089fb95.** The changes are:
  - snapshot 0045 → 0046 and timestamps;
  - the 02-05 task: `last_observed_at`, `latest_report` → step-2 REPORT, `result_state` appended (step 1 merged, step 2 PR, "독립 검토 전"), +3 evidence entries, `next_action`, and `unresolved` appended (23877c1 L-2 and I-1). Status stays 진행 and verification stays unverified;
  - one `notes` line for main CI 43720550428;
  - one event, 02-05-S1-MERGED-20261115, mirrored in `events.jsonl`;
  - resume: `branch_sha`, `current_work`, `last_verified_stage`, `next_action`, `running` (feat/0205-s2), `saved_reports` +1.
  - Nothing else changes. 12/73 and 0/12 are unchanged, and `blocking` and `budget` are unchanged.

## Findings

### L-1 (Low): three rule mutations survive because scenario rows assert transport equality, not the expected answer

R2 (`>=` → `>` in the stale check), R4 (takeover check ignores `session_id`) and R5 (calendar check weakened, so the bad date reaches PostgreSQL as 500 on both transports) all pass the suite. The rules they break are stated in OpenAPI and REPORT:
- "at or before … `stale_on_arrival: true`";
- "a stored `takeover` checkpoint **of that session**";
- impossible dates are 400.

The current code is correct, as the printed rows show. The gap is that nothing pins it, which is exactly the REPORT's own diagnosis for M1, M3 and M4.

Suggested fix, in this PR or the next step: add direct assertions for these three behaviors to `supervisor_operations_answer_the_assignment_and_store_claims`:
- a status with the same `observed_at` gives `stale_on_arrival: true`;
- a first action for a session without its own takeover, while another session's takeover exists under the same assignment and generation, gives 409 `CHECKPOINT_MISMATCH`;
- `observed_at` `2026-02-30T00:00:00Z` gives 400 `SCHEMA_VIOLATION`.

Alternatively, let `Do` rows carry an expected (status, code).

### L-2 (Low): the step-2 deferrals are only in the REPORT, not in the ledger

REPORT §5 defers three things:
- the session-status read path (a PLAN step-2 item: "조회 때 supervisor에 닿지 못해도 마지막 보고와 그 시각을 보인다"), proposed for step 4's chain read;
- image/profile expectations in `getOwnAssignment` (step 3);
- `last_delivery_id` (step 3).

`tasks[02-05].unresolved` and `next_action` do not mention them. The acceptance report (step 9) works from the ledger's carry-over list.

Suggested fix: add one `unresolved` clause, e.g. "단계 2 이월: 세션 상태 조회 경로(단계 4 연결 조회), 자기 배정 조회의 이미지·프로파일·last_delivery_id(단계 3)".

### L-3 (Low): REPORT §3 says 72 new scenario rows; there are 63

`git diff` adds 63 `Do(` rows, and test.log prints 63 step-2 rows. The other numbers in §3 (358, 10, 75, the 359 sum in the log) are right.

Suggested fix: change 72 to 63.

### I-1 (Info): a NUL character in a string field is a 500

Example: `"session_id": "s\u00001"` in a session status gives 500 `INTERNAL` "database error", because PostgreSQL text and jsonb refuse U+0000. Both transports answer the same, and the transaction rolls back. `str_field` is shared with older handlers, so this is probably not new to this PR. I did not verify that.

A 400 would be more accurate. If you fix it, fix it in `str_field` and `parse_body` for all handlers.

### I-2 (Info): `event_offset` above 2^63−1 is clamped in the column

`i64_of` saturates, so the `event_offset` column says 9223372036854775807 while `body` keeps the sent value. A stream offset never gets there, but a 400 for values above `i64::MAX` (and `maximum` in OpenAPI) would keep column and body equal.

### I-3 (Info): sub-microsecond `observed_at` differences collapse

PostgreSQL stores microseconds, so a later status that differs by less than 1 µs is `stale_on_arrival: true`. The supervisor writes millisecond or second precision (`ieo-evidence::time::rfc3339`), so this is theoretical. A one-line note in OpenAPI is enough.

### I-4 (Info): gRPC accepts idempotency keys that HTTP cannot carry

The gRPC field accepts any 8–128-byte UTF-8 string, including spaces, non-ASCII characters and control characters. Example: `"  키키키  "` → 201. An HTTP header cannot carry such a key: `tiny_http` keeps header values as ASCII, and surrounding spaces are not preserved. So a status stored over gRPC with such a key cannot be replayed over HTTP. The contract (§8.2, proto comment) says "HTTP Idempotency-Key와 같은 규칙". Optional: restrict keys to visible ASCII on both transports.

### I-5 (Info): small wording points

- OpenAPI `reportSessionStatus.session_id` is `[string, null], maxLength 256` without `minLength: 1`. The code refuses `""` (`MISSING_FIELD`).
- Step-1 REPORT §5 still says "이 보고는 원장을 바꾸지 않는다". This was 23877c1 L-1's third bullet; the header is fixed, but this sentence is not.
- Step-1 REPORT L-2 note and ledger: "영속 DB의 `sprints` 소유". The tables are `sprint_revisions` (repo_id) and `notion_imports`.

### I-6 (Info): the status's provider value and the own-assignment read

- `provider_status_or_unknown` accepts any 1–64-byte string. Runtime contract §13 item 9 defines `available`, `usage_limit`, `auth_or_billing` and `unknown`. The status also does not carry the supervisor's `provider_status_detail` (the age of the observation). Nothing reads the field yet. When step 4 or the quota grouping reads it, validate the value set or store the detail.
- `getOwnAssignment` gives the `checkpoint:report` holder `sources[]` (refs and digests of the spec's sources). This is fine for the supervisor. It is one more thing a same-UID process that reads the token can see, under the limit runtime contract §2.1 already states.

## Summary for the author

- Approve at a349dc6.
- Optional, in this PR or the next step:
  - L-1: three direct assertions (equal-time stale, takeover of another session, impossible date).
  - L-2: put the step-2 deferrals in `tasks[02-05].unresolved`.
  - L-3: 72 → 63 in REPORT §3.
- I-1 to I-6 are notes only.
