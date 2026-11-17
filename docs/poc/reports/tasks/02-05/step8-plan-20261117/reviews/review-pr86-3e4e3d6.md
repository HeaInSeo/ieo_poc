# Review of PR #86 at 3e4e3d6 (review 1)

PR: HeaInSeo/ieo_poc #86, "02-05 step 8: execution plan for one bundled approval, role launcher, dry run (21/21)". Head `3e4e3d68dfa6e6a82fd6899c53302ad0ca2a9c20`, base main `05cf16e`. Independent, adversarial review. I did not write this PR.

## Verdict

**Changes requested.** I found 4 Medium findings and no High. The values (ids, limits, budget, endpoints, scopes, bodies, target facts) check out against the code, GitHub and the decisions. The dry run reproduces 21/21. The problems are in what the bundle would let happen on the live system:

1. **M-1.** The development role's "Read/Grep/Glob only, no child agents" is **not enforced**. The supervisor passes no `--tools`/`--allowedTools`/`--disallowedTools` for development. 02-04's real init shows that the CLI then offers `Task`, `Bash`, `Write`, `WebFetch`, `Workflow`, `RemoteTrigger` and others, and that a child run made a run unmeasured (D3).
2. **M-2.** The rollback path in the bundle runs `dropdb` on the persistent DB **before** restoring from a dump that was never test-restored. That step is not needed: the migration is atomic and 778ee47 runs on the additive schema.
3. **M-3.** The coordinator stop/start is not specified closely enough to run. The plan says "same `IEO_DB_URL`" but never says where that value comes from. It is only in `/proc/<pid>/environ` of the process that step 2 kills. The plan also leaves out how the new process is detached, where its log and pid go, and checks that the pid really belongs to the old coordinator.
4. **M-4.** No real-run driver is committed. The live sequence (register → assign → R1 → R2 → G1 with the §6.2 checks and the G1 prompt assembly) would be typed ad hoc. The dry run exercises the launcher and the API shapes, not the live commands, the §6.2 checks or the G1 prompt construction.

All four can be fixed in this PR without code changes: launcher settings, plan text and one script.

## What I ran

- Worktree at exact head `3e4e3d6` (detached). `git diff --stat 05cf16e HEAD`: 17 files, docs/launcher/ledger only, no code. The binaries at the merge SHA M are therefore the same as the ones the dry run used.
- Read: the decision `User_Decisions_2026-11-17_step8-l01.md`, `EXECUTION_PLAN.md`, `sprint-POC-S8-01-r1.json`, `launcher/*` (both scripts and the role settings), `evidence/dry-run.log`, RUNBOOK §1–§11, and the 02-04 `run-real.sh`, `role/development/settings.json`, `prompts/*` and `evidence/real/*.json`. Code: `crates/ieo-supervisor/src/args.rs`, `caller.rs` (observation, limits), `budget.rs` (`BUDGET_0205`, 7-day rule), `crates/ieo-coord/src/{delivery.rs,roles.rs,auth.rs,schema.sql,bin/ieo-coord.rs}`, and the 778ee47 versions of `bin/ieo-coord.rs`, `api.rs`, `auth.rs` and `schema.sql`.
- **Dry run rerun** with the committed script, unchanged: `TEST_DB_PASSWORD_FILE=<pgpw> dry-run-step8.sh <copy of target-x08 binaries> <scratch>`. Binaries supervisor `bff0a2fc01a1` and coordinator `cf3070174fc5`, the same hashes as the committed log. Test PG on 55432, test coordinator on 58290, fake claude. **Result: 21 passed, 0 failed**, the same as `evidence/dry-run.log`. The DB `ieo_t_s8dry_2582649` was dropped by the script's trap (I checked `pg_database`). No containers are left. I deleted the scratch workdir.
- From the dry-run volumes: the development supervisor's recorded `argv` (start and resume), and the permissions of `state/work/ieo-poc-test` and `cred/`.
- A loopback reachability probe from a container on `slirp4netns:allow_host_loopback=true`. It was a TCP connect to `10.0.2.2:55432` (the test DB, not 55433) and it **connected**.
- `ieo-spec check sprint-POC-S8-01-r1.json --manifest … --executable` → 201 `ACCEPTED_FOR_SEMANTIC_REVIEW`, digest `bbd2cfbd…7d327c` (= plan §1.2). The dry run's registration returned the same digest.
- GitHub (read-only): `HeaInSeo/ieo_poc_test` main = `2ab9d8760012516f37674a9446ca2fb2699973c0`, public, one branch. Tree: `.github/workflows/bootstrap.yml`, `.gitignore`, `README.md`, `SANDBOX.json`. Contents read: no prompt-injection-like text. `role_credentials` = `NOT_CONFIGURED`. Workflow `sandbox-bootstrap`, triggers `push`/`pull_request`, job `repository-smoke`. `gh pr view 85`: merged, merge commit `05cf16e653fd…`, head `5b21540`. Run `44016542138`: push on main @ `05cf16e`, success. PR #86 checks: docs/proto/toolchain pass, rust job pending when I looked.
- `docs/poc`: `scripts/validate_package.py` → exit 0, `schema_errors` 0, `generated_views_match` true, 680 links checked. `scripts/render_progress.py --check` → ok.
- Secret grep of the whole diff (`sk-ant`, `password=`, `Bearer <value>`, `ghp_`, `github_pat`, long base64). Only the literal fake `sk-ant-oat01-fake-…-not-a-credential` and format/prefix checks matched. Nothing real is in the plan, the scripts or the dry-run log.
- Not done: anything on 58090/55433, real `claude`, `/opt/rust/ieo_poc_secrets`, `/opt/rust/ieo_poc_runs`, `ieo_poc-data`.

## Analysis per area

### 1. Values against code

| Value | Plan | Check | OK |
| --- | --- | --- | --- |
| repo_id | `ieo-poc-test` | lowercase-hyphen. Used as `repo` in the supervisor config → cwd `/state/work/ieo-poc-test` (`args.rs:95`) | yes |
| target_sha / branch_ref | `2ab9d876…c0` / `refs/heads/main` | GitHub main head. The launcher checks `rev-parse` equality | yes |
| sprint / task | `POC-S8-01` r1 / `90-01` | `ieo-spec` 201, digest matches. The dry run registered it | yes |
| assignment limits | dev 1,800 s/150,000, GR 600/50,000. Revision 1,800/150,000, `correction_attempts` 0 | equal to the L01 decision, ≤ the revision | yes |
| budget | `02-05`, no `share`, `max_window_seconds` 600 both | `BUDGET_0205` (`budget.rs:114`): 4 runs, 600 s/run, 2,400 s, 200k, 2M, shares dev 3/1,800/150k/1.5M, GR 1/600/50k/500k, spare dev only | yes |
| first observation | D6 `observed_unix` 1794698043 → deadline 2026-11-21T23:14:03Z | `date -u`: 2026-11-14T23:14:03Z, +604,800 s = 2026-11-21T23:14:03Z. Only R1 depends on it: R2 and G1 use the newest 02-05 record (`caller.rs:549–557`) | yes |
| principals | 7 principals, kinds and scopes as in §2 | `auth.rs`: `checkpoint:report` is an agent scope, `roles:control` a client scope. The 778ee47 → HEAD diff only adds scopes, so the existing `tokens.json` entries still parse | yes |
| ids / keys | attempt `s8-r1/r2/g1`, keys 11–14 bytes, lowercase UUID from `/proc` | 8–128 bytes. The UUID regex is checked before sending | yes |
| delivery body | `{assignment_id, generation, attempt_id, session_id, prompt}` | `delivery.rs`, dry run | yes |
| roles.json | dev with `quota_group_id`, **GR without** | `roles.rs:87–93`: the mismatch check runs only where the entry names one → L-2 | partly |

### 2. Launcher security

- **Tool allowlist (M-1).** Guardrail is solid: `--tools Read,Grep,Glob,Write,Edit`, allow `Read(//state/review/**)` etc., deny `Read(//run/ieo/**)`, `Read(//etc/ieo/**)`, `Read(//proc/**)`, `Bash` (`args.rs:17–32, 80–89`). 02-04 G1's init shows exactly `[Edit, Glob, Grep, Read, Write]`. **Development has none of this** (`args.rs:73–75`: "relies on the read-only settings and permission mode"). The dry-run argv confirms it: no `--tools`, `--allowedTools` or `--disallowedTools`. The real CLI 2.1.322 under the same flags offered development `Task, Bash, CronCreate, …, RemoteTrigger, …, SendMessage, Skill, …, WebFetch, WebSearch, Workflow, Write` (02-04 `evidence/real/D1.json` `init.tools`) and agents `claude, Explore, general-purpose, Plan, statusline-setup`. The plan's step-8 settings only shrink the `allow` list to `Read, Grep, Glob`. Whether `dontAsk` then refuses `Task`/`Agent` and the other tools that ask for no permission was **never observed**: in 02-04 `Agent` was explicitly allowed. The dry run cannot show it because the fake claude prints a fabricated `"tools":["Read","Grep","Glob"]`. Unscoped `Read` also lets the development model read `/run/ieo/cred/oauth_token` and `coordinator_token`, which the guardrail denies.
- **Loopback (L-1).** `slirp4netns:allow_host_loopback=true` maps `10.0.2.2` to **all** host loopback ports. I confirmed this with a TCP connect to 55432 from a container. So 55433 (persistent PG, password-protected), 55432, 58090, 58221/58222 and any other local service can be reached from inside both role containers. 02-04 used plain `slirp4netns`. Only the supervisor needs this, and only for 58090. The model can only use it through a network-capable tool, which ties it to M-1. Postgres still needs a password, and none is in the containers. The plan presents this as "the coordinator stays on 127.0.0.1" and never says that the exposure goes the other way. It should say so and list it in §8.
- **Credentials into containers.** OAuth token (0400, role UID), supervisor agent token (0400), adapter sha256. No GitHub credential and no `.git` (`git archive`). `cred/` is `dr-x------` and owned by the role UID. `$W` is 0700. Adapter header files are 0600, so no token appears in argv. Good.
- **Read-only target (L-3).** `chmod -R a-w` and then `podman unshare chown` to the role UID. The UID owns the files, so this read-only setting is only mode bits (the owner can `chmod`, which needs Bash, which is denied). A separate `:ro` bind mount would be stronger. Running the real CLI with a cwd that has no write permission is also untested.
- **Fresh volumes and names.** The launcher refuses an existing workdir or container. New ports 58221/58222. Good.
- **No child agents.** True for the guardrail (`Task` not in `--tools`). Not enforced for development (M-1).

### 3. Redeploy sequence

- **Stopping 778ee47.** The plan is right that 778ee47 is a host process with no handler (778ee47 `bin/ieo-coord.rs` has no `signal_hook`), so the default SIGTERM ends it at once. 778ee47 has no background jobs (only HTTP workers), so the only in-flight risk is an HTTP request, and its transaction rolls back. The pre-checks cover `ieo-role-call` and the 02-04 ledger, but not other HTTP clients of 58090 (L-4). The plan does not check that the pid in `coord.pid` really is that `ieo-coord`, and does not check for a parent or wrapper that would restart it (M-3).
- **Start of M (M-3).** "Existing arguments + `--roles`, the same `IEO_DB_URL`" is not executable as written. The arguments and the env (with the DB password) must be captured from `/proc/<pid>/{cmdline,environ}` **before** step 2, into a 0600 file, without printing. Otherwise neither M nor the rollback 778ee47 can be started without asking the user for the password. Also unspecified: detach (`setsid nohup …`, so the process outlives the agent's shell), the log file, the update of `coord.pid`, and the cwd.
- **Backup.** `pg_dump -Fc` to `/opt/…/backups/<ts>-pre-step8/` with a sha256 and a `pg_restore --list` check. That check only reads the table of contents; it does not show that the dump restores (M-2).
- **Migration.** `migrate` = `batch_execute(schema.sql)`, which is one simple-query string with no explicit BEGIN/COMMIT and no `CONCURRENTLY`, so PostgreSQL runs it as **one implicit transaction**: a failure leaves the DB unchanged. The schema diff since 778ee47 is additive: one changed line, the `evidence.kind` CHECK widened through the guarded `DO` block. No existing `CREATE OR REPLACE` bodies change, and 778ee47's code has no `INSERT … VALUES` without column lists and no `SELECT *`. **778ee47 therefore runs on a migrated DB.** The migration from a 778ee47-shaped DB was never exercised, though: the dry run starts from an empty DB (M-2).
- **Rollback (M-2).** §3/§5.4: `dropdb` → `createdb` → `pg_restore --exit-on-error` on the persistent DB, automatically in 5.2-4. If the restore fails after the `dropdb`, the persistent DB is gone. Given the atomic migration and 778ee47's compatibility, the right automatic rollback is to stop M, start 778ee47 with the captured arguments and env on the DB as it is, and check health. A DB restore should be a separate user decision, done into a **new** database name and then renamed.
- **Times.** Backup 2–3 min for about 680 MB, stop 1 min, files 3 min, start 1 min, roles 3 min: about 15 min is plausible. The decision points in §5.4 are sensible apart from M-2/M-3.
- **New build fails mid-run** (crash during R1–G1). The role finishes its run, the coordinator thread that would write the record is gone, `inflight.json` stays, and the code blocks further calls. That case is in §6.3 ("`inflight.json`이 남음") and in §7 (restart outside the bundle, RUNBOOK §8). Acceptable. The report should say that the run is then unmeasured until a person reconciles it.

### 4. Run sequence and stop conditions

- §6.1 expectations match the dry run and RUNBOOK §5: R1 `no_checkpoint`→`start`, R2 `resume_allowed`→`reconnect`, G1 `start`.
- §6.3 matches RUNBOOK §7/§9: unmeasured, caps, limit signals, `stop_required`, unexpected judgments, `pending_external_requests`, leftover `inflight.json`, ledger/chain mismatch, 409 first action, secret printed, G1 never retried, no automatic retry.
- Two small gaps (Low). First, the re-send on a timeout with the same key and body is a read of the stored answer, not a retry: say so explicitly. Second, `quota_groups.mismatches` not empty should be a stop condition.
- §7: unmeasured is recorded as unmeasured, never 0. Spare only with the user's own credential and a per-case approval (§6.4, §8 excluded). Observation deadline in §8 and §9. The overage signal check is in §6.2 item 3. Good.
- G1 admission uses R2's stream as its no-overage observation. That is right only if the guardrail credential is on the **same** account (L-5, ties to §9 Q2).

### 5. Prompts

- R1, R2 and G1 are short, read-only and answer-only. They match the target files (README first line `# ieo_poc_test`; `role_credentials` `NOT_CONFIGURED`; keys; `sandbox-bootstrap`; `push, pull_request`; `repository-smoke`). R2's last line checks the resume. Size is fine: 02-04 had similar runs.
- **Can a model action write, push, spawn or reach the network?** Guardrail: no (enforced, and Write/Edit are only allowed in the scratch directory). Development: the prompt says "Use only the Read tool", but enforcement depends on `dontAsk` + `allow` only. `Task` and other tools that need no permission may be usable, and unscoped `Read` reaches the credential files (M-1). There is no push path either way: no `.git`, no GitHub credential, and Bash is not allowed.
- **G1 independence.** It has a separate container, credential and volume, and only reads the commit copy plus the developer's claims. The G1 prompt pastes the developer output verbatim with no length cap (L-6). It is also ambiguous whether the claims are numbered per line or per result (L-6).

### 6. §8 bundle and §9 questions

- **Included** items 1–7 are complete for the normal path, with these exceptions.
- **Missing from the bundle text:** (a) the network posture change `allow_host_loopback=true` (L-1); (b) reading the old process's `/proc/<pid>/environ`, which contains the DB password, without printing it (M-3); (c) rewriting `coord.pid` and the log location (M-3); (d) the `git clone` network read of the target repo (harmless, but it should be stated).
- **The automatic rollback** in item 3 includes `dropdb` on the persistent DB (M-2). It should not be inside a one-time bundle approval.
- **Excluded** (spare, retries, restart, GitHub writes, other budgets, extra charge): correct.
- **User's own tasks** (token and digest, semantic review, spare decisions): correct, and the token handling keeps the user's token away from the agent.
- **§9:** Q1 ids, Q2 `quota_group_id`, Q3 deadline. Minimal and correct. Q2 should add: "if different accounts, G1's no-overage basis is not that account's: decide before G1" (L-5).

### 7. Dry run versus real run

The same `run-step8-roles.sh`, image, network, central checkpoint path, sprint file, principal shapes, endpoints and bodies. Differences:

| Difference | Risk |
| --- | --- |
| Fake claude ignores argv and prints a made-up tool list | the tool restriction is untested (M-1) |
| Empty fresh DB; no `--profiles`, `--bindings` or `--workers`; `tokens.json` has only the new principals | migration from the 778ee47 schema, `check_bindings` on the real bindings and the merged `tokens.json` are untested (M-2, and checked at start by §5.1/§5.2-4) |
| First observation is a fresh fake stream | the age path differs. The code is tested elsewhere; the deadline is a §8 check |
| Real live sequence: no script | the §6.2 checks (overage, env names, tool uses, budget totals) and the G1 prompt assembly are not in any committed code (M-4) |
| Test credentials and DB password from a file | no risk |

### 8. Ledger snapshot 0058

`v1.3.3-EXEC-0058`. The event `02-05-PRE8D-MERGED-20261117` matches GitHub: PR #85 merged as `05cf16e`, CI `44016542138` success on `05cf16e`. 12/73 and 0/12 unchanged; model calls 0. The decision file is cited. `validate_package.py` and `render_progress.py --check` pass.

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| **M-1** | Medium | `launcher/role/development/settings.json:4`; `crates/ieo-supervisor/src/args.rs:73-75`; `EXECUTION_PLAN.md:38,113,165` | The plan states "개발 허용 Read·Grep·Glob만", "자식 에이전트 없음" and "02-04의 Edit·Agent 없음" as if they were enforced. The development argv has no tool restriction (dry-run `argv` confirms). The real CLI offers `Task`, `Bash`, `Write`, `WebFetch`, `Workflow`, `RemoteTrigger`, `SendMessage`, … (02-04 D1 init). Whether `dontAsk` without an allow refuses tools that need no permission (`Task`/`Agent`, possibly others) was never observed. If R1 or R2 spawns a child, the run becomes unmeasured (02-04 D3). That stops 02-05, and the spare cannot be used. Unscoped `Read` also reaches `/run/ieo/cred/*` | Add `permissions.deny` to the development settings: `Task`, `Agent`, `Bash`, `Write`, `Edit`, `NotebookEdit`, `WebFetch`, `WebSearch`, `Workflow`, `RemoteTrigger`, `CronCreate`, `CronDelete`, `ScheduleWakeup`, `SendMessage`, `PushNotification`, `EnterWorktree`, `ExitWorktree`, `Monitor`, `Skill`, `TaskStop`, `ListAgents`, `DesignSync`, `ReportFindings`, plus `Read(//run/ieo/**)`, `Read(//etc/ieo/**)`, `Read(//proc/**)`, `Read(//state/claude/**)`. Narrow allow to `Read(//state/work/ieo-poc-test/**)`. State in the plan that this enforcement comes from settings (deny wins over `dontAsk`) and is not observed on the real CLI. Add a §6.2/§6.3 check that R1's `init.tools`/`agents` and `tool_uses` contain no child or non-Read use, and stop otherwise. A supervisor `--tools Read` for development is the stronger fix but needs code (carry-over) |
| **M-2** | Medium | `EXECUTION_PLAN.md:101-109,173,178,234` | The automatic rollback drops the **persistent** DB and then restores from a dump checked only with `pg_restore --list`. If the restore fails, the DB is lost inside a step the user pre-approved. It is not even needed: `migrate` is one implicit transaction (a failed migration changes nothing), and 778ee47 is compatible with the additive schema. Separately, the migration from the 778ee47 schema with real data and bindings was never rehearsed | (a) Rollback in 5.2-4 = stop M, start 778ee47 with the captured arguments and env on the DB as it is, check health. **No** `dropdb` in the bundle. (b) A DB restore is a separate user decision, done into a new DB name and then renamed. (c) Before step 2, test-restore the dump into a scratch DB on the **test** container 55432, compare row counts per table with read-only counts on 55433, run `ieo-coord migrate` of M against that copy (and optionally a `serve` on 58290 with the real `--bindings`/merged `tokens.json`), then drop it. This checks the backup, the migration and `check_bindings` before anything live changes |
| **M-3** | Medium | `EXECUTION_PLAN.md:139-141,173,234` | "Existing args + `--roles`, same `IEO_DB_URL`" has no source after step 2 kills the process: the env (with the DB password) is only in `/proc/<pid>/environ`. Neither M nor the rollback 778ee47 can then start without asking the user. Also missing: checking that `coord.pid` is the 778ee47 `ieo-coord` on `127.0.0.1:58090` (pid reuse), checking for a parent or restarter that would bring 778ee47 back, how to detach, and where the log and pid go | Before step 2 [read]: check `/proc/$pid/exe` and `cmdline` (binary path, `--listen 127.0.0.1:58090`), the ppid or a unit or wrapper, and `ss -tnp` clients. Copy `cmdline` and `environ` into `ieo_poc-data/private/s8/coord-778ee47.{argv,env}` (0600, `tr '\0' '\n'`, never printed). Give the exact start line, e.g. `set -a; . env; set +a; setsid nohup bin/ieo-coord serve <argv…> --roles … --tokens … >> ieo_poc-data/coord-M.log 2>&1 & echo $! > coord.pid`, and the same for the rollback with `bin/ieo-coord.778ee47` and the old `tokens.json`. Put "read the old process env without printing it" into §8 |
| **M-4** | Medium | `EXECUTION_PLAN.md:46,115,184-200`; no script for the real run | The bundle approves a live sequence (registration, assignments, evidence, R1→R2→G1, per-run checks, G1 prompt assembly) that exists only as prose. The committed dry run checks fewer things than §6.2 (no overage-signal, env-name, tool-use or cumulative-budget checks) and uses a placeholder G1 prompt. Ad-hoc typing against 58090 is how a wrong key, a new-key resend (a second delivery) or a skipped stop happens | Commit `launcher/run-step8.sh`, sharing `req`, `dlv`, `role_wait` and `settled` with the dry run. Parameterized endpoints and token files; hard stops for every §6.2/§6.3 condition; a deadline check (`now < 1795302843` minus a margin) before R1; G1 prompt assembled from the R1/R2 ledger `result` strings with a byte cap; timeout resend only with the same key and body file. Make `dry-run-step8.sh` call that driver against the test coordinator, and rerun it |
| L-1 | Low | `launcher/run-step8-roles.sh:24-27`; `EXECUTION_PLAN.md:114` | `allow_host_loopback=true` exposes **every** host loopback port to both role containers (I confirmed a TCP connect to 55432 from inside). That includes 55433 PG and other local services. It is a change from 02-04's plain `slirp4netns`. Exploiting it needs a network-capable tool (M-1) and, for PG, a password | State the exposure in §4/§5.3 and list it in §8. Rely on M-1's deny list. Optionally, add a post-start check that `GET /v0/role/status` works while nothing besides the supervisor runs |
| L-2 | Low | `EXECUTION_PLAN.md:157`; `launcher/run-step8-roles.sh:91-92` | The guardrail entry in `roles.json` has no `quota_group_id`, so `quota_groups` never flags a guardrail mismatch (`roles.rs:87-93`) | Add `quota_group_id` to the guardrail entry too. Make `mismatches ≠ []` a §6.3 stop |
| L-3 | Low | `launcher/run-step8-roles.sh:65-70` | The read-only target is only mode bits on files the role UID owns. The real CLI was never run with a cwd it cannot write | Optionally, bind-mount the copies `:ro` at `/state/work/ieo-poc-test` and `/state/review/ieo-poc-test`. Note it as untested on the real CLI; a start failure there is a measured result failure (spare path) |
| L-4 | Low | `EXECUTION_PLAN.md:124,139` | Pre-stop checks look only for `ieo-role-call`. Other HTTP clients of 58090 (Notion import, other sessions) would get errors mid-request | Add `ss -tnp state established '( sport = :58090 )'` [read] to §5.1; if it is not empty, wait or report |
| L-5 | Low | `EXECUTION_PLAN.md:255`; `caller.rs:549-557` | G1's no-overage basis is R2's stream, the development credential's. If the two credentials are different accounts, that observation says nothing about the guardrail account | Add to §9 Q2: if the accounts differ, G1 needs that account's own observation, or the user accepts the in-stream stop as the only guard |
| L-6 | Low | `EXECUTION_PLAN.md:46` | The G1 prompt pastes the developer output verbatim with no cap. "Numbered claims" is ambiguous (per line or per result) | Number per output line; cap at about 2 KB; stop if it is exceeded or empty |
| L-7 | Low | `sprint-POC-S8-01-r1.json:17-31,44-73` | `schedule.unresolved_reason`, `delegation.approval_scope_ref` and `sources` are copied from POC-01. They do not cite the step-8 decisions (budget 2026-11-15, L01 values 2026-11-17) that justify the limits | Add the two decisions to `sources` (the digest changes; rerun `ieo-spec check` and update §1.2), or record why not |
| I-1 | Info | `EXECUTION_PLAN.md:196-199` | Re-sending a delivery with the same key and body after a timeout is allowed and returns the stored answer. The text could be read as conflicting with "no automatic retry" | One sentence: "a same-key resend reads the stored answer and starts nothing; it is not a retry" |
| I-2 | Info | `EXECUTION_PLAN.md:224` | A coordinator crash mid-run leaves `inflight.json`, and the run is unmeasured until a person reconciles it | State that in §7 (it is already a stop) |

## Summary

**Must fix before approval (Medium):**
- M-1: enforce the development tool and read restriction in the role settings, and check `init.tools` and `tool_uses` after each run.
- M-2: remove `dropdb` from the automatic rollback, and rehearse the backup restore plus the M migration on the 55432 test container first.
- M-3: capture the old process's argv and env (unprinted) before stopping it, and write out the exact stop, start and rollback commands.
- M-4: commit a real-run driver that implements §6.2/§6.3 and the G1 prompt assembly, and dry-run that same driver.

**Carry-over or optional (Low/Info):**
- L-1: document the host-loopback exposure in §8.
- L-2: add `quota_group_id` to the guardrail entry and stop on mismatches.
- L-3: `:ro` mounts for the target copies.
- L-4: check for other 58090 clients before the stop.
- L-5: the observation-account caveat in Q2.
- L-6: G1 prompt cap and numbering.
- L-7: sprint sources.
- I-1, I-2: wording.
- Code carry-over: a supervisor-level `--tools`/`--disallowedTools` list for development (`args.rs:73`, "set with 03-02").

Verified correct: all ids, limits, the 02-05 budget and shares, the spare exclusion, the endpoints, scopes and bodies, the target repo facts, the observation deadline arithmetic, migration atomicity and 778ee47 compatibility, no secrets in the PR, the dry run reproducing 21/21, the ledger snapshot against GitHub, and the validator and render check.

## User decisions and approvals (final list)

1. **The bundle (§8),** once corrected per M-1 to M-4: pre-checks, backup and rehearsal on 55432, stopping 778ee47 (including reading its env without printing it), redeploying M with migration, the files, the two role containers (including the `allow_host_loopback` exposure), registration, assignments, `branch_head`, and R1→R2→G1. Automatic rollback = restart 778ee47 on the DB as it is. **No** DB restore inside the bundle.
2. **§9 Q1:** confirm `repo_id` `ieo-poc-test`, sprint `POC-S8-01` r1, task `90-01`.
3. **§9 Q2:** `quota_group_id`, plus whether the two credentials are the same account (if not, what G1's no-overage basis is, L-5).
4. **§9 Q3:** the observation deadline. R1 must be admitted before **2026-11-21T23:14:03Z** (KST 2026-11-22 08:14). If it is missed, decide how to get a new no-overage observation.
5. **The user's own tasks:** create the token and give its digest before the redeploy; do the semantic review after registration.
6. **Outside the bundle:** each spare, per case, with the user's credential; any DB restore (into a new DB name); any coordinator restart during the runs (RUNBOOK §8); any retry; GitHub writes; other budgets; extra charge (USD 0).
