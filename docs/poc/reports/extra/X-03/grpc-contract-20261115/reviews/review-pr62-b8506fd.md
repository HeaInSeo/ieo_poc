# Review of PR #62 at b8506fd (review 1)

- PR: HeaInSeo/ieo_poc#62, "design: X-03 internal gRPC contract draft (ieo.internal.v0) — snapshot 0035"
- Head: b8506fd04bae1eae3e609af9f47ff4e836fa9de5 (confirmed with `gh pr view 62 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 0779850 (07798503b8f7…). One commit on top of the base.
- CI at b8506fd (run 43695238838), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories. This is review 1 of at most 4.

## Verdict

**REQUEST CHANGES: four Medium findings, no Blocking.** The proto and the ledger snapshot are in good shape. Every acceptance element from the ledger §7 X-03 row is present. The lint, build, validators and CI pass. The Medium findings are accuracy problems in the description document, which is part of the contract and is the basis for the X-04 regression tests:

- **M-1:** it says a 500 that carries a trailer is a "confirmed rollback". The code also returns 500 when COMMIT fails, and the outcome of a failed COMMIT is unknown.
- **M-2:** the check order in §3 and the per-RPC code table in §4.2 do not match the handlers.
- **M-3:** the size limits differ between transports more than §7.1 and §11 say.
- **M-4:** the automatic 3-retry client policy in §5 needs a reason that fits user decision §7, or it should be removed.

All four are text fixes. None needs new code, a new task or a new prerequisite.

## What I ran

- Made a detached worktree of b8506fd in my scratch directory and removed it at the end.
- `buf lint` (rc 0) and `buf build -o /dev/null` (rc 0) in `contracts/proto` with buf 1.47.2. `protoc -I . --descriptor_set_out=/dev/null ieo/internal/v0/supervisor.proto` (rc 0). The sha256 of `supervisor.proto` (bffa599d…) and `buf.yaml` (324fddb9…) match `evidence/proto-checks.txt`.
- `buf breaking` against 0779850 fails with "no .proto files". This is expected because the base has no proto. See I-4.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: ok, rc 0.
  - `scripts/validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` and `generated_views_match` both true, accepted 12/73, followup denominator 12, 529 internal links.
  - `scripts/validate_contracts.py`: rc 0.
- Read the code at the base: `crates/ieo-coord/src/{api.rs,auth.rs,http.rs,schema.sql}`, `crates/ieo-report/src/lib.rs`, `crates/ieo-spec/src/rejection.rs`, `crates/ieo-supervisor/src/{central.rs,serve.rs}` (staged-copy handling), `contracts/ieo-api-v0.openapi.yaml` 0.8.0 and `docs/poc/ProgressReport_v0.schema.json`.
- Ran `git diff --stat 3157c89 0779850 -- crates contracts`. It is empty, so the code the document cites (3157c89) is the same as the code at the base.
- Walked the JSON structure of `state.json` against 0779850. Compared the two archived PR #61 reviews with `cmp` against `.git/ieo-dev-workspace/pending-reviews/`.
- Checked by hand every relative link in `contracts/ieo-internal-v0.md` and in the X-03 `REPORT.md`, including the `contracts/` links. All 12 resolve.
- Grepped the added lines for secrets. `git diff --check` is clean.
- `gh pr checks 62 --watch`, then `gh pr view`.

## Acceptance (ledger §7 X-03, plan §1, §3, §4, user decision)

| Required element | Present | Note |
| --- | --- | --- |
| Services, messages, version compatibility (no field-number reuse, `reserved`, incompatible change → new package) | Yes | §2 and the proto header. `buf.yaml` sets STANDARD lint (only PACKAGE_VERSION_SUFFIX excepted) and WIRE_JSON breaking. |
| Auth and per-role permissions identical to HTTP | Yes | §3. Same `Credentials`, no new scope, names inside messages are not trusted. Development credentials are refused for repro. |
| HTTP↔gRPC status mapping, detailed reasons per existing code | Yes, with M-1 and M-2 | §4.1 rule table, §4.2 per-RPC codes, §4.3 `ieo-error-bin`. The plan's "per code" table is reduced to a rule over (HTTP status, code). With a 5-RPC surface this is adequate (I-5). |
| Per-operation retryability and idempotency (no re-execution on connection failure alone, re-query first) | Yes, with M-4 | §5 table. |
| Deadline and cancellation (rollback before commit, re-query after) | Yes, with L-2 | §6. |
| HTTP/JSON contract kept, business rules not redefined | Yes | §1.4 and §2. JSON documents travel as bytes and are checked by the same schema. No OpenAPI or code change in this PR. |
| Separation impact recorded | Yes | §9. Re-checked: `run` per route, DB-less role control, compare doing the supervisor call outside the transaction, health outside auth, `http.rs` behaviour, `central.rs` ureq with a 10 s timeout and the 4xx / 5xx split. |
| gRPC does not block 02-05 | Yes, with L-4 | §8.2 last bullet and REPORT §4 say 02-05's prerequisites and acceptance are unchanged. |

### Facts I verified as correct

- The 200 vs 201/202 split is right: report 202/200, repro 202/200, checkpoint 201/200. Mapping it to `Outcome` is sound. On a checkpoint replay, `tamper_recorded` and `current_assignment` are absent in HTTP (`{"duplicate":true,seq,checkpoint_sha256}`). The proto comment "set only when STORED" matches.
- Duplicate detection compares parsed JSON values. In `authorize_claim` it is `s == r`, where `s` is the stored jsonb report and `r` is the `serde_json` body. `authorize_repro` does the same with the stored body. Both run before schema validation.
- The checkpoint SHA-256 is over `raw.as_bytes()`, the decoded `checkpoint_json` string. That is the file bytes when the file is UTF-8. See L-3 for the non-UTF-8 case.
- The `ReproState` values match the `repro_requests.state` CHECK constraint and the OpenAPI enum.
- The `GetLatestCheckpointResponse` fields 1–17 match the `latest_row` keys exactly. Types are fine: the DB columns are non-negative bigints, so uint64 is safe.
- `SubmitReproRequestRequest` matches the `ReproRequest` schema (9 keys, `additionalProperties: false`, optional `timeout_seconds`).
- The `ClaimReceipt` fields match, except the width of `event_revision` (L-1).
- `get_repro` answers 403 before 404. `need_role` (404) runs before `need_reporter` in both checkpoint handlers, and 401 runs before both (in `handle`).
- The 02-04 supervisor rule matches §5: `serve.rs` keeps the staged copy unless `coordinator_status` is 400..499, and `central.rs` uses a 10 s timeout. Deciding by the trailer's `http_status` keeps the same split.
- Leaving out the sprint reads keeps `AGENT_READ_DENIED`. Reusing `agent:repro` for the guardrail supervisor matches `auth.rs`, where `agent:repro` requires role guardrail. Neither adds scope.

## Findings

### M-1 (Medium): "500 with a trailer = confirmed rollback" is not true for a COMMIT failure

**Where:** `contracts/ieo-internal-v0.md` §4.1, row 500 ("트랜잭션은 롤백됐다"), and §5 ("trailer가 있는 `INTERNAL`(HTTP 500)은 롤백이 확정된 실패다").

**Problem:** the closure `run` in `api::handle` maps a failed `tx.commit()` to `e.into()`, which is 500 `INTERNAL` "database error". COMMIT can fail after the server has already committed, for example when the connection drops during COMMIT. So a 500 that the coordinator itself answered can mean the state changed.

The 02-04 supervisor already treats every 5xx as "outcome unknown" and keeps the staged copy. §5's own `http_status` rule does the same. That makes the "confirmed rollback" sentence inconsistent with its own section too. A client that trusts the sentence could treat a committed checkpoint or claim as not applied.

**Fix:** in §4.1 and §5, say that a 500 means "rolled back, except a COMMIT failure, whose outcome is unknown". Treat every 500 like the unknown case: re-query with the same key. Alternatively, X-04 gives a COMMIT failure a distinct trailer marker. The simplest fix is to drop the "confirmed" wording. §5's `http_status` split already gives the right behaviour.

### M-2 (Medium): the check order in §3 and the code table in §4.2 do not match the handlers

**Where:** §3 "검사 순서" and the §4.2 table. The document says the table "was built by reading the code" and is what X-04's common regression must cover (§10 item 1).

**Problem 1, order.** §3 says the order is 401 → 403/409 (scope, role, current assignment) → 400 (format) → 409 (identity) → 422. The code does this:

- `submit_report`: 401 → 403 (credential kind or scope) → 400 (body not a JSON object) → 403 (body repo, channel or evidence level, role, report type) → 200 replay or 409 `EVENT_CONTENT_CONFLICT` → 409 `ASSIGNMENT_SHA_MISMATCH` or `STALE_ASSIGNMENT` → 400/422 schema (`validate_report`) → 422 `REPORT_SEMANTIC_VIOLATION`. The identity check comes before format validation, not after.
- `submit_repro`: 401 → 403 → (400 non-object) → 403 → 200 replay or 409 conflict → 409 stale → 400 format.
- `report_checkpoint`: 401 → 404 role → 403 → 400 (shape and the checkpoint's own fields) → 409 `REPO_MISMATCH` → 200 duplicate or 409 conflict → 409 `CHECKPOINT_SEQUENCE_GAP` → 409 stale, no current assignment or mismatch → 400 tamper fields (after the INSERT, then rolled back). Here format comes before identity, and no "current assignment" 409 precedes the 400s.

**Problem 2, missing `MISSING_FIELD`.**

- `SubmitReproRequest` can return 400 `MISSING_FIELD`. `validate_repro` uses `str_field` and `u64_field`. For example, `expected: ""` or `bundle_digest: ""` on a request that passes `authorize_repro` gives `MISSING_FIELD`.
- `ReportCheckpoint` can return 400 `MISSING_FIELD`: an empty or over-long `checkpoint_json` (`str_field(..., 1 << 20)`), or a missing `seq` on HTTP.
- Neither row lists `MISSING_FIELD`.

**Fix:**

- Replace the generic order in §3 with the per-RPC order above, or say only "the order is the handler's, unchanged". The adapter must not run its own checks first, apart from the transport checks in §7.
- Add `MISSING_FIELD`(400) to the `SubmitReproRequest` and `ReportCheckpoint` rows.
- Note that the 400 for a non-object body cannot be reached over gRPC for repro, because the adapter always builds an object.

### M-3 (Medium): the size limits differ more than §7.1 and §11 say

**Where:** §7.1, §11 ("크기 초과 응답 … 다르다"), and the proto comments "at most 1 MiB" on `report_json` and `checkpoint_json`.

**Problem:**

1. Over HTTP, the 1 MiB limit is on the whole body (`http.rs` `MAX_BODY`). The checkpoint file travels there as an escaped JSON string inside `{"seq":…,"checkpoint_json":"…"}`, so the largest checkpoint HTTP accepts is below 1 MiB. Escaping of `"` and `\n` makes it noticeably smaller. Over gRPC the raw bytes can reach 1 MiB (`str_field` max `1 << 20`), and the message limit is 1 MiB + 64 KiB. A checkpoint near the limit is therefore 400 over HTTP and 201 over gRPC. That breaks §10 item 1 ("the same request gets the same status"), and §11 does not record it.
2. §7.1 says the handler side answers `SCHEMA_VIOLATION` when a document is over 1 MiB. That is not true:
   - `submit_report` has no size check. Only `http.rs` enforces the limit, so the gRPC adapter must check `report_json` itself.
   - For `checkpoint_json`, the handler's over-length answer is `MISSING_FIELD` (from `str_field`), not `SCHEMA_VIOLATION`.
3. Ordering: HTTP answers 400 for an oversized body before authentication, so an unauthenticated oversized request gets 400, not 401. The gRPC adapter must do the same to match.

**Fix:** state the adapter rule explicitly. For example: "before calling the handler, the adapter rejects a request whose HTTP-equivalent body (the report bytes, or the serialized `{seq, checkpoint_json}` object) is over 1 MiB with 400 `SCHEMA_VIOLATION`, before authentication, like `http.rs`." Alternatively, record the remaining difference in §11 and in §10 item 5. Also correct the "처리 쪽이 … SCHEMA_VIOLATION" sentence.

### M-4 (Medium): the automatic retry policy needs a basis in the user decision, or removal

**Where:** §5, the "횟수" bullet: exponential backoff from 0.5 s up to 4 s, at most 3 confirm or resend attempts.

**Problem:** user decision 2026-11-14 §7 says "이번 요구사항을 이유로 예산을 확대하거나 자동 재시도를 추가하지 않는다". The sentence sits next to the model-call budget, and §5 correctly notes that these RPCs do not call the model. Still, the decision text is not limited to model calls.

The 02-04 HTTP client (`central.rs` / `serve.rs`) does no automatic resend. It records the unknown outcome, keeps the staged copy and decides at the next start. A gRPC-only 3× resend adds client behaviour that the HTTP path does not have, for the reason the user named. The same section also defers the values to X-04.

**Fix:** pick one of these.

- (a) Limit the automatic part to read-only confirmation calls (`GetLatestCheckpoint`, `GetReproRequest`). Keep resends of state-changing RPCs on the existing staged-copy / next-start path, which is the same as HTTP.
- (b) Keep the policy, but explain in the text why it does not conflict with §7: a same-key resend is a re-query by server rule and never re-executes. Then list it as a user-visible decision in §11, so the user can confirm it.

### L-1 (Low): `ClaimReceipt.event_revision` is `uint32`, but the source is unbounded

**Where:** `supervisor.proto`, `ClaimReceipt.event_revision = 2`.

**Problem:**

- The ProgressReport schema has `event_revision: {type: integer, minimum: 1}` with no maximum.
- The `events.event_revision` column is bigint, and `receipt()` returns it as i64.
- A report with `event_revision` ≥ 2^32 is accepted on both transports, but its gRPC receipt cannot carry the value.

**Fix:** use `uint64` (or `int64`). It is cheap to change now; after merge the breaking check would forbid it.

### L-2 (Low): server-made `DEADLINE_EXCEEDED` / `CANCELLED` and `statement_timeout` are not fully defined

**Where:** §4.3, §6.1.

**Problem:**

- §4.3 says every non-OK status the coordinator produces carries `ieo-error-bin`. It then lists `DEADLINE_EXCEEDED` and `CANCELLED` as statuses without a trailer.
- §6.1 has the server itself answer `DEADLINE_EXCEEDED` (the pre-start 50 ms check, and the pre-commit check). It does not say whether those answers carry a trailer, or which `http_status` they would carry, since there is no HTTP equivalent.
- A `SET LOCAL statement_timeout` expiry reaches the handler as a `postgres::Error` (SQLSTATE 57014). Through `From<postgres::Error>` it becomes 500 `INTERNAL` with a trailer, not `DEADLINE_EXCEEDED`, unless the executor maps it.

**Fix:**

- State that server-made deadline and cancel answers carry no trailer, so the client treats the outcome as unknown, which is safe.
- Make §4.3's "every non-OK" sentence exclude them.
- Say that the executor maps a statement-timeout error to `DEADLINE_EXCEEDED` after rollback.

### L-3 (Low): the JSON equivalence rule for typed and bytes fields is incomplete

**Where:** §2 ("작은 요청은 타입이 있는 필드로 싣는다", "JSON 문서는 바이트로 싣는다").

**Problem:**

1. `validate_repro` accepts `"timeout_seconds": null`, and the stored body keeps the null. The gRPC adapter cannot produce an explicit null. So an HTTP request sent with an explicit null and re-sent over gRPC compares unequal and gets 409 `EVENT_CONTENT_CONFLICT`.
2. The proto3 non-optional fields have no presence. The rule must say that the adapter always emits them, including `""` and `0`, so the JSON value equals a complete HTTP body. This also decides which code an empty field gets: `""` gives `MISSING_FIELD` from `str_field`, the same as an absent key.
3. `checkpoint_json` is `bytes`. The handler needs a `&str`. The HTTP supervisor client uses `String::from_utf8_lossy`. The adapter's answer to non-UTF-8 bytes is not specified.

**Fix:** add three sentences:

- non-optional fields are always emitted;
- an explicit JSON null of an optional field over HTTP and its absence over gRPC are different requests (or: the handler normalizes null to absent, which would be an X-04 change);
- non-UTF-8 `checkpoint_json` or `report_json` → 400 `SCHEMA_VIOLATION` from the adapter.

### L-4 (Low): the 02-05 wording reads as unconditional scope

**Where:** §8.2 bullet 2, and REPORT §4 ("02-05: 새 작업 … 을 작업 표와 이 서비스에 추가").

**Problem:** §8.2's last bullet and plan §3 make gRPC for 02-05 conditional on X-04 being finished first. REPORT §4 and the first bullets of §8.2 list "add the RPC to `SupervisorService`, add rows to the §4.2 and §5 tables, and an `idempotency_key` of 8–128 characters" as 02-05 work with no condition. Read alone, that adds deliverables to 02-05. Also, the operation table only exists after X-04.

**Fix:** prefix both places with "X-04가 병합된 뒤라면". Otherwise the work is X-04's when it converts the client. Mark the idempotency-key shape as guidance for the 02-05 design, not as an 02-05 acceptance item.

### I-1 (Info): the code reference is 3157c89, not the PR base 0779850

The document, REPORT and the evidence cite "main 3157c89". `git diff 3157c89 0779850 -- crates contracts` is empty, so the facts hold at the base. Consider saying "3157c89 (= 0779850 for crates/ and contracts/)" in the evidence file.

### I-2 (Info): `tonic-health` is push-based

§7.3 says `Check` returns SERVING when "the same read" as `/v0/health` succeeds. `tonic-health`'s reporter sets status from the server side and does not run a query per `Check`. It would need a periodic updater or a custom `Health` implementation. §7.3 already defers this to X-04. A short note would avoid a surprise there.

### I-3 (Info): path-parameter edge cases

- HTTP path parameters cannot be empty or contain `/`. GRPC strings can. For example, `GetReproRequest` with `request_id: ""` gets 403 or 404 from `get_repro` over gRPC, while HTTP has no such route.
- Unknown numeric `Role` values (proto3 open enums) should map to 404 like `ROLE_UNSPECIFIED`.

Neither changes a result that the HTTP path can express. One sentence in §3 or §7 would pin the behaviour.

### I-4 (Info): `buf breaking` against main fails until the first proto is merged

Against 0779850 it reports "no .proto files". §10 item 7 should run the check against main only after this PR is merged, or skip it when the base has no module.

### I-5 (Info): the per-code table in plan §4 item 3

The plan asked for a fixed per-code mapping, giving examples such as `OWNERSHIP_CONFLICT` and `IMPORT_GATED`. The rule table resolves those to `ABORTED` and `FAILED_PRECONDITION`, and §4.2 lists the reachable codes for the five RPCs. For this surface that is sufficient. It needs no change once M-2 is fixed.

## Ledger changes (state.json against 0779850)

The structural walk shows only intended changes:

- **X-03 entry:**
  - `extra_tasks[2]`: status 미착수 → 진행; `result_state`, `next_action`, `last_observed_at` and `latest_report` set; evidence gets REPORT and proto-checks.
  - `unresolved` gets the disposition of the b3cb083 question. Only text is appended.
- **PR #61 merge:** event `X-08-A6-MERGED-20261115` appended, and the CI note for main run 43694942714 added.
- **Review 79b40da fixes:**
  - N-1: X-08 `unresolved` "수용 보고 §7 항목 8" → "§4 항목 8".
  - N-2: a non-blocking pointer in 02-05 `unresolved` that explicitly says it is not a prerequisite or acceptance item.
  - N-3: `resume.current_work` no longer says "이 재개 기록 PR 다음".
  - 02-04 `unresolved` records the N-1 and N-3 fix.
- **02-04 evidence:** the two archived reviews, `review-pr61-86e966d.md` and `review-pr61-79b40da.md`. Both are byte-identical to the pending-reviews copies (`cmp`).
- **Resume and phase:** `branch_sha`, `running`, `next_action`, `last_verified_stage`, `saved_reports`, phase text, and the snapshot id 0034 → 0035 with timestamps.
- **Counts:** 12/73 and 0/12 unchanged (validator). No budget, usage or prerequisite field changed. No main-task status changed. The generated views match.

## Links, secrets, checks

- All relative links in the two new documents resolve, including the 6-level `contracts/` links from REPORT.
- No secrets in the added lines.
- The validators pass.
- buf lint and build pass.
- All three required checks are SUCCESS at b8506fd.
