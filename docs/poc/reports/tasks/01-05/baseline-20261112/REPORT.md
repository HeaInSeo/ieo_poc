# 01-05 원본·기준선·운영 충돌 범위 기록

2026-11-12 KST · 계획 v1.3.3-A1 · 01-05 run `baseline-20261112` · 선행 01-04 검수 완료(PR #14) · 수용 프로파일 poc-acceptance-v1.3.3 rev 4 `01-05.accept`

수용 기준(원문): “NPG 고정 원본·기존 CI·설계와 최소 정책/책임 대응표를 버전으로 기록한다. 사용할 조항의 출처·검증 범위·독립 기대값과 공백을 연결한다. 현재 운영은 별도로 확인하고 기존 actor와 겹치는 외부 효과는 분리한다. 고정 원본을 현재 설치/소유권 증거로 쓰지 않는다.”

## 1. 산출물과 수용 기준 대응

| 수용 기준 문구 | 산출물 | 핵심 |
| --- | --- | --- |
| NPG 고정 원본·기존 CI·설계를 버전으로 기록 | [Source_Baseline.md](Source_Baseline.md), 원본 [fixed-source-compare.txt](evidence/fixed-source-compare.txt) | 고정 커밋 `e5075aff`. 패키지 사본 10개를 고정 커밋 blob과 sha256 전체로 대조해 모두 같다. 기존 CI run 41469168311(head `e5075aff`, success)은 패키지 기록과 API 재조회가 같다. 설계 원본 3개의 sha256 |
| 최소 정책/책임 대응표를 버전으로 기록 | [Policy_Responsibility_Map_v0.md](Policy_Responsibility_Map_v0.md) v0 | 계획 §1.1의 15개 책임과 P21의 2개 원본 영역, 합계 17행 |
| 사용할 조항의 출처·검증 범위·독립 기대값과 공백 연결 | 같은 표의 열. 조항 위치 조사 [npg-clauses-e5075aff.md](evidence/npg-clauses-e5075aff.md) | 행마다 NPG `파일:줄`(@e5075aff)과 검증 P를 적었다. 독립 기대값은 L03 담당·시점과 역사적 원본 시험에 연결했다. 직접 근거 11행, 이름보다 좁은 조항 3행(6·9·17), 공백 3행(13 해소 후 재기동, 14 정지 알림, 16 역할 지침) |
| 현재 운영은 별도로 확인 | [Current_Operations_20261112.md](Current_Operations_20261112.md) §1, 원본 [current-operations.txt](evidence/current-operations.txt) | 읽기 전용으로 조회했다. timer 2개 활성(Operator 매시 07분, Guardrail 37분, 오늘 rc=0), report bridge 비활성, NPG 작업 사본, 장기 claude·codex 세션, 원격 제어 서비스, 다른 저장소의 self-hosted runner, podman 자원 |
| 기존 actor와 겹치는 외부 효과 분리 | 같은 문서 §2 | 겹침 6종과 각 처리: quota group 공유(기술적 분리 불가, D1 지원 구성, 누적 한도 안에서만 사용하고 04-15에서 측정), GitHub 계정, Notion, P26 백로그, 호스트 자원, 원격 제어 |
| 고정 원본을 현재 설치/소유권 증거로 쓰지 않음 | Source_Baseline.md 머리말, Current_Operations §1 | 실측이 이 원칙을 뒷받침한다. 이 서버의 NPG 작업 사본 HEAD `0d03bf6`(fix/sori48 브랜치)는 고정 `e5075aff`를 조상으로 갖지 않는다. 고정 문서의 설치·운영 서술(설치 커밋 `01b8a789…` 등)은 과거 스냅샷으로만 다룬다 |

## 2. 하지 않은 것과 한계

- NPG runtime 경로(`~/.local/share|state/notion-projection-guard`)의 현재 설치 상태는 조회하지 않았다. 실제 전환 전 조사는 P27의 범위다.
- 기존 CI를 다시 실행하지 않았다. 역사적 기준선을 그대로 기록했다(P21 규칙).
- 독립 기대값 자체는 정하지 않았다. 각 실험의 첫 실행 전에 독립 담당이 정한다(L03). 01-05는 연결만 한다.
- 조항 위치 조사는 개발 에이전트가 맡긴 하위 조사다. 인용은 고정 커밋 원문이고, 독립 검토가 대조한다.
- tick 스크립트 내용은 구조(작업 디렉터리, 게이트, 로그 위치)만 읽었다. 프롬프트·비공개 설정은 읽지 않았다.

## 3. 다음 작업에 주는 입력

- P21·P28·P05·P07의 독립 기대값 담당(L03)에게 공백·좁은 행(6·9·13·14·16·17)을 넘긴다. 원본 근거가 없는 새 기준임을 표시해야 한다.
- 01-06(시험실 초기화·증거 보존 경로)은 §2의 호스트 자원 분리(`ieo-probe-*` 이름, 기존 컨테이너 비접촉)를 초기화 절차의 전제로 쓴다.
- 04-15(공유 quota 영향)는 현재 운영 §2의 quota 겹침을 측정 대상으로 쓴다.
