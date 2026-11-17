# Review of PR #87 at 0ef9d60 (review 1)

PR files: `docs/poc/reports/tasks/02-05/step8-plan-20261117/MANUAL_STEPS.md` (214 lines) and `.../launcher/obs-check.sh` (128 lines). Base main 46799b0.

## Verdict

**Changes requested.** There are 3 Medium findings (M-1 wildcard allow rules, M-2 `export` subcommands not covered by any rule, M-3 A8 `prepare` without `EXPECTED_CURRENT_DEV/GR`) and 7 Low. The copies, hashes, obs-check logic, A6 order, rollback, deadline and stop conditions are correct. All fixes are text-only (doc and rule list, plus two small obs-check edits).

## What I checked

- Worktree at exact head 0ef9d60 (detached). `git diff 46799b0 HEAD` touches only the two new files. No merged launcher file or EXECUTION_PLAN.md changed.
- Copies: `$D/step8-46799b0` (16 files) all have SHA-256 equal to the 46799b0 blobs; I compared each one with `git show 46799b0:<path> | sha256sum`. `$D/step8-ops/obs-check.sh` = PR blob `f5086109…3edc`. `sha256sum step8-ops/SHA256SUMS` = `0445557d…a640` (matches the doc). `sha256sum --strict -c` passes for all 17 entries. `find … -perm /222` = 0, so both trees are read-only (dirs `dr-xr-xr-x`, files `r--`/`r-x`). I read nothing in `ieo_poc-data/private`.
- obs-check.sh: `bash -n` and shellcheck (only SC2174 info on `mkdir -p -m`). I ran a copy with D/RUNS rewritten to a scratchpad fake tree (`.../scratchpad/r87b-fake`), with no live paths: [1] PASS and install, [1] rerun idempotent, [2] with a `caller.lock` nobody holds, [3] skipped, then key=value form and URL form, [4] skipped, then install, then rerun "already installed, identical", then two workdirs → FAIL. Exit codes were 0 and 1 as documented.
- Every command in MANUAL_STEPS against coord-switch.sh, run-step8-roles.sh, run-step8.sh, lib-step8.sh, rehearse-migration.sh, dry-run-step8.sh and EXECUTION_PLAN §2, §3, §5.2, §5.3.
- Deadline: 1794698043 + 604800 = 1795302843 = 2026-11-21T23:14:03Z. The doc is correct, and run-step8.sh `runs` enforces a 600 s margin.
- No live command run. I did not touch 55433 or 58090, read no secrets, runs or private paths, made no model call and did no GitHub write.

## Per-step table

| Step | Commands vs scripts | Result |
| --- | --- | --- |
| Rules (L13-40) | prefix `*` rules on capture/stop/start/run-step8-roles; exports not covered | M-1, M-2, L-6 |
| Copy check (L42-54) | hashes right; does not gate (see L-1) | L-1 |
| U1 (L74-84) | `set -C` + umask 077 in subshell, no overwrite, digest = `printf '%s' "$(cat)"` (same as EXECUTION_PLAN §2.1 and dry run); agent never reads it; `Read` deny on `private/user/**` | OK |
| U2 (L86-89) | gate before A4; "different" → stop | OK |
| A1 (L91-102) | exact rule, no args, fixed paths; expected output matches the script | OK |
| A2 (L104-120) | capture argv order `<pid file> <bin> <port> <out>` correct; env0 never opened by the agent; DB user/dbname via obs-check [3] | OK (L-2 on [3]) |
| A3 (L122-131) | prose only; tokens-s8.json/roles content under-specified; manifest equality check from EXECUTION_PLAN L35 missing; `TEST_DB_PASSWORD_FILE` path not given | L-5 |
| A4 (L133-145) | args `<bin dir> <workdir>` and the three required env vars correct; `<YYYYMMDD>` makes the rule a wildcard | M-1, M-2, L-7 |
| A5 (L147-151) | matches §5.2 step 6 and §5.3; pre-stop recheck good | L-5 (roles.json shape) |
| A6 (L153-171) | `stop <pf> 30 && start <bin> <out> <pf> <log> --tokens F -- --roles R` matches the `start` parser (`--tokens` must come directly after the 5 positional args, `--` then extras). The permission check covers the whole compound before it runs, so "stop runs, start blocked" cannot happen inside one call. Rollback `stop …; start old …` is correct: the pid file holds M's pid, a dead pid gives exit 3 and the chain continues; old argv and tokens are unchanged; DB untouched | OK (L-4) |
| A7 (L173-178) | env and `register` correct; SPEC resolves to the copy's `../sprint-POC-S8-01-r1.json`; digest matches | OK (M-2 prompts) |
| U3 (L180-195) | token read from file, never printed; `<(…)` at top level (the 401 issue was only inside a function); 201 expected | OK |
| A8 (L197-202) | `prepare` without `EXPECTED_CURRENT_DEV/GR`; `runs` with OBS_UNIX, in background | M-3 |
| Failures (L204-214) | no spare, DB restore, restart or retry; stop conditions intact | OK |

## Findings

| ID | Sev | Line | Scenario | Exact fix |
| --- | --- | --- | --- | --- |
| M-1 | Medium | 20-23, 135-137 | The prefix rules auto-approve far more than the five planned commands. (a) `coord-switch.sh start *`: any `<binary>` runs, with argv taken from any `<out>/argv0` (agent-writable dir) and the env from that `env0`. That is unprompted arbitrary code execution with the live env, including the DB password. (b) `stop *`: SIGTERM/SIGKILL of the pid in any pid file, for example a podman/conmon pid or the DB. (c) `capture *`: copies `/proc/<pid>/environ` of any own process into any out dir, where the agent's Read is not denied. (d) `run-step8-roles.sh *`: any `<bin dir>` (an arbitrary `ieo-supervisor` given the OAuth credential) and any workdir outside `/opt/rust/src` (`realpath -m` also folds `02-05-s8-x/../../../tmp/y`), so OAuth credentials are copied anywhere. A prompt injection from anything the agent reads (ledger, model output) could use any of these. | Use **exact** rules (no `*`) for each of the six command lines in the doc: capture, stop (A6 and rollback use the same text), start-M, start-rollback, roles, obs-check. Fix the workdir name now: either the user writes the date when adding the rule, or allow the five dates 20261117…20261121 as five exact lines. Replace `<YYYYMMDD>` in L137/145 with that fixed value. See the JSON below. |
| M-2 | Medium | 35, 137, 176, 199-201 | Claude Code checks every subcommand. `export QUOTA_GROUP_ID=…`, `export SUP_DEV_TOKEN_FILE=…` and `export SUP_GR_TOKEN_FILE=…` match no rule, and `export` is not a documented read-only auto-allowed command. So the A4 call is **not** auto-approved and falls to the prompt or classifier: exactly the credential-copy step the narrow rule exists for. The "blocked → give to user" fallback (L38) then becomes the normal path. A7 and A8 are not covered either: run-step8.sh has no rule, and they need four or five exports. | Add exact allow rules for the three A4 exports (values literal). For A7/A8, either accept prompts there (they are not secret-touching; say so in the doc) or add exact rules for `export TOKDIR=…`, `export OUT=…`, `export LEDGER=…`, `export ADAPTER_HEADER=…`, `export OBS_UNIX=1794698043` and `bash $D-abs/step8-46799b0/launcher/run-step8.sh register` / `prepare` / `runs` (exact). State in the doc which calls are expected to prompt. |
| M-3 | Medium | 200 | EXECUTION_PLAN §5.2 step 11 requires reading `assignment_heads` at run time and exporting `EXPECTED_CURRENT_DEV/GR`. A8.1 only says "(A3의 expected_current)" and exports four variables, so run-step8.sh L45-46 sends `expected_current: null`. If a head exists for `ieo-poc-test`/development or guardrail, the assignment gets a conflict and stops after registration and review. Worse, development may succeed and guardrail stop, leaving a half-prepared state with no automatic undo (L213). A3's value can also be stale by A8. | In A8.1: immediately before `prepare`, re-run the A3 read-only `assignment_heads` query, then in the same call `export EXPECTED_CURRENT_DEV=<value or null>; export EXPECTED_CURRENT_GR=<value or null>;` (literal values, and add them to the rules if M-2 is fixed by rules). Expected output: `prepared: S8-DEV-A1, S8-GR-A1, branch_head 2ab9d876…`. |
| L-1 | Low | 46-48 | The copy check does not gate the sensitive command in the same call. `find … -perm /222 \| wc -l` always exits 0, so a writable item does not stop the `&&` chain. The self-hash `0445557…` is only printed, not compared, so a consistently replaced SHA256SUMS plus scripts would pass `-c`, and the sensitive command runs before the agent reads the output. | `cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data && echo '0445557d19b83a621b52a2129b727d5566c5943401bb12b7b582e18f3549a640  step8-ops/SHA256SUMS' \| sha256sum --quiet --strict -c - && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && ! find step8-46799b0 step8-ops -perm /222 \| grep -q . && echo "copies ok"` |
| L-2 | Low | obs-check 84-98 | [3] prints a Python traceback when IEO_DB_URL is a malformed URL. I reproduced it on python 3.6 with `postgres://ieo:SECRETpa/ss@…`: `ValueError: invalid literal for int() with base 10: 'SECRETpa'`, which **prints part of the password**. That is unlikely with a URL the live coordinator accepts, but the script promises "never the password". | Wrap the parse: `try: … except Exception as e: print("  IEO_DB_URL not parseable:", type(e).__name__); sys.exit(1)`. Print nothing derived from the value on error. |
| L-3 | Low | obs-check 109-124 | [4] installs `adapter.header`/`roles-entry.json` into `$D/private/s8` even when the Python check failed (`\|\| fail` does not skip the `put`s). A later correct rerun then fails with "exists and differs", and a wrong roles-entry may already sit where A5 reads it. | `if python3 -I - "$W" <<'PY' … PY; then mkdir …; put …; put …; echo sha256 …; else fail "…"; fi` (same pattern as [1]). |
| L-4 | Low | 163 | The survival check requires parent = 1. An orphan can be reparented to a subreaper (systemd --user, tmux), which is harmless. Under L165 the agent would then roll back a healthy M, a needless second outage plus an old binary on the migrated DB. | "the parent is not in the Bash tool's process tree (1 or a subreaper is fine)". Roll back only on health ≠ 200, a dead pid, wrong exe, wrong listener or a bind error. |
| L-5 | Low | 125-131, 149 | A3 and A5 are prose, yet they produce the two files A6 depends on. (a) tokens-s8.json must carry `repo_ids` (`["ieo-poc-test"]`; agents exactly one) and the agent `role` (EXECUTION_PLAN §2 table and dry-run `pr()`), and the doc lists only name/kind/scope/digest. (b) roles.json must be `{"ieo-poc-test": {…, "budget_plan": {"budget": "02-05", …, "roles": {…}}}}` (§5.3). (c) The run-time check that 58090's `--manifest`/`--profiles` equal the repo manifest (EXECUTION_PLAN L35) is missing. (d) The `TEST_DB_PASSWORD_FILE` path is not named. | Cite §2/§2.1/§3/§5.3 explicitly. Give the dry-run's Python for roles.json with live paths. Add a read-only check printing the principal names, kinds, scopes, repo_ids and role of tokens-s8.json, and of roles.json without values. Add the manifest/profiles `cmp` (or `sha256sum`) step. Name the test DB password file path. |
| L-6 | Low | 26-29 | The deny rules cover only the secrets dir and the user token. The captured `env0`/`argv0` (DB password), `/proc/*/environ`, `W/adapter.token` and `W/*/cred/**` are equally sensitive. The doc promises the agent never opens them (L120, L145) but nothing enforces it. None of these file paths appear literally in any planned command, so denying them breaks nothing. | Add `Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47/env0)`, `…/argv0`, `Read(//proc/*/environ)`, `Read(//opt/rust/ieo_poc_runs/*/adapter.token)`, `Read(//opt/rust/ieo_poc_runs/*/*/cred/**)`. Do not deny the whole `ieo_poc_runs` or `coord-778ee47` dir, because those dirs are literal arguments of A4/A6. |
| L-7 | Low | 137 | A4 runs a `git clone`, possibly an image pull and two health waits, which can exceed the default 120 s Bash tool timeout. A killed call mid-way leaves volumes, and the script then refuses the same workdir. | Say "run with timeout 600000 ms". |

## Recommended final permission JSON

The workdir date is fixed below as `20261118` as an example; the user writes the actual one, or adds one line per date up to 20261121, and the doc uses the same literal. Exact rules (no `*`):

```json
{
  "permissions": {
    "allow": [
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord 58090 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-M.log --tokens /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json -- --roles /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/roles.json)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-778ee47-rollback.log)",
      "Bash(export QUOTA_GROUP_ID=claude-sub-shared)",
      "Bash(export SUP_DEV_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-dev.token)",
      "Bash(export SUP_GR_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-gr.token)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/m-46799b0 /opt/rust/ieo_poc_runs/02-05-s8-20261118)"
    ],
    "deny": [
      "Read(//opt/rust/ieo_poc_secrets/**)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/**)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47/env0)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47/argv0)",
      "Read(//proc/*/environ)",
      "Read(//opt/rust/ieo_poc_runs/*/adapter.token)",
      "Read(//opt/rust/ieo_poc_runs/*/*/cred/**)"
    ]
  }
}
```

The run-step8.sh calls (A7/A8) are optional additions, as described in M-2. The L40 cleanup should say "remove all `allow` lines added for step 8".

## Summary

The copies are exact and read-only, the hashes in the doc are right, and obs-check.sh does what it claims: idempotent installs, safe on missing or extra workdirs, correct exit codes. Two small defects remain in it: L-2, a traceback that can show part of the password on a malformed URL, and L-3, installs that happen after a failed check. The A6 stop/start/rollback sequence matches coord-switch.sh exactly, and the deadline and stop conditions are intact. Nothing outside the approved bundle appears. The three user items are exact, never overwrite, and keep the user token away from the agent.

The changes needed before approval:
- **M-1:** replace the wildcard allow rules with exact command lines, including a fixed workdir name.
- **M-2:** cover the `export` subcommands with rules, or document that those calls will prompt.
- **M-3:** add the run-time `assignment_heads` read and the `EXPECTED_CURRENT_DEV/GR` exports to A8.1.
