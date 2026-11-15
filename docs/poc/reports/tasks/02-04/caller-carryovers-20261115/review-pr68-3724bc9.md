# Review of PR #68 at 3724bc9 (review 1)

- PR: HeaInSeo/ieo_poc#68, "X-08 follow-up: review 681c1d3 (P-2 remaining paths via tamper_recorded, non-object pending file, fd/curl tests) — snapshot 0041"
- Head: 3724bc985a721a8d90c8e16db100f04d0ec2d8ea (confirmed with `gh pr view 68 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main 6e5e502 (6e5e5023011f…), which is also the merge base. Three commits on top: 9b63ce8 code and tests, 5f3cfb0 report, 3724bc9 ledger snapshot 0041.
- CI at 3724bc9 (run 43713811893, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS. coordinator 68 and grpc 9 against the CI PostgreSQL; call_sh 1, caller 32, serve 34.
  - proto buf lint and breaking (not required): SUCCESS
  - The required contexts on main are exactly the first three.
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan, d05wt or x08wt worktrees, or the PostgreSQL on 55433. I used my own detached worktree and target directory and removed both at the end. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking or Medium findings. One Low and five Info findings.**

- L-1 of review 681c1d3 is fixed correctly on all three clearing paths (direct, duplicate, adoption). The coordinator's new `tamper_recorded` value means exactly "a tamper event was stored with this checkpoint's seq", and it is computed under the checkpoint lock.
- I-4 (non-object pending file), I-1 (fd 9) and I-2 (`curl -f`) are fixed or tested meaningfully. Each test I checked fails when its fix is reverted.
- The contract changes are additive. OpenAPI, proto and the `ieo-proto` conversions agree. `buf lint`, `buf build` and `buf breaking` against 6e5e502 pass, and so does `openapi-spec-validator`.
- The ledger settles 681c1d3 L-2 in `resume.blocking` and in a note: real calls and X-02 are blocked until X-08 is **accepted** (검수 완료). That is stricter than A6 and matches the user's 2026-11-15 instruction.
- Several other ledger fields still say "X-08 병합" (L-1 below). It does not block this merge, but it should be fixed in the next ledger update.

## What I ran

- `cargo test --workspace --locked --all-targets` with `IEO_TEST_DB_URL` on the throwaway DB on 55432: rc 0, **349 passed, 0 failed, 0 ignored** across 32 binaries. This equals `evidence/test.log` (349/0/0 at 9b63ce8). Per binary: coordinator 68, grpc 9, serve 34 (+3: the new L-1 test over http and grpc, and the non-object test), caller 32, call_sh 1, central 4.
- `-p ieo-supervisor --test serve --test caller --test call_sh` 3 times in a row: 3/3 green. The call_sh run took 1.3 s, caller 31–36 s and serve 17–19 s.
- `cargo fmt --all --check`: clean. `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `buf lint`, `buf build`, and `buf breaking --against <ieo_poc-0204 .git>#ref=6e5e502,subdir=contracts/proto` (`/opt/go/bin/buf`): all rc 0. `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml`: OK.
- Validators in `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` true, `generated_views_match` true, 562 internal links checked, accepted 12/73.
  - `render_progress.py --check`: ok.
  - `test_render_progress.py` and `test_summarize.py`: PASS.
- `cmp` of the archived `reports/tasks/02-04/caller-carryovers-20261115/reviews/review-pr67-681c1d3.md` against `pending-reviews/review-pr67-681c1d3.md`: identical.
- `gh run view 43712109183`: main, push, head 6e5e502, success. This matches the ledger.
- Secret grep over the whole diff (sk-ant, ghp_, github_pat, Bearer tokens, `password=`): the only hit is a sentence in the archived review. `evidence/test.log` has no DB URL, run path or secrets path; its only "secret" lines are test names.
- My own mutation spot-checks. Each mutation was applied alone, the named tests run, and the file restored; `git status` was clean afterwards:

| # | Mutation | Result |
| --- | --- | --- |
| M1 | supervisor duplicate arm back to `tamper_recorded == true \|\| duplicate == true` | KILLED: `an_unrecorded_tamper_stays_pending_after_a_duplicate_or_an_adopted_takeover` (http and grpc) |
| M2 | adoption clears on `carries_tamper` alone | KILLED: same test (http and grpc) |
| M2b | adoption never clears | KILLED: `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once` (http and grpc). The recorded case still clears. |
| M5 | `.filter(Value::is_object)` removed from the second pending read | KILLED: `a_pending_file_that_is_not_an_object_does_not_panic` |
| M3 | coordinator `latest.tamper_recorded` always false (`… AND false`) | **survived** (I-1) |
| M4 | coordinator duplicate `tamper_recorded` always false | KILLED: coordinator `central_checkpoints_keep_sequence_identity_and_tamper_reports` |
| M6 | call.sh `podman restart` without `9>&-` | KILLED: call_sh. The assertion message shows `fd9-inherited`. |
| M7 | call.sh `curl -sf` → `curl -s` | KILLED: call_sh |

## Item 1: L-1 semantics

### What `tamper_recorded` means

- `role_tamper_events` rows are inserted only in `report_checkpoint`'s store path, with the report's own `seq`, in the same transaction as the `role_checkpoints` row.
- The table is append-only (trigger), and its FK points at `(repo, role, seq)` of `role_checkpoints`.
- So "an event with `seq = N` exists" is exactly "the report that stored checkpoint N recorded the event".
- The duplicate branch and `latest_row` both run after `lock(checkpoint:{repo}:{role})`. `latest_row` computes the value in the same statement as the record. Both reads therefore see a committed checkpoint together with its event, or neither. There is no race.
- A duplicate is the same seq with the same bytes, so its `config_tamper_detected` is the same as the stored report's. The event at that seq, if any, is this detection.

### Duplicate of a checkpoint whose tamper was recorded by another seq

- This cannot arise for a takeover. The event for `new_session_id = X` is stored with the first checkpoint that carries X, which is the takeover. Later after_run/error checkpoints carry the same detection, hit `ON CONFLICT`, and answer `false`.
- The supervisor clears only on takeover answers (`write_checkpoint` takeover branch) and on adoption of a staged copy (staged copies exist only for takeovers). By then pending was already cleared on the takeover, and removal is idempotent.
- So a "false" from a later phase never strands anything.

### New session whose tamper was already recorded earlier (the P-2 case, "once per new session")

- The takeover reuses a `new_session_id` that already has an event, so `ON CONFLICT DO NOTHING` drops the event and the answer is `false` on every path.
- Pending stays. The next new session re-reports it with a new id.
- The coordinator does not refuse that report: `new == session` holds, the field set is unchanged, and nothing keys on `old_session_id`. No `EVENT_CONTENT_CONFLICT` is possible, because the re-report is at a new seq with a new session.
- Pending plus a resume forces a new session (`(Some(p), true, _)`), so nothing resumes over a pending detection.
- After adoption the local takeover checkpoint gives 409 `EXTERNAL_RECONCILIATION_REQUIRED` (the new test asserts the 409). That is the pre-existing "run did not finish" rule. The pending file survives it.
- A detection can stay pending only while every new session reuses an already-recorded id, and that is fail-closed (re-report, no resume).

### Backward compatibility

- An older coordinator, or a gRPC answer from one (proto3 default false), leaves the detection pending. The next new session reports it again: at worst a second event, never a drop.
- The REPORT says this, and says the persistent coordinator must be rebuilt before real calls.

### Transports

- HTTP and gRPC are both covered by `over_both_transports!` for the new serve test.
- On the gRPC side, `checkpoint_answer_from_json` already copied `tamper_recorded` for any status. `checkpoint_answer_to_json` now emits it for `OUTCOME_REPLAYED`, and `latest_from_json`/`latest_to_json` carry field 18.
- Round-trip unit tests include both new fields.

## Item 2: contract consistency

- **proto:**
  - `GetLatestCheckpointResponse.tamper_recorded = 18` is a new field.
  - `ReportCheckpointResponse.tamper_recorded = 4` only changes its comment (it is now also set for `OUTCOME_REPLAYED`).
  - This is wire-compatible. `buf breaking` passes.
- **OpenAPI:**
  - The 200 duplicate response gains a schema: `required: [duplicate, seq, checkpoint_sha256, tamper_recorded]`. It had no schema before, so this is additive for the server. The supervisor tolerates its absence.
  - The `latest` 200 description names the field. There is no schema there; that is pre-existing style (I-4).
- **`contracts/ieo-internal-v0.md`** does not list record or answer fields. It describes order of checks, error codes and the confirmation rule (§3, §4.2, §12). No update was needed.
- No runtime contract or other doc outside `reports/` and `progress/` describes the duplicate or `latest` answer fields. I grepped `docs` and `contracts` for `tamper_recorded`, `duplicate` and `tamper`.

## Item 3: I-4, I-1, I-2

- **I-4:**
  - The second pending read now drops non-objects, so a JSON array reads like an unreadable file.
  - The test covers both halves: with no fresh detection the request resumes (202, `mode: resume`, no panic), and with a fresh detection the array is replaced (`mode: new`, tamper of s1). M5 kills it.
  - The remaining "corrupt or non-object file, no fresh detection → resumes" case is correctly left as a user decision (REPORT §4, ledger X-08 `unresolved`).
  - I-4 (나), the empty `children/` left behind, is not fixed. The reason is given. It was an Info item outside the X-08 list, so not routing it to the user is acceptable.
- **I-1:**
  - The stub's `[ -e /proc/self/fd/9 ]` is a bash builtin, so it tests the stub process's own descriptor table. That is what an inherited fd 9 would affect.
  - The test asserts `fd9-closed`. M6 kills it.
- **I-2:**
  - The 401 body now carries the status fields, so without `-f` the status reads as valid and the restart would run.
  - Only the HTTP status separates the two. M7 kills it.

## Item 5: ledger snapshot 0041 (diff vs 6e5e502)

- **X-08:** stays 진행. `result_state` records the 681c1d3 approval, the merge 6e5e502, main CI 43712109183, the follow-up content and 349 tests. The archived review is added as evidence (byte-identical). `next_action` says real calls are barred until 검수 완료.
- **`resume.blocking`:** "X-08 검수 완료 전(… 병합만으로 풀지 않음)". A note explains that this deliberately reads A6 strictly, and quotes `User_Decisions_2026-11-15_resume.md` ("교정·시험·독립 검토·병합을 마친 뒤에만"). The quote is accurate, and the rule is not weaker than A6 or the user decision. Other fields still say "병합" (L-1).
- **I-3 of 681c1d3:** fixed.
  - 02-05 `next_action` no longer starts "계획 PR 검토·병합", and its stage 8 is now "X-08 검수 완료 뒤".
  - 02-05 `last_observed_at` is bumped.
  - `last_verified_stage` no longer repeats "02-05 계획".
- **Other changes:**
  - CI note 43712109183.
  - Event `X-08-PR67-MERGED-20261115` in both `events.jsonl` and `state.json`.
  - `branch_sha`, `running` and `current_work` updated.
- **Unchanged:** `budget` (context line only), 12/73 and 0/12. Generated views match.
- **REPORT §4 user decision list:** accurate.
  - L-1 is gone, because it is fixed.
  - The corrupt or non-object pending file is still a user decision with options (가)/(나).
  - Q-3 (가)/(나) is unchanged.
  - The kept behaviours are unchanged: Q-2 older-wins, P4, L-c3-2.
  - §6 states this explicitly.

## Findings

### L-1 (Low): the unblock point still reads "X-08 병합" in several ledger fields

`resume.blocking` and the new note say real calls and X-02 wait for X-08 **검수 완료**. These fields still say **병합**:

- `extra_tasks[X-02].next_action`: "X-08 병합 → OC08 …"
- `extra_tasks[X-02].unresolved`: "남은 조건: X-08 병합, …" and "선행 X-08 (…) 병합 전 착수 금지".
- `tasks[02-05].unresolved`: "X-08 병합 전에는 02-05의 실제 모델 호출을 하지 않는다".
- `resume.budget`: "X-02 예산 … 은 X-08 병합과 X-02 남은 조건 뒤에만".
- `unresolved[26]` (EARLY-01): "실제 모델 호출을 쓰면 X-08 병합".
- Historical or plan text, acceptable as is: the 02-04 `unresolved` decision summary, the 02-05 `result_state`/PLAN summary ("X-08 병합 뒤"), and old events.

**Why it matters:** X-08 is already merged (6e5e502). A resuming agent that reads only the X-02 row or `budget` sees its X-08 condition as met.

**Mitigation:** `resume.blocking` is the governing field, the note states the stricter reading, X-02's `prerequisites` lists X-08 (not yet 검수 완료), and X-02 has other unmet conditions. So this is not Medium.

**Ask:** in the next ledger update (at the latest with the X-08 acceptance record), change these to "X-08 검수 완료".

### I-1 (Info): `latest.tamper_recorded = true` is never asserted against the real coordinator

- The coordinator test checks `latest` only after seq 3, where the value is false.
- Mutation M3 (always false) survives `-p ieo-coord`. The author's mutant was the opposite one (always true), which is killed.
- The true adoption path is covered only by the supervisor's fake.
- **Impact:** fail-safe. A regression here would leave detections pending and cause a second report, not a drop.
- **Optional fix:** assert `latest.tamper_recorded == true` right after the seq 2 takeover in `central_checkpoints_keep_sequence_identity_and_tamper_reports`.

### I-2 (Info): REPORT §3 test line is stale

§3 still says "346 통과 … [evidence/test.log](evidence/test.log)(커밋 1feebb1)". The linked file now holds the 9b63ce8 run (349). §6 gives the right numbers. Change §3 to say the file was replaced, or point to the earlier version in git history.

### I-3 (Info): X-08 `last_observed_at` was not bumped

X-08's `result_state`, `evidence` and `next_action` changed, but `last_observed_at` stays 2026-11-15T20:54:57+09:00 (the 02-05 row was bumped). This is the same kind of detail as 681c1d3 I-3.

### I-4 (Info): `latest` 200 has no response schema

The new field exists only in the prose description, as before for the other `latest` fields. This is not a regression. A schema would let `openapi-spec-validator` and clients see the field.

### I-5 (Info): rebuilding the persistent coordinator is an operational step

- REPORT §6 and the X-08 `result_state` say `ieo-coord` must be replaced with a build after 9b63ce8 before real calls. `resume.running` still names the 778ee47 coordinator on 58090.
- An old coordinator is fail-safe (double report), so this is a note for X-02 preparation, next to 681c1d3 I-5. It does not need to be a blocker.

## Summary for the author

- Approve as is.
- Fix L-1 (the "X-08 병합" wording in the X-02, 02-05, `budget` and EARLY-01 fields) in the next ledger update, at the latest with the X-08 acceptance record.
- I-1 to I-5 are optional.
