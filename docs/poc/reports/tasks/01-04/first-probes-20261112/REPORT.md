# 01-04 첫 세 probe 결과 (P1·P2·P3, 판정표 A1-GATE-01-04)

2026-11-12 KST · 계획 v1.3.3-A1 · run `first-probes-20261112` · 계획 [PROBE_PLAN.md](PROBE_PLAN.md)(실행 전 고정 a321cd2, 측정 보강 93a44ad, 실행 후 §5) · 판정표 [probe_gate_table.json](../../01-03/staged-freeze-20261112/probe_gate_table.json) · 예산 [Probe_Budget_Rules.md](../../01-03/staged-freeze-20261112/Probe_Budget_Rules.md)

**01-04 전체의 결과가 아니다.** 판정표가 다른 probe보다 먼저 하라고 정한 세 probe의 결과다. 01-04의 나머지 범위(§6)는 아직 하지 않았으므로 01-04는 검수 완료가 아니고 작업 수도 늘지 않는다.

## 1. 판정 요약

| 순서 | probe | 판정 | 판정표 처분 | 근거 |
| --- | --- | --- | --- | --- |
| ① | P1 같은 계정 두 역할 동시 세션 | **지원** | 진행 | 두 회차 모두 두 프로세스 실행 구간이 겹치고(3.811초, 3.352초) 두 역할 모두 오류 없는 결과 사건과 발급한 session_id를 냄 |
| ② | P2 역할별 자격·세션·저장소 접근 격리 | **성립 확인** | 진행 | 2a~2e 교차 시도 전부 거부. 같은 종류 컨테이너의 양성 대조(소유 UID 접근, 자기 토큰 탐지)는 성공 |
| ③ | P3 공급자 한도 구조 | **공유 확인** | 공유 quota group의 지원 구성으로 기록하고 진행 | 두 토큰의 공급자 발급 식별 클레임(`sub`·`ws`·`plan`)이 같고, 같은 창(`five_hour`·`seven_day`)의 `resetsAt`가 6쌍 관측 모두 같음 |

- 사용자 결정 D1에 따라 ③ 공유 확인은 **지원 구성**이다. 이것을 개발/Guardrail **한도 분리 성공으로 쓰지 않는다.**
- 01-04 수용 항목 “개발/Guardrail 한도 분리”의 결과는 **분리되지 않음(같은 quota group 공유)**이다.
- 자격 격리(②)와 공급자 quota 구조(③)는 따로 판정했다. ②의 성립이 ③의 분리를 뜻하지 않고, ③의 공유가 ②의 격리를 깨지도 않는다.

## 2. 실행 조건

- 자격: 방식 A. 사용자가 `claude setup-token`으로 발급해 `/opt/rust/ieo_poc_secrets`(디렉터리 0700, 파일 0600, 소유 heain)에 둔 토큰 2개를 썼다. 지문은 development `2456b85e9111`, guardrail `da968d393412`이다. 두 토큰은 서로 다르고 접두는 정상이다([setup.json](evidence/setup.json)).
- 토큰 값은 어떤 증거 파일에도 없다. 저장할 때 토큰 값과 `sk-ant-` 패턴을 검사했고, 커밋 전에 증거 전체를 원문 대조로 다시 검사했다.
- 역할 경계: rootless podman(netavark), 역할별 컨테이너, UID 20001/20002. 호스트에서 본 볼륨 소유자는 185536/185537(subuid 165536 기준 매핑)이고 0700이다.
- 실행 파일: `claude` 2.1.322 사본(SHA-256 `bfb3703a…`). 기반 이미지: `golang:1.26.9-bookworm`(`6880d90c0623…`).
- 모든 시도에 `--tools ''`와 빈 MCP 설정(`--strict-mcp-config`)을 지정했다. 결과는 tool_use 0건, `parent_tool_use_id` 사건 0건, 시간 초과 0건이다.

## 3. P1 동시 세션 — 지원

[P1.json](evidence/P1.json) · 시도 01~04

| 회차 | 개발 구간(epoch, 컨테이너 안) | Guardrail 구간 | 겹침 | 결과 |
| --- | --- | --- | --- | --- |
| 1 | 1794467141.446 – 145.257 | 1794467141.400 – 145.288 | 3.811초 | 둘 다 `success`, `ok-development`/`ok-guardrail`, session_id = 발급 UUID |
| 2 | 1794467146.196 – 149.991 | 1794467146.179 – 149.548 | 3.352초 | 같음 |

- 겹침은 컨테이너 안에서 `claude` 프로세스 바로 앞뒤에 찍은 시각으로 쟀다(같은 호스트 시계). 호스트 래퍼 시각은 참고용으로만 같이 두었다.
- 한계: 프로세스 실행 구간이 겹쳤다는 뜻이다. 두 모델 요청이 공급자 쪽에서 같은 순간에 처리됐다는 것을 직접 관측한 것은 아니다.

## 4. P2 접근 격리 — 성립 확인

[P2.json](evidence/P2.json) · 2c는 시도 05 · [P2-2c-target-control.txt](evidence/P2-2c-target-control.txt)

**수단 가용성과 차단 성공을 나눠 기록한다.**

- 수단 가용성(차단 성공의 증거가 아님): [setup.json](evidence/setup.json)의 `podman info`(rootless true), `/etc/subuid`(`heain:165536:65536`), 볼륨 소유자·모드. [host-isolation-check.txt](evidence/host-isolation-check.txt)와 [container-smoke.txt](evidence/container-smoke.txt)도 이 쪽이다.
- 차단 성공은 아래 실제 교차 시도로 판정했다.

| # | 시도 | 결과 | 기대 |
| --- | --- | --- | --- |
| 2a | 개발 컨테이너에서 Guardrail 자격·상태 경로 목록·읽기·쓰기 | 모두 `LIST_DENIED_OR_ABSENT`·`READ_DENIED_OR_ABSENT`·`WRITE_DENIED`. 마운트에 guardrail 없음(`0`). 호스트 쪽 Guardrail 볼륨에는 `INTRUSION_ABSENT`(2a-host) | 충족 |
| 2b | 두 볼륨을 함께 마운트한 컨테이너에서 UID 20001→20002 소유 경로, 반대 방향도 | 양방향 5개 경로 모두 `LIST_DENIED`·`READ_DENIED`, `WRITE_DENIED` | 충족 |
| 2p | 양성 대조: 같은 종류 컨테이너에서 소유 UID가 자기 볼륨에 접근 | 양 역할 모두 `LIST_OK`·`READ_OK`·`WRITE_OK` | 대조 성공(2b의 거부가 SELinux·마운트가 아니라 UID 경계 때문임) |
| 2c | 개발 컨테이너에서 Guardrail P1 세션 `--resume` | 종료 코드 1, `error_during_execution`, “No conversation found with session ID: 73305c11-…”, 모델 사용 0 토큰 | 충족 |
| 2c 대조 | 대상 세션 기록의 실제 위치 | Guardrail 볼륨에 1개(31,167 bytes, uid 20002), 개발 볼륨에 0개 | 재개 실패가 존재하지 않는 ID 때문이 아님 |
| 2d-control | 양성 대조: 같은 검사로 개발 자신의 토큰은 찾음 | `ENV_MATCH`, `FILE_MATCH /run/ieo/cred/oauth_token`, `TOKEN_PRESENT` | 대조 성공 |
| 2d | 개발 컨테이너의 환경과 읽을 수 있는 파일 12개 전체에 Guardrail 토큰과 같은 값 | `TOKEN_ABSENT` | 충족 |
| 2e | 호스트 비특권 사용자 heain이 두 역할의 0700 볼륨 읽기 | 양쪽 `Permission denied`, `READ_DENIED` | 충족 |

- 한계: heain은 `podman unshare`로 user namespace에 들어가면 두 역할 볼륨을 모두 읽을 수 있다. 이 보고서의 식별 증거도 그렇게 읽었다. heain이 두 역할의 상위 관리자라는 뜻이며, 역할 사이의 격리와는 다른 문제다. 운영 경계 S3(Pod·PSA)는 따로 확인한다.
- 2c는 CLI의 세션 기록이 역할 볼륨에 있어서 다른 역할 프로세스가 그 기록에 닿지 못한다는 것을 보인다. 공급자 쪽에 session_id로 다른 토큰의 대화를 재개하는 경로가 없다는 것까지 보인 것은 아니다.

## 5. P3 공급자 한도 구조 — 공유 확인

[P3.json](evidence/P3.json) · 시도 06~09 · [P3-identity.json](evidence/P3-identity.json)

**한도 창 관측.** stream-json의 `rate_limit_event`를 6쌍(P1 2회차 + P3 2회차, 각 회차 두 역할이 동시에) 관측했다.

| 관측 | `status` | `five_hour` utilization / resetsAt | `seven_day` utilization / resetsAt |
| --- | --- | --- | --- |
| P1 1·2회차, 두 역할 | allowed | 0.11 / 1794479400 | 0.29 / 1794729600 |
| P3 1·2회차, 두 역할 | allowed | 0.12 / 1794479400 | 0.29 / 1794729600 |

- 같은 회차의 두 역할은 모든 창의 utilization과 resetsAt가 같았다.
- `status`는 모두 `allowed`였다. 한도 신호(거부·제한)는 없었으므로 예산 규칙 §4의 정지 조건에 해당하지 않는다.

**계정 식별.**
- `claude auth status --json`은 토큰 인증에서 `authMethod=oauth_token`, `apiProvider=firstParty`만 내고 계정·org 필드는 없었다(CLI 기동 2회, 모델 요청 아님).
- 그래서 각 역할 CLI가 공급자에게서 받아 저장한 policy-limits JWS(ES256)의 클레임으로 식별했다. `sub`(uuid), `ws`(uuid), `plan`(`consumer`)이 두 역할에서 같다.
- 로컬에서만 생기는 값(`.claude.json`의 `userID`, 토큰별 stamp)은 다르다. 두 설정 디렉터리가 같은 파일을 공유한 결과가 아니라, 각자 받은 문서라는 뜻이다.

**판정.**
- 공유 확인의 첫 조건을 충족한다: 계정·org 식별이 같고, 같은 `rateLimitType` 창의 `resetsAt`가 같다.
- 두 번째 조건(한 역할의 사용 뒤 다른 역할의 utilization이 함께 움직임)은 근거로 쓰지 않았다. 0.11→0.12 변화는 두 역할 모두 사용한 구간에서 일어났고, 같은 구독의 다른 사용과도 구별할 수 없다.

**검토가 판단할 한계.**
- 식별 출처(JWS 클레임)는 계획한 `auth status`에 계정 필드가 없다는 것을 본 **뒤에** 더했다(계획 §5). 판정 기준은 바꾸지 않았다.
- JWS 서명은 검증하지 않았다(venv에 암호 라이브러리가 없다). 클레임은 CLI가 TLS로 받은 그대로 읽었다.
- `sub`·`ws`의 의미(계정·작업공간)는 클레임 이름과 문서 종류에서 추론한 것이다. 공급자 문서로 확인한 것이 아니다.
- 계정이 같다는 사실만으로 판정하지 않았다. 계획 §3에 따라 같은 창의 resetsAt를 함께 썼다.

**영향.**
- 개발·Guardrail은 같은 5시간·7일 창을 나눠 쓴다. 한쪽이 한도에 닿으면 다른 쪽도 같은 재개 시각을 기다린다.
- 런타임 계약 §8과 D6(공급자 신호 우선 재개)은 이 구성을 전제로 적용한다.
- 비용 필드: `total_cost_usd` 합은 0.0175로, CLI가 계산한 명목값이다. API key는 쓰지 않았고 추가 API 지출은 0이다.

## 6. 예산과 01-04 남은 범위

- 모델 요청 시도 **9/30**: P1 4회, P2 1회, P3 4회. 수동 재시도 0회.
- 벽시계: 첫 시도 07:05:40.941Z부터 마지막 종료 07:06:50.184Z까지 69초. 7,200초 중 일부다.
- 모델 없는 CLI 기동: `auth status` 2회. 모델 없는 컨테이너 실행: P2 파일·프로세스 시험 10건과 setup. 원본은 각 증거에 있다.
- 가짜 토큰 사전 점검([dry-run-dummy-tokens/](evidence/dry-run-dummy-tokens/README.md))은 하네스 점검이다. 판정 증거가 아니다.

판정표 처분이 셋 다 “진행”이므로 01-04의 나머지 범위로 넘어갈 수 있다. 남은 수용 항목은 다음과 같다.

- 기본 시험 repo·DB·수신기와 역할별 허용/거부
- 무인 기동/권한 확인
- 이름↔세션 재개
- 기록 위치·압축 감지
- 동시 세션 수
- 세션/자식별 토큰
- 런타임 갱신 전후 재개

이 범위는 실행 전에 별도 계획으로 고정한다. 01-04 전체 상한은 14,400초(F05)다.

## 7. 재현

```
.venv-poc/bin/python -I probes/01-04/probe.py setup|p1|p2|p3|status
podman unshare .venv-poc/bin/python -I probes/01-04/identity_claims.py /opt/rust/ieo_poc_probe/roles/{development,guardrail}/state/claude
```

다시 실행하면 새 시도로 센다(예산 규칙 §2). 이 결과를 재시험으로 늘려 세지 않는다.
