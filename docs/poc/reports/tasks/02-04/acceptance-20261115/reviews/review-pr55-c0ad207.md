# Review of PR #55, head c0ad207 (review 2 of at most 4; follows correction 1 of at most 3)

- **PR:** HeaInSeo/ieo_poc #55, the 02-04 acceptance report (decision requested), ledger snapshot 0028.
- **Head reviewed:** c0ad207d416968300c5940ee0c4cc0cba15ef6bf, confirmed with `gh pr view 55 --json headRefOid,statusCheckRollup`. Base: main 6aaf427 (`baseRefOid` 6aaf42719c1f…).
- **CI at c0ad207:** all three checks finished COMPLETED SUCCESS:
  - docs/poc package and progress validation
  - rust toolchain pin resolves
  - rust fmt clippy test (this one was still running when I started, so I waited for it)
- **Reviewer:** independent review agent (Claude Code).
  - No model calls.
  - Read-only on private directories. I did not read the secrets directory.
  - No push, comment, merge or branch change.
- **Verdict: APPROVE.**
  - All review-1 findings are resolved: M1, M2, M3, L1 and L2, plus I1 items 2 and 3. I1 item 1 is resolved differently from my suggestion, and that is fine.
  - The remaining findings are low or informational wording fixes in the ledger. None of them over-labels a condition or hides an open item, so none blocks.

## What I ran

- Detached worktree of c0ad207 at `scratchpad/rv55b`, removed at the end.
- From `docs/poc`, using `/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python`:
  - `scripts/render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl).
  - `scripts/validate_package.py`: rc 0, schema_errors 0.
  - `scripts/test_render_progress.py`: RESULT PASS.
- `git diff --name-only 6aaf427 c0ad207`: only `docs/` (11 files). `git diff d80560b c0ad207`: REPORT.md, `state.json` and the re-rendered views only.
- Every relative link in REPORT.md resolves, including `evidence/` and `../../../../User_Decisions_*`.
- **Contract:** read Runtime_Contract_v0.md §2.1 (lines 32-43) and §5 items 1, 2 and 6 (lines 130-147).
- **Code:** read
  - `crates/ieo-supervisor/src/serve.rs` lines 386-409: the `coordinator` object comes from the request.
  - `crates/ieo-coord/src` (`roles.rs`, `schema.sql`, `api.rs`): searched for checkpoint, tamper, digest and sequence.
- **Open items:** compared the report with:
  - `state.json` 02-04 `unresolved` and the global `unresolved` (head and main);
  - images-20261115/PROGRESS.md §4, both the "닫음" (closed) table and the "남음" (remaining) table.

## Resolution of review-1 findings

| Finding | Status | Evidence at c0ad207 |
| --- | --- | --- |
| M1 external store restore over-labelled | Resolved | See note M1 below. |
| M2 real runs used other binaries and image | Resolved | §1 row 1 now says the real runs mounted 652fc14f… (D1-D3) and db2162c2… (D4-D6) into the generic image. It says the role-image binary 0fb18b54… and the PR #54 changes have mock evidence only. |
| M3 missing carry-overs | Resolved | See note M3 below. |
| L1 stale ledger text | Resolved | 02-04 `unresolved` now says "L1은 수용 보고 §7에서 정정함" (L1 corrected in §7 of the acceptance report). It also says the images were rebuilt and tested at 6aaf427 (debug build). |
| L2 unmet and unconfirmed rows | Resolved | §1 has a new row "02-03 이월 … **미충족(이동 제안 확인 전)** \| 문서" (02-03 carry-overs: unmet until the move is confirmed; document). The §2 repo-stop row is now "부분 충족(안전 종료 미확인)" (partly met, safe shutdown unconfirmed). |
| I1 | Resolved / partly differently | Line 6 now lists §2, §5, §7, §10, §11 and §12. §10 item 1 is labelled 실제·모의 (real and mock). §6 now bases "no code change" on `git diff 8fca0a0 6aaf427` and notes that the equal hashes may come from reusing the cargo target directory. |

**M1 (resolved).** The changes:
- §1 now labels the row "**부분 충족**" (partly met).
- The row states that the coordinator-side checkpoint digest and sequence report, the comparison against a coordinator-held record and the tamper report to the coordinator do not exist.
- §2 has a new "§2.1·§5-1·§5-2·§5-6 … **미충족**" (unmet) row.
- §4 has a new item 9.

**M3 (resolved).** The changes:
- §4 item 8 lists the caller carry-overs: 71e408a P1-P4, 12c9a21 N6, 608b367 F8, dc11612 L-c3-2..4 and the deadline-stop test.
- §4 item 7 names item 8 as a **precondition** before any spare run.
- §7 has a new table "그 밖의 열린 이월" (other open carry-overs) with these rows:
  - memory digest (f73ac13 P3);
  - P3-3, P3-4 and P3-6;
  - the §2.1 same-UID limit;
  - the allowlist (03-02);
  - rebuild reproducibility;
  - bc55edf R5.

### Checks of the new content

- **Contract references are correct.**
  - §2.1 (line 41): the canonical record is the coordinator DB, and the supervisor reports checkpoint digest and sequence.
  - §5-1: "digest·순번은 코디네이터에 보고한다" (digest and sequence are reported to the coordinator).
  - §5-2: reconnect compares against the coordinator record digest.
  - §5-6: `config_tamper_detected` is reported "체크포인트와 코디네이터에" (to the checkpoint and to the coordinator).
- **The code claim is correct.**
  - In `serve.rs:386-409`, `generation`, `assignment_id`, `sprint_id`/`sprint_revision`, `spec_digest`, `image_digest` and `profile_*` all come from `req["coordinator"]`, which the caller sends.
  - `ieo-coord` has no checkpoint, tamper or checkpoint-sequence storage. `roles.rs:104-120` reads only `safe_shutdown`, including `checkpoint_saved`, for the stop report.
  - The plan citation ("DB/외부 영속 저장소", DB/external persistent store) is at V1_3_Changes_and_Validation.md:39.
- **Open items are complete.**
  - Every "남음" row of PROGRESS 8 §4 now appears in REPORT §4 or §7.
  - The 02-04 `unresolved` and global `unresolved` items are either covered, or closed in the PROGRESS 8 §4 "닫음" table. The closed ones are:
    - token-cap stop in mid-run;
    - fork/lock serialization;
    - debug log off;
    - f73ac13 P3-D (subagents).
- **Decision count.**
  - §4 has 9 items, and the `state.json` 02-04 `unresolved`, `next_action` and event all say "9건" (9 items). STATUS.md and TASKS.md render it.
  - Option (나) now refers to 1, 4 and 9, which is correct.
  - Option (가) moves 1-4 and keeps "남은 항목" (the remaining items) as carry-overs, so 9 is covered implicitly (see I-2).
- **Binding rules still hold.**
  - D3's failure is preserved.
  - Status stays "진행" (in progress), and 11/73 is unchanged.
  - No acceptance is marked. The report requests a user decision, and state changes happen only after review and that decision.
  - Real, mock, stored-log and document evidence stay distinct.
  - The budget is unchanged: 7/12 runs, 45 s counted, 33,948 billable, 121,189 cache_read.

## Findings

### L-1 (Low): `next_action` says "결정 9건" (9 decisions) but its parenthetical names only 7

- **Location:** `docs/poc/progress/state.json`, 02-04 `next_action`. It is rendered into STATUS.md and TASKS.md.
- **What:** the list in parentheses ends at "…, 실제 정지, 예비 회차" (…, real stop, spare runs). It omits item 8 (caller carry-overs, a precondition for item 7) and item 9 (coordinator-side checkpoint report and comparison, unmet). Someone reading only the ledger views sees 9 announced but 7 named, and the missing two are the newly added unmet ones.
- **Fix:** append "호출자 이월(예비 회차 선행), 코디네이터 쪽 체크포인트 보고·대조(미충족)" (caller carry-overs, required before spare runs; coordinator-side checkpoint report and comparison, unmet) to the parenthetical, then re-render.

### L-2 (Low): `resume.last_verified_stage` says the acceptance report is not yet submitted

- **Location:** `state.json` `resume.last_verified_stage` (around line 2479) and RESUME.md line 9. The text reads "02-04 진행(…, 수용 보고 제출 전·판단 전)" (02-04 in progress; acceptance report not submitted, not decided).
- **What:** this snapshot (0028) is the one that submits the report, and the event 02-04-ACCEPTANCE-REPORT-20261115 says "수용 보고 제출(판단 전)" (acceptance report submitted, not decided). The text was already present at d80560b, and review 1 missed it.
- **Fix:** change it to "수용 보고 제출·판단 전" (acceptance report submitted, not decided) and re-render.

### I-1 (Info): the label on §1 row "외부 영속 저장소 복원" (external persistent store restore)

The row's 구분 (evidence type) is "실제" (real). The restore half is real, but the unmet coordinator half rests on document and code comparison. "실제·문서" (real and document) would be exact. This is optional, because the §2 row already carries "문서" (document) for the unmet part.

### I-2 (Info): option (가)

Item 9 also carries a move-to-02-05 proposal, so (가) could read "1~4·9를 제안대로 옮기고" (move 1-4 and 9 as proposed) to say explicitly how the unmet coordinator-side item is handled under (가). As written, (가) covers 9 only implicitly, through "남은 항목을 이월로" (the remaining items as carry-overs). This is optional.

### I-3 (Info, outside this PR's diff): a stale global `unresolved` entry

The global `unresolved` entry "02-04 진행 기록 1(PR #31·#32): 연결 단계 주의 — …" still lists three items. PROGRESS 8 §4 has closed them:

- token-cap stop in mid-run;
- fork/lock serialization;
- debug log off.

Only "범위 밖 읽기 불가 파일 미개방 시험 없음" (bc55edf R5, no test that out-of-scope unreadable files are not opened) is still open, and REPORT §7 now carries it. Narrow the entry to R5 at the next ledger update. It is also on main and does not block this PR.

## Summary for the author

You may merge as is. L-1 and L-2 are one-line `state.json` text fixes plus a re-render. Fold them in if you make another correction, or carry them to the next snapshot. No change to statuses, counts, budget or condition labels is needed.
