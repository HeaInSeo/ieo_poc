# Independent review: PR #31 (02-04 part 1, supervisor core without model calls)

- Head reviewed: `8cc5c3065c5c73789ddd96e64ad7dba36acdfef4` (branch `task/02-04-supervisor-core`)
- Scope: `crates/ieo-supervisor` (args, env, lock, configdir, checkpoint, budget) and `tests/supervisor.rs`
- Authorities: Runtime_Contract_v0.md rev 6/7 (`docs/poc/reports/tasks/01-02/runtime-contract-20261112/`), `docs/poc/User_Decisions_2026-11-13_x02.md` decision ① (read from PR #30 head `7afd4d1b`; the file is not on this branch)
- Commands: `cargo test -p ieo-supervisor`: 9/9 pass. `cargo clippy -p ieo-supervisor --all-targets -- -D warnings`: clean.
- Extra probes: a throwaway copy of the crate in the scratchpad with 5 probe tests (deleted afterwards). The results are quoted below. No repo files were changed.

## Verdict: CHANGES REQUESTED

One High finding is on the user's hard rule (USD 0 extra billing): the no-overage guard lets a call start while overage is enabled. A second High finding is that the config-dir cleanup deletes files outside `CLAUDE_CONFIG_DIR` and leaves directive content in place. Most of the rest is fail-open handling of missing data, which goes against the decision's rule that unmeasured is never 0.

## Findings

### 1. [High] `overageStatus: "allowed_warning"` passes the no-overage check
`budget::overage_check` holds the call only when the status is `"allowed"` or missing. Every other string counts as "no overage". The installed CLI (2.1.322) defines `overageStatus` as `["allowed","allowed_warning","rejected"]`, and its own text says: "overageStatus allowed / allowed_warning means paid extra usage covers the overflow". So with `allowed_warning`, paid extra usage is enabled.
Probe: `{"status":"allowed","overageStatus":"allowed_warning","isUsingOverage":false}` gives `Decision::Start { timeout_seconds: 1800 }`.
Fix: fail closed. Accept only the known-safe value `"rejected"`, optionally together with an `overageDisabledReason`, and hold on everything else. Add a test for `allowed_warning` and for an unknown value.

### 2. [High] `configdir::clean` follows directory symlinks, deletes files outside the config dir, and the symlink survives
`walk` uses `Path::is_dir()`, which follows symlinks. `clean` then calls `remove_file` on every file it finds. The contract (§2.1, §5-6) says the development role can write into `CLAUDE_CONFIG_DIR`. If it does `ln -s /state/work/<repo> $CLAUDE_CONFIG_DIR/skills`, cleanup deletes the files in the target directory. The symlink itself is never removed, because it was traversed instead of listed, and `remove_dir` on a link fails silently. So the cleaned directory still contains a directive entry (`skills` → an agent-controlled directory), which goes against "keep only `projects/<cwd>/*.jsonl`".
Probe: an outside `victim.txt` was deleted (`victim exists after clean: false`), and the link remained (`symlink survives: true`).
Fix: use `symlink_metadata` / `DirEntry::file_type()` and do not follow links. Unlink links as entries. Treat any symlink inside the config dir as tamper, or at least include it in the digest as a link. Add a test.

### 3. [Medium] I/O errors are swallowed, so tamper detection and cleanup fail open
`walk` ignores `read_dir` errors. `directive_digest` hashes an unreadable file as empty content. `prune` ignores `remove_dir` failures.
Probe: after a clean checkpoint, `skills/x/SKILL.md` was added and then `chmod 000 skills`. The digest was unchanged (`digest unchanged with hidden skill: true`). `clean` returned `Ok`, and `skills/` with its content survived. The development role can undo the mode later.
Fix: return `io::Result` from `walk` and `directive_digest`. Treat any unreadable entry under the config dir as tamper, or as a failed clean that refuses to start.

### 4. [Medium] Budget: missing or malformed fields count as 0 (against decision ① "unmeasured is never 0")
`spent()` reads untyped JSON. It uses `duration_ms.unwrap_or(0)` and `tokens[k].unwrap_or(0)`, and it counts a record only if `model_request == true`.
Probe: `{"model_request":true,"usage":{"run":{"state":"measured","tokens":{}}}}` gives `seconds=0 billable=0`. A record whose `model_request` is missing or not a boolean is not counted at all.
`ieo-evidence` currently always writes these fields. Even so, the guard should not depend on that silently. Either deserialize into `ieo_evidence::record::EvidenceRecord` (typed) or treat any missing or non-numeric field as unmeasured, which leads to Stop. A test should cover it.

### 5. [Medium] Budget: the token cap check is a heuristic, and the PR wording claims more than it does
- `billable + largest_run_billable >= cap` guesses that the next run will be no larger than the largest run so far. The first run is never limited: `largest = 0`, so a single run can go far past 1.5M. Any later run that is larger than the earlier ones can also cross the cap. Nothing here stops a run in flight when the cap is reached. The doc comment says "used to refuse a run that would cross the cap", and the test comment says "a run that would cross the token cap is refused". Neither is guaranteed.
- `cache_read` has no such projection (`spent >= cap` only), so it is inconsistent with the billable check.
- Requested: document that the caps are checked between runs and a single run may overshoot. State whether in-flight enforcement (watching stream usage and terminating at the cap) is planned for a later 02-04 step, and add it to the PR body's "comes later" list. Alternatively, set the projection for the first run from a configured per-run estimate.

### 6. [Medium] The no-overage observation has no freshness, account binding or window coverage
- `next()` accepts any `latest_rate_limit`. The doc comment allows "for the first run the newest stored observation of the same account", but nothing bounds its age. Overage settings can change between the observation and the call; the test uses a stream from 2026-11-12. Decision ① says to confirm the condition, or hold the call.
- `last_rate_limit` takes only the very last event in the stream. Streams can carry events for different `rateLimitType` windows, and the stored ones include events without overage fields (`{"status":"allowed","rateLimitType":"five_hour"}`, 3 cases). One of these as the last event holds the call, which is safe. But an earlier event in the same run with `isUsingOverage:true` is ignored if a later one says false.
- Suggest: require an observation timestamp within a bound (or from the immediately preceding run of the same credential), and check every `rate_limit_event` of the last run.

### 7. [Medium] Session lock TOCTOU: two acquirers can both "hold" the lock
- `create_new` is followed by a separate `write_all`. A second acquirer that reads in between sees an empty file and parses no pid. It treats that as stale and deletes the first holder's live lock. Probe: an empty lock file is taken over (`stolen: true`).
- Two acquirers that both see a truly stale lock can each `remove_file` and re-create it. B can delete A's fresh lock.
- `Drop` removes the path even if another holder has since replaced it.
- Contract §10 item 4 makes single-run per session the supervisor's guarantee. Use `flock`/`fcntl` on the file (the lock dies with the process, so there is no pid probing), or write to a temp file and `link()` it into place. Do not treat an empty or unparsable file as stale without an age check.
- Also: the `/proc/<pid>` liveness check uses the pid from a previous pod's pid namespace. After a pod restart a stale pid (for example 1) can match a live process, and reconnect then reports `AlreadyRunning` forever. This fails closed, but it hurts liveness. Record a start-time or boot id with the pid.

### 8. [Medium] The checkpoint record omits §5-1 fields and hard-codes empty lists
§5-1 requires `image_digest`, `profile ID/revision`, the active child list and the pending external request IDs. `record()` has no `image_digest` or profile fields, and it always writes `active_children: []` and `pending_external_requests: []`, with no parameter to pass them. `decide()` never looks at children or pending requests, although §6 says to reconcile them before resume. The PR body says "the §5-1 record", which overclaims. Either add the fields and parameters, or list them as later work.

### 9. [Low] Reconnect decision edge cases
- A checkpoint with a missing or non-string `session_id` gives `Resume { session_id: "" }`, which would produce `--resume ""`. This should be `NOT_OBSERVED` / external reconciliation.
- A checkpoint generation greater than the coordinator's current generation is accepted and resumed. That is an inconsistent state, so it should go to external reconciliation, not resume.
- `checkpoint == None` returns `NewSession` before `running_pid` is checked. Harmless while the lock is keyed by session id, but worth a comment.
- §5-2 compares "digest" against the coordinator record. Only `spec_digest` is compared. image/profile are not in the record (see 8).

### 10. [Low] Args: development role has no allow/deny rules; the allow-list is passed as one space-separated argv element
- Contract §4.1's common block includes `--allowedTools <role list> --disallowedTools <role list>` for both roles. The development role gets neither. This is acceptable because §9 defers the lists to 03-02, but it should be recorded as a `capability_gap` / later step rather than left silent.
- The Guardrail rules are passed as single space-joined strings. Check that this matches how 01-04 U-alt and G1 passed them, since §10 item 1 says to run "the revision 6 Guardrail rule set as is" in 02-04.
- The `Write(` check passes trivially because `--tools "Read,Grep,Glob,Write,Edit"` contains `Write,` and not `Write(`. That is correct per contract. Just noting that the test does not separately confirm `Edit(//` appears only in the guardrail rules.

### 11. [Low] env: no process spawn yet; `autoMemoryEnabled:false` is not enforced
- `env::build` returns a list. Nothing in the crate does `Command::env_clear()`. "Empty environment" is true only once the later wiring does it. The test compares `names(build())` with `ALLOWED`, which is close to tautological.
- `clean` installs whatever template it is given. It does not check that the template's `settings.json` contains `{"autoMemoryEnabled": false}` (§4.1, §10 item 3).
- §4.1 says a credential file found in the config dir is deleted **and recorded**. `clean` deletes it silently.

### 12. [Low] Tests that would still pass with the behaviour broken
- Overage: removing the `status == "allowed"` branch, or treating a missing `isUsingOverage` / `overageStatus` as OK, still passes. Only `isUsingOverage:true`, `overageStatus:"allowed"` and `None` are tested. There is no `allowed_warning` case (finding 1).
- Budget boundaries: no test at exactly 5→Start / 6→Stop runs, 14,399→Start / 14,400→Stop seconds, or billable just below and at the cap. Changing `>=` to `>` for billable, or to `s.runs + 1 >= max`, passes. `BUDGET_0204.per_run_seconds` and `total_seconds`, and `BUDGET_X02.total_seconds` and `cache_read_tokens`, are not asserted. Changing them passes.
- Tamper (b) covers only skills, memory, agents and hooks, plus `.claude.json` top-level `mcpServers` (digest only, not `decide`). Dropping `commands`, `output-styles`, `plugins`, `settings*.json`, `CLAUDE.md` or `.mcp.json` from the directive set, or the `projects.*.allowedTools` key, passes.
- No symlink, permission or empty-lock tests (findings 2, 3 and 7).
- The tests leave `ieo-sup-*` directories in the system temp dir; I counted about 70 after runs. Consider cleaning up or using `tempfile`.

### 13. [Low] PR body precision
- "a call starts only when the last `rate_limit_event` shows `isUsingOverage:false` and `overageStatus` is not `allowed`". This omits the `status == allowed` condition, which the code does check. It also shows the rule that finding 1 breaks: "not `allowed`" is not the same as "no overage".
- "the §5-1 record" overclaims (finding 8).
- "An empty environment plus an allow-list" describes a list builder, not a spawn (finding 11).
- The budget section should say that caps are checked between runs (finding 5).

## What is correct
- The role argv matches §4.1 for the fixed common flags and the Guardrail rule set. There is no `Write(`, no `--add-dir`, no bypass flags. The resume key is the session_id only, and `-n` is not used for resume.
- `ALLOWED` matches §4.3. There is no `ANTHROPIC_API_KEY` and no `CLAUDECODE*`.
- The directive set and the `.claude.json` key selection match §5-6. The normal-run exclusions match the 01-04 list, and test (c) covers them.
- The decision ① constants match: 02-04 is 12 / 1800 / 14400 / 1.5M / 15M, and X-02 is 6 / 3600 / 14400 / 3M / 30M. Billable is input+output+cache_creation. Every `model_request` run counts. Any unmeasured run gives Stop. The next timeout is `min(per_run, total − spent)`, with no underflow because the `>=` check comes first. Overage problems give Hold, not Stop, which matches "hold only that call".
- The real 01-04 stream parses, and its event (`rejected` / `out_of_credits` / `false` / `allowed`) correctly passes.
