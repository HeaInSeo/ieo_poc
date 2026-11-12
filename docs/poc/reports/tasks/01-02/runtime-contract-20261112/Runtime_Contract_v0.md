# 01-02 · 에이전트 런타임·역할·인증·재개 계약 v0

2026-11-12 KST · 계획 v1.3.3 · 이전 회차 [runtime-comparison-20261112](../runtime-comparison-20261112/REPORT.md)(후보 비교)를 이어받는 회차.
문서 revision 4: 독립 검토 7a39452(reject) F1~F9, bf854bb(reject) N1~N7, 2e5a5ef(reject) R1~R4 반영. 이 문서는 **설계 계약**이다. “근거”는 설치 버전 도움말/상태 출력에 옵션·문구가 **존재**함을 뜻하며, 동작은 01-04 probe 전까지 미확인이다.

## 1. 선택과 적용 범위

| 항목 | 고정값 |
| --- | --- |
| 사용자 선택 | **Claude Code CLI 우선 검증** (2026-11-12 대화, 사용자 원문 “01-02는 기존 권고대로 Claude Code CLI를 우선 검증 대상으로 선택할게.”) |
| 대안 | Codex CLI(이 서버 설치 `codex-cli 0.166.0`, [codex-version.txt](evidence/codex-version.txt)). 01-04에서 필수 계약을 못 채우면 영향·전환 결정을 요청한다. 임의 전환 금지 |
| 적용 런타임 버전 | Claude Code `2.1.322` ([claude-version.txt](evidence/claude-version.txt)). 버전이 바뀌면 이 계약의 근거 표를 재확인하고 01-04 “런타임 갱신 전후 재개” probe 대상이 된다 |
| 두 역할 | 개발(`development`)·Guardrail(`guardrail`). 같은 런타임을 각각 격리해 쓴다. 세션·자격·볼륨·설정을 공유하지 않는다 |
| 제외 | 이 대화를 수행하는 Claude Code 세션(PoC 개발 도구)은 제품 런타임 인스턴스가 아니다. 그 자격·세션·환경 변수를 역할 프로세스·probe에 넘기지 않는다(§4.3) |

## 2. U3 배치 계약 (repo당 두 Pod, 각 컨테이너 하나)

| 항목 | 개발 Pod `<repo>-dev` | Guardrail Pod `<repo>-guardrail` |
| --- | --- | --- |
| 라벨 | `ieo.repo=<repo>`, `ieo.role=development` | `ieo.repo=<repo>`, `ieo.role=guardrail` |
| 컨테이너 | 1개. PID 1 = repo supervisor(development) | 1개. PID 1 = repo supervisor(guardrail) |
| 실행 UID/GID | 20001/20001 (root 아님, 추가 capability 없음) | 20002/20002 |
| 역할 전용 영속 볼륨(RW) | `ieo-<repo>-dev-state` | `ieo-<repo>-guardrail-state` |
| 볼륨 배치 | `/state/home`(HOME) · `/state/claude`(`CLAUDE_CONFIG_DIR`, 세션 기록) · `/state/work`(worktree) | 같은 구조, 별도 볼륨 |
| 읽기 전용 마운트 | `/etc/ieo/role/`(supervisor 제공 설정·MCP 목록·자식 정의), `/run/ieo/cred/`(Secret) | 같은 구조, 자기 역할 것만 |
| 공유 볼륨 | 없음. 상대 역할 볼륨·Secret·설정 마운트 금지 | 없음 |
| GitHub 자격 | 대상 repo 작업 브랜치 push·PR 생성만 | push 없음. 고정 SHA 읽기 |
| 같은 호스트 초기 단계 | podman 컨테이너 2개(위 값 동일), 볼륨 2개 | |

코디네이터는 별도 단일 활성 컨테이너(UID 20000)다. supervisor는 자기 Pod의 역할 하나만 start/reconnect/stop/status 하고 상대 Pod·컨테이너 런타임 admin 권한을 갖지 않는다. 배정·판정·정책 승인 권한도 없다.

### 2.1 같은 컨테이너 안의 supervisor와 에이전트(알려진 한계)

U3의 “Pod당 컨테이너 하나”와 S3의 PSA Restricted(추가 capability 금지)에서는 PID 1 supervisor와 에이전트 프로세스를 **다른 UID로 나눌 수 없다**. 같은 UID의 프로세스는 supervisor의 파일·환경(모델 자격, 코디네이터 claim 자격)을 읽을 수 있다고 가정한다. 따라서 경계는 “그 컨테이너 안에서 **누구의 코드가 실행되는가**”로 둔다.

- **Guardrail 컨테이너에서는 검토 대상(개발이 작성한) 코드·테스트·빌드 스크립트·hook을 실행하지 않는다**(검토 N1). Guardrail 컨테이너에서 실행되는 코드는 런타임·supervisor·Guardrail 모델의 도구 호출뿐이다.
- **Guardrail의 독립 재현·재실행은 격리 실행 경로로 한다**(검토 R1, P07의 “정상 CI에 드러나지 않는 고정 반례 재현”, P20/P24의 독립 재실행, P22의 역할별 쓰기 가능한 빌드/시험 공간): Guardrail은 자기 쓰기 공간 `/state/guardrail-scratch/`에 반례 입력·시험 파일·명령을 작성하고 **재현 요청**(claim: 대상 SHA, 패치/파일 묶음 digest, 명령, 기대값)을 코디네이터에 제출한다. 코디네이터는 **자격·Secret이 없는 일회성 격리 실행기**(코디네이터 소유, 대상 SHA 체크아웃+요청 묶음, 네트워크 기본 차단, 시간/자원 상한)에서 실행하고 원본 로그·종료 코드·실행기 이미지 digest를 Guardrail 입력 위치로 돌려준다. 필수 검사 결과는 CI/코디네이터 실행기가 보호된 경로로 발행한 것을 고정 SHA 기준으로 받는다(§1.2 “완료와 필수 검사”). 이 경로의 API는 01-08 계약에 추가하고, 실행기 구현은 02-06, 실제 검증은 P07(04-07)에서 한다.
- 개발 컨테이너는 개발 코드를 실행한다. 그 안의 자격(개발 모델 자격, 개발 claim 자격, push 자격)은 개발 코드에 노출된다고 보고 **그 범위가 개발 역할의 권한과 같도록** 둔다. 개발 컨테이너에서 Claude 권한 규칙은 보안 경계가 아니다. 경계는 컨테이너·자격 범위·코디네이터 판정이다.
- **push 시점 제한(§1.9, 검토 R2)**: push 자격은 코디네이터가 배정 단계가 `개발` 또는 반려 뒤 `교정`일 때만 발급하는 **단기 GitHub App 설치 토큰**(대상 repo 하나, contents/pull_requests 쓰기, 만료 ≤ 1시간)이며 `REVIEW_PENDING` 진입 시 폐기한다. 토큰은 브랜치를 한정하지 못하므로 main은 보호 설정(PR 필수·필수 체크·관리자 포함)으로 막고, 그 밖의 브랜치는 **탐지**로 다룬다: 코디네이터가 단계 밖 head 변경을 관측하면 정책 위반을 기록하고 검토 배정을 무효화·재배정하며, 병합은 승인 head SHA 서버 측 조건부 병합으로만 한다. 이 방식의 실제 거부·탐지는 05-03/P07에서 검증한다.
- 개발과 Guardrail은 **직접 통신하지 않는다**(§1.2). 서로의 네트워크 주소·자격·볼륨을 받지 않으며 Guardrail 입력은 승인 명세·프로파일·GitHub 고정 SHA·보호된 검사 결과뿐이다.
- **판정·체크포인트의 정본은 코디네이터 DB와 외부 상태(GitHub SHA·CI)**다. supervisor는 체크포인트 digest·순번을 코디네이터에 보고하고, 재개 시 코디네이터 기록·외부 상태와 다르면 `external_reconciliation_required=true`로 멈춘다. 개발 컨테이너의 체크포인트는 개발 코드가 바꿀 수 있으므로(검토 N5) 무결성을 보장하지 않으며, 재개 판단은 GitHub의 브랜치/SHA·CI 기록과의 대조로 한다.
- 코디네이터→supervisor 어댑터 호출은 코디네이터 전용 자격으로 인증하며 supervisor는 그 검증 키(공개 키/토큰 해시)만 가진다.
- 이 경계의 실제 위반 시도(개발 코드가 Guardrail 자격 획득·Guardrail 명의 발행 시도, 0건이어야 함)는 P07(04-07)에서 검증한다. Pod 분리 선언만으로 독립성 PASS를 주지 않는다.

## 3. 인증 계약

설치 관측([claude-auth-status.redacted.txt](evidence/claude-auth-status.redacted.txt)): 현 사용자에 **claude.ai 로그인 1개**(authMethod `claude.ai`, subscriptionType `max`, configDirectory `~/.claude`)만 있다. API key·별도 계정은 관측되지 않았다. `CLAUDE_CONFIG_DIR`을 빈 디렉터리로 지정하면 이 로그인이 보이지 않는다는 관측은 검토 세션의 미커밋 기록뿐이므로 01-04에서 원본 로그로 확인한다.

계약(방식과 무관하게 고정):

1. 역할마다 **별도 자격**을 자기 Secret(`/run/ieo/cred/`)으로만 받는다. 호스트 `~/.claude`·현재 대화 세션의 자격(D)은 금지한다.
2. 자격 획득·교체·폐기는 사람 정책 소유자(사용자)가 한다. 런타임/에이전트는 로그인·토큰 발급을 하지 않는다. 비밀 값은 보고·원장·로그에 넣지 않고 Secret 이름·지문만 남긴다.
3. `quota_group_id`는 공급자/계정 또는 org/모델 계열 단위로 기록한다. 두 역할이 같은 group인지는 **01-04에서 원본 로그로 공유 확인/분리 확인/관측 불가 중 하나로 판정**한다. 별도 컨테이너·`CLAUDE_CONFIG_DIR`·`--max-budget-usd`는 공급자 한도 분리의 근거가 아니다.

후보 방식(선택은 사용자 바인딩, 01-04 첫 probe 전에 필요):

| 방식 | 설치 버전 근거 | 자격 분리 | 공급자 한도 | 확인할 것 |
| --- | --- | --- | --- | --- |
| A. 같은 구독 계정의 역할별 장기 토큰(`claude setup-token`) | `setup-token` 명령 존재(“requires Claude subscription”). `CLAUDE_CODE_OAUTH_TOKEN` 문자열은 실행 파일에 존재하나 우선순위·적용 방식은 미확인 | 토큰 단위 분리·폐기 가능 여부 probe | 같은 계정이므로 **공유 가능성이 높다**(판정 아님) | 토큰 주입 경로, 공유/분리 판정 |
| B. 역할별 별도 구독 계정 | 계정 미보유 | 계정 단위 | 계정 단위 분리 여부 probe | 계정 확보 |
| C. 역할별 API key 또는 `apiKeyHelper`(`--bare`는 이 둘만 사용) | `--bare` 도움말 | key 단위 | org/workspace 한도 구조 probe | key 발급·한도 구조 |
| D. 호스트·대화 세션 로그인 공유 | — | 불가 | — | **금지** |

## 4. 기동·설정 신뢰·재개 계약

### 4.1 역할별 설정 신뢰·실행 범위

**규칙**: 역할 프로세스는 **supervisor가 읽기 전용으로 제공한 설정만** 적용한다. repo worktree나 `CLAUDE_CONFIG_DIR` 아래 에이전트가 쓸 수 있는 위치에서 온 권한 규칙·hook·MCP·플러그인·skill·명령·자식 정의·사용자 CLAUDE.md는 적용하지 않는다.

| 역할 | repo CLAUDE.md | repo `.claude/`·hook·플러그인·`.mcp.json` | 도구 | 설정 출처 |
| --- | --- | --- | --- | --- |
| 개발 | 허용. 작업 브랜치의 미병합 수정도 읽히며 이는 개발 자신의 작업 메모다. 승인된 누적 지식은 main에 병합된 CLAUDE.md다(검토 N7) | 적용 금지 | 쓰기·Bash 포함(컨테이너 안). 권한 규칙은 보안 경계 아님(§2.1) | `/etc/ieo/role/` RO |
| Guardrail | **적용 금지**. 검토 대상으로 읽기만 | **적용 금지** | Read·Grep·Glob(입력 경로 한정) + Write·Edit(`/state/guardrail-scratch/**`만). Bash·네트워크 게시 없음. 검토 대상 실행 금지, 재현은 격리 실행 요청(§2.1) | `/etc/ieo/role/` RO |

고정 인자 집합. 근거는 [claude-help.txt](evidence/claude-help.txt) 옵션 존재이며 각 인자가 규칙을 달성하는지는 §8 probe G로 확인한다:

```text
공통   claude -p --input-format stream-json --output-format stream-json --verbose
         --session-id <코디네이터 발급 UUID> -n <repo>-<role>
         --permission-mode dontAsk --permission-prompts none
         --setting-sources user --settings /etc/ieo/role/settings.json
         --strict-mcp-config --mcp-config /etc/ieo/role/mcp.json
         --agents /etc/ieo/role/agents.json
         --allowedTools <역할 허용 목록> --disallowedTools <역할 거부 목록>
개발   cwd=/state/work/<repo>
Guardrail cwd=/state/guardrail-cwd(빈 디렉터리), --disable-slash-commands,
         --tools "Read,Grep,Glob,Write,Edit" (--add-dir는 쓰지 않음)
         읽기 허용: /state/review/<sha>/** (코드 스냅샷, RO)
                    /state/inputs/<assignment_id>/** (승인 명세·프로파일·보호된 검사 결과·재현 결과 원본, RO)
                    /state/guardrail-scratch/**
         쓰기 허용: /state/guardrail-scratch/**
         읽기 거부: /run/ieo/**, /etc/ieo/**, /state/claude/**, /state/home/**, /proc/**
```

- **--add-dir를 Guardrail에 쓰지 않는다**(검토 N2): 도움말이 `--add-dir`를 CLAUDE.md 출처로 적는다. 검토 대상은 cwd 밖 읽기 전용 스냅샷으로 두고 경로 한정 Read/Grep/Glob 허용 규칙으로 읽게 한다. probe G가 그 스냅샷의 표식 CLAUDE.md가 로드되는지 확인한다. 로드되면 대안은 supervisor가 만든 검토 묶음(지시 파일 이름을 `*.reviewed`로 바꾼 사본+`review.patch`)이며, 그것도 실패하면 01-04 중단·결정 요청이다.
- **`CLAUDE_CONFIG_DIR` 보호**(검토 N3·R3): 자격은 §3대로 `/run/ieo/cred/`에서만 오며 `CLAUDE_CONFIG_DIR`에 자격 파일을 두지 않는다(기동 전 발견 시 삭제하고 기록). supervisor는 매 기동 전에 `CLAUDE_CONFIG_DIR` 아래에서 `--resume`에 필요한 세션 기록만 **제외한 전부**(settings, CLAUDE.md, agents, skills, commands, plugins 등)를 지우고 RO 템플릿으로 다시 만든 뒤 digest를 체크포인트에 남긴다. 실행 중 변조는 Guardrail은 쓰기 도구가 없어 불가하고, 개발은 가능하지만 개발 역할 범위 안의 영향이다(§2.1). 세션 기록 경로 안의 자동 기억(auto-memory) 같은 지시성 파일은 보존 대상이 아니다: 누적 지식은 repo 파일(PR·독립 검토)로만 남긴다(§1.2, Guardrail은 repo 검토 지식만). “user 출처 = `CLAUDE_CONFIG_DIR/settings.json`”, 재개에 필요한 경로 목록, 자동 기억의 위치·비활성 방법은 도움말에 없는 **가정**이며 probe G에서 확인한다(검토 N6·R3).
- **경로 한정 읽기/쓰기 규칙**(`Read(<path>)`류)의 문법·효과는 도움말에 `Bash(...)` 예만 있어 **가정**이다(검토 R4). probe G에서 허용·거부 경로를 실제로 시험하고, 효과가 없으면 `--restricted`(작업 디렉터리 한정)·별도 마운트 네임스페이스 등 대안을 비교한다.
- **비밀 유출 방지**(검토 R4): Guardrail 모델이 지시 주입으로 자기 자격을 검토문에 옮겨도 공개되지 않도록, 코디네이터는 claim을 저장·게시하기 전에 등록된 자격 지문·알려진 토큰 패턴을 검사하고 일치하면 게시를 막고 자격 교체 결정을 요청한다.
- user 출처를 완전히 끄는 방법(빈 출처 목록, `--safe-mode`와 `--agents` 동시 사용, `--bare`+`apiKeyHelper`)은 probe G에서 비교해 가장 좁은 것을 채택한다.
- **probe G에서 Guardrail이 검토 대상의 CLAUDE.md·settings·hook·MCP·플러그인을 하나라도 적용하거나, 읽기 전용 도구 밖의 실행이 일어나면** 그 방식은 불합격이다. 대안이 없으면 Guardrail 독립성(P07)이 성립하지 않으므로 01-04 결과로 중단·결정 요청 대상이다.

### 4.2 식별·재개

| 계약 | 수단(설치 버전 근거) | 01-04 확인 |
| --- | --- | --- |
| 무인 기동 | `-p`, `--output-format stream-json`, `--input-format stream-json`, `--permission-mode dontAsk`, `--permission-prompts none`(“anything that would prompt is denied automatically”) | 허용/거부 실제 적용, 프롬프트 대기 0건 |
| session ID 사전 지정 | `--session-id <uuid>` | 결과 사건의 session_id가 발급 UUID와 같은지 |
| 논리 이름 | `-n/--name`(표시 이름) | **재개 키로 쓰지 않는다**(유일성 근거 없음) |
| 재개 | `-r/--resume <session-id>`; 복제는 `--fork-session` | 같은 ID로 이어지는지, 실행 중 세션 재개 동작 |
| 기록 위치 | `CLAUDE_CONFIG_DIR`(실행 파일 문자열·`auth status`의 configDirectory) | 역할 볼륨 아래에만 기록, 재생성 후 복원 |
| 압축 | `--autocompact <auto|tokens>` | 압축 사건 감지 가능 여부 |
| 구조화 결과 | `--output-format json|stream-json`, `--json-schema` | 결과 사건의 session_id·usage·비용 필드 실제 존재 |
| 자식 사건 귀속 | `--forward-subagent-text`: 자식의 **텍스트·thinking**을 `parent_tool_use_id`와 함께 전달(사용량 전달은 도움말에 없음) | 자식별 사용량을 얻을 수 있는지 별도 확인 |
| 예산 | `--max-budget-usd`(print 전용, 로컬 상한) | 로컬 상한일 뿐 공급자 한도 아님 |
| 사용 안 함 | `--dangerously-skip-permissions`, `bypassPermissions`, `--bg/attach/agents`(프로세스 수명은 supervisor가 관리), `--cloud`, `--remote-control`, `--chrome` | — |

매핑 레코드(제품 DB, state.json 아님): `logical_name ↔ session_id ↔ role ↔ repo_id ↔ workspace_ref ↔ runtime_instance_id ↔ assignment_id/generation ↔ quota_group_id ↔ settings_digest`. 이름이 같다는 사실만으로 재개 성공을 판정하지 않는다.

### 4.3 환경 정리(검토 F9)

supervisor와 01-04 probe는 역할 프로세스를 **빈 환경에서 허용 목록만** 넣어 시작한다(`env -i` 상당): `HOME`, `PATH`, `LANG`, `CLAUDE_CONFIG_DIR`, 선택한 자격 변수 1개. 개발 대화 세션의 `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_MESSAGING_*`, `CLAUDE_CODE_CHILD_SESSION` 등은 넘기지 않는다. probe 원본 로그에 실제 전달한 변수 **이름** 목록을 남긴다(값 제외).

## 5. 체크포인트·재개 절차

1. supervisor는 단계 전환·인수/첫 행동·종료/한도 직전·예외 시 체크포인트를 쓴다: session_id, generation, assignment_id, 대상 SHA, **sprint_id/revision/digest, image_digest, profile ID/revision, settings_digest**, worktree HEAD·미커밋 diff 해시, 마지막 stream-json 사건 오프셋, 활성 자식 목록, 미확인 외부 요청 ID. digest·순번은 코디네이터에 보고한다(§2.1).
2. 재기동 시 `reconnect`: 체크포인트의 session_id로 `--resume`, 현재 배정 세대·코디네이터 기록 digest와 대조. 세대가 낡으면 `STALE_GENERATION`, digest가 다르면 외부 대조 필요로 중단.
3. repo 기록(승인된 명세·코드)이 세션 기억과 다르면 repo 기록이 우선이다(1.2절).
4. 재개 실패/관측 불가는 `NOT_OBSERVED`로 보고하고 새 세션을 조용히 대체하지 않는다.

## 6. 자식 에이전트 경계

- 정의: `/etc/ieo/role/agents.json`(RO)을 `--agents`로 준다. 자식 도구는 부모 역할 허용 목록의 부분집합이다. 자식은 별도 자격을 받지 않고 부모 프로세스 안에서만 동작한다. Guardrail 자식은 개발 맥락을 받지 않는다.
- 기록: supervisor는 stream-json에서 자식 사건(`parent_tool_use_id`)을 읽어 **자식 ID·부모 session_id·세대·부작업·외부 요청·관측 가능한 비용**을 체크포인트와 코디네이터 claim에 남긴다. 자식별 사용량을 얻지 못하면 “미계측”으로 남기고 0으로 쓰지 않는다(P35).
- 부모 중단: 재개 전에 체크포인트의 활성 자식 목록·미확인 외부 요청을 대조한다. 무조건 새 자식을 띄우지 않는다(§1.7).
- 코디네이터의 인수·보고 상대는 부모뿐이다.

## 7. 런타임 어댑터 v0 대응

1.2절의 start/reconnect/stop/status 요청·응답 필드를 그대로 쓴다.

| 동작 | supervisor 구현 |
| --- | --- |
| start | §4.1 고정 인자, `CLAUDE_CONFIG_DIR`/HOME=역할 볼륨, §4.3 환경. `capability_gaps`에 01-04 미확인 항목을 그대로 보고 |
| reconnect | `--resume <session_id>`; `restored_checkpoint_ref`·`external_reconciliation_required` 반환 |
| stop (role 범위) | 체크포인트 기한 내 SIGTERM→관측→필요 시 SIGKILL. 자기 Pod 프로세스만 |
| stop (repo 범위) | 코디네이터가 두 supervisor에 **각각** role 범위 stop을 보낸다. supervisor는 상대 역할을 멈추지 않으며, 각 역할의 안전 종료 조건(미확인 외부 효과·활성 자식·미저장 체크포인트 0)을 따로 보고한다 |
| status | 프로세스 생존·마지막 체크포인트·session_id·`quota_group_id`·`provider_status_or_unknown`(관측 불가면 unknown)·`last_actual_action_at` |

오류 대응. 어댑터 오류 코드(§1.2, 6종)는 **호출 자체의 실패**에만 쓴다. 공급자 한도·인증/과금은 재시도 가능한 일시 오류가 아니므로 `status`의 분류와 운영 상태로 보고한다(검토 N4, §1.2 “한도 종류와 공유”). 실제 런타임 오류 문자열은 01-04 원본 로그로 채운다.

| 관측 | 어댑터 응답 | 재시도 |
| --- | --- | --- |
| 호출자 자격 불일치·범위 밖 요청 | `UNAUTHORIZED` | 없음 |
| 배정 세대가 현재보다 낡음 | `STALE_GENERATION` | 없음 |
| 같은 `request_id`·다른 내용 | `ID_CONTENT_CONFLICT` | 없음 |
| 필요한 옵션/동작이 런타임에 없음 | `UNSUPPORTED_CAPABILITY` | 없음 |
| 프로세스/세션 상태·재개 결과를 관측할 수 없음 | `NOT_OBSERVED` | 외부 대조 후 판단 |
| 일시적 네트워크 오류·공급자 5xx·rate limit(짧은 대기) | `TRANSIENT_FAILURE` | 상한 있는 backoff |
| **공급자 사용량 한도** | 정상 응답 + `status.provider_status_or_unknown=usage_limit`, `quota_group_id`, 최초 오류 원문 참조 | 즉시 재시도 금지. 운영 상태 `작업을 보유한 대기`/`QUOTA_WAIT`, `next_check_at`에 01-03에서 정한 간격·상한의 재개 probe |
| **인증/과금 오류** | 정상 응답 + `provider_status_or_unknown=auth_or_billing` | 재시도 금지. `사람 결정 대기`로 결정 요청(자격 교체는 사람 정책 소유자) |
| 컨텍스트 한계·압축 | 오류 아님. 체크포인트 + 압축 사건 기록 | — |

## 8. 01-04 probe 순서(이 계약의 확인 계획)

1. **동시 두 세션**: 같은 계정 자격으로 개발·Guardrail 역할 프로세스를 실제로 겹쳐 실행(각자 `--session-id`, 각자 `CLAUDE_CONFIG_DIR`, §4.3 환경). 판정: 지원/미지원/관측 불가.
2. **역할별 자격 격리**: 역할 A가 역할 B의 설정 디렉터리·세션 기록·Secret을 읽거나 B 명의로 동작할 수 있는지. 판정: 지원/미지원/관측 불가.
3. **공급자 한도 공유·분리**: 두 역할의 quota group이 같은지. 판정을 **공유 확인 / 분리 확인 / 관측 불가**로 나눈다. 로컬 예산·별도 컨테이너는 근거로 쓰지 않는다.
4. 위 1~3 중 불가 또는 관측 불가가 있으면 원본 로그·영향·대안·재개 결정을 보고하고 다음 probe/작업을 진행하지 않는다(현행 중단 조건, 임의 변경 금지).
5. 이후: **G. 설정 신뢰·실행 범위**(검토 스냅샷에 표식 CLAUDE.md·settings·hook·`.mcp.json`·skill을 두고 Guardrail 고정 인자로 실행해 적용 0건, Guardrail의 Bash·scratch 밖 쓰기·검토 대상 실행·`/run/ieo`·`/proc` 읽기 시도 거부, 재현 요청이 격리 실행기(자격 0)에서 돌고 원본 로그가 돌아오는지, 개발 쪽 repo hook/settings 적용 0건, `CLAUDE_CONFIG_DIR` 아래 settings·CLAUDE.md·agents·skills 변조가 다음 기동에 반영되지 않음, 재개에 필요한 경로 목록 확인), 무인 허용/거부, 이름↔세션 재개, 기록 위치·압축 감지, 세션/자식별 토큰, 런타임 갱신 전후 재개.

## 9. 미확정(이 계약이 정하지 않는 것)

- 역할별 자격 방식 A/B/C의 선택과 획득 — 사람 정책 소유자 결정, 01-04 첫 probe 전 필요. 01-03 결정안과 함께 질문한다(01-03 수용 기준 자체의 항목은 아님).
- probe 1회당 모델 사용 상한·간격(01-03 동결).
- 역할별 허용/거부 도구 목록의 구체값 — 03-02 정책 문서에서 확정, 01-04 probe는 최소 목록으로 시험.
- 역할별 상시 유지/idle 정지(P34 뒤 결정).
