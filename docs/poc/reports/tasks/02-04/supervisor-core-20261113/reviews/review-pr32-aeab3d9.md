# Independent review 2: PR #32 (02-04 supervisor, R1–R3 follow-up plus P1 correction)

- Head reviewed: `aeab3d9b2936442eab11db2dc26a3b4fc89c0e3f` (confirmed with `gh pr view 32 --json headRefOid`). Base: main `0f446d6`. Commits: 119ab95 (R1–R3), aeab3d9 (P1 correction).
- Correction diff `119ab95..aeab3d9`: only `crates/ieo-supervisor/tests/supervisor.rs` changed (+24/−6). budget.rs is unchanged since review 1.
- Commands (worktree, no repo files changed): `cargo test -p ieo-supervisor` 20/20 pass; `cargo clippy -p ieo-supervisor --all-targets -- -D warnings` clean; `cargo fmt --check -p ieo-supervisor` clean. GitHub CI on aeab3d9: 3/3 pass.

## Verdict: APPROVE

P1 is resolved, and the stress runs confirm it against a control that reproduces the flake. The correction introduces no new problem. One Low body-accuracy note below does not block.

## P1 correction checked

`static SPAWN: Mutex<()>` and `spawn_guard()`, which uses `lock().unwrap_or_else(|e| e.into_inner())`.

| Question | Result |
| --- | --- |
| Every spawn guarded? | Yes. The spawn sites are `/usr/bin/env` `.output()` (:150, guarded), `mkfifo` `.status()` in `special_files_are_never_opened` (:773, guarded inside the closure), `mkfifo` in `a_non_regular_claude_json_is_never_opened` (:881, guarded), and `sh` `.spawn()` in the lock test (:478, covered by the test-wide guard at :452). `env::apply(&mut Command::new("true"), &bad)` at :162 only builds a Command, so it does not fork. Library code does not spawn: the only `Command` use in `src/` is `env::apply`, which sets the environment. `std::thread::spawn` watchdogs do not fork. |
| Deadlock? | No. `std::sync::Mutex` is not reentrant, but no code path takes the guard twice: the lock test calls only `SessionLock`/`fs`/`Command`, none of which take the guard. The other guards are scoped to a single spawn: `env` runs to completion and `mkfifo` returns at once. They are dropped before the 10 s `recv_timeout` watchdog waits, so the closure guard is dropped at closure end, before `observe`. The lock test's guard is held across its wait for `sh` to reach `sleep`, bounded at 10 s, and that wait does not depend on any other guarded test. Worst case is serialisation, not deadlock. |
| Poisoning? | Handled. `into_inner()` recovers a poisoned guard. A failing test that holds it (lock test assert, `assert!(ok)` in the fifo closure) does not cascade into unrelated failures. |
| Scope | The mutex covers threads in the one test binary. That is enough because each integration-test binary and the lib unit tests run in separate processes, and the descriptor sharing is per process. |
| Lock-test own spawn | At the `sh` spawn every in-process `SessionLock` has been dropped (`drop(held)` precedes it), so the child inherits no lock description. |

## Stress results (64-core host)

The control was built from `git archive 119ab95` in its own target dir and checked for absence of the `spawn_guard` symbol. A first attempt with a shared target dir silently tested the new code twice, so that run was discarded.

| Binary | Mode | Failures |
| --- | --- | --- |
| head aeab3d9 | `cargo test -p ieo-supervisor --test supervisor`, sequential | 0/100 |
| head aeab3d9 | test binary direct, sequential | 0/100 |
| head aeab3d9 | 8 concurrent workers × 25 (×2 batches) | 0/400 |
| head aeab3d9 | filter `one_running a_non_regular`, sequential | 0/300 |
| control 119ab95 | test binary direct, sequential | 13/100 |
| control 119ab95 | 8 concurrent workers × 25 | 3/200 |

A sample control failure is `supervisor.rs:452:53 leftover file: Running { pid: None }`, the same signature as review 1. Head shows 0 failures in 900 runs, against about 7–13% in the control, so the fix is reproducible.

## R1–R3 re-confirmed

- R1/R2: budget.rs:272 and :284 still use `checked_add(..).is_none_or(|t| t > cap)`, and `spent()` uses `checked_add` (:138–143). These are unchanged since review 1, where M1–M4 were killed. `an_overflowing_projection_stops` passes.
- R3: `a_non_regular_claude_json_is_never_opened` is unchanged apart from the guarded `mkfifo`, so its assertions are the same as in review 1 (M5–M7 killed). It passes.

## PR body check

Accurate: the R1–R3 descriptions, "20 tests pass", "No model calls", the P1 mechanism, "every process spawn in the test file now holds a SPAWN mutex, and the lock test holds it throughout" (verified above), "0 failures in 90 runs" (consistent with 0/900 here), the product-side race note, and the R4/R5 carry-over.

## Findings

### F1. [Low] "This is added to the wiring notes" points at nothing locatable
The body says that serialising role spawns against lock release "is added to the wiring notes". Neither this PR's tree (`git grep` at aeab3d9) nor `ieo-dev-workspace` (state.json, README.md) contains such a note. If it lives in the coordinator DB outside the repo, say where in the body. Otherwise add it there, for example a line in the 02-04 wiring/handoff notes. This does not block, because the test-side fix is complete. The point is that the product race (review 1 product note, extending review 2 N9 of PR #31) must not be lost before the adapter step.

### F2. [Info] Lock-test timeout path leaks the `sleep 30` holder
If `sh` never reaches `sleep`, the assert at :489 panics before `child.kill()`, and that leaves a `sleep 30` process. This was already true before the change and only happens on a failing path. The guard is released by unwinding (poison handled), so no other test hangs. No action needed.

### F3. [Info] Serialising slows the suite slightly
The lock test now holds the guard for its whole run, including the wait for `sh`, so the three other spawning tests wait for it. The measured runtime is still about 0.01 s per run. No action needed.
