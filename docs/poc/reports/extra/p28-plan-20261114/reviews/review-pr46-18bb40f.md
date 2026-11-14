# Review 2 · PR #46 · head 18bb40fbea741cbe9a5d49ab35ee7dbde7ba6d69

**Verdict: APPROVE**

Head confirmed: `gh pr view 46` headRefOid = 18bb40fbea741cbe9a5d49ab35ee7dbde7ba6d69, mergeStateStatus BLOCKED (awaiting review). At review time CI had passed "docs/poc package and progress validation" and "rust toolchain pin resolves", and "rust fmt clippy test" was still pending. The worktree HEAD equals the PR head and is clean. Correction 1 is a single commit (9e81a83..18bb40f) touching 9 files.

## Verification of review 1 findings

1. **Finding 1 (Medium): resolved.** POC-02, POC-04 and POC-07 each now end their `sources` with
   `{kind: user_decision, ref: User_Decisions_2026-11-14_p28.md, revision: 2026-11-14, digest: 07427691…1453, observed_at: null}`.
   The form matches the earlier A1/A2 entries. `sha256sum User_Decisions_2026-11-14_p28.md` = 07427691d3d61b609564aa53eeb6066ba34f7fa75624f7a907d3ee5d77091453, which matches. Nothing else changed in those sources (one entry added, none removed).
   - The canonical digests (sha256 of sort_keys compact JSON without `digest`) recompute correctly: POC-02 a4ac3483…, POC-04 4b69899c…, POC-07 9b1f588e….
   - The other six sprints are byte-identical to 9e81a83 and remain at rev 5.
   - No stale digest (c16299c6, e22bf4d2, f45c2229) or stale file hash (434b77f0, fee5a8d3, 46347c20) remains anywhere in the tree.
2. **Finding 2 (Low): resolved.** state.json resume.plan_and_spec now reads "명세 9개 중 6개 rev 5 · POC-02 rev 7 · POC-04 rev 6 · POC-07 rev 6". This is correct: six sprints are at rev 5 and three are revised. RESUME.md is regenerated to match, and "명세 8개" no longer appears anywhere.
3. **Finding 3 (Info): addressed.** One sentence is added to 02-08: "CI·독립 검토·승인·예산 등 필요한 조건이 남은 후속은 정상 대기로 두고 빠른 인계를 이유로 생략하지 않는다." It sits before "P28의 실제 시험을 대신하지 않는다."
   - It is faithful to decision point 5 ("CI·독립 검토·승인·예산 등 필요한 조건이 남아 있는 경우는 정상 대기로 구분한다. 빠른 인계를 이유로 기존 조건을 생략하지 않는다.") and adds no new authority.
   - The rev 7 profile item 02-08.accept, the task's acceptance.criteria, exit_condition and the ledger row are all byte-equal (checked programmatically).
   - Only `inspection_profiles[rev 7].items.02-08.accept` changed. Profiles rev 4 to 6 and every other rev 7 item are unchanged.
   - In the task, only acceptance.criteria and exit_condition changed (profile_revision stays 7).
   - The A5.md 02-08 bullet carries a shortened paraphrase of the same rule, which is acceptable.
4. **Finding 4 (Info):** no change was needed and none was made.
5. **Finding 5 (Info): not addressed.** The X-02 event and state still say "사용자 결정 2026-11-14 §5" without the file name. It was optional and is carried as finding 2 below.

## Other checks

- **A5 json:** Sprints sha256 ae5c1e7f… and Ledger sha256 0c462034… match the files on disk. Validation_Plan and validate_package.py hashes still match. The original_sha256 values are unchanged and are confirmed by the validator against main.
- **A5.md:** the 문서 정합 line now mentions the added sources.
- **Spec amendments:** the A5 entry (decision_ref User_Decisions_2026-11-14_p28.md, reading_rule including no skipping) is unchanged.
- **`validate_package.py`** (.venv-poc python): rc 0.
  - amendments A1 to A5, delivered 120, amended 12, ledger_spec_match true, generated_views_match true.
  - sprint_digests are POC-02 rev 7, POC-04 rev 6 and POC-07 rev 6, with the new digests.
  - progress is 11/73 and the follow-up denominator is 12.
- **Progress:**
  - The ledger SHA-256 shown in the generated screens moves to 52d7689e…, which equals sha256(progress/state.json).
  - `render_progress.py --check` passes (ok true, 5 files).
  - `test_render_progress.py` passes (RESULT PASS 0).
  - STATUS shows 11/73 (15.1%) and 0/12.
  - The worktree is clean after the runs.
  - Note: the system python3 (3.6) cannot run the scripts. Use the .venv-poc interpreter, as CI does.
- **No unintended changes:** the top-level spec diff vs 9e81a83 is limited to `sprints` and `inspection_profiles`. events.jsonl, the validation plan and the decision file are untouched by the correction.

## Findings

1. **Info — "rust fmt clippy test" CI was pending at review time.** The correction is docs-only, but confirm that the check is green before merge.
2. **Info (carried from review 1 finding 5) — the X-02 references to "사용자 결정 2026-11-14 §5" still omit the file name** (User_Decisions_2026-11-14_pr30.md). They remain ambiguous among the three 2026-11-14 decision files. This does not block merge and can be fixed in a later progress update to the X-02 `unresolved` field.

No new defects found.
