# Review of PR #90 at d971f59 (review 1)

PR: HeaInSeo/ieo_poc #90. Head d971f59e38fe899ebaa001bad00579289a84eb26, base main 90ffef5. Settlement format `ieo-role-run-settlement/v0`, the `ieo-role-call settle` writer, the spare release for a settled run, PR #89 review L-1..L-3, and evidence.

## Verdict

**APPROVE.** I found no High or Medium findings. I found seven Low findings and two Info notes.

The core guarantees hold, and I tested each of them:
- Re-verification on every read.
- No double counting.
- R1 stays failed.
- The spare release is scoped to the settled policy.
- The writer writes nothing on refusal.

The Low findings are listed under Findings. They are about:
- Where the copy lives (L-1).
- Provenance fields that are not checked again on read (L-2).
- The dry run's temporary copy (L-3).
- How generic the special case is (L-4).
- Launcher scripts that crash on a settlement line (L-5).
- A per-run cap that is not checked against the settled usage (L-6).
- An unchecked `billable()` sum (L-7).

## What I ran

All runs used a detached worktree at d971f59 and my own target dir with `-j 4`. `df /` showed 12G free before the build. No model calls were made. Nothing was written to the live ledger. I did not touch 58090 or 55433.

| Check | Result |
|---|---|
| `cargo test --workspace` (test DB 55432) | 44 suites ok, **428 passed, 0 failed**. This matches `evidence/settlement/test.txt`. |
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | ok |
| `validate_package.py` (py3.11 venv) | rc 0, schema_errors 0 |
| `render_progress.py --check` | ok |
| `test_render_progress.py` | PASS |
| `ieo-role-call settle ... --dry-run` against a **scratch copy** of `ledger-0205` | See the next table. |
| Adversarial probes (a temporary test file, deleted afterwards) | See Analysis 1. |
| 6 hand mutations against `tests/settlement.rs` | 6/6 killed. See below. |

Details of the dry run:
- The record's `stream.path` was pointed at the copied stream, so its SHA-256 did not change.
- The real evidence copy `s8-evidence/s8-r1-transcript.jsonl` was used read-only, with the real original SHA-256, decision path, verification report and verdict.
- `TMPDIR` was set to a scratch dir.

| Dry-run result | Value |
|---|---|
| exit code | 0 |
| tokens (input / output / cache_creation / cache_read) | **2 / 130 / 1,411 / 3,223**, so billable 1,543 |
| api_requests / messages | 1 / 1 |
| evidence.sha256 = original_sha256 | `806746fd…24dc68` |
| `written` | false |
| `records.jsonl` SHA-256 before and after | identical |
| `children/` | not created |
| temp dir | empty afterwards |

So the real R1 is eligible under the current rule, and the transcript accounting reproduces the independently checked figures.

Hand mutations (each run against `--test settlement`, then reverted):

| ID | Mutation | Result |
|---|---|---|
| M1 | `spare_releasable`: drop the new guard-stop/settlement check | killed (`other_stops_…`:631) |
| M2 | `transcript_accounting`: drop `messages == api-requests` | killed (:243) |
| M3 | `effective`: drop "correction of a settled run" | killed (:407) |
| M4 | `read_copy`: drop the copy SHA-256 check | killed (:384) |
| M5 | `transcript_accounting`: drop the side-chain refusal | killed (:243) |
| M6 | `apply`: keep the original stop_reason instead of "no result event" | killed (:284) |

## Analysis

### 1. The record format and `settlement::apply` on every read

`effective()` handles a settlement line as follows:
- It dispatches on `format`.
- It refuses a second settlement for the same run.
- It refuses a settlement of an unknown or later run. A settlement placed before its run line is refused (tested).
- It refuses a settlement of a corrected run, and a correction placed after a settlement (tested).
- It refuses any run line that itself carries `settlement` or `before_settlement` (tested). This matters because `spare_releasable` trusts `settlement.policy`, and this refusal means only `apply` can set it.

`apply` then checks, in order:
- Source, policy and `model_request: false`.
- That the decision and `settled_at` are non-empty.
- The independent check: evidence, a 64-character lowercase hex digest, and the verdict ∈ {CONFIRMED, CONFIRMED WITH CAVEAT}.
- That the caveat (`uncovered`/`residual`) is the exact constant.
- That the run is matched: stream SHA-256, session, role and budget.
- That the run is fully eligible.
- The copy: an absolute path whose SHA-256 equals the recorded one. The copy is re-accounted from scratch.
- That the stated tokens, request count and message count equal the accounting.
- That the result is ≥ both observed lower bounds, which must be present.

The lower bound is only a floor. It is never the usage. The output is the run with:
- `usage.run` measured, with `source`.
- `stop_required: true`.
- `stop_reason: "no result event"`.
- The old values under `before_settlement`.
- A `settlement` summary.

`transcript_accounting` is fail-closed:
- Every line must be JSON with this session.
- No side-chain or `agentId` entries.
- One entry per `message.id`, the last one wins.
- api-request entries ≥ 1 and equal to the number of distinct messages.
- The last `cost-state.modelUsage`, summed over models, must equal the message sum in all four token kinds.
- Sums use checked arithmetic.

`eligible` requires all of the following:
- A model run, unmeasured, `stopped_by_guard` with stop `Overage(`.
- No result.
- No children.
- Not a spare.
- Stop-reason parts limited to `no result event`, `usage unmeasured: …` and `this run's signal: …`, with both the first and the last present.
- The stream SHA-256 equal to the record's.
- Every rate-limit event in the stream passing the current rule, and at least one warning.

Reasons are joined with `"; "`, and `other_stop_reason` escapes that separator, so the prefix match cannot be smuggled into another reason.

The scope tests confirm these are refused, and that the spare does not release them:
- a real reject, a warning followed by a reject, extra usage, a warning followed by extra usage
- no warning
- Timeout, BillableCap and CacheReadCap
- not the guard
- an extra reason, measured already, a result event, a child run, a spare, another stream

Probes I added (temporary test, then deleted):

| Probe | Result | Assessment |
|---|---|---|
| A. `ledger_copy` changed to an identical file **outside the ledger dir** | accepted | L-1 |
| B. `ledger_copy` changed to a **symlink** to that outside file | accepted | L-1 |
| C. `evidence.original_sha256` changed to another 64-hex value | accepted | L-2 |
| D. `independent_check.sha256` changed to another 64-hex value; `decision: "whatever"` | accepted | L-2 (format-only check) |
| D3. settlement `model_request: true` | refused | ok |
| E. run with `outcome.tamper` non-null | settled | Info. In `serve.rs` `tamper` is the config-tamper report of a takeover, not a run failure, and no other path treats it as a stop either. |
| F. run `limits.billable_room` 1,500 < settled billable 1,543 | settled | L-6 |
| H. `outcome.cleanup_failed: true` without its stop reason | settled | Info. A real record always adds "process-group cleanup failed" (caller.rs:1424), which `eligible` refuses. Defence in depth only. |

The copy is swapped after settling: the read fails closed and the whole ledger becomes unusable (tested). This is consistent with corrections.

### 2. No double counting

- **Caller budget.** `budget::spent(records)` sees one run line for R1, and the settlement line is no run. The test asserts runs 1, 5 s, billable 1,543, cache_read 3,223 and 0 unmeasured.
- **Coordinator.** `limits::settle` is given effective records, from `records_when_idle` or the budget check's own read. It matches by `attempt_id`, and the settlement line has none. So R1's newest `assignment_run_usage` row becomes `measured` via `usage_of`, and "the newest row counts" means R1 is counted once. The settlement line's `model_request: false` would read as "withdrawn" only if it were in `recs` with an `attempt_id`, and it is in neither.
- **Status and chain views.** `chain.rs:299` reads `assignment_run_usage`, so the same holds.
- **Shell scripts.** The grep-based `settled()` in `lib-step8.sh` and `run-step8.sh:74` match `"attempt_id":"…"`. A settlement line has only `"settles"`, so there is no false match.
- **Python launcher.** `check-run.py` and `dry-run-step8.sh` parse every raw line and index `r["attempt_id"]`. See L-5. They do not double count, but they will crash.

### 3. R1 stays failed, and the special-case release

The settled R1 keeps `stop_required: true` and the reason "no result event", so `asks_for_a_person` still stops plain calls. The original line is untouched, and the old stop history is under `before_settlement`.

`spare_releasable` now refuses any guard-stopped run without `settlement.policy == guard_stop_on_old_rule_warning` (M1 confirms the test). The spare gate then:
- releases the settled R1 once (tested);
- refuses all 15 out-of-scope cases;
- refuses a forged "measured" guard-stopped run;
- refuses an unsettled R1.

After R1b carries `spare.replaces = s8-r1`, `asks_for_a_person_but` treats R1 as answered for R2 and G1. The count stays R1 + R1b + R2 + G1 = 4, because the R1 line counts as a planned run.

### 4. The writer `ieo-role-call settle`

`caller::settle` runs these steps in order:
1. It refuses an empty decision and a bad independent check.
2. It takes `caller.lock` with `try_lock`.
3. It refuses if `inflight.json` exists.
4. It refuses an already-settled or corrected run, and anything other than exactly one record.
5. It reads the ledger fail-closed.
6. It checks eligibility.
7. It checks the given file's SHA-256 against `--original-sha256`.
8. Only then does it write the copy: `write_private`, 0600 file, 0700 dirs. An existing different copy is refused.
9. It calls `build`, which runs `apply` again. If `build` fails, it removes the copy it just created.
10. It appends one line under the file lock.
11. It reads back.

The CLI test covers: a dry run that writes no line, a wrong hash that gives exit 1 and no line, and a normal run that writes one line. `not_enough_evidence_writes_nothing` covers four further refusals, and in each case neither the line nor the copy is written: no cost-state, an exit/message mismatch, the hash, and a lower bound violation.

Two caveats:
- The dry run writes a temporary transcript copy into `TMPDIR` (L-3).
- An append or read-back failure after the copy is written leaves the copy, and on read-back failure also the line. This is practically unreachable, because `build` already ran the same `apply`.

### 5. PR #89 review L-1..L-3

- **L-1, fixed.** `split_lines` splits on `"\n"` only and strips a trailing `\r`. `json.loads(..., parse_constant=raise)` refuses NaN and Infinity. Both callers (`check-run.py`, `obs-check.sh`) now pass the text. The new `the_launcher_reads_a_stream_like_the_supervisor` runs 9 stream cases through Rust and Python: truncated line, NaN, Infinity, U+2028, U+0085, CRLF and others. `dry-run-step8.sh:134` still uses `splitlines()`, but only to doctor a stream, not to judge it, so that is fine.
- **L-2, fixed.** `isUsingOverage` 0 and 1, `overageEnabled` 0 and `overageInUse` 0 were added to the shared table.
- **L-3, fixed.** `a_later_allowed_event_clears_the_displayed_warning`.

### 6. Public-repo hygiene

None of these files contain transcript message text, tokens, OAuth values or secrets:
- `r1-usage-verification.md`
- `r1-reconciliation.json`
- `settlement/*.txt`
- `warning/rate-limit-sources.txt`

They contain only entry types, counts, timestamps, truncated ids, hashes and paths. A grep for `sk-ant`, `Bearer`, password and token assignments found nothing. The test fixtures are synthetic.

## Findings

| ID | Sev | Location | Scenario | Fix |
|---|---|---|---|---|
| L-1 | Low | `crates/ieo-supervisor/src/settlement.rs:292-306` (`read_copy`) | The module doc, RESUME_PLAN appendix B and contract §21 item 32 say the copy is kept **in the ledger** (`children/<attempt>/settlement-transcript.jsonl`). On read, any absolute path is accepted, including a path outside the ledger dir or a symlink (probes A and B). The content is still bound by its SHA-256, so the usage cannot change. However: (1) retention is not guaranteed, so a copy under `/tmp` that is later cleaned makes the whole budget ledger unusable; (2) `write_private` follows a dangling symlink at `to` when creating the copy. | On read, require `ledger_copy == <ledger_dir>/children/<settles>/settlement-transcript.jsonl` (the plan's ledger dir), and open it with `O_NOFOLLOW`, or check `symlink_metadata().is_file()`. Use `create_new` for the copy. Add the two probes as tests. |
| L-2 | Low | `settlement.rs:310-370` (`apply`) | `build` requires the copy SHA-256 to equal `original_sha256`, but `apply` never checks `evidence.original_sha256 == evidence.sha256` again (probe C). `independent_check.sha256` and `decision` are only format-checked: an arbitrary 64-hex value and an arbitrary string are accepted (probe D). These are provenance claims that are not re-verified on read, which falls short of "tamper refused" for those fields. The usage itself stays bound to the hashed copy. | In `apply`, check `evidence.original_sha256 == evidence.sha256`. Optionally pin `decision` to the decision document path. If the report digest is meant to be checked on read, resolve it against the repository. Otherwise say in appendix B that it is a write-time digest only. |
| L-3 | Low | `crates/ieo-supervisor/src/caller.rs:1792-1810` (dry run) | `--dry-run` writes the full private transcript to `std::env::temp_dir()/ieo-settle-dry-<pid>-<attempt>.jsonl`, using a predictable name, `create+truncate` and following existing files, then removes it. The evidence is deliberately kept in a 0700/0400 directory, and a kill between the write and the remove leaves a copy in `/tmp`. Appendix B line 178 also says the dry run writes nothing. | The probe is not needed: `build` reads `ledger_copy` directly, so pass `std::path::absolute(transcript)` and then replace `evidence.ledger_copy` with `to` as now. If a probe is kept, put it under the ledger dir with `create_new`. |
| L-4 | Low | `settlement.rs:217-289` (`eligible`) | The decision limits the special case to **this s8-r1** (§2.3). The code admits any budget and attempt that structurally matches an old-rule warning stop, so a pre-rev-19 run in another ledger could also be settled and then spare-released. Today this is mitigated: a current-rule guard can no longer produce such a stop, and a person must run the writer. | Pin the settlement on read to `budget == "02-05" && settles == "s8-r1"`, or to an allow-list constant that names the decision. Otherwise say explicitly in contract item 32 why the structural scope is equivalent. |
| L-5 | Low | `docs/poc/reports/tasks/02-05/step8-plan-20261117/launcher/check-run.py:57,85`; `dry-run-step8.sh:133` | Once the settlement line is in the live ledger, `{r["attempt_id"]: r for r in records(ledger)}` and `r["attempt_id"] == "s8-r1"` raise `KeyError`, because the line has no `attempt_id`. Appendix B line 179 notes that `check-run.py` reads raw lines and counts R1 as failed or unmeasured. It does not note that the post-run check and the dry-run rehearsal will crash. This is fail-loud, not silent. | In the resume PR, skip or apply `format == ieo-role-run-settlement/v0` lines, preferably by reading effective records through `ieo-role-call`. Add a settlement line to the rehearsal ledger. Mention the crash in appendix B. |
| L-6 | Low | `settlement.rs:346-358` | The settled usage is not compared with the run's own `limits` (`billable_room` / `cache_read_room`). In probe F, a run with room 1,500 was settled at 1,543, and the spare may then release it. The decision says time and token overages are not covered by the special case. Budget totals still catch real overspend, because `spent` includes the settled usage. For R1, 1,543 is far below 150,000. | In `apply` (or `eligible` with the accounting), refuse when `acc.billable() > limits.billable_room` or `tokens[3] > limits.cache_read_room`, and test it. |
| L-7 | Low | `settlement.rs:94-96` (`Accounting::billable`) | `tokens[0] + tokens[1] + tokens[2]` is not checked. A hand-made transcript with huge values panics in debug builds, and in release builds wraps and can pass the lower-bound check. `budget::spent` uses checked arithmetic and would then treat the run as unmeasured, so the end result is safe. | Use `checked_add`, and refuse on overflow. |
| I-1 | Info | `settlement.rs:217` | `outcome.tamper` is not considered. It is the config-tamper report of a takeover and is not a stop anywhere else, so this is consistent. | none |
| I-2 | Info | `caller.rs:1847-1855` | A read-back failure after the append returns an error but leaves the appended line in place. This is unreachable in practice. | Optional: say so in the doc comment. |

## Summary

PR #90 adds the transcript-based settlement as a separate, append-only, re-verified ledger line. I confirmed the following:
- R1 counts once, with 2 / 130 / 1,411 / 3,223 (billable 1,543), in the caller budget, the coordinator's `assignment_run_usage` and the chain view.
- R1 stays a failed run (`stop_required`, "no result event").
- The spare release is gated on the settlement policy. Every out-of-scope stop is refused, both for settling and for release.
- The writer is locked, refuses while a call is in flight, writes nothing on refusal, and its dry run reproduces the independently verified numbers on a scratch copy of the real ledger.
- L-1..L-3 from review 91e87ce are fixed.
- The evidence is clean.
- Tests (428/428), fmt, clippy, the validator and render all pass.
- My 6 hand mutations were all killed.

The remaining findings are Low. Copy placement and symlinks, provenance fields not re-checked on read, and the dry run's `/tmp` copy should be tightened before the live settlement. The generic scope, the launcher `KeyError`, the per-run limit check and the unchecked sum can go into the resume PR. **Verdict: APPROVE.**
