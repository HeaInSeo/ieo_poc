# Review · PR #42 (report/0204-run-guard-20261114) · head 2d8b48850b9aa3959621b35589a60fab40f1b179

Reviewer: independent reviewer (read only, no repo/GitHub writes) · 2026-11-14 · review 1
Head confirmed: `gh pr view 42 --json headRefOid` = 2d8b48850b9aa3959621b35589a60fab40f1b179 (OPEN, base main). Local worktree HEAD is the same SHA, tree clean. One commit on origin/main b5f25d2. PR CI at review time: package validation pass, toolchain pass, rust fmt/clippy/test pending.

## Verdict: APPROVE

Every factual claim checked matches GitHub and the repo. Counts stay at 11/73 and 0/12. 02-04 stays `진행` / unverified. X tasks are not counted. The 8 archived reviews are byte-identical to their originals. The three scripts pass. No secrets. The findings below are Low/Info wording and traceability points. None blocks.

## Checks run

- `git diff origin/main...HEAD`: 14 files. state.json, events.jsonl (+2 events), the 4 generated views, and 8 archived reviews. No code, plan or ledger changes.
- GitHub:
  - PR #39: head 0bb842f, merged as f582403.
  - PR #40: head 23fee80, merged as ccf7291. Its 17 commits include corrections 1, 2 and 3 ("correction 3 (last)"). That gives reviews 4/4 (3103933, d2989f7, 5136d66, 23fee80) and corrections 3/3.
  - PR #41: head b3cb083, merged as b5f25d2. Its commits include correction 1 (ff49e6f) and correction 2 (b3cb083). That gives reviews 3/4 (de64b7b, ff49e6f, b3cb083) and corrections 2/3.
  - `gh run list --branch main`: 43590448077 on ccf7291 is a success, as claimed. 43441103331 and 43442655596 are unchanged and still successes. Run 43591229295 on b5f25d2 was in progress (rust job). The diff makes no CI claim for b5f25d2, which is correct.
- Archived verdicts: 3103933, d2989f7 and 5136d66 are CHANGES REQUESTED, and 23fee80 is APPROVE. For PR #41, de64b7b and ff49e6f are CHANGES REQUESTED, and b3cb083 is APPROVE. 0bb842f is APPROVE. These match the event and result_state text.
- Tests and mutants:
  - `crates/ieo-supervisor/tests/run_guard.rs` has 25 `#[test]`.
  - `evidence/cargo-test.log` shows 25 passed in run_guard and 20 in the other target, i.e. "시험 25건". The 20 belong to progress 1.
  - `evidence/mutants.txt` (at 1ccd75d) has 27 lines with `killed`, 0 survived, and "dirty after run: none". That gives 27/27.
  - `git diff 1ccd75d 23fee80 -- crates` is empty.
  - `run::run`, `run::charge` and `Limits::for_next` exist in `src/run.rs`. `for_next` wraps `budget::next`.
- Model calls: 0. Run-guard PROGRESS line 8 says "0회(가짜 claude 셸 스크립트만 사용)".
- Counts:
  - `validate_package.py`: rc 0, accepted_main_tasks 11, main_denominator 73, followup_denominator 12, generated_views_match true.
  - STATUS shows 주계획 11/73 and 후속 실험 0/12.
  - state.json tasks (85): 11 검수 완료, 1 진행 (02-04, unverified), 73 미착수.
  - Changed task rows: only 02-04 (status unchanged, 진행). Changed extra rows: only X-03 and X-04 (both still 미착수, unresolved text only).
- `render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl). `test_render_progress.py`: RESULT PASS 0. The state.json sha256 6183e29b…e05fa matches the view headers.
- Byte identity: `cmp` of all 8 archived reviews against `$(git rev-parse --git-common-dir)/ieo-dev-workspace/pending-reviews/` gives SAME for all 8.
- Secrets: I scanned the added lines for real token shapes (sk-ant-…, ghp_, github_pat_, ntn_, AKIA, Bearer <value>). Nothing was found. The only matches for a generic grep are existing descriptive text, for example "sk-ant- 형태만 가림".

## Carry-over fidelity

- 02-04 unresolved compared with review-pr40-23fee80.md:
  - F1 (cleanup_failed or a `run` error → stop/report, or `charge` unmeasured): carried.
  - F2 (pin Y1, Y4 and Y7): carried.
  - F3 (end-to-end test that `charge` is what `budget::spent` reads): carried.
  - F4 (PROGRESS §2 last bullet): carried.
  - F5 (explicit unmeasured for a non-object value): carried.
  - F6: omitted. See finding 3.
- X-03 and X-04 compared with review-pr41-b3cb083.md:
  - X-03 "agent:repro는 guardrail 전용 …" matches finding 2 (Info).
  - X-04 "PR 설명·보고에 supervisor 클라이언트 전환을 명시(장부 수용 기준에는 없음)" matches finding 1 (Low).

## Findings

1. **[Low] "PROGRESS §2·§3" in 02-04.unresolved now points at the wrong report.**
   - `latest_report` is now run-guard-20261114/PROGRESS.md. In that file, §3 is the 3103933 review table. The same unresolved field also uses "PROGRESS §2 마지막 항목" for the run-guard file.
   - The following "PROGRESS §2·§3(실제 세션 시험 전부 미실행, 토큰 상한은 회차 사이 확인만·회차 중 정지는 어댑터 단계, …)" refers to supervisor-core-20261113/PROGRESS.md.
   - The phrase "토큰 상한은 회차 사이 확인만" is also partly superseded: `run::run` now stops on the observed usage during a run, and only the wiring is left for the adapter step.
   - Suggest in a later ledger update: name the file, e.g. "진행 기록 1 PROGRESS §2·§3", and reword the token-cap item to "회차 중 정지는 run::run에 있음, 어댑터 연결 전".
2. **[Low] The gRPC report obligation stays open-ended.**
   - User_Decisions_2026-11-14_grpc.md line 14 says "다음 정규 보고에 반영 위치와 실행 순서를 포함한다".
   - This is the first report PR after that decision, and RESUME next_action already gives the order (X-02 → 02-04 실제 시험 → X-03 → X-04 → 02-05, X-05 after X-03 and not at the same time as 02-04).
   - Even so, next_action still says "다음 정규 보고에 gRPC 반영 위치·실행 순서 포함" as a future duty, and the PR body has no placement and order section.
   - Either say that this report discharges it (the placement is X-03..X-07 in A4 and the order is as above), or say which report will.
3. **[Info] Final-review F6 (show `incomplete_lines` for an exited run) is not carried.** The review calls it an optional clean-up, so the omission is defensible. The carry-over note says "P3·정보" but lists only five of the six items.
4. **[Info] The severity label differs from the source.** The ledger says "P3·정보", while review-pr40-23fee80.md uses Low and Info. The meaning is the same, and the label follows the earlier ledger convention.
5. **[Info] review-pr39-0bb842f.md is archived but not referenced.** The file is in `reports/extra/X-01/followup-c1-c2-20261113/reviews/`. No state.json evidence entry and no report links to it. In the same way, the PR #41 reviews de64b7b and ff49e6f are reachable only through the directory, because the event cites only b3cb083. This is acceptable for an archive. A later ledger update could add the PR #39 review to the X-01 evidence.
6. **[Info] Main CI for b5f25d2 (PR #41 merge) was still running at review time.** This PR claims nothing about it, which is correct. Record its result in the next report.

The counts are unchanged at 11/73 and 0/12, and 02-04 is not accepted.
