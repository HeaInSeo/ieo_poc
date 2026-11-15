# Review of PR #62 at 8e3e817 (review 3)

- PR: HeaInSeo/ieo_poc#62, "design: X-03 internal gRPC contract draft (ieo.internal.v0) — snapshot 0035"
- Head: 8e3e817dbe39b9fd8ff570b006e498100ef9ab72 (confirmed with `gh pr view 62 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 0779850 (07798503b8f7…). Three commits on top of the base: b8506fd (draft), 0de371c (correction 1) and 8e3e817 (correction 2 of 3, for review 0de371c).
- CI at 8e3e817 (run 43696529248), waited for completion:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS
- Reviewer: independent review agent, read-only, no model calls, no containers. I did not read the secrets or runs directories. This is review 3 of at most 4. Corrections used: 2 of 3.

## Verdict

**APPROVE: no Blocking and no Medium findings.**

Correction 2 fixes the following:
- N-1, N-2 and N-5, and I-6.
- Most of N-3: the client-side size pre-check is now the rule, and its "refused" classification matches the 02-04 supervisor rule.
- Most of N-4: an empty `repo_id` is now answered as HTTP answers it, and `/` now gets 404 after authentication.

Two Low findings remain (P-1, P-2), plus four Info notes. P-1 is the part of N-4 that is still open: an empty `request_id`, and `?` in path values. P-2 is that §7.1 now disagrees with §4.3, §11 and the proto `Error` comment on how to classify `RESOURCE_EXHAUSTED`. Both are edge cases that a conforming supervisor does not reach. Neither changes a business rule, adds an automatic resend, or touches 02-05.

**Nothing here has to block the merge.** Only one correction and one review remain, so I recommend merging 8e3e817 as it is and recording P-1, P-2 and I-7 to I-10 as X-04 inputs, for example in the X-03 acceptance record or in X-04's `unresolved`. X-04 has to write the tests in §10 item 8 anyway, and it will find these there. If the author still wants to fix P-1 and P-2 in correction 3, the fixes are text-only and listed below, but the new head would use the last review.

## What I ran

- Made a detached worktree of 8e3e817 from `ieo_poc-0204` in my scratch directory and removed it at the end.
- Ran `git diff --quiet 0779850 8e3e817 -- crates contracts/ieo-api-v0.openapi.yaml`: no difference. The code I read is the base code.
- Read `git diff 0de371c 8e3e817` in full, and all of `contracts/ieo-internal-v0.md` and the X-03 `REPORT.md` at the head.
- Checked the correction's claims against the code at 0779850:
  - `crates/ieo-coord/src/api.rs`: `handle` (path split and routes), `parse_body`, `submit_report`, `get_repro`, `need_reporter`, `need_role`, `report_checkpoint` (L3290–3538) and `latest_checkpoint`.
  - `crates/ieo-coord/src/http.rs`: all of it.
  - `crates/ieo-coord/src/auth.rs`: `in_repo`.
  - `crates/ieo-supervisor/src/central.rs`: `call`, `report`, `reconcile`, `not_observed`.
  - `crates/ieo-supervisor/src/serve.rs`: the staged-copy split (L1140–1167) and `local_checkpoint` (L328–372).
  - The OpenAPI `request_id` schema: any string of 8–128 bytes, with no pattern.
- In `contracts/proto` (buf 1.47.2, libprotoc 34.1):
  - `buf lint` rc 0.
  - `buf build -o /dev/null` rc 0.
  - `protoc -I . --descriptor_set_out=/dev/null ieo/internal/v0/supervisor.proto` rc 0.
  - sha256 of `supervisor.proto` (bbca6a70…) and `buf.yaml` (324fddb9…) match `evidence/proto-checks.txt`.
- From `docs/poc`, with `.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: ok, rc 0.
  - `scripts/validate_package.py`: rc 0. `schema_errors` 0. `ledger_spec_match` and `generated_views_match` are true. Progress is 12/73 accepted with followup denominator 12. 531 internal links checked.
  - `scripts/validate_contracts.py`: rc 0.
- `cmp` of the archived `reviews/review-pr62-b8506fd.md` and `reviews/review-pr62-0de371c.md` against `.git/ieo-dev-workspace/pending-reviews/`: both byte-identical.
- Walked the JSON structure of `state.json` against 0de371c and against 0779850.
- Checked every relative link in the description and the REPORT: 14 of 14 resolve.
- Grepped the added lines for secrets: none. `git diff --check 0779850 8e3e817` is clean.
- `gh pr checks 62 --watch`, then `gh pr view`.

## Status of review 2 findings

| ID | Status | Note |
| --- | --- | --- |
| N-1 `ReportCheckpoint` order | **Fixed** | See the order check below. The text now matches the code. One Info remains (I-7). |
| N-2 `report_json` UTF-8 | **Fixed** | §2 passes `report_json` unchanged. `submit_report` runs `agent_identity` (403), then `parse_body`. `serde_json::from_slice` fails on invalid UTF-8, which gives 400 `SCHEMA_VIOLATION`, field `""`, "request body is not a JSON object". This is exactly "401 → 403 → 400, field `""`". The UTF-8 refusal is limited to `checkpoint_json`, happens after authentication (§7.0), and is called gRPC-only. §4.3 and the proto `Error` comment ("checkpoint UTF-8") agree. |
| N-3 transport-limit sentence and its classification | **Mostly fixed** | The client pre-check is now the rule. Its classification is right (see the facts below). The proto comment now gives the HTTP-equivalent body limit. "반드시" is gone, and the path-value exception is stated. What remains: the serialization the client sizes is not named (I-8), and the new `RESOURCE_EXHAUSTED` sentence conflicts with §4.3, §11 and the proto (P-2). |
| N-4 path parameters | **Partly fixed** | Empty `repo_id` is correct, and so is `/` → 404 after authentication. Empty `request_id` is wrong, and `?` is not handled (P-1). |
| N-5 02-05 wording | **Fixed** | §1.4 is now conditional: HTTP form by 02-05; the gRPC form by 02-05 only if X-04 merged first, otherwise by X-04; 02-05 prerequisites and acceptance unchanged. The §5 `SubmitAgentReport` row no longer hands a decision to 02-05 and says not to resend until a rule exists. REPORT §2 item 4 still has the short unconditional form (I-9). |
| I-6 adapter check order | **Fixed** | §7.0 matches `http.rs` and `handle`: the DB reconnect check comes before the body is read; then size; then `handle`, whose first check is 401. A `/` value fails the route match after 401, so the adapter's 404 after authentication is the same position. §7.2 now defers resending to §5. |

### Facts I checked line by line

- **`ReportCheckpoint` order** (`report_checkpoint`, 0779850) runs in this order:
  1. `need_role` → 404.
  2. `need_reporter` → 403 `SCOPE_DENIED`, `REPO_SCOPE_DENIED`, `ROLE_MISMATCH`.
  3. `parse_body`, `only_keys`, `seq`, `checkpoint_json` (1 MiB → `MISSING_FIELD`), "not JSON", `format`, and `seq` ≠ `cp.seq` → 400.
  4. `cp.repo_id`/`cp.role` → 409 `REPO_MISMATCH`.
  5. The `t()` and `n()` field checks and `phase` → 400.
  6. `lock`.
  7. `seq <= last_seq` → 200 replay, or 409 `EVENT_CONTENT_CONFLICT`.
  8. `seq != last+1` → `CHECKPOINT_SEQUENCE_GAP`.
  9. The generation floor, current assignment and `continues`, then the per-field checks → 409.
  10. INSERT.
  11. The `config_tamper_detected` checks → 400, which rolls back through `run`.

  §3 states this order. Its consequence sentence is correct: a different repo with a missing `phase` gets 409. The REPORT §3 sentence agrees.
- **Empty `repo_id` over HTTP:** `handle` does `trim_matches('/')` and `split('/')`, so `/v0/repos//roles/development/checkpoints` yields `["v0","repos","","roles","development","checkpoints"]`. That matches the checkpoint route with `repo = ""`. `need_role` runs first, then `need_reporter`. `in_repo("")` is `repos.contains("")`, which is false for any configured credential. The result is 403 `SCOPE_DENIED` (without `checkpoint:report`) or `REPO_SCOPE_DENIED`. The same holds for `…/checkpoints/latest` with `need_client_scope`. The description's claim and example are right.
- **`/` in a value over HTTP:** the segment count changes, so no route matches (`…/checkpoints` and `…/latest` both have fixed lengths), and `handle` answers 404 `NOT_FOUND` "no such endpoint" after the 401 check. The repo-control route `["v0","repos",repo,"roles"]` (GET) has 4 segments, while a checkpoint path with a `/` in `repo_id` has at least 7, so no accidental match exists for the surface RPCs. "대개" is accurate.
- **Client pre-check and the 02-04 rule:**
  - What the 02-04 HTTP client does: it sends an oversized body and gets the transport's 400 before authentication. `central::report` turns any non-200/201 status into `reconcile(...)` with `coordinator_status` 400. `serve.rs` L1148–1155 then drops the staged copy, because 400 is in 400..500.
  - What §7.1 says the gRPC client does: it records the same case locally as a confirmed refusal, "the same as a 4xx", without sending.
  - Both clients end in the same state: the staged copy is dropped and the start is refused. For a non-takeover phase, both record a refused report. Because the request is never sent, no server state can differ. This is consistent with decision §5/§7: no resend is added.
- **No automatic resend added.** §5's "자동 재전송을 더하지 않는다" and its read-only confirmation (at most 3, draft values) are unchanged. The new §5 `SubmitAgentReport` text says not to resend until a caller rule exists. §7.2 now defers to §5. The proto comments still say "No automatic resend".
- **02-05.** §1.4, §8.2 and REPORT §4 all say that 02-05's prerequisites and acceptance do not change. Against 0de371c, the ledger changed only X-03's `result_state` and its evidence (the archived review). Against 0779850, the only 02-05 (`tasks[12]`) change is the `unresolved` note from the earlier rounds ("비차단 참고 … 02-05의 선행·수용 조건이 아니다"), which was already reviewed. No prerequisite or acceptance field changed.

## New findings

### P-1 (Low): an empty `request_id`, and `?` in path values, are not answered as HTTP answers them

**Where:** `contracts/ieo-internal-v0.md`:
- §3, the last bullet: "경로 조각이 되는 값(`repo_id`, `request_id`)이 빈 문자열이면 어댑터는 그대로 처리에 넘긴다. HTTP도 빈 조각을 같은 작업으로 읽으므로 같은 답…";
- §10 item 8.

**Problem:**

1. An empty `request_id` behaves differently on the two transports.
   - Over HTTP, `GET /v0/agent/repro-requests/` loses its trailing empty segment to `trim_matches('/')`. The segments are `["v0","agent","repro-requests"]`, no route matches, and the answer is 404 `NOT_FOUND` "no such endpoint" after 401.
   - Over gRPC, the adapter passes `""` to `get_repro`. A caller that is not a guardrail with `agent:repro` gets 403 `ROLE_MISMATCH` or `REPO_SCOPE_DENIED` instead of 404. A guardrail gets 404 `NOT_FOUND` with a different message ("unknown request").

   The sentence "HTTP도 빈 조각을 같은 작업으로 읽으므로" is true only for `repo_id`, an inner segment. For `request_id`, the last segment, it is false.
2. `?` (review 2 N-4 mentioned it) is not handled. `http.rs` splits the URL at the first `?`. `request_id` accepts any 8–128 bytes (OpenAPI, `validate_repro`). So `SubmitReproRequest` can store `abcdefgh?x`.
   - Over HTTP, `GET …/repro-requests/abcdefgh?x` looks up `abcdefgh`.
   - Over gRPC, the adapter passes the full value.

   A `repo_id` containing `?` gives an HTTP 404 (no route), while gRPC gives 403 from `need_reporter`. Both are different answers for the same request, which §10 item 1 forbids.

**Effect:** none for a conforming supervisor. Its `request_id` and `repo_id` come from its own configuration and generator. The §5 confirmation path (`GetReproRequest` after an unknown `SubmitReproRequest`) only ever turns a mismatch into "not found → outcome unknown", never into a resend. Only X-04's equivalence tests are affected.

**Fix** (text only, now or in X-04):
- §3: "빈 `repo_id`는 처리에 그대로 넘긴다(HTTP와 같은 답). 빈 `request_id`와, `/`·`?`(필요하면 `#`)를 담은 값은 인증 뒤 처리에 넘기지 않고 `NOT_FOUND`(trailer `http_status` 404)로 답한다."
- Add "빈 `request_id`, `?`" to the cases in §10 item 8.
- Optionally note that HTTP cannot address a stored `request_id` containing `/` or `?`. This is an HTTP limitation, and gRPC does not reproduce the truncation.

### P-2 (Low): how to classify `RESOURCE_EXHAUSTED` now differs between §7.1, §4.3, §11 and the proto

**Where:**
- §7.1, the last bullet: "클라이언트는 이를 결과 불명이 아니라 사전 계산을 어긴 클라이언트 결함으로 기록한다", and before it "위 사전 계산을 지킨 클라이언트는 이 상한에 닿지 않는다".
- §4.3, bullets 4–5: library `RESOURCE_EXHAUSTED` carries no trailer; "클라이언트는 trailer 없는 상태와 `INTERNAL`을 결과 알 수 없음으로 다룬다".
- The proto `Error` comment: "Without a trailer … the outcome on the server is unknown".
- §11, bullet 4: "HTTP는 400이다. 어댑터의 1 MiB 본문 검사 범위 안에서는 같은 답이다".
- REPORT §4: "크기 초과 응답이 HTTP와 다르다".

**Problem:**

1. §7.1 makes `RESOURCE_EXHAUSTED` "not unknown", while §4.3 and the proto make every trailer-less status "unknown". §7.1 also does not say which side of the 02-04 split a "client defect" falls on: whether the staged copy is kept or dropped. That is the only decision the classification feeds (§5, the last bullet; §10 item 6).
2. §11 still carries the correction-1 claim ("HTTP는 400이다"). The new §7.1 replaces it: the pre-check refuses locally, and a message over the limit because of a path value may not be 400 over HTTP.
3. "사전 계산을 지킨 클라이언트는 이 상한에 닿지 않는다" is not strictly true. The sentence after it says path values are excluded from the HTTP body size, so a conforming client with a `repo_id` longer than about 64 KiB still reaches the limit. The handler limits checkpoint `repo_id` fields to 256 bytes, but the pre-check as written does not.

**Effect:** none for a conforming supervisor. Its JSON body is at most 1 MiB and its `repo_id` is short, so it never reaches `RESOURCE_EXHAUSTED`. X-04 would have to pick one of two contradicting texts when it writes the client.

**Fix** (text only, now or in X-04):
- Choose one rule. The simplest keeps §4.3 and the proto as they are, so `RESOURCE_EXHAUSTED` stays outcome-unknown and the staged copy is kept. Then reword §7.1 to: "규칙을 지킨 클라이언트는 이 상한에 닿지 않는다. 닿으면 trailer 없는 상태이므로 결과 불명(§4.3)으로 다루고 클라이언트 결함으로 기록한다."
- Alternatively, make it an explicit refusal and add that exception to §4.3 and to the proto `Error` comment.
- Make the pre-check also bound the path values, for example `repo_id` ≤ 256 bytes and `request_id` ≤ 128 bytes, matching the handler and OpenAPI limits. Then "닿지 않는다" holds.
- Rewrite §11 bullet 4 to match §7.1: the pre-check gives the same refusal as HTTP 400; only a non-conforming message reaches `RESOURCE_EXHAUSTED`. Adjust REPORT §4 to match.

### I-7 (Info): name the two remaining 400 checks before `REPO_MISMATCH`

§3 groups everything before `REPO_MISMATCH` under "본문 형식(`seq`·`checkpoint_json`, JSON 파싱, 400)". The code also checks `format == "ieo-checkpoint/v0"` and `cp.seq == seq` at that point, both 400 `SCHEMA_VIOLATION`. The grouping is accurate, but naming the two checks lets X-04's test in §10 item 8 cover them directly.

### I-8 (Info): the serialization the client sizes is not named

§7.1 says the client sizes "같은 요청을 HTTP로 보낼 때의 JSON 본문", and the adapter builds "HTTP 본문과 같은 값". A value does not fix a byte count. The adapter and the client should both use compact serde_json, which is what `ureq::send_json` sends in `central::report`. Otherwise a checkpoint within a few bytes of the limit could be pre-refused by one side and accepted by the other.

A related point for §10 item 6: `central::report` sends `String::from_utf8_lossy(raw)`, while the gRPC client sends the raw bytes. A checkpoint that is not UTF-8 would be stored over HTTP, with a digest of the lossy bytes, and refused over gRPC. The supervisor writes checkpoints with serde_json, so this cannot happen today. It is worth one sentence in X-04.

### I-9 (Info): REPORT §2 item 4 keeps the short unconditional wording

REPORT §2 item 4 says "02-05의 새 작업도 HTTP와 gRPC를 함께 둔다(설명 §1.4)". It points to §1.4, which now carries the condition, and REPORT §4 and its last bullet state that 02-05 is unchanged. So the wording is not misleading in context. Adding "(gRPC 형태는 X-04 병합 순서에 따라, §8.2)" would make it read correctly on its own.

### I-10 (Info): adapter answers that carry a trailer are listed in two places

§4.3 bullet 1 and the proto `Error` comment list the adapter's trailer-carrying answers as "size, path, checkpoint UTF-8". The DB-unavailable `UNAVAILABLE` also carries a trailer, which §4.3 bullet 4 and §7.2 state. Adding "DB 연결 실패" to bullet 1 and to the proto comment would make each list complete. This finding predates this correction.

## Ledger changes (state.json)

- Against 0de371c, only X-03 (`extra_tasks[2]`) changed:
  - `result_state` appends "→ 0de371c 승인(Low N-1~N-5) → 교정 2(검토 2/4·교정 2/3)".
  - `evidence` gains `reviews/review-pr62-0de371c.md`, which is byte-identical to the pending copy.

  `status` (진행), `next_action`, `unresolved` and `last_observed_at` are unchanged. The generated views changed only in the ledger SHA line and the X-03 TASKS row.
- Against 0779850, the set is the same as in reviews 1 and 2 plus the above:
  - snapshot 0035;
  - the X-03 fields;
  - `events` +1, `notes` +1, `resume.*`, `phase`;
  - the PR #61 review evidence on 02-04 (`tasks[11]`);
  - the non-blocking PR #61 note in 02-05 `unresolved`;
  - X-08's `unresolved`.

  No main-task status, budget, usage, prerequisite or 02-05 acceptance field changed. The counts are 12/73 and 12.
- `REPORT.md` header row: the "상태" row was updated to link the 0de371c review and correction 2 (§6). The new §6 table lists N-1 to N-5 and I-6 and matches the description changes.

## Links, secrets, checks

- All 14 relative links in the two documents resolve. The validator checked 531 internal links.
- No secrets in the added lines. `git diff --check` is clean.
- buf lint and build, and protoc, pass. The evidence hashes match the files (`supervisor.proto` bbca6a70…, `buf.yaml` 324fddb9…).
- The validators pass.
- All three required checks are SUCCESS at 8e3e817 (run 43696529248). The head was still 8e3e817 after the checks finished.
