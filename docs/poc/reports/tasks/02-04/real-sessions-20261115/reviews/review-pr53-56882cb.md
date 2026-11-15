# Review of PR #53 (02-04 real-session evidence supplement, progress record 7)

- **Head reviewed:** 56882cbf2efdd46d2715be3ff5fcdb0090510fbb (confirmed with `gh pr view 53`). Base: main 6626558. One commit.
- **Reviewer:** independent reviewer agent (Claude Code subagent). This is review 1 of at most 4.
- **Verdict: CHANGES REQUESTED.** There are three Medium findings, all about documentation. No code or number in the PR is wrong. The fixes are additions or relabelling inside SUPPLEMENT.md and the evidence files, with no model call and no rerun.

## What I ran or checked

- **CI and PR state.** `gh pr view 53 --json headRefOid,statusCheckRollup`: 3/3 SUCCESS at 56882cb. The CI log for run 43654041031 shows the new step `test_summarize.py`, with `RESULT PASS 0`.
- **Worktree.** I created a detached worktree at 56882cb under `scratchpad/rv53`, and removed it at the end.
- **Package and progress scripts:** `render_progress.py --check` ok; `validate_package.py` rc 0; `test_render_progress.py` RESULT PASS 0.
- **Summarizer test:** `test_summarize.py` passed 9/9.
- **Summary regeneration.**
  - I regenerated all seven summaries (D1 G1 D2 D3 D4 D5 D6) from the live ledger `/opt/rust/ieo_poc_runs/02-04-20261115/ledger/records.jsonl` (sha256 f98ebd52…) with the PR's `summarize.py` (sha256 24cd2de3…).
  - All seven are byte-identical to `evidence/real-v2/`.
- **v1 vs v2 comparison.**
  - The only differences are the `children` key and the new `provenance` key, as stated.
  - D3 is `present`, with child a13188dc0170f8901 and its kept transcript 8e509473…, sourced from the correction line.
  - D1, G1 and D2 are `confirmed_none`, with the "not recorded in this record format" note. D4, D5 and D6 are `confirmed_none`, with record `children` = [].
  - In all seven, `stream_matches_record` is true.
- **D4 numbers, checked against the ledger and the D4 stream (read-only, nothing copied).** Every number in SUPPLEMENT §2 matches:
  - The base, D3, has session cumulative 18/335/76,801/11,535.
  - D4's cumulative is 6,706/1,490/88,194/12,677, so the share is 6,688/1,155/11,393/1,142.
  - `model_usage_since_last_result` gives the same figures, and so it does in D2, D3 and D5.
  - `num_turns` is 0 and parent usage is 0/0/0/0. `usage.children` is empty.
  - There is one `compact_boundary`: manual, pre 17,230, post 3,405, 8,151 ms. `attribution_check` is `mismatch`.
- **D1..D5 sum.** The shares add to 6,708/1,497/99,587/19,132, which equals D5's cumulative.
- **Collector rule for compaction.** `crates/ieo-evidence/src/stream.rs` (lines ~397–440) applies "≥ parent+children in every field, all parts confirmed". This matches the text in §2.
- **Ops-log extract.** It is verbatim. All five commands and every output line match the private work-session log at the cited lines 19626, 19645, 19651, 19660 and 19665, after the `<scratch>` substitution. The timestamps are those of the tool results.
- **Requery.** I re-ran `podman container inspect` and `podman image inspect`; image id, digest, ImageName, StartedAt and User all agree with `requery.txt`. The deployed `<W>/bin` hashes are dde8db51… and db2162c2… .
- **Merge and CI.** PR #51 head 71e408a was merged as d41e70f. Main CI run 43651860463 succeeded at d41e70f. `git diff d41e70f 6626558 -- crates Cargo.lock Cargo.toml rust-toolchain.toml` is empty.
- **D3 observed fact.** In both the D3 stream and the kept child transcript, the Read result has numbered lines 1–4 with line 4 empty. The other lines are 29+11+10 characters, which is 53 bytes with newlines. The child's answer and the final answer are both `4`.
- **Five conditions.** I compared them with child-usage PROGRESS §11. Exit codes for D1–D6 and G1 are all 0. There are 8 ledger lines (7 runs + 1 correction), with no repeated attempt id. Every run has `overageStatus` rejected.
- **Scope documents.** I read Sprints v0 02-04 and 04-05 acceptance, Validation Plan P05-R (line 312), runtime contract §7, §10, §11 and §12, and User_Decisions_2026-11-14_pr30 §3 and §4.
- **Secrets and privacy scan.** I scanned all added files. They contain only env-var names (`CLAUDE_CODE_OAUTH_TOKEN`) and repository-local paths, which are already public elsewhere. There is no credential, raw session text or transcript text. The ops-log replaces the scratch path with `<scratch>`.
- **Progress record.** 02-04 stays `진행` and `unverified`, and 11/73 is unchanged. No earlier report or `evidence/real/` file was modified.

## Findings

### M1 · Medium · SUPPLEMENT.md §3 row "실행 바이너리"; ops-log extract — the pre-swap binary's link to its commit is unsourced, and the binary was not kept

- **The claim.** SUPPLEMENT says "23:13:11Z 교체 전(ca08c76 빌드 652fc14f…)". The extract shows only the hash 652fc14f at 23:13. Nothing public ties 652fc14f to ca08c76. Yet that binary ran D1, G1, R1, D2 and D3, which is most of the real evidence. The user's item 3 asks for the executed binary and its approved source.
- **The evidence exists.** The work-session log has it at line 18816 (2026-11-14T17:36:17Z, "Build binaries from main, then free disk"). That entry shows `bin-ca08c76/ieo-supervisor 652fc14f…` and `ieo-role-call c61c6380…`, produced by a `cargo build --locked` preceded by an empty `git diff --stat origin/main -- crates …`.
- **Pre-swap binary not kept.**
  - In the 23:13:11Z swap command, `cp $W/bin/ieo-supervisor $A/../preserved-02-04-D3-20261115/ieo-supervisor-ca08c76 2>/dev/null` ran with `$A` unset, because shell variables do not persist between tool calls.
  - The copy therefore failed silently. No `ieo-supervisor-ca08c76` exists in the preserved directory.
  - The supplement does not claim the binary was kept, but it does not say it was lost either.
- **Fix.**
  - Add the 17:36Z build entry, verbatim with its line number, to the ops-log extract. Alternatively, mark the ca08c76 link as "not confirmed".
  - State plainly which binary ran D1, G1, R1, D2 and D3 (652fc14f / c61c6380), and which ran D4–D6 (db2162c2 / dde8db51).
  - Under "확인하지 못한 것", note that the pre-swap binary was not preserved and that the copy failed because `$A` was unset.

### M2 · Medium · SUPPLEMENT.md §3 row "이미지" — a current re-query fact sits in the "당시 기록" column

- **The problem.** The "당시 기록" cell reads "run-real.sh가 digest로 실행(PR #49 RUN_PLAN). 생성 시각 2026-11-15 02:40 KST".
  - The creation time comes from today's `podman container inspect` (requery.txt), not from a record made at the time.
  - run-real.sh and RUN_PLAN are plan and script documents, not an execution record.
  - The user instruction is not to use current re-queries as proof of the past.
- **The evidence exists.** The work-session log has a contemporaneous container-start output at 2026-11-14T17:40:41Z (log line 19260). It contains `fixture0204` and the `d9c68c2c` digest, and matches the 02:40:40 KST creation.
- **Fix.**
  - Either add that extract to the ops-log and cite it in "당시 기록", or move the creation time to "현재 재조회".
  - In the second case, mark the at-the-time image evidence as "script pins digest (document); container config re-queried now".

### M3 · Medium · SUPPLEMENT.md §5 — the scope comparison for an in-run real stop leaves out a directly relevant clause and gives no timing

- **Missing clause.** User_Decisions_2026-11-14_pr30 §3 lists "회차별·전체 시간 상한에 따른 종료와 하위 프로세스 정리" as a connection to check *without model calls*. It also says pre-checks must not be recorded as in-run enforcement.
  - This is the user's own statement on where in-run stop belongs for 02-04, and it supports the PR's conclusion: mock is enough for 02-04, and a real stop is later.
  - §5 cites only pr30 §4, and reads its "종료" as a normal container shutdown. That reading is the author's interpretation and should be labelled as one.
- **Missing contract row.** Runtime contract §7 has a "stop (role 범위)" row (SIGTERM → 관측 → SIGKILL, own Pod only). It is the 02-04 adapter mapping and should be part of the comparison.
- **Missing timing.** The user asked, for a follow-up scope, for owner task, timing and criteria.
  - Owner (04-05) and criteria are given.
  - Timing is not: 04-05 starts after 04-04 and 02-04 are accepted, its budget is not approved, and it waits for a user decision.
- **Fix.**
  - Add pr30 §3 and contract §7 to the comparison table, and mark the reading of §4 "종료" as an interpretation.
  - Add the timing and prerequisites for 04-05.
  - Optionally, list "scope classification (02-04 vs 04-05)" as one of the questions in the decision draft, so the user confirms it.

### L1 · Low · SUPPLEMENT.md §2 "귀속하지 못한 부분" — one fact has no evidence class

- **The fact.** "부모 전사에는 압축 경계·요약 항목만 있고 그 호출의 사용량 기록은 없다" is a fact about the parent transcript in the role volume. It is not in the ledger or in the public evidence.
- **Where it came from.** It was checked at 2026-11-14T23:30:43Z ("Look for direct attribution of D4 compaction usage"), so it is a current re-query, not a record from the run.
- **Fix.** Label it "현재 재조회", and add its counts to requery.txt with no content: transcript line count, the line kinds at the compaction, and the absence of a usage field.

### L2 · Low · SUPPLEMENT.md §2 "적용한 조항과 조건" — contract clause and collector rule are blended

- **The problem.** Contract §11 item 7 and §12 item 8 define only the no-compaction case. The rule for the compaction case ("≥ in every field, all parts confirmed") is a collector rule. It came from review 608b367 F5 and is implemented in `stream.rs` in d41e70f; it is not a contract clause.
- **Fix.**
  - Say this explicitly and cite the code location and PROGRESS §9 F5.
  - Optionally note that the contract's own basis for §11 item 7 (01-04 L3: `result.usage` 0, with the cumulative increase being compaction usage) is the earlier real observation of the same shape. The non-assertion that the whole share is compaction cost is correct and should stay.

### L3 · Low · summary `provenance` and SUPPLEMENT §1 — line hashes are over the private ledger

- **The problem.** `record_line_sha256` and `correction_line_sha256` (for example D3 7f162a66…, correction 29d47911…) are hashes of the unredacted private ledger lines. The public copy `evidence/real/records.jsonl` replaces paths with `<W>`, so its lines hash differently (839807a9…, 49b1e8a5…). A public reader cannot verify these hashes.
- **Fix.** State in SUPPLEMENT §1 (and optionally in the summary's provenance) that the line hashes are over the original ledger outside the repository. Optionally also record the public copy's line hashes.

### L4 · Low · SUPPLEMENT.md §3 five-condition table, row 5 — half of the condition is missing

- **The problem.** PROGRESS §11 condition 5 also says "어느 회차든 종료 코드 0·2 밖이면 다시 멈추고 보고한다", and the table omits it.
- **Fix.** Add the evidence for it: every run's `exit_code` is 0, as the v2 summaries show.

### L5 · Low · test_summarize.py — the "old D3 record" test uses only a synthetic record

- **The problem.** The regression test builds a synthetic record shaped like the old D3. The actual old D3 record and its correction are public at `evidence/real/records.jsonl` lines 4 and 5, with paths redacted.
- **Suggestion.** Add one case that runs `summarize(open(evidence/real/records.jsonl).read().splitlines(), "D3", read_stream=False)` and asserts `present` with the correction source. This guards the real shape, not only the synthetic one.
- **Optional cross-check.** When a stream is read, a run that has an `Agent` tool_use but an empty `usage.children` should not be reported as `confirmed_none`.

### I1 · Info · ops-log header

Each entry is labelled "tool call", but the timestamp is the tool-result (completion) time; for example, the swap command started at 23:13:07.98Z. Consider saying "result time".

### I2 · Info · SUPPLEMENT.md §6 table header

The header says "조건(명세 v0 02-04.accept)", but some rows come from other sources:
- "역할 전용 이미지" and "코디네이터 PID 1" detail come from progress record 3 §3.
- The contract §10 and §11 rows come from the runtime contract.

Consider naming the source per row. Separately, "어댑터 … 연결: 충족(stop은 모의)" is acceptable because the mock part is marked. The acceptance review should keep the adapter `stop` as mock-only. The real stops so far (R1, the restart, the final stop) were `podman stop` / `restart` of PID 1, not the adapter `stop`.

### I3 · Info · decision draft in §5

The "expected result" row cites the "guard가 멈춘 회차" rule. `run::charge_reported` treats every non-`Exited` stop as unmeasured, including `Requested`, so the expectation is right. The wording could cite "any non-Exited stop" instead.

## Items confirmed as satisfied (user instruction 2026-11-15)

1. **D3 summary.** The three states (`present`, `confirmed_none`, `unknown`) are separated, sources are given, and provenance and generator version are recorded. The regenerated summaries are compared, the v1 files are kept, and a regression test runs in CI.
2. **D4 compaction exception.** The compaction event, base, totals, parent/child usage, per-field differences and rule are all correct. The D1..D5 sum is described as a cross-check, not as proof. The share is not asserted to be all compaction cost, and the mismatch rule is not widened. L1 and L2 refine the wording.
3. **Binary and restart evidence.** Source commit, PID 1 hash, guard verdict, checkpoints, D3 base and the five conditions are present and verified. M1, M2 and L4 are gaps.
4. **D3 answer.** Expected value 3 and the failure are kept, and observation is separated from inference. The plan criterion is not met, contract §10 item 3 (c) is met through D4, and the final decision is deferred. D3 is not relabelled as passed.
6. **In-run stop.** The conclusion is reasonable and the stop is not marked as met. A decision draft is present. M3 covers the gaps in the comparison and the missing timing.
7. **Acceptance status.** Status is given per criterion with the evidence class. The merged results report is not treated as acceptance, and the completion count is unchanged.
- **No new model calls.** Spare runs stay at development 2 / Guardrail 3: development used 6 of 8, Guardrail 1 of 4. Budget and shares are unchanged.
