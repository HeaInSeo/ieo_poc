# PR #8 독립 검토 · report/01-03-accepted · c86688b

- 대상: `c86688b7619d78cea3cb92f8568fae6f7b5a3134` (base main `4bd4eaf`, merge-base 일치, 커밋 1개)
- 변경: state.json, 생성 화면 5개(STATUS/TASKS/RESULTS/RESUME/events.jsonl), PR #7 검토 기록 2개
- 검토자는 저장소를 수정·push·댓글하지 않았다.

VERDICT: reject

거부 사유는 P2 1건(F1)이다. 고칠 범위는 작다. 시각 두 필드를 고치고 다시 렌더링하면 된다. 나머지 사실 확인은 모두 통과했다.

## 확인 결과

### 1. state.json 사실 대조

| 항목 | 원장 | 실제 | 판정 |
| --- | --- | --- | --- |
| PR #7 검토 | 5c3af86 reject → 814a5fb approve, PR 댓글 | issue comment 6815281502(05:11:52Z, reject 5c3af86), 6815376530(05:18:06Z, approve 814a5fb) | 일치 |
| 병합 SHA | 4bd4eaf | `gh pr view 7` mergeCommit 4bd4eaf00cca…, mergedAt 05:18:11Z, PR commits 5c3af86·814a5fb | 일치 |
| main CI | success, job 3개 | run 43160480600, headSha 4bd4eaf, success, job 3개 모두 success. 단, rust job completedAt 05:19:04Z(→ F1) | 결과는 일치, 시각은 불일치 |
| 승인 head CI | (조건부 병합) | 814a5fb의 pull_request run 43159743414 success(05:12:46Z), 병합 전에 끝남 | 일치 |
| 01-03 | 검수 완료 / PASS / verified, evidence 6개 | evidence의 저장소 경로 5개 모두 c86688b에 있음. 링크는 PR #7 | 일치 |
| 01-04 | 조건 대기 / 미실행 / unverified, latest_report null | 런타임 결과 없음. evidence는 호스트 점검 1개(“모델 호출 0회”, id·sudo·podman·subuid만 있음) | 과장 없음 |
| 진행률 | 주계획 5/73, 후속 0/12 | main 73건: 검수 완료 5(01-01·02·03·07·08), 미착수 67, 조건 대기 1. followup 12건 모두 미착수 | 일치, 따로 집계 |
| D1·D2 수치 | A, 30회·7,200초·300초 | User_Decisions D1(A), D2(30회·2시간·300초), probe_gate_table A1-GATE-01-04 | 일치. “장기 토큰”은 01-02 계약의 A 정의(`setup-token`)와 같음 |
| 동결 대장 | stage_0 F01~F08, later L01~L09 | freeze_register.json stage_0 8건, later 9건 | 일치 |
| 01-03 검토 P3 N1~N9 | unresolved에 기록 | review-pr7-814a5fb.md의 P3 목록 | 일치 |

### 2. 과장 여부

- 01-03: result_state, STATUS·TASKS 행, notes에 “후속 실험값 전체 동결 아님”, “01-03 검수 완료는 모든 실험값 동결이 아니다”가 명시돼 있다. freeze_register.json rules[0]과도 맞는다. 모든 값이 동결됐다고 읽히는 문구는 없다.
- 작업 수: notes는 “73/31·12/5 불변”이라고만 적는다. 작업이 추가되지 않았다.
- 01-04: 미실행 / unverified, latest_report null, running에 “01-04 모델 호출 0회”가 있다. unresolved는 “동작 미확인”이라고 적는다. 런타임 결과 주장은 없다.
- D11: “현재 파일 목록에서 삭제·원문 링크 대체(공개본 SHA-256 동일). 커밋 d92b414~PR #7 이전 이력에는 남음(이력 재작성 안 함)”.
  - `git log --diff-filter=AD` 결과는 추가 d92b414, 삭제 5c3af86(PR #7의 첫 커밋)이다. head 트리에는 해당 PDF가 없다.
  - d11-link-check.txt에서 공개 URL의 sha256 ce786e99…가 SHA256SUMS 값과 같다.
  - 이 문구는 정확하다.

### 3. 생성 화면 일치(깨끗한 worktree)

- `render_progress.py --check`: `{"ok": true, "mode": "check", files 5개}`, rc=0
- `validate_package.py`: rc=0, schema_errors 0, package_integrity delivered 120 / amended 11 / amendment v1.3.3-A1
- state.json sha256 d2319181…ad9844는 화면 머리글의 원장 SHA와 같다.
- PR #8 체크 3개 pass
- worktree는 제거했다.

### 4. 보관 검토 파일과 게시 댓글

본문은 바이트 단위로 같다. 게시 댓글에만 다음 내용이 더 있다.

- 머리말 2줄: `**독립 검토(Guardrail 세션) · <sha> · VERDICT: …**`와 빈 줄
- 5c3af86 댓글: 끝에 “→ 교정 커밋 814a5fb. 이 head는 다시 독립 검토한다.”
- 814a5fb 댓글: 끝에 빈 줄 1개

판정·발견 내용에는 차이가 없다(→ F3).

### 5. 비밀

추가된 줄을 토큰·키 패턴(ghp_, github_pat_, sk-ant, AKIA, BEGIN key, password 등)으로 검색했다. 걸린 것은 검토 본문이 “비밀 패턴 0건”이라고 적은 문장, 그리고 PGP 서명 안의 `AKiA…`를 설명하는 문장뿐이다. 비밀은 없다(→ F4).

## 발견

### F1 (P2) 관측 시각이 관측한 사실보다 앞선다

- `snapshot_created_at`, `last_actual_work_observed_at`, 이벤트 `STAGED-FREEZE-01-03-20261112.observed_at`이 모두 `2026-11-12T14:18:58+09:00`(05:18:58Z)이다.
- 그런데 같은 이벤트와 resume.last_verified_stage는 “main CI success(3 job)”를 주장한다.
- 실제 run 43160480600의 `rust fmt clippy test` job은 05:19:04Z에 끝났고, run 결론은 05:19:05Z에 났다.
- 따라서 기록된 관측 시각에는 CI가 아직 성공하지 않았다. 결과 자체는 지금 사실이다.
- 원장 규칙은 “새 관측을 추정하지 않는다”이다. 이 기록은 관측 전에 결과를 적은 것으로 읽힌다.
- 고치는 방법: 세 시각(그리고 01-03·01-04 `last_observed_at`)을 CI 확인 이후 시각(05:19:05Z 이후, 예: 실제 확인 시각)으로 바꾼다. 그다음 render_progress.py로 다시 생성한다.

### F2 (P3) resume.branch_sha의 로컬 교정 브랜치 설명이 이미 낡았다

- 원장 문구: “로컬 fix/02-03-prereq-stale-revision(… 미커밋 교정, #7 위로 rebase 필요)”
- 실제 상태: 3fbe970이 14:19:47+09:00에 커밋됐다. 이 PR 커밋보다 31초 뒤다. 이미 4bd4eaf 위에 있다(`merge-base --is-ancestor` 확인).
- 로컬 상태이고 다음 원장 갱신 때 바로잡히므로 차단 사유는 아니다. F1을 고칠 때 함께 갱신할 것을 권고한다.

### F3 (P3) 보관 파일이 댓글의 머리말·끝 메모를 담지 않는다

- 판정·본문은 같다. 5c3af86 댓글 끝의 “교정 커밋 814a5fb” 메모는 보관본에 없다.
- PR #5 보관 관례와 같은 수준이라 차단 사유는 아니다.
- 필요하면 보관 파일 머리에 댓글 ID(6815281502, 6815376530)를 적어 대조할 수 있게 할 것을 권고한다.

### F4 (P3) 보관 검토본에 개인 이메일 주소가 나온다

- review-pr7-5c3af86.md에 개인 이메일 주소가 1회 인용돼 있다. p26-raw 커밋 JSON에 들어 있다는 사실을 지적하는 맥락이다.
- 이 주소는 이미 main의 p26-raw와 공개 PR 댓글에 있다. 새로 노출되는 정보는 아니고 비밀도 아니다.
- 01-03 P3 N7(축약 권고)을 처리할 때 함께 다룰 수 있다.

## 실행한 명령

```
git fetch -q origin; git rev-parse origin/report/01-03-accepted
git log --oneline 4bd4eaf..c86688b; git diff --stat 4bd4eaf c86688b; git merge-base 4bd4eaf c86688b
git diff 4bd4eaf c86688b -- docs/poc/progress/*
gh pr view 7 --json state,mergeCommit,headRefOid,mergedAt,reviews,commits
gh pr view 8 --json state,headRefOid,baseRefName,title,files; gh pr checks 8
gh run list --branch main -L 5
gh run view 43160480600 --json headSha,conclusion,createdAt,updatedAt,jobs
gh run list --commit 814a5fb2a6661ba38b013331edde733fb77d25c2
gh api repos/HeaInSeo/ieo_poc/issues/7/comments --paginate
gh api repos/HeaInSeo/ieo_poc/issues/comments/6815281502 --jq .body   (6815376530 동일)
git show c86688b:<reviews/*.md>; diff (댓글 머리말 2줄 제거 후 비교)
git cat-file -e c86688b:docs/poc/<evidence 경로들>
git log --all --diff-filter=AD -- '*FoundationDB_SIGMOD21.pdf'; git ls-tree -r c86688b | grep -i foundationdb
git show c86688b:.../evidence/{host-isolation-check.txt,d11-link-check.txt}
git show c86688b:.../{freeze_register.json,probe_gate_table.json}
python3: state.json의 track별 status 집계, sha256sum state.json
git worktree add --detach <scratch>/wt-pr8 c86688b
  .venv-poc/bin/python docs/poc/scripts/render_progress.py --check
  .venv-poc/bin/python docs/poc/scripts/validate_package.py
git worktree remove <scratch>/wt-pr8
git log -1 --format='%h %cI' 3fbe970 / c86688b; git merge-base --is-ancestor 4bd4eaf 3fbe970
git diff 4bd4eaf c86688b | grep '^+' | grep -iE '<비밀 패턴>'
```
