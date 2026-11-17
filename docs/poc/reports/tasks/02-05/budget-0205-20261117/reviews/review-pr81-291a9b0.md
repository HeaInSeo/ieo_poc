# Review of PR #81 at 291a9b0 (review 2)

PR #81 "02-05 budget name and caps (BUDGET_0205, spare gate)". Head `291a9b0e6ff165f47f47faaaccb6800d1a46bf46`; previous review b38e8df (review 1, request changes, M-1). Correction commits reviewed: `1ea9233`, `cd1910b`, `1cf098a`, `03531f1` (evidence `766c765`, `add39b4`, `7a20769`) and `291a9b0` (REPORT §11/§12, decision record §2/§4, archived review 1, ledger snapshot 0053 update). Binding: user decision 2026-11-17 §2 (reworded) and §4 (the user's own conditions). Model calls in this review: 0.

## Verdict

**Approve.** M-1 is fixed, and so are L-1, L-2, L-3, I-2 and I-4. Every §4 condition is implemented and tested at the caller level. The user's verbatim §4 conditions match the code. I found no High or Medium. One new Low remains (N-1): the only allowed spare path, a person's `deliverAssignment` → 02-05 admission → ledger record with `by`/`approved_at`, has no end-to-end test. A regression there would make the spare unusable, which fails closed, so it is not unsafe. Fix it before step 8 or carry it over.

## What I ran

Logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Step | Result | Log |
| --- | --- | --- |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | `r81b-build.log` |
| `cargo test --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`) | **405 passed, 0 failed, 0 ignored** (REPORT: 405) | `r81b-test.log` |
| Probes (local, not committed): `spare_releasable` table of 11 stop shapes; 3 concurrent spares with the new marker | results below | `r81b-adv.log` |
| 4 new hand mutations (H5, H7, H8, H9) | 3 killed, 1 survived (H5 → N-1) | `r81b-mutants.log` |
| OpenAPI validator; 0.16.0 → 0.17.0 still additive (`spare` optional, `evidence_ref` required inside it, `additionalProperties: false`) | OK | `r81b-ledger.log` |
| `render_progress.py --check`, `test_render_progress`, `validate_package.py`, `validate_contracts.py` | ok, `RESULT PASS 0`, rc 0, rc 0 | `r81b-ledger.log` |
| GitHub: PR #81 OPEN at `291a9b0`; PR #80 merge `e336161`; latest main run 43964837089 success at `e336161` | matches `state.json` (snapshot 0053, `branch_sha`) | `r81b-ledger.log` |
| Archived review `reviews/review-pr81-b38e8df.md` vs my review-1 file | byte-identical | — |
| Relative links in the decision record, REPORT, RESUME, STATUS, runtime contract | none broken | — |
| Evidence provenance | `evidence/test.log` header is `03531f1` with a clean tree; no `crates/` change after `03531f1`; `mutants.txt` 47 KILLED | — |

I restored the worktree after every mutation and after the probes (`git status` clean). The test DBs were dropped by the suites' own helpers, since every e2e passed; there is no psql client on the host to list them independently. I wrote nothing under `/home`. I did not touch 55433, 58090, PR #29, other worktrees, secrets or run directories.

## Review-1 findings

| ID | Status | Evidence |
| --- | --- | --- |
| M-1 | **Fixed** | `delivery.rs:465-476`: a non-null `spare` from a non-`user` credential → 403 `HUMAN_APPROVAL_REQUIRED`, before the idempotency key is stored and before anything is called. `spare_extra` (`delivery.rs:133-143`) writes `by = {name, kind: "user", via: "deliverAssignment"}` and `approved_at` (coordinator clock). The body cannot supply them, because `check_body` requires exactly 3 keys and the e2e shows `by` in the body → 400. The gate (`caller.rs:784-798`) refuses a marker without `by.kind == "user"`, a name and `approved_at`, and writes them to the record. `ieo-role-call.rs:41-56` refuses any of `--spare-reason/--replaces/--evidence-ref/--approver` before reading anything. e2e: `operator` (service) → 403, `human-op` (user) → 201. My H8 (kind check disabled) is killed by both `delivery_e2e` transports. |
| L-1 | **Fixed** | `caller.rs:806-816`: the raw lines decide "exactly one record", the effective record decides "failed". My H9 (back to the raw record) is killed by `the_spare_gate_refuses_ambiguous_records`. |
| L-2 | **Fixed** | (a) `a_failed_spare_stops_the_budget_again`; mutant C6 is recorded as killed. (b) The CLI mapping code is gone, and `ieo_role_call_refuses_a_spare_marker` (E1) covers the refusal. |
| L-3 | **Fixed** | Decision §2 is reworded ("개발 역할은 예비 표식 없이 최대 2회 … R1이 실패하면 예비가 두 번째, R2가 표식 없는 세 번째"), with the original wording kept for history. `budget.rs:47-51,137-138` comments are corrected. The user confirmed the reading in §4.1. |
| I-1 | Recorded (equivalent mutant) | REPORT §11 |
| I-2 | Moot (code removed) | The coordinator path caps `reason` and `evidence_ref` at 2,000 bytes each |
| I-3 | Carry-over | `state.json`: the ledger owner checks the real 02-04 ledger read-only before the next 02-04 call |
| I-4 | **Fixed** | `start_with` filters `spare` out of the role-service body (`caller.rs:1077-1085`); runtime contract revision 17 §19 item 30 says so. Mutant C10 is recorded. |

## User decision 2026-11-17 §4 conditions → code → verdict

| §4 condition | Code / test | Verdict |
| --- | --- | --- |
| 4.1 ≤2 unmarked development runs, ≤1 spare; the spare may be the 2nd call (R1 fail → spare → R2 3rd) | `spare_gate` `planned_used < 2`; tested by `the_0205_spare_may_redo_the_first_planned_run` | OK |
| 4.1 R2 fail → spare | `the_0205_spare_run_needs_a_marker_and_a_failed_development_run` | OK |
| 4.1 development 3, Guardrail 1, total 4, time/token/USD caps unchanged; G1 never retried | `BUDGET_0205` byte-unchanged since review 1 (`git diff b38e8df -- budget.rs` shows comment lines only); Guardrail marker refused, share 1 | OK |
| 4.2 only `kind=user` | coordinator 403 (H8 killed); gate `by.kind`; host caller refuses | OK |
| 4.2 record the replaced id, reason, cause report/evidence reference, approver, time | record `spare = {reason, replaces, evidence_ref, by, approved_at}`; probe output shows all five in the recorded spare | OK |
| 4.2 a reason string is not proof | `evidence_ref` is required and non-blank in both the gate and the delivery schema. The defect judgment is left to the person (§4.5: no per-case approval granted in advance) | OK |
| 4.3 release only the named run's own-result stop | `spare_releasable` (`caller.rs:845-866`): usage must be `measured`; every `"; "`-separated reason fragment must be `result …` or `no result event`; anything else, including an unknown reason, keeps blocking. `asks_for_a_person_but` still releases only the named attempt | OK |
| 4.3 unmeasured / ledger mismatch / limit / overage-signal / unconfirmed external effect stay blocking | Probe table: result+cleanup → Err; environment → Err; result+overage signal → Err; unmeasured → Err; lost answer (unmeasured) → Err; empty reason → Err; a `"; "` inside a subtype → Err (fail closed); `"results tampered"` → Err. Every run-guard stop (time, tokens, overage) is unmeasured by `run::charge_reported` (`!exited` → unmeasured), so limit stops are never releasable. A ledger mismatch is refused by `records()` before the gate. Pending external requests are refused by the supervisor and `services_clear`, unchanged. My H7 (any measured stop releasable) is killed | OK |
| 4.3 original failure record and usage kept; add only | ledger append-only; probe: 2 lines (original + spare), original unchanged | OK |
| 4.3 consumed once; duplicates add no run | "used already"; the same request again → ledger line count unchanged (test); 3 concurrent spares → exactly 1 recorded, 2 refused "used already" (probe; the gate runs under `caller.lock`, held until the record is appended); a lost answer records the spare (consumed, unmeasured, stops) | OK |
| 4.3 a failed spare stops again | `a_failed_spare_stops_the_budget_again` | OK |
| 4.4 refusals: non-person, other budget/role/successful run, reuse, bypassing other blocks | gate units (7 missing-field cases, other budget, Guardrail run, successful R1, corrected-to-success run, unknown, ambiguous), `delivery_e2e` 403/400, `a_spare_marker_releases_nothing_else`, `a_ledger_belongs_to_one_budget` | OK |
| 4.4 decision §2 = contract = code = report | §2, OpenAPI 0.17.0 text, contract revision 17 §19 item 30, REPORT §1/§11/§12 and code agree | OK |
| 4.5 no real call or per-case approval granted | No default enables `02-05` (unchanged since review 1); REPORT/ledger state the rule approval is not per-case approval | OK |

## Regressions in b38e8df..291a9b0

- The source changes are limited to `delivery.rs` (spare shape, person check, `spare_extra`), `ieo-role-call.rs` (refuse spare flags, mapping removed), `caller.rs` (gate fields, effective record, `spare_releasable`, `spare` not forwarded), `budget.rs` comments, and the new `ieo-evidence` dependency of `ieo-coord` (clock only; `Cargo.lock` adds one edge, no new external crate).
- `delivery_e2e` still asserts the 02-04 refusal "has no spare run in the 02-04 budget", now via the user credential. The previous service-credential call became the 403 check. Nothing else in accepted tests changed. The caller.rs diff removes only review-1-era spare assertions that the new rules replace (the old two-field marker).
- The 02-04 paths are unaffected: no spare rule, so the gate is `Ok(None)` without a marker, `spare` never reaches a role service, and `ieo-role-call` without spare flags behaves as before (the HTTP test passes).

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| N-1 | Low | `crates/ieo-coord/src/delivery.rs:649`; tests | My H5 (deliver passes the raw body `spare` instead of `spare_extra(…, who)`, so no `by`/`approved_at`) **survives** `ieo-coord` lib and both `delivery_e2e` transports. The only e2e spare goes to an 02-04 budget, which refuses at "has no spare" before `by` is read. The unit test `the_spare_names_the_person_who_attached_it` checks `spare_extra` but not that `deliver` uses it. Since 03531f1 the coordinator is the **only** spare path, so nothing checks that a person's spare can actually be admitted on 02-05 and recorded with its approver. The failure mode is closed (the gate refuses a marker without `by`), but step 8 would learn this only at a real failure. | Add an e2e (or extend `limits_e2e`, which already switches to an 02-05 plan): a user credential's spare naming a failed development run → admitted, and the ledger record's `spare.by.name` / `approved_at` / `evidence_ref` present; a second identical delivery → no new run. |
| N-2 | Info | `caller.rs:845-866` | `spare_releasable` parses `stop_reason` text. It fails closed (any unknown fragment blocks), but a future new stop reason that happens to start with `"result "` would be releasable. | Optional: record structured stop causes beside `stop_reason`, or a comment in `After::finish` noting that new reasons must not start with `result `. |
| N-3 | Info | `delivery.rs:139` | `approved_at` is when the coordinator handled the request, not an independent approval instant. That is reasonable for an authenticated user request, and the REPORT says "the coordinator's clock". | None, or say "request time" in contract §19. |

## Summary

- **Must fix before merge:** none.
- **Before step 8 (or carry-over):** N-1 (an end-to-end test of the person → coordinator → 02-05 spare path); I-3 (read-only check of the real 02-04 ledger before the next 02-04 call).
- **Informational:** N-2, N-3, I-1.
- **Unchanged from review 1:** all numbers are exact, nothing is enabled by default, one ledger per budget, unmeasured is never 0, USD 0, G1 is never retried, and the concurrent-spare race resolves to exactly one spare. The REPORT §8 step-8 prerequisites remain.
