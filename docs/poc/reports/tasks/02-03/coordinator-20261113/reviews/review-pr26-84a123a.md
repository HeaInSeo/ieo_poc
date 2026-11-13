# PR #26 독립 재검토 (Guardrail) · head 84a123a · base main de1dd4d

VERDICT: approve

- 검토 대상: `report/02-03-accepted` 84a123a2df2bfdeaa6654c374b4d3c2bb8956e81. base main de1dd4d(merge-base도 de1dd4d).
- 교정 범위: `git diff 750ae3c 84a123a`. 바뀐 파일은 7개다.
  - state.json(02-03·02-04·02-05·02-07·04-04)
  - 생성 화면 4개(머리 digest, 02-03 행)
  - `evidence/mutants.txt` 끝 줄 1줄
  - 이전 검토 보관 `reviews/review-pr26-750ae3c.md`
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다.
  - 생성기 검사는 detached worktree `<scratch>/wt26b`에서 돌렸고, 끝난 뒤 `git worktree remove --force`로 지웠다.
  - 같은 검사를 깨끗한 `ieo_poc-r0203` 작업 트리(84a123a)에서도 돌렸다. 그 뒤 `git status --porcelain`은 빈 목록이었다.
- P0/P1/P2는 없다. 새 지적은 P3 두 건이다.

## 이전 지적 처분

| 지적 | 판정 | 근거 |
|---|---|---|
| C1 P2 S1~S3·F5에 담당과 시점이 없음 | **해소** | 아래 표의 후속 9개가 모두 작업 ID와 시점을 가진다. 담당 작업의 `unresolved`에도 같은 항목이 들어 있다. S2만 "이 보고 PR에서 정정"으로 처리했다(D1 참고). |
| C2 P3 담당 작업 쪽 연결 누락 | **해소** | 02-07에 F3 잔여·S3(필드별)이 있다. 02-05에 F4·N5·S1이 있다. 04-04에는 `unresolved`가 새로 생겼고 F5·S3(외래 키)·registry 잠금 성능이 있다. 02-04의 02-03 이월에는 시점 "02-04 수용 전"이 붙었다. |
| C3 P3 경과 시간의 끝 시점 | **해소** | 문구가 "측정 시작 07:25:10Z~마지막 교정 efad0ee 약 3,800초, 병합 de1dd4d까지 약 4,565초"로 바뀌었다. 계산은 아래와 같다. |

### 후속별 담당·시점 대조(state.json)

| 후속 | 02-03 `unresolved`의 담당·시점 | 담당 작업 `unresolved`의 기록 |
|---|---|---|
| F3 잔여(머리가 넘어간 뒤 앞 작업 판정) | 02-07 착수 시 | 02-07: "착수 시 판정 머리 규칙 결정 … F3 잔여" |
| F4(검토 author 신원) | 02-05 착수 전 규칙 결정(수집기 구현 02-04) | 02-05: "02-03 이월(02-05 착수 전) … (F4)". 02-04: 수집기 연결, 규칙은 02-05 착수 전 |
| N5(이전 SHA changes_requested와 PR 상태) | 위와 같음 | 02-05: "(N5)". 02-04: 같음 |
| N2(sprint_id 선점) | 02-05 착수 시 | 02-05: "sprint_id를 repo별 승인 바인딩 … N2"(기존) |
| S1(outputs 배열 전체 비교) | 02-05 착수 전, 정책 다음 버전 | 02-05: "outputs를 필수 산출물 artifact_id만으로 좁힐지 … (S1)" |
| S2(M27 표기) | 이 보고 PR에서 정정 | `evidence/mutants.txt` 끝 줄 |
| S3 필드별(acceptance·exit_condition·requirement_ids) | 02-07 수용 전 | 02-07: "수용 전 … 필드별 … 시험 보강(S3)" |
| S3 외래 키(judgments) | 04-04 원자성 시험 | 04-04: "judgments (sprint_id, sprint_revision) 외래 키 제거를 검출하는 시험(S3)" |
| F5(등록·배정 경쟁)·registry 잠금 성능 | 04-04 착수 시 | 04-04: "02-03 이월(04-04 착수 시): … (F5) … registry 잠금 직렬화 … 영향 측정" |

- 담당 작업 제목도 내용과 맞다.
  - 02-05는 "문서 버전·배정·실제 인수 연결"이다.
  - 02-07은 "완료 판정·보고 대기·현황 조회"다.
  - 04-04는 "원자성·중복·잠금·repo 권한"이다.

### S2 정정 문장 검증

- 문장: "재지정 M27(`IS NOT NULL OR true OR NULL = …`)은 OR가 AND보다 약하게 묶여 G1 WHERE 전체를 참으로 만든다. 판정 행이 하나라도 있으면 모든 선행 조건이 충족된다."
  - 판단: 정확하다.
  - 근거: `mutants.py`의 M27 치환은 `= (SELECT jsonb_build_object` → `IS NOT NULL OR true OR NULL = (SELECT jsonb_build_object`이다. 이것을 `api.rs:1499-1508`의 WHERE(`j.task_id = $1 AND … AND (…) IS NOT NULL OR true OR …`)에 적용하면 WHERE는 `(…) OR true`가 된다. 그러면 join된 판정 행이 하나만 있어도 EXISTS가 참이다.
- 문장: "이름의 의도대로 비교만 `AND true`로 바꾼 변이도 N3·R1 시험이 검출한다(검토자 재현)."
  - 판단: 맞다.
  - 근거:
    - 최종 검토 `review-pr25-66958e2.md` 87·125행에 "Y1 definition comparison removed (AND true): killed"가 있다. 검출 시험은 `verdicts_judge_the_assigned_revision_…`와 R1 시험이다.
    - `tests/coordinator.rs:1832` 함수 안 1878행에 N3 주석, 1955행에 R1 주석이 있다.
- 문장: "검출 수 31/32는 그대로다."
  - 판단: 맞다. 재지정 M27도 검출(killed)이므로 수치는 바뀌지 않는다.
- 정정 줄은 원 실행 출력(`dirty after run: none`) 뒤에 `# [정정 2026-11-13, 최종 검토 66958e2 S2]`라는 표시를 달고 덧붙였다. 원 출력은 바뀌지 않았다.
  - 이 파일을 digest로 고정한 곳은 없다. result.json은 경로만 가진다.

### 경과 시간 대조

| 구간 | 근거 시각 | 계산 | 원장 |
|---|---|---|---|
| 시작 → efad0ee | 07:25:10Z → 커밋 시각 17:29:46+09:00(08:29:46Z) | 3,876초 | "약 3,800초" |
| 시작 → 병합 de1dd4d | 커밋 시각 17:41:14+09:00(08:41:14Z), GitHub mergedAt 08:41:15Z | 4,564초 / 4,565초 | "약 4,565초" |

- 3,876초를 "약 3,800초"로 적었다. REPORT §1과 result.json `unmeasured_reason`도 "08:29Z경 약 3,800초"라고 해서 보고와 일관된다.
- 반올림하면 3,900에 더 가깝지만, "약"이 붙어 있고 상한 14,400초에 대한 판정에도 영향이 없다. 지적하지 않는다.

## 나머지 재확인

- **11/73, 0/12, 합산 비율 없음**
  - state.json 집계: main 검수 완료 11·미착수 62, followup 미착수 12.
  - `resume.last_verified_stage`는 "11/73 · 0/12"다. 합산 비율 표현은 없다.
- **승인된 head에서 병합**
  - `de1dd4d^2 = 66958e2`이고, PR #25 headRefOid도 66958e2다(MERGED 08:41:15Z).
  - `de1dd4d^1 = c501a44`이고, `git diff --stat 66958e2 de1dd4d`는 비어 있다.
- **main CI run 43395168058**: headSha de1dd4d, event push, branch main, conclusion success, updated 08:42:44Z.
- **화면 재생성**
  - `render_progress.py --check`: ok, EXIT=0.
  - `validate_package.py`: EXIT=0.
  - `test_render_progress.py`: PASS 28줄, `RESULT PASS 0`(실패 0), EXIT=0.
    - 이전 검토의 "PASS (29)"는 RESULT 줄까지 센 수로 보인다. 시험 수는 변하지 않았다.
- **과대 주장 없음**
  - 02-03 문장은 "기동 확인(/v0/health)은 liveness만이며 P04 아님"이다.
  - "실제 GitHub·에이전트 연결 없음(고정 입력)"으로 적혀 있다.
  - P04 판정은 04-03·04-04 범위 밖으로 남아 있다.
  - 봉인이나 GH1~GH5 충족은 주장하지 않는다.
- events.jsonl의 02-03 사건(revision 1)은 교정 전과 같다. 그 문장에는 경과 수치가 없으므로 C3 문구와 어긋나지 않는다.

## 새 지적

### D1 · P3 · S2를 "정정 완료"라고 했지만, 최종 검토가 요구한 M27 재정의는 하지 않았다
- **위치**:
  - state.json 02-03 `unresolved`: "S2 … → 이 보고 PR에서 정정 완료(evidence/mutants.txt 끝 줄)"
  - `evidence/mutants.py`의 M27
- **내용**: 최종 검토 S2의 조치는 "다음 증거 갱신 때 M27을 Y1 형태(정의 비교만 제거)로 고친다"였다. 이번 교정은 서술만 정정했다. `mutants.py`의 M27은 여전히 넓은 형태(`IS NOT NULL OR true OR NULL =`)다.
- **영향**: S2의 영향은 "증거 서술의 정확성"이었다. 정정 줄은 정확하고, 시험 공백도 없다(Y1은 검출된다). 따라서 실질 위험은 없다. 다만 "정정 완료"만 보면 변이 정의까지 고친 것으로 읽을 수 있다.
- **권고**: 다음 원장 갱신 때 "서술 정정 완료, M27 정의 교체는 ieo-coord 변이 증거를 다시 만드는 작업(예: 02-07 S3 시험 보강 때)"처럼 범위를 밝힌다. 이번 PR을 막을 사유는 아니다.

### D2 · P3 · F3의 "(02-04에 이어짐)"이 모호하고, 02-04에서는 그 항목이 빠졌다
- **위치**:
  - state.json 02-03 `unresolved`: "F3 잔여 → 02-07 착수 시 판정 머리 규칙 결정(02-04에 이어짐)"
  - state.json 02-04 `unresolved`의 02-03 이월
- **내용**: 이번 교정에서 02-04 이월 문장의 "판정 머리 규칙(머리가 다른 작업으로 넘어간 뒤의 앞 작업 판정)"이 지워졌다. 그런데 02-03 쪽에는 "02-04에 이어짐"이 남았다. 02-04가 무엇을 이어받는지(예: 머리 이동을 만드는 supervisor 동작) 읽히지 않는다.
- **영향**: 담당(02-07)과 시점(착수 시)은 분명하고, 02-07에도 기록돼 있다. 추적이 끊기지는 않는다.
- **권고**: 괄호를 "02-04 이후 머리 이동이 생기므로 02-07에서 결정"처럼 고치거나 지운다.

## 참고(지적 아님)

- F4·N5는 수집기 구현을 02-04 수용 전에 하고, 신원·PR 상태 규칙은 02-05 착수 전에 정한다. 그래서 02-04의 수집기가 규칙 확정 전에 만들어질 수 있다.
  - 02-04와 02-05 양쪽 `unresolved`에 이 순서가 적혀 있다. 그래서 의도된 순서로 본다.
  - 02-04 구현 때 규칙을 넣을 자리를 남기면 충분하다.

## 실행한 명령

```
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc fetch origin
git rev-parse origin/report/02-03-accepted origin/main   # 84a123a… / de1dd4d…
git diff --stat 750ae3c 84a123a; git diff 750ae3c 84a123a -- docs/poc/progress evidence/mutants.txt
git diff --stat de1dd4d 84a123a; git merge-base 84a123a origin/main   # de1dd4d
cat reviews/review-pr26-750ae3c.md; grep S1..S3/Y1 reviews/review-pr25-66958e2.md
sed -n 25,40p evidence/mutants.py                       # M27 치환 문자열
grep -n -B25 "= (SELECT jsonb_build_object" crates/ieo-coord/src/api.rs   # G1 WHERE 1499-1508
grep -n "fn verdicts_judge\|N3\|R1" crates/ieo-coord/tests/coordinator.rs
grep -rn mutants.txt docs/poc (digest 고정 여부)        # 경로 참조만
grep 3,800 / elapsed REPORT.md result.json
git log -1 --format='%aI %cI' efad0ee de1dd4d 66958e2  # 17:29:46 / 17:41:14 / 17:33:17 +09:00
python3: state.json track별 status 집계                 # main 11/62, followup 0/12
python3: 02-03 result_state·unresolved의 P04·GitHub·에이전트·봉인 문구 추출
grep S1|S2|S3|F5 state.json RESUME.md STATUS.md events.jsonl
gh pr view 25 --json headRefOid,mergeCommit,mergedAt,state   # MERGED 66958e2 → de1dd4d 08:41:15Z
gh pr view 26 --json headRefOid,baseRefName,state            # OPEN 84a123a main
git rev-parse de1dd4d^1 de1dd4d^2; git diff --stat 66958e2 de1dd4d   # c501a44 / 66958e2 / empty
gh run view 43395168058 --json headSha,conclusion,event,headBranch,updatedAt   # de1dd4d success push main
git worktree add --detach <scratch>/wt26b 84a123a
cd docs/poc
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/render_progress.py --check   # ok, EXIT=0
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/validate_package.py         # EXIT=0
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/test_render_progress.py      # PASS×28, RESULT PASS 0, EXIT=0
git status --porcelain                                   # empty
git worktree remove --force <scratch>/wt26b
(ieo_poc-r0203, 84a123a에서 validate·test 재실행 → EXIT=0, porcelain empty)
```
