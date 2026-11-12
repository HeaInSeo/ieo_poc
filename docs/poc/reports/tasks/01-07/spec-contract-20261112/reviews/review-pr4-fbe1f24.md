VERDICT: reject
TARGET_SHA: fbe1f2433d3b6a6d0937740646a2caf019c163cf

Reviewer: independent Guardrail (did not author the change). Base main 76ffd03. Read-only; scratch worktree removed after the run.
Raw logs dir: L=/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/logs

## Commands run (exit codes)

| # | Command (in worktree @ fbe1f24, CARGO_TARGET_DIR=scratch/target, rustc 1.99.1) | Exit | Log |
|---|---|---|---|
| C1 | `git worktree add .../rev-pr4 fbe1f24` | 0 | - |
| C2 | `cargo fmt --all -- --check` | 0 | L/fmt.log |
| C3 | `cargo clippy --all-targets --locked -- -D warnings` | 0 | L/clippy.log |
| C4 | `cargo test --locked --all-targets` (4+0+11+16 passed) | 0 | L/test.log |
| C5 | `ieo-spec check-manifest docs/poc/Console_Coordinator_Sprints_v0.json` | 0 | L/cm-draft.json |
| C6 | `ieo-spec check-manifest ... --executable` | 1 | L/cm-exec.json |
| C7 | independent Python `json.dumps(ensure_ascii=False, sort_keys=True, separators=(',',':'))` + sha256 over 9 sprints vs package digest vs Rust computed_digest | 0 (9/9 equal) | inline in session; reproduced in C5 |
| C8 | diff C5/C6 output vs evidence/check-manifest-{draft,executable}.txt (minus trailing `exit=` line) | 0 (identical) | - |
| C9 | reviewer probe tests `cargo test --locked --test zz_review -- --nocapture` (scratch file, not in PR) | 0 | L/probes.log, source L/zz_review.rs |
| C10 | Rust `ieo-spec digest` vs Python on numeric/string edge values | 0 | L/canon-edge.log, L/float-digest.log |
| C11 | Mutations M1 (cycle off), M2 (local plan_version off), M3 (digest off), M4 (binding check off), each `cargo test -q --test registration`, then `git checkout` | 101 each (1 failed / 15 passed) | session output (all 4 killed) |
| C12 | `validate_package.py` / `render_progress.py --check` (docs/poc, .venv-poc) | 0 / 0 | L/validate.log, L/render.log |
| C13 | `gh pr checks 4`; `gh run view 43125825018 --job 130877512567 --log` | 0 (3/3 pass at headSha fbe1f24) | L/ci-rust-job.log |
| C14 | `git worktree remove` | 0 | - |

Reproduced from REPORT.md: 31 tests pass; fmt/clippy clean; 9/9 digests (also with an independent Python implementation); draft 9x201; executable 9x422 with only UNBOUND_DRAFT (1 + 5 bindings = 6 each); 3/3 mutation claims (plus my own M4). CI run 43125825018 at the exact head SHA is green and the rust job really runs fmt/clippy/test/check-manifest (no `|| true`, no pipe masking, `bash -e`, binary exit code propagates).

## Findings

### F1 — P1 — §1.4 fields that SprintSpec v0 cannot represent are neither added nor listed as gaps (acceptance element 1 overclaimed)
- Violation condition: 01-07.accept: "1.4절의 필드 … JSON Schema를 고정한다"; plan §1.4: "표현 불가 필드는 공백 목록에 남긴다". The contract (Registration_Contract_v0.md table row 1) says the schema is fixed "그대로 … 변경 없음", REPORT.md says the element is covered by "기존 스키마 유지 + digest 9/9", and §4 lists only out_of_scope/decisions, kind, 403.
- Reproduction: compare §1.4 table to `docs/poc/SprintSpec_v0.schema.json`:
  - 위임·한도: "한도 도달 시 처리" and the retry ceiling (separate from corrections). `limits` has only elapsed_seconds/total_tokens/correction_attempts/unresolved_reason.
  - 수용 기준: "요구 증거·대상 SHA/이미지/문서 관계, 독립 판정 주체". `acceptance` has only profile_id/profile_revision/item_ids/criteria.
  - 사람의 확인: "필요한 검증·결정, 발생 시점·영향·응답 시 기본 조치". `human_validation` has only required/timing/criteria_ref.
  - 요구와 작업: prerequisites "범위의 증거로 연결". `prerequisites[]` has only task_id/plan_version/required_state.
  - 출처: "Notion page ID". `sources[]` has kind/ref/revision/digest/observed_at, so it is representable only loosely through `ref`.
  - `grep -rn "한도 도달\|독립 판정 주체\|기본 조치" docs/poc` finds no gap record for these in any 01-07 file.
- Expected: either schema fields, or a gap entry with handling and owner per §1.4 element, as §4 does for out_of_scope.
- Observed: no field-by-field mapping. At least 4 §1.4 elements are missing silently, and the report claims coverage.
- Certainty: high on the facts. Medium-high that this blocks acceptance, since it is the first element of the criterion and later tasks (02-03, P01) will treat the schema as complete.
- Raw ref: schema file at fbe1f24; Registration_Contract_v0.md §4; REPORT.md row 1.

### F2 — P1 — The idempotent 200 path skips the binding check: an unbound draft gets a no-rejection outcome in Executable mode
- Violation condition: 01-07.accept "미바인딩 draft의 등록 거부"; plan §1.4 "draft의 문서 검산 성공을 실행 가능 등록으로 보고하지 않는다".
- Reproduction: L/zz_review.rs `r06_idempotent_draft_resubmitted_as_executable`. Register POC-01 r4 (draft) with `reg.add_spec(&s)`, then `check_registration(&s, &reg, CheckMode::Executable)`.
- Expected: 422 UNBOUND_DRAFT (6 rejections), or at least an outcome saying the stored registration is draft and not executable.
- Observed: `PROBE draft_resubmit_exec: http=200 state=AlreadyRegistered []` (L/probes.log). check.rs:164-171 returns before step 8. `Registry` does not record the mode or state of the stored registration, and contract §2 defines idempotency on sprint_id·revision·digest only. Any caller that gates intake on `rejections.is_empty()`, as the CLI does at bin/ieo-spec.rs:359, treats this as success.
- Certainty: high (deterministic). Impact: this is the library 02-03 is told to reuse.
- Fix hint: run the binding check before the idempotent return, or store the registration mode in `Registry` and return the stored state.

### F3 — P2 — A reversed measured estimate in a SprintSpec is accepted (false accept)
- Repro: r05. Set tasks[0].estimate to `measured {min:1200,max:60}` and redigest.
- Expected: rejection (Human_Sprint_Authoring step 4 and Schedule_and_Role_Reports:26 require min≤max outside the schema). Observed: `http=201 AcceptedForSemanticReview []`. ESTIMATE_RANGE_REVERSED exists only in the converter. Direct SprintSpec submissions (the POST /v0/sprints path) are not checked.
- Certainty: high. Raw: L/probes.log.

### F4 — P2 — The converter invents values or crashes on valid human input
- (a) Integer-valued floats. The human schema `"type":"integer"` accepts `10.0` (2020-12 semantics). convert.rs:151-152 then uses `as_u64().unwrap_or_default()`, which emits `range_seconds {min:0,max:0}` as `measured`. That is exactly the "0으로 대체" the contract forbids. The reversed check at :104 is also skipped (`30.0 > 5.0` passes). The converter returns Ok; the draft only fails later against the SprintSpec schema (`SchemaViolation@tasks[0].estimate.range_seconds.*`), at a spec location rather than the user's `tasks[i].expected_duration`. Repro r11.
- (b) Overflow. `min_minutes = 2^62` is schema-valid and panics at convert.rs:151:28 (debug); release builds wrap silently. A request-triggered panic in a reused server library is a DoS. Repro r12: `panicked=true`.
- Certainty: high. Raw: L/probes.log.

### F5 — P2 — The converter does not check generated IDs against registered IDs (Human_Sprint_Authoring step 1 "기존 ID를 재사용/충돌 검사")
- Repro r13 with ctx sprint_number=1 and registered `01-01`.
  - Input `[a, b after "01-01"]` converts OK with a→01-01. b's prerequisite "01-01" (meant as the registered POC-01 task) becomes ambiguous with the local 01-01. Registration later rejects with `DuplicateTaskId@tasks[0..1].task_id`, a location the user never wrote.
  - Input `[a after "01-01"]` gives `DependencyCycle@tasks[0].after`, which is the wrong rejection class (a false cycle).
- Expected: conversion-time ID-collision rejection, or allocation that skips registered IDs. Certainty: high.

### F6 — P2 — The digest canonical form is not specified for numbers and diverges from the documented Python equivalence
- digest.rs doc claims equivalence with Python `json.dumps(..., sort_keys=True, separators=(',',':'))`. Divergences (L/canon-edge.log):
  - `1e-7`: Rust `1e-7`, Python `1e-07`.
  - Integers above u64: serde parses them as f64 (`18446744073709551616`, a 30-digit int).
- The 9 package specs contain no such values, so 9/9 holds. But `tasks[].evidence` is `array of object` with free content, and 02-03/report events will put arbitrary JSON there. Plan §1.4 also does not pin a number canonicalization (e.g. RFC 8785/JCS), so two conforming implementations can disagree.
- Expected: either a number rule in the contract (or rejection of non-integer and out-of-range numbers), or a test asserting it. Certainty: high on divergence; impact is future.

### F7 — P2 — The "unbound" class partly collapses into SCHEMA_VIOLATION/400
- Repro r07: `registration_state=ready` with a non-empty `unresolved_bindings`, Executable mode, gives `400 SchemaViolation@unresolved_bindings` (schema `maxItems:0` under the ready conditional). The UNBOUND_DRAFT per-binding path in check.rs:308-320 is unreachable for ready specs.
- Contract §2 says UNBOUND_DRAFT covers "draft 또는 미바인딩 항목 존재"; §1.3 mentions only url/limits as format-layer. Plan §1.6 maps 등록 바인딩 오류 to 422, not 400. Correctable, but the classes are not as distinct as claimed. Certainty: high.

### F8 — P3 — Minor contract and coverage items
- (a) Lower revision with new content after a higher one is registered is accepted with 201 (r09). §1.4 says "변경은 새 revision"; there is no monotonicity or "attribution unchanged" rule.
- (b) The converter invents `requirement_ids` `"{sprint_id}-REQ-{NN}"` and `kind:"구현"`. Only `kind` is listed as a gap; requirement IDs are not flagged in unresolved or semantic review. ("누락·모호함을 임의로 채우지 않는다".)
- (c) `deadline.reason` is dropped when `state=set` (r15: `set_reason_in_spec=false`). User wording is lost. Other wording (title/goal/output/done_when/how_to_check, task names, out_of_scope, decisions) is preserved verbatim; no original-input copy is kept, only `input_digest`.
- (d) `sprint_number >= 100` produces task_id `100-01` and requirement `NK-01-REQ--01`, an invalid draft (r14). `id[3..]` slicing assumes 2 digits; the same applies to >99 tasks.
- (e) The profile check validates only item-ID membership. A task can cite another task's item, or alter `criteria` text, and still pass. This is acceptable as P01 scope but is not stated.
- (f) DUPLICATE_TASK_ID across sprints maps to 422. Plan §1.6 puts "ID … 충돌" under 409. This should be clarified in 01-08.
- (g) evidence/toolchain.txt records `git base: b12bc73` (PR #1 merge), not the PR base 76ffd03. `git diff b12bc73 76ffd03 -- docs/poc/*.json docs/poc/*.md` is empty and outputs reproduce identically, so this has no effect on results.
- (h) The CI job checks only the positive draft path of the CLI. The executable-mode 422 is covered only by unit tests. That is fine; noted, not a false success.

## Item-by-item answers
1. Coverage: rejection codes, field/correction, converter-filled digest/item IDs, set/none/undecided, and measured/unestimated are present. Missing: the §1.4 field-to-schema mapping and gap list (F1). The P01 link is only nominal (`ACCEPTED_FOR_SEMANTIC_REVIEW`), which is acceptable for v0.
2. Digest: implemented as §1.4 states for the package. 9/9 reproduced, also with independent Python. Number canonicalization is underspecified (F6).
3. Classes: missing field, undefined prerequisite (incl. cross-sprint and wrong plan_version), cycle (incl. self-loop and cross-sprint via registry), unknown profile/item, digest mismatch, duplicate, revision conflict are all distinct and correct in my probes. Deadline edge cases and empty tasks are all correctly 400. False accepts: F2, F3, F8a. Class collapse: F7. False reject or wrong class: F5.
4. Converter: no invented deadlines, limits or profile approval; unknowns go to unresolved and semantic review. It does invent a 0 estimate (F4a), requirement IDs and kind (F8b), and colliding IDs (F5). It drops the set-deadline reason (F8c).
5. Evidence: every REPORT.md claim reproduced (C4-C8, C11). Only the base-SHA label is off (F8g).
6. CI: meaningful, pinned, green at the exact head; no false-success pattern found.
7. For 02-03 reuse: F2 (mode not stored), F4b (panic), F6 (canonical numbers), and F5. Also `Registry` keys tasks by ID only, and later `add_spec` calls silently overwrite.

## Summary
The work is solid and honestly evidenced: all claimed numbers reproduce, the 9 digests match an independent implementation, mutations are killed, and CI is real. I reject on two P1 findings:
- F1: 01-07's first acceptance element (fixing §1.4 fields in the JSON Schema, with unrepresentable fields recorded as gaps) is claimed but several §1.4 elements are neither in the schema nor in the gap list.
- F2: the reference checker gives an unbound draft a no-rejection 200 in Executable mode through the idempotent path, which is the exact rejection 01-07 must guarantee, in a crate earmarked for 02-03.

The P2 findings F3-F7 should be fixed in the same correction round: the SprintSpec estimate order check, converter integer handling and overflow, ID-collision check, number canonicalization, and the 400/422 binding class.
