# 02-05 단계 2 · supervisor 작업 셋(자기 배정 조회·첫 행동·세션 상태) HTTP·gRPC

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 2([계획](../plan-20261115/PLAN.md) §3 단계 2, §3.1 작업 표, §2.5 T1). 이월 C13을 닫는다 |
| 착수 근거 | 계획 §2.6과 계획 머리의 갱신 기록(사용자 결정 2026-11-15, X-08 검수 완료, 런타임 계약은 revision 11로 씀) |
| 검토 반영 | 계획 PR 검토 3f9909f I-1(OpenAPI 범위 문장), PR #72 검토 23877c1 I-2(draft 상태 명세의 C2 사례) |
| 기준 커밋 | main 089fb95. 구현 커밋 15b2f03, 증거 커밋 0574abc |
| 모델 호출 | 0회. 컨테이너·실제 회차 없음. 영속 PoC DB는 쓰지 않았다(일회성 시험 DB만) |
| 상태 | 구현과 시험 완료, **독립 검토 전**. 같은 PR의 원장 snapshot이 이 단계를 기록한다 |

## 1. 더한 작업

세 작업 모두 코디네이터 작업 표(`api::Operation`·`route`·`dispatch`)에 한 번씩 넣었다. HTTP와 gRPC 어댑터는 같은 처리 함수를 부른다. 자격은 셋 다 **그 (repo, role)의 supervisor 자격**(종류 agent, `checkpoint:report`)뿐이다(T1). 모델 자격(`agent:report`), 다른 역할·repo의 supervisor, 클라이언트 자격(`roles:control`·`coordinator:assign` 포함)은 403이다. 새 범위는 없다.

| 작업 | HTTP (operationId) | gRPC (`SupervisorService`) | 상태 변경·멱등 |
| --- | --- | --- | --- |
| 자기 배정 조회 | `GET /v0/repos/{repo_id}/roles/{role}/assignment` (getOwnAssignment) | `GetOwnAssignment` | 없음(읽기). 현재 배정 없음은 404 `NO_CURRENT_ASSIGNMENT` |
| 첫 행동 기록 | `POST /v0/repos/{repo_id}/roles/{role}/first-actions` (recordFirstAction) | `RecordFirstAction` | 있음. 키 (assignment_id, generation, attempt_id), 자격의 (repo, role) 안. 같은 내용 200, 다른 내용 409 `EVENT_CONTENT_CONFLICT` |
| 세션 상태 보고 | `POST /v0/repos/{repo_id}/roles/{role}/session-status` (reportSessionStatus) | `ReportSessionStatus` | 있음(이력 추가). 키 `Idempotency-Key` / `idempotency_key`(8~128바이트). 같은 상태 200, 다른 상태 409 `IDEMPOTENCY_KEY_CONFLICT` |

**자기 배정 조회.** 현재 배정 머리를 코디네이터 기록대로 돌려준다: assignment_id·generation·task_id·target_sha, sprint_id·revision·spec digest·`sources[]`(명세 그대로), 그 revision의 `limits`, (sprint, task, role)의 `attempts_issued`, 정책 digest, 발급 시각, `last_delivery_id`.

**첫 행동 기록.** 본문은 회차의 (assignment_id, generation, attempt_id), session_id, stream 안의 오프셋(`event_offset`), supervisor 시계의 관측 시각(`observed_at`, UTC RFC 3339 `…Z`), 종류(`tool_use`·`text`)다. claim(`evidence_level = claim`)으로 저장한다.
- 그 배정이 이 (repo, role)에 발급된 적이 없으면 409 `STALE_ASSIGNMENT`(에이전트 claim과 같은 코드).
- 그 세션의 takeover 체크포인트가 그 배정·세대로 저장돼 있어야 한다(아니면 409 `CHECKPOINT_MISMATCH`).
- 낡은 세대: 체크포인트의 “회차 뒤 체크포인트는 시작 때 배정으로 끝난다” 규칙과 같다. takeover가 그 세대로 저장된 세션이면 배정이 바뀐 뒤에도 저장하고 `current_assignment: false`로 남긴다. 새 세대에서 옛 세션의 첫 행동은 409다.

**세션 상태 보고.** 본문은 런타임 계약 §7 status 항목을 그대로 싣는다. `observed_at`, `lifecycle`(idle·running·stopping), `session_id`, `assignment_id`·`generation`(둘 다 있거나 둘 다 null), `quota_group_id`, `provider_status_or_unknown`, `last_actual_action_at`, `checkpoint_seq`·`checkpoint_sha256`(둘 다 있거나 둘 다 null), `pending_external_requests`·`active_children`(`"not_tracked"` 또는 객체 목록, 0은 거부)이다. 모든 항목이 있어야 한다(모르면 null).
- 코디네이터 쪽 이력(`role_session_status`)에 쌓는다.
- 이미 저장된 상태보다 관측 시각이 같거나 이른 보고는 이력에는 남기되 `stale_on_arrival: true`로 표시한다. 현재 값으로 읽지 않는다(02-04 항목 4의 신선도 규칙).
- 이름 붙인 배정이 이 (repo, role)에 발급된 적이 없으면 409 `STALE_ASSIGNMENT`. 예전 배정이면 `current_assignment: false`, 배정이 없으면 null.

**claim이며 완료 신호가 아니다.** 두 표는 `evidence_level`이 `claim`만 되도록 DB CHECK로 막았다. 사건(`events`)을 만들지 않고, 스프린트 상태와 판정 입력에 들어가지 않는다. 판정(02-03 `judge`)은 이 표를 읽지 않는다. 시험 `supervisor_operations_answer_the_assignment_and_store_claims`가 이것을 확인한다: 스프린트 상태 불변, 바뀐 표는 `history`·`role_checkpoints`·`role_first_actions`·`role_session_status`뿐.

**검사 순서.**
- 자기 배정 조회: 401 → 역할 이름 404 → 403 → 404 `NO_CURRENT_ASSIGNMENT`.
- 첫 행동: 401 → 404 → 403 → 형식 400 → 잠금 → 키 재조회·충돌 → 배정 409 → takeover 409 → 저장.
- 세션 상태: 401 → 404 → 403 → 멱등 키 400 → 형식 400 → 잠금 → 키 재조회·충돌 → 배정 409 → 저장.

두 쓰기 작업은 체크포인트 보고와 같은 잠금(`checkpoint:{repo}:{role}`)을 잡는다. 저장 전에는 쓰기가 없으므로 거부는 행을 남기지 않는다.

## 2. 바꾼 계약과 코드

- `crates/ieo-coord/src/api.rs`: 작업 셋, 처리 함수 `own_assignment`·`record_first_action`·`report_session_status`, 시각 형식 검사 `is_utc_timestamp`. 기존 작업의 처리는 바꾸지 않았다.
- `crates/ieo-coord/src/schema.sql`: `role_first_actions`·`role_session_status`(추가 전용 트리거). `CREATE … IF NOT EXISTS`이므로 영속 DB에는 다음 기동의 `migrate`가 새 표만 더한다.
- `contracts/ieo-api-v0.openapi.yaml` 0.9.0 → **0.10.0**(추가).
  - 세 경로와 `FirstActionReceipt`·`SessionStatusReceipt`를 더했다. Problem 코드에 `NO_CURRENT_ASSIGNMENT`를 더했다(404 응답).
  - 범위 문장(3f9909f I-1)을 고쳤다. 옛 문장 “Agent credentials carry only `agent:*` scopes”는 `checkpoint:report`를 빠뜨렸다. 새 문장은 이렇다: 에이전트 범위는 `agent:report`·`agent:repro`(guardrail)·`checkpoint:report`이고, `checkpoint:report`는 그 (repo, role)의 supervisor 범위다(체크포인트, 자기 배정, 첫 행동, 세션 상태). 모델 프로세스는 쥐지 않고, 같은 UID가 읽을 수 있으므로 기록은 claim이다.
  - `openapi-spec-validator` 통과.
- `contracts/proto/ieo/internal/v0/supervisor.proto`: RPC 셋과 메시지(새 메시지·새 enum `FirstActionKind`, 기존 메시지의 필드 번호 변경 없음). `buf lint`·`buf build`·`buf breaking`(origin/main 대비, WIRE_JSON) 통과.
  - 세션 상태는 `status_json` 바이트로 싣는다. 보고·체크포인트처럼 HTTP 본문을 바꾸지 않고 넘기므로 두 전송이 같은 검사를 받는다. 단계 6이 목록 항목 모양을 바꿔도 proto를 고치지 않는다.
  - 첫 행동은 타입 필드다(재현 요청과 같은 방식). 어댑터가 모든 필드를 늘 넣은 JSON을 만든다.
- `crates/ieo-proto`: JSON ↔ 메시지 변환(`own_assignment_*`, `first_action_*`, `session_status_receipt_*`)과 왕복 단위 시험.
- `crates/ieo-coord/src/grpc.rs`: `Rpc` 변형 셋. `idempotency_key` 필드를 HTTP `Idempotency-Key`로 옮긴다(빈 값은 헤더 없음과 같고, 둘 다 400 `MISSING_FIELD`). 경로 값 검사(`/`·`?`·`#`)를 새 RPC에도 적용한다.
- `contracts/ieo-internal-v0.md`:
  - §1.1 표, §3(T1: supervisor만 코디네이터 자격을 쥔다, `checkpoint:report`의 범위, 새 RPC의 검사 순서), §4.2 `code` 표, §5 재시도·멱등 표에 행을 더했다.
  - §1.2·§2·§8.2·§11 문장을 고쳤다.
  - 바뀐 문장 목록을 새 §13에 적었다.
- `crates/ieo-supervisor/tests/{central,serve}.rs`: 가짜 서버가 새 RPC를 `unimplemented`로 답하도록 했다(trait 구현 완결). supervisor 코드는 바꾸지 않았다.
- **런타임 계약은 바꾸지 않았다.** 이 단계는 supervisor 동작이나 어댑터 필드의 뜻을 바꾸지 않는다. supervisor가 이 작업을 부르는 동작(단계 3·4)과 체크포인트의 `delivery_id` 등은 그 단계에서 revision 11로 적는다.

## 3. 시험

`crates/ieo-coord/tests/grpc.rs`(실제 PostgreSQL, 실제 HTTP 서버와 gRPC 어댑터):
- **공통 회귀 확장**(`both_transports_give_the_same_answers_and_the_same_database`): 같은 단계를 HTTP 환경과 gRPC 환경에 각각 보내 (상태, 본문)과 DB 표 내용(새 두 표 포함)이 같은지 본다. 거부에서는 DB가 그대로인지도 본다. 새 작업에 넣은 단계는 72개다.
  - 인증·범위: 자격 없음 401, 모델 자격·클라이언트·배정자 403 `SCOPE_DENIED`, 다른 역할 supervisor 403 `ROLE_MISMATCH`, 다른 repo 403 `REPO_SCOPE_DENIED`, 알 수 없는 역할 404, 경로의 `/` 404.
  - 형식 400: `kind` 미지정, 없는 날짜, UTC가 아닌 시각, 빈 attempt_id, 멱등 키 없음·짧음, lifecycle, 0으로 쓴 미추적, 항목 누락·모르는 항목, 짝 없는 digest·세대, JSON 아님, UTF-8 아님, 1 MiB 초과.
  - 멱등: 같은 키 재조회 200(세션 상태는 다른 바이트의 같은 JSON 값도 200), 내용 충돌 409 `EVENT_CONTENT_CONFLICT`·`IDEMPOTENCY_KEY_CONFLICT`.
  - 낡은 배정·세대: 발급되지 않은 배정 409 `STALE_ASSIGNMENT`, takeover 없는 세션 409 `CHECKPOINT_MISMATCH`, 새 세대 뒤 옛 회차의 첫 행동 201 `current_assignment: false`, 새 세대의 옛 세션 409, 옛 세대 세션 상태 `current_assignment: false`, 늦게 도착한 관측 `stale_on_arrival: true`.
  - 도달 확인 목록에 `(404, NO_CURRENT_ASSIGNMENT)`·`(409, IDEMPOTENCY_KEY_CONFLICT)`를 더했다.
- **전송 사이 재조회**(`a_record_stored_over_one_transport_replays_over_the_other`): 첫 행동을 gRPC로 저장하고 HTTP로 재조회, 세션 상태를 HTTP로 저장하고 gRPC로 재조회(그 반대도). 재조회는 200이고 행이 늘지 않는다.
- **새 시험** `supervisor_operations_answer_the_assignment_and_store_claims`:
  - 자기 배정 조회의 전체 응답이 명세·배정과 같은지, 새 세대 뒤 머리를 따르는지(generation 1, SHA2, attempts 2).
  - 자격별 403 코드.
  - takeover 전 첫 행동 409, 내용 충돌 409, 키 충돌 409.
  - claim만 저장되고 스프린트 상태가 바뀌지 않는지, 저장된 `current_assignment`의 재조회 값.
- I-2(23877c1): `c2_a_binding_sprint_id_is_reserved_for_its_repo`에 draft 상태 명세(`registration_state: draft`, 한도 null)의 다른 repo 등록 409 `SPRINT_REPO_CONFLICT` 사례(c2n2-0007)를 더했다.

결과([evidence/test.log](evidence/test.log), 커밋 15b2f03):
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`: 통과.
- `buf lint`·`buf build`·`buf breaking`(origin/main 대비), `openapi-spec-validator`: 통과.
- `cargo test --locked --all-targets`(시험 DB 포함): **358 통과, 0 실패, 0 무시**(기준 357 + 새 시험 1). grpc 10개, coordinator 75개. 기록 끝에는 공통 회귀를 `--nocapture`로 다시 돌린 새 작업 단계의 답이 있다(그래서 기록 안 `test result` 합은 359).
- `checkpoint_e2e …_over_grpc`는 이번 실행에서 실패하지 않았다.

변이 6개, 모두 잡혔다([evidence/mutants.txt](evidence/mutants.txt)).
- 처음 실행에서는 S2-M1(takeover 조건 제거)·M3(자기 배정 조회의 자격 검사 제거)·M4(첫 행동 재조회의 내용 비교 제거)가 살아남았다. 공통 회귀는 두 전송이 **같은지**만 보므로, 두 전송에 똑같이 들어간 규칙 결함은 잡지 못한다.
- 그래서 새 시험에 기대 코드를 직접 확인하는 문장을 더했다. 다시 돌린 결과는 6개 모두 실패(잡힘)다.

## 4. 이월 항목

- **C13 닫음**(02-05 새 작업의 HTTP·gRPC 형태, 토큰을 쥐는 프로세스). 세 작업이 작업 표에 한 번씩 있고 HTTP·gRPC 형태와 공통 회귀를 갖는다. T1은 OpenAPI 범위 문장과 내부 계약 §3·§11에 적었다.
- **C14 일부 준비**(`last_actual_action_at`이 끝난 회차 기준). 첫 행동과 세션 상태를 받을 자리를 만들었다. supervisor가 실제로 보내고 status가 이 관측으로 갱신되는 것은 단계 4다.
- **C1·C8 준비.** 자기 배정 조회는 단계 3에서 supervisor가 start·reconnect 요청의 `coordinator` 기록을 DB와 대조할 때 쓴다.
- 그 밖의 이월(C2·C6은 단계 1에서 닫힘, C7·C9·C10·C11·C12)은 이 단계와 무관하다.

## 5. 계획과 다른 점, 결정이 필요한 것

- **첫 행동에 저장된 takeover를 요구한다.** 계획 §3.1은 키와 멱등만 적었다. 낡은 세대 처리 규칙이 필요해 체크포인트의 회차 연속 규칙을 그대로 썼다. 인수(takeover) 뒤에만 첫 행동이 있다는 단계 4의 연결 순서와도 맞다. 로컬 전용(`local_only`) supervisor는 takeover를 보고하지 않으므로 첫 행동도 기록할 수 없다(시험용 설정).
- **자기 배정 조회에 없는 것.**
  - “남은 양”: 시간·토큰 소비는 단계 5 전에는 코디네이터가 세지 않으므로 넣지 않았다. 대신 revision의 `limits`와 `attempts_issued`를 준다. 고정된 이어가기의 시도 상한(최신 실행 가능 revision 기준)은 배정 처리에만 있는 규칙이라 다시 계산하지 않았다.
  - “이미지·프로파일 기대값”: 코디네이터가 기록하지 않는 값이다. 지금은 start 요청의 `coordinator` 기록에서 온다. 단계 3이 전달 기록에 둘 때 응답에 더한다(추가 필드).
  - `last_delivery_id`는 단계 3까지 null이다.
- **세션 상태의 조회 경로는 아직 없다.** 계획의 “조회 때 supervisor에 닿지 못해도 마지막 보고와 그 시각을 보인다”는 저장과 신선도 표시까지만 했다. 보이는 곳(repo 역할 상태 응답 또는 단계 4 연결 조회)은 기존 `GET /v0/repos/{repo_id}/roles`의 HTTP 동작을 바꾸지 않기 위해 이 단계에서 정하지 않았다. 단계 4에서 연결 조회에 넣는 것을 제안한다.
- **시각은 UTC `…Z`만 받는다.** 문자열 비교 없이 PostgreSQL이 거부할 값을 먼저 거른다(없는 날짜, 1970~9999 밖). supervisor의 기록 형식(`ieo-evidence::time::rfc3339`)과 같다.
- 사용자 결정이 필요한 것은 없다. 정책 파일·판정 규칙·기존 작업의 HTTP 동작은 바꾸지 않았다.

## 6. 한계

- supervisor는 아직 이 작업을 부르지 않는다(단계 3·4). 지금은 코디네이터 쪽 계약과 처리만 있다.
- 기록은 claim이다. supervisor 자격은 같은 UID 프로세스가 읽을 수 있다(런타임 계약 §2.1). 재개의 최종 근거는 단계 7의 GitHub 대조다.
- 세션 상태의 `stale_on_arrival`은 supervisor 시계를 서로 비교한다. 같은 supervisor의 보고끼리만 의미가 있다. 시계가 뒤로 가면 새 관측이 낡은 것으로 표시된다.
- 영속 PoC DB에는 새 표 두 개가 다음 기동의 `migrate`로 생긴다. 이 단계에서는 적용하지 않았다.
