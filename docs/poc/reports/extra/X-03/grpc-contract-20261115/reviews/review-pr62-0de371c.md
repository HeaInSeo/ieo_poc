# Review of PR #62 at 0de371c (review 2)

- PR: HeaInSeo/ieo_poc#62, "design: X-03 internal gRPC contract draft (ieo.internal.v0) — snapshot 0035"
- Head: 0de371c5d2e91317b8d115bb7029a969c9590866 (confirmed with `gh pr view 62 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 0779850 (07798503b8f7…). Two commits on top of the base: b8506fd (draft) and 0de371c (correction 1 of 3 for review b8506fd).
- CI at 0de371c (run 43695917074), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories. This is review 2 of at most 4.

## Verdict

**APPROVE: no Blocking and no Medium findings.** The correction fixes M-1, M-4, L-1 and L-2, fixes most of M-2, M-3, L-3 and L-4, and handles I-1 to I-4. The proto, `buf.yaml`, validators, buf and CI pass. The ledger diff contains only the intended changes.

Five new Low findings remain (N-1 to N-5). All are text fixes in `contracts/ieo-internal-v0.md`, and one also touches a proto comment. Three come from the correction itself: the UTF-8 rule, the path-parameter rule and the transport-limit sentence. They are edge cases, and none changes a business rule. Because the adapter calls the same handler, ordering errors in the text cannot make the two transports behave differently. They can only mislead X-04's tests.

The author can fix them in correction 2 or record them as X-04 inputs (§10). Either is acceptable. A new head would need a new review under the workflow.

## What I ran

- Made a detached worktree of 0de371c from `ieo_poc-0204` in my scratch directory and removed it at the end.
- Ran `git diff --quiet 0779850 0de371c -- crates contracts/ieo-api-v0.openapi.yaml`: no difference. The code I read is the base code.
- Read `git diff b8506fd 0de371c` in full, and the whole of `contracts/ieo-internal-v0.md` at the head.
- Read the code at the base line by line against §3 and §4.2:
  - `crates/ieo-coord/src/api.rs`: `handle`, `run`, `parse_body`, `str_field`, `u64_field`, `only_keys`, `denied`, `agent_identity`, `submit_report`, `validate_repro`, `submit_repro`, `get_repro`, `need_reporter`, `need_role`, `need_client_scope`, `report_checkpoint`, `latest_checkpoint`.
  - `crates/ieo-coord/src/http.rs` (all of it).
  - `crates/ieo-report/src/lib.rs`: `Denied`, `authorize_claim`, `authorize_repro`.
  - `crates/ieo-spec/src/rejection.rs`: `Code::http_status`.
  - `crates/ieo-supervisor/src/central.rs`: `call`, `report`, which uses `send_json` and `from_utf8_lossy`.
  - `crates/ieo-supervisor/src/serve.rs`: the staged-copy handling.
  - tiny_http 0.12.0 `Request::url`, which returns the raw path with no normalization.
- `docs/poc/User_Decisions_2026-11-14_grpc.md` §5 and §7, and `docs/poc/Plan_gRPC_Internal_API_v0.md` §1, §3 and §6 for the 02-05 wording.
- In `contracts/proto` (buf 1.47.2, libprotoc 34.1):
  - `buf lint` rc 0.
  - `buf build -o /dev/null` rc 0.
  - `protoc -I . --descriptor_set_out=/dev/null ieo/internal/v0/supervisor.proto` rc 0.
  - sha256 of `supervisor.proto` (5890c42c…) and `buf.yaml` (324fddb9…) match `evidence/proto-checks.txt`.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: ok, rc 0.
  - `scripts/validate_package.py`: rc 0. `schema_errors` 0. `ledger_spec_match` and `generated_views_match` are true. Progress is 12/73 accepted with followup denominator 12. 530 internal links checked (529 + the new review link).
  - `scripts/validate_contracts.py`: rc 0.
- `cmp` of the archived `reviews/review-pr62-b8506fd.md` and of both PR #61 reviews (`review-pr61-86e966d.md`, `review-pr61-79b40da.md`) against `.git/ieo-dev-workspace/pending-reviews/`: all byte-identical.
- Walked the JSON structure of `state.json` against 0779850.
- Checked every relative link in `contracts/ieo-internal-v0.md` and the X-03 `REPORT.md`: 13 of 13 resolve.
- Grepped the added lines for secrets: none. `git diff --check 0779850 0de371c` is clean.
- `gh pr checks 62 --watch`, then `gh pr view`.

## Status of review 1 findings

| ID | Status | Note |
| --- | --- | --- |
| M-1 500 "confirmed rollback" | **Fixed** | §4.1 row 500 now says "결과 불명" with the COMMIT reason. §4.3 last bullet, §5 "결과를 알 수 없는 경우" and the proto `Error` comment agree: 500 is unknown, and only a trailer `http_status` of 400–499 is a confirmed refusal. This matches `run`, where `tx.commit()` errors become 500, and the 02-04 split in `serve.rs`. |
| M-2 check order, `MISSING_FIELD` | **Mostly fixed** | Per-operation orders added. `MISSING_FIELD` added to the `SubmitReproRequest` and `ReportCheckpoint` rows, including `checkpoint_json` > 1 MiB → `MISSING_FIELD`. The report, repro and the two read orders match the code. The report and repro rows leave out the body-based 403s that follow parsing, but that has no observable effect: those 403s need a parsed body and they precede every 409. The `ReportCheckpoint` order is still incomplete (N-1). |
| M-3 size limits | **Mostly fixed** | §7.1 states the two layers correctly: the transport 1 MiB checked before auth, and the handler's `MISSING_FIELD`. The adapter applies the same 1 MiB limit before auth to the body it builds. The "SCHEMA_VIOLATION from the handler" error is gone. The rule holds for reports, where `report_json` is the HTTP body byte for byte. It holds for checkpoints when the adapter serializes `{seq, checkpoint_json}` the way the 02-04 client does (`ureq::send_json` gives compact serde_json, and key order does not change the size). The transport-limit sentence and the proto comment remain (N-3). |
| M-4 automatic retry | **Fixed** | Option (a) of review 1. §5 says there is no automatic resend of state-changing RPCs, and cites decision §7. Only read-only confirmation (`GetLatestCheckpoint`, `GetReproRequest`) is automatic, at most 3 tries with draft values that X-04 confirms (§11). The proto comments say "No automatic resend". This is consistent with decision §5: no re-execution on connection failure alone, confirm first. The 02-04 HTTP client does not confirm immediately; it confirms at the next start. The gRPC read-only confirmation adds reads, not executions. §11 lists the counts as draft values, which leaves them visible to the user. |
| L-1 `event_revision` width | **Fixed** | `uint64`. No breaking-check concern before the first merge. |
| L-2 server deadline/cancel, `57014` | **Fixed** | §4.3 lists server-made `DEADLINE_EXCEEDED`/`CANCELLED` as carrying no trailer. §6.1 maps the pre-start check and `57014` to `DEADLINE_EXCEEDED` without a trailer, which is unknown to the client. The proto `Error` comment agrees. |
| L-3 JSON equivalence | **Mostly fixed** | Non-optional fields are always emitted (`""`, `0`, `[]`), which gives the same `MISSING_FIELD` and `SCHEMA_VIOLATION` answers as an HTTP body carrying those values. The explicit-`null` limit is recorded, with `GetReproRequest` as the way to confirm. The new UTF-8 sentence is wrong for `report_json` (N-2). |
| L-4 02-05 wording | **Mostly fixed** | §8.2 and REPORT §4 now make the RPC addition conditional on X-04 being merged first. §1.4 still reads as unconditional, and §5 adds a new "02-05가 정한다" (N-5). |
| I-1 base SHA | Fixed | The header names 3157c89 (written) and 0779850 (review base, with no difference in `crates/` or `contracts/`). The evidence base is 0779850. |
| I-2 `tonic-health` | Fixed | §7.3: a periodic `schema_meta` read (5 s draft) records the status. The delay is stated. |
| I-3 path parameters, unknown roles | Partly fixed | The unknown enum value → `NOT_FOUND` is right. The path-parameter rule is not accurate (N-4). |
| I-4 `buf breaking` before the first merge | Fixed | §10 item 7. |
| I-5 per-code table | No change needed | — |

### Facts I re-verified for the correction

- §3 `SubmitAgentReport`: `agent_identity` (403 `SCOPE_DENIED`/`ROLE_MISMATCH`) → `parse_body` (400) → `authorize_claim`. Inside `authorize_claim`: body 403s, then replay 200 or `EVENT_CONTENT_CONFLICT`, then `ASSIGNMENT_SHA_MISMATCH`/`STALE_ASSIGNMENT`. Then `validate_report` (400/422, the lowest status wins) → `REPORT_SEMANTIC_VIOLATION` 422. The text is right.
- §3 `SubmitReproRequest`: `agent_identity` → `parse_body` → `authorize_repro` (403, replay or conflict, `STALE_ASSIGNMENT` including a missing generation or a different SHA) → `validate_repro` only on `Queue`. `MISSING_FIELD` is reachable with `""` values, which the adapter always emits. The text is right. The parse 400 cannot be reached over gRPC, because the adapter always builds an object. The text does not say so, which is harmless.
- §3 `GetReproRequest`: `ROLE_MISMATCH` for an agent that is not a guardrail with `agent:repro`, `REPO_SCOPE_DENIED` for a non-agent, then 404. `GetLatestCheckpoint`: `need_role` 404 → `need_reporter`, or `need_client_scope` plus repo → `NO_CHECKPOINT`. Both right.
- §4.2 code lists: every code each handler can return is listed. 403 `VERIFIED_FROM_AGENT_DENIED` appears only for reports, which is correct.
- The trailer rules agree across §4.1, §4.3, §5, §6.1, §7.2, §11 and the proto `Error` comment. The 02-04 split (4xx refused, 5xx/transport unknown) maps onto "trailer `http_status` 400–499 refused, otherwise unknown". The only exception is N-3: `RESOURCE_EXHAUSTED`.
- 02-05: §8.2 bullet 3 and REPORT §4 keep "02-05의 선행·수용 기준은 바뀌지 않음". The ledger adds no 02-05 prerequisite or acceptance item. Only X-03 fields changed in this correction.

## New findings

### N-1 (Low): `ReportCheckpoint` order in §3 is still incomplete

**Where:** `contracts/ieo-internal-v0.md` §3, the `ReportCheckpoint` bullet ("401 → 역할 이름(404) → 범위·repo·역할(403) → 형식(400) → repo·역할 불일치(409 REPO_MISMATCH) → 순번·내용 식별(200·409) → 배정 대조(409)"). The same order appears in REPORT §3 ("체크포인트는 형식 → REPO_MISMATCH → 식별 순서").

**Problem:** `report_checkpoint` at 0779850 has two more 400 steps that the text leaves out:

- After `REPO_MISMATCH` come the remaining field checks: `phase` and its values, `session_id`, `assignment_id`, `generation`, `sprint_id`, `sprint_revision`, `spec_digest`, `target_sha`, `image_digest`, `profile_id`, `profile_revision`, `settings_digest`. Each fails with 400 `SCHEMA_VIOLATION` `checkpoint_json.<k>`. A checkpoint that names another repo and lacks `phase` therefore gets 409 `REPO_MISMATCH`, not the 400 the text implies.
- After every assignment check, and after the INSERT (which is then rolled back), `config_tamper_detected` is checked: 400 `SCHEMA_VIOLATION` on `checkpoint_json.config_tamper_detected` or `….new_session_id`. A stale checkpoint with a malformed tamper block gets 409. A valid one with a malformed tamper block gets 400 at the end. A duplicate seq gets 200 without that check.

**Fix:** write the order as:

401 → 역할 이름(404) → 범위·repo·역할(403) → 본문·`seq`·`checkpoint_json`·JSON·`format`·seq 일치(400) → `REPO_MISMATCH`(409) → 나머지 필드(400) → 같은 seq 200·`EVENT_CONTENT_CONFLICT` → `CHECKPOINT_SEQUENCE_GAP` → 세대·배정 대조(409) → `config_tamper_detected`(400, 새 저장에서만).

Make the same change in REPORT §3.

### N-2 (Low): the UTF-8 rule is wrong for `report_json`

**Where:** §2, the third sub-bullet ("`checkpoint_json`·`report_json`이 UTF-8이 아니면 어댑터가 처리 전에 `INVALID_ARGUMENT`(`SCHEMA_VIOLATION`, 그 필드)로 거부한다. HTTP의 JSON 문자열은 UTF-8이 아닐 수 없으므로 같은 입력이 HTTP에는 없다"). Also §4.3 ("UTF-8 거부").

**Problem:** for `report_json` the HTTP equivalent exists. `report_json` is the HTTP body byte for byte, and an HTTP client can send non-UTF-8 bytes. Over HTTP that request gets:

- 401 if unauthenticated;
- 403 from `agent_identity` if the scope or role is wrong;
- otherwise 400 `SCHEMA_VIOLATION` with field `""` from `parse_body` ("request body is not a JSON object").

The adapter rule as written answers before `handle`, so before authentication, with field `report_json`. The status, field and order differ from HTTP. That breaks §10 item 1. The sentence about HTTP is true only for `checkpoint_json`, which travels inside a JSON string over HTTP.

**Fix:** pass `report_json` to the handler unchanged with no adapter UTF-8 check. `parse_body` already answers exactly as HTTP does. Keep the adapter check for `checkpoint_json` only, and say that there is no HTTP equivalent for it. The 02-04 HTTP client sends `from_utf8_lossy`. State whether that check runs before or after authentication. "After authentication" is closer to HTTP, where every body-content 400 follows 401. Update §4.3's list to match.

### N-3 (Low): the transport-limit sentence is not always true, and its refusal is classified as unknown

**Where:** §7.1 last bullet ("이 상한을 넘는 메시지는 HTTP 본문으로도 반드시 1 MiB를 넘는다 … 거부라는 점은 HTTP와 같고 상태 표현만 다르다"), §11 bullet 4, and the proto comment on `ReportCheckpointRequest.checkpoint_json` ("at most 1 MiB").

**Problem:**

1. "반드시" fails when the excess is in a field that is a path segment over HTTP: `repo_id` in `ReportCheckpointRequest` and `GetLatestCheckpointRequest`, and `request_id` in `GetReproRequestRequest`. Such a message is over 1 MiB + 64 KiB, but its HTTP body is small or empty. HTTP answers by route or handler, not with the 1 MiB 400. For bodies built from `report_json`, `{seq, checkpoint_json}` or the repro fields, the claim holds. The JSON overhead per field is at least the protobuf tag and length, apart from a few bytes on long repeated items, which the 64 KiB margin covers.
2. "거부라는 점은 HTTP와 같다" contradicts the client rule in §4.3 and §5. `RESOURCE_EXHAUSTED` has no trailer, so the gRPC client records **unknown** and keeps the staged copy. For the same checkpoint, HTTP returns 400, which 02-04 records as **refused** and drops the staged copy. That is a difference in the 02-04 split, and §10 item 6 says the two clients must pass the same staged-copy tests.
3. The proto comment "at most 1 MiB" on `checkpoint_json` is still the raw-bytes limit. The effective limit is now the HTTP-equivalent body: the file plus escaping plus the `{"seq":…,"checkpoint_json":…}` envelope must be at most 1 MiB.

**Fix:**

- Limit the sentence to the document-carrying RPCs.
- Have the gRPC client check the HTTP-equivalent size before sending and record a local refusal equal to HTTP 400 `SCHEMA_VIOLATION`. "보내기 전에 크기를 확인한다" is already there; add what it records. Then `RESOURCE_EXHAUSTED` is reached only by a non-conforming client, and the 02-04 split stays the same.
- Name the adapter's serialization: compact serde_json, as `ureq::send_json` sends it. The "same file, same answer" claim holds for that HTTP client, and another HTTP client's escaping or whitespace can make its body larger.
- Change the proto comment to "the HTTP-equivalent body `{seq, checkpoint_json}` is at most 1 MiB (description §7.1)".

### N-4 (Low): the path-parameter rule does not match HTTP

**Where:** §3, the last bullet ("HTTP에서 경로 조각이 되는 값(`repo_id`, `request_id`)이 비었거나 `/`를 담으면 HTTP는 다른 경로로 읽혀 404가 된다. 어댑터는 이런 값을 처리에 넘기지 않고 `NOT_FOUND`… 로 답한다"). The same rule appears in §4.3 and §10 item 8.

**Problem:**

- An empty `repo_id` is not a different route over HTTP. `handle` does `trim_matches('/')` and then `split('/')`, and tiny_http does not normalize `//`. So `/v0/repos//roles/development/checkpoints` matches the checkpoint route with `repo = ""`. It then gets `need_role` (404 only for an unknown role), then `need_reporter`: 403 `SCOPE_DENIED` without `checkpoint:report`, else 403 `REPO_SCOPE_DENIED`. The same holds for `…/checkpoints/latest`. Only an empty trailing `request_id`, and values containing `/`, give 404 "no such endpoint".
- Over HTTP the 404 always comes after 401, because `handle` checks the credential first. The adapter rule answers before `handle`, so before authentication.

**Fix:**

- Pass an empty `repo_id` to the handler, which then answers exactly as HTTP.
- For an empty `request_id`, or a value containing `/` (or `?`, which HTTP splits as a query), answer 404 `NOT_FOUND` only after the credential resolves, so 401 still comes first.
- Update §10 item 8.

### N-5 (Low): two places still read as 02-05 scope

**Where:**

- §1.4 third bullet: "02-05가 새로 만들 작업도 작업 표에 한 번만 정의하고, **HTTP와 gRPC 형태를 함께 둔다**(계획 §1의 기본값을 X-03에서 확정)". This was already in b8506fd and was not covered by L-4's locations.
- §5 `SubmitAgentReport` row, new in this correction: "다시 보낼지는 호출자의 규칙이 정한다(지금은 없음, 02-05가 정한다)".

**Problem:**

- §1.4 is unconditional and contradicts §8.2, where the RPC is added only when X-04 was merged first and is otherwise X-04's work. It also refers to the operation table, which exists only after X-04.
- "02-05가 정한다" gives 02-05 a decision that is not in its ledger acceptance. Plan §6 and decision §1 say gRPC must not add scope or prerequisites to 02-05.

**Fix:**

- §1.4: "02-05의 새 작업은 HTTP 형태를 둔다. gRPC 형태는 X-04가 먼저 병합됐으면 02-05가, 아니면 X-04가 더한다(§8.2)".
- §5: "(지금은 없음. 보고 호출자를 만드는 작업이 정하며, 이 계약이 02-05의 수용 기준을 더하지 않는다)".
- Optionally, mark the `idempotency_key` 8–128 line in §8.2 as design guidance. Review 1 L-4 asked for this, and the last bullet of §8.2 already covers acceptance.

### I-6 (Info): order of the adapter checks before `handle`

`http.rs` answers in this order:

1. a DB reconnect failure (500), before reading the body;
2. a body over 1 MiB (400);
3. `handle` (401 and the rest).

§3 and §7 do not order the adapter's own answers: DB unavailable (§7.2), size (§7.1), path (N-4) and UTF-8 (N-2). One sentence in §7 matching `http.rs` would let §10 item 5 test it. Also, §7.2's "클라이언트가 다시 보내도 된다" should say "safe to resend; whether to resend follows §5 (no automatic resend), and the 02-04 split treats it as unknown (`http_status` 500)". As written it can be read against §5's no-automatic-resend bullet.

## Ledger changes (state.json against 0779850)

The structural walk shows the same set as review 1, plus this correction's X-03 fields:

- `extra_tasks[2]` (X-03):
  - `result_state` appends the b8506fd review summary and "교정 1(검토 1/4·교정 1/3)".
  - evidence gains `reviews/review-pr62-b8506fd.md`, which is byte-identical to the pending copy.
  - `status` 진행; `next_action` unchanged.
- The PR #61 reviews are archived under `reports/tasks/02-04/acceptance-20261115/reviews/` and are byte-identical to the pending copies.
- Counts: 12/73 accepted, followup denominator 12. No main-task status, budget, usage, prerequisite or 02-05 acceptance field changed. The generated views match, and only the ledger SHA line and the X-03 TASKS row changed in this correction.
- `last_observed_at` stays at 17:32:49 while the evidence file is dated 17:44:30. This is harmless.

## Links, secrets, checks

- All 13 relative links in the two documents resolve. The validator checked 530 internal links.
- No secrets in the added lines. `git diff --check` is clean.
- buf lint and build, and protoc, pass. The evidence hashes match the files.
- The validators pass.
- All three required checks are SUCCESS at 0de371c (run 43695917074).
