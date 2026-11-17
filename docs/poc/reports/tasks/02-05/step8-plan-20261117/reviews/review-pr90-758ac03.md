# Review of PR #90 at 758ac03 (review 3)

PR: HeaInSeo/ieo_poc #90. Head 758ac035d8f2e89bb5ba67078391a22e6b629fd6: one commit, "settlement correction 2", on top of 7b904de (review 2).

## Verdict

**APPROVE.** There are no High or Medium findings.

The three Low findings from review 2 are fixed and tested:
- **N-1:** the ledger keeps its own copy of the independent-check report. The original report is provenance only and may disappear.
- **N-2:** there is one shared check. The ledger directory is compared by its resolved path, and nothing inside the ledger may be a symlink.
- **N-3:** the mutant that survived in review 2 is now killed.

This review raises one new Low finding (O-1) and two Info notes (O-2, O-3). None of them is a correctness or counting issue. O-1 is a small mismatch between the docs and the writer about empty directories left behind after a refusal. It can be fixed in the docs alone, and it does not need the last correction round.

## What I ran

All runs used a new worktree at 758ac03, my own target directory and `-j 4`. `df /` showed 12G free at the start and 11G after the builds. No model calls were made. Nothing was written to the live ledger. 58090 and 55433 were not touched.

| Check | Result |
|---|---|
| `cargo test --workspace --locked` (test DB 55432) | 44 suites, **430 passed, 0 failed** |
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | ok |
| `validate_package.py` | rc 0 |
| `render_progress.py --check` | ok |
| `test_render_progress.py` | PASS |
| `cargo build --release -p ieo-supervisor --bin ieo-role-call` | ok |
| **Release** dry-run `settle` on a scratch copy of `ledger-0205`, with no test pins and the real evidence copy and report | three plans, each naming the ledger differently (real path, a **symlink alias**, `real/../real`): all rc 0, **2 / 130 / 1,411 / 3,223**, 1 request, 1 message, `written: false`. All three record the same paths, which are the resolved ones: `…/real/children/s8-r1/settlement-transcript.jsonl` and `…/independent-check.md`. `find -newer` over the ledger copy and `TMPDIR` found **nothing created**. |
| My 5 hand mutations on the new code | 5 of 5 killed (see below) |
| Temporary probe test (deleted afterwards) | see sections N-2 and O-1 |

## Analysis

### N-1: the report is copied into the ledger

**At settle time** (`caller::settle`):
- The report is read once from `independent_check.evidence`. `ieo-role-call` makes this an absolute path and records its SHA-256 from its own read.
- If the file changes between the binary's hash and this read, `check_report` in `build` refuses.
- The copy is written with `write_in_ledger` to `<resolved ledger>/children/s8-r1/independent-check.md`: `create_new`, mode 0600, directories 0700. An existing regular file must hold the same bytes, and any symlink is refused.
- On any failure in `build`, every file this call created is removed (mutation X3 confirms the test).
- The dry run reads the given bytes and writes nothing.

**On every read** (`apply`), `read_in_ledger(…, REPORT_NAME, independent_check.ledger_copy, independent_check.sha256)` runs. `check_report` then compares the recorded digest with those ledger bytes. `independent_check.evidence` must still be absolute, but it is never read again.

**Tests:**
- If the original report is deleted, the ledger still reads.
- If the ledger copy is changed, the read is refused with "SHA-256".
- If the ledger copy is removed, the read is refused with "missing".
- When the copy is restored, the ledger reads again.

**My probes:**
- `independent_check.ledger_copy` dropped: refused.
- `independent_check.ledger_copy` made relative: refused.
- `independent_check.ledger_copy` pointed at the transcript copy: refused (not this name's path).

### N-2: one shared check, and the stated rule

**One function for every reader and the writer.**
- `child_path` gives `canonicalize(ledger_dir)/children/<attempt>/<name>`.
- `read_in_ledger` is the only reader. It requires an absolute recorded path, then runs `no_symlink_in_ledger`: `symlink_metadata` on `children` and `children/<attempt>` (each must be a directory) and on the file (it must be a regular file). Then `canonicalize(recorded) == want`, then the SHA-256 check.
- The coordinator uses this same function. It reads through `caller::records` (`records_when_idle` at `delivery.rs:667` and `admit_with` at `delivery.rs:833`). That calls `effective`, then `settlement::apply(…, &plan.ledger_dir)`, then `read_in_ledger`.
- The writer (`write_in_ledger`) and the dry run use the same `child_path`.

**The stated rule** is in RESUME_PLAN appendix B and contract item 32: "the ledger directory is compared by its resolved (canonical) path; nothing inside it may be a symlink". The code matches it:

| Spelling or setup | Result |
|---|---|
| Plan through a symlink alias of the ledger dir (test, and my release dry run) | accepted, same resolved paths |
| `ledger/../ledger` (test, and my release dry run) | accepted, same resolved paths |
| Recorded `ledger_copy` written through an alias (probe b) | accepted. It resolves to the same file, and the bytes are still hashed. |
| Recorded copy pointing outside the ledger at an identical file (probe c) | refused |
| `children/s8-r1` replaced by a symlink (test) | refused |
| `children` replaced by a symlink (test) | refused |
| Writer with a symlinked `children/s8-r1` (test) | refused, and nothing written through the link |

**Relative `ledger_dir`.** A relative ledger dir resolves against the reading process's working directory. That is the same rule `records.jsonl` itself already follows, so the reader is simply looking at a different ledger, and the result is consistent. The live coordinator uses an absolute path.

**TOCTOU.** `read_in_ledger` checks with `symlink_metadata` and `canonicalize`, then reads with `std::fs::read(&want)`, which follows symlinks. A swap inside that window needs write access to the ledger. Even then, the bytes read must match the recorded SHA-256, so the counted usage cannot change. On the writer side, a swap after the check is caught by the read-back in `build`, which then removes the created file. This is acceptable for a fail-closed design. Opening with `O_NOFOLLOW` and calling `fstat` would close the window (O-3).

### N-3: the surviving mutant is killed

There is a new test, `the_ledger_is_one_place_however_it_is_spelled_and_has_no_symlink_inside`. It covers:
- a symlinked `children/s8-r1`;
- a symlinked `children`;
- the writer through a symlinked directory.

My equivalents of the review-2 survivor are now killed: X1 accepts a symlinked directory, and X2 drops the resolved-path comparison.

The PR's own list marks one mutant, N05 ("writer accepts another file"), as equivalent. I agree. An existing file with different bytes is not one this call created, and `build`'s read-back through `read_in_ledger` refuses it on its SHA-256. So nothing is appended, and the file is neither overwritten nor removed.

### My hand mutations (each reverted)

| ID | Mutation | Result |
|---|---|---|
| X1 | `no_symlink_in_ledger`: accept any type for `children` and `children/<attempt>` | killed (:987) |
| X2 | `read_in_ledger`: drop `canonicalize(recorded) == want` | killed (:870) |
| X3 | `settle`: do not remove the created copies on failure | killed (:460) |
| X4 | `child_path`: `std::path::absolute` instead of `canonicalize` | killed (:977) |
| X5 | `no_symlink_in_ledger`: accept any type for the file itself | killed (:877) |

### Regressions

- 430/430 tests pass. That is one more than review 2: the new N-2/N-3 test.
- fmt, clippy, the validator and render all pass.
- The pin, the limits, the lower bound, overflow, and the counted-once and stays-failed properties are unchanged from review 2. Their tests pass, and the release dry run reproduces the figures.
- The settlement line has one new field, `independent_check.ledger_copy`. `apply` requires it, so a line written by an earlier head of this PR would be refused. No settlement has been written anywhere, so this has no effect.

## Findings

| ID | Sev | Location | Scenario | Fix |
|---|---|---|---|---|
| O-1 | Low | `crates/ieo-supervisor/src/settlement.rs` `write_in_ledger` (`DirBuilder…create(dir)` before `no_symlink_in_ledger`); `caller.rs` `settle` cleanup; RESUME_PLAN appendix B "어떤 거절에도 … 아무것도 쓰지 않는다" | (1) Suppose the settlement is refused after the copies are written, for example because the report digest does not match (probe g). The created files are removed, but empty `children/` and `children/s8-r1/` directories stay in the ledger. (2) If `children` is a pre-existing symlink to an outside directory (probe f), the writer refuses, but `create_dir_all` has already created an empty `s8-r1` directory **outside** the ledger, through the link. No file content is ever written in either case. The docs say nothing is written on any refusal. | Either state in appendix B that refused settles can leave empty directories behind, or (a) run `no_symlink_in_ledger` on `children` before creating directories and create them one level at a time, and (b) remove the directories this call created on failure. The digest check of the report bytes could also move before the copies are written. |
| O-2 | Info | `caller.rs` `settle` | An `append` failure after the copies are written leaves the copies in place. This is the same as in earlier heads. They are harmless, because a retry reuses identical bytes. | none |
| O-3 | Info | `settlement.rs` `read_in_ledger` | There is a window between the checks and the read (see N-2 above). It cannot change the counted usage. | Optional: `O_NOFOLLOW` and `fstat`. |

## Summary

Correction 2 fixes N-1, N-2 and N-3 correctly, and each fix is tested:
- The report now lives in the ledger, and the original may vanish.
- There is one resolved-path check, shared by the writer, the CLI and the coordinator. It accepts the ledger directory under any name and refuses symlinks inside the ledger.
- The review-2 survivor is killed.

Other results:
- All 430 tests pass, and fmt, clippy, the validator and render are clean.
- All five of my mutations were killed.
- The **release** dry run reproduces 2 / 130 / 1,411 / 3,223 under three different ways of naming the ledger, and creates nothing.

The only new finding, O-1, is about empty directories left after a refusal. It can be resolved by changing the docs. **Verdict: APPROVE.**
