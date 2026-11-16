# 02-05 단계 6 · 외부 요청 추적(3, C9)

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 6([계획](../plan-20261115/PLAN.md) §3.2 단계 6). 이월 C9(02-04 수용 항목 3: 외부 요청 추적, repo 범위 안전 종료)를 닫는다. 자식 추적은 04-05 |
| 착수 근거 | 단계 5 병합(PR #77, main de1336c) |
| 검토 반영 | PR #77 검토 af68c93 C-1·C-2, PR #74 검토 fe924b7 I-4 잔여, PR #75 검토 debf413 I-8·I-9(§9). **PR #78 검토 1b362ec**(변경 요청) M-1, L-1~L-4, I-1~I-4(§10) |
| 기준 커밋 | main de1336c. 구현 커밋 0411024, 시험 보강 e7e4e4b, 증거 90e275b. 교정 1: 코드·계약 9ebf329, 증거 81bfe7a(evidence/test.log 첫 줄이 9ebf329, 작업 트리 깨끗) |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 영속 PoC DB(55433)·58090 코디네이터는 쓰지 않았다(일회성 시험 DB 55432만) |
| 상태 | 교정 1 반영, **재검토 전**. 원장(`state.json`)은 고치지 않았다(원장 담당이 기록) |

## 1. 추적 대상의 정의(먼저 정함)

| 구분 | 무엇 | 어디서 보고 |
| --- | --- | --- |
| 추적함: 역할의 요청 | 역할이 supervisor를 거쳐 코디네이터에 보낸 요청 중 답을 모르는 것. 지금 코드에 있는 것은 둘이다: 체크포인트 보고(`checkpoint_report`, 대기 사본으로 남은 takeover 포함)와 첫 행동 보고(`first_action_report`) | supervisor 상태 `safe_shutdown.pending_external_requests`, 체크포인트 `pending_external_requests` |
| 추적함: 코디네이터의 요청 | 전달 경로에서 결과를 모르는 전달(결과가 아직 없음, 또는 마지막 결과 `unknown`) | repo 범위 정지 보고 `coordinator_pending_external_requests` |
| 아직 없음 | 보고 v0의 진행 보고와 재현 요청. 지금은 supervisor를 거쳐 보내는 코드가 없다. 그래서 목록에 생길 수 없다. 그 경로가 생기면 같은 목록에 든다 | — |
| 관측 불가 | 모델이 Bash·도구로 직접 낸 네트워크 요청. supervisor가 볼 수 없다 | `direct_network_requests: "not_observable"`(상태·체크포인트, 능력 공백). “없음”으로 쓰지 않는다 |
| 범위 밖 | GitHub push 단기 토큰 경로(GH1) | 05-03 |

목록의 각 항목은 요청 id, 종류, 보낸 시각(supervisor 시계), 대상(예: `coordinator reportCheckpoint`)이다. 체크포인트 보고의 id는 `checkpoint:<seq>:<digest 앞 12자>`, 첫 행동 보고의 id는 `first-action:<attempt_id>`다.

체크포인트 보고 항목을 실제로 정리하는 것은 다음 start·reconnect의 중앙 대조다(§2.1 재조회). 답을 받았을 때 바로 지우는 것은 그 사이의 상태 표시를 정확하게 할 뿐이다(검토 1b362ec I-3).

## 2. 더한 작업과 바뀐 동작

### 2.1 supervisor(`crates/ieo-supervisor/src/pending.rs` 새 파일, `serve.rs`)

- 목록은 역할 볼륨 `/state/pending-requests.json`에 둔다. 재시작한 서비스도 같은 목록을 읽는다.
- 체크포인트 보고와 첫 행동 보고는 **보내기 전에** 목록에 넣는다. 코디네이터가 답하면(저장·중복·4xx 거부) 지운다. 서버 오류(5xx)와 답 없음은 남긴다.
- **목록에 넣지 못한 요청은 보내지 않는다**(교정 1, 검토 L-1). 목록 파일을 읽거나 쓸 수 없을 때다(공간 부족, 권한, `pending-requests.tmp` 자리에 디렉터리 등).
  - 체크포인트 보고는 503 `EXTERNAL_REQUEST_NOT_TRACKED`로 실패하고 `central_checkpoint.last_report`에 그렇게 남는다. takeover이면 그 회차는 모델 호출 없이 `CentralRefused`로 끝난다.
  - 첫 행동 보고는 `{"ok": false, "not_sent": true, …}`로 상태의 `first_action_report`에 남고 회차는 계속된다. after_run 체크포인트가 같은 값을 싣는다.
  - 재조회 때 다시 보낼 첫 행동 보고도 같다. 목록을 다시 쓸 수 없으면 보내지 않고, 항목은 목록에 남아 reconnect가 멈춘다.
  - 그래서 “보냈는데 목록에 없는” 요청은 생기지 않는다.
- **재조회**: start·reconnect 전에 한다.
  - 중앙 대조(런타임 계약 §13 항목 10)가 로컬과 중앙을 맞추면, 목록의 체크포인트 보고는 모두 결론이 난 것이다. 반영됐거나, 반영 안 됨이 확인되어 대기 사본을 버렸다. 그래서 지운다.
  - 첫 행동 보고는 같은 본문으로 다시 보낸다. 답이 오면(201·같은 본문 200·4xx 거부) 지운다.
- 재조회 뒤에도 목록이 비어 있지 않거나 목록을 읽을 수 없으면 **reconnect는 409 `EXTERNAL_RECONCILIATION_REQUIRED`** 다. 답에 목록을 싣고 아무것도 실행하지 않는다(런타임 계약 §5-1·§6). start는 남은 목록이 있어도 진행한다(§7, §8). 목록을 읽을 수 없으면 start 요청은 받지만 takeover 보고를 보낼 수 없어 회차가 실행되지 않는다(위).
- 체크포인트의 `pending_external_requests`는 그 체크포인트를 쓸 때의 목록이고 `tracked.pending_external_requests`는 `true`다. 체크포인트의 목록은 기록이다. 재개 판단(`checkpoint::decide`)은 재조회 뒤의 현재 목록으로 한다. 그렇지 않으면 이미 지워진 요청 때문에 재개가 영영 막힌다.
- 상태: `safe_shutdown.pending_external_requests`가 목록(읽을 수 없으면 `"unreadable"`), `safe_shutdown.direct_network_requests: "not_observable"`. 능력 공백 목록의 “pending_external_requests not tracked”를 “direct network requests of the model not observable”로 바꿨다.
- `local_only`(시험용)는 아무것도 보내지 않으므로 목록이 생기지 않는다(교정 1에서 시험함).
- **한계(같은 UID).** 목록 파일은 개발 작업 디렉터리와 같은 역할 볼륨에 있고 모델은 같은 UID로 돈다. 개발 코드가 목록을 지우거나 고칠 수 있으므로 목록은 무결성 보장이 아니다(§8).

### 2.2 코디네이터(`crates/ieo-coord/src/api.rs`)

- repo 범위 정지(stopRepo)는 역할 정지 뒤 읽기 트랜잭션 하나로 그 repo의 **결과를 모르는 전달**을 읽는다. 각 전달을 `coordinator_pending_external_requests`에 싣고(요청 id = 전달 id, 종류 `delivery`, 보낸 시각, 대상, 시도, 상태 `no result recorded` 또는 `result unknown`) 사유에도 적는다.
- **그 읽기가 실패해도 정지 보고를 돌려준다**(교정 1, 검토 M-1). 정지는 이미 보냈으므로 같은 200 보고에 `coordinator_pending_external_requests: "unreadable"`, 사유 “coordinator: pending deliveries could not be read (<DB 오류>)”, `safe_shutdown_confirmed: false`를 싣는다. 맨 500으로 정지 보고를 잃지 않는다.
- 작업 표의 실행 모양이 `NoDatabase`에서 `CallThenTransaction`으로 바뀌었다(§3).
- 역할 목록이 비어 있으면 사유가 아니다. `active_children`은 04-05까지 `not_tracked`이므로 `safe_shutdown_confirmed`는 외부 요청이 0이어도 `false`이고 사유에 자식 미추적이 남는다(사용자 결정 2026-11-15 항목 3).
- `direct_network_requests: not_observable`은 안전 종료의 사유에 넣지 않았다. 따로 보고만 한다. 자식 추적이 생겨 다른 사유가 모두 풀릴 때 이것을 확인 조건에 넣을지는 04-05에서 정한다(§8).

### 2.3 계약

- 런타임 계약 **revision 13**: 머리 기록, §7 stop(repo 범위) 행의 참조, 새 §16 항목 18(추적 대상·재조회·reconnect), 19(관측 불가), 20(repo 범위 정지 보고). **뜻이 바뀐 필드가 하나 있다**: `pending_external_requests`는 “추적 안 함(`not_tracked`, 빈 목록은 관측 아님)”에서 “실제 목록”이 됐다. 머리 기록에 적었다. 교정 1에서 항목 18에 “목록에 넣지 못한 요청은 보내지 않음”, 읽을 수 없는 목록 위의 start, 같은 UID 한계를, 항목 20에 읽기 실패 때의 `"unreadable"` 보고를 더했다(revision 13은 아직 병합 전이라 번호를 올리지 않았다).
- OpenAPI 0.13.0 → **0.14.0**(추가): stopRepo 설명과 `coordinator_pending_external_requests` 스키마(교정 1: 목록 또는 `"unreadable"`의 `oneOf`), 머리 기록의 동작 변경. `openapi-spec-validator` OK.
- proto·내부 계약은 바꾸지 않았다. 세션 상태 보고의 `pending_external_requests`는 이미 목록을 받는다.

## 3. 수용된 작업의 동작 변경

사용자에게 알릴 항목이다.

1. **supervisor 상태(02-04 수용, §7 status)의 `safe_shutdown.pending_external_requests`.** `"not_tracked"`였다. 이제 목록이다(대개 `[]`). 새 필드 `direct_network_requests: "not_observable"`이 생겼고, 능력 공백 문구 하나가 바뀌었다. 수용된 시험 `status_reports_the_section_7_fields_and_an_honest_safe_shutdown`의 기대를 바꿨다.
2. **supervisor 체크포인트(02-04 수용).** `pending_external_requests`가 쓸 때의 실제 목록이고 `tracked.pending_external_requests`가 `true`다. `direct_network_requests` 키가 늘었다. 바이트가 달라지지만 digest는 늘 바이트 기준이라 규칙은 같다.
3. **supervisor reconnect(02-04 수용).** 코디네이터에 보낸 요청의 답을 모르는 채로 남아 있으면(재조회 뒤에도), 또는 목록 파일을 읽을 수 없으면 409 `EXTERNAL_RECONCILIATION_REQUIRED`다. 이전에는 이 경우를 보지 않았다. 닫는 쪽이다. 02-04 경로에서 체크포인트 보고는 중앙 대조로 모두 정리되므로, 실제로 남는 것은 답 없는 첫 행동 보고(02-05 단계 4부터)와 읽을 수 없는 목록 파일이다.
4. **stopRepo(02-04 수용).**
   - 실행 모양이 `NoDatabase`에서 `CallThenTransaction`으로 바뀌었다. 역할 정지 뒤 DB 읽기 하나를 더한다.
   - **DB에 닿지 못해도 답은 200 정지 보고 그대로다**(교정 1). 정지는 읽기 전에 이미 보냈다. 그 경우 `coordinator_pending_external_requests`는 `"unreadable"`이고 DB 오류를 담은 사유가 하나 늘며 `safe_shutdown_confirmed`는 `false`다. 교정 전(1b362ec)에는 이 경우 정지를 보낸 뒤 맨 500을 돌려줬다.
   - 답에 `coordinator_pending_external_requests`가 생겼고, 결과를 모르는 전달이 있으면 사유가 늘어난다.
   - 역할이 빈 목록을 보내면 그 항목은 더는 사유가 아니다(이전에는 `"not_tracked"`라 늘 사유였다). 자식 사유가 남으므로 `safe_shutdown_confirmed`의 값은 지금처럼 `false`다.
   - 수용된 시험 `operations.rs`의 실행 모양 기대를 바꿨다.
5. **supervisor start·체크포인트 보고(02-04 수용), 교정 1.** 목록 파일을 읽거나 쓸 수 없으면 체크포인트 보고를 보내지 않는다(503 `EXTERNAL_REQUEST_NOT_TRACKED`). takeover 보고가 그렇게 되면 start는 받아도(202) 회차는 모델 호출 없이 `CentralRefused`로 끝난다. 02-04에는 목록이 없어 이런 경우가 없었다. 닫는 쪽이고, 사람이 파일을 고칠 때까지 그 역할은 실행되지 않는다.
6. 02-05 진행 중 작업(수용 전)의 변경: 전달 시간 한계의 시험용 환경 변수는 이제 기본값 이하만 받는다(§9 C-1). 기본값과 다른 값이 쓰이면 기동할 때 기록한다.

## 4. 시험

새 시험(8개, 교정 1에서 4개 더함):

| 시험 | 내용 |
| --- | --- |
| `ieo-supervisor` 단위 `pending::tests::entries_are_kept_on_the_volume_and_removed_by_id` | 추가·같은 id 교체·재시작 뒤 같은 목록·id로 지움·보고에 재조회 정보 없음·읽을 수 없는 파일은 `"unreadable"`이고 추가 실패 |
| `ieo-supervisor` `tests/serve.rs` `pending_external_requests_are_tracked_requeried_and_stop_a_reconnect`(HTTP·gRPC) | 아래 표 0~5 |
| `ieo-supervisor` `tests/serve.rs` `a_refused_first_action_is_settled_and_an_untracked_request_is_not_sent`(HTTP·gRPC, 교정 1) | 아래 표 6~7 |
| `ieo-supervisor` `tests/serve.rs` `a_local_only_role_has_no_pending_external_requests`(교정 1, 검토 R6) | `local_only`에서 첫 행동이 있는 회차 → 상태 목록 `[]`, 목록 파일 없음, reconnect 재개(202 `resume`) |
| `ieo-coord` `tests/roles.rs` `pending_external_requests_are_reasons_and_untracked_children_keep_a_stop_unconfirmed` | 개발 역할의 비지 않은 목록이 사유(요청 id 포함), Guardrail의 빈 목록은 사유가 아니고 자식 미추적 사유 하나만 남음, `safe_shutdown_confirmed: false` |
| `ieo-coord` `tests/coordinator.rs` `a_repo_stop_keeps_its_report_when_the_pending_deliveries_cannot_be_read`(교정 1, 검토 M-1) | 시험 DB의 서버 연결을 모두 끊은 뒤 정지 → 200, 두 역할 보고, `coordinator_pending_external_requests: "unreadable"`, 사유 셋째가 “coordinator: pending deliveries could not be read (500 …)”, `safe_shutdown_confirmed: false` |

supervisor 시험 단계(가짜 중앙 기록, 두 전송):

| 단계 | 확인 |
| --- | --- |
| 0 | 처음 상태: 목록 `[]`, `direct_network_requests: not_observable`, `active_children: not_tracked` |
| 1 | 첫 행동 보고에 답이 없음(가짜가 5xx·trailer 없는 상태) → 회차는 계속, 목록에 `first-action:p1`(종류·대상·보낸 시각). after_run 체크포인트의 `tracked.pending_external_requests: true`, `direct_network_requests`. 재시작한 서비스도 같은 목록 |
| 2 | reconnect → 재조회해도 답 없음 → 409 `EXTERNAL_RECONCILIATION_REQUIRED`, 답에 목록, 실행 디렉터리 없음 |
| 3 | 가짜가 답함 → reconnect가 같은 본문을 다시 보내 지움 → 재개(202, `resume`) → 목록 `[]` |
| 4 | takeover 보고의 답을 잃음(5xx) → `CentralRefused`, 목록에 `checkpoint_report` → 다음 start의 중앙 대조가 대기 사본을 채택하고 목록을 지움 → 실행 → 목록 `[]` |
| 5 | 목록 파일을 깨뜨림 → 상태 `"unreadable"`, reconnect 409, 실행 없음 |
| 6 | (검토 R2) 답 없는 첫 행동 보고가 목록에 남은 뒤 가짜가 409 `EVENT_CONTENT_CONFLICT`로 거부 → reconnect의 재조회가 그 보고를 가짜에 보내고(한 번) 4xx로 지움 → 재개(202 `resume`). 그 회차 자신의 첫 행동 보고도 409로 지워짐 → 목록 `[]` |
| 7 | (검토 L-1) 첫 행동 보고가 목록에 남은 상태에서 `pending-requests.tmp` 자리에 디렉터리를 둠(목록을 쓸 수 없음) → reconnect 409, 그 보고는 가짜에 한 번도 가지 않음. 같은 상태의 start → takeover 보고를 보내지 않아 `CentralRefused`, `last_report`의 `EXTERNAL_REQUEST_NOT_TRACKED`, 가짜의 체크포인트 수 그대로. 디렉터리를 치운 뒤 start → 재조회가 그 보고를 보내고 목록 `[]` |

코디네이터 종단(`limits_e2e` 8b, 두 전송):
- 결과 트랜잭션을 잃은 두 전달(aL·aR)이 있을 때 repo 범위 정지 → `coordinator_pending_external_requests`가 **정확히** aL·aR(다른 전달은 결과가 정해짐), 사유에 “coordinator: delivery …”와 “development: active_children”, 개발 역할의 목록 `[]`, `safe_shutdown_confirmed: false`.
- (교정 1, 검토 R3) aL을 재질의해 마지막 결과가 `unknown`이 된 뒤 다시 정지 → aL은 `result unknown`, aR은 `no result recorded`, 사유에 “… has no known outcome (result unknown)”.
- (교정 1, 검토 I-4) 전달 잠금을 시간이 아니라 조건으로 잡는다(§10.2).

결과([evidence/test.log](evidence/test.log), 커밋 9ebf329):
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음.
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **388 통과, 0 실패, 0 무시**. 기준(de1336c) 380에 새 시험 8개(교정 전 4 + 교정 1의 4: coordinator 1, serve 3).
- `validate_package.py`: 이 보고가 있으면 `schema_errors` 0(자리 표시 파일로 확인, 교정 전). 런타임 계약 revision 13이 이 보고에 링크한다.
- 공유 대상 디렉터리의 다른 작업 트리 바이너리를 피하려고 작업 공간 일곱 패키지를 `cargo clean -p` 한 뒤 증거를 만들었다.

## 5. 변이 시험

각 변이를 하나씩 9ebf329 코드에 넣고 대상 시험을 돌린 뒤 파일을 되돌렸다(매번 `git status` 깨끗, [evidence/mutants.txt](evidence/mutants.txt)). **20개 모두 잡혔다.** 교정 전 13개는 교정한 코드에 맞춰 다시 돌렸다(S6-P1·S6-P10은 바뀐 코드에 맞춰 위치만 고침).

| id | 변이 | 잡은 시험 |
| --- | --- | --- |
| S6-P1 | 체크포인트 보고를 목록에 넣지 않음 | serve `pending_external…` |
| S6-P2 | 서버 오류·답 없음에도 목록에서 지움 | 같음 |
| S6-P3 | 재조회가 체크포인트 보고를 정리하지 않음 | 같음 |
| S6-P4 | 재조회가 첫 행동 보고를 다시 보내지 않음 | 같음 |
| S6-P5 | 목록이 있어도 reconnect를 멈추지 않음 | 같음 |
| S6-P6 | 재개 판단이 체크포인트의 옛 목록을 읽음 | 같음 |
| S6-P7 | 첫 행동 보고를 추적하지 않음 | 같음 |
| S6-P8 | 읽을 수 없는 목록이면 reconnect를 통과시킴 | 같음 |
| S6-P9 | 상태가 다시 `not_tracked` | serve 여섯 시험 |
| S6-P10 | 정지 보고가 코디네이터의 결과 불명 전달을 무시 | `limits_e2e` |
| S6-P11 | 정지 보고가 모든 전달을 결과 불명으로 셈 | `limits_e2e`(처음에는 살아남음: 포함 여부만 봤다. 정확히 둘인지 보도록 고쳤다, e7e4e4b) |
| S6-C1 | 시간 한계 덮어쓰기에 상한 없음(검토 af68c93 C-1) | 단위 `delivery_bounds_keep_the_call_inside_the_in_flight_window` |
| S6-C2 | 정산의 `|| sent` 갈래 제거(검토 af68c93의 H5) | `delivery_e2e` |
| C1-M1 | 읽기 실패 때 다시 맨 500(검토 M-1) | coordinator `a_repo_stop_keeps…` |
| C1-M2 | 읽기 실패의 사유를 적지 않음(M-1) | 같음 |
| C1-L1a | 목록에 넣지 못한 첫 행동 보고를 그래도 보냄(L-1) | serve `a_refused_first_action…` |
| C1-L1b | 목록에 넣지 못한 체크포인트 보고를 그래도 보냄(L-1) | 같음 |
| C1-R2 | 첫 행동 보고의 4xx가 항목을 지우지 않음(검토 R2) | 같음 |
| C1-R3 | `unknown` 전달을 정지 보고에서 뺌(검토 R3) | `limits_e2e`(새 정지 확인) |
| C1-R6 | `local_only`도 첫 행동 보고를 추적함(검토 R6) | serve `a_local_only_role…` |

검토의 R5(체크포인트 보고의 4xx 갈래)는 다음 중앙 대조가 어차피 정리하므로 거의 같은 동작이다(검토 I-3). 시험을 더하지 않았다(§1 끝 문장).

## 6. 이월 항목

- **C9 닫음.** 역할이 보낸 요청의 미확인 목록을 실제로 추적하고 보고하며, 목록이 있으면 reconnect가 외부 대조로 멈춘다. 코디네이터의 결과 불명 전달은 정지 보고에 나온다. 모델의 직접 요청은 `not_observable`로 따로 보고한다.
- **자식 추적은 04-05.** `active_children`은 `not_tracked`이고 안전 종료는 미확인으로 남는다(사용자 결정 항목 3).
- C14 일부: `pending_external_requests`가 더는 추적 공백이 아니다. `worktree_head`는 단계 7이다.

## 7. 계획과 다른 점, 결정한 것

- **reconnect만 멈춘다.** 계획대로다. start(새 세션)는 목록이 있어도 진행한다. 체크포인트 보고는 start 전 중앙 대조가 정리하고, 남을 수 있는 것은 답 없는 첫 행동 보고뿐이다. 그것은 앞 세션의 claim이라 새 세션을 막을 근거가 약하다. 목록을 읽거나 쓸 수 없을 때는 takeover 보고를 보낼 수 없어 start도 실행되지 않는다(교정 1).
- **목록에 넣지 못하면 보내지 않는다(교정 1).** 검토가 제시한 두 방법(보내지 않기, 또는 메모리의 “추적 실패” 표시로 목록을 `unreadable`로 만들기) 중 보내지 않기를 골랐다. 메모리 표시는 서비스가 재시작하면 사라져 답을 모르는 요청이 목록 없이 남을 수 있다. 보내지 않으면 결과는 늘 알려져 있다(반영 안 됨).
- **체크포인트의 목록과 재개 판단.** 체크포인트는 쓸 때의 목록을 기록으로 담고, 재개 판단은 재조회 뒤의 현재 목록으로 한다. 기록된 목록으로 판단하면 이미 답을 받은 요청 때문에 재개가 영영 막힌다(변이 S6-P6이 이를 확인한다).
- **`not_observable`은 사유가 아니다.** 안전 종료 확인을 영원히 막는 사유로 넣는 것은 04-05에서 자식 추적과 함께 정할 일로 남겼다. 지금은 자식 미추적 때문에 어차피 미확인이다(검토 I-1: 04-05의 진입 목록에 둔다).
- 사용자 결정이 필요한 것은 없다고 본다.

## 8. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 재현 요청 | supervisor를 거치는 코드가 없어 추적 대상에 없다. 생기면 같은 목록에 넣는다 | 런타임 계약 §2.1대로 API는 01-08, 실행기는 02-06 |
| 진행 보고 | supervisor를 거쳐 보내는 진행 보고는 계획된 작업이 없다(지금은 보고 v0 claim을 에이전트가 직접 보낸다). 만들지, 만들면 목록에 넣을지 | 02-05 단계 9(수용 보고)가 결정 담당 |
| 모델의 직접 요청 | 관측할 수 없다. 안전 종료 확인 조건에 넣을지는 미정 | 04-05 |
| 자식 | `not_tracked`, 안전 종료 미확인 | 04-05 |
| start의 목록 | start는 남은 목록(답 없는 첫 행동 보고)이 있어도 진행한다. 목록을 읽거나 쓸 수 없으면 takeover 보고를 보내지 않아 실행되지 않는다(사람이 파일을 고칠 때까지) | 02-05 단계 9(또는 수용의 사용자 결정) |
| 목록의 무결성(같은 UID) | 목록은 개발 작업 디렉터리와 같은 역할 볼륨에 있고 모델은 같은 UID다. 개발 코드가 목록을 지우면 reconnect가 답을 모르는 첫 행동 보고 위에서 재개할 수 있고, 첫 행동 본문을 심으면 재조회가 supervisor의 보고 자격으로 그것을 보낸다(§2.1이 이미 모델이 그 자격을 읽을 수 있다고 가정하고, 첫 행동은 claim이다). 목록은 무결성 보장이 아니다. 재개의 최종 근거는 코디네이터 기록(전달·takeover·첫 행동)과 단계 7의 GitHub 대조다. §2.1 체크포인트의 N5 한계와 같다 | 단계 7(최종 근거), 04-05(격리) |
| 검토 fe924b7 I-4 잔여 | supervisor 직접 start는 끝나지 않은 체크포인트 위에서도 새 세션을 시작한다. 충돌(crash) 뒤 사람이 새 세션으로 복구하는 것이 02-04의 설계된 복구이므로 막으려면 “누가 끝나지 않은 회차를 복구하는가”를 정해야 한다. 이 단계의 범위(외부 요청)가 아니다. 위험은 어댑터 자격을 가진 쪽(신뢰 경계 안)에 한정되고, 코디네이터 경로는 이미 막는다 | 단계 7(GitHub head를 재개의 최종 근거로 할 때 함께 검토) |
| 검토 debf413 I-8 | 직접 첫 행동 보고는 “그 세션의 takeover가 있음”만 보고, 채움은 “그 시도를 부른 전달에 연결된 takeover”를 본다. 같게 하면 단계 2 회귀 표(grpc.rs)의 첫 행동 사례 다수의 시험 고정값을 바꿔야 한다. 둘 다 claim이고, 연결 조회는 시도 id로 전달에 잇기 때문에 어긋난 보고는 아무 데도 연결되지 않는다 | 단계 8 전 정리(supervisor 보고 경로를 실제 회차로 확인할 때) |
| 검토 debf413 I-9 | 전달이 보낼 요청과 바이트까지 같은 복사본으로 먼저 직접 start하면, supervisor는 전달의 요청에 200 `replayed`로 답하고 전달은 `refused`로 기록된다. 실행은 사실상 그 전달의 것이다. 단계 3의 기존 동작이고 어댑터 자격(신뢰 경계) 안의 일이다 | 기록만(조치 없음) |
| 실행되지 않은 회차의 원장 기록 | takeover가 거부되어 실행되지 않은 회차(`CentralRefused`, `spawned: false`)도 전달 경로의 원장에는 `unmeasured`(사람 확인 요청) 기록이 된다(§10.2에서 확인). 0으로 세지 않는 쪽이라 닫혀 있지만, 아무것도 실행되지 않은 것이 확실한 경우에도 사람이 대조해야 한다 | 02-05 단계 9(수용 보고에 기록, 바꿀지 결정) |

## 9. 검토 잔여 처리

| 항목 | 처분 |
| --- | --- |
| PR #77 검토 af68c93 C-1 전달 시간 한계 덮어쓰기에 상한·기록 없음 | 교정. 두 값 모두 기본값 이하만 받는다(짧게만, fail-closed). 기본값과 다른 값이 쓰이면 코디네이터가 기동할 때 실제 값을 기록한다. 무시된 설정도 기록한다. 단위 시험에 큰 값 셋을 더함. 변이 S6-C1 |
| C-2 `|| sent` 갈래에 시험 없음 | 교정. `delivery_e2e` 10단계: a10의 답 유실 기록이 `not_accepted` 재조회 뒤에도 원장에 남은 상태에서 다음 전달 → 409 `ASSIGNMENT_USAGE_UNMEASURED`, a10 행 `unmeasured`. 사람이 기록을 지운 뒤 → `missing`(409), 그다음 정산에서 닫힘(`withdrawn`). 변이 S6-C2(검토의 H5) |
| C-3 이중 실패 메모 | 원장 담당 몫(지시대로 건너뜀) |
| PR #74 검토 fe924b7 I-4 잔여 | 기록(§8, 담당 단계 7) |
| PR #75 검토 debf413 I-8 | 기록(§8, 단계 8 전) |
| PR #75 검토 debf413 I-9 | 기록(§8) |

## 10. 검토 반영(PR #78 검토 1b362ec)

### 10.1 처분

| 항목 | 처분 |
| --- | --- |
| M-1 stopRepo가 역할을 정지한 뒤 DB 읽기 실패로 맨 500을 돌려줌 | 교정(9ebf329). 읽기가 실패하면(트랜잭션 오류든 전송의 중단이든) `roles::stop`의 상태·본문을 그대로 돌려주고 `coordinator_pending_external_requests: "unreadable"`, DB 오류를 담은 사유, `safe_shutdown_confirmed: false`를 싣는다. 시험 `a_repo_stop_keeps_its_report_when_the_pending_deliveries_cannot_be_read`는 검토의 시험처럼 서버 연결을 끊고 정지한다: 200, 두 역할, 사유, 미확인. OpenAPI 0.14.0 머리 기록·stopRepo 설명·스키마(`oneOf` 목록 / `"unreadable"`), 계약 항목 20, §2.2·§3 4번. 변이 C1-M1·C1-M2 |
| L-1 목록 추가 실패를 무시하고 보냄, 읽을 수 없는 목록 위의 start 문서 없음 | 교정. 추가에 실패한 요청은 보내지 않는다(§2.1, §7). 체크포인트 보고는 503 `EXTERNAL_REQUEST_NOT_TRACKED`, 첫 행동 보고는 `not_sent`. 읽을 수 없는 목록 위의 start는 takeover 보고를 보내지 못해 실행되지 않는다. 계약 항목 18, §3 5번, §8에 적었다. 시험 단계 7, 변이 C1-L1a·C1-L1b |
| L-2 같은 UID 한계가 목록 파일에 적혀 있지 않음 | 교정. 계약 항목 18, §2.1, §8, `pending.rs` 모듈 설명에 한 문장: 목록은 무결성 보장이 아니고 최종 근거는 코디네이터 기록과 단계 7의 GitHub 대조 |
| L-3 R2·R3·R6에 시험 없음 | 교정. R2: 시험 단계 6(가짜의 409 `EVENT_CONTENT_CONFLICT`, 두 전송), R3: `limits_e2e`의 두 번째 정지(`result unknown`), R6: `a_local_only_role_has_no_pending_external_requests`. 변이 C1-R2·C1-R3·C1-R6 모두 잡힘 |
| L-4 §8 두 행에 담당 없음 | 교정. 재현 요청은 01-08(API)·02-06(실행기), supervisor 경유 진행 보고는 계획된 작업이 없어 02-05 단계 9가 결정 담당, “start의 목록”은 02-05 단계 9(§8) |
| I-1 `not_observable`은 정지 사유가 아님 | 기록 유지. 04-05의 진입 목록에 둔다(§7, §8, 계약 항목 19) |
| I-2 api.rs의 낡은 주석 | 교정. RoleStatus는 DB 없음, RoleStop은 정지 뒤 읽기 트랜잭션 하나로 고쳐 적음 |
| I-3 R5는 거의 같은 동작 | §1 끝에 문장 하나: 체크포인트 항목을 실제로 정리하는 것은 중앙 대조다. 시험은 더하지 않음 |
| I-4 `limits_e2e` 948행 한 번 실패 | 원인을 재현으로 확인했다. 시험의 경쟁이고 코드 경쟁은 아니다. 시험을 조건 기반으로 고쳤다(10.2) |

### 10.2 I-4: `limits_e2e` 8b의 aL `unmeasured`

**원인.**
- 8b는 aL의 결과 트랜잭션을 잃게 하려고 전달 잠금(`delivery:repo-a:development` advisory lock)을 따로 잡는다. 교정 전에는 전달을 보내기 직전에 스레드를 띄워 **1.5 s 뒤에** 잠금을 잡고 15 s 쥐었다. 역할 서버는 start를 바로 처리하고 답만 3 s 늦췄다.
- 그런데 supervisor가 회차를 시작할 때 보내는 takeover 보고도 코디네이터에서 같은 잠금을 잡는다(`api.rs` `check_delivery`, 02-05 단계 4: takeover를 전달에 묶는 검사).
- 평소에는 전달의 호출이 1.5 s 안에 supervisor에 닿아 takeover가 먼저 끝난다. 부하가 크면(검토 때처럼 두 시험 바이너리를 함께 돌리면) 호출 전 단계(판단, 예산 검사와 그 상태 조회, 정산)가 1.5 s를 넘을 수 있다. 시작 기한은 2 s라 호출은 여전히 나간다.
- 그러면 takeover 보고가 시험이 쥔 잠금을 기다리다 10 s 문장 한계로 거부되고, 회차는 모델 없이 `CentralRefused`(`spawned: false`)로 끝난다. 빈 stream이라 원장 기록의 `usage.run.state`가 `unmeasured`가 된다. 이것이 검토가 본 실패(`"unmeasured"` ≠ `"measured"`)다.

**재현.** 잠금을 takeover 전에 잡도록 시험을 잠시 바꿔(역할 서버가 start를 처리하기 전에 신호를 주고 0.5 s 기다림) HTTP 쪽을 돌렸다. 같은 줄(`settled("aL")["usage"]["run"]["state"]`)에서 `left: "unmeasured"`, `right: "measured"`로 실패했다. 바꾼 것은 되돌렸다.

**코드 경쟁인가.** 아니다. 코디네이터는 맞게 동작했다. 결과를 기록하는 트랜잭션이 잠금을 쥐고 있는 동안 온 takeover는 기다리고, 한계를 넘으면 거부된다. 거부된 takeover의 회차는 실행되지 않는다. 실제 코디네이터에서 이 잠금은 짧은 트랜잭션 안에서만 쥐어지고, 호출하는 동안에는 쥐지 않는다. 15 s 동안 잠금을 쥐는 것은 시험이 “결과 트랜잭션을 잃음”을 만들려고 넣은 인위적 장치다. 다만 실행되지 않은 회차가 원장에 `unmeasured`로 남는 것은 따로 적었다(§8 마지막 행).

**고침(시험만).** 시간 대신 조건을 기다린다.
- 역할 서버는 느린 start를 처리한 뒤 `handled`를 세우고, `release`가 설 때까지(최대 30 s) 답을 미룬다(교정 전에는 3 s 고정).
- 잠금 스레드는 `handled`를 기다리고, 실행되는 회차(aL)면 서비스 상태로 그 회차가 끝났는지까지 확인한다. takeover 보고는 회차보다 먼저 끝난다. 그다음 잠금을 잡고 `release`를 세운다.
- 그래서 순서는 늘 같다: takeover → 회차 끝 → 시험의 잠금 → 답 → 결과 트랜잭션이 10 s 뒤 롤백. 실행되지 않는 aR(자격 없음, 바로 거부)은 `handled`만 기다린다.
- 고친 시험은 단독 실행, 전체 실행 두 번(353201d와 주석만 다른 9ebf329)에서 두 전송 모두 통과했다. 변이 S6-P10·S6-P11·C1-R3 실행(각 두 전송)에서도 aL 확인 줄은 통과했고, 각 변이를 겨냥한 정지 확인 줄(1018~1061행)에서만 실패했다.

### 10.3 시험 DB

교정 1의 실패한 변이 실행과 위 재현 실행이 `limits_e2e`·`delivery_e2e`의 시험 DB 9개(`ieo_t_lim_*`·`ieo_t_dlv_*`, 모두 이 작업의 끝난 프로세스)를 남겼다. 이 두 종단 시험은 실패하면 자기 DB를 지우지 않는다(기존 동작). 이름으로 지웠고 `ieo_t_*`는 0개다.

## 11. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험, 커밋 9ebf329), [evidence/mutants.txt](evidence/mutants.txt)(20개).
- 시험 DB(55432) 정리: 이 작업의 실행과 변이 실행이 남긴 `ieo_t_*` 데이터베이스를 이름으로 지웠다. 끝난 뒤 `ieo_t_*`는 0개다. 55433·58090은 건드리지 않았다.
