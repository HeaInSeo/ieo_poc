# 02-05 단계 5 · 배정 수준 시간·토큰 한도(1a, C7)

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 5([계획](../plan-20261115/PLAN.md) §3.2 단계 5). 이월 C7(02-04 수용 항목 1a)을 닫는다 |
| 착수 근거 | 단계 4 병합(PR #75, main 1b5bc1f). 원장 PR #76(7dcb2fb) 위로 다시 쌓음 |
| 사용자 결정 | [사용자 결정 2026-11-16](../../../../User_Decisions_2026-11-16_l01-step5.md) (L01 토큰 정의 (가), 시간 정의 (i)): `total_tokens` = input + output + cache_creation(cache_read 제외), `elapsed_seconds` = 배정 회차 시간의 합(벽시계 아님), 둘 다 배정의 모든 세대 합계. 이 결정은 **정의만** 정한다. L01의 값은 정하지 않았고, 실제 스프린트는 `total_tokens = null`(등록 검사 `UNBOUND_DRAFT`) 그대로다 |
| 검토 반영 | PR #75 검토 debf413 L-3(시험·변이 F), PR #74 검토 fe924b7 I-5(HTTP `statement_timeout`) |
| 기준 커밋 | main 7dcb2fb. 구현 커밋 43fad72, 증거 커밋 be95fc2(evidence/test.log 첫 줄이 43fad72, 작업 트리 깨끗) |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 영속 PoC DB(55433)·58090 코디네이터는 쓰지 않았다(일회성 시험 DB 55432만) |
| 상태 | 구현과 시험 완료, **독립 검토 전**. 원장(`state.json`)은 고치지 않았다 |

## 1. 더한 작업과 바뀐 동작

### 1.1 배정의 한도(issueAssignment)

- 요청에 선택 필드 `limits {elapsed_seconds, total_tokens}`를 더했다. 각 값은 null 또는 1 이상의 정수다(다른 키·0·형식 오류는 400 `SCHEMA_VIOLATION`).
- 없거나 null인 값은 명세 revision의 `limits` 값을 쓴다. 요청 값이 revision 값보다 크거나 revision에 값이 없으면 422 `LIMITS_ABOVE_SPEC`(새 코드)다. 숫자를 새로 정하지 않는다(정책 `numeric_values`).
- 실제로 적용할 한도를 세대마다 `assignments.limits`(새 열, `ALTER … ADD COLUMN IF NOT EXISTS`)에 저장한다. 열이 없던 옛 세대는 revision 값으로 읽는다.
- 배정 전체에 적용하는 한도는 **현재 머리 세대**의 것이다. 소비량은 모든 세대 합계다.

### 1.2 소비량(정산)

- 회차의 측정은 단계 3의 공유 예산 라이브러리가 그대로 한다. 회차가 끝나면 코디네이터가 역할 서비스의 stream과 자식 전사를 받아 02-01 계측기로 회차 몫을 계산하고(재개는 누적 차분, 자식은 전사 기준, 런타임 계약 §11·§12) 원장(`records.jsonl`)에 기록한다.
- 새 모듈 `crates/ieo-coord/src/limits.rs`가 원장 기록을 DB로 옮긴다(`assignment_run_usage`, 추가 전용). 행에는 네 토큰 수(input·output·cache_creation·cache_read), 시간(`duration_ms`를 초로 올림), 상태(`measured`/`unmeasured`), 원장 기록의 digest가 있다.
- **원장이 권한이다.** 라이브러리는 보낸 모든 호출(계측됨, 미계측, 사람 확인을 요청하는 답 유실)의 기록을 원장 잠금을 놓기 전에 쓴다. 그래서 정산은 원장이 쉬고 있을 때(잠금 비어 있음, 진행 중 표식 없음)만 한다(`caller::records_when_idle`). 원장에 기록이 없는 시도는 보내지 않은 것이다.
  - 사람이 원장을 보정하면(보정 기록) 그 시도의 기록 digest가 바뀌어 새 행이 생긴다. 가장 새 행이 센다.
  - 사람이 기록을 지우면 `withdrawn` 행이 생겨 그 회차는 더 세지 않는다(원장 따름).
- 정산 시점은 두 번이다.
  - 전달 요청을 기록하기 **전**, 별도 트랜잭션에서. 정산된 사실은 전달의 답과 상관없이 남는다.
  - 예산 검사가 원장 잠금을 쥔 뒤, 그 검사가 읽은 기록으로 다시. 그 사이에 끝난 회차도 반영된다.
- 합계: `total_tokens` = input + output + cache_creation, `elapsed_seconds` = 회차 시간의 합. cache_read는 기록만 하고 세지 않는다.

### 1.3 집행(deliverAssignment)

- 요청을 기록하기 전(트랜잭션 1 안, 배정·세대·시도 확인 다음)에 배정의 상태를 본다.
  - 미계측 회차가 하나라도 있으면 409 `ASSIGNMENT_USAGE_UNMEASURED`(새 코드). 0으로 세지 않는다.
  - `total_tokens`가 남지 않았거나, 남은 `elapsed_seconds`가 실행 감시의 정지 유예(6초) 이하이면 409 `ASSIGNMENT_LIMIT_REACHED`(새 코드). 유예 이하의 창으로는 회차를 시작할 수 없기 때문이다(단계 3의 `lower`와 같은 기준).
  - 거부는 전달·결과 행을 남기지 않는다. 한도 사건 하나만 남는다(`assignment_limit_events`, (세대, 코드, 정산된 회차 수)마다 한 행이라 같은 거부를 되풀이해도 늘지 않는다). 답에 `usage`와 `operational_state: awaiting_human_decision`이 있다.
  - 결정 요청(decision_request)은 만들지 않는다. 정책 G5의 범위를 넓히는 일이라 새 정책 버전이 필요하다(계획 그대로).
- 통과하면 지금처럼 판단·예산 검사를 하고, 예산 잠금을 쥔 뒤 다시 정산해 회차의 창과 billable 여유를 배정의 남은 양으로 낮춘다(`Admission::lower`, 올리지 않음). 결과 한도는 배정 남은 양, 승인 예산 남은 양, 역할 몫 중 가장 작은 값이다. 이 계산은 코디네이터 안, 역할 컨테이너 밖이다.
  - 여기서 처음 드러난 한도(경합)는 `budget_refused`(사유 “assignment limits: …”)로 기록한다.
  - 단계 3의 “revision 한도로 회당 상한을 자름”은 이 남은 양으로 바뀌었다(배정 한도 ≤ revision 한도라 더 엄격하거나 같다).
- 승인된 02-05 예산 (나)는 연결하지 않았다. 예산 이름·상한 코드는 계획대로 별도 PR이다. 시험의 역할 몫은 시험 고정값이다.

### 1.4 조회

- getAssignmentChain에 `usage`(한도, 회차 수, 미계측 회차 수, 시간·토큰 합계와 네 토큰 수, 남은 양, `operational_state` = `open`/`awaiting_human_decision`), `usage_runs`(전달마다 가장 새 정산 행), `limit_events`를 더했다. 모두 코디네이터 자신의 기록이라 verified다. 정산은 전달 때 일어나므로, 끝났지만 아직 정산되지 않은 회차는 다음 전달 전까지 보이지 않는다.
- getOwnAssignment의 `limits`는 이제 배정 자신의 한도다(proto 변경 없음, 같은 필드).

### 1.5 HTTP 문장 상한(검토 fe924b7 I-5)

- HTTP 경로의 모든 트랜잭션에 `SET LOCAL statement_timeout = 10 s`를 둔다(`api::HttpGuard`, `HTTP_STATEMENT_TIMEOUT`). advisory 잠금 대기도 포함한다. 넘으면 그 작업은 500(“statement timeout or cancellation (SQLSTATE 57014); nothing was stored”)이고 트랜잭션은 롤백된다.
- 120초 `DELIVERY_IN_PROGRESS` 판단은 그대로 성립한다. 전달이 결과를 기록하기 전까지 쓰는 짧은 트랜잭션은 넷(정산은 다섯째이지만 잠금 대기가 없는 짧은 것)이라 DB 부분은 40초 안팎이다. 여기에 상태 조회 2번(각 10초), 원장 잠금 대기 3초, start 호출 30초를 더해 최악 약 95초로 120초보다 작다.
- 이 단계의 긴 일(회차 끝 기다림·계측)은 트랜잭션 밖 배경 스레드에서 돈다.

### 1.6 검토 debf413 L-3

- 다른 시도의 뒤 체크포인트가 전달 D를 이름 붙이고 `first_action: "none"`이면 D의 연결 `first_action`은 null(모름)로 남는다. 시험은 `a_lost_first_action_report_is_filled_from_the_run_checkpoint` 끝에 더했고, 변이 F(`chain.rs`의 시도 대조 제거)가 잡힌다.

## 2. 바꾼 계약과 코드

- `crates/ieo-coord`
  - `src/limits.rs`(새 파일): 정산, 상태, 여유, 한도 사건.
  - `src/delivery.rs`: 요청 전 정산과 거부, 예산 잠금 뒤 정산과 한도 낮추기.
  - `src/api.rs`: 배정 `limits`(형식·`effective_limits`), getOwnAssignment, `HttpGuard`, SQLSTATE 57014 답.
  - `src/chain.rs`: `usage`·`usage_runs`·`limit_events`.
  - `src/schema.sql`: `assignments.limits` 열, `assignment_run_usage`·`assignment_limit_events`(추가 전용 트리거). 영속 DB는 다음 `migrate`에 새 열·표만 생긴다.
- `crates/ieo-supervisor/src/caller.rs`: `records_when_idle`, `Admission::records`(읽기 전용 추가).
- `contracts/ieo-api-v0.openapi.yaml` 0.12.0 → **0.13.0**: issueAssignment `limits`, deliverAssignment의 정산·거부 문장과 409 코드, getAssignmentChain `usage`·`usage_runs`·`limit_events`, 코드 셋(`LIMITS_ABOVE_SPEC`, `ASSIGNMENT_LIMIT_REACHED`, `ASSIGNMENT_USAGE_UNMEASURED`), 머리 기록의 동작 변경. `openapi-spec-validator` OK.
- proto·내부 계약·런타임 계약은 바꾸지 않았다(새 작업은 HTTP 전용 코디네이터 작업이고 supervisor 쪽 변경이 없다).

## 3. 수용된 작업의 동작 변경

사용자에게 알릴 항목이다. 모두 닫는 쪽이거나 추가다.

1. **모든 HTTP 작업(02-03·02-04 수용 작업 포함) — 문장 상한 10초.** 지금까지는 잠금을 무한히 기다렸다. 이제 한 문장(잠금 대기 포함)이 10초를 넘으면 500이고 아무것도 저장되지 않는다. 정상 부하에서는 일어나지 않는다. 57014의 500 메시지가 구체적이 됐다(gRPC는 지금처럼 `DEADLINE_EXCEEDED`·`CANCELLED`).
2. **issueAssignment(02-03 수용) — `limits` 필드 추가.** 이전에는 모르는 키라 400이던 요청이 이제 받아들여질 수 있다. revision보다 큰 값은 422 `LIMITS_ABOVE_SPEC`. 없는 경우의 동작은 같다.
3. 02-05 진행 중 작업(수용 전)의 변경:
   - deliverAssignment: 미계측 회차나 한도 도달은 요청을 기록하기 **전에** 409다(이전에는 요청을 기록하고 `budget_refused`). 회차 한도는 revision 한도 대신 배정의 남은 양이다. 그래서 단계 3 종단 시험 `delivery_e2e`의 마지막 단계 기대를 바꿨다: 답 유실로 미계측이 된 뒤의 전달은 `budget_refused`(201) 대신 409 `ASSIGNMENT_USAGE_UNMEASURED`, 끝 집계는 전달 20 → 19, 결과 22 → 21, 한도 사건 1.
   - getOwnAssignment `limits`: revision 값 대신 배정 자신의 값.
   - getAssignmentChain: `evidence_levels.verified`에 세 절 추가(`claims_are_kept…` 시험 기대 변경).

## 4. 시험

새 시험(3개)과 사례:

| 시험 | 내용 |
| --- | --- |
| `crates/ieo-coord/tests/limits_e2e.rs` `assignment_limits_bound_and_stop_deliveries`(HTTP)·`…_over_grpc` | 코디네이터 바이너리, 실제 PostgreSQL, supervisor 서비스(가짜 `claude`), 공유 예산 검사. 아래 표 |
| `tests/coordinator.rs` `an_http_statement_waits_at_most_the_statement_timeout` | 체크포인트 잠금을 쥔 동안의 보고가 9~30초 사이에 500(57014), 행 수 불변. 잠금을 놓은 뒤 201 |
| `a_lost_first_action_report_is_filled_from_the_run_checkpoint`(사례 추가) | 검토 debf413 L-3 |

종단 시험 단계(두 전송 같음. 역할 몫: 8회, billable 120 — 시험 고정값):

| 단계 | 확인 |
| --- | --- |
| 1 | 배정 `limits` 형식·한도: `total_tokens` 100,001·`elapsed_seconds` 3,601 → 422 `LIMITS_ABOVE_SPEC`, 0·모르는 키·숫자 → 400, 행 수 불변. A1/0을 `total_tokens` 40으로 발급, 연결 조회의 한도 40·3,600, `open` |
| 2 | 첫 회차: billable 여유 40(배정 < 역할 몫 120), 창 114,000 ms(계획 창 120초 < 배정 3,600초: 예산 쪽 적용) |
| 3 | 재개 회차: 여유 20(40 − 20), 계측됨. 몫은 누적 차분(행의 input 3·output 7·cache_creation 10·cache_read 20, 세션 누적은 그 두 배) |
| 4 | 새 세대 A1/1(한도 40): 모든 세대 합계 40/40 → 409 `ASSIGNMENT_LIMIT_REACHED`(“total_tokens: 40 of 40”), 전달·결과 행 불변, 한도 사건 1. 같은 거부를 다시 해도 사건 1. 연결 조회: 정산 행 2, 합계 40, cache_read 40(세지 않음), 남은 0, `awaiting_human_decision` |
| 5 | A1/2(한도 60): 여유 20, 계측 뒤 60/60 → 409 |
| 6 | 코디네이터 재시작: 연결 조회가 같은 JSON(합계 60, 회차 3), 같은 409 |
| 7 | 예산이 배정보다 작음: A1/3(한도 없음 → revision 100,000, 99,940 남음) → 여유는 역할 몫의 남은 60 |
| 8 | 시간 한도: 원장의 사용 시간 + 7초를 한도로 → 창 7초(timeout 1,000 ms), 계측된 1초 회차 뒤 6초 남음 ≤ 유예 → 409 `ASSIGNMENT_LIMIT_REACHED`(“elapsed_seconds: n+1 of n+7”) |
| 9 | 창을 다 쓰는 회차: 한도 = 사용 + 9초 → 창 8초(timeout 2,000 ms), 가짜 런타임이 4초 실행 → 감시가 `TotalTimeout`으로 정지, 미계측 → 다음 전달 409 `ASSIGNMENT_USAGE_UNMEASURED`, 연결 조회의 정산 행 `unmeasured` |
| 끝 | 전달 6, 정산 행 6, 한도 사건 4 |

`withdrawn`(원장에서 지운 기록)은 `delivery_e2e`의 10단계가 지난다(사람의 보정을 기록 삭제로 흉내 냄). 그 시도가 먼저 미계측으로 정산됐다가 지워진 뒤 `withdrawn`이 되어 다음 전달이 지나간다.

결과([evidence/test.log](evidence/test.log), 커밋 43fad72):
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음.
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **379 통과, 0 실패, 0 무시**. 기준(1b5bc1f) 376에 새 시험 3개.
- `validate_package.py`: `schema_errors` 0.

## 5. 변이 시험

각 변이를 하나씩 43fad72 코드에 넣고 대상 시험을 돌린 뒤 파일을 되돌렸다([evidence/mutants.txt](evidence/mutants.txt)). **13개 모두 잡혔다.**

| id | 변이 | 잡은 시험 |
| --- | --- | --- |
| S5-M1 | 요청 기록 전 거부 없음 | `limits_e2e` 두 시험 |
| S5-M2 | 회차를 배정 남은 양으로 낮추지 않음 | 같음 |
| S5-M3 | `total_tokens`에 cache_read 포함 | 같음 |
| S5-M4 | 미계측 회차가 정지가 아님 | 같음 |
| S5-M5 | 가장 새 세대의 소비만 셈 | 같음 |
| S5-M6 | 시간은 0초 남았을 때만 거부(유예 무시) | 같음 |
| S5-M7 | revision보다 큰 요청 한도 허용 | 같음 |
| S5-M8 | 요청 확인 전 정산 없음 | 같음 |
| S5-M9 | 원장에서 지운 기록의 행이 남음(`withdrawn` 없음) | `delivery_e2e` 두 시험 |
| S5-M10 | 한도 사건을 기록하지 않음 | `limits_e2e` 두 시험 |
| S5-M11 | 배정 한도 무시(revision 값만) | 같음 |
| S5-M12 | HTTP 문장 상한 없음(`NoGuard`) | `an_http_statement_waits_at_most_the_statement_timeout`(끝나지 않아 1,800초 감싸기가 끝냄. 실패한 시험으로 셈) |
| S5-F | 검토 debf413 변이 F: 연결의 `"none"`이 시도를 보지 않음 | `a_lost_first_action_report_is_filled_from_the_run_checkpoint` |

## 6. 이월 항목

- **C7 닫음.** 배정 수준 시간·토큰 한도를 사용자 결정 2026-11-16의 정의로 코디네이터(컨테이너 밖)에서 집행한다. 미계측 회차는 그 배정의 다음 전달을 멈춘다.
- **검토 fe924b7 I-5 닫음**(§1.5). **검토 debf413 L-3 닫음**(§1.6).
- L01의 **값**은 열려 있다. 실제 스프린트의 `total_tokens = null`은 그대로이고, 그런 revision으로 발급한 배정은 토큰 한도 없이(예산·역할 몫만) 돈다. 등록 검사 `UNBOUND_DRAFT`가 실제 실행 등록을 막는 것은 그대로다.

## 7. 계획과 다른 점, 결정한 것

- **정산은 지연 방식이다.** 회차 끝의 배경 스레드는 DB 연결이 없다. 그래서 다음 전달 때 원장에서 정산한다. 원장이 측정의 권한이고, 정산은 원장이 쉬고 있을 때와 예산 잠금 안에서만 한다. 그 결과 연결 조회는 다음 전달 전까지 끝난 회차를 보이지 않는다.
- **시간 한도와 유예.** 남은 시간이 정지 유예(6초) 이하이면 “도달”로 본다. 그런 창으로는 회차를 시작하지 못한다(단계 3과 같은 기준).
- **현재 머리 세대의 한도.** 세대마다 한도가 다를 수 있고, 배정 전체에는 현재 머리의 것을 적용한다.
- **“거부 무상태”의 범위.** 거부는 전달·결과 행을 남기지 않는다. 앞선 회차의 정산 행(측정된 사실)과 한도 사건 하나는 남는다. 계획의 “거부 사유와 기록을 남긴다”와 “상태 불변”을 함께 지키려는 선택이다.
- 사용자 결정이 필요한 것은 없다고 본다(정의는 사용자 결정 2026-11-16).

## 8. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 정산 표시 지연 | 끝난 회차는 다음 전달 때 정산된다. 그 전에는 연결 조회에 없다 | 02-08(사건 기반 갱신) |
| 원장 보정 경로 | 보정 기록(correction)으로 새 행이 생기는 경로는 코드로 처리하지만 종단 시험은 기록 삭제(`withdrawn`)만 지난다 | 다음 검토 때 보강 가능 |
| 새 배정 id | 한도는 배정 id마다다. 새 배정 id로 발급하면 소비가 0부터다. 예산·역할 몫은 그대로 막는다 | 설계 한계(보고) |
| 승인 예산 (나) | 02-05 예산 이름·상한 코드는 별도 PR(단계 8 전). 이 PR은 시험 고정값만 쓴다 | 단계 8 전 PR |
| 결정 요청 | 한도 도달 때 decision_request를 자동으로 만들지 않는다(G5 범위 확대 = 정책 새 버전) | 사용자 결정 시 |
| HTTP 10초 상한 | 영속 DB에서 긴 문장이 생기면 500이 된다. 지금 작업에는 그런 문장이 없다 | 관찰 |
| 변이 S5-M12 | 시험이 끝나지 않는 방식으로 잡힌다(잠금을 쥔 쪽이 답을 기다림). 시험에 읽기 시간 제한을 두면 더 깔끔하다 | 선택 개선 |

## 9. 검토 잔여 처리

| 항목 | 처분 |
| --- | --- |
| PR #75 검토 debf413 L-3 | 시험 추가, 변이 F 잡힘(§1.6, §5) |
| PR #74 검토 fe924b7 I-5 | HTTP 문장 상한 10초, 120초 판단 유지(§1.5), 시험과 변이 S5-M12 |

## 10. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험), [evidence/mutants.txt](evidence/mutants.txt).
- `docs/poc/scripts/validate_package.py`: `schema_errors` 0.
- 시험 DB(55432) 정리: 이 작업의 중간 실행과 변이 실행이 남긴 `ieo_t_lim_*`·`ieo_t_dlv_*`·`ieo_t_*`(중단된 시험 1개) 데이터베이스를 이름으로 지웠다. 끝난 뒤 `ieo_t_*`는 0개다. 55433·58090은 건드리지 않았다.
