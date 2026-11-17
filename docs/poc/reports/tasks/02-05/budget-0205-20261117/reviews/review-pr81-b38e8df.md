# Review of PR #81 at b38e8df (review 1)

PR #81 "02-05 budget name and caps (BUDGET_0205, spare gate)". Head `b38e8df4fec0141b363014cb7f1a00835342635c`, base main `e336161`. Independent reviewer; I did not write any of it. Model calls in this review: 0 (fake runtime only, test DB 55432 only).

## Verdict

**Request changes.** One Medium (M-1): the spare marker, which the PR and the decision record treat as "the person's decision" that lifts a `stop_required`, is accepted on the coordinator path from any `roles:control` credential, including `kind: service`. A service credential can therefore lift a stop and start the spare run with no person involved. That breaks "no automatic retry" and the user's requirement that a person makes the call. The step-7b recovery approval, which also lifts a person-required stop, already requires `kind: user`. Everything else is numerically exact and fails closed. The remaining findings are Low or Info.

## What I ran

All logs are in the scratchpad: `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/`.

| Step | Result | Log |
| --- | --- | --- |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | `r81-build.log` |
| `cargo test --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`) | **401 passed, 0 failed, 0 ignored** (same as REPORT §5) | `r81-test.log` |
| 4 adversarial probe tests of my own (local only, not committed): overage-stopped run then spare; failed spare then R2/G1/second spare; 3 concurrent spares; spare replacing a corrected run | all ran (results under Findings and §3 below) | `r81-adv.log`, patch `r81-probes.patch` |
| 5 hand mutations (H1–H4, plus H3b re-run against my probe) | 2 killed by existing tests (H2; H3 only by my probe), 3 survived (H3, H4, H1; H1 equivalent) | `r81-mutants.log` |
| `openapi-spec-validator`; diff 0.16.0 → 0.17.0 | OK; additive only (version, description, optional `spare` with `additionalProperties: false`) | `r81-openapi.log` |
| `render_progress.py --check`, `test_render_progress` (its own runner), `validate_package.py`, `validate_contracts.py` | ok / `RESULT PASS 0` / rc 0 / rc 0 | `r81-ledger.log` |
| GitHub: `gh pr view 80`, `gh run view 43964837089`, `gh pr view 81` | PR #80 MERGED as `e336161` (head `d336849`); run 43964837089 push on main at `e336161` = success; PR #81 OPEN at `b38e8df`. Matches `state.json` snapshot `v1.3.3-EXEC-0053` (`branch_sha`, event `02-05-S7B-MERGED-20261117`, main CI list) | `r81-ledger.log` |
| Relative links in the decision, REPORT, archived review, RESUME, STATUS; event evidence paths | none broken | `r81-ledger.log` |
| Evidence provenance | `evidence/test.log` header is `e77ed76`, clean tree; `git diff e77ed76 b38e8df -- crates` is empty; `mutants.txt` has 29 KILLED | — |

I restored the worktree after each mutation and after the probes; `git status` was clean. No `ieo_t_*` test DB of mine is left: the suite's own drop helpers ran, since every e2e passed. There is no psql client on the host, so I could not list databases independently. I wrote nothing under `/home`. I did not touch 55433, 58090, PR #29, other worktrees, the secrets, or the run directories.

## Numbers and rules: decision → code → verdict

The decision is 2026-11-15 §2.1 (나), with spare (나) from 2026-11-17.

| Decision | Code (`crates/ieo-supervisor/src/budget.rs` unless noted) | Verdict |
| --- | --- | --- |
| 4 runs (3 planned + 1 spare) | `BUDGET_0205.max_runs = 4` (:115) | exact |
| 600 s per run (window = timeout + grace) | `per_run_seconds: 600`; caller window = min(600, share left), supervisor auxiliary cap also 600 (`serve.rs` share) | exact |
| 2,400 s in all | `total_seconds: 2_400` | exact |
| billable (input+output+cache_creation) 200,000 | `billable_tokens: 200_000`; `budget::spent` sums i+o+cc | exact |
| cache_read 2,000,000 | `cache_read_tokens: 2_000_000` | exact |
| development share 3 runs · 1,800 s · 150,000 · 1,500,000 | `role_shares[0]` | exact |
| Guardrail share 1 run · 600 s · 50,000 · 500,000 | `role_shares[1]` | exact |
| shares sum to the totals | 3+1=4, 1,800+600=2,400, 150,000+50,000=200,000, 1.5M+0.5M=2M (also asserted in `budget_constants_match_decision_1`) | exact |
| no other numbers invented | only the reused `OBSERVATION_MAX_AGE_SECONDS`, LOCK_WAIT, and the delivery body's 2,000-byte reason cap (shape limit, not a budget number) | OK |
| shares are part of the approval | plan (`caller.rs:82-118`) and role service (`serve.rs` `with_share`) use the approved share; given values must equal it exactly; a role with no approved share is refused | OK (R1–R3 killed) |
| 02-04 / X-02 unchanged | only `role_shares: &[]`, `spare: None` added; numbers byte-identical | OK |
| 01-04 not usable | `budget::named` knows `02-04`, `X-02`, `02-05` only | OK |
| no transfer between budgets | `records()` refuses a ledger holding another budget's record (`caller.rs:251-268`); the spare's replaced run must carry `budget == plan.budget.name` | OK |
| unmeasured never 0 | unchanged `budget::next`; my probe: an overage-stopped run then a spare → `Stop "unmeasured run(s) [d1/development-start]"` | OK |
| no automatic retry | caller has no retry. **But see M-1**: on the coordinator path the spare marker, which lifts a stop, can come from a service credential | **M-1** |
| USD 0 (`overageStatus: rejected` or hold) | unchanged `observation_check`/`stream_stop`; the newest record's stream is still the observation after a spare | OK |
| spare: development only, once, replaces a failed development run of this budget, refuse when ambiguous or unreadable, marker in the ledger | `caller::spare_gate` (:732-805), `asks_for_a_person_but` (:421), `start_with` base record (:1019-1027, also on the lost-answer record via `record_lost(base_rec)`) | OK except L-1 (corrected run) |
| G1 never retried | Guardrail share 1, `spare.role = development`, a Guardrail marker is refused | OK |
| nothing real by default | no default budget anywhere. `Plan::from_json`, `Config::from_json` and the coordinator's `budget_plan` all require an explicit `budget` name; a grep of all non-test `.rs/.sh/.json/.yaml/.toml` finds `"02-05"` only in `budget.rs`'s const and the two `named` lookups | OK |

## Check-by-check

1. **Numbers.** All exact (table above). N1–N9 mutants are recorded as killed. I reviewed the constants by reading, not by mutating them again.
2. **Implicit selection.** None. There is no fallback in `caller.rs:60`, `serve.rs:114` or `delivery.rs:152` (no `budget_plan` means refused).
3. **Spare gate.** Attempts I made:
   - 4th development run without a marker: refused ("planned runs are used"). With a marker after the spare: refused ("used already").
   - 2nd spare: refused. A spare naming the spare: refused ("used already"; probe `rv_spare_own_failure_still_stops`).
   - Spare replacing a successful run: refused. Spare replacing a Guardrail run: refused ("is not a development run"). Spare replacing an 02-04 run: refused in the gate ("of the 02-05 budget"), and in practice earlier, because `records()` refuses the mixed ledger.
   - Spare after the totals: the `budget::next`/share checks still run after the gate.
   - Race, 3 concurrent spares on one ledger: exactly 1 recorded, the 2 others refused "the spare run … was used already". The gate runs under `caller.lock`, which is held until the record is appended.
   - A failed spare: its own `stop_required` still stops R2 and G1. This holds, but no existing test checks it (L-2).
   - Interpretation 2 abuse: the marker lifts only the `stop_required` *flag* of the named run. An overage-stopped run is unmeasured (`Stop`). An over-limit run hits `budget::next` or the share room. A non-`rejected` signal in a measured run fails the next observation (`Hold`). A failed spare stops again. So "stop and report" is still enforced by code for unmeasured, limit and overage cases. For the remaining reasons (unexpected path, error result) the marker is the only human gate, which is why M-1 matters.
4. **Ledger switching.** `limits_e2e` step 10 (both transports, passed): 409, usage unchanged, nothing on the 02-05 ledger.
5. **L-a/L-b/L-c.** L-a: `api.rs:4158-4178` binds the use to `checkpoint_seq = latest seq` and the approval's assignment and generation. 12c and 12d pass. My H1 (assignment clause neutralised) survived, but it is unreachable: `check_delivery` already binds the takeover to the delivery's assignment and generation, and the takeover must be the current assignment (Info I-1). L-b: 12b passes. L-c: runtime contract item 27 now names both unreleased cases; revision 16 is stated in the header.
6. **Accepted-operation changes (REPORT §3).** All three are listed, and the delivery-count edits in `delivery_e2e` are count updates only. In the test diff, removed lines are only those two counts and the struct literals extended by the two new fields. No 02-04 caller assertion was weakened.
7. **Tests vs REPORT claims.** These match, with two gaps (L-2).
8. **Ledger.** Matches GitHub, render `--check` ok, validators rc 0, links ok.

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| **M-1** | **Medium** | `crates/ieo-coord/src/delivery.rs:111-126` (`check_body` accepts `spare`), `:619-625` (passes it to `admit_with`); no `who.kind` check. Compare `crates/ieo-coord/src/recovery.rs:56` (`Kind::User` → else 403 `HUMAN_APPROVAL_REQUIRED`) | Decision 2026-11-17 §2 says the "실행 환경 결함 / 원인 보고 뒤" judgment belongs to **the person** who attaches the marker. Interpretation 2 (§3) makes that marker lift the named run's `stop_required`. `deliverAssignment` needs only `roles:control`, and the suite's own `operator` credential is `kind: service` (`delivery_e2e.rs:310`). A service or automation with `roles:control` can therefore answer every development failure with `{"spare": {"reason": "...", "replaces": "<failed attempt>"}}`, and the gate admits it. That is an automatic retry of a stopped run without a person, which the stop rules forbid ("자동 재시도 없음"). The ledger also does not record *who* attached the marker, so the record cannot show it was a person. The step-7b recovery, the other path that lifts a person-required stop, was given exactly this `kind: user` guard by user decision 2026-11-16 §2. | Refuse `spare` in `deliverAssignment` unless `who.kind == User` (403 `HUMAN_APPROVAL_REQUIRED`, nothing recorded or called), and write the attaching principal into the marker (for example `spare.by = {name, kind}`) in the ledger record. For `ieo-role-call`, record that the marker came from the host operator (this path is inside the trust boundary, as with `--recovery-id`). Add an e2e: a service credential's spare → 403, a user credential's spare → admitted. Document it in OpenAPI 0.17.0 and REPORT §1. |
| L-1 | Low | `crates/ieo-supervisor/src/caller.rs:782-797` (`spare_gate` reads the replaced run's failure from the **raw** line) | A run stopped only for a measurement reason (for example a missing child transcript) but with a clean `success` result, which a person then **corrected** (`effective()` sets `stop_required: false`), still counts as "failed" for the gate. Probe `rv_spare_replacing_a_corrected_run`: `Ok(Some({"reason":"env","replaces":"d2"}))`. That lets the spare "replace" a run the ledger itself considers a clean success, contrary to "a spare replaces a failed run" and "다른 목적에는 쓰지 않음". | Keep the raw scan for the one-record ambiguity check, but decide `failed` from the effective record in `recs` (or refuse a replaced run that has a correction). Add the probe as a test. |
| L-2 | Low | tests | (a) H3: making a spare run's own `stop_required` non-blocking (`.find(|r| r["stop_required"] == true && r["spare"].is_null())` in `asks_for_a_person_but`) **survives** the whole `caller` test file. Only my probe `rv_spare_own_failure_still_stops` kills it. That property is what keeps "stop and report" after a failed spare. (b) H4: swapping `reason`/`replaces` in `crates/ieo-supervisor/src/bin/ieo-role-call.rs:118` **survives**, because the CLI spare is only tested against the 02-04 refusal, so S12's "drops the flags" kill does not cover mapping. | Add (a) "a failed spare stops R2, G1 and a second spare" and (b) an `ieo-role-call` run on a 02-05 plan whose record's `spare` equals the flags. |
| L-3 | Low | `docs/poc/User_Decisions_2026-11-17_spare.md` §2 bullet 2 vs §3 bullet 1; `budget.rs:48,136` comments | §2 (the "반영" of the user's answer) says "개발 역할의 세 번째 회차는 호출에 예비 표식이 있어야 한다". Under interpretation 1 (R1 fails → spare → R2) the third development run is R2 and runs **without** a marker. The code comment "the third development run is the spare" and the `SpareRule` doc ("the role's run after its `planned_runs` needs … marker") say the same inaccurate thing. The record contradicts itself, so a reader cannot tell which rule the user approved. | Reword §2 and the comments to "at most 2 development runs without a marker; the 1 spare carries the marker and replaces a failed development run". Get the user's confirmation (see the note below). |
| I-1 | Info | `crates/ieo-coord/src/api.rs:4175` | H1 (assignment clause neutralised) survives `recovery_e2e`. It is defence in depth (`check_delivery` plus the current-assignment check come first), so the mutant is equivalent in practice. | Optional: a unit test of the SQL, or a comment saying it is a redundant layer. |
| I-2 | Info | `ieo-role-call.rs:109-114` | `flag()` takes the next argv even if it is a flag (`--spare-reason --replaces d1` → reason `"--replaces"`). The reason has no length cap on this path (the coordinator caps it at 2,000). | Optional: reject values starting with `--`. |
| I-3 | Info | REPORT §3 item 1 | The 02-04 caller now refuses a ledger with another budget's record. REPORT asserts that the real 02-04 ledger holds only 02-04 records. I could not check this (reading `/opt/rust/ieo_poc_runs` is forbidden for me). | The ledger owner should confirm it read-only before the next 02-04 call. |
| I-4 | Info | runtime contract revision 16 | The contract does not mention the `spare` key, which `ieo-role-call` now forwards in the start body to the role service (the service ignores it; the tests pass with it). | Optional one-line note, or strip `spare` from the forwarded body. |

## Summary

- **Must fix before merge:** M-1 (the spare marker must come from a person on the coordinator path, and the ledger must record who attached it).
- **Should fix in this PR (cheap):** L-1, L-2, L-3.
- **Carry-over / informational:** I-1 to I-4. The REPORT §8 carry-overs stay as listed: step-8 prerequisites (clean central record, new role volumes), L-c release path → step 9, I-8 before step 8.
- Everything checked against the decision is exact. Nothing enables a real call by default. One ledger per budget, no transfer, unmeasured-never-0, USD 0, G1-never-retried, the concurrent-spare race and L-a/L-b/L-c all hold.

## Note: do the two spare interpretations need the user's confirmation?

**Yes, both, before step 8 (not as a merge condition, since this PR starts nothing).**

- **Interpretation 1 (the spare is "the run after a failed planned run", so it may be the 2nd development run and R2 then runs as the 3rd without a marker).** This matches the user's own stop rule ("실패한 개발 회차(R1 또는 R2)를 … 다시 할 때") and keeps every count (development ≤3, ≤2 without a marker, 1 spare). It does contradict the literal wording of the chosen option (나) ("세 번째 개발 회차는 예비 표식…") and the record's own §2 (L-3). It is faithful in intent but departs from the words, so the user should confirm the reading.
- **Interpretation 2 (the marker lifts the named run's `stop_required`).** Without it the spare can never run: the failure's `stop_required` blocks every call, and the only existing release, a correction, must re-measure and is not a retry decision. So some form of it is necessary. My probes show it lifts only the flag: unmeasured, limit and overage stops remain enforced by other gates, and a failed spare stops again. But it turns the marker into a release of a person-required stop. That is a new power the user did not explicitly grant, and today any `roles:control` service can use it (M-1). The user should confirm it **together with** the M-1 guard (the marker comes from a person, and who attached it is recorded).
