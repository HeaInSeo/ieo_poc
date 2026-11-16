# Review of PR #79 at 47e4e3f (review 2)

Scope: PR #79 (02-05 step 7, C10) at exact head `47e4e3f328734c94e420373d17bad7c4d464003e`. The previous review of record was `0fd1f64` (review 1, changes requested, M-1). The correction commits are:

- `6eaba47`: code, contracts and tests.
- `77aa414`: evidence.
- `47e4e3f`: REPORT §10, the archived review, ledger 0051.

I worked in a fresh detached worktree (`scratchpad/r79b`) and removed it afterwards.

## Verdict

**Approve.** There are no High or Medium findings.

- M-1 is resolved as option (b), and the code, runtime contract items 13 and 22, OpenAPI 0.15.0 and the REPORT now agree.
- L-1, L-3, I-2, I-5, I-6 and I-7 are fixed.
- L-2 is fixed for the case it named. One residual inconsistency remains when the checkpoint names no branch but carries a pushed SHA (new L-4, Low, not a merge blocker).
- **Merge condition (process):** the required check "rust fmt clippy test" **failed** on 47e4e3f (run 43895157928). The failure is a timing flake in untouched gRPC deadline code (I-8). Re-run it and get green before the head-conditional merge.

## What I ran

All logs are in `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/` as `r79b-*.log`.

| Command | Result | Log |
| --- | --- | --- |
| `cargo fmt --check` | OK | `r79b-fmt.log` |
| `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings` (after `cargo clean -p` of the 7 workspace crates) | OK | `r79b-clippy.log` |
| `cargo test -j 4 --workspace --locked` (test DB 55432, `RUST_TEST_THREADS=4`) | **392 passed, 0 failed, 0 ignored** | `r79b-build.log`, `r79b-test.log` |
| `render_progress.py --check`, `validate_package.py`, `validate_contracts.py`, `test_render_progress.py`, `openapi_spec_validator` | ok / rc=0 / rc=0 / PASS / OK | `r79b-validate.log` |
| Relative links in every `.md` changed in `0fd1f64..47e4e3f` | none broken | (inline) |
| Archived `reviews/review-pr79-0fd1f64.md` vs my saved review | byte-identical | (inline) |
| Evidence | `test.log` first line is commit `6eaba47` with a clean tree; `mutants.txt` says 28 of 28 killed | (inline) |
| Check-runs on 47e4e3f | 3 success; **"rust fmt clippy test" failure** (I-8) | (inline) |
| Own hand mutations (`r79b-mut.sh`) | see below | `r79b-mut-*.log` |

Own mutations and experiments. Each was applied alone, its target test was run, and the file was restored (`git status` clean after each):

| id | Mutation | Result |
| --- | --- | --- |
| N1 | L-2 override disabled (`api.rs:4763`, `if false && c != a`) | caught (`delivery_e2e` :1552) |
| N2 | my review-1 M4 again: `gitref::resolve` accepts `..` (`gitref.rs:60`) | **caught** now (`gitref` unit test :136). L-3 resolved |
| N3 | I-6: the result row's `github` dropped after the re-judgment (`delivery.rs` `o.github = None`) | caught (`delivery_e2e` :774) |
| X3 | experiment: a checkpoint reported with `branch_ref: null` and `pushed_sha` = the branch's GitHub head (`SHA_P`), target `SHA1` | `"resume_allowed" "pushed_sha" "3333…" "the checkpoint names no branch: no push observed (target SHA expected)" null`. Establishes L-4 |

The 3 `ieo_t_dlv_http_*` databases my failing runs left were identified by name and creation time (between 00:26 and 00:27 KST, matching my logs) and dropped. The `ieo_t_*` count is now 0. I did not touch 55433, 58090, PR #29 or other worktrees.

## Analysis

### M-1, resolved as option (b)

- **Code.** The gate stays in `compare_checkpoint`. `choose_op` maps `("resume_allowed", true)` to start, so a same-generation explicit new session needs the GitHub match. `no_checkpoint` → start and `stale_generation` + new_session → start never pass through `github_gate`'s development branch, which needs `decision == "resume_allowed"`, so they are not gated. That is what the docs now say.
- **Contract.** The revision-14 header states the same-generation new session is gated and the new-generation and `no_checkpoint` starts are not. Item 13 adds the revision-14 sentence. Item 22 replaces "새 세션(start)의 판단은 바꾸지 않았다" with the full rule and its reason (same branch and work tree → start from a confirmed head).
- **OpenAPI.** The header's behaviour changes, the compareCheckpoint description and the `GithubComparison` description all agree.
- **REPORT.** §2.2, §3 item 7 (now listed as a change to deliverAssignment, step 3, before acceptance), §7 and §10 agree.
- **e2e** (`delivery_e2e`):
  - missing head → `not_observed`/`not_called`, `github.result: not_observed`;
  - different head (`SHA2`) → `external_reconciliation_required`/`not_called`, `mismatch`, with launches and runs unchanged;
  - matching head → the a3 delivery is now `resume_allowed`/`start`/`accepted`, with the new session `s9` and `github.ci` success in the result.

  The reconnect path with a matching head is still covered at step 4 (a2, the sent body's `github`). The REPORT's S7-M1 mutant covers the gate on new sessions.

### L-2

`github_head_check` now returns `external_reconciliation_required` when the checkpoint's `branch_ref` differs from the assignment's, unless the result is already `not_observed`. It records `checkpoint_branch_ref` and `push_provenance`.

- The e2e covers another-branch with the pushed SHA equal to this branch's head → mismatch, with the reason naming both branches. N1 is caught.
- The provenance values are honest for checkpoints the supervisor writes: the supervisor sets `pushed_sha` only through a non-null `branch_ref` (`serve.rs` ~1298).
- They are not honest for a checkpoint with `branch_ref: null` and a SHA `pushed_sha` (L-4).

### Other review-1 items

- **L-1.** Fixed with `\` continuations. The test asserts one line with no double spaces.
- **L-3.** Fixed. The `ref: refs/../HEAD-sha` case kills the mutant (N2).
- **I-2.** Item 21 now states that it follows links and is safe only under the same-UID/same-container assumption. Hardening is deferred (REPORT §8).
- **I-5.** The OpenAPI description lists the server's extra 400 rules.
- **I-6.** Fixed correctly. `github` is re-bound from `again` before `extra` is built, so the sent body (and its digest) and the result row both use the judgment made under the budget lock. N3 is caught.
- **I-7.** Fixed.
- **I-1, I-3, I-4.** Recorded as carry-overs in REPORT §8 and in the 02-05 `unresolved` field of ledger 0051 (1b; step 7b; optional test).

### Regressions in 0fd1f64..47e4e3f

- Code changes are limited to `api.rs` (L-1 and L-2), `delivery.rs` (I-6) and the `gitref.rs` test.
- In `delivery_e2e`, the delivery and result counts rise to 21 and 23 because of the two gated new sessions. The a3 expectation moves from reconnect to start (intended). No other assertion was removed.
- Ledger 0051 adds a correction summary, the archived review and three carry-overs. `render --check` passes.

## Findings

| ID | Sev | Where | Failure scenario | Fix |
| --- | --- | --- | --- | --- |
| L-4 | Low | `crates/ieo-coord/src/api.rs` `github_head_check` (~4752-4779); `Runtime_Contract_v0.md` item 22 ("체크포인트에 브랜치가 없으면 push를 관측하지 않은 것으로 보고 target SHA를 기대한다"); REPORT §2.2 | `github_judgment` still uses the checkpoint's `pushed_sha` when the checkpoint's `branch_ref` is null. A checkpoint with `branch_ref: null` and `pushed_sha` equal to the branch's GitHub head is `resume_allowed` with `expected_from: pushed_sha`, while `push_provenance` says "the checkpoint names no branch: no push observed (target SHA expected)". The contract says target SHA expected (X3: target `SHA1`, head `SHA_P`, decision `resume_allowed`). The supervisor never writes this combination, and a same-UID forger gains nothing beyond naming the branch, so this is a record/contract inconsistency, not a bypass. | When the checkpoint names no branch, pass `Value::Null` as the pushed SHA (expect the target SHA) or refuse a non-null `pushed_sha` as external reconciliation. Keep `push_provenance` aligned and add the X3 case to the e2e. Can be carried to 7b, which reuses the check. |
| I-8 | Info (merge process) | Run 43895157928, job "rust fmt clippy test" on 47e4e3f: `crates/ieo-coord/tests/grpc.rs:1994` `a_statement_past_the_deadline_rolls_back_as_deadline_exceeded` got `Cancelled ("Timeout expired")`, expected `DeadlineExceeded` | A server cap of 700 ms raced the transport timeout on the CI runner. Neither the test nor the gRPC code is touched by this PR (`git diff 9b66300 47e4e3f` of `tests/grpc.rs`, `src/grpc*` and `ieo-proto` is empty), and it passes locally. The required check is nonetheless red at the head. | Re-run the job and merge only with all required checks green at the approved head. If it recurs, record it as a flaky test against the gRPC deadline work. |
| I-9 | Info | REPORT §5 | Review-1's M6 (`disposition = 'current'`) is still not tested, as the REPORT openly states. | Carry-over (I-4). |

## Summary

- **Must fix before merge:** none in the code. The process condition is that the failed required CI job (I-8) is re-run and green at 47e4e3f before the head-conditional merge.
- **Carry-over:** L-4 (no-branch checkpoint with a pushed SHA; fix with or before 7b, which reuses `github_head_check`), plus I-1 (observed-time ordering, 1b), I-3 (7b semantics for crashed checkpoints) and I-4/I-9 (CI-claim disposition test), all recorded in REPORT §8 and ledger 0051.
- **Verified:** M-1, L-1, L-2 (named case), L-3, I-2, I-5, I-6 and I-7 are resolved. 392/392 tests pass. 3/3 of my new hand mutants are caught. The docs, contract and OpenAPI agree, and the ledger renders and validates.
