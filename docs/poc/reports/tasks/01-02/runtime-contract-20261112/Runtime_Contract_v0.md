# 01-02 · 에이전트 런타임·역할·인증·재개 계약 v0

2026-11-12 KST · 계획 v1.3.3 · 이전 회차 [runtime-comparison-20261112](../runtime-comparison-20261112/REPORT.md)(후보 비교)를 이어받는 회차.
이 문서는 **설계 계약**이다. “지원”은 설치 버전의 도움말/상태 출력에 해당 옵션이 존재함을 뜻하며, 동작 성공은 01-04 probe 전까지 미확인이다.

## 1. 선택과 적용 범위

| 항목 | 고정값 |
| --- | --- |
| 사용자 선택 | **Claude Code CLI 우선 검증** (2026-11-12 대화, 사용자 원문 “01-02는 기존 권고대로 Claude Code CLI를 우선 검증 대상으로 선택”) |
| 대안 | Codex CLI(`codex-cli 0.166.0` 설치 관측). 01-04에서 Claude Code CLI가 필수 계약을 못 채우면 영향·전환 결정을 요청한다. 임의 전환 금지 |
| 적용 런타임 버전 | Claude Code `2.1.322` ([claude-version.txt](evidence/claude-version.txt)). 버전이 바뀌면 이 계약의 지원 표를 재확인하고 01-04의 “런타임 갱신 전후 재개” probe 대상이 된다 |
| 두 역할 | 개발(`development`)·Guardrail(`guardrail`). 같은 런타임을 각각 격리해 쓴다. 세션·자격·볼륨·설정 디렉터리를 공유하지 않는다 |
| 제외 | 이 대화를 수행하는 Claude Code 세션(PoC 개발 도구)은 제품 런타임 인스턴스가 아니며 그 자격/세션을 역할 Pod에 재사용하지 않는다. 그 환경 변수(`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_MESSAGING_*` 등)는 역할 프로세스에 상속하지 않는다 |

## 2. U3 배치 계약 (repo당 두 Pod, 각 컨테이너 하나)

| 항목 | 개발 Pod `<repo>-dev` | Guardrail Pod `<repo>-guardrail` |
| --- | --- | --- |
| 라벨 | `ieo.repo=<repo>`, `ieo.role=development` | `ieo.repo=<repo>`, `ieo.role=guardrail` |
| 컨테이너 | 1개. PID 1 = repo supervisor(development) | 1개. PID 1 = repo supervisor(guardrail) |
| 실행 UID/GID | 20001/20001 (컨테이너 전용, root 아님) | 20002/20002 |
| 영속 볼륨(역할 전용, RW) | `ieo-<repo>-dev-state` | `ieo-<repo>-guardrail-state` |
| 볼륨 내부 배치 | `/state/home`(HOME) · `/state/claude`(`CLAUDE_CONFIG_DIR`) · `/state/work`(worktree) · `/state/checkpoints` | 동일 구조, 별도 볼륨 |
| 공유 볼륨 | 없음. 상대 역할 볼륨·Secret 마운트 금지 | 없음 |
| 자격(Secret, RO 마운트) | `ieo-<repo>-dev-cred` | `ieo-<repo>-guardrail-cred` |
| GitHub 자격 | 대상 repo의 작업 브랜치 push·PR 생성만 | push 없음. 고정 SHA 읽기만. 검토 결과는 코디네이터 API로 claim 제출 |
| 같은 호스트 초기 단계 | podman 컨테이너 2개(위 값 동일), 볼륨 2개 | |

코디네이터는 별도 단일 활성 컨테이너(UID 20000)다. supervisor는 자기 Pod의 역할 하나만 start/reconnect/stop/status 하고 상대 Pod·컨테이너 런타임 admin 권한을 갖지 않는다. 배정·판정·정책 승인 권한도 없다.

## 3. 인증 계약

설치 관측([claude-auth-status.redacted.txt](evidence/claude-auth-status.redacted.txt)): 이 서버의 현 사용자에 **claude.ai 로그인 1개**(authMethod `claude.ai`, subscriptionType `max`, planTier `max_5x`, configDirectory `~/.claude`)만 있다. API key·별도 계정은 관측되지 않았다.

| 자격 방식 | 설치 버전 근거 | 역할 Pod 사용 | 자격 분리 | 공급자 한도 분리 |
| --- | --- | --- | --- | --- |
| A. 역할별 장기 OAuth 토큰(`claude setup-token` → `CLAUDE_CODE_OAUTH_TOKEN`) | `setup-token` 명령 존재(구독 필요), 실행 파일에 `CLAUDE_CODE_OAUTH_TOKEN` 문자열 존재 | 역할별 별도 토큰을 각 Secret에 | 토큰 단위로 분리·폐기 가능 여부 probe | **같은 구독 계정이면 공유가 예상됨** → 01-04에서 공유/분리/관측 불가 판정 |
| B. 역할별 별도 구독 계정 | 계정 미보유 | 계정별 토큰 | 분리 | 계정 단위 분리 가능성 probe |
| C. 역할별 API key(`ANTHROPIC_API_KEY`, `--bare`는 이것만 사용) | `--bare` 도움말·실행 파일 문자열 | 역할별 key 또는 별도 workspace | key 단위 분리 | 조직/workspace 한도 구조 probe |
| D. 현재 대화 세션의 `~/.claude` 로그인 공유 | — | **금지** | 분리 불가 | — |

- 역할 프로세스는 자기 Secret의 값만 환경으로 받고 `CLAUDE_CONFIG_DIR=/state/claude`로 설정·세션 기록을 자기 볼륨에 둔다. 호스트 `~/.claude`를 마운트하지 않는다.
- 자격 획득·교체는 사람 정책 소유자(사용자)가 수행한다. 런타임/에이전트가 로그인·토큰 발급을 스스로 하지 않는다. 비밀 값은 보고·원장·로그에 넣지 않고 Secret 이름/지문만 남긴다.
- `quota_group_id`는 “공급자/계정 또는 org/모델 계열” 단위로 기록한다. A/D이면 두 역할이 같은 group이다. 실제 공유 범위는 01-04에서 원본 로그로 판정한다. 별도 컨테이너·`CLAUDE_CONFIG_DIR`·`--max-budget-usd` 같은 로컬 예산은 공급자 한도 분리의 증거가 아니다.

## 4. 기동·재개·식별 계약

| 계약 | 설치 버전 수단(도움말 근거) | 01-04 확인 |
| --- | --- | --- |
| 무인 기동 | `-p/--print`, `--output-format stream-json`(+`--verbose`), `--input-format stream-json`, `--permission-mode dontAsk`, `--permission-prompts none`, `--allowedTools/--disallowedTools`, `--tools`, `--setting-sources`, `--strict-mcp-config`, `--settings` | 역할별 허용/거부가 실제로 적용되는지, 프롬프트 대기 0건 |
| session ID 사전 지정 | `--session-id <uuid>` | 코디네이터가 UUID를 먼저 발급→DB 기록→기동. 결과 사건의 session_id가 같은지 |
| 논리 이름 | `-n/--name <name>`(표시 이름) | 표시 이름은 `<repo>-dev`·`<repo>-guardrail`. **재개 키로 쓰지 않는다**(이름은 유일성 보장 근거 없음) |
| 재개 | `-r/--resume <session-id>`; 복제는 `--fork-session` | 같은 ID로 이어지는지, 실행 중 세션 재개 시 동작(`--bg`의 “copy” 설명 참고) |
| 기록 위치 | `CLAUDE_CONFIG_DIR`(실행 파일 문자열, `auth status`의 configDirectory/projectsDirectory) | 역할 볼륨 아래에만 기록, 재생성 후 복원 |
| 비영속 실행 | `--no-session-persistence` | 재개가 필요 없는 일회성 probe에만 사용 |
| 압축 | `--autocompact <auto|tokens>` | 압축 사건 감지 가능 여부 |
| 구조화 결과 | `--output-format json|stream-json`, `--json-schema` | 결과 사건의 session_id·usage·비용 필드 실제 존재 |
| 예산 | `--max-budget-usd`(print 전용, 로컬 상한) | 로컬 상한일 뿐 공급자 한도 아님 |
| 사용 안 함 | `--dangerously-skip-permissions`, `bypassPermissions`, `--bg/attach/agents`(supervisor가 프로세스 수명 관리), `--cloud`, `--remote-control`, `--chrome` | — |

매핑 레코드(제품 DB, state.json 아님): `logical_name ↔ session_id ↔ role ↔ repo_id ↔ workspace_ref ↔ runtime_instance_id ↔ assignment_id/generation ↔ quota_group_id`. 이름이 같다는 사실만으로 재개 성공을 판정하지 않고, 재개 뒤 첫 결과 사건의 session_id와 체크포인트를 대조한다.

## 5. 체크포인트·재개 절차

1. supervisor는 단계 전환·인수/첫 행동·종료/한도 직전·예외 시 `/state/checkpoints/<generation>/<seq>.json`을 쓴다: session_id, generation, 대상 SHA, worktree HEAD·미커밋 diff 해시, 마지막 stream-json 사건 오프셋, 미확인 외부 요청 ID.
2. 재기동 시 `reconnect`: 체크포인트의 session_id로 `--resume`, 현재 배정 세대와 대조. 세대가 낡으면 `STALE_GENERATION`으로 중단.
3. repo 기록(승인된 명세·코드)이 세션 기억과 다르면 repo 기록이 우선이다(1.2절).
4. 재개 실패/관측 불가는 `NOT_OBSERVED`로 보고하고 새 세션을 조용히 대체하지 않는다.

## 6. 자식 에이전트 경계

- 정의: `--agents`(JSON/파일)로 역할별 자식 정의를 고정한다. 자식 도구는 부모 역할 허용 목록의 부분집합이다.
- 자식은 별도 자격을 받지 않고 부모 프로세스 안에서만 동작한다. Guardrail 자식은 개발 맥락을 받지 않는다.
- 사용량: `--forward-subagent-text`(stream-json)로 `parent_tool_use_id`를 받아 자식 사건을 부모에 귀속한다. 자식 토큰 합산의 정확성은 P35/01-04 probe 대상이다.
- 코디네이터의 인수·보고 상대는 부모뿐이다.

## 7. 런타임 어댑터 v0 대응

1.2절의 start/reconnect/stop/status 요청·응답·오류 코드(`UNAUTHORIZED`, `STALE_GENERATION`, `ID_CONTENT_CONFLICT`, `UNSUPPORTED_CAPABILITY`, `NOT_OBSERVED`, `TRANSIENT_FAILURE`)를 그대로 쓴다. Claude Code 대응:

| 동작 | supervisor 구현 |
| --- | --- |
| start | `--session-id`=코디네이터 발급 UUID, `-n`=논리 이름, 역할별 고정 인자 집합, `CLAUDE_CONFIG_DIR`/HOME=역할 볼륨. `capability_gaps`에 01-04 미확인 항목을 그대로 보고 |
| reconnect | `--resume <session_id>`; `restored_checkpoint_ref`·`external_reconciliation_required` 반환 |
| stop | 체크포인트 기한 내 SIGTERM→관측→필요 시 SIGKILL. 자기 Pod 프로세스만 대상 |
| status | 프로세스 생존·마지막 체크포인트·session_id·`quota_group_id`·`provider_status_or_unknown`(관측 불가면 unknown) |

## 8. 01-04 probe 순서(이 계약의 확인 계획)

1. **동시 두 세션**: 같은 계정 자격으로 개발·Guardrail 역할 프로세스를 실제로 겹쳐 실행(각자 `--session-id`, 각자 `CLAUDE_CONFIG_DIR`). 판정: 지원/미지원/관측 불가.
2. **자격 격리**: 역할 A 프로세스가 역할 B의 설정 디렉터리·세션 기록·Secret을 읽거나 B 명의로 동작할 수 있는지. 판정: 지원/미지원/관측 불가.
3. **공급자 한도 공유·분리**: 두 역할의 quota group이 같은지. 판정을 **공유 확인 / 분리 확인 / 관측 불가**로 나눈다. 로컬 예산·별도 컨테이너는 근거로 쓰지 않는다.
4. 위 1~3 중 불가 또는 관측 불가가 있으면 원본 로그·영향·대안·재개 결정을 보고하고 다음 probe/작업을 진행하지 않는다(현행 중단 조건, 임의 변경 금지).
5. 이후: 무인 허용/거부, 이름↔세션 재개, 기록 위치·압축 감지, 세션/자식별 토큰, 런타임 갱신 전후 재개.

## 9. 미확정(이 계약이 정하지 않는 것)

- 실제 역할별 자격 방식(A/B/C) 선택과 획득 — 사람 정책 소유자 결정. 01-04 첫 probe 전 필요.
- probe 1회당 모델 사용 상한·간격(01-03 동결).
- 역할별 상시 유지/idle 정지(P34 뒤 결정).
