# PoC 진행과 최근 결과

계획 v1.3.3 · snapshot `v1.3.3-EXEC-0008` · 원장 SHA-256 `b7e6daea979cc616ee79e9499f50f4c2ed5c0ea8624e98bf9500d4b8ed8091eb`

자동 생성 화면 · 원본은 state.json · 손편집 금지

**현재: 01-04 · 첫 세 probe 판정표 통과(PR #12) · 나머지 범위 계획 고정·실행 중 · 다음 작업: 01-04**

주계획과 조건부 후속은 별도 진행률이다. 실험 평가 종료·공수·제품 완성률과 구분한다. ZIP은 전달 시점 스냅샷이다.

| 구간 | 검수 완료 | 단순 작업 수 진행률 |
| --- | --- | --- |
| 주계획 | 5/73 | 6.8% |
| 후속 실험 | 0/12 | 0.0% |

| 구간 | 상태별 수 |
| --- | --- |
| 주계획 | 미착수 67 · 준비됨 0 · 진행 1 · 검수 중 0 · 검수 완료 5 · 조건 대기 0 · 종료 미충족 0 |
| 후속 | 미착수 12 · 준비됨 0 · 진행 0 · 검수 중 0 · 검수 완료 0 · 조건 대기 0 · 종료 미충족 0 |

| 구간 | 평가 종료 |
| --- | --- |
| 주계획 실험 | 0/31 |
| 후속 후보 | 0/5 |

## 지금 확인할 내용

| 항목 | 기록 |
| --- | --- |
| 현재 작업 | 01-04 |
| 다음 작업 | 01-04 |
| 마지막 실제 작업 관측 | 2026-11-12T16:23:54+09:00 |
| 화면 snapshot 작성 | 2026-11-12T16:23:54+09:00 |
| 보고 owner | 개발 에이전트 (Claude Code 대화 세션 · 제품 런타임 승인 아님) |
| 열람 상태 | GITHUB_PUBLIC · 익명 HTTP 200 확인(main 병합 기준 화면) |
| 고정 사용자 링크 | https://github.com/HeaInSeo/ieo_poc/blob/main/docs/poc/progress/STATUS.md |
| 대화 전달 | pending |

## 최근 결과와 대기

| 작업 | 상태·판정 | 최근 보고 |
| --- | --- | --- |
| 01-04 | 진행 / 첫 세 probe(A1-GATE-01-04): P1 지원 · P2 격리 성립 확인 · P3 공유 quota 확인(지원 구성 기록, 한도 분리 아님) → 진행. 모델 요청 9/30 · 벽시계 69초. 독립 검토 0303f3e reject(P2-1 토큰 SHA-256 전체값 공개 증거 기록) → 4ada612 approve → 조건부 병합 64750e5 · main CI success. 01-04 전체는 미완(나머지 범위 남음) | [보고](../reports/tasks/01-04/first-probes-20261112/REPORT.md) |
| 01-03 | 검수 완료 / PASS · 단계별 동결(v1.3.3-A1): stage_0 F01~F08 동결, later L01~L09 담당·시점·차단 조건 인수. 후속 실험값 전체 동결 아님 | [보고](../reports/tasks/01-03/staged-freeze-20261112/REPORT.md) |
| 01-08 | 검수 완료 / PASS · 보고 v0·API 계약·참조 구현(ieo-report) | [보고](../reports/tasks/01-08/report-api-contract-20261112/REPORT.md) |
| 01-07 | 검수 완료 / PASS · 등록 검사 계약·참조 구현(ieo-spec) | [보고](../reports/tasks/01-07/spec-contract-20261112/REPORT.md) |
| 01-02 | 검수 완료 / PASS · 설계 계약(실제 지원은 01-04 probe) | [보고](../reports/tasks/01-02/runtime-contract-20261112/REPORT.md) |
| 01-01 | 검수 완료 / PASS · 설계 범위 | [보고](../reports/tasks/01-01/scope-20261112/REPORT.md) |

현재 실행의 대기/실패 기록 없음. 계획상 선행 작업은 아직 미수행.

| 필수 기능/전환 | 상태 | 증거 | 관측 시각 |
| --- | --- | --- | --- |
| S0 | 미확인 | 없음 | 미확인 |
| S1 | 미확인 | 없음 | 미확인 |
| S2 | 미확인 | 없음 | 미확인 |
| S3 | 미확인 | 없음 | 미확인 |
| P26 | 미확인 | 없음 | 미확인 |
| P27-L | 미확인 | 없음 | 미확인 |

## 미확정과 다음 행동

- 01-04 첫 probe 전: 역할별 자격 2개(방식 A) 사용자 발급 필요. 판정표 A1-GATE-01-04: 한도 공유 확인은 지원 구성으로 진행, 동시 실행 불가·격리 실패/미확인·한도 구조 관측 불가는 중단.
- 실제 개발 repo·모델/인증·역할별 자격/공급자 quota 분리·예산·표본·일정은 아직 미바인딩.
- 01-03 later(동결 대장 L01~L09): 각 실행 전 동결 증거 필요. P36 봉인은 동일 UID 0700으로 완료 주장 불가(L05). P26 NodeKit 후보 부족(L04).
- 01-04 첫 세 probe 및 나머지 런타임 probe, P36/P26/실제 보고 사람 평가는 미실행.
- 역할별 상시 유지/idle 정지는 P34의 실측 뒤 결정.
- 01-02 후속(비차단): S1 개발 push 토큰의 API 병합 가능성→main에 개발이 만들 수 없는 판정 체크 요구(05-03/P07), S2 workflows 권한(P20/P23), S3 Guardrail GitHub 측 재현·설정 재조회 경로, S4 격리 실행기 범위(02-06/03-07)·의존성 캐시, S5 계약 문구 2곳.
- D11 반영: 제3자 PDF는 현재 파일 목록에서 삭제·원문 링크 대체(공개본 SHA-256 동일). 커밋 d92b414~PR #7 이전 이력에는 남음(이력 재작성 안 함).
- GitHub 계정 1개: 독립 검토는 PR 댓글+원장 기록이며 GitHub Approve로 강제되지 않음. chatgpt-codex-connector[bot] 자동 검토는 보조 정보로만 취급.
- 01-07 후속(비차단): N1 옛 revision 실행 등록·승격(L08 교정 완료, PR #10), N2 생성 ID 충돌 시 부수 순환 보고, N3 비현실적 예상값·2^53 초과, N4 ready+미바인딩 400 vs §1.6 422 명시(01-08), N5 소수 증거값 문자열화. §1.4 공백 G1~G8의 contract revision 5 여부는 사용자 결정.
- 01-08 후속(02-03 구현 전 반영): N1(L08 교정 완료, PR #10) STALE_ASSIGNMENT 늦은 결과를 원 SHA·배정에 귀속 보존(§1.9)·멱등 재생을 배정 대조보다 먼저·에이전트 재조회 경로, N2 completion_summary가 첫 공백 뒤 확인 범위를 숨김; P3 N3 분수 9자리·음수 오프셋 시험, N4 순서·동일 시각 과다 알림, N5 Guardrail의 불변 명세 revision 조회, N6 ConversionResult additionalProperties·409 명시, N7 evidence 머리글 SHA.
- 01-03 검토 P3(비차단): N1 검증기의 rev4→5 차이 집합이 개정 색인 의존(rev 4 프로파일 digest 고정 권고), N2 명세 최상위 amendments가 스프린트 digest에 묶이지 않음, N3 배정 경로 차단 주체 미기재, N4 낡은 문구 3곳, N5 동결 대장에 체크포인트 임계값·idle 정책 항목 없음, N6 probe 구간 F06 재개는 7,200초 안에서 실질 3회·'CLI 기동' 표현, N7 p26-raw 커밋 JSON의 이메일·서명 축약 권고, N8 호스트 점검 기록 최소화, N9 P26 근거의 shallow clone 줄 번호.
- PR #9 검토 P3: 쓰기 거부가 assert라 python -O로 우회, validation/ mkdir 없음, write-fixed 증거의 결과 SHA가 최종 파일과 다름(재현은 됨), L04 기록에 sori S6 약함 미기재.
- PR #10 검토 P3(02-03 착수 시 처리): N1 01-08 계약 검사 순서·409 목록에 ASSIGNMENT_SHA_MISMATCH 누락·OpenAPI 'An An' 오타·null 본문 SHA 규칙 미기재, N2 Registry가 digest 재계산 없이 호출자 문자열 신뢰, N3 Replay 영수증 counts_for_current_judgment 재계산 미구현(항상 false), N4 판정 입력은 판정 시점의 현재 배정으로 정의·시험 필요, N5 cargo-test 로그 머리줄 '1 changes' 설명 없음. review_currency는 처분을 모름(F6).
- PR #12 검토 P3(01-04 나머지 계획에서 반영): P3-3 2d가 파일 전체 일치만 탐지, P3-4 2b가 errno를 버림, P3-6 budget.log 명칭·meta 누적값·시도 05 표식·auth status 원문 미보존, P3-9 해시는 argv 대신 RO 파일로 전달 권고. P3-8 토큰 SHA-256 전체값의 공개 이력 잔존과 교체 여부를 사용자에게 보고.

다음 의존 작업 착수 전·제어를 돌려받는 지점마다 원장을 갱신하고 재생성한다. 응답 없는 동안 새 관측을 추정하지 않는다. 강제 종료 뒤 실제 외부 효과를 대조한다.

- [주계획·후속 작업](TASKS.md) · [모든 실험 회차](RESULTS.md) · [중단/재개](RESUME.md)
- [원장](state.json) · [파생 사건 기록](events.jsonl) · [인계 지시](../DEV_AGENT_HANDOFF.md)
