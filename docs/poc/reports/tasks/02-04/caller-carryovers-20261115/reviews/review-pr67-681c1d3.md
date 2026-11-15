# Review of PR #67 at 681c1d3 (review 1)

- PR: HeaInSeo/ieo_poc#67, "X-08: 02-04 caller carry-overs (decision item 8) with regression tests — snapshot 0040"
- Head: 681c1d34db89361a90be0034fe3755d2e5453347 (confirmed with `gh pr view 67 --json headRefOid,statusCheckRollup` before and after the review)
- Base: main fda0f2b (fda0f2babe39…), which is also the merge base. Four commits on top: 5ff5f49 fixes and tests, d59ff04 cleanup test, e2d420c report, 681c1d3 ledger snapshot 0040.
- CI at 681c1d3 (run 43711240051, completed, success). I waited for it to finish:
  - docs/poc package and progress validation: SUCCESS
  - rust toolchain pin resolves: SUCCESS
  - rust fmt clippy test: SUCCESS. `tests/call_sh.rs` ran on the ubuntu runner (1 passed in 1.34 s). caller 32, serve 31, collector 38, and coordinator 68 / grpc 9 against the CI PostgreSQL.
  - proto buf lint and breaking (not required): SUCCESS
- Reviewer: independent review agent, read-only. No model calls and no containers. I did not read `/opt/rust/ieo_poc_secrets` or `/opt/rust/ieo_poc_runs`. I did not touch the main checkout, the df02-plan, d05wt or x08wt worktrees, target-x08, or the PostgreSQL on 55433. I used my own detached worktree and target directory and removed both at the end. This is review 1 of at most 4.

## Verdict

**APPROVE: no Blocking or Medium findings. Two Low and five Info findings.**

- Every X-08 item is handled.
- Each fix I reverted was caught by its named regression test (9 of 9 code mutations).
- The four items that were not fixed or were kept are routed to the user decision list in REPORT §4 and in the ledger. None is claimed as fixed: Q-3 is not fixed; Q-2 "older detection wins", L-c3-2 and P4 are kept.
- Budget, usage and correction semantics are unchanged.
- The full suite passes, and caller/serve/call_sh were stable over 4 repeat runs.

**Before X-08 is accepted:** L-1 is a small part of P-2 that is still unfixed. Under the A6 rule ("any no-fix item goes to the user"), it should be added to the user decision list. This can go in the acceptance record or the decision request; it needs no code change. L-2 should be settled when the ledger is updated at merge.

## What I ran

- `cargo test --workspace --locked --all-targets` with `IEO_TEST_DB_URL` on the throwaway DB on 55432: rc 0, **346 passed, 0 failed, 0 ignored** across 32 binaries. This equals `evidence/test.log` (346/0). New and changed binaries: call_sh 1, caller 32, serve 31, collector 38.
- `-p ieo-supervisor --test caller --test serve --test call_sh` 4 times in a row: 4/4 green. The call_sh run took 1.3–1.6 s, caller 32–35 s and serve 13–15 s. `--test collector` once more: 38 passed.
- Validators in `docs/poc` with `.venv-poc`:
  - `validate_package.py`: rc 0, `schema_errors` 0, `ledger_spec_match` true, `generated_views_match` true, 560 internal links checked.
  - `render_progress.py --check`: ok.
  - `test_render_progress.py` and `test_summarize.py`: PASS.
- `cmp` of the archived `reports/tasks/02-05/plan-20261115/reviews/review-pr66-3c017a4.md` against `pending-reviews/review-pr66-3c017a4.md`: identical.
- Secret grep over the whole diff (sk-ant, password, tokens, PATs, Bearer): only test names and the archived review's own sentences. `evidence/test.log` and `mutants.txt` contain no DB URL, run path or secrets path.
- My own mutation spot-checks. Each fix was reverted alone, the named test run, and the file restored; `git status` was clean afterwards:

| # | Mutation | Result |
| --- | --- | --- |
| 1 | P-2: clear the pending tamper on any accepted takeover | KILLED: `a_tamper_the_coordinator_did_not_record_stays_pending` (http and grpc) |
| 2 | L-c3-3: `services_clear` asks only the target role | KILLED: caller `an_unknown_run_of_another_role_blocks_the_call` |
| 3 | same as 2, against call.sh | KILLED: `the_restart_step_runs_only_when_every_guard_holds` |
| 4 | P1: no cleanup of copies after a refusal | KILLED: `a_refused_correction_removes_the_copies_it_made` |
| 5 | P1: agent-id / own-stream check in `remeasure` removed | KILLED: `transcripts_only_for_the_run_s_own_children_and_kept_privately` |
| 6 | Q-2: `pending_readable` back to `exists()` | KILLED: `an_older_pending_detection_is_kept_and_an_unreadable_one_is_replaced` |
| 7 | deadline: the "stop at its deadline" note removed | KILLED: `the_caller_sends_a_stop_at_its_deadline` |
| 8 | F8: read error reported as "no transcript" | KILLED: collector `an_unreadable_transcript_is_reported_as_a_read_error` |
| 9 | call.sh: no refusal on an empty `before` | KILLED: call_sh |
| 10 | call.sh: `podman restart` without `9>&-` | **survived** (I-1) |
| 11 | call.sh: `curl -sf` → `curl -s` alone | **survived** (I-2) |

## Items against the X-08 row (ledger §7, amendment A6)

| Item | Fix real and correct? | Test catches revert? | Report accurate? |
| --- | --- | --- | --- |
| P1 (71e408a) | Yes. `remeasure` refuses an id that is not plain or not a `tool_use_result.agentId` of the run's own stream, and the pre-check runs before any copy. `finish` refuses unsafe ids from the run's own stream: no fetch, no file, the child stays unmeasured. `reevaluate` removes what it created after a later refusal. `stream_agents` is a superset of the parser's `child_results` keys, so the existing public D3 correction (agent `a13188dc0170f8901`, `evidence/real/records.jsonl`) still re-measures. | Yes (5, 4) | Yes. Residue: I-4 |
| P2 C4/C6 | Tests only. The code already did this. | per author's mutants | Yes |
| P3 | `write_private`: new dirs 0700, new files 0600. Existing dirs and files are untouched (`DirBuilder::recursive` does not chmod), so existing ledgers are unaffected. | per author's mutants | Yes |
| P4 | Diagnosis only (moved hint). Routed to the user list as "kept". | per author's mutants | Yes |
| N6 (12c9a21) | `ieo-role-call check` runs the call's read-only guards and re-measures each correction. call.sh no longer trusts correction lines. An old binary without `check` would fall into the call path and fail on missing args (exit 1 → refused). Fail-closed. | Yes (call_sh correction case; caller `check_refuses_what_a_call_would_refuse`) | Yes |
| F8 (608b367) | `parse_with_child_reads` keeps the read error as the unmeasured reason. Still fail-closed. | Yes (8) | Yes |
| L-c3-2 | Kept, named `LOCK_WAIT`, documented. Routed to the user list. New test pins the wait. | per author's mutants | Yes |
| L-c3-3 | Every role in the plan is asked before each call and before R1. Any running role, or any unknown `last` or checkpoint attempt, refuses. | Yes (2, 3) | Yes; the behaviour change is stated in §2 |
| L-c3-4 | `flock -w` on `caller.lock` around check, restart and readiness. `curl -sf`. Refusal on an empty `before`; exit 1 on an empty `after`. | (b) and the empty-status refusal yes; `-f` alone no (I-2) | Yes |
| Deadline-stop test | Part 1: one stop at window + margin, never before, noted in the record. Part 2: the fake runtime sleeps, the run guard stops it at 2 s (`RunTimeout`, `stopped_by_guard`, unmeasured, `stop_required`, marker cleared, next call refused). | Yes (7) | Yes |
| P-2 (704330f) | Direct answer path fixed: pending is cleared only on `tamper_recorded: true` or `duplicate`. Without a coordinator the behaviour is unchanged. The fake now models `ON CONFLICT DO NOTHING`. | Yes (1) | Partly: L-1 |
| Q-1 (ae65f0c) | The staged copy now has the same seq with different bytes, and the refused request shows the copy is dropped and `checkpoint.json` unchanged. | per author's M2 | Yes |
| Q-2 corrupt file | A pending file that does not read as an object is replaced by a fresh detection. | Yes (6) | Yes. The remaining "corrupt file, no fresh detection" case is reported as a new residual with options |
| Q-2 older wins / match-arm pin | Kept and pinned by a test. Routed to the user list. | per author's M4/M5 | Yes |
| Q-3 | Not fixed. Reasons and options (가)/(나) are given, and it is in the user decision list. | — | Yes |

Nothing from the X-08 list is silently dropped. Out-of-scope items (L-c3-1, L-c3-5, F9, F10, PR #58 P-3 4xx) are named in REPORT §5.

## Safety for later real use

- **Every role queried before each call.** Acceptable.
  - An unreachable service gives `Err` (exit 1) before the in-flight marker is written. Nothing is recorded and nothing is charged.
  - A non-200 status gives `NotStarted` (exit 2).
  - Consequence: a ledger can only run against services whose checkpoints name attempts it knows. A future budget (X-02) that reuses the 02-04 role volumes with a new ledger will be refused until reconciled. That was already true for the target role. Note it in X-02 preparation (I-5).
- **call.sh R1.** I read it line by line under `set -uo pipefail` without `-e`:
  - Every failure path ends in an explicit `exit 1`. The lock is released on exit because fd 9 closes with the shell.
  - `chk=$(…)` treats check exit 2 (refused) and exit 1 (error) both as a refusal.
  - `before`/`after` are empty on any curl or Python failure, including `checkpoint: null`. R1 is only run after D1 and G1, so that is fail-closed and correct.
  - `flock -w N 9` is the util-linux fd form. It is compatible with the caller's `File::try_lock`, since both use flock(2).
  - Every external command under the lock gets `9>&-`.
  - A refused restart exits 1, not 2. That is conservative ("a person looks") and consistent with the old script.
- **P-2.** A tamper cannot stay pending forever. Every later new session re-reports it with a new id, and pending plus resume forces a new session, so nothing resumes over it. A silent drop is still possible only in L-1's narrow path.
- **Budget and ledger semantics.**
  - `limits_for`, share arithmetic, record and correction formats are untouched.
  - `notes` gains one informational string, and nothing reads `notes` for accounting.
  - The new agent-id check only adds refusals, never acceptance.
  - No reset path was added.

## Tests: portability and flakiness

- `call_sh.rs` needs bash, curl, flock (util-linux), python3 and GNU `grep -P` / `date +%N`. All are on ubuntu-latest, and CI ran it green.
- The stubs bind port 0 and stay alive for the whole test.
- All scenarios are in one `#[test]`, so there are no other threads forking while the test process holds `caller.lock`. This avoids the fork-window race seen elsewhere.
- The only timing is `CALLER_LOCK_WAIT=1` with an assertion of under 10 s.
- New caller and serve tests take `serial()`.
- The deadline test's bounds are wide: [7.3 s, 12 s). The time-limit test allows under 20 s, duration 2–8 s.
- 4/4 repeat runs were green.

## Ledger snapshot 0040 (diff vs fda0f2b)

- **X-08:** 미착수 → 진행, with REPORT/test.log/mutants as evidence. `unresolved` keeps the acceptance text and adds the user-decision list: Q-3 (가)/(나); kept Q-2, L-c3-2 and P4; the new corrupt-pending residual. X-08 is not marked done, and its `next_action` keeps real calls barred until it is accepted.
- **02-05:** records 3c017a4 approve and merge fda0f2b. Its `unresolved` gains N-1, N-2, N-3 and N-5. N-4 (the resume line treating X-08 as a gate) is addressed in `resume.next_action`/`current_work` ("X-08은 02-05 단계 1~7의 관문이 아니다") rather than listed. The archived review is byte-identical.
- **resume:** branch_sha fda0f2b with main CI 43710871501, running, next_action, current_work; CI note added; event `X-08-PR-20261115` in both `events.jsonl` and `state.json`.
- **Unchanged:** `budget`, `blocking`, 12/73 and 0/12. Generated views match.
- Small items: I-3.

## Findings

### L-1 (Low): P-2 is fixed on the direct answer path only; two other clearing paths still drop a tamper the coordinator did not store

- **Where:** `serve.rs` `write_checkpoint`, the `a["duplicate"] == true` arm (about line 1170); `local_checkpoint`, staged-copy adoption (lines 355–364: `if carries_tamper { remove pending }`).
- **Scenario:** this is the P-2 case (a new session id already used by an earlier tamper event, so the coordinator answers `tamper_recorded: false`), combined with a lost answer.
  - The coordinator stored the takeover but dropped the event.
  - The supervisor sees no answer. On the next start it adopts the staged copy because the coordinator holds exactly those bytes, and removes the pending file because the copy "carries" the tamper.
  - The same happens if a resend gets `duplicate: true`. Neither the `duplicate` answer nor `latest` says whether the event was stored.
- **Impact:** the narrow P-2 drop survives in this path. It is not a regression; this is the old behaviour on these paths.
- **Ask:** fix it (for example, have the coordinator return `tamper_recorded` for duplicates and latest), or list it next to Q-3 in the user decision list as an unfixed part of P-2. Under A6, reviewer agreement alone does not exempt it. REPORT §1 currently describes P-2 as fixed without this qualifier.

### L-2 (Low): the ledger states two different unblock points for real calls

- `resume.blocking` (unchanged, per A6) says real calls are barred until X-08 is **merged**. X-08's `next_action` says they are barred until X-08 is **accepted** (검수 완료), which comes after the user's Q-3 decision.
- The planned order is merge, then the user decision. Between those two, a resuming agent reading only `blocking` could start real calls while Q-3 and L-1 are still open.
- **Ask:** when the merge is recorded, say which condition applies, for example "X-08 병합 및 사용자 결정 반영 전" in `blocking`, or an explicit statement that the merge alone unblocks. Keeping `blocking` unchanged in this PR is correct.

### I-1 (Info): `9>&-` on `podman restart` is untested

- Mutation 10 survives. The stub podman's `flock -n` opens a new file description, so it sees the lock held whether or not fd 9 was inherited.
- The guard matters in real use: a long-lived conmon spawned by podman that inherited fd 9 would hold `caller.lock` and refuse every later call.
- An optional test: a stub `restart` that leaves a background `sleep`, then assert the lock is free after call.sh exits. REPORT does not claim a test for it.

### I-2 (Info): `-f` alone is untested

- With `curl -s`, a 401 body still makes `before` empty (Python KeyError), so the refusal holds.
- The `-f` mainly keeps the readiness loop from breaking on a 401 or 503 answer. Even then, the `after` check gives exit 1.
- The author's mutant combined `-f` with the empty check. Fail-closed either way.

### I-3 (Info): small ledger details

- 02-05 `next_action` still starts "계획 PR 검토·병합 →" although the plan is merged.
- 02-05 `last_observed_at` was not bumped although its `result_state` changed.
- `resume.last_verified_stage` lists "02-05 계획, 02-05 계획 병합(fda0f2b)" (redundant).

### I-4 (Info): cleanup leaves an empty parent and does not cover non-object pending JSON

- `reevaluate` cleanup removes `children/<attempt>/` if it created it, but not a newly created `children/`. This is reachable only for a refusal after the pre-check, for example a write failure.
- Pre-existing and unchanged: the second pending read (serve.rs about line 772) accepts any JSON. A pending file holding a JSON array, with no fresh detection, panics at `t["new_session_id"] = …`. Q-2 now uses `is_object()` for the save condition. Using the same filter at the read would turn this into the residual REPORT §4 already offers options for.

### I-5 (Info): operational note for X-02

Because every role is now checked, a new ledger over existing role volumes is refused until a person reconciles it. X-02 preparation should use fresh volumes, or plan that step. REPORT §2 already says the real `bin/ieo-role-call` must be replaced with the merged build before R1.

## Summary for the author

- Approve as is.
- Before X-08 acceptance: add L-1 to the user decision list (or fix it), and settle the unblock wording (L-2) when you record the merge.
- I-1 to I-5 are optional.
