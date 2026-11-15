# 02-05 단계 3 · 코디네이터의 배정 전달(compare 경유 start·reconnect)과 공유 예산 검사

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 3([계획](../plan-20261115/PLAN.md) §3 단계 3, §3.1 작업 표). 이월 C8·C12를 닫고 C1의 단계 3 몫을 한다 |
| 착수 근거 | 계획 머리의 갱신 기록(사용자 결정 2026-11-15, X-08 검수 완료, 런타임 계약은 revision 11로 씀). 단계 1·2 병합(PR #72·#73) |
| 검토 반영 | PR #73 검토 a349dc6 L-1(변이 R2·R4·R5를 잡는 기대값 확인), 단계 2 보고 §5의 이월(자기 배정 조회의 `last_delivery_id`·이미지·프로파일 기대값) |
| 기준 커밋 | main 3701f6b. 구현 커밋 2c8fe8d, 증거 커밋 87956da |
| 모델 호출 | 0회. 컨테이너·실제 회차 없음. 영속 PoC DB와 58090 코디네이터는 쓰지 않았다(일회성 시험 DB만) |
| 상태 | 구현과 시험 완료, **독립 검토 전**. 같은 PR의 원장 snapshot이 이 단계를 기록한다 |

## 1. 더한 작업과 바뀐 동작

### 1.1 배정 전달 deliverAssignment(코디네이터)

`POST /v0/repos/{repo_id}/roles/{role}/deliveries`. 자격은 `roles:control`(service·user, repo 범위)이다. `Idempotency-Key`가 전달의 request_id다. 운영 주체 작업이라 **HTTP 전용**이고 RPC는 없다(내부 계약 §1.2).

순서(작업 표의 새 실행 모양 `Shape::Delivery`):
1. 401 → 404 역할 → 403 → 400 Idempotency-Key → 400 본문 → 404 역할 설정 없음.
2. 트랜잭션 1에서 요청을 기록한다(`role_deliveries`, `evidence_level = verified`).
   - 같은 키·같은 본문이면 재조회다.
   - 같은 키·다른 본문은 409 `IDEMPOTENCY_KEY_CONFLICT`다.
   - 현재 배정이 없으면 404 `NO_CURRENT_ASSIGNMENT`다.
   - 배정·세대가 머리와 다르면 409 `STALE_ASSIGNMENT`다.
   - 다른 전달이 쓴 attempt_id는 409 `ATTEMPT_ID_CONFLICT`(새 코드)다.
   - 거부는 행을 남기지 않는다.
3. 재개 판단은 기존 `compare`(`judge_resume`)와 같은 함수다.
   - `no_checkpoint`이면 start, `resume_allowed`이면 reconnect를 부른다.
   - `new_session: true`이면 `resume_allowed`·`stale_generation`에서도 start를 부른다.
   - `already_running`·`external_reconciliation_required`·`not_observed`, 그리고 `new_session` 없는 `stale_generation`은 아무것도 부르지 않는다(`outcome: not_called`).
4. 공유 예산 검사(§1.2)를 한다. 거부되면 부르지 않는다(`budget_refused`).
5. supervisor의 start·reconnect를 트랜잭션 밖에서 부른다.
   - 코디네이터 기록(배정·세대·sprint revision·명세 digest·이미지·프로파일), 대상 SHA, 한도는 **DB와 역할 설정에서 만든다**. 요청에서 받지 않는다.
   - 회차 한도는 예산·역할 몫 한도 위에 revision의 `elapsed_seconds`·`total_tokens`로 한 번 더 자른다.
6. 트랜잭션 2에서 결과를 기록한다(`role_delivery_results`, 추가 전용).
   - 결과는 `accepted`(202), `refused`(supervisor의 거부), `unknown`(답 유실) 중 하나다.
   - 보낸 본문의 digest(`sent_sha256`)도 같이 남긴다.
   - 받아들여진 회차는 같은 라이브러리가 배경에서 끝까지 기다려 계측·과금하고 원장에 기록한다. 그동안 원장 잠금을 쥔다.

재조회:
- 같은 키·같은 본문이 다시 오면 저장된 전달을 200으로 준다.
- 결과가 `unknown`이거나 결과 행이 없으면, 먼저 supervisor에 그 시도의 저장된 답(`GET /v0/role/attempts/{id}`)을 묻는다. 이 조회는 실행을 일으키지 않는다.
- 답에 따라 다음 결과 행을 더한다: `accepted`(보낸 digest와 같을 때), `not_accepted`(404), 또는 다시 `unknown`.

전달 응답은 인수가 아니다. 인수는 supervisor의 takeover 보고이고 그 연결은 단계 4다.

설정(`--roles`):
- 역할 항목에 `image_digest`·`profile_id`·`profile_revision`을 더한다.
- repo 항목에 `budget_plan`(02-04 호출자 계획에서 역할 URL을 뺀 것)을 둔다. 역할 URL·어댑터 자격은 역할 항목 하나에서 가져온다.
- 설정이 없으면 404이거나 `budget_refused`다. 58090의 지금 설정에는 이것이 없으므로 그 코디네이터는 전달을 하지 않는다.

### 1.2 공유 예산 검사 라이브러리

02-04 호스트 호출자의 `call_with`를 세 단계로 나눴다. 호출자는 세 단계를 이어 부르고, 전달 경로는 같은 세 단계를 쓴다(계획 §3 “예산 검사의 위치”).
- `caller::admit`:
  - 원장 잠금(`caller.lock`)을 잡는다.
  - 진행 중 표식이 있으면 거부한다.
  - 원장을 fail-closed로 읽는다(보정은 다시 측정).
  - 사람 확인을 요청한 기록, 이미 기록된 시도를 거부한다.
  - `limits_for`로 미계측 회차, 추가 결제 없음 관측, 예산·역할 몫 한도를 본다.
  - 모든 역할 서비스가 유휴이고 그 회차를 원장이 알아야 한다.
- `Admission::start`: 진행 중 표식을 남기고 요청을 보낸다.
  - 거부되면 표식을 지운다.
  - 답이 유실되면 미계측·`stop_required` 기록을 남긴다.
  - `Admission::lower`는 한도를 낮추기만 한다.
- `Running::finish`: 회차 끝을 기다린 뒤 stream과 자식 전사를 받아 02-01 계측기로 측정하고 과금한다. 결과 확인, 추가 결제 신호, 환경 이름 검사를 하고 기록을 덧붙인다.

권한 있는 검사는 역할 컨테이너 밖(코디네이터·호스트 호출자)에서 돈다. supervisor의 `budget`·`share`는 보조 상한으로 남는다. 02-04 호출자의 동작은 그대로다(`tests/caller.rs` 32개를 고치지 않고 통과, X-08 교정 유지). 02-05 예산 이름·상한은 더하지 않았다(단계 8 전 별도 코드 PR).

### 1.3 supervisor

- **같은 시도의 재전송**(런타임 계약 §7에 코드를 맞춤):
  - 받아들인 start·reconnect의 답과 요청 digest를 실행 전에 `/state/attempts/<attempt_id>.json`에 둔다.
  - 같은 `attempt_id`·같은 연산·같은 내용(같은 JSON 값)이면 저장된 답을 200(`replayed: true`)으로 주고 실행하지 않는다.
  - 다른 내용·다른 연산은 409 `ID_CONTENT_CONFLICT`다.
  - 거절은 저장하지 않는다.
  - `GET /v0/role/attempts/{id}`는 저장된 답을 읽기만 한다.
- **현재 배정과의 대조**:
  - 중앙 기록이 코디네이터이면 start·reconnect의 중앙 체크포인트 대조 전에 `getOwnAssignment`(HTTP·gRPC 클라이언트 모두)로 요청의 코디네이터 기록을 본다.
  - 배정·세대·sprint·revision·digest를 대조하고, 전달이 기록돼 있으면 그 이미지·프로파일 기대값도 대조한다.
  - 같은 배정의 낮은 세대는 `STALE_GENERATION`이다. 그 밖의 차이와 현재 배정 없음은 `EXTERNAL_RECONCILIATION_REQUIRED`, 무응답은 `NOT_OBSERVED`다. 어느 쪽도 실행하지 않는다.
  - 대상 SHA는 대조하지 않는다(takeover 저장 조건 (b)가 거부한다).
- **자기 배정 조회**가 `last_delivery_id`(현재 머리의 최신 전달)와 `expected_runtime`(그 전달의 이미지·프로파일)을 돌려준다. 단계 2의 이월이다.

### 1.4 C1·C12

- C1:
  - 논리 이름(`logical_name`, 기본 `<repo>-<role>`)은 코디네이터의 전달 기록(매핑 레코드)에만 남는다.
  - reconnect는 체크포인트·중앙 기록의 session_id로만 재개한다.
  - 코드 검토: `checkpoint::decide`는 체크포인트 `session_id`만 `Resume`에 쓰고, `args::build`의 `-n`은 고정된 표시 이름이다.
  - 시험: 논리 이름만 바꾼 전달이 같은 session_id로 재개되고, 이름이 같고 session_id만 바꾼 로컬 기록은 외부 대조로 멈춘다(부르지 않음).
  - 인수 기록과의 연결은 단계 4다.
- C12, f73ac13 P3: 지시성 digest는 파일과 링크로 계산한다. 그래서 CLI가 만드는 빈 `projects/*/memory/`는 변조가 아니고 정리가 지운다. 그 안의 파일은 변조다. 동작은 이미 그랬고, 이번에 시험과 계약 문장(revision 11 §14 항목 14)을 더했다.
- C12, bc55edf R5: 지시성 대상 밖의 읽을 수 없는 파일(권한 000)은 열지 않는다. digest와 정리가 막히지 않는다. 대상 안의 같은 파일은 fail-closed다.

## 2. 바꾼 계약과 코드

- `crates/ieo-coord`:
  - `src/delivery.rs`(새 파일), `src/api.rs`(작업 표 `DeliverAssignment`, `Shape::Delivery`, 자기 배정 조회 필드, 몇 함수를 `pub(crate)`로).
  - `src/schema.sql`: `role_deliveries`·`role_delivery_results`, 추가 전용 트리거. `CREATE … IF NOT EXISTS`라 영속 DB는 다음 `migrate`에 새 표만 생긴다.
  - `Cargo.toml`: `ieo-supervisor`를 일반 의존으로 옮겼다. Cargo.lock은 바뀌지 않았다.
- `crates/ieo-supervisor`:
  - `src/caller.rs`: 공유 라이브러리로 나눔.
  - `src/central.rs`: `own_assignment`(두 클라이언트), `check_assignment`.
  - `src/serve.rs`: 재전송, 저장된 답 조회, 현재 배정 대조.
- `crates/ieo-proto`: `expected_runtime` 변환과 왕복 시험.
- `contracts/ieo-api-v0.openapi.yaml` 0.10.0 → **0.11.0**(추가). deliverAssignment 경로, `Delivery` 스키마, `expected_runtime`, `ATTEMPT_ID_CONFLICT`. `openapi-spec-validator` 통과.
- `contracts/proto/.../supervisor.proto`: `GetOwnAssignmentResponse.expected_runtime = 16`, 메시지 `RuntimeExpectation`. 추가만 했고 `buf lint`·`build`·`breaking`(main 3701f6b 대비)을 통과했다. RPC는 더하지 않았다.
- `contracts/ieo-internal-v0.md`: §1.2에 deliverAssignment는 HTTP 전용이라고 적었다. 새 §14(바꾼 문장).
- 런타임 계약 **revision 11**: 머리 기록, §7 오류 대응표의 한 행, 새 §14 항목 11~14(재전송, 현재 배정 대조, 배정 전달과 예산 검사 위치, 재개 키·지시성 digest 확인).

## 3. 시험

새 시험 `crates/ieo-coord/tests/delivery_e2e.rs`:
- 코디네이터 바이너리, 실제 PostgreSQL, 역할 서비스(가짜 `claude`), 시험용 원장(02-04 예산 이름, 역할 몫 4회 뒤 6회)으로 돈다.
- supervisor가 코디네이터에 닿는 전송을 HTTP와 gRPC로 각각 한 번씩 돌린다.

| 경우 | 기대값 |
| --- | --- |
| 자격·범위·형식·설정 | 401, 403(`SCOPE_DENIED`·`REPO_SCOPE_DENIED`), 404 역할, 400(키·id·모르는 필드·`new_session`) |
| 배정 없음 | 404 `NO_CURRENT_ASSIGNMENT` |
| 낡은 세대 | 409 `STALE_ASSIGNMENT`. 행 0, 호출 0 |
| 체크포인트 없음 | start, `accepted`. 코디네이터 기록·대상 SHA가 DB와 설정 값과 같고, 한도의 `billable_room`이 revision의 100,000으로 잘림. 원장 기록은 계측됨·`stop_required: false`. 자기 배정 조회가 `last_delivery_id`·`expected_runtime`을 줌 |
| 같은 키 재조회, 다른 본문, 같은 시도 다른 키 | 200 같은 답, 409 `IDEMPOTENCY_KEY_CONFLICT`, 409 `ATTEMPT_ID_CONFLICT`. 호출·실행 수 불변 |
| after_run 뒤 | reconnect로 같은 session_id 재개. 논리 이름만 바뀜(C1) |
| supervisor 쪽 대조 | 명세 digest·revision·이미지·프로파일·세대가 다른 직접 start는 409 외부 대조, 실행 0. 새 세대 뒤 옛 세대는 `STALE_GENERATION` |
| 실행 중 | `already_running`, 부르지 않음 |
| 로컬 체크포인트 변경, session_id만 바꾼 기록 | `external_reconciliation_required`, 부르지 않음 |
| supervisor 무응답(guardrail) | `not_observed`, 부르지 않음 |
| 새 세대 | `new_session` 없으면 `stale_generation`·부르지 않음. 있으면 start, 새 session_id |
| 예산: 원장의 미계측 회차 | `budget_refused`, 부르지 않음 |
| 예산: 역할 몫 4회 소진 | `budget_refused`(“4 of 4 runs”), 부르지 않음 |
| 코디네이터 재시작 | 저장된 전달이 같은 답 |
| supervisor 거부(자격 파일 없음) | `refused`(503 `CREDENTIAL_UNAVAILABLE`), 원장 기록 없음 |
| 거부 응답 유실 | `unknown` → 재조회 `not_accepted`(n=2) → 다시 같은 답 |
| 받아들인 응답 유실 | `unknown`, 원장은 사람 확인 요청 → 재조회 `accepted`(n=2, 같은 `sent_sha256`) → 다시 같은 답. 실행 1회, 재조회는 아무것도 시작하지 않음 |
| 그 뒤 | 모든 전달 `budget_refused`(“asks for a person”) |
| 끝 | 실행 5, `role_deliveries` 15, 결과 17, `accepted` 5, `role_checkpoints` 10 |

그 밖의 새 시험:
- `crates/ieo-supervisor/tests/serve.rs`:
  - `the_same_attempt_with_the_same_content_replays_the_stored_answer`: 같은 바이트·같은 값의 다른 바이트 200, 다른 내용·다른 연산 409, 저장된 답 조회, 거절은 저장 안 함, 재시작 뒤 재전송.
  - `a_start_is_checked_against_the_coordinators_current_assignment`(HTTP·gRPC): 항목별 불일치, 낡은 세대, 이미지·프로파일 기대값. 가짜 중앙 기록이 자기 배정 조회에 답하도록 고쳤다.
- `crates/ieo-supervisor/tests/supervisor.rs`: `an_empty_memory_directory_is_not_a_tamper`, `an_unreadable_file_outside_the_directive_scope_is_never_opened`.
- `crates/ieo-coord/tests/grpc.rs`(L-1): 같은 `observed_at`은 `stale_on_arrival: true`. 없는 날짜 셋(2026-02-30, 2025-02-29, 2026-04-31)은 400 `SCHEMA_VIOLATION`. 다른 세션의 takeover만 있는 첫 행동은 409 `CHECKPOINT_MISMATCH`.
- `crates/ieo-coord/tests/operations.rs`: 경로와 실행 모양.

결과([evidence/test.log](evidence/test.log), 커밋 2c8fe8d):
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`: 통과.
- buf lint·build·breaking(main 3701f6b 대비), `openapi-spec-validator`: 통과.
- `cargo test --locked --all-targets`(시험 DB, `RUST_TEST_THREADS=4`): **365 통과, 0 실패**. 기준 358에 새 시험 7개(종단 2, serve 3, configdir 2)다. 02-04 호출자 시험 32개, `checkpoint_e2e` 2개는 고치지 않고 통과했다.

변이 16개를 모두 잡았다([evidence/mutants.txt](evidence/mutants.txt)):
- 전달 쪽 6개: `new_session` 없이 start, 머리 대조 제거, `unknown` 재조회 생략, 404를 수락으로, revision 한도 미적용, 이미지를 설정에서 받지 않음.
- 예산 라이브러리 1개: 사람 확인 기록 검사 제거.
- supervisor 쪽 4개: 재전송 내용 무시, 배정 대조 제거, 기대값 무시, 낡은 세대 판정 반전.
- configdir 2개: 범위 검사 전 모든 파일 읽기(bc55edf R5 변이), 디렉터리도 digest.
- L-1의 R2·R4·R5.

## 4. 이월 항목

- **C8 닫음.** 코디네이터가 `compare`를 거쳐 start·reconnect를 부르고, 요청·판단·응답 또는 `unknown`을 기록한다.
- **C12 닫음.** f73ac13 P3와 bc55edf R5의 시험, 계약 문장.
- **C1 단계 3 몫.** 매핑 레코드의 논리 이름, session_id로만 재개하는 시험과 코드 검토. 인수 기록과의 연결은 단계 4에서 닫는다.
- **단계 2 이월**(자기 배정 조회의 `last_delivery_id`·이미지·프로파일 기대값) 닫음. 세션 상태 조회 경로는 단계 4로 남는다.
- **PR #73 검토 a349dc6 L-1 닫음.**
- C14 일부: 실제 회차가 중앙 체크포인트 경로를 지나는 전달 경로가 생겼다. 실제 회차는 단계 8이다.

## 5. 계획과 다른 점, 결정이 필요한 것

- **명시적 새 세션(`new_session`).** 계획 표에서는 `stale_generation`이면 부르지 않는다. 그러면 세대가 바뀐 뒤 중앙 기록이 있는 한 전달로는 새 세션을 시작할 길이 없다. 그래서 운영 주체가 명시하면 `resume_allowed`·`stale_generation`에서 start를 부르게 했다. 옛 세션은 재개하지 않는다. 런타임 계약의 기존 동작(새 세대에서 start는 허용)과 같다. 검토에서 확인을 바란다.
- **재조회는 다시 보내지 않고 읽기만 한다.** 계획은 “같은 request_id로 재조회·재전송”이었다.
  - 다시 보내지 않는 이유: 답을 잃은 시도는 02-04 규칙대로 원장에 사람 확인 요청으로 남는다. 다시 보내면 supervisor가 받지 못한 요청을 예산 검사 없이 실행할 수 있다.
  - 그래서 재조회는 실행을 일으키지 않는 `GET /v0/role/attempts/{id}`다.
  - 같은 `attempt_id`·같은 내용의 POST 재전송(§7)도 구현했다(다른 호출자용).
- **답 유실 뒤 예산 정지.** 재조회가 `accepted`를 보여도 원장의 사람 확인 요청은 남는다. 그 예산의 다음 전달은 사람이 원장을 대조(보정 기록)할 때까지 멈춘다. 02-04 규칙 그대로이고 fail-closed다.
- **배경 마무리.** 받아들인 회차의 계측·기록은 코디네이터 프로세스의 배경 스레드가 한다. 회차 중에 코디네이터가 재시작되면 진행 중 표식이 남아 다음 전달을 막는다. 사람이 대조하고, 0으로 세지 않는다.
- **대상 SHA는 supervisor 쪽 대조에 넣지 않았다**(takeover 저장 조건이 이미 거부한다).
- **재조회의 남는 틈.** 저장된 답 조회는 실행 시작과 같은 잠금을 잡는다. 그래도 아직 도착하지 않은 요청이 재조회 뒤에 도착하는 경우는 막지 못한다. 같은 호스트에서는 무시할 만하다.
- **회차 한도.** revision의 `elapsed_seconds`·`total_tokens`로 회당 상한만 자른다. 배정의 남은 양은 단계 5다.
- **새 코드 `ATTEMPT_ID_CONFLICT`**(추가).
- 사용자 결정이 필요한 것은 없다고 본다. 정책 파일·판정 규칙·기존 작업의 HTTP 동작은 바꾸지 않았다. 위의 `new_session`은 검토로 확인한다.

## 6. 한계

- 전달 종단 시험은 중앙 앞섬·로컬 앞섬을 따로 넣지 않았다. 같은 판단 함수가 `checkpoint_e2e`에서 시험된다.
- supervisor 재시작 뒤 같은 판단은 serve 시험(재시작 뒤 재전송)과 `checkpoint_e2e`가 덮는다. 전달 종단 시험은 코디네이터 재시작만 한다.
- 전달 기록은 verified다. supervisor가 보내는 기록은 여전히 claim이다(런타임 계약 §2.1). 재개의 최종 근거는 단계 7의 GitHub 대조다.
- 예산 설정은 02-04·X-02 이름만 받는다. 02-05 예산은 단계 8 전 별도 PR이다. 58090 코디네이터에는 전달 설정이 없어 실제 전달은 일어나지 않는다.
- 시험 정리 기록: /home 사용률이 99%라 시험 DB 생성이 한 번 실패했다. 시험 DB(55432)에서 만든 프로세스가 끝난 `ieo_t_*` 데이터베이스 114개를 지웠다. 다른 세션이 남긴 것도 포함한다. 영속 DB(55433)와 58090은 건드리지 않았다. 증거 실행은 `RUST_TEST_THREADS=4`로 했다.
