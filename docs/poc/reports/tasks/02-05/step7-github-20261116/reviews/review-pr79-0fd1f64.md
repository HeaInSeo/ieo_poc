# Review of PR #79 at 0fd1f64 (review 1)

Scope: PR #79 (02-05 step 7, GitHub SHA·CI comparison as the final basis of a resume, C10) at exact head `0fd1f645f6f7cd6934201b20a8489c9a0927c54b`, base main `9b66300`. Detached worktree, removed afterwards. I did the full review at `f0e0ca9`, then checked the delta `f0e0ca9..0fd1f64` (commits `005dd08`, `0fd1f64`). `git diff f0e0ca9 0fd1f64 -- crates contracts Cargo.lock` is empty, so every code and contract result below carries over unchanged to 0fd1f64.

### Delta f0e0ca9 → 0fd1f64 (docs and evidence only)

- `evidence/mutants.txt` adds S7-G21 (the check reports `not_observed` as `mismatch`) and S7-G22 (it reports a match as `mismatch`), for 25/25 killed. REPORT §5 and §10 now say 25. `state.json` changes `23/23.` to `25/25.`, and the rendered views (STATUS, TASKS, RESULTS, RESUME, including the ledger SHA-256 line) were regenerated.
- I re-ran both mutants at 0fd1f64 on `api.rs:4757-4758`. **Both are caught** by `delivery_e2e` `deliveries_go_through_compare_budget_and_the_supervisor`: G21 at :689, G22 at :709. Logs: `r79-mut-G21.log`, `r79-mut-G22.log`.
- At 0fd1f64, `render_progress.py --check` reports ok and `validate_package.py` and `validate_contracts.py` return rc=0 (`r79-0fd1f64-render.log`). The 4 required check-runs on 0fd1f64 pass.
- The delta does not touch any finding below.

## Verdict

**Changes requested.** One Medium finding (M-1): an explicit new-session delivery is now gated by the GitHub comparison. The PR's report, runtime contract item 22 and OpenAPI all say the start judgment did not change, so this is an undocumented behaviour change. Everything else is either Low or Info. The resume rule, CI handling, guardrail `stale_target`, the L-a pre-check and the ledger are otherwise correct and well tested.

## What I ran

All logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/` as `r79-*.log`.

| Command | Result | Log |
| --- | --- | --- |
| `cargo fmt --check` | OK | `r79-fmt.log` |
| `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings` (after `cargo clean -p` of the 7 workspace crates) | OK | `r79-clippy.log` |
| `cargo test -j 4 --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`) | **392 passed, 0 failed, 0 ignored** (matches the REPORT: 388 + 4) | `r79-build.log`, `r79-test.log` |
| `render_progress.py --check` | `ok: true` (5 files) | `r79-render.log` |
| `validate_package.py`, `validate_contracts.py`, `test_render_progress.py`, `openapi_spec_validator` | all rc=0 / PASS / OK | `r79-validate.log` |
| Relative links in every changed `.md` | none broken | (inline) |
| `gh pr view 78`, `gh run view 43836416165`, check-runs of f0e0ca9 | see "Ledger" below; the 4 required checks on f0e0ca9 and on 0fd1f64 pass | (inline) |
| Own mutations and experiments (script `r79-mut.sh`) | see below | `r79-mut-*.log` |
| At 0fd1f64: render `--check`, validators, mutants S7-G21/G22 | ok / rc=0 / both caught | `r79-0fd1f64-render.log`, `r79-mut-G21.log`, `r79-mut-G22.log` |

Own mutations. Each was applied alone, its target test was run, and the file was restored (`git status` clean after each):

| id | Mutation | Result |
| --- | --- | --- |
| M1 | newest `branch_head` taken as the oldest (`ORDER BY seq ASC`, `api.rs:4738`) | caught (`delivery_e2e` grpc, :1445) |
| M2 | `Pending::probe` reads but does not write back (`pending.rs:85`) | caught (`serve` `a_refused_first_action…` http+grpc) |
| M3 | guardrail `stale_target` not applied to `no_checkpoint` (`api.rs:4667`) | caught (`delivery_e2e` :1516) |
| M4 | `gitref::resolve` accepts ref names containing `..` (`gitref.rs:60`) | **survived** (L-3) |
| M5 | central checkpoint lookup in `github_gate` by a wrong key, so `pushed_sha` is always null (`api.rs:4700`) | caught (`delivery_e2e` :1423) |
| M6 | CI claim query drops `disposition = 'current'` | **survived** (I-4) |
| X1 | experiment: `limits_e2e` head fixture on another branch | fails at :577 (a plain reconnect becomes `not_observed`, as designed) |
| X2 | experiment: in `delivery_e2e`, an explicit `new_session: true` delivery at the point where the after_run checkpoint has no GitHub record | `201 {"decision":"not_observed","op":null,"outcome":"not_called"}`. Confirms M-1 |

The failing mutant and experiment runs left 5 `ieo_t_dlv_*`/`ieo_t_lim_*` databases on 55432. All were mine: each DB name's pid is one less than the panicking thread id in my logs. I dropped them by name. The 2 left by the G21/G22 re-runs were dropped the same way, and the `ieo_t_*` count is now 0. I did not touch 55433, 58090, PR #29 or other worktrees.

## Analysis

### 1. Resume rule (development)

- `github_judgment` (`crates/ieo-coord/src/api.rs` ~4521) works as follows:
  - no `branch_ref` → `not_observed`;
  - `pushed_sha` null → expect `target_sha`;
  - `pushed_sha` a lowercase SHA → expect it;
  - anything else → `external_reconciliation_required`;
  - no head record → `not_observed`;
  - head ≠ expected → external reconciliation;
  - otherwise `resume_allowed`.

  This matches PLAN §3.2 step 7 exactly. The gate runs only when `judge_resume` already returned `resume_allowed`, so it only ever narrows. It never turns a refusal into `resume_allowed`.
- **"Newest"** is the highest `seq` (bigserial, ingest order) for `(repo_id, kind='branch_head', body->>'ref' = assignment branch_ref)`. Same-id replays are idempotent (200 with no new row; different content is 409), so a replay cannot move the head. A *new* record carrying an older observation does win, because there is no observed-at time. That is acceptable for trusted service input, but it belongs to the 1b collector (I-1).
- **Ref matching** is an exact string match. Both sides go through the same `branch_ref_of`, which requires the full `refs/heads/<name>` and normalizes nothing, so a short name is a 400. That is fail-closed: a collector writing a different spelling gets `not_observed`. The check is looser than git's refname rules (accepts `a//b`, `x.lock`, `@{`, `~`). It is harmless, but the OpenAPI pattern `^refs/heads/` is looser still (I-5).
- **Checkpoint claim.** The expected SHA is read from the *central* row (`role_checkpoints` by the central seq), and that row is digest-equal to the local file whenever `judge_resume` allowed the resume. So editing the local file alone turns into external reconciliation (tested). Same-UID code can still move `refs/remotes/origin/<b>` before the after_run checkpoint is written. That only yields `resume_allowed` if the forged value equals the evidence-service head, so it cannot produce a resume against GitHub's record. The documented limit (N5/§2.1) covers this. `worktree_head` is recorded but not compared, which matches the plan.
- The checkpoint's own `branch_ref` is never compared with the assignment's `branch_ref` (L-2).
- **Supervisor reading** (`crates/ieo-supervisor/src/gitref.rs`):
  - No `git` is executed.
  - It reads loose refs, then `packed-refs`. Peeled `^` lines are skipped, and the loose ref takes precedence, both tested.
  - It reads `HEAD` with one level of `ref:`, and linked work trees via `gitdir`/`commondir`.
  - It accepts lowercase 40/64-hex only.
  - Files over 1 MiB and non-regular files (FIFO, device) are not read.
  - It **does** follow symlinks and an absolute `gitdir:`/`commondir` anywhere on the filesystem (I-2). Under the runtime contract this is not a boundary crossing: the supervisor is PID 1 in the same container with the same UID (`Runtime_Contract_v0.md:34`, `run-real.sh:82-87`).

### 2. CI

- `ci_at` reads only `ci_run` rows with `sha = head` (latest per `source_ref`): all success → `success`, any other conclusion → `failure`, none → `none`. Other SHAs never enter the query, which is tested (S7-G6, and a failure at another SHA is ignored).
- CI never changes the decision, as the plan says.
- The agent's CI claim comes from the newest current `development_progress` report of the same assignment and generation whose `target_sha` is the head. When it differs from the GitHub record, it is recorded with `matches_github: false` and the GitHub result is used. This is consistent with PLAN.

### 3. Guardrail `stale_target`

- The current development target is `assignment_heads(repo, development)` → `assignments.target_sha`. `recordVerdict` uses the same SHA as the judged head (OpenAPI :1343), so it is the right definition.
- Extending the check to `no_checkpoint` and `stale_generation`, so new sessions are blocked too, is a reasoned choice and is documented in REPORT §7 and contract item 24. It is tested both ways: stale → `not_called` with no supervisor call, re-aligned → `no_checkpoint`. M3 is caught.
- The reason strings contain 18 embedded spaces (L-1).

### 4. Delivery and the supervisor L-a pre-check

- `deliverAssignment` records `github` in the new `role_delivery_results.github` column on every path: the base outcome, the re-judgment (`again`) and replay.
- The sent body gains `branch_ref`, plus `github` {branch_ref, head_sha, ci} whenever the judgment carried a comparison. That is also the case for a `resume_allowed` + `new_session` start, not only a resume (part of M-1).
- The `extra` map is applied to the digested `sent` body and to `start_with`, so the digest covers what is actually sent.
- The `github` values come from the first judgment. The re-judgment under the budget lock only compares decision and op, so a CI change in between is not reflected in the body. That is Info only: CI does not gate.
- The supervisor's `Pending::probe` runs under `launch_gate`, after the accepted-attempt replay check and the central comparison and re-query, before any attempt directory, staged copy or run. A failure returns 409 `EXTERNAL_RECONCILIATION_REQUIRED` with `file`, and no attempt id is consumed. `local_only` is skipped. M2 is caught. The accepted step-6 test that expected 202 → `CentralRefused` was replaced by the 409 expectation, and the unsent-report path moved to a new mid-run step 8. That is a legitimate re-targeting, not a weakening: S7-N1..N3 and my M2 are caught.

### 5. Behaviour changes to accepted operations (REPORT §3)

- Items 1–7 are accurate.
- No accepted test was weakened. The removed lines are fixture additions, the 200 ms sleep replaced by a lock-held signal (I-a), the gap count going from 4 to 3, and the step-6 start expectation (now a 409).
- **Missing from §3:** M-1.

### 6. `github_head_check` reuse contract

- It is `pub(crate)` and read-only: three `SELECT`s and no writes.
- It takes an explicit `(assignment_id, generation)` and a checkpoint value, so a non-head generation works. `query_one` errors if the generation does not exist.
- One caveat for 7b on unfinished checkpoints: a takeover-only checkpoint records `pushed_sha` *before* the run. A crashed run that pushed will therefore always be `mismatch`. That may be the intended outcome, but 7b must define it (I-3).

### 7. Tests

- The REPORT's claims are covered (unit decision table, coordinator ref checks, gitref unit test, serve worktree/pushed test, the delivery_e2e step-7 table).
- Two of my six mutants survived: M4 → L-3, M6 → I-4. The REPORT's S7-G21/G22 (added at 0fd1f64) are caught on re-run.

### 8. Ledger, docs, decision record

- `gh pr view 78`: MERGED, merge commit `9b663005…`, head `6e0ad857…`, merged 2026-11-16T09:03:54Z.
- `gh run view 43836416165`: `ci`, push, main, `9b663005…`, success.
- Snapshot 0051:
  - The new event `02-05-S6-MERGED-20261116` matches GitHub. Its observed_at is the snapshot time, the same convention as the S5 event.
  - `branch_sha` and `plan_and_spec` are updated (revision 14, OpenAPI 0.15.0).
  - The review 6e0ad85 archive is added.
  - The stale "(500 가능)" wording from 6e0ad85 I-d is gone from `state.json`, although REPORT §9 still says I-d was left to the ledger owner. Cosmetic.
- `render --check` and the validators pass.
- `User_Decisions_2026-11-16_recovery.md` is faithful to REPORT §9 and does not overreach. §3 explicitly says it approves no real model call, no reserve run, no payment and no budget change. Recovery is not implemented in this PR, only the reusable check.
- OpenAPI 0.14.0 → 0.15.0 is additive in schema terms: two enum values, optional fields and a new schema. The behaviour changes are listed in the header except M-1.

## Findings

| ID | Sev | Where | Failure scenario | Fix |
| --- | --- | --- | --- | --- |
| **M-1** | **Medium** | `crates/ieo-coord/src/api.rs:4494-4504` (gate in `compare_checkpoint`, which has no `new_session` input), `crates/ieo-coord/src/delivery.rs:714` (`("resume_allowed", true) => start`); contradicted by REPORT §2.2 and §7 ("새 세션(start)의 판단은 바꾸지 않았다"), `Runtime_Contract_v0.md` §17 item 22 (same sentence) and §14 item 13 (explicit new session over `resume_allowed` → start) | The GitHub gate turns `resume_allowed` into `not_observed`/`external_reconciliation_required` even when the caller asked for an **explicit new session** of the same generation after an after_run. Example: an assignment without `branch_ref`, or with no head record yet, finishes one run. A `deliverAssignment` with `new_session: true` is then `not_called`. Before this PR it started a new session. Confirmed by experiment X2 (`201 … "decision":"not_observed","op":null,"outcome":"not_called"`). Only a new generation (`stale_generation` + new_session, not gated) gets out. The start body also carries `github`, which the docs restrict to "a resume". It fails closed, but it is an undocumented change to the delivery contract (step 3, item 13), and the docs claim the opposite. | Pick one and make code, docs and tests agree. (a) Apply the GitHub gate only to the reconnect path: pass `new_session` into the judgment, or apply the gate in `deliver` when `op == "reconnect"`, keeping compare's answer informative. Or (b) keep the gate for any op chosen from `resume_allowed`, and then list it in REPORT §3, correct item 22's "start unchanged" sentence and item 13, and state it in the OpenAPI 0.15.0 header. Either way, add an e2e assertion for `new_session: true` over an after_run checkpoint with a missing or mismatched head. |
| L-1 | Low | `crates/ieo-coord/src/api.rs:4685`, `:4688` | The `stale_target` reason strings contain a literal line break of 18 spaces ("…assignment's target                  SHA …", "…a new review                  assignment…"), probably from a lost `\` continuation. They are returned by compareCheckpoint and persisted in `role_delivery_results.reasons`. | Use `\` line continuations (as elsewhere in the file), or split the `format!` arguments. |
| L-2 | Low | `crates/ieo-coord/src/api.rs` `github_head_check` (~4796) | The checkpoint's `pushed_sha` is used without checking that the checkpoint's `branch_ref` equals the assignment's `branch_ref`. A checkpoint written for another branch (direct reconnect with a different `branch_ref`), or one with `branch_ref` absent or null (no push observed), is read as "this branch's pushed SHA", or as "nothing pushed" so `target_sha` is expected. This fails closed in practically every case, since GitHub then usually differs, but the expected SHA's provenance is not what the record says. | When the checkpoint's `branch_ref` is not equal to the assignment's, return `external_reconciliation_required` (or treat the claim as unusable and record it in `github`). Test it. |
| L-3 | Low | `crates/ieo-supervisor/src/gitref.rs:60`; test `head_and_pushed_sha_are_read_from_the_ref_files_only` | Mutant M4 (drop the `..` guard in `resolve`) survives. The test's `pushed(&d, "refs/heads/../../HEAD")` lands on a non-existent `.git/refs/HEAD`, so it returns `None` with or without the guard. The guard matters for a model-written `HEAD` such as `ref: refs/../../x`. | Add a case that would resolve to a real SHA file without the guard, for example `HEAD` = `ref: refs/../HEAD-sha` with `.git/HEAD-sha` holding a SHA, and assert `None`. |
| I-1 | Info | `api.rs` branch_head query (`ORDER BY seq DESC`) | "Newest" means newest *ingested*. A collector that delivers an older observation late (with a new `evidence_id`) makes that the head, which could match a stale expected SHA. This is trusted input in this PoC. | Record it for 1b: give `branch_head` an observed time (or GitHub's push or event id) and order by that. |
| I-2 | Info | `crates/ieo-supervisor/src/gitref.rs` `read_small` and `dirs` | It follows symlinks and absolute `gitdir:`/`commondir` anywhere. There is also a check-then-read race: a FIFO swapped in after `metadata` blocks the read, and a file grown after `metadata` is read unbounded. This is not a privilege gain under the same-UID single-container contract, but REPORT §2.1 and item 21 ("`.git`의 ref 파일만 읽는다") overstate it. | Add a sentence to item 21 saying it follows links and that this is safe only because of the same-UID/same-container assumption. Optionally use `symlink_metadata` and open with `O_NONBLOCK` plus a bounded `take()` read. |
| I-3 | Info | `github_head_check` reuse for 7b | On a takeover-only (crashed) checkpoint, `pushed_sha` is the pre-run value, so a crashed run that pushed is always `mismatch`. | 7b should state whether `mismatch` on an unfinished checkpoint is a refusal (human) or a recovery input, and test it. |
| I-4 | Info | `api.rs` `ci_claim` query | Mutant M6 (drop `disposition = 'current'`) survives: no test has a non-current progress report at the head SHA. It affects only the recorded claim, never the decision. | Optional test. |
| I-5 | Info | `contracts/ieo-api-v0.openapi.yaml` `branch_ref` (`pattern: '^refs/heads/'`) | The pattern accepts `refs/heads/` and `refs/heads/a..b`, which the server rejects with 400. | State the server's extra rules (non-empty name, no `..`, no leading or trailing `/`, no whitespace or control characters) in the description. |
| I-6 | Info | `delivery.rs` (~650) | The body's `github` (CI) comes from the first judgment. The re-judgment under the budget lock compares decision and op only. | Optional: take `github` from `again`. |
| I-7 | Info | REPORT §9 row I-d | It says "원장 담당 몫(손대지 않음)", but snapshot 0051 already removed the "(500 가능)" wording. | Say "원장 0051에서 정리". |

## Summary

- **Must fix before merge:** M-1. Decide whether an explicit new session over `resume_allowed` is gated by GitHub, then align code, REPORT §2.2/§3/§7, runtime contract items 13 and 22, and the OpenAPI header, and add the e2e assertion.
- **Should fix (Low, can ride along):** L-1 (reason strings), L-2 (checkpoint `branch_ref` vs the assignment's), L-3 (test for the gitref `..` guard).
- **Carry-over and Info:** I-1 (observed-time ordering, 1b), I-2 (symlink and gitdir wording, optional hardening), I-3 (7b semantics on unfinished checkpoints), I-4 to I-7 (test and doc nits).
- **Verified correct:** the resume rule and its inputs, CI restricted to the head SHA with the claim mismatch recorded, guardrail `stale_target`, the L-a pre-check (409 before an attempt is consumed), I-a, I-b, I-c, migrations, ledger 0051 against GitHub, and the decision record's scope. 392/392 tests pass.
