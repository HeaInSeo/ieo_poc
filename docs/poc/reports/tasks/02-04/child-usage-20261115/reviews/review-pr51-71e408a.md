# Review of PR #51, round 3 (of at most 4), after correction 2

- **PR:** #51 "02-04: a child's usage from its transcript, D3 re-evaluation (user decision 2026-11-15)"
- **Head reviewed:** 71e408a088c1bdbc9a821ce99829b20629f58fa0, confirmed with `gh pr view 51 --json headRefOid,statusCheckRollup`. Base: main e03acf4 (origin/main is still e03acf4).
- **CI on the head:** all 3 checks SUCCESS (docs/poc package and progress validation; rust toolchain pin resolves; rust fmt clippy test). The rust check was still in progress at the start of this review and finished green.
- **Reviewer:** independent reviewer agent (Claude). No model calls. Nothing written to `/opt/rust/ieo_poc_runs`. Ports 58211/58212 not called. `ieo-role-call` not run against the real plan. No push, comment, merge or branch change.
- **Verdict: APPROVE.**
  - N1 (blocking in round 2) is resolved and verified on a copy of the real ledger from several directories.
  - N2–N6 are resolved or recorded.
  - Nothing remains that blocks applying the D3 correction to the real ledger or running D4.
  - The new findings (P1–P4) are low or info and can be recorded. None needs the last correction.

## What I ran

All runs used a detached worktree of the head (`<scratch>/rv51c`) with my own `CARGO_TARGET_DIR` (`<scratch>/target-review51c`).

- **fmt:** `cargo fmt --all -- --check` passed (exit 0).
- **clippy:** `cargo clippy --workspace --all-targets --locked -- -D warnings` passed (exit 0).
- **tests:** `cargo test -p ieo-evidence -p ieo-supervisor` passed (exit 0).
  - collector 37; caller 23 (one new: `reevaluation_checks_the_decision_the_kept_digest_and_the_base_session`); run_guard 26; serve 18; supervisor 20; unit tests 6 + 1.
- **01-04 crosscheck:** `crosscheck_0104.py` with the head's `ieo-evidence`: 35 streams, 0 mismatches, measured/unmeasured 32/3; new 25, resume 10; semantic 30/30.
- **Preserved evidence (read only):** `sha256sum -c SHA256SUMS.v2` gives 21/21 OK. The preserved `caller-ledger/records.jsonl` is byte-identical to the live ledger (sha256 1fcf6dfd…). The live ledger and plan were only read.
- **N1/N2 end to end on a copy:** see the next section.
- **Mutation tests:** 13 mutants, each `timeout 900 cargo test -q -p <crate> --test <file>`. The source was restored after each; `git status` afterwards showed only my throwaway example.
- **§10:** re-read the preserved child transcript (the Read tool_result, the child's text, the absence of thinking) and the PROGRESS §10 and PR body wording.
- **Cleanup:** the worktree, the target directory, the ledger copy, the crosscheck output and my scripts were removed afterwards.

## N1/N2 verification on a copy of the real ledger

**Setup.**
- I copied the preserved `caller-ledger/` and the plan into `<scratch>/r51c-w/`.
- `ledger_dir` was pointed at the copy, and the four stream paths in the records were rewritten to the copy.
- The preserved child transcript was copied to `src/child.jsonl`, so it could be moved and deleted.
- Ledger reads used a throwaway example (not committed) that calls `caller::records` and `caller::limits_for` for both roles.

**Before.** 4 runs, billable 11,627, cache_read 50,206, D3 unmeasured, both roles `Stop` ("unmeasured run(s) … no further call").

**Refusals leave nothing behind.** All three were run from `elsewhere/deep` with relative `--child-transcript` paths. Each one exited 1, the `records.jsonl` sha256 was unchanged, and no `children/` directory was created.
- the last response only: `mismatch` (8/163/35,662/546 against 10/224/35,662/2,613);
- a wrong agent id only: `not_checked`;
- no transcript: `not_checked`.

**Correction with a relative path.** From `<W>/elsewhere/deep`, I ran `reevaluate --child-transcript a13188dc0170f8901=../../src/child.jsonl`.
- It exited 0, `consistent`, 10/224/2,613/35,662.
- `children/D3/agent-a13188dc0170f8901.jsonl` was created, byte-identical to the source.
- The correction line records that **absolute** path, 25,972 bytes, and sha256 8e509473….

**Read back from 5 directories** (`/`, `<W>`, `<W>/elsewhere/deep`, the main checkout, `$HOME`):
- every read gives `records ok`: 4 runs, 26 s, billable 14,474, cache_read 85,868, no unmeasured runs;
- D3 is measured with `stop_required=false`;
- development room: billable 988,112, cache_read 9,923,199;
- guardrail room: billable 497,414, cache_read 4,990,933.
- This matches PROGRESS §5 and the round-2 preview.

**Source moved, then deleted:** reads still OK from `/tmp` and `$HOME`.

**Second reevaluate:** refused, "D3 is already corrected".

**Hand edits of the correction line.** Each edit was tested from two cwds, including the cwd where the relative path would resolve:
- `children/D3/agent-….jsonl`: refused, "transcript path … is not absolute";
- `./ledger/children/D3/…`: refused the same way;
- an absolute path to a missing file: refused, "does not hold: … No such file or directory";
- restored: OK.

**Different file already at the copy location:** refused, "already holds a different transcript". The records were unchanged.

**Real ledger.**
- `plan.ledger_dir` and every recorded stream path are absolute (`/opt/rust/ieo_poc_runs/02-04-20261115/ledger/...`).
- After the correction, every later `records()` call depends only on files inside the ledger directory: `streams/D3.jsonl` and `children/D3/agent-<id>.jsonl`.
- I found nothing that would make later calls refuse, other than moving or renaming the whole ledger directory. That was already true for the stream paths before this PR (P4).
- **D4 base.** D4's resume base is taken from the effective (corrected) D3 record's `usage.session`.
  - That field is re-derived by the same parser from the same stream, and it was already `measured` in the original line.
  - §11 item 3 requires the checkpoint at D3, which `session_cumulative_attempt == newest` needs.

## N1–N6 status

| # | Status | Evidence |
| --- | --- | --- |
| N1 (blocking) | **Resolved** | `reevaluate` copies each transcript to `children/<attempt>/` under `std::path::absolute(ledger_dir)` and records that path. `effective()` refuses a non-absolute path. Verified from 5 cwds and by edits (above). Mutants C1 and C2 killed. |
| N2 | **Resolved** | The transcript lives with the ledger. Deleting or moving the source has no effect (verified; also tested). A refused reevaluate copies nothing: the pre-check `remeasure` runs on the given files first (mutant C3 killed; verified by three CLI refusals). |
| N3 | **Resolved**, except two | F4 digest (C7), F3 base session (C8), blank decision (C9), recorded transcript hash (C10), and F5 input/cache_read/cache_creation (E1–E3) are all killed now. Still survived: the read-back after append (C6, was A10) and the "different transcript already in place" refusal (C4); see P2. |
| N4 | **Resolved** | See the §10 section below. |
| N5 | **Resolved** | The PR body has a "D3 answer `4` vs expected `3`" section with the classification, the preserved failure, the acceptance impact and a pointer to §11. |
| N6 | **Recorded** | PROGRESS §12 says `call.sh` trusts correction lines and `ieo-role-call` verifies on the next read. Acceptable (fail-closed one step later). |

## §10 (D3 answered `4`, expected `3`)

I re-checked against the preserved records, with no model call:
- the child's only Read tool_result is `1\t…first line\n2\tsecond line\n3\tthird line\n4\t`;
- the child's only answer text is `4`, with no thinking block;
- the instruction to the child does not define how lines are counted.

**Assessment.** The wording is now appropriately cautious and supported:
- "생긴 것으로 본다" (we consider the difference arose at the display stage);
- the display behaviour is stated as a single observation, not as general CLI 2.1.322 behaviour;
- that the child counted the numbered lines is marked as an inference, because the transcript holds only the answer;
- the model-error question is answered as "no basis for a plain counting error, not asserted to be free of model judgement".

**Classification.** Tool display plus test/expected-value ambiguity, with a possible model-judgement contribution not excluded. This matches the evidence and the binding instruction:
- analysed from existing records only;
- no rerun;
- expected value and failure kept.

**Failure preserved.**
- RUN_PLAN expected `3` is unchanged.
- D3 is recorded as "답 기대값 불충족" (expected answer not met).
- The PR body repeats this.

**Acceptance impact and D4–D6.**
- The acceptance impact is stated with the right hedge: the contract item §10 (3)(c) is checked at D4; the failure goes into the acceptance report; the decision is left to review and the user.
- §11 lists the D4–D6 resume conditions concretely: approval/CI/merge/main CI; the correction applied with the budget equal to §5; a restart with the merged binary under the guard with checkpoint S at D3; the 7-day observation (until 2026-11-21); no reset, retry or extra billing.

## Mutation results (head 71e408a)

| Mutant | Result |
| --- | --- |
| E1/E2/E3 F5: input / cache_read / cache_creation not compared | killed (all three) |
| C1 `effective`: absolute-path check removed | killed |
| C2 `reevaluate`: store the source paths instead of the ledger copies | killed |
| C3 `reevaluate`: pre-check `remeasure` on the given files removed | killed |
| C4 `reevaluate`: a different transcript already at the copy location accepted | **survived** (P2) |
| C5 `reevaluate`: `std::path::absolute` removed | survived (equivalent while `ledger_dir` is absolute, as in the real plan and the tests) |
| C6 `reevaluate`: read-back `records(plan)?` after append removed | **survived** (P2) |
| C7 `remeasure`: F4 recorded child digest check removed | killed |
| C8 `remeasure`: F3 base-session check removed | killed |
| C9 `reevaluate`: blank-decision check removed | killed |
| C10 `effective`: kept vs recorded `child_transcripts` comparison removed | killed |

## New findings (none blocking)

### P1 · Low · non-blocking (record) · `crates/ieo-supervisor/src/caller.rs` `remeasure`/`reevaluate`: transcripts for agents absent from the stream are accepted and recorded

**Problem.**
- `parse_with_children` looks transcripts up by the stream's `agentId` and ignores extra keys.
- `reevaluate --child-transcript <real>=… --child-transcript zz=<same file>` exited 0 and copied both files (`agent-zz.jsonl`). The correction lists both.
- An agent id containing path separators (`../../../escape`) was refused only because the open failed ("transcript copy: No such file or directory"). That refusal left an empty `children/D3/` behind.
- With BTreeMap ordering, a valid id that sorts before the bad one would leave its copy behind.
- No write outside the ledger is possible: `agent-<id>.jsonl` needs a directory named `agent-…` to exist, and nothing creates one.

**Impact.**
- None on budget or measurement: extra entries are not used, and leftover identical copies are accepted on a retry.
- It is metadata noise, and it is a gap in the round-2 claim that a refusal leaves nothing behind.

**Fix (later or record).** Refuse `--child-transcript` ids that are not an `agentId` in the run's stream, or that contain `/`, `\` or `..`, before any copy.

**Operator note for the real application:** pass exactly one transcript, `a13188dc0170f8901=<absolute path of the preserved subagents/agent-a13188dc0170f8901.jsonl>`.

### P2 · Low · non-blocking · tests: two checks of correction 2 untested

- **C4:** with the "already holds a different transcript" refusal removed, the second `remeasure` still catches a non-reconciling file. Only a different file that also reconciles slips through.
- **C6:** with the read-back after append removed, a ledger left unreadable by the append is only discovered at the next call. That is still fail-closed.
- **Fix:** add a test that pre-creates a different `children/d2/agent-c1.jsonl`. C6 is hard to reach and can be recorded.

### P3 · Info · file modes of the copies

`children/D3/` is created as 775 and the copy as 664 (umask 002), the same as the existing `streams/`. The run directory `/opt/rust/ieo_poc_runs/02-04-20261115` and its parent are 0700, so this is not exposed. No action.

### P4 · Info · the ledger is tied to its absolute location

Correction paths and stream paths are absolute, so moving or renaming the ledger directory would make every read refuse. This was already true for streams before this PR. PROGRESS §7/§11 could say that the ledger directory must not be moved while 02-04 is open. Record only.

## Before applying the D3 correction to the real ledger, and before D4

- **Nothing blocking remains.**
- **Apply the correction** with the merged binary (absolute transcript path, a single agent; P1).
  - It should reproduce exactly what the copy produced: `consistent`, 10/224/2,613/35,662, effective 4 runs, billable 14,474, cache_read 85,868, 8 runs remaining.
  - The copy goes to `/opt/rust/ieo_poc_runs/02-04-20261115/ledger/children/D3/agent-a13188dc0170f8901.jsonl`.
- **Then follow §11:** restart with the merged binary under the guard, keep checkpoint S at D3, and resume D4 by 2026-11-21.
- **D4 under F5.** A `/compact` resume stays measurable (L3 replay unchanged), and E1–E3 now prove all four fields are compared.
