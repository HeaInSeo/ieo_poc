# 02-02 independent expectations — author notes (phase 1, frozen)

Author: independent expectation owner (no development context). Created 2026-11-12 (UTC).
Candidate targeted: `ieo-evidence` at repository commit 1785cb7. The candidate was **not** built or run in this phase.
I did not read `crates/ieo-evidence/src`, `crates/ieo-evidence/tests`, the 02-01 `crosscheck_0104.py`, or 02-01 `evidence/`.

## Inputs used (only)

- 02-01 REPORT.md §1 (behaviour table), §3 (usage rules, resume base findings), §4/§5 (scope, limits).
- `contracts/evidence-record-v0.schema.json` (field names, enums, conditional constraints).
- Runtime_Contract_v0.md §11 item 7 and the §4.2 row on child usage.
- Reviews review-pr21-2df3c35.md and review-pr21-273907b.md (accepted clarifications).
- Task brief items 5 (carried fixes) and 6 (CLI interface and exit codes).
- 01-04 real stream-json files (14 copied verbatim as fixtures `fx-01`..`fx-14`; source map in `fixtures/SOURCES.json`), plus 8 synthetic streams (`fx-20`..`fx-27`) I wrote (`tools/make_fixtures.py`).

## How expectations were derived

- **Stream values** (result-event fields, parent/session/child tokens, costs, compaction and rate-limit counts) come from my own reader, `tools/oracle_stream.py`. It applies the rules exactly as written: the selected result is the last one with num_turns ≥ 1, or the last one if none has that; parent = `result.usage`; session = sum of `result.modelUsage`; one child per Agent tool_use, taken from `tool_use_result.usage`; a measure missing any of its four fields is unmeasured.
- **Run share.** A new session's share is its session total. A resumed session's share is its session total minus the base record's session total (contract §11 item 7). The share is unmeasured when there is no base, when the base belongs to a different session, when the totals decrease, or (for a resumed run) when share ≠ parent + children and there was no compaction. A new-session mismatch is recorded as `mismatch` and is not withdrawn. Run cost = cumulative `total_cost_usd` minus the base cost.
- I checked my reader against the numbers the REPORT/review state independently (N1+N2 usage = N2 cumulative, L3 compaction share (2020,1354,2761,1019), T23 parent + child = modelUsage). All agree.
- **Process, judgment and redaction values** come straight from the rule rows. Each case's `basis` gives the doc and row behind every field. File sizes and SHA-256 values are computed with hashlib from the exact bytes the command prints.
- **Duration** gets bounds only, and only on timeout paths: ≥ timeout − 20 ms, plus a loose upper bound when TERM is honoured.
- **Summarize checks** use the field name `completeness` (named in REPORT §1). The measured-sum key is `tokens_measured_sum`, the name used in accepted review 2df3c35 §3.4. Both are found by breadth-first key search, so nesting depth does not matter. The unmeasured listing is checked only as "the run's identifier appears in the output". I assumed summarize prints JSON on stdout, inferred from `runs-summary.json` / `replay-0104-summary.json` being summarize output (review 273907b §5).

## What is undetermined (excluded from scoring, counted)

Fields:
- the wording of `launch_error`, `judgment.reasons`, `attribution_detail` and the redaction placeholder (only null/non-null or count bounds are checked)
- `secret_sources`: which things count as a source is not defined
- `usage.by_model` contents: cumulative or run share is not defined
- `result_event.runtime_version`: the source field is not defined
- `process.signal` for timed_out
- `process.exit_code` for signaled
- `stray_group_killed` when the leader itself ignored TERM
- `usage.session` for a new session whose modelUsage is empty: only the run share is fixed
- whether a rejected base is still echoed in `resume_base`
- `judgment.status` whenever the only open question is an unmeasured run share, because the rules list no blocking effect of unmeasured usage

Open ledger items owned by 02-02 that the approved rules do not settle:
- P3-2: result subtype ≠ success with is_error false. Judgment and exit are undetermined (V-33).
- P3-4: model request with `--expect timeout`. Judgment and exit are undetermined (V-34).
- P3-6: natural exit racing the deadline. The whole case is undetermined and not executed (V-35).

Not asserted anywhere: default role, default timeout/grace, `argv`/`cwd` text, timestamps beyond format, the CLI exit code of a successful summarize.

Known weaknesses of the checks:
- The "unmeasured listed" check is a substring check. A summary that lists every run would also pass it.
- `$tree_not_contains` checks only the evidence tree under OUT, not the candidate's own stderr (secret-refusal cases check CLI stderr separately).
- The SHA-256 secret form is assumed to be lowercase hex.

## Scorer

`score.py --set PATH --bin PATH --out RESULT.json [--work DIR] [--keep] [--only IDS]`. It uses only the stdlib. Each case gets a fresh temp dir with OUT/FIX/TMP. The scorer copies fixtures in and runs the invocations with stdin set to /dev/null and a 90 s cap each. Then it reads the records, summary output and exit codes, and compares only the determined fields. Expected values never reach the candidate.

Dry check (no candidate):
- With a silent stub that exits 0, every determined case is reported FAIL and nothing crashes (`dryrun/stub-*.txt`).
- With a tiny plumbing-only fake for non-model paths, PASS is reachable (`dryrun/fake-public.txt`).

## Access / sealing (unverified)

These files sit in a scratchpad readable by the same OS account the development agent uses. **Nothing blocks the developer's access to them.** Strong sealing is deferred by user decision A2 (amendment A2, commit 1785cb7). Independence is therefore **unverified**. The only evidence is the SHA-256 of every file in `manifest.json`, taken at freeze time, so later changes can be detected but earlier reads cannot be ruled out. The developer may legitimately read `public.json` and later `authoring.json`. `validation.json` is meant to be used once.
