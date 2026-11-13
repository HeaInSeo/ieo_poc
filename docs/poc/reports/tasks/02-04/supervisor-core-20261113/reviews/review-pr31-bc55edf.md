# Independent re-review 3: PR #31 (02-04 part 1, supervisor core without model calls)

- Head reviewed: `bc55edfd43044f1043d5c00d75ee959283b26ba5` (confirmed with `gh pr view 31 --json headRefOid`). Correction 2 = `git diff 7f06184 bc55edf` (budget.rs, checkpoint.rs, configdir.rs, tests/supervisor.rs; +261/−48).
- Previous reviews: `review-pr31-8cc5c30.md` (1–13), `review-pr31-260089f.md` (N1–N10, on 7f06184).
- Authorities: Runtime_Contract_v0.md §4.1, §4.3, §5, §6, §10, §11; user decision 2026-11-13 ① (PR #30).
- Commands (worktree, unchanged): `cargo test -p ieo-supervisor` 18/18 pass; `cargo clippy -p ieo-supervisor --all-targets -- -D warnings` clean; `cargo fmt --check` clean. Lock test `one_running_process_per_session`: 0/30 failures, 0/15 pinned to one CPU (`taskset -c 0`). GitHub CI on bc55edf: 3/3 pass.
- Probes: 7 probe tests and 9 mutants in a throwaway `git archive` copy, deleted afterwards. No repo files changed.

## Verdict: APPROVE

N1–N10 are resolved (N5 only in `spent()`, see R1). No regressions against findings 1–13. The remaining items are Low/Info and do not block this no-model-call core; R1 should be fixed before the guard is wired to real runs (one line).

## N1–N10 re-checked

| # | Status | Evidence |
| --- | --- | --- |
| N1 link at `projects`/`projects/<x>` | Resolved | Links are digested at every path (`kind != b'l' && …` filter). New test plants links at `projects`, `projects/-state-work-sori`, `.claude.json`, `agents/a.md`, `backups` → `TamperDetected`, and changing the target leaves the digest unchanged (not followed). Mutant "walk uses `metadata`" now killed. Real 01-04 listings (G.json) show no links from normal CLI runs. |
| N2 FIFO blocks `observed()` | Resolved | `walk` classifies non-regular, non-dir, non-link as `Entry::Other`; scope check precedes any read; `Other` is digested by path+kind without opening. Test with a 10 s watchdog: FIFO outside scope ignored, inside scope tamper, cleanup restores the digest. Probe: Unix socket in `hooks/` → tamper, removed by cleanup. Mutant "skip `Other`" killed. |
| N3 `unreadable:` checkpoint matches | Resolved | `decide` accepts only a 64-hex `settings_digest`, else reconciliation; tested with the exact `unreadable: Permission denied (os error 13)` value. Mutant reverting to non-empty killed. |
| N4 `.claude.json` read through a link | Resolved | `symlink_metadata` guard returns `Null` for any non-regular `.claude.json`; that differs from the missing-file value, so link/FIFO/directory there is tamper. Probes: FIFO `.claude.json` → tamper, no hang; directory `.claude.json` → tamper. See R3 for test coverage. |
| N5 token overflow | Partly | `spent()` uses `checked_add` (overflow → unmeasured → Stop) and `saturating_add` for seconds; tested. The decision step still adds unchecked: R1. |
| N6 malformed `rate_limit_event` | Resolved | `rate_limit_events` returns `Err` and `stream_stop` returns `Some` for an unparsable line containing `rate_limit_event`; tested. Mutant killed. Stored 01-04 events start with `{"type":"rate_limit_event"`, so a truncation that loses the name leaves almost nothing (R4). |
| N7 root chmod | Resolved | `clean_dir` skips chmod on the root; test with root 0750. Mutant killed. The "no live role process during cleanup" note was not added (R5). |
| N8 surviving mutants | Resolved | New tests cover all three `.claude.json` project keys, non-builtin `plugins/store` files, link-to-existing-directory, file-vs-link kind. |
| N9 lock | Resolved | Test now waits until `/proc/<pid>/comm == "sleep"` (only after `flock 9 &&` succeeded) and then asserts `Running`. Stable in 45 local runs. Product notes from review 2 still apply when wiring. |
| N10 image/profile not compared | Resolved | `CoordinatorRecord` gains `image_digest`, `profile_id`, `profile_revision`, compared in `decide`; mismatch → reconciliation, tested. Mutant dropping `image_digest` killed. |

Review 1 findings 1–13: no regression. Overage rule, unmeasured-never-zero, between-runs token projection, freshness, lock semantics, §5-1 fields, argv and env code are unchanged by correction 2 except as listed above. Caps still match decision ① (02-04: 12/1800/14400/1.5M/15M; X-02: 6/3600/14400/3M/30M).

## New findings

### R1. [Low] `next()` still overflows on `spent + reserve`; in release it starts a run
`budget::next` computes `s.billable_tokens + reserve` and `s.cache_read_tokens + reserve_cr` unchecked. `spent()` now prevents the running total from overflowing, but total + largest-run can still exceed `u64::MAX` (any total ≥ 2^63 with that single run as the largest).

Probe: one measured record with `input = 2^63`:
- debug: panic at `budget.rs:270` (cache_read variant at `:279`);
- `--release` (overflow checks off): `2^63 + 2^63` wraps to 0 → `Start { timeout_seconds: 3600 }` with 2^63 billable tokens spent.

The input comes from our own collector, so this is implausible in practice, which is why it is Low. But the PR body says token values that overflow are handled, and this is the wrong-result path N5 described. Fix: `s.billable_tokens.checked_add(reserve).is_none_or(|x| x > cap)` → Stop (same for cache_read), and add a test with a single 2^63 run.

### R2. [Low] Body/test claim on overflow is wider than the code
"A run … with token values that overflow counts as unmeasured" is true for `spent()`; the test `overflowing_or_truncated_measurements_are_not_trusted` only covers sums that overflow inside `spent()`, not R1. Fix with R1.

### R3. [Low] The `.claude.json` non-regular guard is not pinned by a test
Removing the `symlink_metadata(...).is_ok_and(!is_file)` guard still passes all 18 PR tests: the test links `.claude.json` to a directory, so the follow-read fails with EISDIR, `observed()` becomes a stable `unreadable:` value, and both assertions still hold. With the guard removed, a FIFO `.claude.json` hangs `observed()` (probe; this mutant was only killed by my FIFO probe, by hanging). Add a FIFO `.claude.json` case (with the watchdog) or a link to a regular outside file and assert the digest does not change when that file changes.

### R4. [Info] A line truncated before the event name is skipped silently
`{"ty` alongside a good event: `observation_check` = Ok. A truncation that short carries no status, and the CLI emits `type` first, so this is acceptable; just note the "never skipped" body wording covers lines that still contain `rate_limit_event`.

### R5. [Info] Carry-overs to the wiring step
- Cleanup and `observed()` assume no role process is alive (review 2 N7 TOCTOU note); state this where the adapter calls them.
- The CLI's `debug/latest` symlink (seen in a local `~/.claude/debug/`, not in the 01-04 listings) would make every reconnect a tamper detection if debug logging is ever enabled. That fails closed; keep debug logging off or list it.
- Mutant "read every regular file before the scope check" survives: there is no test that an unreadable non-directive regular file (e.g. `chmod 000 sessions/x`) is not opened. Minor.
- Lock notes from review 2 N9 (O_CLOEXEC, PID 1) still apply.

## PR body check
- Accurate: no-model-call scope, later steps (pods, PID 1, adapter, real-session tests) not claimed; links at any path are tamper; special files never opened; 64-hex digest rule; root mode untouched; image/profile comparison; caps; no-overage rule; 18 tests.
- Overclaim: overflow handling (R1/R2), Low.
- "0 failures in 60 repeated runs, 20 pinned" is consistent with my 45 runs (0 failures).
