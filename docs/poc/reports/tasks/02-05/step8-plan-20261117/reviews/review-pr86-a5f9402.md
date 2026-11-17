# Review of PR #86 at a5f9402 (review 2)

PR: HeaInSeo/ieo_poc #86. Head `a5f9402778f26018790b90b848335af9f4c1893e` (previous review: 3e4e3d6, review 1, changes requested with M-1 to M-4). The corrections are `750e904` (supervisor opt-in read-only development role), `338e5ac` (launcher, scripts, plan v3), and `1c001cb`/`a5f9402` (ledger and archived review). This is an independent, adversarial review.

## Verdict

**Changes requested: 1 Medium (N-1), no High.**

All of review 1's findings are fixed or properly carried. The supervisor change is correct, opt-in and tested. The full workspace tests pass (412/0) against the 55432 test DB. The rerun dry run at this exact head passes 21/21 and drives the real scripts end to end, including the 778ee47 host process, capture, stop, start, runs and rollback.

The one remaining Medium is in `coord-switch.sh start`, the step that both the redeploy **and the automatic rollback** depend on. It rebuilds the captured environment by splitting `/proc/<pid>/environ` on newlines. It then passes every `NAME=VALUE` as an argv entry to `env -i`. That causes three problems:

- **(a) Demonstrated failure.** An exported shell function has a multi-line value. On this host, Rocky's `/etc/profile.d/which2.sh` exports `BASH_FUNC_which%%` into every login or interactive bash. That value splits into stray lines, which `env` then tries to execute as the command (exit 127, reproduced below). The new build would not start, and the rollback 778ee47 would fail the same way. The live coordinator would be left down.
- **(b) DB password in argv.** The password in `IEO_DB_URL` goes into the argv of the `env` process. `/proc` is mounted without `hidepid`, and this host has other accounts. The plan states that values never appear on a command line.
- **(c) No working directory captured.** The process's working directory is not captured, so relative paths in the old argv would not resolve on restart.

The fix is a few lines in one script (below) plus a rerun of the dry run.

## What I ran

- Fresh detached worktree at `a5f9402` (since removed). `git diff --stat 3e4e3d6 HEAD`: 24 files. Code is only `crates/ieo-supervisor/src/{args.rs,serve.rs}` and `tests/supervisor.rs`.
- **Full workspace**, with my own build in the shared target dir (`CARGO_PROFILE_DEV_DEBUG=0`):
  - `cargo fmt --all -- --check`: ok.
  - `cargo clippy --all-targets --locked -- -D warnings`: clean.
  - `cargo test -j 4 --workspace --locked` with `RUST_TEST_THREADS=4` and `IEO_TEST_DB_URL` = 55432 (password from the file, not printed): **412 passed, 0 failed**, 35 test binaries, no `SKIP` (the DB tests ran). Both new tests ran: `a_read_only_development_role_is_chosen_in_the_config` and `role_arguments_use_edit_paths_and_session_id_only`.
  - GitHub checks on PR #86 at `a5f9402`: all four pass.
- **Dry run rerun at this head** with the committed `dry-run-step8.sh`:
  - New binaries from my build (supervisor `26447e04fa0c`, coordinator `371a8b676400`). These hashes differ from the log because the embedded build paths differ.
  - The author's 778ee47 build at `target-778/debug/ieo-coord`, sha `bbe448879962`, the same as the log. I did not rebuild 778ee47; its provenance rests on that hash.
  - Test PG 55432 and ports 58290/58291/58221/58222.
  - **Result: 21 passed, 0 failed.** The old host process went through fixture → backup → rehearsal PASS (15 tables, 11 new, `branch_head`, serve and SIGTERM 0) → capture, stop, start of M with the new tokens and `--roles` → register → user review → prepare → R1/R2/G1 with every check → the two negative tests (overage, `Bash`) → stop of M (graceful) → rollback start of 778ee47 on the migrated DB, which read the step-8 sprint (200) → stop.
  - From those volumes: the development argv now has `--disable-slash-commands --tools Read,Grep,Glob --allowedTools Read(//state/work/ieo-poc-test/**) Grep(…) Glob(…) --disallowedTools Read(//run/ieo/**) Read(//etc/ieo/**) Read(//state/claude/**) Read(//state/home/**) Read(//proc/**) Bash`. The settings have `__REPO__` substituted, `supervisor.json` has `"tools":"read_only"`, and the captured `argv`/`env` are 0600 in a 0700 directory.
  - Cleanup verified: no `ieo_t_s8*` databases, no `s8dry` containers, no listeners on those ports.
- **Reproduction of N-1(a), no live access.** I started `/usr/bin/tail -F /dev/null 127.0.0.1:58293` from a script that does `export -f` on a two-line `which` function (the shape of Rocky's `which2.sh`). `coord-switch.sh capture` wrote `env` with `BASH_FUNC_which%%` and **2 lines that are not `NAME=VALUE`**. It then stopped at the test-only "no IEO_DB_URL" check, as expected. Feeding that file through the script's own start line (`mapfile -t envs; env -i "${envs[@]}" /usr/bin/true`) gave `env: ' eval ${which_declare} ) | /usr/bin/which …': No such file or directory`, **exit 127**. On this host, `bash -lc` and `bash -ic` both export `BASH_FUNC_which%%`. The agent Bash tool's environment does not (0). So the outcome depends on how the live 778ee47 process was launched on 2026-11-14, which nobody has checked. Checks: `/proc` mounted without `hidepid`; 3 accounts with uid ≥ 1000.
- `ieo-spec check` of the sprint: 201 `ACCEPTED_FOR_SEMANTIC_REVIEW`, digest `a8d97f12…7bb7` (= the plan). The two new `sources` digests equal `sha256sum` of `User_Decisions_2026-11-15_0205-x08.md` (`b41d380f…`) and `User_Decisions_2026-11-17_step8-l01.md` (`d386e3f4…`).
- `docs/poc`: `validate_package.py` exit 0 (`schema_errors` 0, generated views match, 687 links); `render_progress.py --check` ok. Secret grep of `3e4e3d6..a5f9402`: only the fake `sk-ant-oat01-fake-…`, test-DB `password=$(cat file)` assignments (environment variables, not argv), and documentation text.
- Not done: anything on 58090/55433, real `claude`, secrets or run records.

## Review 1 findings

| ID | Status | Evidence |
| --- | --- | --- |
| M-1 development tools | **Fixed** (residual L-8) | `750e904`: `"tools": "read_only"` → `--tools Read,Grep,Glob` (CLI 2.1.322 honoured `--tools` for the Guardrail in 02-04 G1: `init.tools` was exactly the list), scoped allow, denies, slash commands off. The settings deny list is a second layer. `check-run.py` checks `init.tools` ⊆ the allowed set and `tool_use` ⊆ {Read, Grep, Glob} (negative test passes). The plan states honestly that the real CLI's development `--tools` is first observed at R1 |
| M-2 rollback `dropdb` | **Fixed** | §5.4: rollback = 778ee47 on the DB as it is, no restore. Restore is a separate decision, into a new DB name (§3). `rehearse-migration.sh` does restore, count, migrate, recount, new tables, `branch_head`, serve, SIGTERM. The dry run showed 778ee47 serving the migrated DB |
| M-3 stop/start | **Partly fixed → N-1** | `coord-switch.sh` checks pid → exe, `127.0.0.1:<port>` in the cmdline, parent, clients, 0600 capture, `setsid nohup`, pid file, log, port-in-use refusal, health wait. The environment and working-directory reconstruction is not faithful (N-1) |
| M-4 run driver | **Fixed** | `run-step8.sh` (register/prepare/runs) + `lib-step8.sh` + `check-run.py`. Hard `STOP` (exit 3) on every §6.2/§6.3 condition; deadline check with 10 min margin; G1 prompt numbered per line, capped at 2,048 B; resend only on `000` with the same key and the same body file. The dry run drives these exact scripts |
| L-1 loopback | Fixed (documented in §4/§5.3/§8 and the launcher header) | |
| L-2 quota on Guardrail | Fixed (both entries; mismatch is a stop in `prepare` and after each run) | |
| L-3 `:ro` | Fixed (target outside the volumes, `-v …:ro`); real CLI with a read-only cwd still unobserved, stated in §4 | |
| L-4 other clients | Fixed (§5.1 item 3, count printed at capture) | |
| L-5 account caveat | Fixed (§9 Q2) | |
| L-6 G1 cap/numbering | Fixed | |
| L-7 sprint sources | Fixed (digest checked above) | |
| I-1, I-2 | Fixed (§6.1 sentence, §7 row) | |

## Analysis

### Supervisor code change (`750e904`)

- **Opt-in.** `serve.rs:80-90`: `tools` absent or `null` → `read_only: false`; `"read_only"` → true; anything else → error. Guardrail with any non-null `tools` → error. For a plain development role, the test asserts that none of `--tools`, `--allowedTools` or `--disallowedTools` appears (`supervisor.rs`), so 02-04-style roles are unchanged. All other `Role::Development { .. }` uses are wildcard matches (`args.rs:74,133`, `serve.rs:248`), so nothing else changes.
- **argv.** It follows the Guardrail pattern proven on CLI 2.1.322: variadic `--allowedTools`/`--disallowedTools` each followed by their rules, with no positional prompt to swallow because the prompt comes over stream-json stdin. `Read(//abs/**)` is the proven form. **`Grep(//…/**)` and `Glob(//…/**)` have never been given to the real CLI before**, in argv or in settings (L-8).
- **Read scoping.** The allow list is scoped to `/state/work/<repo>/**`, which is exactly the `:ro` mount. The denies cover `/run/ieo` (credentials), `/etc/ieo`, `/state/claude`, `/state/home` and `/proc`. In `dontAsk` anything not allowed is refused, and deny wins.
- **Mutation evidence.** 4 of 4 mutants killed (`mutants-readonly.txt`).

### `coord-switch.sh`

- **capture**
  - The pid check compares `readlink /proc/pid/exe` with `realpath bin`. A binary replaced on disk shows `(deleted)` and fails, which is conservative.
  - The listen check `grep -qx 127.0.0.1:<port>` requires the exact `--listen` value.
  - Parent and client counts are only printed; the plan leaves the judgment to the operator (§5.2 step 2, §5.4). That is acceptable, but see L-10 for systemd units.
  - Capture uses `umask 077`. No values are printed, only counts and names.
  - **But:** `tr '\0' '\n'` loses the boundaries between entries, and there is no `/proc/<pid>/cwd` (N-1).
- **stop.** SIGTERM, wait, SIGKILL after the timeout; it refuses a missing pid. Correct for 778ee47 (no handler). For M in the rollback no run exists, so 30 s is enough (M stops gracefully, as the dry run shows).
- **start**
  - Refuses a port in use. `setsid` does not fork here: no job control, so the background child is not a group leader, and `$!` is the coordinator (shown by the dry run's pid and health).
  - Health wait is 30 s. Migration timing on 680 MB is measured by the rehearsal first; the only table scan is the `evidence` CHECK re-add.
  - **`env -i "${envs[@]}"`**: N-1 (a) and (b).
- **Rollback on a migrated DB.** Shown in the dry run (778ee47 reads the step-8 sprint). This matches my review-1 analysis: atomic migration, additive schema, no shared `CREATE OR REPLACE` bodies changed.

### `run-step8.sh`, `lib-step8.sh`, `check-run.py` against RUNBOOK §7/§9 and the decisions

| RUNBOOK condition | Where enforced |
| --- | --- |
| unmeasured (never 0) | `check-run.py:84-85,120-122` |
| caps, shares, assignment limits | `:127-132` (the code is also authoritative) |
| limit signals (allowed/rejected/false, ≥1 event) | `:102-104` |
| `stop_required` | `:82-83` |
| unexpected judgment | `run-step8.sh:70-71` (exact tuple) |
| `pending_external_requests` | `:134-135` |
| `inflight.json` | `settled` + `:145-146` |
| first action not linked | `:141-144` |
| quota mismatch | `prepare` + `:138-140` |
| tools | `:105-110` |
| deadline | `runs:60` |
| G1 never retried | single `run_one`; a STOP ends the script |
| no automatic retry | `req` only resends on `000`, same key and bytes; a POST without a key is not resent |

The spare stays outside the script, with the user's credential. Re-running `runs` after a STOP resends the same keys and bodies (stored answers), and the checks re-fail deterministically on the same records, so a re-run cannot get past a failed check. Re-running after R2 is not idempotent for R1's `role_wait`, which looks at `last.attempt_id` (I-3).

### Dry run fidelity

The same scripts run in the same order: old host process → backup and rehearsal → capture, stop, start → roles → register → review → prepare → runs → negative checks → rollback. The fake claude now echoes the `--tools` it received, so the dry run proves the supervisor passes the option, not what the real CLI does (stated in §4). The differences that matter:

- The old process's environment came from the agent shell (no exported functions); that is how N-1(a) went unseen.
- Its argv used absolute paths (N-1(c) unexercised).
- `rehearse-migration.sh` ran without `--bindings` and without the merged `tokens.json` (L-9).
- `evidence/dry-run.log` was produced at `750e904` with the scripts not yet committed (header "branch 750e904"). My rerun at `a5f9402` gives the same 21/21 (I-4).

### §8 bundle

It is complete and explicit. Items 3 and 4 name the env capture (with the password, unprinted), `setsid nohup`, the log, `coord.pid`, the migration and the automatic rollback without restore. Item 6 names the loopback exposure and the `git clone` read. The DB restore and the other exclusions are listed. §9 Q1 to Q3 are correct and minimal. One sequencing point: §5.2 lists step 4 (`roles.json`) and step 5 (start with `--roles`) before step 6 (role containers, which produce `roles-entry.json`). §5.3 says to do step 6 first, and the dry run does so. The table should simply be in that order (L-11).

## Findings

| ID | Severity | Where | Scenario | Fix |
| --- | --- | --- | --- | --- |
| **N-1** | **Medium** | `launcher/coord-switch.sh:28,45,54`; `EXECUTION_PLAN.md:175,178,206,276` | (a) `/proc/<pid>/environ` is split on newlines. Any multi-line value, such as the exported function `BASH_FUNC_which%%` that Rocky's `/etc/profile.d/which2.sh` puts in every login or interactive bash on this host, becomes stray lines that `env -i` runs as the command. **Reproduced: exit 127.** M's start fails, and **the automatic rollback start of 778ee47 fails the same way**, leaving 58090 down until someone repairs it by hand. (b) The DB password goes into the argv of the `env` process, readable from `/proc/<pid>/cmdline` by other local accounts (`/proc` without `hidepid`) until `env` execs. This contradicts §2.1/§8 "values never on a command line". (c) The working directory is not captured: relative paths in the old argv (`--tokens`, `--manifest`, `--profiles`, `--bindings`, `--policy` are not known to be absolute) would not resolve on restart, with the same effect as (a) | Capture keeps NUL boundaries (`cp /proc/$pid/environ "$out/env0"`; `cp /proc/$pid/cmdline "$out/argv0"`; `readlink /proc/$pid/cwd > "$out/cwd"`, all 0600). It refuses or records any `BASH_FUNC_*` entry and prints names only. Start runs in a subshell that clears the environment (`unset $(compgen -e)`), reads `env0` with `while IFS= read -r -d '' kv; do case $kv in BASH_FUNC_*) ;; *) export "$kv";; esac; done` (a builtin, so no argv), then `mapfile -d '' argv < argv0`, `cd "$(cat cwd)"`, and `exec setsid nohup "$bin" "${args[@]}"`. In capture, check that every path-valued argument exists from that cwd. Add a dry-run case whose old process carries an exported two-line function and a relative `--tokens`, and rerun |
| L-8 | Low | `crates/ieo-supervisor/src/args.rs` (read-only allow), `launcher/role/development/settings.json:4` | The `Grep(//…/**)`/`Glob(//…/**)` rule forms have never been given to CLI 2.1.322. Most likely they are accepted or ignored, but if the CLI rejected them at start, R1 would end without a session and use a development run (the spare path) | Optionally, use plain `Grep`, `Glob` (02-04 used these forms in settings) plus the scoped `Read(//…)`, which the CLI applies to Grep/Glob as well, and keep the denies. Otherwise list it in §4 as unobserved next to `--tools` |
| L-9 | Low | `EXECUTION_PLAN.md:112`; `launcher/rehearse-migration.sh:34-36` | The rehearsal serves with `{"principals": []}` and no `--bindings` unless `SERVE_ARGS` is given, so `check_bindings` on the real bindings file and the merged `tokens.json` are first exercised at the live start (step 5). With N-1 fixed the rollback covers that failure, but it is avoidable downtime. Also, comparing the restored counts with `live-counts.txt` is manual | Make step 1b pass `SERVE_ARGS="--bindings <the captured --bindings path> --tokens <new tokens.json>"` (prepare `tokens.json` before 1b), and have the script compare with an optional `live-counts.txt` argument |
| L-10 | Low | `launcher/coord-switch.sh:24-26`; `EXECUTION_PLAN.md:175` | "Parent is not a restarter" is judged from `ppid` and `comm`. The dry run printed `parent 1 (systemd)`, which is also what a setsid'd process shows, while a systemd **user unit** would show a `systemd` parent too | Also print `/proc/$pid/cgroup` (a `*.service` other than the session scope means a unit) and say in §5.2 that this is a stop |
| L-11 | Low | `EXECUTION_PLAN.md:177-179,196` | The table order 4 (`roles.json`) → 5 (start with `--roles`) → 6 (role containers that write `roles-entry.json`) cannot be run as written. §5.3 says to do 6 first | Reorder the table: 6 before 4 |
| I-3 | Info | `launcher/lib-step8.sh:23-25`; `run-step8.sh:61-80` | Re-running `runs` after R2 has run: R1's `role_wait` waits for `last.attempt_id == s8-r1` and times out after 30 min (a STOP, harmless) | Note it, or skip the wait when the ledger already holds the attempt |
| I-4 | Info | `evidence/dry-run.log:2` | The log header says `branch 750e904` (scripts not yet committed then). My rerun at `a5f9402`: 21/21 | Regenerate the log at the final head after N-1 |

## Summary

**Must fix before approval (Medium):**
- **N-1:** `coord-switch.sh` must rebuild the captured environment faithfully. Keep NUL boundaries and drop `BASH_FUNC_*`. Export with builtins, so no value appears in argv. Capture and restore the working directory, and check path arguments. Add a dry-run case for an exported function and a relative path, then rerun.

**Carry-over or optional (Low/Info):**
- L-8: use the proven rule forms for Grep/Glob, or note that they are unobserved.
- L-9: rehearse with the real `--bindings` and the merged tokens; compare the restored counts automatically.
- L-10: print the cgroup when checking for a restarter.
- L-11: reorder the §5.2 table.
- I-3: `runs` re-run wait.
- I-4: regenerate the log.

**Verified:**
- Review 1's M-1, M-2 and M-4 are fixed.
- The supervisor change is opt-in and argv-correct, refuses `tools` on the Guardrail, and changes nothing without the option.
- Full workspace: fmt, clippy and 412 tests clean.
- The dry run reproduces 21/21 at this head with the real scripts, including the rollback on a migrated DB.
- The stop conditions match RUNBOOK §7/§9 and the decisions.
- The sprint digest and its sources are correct.
- The ledger, validator and render check pass, and GitHub checks are green.
- No secrets in the diff.

## User decisions and approvals (current)

1. **The bundle (§8 v3)**, after N-1 is fixed, covering:
   - pre-checks;
   - backup and the rehearsal on 55432;
   - capture of the 778ee47 process's arguments, environment (DB password included, never printed or put in argv) and cwd;
   - the stop;
   - redeploy of M with migration, with automatic rollback to 778ee47 on the DB as it is (no restore);
   - token and config files;
   - the two role containers (read-only development tools, `:ro` target, `allow_host_loopback` exposure);
   - register, assign, `branch_head`;
   - R1→R2→G1 through `run-step8.sh` with automatic stops.
2. **§9 Q1:** `repo_id` `ieo-poc-test`, sprint `POC-S8-01` r1, task `90-01`.
3. **§9 Q2:** the `quota_group_id` value and whether both credentials are on the same account. If not, decide G1's no-overage basis.
4. **§9 Q3:** the observation deadline. R1 must be admitted before **2026-11-21T23:14:03Z** (KST 2026-11-22 08:14); the driver stops 10 min before. If it is missed, decide how to get a new no-overage observation.
5. **The user's own tasks:** create the token and give its digest before the redeploy; do the semantic review after `register`.
6. **Outside the bundle (each needs its own approval):** each spare, per case, with the user's credential; any DB restore (into a new DB name); a coordinator restart during the runs (RUNBOOK §8, `stop … 900`); any retry; GitHub writes; other budgets; extra charge (USD 0).
