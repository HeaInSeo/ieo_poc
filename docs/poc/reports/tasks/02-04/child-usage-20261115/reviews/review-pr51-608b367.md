# Review of PR #51, round 1 (of at most 4)

- **PR:** #51 "02-04: a child's usage from its transcript, D3 re-evaluation (user decision 2026-11-15)"
- **Head reviewed:** 608b367185671743411bfe6ba2c91386cbf3d4f7, confirmed with `gh pr view 51 --json headRefOid`. Base: main e03acf4.
- **Reviewer:** independent reviewer agent (Claude). No model calls were made.
- **Verdict: APPROVE.** Nothing blocks merging or applying the D3 correction to the real ledger. F1 and F2 should be fixed in a follow-up (or in this PR if another round happens anyway). F5 should be decided before D4, which is the `/compact` run.

## What I ran

All of this ran in a detached worktree of the head under my scratch directory, with my own `CARGO_TARGET_DIR`.

- **fmt:** `cargo fmt --all -- --check` passed (exit 0).
- **clippy:** `cargo clippy --workspace --all-targets --locked -- -D warnings` passed (exit 0).
- **tests:** `cargo test -p ieo-evidence -p ieo-supervisor` passed (exit 0), including:
  - collector: 35 tests;
  - caller: 20 tests;
  - 26 + 18 + 20 + 6 + 1 tests in the other suites.
- **01-04 crosscheck:** `crosscheck_0104.py` with the head's `ieo-evidence` binary.
  - Result: 35 streams, 0 mismatches, run measured/unmeasured 32/3.
  - The output is identical to the committed `evidence/crosscheck-0104.txt`, ignoring comment lines.
- **Independent recomputation** from the private preserved evidence, read only.
  - The preserved files match `SHA256SUMS`: 21 of 22 entries verify. The 22nd entry is `SHA256SUMS` itself (see F7).
  - The preserved ledger is byte-identical to the live ledger (same sha256). The live ledger was unchanged after my work.
- **D3 re-evaluation on a copy** of the preserved ledger, with a copied plan whose `ledger_dir` points at the copy.
  - Ran `ieo-role-call reevaluate` with the preserved child transcript, then a second time.
  - Computed `budget::spent` and `limits_for` on the effective records with a throwaway example binary (not committed).
- **Forgery probes** on further copies with hand-edited correction lines (see F1).
- **Mutation tests**, each one `timeout 900 cargo test -q -p <crate>` on a temporary source edit:

| Mutant | Result |
| --- | --- |
| M1: resume `not_checked` no longer withdrawn | killed (2 tests) |
| M2: transcript last response ≠ `tool_use_result.usage` check removed | **survived** |
| M3: conflicting final entries accepted | killed |
| M4: response with only in-progress entries accepted | killed |
| M5: line of another agentId accepted | killed |
| M6: identical repeated finals summed | killed |
| M7: correction stream-digest check removed (`effective`) | killed |
| M8: `reevaluate` "only usage reason" (`contains("; ")`) check removed | **survived** |
| M9: `reevaluate` kept-stream digest check removed | killed |
| M10: `reevaluate` "already corrected" check removed | killed |

## 1. Cause analysis: confirmed

I recomputed everything from the preserved D3 stream, the D2 stream and the child transcript. Order is input/output/cache_creation/cache_read.

- **Parent `result.usage` (D3):** 6/160/456/33,595.
- **Child transcript:** 2 assistant entries, two distinct `message.id`s, both final.
  - `tool_use`: 2/61/2,067/0.
  - `end_turn`: 2/3/90/2,067.
- **`tool_use_result.usage`:** 2/3/90/2,067 with `totalTokens` 2,162. This equals the child's last response only.
- **Child entry in the live stream:** 2/16/2,067/0. Output 16 is a streaming lower bound, against 61 final.
- **modelUsage:** D2 is 8/111/8,922/41,139 and D3 is 18/335/11,535/76,801.
  - D3 is in the same session as D2 (D1's session), and the recorded `resume_base_attempt` is D2.
  - D3 minus D2 is **10/224/2,613/35,662**.
- **Parent + both child responses:** 6+2+2 / 160+61+3 / 456+2,067+90 / 33,595+0+2,067 = **10/224/2,613/35,662**. All four fields reconcile.
- **The old aggregation** (parent + last response) gave 8/163/546/35,662. The missing first response (2/61/2,067/0) is exactly the difference.
- **Charge:** billable 2,847, cache_read 35,662.

The original D3 record says `mismatch`, unmeasured, `stop_required`, stop_reason `usage unmeasured: "…"`, and `children[0].source = tool_use_result.usage`. That matches the PR description.

## 2. Collector: correct and fail-closed for resumed runs

**`child_transcript_total`**
- Counts only final entries (`stop_reason` non-null), once per `message.id`.
- Refuses each of these (M3–M6 confirm the tests catch them):
  - unreadable lines;
  - missing ids;
  - responses with no final entry;
  - conflicting finals;
  - lines from another agentId;
  - an empty transcript.

**`parse_with_children`**
- Never uses `tool_use_result.usage` as a total; it is kept only as `reported_last_response`.
- Never uses streaming snapshots: `partial_observations` exist only when there is no result event, and they are not used for children.
- With no transcript the child is unmeasured.

**The resume rule**
- `mismatch` or `not_checked` without a compact boundary → unmeasured. This is right.
- It does not accept every mismatch, because equality in all four fields is required.

**New-session runs**
- The share is still the session's modelUsage, so a child without a transcript only turns attribution into `not_checked`. This is T23 in the replay.
- The 01-04 replay is unchanged at 32/3 with 0 mismatches.

## 3. Role service and caller

- **Double counting:** a correction cannot double count.
  - Corrections are partitioned out of the runs before `spent`.
  - A second correction is refused by `reevaluate`, and `effective` refuses the whole ledger if one exists.
  - On the copy, effective records give runs 4, 26 s (as counted), billable 14,474, cache_read 85,868, and no unmeasured runs.
  - Development limits: 594 s, billable room 988,112, cache_read room 9,923,199.
- **Original record:** preserved. The line is not rewritten, and the effective view carries `before_correction` and `corrected_by`.
- **Base:** the D3 base used by `reevaluate` is the `resume_base_attempt` recorded at run time (D2), which is the run actually resumed.
- **Stream:** the kept stream's digest is checked (killed by M9), and the correction records it.
- **Next resume (D4):** its base will be the corrected D3 record, whose `usage.session` is unchanged (same stream).
- **Exposure:** what can go wrong is limited to hand-edited correction lines (F1) and two untested guards (F2).

## 4. Fixture

`crates/ieo-evidence/tests/fixtures/d3-child-usage/` holds only numbers and structure:
- ids are renamed (`session-1`, `agent-1`, `msg-n`, `toolu-n`, `model-1`);
- there is no text, no path and no real session, agent or tool id;
- the only non-token values are the CLI version, the reported costs (0.0409279 and 0.0539706) and rate-limit status strings.

Its numbers match the originals:
- parent assistant snapshots: 4/16/57/16,769 and 2/3/399/16,826;
- child snapshot: 2/16/2,067/0;
- `tool_use_result`: 2/3/90/2,067 with `totalTokens` 2,162;
- `result.usage`, modelUsage and both costs;
- D2's session totals;
- both transcript responses.

It drops system events and the inner Read `tool_result`; neither affects usage. Grepping the PR for the real session id, agent id, tool_use id and the run directory path found nothing.

## 5. Docs and evidence

**Budget**
I recomputed the PROGRESS §5 figures from the ledger. They are exact:

| Item | Figure |
| --- | --- |
| Runs | 4 |
| Seconds, measured | 4.121 + 6.052 + 5.192 + 7.966 = 23.331 |
| Seconds, as counted (rounded up per run) | 26 |
| Billable | 5,250 + 2,586 + 3,791 + 2,847 = 14,474 |
| cache_read | 11,393 + 9,067 + 29,746 + 35,662 = 85,868 |
| Remaining | 8 runs, 14,376.669 s, billable 1,485,526, cache_read 14,914,132 |
| Development share used | 3 runs, 17.279 s, 11,888 / 76,801 |
| Guardrail share used | 1 run, 6.052 s, 2,586 / 9,067 |

**Evidence files**
- **`d3-reevaluation-preview.json`:**
  - its stream sha, transcript bytes and sha, and the `SHA256SUMS` sha all match the preserved files;
  - its parent, child total, share and charge match my reproduction.
- **`crosscheck-0104.txt`:** reproduced exactly.
- **`cargo-test.log`:** 282 passed, 0 failed, recorded at 3ab0129. The head only adds the log.
- **`dry-run.log`:** labelled "fake claude, fake credentials, no model call".

**Labelling and other docs**
- Labelling of real, mock and stored-log results is clear: no model call, stored records, a copy of the ledger, and fake claude.
- Runtime contract revision 8 §12 item 8 is consistent with the code.
- The `summarize.py` robustness fix and the forward-looking RUN_PLAN §4 note are acceptable. Neither rewrites earlier results.

## Findings

### F1 · Medium · non-blocking (fix in a follow-up, or here) · `crates/ieo-supervisor/src/caller.rs` `effective()`

**Problem.** `effective()` checks only three things about a correction:
- it names a known run;
- it is the only correction of that run;
- its `original_stream_sha256` matches and `after.usage` is an object.

It then copies `after.usage` and `after.stop_required` and nulls `stop_reason`. It does not re-check what `reevaluate` checked.

**Scenarios I reproduced on ledger copies:**
- (a) The original D3 line's stop_reason is changed to `process-group cleanup failed; usage unmeasured: …`. The correction still applies, and the stop is cleared.
- (b) A correction whose `after.usage.run.tokens` is all zero is accepted. The budget drops to billable 11,627 and cache_read 50,206.

**Effect.** A hand-edited correction line can clear a stop that had other reasons, or zero a run's charge. Only someone with write access to the private ledger can do this, so the risk is low. But the PR body's claim "Forged, duplicate or mismatched corrections make the ledger unusable" is broader than the code. PROGRESS §3's narrower list is accurate.

**Fix.** In `effective()`, refuse the ledger unless all of these hold:
- the correction line comes after its run line;
- the original run's `usage.run.state` is `unmeasured`, and its stop_reason starts with `usage unmeasured` with no other reason;
- `after.usage.run.state == "measured"` and `after.usage.attribution_check == "consistent"`;
- `after.stop_required == false`.

Optionally also require that `after.usage.run.tokens` equals `after.usage.collector_run.tokens`, and that each `child_transcripts` digest is present. Otherwise narrow the PR wording.

### F2 · Low · non-blocking · tests (mutants M2 and M8 survived)

1. **`stream.rs`, the check that the transcript's last response equals `tool_use_result.usage`.** No test fails when it is removed. Add a collector test with a transcript whose last final differs from the reported usage, and expect unmeasured.
2. **`caller.rs` `reevaluate`, the `reason.contains("; ")` guard.** This guard stops a run that also had a signal violation or another reason from being re-evaluated. No test fails when it is removed. Add a caller test with a reason such as `usage unmeasured: …; this run's signal: …` and expect a refusal.

### F3 · Low · non-blocking · `caller.rs` `reevaluate`: base session id

`ResumeBase.session_id` is taken from the run being corrected (`r.session.session_id`), not from the base record. The base record's own session is never compared with the run's, so `run_share`'s session check cannot fire here.

- **D3 is not affected:** D2 and D3 share the session, and the base was selected by `After` under that condition.
- **Fix:** use `br["session"]["session_id"]`, and refuse when it differs from the run's session.

### F4 · Low · non-blocking · `caller.rs` `reevaluate`: transcripts from the command line

The transcripts given with `--child-transcript` are not tied to the record.

- **What is missing:**
  - Nothing requires the set of agent ids to equal the stream's `tool_use_result.agentId`s. A missing one does end as unmeasured, so this direction fails closed.
  - When the record has `children[].sha256` (runs made with the new binary), nothing requires the supplied file's digest to equal it.
- **D3 is not affected:**
  - its record predates the new binary, so it has no `children`;
  - the correction stores the digest 8e509473…, and that matches the preserved transcript.
- **Fix:** require equality with the recorded digest when one exists.

### F5 · Low / Medium · non-blocking for this PR; decide before D4 · `stream.rs` compaction exemption (pre-existing rule, revision 7 §11)

**Current behaviour.** When `compact_boundaries > 0`, a resumed run's share stays measured whatever the attribution says. That includes:
- a share smaller than parent + children;
- `not_checked`, for example an unconfirmed or missing child transcript.

**Why it matters for D4.** D4 is the `/compact` run, so this is the run where "a difference explained by compaction" is assumed. The charge is still the session difference, which is the right charge. However, wrong-base detection is lost, and a child the collector could not confirm passes silently.

**Suggestion.** Under compaction, require the share to be at least parent + confirmed children in every field. Treat `not_checked` caused by an unconfirmed child as unmeasured, unless the user accepts the current rule for D4.

### F6 · Low · non-blocking · `real-sessions-20261115/call.sh` (restart guard) and `summarize.py`

Both read `records.jsonl` raw, so after the correction they still see D3 with `stop_required: true`.

- **`call.sh` restart guard:** no `restart` step remains in `steps.tsv`, and the PROGRESS §8 step 3 container swap is manual. Even so, a restart through `call.sh` would now be refused.
- **`summarize.py`:** a D3 summary would show the original values, which is correct as history.
- **Fix:** have the guard skip runs that have a correction, or call the Rust `records()`. At minimum, note this in RUN_PLAN.

### F7 · Info · preserved `SHA256SUMS`

The file lists itself with a stale hash, so `sha256sum -c` always reports one failure. The other 21 files verify. "22 files" in PROGRESS §7 counts that self-entry.

**Fix:** drop the self-entry next time. The preserved directory itself does not need to change.

### F8 · Info · `crates/ieo-evidence/src/run.rs`

An unreadable `--child-transcript` file is silently dropped, so the child is unmeasured with the reason "전사 없음" (no transcript). This fails closed, but the reason is misleading. Consider reporting the read error.

### F9 · Info · `serve.rs` `keep_child_transcripts`

`Vec::dedup` removes only adjacent duplicates, so a duplicate agent id is copied twice. This is harmless.

If one agentId appears in two `tool_use_result`s in one run, each child entry receives the whole transcript total. That double-counts in `all_children`, which gives a mismatch and unmeasured, so it fails closed.

### F10 · Info · `ieo-role-call reevaluate --decision`

The value is free text. Consider requiring an existing file in the repository and recording its git blob hash in the correction.

## Before applying the D3 correction to the real ledger, and before D4

- **Applying D3:** nothing blocks it. Applying it with the merged binary reproduces the preview: `consistent`, 10/224/2,613/35,662, billable 2,847. After that, the effective budget is the PROGRESS §5 table.
- **Before D4:** make an explicit decision on F5 (the compaction exemption) and record it in the D4 plan or report.

