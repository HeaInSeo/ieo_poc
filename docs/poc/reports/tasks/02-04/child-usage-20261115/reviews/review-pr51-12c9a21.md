# Review of PR #51, round 2 (of at most 4), after correction 1

- **PR:** #51 "02-04: a child's usage from its transcript, D3 re-evaluation (user decision 2026-11-15)"
- **Head reviewed:** 12c9a2145e10146e39377ab7d4835563c6454a07, confirmed with `gh pr view 51 --json headRefOid,statusCheckRollup`. Base: main e03acf4 (the merge base; origin/main is still e03acf4).
- **CI on the head:** all 3 checks SUCCESS:
  - docs/poc package and progress validation;
  - rust toolchain pin resolves;
  - rust fmt clippy test.
- **Reviewer:** independent reviewer agent (Claude). No model calls. Nothing was written to `/opt/rust/ieo_poc_runs`. No push, comment or merge.
- **Verdict: CHANGES REQUESTED.**
  - **One blocking finding, N1:** a relative `--child-transcript` path is stored as given.
    - The correction then reads back only from the directory it was written from.
    - On the real ledger, that would make every later ledger read fail from any other directory, which blocks D4.
    - The ledger is append-only and needs a user decision to edit, so the fix should go in the code before the real D3 correction is applied.
    - The fix is small.
  - Everything else is resolved or honestly recorded, or is a non-blocking test gap or wording issue.

## What I ran

All runs used a detached worktree of the head (`<scratch>/rv51b`) with my own `CARGO_TARGET_DIR`.

- **fmt:** `cargo fmt --all -- --check` passed (exit 0).
- **clippy:** `cargo clippy --workspace --all-targets --locked -- -D warnings` passed (exit 0).
- **tests:** `cargo test -p ieo-evidence -p ieo-supervisor` passed (exit 0).
  - collector: 37;
  - caller: 22;
  - run_guard: 26;
  - serve: 18;
  - supervisor: 20;
  - unit tests: 6 + 1.
  - The committed `cargo-test.log` (at 3473487) reports 286 passed, 0 failed. The head only adds that log.
- **01-04 crosscheck:** `crosscheck_0104.py` with the head's `ieo-evidence`.
  - 35 streams, 0 mismatches, measured/unmeasured 32/3.
  - The output is identical to the committed `evidence/crosscheck-0104.txt`, ignoring comment lines.
  - L3 (the real `/compact` resume) is still measured: share 2,020/1,354/2,761/1,019 against parent 0/0/0/0 with no children, attribution `mismatch`.
- **Preserved evidence (read only):** `sha256sum -c SHA256SUMS.v2` gives 21/21 OK. The preserved `caller-ledger/records.jsonl` is byte-identical to the live ledger (sha256 1fcf6dfd…). The live ledger was only read.
- **D3 re-evaluation on a copy** of the preserved ledger. I copied the plan with `ledger_dir` pointed at the copy and the stream paths rewritten to the copy.
  - **Before:** D3 unmeasured, spent billable 11,627, cache_read 50,206, and both roles Stop.
  - **`ieo-role-call reevaluate`:** exit 0, `consistent`, 10/224/2,613/35,662.
  - **After, read back through `caller::records`** with a throwaway example that is not committed:
    - 4 runs, 26 s as counted, billable 14,474, cache_read 85,868, no unmeasured runs;
    - development limits: 594 s, billable room 988,112, cache_read room 9,923,199;
    - guardrail limits: 497,414 / 4,990,933.
    - This matches PROGRESS §5.
  - **A second reevaluate** is refused ("already corrected").
- **F1 probes on the corrected copy:** 14 hand edits; see the F1 row below.
- **Relative-path probe** for N1.
- **Mutation tests:** 15 mutants, each `timeout 900 cargo test -q -p <crate>`. The source was restored after each one; see the table below.
- **§10:** read the preserved child transcript, the parent transcript, `summary-D3-initial.json` and the fixture README.
- **Cleanup:** the worktree, target dir and scratch copies were removed afterwards.

## F1–F10 status

| # | Status | Evidence |
| --- | --- | --- |
| F1 | **Resolved** | `effective()` now calls `remeasure(&runs[pos], &runs[..pos], transcripts-from-correction)` and applies the **re-measured** usage, not the line's. See the probe results after this table. |
| F2 | Resolved | Tests were added for the last-response check and the "; " reason check. Both were killed in round 1's M2 and M8 equivalents: the new tests fail when those checks are removed. |
| F3 | Fixed, **untested** | The base session id now comes from the base record, and a base from another session is refused. Mutant A8, which removes this check, survives. |
| F4 | Fixed, **untested** | A recorded child digest must equal the supplied transcript's digest. Mutant A7 survives. The existing "last-only" test is refused by F4 before the mismatch check, so it does not tell the two apart. |
| F5 | **Resolved** | See the F5 notes below. |
| F6 | Resolved | See the F6 notes below. |
| F7 | Resolved | `SHA256SUMS.v2` lists 21 files and all 21 verify. The original file is left as it was. |
| F8 | Recorded honestly (wording only, fail-closed) | — |
| F9 | Resolved | A `BTreeSet` replaces `dedup`. |
| F10 | Partially resolved, as described | An empty decision is refused when the ledger is read (mutant A5 is killed). It is also refused in `reevaluate`, but mutant A9 survives: the test calls `reevaluate(&p, "d1", …, " ")`, which fails anyway because d1 is not a stopped run. Any non-empty text is accepted (probe e). That is acceptable; it is a reference, not a check. |

**F1 probe results.** The questions were: can an edited correction change the charge, clear a stop, or apply to a different run, and does a legitimate correction survive normal operations?

- **Refused (records() errors):**
  - zeroed `after.usage.run`: "differs";
  - an extra stop reason on the original D3 line: "not a run stopped only for unmeasured usage";
  - `corrects` set to D2: digest mismatch;
  - `corrects` set to D2 together with D2's digest: not a usage-only stop;
  - `after.stop_required: true`;
  - the correction placed before its run: "unknown or later run";
  - a duplicate correction;
  - the original D3 line edited to measured;
  - the transcripts removed from the correction: `not_checked`;
  - a transcript without the first response, with its recorded digest updated: `mismatch`;
  - the transcript file moved;
  - the D3 stream moved.
- **Accepted, harmlessly:** edits to fields of `after.usage` other than `run`, and a changed decision text. These values are not applied; the applied usage is re-measured.
- **Later runs appended** after the correction (a fake D4 resuming D3 and a G2): the correction still applies and the budget adds normally.
- **Answer:** an edited correction cannot change the charge, clear a stop with other reasons, or move to another run. A legitimate correction survives appends.
- **But** it depends permanently on its transcript path being readable (see N1 and N2).
- **Untested parts:** mutant A2 (the `json!(kept) != child_transcripts` comparison removed) survives. That comparison guards only the correction's own metadata, so it is low.

**F5 notes**

- Under compaction, a resumed share must be at least parent + children in every field, and both parts must be confirmed.
- The 01-04 L3 replay is unchanged (measured). D4 is the same shape (`/compact` resume, parent likely about 0 and no child), so it should measure.
- It blocks nothing that should pass, given that compaction adds calls and never removes them.
- The residual risk is fail-closed: if D4's `/compact` reported parent usage larger than the session increase in some field, D4 would stop as unmeasured, but nothing would be overspent.
- Mutants B1 and B2 are killed. B3, B4 and B5 (dropping the comparison of cache_creation, cache_read or input) survive, because the test perturbs only output.

**F6 notes**

- **`call.sh`:** the restart guard drops correction lines and treats corrected runs as not stopped. It does not verify the correction, but `ieo-role-call` does on its next read, so a forged correction still fails closed one step later. Acceptable.
- **`summarize.py`:** on the corrected copy it shows the original `usage_run` (unmeasured) plus a `correction` block (measured, `consistent`, `stop_required` false). That is correct as history.

## §10 assessment (D3 answered `4`; RUN_PLAN expected `3`)

Verified independently against the preserved records, with no model call:

- **README:** 53 bytes, `fixture0204 README first line\nsecond line\nthird line\n`. That is 3 newline-terminated lines with no blank line, and `wc -l` gives 3.
  - Same sha256 (12e7986b…) as the repository fixture.
  - The live copy's mtime (17:40:40Z) is before D3 (17:42Z).
- **Prompts:**
  - the parent prompt is `prompts/D3.txt`;
  - the `reader` definition is `agents.json`;
  - the parent's instruction to the child appears in the transcript exactly as quoted.
  - None of them defines how to count.
- **What the Read tool returned:** the child's single Read `tool_result` is exactly `1\t…first line\n2\tsecond line\n3\tthird line\n4\t`. This is a numbered empty 4th line after the final newline.
- **Answers:**
  - the child's only text is `4`, with no thinking or other reasoning recorded;
  - the parent relayed `4` unchanged;
  - `summary-D3-initial.json` confirms Agent(`reader`) followed by Read of README.md under that agent, `tamper: null`, exit 0.

**Where the difference arises.** The difference arises at the Read display stage, as §10 says. The classification (tool display plus prompt/expected-value ambiguity) is supported by the evidence.

**Overstated (non-blocking, N4):**
- "자식은 그 표시의 번호 줄을 셌다" (the child counted the numbered display lines) is an inference. The transcript records no reasoning, only the final `4`. It is the explanation most consistent with the evidence, not an observation.
- "모델의 답 오류는 아니다" (not a model error) is too categorical. A model could reasonably have discounted the empty trailing line. Counting it is exactly where the ambiguity lies, so a model contribution cannot be excluded.
- "CLI 2.1.322의 Read는 … 붙여 보여 준다" (CLI 2.1.322's Read appends a numbered line) generalises from one observation, which should be stated.

**Preservation and acceptance impact**
- The RUN_PLAN D3 expected answer `3` is unchanged on the head (line 49). The failure is kept as "답 기대값 불충족" (expected answer not met).
- The acceptance-impact statement is appropriately cautious: "본다" (we consider), and the decision is left to review and the user.
- It states the D4 note correctly: the D4–D6 prompts do not count lines.
- **Minor gap:** the PR body does not mention the D3 answer failure or §10 at all (N5).

## Mutation results (head 12c9a21)

| Mutant | Result |
| --- | --- |
| A1 `effective`: run-usage comparison removed | killed |
| A2 `effective`: `kept` vs `child_transcripts` comparison removed | **survived** (low: metadata only) |
| A3 `effective`: no remeasure, trust the correction | killed |
| A4 `effective`: `after.stop_required != false` check removed | survived (equivalent: the stop is forced false anyway) |
| A5 `effective`: empty-decision check removed | killed |
| A6 `effective`: `earlier` = all runs, not `runs[..pos]` | survived (practically equivalent: the base is found by id) |
| A7 `remeasure`: F4 recorded-digest check removed | **survived** |
| A8 `remeasure`: F3 base-session check removed | **survived** |
| A9 `reevaluate`: blank-decision check removed | **survived** |
| A10 `reevaluate`: read-back after append removed | **survived** |
| B1 F5: `below` never applied | killed |
| B2 F5: unconfirmed part treated as not below | killed |
| B3/B4/B5 F5: cache_creation, cache_read or input not compared | survived |

## New findings

### N1 · Medium · **BLOCKING** (before the D3 correction is applied to the real ledger) · `crates/ieo-supervisor/src/caller.rs` `remeasure` / `reevaluate`: relative transcript paths are stored as given

**Problem.** `remeasure` stores the transcript entry as `json!({"agent_id": a, "path": p, ...})`, with `p` exactly as passed on the command line. Since correction 1, `effective()` re-reads that path on every `records()` call.

**Reproduced on the ledger copy:**
1. I ran `cd <ledger copy> && ieo-role-call reevaluate ... --child-transcript <agent>=child.jsonl`. It exited 0, and the read-back passed because the cwd was the same.
2. Reading the same ledger from another directory fails with `RECORDS ERR: correction of D3 does not hold: transcript child.jsonl: No such file or directory`.

**Consequence on the real ledger:**
- every `ieo-role-call` call (D4–D6) and every budget check would refuse unless run from that exact directory;
- the correction line cannot be removed without a hand edit, which needs a user decision.

The PROGRESS §8 command shows `--child-transcript <보존 전사>` (the preserved transcript) and does not require an absolute path. The stream path is safe because the caller already records it as absolute.

**Fix (pick one):**
- **Code (preferred):** in `remeasure`/`reevaluate`, `std::fs::canonicalize` each transcript path before reading and storing it, or refuse a relative path. Add a caller test that reevaluates with a relative path and reads back from another cwd.
- **Procedure (minimum):** make §8 and §11 item 2 require an absolute path.

### N2 · Low · non-blocking · PROGRESS §8 / §11: the ledger now depends permanently on the transcript location

**Problem.** After the correction, the real ledger is readable only while the named transcript file exists unchanged at that path. Today that path is the preserved directory or wherever the operator points.

**Fix.**
- Copy the preserved transcript into the ledger's own `children/D3/` (as new runs do), and pass that absolute path.
- Or state in §8 and §11 that the named file must never be moved or changed.

### N3 · Low · non-blocking · tests: untested checks

Add tests that kill these mutants:
- **A7 (F4):** pass a transcript whose digest differs from a run's recorded `children[].sha256`, but which would otherwise reconcile. For example, add a trailing duplicate final entry.
- **A8 (F3):** use a base record from another session.
- **A9:** call `reevaluate` with a blank decision on the stopped run `d2`, not `d1`.
- **A10:** test the read-back after append.
- **B3–B5:** perturb input, cache_read and cache_creation in the F5 test, not only output.
- **A2 (optional):** an edited `child_transcripts[].sha256` with the file unchanged.

### N4 · Low · non-blocking · PROGRESS §10 wording

**Fix.**
- Say the child's counting of the numbered display lines is the most consistent explanation, and that the transcript records no reasoning.
- Replace "모델의 답 오류는 아니다" (not a model error) with: primarily tool display plus prompt/expected-value ambiguity; a model contribution (not discounting the empty trailing display line) cannot be excluded.
- Note that the Read display behaviour is from one observation.

Keep the expected value and the failure as they are.

### N5 · Info · PR body

The body says nothing about the D3 answer `4` versus the expected `3`, or §10. Add one line, so the merge record carries the preserved failure and its classification.

### N6 · Info · `call.sh` restart guard

It trusts a correction line without verifying it, and `ieo-role-call` verifies on the next read. This is fail-closed one step later. If you want, run a read-only `ieo-role-call` check before restarting.

## Before applying the D3 correction to the real ledger, and before D4

- **Fix N1 first:** canonicalize or refuse relative paths, or at minimum mandate an absolute path. Preferably also follow N2: copy the transcript into the ledger's `children/D3/` and pass that absolute path.
- **Then apply the correction:** applying it with the merged binary reproduces the copy result exactly: `consistent`, 10/224/2,613/35,662, effective budget 4 runs, 14,474 / 85,868, 8 runs remaining.
- **D4:** F5 as now implemented keeps a `/compact` resume measurable (L3 replay unchanged) and fails closed otherwise. No further blocker for D4 besides N1.
