# 02-05 단계 8 전 PR · 예비 경로 종단 시험, 멈춤 사유 구조, 첫 행동 조건 통일, 단계 8 준비 점검표

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 단계 8(실제 회차) 전 마지막 코드 PR. PR #81 검토 291a9b0 N-1~N-3, 이월 I-8(PR #75 검토 debf413), 단계 8 준비 점검표(§6, 행동 없음) |
| 검토 반영 | **PR #82 검토 63fefc1**(변경 요청) M-1 (a)~(e), L-1, I-1~I-3(§10) |
| 착수 근거 | PR #81 병합(main 0a3d2a3, 검토 2 approve) |
| 기준 커밋 | main 0a3d2a3. 구현 c552c99, 보고 63fefc1. 검토 63fefc1 반영: 교정 e2dd5f3, 증거 c9a4ba1(evidence/test.log 첫 줄이 e2dd5f3, 작업 트리 깨끗). 5c673f0·f5d2e2e의 증거는 c9a4ba1이 대신한다 |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 영속 PoC DB(55433)·58090 코디네이터는 쓰지도 읽지도 않았다(일회성 시험 DB 55432만). 비밀 파일·실행 기록(`/opt/rust/ieo_poc_secrets`·`/opt/rust/ieo_poc_runs`)은 읽지 않았다. 점검표(§6)는 코드·문서에서만 끌어냈다 |
| 상태 | 구현과 시험 완료, 독립 검토 1회(63fefc1, 변경 요청) 반영, **재검토 전**. 원장(`state.json`)은 고치지 않았다(원장 담당이 기록) |

## 1. 한 일

| 항목 | 내용 | 코드 | 시험 | 변이 |
| --- | --- | --- | --- | --- |
| **N-1**(PR #81 검토 291a9b0) 유일한 예비 경로의 종단 시험 | 사람 자격 → `deliverAssignment` → 02-05 공유 예산 검사 → 역할 supervisor를 실제 코디네이터 바이너리·실제 PostgreSQL·역할 서비스(가짜 `claude`)로 지난다. 검토의 H5(전달이 `spare_extra` 대신 본문의 `spare`를 그대로 넘김)를 잡는다 | 코드 변경 없음(시험만) | 새 종단 시험 `ieo-coord` `tests/spare_e2e.rs` `a_person_attaches_the_0205_spare_through_deliver_assignment`(§4) | P1 |
| **N-2** 새 멈춤 사유가 우연히 결과 사유로 읽히지 않게 | 회차 자신의 결과 사유(`result …`, `no result event`)만 예비가 풀 수 있다. 그 밖의 모든 멈춤 사유는 `caller::other_stop_reason`을 지난다: 구분자 `"; "`를 `", "`로 바꾸고, 결과 사유처럼 시작하면 `other: `를 앞에 붙인다. 그래서 나중에 더한 사유가 문구만으로 풀리는 사유가 되지 못하고, 사유 하나가 여러 조각으로 나뉘어 결과 사유 조각을 만들지도 못한다. 결과 사유 판별(`is_result_stop_reason`)은 기록 쓰기와 `spare_releasable`이 같은 상수를 쓴다. 지금의 사유(정리 실패, 구독 전용이 아닌 환경, 미계측, 이 회차의 신호, 답을 잃은 호출)가 모두 이 길을 지난다 | `crates/ieo-supervisor/src/caller.rs`: `RESULT_STOP_REASON`·`NO_RESULT_STOP_REASON`·`is_result_stop_reason`·`other_stop_reason`, `After::finish`의 사유, `record_lost` | `tests/caller.rs` `other_stop_reasons_never_read_as_a_result`(접두, 구분자, 그대로인 사유, 그렇게 만든 기록은 예비가 풀지 못함) | P5~P8 |
| **N-3** `approved_at`의 뜻 | 계약에 “사람이 보낸 `deliverAssignment` 요청을 코디네이터가 처리한 시각(별도의 승인 시각이 아님)”으로 적었다 | 코드 변경 없음 | — | — |
| **I-8**(PR #75 검토 debf413) 직접 첫 행동 보고의 조건을 채움과 같게 | `recordFirstAction`은 지금까지 “그 세션의 takeover 체크포인트가 그 배정·세대로 저장됨”만 봤다. 이제 채움과 같이 “그 세션의 takeover가 그 배정·세대로 저장됐고 **그 시도를 부른 전달에 연결됨**”을 본다. 아니면 409 `CHECKPOINT_MISMATCH`(문구: “…is linked to the delivery that called this attempt”), 행 없음. 두 경로가 함수 하나(`takeover_linked`)를 쓴다 | `crates/ieo-coord/src/api.rs`: `takeover_linked`(옛 `taken_over` 대신), `record_first_action`, `fill_first_action` | `grpc.rs` `supervisor_operations_answer_the_assignment_and_store_claims`: 전달이 부르지 않은 시도 → 409, 전달된 시도 → 201, R4(다른 세션) → 409, 새 세대 뒤 같은 세션의 두 번째 전달(seq 3)의 시도 → 201 `current_assignment: false`. `a_record_stored_over_one_transport_replays_over_the_other`: 전달된 시도로 | P2~P4 |

| **검토 63fefc1 M-1 (a)** 실행 중 코디네이터를 멈추면 회차 기록이 사라짐 | 받아들인 회차는 코디네이터 프로세스 안의 스레드가 끝내고 계측해 02-05 장부에 쓴다. 지금까지 SIGTERM은 HTTP 작업자만 풀고 그 스레드를 기다리지 않아, 재배포·재시작 중인 회차는 `inflight.json`만 남기고 기록이 없었다(예산이 멈추고 예비로 풀 수 없음). 또 역할의 after_run 체크포인트 보고가 닫힌 코디네이터에 닿지 못해 다음 전달이 “local checkpoint … is not yet recorded”로 외부 대조가 됐다(시험으로 확인). 이제 신호를 받으면 새 전달을 503 `COORDINATOR_STOPPING`(기록 없음)으로 거절하고, 받아들인 회차가 모두 끝나 기록될 때까지(한도 `IEO_COORD_SHUTDOWN_WAIT_SECS`, 기본 720초(병합 뒤 780초로 정정, §11) = 회당 창 600초 + 멈춤 유예 + 호출자 여유) **계속 서비스한 뒤** 작업자를 푼다. 한도를 넘으면 남은 회차 수를 기록하고 멈춘다(그 기록은 없다: 사람이 대조) | `delivery.rs`: `FINISHING`·`Finishing`·`finishing`·`wait_finishing`, `STOPPING`·`begin_stop`, 전달의 503. `bin/ieo-coord.rs`: 신호 스레드 | `spare_e2e` 7단계: R2 실행 중 SIGTERM → 새 전달 503·기록 없음 → 코디네이터가 끝난 뒤 장부에 R2 기록, `inflight.json` 없음 → 재시작 뒤 다음 전달이 정상 판단(“planned runs are used”). `delivery.rs` 단위 `the_shutdown_waits_for_the_runs_being_finished` | Q1~Q3, Q6 |
| **검토 63fefc1 I-1** 관문의 사람 검사를 독립 검사로 | `spare_extra`가 `by.kind`를 늘 `user`로 쓰던 것을 호출자의 실제 종류로 쓴다. 그래서 전달의 403 뒤에 관문의 `kind == "user"` 검사가 따로 한 번 더 막는다 | `delivery.rs` `spare_extra` | `delivery.rs` 단위(서비스 자격 → `kind: service`) | Q4 |
| **검토 63fefc1 L-1** 세대 불일치 | 새 세대 뒤 세대 0에서 전달된 시도를 세대 1로 보고 → 409(코드는 이미 맞음, 시험 보강) | — | `grpc.rs` `supervisor_operations…` | Q5(검토의 R82-B) |

계약: OpenAPI 0.17.1(recordFirstAction 조건, `approved_at` 뜻, deliverAssignment 503 `COORDINATOR_STOPPING`), 내부 계약(`ieo-internal-v0.md` `RecordFirstAction` 순서), 런타임 계약 revision 18(§15 항목 15의 직접 보고 조건, §19 항목 30의 `approved_at`). proto 변경 없음.

## 2. 더한 작업

- `crates/ieo-coord/tests/spare_e2e.rs`(새): limits_e2e의 시험 틀(코디네이터 바이너리, 역할 서비스, 가짜 `claude`)을 가져와 가짜 `claude`에 `FAIL`(오류 결과)을 더했다. 역할 서비스 둘 다 `budget: "02-05"`(몫 생략), 코디네이터 예산 계획 `02-05`.
- `crates/ieo-supervisor/src/caller.rs`: 멈춤 사유 구조(N-2).
- `crates/ieo-coord/src/api.rs`: `takeover_linked`(I-8).
- `crates/ieo-coord/tests/grpc.rs`: 단계 2 회귀의 첫 행동 시험 고정값을 전달된 시도(`att(seq)` = 그 takeover의 시험용 전달 id)로 바꿨다. 세대 불일치 단언(L-1).
- 검토 63fefc1 반영(e2dd5f3): `crates/ieo-coord/src/delivery.rs`(끝내는 중인 회차 수, 멈춤 표시, 503, `spare_extra`의 종류), `crates/ieo-coord/src/bin/ieo-coord.rs`(신호 스레드가 기다린 뒤 작업자를 풂).

## 3. 수용된 작업의 동작 변경

사용자에게 알릴 항목이다.

1. **recordFirstAction(02-05 단계 2, 수용)** — 검토 debf413 I-8. 직접 보고는 그 세션의 takeover가 **그 시도를 부른 전달에 연결돼 있을 때만** 받는다. 지금까지는 그 세션의 takeover가 그 배정·세대에 하나라도 있으면 어떤 `attempt_id`든 받았다(201). 이제 그런 보고는 409 `CHECKPOINT_MISMATCH`이고 행을 남기지 않는다. 정상 경로(코디네이터 전달이 부른 시도를 supervisor가 보고)는 같다: supervisor는 전달의 `attempt_id`로 회차를 돌리고 그 값을 보고한다. 같은 세션의 두 번째 회차도 그 회차의 전달이 takeover로 연결되면 받는다. 채움(`source: checkpoint`)은 이미 이 조건이었다.
2. **멈춤 사유 문구(02-04 호출자·공유 예산 검사, 수용)** — N-2. 결과 사유가 아닌 사유 안의 `"; "`는 `", "`로 바뀌고, 결과 사유처럼 시작하는 사유에는 `other: `가 붙는다. 지금의 사유 중 결과 사유처럼 시작하는 것은 없다. `"; "`가 들어갈 수 있는 것은 외부 문자열을 품는 사유(미계측 이유, 이 회차의 신호, 답을 잃은 호출의 오류)뿐이다. 그 경우 `stop_reason`의 한 사유가 더는 여러 조각으로 나뉘지 않는다(예: 재계측의 “미계측 하나뿐” 판단이 그 사유를 하나로 본다). 이미 쓰인 기록은 바뀌지 않는다. 멈춤 여부(`stop_required`)는 같다.
3. **코디네이터의 멈춤(SIGTERM/SIGINT, 02-04 수용: “받기를 멈추고 진행 중인 요청을 끝내고 exit 0”)** — 검토 63fefc1 M-1 (a). 이제 받아들인 회차가 끝나 기록될 때까지 **계속 서비스하고**(한도 기본 780초 = 회당 창 600초 + 호출자 여유 60초 × 2 + 60초, 그보다 긴 창의 회차가 진행 중이면 그 회차의 한도까지. 2026-11-17 정정, §11) 그동안 `deliverAssignment`는 503 `COORDINATOR_STOPPING`(기록 없음)이다. 다른 요청(체크포인트 보고 등)은 그동안 그대로 받는다. 진행 중인 회차가 없으면 지금처럼 곧 멈춘다. 멈춤에 걸리는 시간이 길어질 수 있으므로 컨테이너의 정지 유예(`podman stop -t` 등)가 짧으면 그 전에 강제 종료된다(§6 항목 11).
4. 추가만: 시험(`spare_e2e`), 계약 문구. 예비 표식의 `by.kind`는 호출자의 실제 종류다(받아들인 표식은 늘 `user`이므로 기록은 같다).

## 4. 시험

새 시험 3개(검토 63fefc1 반영 1개 포함):

| 시험 | 내용 |
| --- | --- |
| `ieo-coord` `tests/spare_e2e.rs` `a_person_attaches_the_0205_spare_through_deliver_assignment`(N-1) | 실제 코디네이터·PostgreSQL·역할 서비스(02-05 설정)·가짜 `claude`. 1) R1(`FAIL`) → 받음, 기록 `stop_required`. 2) 표식 없는 전달 → `budget_refused` “asks for a person”. 3) 서비스 자격(`operator`)의 예비 → 403 `HUMAN_APPROVAL_REQUIRED`, 전달 행 그대로. 4) 사람 자격(`human-op`, `kind: user`)의 예비(`replaces: r1`, `evidence_ref`) → 받음(두 번째 호출). 예비 기록의 `spare` = 사유·`r1`·증거 참조·`by {name: human-op, kind: user, via: deliverAssignment}`·`approved_at`(UTC `Z`), 그 기록은 `stop_required: false`. R1 기록은 그대로 하나. 5) 같은 멱등 키·같은 본문 다시 → 200, 저장된 답 그대로, 전달 행·장부 줄 수 그대로. 6) 다른 키의 두 번째 예비 → `budget_refused` “used already”. 7) R2(표식 없음, 4초쯤 걸리는 회차) → 받음(세 번째 호출). 실행 중 SIGTERM → 새 전달 503 `COORDINATOR_STOPPING`·전달 행 그대로 → 코디네이터가 끝난 뒤 장부에 R2 기록(`stop_required: false`), `inflight.json` 없음 → 재시작(검토 63fefc1 M-1 (a)). 8) 그다음 → `budget_refused` “planned runs are used”. 장부의 모델 회차는 `r1`·`sp`·`r2` 셋 |
| `ieo-coord` `delivery.rs` 단위 `the_shutdown_waits_for_the_runs_being_finished`(검토 63fefc1 M-1 (a)) | 끝내는 중인 회차가 있으면 기다림은 한도까지, 끝나면 0을 돌려줌 |
| `ieo-supervisor` `tests/caller.rs` `other_stop_reasons_never_read_as_a_result`(N-2) | `other_stop_reason`: 결과처럼 시작하는 사유 → `other: …`, 구분자 → `, `, 보통 사유 → 그대로. 그렇게 만든 사유로 멈춘 기록은 `spare_releasable`이 풀지 않음 |

바꾼 시험: `grpc.rs`의 두 시험(§1 I-8 행, 세대 불일치 단언 L-1), `delivery.rs` 단위 `the_spare_names_the_person_who_attached_it`(서비스 자격 → `kind: service`, I-1).

결과([evidence/test.log](evidence/test.log), 커밋 e2dd5f3):
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음.
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **408 통과, 0 실패, 0 무시**. 기준(0a3d2a3) 405에 새 시험 3개.
- 공유 대상 디렉터리의 다른 작업 트리 바이너리를 피하려고 작업 공간 일곱 패키지를 `cargo clean -p` 한 뒤 증거를 만들었다.

## 5. 변이 시험

각 변이를 하나씩 e2dd5f3 코드에 넣고 대상 시험을 돌린 뒤 파일을 되돌렸다(매번 `git status` 깨끗, [evidence/mutants.txt](evidence/mutants.txt)). **15개 중 14개가 잡혔고 1개(P7)는 동등 변이다.** 검토 전의 9개를 다시 돌렸고 검토 63fefc1 반영 6개(Q1~Q6)를 더했다.

| id | 변이 | 결과 |
| --- | --- | --- |
| P1 | 전달이 `spare_extra` 대신 본문의 `spare`를 넘김(검토 H5) | `spare_e2e`가 잡음 |
| P2 | `recordFirstAction`이 연결 조건을 보지 않음(I-8 되돌림) | `grpc.rs` |
| P3 | 연결이 전달의 시도를 묶지 않음(두 경로) | `grpc.rs` |
| P4 | 연결이 세션을 묶지 않음 | `grpc.rs` |
| P5 | 다른 사유가 결과 사유로 읽힐 수 있음(접두 없음) | `other_stop_reasons…` |
| P6 | 다른 사유가 구분자를 품을 수 있음 | 같음 |
| P7 | 정리 실패 사유가 가드를 지나지 않음 | **살아남음, 동등**: 그 문구는 결과처럼 시작하지도 `"; "`를 품지도 않아 가드가 그대로 돌려준다. 가드 자체는 P5·P6이 잡는다 |
| P8 | 예비 검사가 “result”를 품은 사유를 풂 | `other_stop_reasons…` |
| P9 | 관문이 사람이 아닌 `kind`의 표식을 받음 | `caller.rs` 예비 시험 |
| Q1 | 멈추는 코디네이터가 전달을 받음 | `spare_e2e` |
| Q2 | 멈춤이 끝내는 중인 회차를 기다리지 않음(한도 0) | `spare_e2e`(R2 기록 없음) |
| Q3 | 받아들인 회차를 끝내는 중으로 세지 않음 | `spare_e2e` |
| Q4 | `spare_extra`가 호출자와 상관없이 `kind: user`를 씀(I-1) | `delivery.rs` 단위 |
| Q5 | 첫 행동 연결이 세대를 묶지 않음(L-1, 검토의 R82-B) | `grpc.rs` |
| Q6 | `wait_finishing`이 곧 돌아옴 | `delivery.rs` 단위 |

## 6. 단계 8 준비 점검표(행동 없음)

실제 회차 전에 참이어야 하는 것이다. 이 PR은 아무것도 실행하거나 바꾸지 않았다. 코드와 문서에서만 끌어냈고 영속 DB·58090·비밀·실행 기록은 보지 않았다. 확인은 원장 담당이 한다.

| # | 전제 | 근거(코드·문서) | 확인 방법(읽기 전용) |
| --- | --- | --- | --- |
| 1 | **재배포.** `ieo-role-call`과 `ieo-coord`(영속 코디네이터 58090 포함)를 **이 PR 병합 뒤의 빌드**로 바꾼다. 역할 컨테이너 이미지도 그 병합 SHA로 만든다 | PLAN §4.2(“실제 회차는 그 병합 SHA의 이미지로”), 02-04 호출자 이월 보고 §(검토 3724bc9 I-5). RESUME.md: 58090은 **main 778ee47 빌드**로 실행 중 | 실행 중인 바이너리·이미지의 빌드 SHA를 병합 SHA와 대조 |
| 2 | **영속 DB 스키마.** `ieo-coord serve`는 기동 때 `migrate`(schema.sql, 모두 `IF NOT EXISTS`·`CREATE OR REPLACE`)를 돈다(`bin/ieo-coord.rs`). 778ee47 뒤 schema.sql에 더해진 것(319줄 추가, 1줄 바뀜): 새 표 `role_checkpoints`·`role_tamper_events`(X-08/단계 1), `role_first_actions`·`role_session_status`(단계 2), `role_deliveries`·`role_delivery_results`(단계 3), `role_takeovers`(단계 4), `assignment_run_usage`·`assignment_limit_events`(단계 5), `role_recoveries`·`role_recovery_uses`(단계 7b). 열: `assignments.limits`, `role_checkpoints.current_assignment`, `role_first_actions.source`, `role_delivery_results.github`. 제약: `evidence.kind`에 `branch_head`(제약을 지우고 다시 만듦, 단계 7), `role_recovery_uses`의 옛 유일 제약 삭제(d336849 L-a). 새 표마다 추가 전용 트리거. 지우거나 바꾸는 표·열은 없다 | `git diff 778ee47 HEAD -- crates/ieo-coord/src/schema.sql` | 재배포 전 영속 DB의 백업 여부. 기동 뒤 `GET /v0/health`의 `schema_version`(`ieo-coord-v0`, 이 값은 바뀌지 않는다)과 표 목록 |
| 3 | **바인딩과 sprint 소유의 읽기 전용 대조.** 코디네이터는 기동 때 `--bindings`의 각 repo `sprint_id`를 본다: 두 repo가 같은 id를 대면, Notion 가져오기(`notion_imports.test_id`)가 그 id를 가지면, `sprint_revisions`에 다른 repo로 등록돼 있으면 기동하지 않는다(`check_bindings`, PLAN C2) | `api.rs` `check_bindings` | 비공개 바인딩 파일의 repo·sprint_id 목록과 영속 DB의 `sprint_revisions(sprint_id, repo_id)`·`notion_imports(test_id)`를 읽기만 해서 대조. 문제가 있으면 기동 실패로 드러나므로 재배포 **전에** 본다 |
| 4 | **다른 배정의 중앙 체크포인트가 없는 repo·역할.** 다른 배정의 체크포인트가 중앙 기록에 있으면 새 배정은 외부 대조로 멈추고 새 세션도 시작하지 않는다 | 예산 보고 §4·§8, §13 항목 10 | 영속 DB `role_checkpoints`에서 대상 repo·역할의 행(읽기 전용). 02-04 실제 7회는 중앙 경로를 지나지 않았다는 기록(02-04 수용 기록 §2) |
| 5 | **새 역할 볼륨.** supervisor의 보조 상한은 볼륨의 시도 장부와 실행 디렉터리를 예산과 무관하게 센다. 02-04 회차를 돈 볼륨을 `02-05`로 쓰면 시작하지 않는다 | 예산 보고 §4·§8 | 단계 8의 개발·Guardrail 볼륨이 새것인지 |
| 6 | **설정이 예산 `02-05`를 이름으로 댄다.** 역할 서비스 `config.budget: "02-05"`(몫은 쓰지 않거나 승인값과 같게), 코디네이터 `roles.json`의 `budget_plan.budget: "02-05"`, **새 장부 디렉터리**(02-04 장부와 다름: 장부 하나는 예산 하나), `first_observation`(`overageStatus: rejected` 관측) | `budget::named`, `Plan::from_json`, `serve.rs` | 설정 파일을 읽어 확인(비밀 값은 출력하지 않음) |
| 7 | **예비용 사람 자격.** 예비는 `deliverAssignment`로만, `kind: user`·`roles:control` 자격으로만 붙는다. `ieo-role-call`은 예비를 거절한다 | 예산 보고 §1·§12, 런타임 계약 §19 항목 30 | 코디네이터 자격 설정(`tokens.json`)에 그런 주체가 있는지(값 출력 없이). 예비를 쓸 때는 원인 보고·증거 참조를 먼저 남긴다 |
| 8 | **GitHub head 입력.** R2(개발 재개)는 배정 브랜치의 GitHub head가 체크포인트의 push SHA(없으면 배정 `target_sha`)와 같아야 `resume_allowed`다. 단계 8은 GitHub 쓰기·push가 없으므로 그 비교 대상은 `target_sha`이고, `evidence:ingest`가 `branch_head`(그 브랜치, `target_sha`)를 넣어 둬야 한다(신뢰 입력, 실제 수집 아님) | 단계 7 보고, `api.rs` `branch_head` | 단계 8 절차서에 `branch_head` 입력을 적는다 |
| 9 | **Guardrail 대상.** G1은 검토 대상 SHA가 현재 개발 배정의 `target_sha`일 때만 실행한다(아니면 `stale_target`) | `api.rs` `stale_target` | 배정 순서를 절차서에 적는다 |
| 10 | **추가 청구 USD 0, 02-04 예비 5회 미사용.** 02-04·X-02 장부는 쓰지 않는다 | 결정 2026-11-15·2026-11-17 | 계획·설정의 장부 경로 |
| 11 | **(a) 회차 중에는 코디네이터를 재시작·재배포하지 않는다.** (2026-11-17 병합 뒤 정정, §11) 받아들인 회차는 코디네이터 프로세스 안의 스레드가 끝내고 계측해 장부에 쓴다(`delivery.rs` `deliver`의 `Sent::Accepted`). SIGTERM/SIGINT는 새 전달을 503으로 막고, 진행 중인 전달과 그 회차가 끝날 때까지 서비스한 뒤 멈춘다. 한도는 기본 780초(회당 창 600초 + 호출자 여유 60초 × 2 + 60초)이고, 더 긴 창의 회차가 진행 중이면 그 회차의 한도까지다(`delivery::shutdown_wait`, `IEO_COORD_SHUTDOWN_WAIT_SECS`로 덮어씀). 정지 유예가 그보다 짧은 강제 종료(`podman stop`의 기본 10초, SIGKILL, 호스트 재시작)는 여전히 `inflight.json`만 남기고 기록을 잃는다. 그러면 예산이 멈추고(미계측), 배정은 다음 전달을 `ASSIGNMENT_USAGE_UNMEASURED`로 거절하며, 예비로 풀 수 없다(`spare_releasable`) | `delivery.rs`(`Finishing`, `shutdown_wait`, `SHUTDOWN_WAIT_DEFAULT_SECS`), `bin/ieo-coord.rs`(신호 스레드, `serve` 뒤의 두 번째 기다림) | **재시작 전 진행 중 회차가 없는지 확인(읽기 전용):** (1) 02-05 장부 디렉터리에 `inflight.json`이 없다. (2) 장부 잠금 파일(`caller.lock`)을 연 프로세스가 없다(`fuser`·`lsof`로 보기만 한다. `flock`으로 잠그지 않는다). (3) 두 역할 서비스의 `GET /v0/role/status`(어댑터 자격)가 `running: null`. (4) 최근 전달의 결과가 `accepted`인데 장부에 그 시도의 기록이 없는 것이 없다(연결 조회 `GET …/chain`). 넷 모두 참일 때만 멈춘다. 멈출 때는 정지 유예를 **900초 이상**으로 준다(`podman stop -t 900`: 기다림 780초 + 전달의 진행 창 120초. PR #83 검토 67b5cae L-1) |
| 12 | **(b) 예비의 모양과 전제.** (2026-11-17 병합 뒤 정정, §11) 예비를 붙이기 전에 사람이 실패한 회차를 먼저 본다. **관문이 실제로 보는 전제**(`caller::spare_gate`·`spare_releasable`): 대체할 회차가 이 예산의 개발 회차로 장부에 기록이 하나뿐이고, 그 유효 기록이 실패(`stop_required`, 또는 깨끗한 성공이 아닌 결과)이며, **사용량이 계측돼 있고**(`usage.run.state == "measured"`) **멈춤 사유가 회차 자신의 결과뿐**(`result …`, `no result event`)이어야 한다. 예비가 아직 쓰이지 않았고 예산·몫이 남아 있어야 한다. 이 전제가 아니면 예비는 불가능하다: 사람은 보고하고 사용자가 정한다 | `caller.rs` `spare_gate`·`spare_releasable`, `delivery.rs` `choose_op`, `serve.rs`(after_run·error 체크포인트) | 장부의 실패 회차 기록(`usage.run.state`, `stop_reason`, `outcome.result`), 연결 조회의 체크포인트 `phase` |
| 13 | **(c) 경로·신선도·한도.** `first_observation`은 R1 시점에 7일 이내여야 한다(아니면 그 호출은 보류). `ledger_dir`와 관측 stream은 코디네이터 프로세스가 읽고 쓸 수 있고 재배포 뒤에도 남는 경로여야 한다(컨테이너 안 임시 경로이면 재배포 때 장부가 사라진다). 개발 배정은 `branch_ref`가 있어야 R2가 GitHub head를 대조한다. sprint revision·배정의 `limits`(`elapsed_seconds`·`total_tokens`)가 600초·150,000 계획을 덮을 만큼 남아야 한다(아니면 회차 창이나 토큰 방이 잘려 가드 멈춤 → 미계측 또는 한도 멈춤, 예비로 풀 수 없음). 계획의 `max_window_seconds`는 600 이하. R1·R2 지시는 짧게(가드 멈춤은 예비가 풀지 못한다) | `budget.rs:148,291`(`OBSERVATION_MAX_AGE_SECONDS`), `caller.rs:616`(`max_window_seconds`), `api.rs:4599`(`branch_ref` 없음 → 대조 불가), `limits.rs`(배정 한도), `run.rs:662` `charge_reported`(가드 멈춤 → 미계측) | `roles.json`의 `budget_plan`(`ledger_dir`, `first_observation.observed_unix`, `max_window_seconds`)과 코디네이터 컨테이너의 마운트, 배정 요청의 `branch_ref`·`limits`, sprint revision의 `limits` |
| 14 | **(d) 코디네이터가 유일한 시작 경로.** 두 역할 서비스 모두 `checkpoint_central`이 코디네이터여야 한다(`local_only`가 아님). 중앙 체크포인트 모드에서는 전달 없는 start의 takeover가 거절돼 아무것도 실행되지 않는다(`serve.rs:1192-1199`). `local_only`이면 어댑터 토큰을 가진 누구든(자기 계획·장부를 쓴 `ieo-role-call` 포함) 두 번째 장부로 02-05 회차를 시작할 수 있고 예비 규칙도 우회된다(볼륨의 보조 상한만 남음) | `serve.rs:1192-1199`, `serve.rs:494`(`local_only` 표시), 런타임 계약 §15 항목 16 | 두 역할 서비스 설정의 `checkpoint_central`(값이 URL·자격 파일), 상태 응답의 중앙 비교 표시(`central checkpoint comparison off (local_only)`가 없음). 코디네이터 `tokens.json`에 두 supervisor의 `agent` 주체(`checkpoint:report`, 그 역할)가 있는지, `roles:control` 보유 주체 목록(이름·종류만), 단계 8 동안 이 서비스들에 `ieo-role-call`을 쓰지 않는다 |
| 15 | **(e) 새 표가 미리 있지 않다.** `migrate`는 `CREATE TABLE IF NOT EXISTS`라 main이 아닌 빌드가 만든 같은 이름의 표가 있으면 그 모양을 그대로 둔다 | `schema.sql:291`(`role_checkpoints`)~`:581`(`role_recovery_uses`), 항목 2의 11개 표 | 재배포 **전** 영속 DB(55433)에서 읽기 전용으로 `SELECT table_name FROM information_schema.tables WHERE table_name IN ('role_checkpoints','role_tamper_events','role_first_actions','role_session_status','role_deliveries','role_delivery_results','role_takeovers','assignment_run_usage','assignment_limit_events','role_recoveries','role_recovery_uses')` → 0행이어야 한다. 있으면 그 열 정의를 schema.sql과 대조하고 사람이 정한다 |

**R1·R2·G1이 각각 보내는 것(코드에서).** 모두 `roles:control` 자격(예비가 없으면 서비스 자격도 된다)으로 `POST /v0/repos/{repo}/roles/{role}/deliveries`(멱등 키 필수)를 보낸다. 본문 키는 `assignment_id`·`generation`·`attempt_id`·`session_id`·`prompt`(그리고 선택 `logical_name`·`new_session`·`recovery_id`·`spare`)뿐이다. 코디네이터는 재개 판단 → 배정 한도 → 공유 예산 검사(02-05 장부, 잠금) 뒤 역할 서비스에 start 또는 reconnect를 보낸다. 그 본문: `attempt_id`, `prompt`, `coordinator`(generation·assignment_id·sprint_id·sprint_revision·spec_digest·image_digest·profile_id·profile_revision), `limits`(`timeout_ms`·`grace_ms`·`billable_room`·`cache_read_room`·`timeout_is_total`), start는 `session_id`·reconnect는 `new_session_id`, 그리고 `target_sha`·`delivery_id`·`branch_ref`(배정에 있으면)·재개면 `github`(branch_ref·head_sha·ci). 예비 표식은 역할 서비스로 가지 않는다.
- **R1(개발).** 중앙 체크포인트 없음 → `no_checkpoint` → 새 세션 **start**. 역할 서비스가 takeover 체크포인트(그 `delivery_id`·`attempt_id`)를 보고하고 첫 행동을 `recordFirstAction`으로 보고하며(이 PR의 조건: 그 전달에 연결된 takeover), 끝에 after_run.
- **R2(개발).** after_run이 최신이고 GitHub head가 맞으면 `resume_allowed` → **reconnect**(같은 세션 재개, `github` 포함). R1이 실패했으면 R2는 시작하지 않는다(“asks for a person”). 그 경우 사람이 원인 보고 뒤 예비를 붙일 수 있다(§6 항목 7).
- **G1(Guardrail).** Guardrail 배정의 첫 전달 → **start**(대상 SHA가 현재 개발 `target_sha`). G1은 실패해도 다시 하지 않는다.
- **예비(개발, R1 또는 R2가 실패했을 때만, 사람 자격으로).** (2026-11-17 병합 뒤 정정, §11) 사람이 원인 보고·증거 참조를 남기고 항목 12의 전제를 먼저 본다. 본문은 위 키에 `spare {reason, replaces, evidence_ref}`를 더한다. 어느 연산이 되는지는 재개 판단이 정한다(`choose_op`):
  - **전제를 채우고 최신 체크포인트가 `after_run`, 실패한 회차의 세션이 실제로 있었다**(stream에 그 세션의 `system`/`init`) → 표식만 단 전달은 **reconnect**로 그 세션을 이어 간다(본문의 `session_id`는 쓰이지 않는다). 세션을 새로 시작하려면 `new_session: true`(**start**). 실행 환경 결함으로 실패한 회차를 다시 하는 예비는 원칙적으로 `new_session: true`를 권한다.
  - **전제를 채우고 최신 체크포인트가 `after_run`이지만 세션이 생기기 전에 실패했다**(`init`이 없음) → 반드시 `new_session: true`. 표식만이면 reconnect의 `--resume`이 없는 세션을 찾다 실패해 예비가 쓰인다.
  - **전제를 채우지 못한다** — 최신 체크포인트가 `error`·`takeover`인 회차(실행이 실패하거나 중간에 끊긴 회차: 호출자가 답을 잃은 기록으로 쓰므로 사용량이 미계측), 가드가 멈춘 회차(시간·토큰 상한, overage: 미계측), 정리 실패나 구독 전용이 아닌 환경으로 멈춘 회차 → **예비를 쓸 수 없다.** 예산은 멈춘 채이고(미계측 회차는 모든 호출을 멈춘다), 사람이 보고하고 사용자가 정한다. 복구 승인(`recovery_id`)을 더해도 관문이 거절한다(복구는 소비되지 않지만 쓸모가 없다).
  - 예비는 한 번뿐이다. 같은 멱등 키로 다시 보내면 저장된 답이 오고 새 회차는 없다.

## 7. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 점검표 확인 | §6은 코드·문서로만 만들었다. 실행 중인 빌드, 영속 DB, 비공개 바인딩, 자격 설정은 이 작업이 볼 수 없다 | 원장 담당, 단계 8 전 |
| 동등 변이 P7 | 정리 실패 문구가 가드를 지나지 않아도 결과는 같다(§5) | — |
| 이월 I-9(PR #75 검토 debf413) | 바이트가 같은 복사 start가 `refused`로 기록되는 것은 그대로(신뢰 경계 안, 기존 기록) | 기존 기록 그대로 |
| 사용 뒤 풀리지 않는 체크포인트(검토 d336849 L-c) | 그대로. 해제 경로는 단계 9 | 02-05 단계 9 |
| 강제 종료 때 회차 기록(검토 63fefc1 M-1 (a)) | 이 PR은 정상 멈춤(SIGTERM/SIGINT)에서 받아들인 회차를 기다린다(기본 720초(병합 뒤 780초로 정정, §11)). 그보다 짧은 정지 유예, SIGKILL, 호스트·컨테이너 강제 종료에서는 여전히 기록이 없다. 끝내는 스레드가 프로세스 안에 있는 구조는 그대로다(회차 기록을 프로세스 밖에서 이어 쓰는 방법은 없다) | 운영: 단계 8 절차서(§6 항목 11). 구조: 02-05 단계 9(실행 중 회차의 인계·대조) |
| 예비 + `recovery_id` 종단 | `error` 체크포인트 뒤의 예비(복구 승인 + 표식)는 caller·복구 시험이 각각 다루지만 둘을 함께 쓴 종단 시험은 없다 **(병합 뒤 정정: 불가능, §11. 그런 회차는 미계측이라 관문이 거절하므로 시험할 경로가 아니다)** | — |
| 영속 DB | 스키마 변경 없음(이 PR) | — |

## 8. 검토 잔여 처리

| 항목 | 처분 |
| --- | --- |
| PR #81 검토 291a9b0 N-1(예비 경로 종단 시험 없음, H5 생존) | 교정. `spare_e2e`. 변이 P1 |
| 291a9b0 N-2(멈춤 사유 문구에 기댐) | 교정. `other_stop_reason` 가드와 같은 상수. 변이 P5~P8 |
| 291a9b0 N-3(`approved_at`의 뜻) | 계약 문구(§19 항목 30, OpenAPI 0.17.1) |
| PR #75 검토 debf413 I-8(직접 첫 행동 보고와 채움의 조건 차이) | 교정. `takeover_linked` 하나. 단계 2 시험 고정값을 전달된 시도로. 변이 P2~P4. 동작 변경 §3 항목 1 |

## 9. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험, 커밋 e2dd5f3), [evidence/mutants.txt](evidence/mutants.txt)(15개).
- 시험 DB(55432) 정리: 이 작업의 실행이 남긴 `ieo_t_*`(변이 실행이 남긴 4개와 7개 포함)를 이름으로 지웠다. 끝난 뒤 `ieo_t_*`는 0개다. `/home`에는 아무것도 쓰지 않았다. 55433·58090은 건드리지 않았다.

## 10. 검토 반영(PR #82 검토 63fefc1)

| 항목 | 처분 |
| --- | --- |
| **M-1**(점검표가 회차를 망치거나 예비를 태우는 조건을 빠뜨림) | §6 항목 11~15와 “예비” 줄을 더했다. 각 항목에 근거(파일:줄)와 읽기 전용 확인 방법을 적었다 |
| M-1 (a)(실행 중 재시작 → 기록 없음) | 점검표 항목 11(재시작 전 진행 중 회차가 없음을 확인하는 절차). **코드도 교정**(e2dd5f3): 멈춤 신호에 새 전달을 503으로 막고, 받아들인 회차가 끝나 기록될 때까지 서비스한 뒤 멈춘다(한도 기본 720초(병합 뒤 780초로 정정, §11)). 시험 `spare_e2e` 7단계·`delivery.rs` 단위, 변이 Q1~Q3·Q6. 강제 종료는 §7(운영: 절차서, 구조: 02-05 단계 9). 동작 변경 §3 항목 3 |
| M-1 (b)(예비의 모양) | §6 항목 12와 “예비” 줄: after_run이고 세션이 있으면 reconnect 또는 `new_session: true`, 세션이 생기기 전 실패면 `new_session: true`, error·takeover면 복구 승인 + `recovery_id` **(병합 뒤 정정: 불가능, §11. error·takeover 회차는 미계측이라 예비를 쓸 수 없다)** |
| M-1 (c)(경로·신선도·한도) | §6 항목 13 |
| M-1 (d)(코디네이터가 유일한 시작 경로) | §6 항목 14 |
| M-1 (e)(새 표가 미리 있음) | §6 항목 15(읽기 전용 질의) |
| L-1(세대 불일치 R82-B 생존) | 교정. `grpc.rs` 단언. 변이 Q5 |
| I-1(`spare_extra`가 `kind: user` 고정) | 교정. 호출자의 실제 종류. 변이 Q4 |
| I-2(N-2 가드는 규약) | 기록. 사유 목록은 `Vec<String>`이라 가드를 거치지 않은 사유를 막는 타입은 없다. 거치지 않은 사유는 결과 사유처럼 시작하지 않는 한 닫는 쪽이다(목록 밖은 막음). 새 사유를 더하는 변경의 검토 항목으로 남긴다 |
| I-3(예비 start·R2 실패 경로의 종단 시험 없음) | 기록. 호출자 수준 시험이 결정 §4.4를 채운다(`caller.rs`). §7의 예비 + `recovery_id` 행 **(병합 뒤 정정: 그 경로는 불가능, §11)** |

## 11. 병합 뒤 정정(PR #82 검토 4531772)

PR #82 병합(5d69102) 뒤 검토 4531772가 찾은 것을 반영했다. 코드는 후속 PR(feat/0205-pre8b, 1999494·6a19059)이고, 이 보고의 §3 항목 3, §6 항목 11·12와 “예비” 줄은 이 정정에 맞춰 그 자리에서 고쳤고 각 곳에 “(2026-11-17 병합 뒤 정정, §11)”을 달았다. 단계 8의 절차는 [단계 8 절차서](../step8-runbook-20261117/RUNBOOK.md)에 모았다.

| 항목 | 처분 |
| --- | --- |
| **L-2**(§6 “예비” 줄의 셋째 경우가 동작하지 않음) | **정정.** 병합된 §6은 “최신 체크포인트가 `error`(또는 `takeover`)면 복구를 승인하고 `recovery_id`와 표식을 함께 단다”고 적었다. 그런 회차는 호출자가 답을 잃은 기록(`record_lost`)으로 써서 사용량이 미계측이다. 그래서 관문(`spare_releasable`)이 거절하고 예산은 미계측 회차로 멈춘다(`budget::next`). 재계측도 해당하지 않는다. 그 경우는 “예비를 쓸 수 없음, 사람이 보고하고 사용자가 정함”으로 고쳤다. 항목 12에 관문이 실제로 보는 전제(계측됨, 결과 사유뿐)를 적었다. 닫는 쪽이라 비용은 없었다(복구는 소비되지 않음) |
| **L-3**(720초 기본값이 늦게 끝나는 회차를 놓침) | **교정**(1999494). `After::finish`는 창 + 여유(60초)를 기다리고, 멈춤을 보낸 뒤 여유를 한 번 더 기다린다. 그 뒤에 stream·자식 기록을 읽고 기록을 쓴다. 그래서 기본값을 **780초** = 600 + 2 × 60 + 60으로 올렸다(`SHUTDOWN_WAIT_DEFAULT_SECS`). 진행 중인 회차마다 자기 한도(창 + 2 × 여유 + 60초)를 세고, 더 긴 창(02-04 1,800초, X-02 3,600초)의 회차가 있으면 그 한도까지 기다린다(`shutdown_wait`). 항목 11의 `podman stop -t`는 780 이상. 시험: `delivery.rs` 단위(780의 산술, 긴 한도가 기다림을 늘림). 변이 R1~R3 |
| **L-4**(멈춤 검사를 지난 전달이 세지지 않음) | **교정**(1999494). 전달은 멈춤 표시를 읽기 **전에** 세진다(`Finishing`). 받아들인 회차는 그 전달의 셈이 풀리기 전에 자기 셈을 시작한다. 또 `main`이 작업자가 끝난 뒤 한 번 더 기다린다. 변이 R4(전달 셈 제거)는 살아남는다: 경쟁 구간을 결정적으로 만들 수 없고, `main`의 두 번째 기다림이 같은 회차의 기록을 지키려 한다(완전하지는 않다: 멈춤 검사를 지난 전달이 시작 마감 75초 뒤에 회차를 시작하면 기록이 `-t`를 넘길 수 있었다. 검토 67b5cae L-1, §11.1에서 호출 직전 다시 보기와 `-t 900`으로 좁힘). 검토자 탐침대로 닫는 쪽이다(`inflight.json`, 예산 멈춤) |
| I-4(항목 11 확인 (2)가 명령이 아님) | 정정. `fuser`·`lsof`로 보기만 하고 `flock`으로 잠그지 않는다 |

시험: `cargo test --workspace --locked` 408 통과, 0 실패([증거](../step8-runbook-20261117/evidence/test.log), 커밋 1999494). 변이 8개 중 7개가 잡혔다(R4는 위 이유, [증거](../step8-runbook-20261117/evidence/mutants.txt)).

### 11.1 PR #83 검토 67b5cae 반영

| 항목 | 처분 |
| --- | --- |
| M-1(start의 `session_id`가 UUID가 아니면 회차가 곧 실패하고 예비를 태움) | **교정**(072d99e). 판단이 `start`를 고른 전달의 `session_id`가 소문자 UUID가 아니면 `not_called`(이유 기록), 아무것도 부르지 않는다(`claude --session-id`는 UUID만 받는다). 판단 뒤라 전달 행은 남는다. 절차서 §5·§6은 회차마다 새 소문자 UUID를 요구한다. 시험 `spare_e2e` 0단계, `delivery.rs` 단위 `a_start_session_is_a_lowercase_uuid`, 종단 시험의 세션 이름은 고정 UUID로(`sid`). 변이 U1·U2. 호스트 경로(`ieo-role-call`)와 supervisor는 검사하지 않는다(단계 8은 코디네이터 경로만 쓴다) |
| L-1(멈춤 검사 한 번, 기다림 한 번 계산) | **교정**(072d99e). supervisor 호출 직전에 멈춤을 다시 보고 `not_called`로 끝낸다. 기다림은 기다리는 동안 한도를 다시 읽는다(`wait_finishing_bounded`). 가장 늦은 한도는 최댓값으로 남는다(검토자 변이 A, U5). 절차서의 `podman stop -t`는 900(780 + 전달의 진행 창 120). 변이 U3(다시 보기 제거)는 살아남는다: 신호가 첫 검사와 호출 사이에 와야 해 결정적이지 않다. 닫는 쪽이다 |
| L-2(남은 옛 문장) | 정정. §7·§10의 `recovery_id` 예비 문장에 “병합 뒤 정정: 불가능” 표시, §6 항목 11·12와 “예비” 줄에 정정 표시, “위 A의 글” 문구 |

**동작 변경(수용된 작업, §3의 추가분: §3 본문은 바꾸지 않았다).** (1) `deliverAssignment`: 판단이 `start`인데 `session_id`가 소문자 UUID가 아니면 `not_called`이다(지금까지는 supervisor를 불렀고 `claude`가 곧 실패했다). 재개(`reconnect`)는 그대로다. (2) 멈추는 코디네이터에서 이미 받은 전달도 supervisor 호출 직전에 멈춤을 보면 `not_called`이다.

시험: `cargo test --workspace --locked` 409 통과, 0 실패([증거](../step8-runbook-20261117/evidence/test.log), 커밋 5d1ca18). 변이 13개 중 11개가 잡혔다(R4·U3은 경쟁 구간, [증거](../step8-runbook-20261117/evidence/mutants.txt)).

## 12. 병합 뒤 정정(PR #83 검토 8336044)

PR #83 병합(ad96116) 뒤 검토 8336044가 찾은 것과 ad96116의 main CI 실패(첫 시도)를 반영했다. 코드는 후속 PR(feat/0205-pre8c, 79bf6a3·88b65d9·7519738·40fdfea)이고, 절차서의 정정은 [단계 8 절차서](../step8-runbook-20261117/RUNBOOK.md) §11이다.

| 항목 | 처분 |
| --- | --- |
| **L-A**(멈춤 다시 보기가 복구 승인의 사용 뒤에 있어 그 순간의 멈춤이 승인을 씀) | **교정**(79bf6a3). 다시 보기를 `record_use` **앞**(시작 마감 검사 바로 뒤)으로 옮겼다. 그래서 멈추는 코디네이터는 승인을 쓰기 전에 `not_called`로 끝내고, 그 승인은 다음 전달이 쓸 수 있다. 다시 보기와 `record_use` 사이에 신호가 오는 짧은 구간은 남는다(닫는 쪽: 승인만 쓰이고 예산·회차는 없음). 변이 W1(다시 보기 제거)은 신호 시점이 결정적이지 않아 살아남는다 |
| I-A(새 `not_called`에 `op`가 남음) | 교정. 비-UUID와 멈춤의 `not_called`는 `op`가 없다(시작 마감·복구 거절과 같음). 변이 W5 |
| I-B(재개에 비-UUID 세션을 보낸 시험 없음, 검토의 V4) | 교정. `delivery_e2e` 4단계의 reconnect가 본문 세션 `s9`(UUID 아님)로도 `accepted`. 변이 W4 |
| I-C, I-E(절차서) | 절차서 §11 |
| I-D(“§3에 더함”) | 위 A |
| I-F(두 기다림이 합쳐 약 1,560초) | 교정. 두 기다림 모두 멈춤 시각(`begin_stop`이 처음 정한 때)부터 센다. 두 번째 신호는 그 시각을 바꾸지 않는다. 그래서 합쳐서 한 한도(기본 780초 또는 진행 중 회차의 한도)를 넘지 않는다. `delivery.rs` 단위 `the_shutdown_waits_count_from_the_stop`, 변이 W2·W3 |

| **main CI 44003424073(ad96116) 첫 시도 실패** — `recovery_e2e::a_person_approves_one_new_session_over_an_unfinished_checkpoint_over_grpc`, “the coordinator did not start”(재시도는 성공) | **원인 조사**([실패 로그](../step8-runbook-20261117/evidence/ci-44003424073-attempt1-failed.log)). 그 시험 파일은 20.13초에 끝났다. 시험의 기동 대기는 20초이고, 중간의 재시작은 멈춤(최대 10초) 뒤에 오므로 그 실패는 **첫 기동**이다. 그래서 PR #82·#83의 멈춤 변경(진행 중 회차 기다림, `STOPPING`)과는 관계가 없다. 첫 기동이 20초 안에 답하지 못한 원인으로 가장 그럴듯한 것은 시험의 포트 고르기다: 포트 0을 묶어 얻은 포트는 임시 포트 범위(32768~60999)에 있고, 코디네이터가 그 포트를 묶기 전에 같은 범위에서 나가는 연결(PostgreSQL·역할 서비스로)의 지역 포트로 쓰일 수 있다. 그러면 코디네이터는 묶기 오류로 곧 끝나고 상태 조회는 답이 없다. 로그에 코디네이터 stderr가 없어(`/dev/null`) 확정은 못 했다. 로컬에서 그 시험을 13번 돌렸고 재현되지 않았다. **교정(시험):** 다섯 종단 시험의 `free_port`가 임시 범위 아래(20000~32767)에서 프로세스·호출마다 흩어 고르고 묶어 확인한다. `recovery_e2e`는 코디네이터 stderr를 파일에 남기고, 프로세스가 끝나면 곧 그 내용과 함께 실패한다(다음 실패의 원인이 보인다) |
| **조사 중 찾은 결함(실제)**: gRPC 쪽이 열린 연결을 기다려 SIGTERM이 끝나지 않음 | 재현하며 코디네이터 stderr를 보니 시험 안의 첫 멈춤에 “stopped”가 없었다. HTTP 쪽은 곧 끝났지만 gRPC 쪽(tonic의 우아한 멈춤)이 supervisor의 열린 HTTP/2 연결이 닫히기를 끝없이 기다렸고, 시험이 10초 뒤 SIGKILL로 끝냈다(X-04부터 있던 동작). 운영에서는 `--grpc-listen`에 클라이언트가 붙어 있으면 `podman stop -t 900`이 900초를 다 기다린 뒤 SIGKILL이 된다(회차 기록은 그 전의 기다림으로 이미 써졌으므로 잃지 않지만, 깨끗한 종료가 아니고 재시작이 늦다). **교정**(7519738): 멈춤 뒤 gRPC 쪽은 진행 중인 호출에 가장 긴 호출 마감(`stop_grace` = 최대 마감 15초 + 여유 1초 = 16초)만 주고, 그 뒤에는 열린 연결이 있어도 내려간다. `recovery_e2e`의 재시작은 이제 코디네이터가 스스로 0으로 끝나는지 단언한다(멈춤은 SIGTERM 뒤 30초까지 기다리고 SIGKILL이면 실패). 변이 G1(한도 없음)은 잡힘, G2(한도가 호출 마감보다 짧음)는 살아남음(멈춤을 가로지르는 gRPC 호출 시험 없음). 단계 8 위험 질문에 대한 답: 이전 코디네이터가 포트·DB를 쥐고 남는 경로는 없다(재시작은 이전 프로세스가 끝난 뒤에 시작되고, 끝나면 포트·연결이 풀린다). 남은 것은 위의 늦은 종료뿐이었고 고쳤다 |

**동작 변경(수용된 작업).** 코디네이터의 멈춤(SIGTERM/SIGINT, 02-04 수용, X-04 gRPC): 지금까지 gRPC 쪽은 진행 중 호출과 **열린 연결이 모두 닫힐 때까지** 기다렸다. 이제 멈춤 뒤 16초(진행 중 호출의 가장 긴 마감)까지만 기다리고 내려간다. 그보다 긴 호출은 없으므로(서버 시간 제한이 같은 값) 답을 잃는 호출은 없다. 그 밖에는 없음. 멈추는 코디네이터가 복구를 단 전달을 `not_called`로 끝낼 때 승인을 쓰지 않게 된 것은 이 PR들 안의 새 경로(PR #82·#83)를 좁힌 것이다. 결과의 `op`가 비는 것도 그 새 `not_called`에만 해당한다.

시험: `cargo test --workspace --locked` 410 통과, 0 실패([증거](../step8-runbook-20261117/evidence/test-pre8c.log), 커밋 7519738). 변이 19개 중 16개가 잡혔다(R4·W1은 신호 시점 경쟁, G2는 위 이유, [증거](../step8-runbook-20261117/evidence/mutants-pre8c.txt)). 이전 증거 파일(`test.log`, `mutants.txt`)은 그대로 두었다.
