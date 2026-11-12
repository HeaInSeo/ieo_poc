VERDICT: approve
TARGET_SHA: f75ea87eb67f04baea1dac35f6b90c891cb4c4a7

Independent Guardrail re-review of PR #5 (01-08), correction after reject of 8637403.
- **Scope:** full diff 6cd0816..f75ea87, with focus on 8637403..f75ea87.
- **Method:** read-only, in the scratch worktree `rev-pr5b`. Nothing was pushed, commented on or merged.
- **Raw logs:** `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/logs-f75/` (written as `logs-f75/` below).
- **Verdict basis:** no P0/P1 remains. P2 items are listed for the coordinator to dispose of: fix now or record as follow-up.

## Commands run (exit codes)

| # | Command | Exit | Raw log |
|---|---|---|---|
| 1 | `git fetch origin pull/5/head`; `git worktree add …/rev-pr5b f75ea87` | 0 | n/a |
| 2 | `cargo test --locked --all-targets` (scratch CARGO_TARGET_DIR) | 0. Results 1+30 (ieo-report), 4+0+16+21 (ieo-spec) = 72 tests | logs-f75/test.log |
| 3 | `cargo clippy --all-targets --locked -- -D warnings` | 0 | logs-f75/clippy.log |
| 4 | `cargo fmt --all -- --check` | 0 | logs-f75/fmt.log |
| 5 | `validate_package.py`, `render_progress.py --check` (.venv-poc) | 0, 0 | logs-f75/validate_package.log, render_progress.log |
| 6 | `openapi-spec-validator ieo-api-v0.openapi.yaml` (cwd contracts/) | 0 | logs-f75/openapi.log |
| 7 | SprintState schema check: claim in `verified`, verified in `latest_claims` (jsonschema + referencing, external $ref resolved) | 0 | logs-f75/state_schema.txt |
| 8 | Parity corpus, 59 cases (the 44 from the previous review plus 15 new: fraction length, ns ordering, year 0001/9999, offset edges, junk suffixes, non-ASCII). Python `validate_contracts.validate_report` vs Rust `validate_report`. | 0 / 0 | logs-f75/cases.tsv, py.tsv, rs.tsv, parity.txt |
| 9 | Notification corpus, 29 prev/next pairs (the previous 19 plus 10 new over-notify probes) | 0 | logs-f75/notify.txt, logs-f75/ntf-inputs/ |
| 10 | `render_human` on a dev claim marked 완료, a late verified verdict, and a guardrail claim marked 완료 | 0 | logs-f75/render.txt |
| 11 | `completion_summary` on 5 fact sets | 0 | logs-f75/completion.txt |
| 12 | Authorization probes: `authorize_claim`, `authorize_repro`, `may_read_*` | 0 | logs-f75/authz.txt |
| 13 | Mutants: 16 in batch 1 plus 7 valid re-runs in batch 2. Source restored after each. | see below | logs-f75/mutants.log, mutants2.log |
| 14 | `gh pr view 5 …`; `gh api …/commits/f75ea87…/check-runs`; `gh run view 43129396921 --log` | 0 | logs-f75/ci-run.log |

**Mutants** (`cargo test -p ieo-report`):

| Result | Mutants |
|---|---|
| Killed | options check off; semantic ts off; claim stale check off; agent read allowed; completion filter instead of prefix; year 0 allowed; order-sensitive set compare; claim headline off; repro role check off; repro-read repo check off; notify decision-deadline / phase+opstate / next_owner / decision-withdrawn off |
| Survived | `n > 9` fraction limit removed (N4); negative offset sign ignored (N12); notify `delay_reasons` path off (N9b); notify `body.verdict` path off (N13) |
| Invalid | Batch-1 deletions of entries from the fixed-length notify arrays did not compile, so they say nothing. They were re-run as valid replacements in batch 2. |

**CI on f75ea87:** all 3 check runs completed/success with head_sha = f75ea87.
- The CI log shows ieo-report at 1+30 tests and the workspace total at 72.
- `ieo-api-v0.openapi.yaml: OK` and report_contract_checks appear in the CI log.

## Disposition of F1–F12 (from review of 8637403)

| ID | Was | Now | Evidence |
|---|---|---|---|
| F1 | P1: mode outside the idempotency identity | **Resolved.** Identity is (key, body digest, mode). Same key with a different mode returns 409 `IDEMPOTENCY_KEY_CONFLICT`. CLI default key is `sha256(body):mode`. `RegistrationResult.mode` is required. A stored-draft revision submitted as executable goes through binding again. | openapi `registerSprint`, contract §2 and §4 |
| F2 | P2: parity and parser bugs | **Resolved on the Rust side.** `+-1:00`, `++1:00`, year 0000 and `+05:60` are rejected, and fractions are ordered to the ns. The remaining DIFFs are the Python defects declared in §6 (lowercase t/z in comparisons, offset minute 60), plus the new P3 items in N3. | logs-f75/parity.txt |
| F3 | P2: material changes suppressed, reorder notified | **Resolved.** All 11 previously suppressed material changes now notify. Reordering of evidence or unknown_facts is suppressed. New over-notify edge cases are in N4. | logs-f75/notify.txt |
| F4 | P2: render | **Resolved.** The headline carries `[claim · 판정 아님]` / `[verified 판정]`, and a claimed 완료 renders as "완료 주장(판정 아님)". Evidence comparison, guardrail verdict/findings/SHA, dev change summary, delay reasons and delay-affected tasks are rendered. Deadline-lapse wording is added. "계속할 수 있는 작업" and "승인 범위" are declared gaps RG1 and RG2 (§5). | logs-f75/render.txt |
| F5 | P2: completion wording | **Mostly resolved.** `completion_summary` covers all six scopes, including 산출물. Remaining issue: N2. | logs-f75/completion.txt |
| F6 | P2: authorization holes | **Resolved.** Claims and repro requests must match the current (assignment_id, generation), otherwise 409 `STALE_ASSIGNMENT`. Agents get 403 `AGENT_READ_DENIED` on `/v0/sprints*`. Repro read is limited to the same repo's Guardrail. New side effects: N1, N5. | logs-f75/authz.txt |
| F7 | P2: CLI local binding | **Resolved.** New `POST /v0/sprint-inputs` converts on the server with the approved binding, and the CLI `--human` uses it. | openapi `convertHumanInput` |
| F8 | P2: state schema | **Resolved.** The allOf const constraints work: a claim in `verified` gives "'verified' was expected", and the reverse gives "'claim' was expected". The null/array description is fixed. | logs-f75/state_schema.txt |
| F9 | P3: cursor | **Resolved.** `next_cursor` is always non-null, `has_more` is added, the stream is append-only, and a non-full page may grow. | openapi `listSprintEvents` |
| F10 | P3: error order | **Resolved.** Order is now 401 → 403/409 (assignment) → 400 → 409 → 422. Nit: a non-object body gets 403 `REPO_SCOPE_DENIED` instead of 400. | logs-f75/authz.txt (`non-object body`) |
| F11 | P3: evidence | **Resolved.** Counts are consistent (72 / 31). The mutation log lists M4–M10, and I reproduced M4–M10-equivalent kills. The evidence header says "8637403 + working tree", not f75ea87 (P3, N7). | evidence/, logs-f75/mutants*.log |
| F12 | P3: CI | No false success. Parity-in-CI is deferred to 01-06 (§6). | logs-f75/ci-run.log |

## New findings

### N1 · P2 · `STALE_ASSIGNMENT` (409, zero state change) discards late old-SHA results that §1.9 says to preserve, and it blocks idempotent replay after a generation bump.
- **Violated condition:**
  - Plan §1.9 (line 185): "옛 SHA의 늦은 결과는 원 SHA·배정에 귀속해 **보존**하되 현재 후보 판정에 쓰지 않는다."
  - Plan §1.2: when the save ACK is missing, the agent reconciles by re-query and by retrying with the same event ID.
  - Design log POC-03: a same-ID retry returns the original result.
- **Contract text:** the contract cites §1.9 for 409 `STALE_ASSIGNMENT` with "state unchanged", so the late result is neither stored nor attributed to its original assignment.
- **Check order:** the assignment check (403/409) runs before identity (409/200 replay). An agent whose claim was stored, whose ACK was lost, and whose assignment generation was then bumped (for example, a new head during review) gets 409 `STALE_ASSIGNMENT` on the same-ID retry instead of 200.
- **No read path:** agents are now barred from every read path (`AGENT_READ_DENIED`), so they cannot re-query either.
- **Reproduction:** logs-f75/authz.txt. `dev current: Ok(())` becomes `dev after generation bump (retry of stored event): Err(StaleAssignment)`.
- **Expected:**
  - Stale-generation claims are recorded, attributed to the old assignment and SHA, and excluded from current judgement. One example: 202 with a `stale` flag.
  - An exact replay of an already stored (event_id, event_revision) returns 200 before the staleness check.
- **Observed:** both are rejected with 0 state change.
- **Severity:** P2. Nothing invalid reaches a verdict, but audit evidence required by §1.9/P07 is lost and the §1.2 reconciliation path is broken in this case.
- **Certainty:** reproduced (the function), supported (the contract reading).

### N2 · P2 · `completion_summary` reports only the leading prefix of done scopes, so out-of-order facts disappear, including a merge without current-SHA approval.
- **Reproduction:** logs-f75/completion.txt.
  - Case `merged_without_current_review`: CI ok, review only on an old SHA, merged, main CI ok, outputs present. Output: "현재 SHA bbbbbbb CI 통과까지만 확인. 미완료: 현재 SHA bbbbbbb 독립 검토 승인 (이전 SHA 승인 aaaaaaa은 현재 판정에 쓰지 않음)".
  - Case `ci_failed_but_merged_mainci_ok`: output "확인된 완료 범위 없음".
- **Violated condition:**
  - §1.5: "완료 범위를 **각각** 표현한다".
  - §1.9/P07: a merge on an old-SHA approval is exactly the case Guardrail and the human must see.
  - Contract §3 "각각 따로 판정" is inconsistent with the `take_while` prefix logic.
- **Expected:** every scope is shown with its own state, and a confirmed later scope after an unmet earlier scope is flagged as an anomaly.
- **Observed:** the confirmed merge and main CI are omitted.
- **Certainty:** reproduced.

### N3 · P3 · Parity residue not declared in contract §6.
- **Rust is stricter than RFC 3339:** it rejects more than 9 fraction digits. `…09:00:00.1234567890Z` is ACCEPTED by Python and REJECTED by Rust. The limit is stated in the lib.rs doc comment but not in contract §6.
- **Python truncates to microseconds:** a forecast reversed by 1 ns, or at the 7th fraction digit, is ACCEPTED by Python and REJECTED by Rust. Rust is correct here. This is a Python defect missing from the §6 list.
- **Comparison with before:** everything else agrees, including Feb 29 in leap and non-leap years, year 0001/9999, ±23:59, -24:00, ZZ, Z+offset, trailing junk, NBSP and Arabic-Indic digits.
- **Test gaps:** mutants N4 (no limit on fraction digits) and N12 (negative offset treated as positive) survive. No test covers negative offsets or the 9-digit bound.
- **Raw ref:** logs-f75/parity.txt, mutants.log.
- **Certainty:** reproduced.

### N4 · P3 · Set-based notification over-notifies for some non-material changes.
- **Over-notifies (each gives `Notify(["schedule"])` or `Notify(["body_verdict"])`):**
  - reordered `schedule.forecast.evidence` (forecast compared as a whole object)
  - the same instant written with a different offset, in `forecast.completion_range` or `schedule.deadline_at` (string compare)
  - reordered guardrail `body.findings`
- **Suppressed although arguably material:**
  - `next_action` text change ("병합 보류: main CI 실패"). This is a declared design choice.
  - a reassignment (`assignment_id`/`generation` change).
- **Untested paths:** the `delay_reasons` and `body.verdict` notification paths have no test that kills their removal (N9b, N13).
- **Raw ref:** logs-f75/notify.txt, mutants2.log.
- **Certainty:** reproduced.

### N5 · P3 · The blanket `AGENT_READ_DENIED` also blocks the immutable-spec read.
- **Reproduction:** `may_read_sprint(Agent, …)` returns `AgentReadDenied` for every `/v0/sprints*` path, including `GET …/revisions/{rev}`, which holds no claims.
- **Conflict:** §1.2/01-02 §2 say the Guardrail input is the approved spec and profile. No contract path now delivers spec content to an agent. Assignment delivery carries only id, version and digest.
- **Expected:** either allow agents to read the revision (spec and digest only), or state where the agent obtains the spec body (01-03/02-0x).
- **Certainty:** supported.

### N6 · P3 · Small contract nits.
- `ConversionResult.conversion` lacks `additionalProperties: false`.
- The `/v0/sprint-inputs` description states replay (200) but not the 409 for same key with a different body.
- render_human never calls completion_summary, and test `input1_…` still passes through a hand-written `next_action` ("CI 통과까지만"). The contract §3 table attributes input 1 to `completion_summary`, which is tested separately in `completion_summary_scopes`.

### N7 · P3 · Evidence provenance.
- `evidence/cargo-test-workspace.log` is headed "base commit 8637403 + working tree of 01-08 rev2", not f75ea87.
- My run and CI at f75ea87 reproduce the same counts (72; ieo-report 1+30), so the claim holds. Only the provenance line is imprecise.

## Summary
The correction addresses every earlier finding in substance.
- **F1 (P1):** fixed. The idempotency identity now includes `mode`, the CLI key includes `mode`, and the result echoes `mode`.
- **F2–F12:** resolved or reduced to declared gaps.
- **Verification:** the claim/verified separation is now schema-enforced (verified). The Rust timestamp parser is strict and ns-ordered. Notification covers the material changes. Rendering shows facts, delay reasons and the evidence level. Tests, clippy, fmt, the package validation, the OpenAPI validation and CI on f75ea87 are green.
- **New issues from the correction, both P2:**
  - N1: stale-assignment 409 drops §1.9-preserved late results and blocks replay after a generation bump.
  - N2: `completion_summary` hides confirmed scopes after the first gap, including a merge without a current-SHA approval.
- **Minor items, P3:** N3–N7.
- **Verdict:** no P0/P1, so approve. I recommend fixing N1 and N2 before 02-03 implements these rules, or recording them as follow-up.
