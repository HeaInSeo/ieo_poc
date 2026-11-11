# 01-02 · 에이전트 런타임·역할·인증·재개 계약 · runtime-contract-20261112

| 필드 | 기록 |
| --- | --- |
| 계획·작업·회차 | v1.3.3 · 01-02 · run `runtime-contract-20261112` rev 1 · 이전 회차 [runtime-comparison-20261112](../runtime-comparison-20261112/REPORT.md)(후보 비교·사용자 선택 대기) |
| 담당·단계 | 개발 에이전트(설계) → 독립 검토 세션 · 2026-11-12 KST |
| 수용 기준 | `poc-acceptance-v1.3.3` rev 4 `01-02.accept` |
| 사용자 선택 | **Claude Code CLI 우선 검증** — 2026-11-12 대화 원문 “01-02는 기존 권고대로 Claude Code CLI를 우선 검증 대상으로 선택할게. 설치 버전에서 실제 지원을 확인하고, 역할·인증·세션·재개 계약을 구체화해줘.” |
| 산출물 | [Runtime_Contract_v0.md](Runtime_Contract_v0.md) |

## 수용 기준 대조

| 기준 요소 | 위치 |
| --- | --- |
| 런타임 후보·권고·사용자 선택 | 이전 회차 비교표 + 계약 §1 |
| 역할·U3 두 Pod(각 컨테이너 하나)·역할별 UID·파일시스템·자격·영속 볼륨 | §2 |
| 인증 | §3 (설치 관측: claude.ai 구독 로그인 1개) |
| 이름↔session ID·체크포인트 | §4·§5 |
| 자식 경계 | §6 |
| HTTP/JSON 어댑터·PID 1 supervisor 자기 역할 권한 경계 | §2·§7 |
| 지원 여부는 01-04 probe로 확인 | §4 “01-04 확인” 열·§8 |
| 선택 전 기동/인증 변경 없음 | 선택 전·후 모두 모델 호출·세션 생성·로그인·토큰 발급 0건. 수행은 `--version`, `--help`, `auth --help`, `auth status`(읽기) 뿐 |

## 설치 버전 확인(정적)

| 명령 | 결과 | 원본 |
| --- | --- | --- |
| `claude --version` | `2.1.322 (Claude Code)` | [claude-version.txt](evidence/claude-version.txt) |
| `claude --help` | exit 0, 계약 §4 옵션 모두 존재 | [claude-help.txt](evidence/claude-help.txt) |
| `claude auth --help` | login/logout/status | [claude-auth-help.txt](evidence/claude-auth-help.txt) |
| `claude auth status` | loggedIn, authMethod claude.ai, subscription max/max_5x, configDirectory ~/.claude (메일·org ID 가림) | [claude-auth-status.redacted.txt](evidence/claude-auth-status.redacted.txt) |
| 실행 파일 문자열 검사 | `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, 텔레메트리 변수 존재 | [env-var-strings.txt](evidence/env-var-strings.txt) |

이 결과는 **옵션 존재**까지다. 무인 동작·동시 세션·재개·자격 격리·한도 공유는 01-04에서 원본 로그로 판정한다.

## 판정

설계 계약 산출 완료(claim). 검수 완료 여부는 독립 검토 기록과 원장에서 정한다.

## 01-04에 미리 드러난 위험(판정 아님)

현재 자격은 구독 로그인 하나뿐이다. 두 역할이 같은 계정을 쓰면 공급자 한도는 **공유**될 가능성이 높다. 계획의 01-04 첫 probe가 “한도 분리 가능성”을 요구하므로, 공유가 확인되면 중단 조건에 걸린다. 역할별 자격 방식(계약 §3 A/B/C) 선택은 01-03 결정안에 포함한다.

## 비용

시간·토큰 미계측: 이 대화 세션의 역할별 원시 사용량을 얻는 수단이 없다(0으로 대체하지 않음).
