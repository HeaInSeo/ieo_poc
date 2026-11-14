# Review: PR #52 (02-04 real-session results, ledger snapshot 0025)

- Head reviewed: 29690b50d6bf9caced96a42cf1ea5625491bc8d6 (confirmed with `gh pr view 52`), base main d41e70f
- Reviewer: independent Claude reviewer (no model calls, read-only on the run directories)
- CI at head: all 3 checks COMPLETED/SUCCESS (docs/poc package and progress validation, rust toolchain pin, rust fmt clippy test)
- **Verdict: APPROVE**. There are no blocking or medium findings. Five Low findings and four Info notes follow. They can be fixed in this PR or carried into the 02-04 acceptance report.

## What I ran / checked

1. **Worktree.** Created a detached worktree of the head under the scratchpad (`rv52`) and removed it at the end. Only docs changed: 28 files, of which 22 were added and 6 modified, all under `docs/poc/progress/`. No earlier report was modified.
2. **Public ledger copy vs the live ledger.**
   - Masking every occurrence of the run directory as `<W>` in the live `/opt/rust/ieo_poc_runs/02-04-20261115/ledger/records.jsonl` gives a file identical to `evidence/real/records.jsonl`: 8 lines, and line 5 is the D3 correction `ieo-role-run-correction/v0`.
   - The preserved pre-correction ledger equals the first 4 live lines byte for byte.
   - `preserved-02-04-D3-20261115/SHA256SUMS.v2` verifies (`sha256sum -c`, rc 0).
3. **Independent recomputation from the live records** (billable = input+output+cache_creation; D3 taken from the correction's `after.usage.run`).

   | Run | ms | billable | cache_read | ceil s |
   | --- | ---: | ---: | ---: | ---: |
   | D1 | 4,121 | 5,250 | 11,393 | 5 |
   | G1 | 6,052 | 2,586 | 9,067 | 7 |
   | D2 | 5,192 | 3,791 | 29,746 | 6 |
   | D3 | 7,966 | 2,847 | 35,662 | 8 |
   | D4 | 10,124 | 8,985 | 11,393 | 11 |
   | D5 | 3,350 | 6,464 | 11,393 | 4 |
   | D6 | 3,861 | 4,025 | 12,535 | 4 |
   | **Total** | **40,666** | **33,948** | **121,189** | **45** |

   - Remaining: 5 runs, 1,466,052 billable, 14,878,811 cache_read, 14,355 s.
   - Remaining room per role: development 968,638 / 9,887,878, guardrail 497,414 / 4,990,933.
   - Every figure matches RESULTS.md §4, `budget-final.txt`, state.json and the event. The pre-D4 reconciliation (4 runs, 26 s counted, 14,474, 85,868) matches PROGRESS §5.
   - D3 correction: parent 6/160/456/33,595 plus child 4/64/2,157/2,067 equals the run share 10/224/2,613/35,662 in all four fields. `attribution_check` is `consistent`, the original line is untouched, and `budget-final.txt` shows D3 `corrected true` with 7 runs, so nothing is double counted.
   - §11 item 7: the sum of D1..D5 shares is 6,708 / 1,497 / 99,587 / 19,132 (input/output/cache_read/cache_creation), which equals D5 `usage.session`.
4. **Streams.** All 7 have exactly one `rate_limit_event` with status `allowed`, `overageStatus` `rejected` and `isUsingOverage` false. Every result is `success` with `is_error` false. `apiKeySource` is `none`. `env_names` is HOME, PATH, LANG, CLAUDE_CONFIG_DIR, CLAUDE_CODE_OAUTH_TOKEN in every run, with no `ANTHROPIC_*`.
5. **Per-run facts vs the RUN_PLAN §2 criteria.**
   - **D1:** `removed` 4, `credentials_removed` [], answer exactly `MARKER-SUPPLIED-0204`, no marker skill, `memory_paths` null, HOOK-RAN absent. The `ls -dZ` label and mode of `/state` and `/state/claude` are the same before and after.
   - **G1:** `g1.txt` is OK, both denied files are absent, there are 2 Write permission denials, and the init tools are Edit/Glob/Grep/Read/Write with no Bash.
   - **R1:** restart 0.6 s, PID 1 is the supervisor, checkpoint before and after is S `after_run` D1.
   - **D2–D5:** session S throughout. `resume_base` chains D1→D2→D3→D4, `tamper` is null, D2 used Read and answered with the README first line, D3 called Agent(`reader`) and the child used Read, D4 has 1 `compact_boundary`, D5 answered `RESUMED`.
   - **D6:** `mode` new, new session c84c7b5c…, `config_tamper_detected` with old session S, `tamper0204` absent afterwards, answer `FRESH`.
   - **Binary and stop:** the deployed `bin/ieo-supervisor` sha256 starts with `db2162c2`. `stop.txt` matches the report.
6. **D3 answer.** The stream result is `4`. RESULTS keeps the expected value 3 and records the run as "불충족 1건" (one criterion not met). It does not pass D3 afterwards. Acceptance is left to review and the user, and 02-04 stays 진행 (in progress) at 11/73.
7. **Secrets and privacy scan of all added lines.**
   - No `sk-ant`/OAuth/Bearer/PAT/private-key/JWT patterns. The only hit is the pre-existing prose mention "sk-ant- 형태".
   - The adapter token value appears 0 times.
   - No `/home`, user names or secrets path.
   - No string in the evidence JSON is longer than 200 chars, the README body is absent, and no child transcript or session body is present.
8. **Mechanics** (all with `.venv-poc/bin/python`):
   - `render_progress.py --check`: ok.
   - `validate_package.py`: rc 0.
   - `test_render_progress.py`: RESULT PASS.
   - The archived reviews `review-pr51-{608b367,12c9a21,71e408a}.md` are byte-identical to the pending-reviews copies.
   - All state.json and events.jsonl evidence paths and all RESULTS.md links exist.
   - PR #51 merge is d41e70f (approved head 71e408a), and main CI 43651860463 is success on d41e70f.

## Findings

| # | Severity | Location | Finding | Fix |
| --- | --- | --- | --- | --- |
| L1 | Low | RESULTS.md §1 D4 row / §2 | Live record D4 has `usage.attribution_check: "mismatch"`: parent `result.usage` is 0/0/0/0, while the share is 6,688/1,155/11,393/1,142. The share was accepted as measured under the compaction exception (contract §11 item 7 and §12 item 8: "압축 사건이 없으면", meaning a mismatch is unmeasured only when there was no compaction). The report says only "7회 모두 측정" (all 7 runs measured) and does not mention this. It is correct under the merged rules. The user's D3 decision warns against accepting mismatches loosely, so this should be explicit. | Add one line: D4 attribution is `mismatch`, measured by the compaction rule (the `/compact` call is not in `result.usage`), and the share was not inferred from the parent. |
| L2 | Low | evidence/real/D1.json, G1.json, D2.json, D3.json | These four summaries are not the files in the live `summaries/`. They were regenerated with the PR #51 `summarize.py`, which adds `children` and, for D3, `correction`. D4–D6 and the volume files are byte-identical. RESULTS does not say the four were regenerated. The regenerated `D3.json` also shows `"children": []`, although D3 had child a13188dc… (the script reads the top-level `children`, which the pre-PR #51 record lacks; the transcript is listed in the correction's `child_transcripts`). | Say in RESULTS or the evidence README that D1/G1/D2/D3 summaries were regenerated from the unchanged records and streams after PR #51. Either make `summarize.py` fall back to `usage.children` or the correction's `child_transcripts`, or note that D3 `children: []` means "not recorded at top level". |
| L3 | Low | RESULTS.md §3 item 6 | Of the five PROGRESS §11 resume conditions, only the budget reconciliation and the restart are restated. Items 1 (approval/CI, given only in the PR body), 4 (newest stream within 7 days; D3 is 2026-11-14, so it holds until 11-21) and 5 (no reset, retry or extra billing) are covered by "모두 확인" (all confirmed) alone. | List all five with their check, one line each. |
| L4 | Low | evidence/real/ | No public evidence shows the restart guard passing or the checkpoint surviving the binary-swap restart, apart from indirect evidence that D4's `resume_base` is D3 in session S. The claim that the merged binary was built from d41e70f is not evidenced; I could only confirm the deployed hash `db2162c2…`. | Add a short guard and restart log (masked), and the build commit with its sha256 for both binaries (`ieo-role-call` is `dde8db51…`). |
| L5 | Low | RESULTS.md §4/§5, "예비 5회" (5 spare runs) | The 5 spare runs are not split by role. By share only **2 development** runs (8−6) and **3 guardrail** runs (4−1) remain. This matters for any harness-cause retry decision (RUN_PLAN §4), since most remaining runs are guardrail. | State the split: development 2 runs and 9,562 s counted remaining; guardrail 3 runs. |
| I1 | Info | RESULTS.md §1 D5 / §2 | The §11 item 7 equality holds by construction: each resumed share is the cumulative difference from the actual base, and the bases chain D1→D5. Its real content is that every share was measured, the base chain is unbroken and no cumulative total decreased. The report could say this so the check is not read as an independent cross-check. | Optional wording. |
| I2 | Info | RESULTS.md §1 common | "환경 변수 이름은 OAuth 변수만" (the environment variable names are OAuth variables only) is loose. The names are HOME, PATH, LANG, CLAUDE_CONFIG_DIR, CLAUDE_CODE_OAUTH_TOKEN, and the only credential variable is the OAuth one. | Optional: list the names. |
| I3 | Info | private `preserved-02-04-D3-20261115/caller-ledger/records-before-D3-correction.jsonl` | This file was added at 08:12, after `SHA256SUMS.v2` (07:44), so the manifest does not cover it. It is identical to the preserved `records.jsonl`. The state.json `unresolved` text says "SHA256SUMS.v2 21개" (21 files), which remains accurate. | Optional: add a v3 manifest or note it. Private only. |
| I4 | Info | RESULTS.md header, state.json `running`/`unresolved` | The run directory path is masked as `<W>` in records.jsonl but written out in RESULTS.md and state.json. It contains no user name or secret, so this is harmless. It is only inconsistent. | None needed. |

## Rule checklist

- The budget stayed within the limits: 7 of 12 runs, at most 10.1 s per run (limit 1,800 s), 45 s of 14,400 s counted, 33,948 of 1.5M billable, 121,189 of 15M cache_read.
- Every run shows `overageStatus` rejected, and nothing indicates extra billing.
- Unmeasured was never written as 0: the D3 original line stays unmeasured and stop_required.
- There was no automatic retry and no budget reset, D3 was not rerun, and the spare runs were not used.
- D3's expected value 3 and its failure are preserved. The correction is traceable (decision document, stream hash, child transcript hash, before/after) and counted once.
- Required report contents are present: the cause, the corrected aggregation, the D3 replay or correction result, the corrected and remaining budget, the actual resumption (D4–D6 ran), the D3 answer analysis and its acceptance impact (§5 plus a link to PROGRESS §10), and the resume conditions (partially restated, see L3).
- Real, mock and stored-log evidence are distinguished (§2 and the header table).
- No raw streams, credentials, adapter tokens or session bodies are published.
