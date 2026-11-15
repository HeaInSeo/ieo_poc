# ieo 내부 API v0 · gRPC 계약 설명 (X-03 초안)

| 항목 | 기록 |
| --- | --- |
| 작업 | X-03 gRPC 내부 API 계약과 분리 영향 확인(개정 A4, 장부 §7 수용 기준) |
| 정의 파일 | [`proto/ieo/internal/v0/supervisor.proto`](proto/ieo/internal/v0/supervisor.proto) · lint·호환 규칙 [`proto/buf.yaml`](proto/buf.yaml). 이 문서도 계약의 일부다 |
| 근거 | [gRPC 내부 API 계획 v0](../docs/poc/Plan_gRPC_Internal_API_v0.md) §1·§3·§4, [사용자 결정 2026-11-14 · gRPC](../docs/poc/User_Decisions_2026-11-14_grpc.md), HTTP 계약 [`ieo-api-v0.openapi.yaml`](ieo-api-v0.openapi.yaml) 0.8.0, 코드 main 3157c89 |
| 상태 | 계약 초안. 구현은 X-04, 분산·복구 시험은 X-05·X-06. 이 문서는 업무 규칙을 새로 정하지 않는다. 업무 규칙은 같은 이름의 HTTP 작업과 `ieo_coord::api`의 처리다 |

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
- 02-05가 새로 만들 작업도 작업 표에 한 번만 정의하고, **HTTP와 gRPC 형태를 함께 둔다**(계획 §1의 기본값을 X-03에서 확정). HTTP 시험 체계로 규칙을 검증할 수 있게 하기 위해서다.

## 2. 메시지와 버전 호환

- **패키지**: `ieo.internal.v0`. HTTP API v0과 같은 주 버전이다. buf의 `PACKAGE_VERSION_SUFFIX` 규칙은 v1부터만 받으므로 이 한 규칙만 예외로 둔다(`buf.yaml`). 나머지는 buf `STANDARD` lint를 통과한다.
- **필드 번호를 다시 쓰지 않는다.** 필드를 지우면 번호와 이름을 `reserved`로 막는다. enum 값도 같다. enum의 0은 `…_UNSPECIFIED`이고 업무 값으로 쓰지 않는다.
- **v0 안에서는 추가만 한다.** 새 필드는 없을 때의 의미가 옛 동작과 같아야 한다. 옛 클라이언트나 옛 서버가 잘못 읽을 변경(의미 변경, 필수화, 타입 변경, 번호 변경)은 새 패키지 `ieo.internal.v1`로 한다. 호환 검사는 `buf breaking`의 `WIRE_JSON` 규칙이다. X-04가 CI에서 main 대비로 돌린다.
- **JSON 문서는 바이트로 싣는다.** `report_json`(ProgressReport v0)과 `checkpoint_json`(체크포인트 파일)은 각자의 JSON 스키마·규칙이 정본이다. proto로 다시 정의하지 않는다. 두 전송이 같은 문서를 같은 규칙으로 검사한다.
  - 보고의 중복 판정은 파싱한 JSON 값을 비교한다. HTTP로 보낸 보고와 gRPC로 보낸 같은 보고는 같은 보고다.
  - 체크포인트의 SHA-256은 받은 바이트로 계산한다. 같은 파일은 어느 전송으로 보내도 digest가 같다.
- **작은 요청은 타입이 있는 필드로 싣는다**(`SubmitReproRequestRequest`). 어댑터는 HTTP 본문과 **같은 JSON 값**을 만든다. 설정하지 않은 `optional` 필드는 JSON에서도 빠진다. 재현 요청의 중복 판정도 파싱한 JSON 값을 비교하므로, 이 규칙이 깨지면 같은 요청이 `EVENT_CONTENT_CONFLICT`가 된다. X-04 공통 회귀가 확인한다.
- **알 수 없는 필드.** proto3는 모르는 필드를 무시한다. HTTP는 정의되지 않은 키를 거부한다(`additionalProperties: false`, `SCHEMA_VIOLATION`). 그래서 v0의 새 필드는 무시돼도 안전한 것만 더한다(위 “추가만”). JSON 문서 안의 키는 바이트째 전달되므로 HTTP와 똑같이 거부된다.
- **응답의 결과 구분.** HTTP는 새 저장(201·202)과 재조회(200)를 상태 코드로 나눈다. gRPC는 `Outcome`(`OUTCOME_STORED`·`OUTCOME_REPLAYED`)으로 나눈다.

## 3. 인증과 역할별 권한

- **자격 전달**: metadata `authorization: Bearer <token>`. HTTP의 `Authorization` 헤더와 같은 값이다.
- **해석**: HTTP와 같은 `auth::Credentials`(토큰 digest → 주체·종류·범위·repo·role)를 쓴다. gRPC용 별도 자격·범위를 만들지 않는다. 메시지 안의 이름(repo_id, role, 보고의 actor)은 권한에 쓰지 않는다.
- **supervisor 주체**: 종류 `agent`, 하나의 (repo, role)에 묶인 자격이다. 범위는 지금 있는 에이전트 범위 셋만 쓴다: `agent:report`, `agent:repro`(guardrail 전용), `checkpoint:report`. 새 범위는 만들지 않는다.
  - PR #41 검토 b3cb083의 질문(재현 요청을 supervisor가 재사용하는가)의 답: 재사용한다. guardrail 역할의 supervisor만 `agent:repro`로 부른다. development 자격은 HTTP와 같이 거부된다.
  - 어느 프로세스가 어느 토큰을 쥐는가(모델 프로세스와 supervisor의 분리)는 02-05의 배정·인수 연결에서 정한다. 이 계약은 “HTTP가 받는 자격만 gRPC도 받는다”만 정한다.
- **검사 순서**: HTTP와 같다(같은 처리이므로). 401 → 403/409(범위·역할·현재 배정) → 400(형식) → 409(식별) → 422(참조). 체크포인트 작업은 역할 이름 검사(404)가 자격 검사보다 먼저다(HTTP와 같음).
- **역할 값**: `ROLE_UNSPECIFIED`는 알 수 없는 역할이다. HTTP의 알 수 없는 역할과 같이 `NOT_FOUND`(`NOT_FOUND`)다.

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
| 500 | `INTERNAL` | 트랜잭션은 롤백됐다 |
| 500, 작업 전 DB 연결 실패 | `UNAVAILABLE` | 아무것도 실행하지 않았다(§7.2) |
| 503 | `UNAVAILABLE` | 지금은 나오지 않는다 |

409의 `rejections[]`에 여러 `code`가 섞이면 위 두 묶음 중 하나에 모두 들지 않는 한 `FAILED_PRECONDITION`이다.

### 4.2 표면 작업에서 나올 수 있는 `code`

| RPC | `code`(HTTP 상태) |
| --- | --- |
| `SubmitAgentReport` | `UNAUTHENTICATED`(401) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`·`VERIFIED_FROM_AGENT_DENIED`(403) · `MISSING_FIELD`·`SCHEMA_VIOLATION`(400) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `STALE_ASSIGNMENT`·`ASSIGNMENT_SHA_MISMATCH`(409) · `REPORT_SEMANTIC_VIOLATION`(422) · `INTERNAL`(500) |
| `SubmitReproRequest` | `UNAUTHENTICATED` · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `STALE_ASSIGNMENT`(409, 현재가 아닌 배정·세대 또는 그 배정의 SHA와 다른 `target_sha`) · `SCHEMA_VIOLATION`(400, 권한·식별 검사를 통과한 새 요청만 형식 검사) · `INTERNAL` |
| `GetReproRequest` | `UNAUTHENTICATED` · `REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `NOT_FOUND`(404, 권한 검사 뒤) · `INTERNAL` |
| `ReportCheckpoint` | `UNAUTHENTICATED` · `NOT_FOUND`(404, 알 수 없는 역할) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `SCHEMA_VIOLATION`(400) · `EVENT_CONTENT_CONFLICT`(409 → `ALREADY_EXISTS`) · `CHECKPOINT_SEQUENCE_GAP`·`STALE_ASSIGNMENT`·`CHECKPOINT_MISMATCH`·`NO_CURRENT_ASSIGNMENT`·`DIGEST_MISMATCH`·`ASSIGNMENT_SHA_MISMATCH`·`REPO_MISMATCH`(409) · `INTERNAL` |
| `GetLatestCheckpoint` | `UNAUTHENTICATED` · `NOT_FOUND`(알 수 없는 역할) · `SCOPE_DENIED`·`REPO_SCOPE_DENIED`·`ROLE_MISMATCH`(403) · `NO_CHECKPOINT`(404) · `INTERNAL` |

이 표는 main 3157c89의 처리 코드를 읽어 만들었다. 정본은 처리 코드이고, X-04 공통 회귀가 두 전송의 상태·`code`가 같은지 확인한다.

### 4.3 상세 사유: `ieo-error-bin`

- 코디네이터가 만든 모든 비정상 응답은 바이너리 trailer `ieo-error-bin`에 `ieo.internal.v0.Error`를 싣는다: HTTP 상태(`http_status`), `Problem.code`·`message`, 또는 `rejections[]`(code·field·message·correction·retryable_after_edit)를 바꾸지 않고.
- gRPC 상태 메시지(`grpc-message`)에는 사람이 읽을 요약만 둔다. 클라이언트의 판단은 trailer로 한다.
- `google.rpc.Status`(`grpc-status-details-bin`)는 쓰지 않는다. 그 형식의 의존을 들이지 않고 기존 오류 모양을 그대로 옮기기 위해서다.
- **trailer가 없는 비정상 상태**(`DEADLINE_EXCEEDED`, `CANCELLED`, 연결 끊김, 라이브러리가 만든 `UNAVAILABLE`·`RESOURCE_EXHAUSTED` 등)는 코디네이터 처리의 답이 아니다. 클라이언트는 서버의 결과를 **알 수 없음**으로 다룬다(§5).

## 5. 재시도와 멱등

원칙(사용자 결정 2026-11-14 §5): **연결 실패만으로 재실행하지 않는다.** 결과를 알 수 없으면 같은 키로 서버의 실제 반영 여부를 먼저 확인한다. 상태를 바꾸는 작업은 키 없이 재시도하지 않는다.

**결과를 알 수 없는 경우**: `UNAVAILABLE`, `DEADLINE_EXCEEDED`, `CANCELLED`, 연결 끊김, trailer 없는 `INTERNAL`·`UNKNOWN`. trailer가 있는 `INTERNAL`(HTTP 500)은 롤백이 확정된 실패다.

| RPC | 멱등 키 | 결과를 알 수 없을 때 | 하지 않는 것 |
| --- | --- | --- | --- |
| `SubmitAgentReport` | (event_id, event_revision), 자격의 (repo, role) 안 | **같은 바이트**를 다시 보낸다. 서버 규칙상 이것이 재조회다: 저장돼 있으면 `OUTCOME_REPLAYED`와 저장된 영수증, 상태 변화 없음 | 내용을 바꿔 같은 키로 보내기(→ `ALREADY_EXISTS`). 새 내용은 새 revision |
| `SubmitReproRequest` | request_id | 같은 요청을 다시 보내거나 `GetReproRequest`로 확인한다. 저장된 요청은 다시 큐에 넣지 않는다 | 새 request_id로 다시 보내기(새 실행이 된다) |
| `GetReproRequest` | — | 다시 부른다 | — |
| `ReportCheckpoint` | (repo, role, seq) + 바이트 | **먼저 `GetLatestCheckpoint`**. 최신 seq·SHA가 보낸 것과 같으면 반영됨. 최신 seq가 보낸 seq − 1이면 같은 바이트로 다시 보낼 수 있다. 그 밖이면 외부 대조(02-04 `EXTERNAL_RECONCILIATION_REQUIRED`) | 같은 seq에 다른 바이트(→ `ALREADY_EXISTS`). 확인 없이 다음 seq로 넘어가기 |
| `GetLatestCheckpoint` | — | 다시 부른다 | — |

- **자동 재시도하지 않는 상태**: `INVALID_ARGUMENT`, `UNAUTHENTICATED`, `PERMISSION_DENIED`, `NOT_FOUND`, `ALREADY_EXISTS`, `FAILED_PRECONDITION`, `RESOURCE_EXHAUSTED`. 고치거나 다시 읽은 뒤의 새 판단으로만 다시 보낸다. `ABORTED`는 다시 읽고 판단한다.
- **횟수**: 확인·재전송은 지수 대기(0.5초에서 시작, 최대 4초)로 최대 3회다. 그래도 모르면 그 상태를 기록하고 멈춘다. 이 값은 클라이언트 정책이며 X-04에서 확정한다.
- **02-04 supervisor 규칙과의 대응**(`central::report`): 지금 HTTP 4xx는 “거부 확정”(대기 사본 삭제, 시작하지 않음), 5xx·전송 오류는 “결과 불명”(대기 사본 유지, 다음 기동에 채택 판단)이다. gRPC 클라이언트는 **trailer의 `http_status`**로 같은 구분을 한다: 400~499면 거부 확정, 그 밖(500, trailer 없음)이면 결과 불명. gRPC 상태 이름만으로 나누지 않는다.
- 이 RPC들은 모델을 부르지 않는다. 재조회·재전송은 모델 호출의 재시도가 아니다. 모델 호출의 자동 재시도 금지는 그대로다.

## 6. 제한 시간과 취소

### 6.1 규칙

- **클라이언트**는 모든 호출에 deadline을 둔다. supervisor 기본값은 10초다(02-04 HTTP 클라이언트의 시간 제한과 같다).
- **서버 최대 처리 시간**은 15초다. 실제 처리 시간은 min(클라이언트 deadline, 15초)이다. 지금 표면 작업은 트랜잭션 하나로 끝나며 재현 요청도 대기열에 넣기만 한다. X-04에서 측정해 기록하고, 바꾸면 이 문서를 고친다.
- **시작 전**: 남은 시간이 50ms보다 짧으면 트랜잭션을 열지 않고 `DEADLINE_EXCEEDED`로 답한다. 아무것도 실행하지 않았다.
- **커밋 전 만료·취소는 롤백한다.** 처리는 트랜잭션 안에서 문장마다 남은 시간을 넘지 않게 하고(`SET LOCAL statement_timeout`), 커밋 직전에 deadline과 취소를 다시 본다. 지났으면 롤백하고 `DEADLINE_EXCEEDED`·`CANCELLED`다.
- **커밋 뒤 끊김은 반영된 상태다.** 커밋과 응답 사이에 클라이언트가 떠났으면 상태는 바뀌었고 답만 잃었다. 클라이언트는 §5대로 같은 키로 재조회해 결과를 얻는다.
- 커밋 직전 확인과 커밋 사이의 짧은 틈에 취소가 오면 커밋될 수 있다. 이 경우도 위 “커밋 뒤 끊김”과 같이 재조회로 해결한다. 클라이언트는 `CANCELLED`·`DEADLINE_EXCEEDED`를 “실행되지 않음”으로 해석하지 않는다.

### 6.2 동기 처리와의 연결

- tonic(비동기)에서 동기 처리(`postgres` 동기 클라이언트)를 `spawn_blocking` 또는 연결을 쥔 작업자 풀에서 부른다. 비동기 요청이 취소돼도 이미 시작한 동기 처리는 끝까지 돈다. 그래서 위 커밋 전 확인을 처리 쪽에 둔다(취소 표지와 deadline을 처리에 넘긴다).
- 처리 함수 시그니처에 “커밋 전 확인”을 넣는 것은 동작 변경이 아니다. HTTP 경로는 deadline이 없으므로 확인이 늘 통과한다.

## 7. 전송 쪽 동작

HTTP 어댑터(`http.rs`)와 `handle`이 전송 쪽에서 하는 일을 gRPC에서 이렇게 한다.

### 7.1 크기 상한

- HTTP는 본문 1 MiB를 넘으면 400 `SCHEMA_VIOLATION`이다.
- gRPC는 메시지 상한을 1 MiB + 64 KiB로 둔다(문서 1 MiB와 봉투). 이 상한을 넘는 메시지는 라이브러리가 `RESOURCE_EXHAUSTED`로 거부한다. **HTTP와 다르다**(400이 아님). trailer가 없으므로 클라이언트는 원인을 크기로 알 수 없다. 클라이언트는 보내기 전에 크기를 확인한다.
- 상한 안의 메시지에서 `report_json`·`checkpoint_json`이 1 MiB를 넘으면 처리 쪽이 HTTP와 같이 `INVALID_ARGUMENT`(`SCHEMA_VIOLATION`)로 답한다.

### 7.2 DB 연결

- HTTP 작업자는 스레드마다 DB 연결 하나를 쥐고, 끊기면 다시 연다. 다시 열지 못하면 500(`INTERNAL`, “database unavailable”)이다.
- gRPC는 같은 방식의 연결을 쥔 작업자를 쓴다. 다시 열지 못하면 **`UNAVAILABLE`**(trailer: `http_status` 500, `code` `INTERNAL`)이다. 처리를 시작하지 않았으므로 클라이언트가 다시 보내도 된다. 이 경우만 HTTP 500과 gRPC 상태 대응이 다르다(§4.1).

### 7.3 헬스

- HTTP `/v0/health`는 인증 없이 트랜잭션 밖에서 `schema_meta`를 읽어 답한다(생존 확인, P04 결과 아님).
- gRPC는 표준 `grpc.health.v1.Health/Check`를 쓴다. 서비스 이름 `""`과 `ieo.internal.v0.SupervisorService`. 같은 읽기가 성공하면 `SERVING`, 실패하면 `NOT_SERVING`. 자격 없이 답한다. 스키마 버전과 정책 digest는 HTTP `/v0/health`에서 읽는다.
- 표준 헬스 서비스 구현(후보 `tonic-health`)을 X-04에서 확인한다. 쓸 수 없으면 같은 의미의 RPC를 이 패키지에 더하고 이 문서를 고친다.

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
- `SupervisorService`에 RPC를 **추가**한다(새 필드 번호, v0 안). 상태를 바꾸는 작업은 요청에 `idempotency_key`(HTTP `Idempotency-Key`와 같은 규칙, 8~128자)를 두거나 (assignment_id, generation)을 키로 쓴다. 재시도 표(§5)와 `code` 표(§4.2)에 행을 더한다.
- X-04가 준비되지 않았으면 02-05는 HTTP로 진행한다. gRPC 전환은 X-04의 몫이다(02-05의 선행·수용 기준은 바뀌지 않음).

## 9. 분리 영향(계획 §3을 main 3157c89에서 다시 확인)

| 대상 | 확인한 사실 | X-04에서 할 일(동작 변경 없음) |
| --- | --- | --- |
| `api::handle` | 전송과 무관하다. `Req`(method·path·query·Idempotency-Key·본문)를 받아 `Resp`(u16 상태·JSON)를 돌려준다. 업무 규칙은 헤더·소켓을 보지 않는다. 요청마다 트랜잭션 하나이고 2xx가 아니면 롤백한다 | 경로 일치를 **작업 표**(`Operation` 열거와 경로 → 작업 대응)로 뽑는다. HTTP 어댑터는 경로로 작업을 찾고, gRPC 어댑터는 작업을 직접 고른다. 둘 다 같은 처리 함수를 부른다. 기존 HTTP 시험이 그대로 통과해야 한다 |
| 작업마다 다른 실행 모양 | 계획 이후 생긴 차이다. repo 역할 상태·정지는 DB 없이 처리한다. `compare`는 권한 확인 → 트랜잭션 밖의 supervisor 호출 → 트랜잭션 순서다. 헬스는 인증 없이 트랜잭션 밖이다 | 작업 표의 각 행에 실행 모양(트랜잭션 / 트랜잭션 밖 / 앞선 외부 호출)을 둔다. gRPC 표면의 다섯 작업은 모두 “트랜잭션 하나” 모양이다 |
| 상태 코드 | 내부 표현은 HTTP 숫자(u16)다 | 그대로 둔다. gRPC 어댑터에 §4.1 대응표만 둔다 |
| `http.rs` | 약 100줄. 1 MiB 상한, 작업자별 DB 재연결(실패 시 500), `Authorization`·`Idempotency-Key` 읽기 | gRPC 어댑터가 §7의 대응을 따로 구현한다 |
| 커밋 전 확인 | 지금 처리에는 deadline·취소 개념이 없다 | 트랜잭션 실행기(`handle` 안의 `run`)에 커밋 전 확인을 넣는다(§6.2). HTTP는 늘 통과 |
| 런타임 | 동기(tiny_http, postgres 동기, 작업자 스레드마다 연결 하나) | tonic·tokio를 더하고 동기 처리를 작업자 풀에서 부른다. 같은 바이너리에 둘지 따로 둘지는 X-04 설계에서 정한다. 계약에는 영향이 없다 |
| supervisor 클라이언트 | `ieo-supervisor/src/central.rs`가 `ureq`로 체크포인트 보고·최신 조회를 직접 부른다(시간 제한 10초). 4xx와 5xx·전송 오류로 대기 사본 처리를 나눈다 | 호출을 `CoordinatorClient` 같은 경계 뒤로 옮긴다(HTTP 구현 = 지금 코드). gRPC 구현은 §5의 `http_status` 구분을 쓴다. 02-04 시험(`tests/serve.rs`의 FakeCentral 시험)이 두 구현에 같이 돌아야 한다 |
| 빌드 | 새 의존성 tonic·prost·tonic-health·tokio. 디스크가 빠듯하다(약 90% 사용) | X-04에서 빌드 산출물 경로를 정리하며 진행한다 |

02-04 영향: 이 계약은 02-04의 체크포인트 규칙(순번·바이트·digest·대기 사본·채택)을 바꾸지 않는다. 같은 규칙을 gRPC로 옮길 때 지킬 대응만 정했다(§5).

## 10. X-04가 확인할 것(공통 회귀로 넘김)

계약을 정할 때 가정한 것이다. X-04의 HTTP·gRPC 공통 회귀(장부 §7 X-04 수용 기준)에 넣는다.

1. 같은 요청에 두 전송이 같은 HTTP 상태·`code`·`rejections`와 같은 DB 상태를 낸다(§4.2 표의 모든 행).
2. 같은 보고·재현 요청·체크포인트를 한 전송으로 저장하고 다른 전송으로 다시 보내면 `REPLAYED`·200이다(§2의 JSON 값·바이트 동일성).
3. 커밋 전 deadline·취소는 롤백한다. 커밋 뒤 끊김은 재조회로 같은 결과를 얻는다. 전송 성공 여부와 서버 반영 여부가 다른 경우를 포함한다.
4. 에이전트 자격의 금지 범위가 같다(development의 재현 요청, 다른 repo·역할, 범위 밖 작업).
5. 크기 상한(§7.1)과 DB 연결 실패(§7.2)의 대응.
6. supervisor 클라이언트의 두 구현이 02-04 대기 사본 시험을 같이 통과한다.
7. `buf lint`와 `buf breaking`(main 대비)을 CI에서 돌린다.

## 11. 남은 결정과 한계

- **토큰을 쥐는 프로세스**(모델 프로세스와 supervisor의 분리)는 02-05에서 정한다(§3).
- **코디네이터 → supervisor 방향의 gRPC**는 이 계약 범위가 아니다. 필요하면 별도 결정으로 다룬다(§1.3).
- **TLS·상호 인증**은 정식 설계의 몫이다. kind 시험의 평문은 시험 환경에서만 쓴다(§7.4).
- **크기 초과 응답**이 HTTP(400 `SCHEMA_VIOLATION`)와 gRPC(`RESOURCE_EXHAUSTED`, trailer 없음)에서 다르다(§7.1).
- **오류 상세**는 `ieo-error-bin`이라는 자체 trailer다. 다른 생태계의 표준 클라이언트는 상태와 메시지만 본다(§4.3).
- **서버 최대 처리 시간 15초와 클라이언트 재시도 횟수·간격**은 초안 값이다. X-04 측정으로 확정한다(§5·§6).
- **라이브러리 지원**(tonic의 deadline 전달, 취소 감지, 헬스, 메시지 상한 설정)은 X-04에서 정확한 버전의 문서·소스와 시험으로 확인한다. 이 문서는 아직 확인하지 않았다.
