# 02-04 실제 세션 시험 계획 (실행 전 검토용)

| 항목 | 내용 |
| --- | --- |
| 근거 | 사용자 지시 2026-11-14 “남은 02-04를 기존 순서대로 … 첫 실제 호출에서 역할별 자격 확인 … 기존에 승인된 12회·시간·토큰 예산 안에서 나머지 실제 세션 시험 수행”. 런타임 계약 rev 6 §10 항목 1~3, rev 7 §11 항목 7 |
| 예산 | 결정 ① 02-04: 12회, 회당 1,800초, 총 14,400초, billable 1,500,000, cache_read 15,000,000. 추가 청구 USD 0. 미계측을 0으로 두지 않고 자동 재시도하지 않는다 |
| 계획 회차 | 모델 호출 7회(개발 6, Guardrail 1)와 모델 호출 없는 재시작 1단계(R1). 나머지 5회는 예비다. 예비는 자동으로 쓰지 않는다(§4) |
| 실행 위치 | 저장소 밖 작업 디렉터리(예: `/opt/rust/ieo_poc_runs/02-04-<날짜>`). 자격·어댑터 토큰·원본 stream·회차 기록이 여기 있다. 공개 저장소에는 요약과 기록의 사본만 올린다(§5) |
| 선행 조건 | 이 계획과 호출자(`ieo-role-call`)가 검토·병합되고, 컨테이너 예행([dry-run.sh](dry-run.sh), 가짜 Claude·가짜 자격)이 통과해야 한다. 진행 기록 3 §1의 실제 호출 선행 조건이 이것이다 |

## 1. 준비(모델 호출 없음)

1. [run-real.sh](run-real.sh)로 두 역할 컨테이너를 띄운다.
   - 새 작업 디렉터리에서만 동작한다. 역할 볼륨·기록·같은 이름의 컨테이너가 이미 있으면 아무것도 지우지 않고 멈춘다(검토 c4620d2 H2).
   - 이미지 `golang:1.26.9-bookworm`을 digest(`sha256:d9c68c2c…`)로 고정한다. 실제 CLI `claude-2.1.322`(01-04와 같은 파일, 읽기 전용 마운트), 네트워크 `slirp4netns`(01-04와 같음).
   - `--stop-timeout 15`(검토 475b409 R5).
   - 개발 UID 20001, Guardrail UID 20002. U3 배치는 가짜 시험(진행 기록 3)과 같다.
2. 자격은 `/opt/rust/ieo_poc_secrets/<역할>.token`(결정 ②)을 역할의 `/run/ieo/cred/oauth_token`(0400)에 복사한다.
   - 값은 출력하지 않는다.
   - OAuth 토큰 형식(구독 자격)인지 앞부분만 확인한다. 형식이 다르면(API 키 등) 중단한다.
3. 역할 설정(`/etc/ieo/role`, 읽기 전용):
   - 템플릿 `{"autoMemoryEnabled": false}`, 빈 `mcp.json`.
   - 개발 `settings.json`: 허용 규칙 `Read`·`Grep`·`Glob`·`Agent`·`Edit(//state/work/fixture0204/**)`. 개발 역할의 허용 목록은 03-02 몫이다(§9). 여기서는 시험에 필요한 만큼만 읽기 전용 설정으로 둔다.
   - 개발 `agents.json`: 자식 `reader`(Read만).
   - 개발 공급 CLAUDE.md: `MARKER-SUPPLIED-0204`, `--append-system-prompt-file`.
4. 개발 repo는 실제 Sori가 아니다. 시험용 저장소 `fixture0204`를 쓴다.
   - 작업 트리 CLAUDE.md에는 `MARKER-REPO-ONLY-0204`를 둔다.
   - `.claude/settings.json`의 SessionStart hook은 `HOOK-RAN` 파일을 만들도록 둔다.
   - 둘 다 적용되면 안 된다.
5. 첫 기동 전 개발 설정 디렉터리에 (a) 표식을 심는다: `settings.json`(자동 기억 켬), `CLAUDE.md`, `skills/marker0204`, `projects/-state-work-fixture0204/memory/MEMORY.md`.
6. 호출자 계획(`plan.json`):
   - 역할 몫은 개발 8회·9,600초·1,000,000·10,000,000, Guardrail 4회·4,800초·500,000·5,000,000이다. 합은 승인 예산과 같다.
   - 회당 창은 600초 이하다.
   - 첫 관측은 01-04의 가장 새로운 stream(27-AM2, 2026-11-12T08:05:53Z, `overageStatus: rejected`·`isUsingOverage: false`)이다. 이 관측은 7일 한도 안(2026-11-19T08:05Z까지)에서만 쓸 수 있다.

## 2. 회차와 합격 조건

회차마다 [call.sh](call.sh) `<작업 디렉터리> <회차>`를 한 번 실행한다. [steps.tsv](steps.tsv)의 순서대로 하고, 프롬프트는 [prompts/](prompts/)에 있다. 한 회차의 결과를 확인한 뒤에 다음 회차로 간다.

- call.sh는 매 호출 전에 자격 파일이 OAuth 형식인지 앞부분만 다시 확인한다.
- 종료 코드 0이면 기록됐고 볼 것이 없다. 2이면 시작되지 않았다. **그 밖(3, 1 등)은 모두 중단**이다. 결과 subtype·`is_error`·`stop_required`를 함께 출력한다.

| 회차 | 역할·조작 | 확인하는 것 | 합격 조건(모두) |
| --- | --- | --- | --- |
| D1 | 개발 start(새 세션 S) | **개발 자격**, §10 항목 3 (a) 정리, 항목 2 공급 CLAUDE.md와 repo 설정 미적용, (c)의 “도구 없음” 실행 | 결과 `success`·`is_error` false, 실행 신호 `overageStatus: rejected`·`isUsingOverage: false`. 답에 `MARKER-SUPPLIED-0204`가 있고 `REPO-ONLY`·`CONFIGDIR`·`SKILL`·`MEMORY` 표식은 없음. init에 표식 skill·`memory_paths` 없음. `HOOK-RAN` 없음. 정리 보고 `removed` ≥ 4, 자격 파일 삭제 없음. `/state`·`/state/claude`의 SELinux 라벨·모드 불변(호출 전후 `ls -dZ`) |
| G1 | Guardrail start | **Guardrail 자격**, §10 항목 1 rev 6 Guardrail 규칙 그대로(`Edit(//…)`만, `Write(` 없음) | 결과 `success`. `/state/guardrail-scratch/g1.txt` = OK. `/state/review/fixture/g1-denied.txt`·`/state/inputs/0204-RT/g1-denied.txt` 없음(거부 도구 오류 2건 이상). init 도구에 Bash 없음 |
| R1 | 개발 컨테이너 재시작(모델 호출 없음) | 기동·종료와 체크포인트 복원(사용자 결정 2026-11-14 pr30 §4) | `podman restart -t 15`가 정상 종료하고 PID 1이 다시 supervisor다. 재시작 전후 체크포인트(session_id S, `after_run`, 기준 회차 D1)가 같다. 이어지는 D2가 S로 재개되고 D1 기준으로 측정된다 |
| D2 | 개발 reconnect | (c) 도구 없는 실행 뒤 재개, 허용 도구(Read) | 같은 session_id S로 `resume`, 변조 오판 없음(`tamper` null). Read 사용, 답이 README 첫 줄 |
| D3 | 개발 reconnect | (c) 도구 실행 뒤 재개, 자식 에이전트 | 같은 S로 `resume`, 변조 오판 없음. Agent(`reader`) 호출, 답 3 |
| D4 | 개발 reconnect | (c) 자식 실행 뒤 재개, `/compact` | 같은 S로 `resume`, 변조 오판 없음. `compact_boundary` 1건 이상 |
| D5 | 개발 reconnect | (c) `/compact` 뒤 재개, §11 항목 7 | 같은 S로 `resume`, 변조 오판 없음. D1~D5 회차 몫 합 = D5 세션 누적(네 토큰 모두). 각 회차 몫은 측정됨 |
| D6 | 개발 reconnect(직전에 skill 표식 심음) | §10 항목 3 (b) 변조 탐지 | `--resume` 미실행(`mode: new`), 새 session_id, `config_tamper_detected`(이전 S), 심은 skill은 정리됨 |

- 자격 확인은 각 역할의 첫 실제 호출(D1, G1)에서 한다.
  - 기준: 위 결과 조건과 인증 오류 없음이다.
  - 자격 값과 그 일부는 어디에도 쓰지 않는다.
- 실제·모의 구분(사용자 결정 2026-11-14 pr30 §4):
  - **실제 회차로 확인**: 자격, §10 항목 1~3, §11 항목 7, 재시작 뒤 체크포인트 복원(R1, 회차 비용 없음).
  - **모의로만 확인(가짜 Claude, 진행 기록 3)**: 실행 중인 회차의 정지(어댑터 stop·시간 초과·실행 중 SIGTERM), §10 항목 4(세션당 단일 실행), 항목 5(재개 키는 session_id만). 보고에서도 “모의”로 적는다.
  - 실제 Claude 회차를 일부러 멈추면 그 회차는 설계상 미계측이고, 미계측 회차는 예산을 멈춘다(`budget::next`). 그래서 실제 정지 시험은 이 계획에 넣지 않는다. 필요하면 마지막 회차로 하되, 사용자 결정을 받은 뒤에만 한다.
- 예상 사용량: 01-04의 비슷한 회차는 회당 수 초~50초였고, 자식 회차의 세션 누적이 billable 약 3,900·cache_read 약 7,900이었다. 7회 합은 역할 몫보다 훨씬 작을 것으로 본다. 이것은 추정이며 상한 집행은 호출자와 guard가 한다.

## 3. 매 호출의 예산·추가 결제 확인(호출자)

무추가결제 조건은 세 출처를 함께 본다. 다만 출처마다 확인하는 시점이 다르다(검토 c4620d2 M2).

| 출처 | 언제 | 무엇을 |
| --- | --- | --- |
| 과거 관측 stream | 호출 전 | 가장 새로운 관측 stream(첫 회차는 01-04 27-AM2)이 7일 이내이고, 모든 `rate_limit_event`가 rejected·false·allowed다. 시계가 관측보다 이르면 보류한다 |
| 자격·환경 | 준비할 때와 매 호출 전(자격 형식), 실행 뒤(환경) | 자격 파일이 OAuth 토큰 형식이다(앞부분만, run-real.sh·call.sh). 실행 환경은 `env::build`의 허용 목록으로 고정된다. 실행 뒤 그 실행이 보고한 변수 이름에 `CLAUDE_CODE_OAUTH_TOKEN`이 있고 `ANTHROPIC_*`가 없는지 확인한다 |
| 이번 실행의 신호 | 실행 중과 실행 뒤 | guard가 첫 위반 `rate_limit_event`에서 멈춘다. 그 사건은 첫 API 요청 뒤에야 오므로, 이것은 **관측 뒤 정지**이고 공급자 쪽 차단이 아니다. 실행 뒤 stream을 다시 검사하고, 사건이 하나도 없으면 기록에 적는다(다음 호출은 보류된다) |

- **호출 전**:
  - 다른 호출자가 기록을 쥐고 있거나, 기록 없이 끝난 호출의 표식(`inflight.json`)이 있거나, 서비스의 마지막 회차가 호출자 기록에 없거나, 사람이 볼 기록이 있으면 시작하지 않는다.
  - 02-01 기록(두 역할 전부)으로 `budget::next`·`Limits::for_next`를 계산하고, 역할 몫과 회당 창 600초로 줄인다.
- **시작 요청 직전**: `inflight.json`을 쓴다. 회차 기록을 덧붙인 뒤에만 지운다. 호출자가 실행 중에 죽으면 표식이 남아 다음 호출을 막는다(검토 c4620d2 H1).
- **실행 중**: guard가 관측 사용량(하한)·시간·첫 위반 `rate_limit_event`에서 멈춘다.
- **실행 뒤**:
  - stream을 계측기로 읽고 `run::charge_reported`로 반영한다(guard가 멈춘 회차는 미계측).
  - 재개 기준은 호출자 자신의 기록 중 실제로 재개한 직전 회차다.
  - 결과가 `success`·`is_error` false가 아니면(인증 실패 등) `stop_required`다.
  - 시작 요청 뒤의 모든 오류(응답 분실, 상태 조회 실패, stdout 조회·저장 실패)는 미계측·`stop_required` 기록이 된다.
  - 기록은 잠금 아래 한 번에 쓰고 fsync한다.
  - stream은 서비스가 UTF-8로 읽은 텍스트이며, 저장본과 SHA-256도 그 텍스트의 것이다.

## 4. 중단 조건과 결정 요청

다음 중 하나면 그 자리에서 멈춘다. 우회하거나, 예산을 초기화하거나, 자동 재시도하지 않는다. 정확한 상태와 필요한 결정을 보고한다.

| 상황 | 기록 | 필요한 결정 |
| --- | --- | --- |
| 호출 전 보류(`Hold`: 관측 없음·7일 초과·rejected 아님) | 호출 안 함(기록 없음) | 무추가결제 조건을 다시 확인할 방법 |
| 예산·몫 소진(`Stop`, `BUDGET_EXCEEDED`) | 호출 안 함 | 예산 |
| 인증 실패(첫 호출의 인증 오류) | 그 회차 기록(사용량은 측정된 만큼, 없으면 미계측) | 자격 교체(사용자 소유, 계약 §3) |
| 미계측 회차(guard 정지, 재개 기준 불일치, 결과 없음) | 기록 `usage.run` unmeasured, 다음 호출은 `budget::next`가 막음 | 그 회차 사용량의 처리 |
| `stop_required`(정리 실패, 실행 오류, 회차 분실, 이번 실행 신호 위반, 환경 불일치) | 기록에 사유 | 원인 조사와 재개 여부 |
| 합격 조건 불충족(예: 허용 도구 거부, 표식 로드, 변조 오판) | 기록과 요약 | 원인. 하네스 원인이면 검토된 수정 뒤 예비 회차 사용 여부 |

**수동 대조 절차(검토 475b409 R3)**:
- 역할 서비스의 `/state/budget/attempts.jsonl`에 끝 줄이 없는 회차가 있으면 서비스가 다음 시작을 거부한다.
- 사람이 원인을 확인한 뒤에만 끝 줄(그 회차의 관측 하한, 모르면 회차 몫 전체)을 덧붙인다.
- 끝 줄이 `{"unknown": true}`(실행 오류)인 회차는 끝 줄을 덧붙여도 풀리지 않는다. 사람이 원인을 확인한 뒤 그 줄을 관측 하한이나 회차 몫 전체로 바꾸고, 바꾼 사실을 보고에 적는다(검토 c4620d2 L9).
- 호출자의 `inflight.json`이 남았으면 서비스 상태·실행 디렉터리로 그 회차를 확인한다. 확인 결과를 기록(미계측이면 미계측으로)에 남긴 뒤에만 표식을 지운다.
- 호출자 기록의 `stop_required`와 미계측 회차는 사용자 결정 없이 지우거나 고치지 않는다.

## 5. 증거와 공개 범위

- **공개 저장소에 올리는 것**:
  - 회차 요약(`summaries/<회차>.json`, [summarize.py](summarize.py): init 항목, 도구 사용·오류 요약, 결과 400자, 압축, rate-limit 상태, 사용량, stream 크기·SHA-256)
  - 호출자 기록 사본
  - 볼륨 라벨 확인
  - 보고
- **저장소 밖에 두는 것**: 원본 stream, 자격, 어댑터 토큰, 역할 볼륨.
- 공개 전에 요약·기록에서 자격 형태 문자열을 검사한다.

## 6. 예행(가짜 Claude)

[dry-run.sh](dry-run.sh)가 같은 run-real.sh·ieo-role-call·call.sh·summarize.py로 7회를 컨테이너에서 실행한다. 가짜 Claude와 가짜 OAuth 형식 문자열을 쓴다. 결과는 [evidence/dry-run.log](evidence/dry-run.log)에 있다.

| 확인 | 결과 |
| --- | --- |
| 기록 | 7건, `stop_required` 0건, 모두 측정됨. R1 재시작 뒤에도 체크포인트가 같다 |
| 재개 기준 | D2~D5가 D1 세션을 직전 회차 기준으로 재개함 |
| §11 항목 7 | D1~D5 몫 합 = D5 세션 누적 |
| (b) D6 | 변조 탐지, 새 세션 |
| (a) D1 | 심은 항목 정리 |
| 환경 | OAuth 변수만 있음 |
| 비밀 | 요약·로그에 비밀 문자열 0건 |
| 정지 | 정지 제한 15초 |
| 거부 | 진행 중 표식이 있으면 호출을 거부하고, 사용한 작업 디렉터리에서는 준비를 거부한다 |

모델 동작(표식 로드 여부, 도구 허용·거부, 자식, 압축)은 가짜 Claude로 확인할 수 없다. 이것은 실제 회차의 몫이다.
