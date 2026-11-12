# 01-08 · 진행 보고 v0 · 등록/조회 API 계약

2026-11-12 KST · 계획 v1.3.3 · contract_revision 4 · 선행 01-07([등록 검사 계약](../../01-07/spec-contract-20261112/Registration_Contract_v0.md)).

| 구성 | 정본 | 이번 회차 |
| --- | --- | --- |
| 보고 v0 필드(주장/판정·단계·운영 상태·사유·증거·다음 담당·결정·일정) | [ProgressReport_v0.schema.json](../../../../ProgressReport_v0.schema.json), 계획 §1.5 | 그대로 고정 |
| actor={role,name}, 네 역할 본문, 일정 보고, 구 actor 마이그레이션 금지 | [Schedule_and_Role_Reports_v0.md](../../../../Schedule_and_Role_Reports_v0.md), [역할 예시 4개](../../../contract_examples/) | 그대로 고정 |
| 인증된 등록·현재 상태·사건 조회 + 에이전트 claim 경로 | **`contracts/ieo-api-v0.openapi.yaml`** (OpenAPI 3.1, 스키마는 위 JSON Schema를 `$ref`) | 신규 |
| CLI 계약 | 이 문서 §4 | 신규 |
| 5부 보고·재알림·완료 표현·4개 시험 입력 | 이 문서 §3, 참조 구현 `crates/ieo-report` | 신규(설계 로그 보존) |

## 1. 근거 수준과 경로

| 경로 | channel | evidence_level | 발행 주체 | 허용 report_type |
| --- | --- | --- | --- | --- |
| 에이전트 → 코디네이터 `POST /v0/agent/reports` | `agent_to_coordinator` | **claim** | 개발 supervisor/에이전트 | `development_progress` |
| 〃 | 〃 | **claim** | Guardrail | `guardrail_review` |
| 코디네이터 → 사람/클라이언트 (`GET …/state`, `GET …/events`) | `coordinator_to_client` | **verified** | 코디네이터 | `coordinator_verdict`, `decision_request` |

- 에이전트 자격은 (repo_id, role) 하나에 묶인다. 본문의 `actor.name`·라벨은 인가 근거가 아니다. 역할 불일치 → 403 `ROLE_MISMATCH`, verified/코디네이터 보고 제출 → 403 `VERIFIED_FROM_AGENT_DENIED`, 다른 repo → 403 `REPO_SCOPE_DENIED`. 거부 시 상태 변화·사건 0건.
- claim 저장(202)은 판정이 아니다. 완료 판정은 GitHub·현재 SHA·실행 증거 대조로 코디네이터가 하며 에이전트의 별도 완료 메시지를 필수 입력으로 요구하지 않는다(02-07).
- `(event_id, event_revision)`이 멱등 키다. 같은 내용 재전송 200, 다른 내용 409 `EVENT_CONTENT_CONFLICT`.
- `GET …/state`는 verified와 `latest_claims`를 **다른 필드**로 돌려 클라이언트가 claim을 판정으로 오인하지 못하게 한다. 외부 전달 상태(`pending/sent/unknown`)는 작업 판정과 별도 필드다.

## 2. 등록·조회 API (요약, 정본은 OpenAPI)

| 경로 | 범위(scope) | 성공 | 거부(상태 변화 0) |
| --- | --- | --- | --- |
| `POST /v0/sprints?mode=draft|executable` + `Idempotency-Key` | `sprints:register` + 대상 repo | 201 접수(의미 검토 대기), 200 동일 재제출 | 400 형식, 409 revision/멱등 키 충돌, 422 참조·프로파일·바인딩, 401/403 |
| `GET /v0/sprints/{id}/revisions/{rev}` | `sprints:read` | 불변 명세·digest·검토/접수 상태 | 401/403/404 |
| `GET /v0/sprints/{id}/state` | `sprints:read` | verified 현재 상태, claim 별도, 마지막 관측 시각, 전달 상태 | 401/403/404 |
| `GET /v0/sprints/{id}/events?cursor=&limit=` | `sprints:read` | 서버 커밋 순서, 같은 cursor는 같은 페이지 | 400 `INVALID_CURSOR`, 401/403/404 |
| `POST /v0/agent/reports` | `agent:report`(repo, role 고정) | 202 저장, 200 동일 재전송 | 400/409/422, 403(위 §1) |

- 거부 본문은 01-07의 `{code, field, message, correction, retryable_after_edit}` 목록이다. 01-08에서 `IDEMPOTENCY_KEY_CONFLICT`, `EVENT_CONTENT_CONFLICT`, `REPORT_SEMANTIC_VIOLATION`, `INVALID_CURSOR`를 추가했다.
- **조회는 사건·알림·상태 전환을 만들지 않는다.** 재조회로 완료 수가 늘지 않는다.
- 등록 접수(201)는 실행 인수나 완료가 아니다.

## 3. 사람용 보고·재알림·완료 표현

**5부 구성**(설계 로그 「사람이 읽고 결정하는 운영 보고」, 참조 구현 `render_human`):

1. 한 줄 요약 — `결정 필요|상태 변경 · repo 작업 · 단계/운영 상태 · 사용자 결정 필요|불필요`
2. 확인한 사실과 영향 — 근거 수준·관측 시각(시간대), 대기 사유, 모르는 사실, 영향 작업, 일정(기한 상태·남은 작업 또는 미산정 이유·예상 완료 범위 또는 미산정)
3. 다음 행동 — 행동·담당, 재개 조건, **다음 확인 시각(해결 예상 시각 아님)**
4. 결정 요청 — 질문 하나, 선택지별 영향, 권고와 이유, 실제 시한 또는 없음 이유, 미응답 기본 조치
5. 확인 근거 — 사건 ID/revision, 작성 역할/이름, 스프린트 revision, 대상 SHA, 증거 참조

**재알림 규칙**(`notification`): 처음 보는 사건은 알린다. 이전에 알린 revision 대비 단계·운영 상태·증거·영향 작업·재개 조건·결정 시한/질문/선택지·모르는 사실·일정 상태가 바뀌었거나 결정이 처음 필요해지면 다시 알린다. `observed_at`·`next_check_at`·`event_revision`만 바뀐 재관측은 DB에 남기고 알리지 않는다.

**완료 표현**: CI 통과, 현재 SHA 독립 승인, 병합, main CI, 후속 인수를 각각 따로 표시한다. 이전 SHA 승인은 현재 SHA 승인이 아니다(`review_currency`). 외부 요청 결과 미확인은 성공·실패·미전송과 구분해 그대로 쓴다.

**설계 로그의 시험 입력 4개**(적합성 사례로 보존):

| 입력 | 기대 해석 | 사례 |
| --- | --- | --- |
| 현재 커밋 CI 통과 + 이전 커밋 독립 승인 | CI 통과까지만 확인, 현재 SHA 검토 필요(`REVIEW_PENDING`), 완료로 읽히지 않음 | `input1_…` |
| 외부 요청 응답 유실·원 결과 미확인 | 인계 보류, 담당이 원 요청 대조, 사용자 재전송 결정 불필요, 같은 요청 재전송 안 함 | `input2_…` |
| 승인 일정 불가로 범위/일정 결정 필요 | 질문·선택지별 영향·권고와 이유·시한·미응답 조치를 읽을 수 있음 | `input3_…` |
| 같은 사건 재관측 | 새 증거·영향 변화 없으면 추가 알림 0건; 새 증거/결정 필요/재개 조건 변화면 알림 | `input4_…` |

사람이 실제로 이해하는지는 04-09(P12), 실제 발송·중복 억제 동작은 02-07/04-08에서 확인한다. 이 계약과 참조 구현은 문안·규칙의 결정 가능성까지만 보인다.

## 4. CLI 계약 (`ieo`, 시험 클라이언트)

CLI는 위 API로만 동작한다. DB·파일을 직접 읽지 않는다. 자격은 `IEO_TOKEN`(또는 `--token-file`), 서버는 `IEO_API`(기본 `http://127.0.0.1:8080`).

| 명령 | API | 출력·종료 코드 |
| --- | --- | --- |
| `ieo sprint register <spec.json> [--executable] [--idempotency-key K]` | `POST /v0/sprints` | 201/200 → 결과 JSON, exit 0. 거부 → 거부 목록(필드·교정), exit 1 |
| `ieo sprint register --human <input.json> --context <binding.json>` | 로컬 변환(01-07 규칙) 후 `POST /v0/sprints?mode=draft` | 변환 거부 → exit 1, 변환 기록을 함께 출력 |
| `ieo sprint show <id> <revision>` | `GET …/revisions/{rev}` | exit 0 / 404 → exit 1 |
| `ieo sprint state <id> [--human]` | `GET …/state` | `--human`이면 5부 보고로 출력(verified와 claim을 별도 절로) |
| `ieo sprint events <id> [--cursor C] [--limit N]` | `GET …/events` | 사건 JSON 줄 + 마지막 줄에 `next_cursor` |
| 공통 | — | 401/403 → exit 3, 네트워크/5xx → exit 4(재시도 판단은 사용자), 사용법 오류 → exit 2 |

멱등 키를 주지 않으면 CLI가 요청 본문 digest로 만든다(같은 파일 재제출이 같은 키). 에이전트 claim 제출 명령은 CLI에 두지 않는다(역할 자격 전용 경로).

## 5. 남은 공백

- 인증 토큰 발급·회전 방식과 사람 사용자 principal 등록 — 02-03 구현에서 결정(사용자 결정 필요 시 보고).
- `review_state` 전환(의미 검토 통과/반려)의 발행 주체 — P01(04-01)에서 검증.
- 사건 보존 기간·cursor 만료 — 02-03.
