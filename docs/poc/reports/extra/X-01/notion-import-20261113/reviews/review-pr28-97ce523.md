# PR #28 독립 재검토 (Guardrail) · head 97ce523 · base main 74b00fd

VERDICT: approve

검토 대상: `report/x01-accepted` head `97ce5239bda67d3067871d960e0f2e17adcd957a`(gh PR #28 headRefOid와 일치, baseRefOid `74b00fd`, OPEN). 이번 교정은 `git diff 3843945 97ce523`이다. 바뀐 파일은 state.json 4개 `unresolved`, 생성 화면 4개의 원장 SHA 줄, 보관 검토문 `review-pr28-3843945.md` 추가다.

## 이전 지적 처리

| 이전 지적 | 판정 | 근거 |
|---|---|---|
| E1 P2 · PR #26 D1(02-03 mutants.py의 M27 재정의)에 담당·시점 없음 | **해소** | 02-03 `unresolved`의 D1이 "→ 02-07 수용 전 S3 시험 보강과 함께 M27을 비교만 `AND true`로 바꾸는 좁은 형태(Y1)로 재정의"로 바뀌었다. 담당 작업 02-07의 `unresolved`에도 "PR #26 검토 84a123a D1(02-07 수용 전): …/evidence/mutants.py의 M27을 좁은 형태(G1 비교만 `AND true`)로 재정의하고 다시 실행"이 들어갔다. 좁은 형태는 `review-pr26-84a123a.md:51,95`의 Y1 정의("정의 비교만 제거, AND true")와 같다. 사용자 결정 `User_Decisions_2026-11-12_0202.md:27`("이월 항목은 담당 작업과 처리 시점을 정해 원장에 연결")을 충족한다. |
| E2 P3 · U1에 딸린 보고 §6/§8·api.rs 안내 문구 정정에 시점 없음 | **해소** | X-01 `unresolved`의 U1이 "같은 시점(X-02 착수 전)에 §6 R6·§8 T1 문장과 api.rs 거부 안내를 실제 강제 범위(작업·Notion 출처만 고정)에 맞게 정정(PR #28 검토 E2)"을 포함한다. X-02 `unresolved`도 "X-01 최종 검토 U1(보고·안내 문구 정정 포함)·U4를 착수 전 조건으로 처리"로 바뀌었다. 정정 대상 문장이 실제로 있다는 것도 확인했다. REPORT.md:105 "실행 가능으로 바꿀 수는 없다", :124 "한도·준비 상태를 바인딩", `crates/ieo-coord/src/api.rs:966` "bind only limits/readiness". |

## 이월 항목 담당·시점 재확인

담당 작업의 `unresolved`에 모든 항목이 들어 있다.

- **U1(+E2)**: X-02 착수 전. X-02 `unresolved`에 있다.
- **U2**: 02-05 착수 전. 02-05 `unresolved`의 "X-01 이월(02-05 착수 전) … U2"에 있다.
- **U3**: 02-04 기동 검사. 02-04 `unresolved`의 "X-01 이월(02-04 기동 검사) … U3"에 있다.
- **U4**: X-02 착수 전. X-02 `unresolved`에 있다.
- **U5**: 이 PR에서 교정했다(아래 셀 이동).
- **PR #26 D1**: 02-07 수용 전. 02-07 `unresolved`에 있다.
- **PR #26 D2**: 이 갱신에서 처리했다. 02-03 `unresolved`에 '(02-04에 이어짐)'이 없다.
- **02-03의 기존 이월 항목**: F3·S3는 02-07, F4·N5·N2·S1은 02-05, 수집기 연결은 02-04에 있다. 모두 그대로 연결돼 있다.

## 나머지 재확인

- **완료 수**: `tasks`는 85건(주계획 73 + 후속 12)이다. 주계획은 검수 완료 11, 미착수 62이고, 후속은 12건 모두 미착수다. STATUS.md:13-14는 "11/73", "0/12"다. X-01·X-02는 `extra_tasks`에만 있고 `tasks`에는 없다. X-01은 검수 완료·verified, X-02는 조건 대기다.
- **PR #27**: MERGED다. headRefOid `74bd7965…`(승인 head)이고 merge commit은 `74b00fd`다. `74b00fd`의 부모는 `2c86f3d`와 `74bd796`이다.
- **main CI**: run 43425920936은 `ci`·main·push 실행이다. headSha는 `74b00fd`이고 completed/success다.
- **보관 검토문**:
  - `review-pr27-74bd796.md`, `review-pr28-3843945.md`, `review-pr26-84a123a.md`는 리뷰어 원본과 byte 단위로 같다.
  - `review-pr27-38b671b.md`의 main 대비 변경은 R7 행의 `[보관 시 생략 …]` 각주를 표 셀 안(마지막 `|` 앞)으로 옮긴 1줄뿐이다. 문서화된 U5 교정이다.
  - 큐 상태 토큰 가림은 PR #27에서 이미 있던 것이다.
- **과대 주장**: 찾지 못했다.
  - events.jsonl 새 줄은 "조건부 병합", "73·12 미산입", "모델 호출 0"이다. X-01 `result_state`는 "DB 등록은 개발 완료가 아님"이라고 쓴다. 둘 다 사실 범위 안이다.
  - 서버 직접 Notion 조회는 미시험으로 남아 있다.
- **비밀·큐 원문**: `git diff 74b00fd 97ce523`의 추가 줄을 검사했다. `sk-ant|ghp_|github_pat|secret_|ntn_|token[=:]|KEEP_GATED|NOT ACTIVE|NEXT_NODEKIT|password|BEGIN .*KEY` 패턴으로 찾은 결과, 실제 값은 0건이다. 걸린 것은 보관 검토문의 `password=$(cat <scratchpad>/pgpw)` 자리표시와 검사 명령 서술, 그리고 02-04 `unresolved`의 기존 "--secret-file" 문구뿐이다.
- **스냅샷**: `v1.3.3-EXEC-0015`는 그대로이고 원장 SHA만 갱신됐다. 생성 화면은 state.json과 일치한다(`--check` ok).

## 새 발견 사항

없음(P0~P3 0건).

## 실행한 명령

```
git fetch origin; git rev-parse origin/report/x01-accepted origin/main     # 97ce523…, 74b00fd…
git log --oneline 74b00fd..97ce523; git diff --stat 3843945 97ce523; git diff 3843945 97ce523 -- docs/poc/progress/
git worktree add --detach <scratchpad>/wt28 97ce523      # 검토 후 git worktree remove·prune
.venv-poc/bin/python scripts/render_progress.py --check   # {"ok": true, "mode": "check", …} exit 0
.venv-poc/bin/python scripts/validate_package.py          # exit 0, schema_errors 0, ledger_spec_match true, generated_views_match true, links 373
.venv-poc/bin/python scripts/test_render_progress.py      # RESULT PASS 0, exit 0
python: state.json tasks/extra_tasks 상태 집계, 02-04/02-05/02-07/X-01/X-02 unresolved 확인
grep -n 'Y1|D1|M27' reviews/review-pr26-84a123a.md
grep REPORT.md(§6 R6·§8 T1 문장), grep -rn 'bind only limits' crates
diff <(git show 97ce523:<보관 검토문>) <scratchpad 원본>   # 3건 동일, 38b671b는 U5 셀 이동만
gh pr view 27 --json state,headRefOid,mergeCommit,mergedAt
gh run view 43425920936 --json headSha,headBranch,event,status,conclusion,name
gh pr view 28 --json headRefOid,baseRefOid,state
git diff 74b00fd 97ce523 | grep '^+' | grep -iE '<비밀·큐 토큰 패턴>'
```
