# PR #26 독립 검토 (Guardrail) · head 750ae3c · base main de1dd4d

VERDICT: reject

- 검토 대상: `report/02-03-accepted` 750ae3cb7d9b9e104ede61766f54abb11ad5f915 (커밋 1개). base main de1dd4d.
  - 바뀐 파일 7개: state.json, 생성 화면 5개(STATUS·TASKS·RESULTS·RESUME·events.jsonl), 최종 검토 보관 `reviews/review-pr25-66958e2.md`(신규).
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다.
  - detached worktree `<scratch>/wt26`에서 확인했고, 끝난 뒤 `git worktree remove --force`로 지웠다.

## 요약

- 원장 수치와 근거는 맞다. 11/73, 0/12이고, 합산 비율은 없다. 02-03 한 건만 `검수 완료`로 바뀌었다(주계획 검수 완료 10→11, 미착수 63→62, 후속 12개 모두 미착수).
- PR #25는 승인된 head에서 병합됐다.
  - `de1dd4d^2 == 66958e2`이고, PR headRefOid도 66958e2다.
  - `de1dd4d^1 == c501a44`이고, 이것이 66958e2의 merge-base다. `git diff 66958e2 de1dd4d`는 비어 있다. 병합으로 들어간 다른 변경은 없다.
- main CI run 43395168058을 확인했다. push로 실행됐고, headSha는 de1dd4d, 결론은 success다. 작업 3개가 모두 success이고, 08:42:44Z(17:42:44 KST)에 끝났다. event의 observed_at 17:43:37 KST는 그 뒤다.
- 보관된 최종 검토 파일은 승인 근거 파일과 바이트 단위로 같다(`cmp`, SHA-256 `6a336f32…acbf`).
- 화면은 생성물과 일치한다. `render_progress.py --check`, `validate_package.py`, `test_render_progress.py`(PASS 29)가 모두 EXIT=0이다.
- 과대 주장은 없다. P04는 "기동 확인은 P04 아님"으로 적혀 있다. 실제 GitHub·에이전트 연결은 "없음(고정 입력)"으로 적혀 있다. 봉인이나 GH1~GH5 충족도 주장하지 않는다. L07 고정 항목도 "계획 변경 아님, 완료 수 미산입"으로 적혀 있다.
- **reject 사유는 P2 하나다(C1).** 최종 검토의 후속 S1~S3에 담당 작업이 없다. 사용자 결정(2026-11-12 0202)은 "이월 항목은 담당 작업과 처리 시점을 정해 원장에 연결"하라고 했는데, 이 원장에는 그 연결이 없다.

## 근거 대조

| 주장(02-03 항목·event) | 1차 근거 | 결과 |
|---|---|---|
| 검토 순서: 1e98f17 reject(P1 F1·F2) → dfac60a → 0fa839d reject(P2 N1) → eb7b0af·02c61a0 → 4bc8081 reject(P2 R1) → efad0ee → 66958e2 approve(P3 S1~S3) | `gh pr view 25` commits 10개, reviews/*.md 4개의 VERDICT와 지적 제목 | 일치 |
| 검토 4/4·교정 3/3 | 검토 파일 4개. 교정 묶음 3개(dfac60a / eb7b0af·02c61a0 / efad0ee). REPORT §1의 F04(4회·3회) | 일치 |
| 병합 de1dd4d, main CI success(run 43395168058) | `gh pr view`의 mergeCommit, `gh run view` | 일치 |
| 시험 142건(실DB·실HTTP 28) | cargo-test.log의 test result 합: 28+6+29+1+33+4+16+25 | 일치 |
| 변이 32개 중 31개 검출(M13 동등) | mutants.txt 끝 줄 "31 of 32 killed; M13 … equivalent" | 일치 |
| L07 정책 0.4.0, digest 7b54ae55… | `contracts/judgment-policy-v0.json`의 version, sha256sum 7b54ae55fdee… | 일치 |
| 정책 규칙 G1·G2·G3·G4·G5·RG2·G7 | 정책 파일의 rules 7개. RG2는 객체 승인 범위이고, 01-08 결정 답변 범위는 다음 contract revision 후보로 남음 | 일치 |
| 수용 기준 rev 4(02-03.accept) | REPORT §1, result.json의 profile_revision | 일치 |
| 측정 경과 약 3,800초, 상한 14,400초 이내 | REPORT §1: 07:25:10Z~efad0ee 08:29Z경 ≈ 3,830초. F05 14,400초 | 수치는 맞음. 서술은 C3 참조 |
| 모델 호출 0 | REPORT §1·§6 | 일치 |
| resume.running의 시험 DB 컨테이너 | `podman ps`: ieo-poc-0203-pg Up | 일치 |
| 02-03 evidence 목록 11개 | 경로 모두 존재. review-pr25-66958e2.md는 이 PR이 추가함 | 일치 |

## 지적

### C1 · P2 · 최종 검토 후속 S1~S3에 담당 작업과 처리 시점이 없다
- **위치**: `docs/poc/progress/state.json`의 02-03 `unresolved`.
- **내용**: 원장에 적힌 각 후속의 처리처는 다음과 같다.
  - S1 → "정책 다음 버전": 작업 ID가 없다.
  - S2 → "(표기 문제, S2)": 담당도 시점도 없다.
  - S3 → "다음 계측·정책 수정 때 시험 보강": 작업 ID가 없다.
  - 이 세 항목을 담는 작업 `unresolved`는 state.json 어디에도 없다.
  - 같은 목록의 F5("등록·배정 경쟁 자동 시험 없음")에도 담당이 없다.
- **기준**: 사용자 결정 `docs/poc/User_Decisions_2026-11-12_0202.md` 27행은 "이월 항목은 담당 작업과 처리 시점을 정해 원장에 연결해줘"라고 한다. 02-01·02-02 이월은 이 규칙대로 기록됐다(예: 02-04 unresolved의 "담당 02-04, 시점: …").
- **영향**: 정책 다음 버전이나 시험 보강을 맡는 작업에서 S1~S3를 확인할 근거가 없다. 그래서 후속이 사라질 수 있다.
- **조치**: S1~S3(와 F5)마다 실제 작업 ID와 시점을 정한다. 그 작업의 `unresolved`에도 같은 내용을 넣는다.
  - 담당 예: ieo-coord 판정 경로를 다시 다루는 02-07, 또는 정책을 다음에 개정하는 작업.
  - 시점 예: "02-07 착수 전 정책 0.5.0에서 outputs 정규화 검토", "02-07 증거 갱신 때 M27 Y1 형태 교정·필드별 표 시험·FK 시험 추가".
  - 그 뒤 `render_progress.py`로 화면을 다시 만든다.

### C2 · P3 · 02-03이 지정한 담당 작업 쪽에 연결이 빠진 항목이 있다
- 02-03 unresolved는 담당을 다음과 같이 지정했다. 그런데 대상 작업의 `unresolved`에 그 항목이 없다.
  - F3 잔여(머리가 넘어간 뒤 앞 작업 판정) → "02-04·02-07": 02-07 unresolved에 없다(02-04에는 "판정 머리 규칙"이 있다).
  - F4(author 신원)·N5(이전 SHA changes_requested와 PR 상태) → "02-04·02-05": 02-05 unresolved에 없다(02-04에는 있다).
  - registry 잠금 직렬화 성능 → "04-04": 04-04에는 `unresolved` 자체가 없다.
- 02-04에 넣은 항목도 "02-03 이월"이라고만 적었다. 02-02 선례처럼 처리 시점을 적지 않았다(예: "supervisor 판정 입력 연결 전").
- 담당이 적어도 하나는 연결돼 있어 추적이 끊기지는 않는다. 그래서 P3으로 둔다. C1을 고칠 때 함께 맞추기를 권한다.

### C3 · P3 · "측정 경과 약 3,800초"에 끝 시점이 적혀 있지 않다
- 3,800초는 측정 시작(07:25:10Z)부터 마지막 교정 efad0ee(08:29Z경)까지다. 최종 검토와 병합(08:41:15Z, 약 4,565초)은 빠져 있다.
- 보고(result.json `cost.elapsed_seconds: null`)는 시작 전 읽기가 빠졌다는 이유로 이 값을 공식 경과로 쓰지 않았다. 그런데 원장 문장은 작업 총 경과처럼 읽힌다.
- 두 값 모두 상한 14,400초 안이므로 판정에는 영향이 없다. "측정 시작~efad0ee 약 3,800초(병합까지 약 4,565초, 시작 전 읽기 제외, 공식 elapsed_seconds 미기록)"처럼 범위를 밝히기를 권한다.

## 참고(지적 아님)

- main의 result.json은 `verification.level: claim`, `verdict: 미판정`, `execution_state: 마지막 검토 대기` 그대로다.
  - 검토 전에 쓴 보고라 이전 작업들과 같은 방식이다. 진척의 유일한 근거는 state.json(verified)이므로 문제로 보지 않는다.
- 최종 검토 파일의 제목은 "base main c501a44"다. 승인 당시 base와 같고, 보관 파일은 원본과 동일하다.

## 실행한 명령

```
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc fetch origin
git rev-parse origin/report/02-03-accepted        # 750ae3cb7d9b9e104ede61766f54abb11ad5f915
git worktree add --detach <scratch>/wt26 750ae3c
git rev-parse de1dd4d^2                           # 66958e26e1698d210c102e6c20044fe739e628b4
git rev-parse de1dd4d^1; git merge-base 66958e2 de1dd4d^1   # c501a44 / c501a44
git diff --stat 66958e2 de1dd4d                   # empty
git merge-base HEAD origin/main                   # de1dd4d
git diff --stat de1dd4d HEAD; git diff de1dd4d HEAD -- docs/poc/progress/{state.json,events.jsonl,STATUS.md,RESUME.md}
gh pr view 25 --repo HeaInSeo/ieo_poc --json commits,mergeCommit,mergedAt,headRefOid,state
   # MERGED 08:41:15Z, merge de1dd4d, head 66958e2, commits 10
gh run view 43395168058 --repo HeaInSeo/ieo_poc --json headSha,conclusion,event,headBranch,workflowName,createdAt,updatedAt,jobs
   # push/main/de1dd4d success, jobs 3 success, updated 08:42:44Z
cmp <wt>/…/reviews/review-pr25-66958e2.md <scratch>/review-pr25-66958e2.md   # identical
sha256sum (both)                                   # 6a336f3249cc…acbf
cat result.json; grep REPORT.md (상한·시험·변이·정책·P04·GH·모델)
grep "test result" evidence/cargo-test.log         # 28+6+29+1+33+4+16+25 = 142
tail evidence/mutants.txt                          # 31 of 32 killed, M13 equivalent
sha256sum contracts/judgment-policy-v0.json        # 7b54ae55fdee…
python3: state.json의 track별 status 집계       # main 73 (검수 완료 11, 미착수 62), followup 12 (미착수 12)
python3: state.json 02-04/02-05/02-06/02-07/04-03/04-04 unresolved 확인
grep 담당 docs/poc/*.md                          # User_Decisions_2026-11-12_0202.md:27
podman ps                                          # ieo-poc-0203-pg Up
cd docs/poc
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/render_progress.py --check     # ok, EXIT=0
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/validate_package.py           # EXIT=0
/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python scripts/test_render_progress.py        # RESULT PASS (29), EXIT=0
git status --porcelain                             # empty
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree remove --force <scratch>/wt26
```
