# ieo 내부 API v0 · gRPC 계약 설명 (X-03 계약, X-04 구현 반영)

| 항목 | 기록 |
| --- | --- |
| 작업 | X-03 gRPC 내부 API 계약과 분리 영향 확인(개정 A4, 장부 §7 수용 기준) |
| 정의 파일 | [`proto/ieo/internal/v0/supervisor.proto`](proto/ieo/internal/v0/supervisor.proto) · lint·호환 규칙 [`proto/buf.yaml`](proto/buf.yaml). 이 문서도 계약의 일부다 |
| 근거 | [gRPC 내부 API 계획 v0](../docs/poc/Plan_gRPC_Internal_API_v0.md) §1·§3·§4, [사용자 결정 2026-11-14 · gRPC](../docs/poc/User_Decisions_2026-11-14_grpc.md), HTTP 계약 [`ieo-api-v0.openapi.yaml`](ieo-api-v0.openapi.yaml) 0.8.0, 코드 main 3157c89(작성)·0779850(검토 기준, crates·contracts 차이 없음) |
| 상태 | X-03 계약에 X-04 구현의 확인 결과를 반영했다(바뀐 문장은 §12). 구현: `crates/ieo-proto`(생성 코드·상태 대응), `crates/ieo-coord/src/grpc.rs`(어댑터), `crates/ieo-supervisor/src/central.rs`(클라이언트). 분산·복구 시험은 X-05·X-06. 이 문서는 업무 규칙을 새로 정하지 않는다. 업무 규칙은 같은 이름의 HTTP 작업과 `ieo_coord::api`의 처리다 |

## 1. 범위

### 1.1 gRPC 표면(v0)

supervisor(역할 Pod의 실행 관리 서비스)가 코디네이터에 하는 호출만 둔다. 서비스는 `ieo.internal.v0.SupervisorService` 하나다.

| RPC | 같은 HTTP 작업(operationId) | 자격·범위 | 상태 변경 |
| --- | --- | --- | --- |
| `SubmitAgentReport` | `POST /v0/agent/reports` (submitAgentReport) | agent, `agent:report`, (repo, role)에 묶임 | 있음(claim 저장) |
| `SubmitReproRequest` | `POST /v0/agent/repro-requests` (submitReproRequest) | agent, `agent:repro`, guardrail만 | 있음(재현 대기열) |
| `GetReproRequest` | `GET /v0/agent/repro-requests/{request_id}` (getReproRequest) | 그 요청 repo의 guardrail 자격 | 없음 |
| `ReportCheckpoint` | `POST /v0/repos/{repo_id}/roles/{role}/checkpoints` (reportCheckpoint) | agent, `checkpoint:report`, (repo, role)에 묶임 | 있음(체크포인트 행) |
| `GetLatestCheckpoint` | `GET …/checkpoints/latest` (latestCheckpoint) | 보고한 supervisor 자신(`checkpoint:report`) 또는 repo의 `roles:control` | 없음 |

헬스 확인은 표준 `grpc.health.v1.Health`다(§7.3).

### 1.2 넣지 않는 작업

- **스프린트 상태·사건 조회**(`GET /v0/sprints/{id}/state`, `…/events`). 계획 §1이 미결로 둔 질문이다: supervisor가 이 조회를 쓸 수 있는가. **v0에서는 넣지 않는다.** 에이전트 자격의 `/v0/sprints*` 금지(403 `AGENT_READ_DENIED`, 역할은 서로를 보지 않는다)를 그대로 둔다. supervisor가 필요한 자기 배정 정보는 02-05가 만들 자기 배정 조회로 얻는다(§8).
- **`compareCheckpoint`**(`GET …/checkpoints/compare`). `roles:control` 클라이언트(코디네이터 쪽 운영 주체)의 작업이다. supervisor는 `GetLatestCheckpoint`로 중앙 기록을 읽고 스스로 대조한다(02-04 `central::compare`).
- 사람·등록자·배정자용 작업 전부: 명세 등록·변환, 의미 검토, Notion 가져오기, 예외 승인, 배정, 증거 입력, 판정, outbox, repo 역할 상태·정지. HTTP/JSON에만 둔다.

### 1.3 호출 방향

v0은 supervisor → 코디네이터 방향만 정한다. 코디네이터 → supervisor 호출(역할 상태·정지, 02-05의 start·reconnect)은 역할 서비스의 HTTP(`/v0/role/*`)로 둔다. 계획 §1의 대상이 “supervisor가 코디네이터에 하는 호출”이기 때문이다. 이 방향의 gRPC 여부는 X-03 범위가 아니다(§9).

### 1.4 HTTP 형태와의 관계

- 기존 HTTP/JSON 계약과 시험은 그대로다. gRPC는 경로를 **추가**한다.
- 표면의 다섯 작업은 HTTP 형태가 이미 있다. 둘은 같은 작업 표(§6.2)로 같은 처리에 보낸다.
- 02-05가 새로 만들 작업도 작업 표에 한 번만 정의하고, **HTTP와 gRPC 형태를 함께 둔다**(계획 §1의 기본값을 X-03에서 확정). HTTP 시험 체계로 규칙을 검증할 수 있게 하기 위해서다. HTTP 형태는 02-05가 만든다. gRPC 형태는 X-04가 먼저 병합됐으면 02-05가, 아니면 X-04가 더한다(§8.2). 02-05의 선행·수용 기준은 바뀌지 않는다.

## 2. 메시지와 버전 호환

- **패키지**: `ieo.internal.v0`. HTTP API v0과 같은 주 버전이다. buf의 `PACKAGE_VERSION_SUFFIX` 규칙은 v1부터만 받으므로 이 한 규칙만 예외로 둔다(`buf.yaml`). 나머지는 buf `STANDARD` lint를 통과한다.
- **필드 번호를 다시 쓰지 않는다.** 필드를 지우면 번호와 이름을 `reserved`로 막는다. enum 값도 같다. enum의 0은 `…_UNSPECIFIED`이고 업무 값으로 쓰지 않는다.
- **v0 안에서는 추가만 한다.** 새 필드는 없을 때의 의미가 옛 동작과 같아야 한다. 옛 클라이언트나 옛 서버가 잘못 읽을 변경(의미 변경, 필수화, 타입 변경, 번호 변경)은 새 패키지 `ieo.internal.v1`로 한다. 호환 검사는 `buf breaking`의 `WIRE_JSON` 규칙이다. X-04가 CI에서 main 대비로 돌린다.
- **JSON 문서는 바이트로 싣는다.** `report_json`(ProgressReport v0)과 `checkpoint_json`(체크포인트 파일)은 각자의 JSON 스키마·규칙이 정본이다. proto로 다시 정의하지 않는다. 두 전송이 같은 문서를 같은 규칙으로 검사한다.
  - 보고의 중복 판정은 파싱한 JSON 값을 비교한다. HTTP로 보낸 보고와 gRPC로 보낸 같은 보고는 같은 보고다.
  - 체크포인트의 SHA-256은 받은 바이트로 계산한다. 같은 파일은 어느 전송으로 보내도 digest가 같다.
- **작은 요청은 타입이 있는 필드로 싣는다**(`SubmitReproRequestRequest`). 어댑터는 HTTP 본문과 **같은 JSON 값**을 만든다. 재현 요청의 중복 판정도 파싱한 JSON 값을 비교하므로, 이 규칙이 깨지면 같은 요청이 `EVENT_CONTENT_CONFLICT`가 된다. X-04 공통 회귀가 확인한다.
  - `optional`이 아닌 필드는 기본값(빈 문자열·0·빈 목록)이어도 **늘 JSON에 넣는다**. 설정하지 않은 `optional` 필드는 JSON에서 빠진다.
  - gRPC로는 명시적 `null`을 보낼 수 없다. HTTP로 `"timeout_seconds": null`을 넣어 저장한 요청을 gRPC로 다시 보내면 다른 JSON 값이 되어 `EVENT_CONTENT_CONFLICT`다. 그 요청의 결과 확인은 `GetReproRequest`로 한다(§5).
  - `report_json`은 HTTP 본문 그 자체이므로 바이트를 **바꾸지 않고** 처리에 넘긴다. UTF-8이 아니어도 HTTP와 같이 처리가 판단한다(401 → 403 → 400 `SCHEMA_VIOLATION`, field `""`).
  - `checkpoint_json`은 HTTP에서 JSON 문자열 안에 들어가므로 UTF-8이 아닌 바이트를 HTTP로는 보낼 수 없다. 어댑터는 인증 뒤(§7.0의 순서) `INVALID_ARGUMENT`(trailer: `http_status` 400, `SCHEMA_VIOLATION`, field `checkpoint_json`)로 거부한다. HTTP에 대응하는 입력이 없는 gRPC만의 거부다.
- **알 수 없는 필드.** proto3는 모르는 필드를 무시한다. HTTP는 정의되지 않은 키를 거부한다(`additionalProperties: false`, `SCHEMA_VIOLATION`). 그래서 v0의 새 필드는 무시돼도 안전한 것만 더한다(위 “추가만”). JSON 문서 안의 키는 바이트째 전달되므로 HTTP와 똑같이 거부된다.
- **응답의 결과 구분.** HTTP는 새 저장(201·202)과 재조회(200)를 상태 코드로 나눈다. gRPC는 `Outcome`(`OUTCOME_STORED`·`OUTCOME_REPLAYED`)으로 나눈다.

## 3. 인증과 역할별 권한

- **자격 전달**: metadata `authorization: Bearer <token>`. HTTP의 `Authorization` 헤더와 같은 값이다.
- **해석**: HTTP와 같은 `auth::Credentials`(토큰 digest → 주체·종류·범위·repo·role)를 쓴다. gRPC용 별도 자격·범위를 만들지 않는다. 메시지 안의 이름(repo_id, role, 보고의 actor)은 권한에 쓰지 않는다.
- **supervisor 주체**: 종류 `agent`, 하나의 (repo, role)에 묶인 자격이다. 범위는 지금 있는 에이전트 범위 셋만 쓴다: `agent:report`, `agent:repro`(guardrail 전용), `checkpoint:report`. 새 범위는 만들지 않는다.
  - PR #41 검토 b3cb083의 질문(재현 요청을 supervisor가 재사용하는가)의 답: 재사용한다. guardrail 역할의 supervisor만 `agent:repro`로 부른다. development 자격은 HTTP와 같이 거부된다.
  - 어느 프로세스가 어느 토큰을 쥐는가(모델 프로세스와 supervisor의 분리)는 02-05의 배정·인수 연결에서 정한다. 이 계약은 “HTTP가 받는 자격만 gRPC도 받는다”만 정한다.
- **검사 순서**: 작업마다 처리 코드가 정하고, gRPC는 같은 처리를 부르므로 순서가 같다. OpenAPI 머리말의 일반 순서(401 → 403/409 → 400 → 409 → 422)와 다른 작업이 있다(main 0779850):
  - `SubmitAgentReport`: 401 → 범위·역할(403) → 본문 파싱(400) → 저장된 같은 키의 재조회(200)·내용 충돌(409 `EVENT_CONTENT_CONFLICT`)·배정 확인(409) → 형식(400)·의미(422). 재조회와 식별 검사가 형식 검사보다 먼저다.
  - `SubmitReproRequest`: 401 → 범위·역할(403) → 본문 파싱(400) → 재조회(200)·내용 충돌·현재 배정(409) → 새 요청만 형식(400 `MISSING_FIELD`·`SCHEMA_VIOLATION`).
  - `ReportCheckpoint`: 401 → 역할 이름(404) → 범위·repo·역할(403) → 본문 형식(`seq`·`checkpoint_json`, JSON 파싱, `format`이 `ieo-checkpoint/v0`인지, `seq`가 1 이상이고 체크포인트 자신의 `seq`와 같은지, 모두 400) → 체크포인트의 repo·역할 불일치(409 `REPO_MISMATCH`) → 나머지 체크포인트 필드 형식(400) → 잠금 → 순번·내용 식별(200·409 `EVENT_CONTENT_CONFLICT`·`CHECKPOINT_SEQUENCE_GAP`) → 배정 대조(409) → 저장 → `config_tamper_detected` 형식(400, 롤백). 그래서 repo가 다르고 `phase`가 빠진 체크포인트는 400이 아니라 409이고, repo가 다르고 `format`이나 `seq`가 틀린 체크포인트는 400이다.
  - `GetReproRequest`·`GetLatestCheckpoint`: 401 → (`GetLatestCheckpoint`만 역할 이름 404) → 권한(403) → 없음(404).
  - **HTTP 전송 쪽**의 본문 크기 검사(400)는 인증보다 먼저다. gRPC 어댑터도 같은 크기 검사를 인증 전에 한다(§7.1).
- **역할 값과 경로 매개변수**: `ROLE_UNSPECIFIED`와 정의되지 않은 숫자 값은 알 수 없는 역할이다. HTTP의 알 수 없는 역할과 같이 처리가 `NOT_FOUND`(`NOT_FOUND`)로 답한다(인증 뒤). 경로 조각이 되는 값(`repo_id`, `request_id`)은 이렇게 다룬다(X-03 검토 8e3e817 P-1).
  - 빈 `repo_id`는 안쪽 경로 조각이다. HTTP도 빈 안쪽 조각을 같은 작업으로 읽으므로 어댑터는 그대로 처리에 넘기고, 같은 답(예: 403 `REPO_SCOPE_DENIED`)이 나온다.
  - 빈 `request_id`는 마지막 조각이다. HTTP는 끝의 빈 조각을 지우므로 어느 작업에도 맞지 않는다(인증 뒤 404 “no such endpoint”). 어댑터도 인증 뒤 처리에 넘기지 않고 같은 `NOT_FOUND`(trailer: `http_status` 404, `code` `NOT_FOUND`)로 답한다.
  - `/`·`?`·`#`를 담은 값도 인증 뒤 처리에 넘기지 않고 같은 `NOT_FOUND`로 답한다. HTTP에서 `/`는 다른 경로가 되고, `?`는 경로를 잘라(`http.rs`가 첫 `?`에서 나눔) 잘린 값으로 조회하거나 다른 작업에 맞는다. `#`는 HTTP 클라이언트가 보내지 않는 조각 구분자다. 이 경우들을 어댑터는 따라 하지 않는다. 그래서 이 문자를 담은 `request_id`로 저장된 재현 요청은 어느 전송으로도 `GetReproRequest`로 읽을 수 없다(정상 식별자는 이 문자를 담지 않는다).
  - 그 밖의 문자는 바꾸지 않고 넘긴다(HTTP 쪽도 퍼센트 인코딩을 풀지 않는다).

## 4. 상태 코드 대응과 상세 사유

### 4.1 규칙

gRPC 상태는 **같은 요청이 HTTP에서 받을 상태 코드와 `code`**로 정한다. 처리 결과는 하나이고 어댑터가 그것을 옮길 뿐이다. 같은 `code`라도 작업에 따라 HTTP 상태가 다르다(예: `DIGEST_MISMATCH`는 등록에서 422, 체크포인트에서 409). 그래서 `code`만 보지 않는다.

| HTTP | gRPC | 비고 |
| --- | --- | --- |
| 200·201·202 | `OK` | 200은 `OUTCOME_REPLAYED`, 201·202는 `OUTCOME_STORED` |
| 400 | `INVALID_ARGUMENT` | |
| 401 | `UNAUTHENTICATED` | |
| 403 | `PERMISSION_DENIED` | |
| 404 | `NOT_FOUND` | `NO_CHECKPOINT` 포함 |
| 409, 모든 `code`가 `EVENT_CONTENT_CONFLICT`·`IDEMPOTENCY_KEY_CONFLICT` | `ALREADY_EXISTS` | 같은 키에 다른 내용 |
| 409, 모든 `code`가 `OWNERSHIP_CONFLICT`·`OUTBOX_STATE_CONFLICT` | `ABORTED` | 동시성·CAS. 다시 읽고 판단한다(gRPC 표면에서는 나오지 않음) |
| 그 밖의 409 | `FAILED_PRECONDITION` | 배정·세대·순번·SHA·digest·repo 불일치 등 |
| 422 | `FAILED_PRECONDITION` | 참조·결합 |
| 500 | `INTERNAL` | **결과 불명.** 처리 중 실패는 롤백되지만, 커밋 실패도 500이고 그때 서버는 이미 커밋했을 수 있다. 그래서 500은 거부 확정이 아니다(§5) |
| 500, 작업 전 DB 연결 실패 | `UNAVAILABLE` | 아무것도 실행하지 않았다(§7.2) |
| 503 | `UNAVAILABLE` | 지금은 나오지 않는다 |

409의 `rejections[]`에 여러 `code`가 섞이면 위 두 묶음 중 하나에 모두 들지 않는 한 `FAILED_PRECONDITION`이다.

### 4.2 표면 작업에서 나올 수 있는 `code`

| RPC | `code`(HTTP 상태) |
| --- | --- |
| `SubmitAgentReport` | `UNAUTHENTICATED`(401) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`·`VERIFIED_FROM_AGENT_DENIED`(403) · `MISSING_FIELD`·`SCHEMA_VIOLATION`(400) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `STALE_ASSIGNMENT`·`ASSIGNMENT_SHA_MISMATCH`(409) · `REPORT_SEMANTIC_VIOLATION`(422) · `INTERNAL`(500) |
| `SubmitReproRequest` | `UNAUTHENTICATED` · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `STALE_ASSIGNMENT`(409, 현재가 아닌 배정·세대 또는 그 배정의 SHA와 다른 `target_sha`) · `MISSING_FIELD`·`SCHEMA_VIOLATION`(400, 본문 파싱 실패는 처음에, 필드 형식은 권한·식별 검사를 통과한 새 요청에만) · `INTERNAL` |
| `GetReproRequest` | `UNAUTHENTICATED` · `REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `NOT_FOUND`(404, 권한 검사 뒤) · `INTERNAL` |
| `ReportCheckpoint` | `UNAUTHENTICATED` · `NOT_FOUND`(404, 알 수 없는 역할) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `MISSING_FIELD`·`SCHEMA_VIOLATION`(400, `checkpoint_json` 1 MiB 초과는 `MISSING_FIELD`) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `CHECKPOINT_SEQUENCE_GAP`·`STALE_ASSIGNMENT`·`CHECKPOINT_MISMATCH`·`NO_CURRENT_ASSIGNMENT`·`DIGEST_MISMATCH`·`ASSIGNMENT_SHA_MISMATCH`·`REPO_MISMATCH`(409) · `INTERNAL` |
| `GetLatestCheckpoint` | `UNAUTHENTICATED` · `NOT_FOUND`(알 수 없는 역할) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `NO_CHECKPOINT`(404) · `INTERNAL` |

이 표는 main 3157c89의 처리 코드를 읽어 만들었고 0779850에서 검토했다. 모든 작업에 어댑터의 본문 크기 거부(400 `SCHEMA_VIOLATION`, §7.1)가 더해지고, `GetReproRequest`·`ReportCheckpoint`·`GetLatestCheckpoint`에는 경로 값 거부(404 `NOT_FOUND`, §3), `ReportCheckpoint`에는 UTF-8 거부(400 `SCHEMA_VIOLATION`, §2)가 더해진다. DB 연결 실패는 모든 작업에서 `UNAVAILABLE`이다(§7.2). 정본은 처리 코드이고, X-04 공통 회귀가 두 전송의 상태·`code`가 같은지 확인한다.

### 4.3 상세 사유: `ieo-error-bin`

- **처리(`handle`)가 만든 비정상 응답**과 어댑터가 처리 대신 답한 DB 연결 실패·크기·경로·UTF-8 거부(§7.2·§7.1·§3·§2)는 바이너리 trailer `ieo-error-bin`에 `ieo.internal.v0.Error`를 싣는다: HTTP 상태(`http_status`), `Problem.code`·`message`, 또는 `rejections[]`(code·field·message·correction·retryable_after_edit)를 바꾸지 않고.
- gRPC 상태 메시지(`grpc-message`)에는 사람이 읽을 요약만 둔다. 클라이언트의 판단은 trailer로 한다.
- `google.rpc.Status`(`grpc-status-details-bin`)는 쓰지 않는다. 그 형식의 의존을 들이지 않고 기존 오류 모양을 그대로 옮기기 위해서다.
- **trailer가 없는 비정상 상태**: `DEADLINE_EXCEEDED`·`CANCELLED`(서버가 만든 것도 trailer를 싣지 않는다, §6), 연결 끊김, 라이브러리가 만든 `UNAVAILABLE`, 메시지 크기 상한 초과(tonic 0.14는 `OUT_OF_RANGE`, 다른 구현은 `RESOURCE_EXHAUSTED`, §7.1) 등. `UNAVAILABLE` 중에는 DB 연결 실패(§7.2)만 trailer를 싣는다.
- 클라이언트는 trailer 없는 상태(위 목록 전부, 크기 상한 초과 포함)와 `INTERNAL`을 결과 **알 수 없음**으로 다룬다(§5). 거부 확정은 trailer의 `http_status`가 400~499일 때와 클라이언트 자신의 사전 계산 거부(§7.1)뿐이다.

## 5. 재시도와 멱등

원칙(사용자 결정 2026-11-14 §5): **연결 실패만으로 재실행하지 않는다.** 결과를 알 수 없으면 같은 키로 서버의 실제 반영 여부를 먼저 확인한다. 상태를 바꾸는 작업은 키 없이 재시도하지 않는다.

**결과를 알 수 없는 경우**: `INTERNAL`(trailer가 있어도, HTTP 500), `UNAVAILABLE`(trailer가 있어도, DB 연결 실패의 HTTP 500), `DEADLINE_EXCEEDED`, `CANCELLED`, `UNKNOWN`, trailer 없는 `RESOURCE_EXHAUSTED`·`OUT_OF_RANGE`(크기 상한, §7.1), 연결 끊김. 02-04의 “5xx는 결과 불명” 규칙과 같다. 이때는 같은 키로 확인한다.

| RPC | 멱등 키 | 결과를 알 수 없을 때 | 하지 않는 것 |
| --- | --- | --- | --- |
| `SubmitAgentReport` | (event_id, event_revision), 자격의 (repo, role) 안 | 결과 불명을 호출자에게 돌려준다. 다시 보내는 호출자 규칙은 지금 없다. 정해지기 전에는 다시 보내지 않는다(그 규칙을 정하는 것은 이 계약의 범위가 아니며, 어느 작업의 수용 기준도 바꾸지 않는다). 다시 보낼 때는 **같은 바이트**만: 저장돼 있으면 `OUTCOME_REPLAYED`와 저장된 영수증이고 상태 변화가 없다 | 내용을 바꿔 같은 키로 보내기(→ `ALREADY_EXISTS`). 새 내용은 새 revision |
| `SubmitReproRequest` | request_id | `GetReproRequest`로 확인한다(자동 확인 가능). 없으면 결과 불명을 호출자에게 돌려준다. 다시 보낼 때는 같은 요청만: 저장된 요청은 다시 큐에 넣지 않는다 | 새 request_id로 다시 보내기(새 실행이 된다) |
| `GetReproRequest` | — | 다시 부른다 | — |
| `ReportCheckpoint` | (repo, role, seq) + 바이트 | **`GetLatestCheckpoint`로 확인**(자동 확인 가능). 최신 seq·SHA가 보낸 것과 같으면 반영됨. 아니면 결과 불명을 호출자에게 돌려준다. 다시 보낼지는 02-04 규칙(대기 사본 유지, 다음 기동의 채택 판단)이 정한다. 그 밖의 불일치는 외부 대조(`EXTERNAL_RECONCILIATION_REQUIRED`) | 같은 seq에 다른 바이트(→ `ALREADY_EXISTS`). 확인 없이 다음 seq로 넘어가기 |
| `GetLatestCheckpoint` | — | 다시 부른다 | — |

- **자동 재시도하지 않는 상태**: `INVALID_ARGUMENT`, `UNAUTHENTICATED`, `PERMISSION_DENIED`, `NOT_FOUND`, `ALREADY_EXISTS`, `FAILED_PRECONDITION`, `RESOURCE_EXHAUSTED`, `OUT_OF_RANGE`. 고치거나 다시 읽은 뒤의 새 판단으로만 다시 보낸다. `ABORTED`는 다시 읽고 판단한다.
- **자동 재전송을 더하지 않는다**(사용자 결정 2026-11-14 §7 “자동 재시도를 추가하지 않는다”). gRPC 클라이언트는 상태를 바꾸는 RPC를 스스로 다시 보내지 않는다. 지금 02-04 HTTP 클라이언트도 자동 재전송을 하지 않는다.
- **자동으로 하는 것은 읽기 전용 확인뿐이다**(`GetLatestCheckpoint`, `GetReproRequest`): 상태를 바꾸지 않으므로 다시 불러도 된다. **X-04 확정: 자동 재조회는 0회다.** 확인 읽기는 지금 코드가 확인하는 곳(기동 때 대기 사본 채택 판단과 중앙 기록 대조)에서 한 번씩 한다. 답이 없으면 결과 불명(`NOT_OBSERVED`)으로 멈추고, 다음 기동이 다시 확인한다. HTTP 클라이언트와 같은 동작이고, 사용자 결정 2026-11-14 §7(자동 재시도를 추가하지 않는다)을 따른다. 지금 supervisor는 재현 요청 RPC를 부르지 않는다.
- **02-04 supervisor 규칙과의 대응**(`central::report`): 지금 HTTP 4xx는 “거부 확정”(대기 사본 삭제, 시작하지 않음), 5xx·전송 오류는 “결과 불명”(대기 사본 유지, 다음 기동에 채택 판단)이다. gRPC 클라이언트는 **trailer의 `http_status`**로 같은 구분을 한다: 400~499면 거부 확정, 그 밖(500, trailer 없음)이면 결과 불명. gRPC 상태 이름만으로 나누지 않는다. 클라이언트의 사전 계산 거부(§7.1)는 HTTP 400과 같이 거부 확정이다(대기 사본 삭제).
- **보내는 바이트**(X-03 검토 8e3e817 I-8): HTTP 클라이언트(`central::report`)는 체크포인트를 `String::from_utf8_lossy`로 JSON 문자열에 넣고, gRPC 클라이언트는 원 바이트를 그대로 보낸다. UTF-8이 아닌 체크포인트는 HTTP로는 손실 변환된 바이트(와 그 digest)로 저장되고, gRPC로는 400(`checkpoint_json`)으로 거부된다. supervisor는 체크포인트를 serde_json으로 쓰므로 지금은 생기지 않는다.
- 이 RPC들은 모델을 부르지 않는다. 재조회·재전송은 모델 호출의 재시도가 아니다. 모델 호출의 자동 재시도 금지는 그대로다.

## 6. 제한 시간과 취소

### 6.1 규칙

- **클라이언트**는 모든 호출에 deadline을 둔다. supervisor 기본값은 10초다(02-04 HTTP 클라이언트의 시간 제한과 같다).
- **서버 최대 처리 시간**은 15초다. 실제 처리 시간은 min(클라이언트 deadline(`grpc-timeout`), 15초)이다. 지금 표면 작업은 트랜잭션 하나로 끝나며 재현 요청도 대기열에 넣기만 한다. X-04 측정(공통 회귀의 94개 gRPC 호출, 디버그 빌드, 같은 호스트, 호출마다 새 연결, 1 MiB 본문 포함): 중앙값 4ms, 95번째 백분위 42ms, 최대 107ms. 15초를 유지한다.
- **시작 전**: 남은 시간이 50ms보다 짧으면 트랜잭션을 열지 않고 `DEADLINE_EXCEEDED`로 답한다(trailer 없음). 아무것도 실행하지 않았다. 이 확인은 어댑터 자체 검사(§7.0) 뒤, 트랜잭션을 열기 직전에 한다.
- **작업 기한**: 처리 쪽의 기한은 deadline보다 50ms 앞선 시각이다. tonic의 서버 시간 제한 계층(`grpc-timeout`과 서버 상한 중 짧은 쪽)은 deadline에 처리 future를 버리고 trailer 없는 `CANCELLED`(“Timeout expired”)로 답한다. 서버의 `DEADLINE_EXCEEDED`가 그보다 먼저 나가도록 50ms를 남긴다. 그 계층이 먼저 버린 호출도 아래 취소처럼 커밋 전에 롤백된다.
- **커밋 전 만료·취소는 롤백한다.** 처리는 트랜잭션을 열 때 그때 남은 작업 시간으로 `SET LOCAL statement_timeout`을 한 번 정한다. 어느 문장도 그보다 오래 돌지 않고, 문장들의 합이 기한을 넘으면 커밋 직전 확인이 잡는다. 커밋 직전에 작업 기한과 취소를 다시 본다. 지났으면 롤백하고 `DEADLINE_EXCEEDED`·`CANCELLED`다. 문장 시간 제한에 걸린 실패(PostgreSQL `57014 query_canceled`)도 트랜잭션 실행기가 500 `INTERNAL`이 아니라 `DEADLINE_EXCEEDED`로 바꾼다. 서버가 만든 `DEADLINE_EXCEEDED`·`CANCELLED`는 trailer를 싣지 않는다. 롤백했더라도 클라이언트는 결과 불명으로 다룬다(§4.3·§5).
- **커밋 뒤 끊김은 반영된 상태다.** 커밋과 응답 사이에 클라이언트가 떠났으면 상태는 바뀌었고 답만 잃었다. 클라이언트는 §5대로 같은 키로 재조회해 결과를 얻는다.
- 커밋 직전 확인과 커밋 사이의 짧은 틈에 취소가 오면 커밋될 수 있다. 이 경우도 위 “커밋 뒤 끊김”과 같이 재조회로 해결한다. 클라이언트는 `CANCELLED`·`DEADLINE_EXCEEDED`를 “실행되지 않음”으로 해석하지 않는다.

### 6.2 동기 처리와의 연결

- tonic(비동기)에서 동기 처리(`postgres` 동기 클라이언트)를 연결을 쥔 작업자 풀(스레드마다 DB 연결 하나, HTTP 작업자와 같은 방식)에서 부른다. 비동기 요청이 취소돼도 이미 시작한 동기 처리는 끝까지 돈다. 그래서 위 커밋 전 확인을 처리 쪽에 둔다(취소 표지와 deadline을 처리에 넘긴다).
- **취소 감지**(X-04 확인, tonic 0.14.9·hyper 1.x): 클라이언트가 호출을 버리면(RST_STREAM) hyper가 처리 future를 버린다. 어댑터는 future가 버려질 때 취소 표지를 세우고, 작업자는 트랜잭션 시작 전과 커밋 직전에 그 표지를 본다. 문장 하나가 도는 동안의 취소는 그 문장이 끝난 뒤 커밋 직전에 잡힌다.
- 처리 함수 시그니처에 “커밋 전 확인”을 넣는 것은 동작 변경이 아니다. HTTP 경로는 deadline이 없으므로 확인이 늘 통과한다.

## 7. 전송 쪽 동작

HTTP 어댑터(`http.rs`)와 `handle`이 전송 쪽에서 하는 일을 gRPC에서 이렇게 한다.

### 7.0 어댑터 자체 검사의 순서

HTTP 전송 쪽과 같은 순서로 한다: DB 연결 확인(실패면 `UNAVAILABLE`, §7.2) → 본문 크기(400, §7.1) → 인증(401) → 어댑터의 경로 값 거부(404, §3) → `checkpoint_json` UTF-8 거부(400, §2) → 처리(트랜잭션 전 남은 시간 확인, §6.1). 처리는 HTTP와 같은 작업 표의 같은 함수(`api::dispatch`)다.

### 7.1 크기 상한

HTTP의 크기 규칙은 두 겹이다(main 0779850).
- **전송 쪽**(`http.rs`): 본문 전체가 1 MiB를 넘으면 인증 전에 400 `SCHEMA_VIOLATION`(field `""`)이다. 체크포인트는 JSON 문자열로 이스케이프된 채 이 본문 안에 들어가므로, 받을 수 있는 체크포인트 파일은 1 MiB보다 작다.
- **처리 쪽**: `checkpoint_json` 문자열이 1 MiB를 넘으면 400 `MISSING_FIELD`(field `checkpoint_json`)다. `submit_report`에는 처리 쪽 크기 검사가 없다. 전송 쪽 검사만 있다.

gRPC는 같은 입력에 같은 답을 내도록 이렇게 한다.
- 어댑터는 처리에 넘길 JSON 본문(HTTP 본문과 같은 값, §2)을 만든 뒤 **같은 1 MiB 상한을 인증 전에** 적용한다. 크기는 compact serde_json 직렬화(`serde_json::to_vec`, 02-04 HTTP 클라이언트의 `ureq::send_json`과 같은 바이트)로 잰다(X-03 검토 8e3e817 I-8). `report_json`은 받은 바이트 그대로 잰다. `checkpoint_json`이 UTF-8이 아니면 손실 변환한 문자열로 재고, 인증 뒤 400으로 거부한다(§2). 넘으면 `INVALID_ARGUMENT`(trailer: `http_status` 400, `SCHEMA_VIOLATION`, field `""`)다. 그래서 같은 체크포인트 파일이 HTTP에서 400이면 gRPC에서도 400이다. `report_json`은 처리 쪽 검사가 없으므로 이 어댑터 검사가 유일한 상한이다.
- 그 밖의 크기 검사(`checkpoint_json`의 `MISSING_FIELD`)는 처리가 그대로 한다.
- **클라이언트는 보내기 전에 같은 크기를 계산한다**(같은 compact serde_json 직렬화): 같은 요청을 HTTP로 보낼 때의 JSON 본문이 1 MiB를 넘거나, 경로 값이 처리의 상한(`repo_id` 256바이트, `request_id` 128바이트)을 넘으면 보내지 않고 HTTP 400과 같이 **거부 확정**으로 기록한다(02-04 규칙에서 4xx와 같은 처리: 대기 사본 삭제). 본문 초과의 기록 내용은 HTTP 전송 쪽 400 본문과 같다. 그래서 두 클라이언트가 같은 체크포인트를 같은 상태로 다룬다.
- 전송 메시지 상한(1 MiB + 64 KiB)은 안전 상한이다. 위 사전 계산을 지킨 클라이언트의 메시지는 본문 1 MiB 이하, 경로 값 256바이트 이하, 나머지 필드 몇 바이트이므로 이 상한에 닿지 않는다. 그래도 닿으면 라이브러리가 trailer 없이 거부한다(tonic 0.14는 `OUT_OF_RANGE`, 다른 구현은 `RESOURCE_EXHAUSTED`). **규칙은 하나다**(X-03 검토 8e3e817 P-2): trailer가 없으므로 §4.3대로 결과 불명으로 다룬다. 02-04 대기 사본은 지우지 않는다. 서버는 처리를 시작하지 않았으므로 다음 기동의 `GetLatestCheckpoint`가 그 순번이 없음을 보고 대기 사본을 지운다(02-04 채택 판단). 클라이언트는 이 경우를 사전 계산을 어긴 결함으로도 기록한다.

### 7.2 DB 연결

- HTTP 작업자는 스레드마다 DB 연결 하나를 쥐고, 끊기면 다시 연다. 다시 열지 못하면 500(`INTERNAL`, “database unavailable”)이다.
- gRPC는 같은 방식의 연결을 쥔 작업자를 쓴다. 다시 열지 못하면 **`UNAVAILABLE`**(trailer: `http_status` 500, `code` `INTERNAL`, message “database unavailable”, HTTP 500 본문과 같은 값)이다. 인증보다 먼저다(§7.0). 처리를 시작하지 않았다. 끊긴 연결은 다음 호출에서 알게 되므로, 끊긴 직후 첫 호출은 처리 중 DB 오류(`INTERNAL`, HTTP 500)일 수 있다. HTTP도 같다. 다시 보낼지는 §5를 따른다(상태를 바꾸는 RPC의 자동 재전송 없음). 이 경우만 HTTP 500과 gRPC 상태 대응이 다르다(§4.1).

### 7.3 헬스

- HTTP `/v0/health`는 인증 없이 트랜잭션 밖에서 `schema_meta`를 읽어 답한다(생존 확인, P04 결과 아님).
- gRPC는 표준 `grpc.health.v1.Health/Check`를 쓴다. 서비스 이름 `""`과 `ieo.internal.v0.SupervisorService`. 자격 없이 답한다. 구현(`tonic-health` 0.14.9)은 호출마다 읽지 않고 서버가 기록한 상태를 돌려준다. 그래서 서버가 같은 읽기(`schema_meta`, HTTP `/v0/health`와 같은 처리)를 작업자 풀에서 주기적으로(5초, X-04 확정) 하고, 성공하면 `SERVING`, 실패하면 `NOT_SERVING`으로 기록한다. 첫 읽기는 서비스를 열기 전에 한다. 상태는 최대 그 주기만큼 늦다. 작업자가 모두 바쁘면 읽기가 늦어질 수 있고, 주기 안에 답이 없으면 `NOT_SERVING`이다. 등록하지 않은 서비스 이름은 `NOT_FOUND`다. 스키마 버전과 정책 digest는 HTTP `/v0/health`에서 읽는다.
- 표준 헬스 서비스 구현은 X-04에서 `tonic-health` 0.14.9로 확인했다. 이 패키지에 헬스 RPC를 더하지 않는다.

### 7.4 전송 보안

PoC의 같은 호스트·kind 시험은 평문 HTTP/2를 쓴다. 평문 위의 Bearer 토큰은 시험 환경에서만 허용한다. 실제 다중 서버 배치의 TLS·상호 인증은 정식 설계의 몫이다(§9).

## 8. 긴 스트림과 02-05의 새 작업

### 8.1 스트림

v0에는 스트림 RPC가 없다. 지금 supervisor가 구독할 사건이 없고, 사건 조회는 HTTP의 페이지 단위 조회다. 02-05·P28(사건 기반 즉시 인수, 개정 A5)이 배정 통지 스트림을 필요로 하면 다음 규칙으로 더한다.

- 서버 스트리밍만 쓴다. 스트림의 각 항목에 순번을 붙인다.
- 끊기면 클라이언트가 마지막으로 받은 순번 다음부터 다시 연다. 서버는 그 순번부터 다시 보낸다(중복은 클라이언트가 순번으로 거른다).
- 열린 스트림이 다른 Pod로 옮겨 간다고 가정하지 않는다. 새로 열 때 새 Pod에 붙을 수 있다.
- 스트림이 빠뜨린 통지는 단항 조회(정기 대조, A5의 보조 경로)로 복구한다. 스트림은 지연을 줄이는 수단이고 유일한 경로가 아니다.

### 8.2 02-05의 새 작업

계획 §1이 02-05 범위로 둔 작업은 자기 배정 조회·인수, 세션 상태 보고다. 지금은 HTTP에도 없다. 02-05가 이 작업을 만들 때:

- 업무 규칙과 HTTP 형태를 먼저 정하고, 작업 표에 한 번 넣는다.
- **X-04가 먼저 병합됐으면** `SupervisorService`에 RPC를 **추가**한다(새 필드 번호, v0 안). 아니면 02-05는 HTTP 형태만 만들고, RPC 추가는 X-04가 맡는다. 상태를 바꾸는 작업은 요청에 `idempotency_key`(HTTP `Idempotency-Key`와 같은 규칙, 8~128자)를 두거나 (assignment_id, generation)을 키로 쓴다. 재시도 표(§5)와 `code` 표(§4.2)에 행을 더한다.
- X-04가 준비되지 않았으면 02-05는 HTTP로 진행한다. gRPC 전환은 X-04의 몫이다(02-05의 선행·수용 기준은 바뀌지 않음).

## 9. 분리 영향(계획 §3을 main 3157c89에서 다시 확인)

| 대상 | 확인한 사실 | X-04에서 할 일(동작 변경 없음) |
| --- | --- | --- |
| `api::handle` | 전송과 무관하다. `Req`(method·path·query·Idempotency-Key·본문)를 받아 `Resp`(u16 상태·JSON)를 돌려준다. 업무 규칙은 헤더·소켓을 보지 않는다. 요청마다 트랜잭션 하나이고 2xx가 아니면 롤백한다 | 경로 일치를 **작업 표**(`Operation` 열거와 경로 → 작업 대응)로 뽑는다. HTTP 어댑터는 경로로 작업을 찾고, gRPC 어댑터는 작업을 직접 고른다. 둘 다 같은 처리 함수를 부른다. 기존 HTTP 시험이 그대로 통과해야 한다 |
| 작업마다 다른 실행 모양 | 계획 이후 생긴 차이다. repo 역할 상태·정지는 DB 없이 처리한다. `compare`는 권한 확인 → 트랜잭션 밖의 supervisor 호출 → 트랜잭션 순서다. 헬스는 인증 없이 트랜잭션 밖이다 | 작업 표의 각 행에 실행 모양(트랜잭션 / 트랜잭션 밖 / 앞선 외부 호출)을 둔다. gRPC 표면의 다섯 작업은 모두 “트랜잭션 하나” 모양이다 |
| 상태 코드 | 내부 표현은 HTTP 숫자(u16)다 | 그대로 둔다. gRPC 어댑터에 §4.1 대응표만 둔다 |
| `http.rs` | 약 100줄. 1 MiB 상한, 작업자별 DB 재연결(실패 시 500), `Authorization`·`Idempotency-Key` 읽기 | gRPC 어댑터가 §7의 대응을 따로 구현한다 |
| 커밋 전 확인 | 지금 처리에는 deadline·취소 개념이 없다 | 트랜잭션 실행기(`handle` 안의 `run`)에 커밋 전 확인을 넣는다(§6.2). HTTP는 늘 통과 |
| 런타임 | 동기(tiny_http, postgres 동기, 작업자 스레드마다 연결 하나) | tonic·tokio를 더하고 동기 처리를 작업자 풀에서 부른다. **X-04 결정: 같은 `ieo-coord` 바이너리**의 선택 인수 `--grpc-listen ADDR [--grpc-workers N]`. 없으면 HTTP만 연다(지금과 같다). 같은 자격·정책·DB를 쓰고, 정지 신호에는 HTTP가 끝난 뒤 gRPC가 진행 중인 호출을 마치고 멈춘다. 계약에는 영향이 없다 |
| supervisor 클라이언트 | `ieo-supervisor/src/central.rs`가 `ureq`로 체크포인트 보고·최신 조회를 직접 부른다(시간 제한 10초). 4xx와 5xx·전송 오류로 대기 사본 처리를 나눈다 | 호출을 `CoordinatorClient` 같은 경계 뒤로 옮긴다(HTTP 구현 = 지금 코드). gRPC 구현은 §5의 `http_status` 구분을 쓴다. 02-04 시험(`tests/serve.rs`의 FakeCentral 시험)이 두 구현에 같이 돌아야 한다 |
| 빌드 | 새 의존성 tonic·prost·tonic-health·tokio. 디스크가 빠듯하다(약 90% 사용) | X-04: 생성 코드는 새 크레이트 `ieo-proto`가 빌드 때 만든다(`protox`로 .proto를 Rust에서 파싱, `tonic-prost-build`). CI와 개발 호스트에 protoc가 필요 없다. 상태 대응·trailer·JSON 변환도 이 크레이트에 한 벌만 두고 코디네이터와 supervisor가 같이 쓴다 |

02-04 영향: 이 계약은 02-04의 체크포인트 규칙(순번·바이트·digest·대기 사본·채택)을 바꾸지 않는다. 같은 규칙을 gRPC로 옮길 때 지킬 대응만 정했다(§5).

## 10. X-04가 확인할 것(공통 회귀로 넘김)

계약을 정할 때 가정한 것이다. X-04의 HTTP·gRPC 공통 회귀(장부 §7 X-04 수용 기준)에 넣는다.

1. 같은 요청에 두 전송이 같은 HTTP 상태·`code`·`rejections`와 같은 DB 상태를 낸다(§4.2 표의 모든 행).
2. 같은 보고·재현 요청·체크포인트를 한 전송으로 저장하고 다른 전송으로 다시 보내면 `REPLAYED`·200이다(§2의 JSON 값·바이트 동일성).
3. 커밋 전 deadline·취소는 롤백한다. 커밋 뒤 끊김은 재조회로 같은 결과를 얻는다. 전송 성공 여부와 서버 반영 여부가 다른 경우를 포함한다.
4. 에이전트 자격의 금지 범위가 같다(development의 재현 요청, 다른 repo·역할, 범위 밖 작업).
5. 크기 상한(§7.1)과 DB 연결 실패(§7.2)의 대응.
6. supervisor 클라이언트의 두 구현이 02-04 대기 사본 시험을 같이 통과한다.
7. `buf lint`와 `buf breaking`(main 대비)을 CI에서 돌린다. `buf breaking`은 이 proto가 main에 병합된 뒤부터 비교 대상이 생긴다(X-03 병합 0c243ee부터 있다).
8. 크기 상한이 두 전송에서 같은 답을 내는지(§7.1, 클라이언트 사전 계산 포함), 어댑터 자체 검사의 순서(§7.0), 경로 값·빈 값의 답(§3: 빈 `repo_id`, 빈 `request_id`, `/`·`?`·`#`), `ReportCheckpoint`의 검사 순서(§3: `format`·`seq` 400과 `REPO_MISMATCH`), 문장 시간 제한의 `DEADLINE_EXCEEDED` 변환(§6.1).

## 11. 남은 결정과 한계

- **토큰을 쥐는 프로세스**(모델 프로세스와 supervisor의 분리)는 02-05에서 정한다(§3).
- **코디네이터 → supervisor 방향의 gRPC**는 이 계약 범위가 아니다. 필요하면 별도 결정으로 다룬다(§1.3).
- **TLS·상호 인증**은 정식 설계의 몫이다. kind 시험의 평문은 시험 환경에서만 쓴다(§7.4).
- **전송 상한을 넘는 메시지**는 gRPC에서 trailer 없는 거부(tonic 0.14 `OUT_OF_RANGE`, 다른 구현 `RESOURCE_EXHAUSTED`)이고 결과 불명으로 다룬다. 같은 바이트의 HTTP 요청은 400일 수도 아닐 수도 있다(경로 값은 HTTP 본문에 들지 않는다). 사전 계산(§7.1)을 지킨 클라이언트는 닿지 않고, 그 범위에서 두 전송의 답은 같다.
- **오류 상세**는 `ieo-error-bin`이라는 자체 trailer다. 다른 생태계의 표준 클라이언트는 상태와 메시지만 본다(§4.3).
- **서버 최대 처리 시간 15초, 읽기 확인의 자동 재조회 0회, 헬스 갱신 주기 5초**는 X-04에서 확정했다(§5·§6·§7.3).
- **라이브러리 지원**은 X-04에서 tonic 0.14.9·tonic-health 0.14.9·hyper 1.x의 소스와 공통 회귀로 확인했다: `grpc-timeout` 전달(클라이언트 `Request::set_timeout`, 서버는 메타데이터로 읽고 tonic 계층도 같은 값으로 자른다), 취소 감지(RST_STREAM에 future 버림), 헬스, 메시지 상한(초과는 `OUT_OF_RANGE`). 버전을 올리면 공통 회귀가 다시 확인한다.

## 12. X-04에서 바꾼 문장

구현과 공통 회귀로 확인한 결과다. 업무 규칙은 바뀌지 않았다. proto의 필드 번호·타입은 바꾸지 않았다(주석만).

| 절 | 바뀐 내용 | 이유 |
| --- | --- | --- |
| §3 검사 순서 | `ReportCheckpoint`의 `REPO_MISMATCH` 앞 400에 `format`·`seq` 일치를 적었다 | 검토 8e3e817 I-7. 공통 회귀가 순서를 확인한다 |
| §3 경로 값 | 빈 `request_id`와 `?`·`#`를 담은 값도 인증 뒤 404. 빈 `repo_id`만 처리에 넘긴다 | 검토 8e3e817 P-1. 옛 문장(빈 값은 모두 처리에 넘김)은 마지막 조각인 `request_id`에서 HTTP와 달랐다 |
| §4.3 | trailer를 싣는 어댑터 답에 DB 연결 실패를 넣고, 크기 상한 초과를 trailer 없는 상태(결과 불명)로 적었다. 거부 확정에 클라이언트 사전 계산을 더했다 | 검토 8e3e817 I-10·P-2 |
| §5 | 결과 불명 목록에 trailer 있는 `UNAVAILABLE`(DB)과 `RESOURCE_EXHAUSTED`·`OUT_OF_RANGE`를 넣었다. 읽기 확인의 자동 재조회를 0회로 확정했다(초안: 최대 3회 가능). HTTP·gRPC 클라이언트가 보내는 바이트의 차이를 적었다 | P-2·I-8. 재조회 0회는 HTTP 클라이언트와 같은 동작이고 사용자 결정 §7을 따른다 |
| §6.1 | 처리 쪽 기한을 deadline − 50ms로 했다. `statement_timeout`은 트랜잭션 시작 때 한 번 정한다(옛 문장: 문장마다 남은 시간). 15초를 측정값과 함께 유지했다 | tonic의 서버 시간 제한 계층이 deadline에 `CANCELLED`로 답하므로 서버의 `DEADLINE_EXCEEDED`가 먼저 나가게 했다. 문장마다 다시 정하려면 모든 처리 함수를 고쳐야 하고, 합계 초과는 커밋 직전 확인이 잡는다 |
| §6.2 | `spawn_blocking` 대신 연결을 쥔 작업자 풀로 정했고, 취소 감지 방법을 적었다 | 구현 |
| §4.2 | 어댑터가 더하는 경로 값 404·UTF-8 400·DB `UNAVAILABLE`을 적었다 | P-1·I-10 |
| §7.0 | 트랜잭션 전 남은 시간 확인의 위치를 적었다 | 구현 |
| §7.1 | 크기를 compact serde_json으로 잰다. 사전 계산에 경로 값 길이(256·128바이트)를 넣었다. 전송 상한 초과는 결과 불명(대기 사본 유지, 다음 기동이 정리)으로 하나만 정했다. tonic 0.14는 `RESOURCE_EXHAUSTED`가 아니라 `OUT_OF_RANGE`로 답한다 | P-2·I-8. 옛 문장은 같은 상태를 “결과 불명이 아님”(§7.1)과 “결과 불명”(§4.3·proto)으로 달리 적었다 |
| §7.2 | trailer 내용과 인증보다 먼저임을 적었다. 끊긴 직후 첫 호출이 500일 수 있음을 적었다 | 구현(HTTP와 같다) |
| §7.3 | `tonic-health` 0.14.9로 확인, 5초 확정, 첫 읽기 시점 | 구현 |
| §9 | 같은 바이너리(`--grpc-listen`), 코드 생성(`protox`, protoc 없음) | X-04 설계 결정 |
| §10·§11 | 확인할 것과 한계를 확인 결과로 고쳤다 | 구현 |
| proto `Error` 주석 | trailer를 싣는 어댑터 답에 DB 연결 실패를 넣고, 크기 상한 초과를 trailer 없는 상태로 적었다 | I-10·P-2 |
