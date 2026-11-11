# 01-02 · 런타임 후보 비교와 선택안

2026-11-12 KST · 사용자 선택 대기. 모델 품질/가격 비교가 아니라 로컬 런타임 계약 적합성의 문서 조사다.
아래 “문서 지원”은 현재 원격 개발 서버의 설치 버전·계정·무인 권한·실제 성공을 뜻하지 않는다. 전부 01-04에서 재현한다.

| 필수 항목 | Claude Code CLI | Codex CLI | 공식 근거 / 남은 probe |
| --- | --- | --- | --- |
| 무인 기동 | 문서 지원: print 모드, 구조화 출력 | 문서 지원: exec, JSON 사건 | C1/O1. 두 역할 최소 권한·무인 허용/거부를 실제 확인 |
| 이름으로 재개 | 문서 지원: name·resume | 문서 지원: resume 이름/ID. 이름 생성/관리의 설치 버전 연결은 probe 필요 | C1/C2/O2. repo/역할↔고정 이름↔실제 세션 ID는 코디네이터 DB에 기록 |
| 기록 위치 지정 | 문서 지원: 설정 디렉터리 환경 변수 | 문서 지원: 상태 루트 환경 변수 | C3/O3. 역할별 전용 PV·재생성·권한 차단 실제 확인 |
| 자식 에이전트 | 문서 지원: 사용자 정의 subagents | 문서 지원: subagents | C1/O4. 부모 복원·역할/자격·자식 사용량 합산은 P35 probe 필요 |
| 사용량 조회 | 문서 지원: token/cost·API 이벤트 | 문서 지원: JSON turn 사용량 | C4/O1. 실패/중단/캐시/자식 누락·중복 실제 대조; 공급자 남은 quota 조회와 구분 |
| 한도 신호 | API 오류의 상태/재시도 계측은 문서에 있음. 구독 한도·해소 시각은 probe 필요 | 실제 계정의 구조화 한도 오류/해소 조회는 이번 근거로 미확정: probe 필요 | C4. 정상 실행의 종료를 quota 해소로 세지 않음 |
| 같은 계정 동시 두 세션 | probe 필요 | probe 필요 | 두 독립 세션의 중첩 실행·ID·한도를 원본 로그로 확인 |
| 자격 분리 / 공급자 quota 분리 | 둘을 분리해 probe 필요 | 둘을 분리해 probe 필요 | 별도 Pod·상태 루트·로컬 예산이 공급자 hard quota 분리의 증거가 아님 |

**권고: Claude Code CLI 하나를 우선 후보로 선택한다.** 두 역할은 같은 런타임을 각각 격리해 사용하되 세션·자격·볼륨은 공유하지 않는다. 명시적인 이름 지정/재개와 구조화 출력·자식·사용량/오류 계측을 한 후보에서 검증하기 쉽다는 문서상의 적합성 판단이다. 성능 우위나 실제 운영 적합성을 입증한 결과는 아니다.
Codex CLI도 대안이며 모델 또는 공급자 계정을 임의로 고정하지 않는다. 설치 버전의 이름 관리·권한·계측 차이와 사용 가능한 인증을 실제 확인해야 한다.

사용자 결정은 **우선 probe할 런타임을 Claude Code CLI로 할지, Codex CLI로 할지**다. 인증 방식/공급자 한도 분리 가능성이 선택을 제한하면 영향·정상 대안을 제시한다. 응답 전 기본 조치는 후보 유지·실행 대기이며 대체 계정/자격으로 우회하지 않는다. 확정된 응답 기한은 없다.
선택 뒤 01-03의 한도·측정 기준을 동결하고 01-04에서 같은 계정 동시 두 세션→개발/Guardrail 자격 및 한도 분리부터 시험한다. 하나라도 불가 또는 관측 불가이면 다음 작업을 진행하지 않는다.

## 공식 자료와 조회 범위

| 키 | 조회한 원문 | 확인한 범위 |
| --- | --- | --- |
| C1 | [Claude CLI reference](https://code.claude.com/docs/en/cli-reference) | print·name/resume·subagents·출력 옵션 |
| C2 | [Claude sessions](https://code.claude.com/docs/en/sessions) | 이름/ID 재개·세션 기록과 실행 중 세션의 예외 |
| C3 | [Claude environment variables](https://code.claude.com/docs/en/env-vars) | 구성/세션 경로 지정 |
| C4 | [Claude monitoring](https://code.claude.com/docs/en/monitoring-usage) | 토큰/비용·API error 이벤트. 공급자 quota 분리 보장으로 읽지 않음 |
| O1 | [Codex non-interactive](https://learn.chatgpt.com/docs/non-interactive-mode) | exec·JSON 사건·사용량·재개 |
| O2 | [Codex CLI reference](https://learn.chatgpt.com/docs/cli/reference) | 이름/ID resume·작업공간/출력·권한 옵션 |
| O3 | [Codex environment variables](https://learn.chatgpt.com/docs/config-file/environment-variables) | 상태 루트·세션/기록 위치 |
| O4 | [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents) | 자식 정의/설정 |

현재 Work 로컬 환경의 Codex --version/--help만 evidence에 저장했다. 원격 개발 서버의 설치/기능 probe가 아니다. Claude 로컬 실행 파일은 이 환경에 없으며 원격 설치 유무를 추정하지 않는다. 실제 모델 호출·세션 생성·로그인·설정 변경·운영 쓰기는 수행하지 않았다.
01-02의 런타임/인증/재개 계약은 선택과 실제 지원 확인 전 미완료다.
