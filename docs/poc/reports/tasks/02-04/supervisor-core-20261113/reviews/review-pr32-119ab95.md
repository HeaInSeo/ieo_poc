# Independent review: PR #32 (02-04 supervisor, review bc55edf R1–R3 follow-up)

- Head reviewed: `119ab951edf8b84f3a96b90cb41de4257f4d8b15` (from `gh pr view 32 --json headRefOid`). Base: main `0f446d6` (PR #31 merge). Diff: budget.rs +9/−2, tests/supervisor.rs +56.
- Findings addressed: `review-pr31-bc55edf.md` R1–R3.
- Commands (worktree, no repo files changed): `cargo test -p ieo-supervisor` 20/20 pass; `cargo clippy -p ieo-supervisor --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `--release` 20/20 pass. GitHub CI on 119ab95: 3/3 pass (one run).
- Probes: 7 mutants in a throwaway `git archive` copy, plus repeated full-suite runs of head and of main `0f446d6`. Copies and target dir deleted afterwards.

## Verdict: CHANGES REQUESTED

R1–R3 themselves are fixed correctly and the new tests kill every relevant mutant. But the new FIFO test makes the existing lock test flaky (P1): 7/60 full-suite failures at head against 0/60 on main. One test change fixes it.

## R1–R3 checked

| # | Status | Evidence |
| --- | --- | --- |
| R1 `spent + reserve` overflow | Resolved | Both projections use `checked_add(..).is_none_or(|t| t > cap)`, so overflow means Stop. No other unchecked arithmetic remains in budget.rs. `spent()`: token sums use `checked_add` (overflow → unmeasured → Stop), seconds use `saturating_add` (a saturated `u64::MAX` is `>= total_seconds` → Stop), `runs += 1` is bounded by the slice length. `next()`: the timeout `budget.total_seconds - s.seconds` runs only after the `s.seconds >= total_seconds` Stop, so it cannot underflow. `observation_check` only compares `age_seconds`, and `i + 1`/`n + 1` are enumerate indexes. |
| R2 body/test claim | Resolved | `an_overflowing_projection_stops` covers a single 2^63 run for billable and for cache_read, asserts `spent()` treats it as measured (so the projection is what gets tested), and asserts Stop under both budgets. |
| R3 `.claude.json` guard test | Resolved | `a_non_regular_claude_json_is_never_opened`: a FIFO with a 10 s watchdog, and a link to an outside regular file with the content changed. The link part's `assert_ne` would hold without the guard because the walk digests the link anyway. The `assert_eq` after the content change is the assertion that pins the guard. |

Mutants (scratch copy):

| Mutant | Result |
| --- | --- |
| M1 billable check reverted to `+` | killed: debug panics at budget.rs:271; release fails the Stop assert (853) |
| M2 cache_read check reverted to `+` | killed: debug panics at :283; release fails 853 |
| M3/M4 `is_none_or` → `is_some_and` (overflow treated as within the cap), each check | killed (853) |
| M5 guard removed (`if false`) | killed: watchdog fires after 10 s (877) |
| M6 guard only for symlinks (FIFO opened) | killed (877, watchdog) |
| M7 guard only for non-links (link followed) | killed (898, content change alters the digest) |

The PR body's claim that both new tests fail with their fix reverted, in debug and release for R1 and with the guard mutated out for R3, is confirmed.

## Findings

### P1. [Medium] The new FIFO test makes `one_running_process_per_session` flaky
Repeated `cargo test -p ieo-supervisor --test supervisor` (64-core host):

| Tree / mode | Failures |
| --- | --- |
| main 0f446d6 (18 tests) | 0/60 |
| head 119ab95 | 7/60 (plus 3/30 in an earlier batch) |
| head `--skip a_non_regular_claude_json_is_never_opened` | 0/60 |
| head `--test-threads=1` | 0/60 |
| head `--skip an_overflowing_projection_stops` | 1/60 |

Each failure is in the lock test, when it reacquires right after `drop(held)`: `supervisor.rs:447` `after release: Running { pid: Some(<self>) }` or `:452` `leftover file`.

Likely mechanism: `a_non_regular_claude_json_is_never_opened` sorts first and runs alongside the lock test, and it spawns `mkfifo` with `Command::status()`. The lock fd is O_CLOEXEC, but a child spawned in the window between fork and exec still shares the open file description that holds the flock. The lock test's non-blocking `try_lock` after `drop(held)` then sees the lock as still held. The pre-existing `mkfifo` spawn at :758 is in a test that sorts later and did not hit the window in 60 runs. The new test is what exposes it.

CI passed this once, but a failure rate of about 1 in 9 will hit later CI runs and merges.

Fix (any one):
- In the new test, create the FIFO without spawning a process, e.g. `libc::mkfifo` or `nix` as a dev-dependency, or reuse whatever the :758 test needs.
- Or put a test-file `static SPAWN: Mutex<()>` around the lock test's release/reacquire section and around every `Command` spawn in the test file. This is more robust because :136, :149, :464 and :758 also spawn.

A bounded retry in the lock test would hide the race, so the two fixes above are better.

Product note, Info, for the adapter step: the same race applies to the real supervisor. If it spawns a role process on one thread while it releases and reacquires a session lock on another, it gets a transient `Running`. That fails closed, but the adapter should serialize spawns against lock release, or retry. This extends the O_CLOEXEC note from review 2 N9.

### P2. [Info] A leaked thread when the R3 watchdog fires
If `observed()` ever blocks, the test fails after 10 s and leaves a thread stuck in `open()` on the FIFO. The harness still exits (verified with M5/M6), so this is acceptable for a failing-path test.

## PR body check
- Accurate: R1/R2 description, including the debug panic and the release wrap to Start, matches the old code and my probes; the decision step now treats overflow as over the cap; both R3 cases are described correctly; "20 tests pass" is correct; the claims about failing with the fix reverted are confirmed by M1–M7; "No model calls" is correct; the R4/R5 carry-over notes are present.
- Missing: nothing is overclaimed. After the P1 fix, the body should mention the change to the test setup.
