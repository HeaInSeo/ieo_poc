# Review of PR #53 (02-04 real-session evidence supplement), review 2 of at most 4

- **Head reviewed:** 5f4fc7ffe30a809e1446b8181ec81d25d0b4927d. I confirmed it with `gh pr view 53`.
  - Base: main 6626558.
  - Two commits: 56882cb, plus correction 1 (5f4fc7f).
- **Reviewer:** independent reviewer agent (Claude Code subagent). This review follows correction 1 of at most 3.
- **Verdict: CHANGES REQUESTED.** There is one Medium finding: the supplement says the pre-swap binary file no longer exists, but it does exist. The other findings are Low or Info.
  - All three Medium findings and all five Low findings from review 1 are resolved.
  - All three Info items from review 1 are addressed.

## What I ran or checked

- **CI.** `gh pr view 53 --json headRefOid,statusCheckRollup`: 3/3 SUCCESS at 5f4fc7f (run 43654919335). The job log shows `test_summarize.py` with the five new "public D3/D4 record" checks, and `RESULT PASS 0`.
- **Worktree.** I created a detached worktree at 5f4fc7f under `scratchpad/rv53b`, and removed it at the end. In it:
  - `test_summarize.py` passed: `RESULT PASS 0` (14 checks).
  - `render_progress.py --check` returned ok.
  - `validate_package.py` exited with rc 0.
  - `test_render_progress.py` reported `RESULT PASS 0`.
- **Correction diff.** I read `git diff 56882cb 5f4fc7f`.
  - Changed: SUPPLEMENT.md, the new `ops-log-20261114T1736Z.txt`, `ops-log-20261114T2312Z.txt` (D5 and D6 added, "completed" labels), `requery.txt` (a new parent-transcript section), `test_summarize.py`, and the progress snapshot. The snapshot keeps the same snapshot id 0026; the ledger hash is updated and `unresolved` is extended.
  - Unchanged: `summarize.py` and `evidence/real-v2/`.
- **Ops-log extracts against the private work-session log.**
  - I parsed every block of both extracts and matched it, by tool_use id suffix, to the log. All 9 blocks match on four points:
    - The cited line is the tool_result line.
    - The timestamp equals the result timestamp.
    - The description is the same.
    - The command and output are identical after the `<scratch>` substitution.
  - The blocks are at lines 18816, 18853, 19626, 19645, 19651, 19660, 19665, 19677 and 19691.
  - The line 18853 citation for 17:40:41Z is correct. Review 1's "line 19260" was my own error; 19260 is an unrelated 22:29Z step.
- **"sha256 at extraction".** Each stated hash is the SHA-256 of a byte prefix of the log:
  - d4f7f142… (1736Z extract) is the first 20,294 lines.
  - fd56e87a… (2312Z extract, new) is the first 20,360 lines.
  - The superseded 31682275… is the first 19,861 lines.
  - The log now has more than 20,460 lines and a different hash (18699dba… when I read it).
  - "At extraction" is therefore stated correctly.
- **Binary rows.**
  - 17:36:17Z build output: 652fc14f… / c61c6380…. The `git diff --stat origin/main -- crates Cargo.lock Cargo.toml` output was empty.
  - There was no fetch between the 17:35:06Z `git fetch` (log line 18769) and 17:40:38Z. So origin/main at the build was the ca08c76 printed at 17:40:41Z.
  - `run-real.sh` line 34 copies `$BIN` and `$(dirname $BIN)/ieo-role-call` into `$W/bin`, and the 17:40 command passed `$S/bin-ca08c76/ieo-supervisor`.
  - The 23:13:11Z pre-swap hashes are 652fc14f… / c61c6380…, and the post-swap hashes are db2162c2… / dde8db51…. PID 1 is db2162c2 in both containers.
  - D1, G1, R1, D2 and D3 ran on 652fc14f; D4, D5 and D6 ran on db2162c2. The rows state this correctly.
- **Image row.**
  - At-the-time evidence is now the 17:40:41Z run-real.sh output carrying the d9c68c2c… digest.
  - The creation time 02:40:40 KST has moved to the "현재 재조회" column.
  - Today's `podman container inspect` agrees: created 02:40:40.255 and 02:40:40.698 KST, ImageName the digest, image id 6880d90c….
- **requery.txt parent-transcript section.**
  - I re-counted the development session transcript read-only with `podman unshare cat`. It has 101 lines, with the compact_boundary at line 60 and the compact summary at line 61; neither has a usage field.
  - The only assistant usage after the boundary is at line 94: 2/7/6,455/11,393, which is D5's share.
  - The file mtime is 08:13:44 KST (end of D5), so it has not changed since. The re-query time of about 23:44:4xZ matches the log.
- **Condition 5 exit codes.**
  - The caller printed "exit 0" for D1 (log 18871), G1 (18893), D2 (18928), D4 (19665), D5 (19677) and D6 (19691), and "exit 3" for D3 (18934).
  - "0·2 밖은 D3의 3뿐이고 그때 멈췄다" is correct.
- **§5 sources.**
  - The pr30 §3 quote is accurate.
  - The contract §7 role-scope and repo-scope rows are accurately quoted (`Runtime_Contract_v0.md` lines 163–164; the repo row is elided with "…" and the bold is dropped).
  - The 04-05 prerequisites (04-04 and 02-04 accepted) and the 04-04 prerequisites (04-03 and 02-03) match Sprints v0.
  - Progress record 2 covers "회차·전체 시간 마감과 그룹 정리" in the mock, so "결정 §3 … 모의로 확인했다(진행 기록 2·3)" is supported.
  - `run::charge_reported` (run.rs:636–656) returns unmeasured whenever `!exited`.
  - The §4 reading is now labelled "작성자의 해석", and acceptance review decides it.
- **New §6 row "repo 범위 stop … 미충족".**
  - It is supported. role-container PROGRESS §1 lists the "안전 종료 보고" status item as missing.
  - No crate other than ieo-supervisor calls `/v0/role/…`; `ieo-coord` does not call stop.
  - Recording it as 미충족 (rather than out of scope) is conservative and acceptable.
  - The per-row source labels in §6 resolve I2.
- **Privacy.**
  - I scanned the added and changed files for `sk-ant`, Bearer and OAuth values, private keys, the scratch path, and the user email. The only hit is the pre-existing prose "sk-ant- 형태만 가림" in state.json.
  - Usernames, run-directory paths and session ids are already public in earlier evidence.
  - There is no transcript or session text; only counts and the four usage numbers already in the D5 summary.
- **Invariants.**
  - No model call and no rerun.
  - Spare runs stay at development 2 / Guardrail 3, and budget is unchanged.
  - 02-04 stays "진행" and 11/73 is unchanged.
  - The D3 expected value and its failure are kept.

## Status of review-1 findings

| Finding | Status |
| --- | --- |
| M1 pre-swap binary link and loss | **Partly resolved.** The 17:36Z build and the 17:40Z copy are now cited verbatim, and the binary-to-run mapping is correct. However, the new statement that the file no longer exists is wrong (N1 below). |
| M2 image row | Resolved. The 17:40:41Z output is cited as at-the-time evidence, and the creation time moved to the re-query column. |
| M3 §5 scope | Resolved. pr30 §3 and contract §7 rows were added, the §4 reading is labelled as the author's interpretation, and timing and prerequisites were added. The optional "scope classification" question was not added to the decision draft (I-a). |
| L1 transcript fact class | Resolved. It is labelled 현재 재조회, with counts in requery.txt, which I verified. |
| L2 contract vs collector rule | Resolved. The compaction-case rule is attributed to `stream.rs` and 608b367 F5, not to the contract. |
| L3 private-ledger line hashes | Resolved. There is a new §1 row "출처 해시의 범위". |
| L4 condition-5 second half | Resolved, and verified against the log. |
| L5 real-record test | Resolved. Public `records.jsonl` D3 and D4 cases are in CI. The optional Agent-without-children cross-check was not added, which is acceptable. |
| I1 / I2 / I3 | Addressed: the "completed" labels and header note, the per-row sources in §6, and the "사용량 규칙 근거" row. |

## Findings

### N1 · Medium · SUPPLEMENT.md §3, row "실행 바이너리(D1·G1·R1·D2·D3)", column "확인하지 못한 것"; state.json 02-04 `unresolved` — the pre-swap binary does still exist

- **The claim.** The text says "교체 전 바이너리는 보존되지 않았다 … **이 바이너리 파일은 남아 있지 않다.** 해시는 당시 기록에만 있다". state.json `unresolved` repeats "교체 전 바이너리(652fc14f…)는 보존되지 않음(… 해시는 당시 기록만)".
- **What exists now.** The 23:13 preservation copy did fail, but the build-output copy that run-real.sh read from still exists in the work-session scratch directory:

  ```
  <scratch>/bin-ca08c76/ieo-supervisor  652fc14f8e542b49a01f3146443adc4d15a802f95f6cd5692d4b3f2ddb3f42f4  (mtime 2026-11-15 02:36 KST = 17:36Z build)
  <scratch>/bin-ca08c76/ieo-role-call   c61c63802b80c4e8605dec434f3dae87174357fa9766cb5449ce9763670de642
  ```

  `<scratch>/bin-d41e70f/` also still holds db2162c2… / dde8db51….
- **Why it matters.**
  - The statement is a factual error in the evidence table, on the binary that produced most of the real evidence.
  - The scratch directory is session-scoped and can disappear, so the file is still at risk of real loss.
- **Fix.**
  - Copy both `bin-ca08c76` files into `/opt/rust/ieo_poc_runs/preserved-02-04-D3-20261115/` (or a sibling directory), and record their SHA-256 in a new manifest. Do not rewrite `SHA256SUMS.v2`.
  - Record the copy as a current re-query: time, source path as `<scratch>/bin-ca08c76`, hashes and mtime.
  - Rewrite the row as follows: the 23:13:11Z preservation copy failed (`$A` unset, stderr discarded); the 17:36Z build output in the scratch directory survived, with hash equal to the at-the-time record; it was preserved at <time> (current re-query). Rebuild reproduction is still unconfirmed.
  - Update the state.json `unresolved` wording to match.
  - Optional: because both original target paths are known, a rebuild in the same worktree path and the same `CARGO_TARGET_DIR` could test the "debug build embeds paths" explanation. This is not required.

### N2 · Low · ops-log headers ("Extracted verbatim … lines cited") — the command text comes from a different log record than the cited line

- **The difference.** For log lines 18816, 18853 and 19626, the extract's `$ ` line begins with `cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-0204 && `.
  - The assistant `tool_use` record just before each cited result (18815, 18852, 19625) has the command *without* that prefix. The shell's cwd was already `ieo_poc-0204`.
  - The prefixed form appears in a separate `api-request-blob` record (for example, line 18817).
- **Impact.** The effect is accurate, because the cwd was indeed `-0204`. But "verbatim at the cited line" holds only for the output, description and timestamp.
- **Fix.** Add one header sentence: "the command is as recorded in the session's request record; where the cwd was carried over, it appears as a leading `cd`". Alternatively, cite both line numbers.

### I-a · Info · §5 decision draft

- The scope conclusion ("실행 중인 실제 회차의 정지는 04-05(P05-R)의 수용 범위다") is stated firmly in §5 **판단**. The §4 reading under it is labelled as an interpretation for acceptance review.
- Listing "02-04 vs 04-05 classification" as an explicit question for the user in the decision draft would make the confirmation path visible. This was optional in review 1, and is not required.

### I-b · Info · ops-log header hashes

- The "sha256 at extraction" values are prefix hashes of the first 20,294 and 20,360 lines respectively.
- Stating the line count at extraction would let the owner re-verify them against the growing log, as I did.

### I-c · Info · §3 condition 5 and the D1–D3 binary row

- Both claims are correct, but each relies on evidence the supplement does not cite:
  - Condition 5 cites at-the-time "exit" lines only for D4–D6. The D1/G1/D2/D3 caller exits (0/0/0/3) are in the same log at lines 18871, 18893, 18928 and 18934. Citing them, or the PR #52 RESULTS lines, would close the loop.
  - "당시 origin/main(ca08c76)" at the 17:36Z build depends on there being no fetch between 17:35:06Z (line 18769) and 17:40:41Z. That is true, and could be stated in one clause.
  - "run-real.sh가 이 바이너리를 `<W>/bin`에 복사했다" comes from the script (line 34) plus the argument passed, not from printed output. The 23:13 pre-swap hash corroborates it.
