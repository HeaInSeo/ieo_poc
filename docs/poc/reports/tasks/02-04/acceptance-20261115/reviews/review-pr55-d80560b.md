# Review of PR #55, head d80560b (review 1 of at most 4; corrections capped at 3)

- **PR:** HeaInSeo/ieo_poc #55, 02-04 acceptance report (decision requested), ledger snapshot 0028, archived PR #54 reviews.
- **Head reviewed:** d80560b575b7737c1ce1d12ad28a650dfa582052 (confirmed with `gh pr view 55 --json headRefOid,statusCheckRollup`). Base: main 6aaf427.
- **CI at d80560b:** all three checks COMPLETED SUCCESS (docs/poc package and progress validation; rust toolchain pin resolves; rust fmt clippy test). The two Rust jobs were still in progress at the start of the review and finished green.
- **Reviewer:** independent review agent (Claude Code). No model calls. Read-only. Did not read /opt/rust/ieo_poc_runs or the secrets directory. No push, comment, merge or branch change.
- **Verdict: CHANGES REQUESTED.** The arithmetic, evidence files, D3 handling, budget and the §7 wording corrections are right. Three medium issues remain. (1) One acceptance row is labelled "충족" (met) although the contract's coordinator half is missing. (2) The real-run evidence is attributed to the role images without saying it came from a different image and an older binary. (3) Several open carry-overs, one of them a precondition for the spare-run decision, are not in §4 or §7. The user asked that every unmet or unconfirmed item appear in the report, so these block.

## What I ran

- Detached worktree at d80560b under the scratchpad (`rv55`, removed afterwards), using `/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python` from `docs/poc`:
  - `scripts/render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl)
  - `scripts/validate_package.py`: rc 0, schema_errors 0, 498 internal links checked
  - `scripts/test_render_progress.py`: RESULT PASS
- `git diff --name-only 6aaf427 d80560b`: docs only (11 files). `git diff --name-only 8fca0a0 6aaf427`: docs only. This independently confirms that the merge did not change any code.
- Every relative link in REPORT.md resolves.
- Secret scan of the diff (tokens, keys, `password=`): only prose about checks. No values.
- Compared the report with: spec `02-04.accept` (Console_Coordinator_Sprints_v0.json); Runtime_Contract_v0 §2, §2.1, §5, §7 (lines 155-179), §10, §11, §12; PROGRESS 1-8 (supervisor-core, run-guard, role-container, real-sessions PROGRESS/RUN_PLAN/RESULTS/SUPPLEMENT, child-usage, images); reviews pr48-3479675, pr53-1729c9e, pr54-adf760e, pr54-fa39d86; `state.json` 02-04 `unresolved` and the global `unresolved` on 6aaf427 and on the head.
- Budget cross-check against `real-sessions-20261115/evidence/real/budget-final.txt` and RESULTS §4.
- Binary hashes: compared `acceptance-20261115/evidence/build-images-6aaf427.txt` with `images-20261115/evidence/build-images.txt`.
- Read `crates/ieo-supervisor/src/serve.rs` (reconnect and coordinator-record handling), `crates/ieo-coord/src/roles.rs` and `tools/containers/build-images.sh` to check the claims.

## Verified as correct

- **Budget (§5):** 7 runs; 40,666 ms = 40.666 s, counted as 45 s; billable 33,948; cache_read 121,189. Remaining: 5 runs, 14,400 − 45 = 14,355 s, 1,466,052 and 14,878,811. The role rooms add up exactly: 968,638 + 497,414 = 1,466,052 and 9,887,878 + 4,990,933 = 14,878,811. Dev 6 of 8 used, Guardrail 1 of 4. All runs `measured`, D3 `corrected true`, nothing unmeasured. The budget is unchanged and no model calls are claimed.
- **Merge-commit binaries (§6):** `ieo-coord` 9253c2a1…, `ieo-supervisor` 0fb18b54… and `claude` bfb3703a… are identical in both build files. The image IDs differ, as expected from the revision labels. The re-run log agrees with §6: PID 1 UID 20000, 401, DB password 0/0/0, 15 tables, dev 200 / Guardrail 202, `safe_shutdown_confirmed` false with 4 `not_tracked` reasons, 0 leftover processes, SIGTERM 2.3/0.4/0.4 s with exit 0. The slower coordinator stop is disclosed.
- **D3:** the expected value 3 and the failure are kept. Observed (Read showed an empty line 4) is separated from inferred (the child counted it). The usage correction follows the user decision and is not double-counted.
- **Overstatement guards:** the D1..D5 sum is called a check, not proof of attribution (§2). The unattributed D4 share is "압축 가능성이 크지만 호출별 기록 없음" (likely compaction, but no per-call record), not asserted as compaction. In-run real stop is "미실행(04-05 범위)" (not run, 04-05 scope), based on the plan and contract comparison in PROGRESS 7 §5. The spare runs (dev 2, Guardrail 3) are unused. The merged results reports (PR #52, #53) are explicitly not 02-04 acceptance. 11/73 and "진행" are unchanged.
- **§2 §7 status row:** "부분 충족" (partly met) with the three limits from review fa39d86 M-a is accurate. It is more conservative than PROGRESS 8 §5.
- **§7 corrections:** PR #48 F1 (1,800 s and 3,600 s per run), F2 (RUN_PLAN 7 + 5 spare), F3 (15 s, R5) and F4 (X-02 `next_action` = "02-04 수용 → …", i.e. after 02-04 acceptance) match review 3479675 and `state.json`. The PR #53 L1 correction (`wireToolInputs` of the tool-call record just before the cited line; 19665 and 19691 started with `cd`) matches review 1729c9e L1. The PR #54 L-a, L-b, L-c and adf760e L-2 summaries match the archived reviews.
- **Decision options (§4):** labelled as the author's summary. They cover accept-with-carry-overs, finish-first and other, and do not presume an outcome. Neutral enough, apart from the missing precondition in M3.

## Findings

### M1 (Medium): "외부 영속 저장소 복원 — 충족(실제)" (external persistent store restore: met, real) omits the coordinator half of the contract
- **Location:** REPORT.md §1, row "외부 영속 저장소 복원" (line 26). Line 6 says §2 of the contract is compared, but no §2 or §5 checkpoint-reporting row exists.
- **What:** the evidence (R1, the binary-swap restart, D2-D5) shows restore from the **role-volume** checkpoint. Contract §2.1 says the canonical checkpoint and verdict record is the **coordinator DB and external state**. §5-1 says the supervisor reports the checkpoint digest and sequence to the coordinator. §5-2 says reconnect compares against the coordinator record. §5-6 says `config_tamper_detected` is reported "체크포인트와 코디네이터에" (to the checkpoint and to the coordinator). In the code, the coordinator record is a `coordinator` object that the out-of-container caller puts in each request (`serve.rs` around line 386; `images-fake.sh`/`run-real.sh` bodies). `ieo-coord` stores no checkpoint digest or sequence and receives no tamper report: `roles.rs` reads only the safe-shutdown fields. The plan's architecture also lists "DB/외부 영속 저장소" (DB/external persistent store) separately from the repo Pod (V1_3_Changes_and_Validation.md:39). So "외부 영속 저장소 = role volume" is an interpretation.
- **Fix:** label the row "부분 충족" (partly met), or "충족(역할 볼륨 기준, 해석)" (met, role-volume basis, interpretation). State that coordinator-side checkpoint digest/sequence reporting, the comparison against a coordinator-held record and the tamper report to the coordinator are not implemented. Add this to §4: with item 2 → 02-05, or as a separate decision.

### M2 (Medium): the real-run evidence for supervisor PID 1 came from another image and an older binary; the report does not say so
- **Location:** REPORT.md §1 row 1 ("충족 | 실제·모의 | 실제 회차의 PID 1 supervisor(진행 기록 6), 역할 이미지의 PID 1(진행 기록 8·이 보고)"; met, real and mock, PID 1 supervisor in the real runs and PID 1 of the role images) and row 3 (U3).
- **What:** PROGRESS 8 §3 says no real Claude run used the role images. The real runs (PROGRESS 6) used the generic `golang@sha256:d9c68c2c…` image with mounted binaries, `ieo-supervisor` 652fc14f… (D1-D3) and db2162c2… (D4-D6; SUPPLEMENT lines 62-63). The image binary is 0fb18b54… and contains the PR #54 changes: §7 status fields, safe-shutdown report and `tracked`. Row 2 states the equivalent caveat for the coordinator ("실제 Claude 회차와 함께 돌린 적은 없다", it has never run together with a real Claude run). Row 1 does not, so the "실제" (real) label reads as covering the shipped role images.
- **Fix:** add to row 1 (and the §6 bullet): "실제 회차는 범용 이미지에 이전 supervisor 바이너리(652fc14f/db2162c2)를 마운트한 구성. 역할 이미지(0fb18b54)와 PR #54 변경분은 모의만" (the real runs mounted the earlier supervisor binaries 652fc14f/db2162c2 into a generic image; the role images (0fb18b54) and the PR #54 changes have mock evidence only). Optionally list "역할 이미지로 실제 회차" (a real run on the role images) as an option under §4 item 7.

### M3 (Medium): open carry-overs missing from §4 and §7, one of them a precondition for decision 7
- **Location:** REPORT.md §4 (items 1-7) and §7. Neither points to the "남음" (remaining) table of PROGRESS 8 §4.
- **What:** these PROGRESS 8 §4 "남음" rows are not represented:
  - (a) The low caller carry-overs (PR #51 71e408a P1-P4, 12c9a21 N6, 608b367 F8; PR #49 dc11612 L-c3-2..4; the deadline-stop test). PROGRESS 8 says to handle them "실제 회차를 다시 하기 전에" (before any further real run). Decision 7 offers to spend spare runs "위 6이나 다른 실제 확인에" (on item 6 or other real checks) without this precondition.
  - (b) The empty `projects/*/memory` digest branch (f73ac13 P3).
  - (c) 02-01→02-02 P3-3, P3-4 and P3-6.
  - (d) The §2.1 limit that the same UID can rewrite run records.
  - (e) The dev allowlist (03-02).
  - (f) Rebuild reproducibility, which §6 mentions only as "확인하지 않았다" (not checked).

  The global `unresolved` entry "02-04 진행 기록 1 … 범위 밖 읽기 불가 파일 미개방 시험 없음" (no test for not opening unreadable out-of-scope files; supervisor-core PROGRESS §3, surviving mutant bc55edf R5) appears in no triage table: not PROGRESS 8 §4, not this report. It is still open on the head.
- **Fix:** add a short "기타 이월(02-04 수용 영향 없음으로 분류)" (other carry-overs, classed as no impact on 02-04 acceptance) list, or a sentence pointing to the PROGRESS 8 §4 remaining table, with the bc55edf R5 item triaged. Add to §4 item 7: "예비 회차 사용 전 호출자 낮은 이월 처리 필요" (the low caller carry-overs must be handled before any spare run is used).

### L1 (Low): stale text in `state.json` 02-04 `unresolved` (and the rendered views)
- **Location:** `docs/poc/progress/state.json` 02-04 `unresolved`.
- **What:** two sentences carried over from 6aaf427 are now false or done. The first, "이 PR의 이미지는 PR 커밋 8fca0a0의 디버그 빌드(디버그 정보 없음), 병합 커밋 재빌드 전" (this PR's images are the debug build of 8fca0a0, before the merge-commit rebuild), is superseded: this PR rebuilt at 6aaf427. The second, "PR #53 검토 1729c9e L1 … 수용 보고에서 정정" (to be corrected in the acceptance report), is now done in §7.
- **Fix:** replace them with "병합 커밋 6aaf427 재빌드·재시험 완료(바이너리 해시 = 8fca0a0), 디버그 빌드" (merge commit 6aaf427 rebuilt and re-tested, binary hashes = 8fca0a0, debug build) and "L1 정정 완료(수용 보고 §7)" (L1 corrected, acceptance report §7). Then re-render.

### L2 (Low): §1 and §2 do not show the unmet 02-03 carry-overs or the unconfirmed items
- **Location:** REPORT.md §1 and §2.
- **What:** the user asked for met / unmet / unconfirmed. §4 item 1 says the 02-03 carry-overs count as **unmet** until confirmed, but the condition tables show no unmet row, and "미확인" (unconfirmed) never appears as a status, although the safe-shutdown condition is unconfirmed. A reader of §1 alone sees "one partial, the rest met".
- **Fix:** add a §1 row "02-03 이월(검수된 02-03이 02-04 수용 전으로 정함) | 미충족(이동 확인 전) | 문서 | §4-1" (02-03 carry-overs, which the accepted 02-03 set as before 02-04 acceptance | unmet until the move is confirmed | document | §4-1). In the §7 repo-stop row, write "부분 충족(안전 종료 미확인)" (partly met, safe shutdown unconfirmed).

### I1 (Info): small precision points
- Line 6 lists "§2, §7, §10, §11", but §2 of the report also covers §12 item 8.
- §10 item 1's completion condition also includes the argument-generation test (mock), so "실제·모의" (real and mock) is the exact label.
- §6 "바이너리 해시가 … 같다. 병합 사이에 코드 변화가 없다" (the binary hashes are the same; no code changed across the merge): `build-images.sh` takes a caller-supplied `CARGO_TARGET_DIR`. If the same target directory was reused, equal hashes reflect cargo reuse rather than an independent rebuild. The "no code change" claim is better grounded in `git diff --name-only 8fca0a0 6aaf427` (docs only), which I confirmed. Cite it.

## Summary for the author

Fix M1-M3 and L1. L2 and I1 are recommended. No change to statuses, counts or budget is needed. All fixes are wording or additions in REPORT.md and `state.json` `unresolved`, followed by a re-render.
