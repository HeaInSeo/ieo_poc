# Review · PR #41 (docs/grpc-plan-20261114) · head b3cb083631fe79b76741babc0d4ac38713343f1a

Reviewer: independent reviewer (read only, no repo/GitHub writes) · 2026-11-14 · review 3 of at most 4
Head confirmed: `gh pr view 41` headRefOid b3cb083631fe79b76741babc0d4ac38713343f1a, mergeStateStatus BLOCKED. CI: package validation SUCCESS, toolchain SUCCESS, rust fmt/clippy/test pending at review time. Local HEAD is the same SHA. Correction 2 is one commit (b3cb083) on ff49e6f. It touches 6 files: the plan, state.json, and the four generated views.

## Verdict: APPROVE

Correction 2 resolves all three ff49e6f findings. It changes no ledger text, acceptance or prerequisite, and it reopens none of the de64b7b findings. The §1 surface claims match the code. Every "before 02-05" statement now reads as a target. The supervisor gRPC client has an owner (X-04). The counts stay at 11/73 and 0/12, and nothing claims work that has not been done.

## Checks run

- `.venv-poc/bin/python scripts/validate_package.py`: rc 0. Amendment v1.3.3-A4, amendments A1..A4, generated_views_match true, accepted_main_tasks 11, main_denominator 73, followup_denominator 12.
- `scripts/render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl). `scripts/test_render_progress.py`: RESULT PASS 0.
- Hashes:
  - A4's ledger sha256 d16a4669…ac7ff4 and validate_package.py sha256 85242cd9…e9f50 match the working tree. Neither file is in `git diff ff49e6f b3cb083`.
  - The generated-view header changed from 8fc1bf79… to d4c60630…, which is the new state.json sha256. This is the expected header convention.
- `git diff ff49e6f b3cb083`: plan §1, §2 row 1, §3 "02-04 영향" and the X-04 row of §6; state.json X-03.unresolved, X-04.next_action and X-04.unresolved; the regenerated view headers and the TASKS X-04 row. No other changes.
- Ledger: the 02-05 row is still "02-03 · 02-04" with the same acceptance. The diff against origin/main touches no 02-05 line. The X-03..X-07 rows are unchanged since review 2.
- STATUS shows 주계획 11/73 and 후속 실험 0/12. X-03..X-07 are all 미착수 / unverified. X-01 and X-02 are unchanged.
- Code read:
  - crates/ieo-coord/src/api.rs: the `handle` routes (lines 252–314), `need_client_scope` (318–342), `agent_identity`, `get_repro`, `ingest`, `judge` and `list_outbox`.
  - crates/ieo-coord/src/auth.rs: `Kind`, AGENT_SCOPES, CLIENT_SCOPES and the loader rules.
  - The path list in contracts/ieo-api-v0.openapi.yaml.

## Disposition of review 2 findings

1. **Resolved.** Each claim in §1 now matches the code:
   - The "기존 작업 중 재사용" list names the routes that exist: POST /v0/agent/reports, POST and GET /v0/agent/repro-requests[/{id}], GET /v0/sprints/{id}/state and /events. These match the router and the OpenAPI paths.
   - The caveat that agent credentials get 403 `AGENT_READ_DENIED` on /v0/sprints* is accurate (`need_client_scope` with sprints_path=true). Leaving supervisor read access to the 02-05/X-03 principal decision is correct.
   - "새로 만들 작업" marks assignment take-over and query, checkpoint/generation/sequence reports and session state as new. The statement that the coordinator has no assignment, evidence or judgment query is correct: the only GETs are revision, state, events, repro and outbox.
   - The scopes are attributed correctly. `coordinator:assign`, `evidence:ingest` and `judge:verify` are client scopes that agents can never hold, and `judge:verify` is service-only in both the loader and `judge`.
   - Outbox, assignment and judgment stay HTTP-only, which is consistent with this.
   - §2 row 1 now says the adapter routes only the §1 subset. This removes the contradiction.
2. **Resolved.** The four places now read as a target, "02-05 전에 끝내는 것이 목표(02-05의 선행 아님)", or defer to the X-04 row:
   - plan §3 "02-04 영향";
   - state X-03.unresolved;
   - state X-04.next_action;
   - the generated TASKS X-04 row.

   No unqualified "02-05 전에 완료" remains (checked by grep over docs/poc). Plan §6 step 5 "X-04 → 02-05" is qualified by the closing paragraph and the X-04 row.
3. **Resolved.** The plan's X-04 row and §3 assign the supervisor-side gRPC client to X-04. This includes switching it over if 02-05 first went over HTTP. state X-04.unresolved records the same. As review 2 allowed, no ledger change was made.

## De64b7b findings: no regression

- Findings 1 and 2 (the X-05, X-06 and X-07 acceptance texts) are unchanged in the ledger and A4. They are not in this diff.
- Finding 3 is unchanged except for the §1 and §2 refinements above, which strengthen it.
- Finding 4 (the 02-05 gate) is further fixed.
- Finding 5 (the 1 MiB, reconnect and health adapter behaviours in §3) is still present.
- Finding 6 (the PR #40 merge record, resume.plan_and_spec including A4, and the report obligation in next_action) is untouched by this diff.

## Findings (this review)

1. **[Low, non-blocking] The ledger's X-04 acceptance does not mention the supervisor client.** Only the plan and state X-04.unresolved now place the supervisor gRPC client in X-04. The ledger X-04 acceptance covers only the coordinator gRPC path and the common regression. A later reviewer of X-04 could accept it without the client switch. Review 2 allowed fixing this in the plan only, so it does not block. When X-04 is opened, its PR description or report should list the client switch explicitly, or X-04 should name a follow-up that owns it.
2. **[Info] Repro requests are guardrail-only.** §1 lists the repro request among the reused operations. `agent:repro` is guardrail-only (auth.rs loader, and the check in `get_repro`/`submit_repro`). So only a guardrail-role supervisor could reuse it. X-03 can settle this when it defines the supervisor principal and scopes. No change is needed now.
3. **[Info] Nothing claims unperformed work.**
   - Every X task is 미착수 / unverified.
   - Library support is still marked unconfirmed.
   - The §3 wording "X-04가 02-05보다 먼저 끝나면 … 늦으면 …" is conditional, not a claim.
   - Counts are unchanged at 11/73 and 0/12.
