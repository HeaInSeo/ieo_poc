<!-- 보관본: PR #17 독립 검토 00a7566 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/17#issuecomment-6819432787 (머리말·끝 교정 안내 생략) -->
# PR #17 독립 재검토 — 01-05 원본·기준선·운영 충돌 범위 기록

- 저장소: HeaInSeo/ieo_poc (public) · 브랜치 task/01-05-baseline · head `00a7566` · 직전 검토 `efa4a5d`(reject)
- 검토자: Guardrail(독립 검토) · 01-05 검토 2/4 · 2026-11-12 KST
- 방법: 읽기 전용. 모델 요청 0회. 쓰기 없음. 토큰·env 값·프롬프트·로그 본문은 읽거나 출력하지 않음.
- 범위: `efa4a5d..00a7566` diff(01-05 산출물 5개 파일만 변경)와 직전 지적 해소 여부. 바뀌지 않은 부분(고정 원본 sha256 10개, CI run, 조항 16행 대조, P3 identity)은 직전 검토 결과를 그대로 유지한다.

VERDICT: approve

## 직전 지적 해소 확인

| 지적 | 확인 |
| --- | --- |
| P2-1 report bridge 상태 | 해소. `evidence/current-operations.txt:7-13`에 세 timer의 `systemctl --user show` 값과 `On*`·`Persistent` 줄이 들어갔다. 독립 재조회 결과와 같다: report bridge `enabled / active / elapsed`, NextElapse 없음, 마지막 트리거 2026-10-13, `OnBootSec=1min`·`OnUnitActiveSec=5min`·`Persistent=true`. Operator·Guardrail은 `enabled / active / waiting`이고 `Persistent=true`다(unit 파일에서 확인). `Current_Operations_20261112.md:13`의 문구는 정확하며 첫 기록의 오류도 인정했다. §2의 새 행 `:37`(재부팅·user manager 조작 없음, 01-06과 각 실험 첫 실행 전에 재조회)도 적절하다. REPORT.md §1·§2와 result.json(`revision` 2, observed, unresolved)이 서로 일관된다. 첫 조회본이 efa4a5d에 남는다는 공개도 정직하다 |
| P3-1 행 15 | 해소. `Policy_Responsibility_Map_v0.md:31`은 “이름보다 좁다”로 바뀌었고 집계 `:39-40`은 10/4/3이다. 인계 대상도 REPORT.md:29와 result.json `unresolved[0]`에 15를 넣었다 |
| P3-2 역방향 겹침 | 해소. §2 `:38`에 새 행이 생겼다. actor 구별의 한계(같은 계정)를 밝혔고, tick의 작업 디렉터리 근거와 P21 집계 방법을 적었다. API로 독립 재집계한 결과는 다음과 같다. ieo_poc의 PR 리뷰 댓글과 issue 댓글은 `chatgpt-codex-connector[bot]` 26건, `HeaInSeo` 41건이다. 41건 중 1건은 스냅샷(09:03:56Z) 뒤인 09:05:02Z에 달렸다. 여기에 PR 작성자 `HeaInSeo` 17건을 더하면 스냅샷 시점의 57건과 맞는다. ieo_poc_test는 0건이다. 숫자는 맞다. 다만 집계 항목의 이름표가 문서마다 다르다(P3-B) |
| P3-3 위생 | 해소. 스냅샷에서 openclaw 포트·버전, 관리 경로, runner 이름, ExecStart를 뺐다. 원격 서비스는 이름만 남았고(`current-operations.txt:23`), codex cwd는 `<home path>`로 가렸다. 00a7566 산출물에서 `18789`·`openai-dc-additional-manager`·`rocky-baremetal`·`ghrunner`를 grep하면 0건이다. 남은 `/home/heain/.local/...` 경로(Current_Operations:20)는 public NPG 원문에 이미 있는 값이다 |
| P3-4 시점 표시·근거 | 해소. 행 17 `:33`에 “고정 문서 작성 시점 … 현재 배치 상태는 조회하지 않음”이라고 시점을 붙였다. tick 로그 주장은 마지막 `TICK_END`와 마지막 40줄의 단어 개수로 낮췄다(본문 미기록). “주간” 주장은 근거 줄(`current-operations.txt:30`)로 뒷받침된다. 원문은 tick 스크립트 두 개의 33번째 줄 주석 “# Policy (user-corrected 2026-09-22): weekly(all models) is the ONLY …”이다 |

## 도구·CI

- `git archive 00a7566`으로 푼 사본에서 실행한 결과: `validate_package.py` rc=0, `render_progress.py --check` ok(`.venv-poc/bin/python -I`, docs/poc에서 실행)
- PR CI @00a7566: 3개 모두 SUCCESS
- worktree는 00a7566이고 작업 트리 변경은 없다

## 남은 지적(비차단)

### P3-A · 대응표 본문의 인계 대상 목록에 P12가 빠졌다

- `Policy_Responsibility_Map_v0.md:43`은 여전히 “P21·P28·P05·P07의 독립 기대값”이다. REPORT.md:29는 행 15 재분류에 맞춰 `P12`를 추가했다. 다음 수정 때 맞추면 된다.

### P3-B · actor 집계의 이름표가 실제 집계 범위와 다르다

- `evidence/current-operations.txt:43`의 머리말은 “PR review comments and issue comments”다. `Current_Operations_20261112.md:38`은 “PR 댓글·검토·PR 작성자”다.
- 재집계해 보면 실제 숫자는 “리뷰 댓글 + issue 댓글 + PR 작성자”다. PR review 객체(bot 6건, HeaInSeo 4건)는 들어 있지 않다.
- 결론에는 영향이 없다. 출처를 추적할 수 있도록 이름표를 실제 범위에 맞추길 권한다.

### 참고 · 공개 이력

- 지운 호스트 세부(포트·관리 경로·runner 이름)는 커밋 efa4a5d의 evidence에 남아 있고, 그 커밋은 이미 public 브랜치에 올라 있다.
- 비밀은 아니므로 차단 사유가 아니다. squash 병합이 아니면 main 이력에도 남는다는 점만 알고 진행하면 된다.

## 판정

01-05의 수용 문장 네 개를 모두 충족한다.

- 고정 원본·CI·설계와 17행 대응표를 버전으로 기록했다.
- 조항 출처·검증 범위·L03 연결과 공백(좁음 4·공백 3)을 정직하게 연결했다.
- 현재 운영을 고정 원본과 따로 실측했다.
- 겹치는 외부 효과 8종을 분리했다. quota 공유, report bridge 재기동, 기존 actor의 PoC 저장소 쓰기가 포함된다.
- 고정 원본을 현재 설치·소유권 증거로 쓰지 않았다.

P3-A·P3-B는 병합 뒤 후속 수정이나 다음 작업에서 반영해도 된다.
