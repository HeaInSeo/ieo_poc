# PoC 진행과 최근 결과

계획 v1.3.3 · snapshot `v1.3.3-EXEC-0019` · 원장 SHA-256 `b5d7b2cd024a5d48538c61e7e8deee9cb58bf5ff656a34b48aca795443c3d99b`

자동 생성 화면 · 원본은 state.json · 손편집 금지

**현재: 02-03 검수 완료 · X-01 검수 완료 + 후속 교정 C1·C2(PR #30)와 후속 PR #34~#37 병합(73·12 미산입) · 02-04 진행(모델 호출 없는 supervisor 핵심 PR #31·#32 병합, 수용 전) · 다음 작업: 02-04**

주계획과 조건부 후속은 별도 진행률이다. 실험 평가 종료·공수·제품 완성률과 구분한다. ZIP은 전달 시점 스냅샷이다.

| 구간 | 검수 완료 | 단순 작업 수 진행률 |
| --- | --- | --- |
| 주계획 | 11/73 | 15.1% |
| 후속 실험 | 0/12 | 0.0% |

| 구간 | 상태별 수 |
| --- | --- |
| 주계획 | 미착수 61 · 준비됨 0 · 진행 1 · 검수 중 0 · 검수 완료 11 · 조건 대기 0 · 종료 미충족 0 |
| 후속 | 미착수 12 · 준비됨 0 · 진행 0 · 검수 중 0 · 검수 완료 0 · 조건 대기 0 · 종료 미충족 0 |

추가 작업은 사용자 지시로 더한 별도 범위다. 주계획 73·후속 12의 검수 완료 수와 진행률에 넣지 않는다.

| 추가 작업 | 상태 | 선행 | 최근 보고 |
| --- | --- | --- | --- |
| X-01 · Notion 작업 가져오기·초안 등록 | 검수 완료 | 02-03 | [보고](../reports/extra/X-01/notion-import-20261113/REPORT.md) |
| X-02 · Notion 시험 작업의 실제 개발 세션 연결 | 조건 대기 | X-01, 02-04 | 없음 · 미실행 |
| X-03 · gRPC 내부 API 계약(서비스·메시지·버전 호환·인증·상태 대응·재시도·멱등·deadline·취소)과 분리 영향 확인 | 미착수 | 02-03 | 없음 · 미실행 |
| X-04 · gRPC 경로 구현(작업 표 분리·tonic 어댑터·metadata 인증)과 HTTP·gRPC 공통 회귀 | 미착수 | X-03 | 없음 · 미실행 |
| X-05 · 기본 kind(Cilium 없음, 멀티 노드) gRPC 서비스 발견·분산·장애 복구 시험(시험용 서버·클라이언트) | 미착수 | X-03 | 없음 · 미실행 |
| X-06 · 제품 경로 kind 재검증: 실제 코디네이터 gRPC 경로의 판정·DB 상태·중복 처리 | 미착수 | X-04, X-05 | 없음 · 미실행 |
| X-07 · Cilium L7 분산 경로 추가 시험(별도 kind 구성) | 미착수 | X-05 | 없음 · 미실행 |

| 구간 | 평가 종료 |
| --- | --- |
| 주계획 실험 | 0/31 |
| 후속 후보 | 0/5 |

## 지금 확인할 내용

| 항목 | 기록 |
| --- | --- |
| 현재 작업 | 02-04 |
| 다음 작업 | 02-04 |
| 마지막 실제 작업 관측 | 2026-11-14T19:59:13+09:00 |
| 화면 snapshot 작성 | 2026-11-14T19:59:13+09:00 |
| 보고 owner | 개발 에이전트 (Claude Code 대화 세션 · 제품 런타임 승인 아님) |
| 열람 상태 | GITHUB_PUBLIC · 익명 HTTP 200 확인(main 병합 기준 화면) |
| 고정 사용자 링크 | https://github.com/HeaInSeo/ieo_poc/blob/main/docs/poc/progress/STATUS.md |
| 대화 전달 | pending |

## 최근 결과와 대기

| 작업 | 상태·판정 | 최근 보고 |
| --- | --- | --- |
| 02-04 | 진행 / 진행 기록 1(수용 아님): crates/ieo-supervisor — 역할 인자(Edit(//…)만, session_id 재개), 허용 목록 환경, CLAUDE_CONFIG_DIR 지시성 digest·변조 탐지·정리(링크·특수 파일·읽기 오류 닫힌 실패), flock 세션 잠금, §5-1 체크포인트·재연결 결정, 결정 ① 예산 가드(미계측·넘침 중지, 추가 과금 없음 조건 미확인 시 보류). 시험 20건. PR #31 bc55edf approve(검토 3/4·교정 2/3) → 병합 0f446d6, PR #32 aeab3d9 approve(검토 2/4·교정 1/3) → 병합 3426c77, main CI run 43441103331(0f446d6)·43442655596(3426c77) success. 모델 호출 0(02-04 예산 사용 0) | [보고](../reports/tasks/02-04/supervisor-core-20261113/PROGRESS.md) |
| 02-03 | 검수 완료 / 수용 기준 rev 4(02-03.accept). crates/ieo-coord: PostgreSQL 17 스키마(이력 표 추가 전용 트리거)·Bearer 인증(토큰 digest만)·요청당 트랜잭션(비2xx 롤백)·(repo, role) 소유권 머리 CAS·권한 세대·스프린트별 커밋 순서 사건·커서·증거 입력(GitHub·02-01 실행 기록)·판정+outbox+판정 행 한 트랜잭션·verified/claim 분리 조회. 등록(형식)과 독립 의미 검토 분리, 판정 입력은 GitHub/실행 증거. 기동 확인(/v0/health)은 liveness만이며 P04 아님. 선행: L07 판정 정책 contracts/judgment-policy-v0.json 0.4.0(digest 7b54ae55…, 숫자 값 없음)으로 고정, L08 재사용, 01-08 N2·PR #10 N1~N5 반영. 시험 142건(실DB·실HTTP 28)·변이 32개 중 31 검출(M13 동등). 실제 GitHub·에이전트 연결 없음(고정 입력). PR #25 검토 1e98f17 reject(P1 F1·F2) → 0fa839d reject(P2 N1) → 4bc8081 reject(P2 R1) → 66958e2 approve(02-03 검토 4/4·교정 3/3) → 병합 de1dd4d · main CI success(run 43395168058). 측정 경과: 측정 시작 07:25:10Z~마지막 교정 efad0ee 약 3,800초, 병합 de1dd4d까지 약 4,565초(시작 전 읽기 몇 분 제외, 상한 14,400초 이내). 모델 호출 0 | [보고](../reports/tasks/02-03/coordinator-20261113/REPORT.md) |
| 02-02 | 검수 완료 / 수용 기준 rev 6(개정 A2). 후보 ieo-evidence @1785cb7을 독립 담당(같은 모델 하위 에이전트, 개발 맥락 없음) 기대값과 대조: public 5/5·authoring 14/14·validation 33/34(V-22는 사례 작성 오류로 독립 담당 분류·검토 재채점 확인), 미확정 사례 1·필드 74 제외. 첫 평가 전 manifest 동결 61a27aa. 누출 검사(현재 범위) 통과. 모의 시계·02-01 이월 교정. **개발 계정의 정답 접근 차단은 미검증**(강한 봉인 04-07 전으로 이월) — 대조 통과는 봉인 검증·개발 전체 정확성 보장이 아님. 실측 약 2,350초(준비 작업 상한 미정). PR #23 f46818a approve(02-02 검토 1/4) → 병합 dca7911 · main CI success(run 43234135722) | [보고](../reports/tasks/02-02/eval-20261112/REPORT.md) |
| 02-01 | 검수 완료 / ieo-evidence(실행·원본 로그 가림 뒤 저장·SHA-256·판정·사용량·합산)와 기록 형식 evidence-record-v0. 종료 코드 0만으로 성공 처리 안 함, 미계측은 unmeasured(합은 null). 01-04 실제 stream 35개 재생 독립 대조 35/35(32 계측·P2c·N4b·L2 미계측) — 재개 세션 modelUsage 누적 발견 → 회차 몫 규칙(계약 rev 7 §11 항목 7). 시험 32·작업공간 110·생성기 28 PASS, 실제 실행 10건 기대대로. 모델 호출 0. PR #21 검토 2df3c35 reject(P2×4) → 97b7231 교정·273907b approve(02-01 검토 2/4·교정 1/3) → 조건부 병합 a0c59fd · main CI success(run 43222844640). F05 약 5,100초/14,400초 | [보고](../reports/tasks/02-01/collector-20261112/REPORT.md) |
| 01-06 | 검수 완료 / 시험실 보관(sha256 a26b7290…, 자격 제외)→정리→초기화→대조, 재정리→재초기화→대조 모두 기대 일치(E 16·E1b 7, check-init 통과). 생성기 render_progress.py 일괄 교체·남은 임시 파일 탐지 보강, 시험 26건 PASS(CI 실행). STATUS 고정 경로·GitHub 익명 200, 사용자 실제 열람 미확인. PR #19 검토 b301843 approve·자동 검토 P1/P2 교정 → f873fa4 approve(01-06 검토 2/4) → 조건부 병합 c0f93d7 · main CI success | [보고](../reports/tasks/01-06/lab-20261112/REPORT.md) |
| 01-05 | 검수 완료 / NPG 고정 e5075aff 사본 10개=고정 커밋 blob(sha256), CI 41469168311 success 재조회 일치. 정책·책임 대응표 v0 17행(직접 10·좁음 4·공백 3, 독립 기대값은 L03 연결). 현재 운영 별도 조회: timer 3 enabled·active(report bridge elapsed), NPG 작업 사본 HEAD 0d03bf6은 고정 커밋 비조상. 겹치는 외부 효과 8종 분리. PR #17 검토 efa4a5d reject(P2 report bridge 상태) → 00a7566 approve(01-05 검토 2/4) → 조건부 병합 4bcba6a · main CI success | [보고](../reports/tasks/01-05/baseline-20261112/REPORT.md) |
| 01-04 | 검수 완료 / 첫 세 probe: P1 지원 · P2 격리 성립 확인 · P3 공유 quota 확인(분리 아님). 나머지: E 성립 확인(E1b forge 포함) · U 고정 Write(path) 미지원→사용자 결정으로 Edit(//path) 채택·지원 · G 합격 · N session-id 재개 지원(동시 재개 CLI 미차단) · L 지원 · C 4 동시 · T 세션/자식별 지원 · R 2.1.316→2.1.322 지원 · 자동 기억 비활성 2방법 확인. 모델 요청 9/30 + 27/30(B2). PR #14 검토 c20e7e3 reject → 사용자 결정 → a3d01ce approve(01-04 검토 4/4) → base 갱신 7c365d1(PR #13과 동일 차이 대조·사용자 결정) → 조건부 병합 74a1fa3 · main CI success | [보고](../reports/tasks/01-04/remaining-probes-20261112/REPORT.md) |
| 01-03 | 검수 완료 / PASS · 단계별 동결(v1.3.3-A1): stage_0 F01~F08 동결, later L01~L09 담당·시점·차단 조건 인수. 후속 실험값 전체 동결 아님 | [보고](../reports/tasks/01-03/staged-freeze-20261112/REPORT.md) |

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
- PR #12 자동 검토 P2(codex 4547423210, probe.py P3 2회차가 1회차의 한도·인증 신호를 보지 않고 기동 — Probe_Budget_Rules §4): 유효. 이번 실행은 모든 rate_limit_event가 allowed·오류 없음이라 판정 영향 없음. 하네스 교정(probe.py stop_on_signal, probe2.py stop_signals)은 task/01-04-remaining c4c4113, 01-04 나머지 결과 PR에서 검토. 처분 답글 https://github.com/HeaInSeo/ieo_poc/pull/12#discussion_r4547547000
- PR #14 병합 처리 근거: F04 상한(검토 4/4) 도달 뒤 branch protection 최신 base 요건으로 승인 head a3d01ce 병합 불가 → 사용자 결정(이번 건 한정): main 병합 head 7c365d1의 추가 변경이 PR #13과 동일함을 독립 검토자가 대조(5번째 작업 검토 아님)·필수 CI 통과 뒤 그 SHA 조건 병합. 재발 방지: 작업 브랜치를 보고 PR 병합 뒤 최신 main에서 만들거나 검토 요청 전에 main을 반영한다. base 갱신 head 7c365d1의 필수 CI run 43181686192(success).
- PR #14 자동 검토 P2 2건(auto-memory 보존, 동시 묶음 정지 시 미기록): 유효, 결과 영향 없음, 하네스 교정 bc09fe4, 답글 discussion_r4547713346·r4547713614.
- 하네스 잔여 위험(결과 사건 없이 종료 0, 산출물 빈 출력) — 처분: 교정(probe.py·probe2.py, PR #16 cd19663)·01-04 판정 영향 없음(harness-result-check-20261112). 회귀 시험 test_result_check.py PASS.
- PR #16 검토 P3(이월): f73ac13 P3-C projects/*/memory 빈 디렉터리 digest 처리, P3-D 정리가 subagents 기록 삭제 — 02-04 시험 (c)에서 확인(02-04 unresolved에 연결).
- PR #21(02-01) 처리: a11f107 CI 실패(새 시험 2건의 픽스처 문제 — dash에 없는 exec -a, setsid 경쟁)로 그 head의 검토를 판정 전에 중단(검토 횟수 미산입으로 기록, 시험만 고친 273907b를 검토 2/4로 승인). base 갱신 예외는 쓰지 않음(main 불변 상태에서 병합)
- 런타임 계약 revision 7(§11 항목 7, PR #21): 재개 세션 modelUsage·total_cost_usd는 세션 누적 — 01-04 판정 영향 없음(01-04 예산은 시도 수·시간)
- 개정 v1.3.3-A2(PR #23, 사용자 결정 2026-11-12 02-02): 02-02 수용 기준 rev 6 — 비공개 평가 강한 봉인·개발 계정 정답 접근 차단 실증을 04-07(P07) 착수 전 조건으로 이월(04-07 unresolved 연결). 제품 런타임 역할·자격 격리 불변. 봉인 구축안 이번 단계 보류. 01-03 'sudo는 없고' 정정(heain wheel, (ALL) ALL 비밀번호, kind NOPASSWD). 계획 변경·환경 준비는 완료 수 미산입
- 판정 정책 L07(D12) 고정(PR #25): contracts/judgment-policy-v0.json 0.4.0, 버전·digest 고정 기계 판독 정책 참조 방식(명세 스키마 r4·9개 digest 불변). G1 선행 증거·G2 SHA 귀속·G3 독립 판정·G4 누적 시도(재시도·교정 한 상한으로 셈 — 별도 재시도 상한 값 미결정이라 새 숫자 없이 엄격 해석)·G5 한도 도달·RG2(대상 승인 범위 — 01-08 RG2 결정 답변 범위의 필드화는 다음 contract revision 후보)·G7 제외 범위. 계획 변경 아님, 완료 수 미산입
- 추가 작업 X-01 검수 완료(PR #27, 개정 A3): Notion 작업 가져오기·초안 등록. 73·12 완료 수 미산입. 서버에 Notion 자격 없음(대화 연결 스냅샷으로 시험) — 서버 직접 조회는 통합 토큰 발급 결정 뒤. X-02(실제 개발 세션)는 02-04 수용·모델 예산·큐 선행 처분 대기 → 2026-11-13 사용자 결정으로 예산·자격·큐 선행(PoC 예외)·Sori PR·Notion 경로 처분, X-01 후속 C1·C2 병합(PR #30). X-02 남은 조건은 extra_tasks X-02
- 02-04 진행 기록 1(PR #31·#32): 연결 단계 주의 — 토큰 상한 회차 중 정지, 역할 기동(fork)과 세션 잠금 해제 직렬화, 디버그 로그 끔, 범위 밖 읽기 불가 파일 미개방 시험 없음(reports/tasks/02-04/supervisor-core-20261113/PROGRESS.md §3)
- X-01 후속 C2 P3 처리 완료: aa38041 I1~I3 → PR #34, f0c4e87 J1~J3 → #35, 6352413 K1~K2 → #36, f571d1e L1~L3 → #37, 784d60e M1~M3 → 보고 PR(문장·정확한 SQL·영속 재적용). 남은 관찰: from_snapshot의 page_id와 url ID 일치 미확인(X-02 착수 전 확인)

다음 의존 작업 착수 전·제어를 돌려받는 지점마다 원장을 갱신하고 재생성한다. 응답 없는 동안 새 관측을 추정하지 않는다. 강제 종료 뒤 실제 외부 효과를 대조한다.

- [주계획·후속 작업](TASKS.md) · [모든 실험 회차](RESULTS.md) · [중단/재개](RESUME.md)
- [원장](state.json) · [파생 사건 기록](events.jsonl) · [인계 지시](../DEV_AGENT_HANDOFF.md)
