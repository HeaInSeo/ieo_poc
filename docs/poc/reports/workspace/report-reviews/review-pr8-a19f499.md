# PR #8 독립 재검토 · report/01-03-accepted · a19f499

- 대상: `a19f499bb70192e5282281fc91f1d5f3ddc72b61`. base는 main `4bd4eaf`이고, 이전 head는 c86688b다. 그 위에 커밋 1개가 추가됐다. origin 브랜치 head와 일치한다.
- 변경: state.json, 생성 화면 5개, 보관 검토본 2개의 머리글. 8개 파일에서 23줄 추가, 19줄 삭제.
- 검토자는 저장소를 수정·push하지 않았고 댓글도 달지 않았다.

VERDICT: approve

P1과 P2는 없다. 이전 검토의 P2(F1)는 해소됐다. 남은 것은 모두 P3(비차단)다.

## 1. 이전 발견 처리

| ID | 이전 등급 | 처리 | 근거 |
| --- | --- | --- | --- |
| F1 | P2 | 해소 | 다섯 시각을 모두 `2026-11-12T14:22:39+09:00`(05:22:39Z)로 바꿨다. 대상은 snapshot_created_at, last_actual_work_observed_at, 01-03과 01-04의 last_observed_at, STAGED-FREEZE 이벤트의 observed_at이다. main CI run 43160480600은 05:19:05Z(updatedAt)에 끝났고 마지막 job은 05:19:04Z에 끝났다. 새 시각은 그 뒤다. a19f499의 author·committer 시각도 14:22:39+09:00이라 미래 시각이 아니다. 생성 화면도 같은 값으로 다시 만들어졌다. |
| F2 | P3 | 해소 | branch_sha 문구는 "fix/02-03-prereq-stale-revision 3fbe970(4bd4eaf 위, L08 교정, PR 전)"이다. 실제로 3fbe970은 4bd4eaf 위에 커밋 1개가 있는 상태이고, 해당 브랜치의 PR은 없다(`gh pr list --state all`). worktree는 ../ieo_poc-fix다. 문구와 일치한다. |
| F3 | P3 | 해소 | 두 보관본 첫 줄에 HTML 주석으로 댓글 URL을 적었다(#issuecomment-6815281502, 6815376530). 두 ID 모두 PR #7의 issue comment로 존재한다. 6815281502는 05:11:52Z에 게시됐고 머리말은 "5c3af86 · VERDICT: reject"다. 6815376530은 05:18:06Z에 게시됐고 머리말은 "814a5fb · VERDICT: approve"다. SHA와 판정이 보관본과 맞는다. 주석은 머리말 2줄과 끝 메모의 차이도 밝힌다. |
| F4 | P3 | 미처리 | review-pr7-5c3af86.md에 개인 이메일 인용이 그대로 있다. 이미 공개된 정보이고 비밀이 아니다. 01-03 P3 N7과 함께 처리하면 된다. 여전히 비차단이다. |

## 2. 새 diff(c86688b..a19f499) 검토

### 시각
- 위 F1 행 참조. 새 시각은 CI 완료 뒤이고 커밋 시각과 같다. 추정 관측은 없다.
- event_revision은 1을 유지한다. c86688b는 main에 병합되지 않았으므로 게시된 이벤트를 고쳐 쓴 것이 아니다. 문제없다.

### 새 unresolved 항목(`--write`)
- 원장 문구: "PR #7 자동 검토 P2(814a5fb): validate_package.py --write가 전달본 v1_3_3_validation.json을 덮어써 다음 검산 실패(재현). 별도 경로 저장으로 교정 PR 예정."
- `gh api repos/HeaInSeo/ieo_poc/pulls/7/comments`로 확인했다. id 4546491247은 chatgpt-codex-connector[bot]의 P2 badge 댓글이다. 경로는 validate_package.py:101이고 게시 시각은 04:57:42Z다. original_commit_id는 5c3af86, commit_id는 814a5fb다.
- 지적 내용과 원장 요약이 일치한다. 원장은 이 항목을 P2로 정직하게 기록했고, 해결됐다고 주장하지 않는다.
- 재현 주장도 직접 확인했다. a19f499의 깨끗한 worktree에서 `--write`를 실행하면 rc=0이고 `M docs/poc/v1_3_3_validation.json`이 생긴다. 이어서 `validate_package.py`를 실행하면 rc=1이고 `AssertionError: v1_3_3_validation.json`이 난다. 원장의 "(재현)"은 사실이다.
- 이 결함은 main(4bd4eaf)에 있다. 이 PR은 결함을 기록만 하고 도입하지 않으므로 이 PR의 차단 사유가 아니다. `--write`를 쓰지 않는 CI는 통과한다.

### 브랜치 상태 문구
- `git worktree list`: ieo_poc는 a19f499 [report/01-03-accepted], ieo_poc-fix는 3fbe970, ieo_poc-val은 fix/validate-write-path다.
- 원장은 "fix/validate-write-path(4bd4eaf 위, … 커밋 전)"이라고 적는다. 첫 확인 때 이 브랜치는 4bd4eaf에 있었고 커밋 0개, `git status` clean이었다. 원장과 맞았다.
- 검토 도중 224c35e가 커밋됐다. 커밋 시각은 14:23:06+09:00으로 a19f499보다 27초 뒤이고, 4bd4eaf 위에 있다. 원장의 "커밋 전"은 a19f499 시점의 사실이고 지금은 이미 낡았다(→ N1).

### 보관 검토본 머리글
- 댓글 ID 2개가 존재하고, SHA와 판정이 일치한다(위 F3 행).

### 기타
- next_action은 "--write 교정 PR(사용자 후속 지시 1)"을 L08 교정 PR 앞에 넣었다. 순서 변경만 있고 새 결과 주장은 없다.
- 01-03과 01-04의 result_state, 진행률(5/73, 0/12), 작업 수는 바뀌지 않았다.

## 3. 생성 화면·패키지 검산(a19f499 깨끗한 worktree)

- `render_progress.py --check`: `{"ok": true, "mode": "check", files 5개}`, rc=0
- `validate_package.py`: rc=0
  - schema_errors 0, ledger_spec_match true, generated_views_match true
  - progress는 accepted 5/73, followup 12
  - package_integrity는 delivered 120 / amended 11 / gitignored_paper_pdfs_absent 4 / amendment v1.3.3-A1
- state.json sha256은 `988069b9…c94b956`이다. 화면 머리글의 원장 SHA와 같다.
- worktree는 `git worktree remove --force` 후 prune으로 제거했다. `--write` 재현 때문에 수정된 파일이 생겨 --force가 필요했다. 원본 저장소는 건드리지 않았다.
- PR #8 체크: docs validation pass, toolchain pin pass. `rust fmt clippy test`는 검토 시점에 pending이었다(run 43160988510, headSha a19f499). 병합 전에 success를 확인할 것.

## 발견

### N1 (P3) fix/validate-write-path의 "커밋 전" 문구가 27초 뒤 낡았다
- a19f499 시점에는 사실이었다. 224c35e(14:23:06+09:00)가 바로 뒤에 커밋됐다.
- 로컬 상태이고 다음 원장 갱신 때 바로잡히므로 비차단이다.

### N2 (P3) `--write` 항목의 출처를 더 정확히 적을 수 있다
- 원장은 "(814a5fb)"라고만 적는다. 실제 댓글은 5c3af86에 처음 달렸고(original_commit_id), 지금은 814a5fb에 고정돼 있다. 댓글 ID는 4546491247이다.
- 대조할 수 있도록 댓글 ID를 함께 적기를 권고한다.

### N3 (P3) 보관본 머리 주석이 두 파일에 같은 문장으로 들어갔다
- review-pr7-814a5fb.md(approve 댓글)에도 "reject 댓글에는 끝에 교정 커밋 안내가 더 있다"가 들어 있다. 일반 설명으로 읽으면 틀리지 않지만, 이 파일에는 해당하지 않는 문장이라 혼동 여지가 있다.
- approve 댓글의 실제 차이(끝 빈 줄 1개)는 적혀 있지 않다. 판정과 본문에는 영향이 없다.

### F4 (P3, 이월) 보관 검토본의 개인 이메일 인용
- 01-03 P3 N7과 함께 처리한다.

## 실행한 명령

```
cat <scratch>/review-pr8-c86688b.md
git fetch -q origin; git rev-parse origin/report/01-03-accepted
git log --format='%h %ci %s' -5 a19f499; git log -1 --format='author %aI%ncommitter %cI' a19f499
git diff --stat c86688b a19f499; git diff c86688b a19f499
git branch -a -vv; git worktree list
git log -3 --format='%h %cI %s' fix/02-03-prereq-stale-revision / fix/validate-write-path
git merge-base --is-ancestor 4bd4eaf <branch>; git rev-list --count 4bd4eaf..<branch>
git -C ../ieo_poc-val status --short; git -C ../ieo_poc-fix status --short
gh pr list --state all -L 10 --json number,headRefName,state,createdAt
gh api repos/HeaInSeo/ieo_poc/pulls/7/comments --paginate
gh api repos/HeaInSeo/ieo_poc/issues/comments/6815281502   (6815376530 동일)
gh run view 43160480600 --json headSha,conclusion,updatedAt,jobs
git worktree add -q --detach <scratch>/wt-pr8b a19f499
  /opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python docs/poc/scripts/render_progress.py --check
  .../.venv-poc/bin/python docs/poc/scripts/validate_package.py
  sha256sum docs/poc/progress/state.json
  .../.venv-poc/bin/python docs/poc/scripts/validate_package.py --write; git status --short
  .../.venv-poc/bin/python docs/poc/scripts/validate_package.py   (rc=1, AssertionError 재현)
git worktree remove --force <scratch>/wt-pr8b; git worktree prune
git log -1 --format='%h %cI %s' 224c35e; git merge-base --is-ancestor 4bd4eaf 224c35e
gh pr checks 8; gh run view 43160988510 --json headSha,status
```
