# 01-04 첫 세 probe 계획 (실행 전 고정)

계획 v1.3.3-A1 · 01-04 run `first-probes-20261112` · 판정표 [A1-GATE-01-04](../../01-03/staged-freeze-20261112/probe_gate_table.json) · 예산 [Probe_Budget_Rules.md](../../01-03/staged-freeze-20261112/Probe_Budget_Rules.md) · 런타임 계약 [rev 5 §8](../../01-02/runtime-contract-20261112/Runtime_Contract_v0.md)

**판정 기준은 실행 전에 이 문서로 고정한다.** 결과를 본 뒤 기준을 바꾸면 새 revision으로 남기고 최초 기준으로 소급하지 않는다. P26 후보 부족은 01-04의 차단 조건이 아니다([후속 지시 5](../../../../User_Decisions_2026-11-12_followup.md)).

## 0. 준비 (모델 호출 없음)

| 항목 | 방법 | 증거 구분 |
| --- | --- | --- |
| 역할 자격 | 사용자가 `claude setup-token`을 두 번 실행해 개발·Guardrail 토큰을 각각 `/opt/rust/ieo_poc_secrets/{development,guardrail}.token`(디렉터리 0700, 파일 0600)에 저장한다. 개발 에이전트는 값을 출력·기록하지 않고 SHA-256 앞 12자 지문만 남긴다. 두 지문이 다르면 서로 다른 토큰이다 | 지문 기록 |
| 역할 경계 | rootless podman 컨테이너를 역할마다 하나 만든다(U3의 Pod 대응). 컨테이너 안 UID는 개발 20001, Guardrail 20002다. 역할 볼륨(`/state`, `/run/ieo/cred`)은 해당 UID 소유 0700이며 자기 컨테이너에만 마운트한다. `claude` 실행 파일 2.1.322는 SHA-256이 같은 0755 사본(`/opt/rust/ieo_poc_probe/bin/claude-2.1.322`)을 읽기 전용으로 마운트하고, 기반 이미지는 로컬 `golang:1.26.9-bookworm`이다 | — |
| 기동 점검 | 토큰 없이 역할 UID로 `claude --version`·`auth status` 실행([container-smoke.txt](evidence/container-smoke.txt)): 호스트 로그인이 보이지 않음(`loggedIn=false`) | 가용성·누출 없음 증거 |
| **수단 가용성** | `podman info`(rootless), `/etc/subuid` 범위, 두 볼륨의 호스트 측 소유 UID(165536 기준 매핑)를 기록한다 | **가용성 증거.** 접근 차단 성공의 증거가 아니다 |

## 1. P1 같은 계정 두 역할 동시 세션

- 실행: 두 컨테이너에서 `claude -p`를 동시에 시작한다. 인자는 `--output-format stream-json --verbose --session-id <역할별 UUID> --tools "" --strict-mcp-config --mcp-config <빈 파일> --forward-subagent-text --setting-sources user --permission-mode dontAsk`이고, 환경은 §4.3 허용 목록 + `CLAUDE_CODE_OAUTH_TOKEN`(파일에서 읽음)이다. 프롬프트는 “Reply with exactly: ok-<role>”이다.
- **지원**: 두 프로세스의 실행 구간이 겹친다(기동·종료 시각). 두 프로세스 모두 결과 사건(`type=result`, 오류 아님)을 내고, session_id가 각자 발급 UUID와 같다. 두 번째 시도도 같으면 확정한다(1회 우연 배제).
- **미지원**: 동시 실행 때 한쪽이 동시성·세션 충돌 오류로 실패하고, 단독 실행에서는 성공한다.
- **관측 불가**: 인증 실패 등으로 두 역할 모두 결과를 내지 못하거나 겹침을 확인할 수 없다.
- 예산: 시도 8회 이내(동시 실행 1회 = 2회).

## 2. P2 역할별 자격·세션·저장소 접근 격리

모델 호출 없이 하는 파일·프로세스 시험이 중심이다(30회에 넣지 않음). 시험마다 명령과 결과 원문을 남긴다.

| # | 시도 | 기대(성립) |
| --- | --- | --- |
| 2a | 개발 컨테이너(UID 20001)에서 Guardrail의 `/run/ieo/cred`·`/state`(설정·세션 기록·scratch) 읽기·쓰기·목록 | 경로가 보이지 않음(마운트 없음) |
| 2b | 같은 파일시스템을 공유할 때를 가정한 교차 시험: 두 역할 볼륨을 모두 마운트한 시험 컨테이너를 UID 20001로 실행해 20002 소유 0700 경로를 읽기·쓰기·목록 시도, 반대 방향도 | 모두 권한 거부 |
| 2c | 개발 프로세스가 Guardrail 세션 재개 시도: 개발 컨테이너에서 `--resume <Guardrail session_id>`(모델 요청 시도 1회) | 세션 없음 오류(다른 역할 기록 접근 불가) |
| 2d | 다른 역할 명의 동작: 개발 컨테이너 환경·파일에 Guardrail 토큰이 존재하지 않음(지문 대조) | 없음 |
| 2e | 호스트의 비특권 사용자(개발 대화 세션과 같은 `heain`)가 역할 볼륨의 0700 경로를 직접 읽기 | 권한 거부 |

- **성립 확인**: 2a~2e의 모든 시도가 거부된다는 것을 원본 로그로 확인한다.
- **실패**: 하나라도 읽기·쓰기·재개·명의 사용이 된다.
- **미확인**: 경계를 만들 수 없거나 결과를 관측할 수 없다.
- 2e의 한계: `podman unshare`로 들어가면 heain이 subuid 범위 파일을 읽을 수 있다. 이것은 호스트 사용자가 두 역할의 상위 관리자라는 뜻이며, 역할 사이 격리와는 별개로 기록한다(운영 경계 S3는 Pod·PSA로 별도 확인).

## 3. P3 공급자 한도 구조

- 관측 경로: 두 역할의 stream-json에서 나오는 `rate_limit_event`(`rateLimitType`·`resetsAt`·`utilization` 등)와 `claude auth status`(토큰별 계정·org 식별. 원문은 지문으로만 기록).
- **공유 확인**: 두 토큰의 계정·org 식별이 같고, 같은 `rateLimitType` 창에 대해 두 역할이 보고한 `resetsAt`가 같다. 또는 한 역할의 사용 뒤 다른 역할이 보고한 `utilization`이 함께 움직인다. → 지원 구성(공유 quota group)으로 기록하고 진행한다.
- **분리 확인**: 같은 창의 `resetsAt`·`utilization`이 독립적으로 다르고, 계정·org 식별도 다르다.
- **관측 불가**: 한도 정보가 출력에 없고, 계정 식별도 얻을 수 없다. → 중단한다.
- 별도 컨테이너·`CLAUDE_CONFIG_DIR`·로컬 예산은 근거로 쓰지 않는다. 계정이 같다는 사실만으로는 공유 확인으로 판정하지 않는다(한도 창 관측이 필요).
- 예산: 시도 10회 이내. 한도 신호가 나오면 새 시도를 멈춘다(예산 규칙 §4).

## 4. 기록

- 시도별 원본은 `evidence/attempts/NN-<probe>-<role>/`에 둔다(argv, 환경 변수 이름, 시각, 종료 코드, stdout, stderr). 토큰 값은 어떤 파일에도 쓰지 않는다. 저장 전에 토큰 지문과 `sk-ant-` 접두 패턴을 검사한다.
- 누적 시도 수와 벽시계는 `evidence/budget.log`에 둔다.

## 5. 실행 후 덧붙인 측정 (기준 불변, 실행 뒤 추가)

- 이 절은 P1~P3 실행(2026-11-12 07:05~07:07Z) **뒤에** 추가했다. §1~§3의 판정 기준은 바꾸지 않았다.
- P3 계정 식별: 계획한 관측 경로 `claude auth status --json`은 토큰 인증(`authMethod=oauth_token`)에서 계정·org 필드를 내지 않았다(`fields_present`에 없음, [P3.json](evidence/P3.json)).
- 그래서 결과를 본 뒤 식별 출처를 하나 더했다. 각 역할 CLI가 공급자에게서 받아 자기 설정 디렉터리에 저장한 policy-limits JWS의 클레임(`sub`·`ws`·`plan`)이다. 모델 요청은 쓰지 않았다. 값은 남기지 않고 형태(uuid 가림)와 SHA-256 앞 12자만 남겼다([P3-identity.json](evidence/P3-identity.json), [identity_claims.py](../../../../../../probes/01-04/identity_claims.py)).
- 판정 기준을 바꾸지 않은 사후 식별 출처 추가라는 점을 그대로 기록하고, 독립 검토가 그 타당성을 판단한다.
- P2 2c 대조: 재개 대상 세션 기록이 Guardrail 볼륨에만 있고 개발 볼륨에는 없음을 확인했다([P2-2c-target-control.txt](evidence/P2-2c-target-control.txt)). 재개 실패가 존재하지 않는 세션 ID 때문이 아님을 보이는 대조다.
