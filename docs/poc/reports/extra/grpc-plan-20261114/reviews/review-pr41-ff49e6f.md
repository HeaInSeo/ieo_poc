# Review · PR #41 (docs/grpc-plan-20261114) · head ff49e6fcdf9cb020eab26eb1651bb2358a1c59e9

Reviewer: independent reviewer (read only, no repo/GitHub writes) · 2026-11-14 · review 2 of at most 4
Head confirmed: `gh pr view 41` headRefOid ff49e6fcdf9cb020eab26eb1651bb2358a1c59e9, mergeStateStatus BLOCKED. CI at review time: package validation pass, toolchain pass, rust fmt/clippy/test pending. Local HEAD is the same SHA. The correction is one commit, ff49e6f, on top of de64b7b.

## Verdict: CHANGES REQUESTED

Review 1's findings 1, 2, 5 and 6 are resolved, and findings 3 and 4 are mostly resolved. Two small text defects remain, both in the areas findings 3 and 4 covered. The draft gRPC surface in §1 names "existing" query operations that the coordinator does not have. The 02-05 ordering is still stated without qualification in plan §3 and in three state/TASKS fields, which contradicts the new X-04 row. Both are a few lines to fix. Nothing else blocks.

## Checks run

- `.venv-poc/bin/python scripts/validate_package.py`: rc 0. Amendment v1.3.3-A4, amendments A1..A4, generated_views_match true, progress accepted_main_tasks 11 / 73, followup_denominator 12.
- A4 hashes: the ledger hash d16a4669…7ac7ff4 and the validate_package.py hash 85242cd9…e9f50 match `sha256sum` of the working tree. The original_sha256 values are unchanged. The "원장 SHA-256" header of the generated views is 8fc1bf79…, which is the state.json digest, as before.
- `render_progress.py --check`: ok. `test_render_progress.py`: RESULT PASS 0.
- Consistency (checked programmatically): for X-03..X-07, the ledger §7 title, kind, prerequisites and acceptance are byte-equal to Amendment_v1.3.3-A4.json `extra_tasks`. The state.json `extra_tasks` prerequisites match, and status is 미착수 / unverified.
- Against origin/main: the ledger diff touches only §7 (the heading, the X-03..X-07 rows and one note). In state.json only snapshot_id/created_at, last_actual_work_observed_at, notes, events, resume and extra_tasks changed. Baselines, tasks, experiments and milestones are untouched. A4 has changed_acceptance [] and profile_revisions []. No accepted task's criteria changed. Counts stay 11/73 · 0/12.
- Code read: crates/ieo-coord/src/api.rs (router `handle`, lines 251–315), http.rs (whole), auth.rs (Kind, AGENT_SCOPES, CLIENT_SCOPES, loader rules), crates/ieo-supervisor (Cargo.toml and src), and the path list and SprintState schema in contracts/ieo-api-v0.openapi.yaml.

## Disposition of review 1 findings

1. **Resolved.** X-05 acceptance now carries five items: keep gRPC in the fallback; state the balancing path and settings in the deployment config; do not assume an existing stream moves to another Pod; no real Claude calls; check host resources and run the setups sequentially if needed. The ledger and A4 match.
2. **Resolved.** X-06 adds three clauses: no unconditional re-run on connection failure; include the case where send success and server-side application differ; commit the criteria in advance and do not change them after seeing results, plus no dev cluster or production DB. X-07 adds three: an isolated environment that does not change the dev cluster or production DB; host resources with sequential runs; criteria fixed in advance.
3. **Mostly resolved.** These parts are correct against the code:
   - The auth row now reads "기반만 있다", and "supervisor 주체 종류·범위는 아직 없다" is correct: auth.rs `Kind` is user/service/agent only.
   - The new row "supervisor가 실제로 쓸 작업 … 없다" is correct. The router and OpenAPI have no checkpoint, session status or takeover endpoint, and ieo-supervisor depends only on hex, serde_json and sha2, with no client. The row places the work in 02-05, which matches the 02-05 ledger text ("전달·실제 인수·첫 행동·체크포인트를 연결").
   - §1 now says which operation groups go over gRPC and which stay HTTP-only, and how the HTTP form of new operations is decided.

   One new inaccuracy is in finding 1 below.
4. **Mostly resolved.** The X-04 row and the closing paragraph of §6 now say that 02-05's prerequisites and acceptance criteria are unchanged and that 02-05 proceeds over HTTP if X-04 is not ready. That wording adds no prerequisite: the ledger 02-05 row is still "02-03 · 02-04", and A4 does not touch it. Leftover unqualified statements remain; see finding 2 below.
5. **Resolved.** Plan §3 now lists three behaviours: the 1 MiB body limit with 400 SCHEMA_VIOLATION (http.rs MAX_BODY, serve_one); the per-worker DB reconnect returning 500 "database unavailable" (serve); and the unauthenticated `/v0/health`, answered before the auth check and outside `run` (api.rs:253). Each is assigned to X-03 for its gRPC form. All three descriptions are accurate.
6. **Resolved.** Plan §6 step 1 now records PR #40 as merged (ccf7291). resume.plan_and_spec includes A4, and resume.next_action records that the next regular report must carry the gRPC placement and order.

## Findings (this review)

1. **[Medium] Plan §1 "gRPC 표면" names existing operations that do not exist, and §2 row 1 contradicts §1.** §1 says the surface carries "그에 필요한 기존 작업(배정·증거·판정 조회)". The coordinator has no query operation for assignments, evidence or judgments. The only GET routes are sprint revision, sprint state, sprint events, the repro request and the outbox. SprintState returns verified reports and latest claims, with no assignment or generation data. The existing assignment, evidence and judgment operations are writes. Each is held by a non-supervisor principal: `issueAssignment` needs `coordinator:assign` (service/user, never an agent), `ingestEvidence` needs `evidence:ingest`, and `recordVerdict` needs `judge:verify` (services only). §1 also lists "배정 조회·인수" as a supervisor call, which would be new, not existing. Separately, §2 row 1 still says the gRPC adapter routes all business rules through `handle`, including "Notion 가져오기·예외", while §1 now keeps Notion import and exception approval HTTP-only. Fix:
   - (a) Name the existing operations a supervisor would actually reuse, for example `POST /v0/agent/reports` and `POST /v0/agent/repro-requests` (agent:*), `GET /v0/sprints/{id}/state` (`/events` likewise). If evidence ingest is meant to come from the supervisor, say that this requires the new supervisor principal and scope.
   - (b) Mark assignment/evidence/judgment queries and the takeover operation as new (02-05/X-03).
   - (c) In §2 row 1, say the adapter routes only the §1 subset.
2. **[Low] "02-05 전에 완료" is still unqualified in four places, and one contradicts the new X-04 row.** The four places:
   - Plan §3 "02-04 영향" says "gRPC 전환은 X-04·02-05에서 한다". The X-04 row now says 02-05 may proceed over HTTP and the switch happens after X-04.
   - state.json X-03 `unresolved` reads "… 02-05 전에 완료".
   - state.json X-04 `next_action` reads "X-03 검수 뒤 착수. 02-05 전에 완료(…§6)".
   - The generated TASKS.md row repeats that `next_action`.

   Make these read as a target ("02-05 전 완료 목표, 02-05의 선행 아님") or reference the X-04 row, so the progress views do not show a gate the ledger does not have.
3. **[Low] The supervisor-side gRPC switch has no owner when 02-05 goes over HTTP.** The X-04 row says "gRPC 전환은 X-04 뒤에 한다". X-04's acceptance covers the coordinator gRPC path and the common regression. X-06 covers the real coordinator's gRPC path in kind. Neither covers switching the supervisor's `CoordinatorClient` to gRPC. If 02-05 completes over HTTP, that switch is untracked scope. The user asked for added scope to be tracked separately. Name the task that owns it, either by stating in X-04's or X-06's plan row that it uses the supervisor client, or by recording it as a follow-up note. No ledger change is needed if the plan names it.
4. **[Info] Accurate, no change needed.** These hold:
   - Adding the HTTP/JSON form of new operations by default fits the user's "keep HTTP and add gRPC".
   - Plan §3's 1 MiB, reconnect and health descriptions match the code.
   - X-05/X-06/X-07 add no authority beyond the decision file; X-07's "관측 기준은 실행 전에 정하고" follows decision §6 ("기본 경로와 같은 기준").
   - Nothing claims implementation or verification: all X tasks are 미착수 / unverified, and the library support is still marked "아직 확인 안 함".
   - The scope limits are kept: no budget change, no auto-retry, no count change, and the kind wording limit stays in §8 and scope_limit.

## Required before approval
Finding 1 (a)–(c) and finding 2. Finding 3 may be fixed in the same push. If the fixes are text only and the A4 ledger hash, the validator and generated views are refreshed and stay consistent, I expect to approve in the next review.
