VERDICT: approve
TARGET_SHA: ddbef27546a715abafb87170f4cb2c7835da32ac

Independent Guardrail re-review of PR #4 after my reject of fbe1f24. Full diff reviewed: 76ffd03..ddbef27 (correction commit fbe1f24..ddbef27). Read-only; scratch worktree removed; scratch CARGO_TARGET_DIR deleted.
Raw logs: L2=/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/logs2 (probe source L2/zz_review.rs = previous probes r01–r15 + new n01–n06)

## Commands run (exit codes)

| # | Command (worktree @ ddbef27, rustc 1.99.1, scratch target) | Exit | Log |
|---|---|---|---|
| C1 | `git fetch origin pull/4/head`; `git worktree add .../rev-pr4b ddbef27` | 0 | - |
| C2 | `cargo fmt --all -- --check` | 0 | L2/fmt.log |
| C3 | `cargo clippy --all-targets --locked -- -D warnings` | 0 | L2/clippy.log |
| C4 | `cargo test --locked --all-targets` (4 + 0 + 16 + 21 = 41 passed) | 0 | L2/test.log |
| C5 | `ieo-spec check-manifest <manifest>` (9×201) | 0 | L2/cm-draft.json |
| C6 | `ieo-spec check-manifest <manifest> --executable` (9×422, UNBOUND_DRAFT only, 6 each) | 1 (expected) | L2/cm-exec.json |
| C7 | diff C5/C6 vs evidence/check-manifest-*.txt (minus `exit=` line) | 0 (identical) | - |
| C8 | reviewer probes `cargo test --locked --test zz_review -- --nocapture` (r01–r15, n01–n05) | 0 | L2/probes.log |
| C9 | probe n06 (old revision executable while newer exists) | 0 | L2/probe-n06.log |
| C10 | mutations M5 covers:=true, M6 ordering off, M7 integer-only off, M8 spec estimate order off, M9 converter ID-conflict off; each `cargo test -q` then `git checkout` | 101 each (1 named test fails each) | session output |
| C11 | `validate_package.py`, `render_progress.py --check` (.venv-poc) | 0 / 0 | L2/validate.log, L2/render.log |
| C12 | `gh pr view 4 --json headRefOid` = ddbef27; `gh pr checks 4`; `gh run list --commit ddbef27…` → run 43127002941 success (3/3 jobs); `gh run view --job 130881526370 --log` shows 4/0/16/21 passed | 0 | L2/ci-rust-job.log |
| C13 | `git worktree remove`; `rm -rf target2` | 0 | - |

Evidence check: cargo-test.log counts (4/0/16/21), check-manifest outputs, and toolchain.txt (base now 76ffd03) all match my reproduction. mutation-check.txt is honestly labelled as rev-1-code. My M5–M9 cover the new code and are all killed.

## Status of previous findings

| Prev | Status | Evidence |
|---|---|---|
| F1 P1 §1.4 gaps | **Resolved.** Contract §4 now maps each §1.4 element to an r4 field or to gap G1–G8 / delegation. Every element I named last time has a G entry: G1 prerequisite evidence scope, G2 evidence/SHA relation, G3 independent judge, G4 retry ceiling, G5 handling when a limit is reached, G6 impact/default action. The text states that revision 4 cannot represent these, that filling them needs contract rev 5 plus a new digest, and that this is a user decision; result.json `unresolved` carries it. This meets the plan's rule "표현 불가 필드는 공백 목록에 남긴다" together with keeping the schema fixed. | contract §4 |
| F2 P1 idempotent executable bypass | **Resolved.** The registry stores the mode it registered a spec in; 200 is returned only when the stored mode covers the request. | r06: draft stored + executable request now gives 422 with 6×UNBOUND_DRAFT. n01: ready stored-as-draft + executable gives 201; executable stored gives 200 for exec and for draft. M5 is killed. |
| F3 spec estimate min>max | Resolved | r05 gives 422 `EstimateRangeReversed@tasks[0].estimate.range_seconds`; M8 killed |
| F4a 10.0 → 0 | Resolved | r11: 10.0/20.0 gives 600/1200 s; 30.0/5.0 gives ESTIMATE_RANGE_REVERSED |
| F4b overflow panic | Resolved | r12 / n05: `checked_mul` gives SchemaViolation@tasks[0].expected_duration with no panic; u64::MAX/60 is still accepted (see N3) |
| F5 converter ID collision | Resolved; a cosmetic extra remains (N2) | r13 gives TASK_ID_CONFLICT@tasks[0]; M9 killed |
| F6 number canonical form | Resolved by contract choice: v0 accepts only i64/u64 numbers, checked before the digest | n03: 0.93, 4.0, revision 4.0 and >u64 values are all 400 with exact field; negative ints and u64::MAX are accepted. The 9 package digests still reproduce. |
| F7 ready+bindings 400 | Resolved by documentation (§1.3: ready + bindings is format layer 400; UNBOUND_DRAFT is draft-only); covered by test | r07 unchanged; a wording inconsistency remains (N4) |
| P3 a: lower revision | Resolved for new content | n02: r3-after-r4 gives 409; identical r3 resubmit gives 200; r5 gives 201; r5 below r6 gives 409; M6 killed. See N1 for the remaining hole. |
| P3 b: invented requirement_ids/kind | Resolved | listed in `generated_fields`; G8 |
| P3 c: set-deadline reason | Resolved | kept in `preserved_unrepresented.deadline_reason` (not in spec, by design) |
| P3 d: sprint number ≥100 | Resolved | r14 gives SchemaViolation@context.sprint_number (range 1–99) |
| P3 f: cross-sprint task ID 409 | Resolved | new TASK_ID_CONFLICT (409); n04 gives 409 with precedence over 422 |
| P3 g: base SHA label | Resolved | toolchain.txt shows 76ffd03 |

Regression probes (unchanged behaviour, still correct):
- self-loop and cross-sprint cycle give 422 DEPENDENCY_CYCLE
- cross-sprint prerequisite with the wrong plan_version, or an unregistered one, gives UNDEFINED_PREREQUISITE
- all deadline_state edge cases and empty tasks give 400 at the right field
- baseline POC-01 and POC-02 drafts give 201

## New / remaining findings (none P0/P1)

### N1 — P2 — An executable intake of a superseded revision is accepted
- Violation condition: the correction adds the rule "최신 등록 revision보다 낮으면 충돌" (contract §1.2, §2 REVISION_CONFLICT), and plan §1.4 says "변경은 새 revision". The ordering check runs only when (sprint_id, revision) is **not** already stored (check.rs `else if` branch). The new mode-scoped path lets an old stored-as-draft revision fall through to a full check.
- Reproduction: L2/zz_review.rs `n06_old_revision_executable_after_newer`. Store a bound ready r4 and a newer r5 (both draft mode), then call `check_registration(r4, reg, Executable)`.
- Expected: 409 REVISION_CONFLICT (r5 is the latest), or at least no executable intake of r4.
- Observed: `PROBE r4_exec_while_r5_latest: http=201 state=AcceptedForSemanticReview []` (L2/probe-n06.log).
- This is not an unbound-draft bypass: the spec is fully bound and ready. It is an ordering gap in a corner of the new code.
- Fix: also apply the latest-revision comparison when `!covers`.
- Certainty: high. It should be fixed before or in 02-03, which reuses this path.

### N2 — P3 — The converter still reports a false DEPENDENCY_CYCLE next to TASK_ID_CONFLICT
- Reproduction: n04/r13 with sprint_number=1, registered 01-01, and input `[a after "01-01"]`. Observed: `["DependencyCycle@tasks[0].after", "TaskIdConflict@tasks[0]"]`.
- Expected: TASK_ID_CONFLICT only. The cycle is an artifact of the generated ID shadowing the registered one. The correct code is present, so the user can fix the input.

### N3 — P3 — The converter accepts absurd or imprecise estimates without bound
- 9007199254740993.0 min is stored as 540431955284459520 s, a silent 2^53 precision loss.
- u64::MAX/60 minutes is accepted.
- These are schema-valid but meaningless. Suggest a sane upper bound, or refusing floats above 2^53. Low impact.

### N4 — P3 — UNBOUND_DRAFT wording is inconsistent
- §2 says UNBOUND_DRAFT applies to "draft 또는 미바인딩 항목 존재", while §1.3 says it is draft-only and ready+bindings is 400.
- Plan §1.6 maps "등록 바인딩 오류" to 422; the contract's 400 choice for ready+bindings should be stated explicitly when 01-08 binds the OpenAPI.

### N5 — P3 — The integer-only rule rejects legitimate fractional evidence
- `tasks[].evidence` objects with ratios (e.g. coverage 0.93) are now rejected with 400. This is a deliberate trade-off and documented in §1.1, but report and evidence producers for 02-03 must encode such values as strings.
- The integer-valued float `4.0` is also rejected even though the schema type accepts it, while the converter normalizes 10.0. The two are consistent for their respective inputs, but the asymmetry should be documented for API users.

## Summary
Both P1s from fbe1f24 are fixed in substance, not just in wording:
- F1: the §4 mapping and the G1–G8 gap list cover every §1.4 element that schema r4 cannot represent, and record the rev-5 decision. That meets "1.4절의 필드…고정" under the plan's gap-list rule.
- F2: the executable-idempotency bypass is closed and covered by a test that a mutation (M5) proves meaningful.

F3–F7 and the P3 items are resolved, with tests that my mutations M5–M9 confirm. Evidence reproduces exactly, and CI run 43127002941 at ddbef27 is green across 3/3 jobs.

The new code has one P2 (N1: an old revision stored as draft can still be executable-registered after a newer revision exists) and minor P3s. None blocks 01-07 acceptance. N1 should be fixed in a follow-up or at the start of 02-03.
