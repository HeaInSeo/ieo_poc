# Review · PR #45 (report/x02-prep-url-20261114) · head 444970e82961abe34873f9fe18e66dc756a167ec

Reviewer: independent reviewer (read only, no repo/GitHub writes) · 2026-11-14 · review 1
Head confirmed: `gh pr view 45 --json headRefOid` = 444970e82961abe34873f9fe18e66dc756a167ec (OPEN). Local worktree HEAD is the same SHA, tree clean, one commit on origin/main 778ee47. PR CI at review time: package validation success, toolchain success, rust fmt/clippy/test in progress.

## Verdict: APPROVE

Every factual claim checked in the diff and the PR body matches GitHub and the repo. Counts stay at 11/73 and 0/12; 02-04 stays `진행`; X-* are not counted. The three archived reviews are byte-identical to their originals. The scripts pass. No secrets. The findings are Low/Info traceability and wording points; none blocks.

## Checks run

- `git diff origin/main...HEAD`: 10 files — state.json, events.jsonl (+1 event X-02-PREP-URL-20261114), the 4 generated views, 1 new evidence file, 3 archived reviews. No code or plan changes.
- GitHub:
  - PR #42: head 2d8b488, merged as 86aed04.
  - PR #43: head 5457cb3, merged as 42b435e. Archived review 5457cb3: `VERDICT: approve`.
  - PR #44: head 65d56ce, merged as 778ee47. Archived review 65d56ce: `VERDICT: approve` (P3 x2: connector-path pin N3, `url_page_id` delegation pin N5; host allow-list out of scope). Body and ledger "remaining" items match.
  - Archived review 2d8b488 (PR #42): APPROVE.
  - `gh run list --branch main`: 43592324926 on 42b435e = success (as claimed). 43593112437 on 778ee47 = success, completed 12:07:37Z, i.e. after the snapshot observed_at 21:07:29+09:00 (12:07:29Z) — it was still in progress when the ledger was written; the diff and body make no CI claim for it, which is correct. 43591229295 (b5f25d2) and 43591506594 (86aed04) = success, not mentioned.
  - RESUME branch_sha main 778ee47808ae1532628c8f34c3d9aa12d2dcbec4 = origin/main.
- Evidence `x02-url-live.txt`: header main 778ee47, 12:06:44Z; backup line precedes changes; before counts 3/3/4/10/0; read-only SQL inside `BEGIN READ ONLY` returns `3|0|0|1` (imports, non-canonical, pages under two test ids, distinct pages); mismatching URL → `SCHEMA_VIOLATION source.url` HTTP 400; after counts identical. Body and event ("3·0·0·1", "400(source.url)·행 불변") match. File is added in this single commit, not edited afterwards.
- gRPC table vs `Plan_gRPC_Internal_API_v0.md` §6 and ledger `extra_tasks.prerequisites`: X-03←02-03, X-04←X-03, X-05←X-03, X-06←X-04,X-05, X-07←X-05 — all match. 02-05 prerequisites "02-03, 02-04 (unchanged)" match Execution Ledger line 87; plan line 90 says 02-05 gets no new X-03/X-04 prerequisite; the 02-05 task row is unchanged in this diff. X-04 "target before 02-05, not a prerequisite, HTTP fallback" and X-05 "after X-03, never concurrent with 02-04 real sessions", X-07 "optional, never blocking" match §6 rows and steps 4–7. All X-03..X-07 are 미착수 in state.json.
- 02-04 unresolved rewrite: `crates/ieo-supervisor/src/run.rs` `on_line` (called from `watch` for every stdout line during the run) stops with `Stop::TokenCap` when `usage.observed.billable >= limits.billable_room` and `Stop::CacheReadCap` when `observed.cache_read >= limits.cache_read_room` (lines ~460–464); `Observed.billable` is documented as a lower bound (module doc lines 26–32, run-guard PROGRESS §2). So "토큰 상한의 회차 중 정지는 … run::run으로 구현됨(관측 하한 기준), 남은 것은 어댑터 연결" is accurate. The "PROGRESS §2·§3" reference now names supervisor-core-20261113/PROGRESS.md, whose §2/§3 contain the cited items (실제 세션 시험 미실행, 기동·잠금 해제 직렬화, 디버그 로그 끔). This discharges PR #42 review finding 1. No remaining "회차 사이" wording in state.json or the views.
- Counts: `validate_package.py` rc 0, accepted_main_tasks 11, main_denominator 73, followup_denominator 12, generated_views_match true. state.json tasks: main 11 검수 완료 / 1 진행 (02-04) / 61 미착수; followup 12 미착수. Changed task rows: 02-04 (evidence, unresolved only); changed extra rows: X-02 (last_observed_at, evidence, unresolved; status still 조건 대기, 미실행/unverified).
- `render_progress.py --check`: ok (STATUS, TASKS, RESULTS, RESUME, events.jsonl). `test_render_progress.py`: RESULT PASS 0. state.json sha256 26d45f96…edc1c matches the view headers.
- Byte identity (sha256 of `git show HEAD:<path>` vs `pending-reviews/`): review-pr42-2d8b488, review-pr43-5457cb3, review-pr44-65d56ce — all identical.
- Secrets: scanned added lines for sk-ant-, ghp_, github_pat_, ntn_, AKIA, Bearer <value>, private keys, passwords. Only match is descriptive text inside the archived PR #42 review. Evidence file has no credentials or paths to the secrets directory.

## Findings

1. **[Low] "사용자 결정 2026-11-14 §5" is ambiguous.** Two decision files carry that date. The X-02 prep item is §5 of `User_Decisions_2026-11-14_pr30.md`; §5 of `User_Decisions_2026-11-14_grpc.md` is the gRPC common regression. The body and the new event cite only the date (and the body's gRPC header cites "2026-11-14 §1", which is the grpc file). Name the file (as 02-04/X-02 entries already do for the 2026-11-13 decision) in the next ledger update.
2. **[Low] X-02 unresolved does not carry all of pr30 §5.** §5 bullet 4 (report which parts of the exception record — branch, base SHA, no-merge/deploy — the server enforces vs. operational rules; copy with both source and fingerprint changed left to independent semantic review) is not tracked anywhere in state.json. Pre-existing, not introduced here, but this PR rewrites X-02 unresolved under §5; add it as a remaining X-02 precondition.
3. **[Info] Main CI results not recorded.** 43593112437 (778ee47, PR #44 merge) was still in progress at snapshot time and correctly not claimed; it has since completed success. 43591229295 (b5f25d2, PR #41 merge — PR #42 review finding 6 asked for it to be recorded) and 43591506594 (86aed04, PR #42 merge) are success but not mentioned. Record them in the next report.
4. **[Info] Evidence health line is truncated** (`…"schema_version":"ieo-coord-v0","sco`). It does not carry the build SHA; "redeployed at main 778ee47" rests on the evidence header only. Acceptable; a future live record could print the full health JSON or the binary's build id.
5. **[Info] One unnamed PROGRESS reference remains** in 02-04 unresolved: "PROGRESS §2 마지막 항목에 charge 미계측 명시" (inside the PR #40 23fee80 carry-over) refers to run-guard-20261114/PROGRESS.md but does not say so, while the following clause now names supervisor-core. Context makes it inferable.
6. **[Info] PR #45's own rust CI job was in progress at review time** (validation and toolchain jobs succeeded). Merge after it completes.

Counts unchanged at 11/73 and 0/12; 02-04 not accepted; X-02 still 조건 대기.
