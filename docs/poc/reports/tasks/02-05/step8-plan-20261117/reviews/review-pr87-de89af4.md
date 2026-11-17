# Review of PR #87 at de89af4 (review 2)

Head de89af4 (correction e5e61a7 plus de89af4, which archives review 1). The previous review was at 0ef9d60. Base main 46799b0.

## Verdict

**Approve.** M-1, M-2 and M-3 are fixed. L-1 through L-7 are fixed: L-1 is fixed apart from one untested parser detail, which the doc discloses and which falls back to a read-only prompt. No High or Medium findings remain. There are 3 new Low findings and 2 Info notes below. They are optional, but N-2 and N-3 are cheap and worth doing before the run.

## What I checked

- Fresh detached worktree at de89af4. `git diff 46799b0 HEAD` changes no merged launcher file, EXECUTION_PLAN.md or `crates/`. The changes are MANUAL_STEPS.md, obs-check.sh and the archived review.
- Copies: `sha256sum step8-ops/SHA256SUMS` = `c92f4fec198cb1374b01e493c23b055ef11cfb35124ec0af7d30df005f30223b`, which is the value in the doc's gate line and rule. `sha256sum --strict -c` passes for all 17 entries. Each of the 16 `step8-46799b0/*` entries equals the 46799b0 blob. `step8-ops/obs-check.sh` = PR blob `b357383f…d868`, as in the doc table. There are 18 files, `-perm /222` = 0, and both dirs are read-only. Nothing in `private` was read; for pgpw I ran only `ls -l` (0600, 25 B), not its content.
- Rule matching. A script parsed the JSON allow list: 27 rules, none containing `*`. It expanded `<확인>` in all 9 `<확인> && …` calls and split them on ` && ` and ` | `. Every subcommand equals a rule character for character, and every rule is used. The only exception is `! find …`, which matches only if the harness strips the leading `!` (see N-2). JSON-decoded `echo 'c92f…  step8-ops/SHA256SUMS'` (two spaces) and `echo "copies ok"` equal the command text.
- obs-check.sh ran on a scratchpad copy with D and RUNS rewritten to a fake tree:
  - [3] URL with an unescaped `/` in the password → `IEO_DB_URL not parseable: ValueError` (no fragment shown).
  - [3] out-of-range port → `ValueError`.
  - [3] percent-encoded URL and key=value forms → fields plus `password set: True`.
  - [3] no `IEO_DB_URL` → `present: False` and FAIL. `sys.exit` inside the `try` is not swallowed (SystemExit is not an Exception).
  - [4] bad adapter path or bad header form → FAIL "(nothing installed)", with no file created.
  - [4] missing header → `FileNotFoundError` only, nothing installed.
  - [4] valid → installs, then identical on rerun.
  - [4] non-object roles-entry → traceback of an `IndexError` (non-secret file) and FAIL, nothing installed.
- Coordinator parsing at HEAD (`crates/ieo-coord/src/auth.rs` L84-165):
  - Principals need `name`, a 64 lowercase-hex `token_sha256`, a `kind` and `scopes` from the client or agent sets. Agents need exactly one repo and a role of development or guardrail. Names and digests must be unique.
  - `token_digest` = hex(sha256(token bytes)). The tokens-s8.json Python satisfies all of this: `repo_ids ["ieo-poc-test"]`, `role` only on the two agents, `checkpoint:report` in AGENT_SCOPES, client scopes all in CLIENT_SCOPES. Its digest of the raw file bytes equals the header token, because the token is written without a newline.
  - Roles (`roles.rs` L19-22, `delivery.rs`/`limits.rs`, `ieo-supervisor/caller.rs` L136-160): the A5 shape `{"ieo-poc-test": {development, guardrail, budget_plan{budget, ledger_dir, first_observation{stream, observed_unix}, roles.*.max_window_seconds}}}` equals EXECUTION_PLAN §5.3 and the dry run that passed.
- `assignment_heads`, `sprint_revisions` and `notion_imports` exist in the 778ee47 schema, so the A3.5 queries are valid before migration. The 11 new tables listed in A3.5 equal rehearse-migration.sh L35.
- Manifest SHA-256 at 46799b0 = `ae5c1e7f…4e85`, as in A3.4. `inspection_profiles` r7 `items` is a dict whose keys include `02-05.accept`, so the A3.4 `in` test is correct.
- Rehearsal env (A3.7): the names `TEST_DB_PASSWORD_FILE`, `TOKENS_FILE`, `BINDINGS_FILE`, `MANIFEST` and `POLICY` equal the ones rehearse-migration.sh reads.
- run-step8.sh `runs` (L61-90): each delivery has a fixed key and a body kept in OUT, so a re-invocation reads stored answers and starts no second run of R1, R2 or G1 once they were accepted.
- No live command, no 55433/58090 access, no secrets/runs/private read, no model call, no GitHub write.

## Previous findings

| ID | Status | Evidence |
| --- | --- | --- |
| M-1 wildcard rules | **Fixed** | 27 exact rules (L21-47). Capture, stop, start-M, start-rollback and run-step8-roles are fixed full lines. The workdir is fixed to `02-05-s8-20261118` (L9, L36, L219), and the doc says to change both if the date moves. |
| M-2 exports uncovered | **Fixed** | Each export used in A4, A7 and A8 has an exact literal rule (L33-44). The `QUOTA_GROUP_ID` line is added only on a "same" answer to U2 (L62). The calls that still prompt are listed (L70). |
| M-3 EXPECTED_CURRENT | **Fixed** | A8.1 re-reads `assignment_heads` right before `prepare`, and any row → stop (L303-307, failure table L329). `prepare` exports `EXPECTED_CURRENT_DEV/GR=null` literally (L310). This matches the plan (`null` when there are no heads, EXECUTION_PLAN L26) and the dry run. |
| L-1 gate | **Fixed** (see N-2) | The self-hash is compared via `sha256sum -c -`, and a writable item fails the chain (`! find … \| grep -q .`). Every rule call starts with the gate and `&&`, and `;` is forbidden (L68, L78-80). |
| L-2 [3] traceback | **Fixed** | Verified above. |
| L-3 [4] install on failure | **Fixed** | Verified above. |
| L-4 parent = 1 | **Fixed** | L260-262: a non-1 parent or subreaper is fine; the explicit rollback triggers are listed. |
| L-5 A3/A5 prose | **Fixed** | Token generation (L161), tokens-s8.json with O_EXCL, clash stop, repo_ids and role (L166-188) plus a digest-free summary, manifest/profiles comparison (L193-204), concrete A3.5 queries, the pgpw path (L207-209), roles.json with O_EXCL (L234-245) plus a summary. |
| L-6 deny rules | **Fixed** | env0, argv0, `/proc/*/environ`, `W/adapter.token` and `W/*/cred/**` are denied (L52-56). No planned command names these files. |
| L-7 timeout | **Fixed** | A4 uses a 600000 ms timeout (L217). |

## Per-step table

| Step | Result |
| --- | --- |
| Rules / gate (L14-89) | Exact rules and a correct gate. `!` is untested (N-2). |
| U1, U2, U3 | Unchanged from review 1 and correct. `set -C` means no overwrite. The agent never reads the user token (deny rule). U3 reads the token only inside the user's shell. |
| A1, A2 | Match the rules. A2 `mkdir` is outside the rules (prompts, harmless). |
| A3 | Correct against auth.rs and the plan. O_EXCL makes a rerun fail closed. The `ieo-spec` build fallback is local and non-live (Info). |
| A4 | Matches the rule. On "different" the rule is never added, so A4 cannot run silently. |
| A5 | Correct against §5.3 and roles parsing. Pre-stop recheck is complete. |
| A6 | One call (gate, stop, `&&`, start). Permission is checked before anything runs, so there is no half-executed state. Rollback is two gated calls: an exit 3 "not running" on the first is expected. Survival checks and rollback triggers are right. |
| A7, A8 | Exact rules. Heads stop. Background `runs`. Expected lines equal the script's echo strings (`roles ready: …`, `prepared: …`, `s8-r1: (…)`, `R1, R2 and G1 recorded and checked`). |
| Deadline / bundle | 2026-11-21T23:14Z, with a 600 s margin in the script. No spare, DB restore, restart or retry. |

## New findings

| ID | Sev | Line | Scenario | Fix |
| --- | --- | --- | --- | --- |
| N-1 | Low | 228, obs-check L114 | The doc says [4] checks both `quota_group_id` = `claude-sub-shared` before copying. obs-check only **prints** them: in my fake tree, `None` passed. A mismatch would surface later, in the A5 summary or as a `prepare` quota mismatch, but [4] does not gate it. | Either change the doc wording to "prints; the agent confirms `claude-sub-shared`", or add `and all(e[r].get("quota_group_id") == "claude-sub-shared" for r in e)` to `ok`. The latter changes the obs-check hash, SHA256SUMS and the doc's self-hash. |
| N-2 | Low | 25-26, 61, 80 | The parser may keep the `!` as part of the subcommand. Then `! find … -perm /222` does not equal `Bash(find step8-46799b0 step8-ops -perm /222)`, and **every** gated call prompts or goes to the classifier, including A6. That is not unsafe (the doc says so at L61), but it defeats auto-approval at the outage step. | Replace the clause with `find step8-46799b0 step8-ops -perm /222 \| wc -l \| grep -qx 0` (no `!`; the pipeline status is grep's). Replace the `Bash(grep -q .)` rule with `Bash(wc -l)` and `Bash(grep -qx 0)`. |
| N-3 | Low | 30-32, 45-47 | The exact rules stay valid for the whole run, so a fixed line can be reused at the wrong **time** without a prompt. For example, the `stop` line during `runs` (a restart during runs is outside the bundle), or `runs` re-invoked after a `STOP`. Replays of R1 and R2 are idempotent by key, but a transient STOP (such as a `role_wait` timeout) followed by a re-invocation could continue on to G1, which is a model call after a stop. | Add to the "명령 형식의 약속" section: "a rule command is invoked only at its step; after its step completed or ended in `STOP`, it is never invoked again without a user decision (except the documented rollback pair)". Optionally, the user removes the capture, roles and start-M lines once A6 is verified. |
| I-1 | Info | 207 | The test DB password file lives in this session's scratchpad (exists, 0600). If the scratchpad is cleaned, A3.7 fails closed: `migrate` FAIL, nothing live. | None needed. |
| I-2 | Info | 22, 27 | Whether the harness compares quoted `echo` text raw or normalised is untested. `echo` is read-only, so at worst it prompts. | None. |

## Recommended final permission JSON

This is the PR's list with N-2 applied. The deny rules are unchanged.

```json
{
  "permissions": {
    "allow": [
      "Bash(cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data)",
      "Bash(echo 'c92f4fec198cb1374b01e493c23b055ef11cfb35124ec0af7d30df005f30223b  step8-ops/SHA256SUMS')",
      "Bash(sha256sum --quiet --strict -c -)",
      "Bash(sha256sum --quiet --strict -c step8-ops/SHA256SUMS)",
      "Bash(find step8-46799b0 step8-ops -perm /222)",
      "Bash(wc -l)",
      "Bash(grep -qx 0)",
      "Bash(echo \"copies ok\")",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord 58090 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-M.log --tokens /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json -- --roles /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/roles.json)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-778ee47-rollback.log)",
      "Bash(export QUOTA_GROUP_ID=claude-sub-shared)",
      "Bash(export SUP_DEV_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-dev.token)",
      "Bash(export SUP_GR_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-gr.token)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/m-46799b0 /opt/rust/ieo_poc_runs/02-05-s8-20261118)",
      "Bash(export COORD=http://127.0.0.1:58090)",
      "Bash(export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8)",
      "Bash(export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out)",
      "Bash(export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205)",
      "Bash(export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header)",
      "Bash(export EXPECTED_CURRENT_DEV=null)",
      "Bash(export EXPECTED_CURRENT_GR=null)",
      "Bash(export OBS_UNIX=1794698043)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh register)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh prepare)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh runs)"
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

The gate line that goes with it:
`cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data && echo 'c92f4fec198cb1374b01e493c23b055ef11cfb35124ec0af7d30df005f30223b  step8-ops/SHA256SUMS' | sha256sum --quiet --strict -c - && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && find step8-46799b0 step8-ops -perm /222 | wc -l | grep -qx 0 && echo "copies ok"`

If N-2 is not applied, the PR's own list (with `grep -q .`) is acceptable. Every rule is harmless on its own:
- `cd`, `echo`, `sha256sum -c`, `find -perm`, `wc` and `grep` are read-only.
- The exports set only literals.
- The script lines are fixed full commands.

The residual risk is timing (N-3).

## Summary

All three Medium findings and all seven Low findings from review 1 are resolved. Copies, hashes and the self-hash `c92f4fec…` are verified. The JSON writers agree with the coordinator's parsers. The heads gate plus `null` matches the plan. The obs-check fixes are tested. The remaining items are Low:
- **N-1:** [4] prints the quota groups but does not check them, contrary to the doc.
- **N-2:** the `!` in the gate may force a prompt on every gated call. The fix needs no `!`.
- **N-3:** add a "no re-invocation after the step or a STOP" sentence.

**Approve.**
