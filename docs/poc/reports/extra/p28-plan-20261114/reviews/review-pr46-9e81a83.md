# Review 1 · PR #46 · head 9e81a83eeaabae9b2d85bbc175b3f6a474304715

**Verdict: CHANGES REQUESTED** (one Medium finding; everything else checks out)

Head confirmed: `gh pr view 46` headRefOid = 9e81a83eeaabae9b2d85bbc175b3f6a474304715, mergeStateStatus BLOCKED (awaiting review; at review time CI "docs/poc package and progress validation" and "rust toolchain pin resolves" had passed, "rust fmt clippy test" was pending). Base: merge-base = origin/main = 856f779. Single commit, 13 files.

## What was verified

1. **Decision file** (User_Decisions_2026-11-14_p28.md): it matches the decision as briefed, point by point. It covers the goal, normal path vs auxiliary reconciliation, normal tests with reconciliation off or set long, the five separate timestamps plus allowed latency and measurement method fixed before the run, missed-event recovery with no duplicates under duplication, late arrival and restart, remaining CI/review/approval/budget as normal waiting, placement against 02-08/04-11/07-05 with no new experiment and transport independence (no gRPC prerequisite), and no stop and no new model calls, with history, counts and denominators kept. I cannot compare it byte for byte with the user's original message.
2. **A5 md/json**: baselines 73/31/12/5 unchanged. changed_acceptance = profile changed_items = {02-08, 04-11, 07-05}. Sprint revisions are POC-02 7, POC-04 6 and POC-07 6. scope_limit states no new experiment, no stop of 02-04, no model calls and no gRPC prerequisite.
   - 02-08: event-driven normal path, reconciliation as auxiliary, the five timestamps plus execution cause, transport independence. The original constraints ("별도 수명/자격의 감시기", "P28의 실제 시험을 대신하지 않는다") are kept. No authority is added.
   - 04-11: reconciliation off or long, takeover and first action before reconciliation, latency and method fixed beforehand with reasons, missed event recovered by reconciliation, no duplicate assignment or state change under duplication, late arrival and restart, normal waiting, no skipping. Faithful.
   - 07-05: separates event path from reconciliation recovery by cause and the five timestamps in the real flow. The existing "04의 시험 환경 증거와 구분" is kept.
   - 04-12 left unchanged: **justified**. 04-12 is P28-M (watcher stop, normal wait and recovery liveness and alerting), not follow-up takeover. The missed-event and reconciliation recovery case belongs in P28-F/04-11, and A5 says so explicitly.
3. **Spec integrity** (checked programmatically against origin/main):
   - The rev 7 profile has the same key set as rev 6 and differs only in 02-08/04-11/07-05.accept. 04-12 is identical. Rev 4 to 6 profiles are byte-identical to main.
   - The only task changes are 02-08, 04-11 and 07-05: acceptance.profile_revision goes from 4 to 7, and criteria = exit_condition = the profile item. No other task field changed (prerequisites, outputs, status 미착수 unchanged).
   - Sprint changes: only revision (POC-02 6 to 7, POC-04 5 to 6, POC-07 5 to 6) and digest. The canonical digests recompute correctly (c16299c6…, e22bf4d2…, f45c2229…).
   - The amendments entry for v1.3.3-A5 is present, and the earlier entries are unchanged.
   - The ledger rows for 02-08/04-11/07-05 are byte-equal to the profile items.
   - `validate_package.py` (.venv-poc python): rc 0. amendments A1 to A5, delivered 120, amended 12, ledger_spec_match true, progress 11/73, follow-up denominator 12. The A5 original_sha256 and sha256 values for all four files are confirmed by the validator.
4. **Validation plan**: three rows were added, and the existing P28-F/P28-M rows are unchanged (additions only in the diff). The rows match the 04-11 criteria: immediate takeover within the fixed latency with five timestamps and cause "event path", missed event recovered with cause "reconciliation" and 0 duplicates, and normal waiting with no skipping.
5. **gRPC / 02-04**: gRPC appears only as "not a prerequisite". The prerequisites of 02-08/04-11/07-05 are unchanged. STATUS current and next task are 02-04, and next_action continues 02-04 after this plan PR.
6. **Progress**:
   - The state.json note, the event A5-P28-EVENT-HANDOVER-20261114 and resume are accurate. Counts are 11/73 and 0/12.
   - CI note: the run ids match `gh run list --branch main`. 43591229295 is b5f25d2, 43591506594 is 86aed04, 43593112437 is 778ee47 and 43593523474 is 856f779 (PR #45). All four succeeded.
   - The X-02 carry now names User_Decisions_2026-11-14_pr30.md §5. The "넷째" item (separate server-enforced parts from operating-rule parts) matches §5 bullet 4.
7. `render_progress.py --check`: ok. `test_render_progress.py`: RESULT PASS. The worktree is clean after the runs.

## Findings

1. **Medium — the decision is not recorded as a source of the three revised sprints.** Earlier amendments added their decision file to `sources` of every sprint they revised. A1 added User_Decisions_2026-11-12_0103.md to all nine sprints, and A2 added User_Decisions_2026-11-12_0202.md to POC-02. A5 revises POC-02, POC-04 and POC-07, but none of them lists User_Decisions_2026-11-14_p28.md (kind user_decision, revision 2026-11-14, sha256 of the file). As a result each sprint's digest-covered provenance no longer explains why its criteria changed. A consumer of a single sprint spec cannot trace rev 7 criteria to their decision. The validator does not catch this because it only checks digests of the sources that are listed. Fix: add the source entry to the three sprints, recompute the three digests, and update the A5 json sha256 for Console_Coordinator_Sprints_v0.json and the digests mentioned anywhere (resume and amendment text do not cite digests, so the change is limited to the spec and the A5 json).
2. **Low — resume.plan_and_spec miscounts sprints.** The text "명세 8개 rev 5(단 POC-02 rev 7·POC-04 rev 6·POC-07 rev 6)" is wrong, because only 6 of the 9 sprints are now at rev 5. The old wording "8개 rev 5·POC-02 rev 6" was correct for its time. Suggest "명세 6개 rev 5 · POC-02 rev 7 · POC-04 rev 6 · POC-07 rev 6" (state.json, then regenerate RESUME.md).
3. **Info — "no condition skipped for speed" appears only in 04-11, the validation-plan row and the spec reading_rule, not in 02-08's implementation criteria.** 02-08's "필요한 조건을 대조하고" covers it implicitly, and the A5 reading_rule applies globally. Optional: add "남은 조건은 정상 대기로 두고 생략하지 않는다" to 02-08 so that the implementation contract carries it directly. Not blocking.
4. **Info — 07-05 has no pre-fixed allowed latency.** This is consistent with the decision, which ties the fixed latency to normal tests (04-11). 07-05 reports cause and timestamps only. No change needed.
5. **Info — event X-02-PREP-URL-20261114 still says "사용자 결정 2026-11-14 §5"** without naming the file, and three 2026-11-14 decision files now exist. Leaving the appended event untouched and fixing the reference in the X-02 `unresolved` field is acceptable.
