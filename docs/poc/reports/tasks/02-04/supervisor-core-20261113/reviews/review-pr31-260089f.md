# Independent re-review 2: PR #31 (02-04 part 1, supervisor core without model calls)

- Head reviewed: `7f06184d8f565f143fd5f0a314bb8b572c09415f` (confirmed with `gh pr view 31 --json headRefOid`). The review began on `260089ff2a368b041670eeda2ffab30934784ce4` (correction 1). The head then moved to 7f06184, which changes only the lock test holder (`git diff 260089f 7f06184`: `tests/supervisor.rs`, +5/−4). Everything below applies to 7f06184. The file name keeps 260089f because that is the correction under review.
- Previous review: `review-pr31-8cc5c30.md` (findings 1–13).
- Authorities: Runtime_Contract_v0.md §4.1, §4.3, §5, §6, §10, §11; user decision 2026-11-13 ① (PR #30).
- Commands (scratch copy of 7f06184): `cargo test -p ieo-supervisor` passes 13/13. `cargo clippy -p ieo-supervisor --all-targets -- -D warnings` is clean. `cargo fmt --check` is clean. GitHub CI on 7f06184: all 3 jobs pass.
- Probes: 8 probe tests and 17 mutants in a throwaway copy, deleted afterwards. No repo files were changed.

## Verdict: CHANGES REQUESTED

Correction 1 resolves findings 1 and 4–11 and most of 2, 3, 12 and 13. The 260089f CI failure is fixed by 7f06184. Two new Medium defects remain in the tamper detection that finding 2/3 rewrote:
- A link at `projects` or `projects/<x>` hides `projects/*/memory/` from the digest (N1).
- A FIFO anywhere in `CLAUDE_CONFIG_DIR` makes `observed()` block forever (N2).

Both are small fixes.

## Findings from review 1, re-checked

| # | Status | Evidence |
| --- | --- | --- |
| 1 overage `allowed_warning` | Resolved | Only `"rejected"` passes, and `isUsingOverage:false` and `status:"allowed"` are also required. The test covers `allowed`, `allowed_warning`, null, `isUsingOverage` true or null, and `status` rejected or `allowed_warning`. The mutant that restores the old rule is killed. |
| 2 links followed / deleted outside | Resolved for cleanup; partly for detection | Cleanup uses `symlink_metadata` and `remove_file` on links. The test keeps the outside files and removes `skills` → outside and `projects/p/l.jsonl` → outside. Detection still has gaps: see N1 and N4. The mutant "digest walk follows links" survives: see N8. |
| 3 I/O errors swallowed | Resolved, with new defects | `walk` and `directive_digest` propagate errors, `observed()` maps them to `unreadable: …`, and `clean_dir` restores 0700 before `read_dir`. Mutants for ignoring errors and skipping chmod are killed. New: N2 (FIFO hang) and N3 (an `unreadable:` string can match). |
| 4 missing fields = 0 | Resolved | Missing `duration_ms`, a missing token key, a non-measured state or a missing `model_request` each give Stop. Mutants for duration-as-0 and for ignoring unclassified records are killed. Overflow is a separate issue: N5. |
| 5 token projection | Resolved | The docs and the PR body now say tokens are checked between runs against spent plus the largest run, that the first run is bounded only by its timeout, and that in-stream stop is later. The code matches the docs: it stops when `spent + reserve > cap`, so landing exactly on the cap still starts (tested at 2M+1M == 3M). cache_read uses the same reserve (mutant killed). |
| 6 observation freshness / all events | Resolved (age and all events) | Every event must pass (mutant killed). Age ≤ 7 d, tested at the boundary. `stream_stop` covers a change during a run. Account binding is the caller's job: `Observation` carries no account. That is acceptable but should be stated. Malformed lines: N6. |
| 7 lock TOCTOU | Resolved | Uses `File::try_lock` (Rust ≥ 1.89; the pin is 1.99.1), which is `flock(LOCK_EX\|LOCK_NB)` on Linux. The lock belongs to the open file description, so two fds in the same process conflict (tested: the second `acquire` gives `Running{pid}`). It is released when the last fd closes, so on process death. The file is never removed, and there is no `Drop` removal. An empty or leftover file is not a lock (tested). See N9 for the remaining notes. |
| 8 §5-1 fields | Resolved | Image digest, profile ID and revision, children and pending requests are added. `decide` refuses resume while either list is non-empty or missing (mutant killed). `decide` still does not compare image or profile with the coordinator: see N10. |
| 9 reconnect edge cases | Resolved | An empty or missing session_id, a newer generation, a missing generation and a missing settings_digest each go to reconciliation (tested, mutant killed). The comment on `None` before `running` was not added. That is trivial. |
| 10 args | Resolved | One argv entry per Guardrail rule, in the tested order. The open development lists are noted in a code comment and in the PR body. |
| 11 env / autoMemory / credentials | Resolved | `apply()` calls `env_clear`, and the test runs `/usr/bin/env` and compares what the child sees. A template without `autoMemoryEnabled:false` is refused (mutant killed). `credentials_removed` is reported (tested). |
| 12 weak tests | Mostly resolved | Boundaries for runs, seconds, billable and cache_read are covered, and temp dirs are removed by `Tmp::drop`. The `projects.*.allowedTools` and `projects.*.enabledMcpjsonServers` mutants still survive: N8. |
| 13 PR body | Mostly resolved | One remaining overclaim: "Symlinks are never followed" (N4). |

## New findings

### N1. [Medium] A link at `projects` or `projects/<x>` hides `projects/*/memory/` from tamper detection
`walk` lists a link as `Entry::Link` and hashes it only if `is_directive(rel)` is true.
- For `projects`, `is_directive` returns false: the `[f]` arm excludes `"projects"`.
- For `projects/<x>`, it returns false: `["projects", ..] => false`.

The CLI follows the link, so `projects/<x>/memory/…` behind it is a §5-6 directive location that is never digested.

Probe: after `clean`, `projects/-state-work-sori` (and separately `projects`) was replaced with a link to a directory containing `memory/MEMORY.md`. Result: `observed == checkpoint digest: true` in both cases, so `decide` gives `Resume`. Cleanup removes the link afterwards, but by then the session has already been resumed.

The module doc says "a link inside the directive scope is digested … (so planting one is tamper)". The memory path is in scope; only its parent is out of it.

Fix: treat every symlink anywhere under `CLAUDE_CONFIG_DIR` as tamper. The CLI's normal runs (01-04) are not known to create links there; otherwise list the known ones. At minimum, digest links whose path is a prefix of a directive path. Add a test.

### N2. [Medium] A FIFO in `CLAUDE_CONFIG_DIR` blocks `observed()` forever
`directive_digest` calls `std::fs::read(p)` for every `Entry::File` before it checks `is_directive`. Every non-directory, non-link entry becomes `Entry::File`, including FIFOs, sockets and device nodes.

Probe: `mkfifo $CONFIG/shell-snapshots-fifo` (a non-directive path). `observed()` was still blocked after 3 s and returned only when the probe opened the FIFO for writing.

The development role can write to this directory (§2.1), so it can stall `reconnect` indefinitely instead of getting a tamper report. That contradicts the doc's "fail closed". A socket instead gives `ENXIO`, which produces a false `unreadable` tamper.

The same read also pulls every session `.jsonl` and any large file into memory, although only directive items are hashed.

Fix:
- Compute `rel` and `is_directive` before reading.
- Read only regular files: check `file_type().is_file()`.
- Treat any other kind in the directive scope as tamper, without opening it.
- Read `.claude.json` the same way (see N4).

Add a FIFO test.

### N3. [Low] `observed()` can match a checkpoint that itself stored an `unreadable:` value
`io::Error` text has no path, so the string is deterministic: `unreadable: Permission denied (os error 13)`.
- `decide` accepts any non-empty `settings_digest`.
- `record()` accepts any `&str`.

If the adapter writes `observed()` into a checkpoint (an obvious API to call at checkpoint time) while a directory is unreadable, every later unreadable state matches it.

Probe: checkpoint digest `unreadable: Permission denied (os error 13)`. `CLAUDE.md` was then planted. `decide` gave `Resume`.

Against a real digest from `clean()`, a false match is not possible: the value is 64 hex characters, never `unreadable…`.

Fix: return `Result`/an enum from `observed()`, or have `decide` accept only a 64-hex `settings_digest` (otherwise reconcile), and test it.

### N4. [Low] `.claude.json` is read through a link
A link named `.claude.json` is skipped in the entry loop, because `is_directive(".claude.json")` is false. `claude_json_directives` then calls `std::fs::read`, which follows the link.

Probe: `.claude.json` → an outside file with no directive keys gives `observed == digest: true`. The planted link is not tamper. A FIFO target also hangs here (N2).

This contradicts "Symlinks are never followed" in the module doc and the PR body. Fix: treat a non-regular `.claude.json` as tamper.

### N5. [Low] Token sums can overflow
`i + o + cc` and the `+=` accumulators are unchecked u64 operations.

Probe: `input = u64::MAX, output = 2` panics in debug (`budget.rs:138`). In release (overflow checks off) the sum wraps to 1, so the result is `Start`.

The input comes from our own collector, so this is unlikely. Still, an implausible value should count as unmeasured, not wrap. Use `checked_add` and treat `None` as unmeasured.

### N6. [Low] Malformed `rate_limit_event` lines are skipped silently
`rate_limit_events` and `stream_stop` drop lines that do not parse.

Probe: a truncated event with `"overageStatus":"allowed","isUsingOverage":true`, followed by a good event, gives `observation_check` = Ok. `stream_stop(truncated)` = None.

Suggest: a line that mentions `rate_limit_event` but does not parse should fail the check (Hold) or stop the run.

### N7. [Low] Cleanup changes the mode of the config dir root to 0700
`clean_dir` runs `set_permissions(0o700)` on every directory, including the root.

Probe: a root at mode `2775` becomes `700` (setgid and group bits are lost).
- This is fine if the supervisor and the role share a uid (same container, §2.1).
- If the volume root is owned by another uid (for example a root-owned mount point, or k8s `fsGroup`), the chmod fails with EPERM and every cleanup fails, which is fail closed.
- If it succeeds, group access is cut.

Suggest: leave the root's mode alone, or apply 0700 only to subdirectories that cannot be read. Also note that `symlink_metadata` → `set_permissions`/`read_dir` is a TOCTOU if any role process is still alive during cleanup, because both calls follow links. State that cleanup runs only when no role process is alive.

### N8. [Low] Tests that still pass with a fix reverted
Mutants run on 7f06184 (`tests/supervisor.rs`).

**Killed (12):** old overage rule, ignoring `read_dir` errors, no chmod in cleanup, newer generation accepted, last event only, missing duration as 0, no age check, no cache_read reserve, link not removed, children/pending unchecked, template autoMemory check skipped, unclassified records ignored.

**Survived (5):**
- The digest walk uses `metadata` (follows links). The only link test uses a dangling `/elsewhere` link, which fails as an error either way. Use a link to an existing directory with a directive file.
- Dropping `projects.*.allowedTools` from the `.claude.json` directive keys (carried from review 1 #12).
- Dropping `projects.*.enabledMcpjsonServers` (same).
- Dropping the `kind` byte from the digest (minor).
- Widening the builtin exclusion to every `plugins/store/*.json`. There is no test that a non-builtin store file is directive.

CI robustness:
- The `chmod 000` assertions (`observed.starts_with("unreadable")` and the cleanup reopen) need a non-root user, because root bypasses DAC. The ubuntu-latest runner runs as `runner`, so they pass there. They would fail in a root container job. A short comment would help.
- `sh`/dash `exec 9<>"$0"`, `flock` (util-linux) and `/usr/bin/env` are present on ubuntu-latest.

### N9. [Info] Lock: the 260089f CI failure is fixed; remaining notes
- On 260089f the test killed `flock(1)`, but the forked `sleep` inherited the locked fd. CI failed. Locally it failed 10 of 30 runs, and 20 of 20 pinned to one CPU.
- 7f06184 uses `sh -c 'exec 9<>"$0" && flock 9 && exec sleep 30'`, which is one process holding the fd. It failed 0 of 30 runs, and 0 of 20 pinned to one CPU. CI is green.
- Product notes:
  - std opens with `O_CLOEXEC`, so the `claude` child will not inherit the lock fd. If the supervisor dies while `claude` survives, the lock is released during a live session. In the pod, the supervisor is PID 1, so its death ends the container; say so when wiring.
  - Deleting the lock file lets a second acquirer lock a new inode (probe: `true`). This is the documented §5-5 limit.

### N10. [Info] Reconnect does not compare image or profile
`CoordinatorRecord` has no `image_digest` or `profile_id`/`profile_revision`, so a checkpoint from a different image or profile resumes. §5-2 says only "digest", which is ambiguous. Record this as a decision point, or add the comparison, when the coordinator record is wired.

## What is correct
- The caps match decision ①:
  - 02-04: 12 runs / 1800 s / 14400 s / 1.5M billable / 15M cache_read.
  - X-02: 6 runs / 3600 s / 14400 s / 3M / 30M.
  - Both are asserted in full.
- Billable = input + output + cache_creation. Unmeasured is never 0 and leads to Stop. There are no retries. An overage problem leads to Hold for that call only. Only `rejected`/`false`/`allowed` lets a call start. The real 01-04 stream passes.
- §4.1 argv and §4.3 environment: unchanged and correct. The environment is now verified on a real child.
- Lock semantics are as claimed: same-process separate fds conflict, the lock is released on process death, and the file is never removed.
