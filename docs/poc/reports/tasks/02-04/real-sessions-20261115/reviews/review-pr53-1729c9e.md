# Review of PR #53 at 1729c9e (review 3 of at most 4)

- Head: `1729c9e78b20741ea2d8ff5ecbb6ddff8ea3709e` (confirmed with `gh pr view 53`); base main `6626558`
- Reviewer: independent reviewer agent (Claude), 2026-11-15
- Scope: correction 2 (`git diff 5f4fc7f 1729c9e`, 7 files) plus a re-check of N1/N2 from review 5f4fc7f and the binding rules
- **Verdict: APPROVE.** Nothing blocks the merge. N1 is resolved. N2 is resolved in substance, but the new provenance sentence is partly inaccurate (L1 below). Fix it only if a correction 3 happens anyway; otherwise record it.

## What I ran

- `gh pr view 53 --json headRefOid,statusCheckRollup`: head matches. CI at review time: "docs/poc package and progress validation" SUCCESS, "rust toolchain pin resolves" SUCCESS, "rust fmt clippy test" still IN_PROGRESS. Correction 2 touches no Rust, so a failure there would not come from this change, but confirm it is green before merge.
- Detached worktree at 1729c9e under the scratchpad (`rv53c`, now removed), using the repo `.venv-poc` Python:
  - `scripts/validate_package.py`: rc 0, schema_errors 0, ledger_spec_match true, generated_views_match true, 477 internal links checked
  - `scripts/render_progress.py --check`: ok
  - `scripts/test_render_progress.py`: RESULT PASS
  - `reports/tasks/02-04/real-sessions-20261115/test_summarize.py`: 15 PASS, RESULT PASS 0
  - The working tree stayed clean. The `state.json` sha256 `ddf2dd12…` matches the hash in all four generated views. Snapshot `v1.3.3-EXEC-0026`. STATUS still shows 11/73 on base and head, so the completion count is unchanged.
- Private, read-only checks (nothing copied out):
  - `sha256sum -c` on `SHA256SUMS.binaries-ca08c76`: 2/2 OK. On `SHA256SUMS.binaries-d41e70f`: 2/2 OK. On `SHA256SUMS.v2`: 21/21 OK. The `SHA256SUMS.v2` mtime is still 07:44:16 KST, so it was not modified.
  - The preserved binaries and the session scratch copies (`bin-ca08c76`, `bin-d41e70f`) have the same sizes, mtimes and SHA-256 values. `ieo-supervisor` 652fc14f… and `ieo-role-call` c61c6380… match the hashes in the 17:36:17Z build output that the ops-log extract quotes. db2162c2… and dde8db51… match the 23:12:21Z build.
  - mtime claim: the ca08c76 files have mtime 2026-11-15 02:36:16 KST (17:36:16Z), which is correct. The preservation directories were created at 08:51:40 and 08:51:50 KST (23:51Z), which is consistent with requery.txt.
  - For each of the 9 tool calls cited in the two ops-log extracts, I read its records in the work-session log. Results are under L1.

## N1/N2 status

- **N1 (Medium): resolved and correctly labelled.** The SUPPLEMENT row for D1·G1·R1·D2·D3 now says three separate things. The 23:13:11Z preservation copy failed (time-of-run record). The build output survived in the scratch directory. It was copied at 23:51Z with its own manifests, and this is labelled **현재 재조회** (current re-query), pointing to the new last section of requery.txt. The hash in that section is not used as proof of what ran. The link to the run is still the contemporaneous build-output hash, which now matches the preserved files. `SHA256SUMS.v2` is unchanged and still lists 21 files. The `state.json` "unresolved" text is updated to match ("사후에 보존함(현재 재조회 requery.txt)", i.e. "preserved after the fact (current re-query, requery.txt)"). The repo no longer contains any text saying these binaries are not preserved.
- **N2 (Low): resolved in substance; wording issue L1.** The SUPPLEMENT now discloses that the command lines in the extracts carry `cd …ieo_poc-0204 &&`.

## Findings

### L1 (Low; record, or fix if correction 3 happens): the new provenance note for the `cd` prefix is inaccurate for some entries
- Location: `SUPPLEMENT.md` line 8 (the **당시 기록** (time-of-run record) bullet). The note says the command line is the tool-call command with its working directory **prepended**, and that this directory is in the request record on **the line immediately after** the quoted line.
- What the private log shows for the 9 cited calls:
  - 18816, 19626, 19677: as described. The tool_use command has no `cd`, and the next line (an api-request record) has the `cd`-prefixed command.
  - 18853: the request record is **2 lines** after (18855), not the next line.
  - 19665, 19691: the agent's own command **already started with** `cd …ieo_poc-0204 &&` (the record cwd was the main checkout). Nothing was prepended, and no request record follows.
  - In every `cd` case, the `cd`-prefixed command is also stored verbatim in the `wireToolInputs` field of the tool-call (assistant) record, which is the line just before the cited result line. So the extracts' header claim "Extracted verbatim (command and output)" holds. Only the explanation of where the text comes from is wrong.
- Impact: the evidence content is correct and nothing about the runs is overstated. Only the description of the private source is imprecise, and public readers cannot check that source anyway. This does not block.
- Fix (one sentence): for example, "명령 줄은 실제 실행된 명령(작업 세션 기록의 도구 호출 줄 `wireToolInputs`)이다. 일부는 도구가 작업 디렉터리 `cd …ieo_poc-0204 &&`를 앞에 붙인 것이다." ("The command line is the command that was actually run (the `wireToolInputs` field of the tool-call line in the work-session log). In some entries the tool prepended the working directory `cd …ieo_poc-0204 &&`."). Alternatively, drop the "바로 다음 줄" ("the line immediately after") claim.

### I1 (Info; record): the D4–D6 binary row does not mention that its binaries were preserved
- Location: `SUPPLEMENT.md` line 62, the "실행 바이너리(D4~D6)" (run binaries D4–D6) row.
- The db2162c2…/dde8db51… binaries were also copied to `binaries-d41e70f` at 23:51Z. Only requery.txt and the D1–D3 row's parenthesis say so. That is enough. An optional cross-reference would make the row complete.

### I2 (Info): rust CI job still running at review time
- Merge only after "rust fmt clippy test" reports SUCCESS for 1729c9e.

## Binding-rule check (unchanged and OK)
- Model calls: 0, no reruns. The spare runs stay at development 2 / Guardrail 3.
- D3's expected value 3 and the "불충족(기록 유지)" (not met, record kept) line are kept (SUPPLEMENT lines 87 and 139).
- Time-of-run record, current re-query and unconfirmed items are separated, and the re-query is not used as proof of the past. Rebuild reproducibility is still stated as unconfirmed.
- Completion count stays 11/73.

## Must fix before merge vs record
- Must fix: none (only wait for the rust CI job to pass).
- Record, or fold into a correction only if one is needed for another reason: L1, I1.
