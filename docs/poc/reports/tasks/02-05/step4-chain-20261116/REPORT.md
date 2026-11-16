# 02-05 단계 4 · 인수·첫 행동 연결과 연결 조회

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 4([계획](../plan-20261115/PLAN.md) §3.2 단계 4, §3.1 작업 표의 인수 보고·연결 조회 행). 수용 기준 본문(전달·실제 인수·첫 행동·체크포인트 연결, claim 보존), C1을 닫고 C14 일부를 해소한다 |
| 착수 근거 | 단계 3 병합(PR #74, main 375f21f). 계획 머리의 갱신 기록(런타임 계약은 이 단계에서 revision 12) |
| 검토 반영 | PR #74 검토 fe924b7의 L-1·I-2·I-4·I-5·I-6(§9). PR #75 검토 5dbb3d8(변경 요청)의 M-1·L-1·L-2·I-1~I-6(§11) |
| 기준 커밋 | main 375f21f. 구현 커밋 bd2cb8b, C1 단언·L-1 정정 84c61d9, 첫 증거 a83d51c, 보고 5dbb3d8. 검토 5dbb3d8 교정 커밋 1b7f51c·40e2c98, 다시 만든 증거 8717b88(evidence/test.log 첫 줄이 40e2c98, 작업 트리 깨끗) |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 영속 PoC DB(55433)·58090 코디네이터는 쓰지 않았다(일회성 시험 DB 55432만) |
| 상태 | 구현과 시험 완료. 검토 5dbb3d8(변경 요청)을 교정했고 **재검토 전**이다. 원장(`state.json`)은 이 PR에서 고치지 않았다(원장 담당이 기록) |

## 1. 더한 작업과 바뀐 동작

### 1.1 인수(takeover)와 전달의 연결(코디네이터)

- 전달(deliverAssignment)은 start·reconnect 본문의 최상위에 `delivery_id`를 싣는다. 보낸 본문 digest(`sent_sha256`)에도 들어간다.
- supervisor는 그 값을 그 회차의 모든 체크포인트(takeover·after_run·error)에 `delivery_id`로 쓴다. 없으면 null이다. 그 회차의 시도 id도 `attempt_id`로 쓴다(검토 5dbb3d8 M-1).
- 코디네이터의 체크포인트 저장(reportCheckpoint, HTTP·gRPC 같은 처리)은 takeover에 대해 02-04 조건 (a)~(c) **뒤에** 다음을 본다.
  - `delivery_id`가 없으면 409 `DELIVERY_REQUIRED`(새 코드).
  - 그 전달이 기록돼 있고, 같은 repo·역할·배정·세대이며, **그 전달이 부른 `attempt_id`가 체크포인트의 `attempt_id`와 같고**(takeover는 그 전달 자신의 호출에 답한다. 전달 id만 이름 붙인 다른 시도나 시도 없는 체크포인트는 그 전달을 소비하지 못한다), 마지막 결과가 **아직 없거나**(아래) `accepted`·`unknown`이어야 한다. 아니면 409 `DELIVERY_MISMATCH`(새 코드). `not_called`·`budget_refused`·`refused`·`not_accepted`는 “보낸 것이 없음”이라 거부한다.
  - 전달 잠금(`delivery:{repo}:{role}`)을 잡고 읽는다(체크포인트 잠금 다음. 이 순서만 쓰므로 교착 없음). 결과 기록과 엇갈리지 않는다.
- **같은 트랜잭션**에서 체크포인트 행 다음에 인수 행(`role_takeovers`: 전달, 순번, digest, session_id, 적용 sprint revision·digest, 보고자, 수신 시각, `claim`)을 쓴다. `ON CONFLICT (delivery_id) DO NOTHING`의 결과가 1행이 아니면 409 `DELIVERY_MISMATCH`(“already has a takeover”)로 **체크포인트 행까지 롤백**한다. 전달 하나는 한 번만 인수된다.
- 같은 순번·같은 바이트의 takeover 재보고는 지금처럼 200 중복이다(인수 행을 다시 쓰지 않음). 02-04의 대기 사본 채택·답 유실 규칙은 그대로다.
- “결과가 아직 없음”을 받아들이는 이유: supervisor는 202를 돌려준 직후 실행 스레드에서 takeover를 보고한다. 코디네이터는 그 202를 받은 뒤에 결과를 기록한다(트랜잭션 2). 그래서 takeover가 결과보다 먼저 도착하는 것이 정상 경로다(§7).

### 1.2 첫 행동(supervisor)

- 새 모듈 `crates/ieo-supervisor/src/first_action.rs`.
  - 정의: `system`/`init` **다음**의 첫 `assistant` 사건(자기 세션의 것, `parent_tool_use_id` 없음) 중 `tool_use` 블록이나 비지 않은 `text` 블록이 있는 것. `tool_use`가 있으면 `tool_use`, 아니면 `text`.
  - `init` 앞의 사건, `init` 자체, 내용 없는·`thinking`만 있는 사건, `user`·`result`·`rate_limit_event`, 자식 사건은 행동이 아니다.
  - 오프셋은 회차 `stdout.jsonl`의 0부터 센 줄 번호, 시각은 supervisor가 그 줄을 읽은 시계 시각이다.
- 실행 감시(`run::run_with`)가 `stdout.jsonl`에 쓰는 모든 줄(정지 뒤 복사되는 줄 포함)을 순서대로 probe에 넣는다(`Options.probe`).
- 처음 관측하면 콜백이 (1) 상태의 `last_actual_action_at`을 그 시각으로 바꾸고 (2) 다른 스레드에서 `recordFirstAction`을 보낸다.
  - HTTP·gRPC 클라이언트에 `record_first_action`을 더했다. 기본 구현은 “지원 안 함”(답 없음)이라 시험용 가짜 중앙 기록은 그대로 컴파일된다.
  - 결과는 상태의 `first_action_report`에 남는다. 실패는 회차를 멈추지 않는다.
- after_run 체크포인트의 `first_action`은 관측값(`attempt_id`·`event_offset`·`observed_at`·`kind`)이고, 없으면 `"none"`이다. error는 관측값 또는 null(모름), takeover는 null이다. 0이나 시작 시각으로 채우지 않는다.
- 회차 결과(`outcome.json`·상태 `last`)에도 `first_action`을 남긴다. 서비스 재시작 뒤 `last_actual_action_at`은 볼륨의 회차 결과 중 가장 새 첫 행동 시각에서 복원한다.

### 1.3 보고 유실 뒤 채움(코디네이터)

- after_run·error 체크포인트의 `first_action`이 객체이고, 그 시도(배정·세대·attempt)의 첫 행동 기록이 없고, 체크포인트의 `attempt_id`(있으면)가 첫 행동의 것과 같고, 그 세션의 저장된 인수 행이 그 시도를 부른 전달에 연결돼 있으면, 체크포인트 저장 트랜잭션에서 기록한다(검토 5dbb3d8 L-2·I-2). 본문은 recordFirstAction과 **같고**, 새 열 `source`가 `checkpoint`다. 뒤늦은 supervisor 보고는 같은 본문이므로 200이다.
- 이미 있는 기록은 바꾸지 않는다. 체크포인트의 값이 기록과 다르면 연결 조회의 `first_action_conflict`가 참이다.
- `first_action`·`delivery_id`의 형식이 틀리면 체크포인트 전체가 400 `SCHEMA_VIOLATION`이다.

### 1.4 02-08 준비(전달 행)

- `cause`(지금은 `operator`만, CHECK 제약)와 요청 시각 `requested_at`은 단계 3에 이미 있다. 결과마다 `recorded_at`이 있다.
- 새 열은 더하지 않았다. 연결 조회가 `responded_at`을 계산해 보인다. supervisor의 답 또는 그 유실을 기록한 첫 결과의 시각이다. 조건 충족·사건 수신 시각은 02-08이 더한다.

### 1.5 연결 조회 getAssignmentChain

- `GET /v0/coordinator/assignments/{assignment_id}/generations/{generation}/chain`. HTTP 전용, 읽기 전용, 트랜잭션 하나다(작업 표 `Operation::AssignmentChain`, 모양 `Transaction`). 새 파일은 `crates/ieo-coord/src/chain.rs`다.
- 자격: service·user의 `coordinator:assign` 또는 `roles:control`. agent는 403 `SCOPE_DENIED`다. 범위 밖 repo의 배정과 없는 배정·세대는 같은 404 `NOT_FOUND`다(존재 노출 없음). 세대가 음이 아닌 정수가 아니면 400이다.
- 답은 모두 DB에서 만든다. 코디네이터 재시작 뒤에도 같다.

| 절 | 내용 | 수준 |
| --- | --- | --- |
| `sprint` | sprint_id, revision, digest, `sources[]` | verified |
| `assignment` | 세대, task, 대상 SHA, revision 한도, 발급자, 발급 시각, 정책 digest, 현재 머리인지 | verified |
| `deliveries` | 요청, cause, 모든 결과(판단·연산·예산·보낸 digest·supervisor 답), `requested_at`, `responded_at`, 마지막 `outcome` | verified |
| `takeovers` | 전달, 체크포인트 순번·digest, session_id, 적용 revision·digest, 수신 시각 | claim |
| `first_actions` | attempt, 오프셋, 관측 시각, 종류, `source`, 전달 | claim |
| `checkpoints`·`tamper_events` | 이 세대의 체크포인트(각각의 `delivery_id`·`first_action`)와 변조 사건 | claim |
| `agent_reports` | 이 배정·세대의 보고 v0 claim(별도 필드) | claim |
| `session_status` | (repo, role)의 최신 세션 상태(도착 시 낡은 것 제외), 이 세대를 이름 붙였는지 | claim |
| `links` | 전달마다: 인수 여부·순번, 첫 행동(기록 / `"none"` / null=모름), 충돌 표시. `"none"`은 그 전달의 시도와 같은 `attempt_id`를 가진 체크포인트에서만 | (파생) |

- `evidence_levels`가 verified 절과 claim 절의 이름을 적는다. 코디네이터가 스스로 한 일(명세·배정·전달)만 verified다.

### 1.6 claim 분리

- 완료 판정(02-03 `judge`)은 전달·인수·첫 행동·체크포인트·세션 상태를 읽지 않는다.
- 시험은 에이전트 보고·인수·첫 행동이 하나도 없을 때 판정이 `accepted`로 나옴을 보인다. 그 뒤 claim이 모두 도착해도 같은 판정(같은 `outcome`·`facts`)이 나온다(§4).
- **관찰(사용자에게 알릴 것).** 계획 §1.1·§3.2의 “02-03 `judge`는 GitHub·실행 증거만 입력”은 정확하지 않다.
  - 수용된 정책 0.6.0의 G3는 **현재 Guardrail 배정의 `guardrail_review` claim**을 검토 입력으로 읽는다(`api.rs` `judge`의 “G3 review inputs (N4)”).
  - 그 claim은 필수가 아니다. GitHub 검토 증거로도 G3가 채워지고, 위 시험은 에이전트 보고 0건으로 `accepted`다.
  - 이 단계는 정책을 바꾸지 않았다. 개발 역할의 진행 보고와 supervisor claim은 판정 입력이 아니다.

## 2. 바꾼 계약과 코드

- `crates/ieo-coord`
  - `src/api.rs`
    - `report_checkpoint`: 전달 확인(시도 id 대조 포함), 인수 행, 첫 행동 채움, 형식 검사(`attempt_id` 포함).
    - 새 함수: `check_delivery`, `checkpoint_first_action`.
    - recordFirstAction과 공유: `insert_first_action`·`taken_over`·`fill_first_action`.
    - 작업 표에 `AssignmentChain`.
  - `src/chain.rs`(새 파일), `src/delivery.rs`(`delivery_id` 전송), `src/lib.rs`.
  - `src/schema.sql`
    - 새 표 `role_takeovers`(추가 전용 트리거).
    - `role_first_actions.source`(`ALTER … ADD COLUMN IF NOT EXISTS`, 기본 `report`).
    - 영속 DB는 다음 `migrate`에 새 표와 열만 생긴다.
  - **버그 교정.** recordFirstAction의 takeover 확인이 `query_opt`였다. 그래서 같은 세대에서 재개된 세션(takeover 행이 둘 이상)의 두 번째 회차 첫 행동이 500 `INTERNAL`이었다(단계 2부터). `EXISTS`로 고쳤다. 종단 시험이 처음 드러냈고 회귀 시험을 더했다(§4, 변이 S4-M13).
- `crates/ieo-supervisor`
  - `src/first_action.rs`(새 파일), `src/run.rs`(probe), `src/central.rs`(`record_first_action` 두 클라이언트), `src/lib.rs`.
  - `src/serve.rs`: `delivery_id` 수용·형식 검사, 체크포인트 필드(`delivery_id`·`attempt_id`·`first_action`), 보고 스레드, `last_actual_action_at`, `first_action_report`.
- `contracts/ieo-api-v0.openapi.yaml` 0.11.0 → **0.12.0**(추가)
  - getAssignmentChain과 `AssignmentChain` 스키마.
  - reportCheckpoint의 `delivery_id`·`attempt_id`·`first_action` 규칙.
  - recordFirstAction의 채움·정의 문장, deliverAssignment의 `delivery_id` 전송.
  - 코드 `DELIVERY_REQUIRED`·`DELIVERY_MISMATCH`.
  - reportSessionStatus `assignment_id`에 `minLength: 1`(I-6).
  - 머리 기록에 수용된 작업의 동작 변경을 적었다. `openapi-spec-validator` 통과.
- proto는 **바꾸지 않았다.**
  - `delivery_id`·`first_action`은 체크포인트 JSON 안이라 `ReportCheckpoint.checkpoint_json`에 실린다. `RecordFirstAction`은 단계 2에 있다.
  - proto 파일 diff는 0이고, 그래서 buf는 해당 없다.
- `contracts/ieo-internal-v0.md`
  - §1.2에 getAssignmentChain(HTTP 전용).
  - `ReportCheckpoint` 검사 순서에 전달 확인·인수 행·채움의 자리.
  - 새 §15(바꾼 문장). 새 409 코드는 §4.1 규칙대로 `FAILED_PRECONDITION`이다.
- 런타임 계약 **revision 12**: 머리 기록, §5-1 문장(체크포인트 항목에 `delivery_id`·`first_action`), §7 status 행의 참조, 새 §15 항목 15~17.
  - 항목 15: 첫 행동 정의·보고·after_run·`last_actual_action_at`.
  - 항목 16: 인수와 전달의 연결(같은 시도), 전달 없는 직접 start의 결과.
  - 항목 17: after_run으로 채움.
  - 모두 추가이고 기존 필드의 뜻은 바뀌지 않는다.
- 단계 3 보고 §5 마지막 문장 정정(L-1, §9).

## 3. 수용된 작업의 동작 변경(behaviour changes to accepted operations)

사용자에게 그대로 알릴 항목이다. 모두 닫는 쪽(더 엄격)이거나 관측 값의 정정이다. 계획 §3.2 단계 4가 정한 규칙이므로 사용자 결정이 필요한 것은 없다고 본다.

1. **reportCheckpoint(02-04 수용) — 전달 없는 takeover 거부.** 지금까지 201로 저장되던 takeover가 이제 거부된다. `delivery_id`가 없으면 409 `DELIVERY_REQUIRED`, 맞지 않는 전달이면 409 `DELIVERY_MISMATCH`다.
   - 결과: 중앙 기록이 코디네이터인 supervisor에 **전달 없이 직접 보낸 start·reconnect**(02-04 호스트 호출자 `ieo-role-call`의 경로)는 실행되지 않는다. 202로 받아들여진 뒤 takeover가 거부되어 `CentralRefused`, `spawned: false`, 예산 원장 0/0으로 끝난다(기존 02-04 규칙 그대로). 종단 시험 8단계가 확인한다.
   - 02-04 실제 회차는 중앙 체크포인트 경로를 지나지 않았다(C14). `local_only` 설정의 supervisor와 호스트 호출자 시험(`tests/caller.rs` 32개)은 영향이 없고 고치지 않고 통과한다.
   - 계획의 권고대로 fail-closed로 했다. 직접 start를 코디네이터 경로에서 계속 쓰려면 전달을 거쳐야 한다.
2. **reportCheckpoint — 새 필드의 형식 검사.** `delivery_id`·`attempt_id`(각각 null 또는 1~128바이트 문자열)와 `first_action`(null, `"none"`, 네 키 객체)의 형식이 틀리면 체크포인트 전체가 400이다. 이전에는 모르는 키를 읽지 않았다. 02-04 supervisor가 쓰는 체크포인트에는 이 키가 없어 영향이 없다.
3. **supervisor 상태 `last_actual_action_at`(02-04 수용, §7 status).** 마지막 회차의 `ended_at`이었다. 이제 관측한 첫 행동의 시각이고, 관측이 없으면 null이다(C14의 “끝난 회차 기준” 해소). 내용 없는 `assistant` 사건만 내는 02-04 가짜 런타임에서는 null이 된다. 그래서 `serve.rs` `status_reports_the_section_7_fields_and_an_honest_safe_shutdown`의 기대를 문자열에서 null로 바꿨다(시험 기대 변경 1건).
4. **supervisor start·reconnect(02-04 수용).** 선택 필드 `delivery_id`를 받는다. 있으면 안전한 id(1~64자 `[A-Za-z0-9_-]`)여야 하고, 아니면 400 `SCHEMA_VIOLATION`이다(이전에는 무시). 체크포인트 JSON에 키 셋(`delivery_id`, `attempt_id`, `first_action`)이 늘어 바이트가 달라진다. digest는 늘 바이트 기준이라 규칙은 같다.
5. 02-05 진행 중 작업(아직 수용 전)의 변경: recordFirstAction의 500 교정(§2), 기록 이력 `subject`에 `source` 추가.
6. **수용된 시험을 고친 것.** 02-04의 takeover 시험이 전달을 갖도록 시험 고정값을 더했다. 판단 기대값은 바꾸지 않았다.
   - `tests/common/mod.rs`
     - `checkpoint()`가 takeover에 `D-t-…` 전달 id를 넣는다. 시험이 직접 정하면 그 값을 쓴다. 전달 id가 있으면 `attempt_id`도 그 값으로 넣는다(고정값 전달은 id를 시도 id로 쓴다, 검토 5dbb3d8 M-1).
     - `ensure_delivery()`가 보고 직전에 그 전달 행을 DB에 넣는다. 결과 없는 전달이고, 배정이 발급된 경우만이다.
     - `Env::counts`에 `role_takeovers`를 더했다.
   - `coordinator.rs` `central_checkpoints_keep_sequence_identity_and_tamper_reports`.
   - `grpc.rs`
     - 모든 체크포인트 호출: HTTP·gRPC 양쪽, `prepare()`.
     - `supervisor_operations_answer_the_assignment_and_store_claims`의 “바뀐 표” 목록과 스냅숏 비교에 `role_takeovers`.
   - `checkpoint_e2e.rs`(02-04 종단): 직접 start·reconnect 요청마다 `delivery_id`와 고정값 전달 행(그 요청의 시도 id). 13단계의 손으로 만든 takeover에는 별도 전달과 시도 id.
   - `delivery_e2e.rs`(단계 3): 7단계의 손으로 만든 takeover에 고정값 전달과 그 시도 id. 끝 집계는 전달 19 → 20이고, 인수 6건 확인을 더했다.

## 4. 시험

새 시험(9개):

| 시험 | 내용 |
| --- | --- |
| `ieo-supervisor` 단위 `first_action::tests::the_first_action_is_the_first_tool_use_or_text_after_init` | 정의의 각 경우: 텍스트, tool_use, 건너뜀(내용 없음·rate limit·thinking·빈 텍스트·자식 사건), `init` 앞 무시, `init`만, 없음, JSON 아닌 줄도 줄 수에 듦. 실시간 probe와 사후 판독(`scan`)이 같음 |
| `first_action::tests::the_callback_runs_once_with_the_observation` | 콜백 1회, 오프셋·시각 |
| `ieo-supervisor` `tests/serve.rs` `checkpoints_carry_the_delivery_and_the_first_action` | 틀린 `delivery_id` 400(실행 디렉터리 없음), takeover·after_run의 `delivery_id`·`attempt_id`, 행동 없는 회차의 `"none"`, `last_actual_action_at` null, `delivery_id` 없는 start의 null |
| `ieo-coord` `tests/coordinator.rs` `a_takeover_is_stored_only_for_a_delivery_of_its_assignment_generation` | 거부 16가지 뒤 7개 표의 행 수 불변: 없음 2, 모름, 옛 세대 2, `not_called`·`budget_refused`·`refused`·`not_accepted`, **맞는 전달·다른 시도와 시도 없음**(검토 5dbb3d8 M-1), 형식 4(`attempt_id` 포함), 02-04 조건이 먼저인 낡은 세대. 결과 없음·`accepted`·`unknown` 전달은 저장. 인수 행 내용, 중복 200, 두 번째 인수 409와 체크포인트 롤백(행 수 불변) |
| `a_lost_first_action_report_is_filled_from_the_run_checkpoint` | after_run으로 채움(`source: checkpoint`, 같은 본문). 뒤늦은 같은 보고 200, 다른 내용 409(행 수 불변). 보고가 먼저면 after_run이 바꾸지 않고 충돌 표시. `"none"`. 재개 세션의 두 번째 회차 첫 행동 201(500 교정 회귀). 채움 조건(검토 5dbb3d8 L-2·I-2): 인수되지 않은 세션, 그 세션의 전달이 부르지 않은 시도, 체크포인트의 시도와 다른 첫 행동은 체크포인트만 저장되고 첫 행동 행 0, 연결 조회에도 없음. 형식 틀린 `first_action` 6가지 400(행 수 불변) |
| `the_assignment_chain_is_read_by_clients_of_its_repo_only` | 401. 403: supervisor·모델·user·ingest 자격. 404: 범위 밖, 없는 세대·배정. 400: 세대 형식 4가지. `operator`·`assigner` 200과 빈 연결 |
| `claims_are_kept_apart_from_the_completion_judgment` | 에이전트 보고·인수·첫 행동 0건으로 판정 `accepted`, 연결 조회 진행. 그 뒤 인수·첫 행동(after_run 채움)·개발 진행 보고·세션 상태가 도착해도 다음 판정의 `outcome`·`facts`가 같음. 연결 조회의 수준 표시(verified 3절, claim 6절) |
| `tests/chain_e2e.rs` `the_chain_links_delivery_takeover_and_first_action`(HTTP)·`…_over_grpc` | 코디네이터 바이너리, 실제 PostgreSQL, supervisor 서비스(가짜 `claude`), 공유 예산 검사. 아래 표 |

종단 시험 단계(두 전송 같음):

| 단계 | 확인 |
| --- | --- |
| 0 | 전달 전 연결 조회는 빈 목록. supervisor·user 403, 다른 repo 404, `assigner` 200 |
| 1 정상 연결 전체 | 전달(start, verified, `requested_at`·`responded_at`) → 인수(순번 1, s1, 적용 revision 5·digest) → 첫 행동(오프셋 2, `tool_use`, `source: report`, 보고 성공) → after_run(같은 값). 상태 `last_actual_action_at` = 관측 시각 ≠ `ended_at`. 같은 첫 행동 200, 다른 내용 409, 행 수 불변 |
| 2 요청만으로 인수 없음 | supervisor가 거부한(자격 파일 없음) 전달은 연결에 `taken_over: false`, 첫 행동 null. 이 전달·전달 없음·모르는 전달·이미 인수된 전달을 이름 붙인 takeover는 409(`DELIVERY_MISMATCH`/`DELIVERY_REQUIRED`), 행 수 불변 |
| 3 행동 없는 회차 | `init` 앞 텍스트와 내용 없는 사건만 낸 reconnect: after_run `"none"`, 첫 행동 기록 없음, 연결 `"none"`. 인수 행의 session_id는 체크포인트의 s1(전달 본문은 s9, C1). `last_actual_action_at`은 1단계 값 그대로 |
| 4 낡은 세대 전달 뒤 새 세대 인수 | 세대 1 발급 뒤 세대 0 전달 409 `STALE_ASSIGNMENT`. 세대 0 전달을 이름 붙인 세대 1 takeover 409 `DELIVERY_MISMATCH`(행 수 불변). `new_session` 전달(start, `text` 첫 행동)이 세대 1의 유일한 전달·인수. 세대 0 연결은 전달 3·인수 2 그대로, `current: false` |
| 5 보고 유실 뒤 채움 | 인수 저장을 확인한 뒤 supervisor의 코디네이터 자격 파일을 치움 → 첫 행동 보고 실패(`ok: false`), 회차는 계속 → 자격을 되돌린 뒤 after_run이 기록을 채움(`source: checkpoint`). 뒤늦은 같은 보고 200, 행 수 불변 |
| 6 재시작 | 코디네이터 재시작 뒤 세대 0·1 연결 조회가 같은 JSON |
| 7 검토 fe924b7 I-2 | 트랜잭션 2 전에 멈춘 전달(고정값: 결과 없음, 10분 전 요청)과 그 attempt를 받아들인 supervisor: 같은 키 재전송의 재조회가 `unknown`(“no recorded request digest”), `accepted` 아님 |
| 8 전달 없는 직접 start | 202 뒤 takeover 409 `DELIVERY_REQUIRED` → `CentralRefused`, `spawned: false`, 실행 파일 없음, 7개 표 행 수 불변 |
| 끝 | 저장된 takeover 중 인수 행 없는 것 0건, 인수 행 5건(a1·a3·a4·a5·a6) |

결과([evidence/test.log](evidence/test.log), 커밋 40e2c98. 검토 5dbb3d8 교정은 기존 시험 안의 사례로 더했으므로 시험 수는 같다):
- `cargo fmt --all -- --check`, `cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음(buf 해당 없음).
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **376 통과, 0 실패, 0 무시**.
  - 기준(main 375f21f) 367에 새 시험 9개다.
  - 02-04 호출자 시험 32개는 고치지 않고 통과했다.
- `validate_package.py`: §10.

## 5. 변이 시험

각 변이를 하나씩 1b7f51c 코드(S4-M9·M17·M18은 40e2c98)에 넣고 시험 대상을 차례로 돌린 뒤 파일을 되돌렸다([evidence/mutants.txt](evidence/mutants.txt)). 대상은 코디네이터 단계 4 시험 4개(`--test coordinator`), `--test chain_e2e`, supervisor `--lib first_action`·`--test serve`다. 첫 대상이 잡으면 다음 대상은 돌리지 않았다. 처음 17개를 교정 뒤 코드에서 다시 돌리고 교정의 4개(S4-M16~M19)를 더했다. **21개 모두 잡혔다.** S4-M18은 1b7f51c에서 살아남았다(채움 시험 (c)가 인수 연결 조건에도 걸려 있었다). 40e2c98에서 (c)를 체크포인트 시도 조건에만 걸리게 고친 뒤 잡혔다

| id | 변이 | 잡은 시험 |
| --- | --- | --- |
| S4-M1 | takeover의 전달 확인 제거 | `a_takeover_is_stored_only_for_a_delivery_of_its_assignment_generation` |
| S4-M2 | 다른 배정·세대의 전달 허용 | 같음 |
| S4-M3 | 보낸 것이 없는 전달(`not_called` 등) 허용 | 같음 |
| S4-M4 | 인수 행 충돌 무시(인수 행 없이 체크포인트 저장, “같은 트랜잭션” 깨짐) | 같음 |
| S4-M5 | 인수 행을 쓰지 않음 | 위 + `a_lost_first_action…`, `claims_are_kept…` |
| S4-M6a | `init` 앞 사건도 행동으로 셈 | `first_action::tests::the_first_action_is…` |
| S4-M6b | 내용 없는 `assistant`도 행동 | 단위 2개 |
| S4-M6c | `init` 자체가 첫 행동 | 단위 2개 |
| S4-M7 | 행동 없음을 오프셋 0·시작 시각으로 채움 | `checkpoints_carry_the_delivery_and_the_first_action` |
| S4-M8 | after_run에 첫 행동을 싣지 않음 | `chain_e2e` 두 시험 |
| S4-M9 | 체크포인트로 채우지 않음 | `a_lost_first_action…`, `claims_are_kept…` |
| S4-M10 | 연결 조회에서 takeover를 verified로 표시 | `claims_are_kept…` |
| S4-M11 | 범위 밖 repo의 연결 조회 허용 | `the_assignment_chain_is_read_by_clients_of_its_repo_only` |
| S4-M12 | `last_actual_action_at`을 회차 끝 시각으로 되돌림 | `checkpoints_carry…`, `status_reports_the_section_7_fields…` |
| S4-M13 | takeover 확인을 `query_opt`로 되돌림(재개 세션 500) | `a_lost_first_action…` |
| S4-M14 | I-2: digest 없는 재조회를 `accepted`로(`is_none_or`, 검토 X3) | `chain_e2e` 두 시험 |
| S4-M15 | 전달이 `delivery_id`를 보내지 않음 | `chain_e2e` 두 시험 |
| S4-M16 | takeover의 시도 대조 제거(전달 id만 이름 붙인 start가 전달을 소비) | `a_takeover_is_stored_only…` |
| S4-M17 | 채움의 인수 연결 조건 제거(검토의 변이 D) | `a_lost_first_action…` |
| S4-M18 | 채움이 체크포인트의 시도를 보지 않음 | `a_lost_first_action…`(40e2c98) |
| S4-M19 | supervisor가 `attempt_id`를 쓰지 않음 | `checkpoints_carry…` |

## 6. 이월 항목

- **C1 닫음.** 코드 검토 기록:
  - 재개 키는 체크포인트의 session_id 하나다. `checkpoint::decide`(`crates/ieo-supervisor/src/checkpoint.rs`)는 `Reconnect::Resume { session_id }`를 체크포인트의 `session_id`에서만 만든다.
  - supervisor `launch`의 reconnect 경로는 그 값만 `Job.session_id`로 쓴다(`serve.rs`). 그 체크포인트는 실행 전에 중앙 기록과 순번·digest가 같아야 한다(`central::compare`).
  - 요청 본문의 `session_id`는 start의 새 세션 id로만 쓴다. `new_session_id`는 변조 뒤 새 세션 id로만 쓴다.
  - `args::build`의 `--resume`은 그 session_id이고, `-n`은 고정 표시 이름(`<repo>-<role>`)이다. 논리 이름(`logical_name`)은 코디네이터 전달 기록에만 있고 supervisor에 보내지 않는다(`delivery.rs`의 `Call`에 없음).
  - 인수 행의 session_id는 저장된 takeover 체크포인트의 값이다. 전달 요청의 `session_id`가 아니다.
  - 시험(코디네이터 종단): `delivery_e2e`의 두 시험이 4단계에서 논리 이름만 바꾼 전달이 같은 s1로 재개됨을, 6단계에서 이름이 같고 session_id만 바꾼 로컬 기록은 외부 대조로 부르지 않음을 보인다. `chain_e2e`의 두 시험(3단계)은 전달 본문의 session_id `s9`와 상관없이 체크포인트의 s1로 재개되고 인수 행이 s1을 적음을 보인다.
  - 시험(supervisor): `role_arguments_use_edit_paths_and_session_id_only`, `reconnect_decisions`, `reconnect_resumes_an_untouched_session_and_starts_anew_after_tamper`.
- **C14 일부 해소.** `last_actual_action_at`이 끝난 회차가 아니라 관측한 첫 행동이다(§3 항목 3).
  - 남는 것: `worktree_head`·외부 요청 미추적(단계 6·7), 실제 회차의 중앙 경로(단계 8), 회차 안 두 번째 이후 행동의 시각(§8).
- **수용 기준 본문.** 명세 revision·digest·출처 → 배정 → 전달 → 실제 인수 → 첫 행동 → 체크포인트가 한 조회로 연결된다. 에이전트 기록은 claim으로 별도 필드에 남고 판정의 선행 신호가 아니다(§1.6의 관찰 포함).
- **단계 2·3 이월.** 세션 상태 조회 경로는 연결 조회의 `session_status`로 닫았다.

## 7. 계획과 다른 점, 결정이 필요한 것

- **“결과가 기록됐거나 `unknown`”의 구체화.** 결과가 아직 없는 전달도 인수를 받는다(§1.1). 다만 그 takeover는 **그 전달이 부른 시도**의 것이어야 한다(검토 5dbb3d8 M-1로 더함).
  - 그래서 결과가 나중에 `refused`·`not_accepted`가 될 수는 없다(예외: 어댑터 자격으로 전달·시도 id를 베낀 직접 start가 먼저 닿은 경우. 그 전달은 인수와 함께 `refused`로 남는다, §8. 병합 뒤 정정, 검토 debf413 I-7). 시도 id가 같은 takeover는 그 전달 자신의 start·reconnect를 supervisor가 받아들였을(202) 때만 생긴다. supervisor는 같은 시도 id로 다른 요청을 받아들이지 않는다(`ID_CONTENT_CONFLICT`, 런타임 계약 §14 항목 11). 202를 받으면 코디네이터는 `accepted`, 답을 잃으면 `unknown`을 기록한다.
  - 처음 제출본에는 이 대조가 없었다. 전달 id만 이름 붙인 다른 시작(직접 start, 새 시도 id로 다시 보낸 도구)이 결과 없는 전달을 먼저 소비할 수 있었고, 그러면 그 전달 자신의 호출이 `refused`·`not_accepted`가 되거나 자기 takeover가 거부되었다(검토 M-1의 시나리오).
  - 코디네이터가 트랜잭션 2 전에 멈추면 결과 없는 전달에 인수가 남는다. 같은 키의 재조회가 120초 뒤 `unknown`(digest 없음, 종단 7단계)을 기록한다.
- **전달 하나에 인수 하나.** 계획에는 없던 제약이다. 같은 전달 id를 다른 start에 다시 쓰는 것을 막는다. 같은 바이트의 재보고(답 유실 뒤)는 중복으로 그대로다.
- **첫 행동 채움.** 계획의 “after_run 체크포인트에 같은 값을 넣어 다음 보고로 채운다”를, 코디네이터가 체크포인트 저장 때 채우는 것으로 구현했다. supervisor가 따로 재전송하지 않는다. 체크포인트 보고에는 이미 보관 사본·재전송 경로가 있어서다.
- 사용자 결정이 필요한 것은 없다고 본다. §3의 동작 변경과 §1.6의 관찰은 보고한다.

## 8. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 검토 fe924b7 I-4 | supervisor의 start 경로는 스스로는 끝나지 않은 체크포인트를 거부하지 않는다. 다만 중앙 기록이 코디네이터이면 이제 start에 전달이 있어야 하고(takeover 거부), takeover는 그 전달이 부른 시도의 것이어야 하며(검토 5dbb3d8 M-1), 전달은 `judge_resume`을 거친다(끝나지 않은 회차는 부르지 않음). 남는 틈: `local_only`(시험용), 그리고 어댑터 자격을 가진 쪽이 아직 supervisor에 닿지 않은 전달의 **전달 id와 시도 id를 함께** 베껴 먼저 직접 start하는 경우다. 그 경우 그 전달 자신의 호출은 같은 시도 id·다른 내용이라 409 `ID_CONTENT_CONFLICT`(`refused`)가 되고, 연결 조회에는 인수가 있는 `refused` 전달로 드러난다(조용히 통과하지 않음). 어댑터 자격을 가진 쪽은 런타임 계약 §2.1의 신뢰 경계 안이다. supervisor `start`에 phase 거부를 직접 넣는 것은 `new_session` 의미와 02-04 시험 여럿에 닿아 이 PR에서 하지 않았다 | 02-05 단계 6(같은 start 경로를 고침) |
| 검토 fe924b7 I-5 | HTTP 경로에 `statement_timeout`이 없다. 이 단계는 takeover 트랜잭션에 전달 잠금 하나를 더했다. 전달 쪽은 그 잠금을 짧은 트랜잭션 1·2·재조회에서만 쥐므로 120초 판단은 그대로 성립한다 | 02-05 단계 5(코디네이터가 회차 stream을 받아 계측하는 긴 작업이 생기는 단계). `IN_FLIGHT`와 함께 다시 본다 |
| 마지막 행동 시각 | `last_actual_action_at`은 회차마다 첫 행동만 반영한다. 같은 회차의 뒤 행동은 시각을 바꾸지 않는다 | 02-08(사건 기반 갱신) |
| 세션 상태 보고 | supervisor는 reportSessionStatus를 아직 스스로 보내지 않는다. 연결 조회의 `session_status`는 보고된 것만 보인다 | 02-08(주기·사건 보고) |
| 수용된 시험의 고정값 | 02-04 체크포인트 시험은 이제 고정값 전달(결과 없음)을 쓴다. 전달 규칙 자체는 실제 전달(종단)과 단계 4 시험이 덮는다 | — |
| 시각 표기 | 연결 조회는 DB 시각을 마이크로초(`.123000Z`)로, 체크포인트·supervisor는 밀리초(`.123Z`)로 적는다. 같은 순간이다 | — |
| 판정과 claim | §1.6의 G3 관찰. 정책 변경은 사용자 결정 | 사용자 보고 |
| 첫 행동의 종류(검토 5dbb3d8 I-3) | `tool_use`·`text` 블록만 행동으로 센다. 실제 CLI가 다른 블록(예: `server_tool_use`)을 내면 행동으로 세지 않는다 | 단계 8 확인 항목(실제 stream 대조, 계획 §4.1 (1)) |
| 재시작 뒤 복원(검토 5dbb3d8 I-4) | `last_actual_action_at`은 재시작 때 `outcome.json`에서만 복원한다. 회차 결과를 쓰기 전에 죽은 회차에서 본 첫 행동은 복원된 상태에서 빠진다. 보고됐다면 코디네이터 기록에는 있다 | 02-08과 함께 |
| 02-04 예비 회차(검토 5dbb3d8 I-5) | 사용자 결정 기록용: 남은 02-04 예비 회차 5회는 이제 중앙 기록이 코디네이터인 supervisor에 `ieo-role-call`로 직접 쓸 수 없다. `local_only` 설정이나 전달(deliverAssignment)을 거쳐야 한다 | 사용자 보고 |
| 충돌 표시(검토 5dbb3d8 I-1) | 연결 조회의 `first_action_conflict`는 오프셋과 종류만 비교한다. `observed_at`만 다른 경우는 표시하지 않는다(밀리초·마이크로초 표기 차이 때문) | 선택 개선, 담당 없음 |

## 9. 검토 잔여 처리(PR #74 검토 fe924b7)

| 항목 | 처분 |
| --- | --- |
| L-1 단계 3 보고 §5 마지막 문장 | 정정. “기존 작업의 HTTP 동작은 compareCheckpoint 판단 하나(닫는 쪽) 말고는 바꾸지 않았다”와 정정 기록 |
| I-2 digest 없는 재조회 시험 | 더함. `chain_e2e` 7단계(두 전송). 검토의 X3(`is_none_or` 복원)는 이제 잡힌다(S4-M14) |
| I-4 supervisor start의 미완료 체크포인트 | 코디네이터 쪽 전달 요구로 막힘. 남는 틈과 담당은 §8 |
| I-5 HTTP `statement_timeout` | 넘김(§8) |
| I-6 reportSessionStatus `assignment_id` | OpenAPI `minLength: 1`. 코드는 이미 빈 값을 400으로 거부(`str_field`) |

## 10. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험), [evidence/mutants.txt](evidence/mutants.txt).
- `docs/poc/scripts/validate_package.py`: `schema_errors` 0으로 통과(커밋 40e2c98, evidence/test.log에 기록).
- 시험 DB(55432) 정리: 이 작업이 남긴 데이터베이스를 이름으로 지웠다. 실패한 중간 실행의 `ieo_t_chain_*`·`ieo_t_cpe2e_*` 5개, 잡힌 종단 변이 실행의 `ieo_t_chain_*` 6개, 교정 1의 변이 실행이 남긴 `ieo_t_chain_*` 6개다. 끝난 뒤 `ieo_t_*`는 0개다. 55433·58090은 건드리지 않았다.

## 11. 검토 반영(PR #75 검토 5dbb3d8)

교정 커밋 1b7f51c(코드·계약·시험)와 40e2c98(채움 시험 (c)를 체크포인트 시도 조건만 걸리게 고침). 다시 만든 증거는 8717b88(evidence/test.log 첫 줄이 40e2c98).

| 항목 | 처분 | 바꾼 것 | 시험·변이 |
| --- | --- | --- | --- |
| M-1 takeover가 전달의 호출에 묶이지 않음 | 교정 | supervisor가 모든 체크포인트에 `attempt_id`를 쓴다. `check_delivery`가 `role_deliveries.attempt_id = 체크포인트 attempt_id`를 요구한다(다르거나 없으면 409 `DELIVERY_MISMATCH`, 행 없음). 세션 일치는 요구하지 않았다: reconnect는 체크포인트의 세션으로, 변조 뒤 reconnect는 `new_session_id`로 재개하므로 전달의 `session_id`와 takeover의 세션이 정상적으로 다를 수 있다. 시도 일치로 충분하다(같은 시도 id·다른 내용은 supervisor가 거부). §7·§8 문장 정정. OpenAPI 0.12.0 문장, 런타임 계약 revision 12 §15 항목 16, 내부 계약 §1.2·§15 | `a_takeover_is_stored_only…`: 맞는 전달·다른 시도, 시도 없음 409, `attempt_id` 형식 400, 행 수 불변. `checkpoints_carry…`: takeover·after_run의 `attempt_id`. 변이 S4-M16(대조 제거), S4-M19(supervisor가 쓰지 않음) |
| L-1 “claim은 판정 입력이 아님” 문장 | 교정 | OpenAPI getAssignmentChain 설명과 `chain.rs` 모듈 문서: claim은 필수 입력이 아니고, 판정이 읽는 claim은 현재 Guardrail의 `guardrail_review` 하나(G3, 정책 0.6.0, 이 보고 §1.6)이며, supervisor claim은 판정 입력이 아니다 | — |
| L-2 채움의 takeover 조건 시험 없음 | 교정 | 채움 조건을 “그 세션의 인수 행이 그 시도를 부른 전달에 연결됨”으로 좁혔다(I-2와 함께) | `a_lost_first_action…` (a) 인수되지 않은 세션 (b) 세션의 전달이 부르지 않은 시도 (c) 체크포인트의 시도와 다른 첫 행동: 첫 행동 행 0, 연결 조회에 없음. 검토의 변이 D에 해당하는 S4-M17, S4-M18 |
| I-1 충돌 비교가 오프셋·종류만 | 기록 | §8 | — |
| I-2 after_run의 `delivery_id`·시도 대조 | M-1과 함께 교정 | 채움은 체크포인트의 `attempt_id`(있으면)가 첫 행동의 것과 같고 세션의 인수 행이 그 시도의 전달에 연결될 때만. 연결 조회의 `"none"`도 그 전달의 시도와 같은 `attempt_id`의 체크포인트에서만 | L-2 시험 (b)(c)(`"none"` 링크는 이 시험이 덮지 않음, 시험은 단계 5 PR, §12 L-3) |
| I-3 실제 stream의 다른 블록 | 기록 | §8(단계 8 확인 항목) | — |
| I-4 재시작 뒤 복원 | 기록 | §8 | — |
| I-5 02-04 예비 회차 | 기록 | §8(사용자 보고) | — |
| I-6 `minLength: 1` | 조치 없음 | 코드가 이미 거부하던 값이라 문서만 맞춘 것이다 | — |

교정 뒤 결과: fmt·clippy·openapi-spec-validator 통과, `cargo test --workspace --locked` **376 통과, 0 실패**(새 사례는 기존 시험 안에 더함), `validate_package.py` 통과, 변이 21개 모두 잡힘(§5). 공유 대상 디렉터리에 다른 작업 트리에서 빌드된 `ieo-evidence` 시험 바이너리가 남아 있어 첫 실행에서 fixture 경로 실패가 났다(단계 3 보고 §7.3과 같은 현상). 작업 공간 일곱 패키지를 `cargo clean -p` 한 뒤 다시 돌려 통과했다. 코드 문제는 아니다.

## 12. 병합 뒤 기록(PR #75 검토 debf413)

검토 2(debf413)는 approve였고, 그 head로 병합했다(1b5bc1f). 남은 지적의 처분이다. 이 절과 §7의 괄호는 병합 뒤 원장 기록 PR에서 더했다.

| 항목 | 처분 | 담당 |
| --- | --- | --- |
| L-3 연결 조회 `"none"` 링크의 시도 조건이 시험되지 않음(변이 F 생존). §11 I-2 행의 “L-2 시험 (b)(c)” 근거는 `"none"`을 덮지 않는다 | 다음 코드 PR(02-05 단계 5)에 시험을 더한다: 다른 시도의 after_run이 전달 D와 `first_action: "none"`을 실으면 `links[D].first_action`은 null. 변이 F를 그 PR의 변이 표에 넣는다 | 02-05 단계 5 PR |
| I-7 §7이 §8보다 단정적 | §7에 예외를 적음(이 기록) | — |
| I-8 직접 첫 행동 보고와 채움의 조건 차이 | 기록. 둘 다 claim이고 시도 id가 맞지 않는 직접 보고는 어느 전달에도 연결되지 않는다 | 02-05 단계 6 또는 8에서 같은 조건으로 맞출지 검토 |
| I-9 전달 요청과 바이트가 같은 베낀 start도 `refused`로 기록 | 기록. 단계 3의 기존 동작(`caller::start_with`가 202 외 응답을 `Refused`로 봄), 신뢰 경계 안 | §8 I-4 남은 틈과 함께 02-05 단계 6 |
