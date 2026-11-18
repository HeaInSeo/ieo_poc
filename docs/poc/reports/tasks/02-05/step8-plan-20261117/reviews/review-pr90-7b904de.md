# Review of PR #90 at 7b904de (review 2)

PR: HeaInSeo/ieo_poc #90. Head 7b904debfff601a982e63a97d130f099e5b51fd3: one commit, "settlement correction 1", on top of d971f59, which was reviewed in review 1.

## Verdict

**APPROVE.** The commit has no High or Medium findings. Review 1 raised seven Low findings; this commit addresses six of them and all six are fixed. L-5, the launcher `KeyError`, is out of scope here and is handled on the resume branch.

What I verified:
- The pin cannot be bypassed in a release build. I checked this on the actual release binary, not only by reading the code.
- The copy-location and symlink checks work. I tested both the read side and the write side.
- A missing or changed independent-check report makes the ledger unusable for every reader, including the coordinator. The failure is closed, not open.

This review has three new Low findings and two Info notes:
- **N-1:** a ledger read now depends on a repository file that can change.
- **N-2:** the ledger must be named by the same path string everywhere it is read.
- **N-3:** one test gap. It concerns the symlinked-directory branch of the copy check.

N-1 and N-2 are operational. I recommend handling them before the live settlement, either with a small code change or with an explicit line in the resume steps.

## What I ran

All runs used a new worktree at 7b904de, my own target dir and `-j 4`. `df /` showed 12G free at the start and 11G after the builds. No model calls were made. Nothing was written to the live ledger, and 58090 and 55433 were not touched.

| Check | Result |
|---|---|
| `cargo test --workspace --locked` (test DB 55432) | 44 suites, **429 passed, 0 failed**. This matches `evidence/settlement/test.txt`. |
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | ok |
| `validate_package.py` | rc 0 |
| `render_progress.py --check` | ok |
| `test_render_progress.py` | PASS |
| `cargo build --release -p ieo-supervisor --bin ieo-role-call` | ok |
| Dry runs of `settle` on scratch copies of `ledger-0205` | see the table below |
| 7 hand mutations on the new code | 6 killed, 1 survived (N-3) |
| Temporary probe tests (file deleted afterwards) | see sections (c) and (d) |

All dry runs used the real evidence copy (read-only) and the real report. `TMPDIR` pointed at a scratch dir. No test pins were set unless the table says so.

| Binary | Ledger copy | Test pin set | Result |
|---|---|---|---|
| **release** | real `s8-r1` | none | rc 0. Tokens **2 / 130 / 1,411 / 3,223**, api_requests 1, messages 1, `written: false` |
| debug | real `s8-r1` | none | rc 0, same numbers |
| release, run from the repository with a relative `--independent-check` path | real | none | rc 0, same numbers. The report path is recorded as absolute. |
| debug | record renamed to `s8-r9` (stream unchanged) | `IEO_SETTLEMENT_TEST_PINS=02-05\|s8-r9\|<session>\|*` | rc 0. The debug build honours the test pin. |
| **release** | `s8-r9` | same env pin | **rc 1**: "is not the run the decision allows to settle" |
| debug | `s8-r9` | none | rc 1, same refusal |

After every run:
- Both `records.jsonl` files had the same SHA-256 as before.
- No `children/` directory was created.
- `TMPDIR` was empty.

So the dry run writes nothing, not even a temporary file.

## Analysis

### (a) Each review-1 finding against its fix

| Review-1 ID | Fix | Correct? | Tested? |
|---|---|---|---|
| L-1: copy location and symlinks | `copy_path` gives `<ledger>/children/<attempt>/settlement-transcript.jsonl`. `read_copy` requires the recorded path to equal it exactly. The final file must be a regular file (`symlink_metadata`). `canonicalize(copy)` must equal `canonicalize(ledger)/children/…`, which catches a symlinked `children` or `children/<attempt>` directory. The writer refuses a non-regular file at the copy path and creates the copy with `create_new` and mode 0600. | Yes. My probe P1 swapped `children/s8-r1` and then `children` for symlinks to outside directories; both are refused. | Path and file symlink: yes (mutants C01 and C02 in the PR's own list). The directory branch is not tested (N-3). |
| L-2: provenance re-checked on read | `apply_bytes` now requires: `original_sha256 == sha256 ==` the copy's real hash; `decision ==` the `DECISION` constant; the report path absolute, readable, and with the recorded SHA-256 (`check_report`). | Yes | Yes. My mutations N3, N4 and N6 were all killed. |
| L-3: dry run's temporary copy | The dry run passes the bytes into `build(…, on_disk=false)`, which calls `apply_bytes`. No file is written. | Yes. Confirmed in the table above. | Yes (W04, plus the dry-run test) |
| L-4: limited to s8-r1 | `pinned()` matches budget `02-05`, attempt `s8-r1`, the session and the stream SHA-256 `252609c7…`. It is called in `eligible` and in `spare_releasable`, so even a settled run that is not pinned is not released. | Yes | Yes. My mutation N1 was killed (P01–P03). |
| L-6: the run's own limits | `apply_bytes` refuses when billable exceeds `limits.billable_room` or cache_read exceeds `limits.cache_read_room`. Missing limits also refuse. | Yes. The real R1 has room 150,000 / 1,500,000. | Yes. My mutation N5 was killed. |
| L-7: overflow | `billable()` now returns `Option` built with `checked_add`. | Yes | Yes (L07) |

### (b) Can the pin be bypassed in a release build?

No.
- `allow_test_pin` and the `IEO_SETTLEMENT_TEST_PINS` branch of `pins()` are both behind `if cfg!(debug_assertions)`.
- The workspace and crate `Cargo.toml` files set no `[profile.*]` overrides, and there is no `.cargo/config`. So a release build has `debug_assertions` off.
- RESUME_PLAN line 75 says the coordinator and caller are built as release from the merged SHA into `$D/bin/m-<SHA>/`.
- The test above shows it: the release `ieo-role-call` refused a test pin that the debug build honoured.

Remaining exposure: a debug build started with the environment variable set would honour it (Info I-1).

### (c) Canonical paths, symlinks, TOCTOU and the ledger path

I ran these as a temporary test against the library (`caller::records`):

| Probe | Result | Assessment |
|---|---|---|
| P1: `children/s8-r1` replaced by a symlink to an outside directory holding the identical copy | refused, "not a regular file inside the ledger" | ok |
| P1b: `children` itself replaced by a symlink | refused | ok |
| P2: the plan names the ledger through a **symlinked alias** of the ledger dir | refused: the recorded `ledger_copy` path does not match | fails closed. See N-2. |
| P3: plan `ledger_dir` written as `<ledger>/.` | accepted, because `Path` comparison drops `.` | ok |
| P3b: plan `ledger_dir` written as `<ledger>/../ledger` | refused, because `std::path::absolute` does not resolve `..` | fails closed. See N-2. |

**Ledger dir that is itself a symlink.** When every reader names it the same way, it is accepted: `canonicalize` is applied to both sides. When readers name it differently, it is refused (N-2).

**Relative ledger dir.** `copy_path` uses `std::path::absolute`, which resolves against the process's working directory. A writer and a reader in different working directories would therefore disagree, and the ledger would fail closed. The live coordinator's `budget_plan` uses the absolute path `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205` (EXECUTION_PLAN.md:195), which is already its real path.

**TOCTOU.** `read_copy` checks the file (`symlink_metadata` and `canonicalize`) and then reads it with `std::fs::read`, which follows symlinks. Someone with write access to the ledger could swap in a symlink between those two steps. The bytes read are still compared with the recorded SHA-256, so the usage cannot change; the most such a swap can do is point at an identical file elsewhere. On the writer side, the copy is created with `create_new` and then re-read through `apply`, so a swap there is refused and the new copy removed. Info I-2.

### (d) The report is re-read on every ledger read

Which processes read the ledger, all on the host:
- the coordinator (`ieo-coord`, a host process restarted by `coord-switch.sh`; not a container), through `delivery.rs:667-669` `records_when_idle` and `delivery.rs:833` `admit_with`, which calls `records`;
- the `ieo-role-call` CLI (`settle`, `check`, `reevaluate`, calls).

The role containers' `serve` does not read the caller ledger.

What happens when the report is missing or moved (probe P4): both `records()` and `records_when_idle()` return an error naming the report. In the coordinator:
- `idle` becomes `None`, so `limits::settle` is skipped and the earlier `assignment_run_usage` rows are kept. A missing record never lowers usage.
- `admit_with` returns an error, so the delivery is recorded as `budget_refused`.

So a missing or changed report **fails closed**. It blocks every call of the budget until the file is restored; it never admits a call. The coordinator runs on the same host, as the same user, so it can read the absolute repository path. This is safe, but it is a new operational dependency (N-1).

### (e) Regressions

- **Tests:** 429/429, one more than review 1. fmt, clippy, the validator and render all pass.
- **`spare_releasable`:** the only behaviour change is that a guard-stopped run must now also be pinned. This is stricter.
- **`reevaluate`:** passes the ledger dir through. No behaviour change.
- **Writer:** the old `write_private` path is replaced for the copy only.
- **`ieo-role-call settle`:** records the report as an absolute path.

### Hand mutations on the new code (each reverted)

| ID | Mutation | Result |
|---|---|---|
| N1 | `pinned()` always Ok | killed (:786) |
| N2 | `read_copy`: drop the canonical-path comparison | **survived** (N-3) |
| N3 | `apply_bytes`: drop `original_sha256 == sha256` | killed (:891) |
| N4 | `check_report`: drop the SHA-256 comparison | killed (:575, :898) |
| N5 | drop the run-limits check (L-6) | killed (:835) |
| N6 | `decision` only required non-null, not pinned | killed (:406, :545) |
| N7 | `read_copy`: drop the exact-path comparison | killed (:870, :406) |

I also checked the PR's own mutant list. E07 (stream digest not compared) shows SURVIVED on the rerun line, but the footnote says a changed-stream-file test was then added and E07 killed, and the test is at `tests/settlement.rs:918-928`.

### Public-repository hygiene

The diff contains only code, tests, docs and evidence summaries (test and mutant lists). There is no transcript content and no secrets.

## Findings

| ID | Sev | Location | Scenario | Fix |
|---|---|---|---|---|
| N-1 | Low | `crates/ieo-supervisor/src/settlement.rs` `check_report` (around lines 433-450); `bin/ieo-role-call.rs:95-98` | Every read of the budget ledger, by the coordinator on each delivery and by the CLI, re-reads the independent-check report at the absolute path recorded at settle time. With the RESUME command, that is a file in whichever repository checkout or worktree the operator ran from. If that checkout is switched to a branch without the file, edited by a later docs PR and pulled, or removed (for example a temporary merge worktree), the 02-05 ledger becomes unusable. Every delivery is then `budget_refused`, possibly between R1b and R2. This is fail-closed (probe P4) and recoverable by restoring the file, but it is an avoidable dependency. | Copy the report into the ledger next to the transcript copy, for example `children/s8-r1/independent-check.md` (0600, `create_new`), and check it there with the same in-ledger and symlink rules. Alternatively, add a RESUME step saying the report path must stay unchanged for the life of the budget, with a pre-delivery check that its SHA-256 still matches. |
| N-2 | Low | `settlement.rs` `copy_path` / `read_copy` (around lines 387-405); RESUME_PLAN appendix B "쓰는 명령" | `ledger_copy` must equal `std::path::absolute(plan.ledger_dir)/children/…` *textually*. If the settle plan spells the ledger differently from the coordinator's `budget_plan`, for example through a symlinked alias, with `..`, or as a relative path from another working directory, the CLI's own read-back passes but the coordinator refuses the ledger (probes P2 and P3b). | Compare canonical paths: canonicalize the recorded `ledger_copy`, check it lies under `canonicalize(ledger_dir)/children/<attempt>/`, and keep the regular-file check. Or state in RESUME that `--plan` must carry exactly the coordinator's `budget_plan.ledger_dir` (`/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205`), and add a coordinator-style read (`ieo-role-call check` with the coordinator's plan) as the step right after settling. |
| N-3 | Low | `tests/settlement.rs` (L-1 tests) | The canonical-path comparison in `read_copy` is the only guard against a symlinked `children` or `children/<attempt>` directory. My mutation N2 removed it and all tests still passed. The code itself is correct (probes P1 and P1b are refused). | Add a test that moves `children/s8-r1` (and `children`) outside the ledger and replaces it with a symlink, then expects `records()` to refuse. |
| I-1 | Info | `settlement.rs` `pins()` | A *debug* binary honours `IEO_SETTLEMENT_TEST_PINS` from its environment. Deployment is release (RESUME line 75) and the coordinator's environment is captured and restored as it is, so there is no exposure today. | Optional: say in the RESUME build step that a release build is required for the pin. |
| I-2 | Info | `settlement.rs` `read_copy` | There is a check-then-read window. It cannot change the counted usage because of the SHA-256 compare. | Optional: open with `O_NOFOLLOW` and `fstat` the open file. |

## Summary

Correction 1 correctly fixes review-1 findings L-1, L-2, L-3, L-4, L-6 and L-7, and each fix is tested.
- The settlement is pinned to 02-05 / s8-r1 / session 5c991c71 / stream 252609c7…. A release build ignores test pins; I showed this with the release binary.
- The copy must sit exactly in the ledger and must not be reached through a symlink, on both read and write.
- Both copy digests are compared, and the decision and the report are re-verified.
- The run's own limits and overflow are checked.
- The dry run writes nothing at all and reproduces 2 / 130 / 1,411 / 3,223 (billable 1,543) from the real evidence with a release build.
- Tests 429/429, fmt, clippy, the validator and render all pass. Six of my seven mutations were killed.

The new findings are Low:
- the ledger's dependency on the report file staying where it is (N-1);
- the requirement that every reader spells the ledger path the same way (N-2);
- a missing test for the symlinked-directory branch (N-3).

All three fail closed. I recommend handling N-1 and N-2 before the live settlement, in code or as explicit resume steps.

**Verdict: APPROVE.**
