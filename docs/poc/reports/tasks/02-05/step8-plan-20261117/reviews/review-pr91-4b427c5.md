# Review of PR #91 at 4b427c5 (review 1)

PR: HeaInSeo/ieo_poc #91, head 4b427c5c747734be80184084d76a8991b6cc9acb, base main 14daf8f. Step-8 resume bundle: `ieo-role-call records`, `check-run.py` EFFECTIVE_RECORDS, `restart-roles.sh`, `run-step8-resume.sh`, `make-resume-copies.sh`, `apply-perm-resume.py`, `dry-run-resume.sh`, the O-1 fix (`settlement::prepare_dirs`/`remove_made`), RESUME_PLAN §9, contract item 32.

## Verdict

**CHANGES REQUESTED.** There are 2 Medium findings, 6 Low and 4 Info. No High.

The code parts are correct and well tested: the O-1 fix, `records` and the EFFECTIVE_RECORDS counting. All 32 new allow rules are exact strings. None has a wildcard or prefix, and none gives a way to read secrets or start runs outside the plan. The budget figures in §9.1 are correct.

The two Medium findings are both about operational safety in the split-phase driver and its documented rollback:

- **F-1.** `restart-roles.sh rollback` cannot run in the failure states it is documented for (§9.4 "A4 실패 → rollback").
- **F-2.** `r2g1` does not check that `r1b-check` passed. An allowed command can therefore start R2, a model call, after R1b failed its checks.

Both are small fixes.

## What I ran

All runs were in my own worktree at 4b427c5 and target dir `scratchpad/target-r91`. Root had ≥ 8G free before building (12G).

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` (test DB 55432) | **432 passed, 0 failed** |
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | ok |
| `validate_package.py` (py3.11 venv) | exit 0, schema_errors 0 |
| `render_progress.py --check` | `{"ok": true, ...}` |
| shellcheck on the new scripts, `-S warning` | only SC1090 (non-constant `source`), the same as the existing scripts. The default level also shows SC2015 `A && B \|\| C` in `restart-roles.sh`. Read by hand: in every case C is `die`/`stop`, which is the intended result when any part fails |
| `dry-run-resume.sh` | I read it first. It uses test ports only: coordinator 58290, roles 58231/58232. It refuses 58090/55433/58221/58222, uses the test DB 55432 with a fresh `ieo_t_s8res_$$` database (dropped on exit), container prefix `ieo-s8dry`, fake claude, fake secrets and a scratch ledger. Old binaries were read-only copies of `$D/bin/m-46799b0` (same hashes as the PR's log: 5fca0158…, d6a74214…). New binaries were my debug build. **36 passed, 0 failed.** Afterwards no `ieo-s8dry` container was left and no `ieo_t_s8res%` database was left |
| Hand mutations, Rust, `--test settlement` | 4/4 killed: H1 `records_when_idle` ignores inflight → `the_records_command_prints_the_settled_run`. H2 `prepare_dirs` accepts a symlinked dir → `a_refused_settlement_creates_no_directory_anywhere`. H3 `settle` does not call `remove_made` → `not_enough_evidence_writes_nothing`. H4 `remove_made` is a no-op → `not_enough_evidence_writes_nothing` |
| Python negative path | `check-run.py` with effective records where s8-r1 has no settlement: `CHECK FAILED s8-r2: s8-r1 does not count through its settlement`, rc 1. On the real effective records it passes, `measured totals {'runs': 4, ...}`. No test in the repo covers this negative path (F-9) |
| Hygiene grep of the diff | no credential, no transcript text, no queue text (see §8) |

Nothing touched 58090, 55433, `ieo-s8-*`, the real ledger, `cred/`, `ieo_poc_secrets` or `ieo_poc-data/private`. I made no model call.

## Analysis

### 1. restart-roles.sh

- **Idle check, containers, volumes, credentials.** It checks both roles are idle before doing anything. It only uses `podman stop`/`podman start`, never `rm` or `run`. Volumes are untouched. It never reads `cred/`. The status header is `W/adapter.header`, sent via `curl -H @file`, and is never printed. The bind mount is the directory `W/bin:/opt/ieo:ro`, so the `mv` over `W/bin/ieo-supervisor` is visible after the restart. The image (golang/debian) has `sha256sum`, so the in-container check works.
- **Running twice.** After a successful swap, OLD == WANT, so the `.prev` check passes and the script only restarts again. This is benign. A swap to a *different* new binary is refused, because `.prev` is not the current binary and OLD ≠ WANT. That correctly preserves the original 46799b0 binary.
- **Rollback, then swap again.** `.prev` == OLD, so this is fine.
- **Partial `cp` of `.prev`.** A rerun refuses and asks for a person. This fails closed.
- **Swap failure after `podman stop`.** The script stops with a message. The state is safe: no role is running, so no run is possible. But it is **not restartable by the scripts**: both `swap` and `rollback` begin by requiring both roles to answer idle. A stopped or crashed container answers `000`, so both modes die with "nothing changed". → **F-1**.
- **`idle()`** treats any 200 body without a `running` key as idle → F-7.
- **Rollback SHA.** Rollback takes WANT from `.prev` itself and has no independent pin to 46799b0's 5fca0158… → F-10 (Info).

### 2. run-step8-resume.sh

**settle**
- The dry run comes first. It needs `DRY=1`, and the script prints `still unmeasured`.
- The real run then rebuilds the plan from the coordinator's `--roles` `budget_plan` (`COORD_ROLES`). It reads the ledger with the new `ieo-role-call records` and requires s8-r1 to be measured and settled under `guard_stop_on_old_rule_warning`, with `stop_required` True and `stop_reason == "no result event"`. In other words, R1 stays failed.
- I checked `original_path` and `decision`: settle only records them and never reads them (`settlement.rs:692,725`). So exporting a path into the role volume does not make the agent read it.
- A second settle is refused (dry run step).

**r1b-body**
- Refuses unless s8-r1 is settled.
- Writes the body once and keeps it, so the same bytes go with the same key `s8-dlv-r1b-0001`.
- Body contents: `new_session: true`, and the spare is `{reason, replaces: "s8-r1", evidence_ref}`.
- The script never reads the user token. It only *prints* the path inside the command the user runs.

**r1b-check**
- Requires `('resume_allowed','start','accepted')`, then the wait, then the ledger record, then every §6.2 check on the effective records.
- Also requires `spare.replaces == s8-r1` and `spare.by.kind == user`.

**r2g1**
- R2 reconnects, and `check-run.py` compares R2's session with `R1_ATTEMPT=s8-r1b`. G1 gets a fresh start.
- Retries: `req` only resends on HTTP 000, with the same key and body (a stored answer). Reruns of `run_one` reuse the stored body and key. So there is no automatic retry and no second spare.
- Unmeasured runs are flagged and never counted as 0.
- **Missing:** a precondition that R1b passed its checks → **F-2**.
- **Deadline:** the deadline check uses s8-r1's end. For R2 the coordinator's observation is R1b's stream (`caller.rs:590` takes the newest record). The call that actually uses R1's stream is R1b, and no script checks the deadline before it → F-3.

**Budget.** R1 (settled, 5 s, 1,543/3,223) + R1b + R2 + G1 = 4 runs, checked against `CAPS`/`SHARES`/`LIMITS`. The dry run counted 4 runs, each once.

### 3. check-run.py

- `records()` reads `EFFECTIVE_RECORDS` and keeps only `format == ieo-role-run/v0`. A settlement or correction line is therefore never a run, so there is no double count.
- The settled s8-r1 is counted once with its settlement usage and `duration_ms` 4,119 → 5 s.
- `SETTLED` requires the settlement policy and `measured`. R1 stays `stop_required`, and that is never checked as a success, because `att` is never s8-r1 in the resume.
- The R2 session comparison and the G1 claims use `R1_ATTEMPT`.
- On raw lines the check fails on s8-r1 as unmeasured. The dry run proves this, and it is why EFFECTIVE_RECORDS is needed.

### 4. make-resume-copies.sh, apply-perm-resume.py, and the 32 rules

**Copies.** `step8-resume/` is written from `git show HEAD:<path>` and each file is checked against the HEAD blob.

- Exception: `step8-ops/obs-check.sh` and `rate_limit_policy.py` are copied from the **working tree**, and the old-rule list is parsed from the working-tree `MANUAL_STEPS.md`. There is also no clean-tree check → F-5.
- `SHA256SUMS` covers both trees, and both are `a-w`.
- An existing `step8-resume` is refused.

**apply-perm-resume.py**
- It makes a 0600 backup with a unique stamp, never overwritten, and compares the backup to the original byte for byte.
- It requires every old line exactly once and no new line already present. Lines that appear in both lists are handled: `cd`, `sha256sum -c -`, `wc -l`, `grep -qx 0` and `echo "copies ok"` are removed and then re-added once.
- Deny rules and all other keys are preserved. The dry run checks `deny` and the extra `hooks`/`additionalDirectories`.
- The write is atomic: an `O_EXCL` temp file with the same mode, then `os.replace`. It is read back and checked.
- It is idempotent: a second run prints `already applied`.
- Count check: 8 gate + 3 coordinator + 2 roles + 15 exports + 4 driver = 32. The 28 old lines match the MANUAL_STEPS block, and the dry run printed `allow 28 -> 32`.

**The 32 new allow lines, checked one by one.** Every line is an exact `Bash(...)` string with no `:*` or wildcard.

| Rules | Assessment |
| --- | --- |
| `cd $D`, `echo '<SUMS>  …'`, `sha256sum --quiet --strict -c -`, `sha256sum … -c step8-ops/SHA256SUMS`, `find step8-resume step8-ops -perm /222`, `wc -l`, `grep -qx 0`, `echo "copies ok"` | Unchanged in kind from the approved old set. The stdin readers (`sha256sum -c -`, `wc -l`, `grep -qx 0`) can at most leak a line count, a boolean, or "improperly formatted" for a file fed by redirection. They never leak content (F-8, Info) |
| `coord-switch.sh stop <pid> 900`, `start <new> … --tokens tokens-s8.json -- --roles roles.json`, `start ieo-coord-M … coord-M-rollback.log …` | Fixed arguments. They reuse the captured argv/env/cwd `coord-778ee47`, as in the M start. `--roles` is appended, and the captured argv comes from 778ee47, which has no `--roles`, so there is no duplicate. `env0`/`argv0` stay under the deny list |
| `restart-roles.sh swap <W> <BIN>/ieo-supervisor <SUP>`, `rollback <W>` | Fixed arguments. They cannot create or remove containers or volumes (only `podman stop`/`start`), and cannot read `cred/` |
| 15 `export` lines | All are fixed values. `TOKDIR`/`ADAPTER_HEADER` point at header files that are only used via `curl -H @`. `ORIGINAL_PATH` is only recorded, never read. `DRY=1` exists but the real settle is also allowed, which is fine after the bundled approval |
| `run-step8-resume.sh settle\|r1b-body\|r1b-check\|r2g1` | Fixed phases. There is no rule for `curl`, `cat` or `podman`. None of the rules can read `/opt/rust/ieo_poc_secrets`, any `cred/`, or `private/user/heain.token`. None can start a run other than the planned R2/G1 (R1b requires the user's token). One gap: `r2g1` is not gated on R1b → F-2 |

Also note: the `<확인> &&` gate is advisory. Every gated command has its own exact rule, so it can be run without the gate (F-8, Info).

### 5. Coordinator switch and rollback (§9.3 A5, §9.4)

- `stop` waits up to 900 s, which matches the new build's wait for runs in progress.
- `start` uses only the captured environment, cwd and argv, plus `--tokens` and `--roles`. It refuses a port that is still in use and waits for health 200.
- Rollback to M after the settlement: M reads the settlement line as a non-run line and counts s8-r1 as unmeasured. It then refuses the next delivery with `ASSIGNMENT_USAGE_UNMEASURED` and nothing starts. I reproduced this in my dry run. It fails closed, and nothing in the ledger is rewritten.

### 6. The O-1 fix

- `prepare_dirs` canonicalizes the ledger and checks `children` and `children/<attempt>` with `symlink_metadata` before creating anything.
- It creates each level non-recursively, so a symlink that appears in between makes `create` fail.
- It cleans up its own partial creations.
- `settle` removes the files it created and then the directories it made. Directories that already existed are left alone (tested).
- One residual case: a `write_all`/`sync_all` failure leaves a partial file. That file is not in `created`, so neither the file nor its directory is removed → F-4 (fails closed, Low).
- Tests: two new tests, plus an assertion in `not_enough_evidence_writes_nothing`. My 4 mutations were all killed.

### 7. RESUME_PLAN §9 against decision §2.5

Every item required by §2.5 is present:
- independent check (CONFIRMED WITH CAVEAT, report SHA `28b2f04e…9267`; I verified it equals the blob at head)
- settlement candidate
- remaining budget
- special-case scope
- code SHA placeholder `<SHA>` (release build required)
- swap and rollback
- rule change
- observation validity
- user commands U1 and U2

Arithmetic, checked:
- Total: 4 runs, 2,400 s, 200,000 billable, 2,000,000 cache_read → after R1: 3 runs, 2,395 s, 198,457, 1,996,777. ✓
- Development: 3 runs, 1,800 s, 150,000, 1,500,000 → 2 runs, 1,795 s, 148,457, 1,496,777. ✓
- Guardrail: 1 run, 600 s, 50,000, 500,000 (unchanged). ✓
- R1 elapsed 5 s is `ceil(4,119 ms)`. 4,119 is the ledger `duration_ms` (`evidence/r1-reconciliation.json`), and the budget rounds with `div_ceil(1000)` (`budget.rs:222`). ✓
- Billable 1,543 = 2 + 130 + 1,411; cache_read 3,223. ✓

Inconsistencies:
- §9.1 "관측 … 구동 스크립트는 10분 여유로 확인": the script checks before R2, not before R1b (F-3).
- §9.4 "A4 실패 → rollback" does not work when a container is down (F-1).
- Appendix B's "재개 스크립트" row repeats the same sentence twice (F-12, Info).

### 8. Public-repo hygiene

- The only `sk-ant` string is the dry run's fake `sk-ant-oat01-fake-*-not-a-credential`.
- The DB password is only read from a file.
- The user token appears as a path only.
- The session id and the transcript SHA are already public in contract item 32.
- The dry-run log contains scratchpad paths only. The PR #90 review files contain no transcript or queue text.

## Findings

| ID | Severity | File:line | Scenario | Fix |
| --- | --- | --- | --- | --- |
| F-1 | **Medium** | `launcher/restart-roles.sh:36-38` (both modes), `RESUME_PLAN.md` §9.4 row "A4 실패" | A swap fails after `podman stop`: guardrail stop fails, `podman start` fails, the new binary crashes or never answers idle, or the in-container SHA differs. The containers are then stopped or not answering. `rollback` (and a rerun of `swap`) first requires both roles to answer 200 with `running: null`, so it dies with "role … is not idle (or does not answer): nothing changed". The documented rollback is unavailable in exactly the failure states it exists for. The only way back is manual `podman`/`cp`, which no allow rule covers. Safe (no run can start) but not restartable as documented | In `rollback` (and optionally `swap`), accept for each role either "answers 200 with `running` present and null" or "container not running" (`podman inspect -f '{{.State.Running}}'` = false). Still refuse a role that is running but does not answer, or that reports a run. Skip `podman stop` for a container that is already stopped. Add a dry-run case: stop one container, then `rollback` restores both. Or, at minimum, correct §9.4 and give the user the manual commands |
| F-2 | **Medium** | `launcher/run-step8-resume.sh:221-227` (phase `r2g1`) | The phases are separate commands, each with its own allow rule. If `r1b-check` STOPs on a check the coordinator does not itself enforce, `r2g1` still runs. Examples of such checks: chain not taken over or no first action, `rate_limit_warnings` differing from the stream, pending external requests, a quota mismatch, `no rate_limit_event`, the spare not attributed to `kind: user`. `r2g1` sends R2, which the coordinator admits because the R1b record itself is fine, and then sends G1. That is a model call after a failed run check, against the decision ("R1b·R2·G1 중 하나라도 실패하면 멈추고 보고") and the brief ("stops on any unexpected state"). The original `run-step8.sh runs` gated this implicitly by running R1→R2→G1 in one process. Today only the prose rule in §9.3 ("멈춘 뒤에는 … 다시 실행하지 않는다") prevents it | At the start of `r2g1`, refresh `effective` and rerun `check-run.py "$LEDGER" "$OUT" s8-r1b development S8-DEV-A1` and the spare check, using the `status-/roles-/chain-` files `r1b-check` saved. Stop if any of them fails. Alternatively, have `r1b-check` write a marker containing the hash of the effective s8-r1b record, and have `r2g1` require it. The rerun is better. Add a dry-run negative case |
| F-3 | Low | `run-step8-resume.sh:221-223`, `RESUME_PLAN.md` §9.1 "관측", §7 | The deadline `R1_END + 7 d − 600 s` is checked only before R2. The coordinator's observation is the newest record's stream (`caller.rs:590`), so R1's stream matters only for **R1b**, and `r1b-body` does not check the deadline. For R2 the check is stale: it would wrongly STOP a valid R2 after 2026-11-24T12:09Z. G1 is never checked. The coordinator enforces the 7 days itself, so this fails closed | Check `R1_END + 604800 − 600` in `r1b-body` before printing U2. In `r2g1`, drop the R1-based check (or use R1b's `ended_unix` from the effective records). Correct §9.1 |
| F-4 | Low | `crates/ieo-supervisor/src/settlement.rs:533-540` (`write_in_ledger`) | `create_new` succeeds but `write_all`/`sync_all` fails (ENOSPC: the root FS has filled before). The function returns Err without the path, so `settle` neither removes the partial file nor its directory. That contradicts the O-1 claim "no file content and no directory is left behind". It fails closed: the next settle sees "already holds a different file" | On a write or sync error, `remove_file(&to)` before returning Err |
| F-5 | Low | `launcher/make-resume-copies.sh:38,45-48` | `step8-ops/obs-check.sh` and `rate_limit_policy.py` are `cp`'d from the working tree, and the old-rule list is parsed from the working-tree `MANUAL_STEPS.md`. Only `step8-resume/` is copied from blobs. A dirty checkout would put unverified bytes under `SHA256SUMS` (they are hashed as copied). There is also no check that the tree is clean | Copy those two files from the blob-verified `$D/step8-resume/launcher/`. Read `MANUAL_STEPS.md` with `git show HEAD:`. Refuse when `git -C "$SRC" status --porcelain` is non-empty |
| F-6 | Low | `run-step8-resume.sh:125-128` | When `ieo-role-call settle` fails after the append (readback refused), the script still says "refused (nothing written)" | Say "settle failed: check the ledger (a line may have been appended)" and print the record count |
| F-7 | Low | `restart-roles.sh:34` | `d.get("running") is None` treats any 200 JSON without a `running` key as idle | Require `"running" in d and d["running"] is None` |
| F-8 | Info | `make-resume-copies.sh:52-59`, §9.3 last paragraph | The `<확인> &&` gate is advisory: each gated command has its own exact rule. `wc -l`, `grep -qx 0` and `sha256sum -c -` accept stdin from any redirection. They leak only a count or a boolean, never content. These lines were already in the old set | None needed. Optionally note in §9.3 that the gate is procedural |
| F-9 | Info | `check-run.py:143-147`, `48-49` | The negative paths (settled run missing its settlement, raw settlement line filtered) are only exercised by hand. The dry run covers the positive path and the raw-line failure. I confirmed the negative path by hand | Optional: add a small Python test, or a dry-run case |
| F-10 | Info | `restart-roles.sh:26` | `rollback` trusts `.prev` by its own hash. Nothing pins 46799b0's `5fca0158…` | Optional: print or compare `.prev` against an expected SHA argument |
| F-11 | Info | `make-resume-copies.sh:37-40` | The old `step8-ops/SHA256SUMS` (50507966…) and `obs-check.sh` are overwritten without being kept. They can be reproduced from git, and §5 says `step8-46799b0` is the evidence | Optional: copy them to `step8-ops-46799b0/` first |
| F-12 | Info | `RESUME_PLAN.md:236` | The "재개 스크립트" row repeats the PR #90 L-5 sentence twice | Delete the duplicate |

## Summary

The settlement O-1 fix, the `records` command and the effective-record counting in `check-run.py` are correct and well tested. I got 432/432 tests, 4/4 hand mutations killed, and my own resume dry run gave **36/36** on test-only systems. The budget arithmetic and the 5 s R1 elapsed figure check out against the ledger. All 32 new allow rules are exact. None gives a way to read secrets or to start runs outside the plan, except the ordering gap below. Hygiene is clean.

Two operational-safety gaps block approval:

- **F-1.** The documented role rollback cannot run once a container is stopped or crashed.
- **F-2.** `r2g1` can send R2 and G1 after `r1b-check` failed.

Fix both, with a dry-run case for each. F-3 to F-7 are recommended in the same pass; they are small, and F-3 also corrects §9.1.
