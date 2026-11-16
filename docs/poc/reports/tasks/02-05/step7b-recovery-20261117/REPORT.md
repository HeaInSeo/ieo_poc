# 02-05 단계 7b · 충돌 뒤 공식 복구

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 단계 7b. [사용자 결정 2026-11-16 (나)](../../../../User_Decisions_2026-11-16_recovery.md)의 공식 복구 작업. PR #74 검토 fe924b7 I-4의 남은 틈(충돌 뒤 복구 경로가 없음)을 닫는다 |
| 착수 근거 | 단계 7 병합(PR #79, main 78d7fe5) |
| 검토 반영 | PR #79 검토 47e4e3f I-3·L-4, CI에서 한 번 실패한 `grpc.rs` 시험, 이월 I-8(§9) |
| 기준 커밋 | main 78d7fe5. 잔여 교정 e5e88aa, 복구 구현 3481111, 시험 보강 180a2a4, 증거 76ab655(evidence/test.log 첫 줄이 180a2a4, 작업 트리 깨끗) |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 실제 GitHub도 읽지 않았다. 영속 PoC DB(55433)·58090 코디네이터는 쓰지 않았다(일회성 시험 DB 55432만). 추가 청구 USD 0, 02-04 예비 5회 미사용 |
| 상태 | 구현과 시험 완료, **독립 검토 전**. 원장(`state.json`)은 고치지 않았다(원장 담당이 기록) |

## 1. 사용자 조건과 대응

조건 번호는 [결정 기록](../../../../User_Decisions_2026-11-16_recovery.md) §2의 것이다. 시험은 별도 표기가 없으면 `crates/ieo-coord/tests/recovery_e2e.rs`(실제 PostgreSQL, 코디네이터 바이너리, 가짜 런타임의 supervisor, 두 전송)의 단계 번호다. 변이는 §5.

| 조건 | 코드 | 시험 | 변이 |
| --- | --- | --- | --- |
| 2.1 승인은 사람으로 확인된 운영 주체만. `roles:control`과 호출 주체를 함께 확인. 개발·Guardrail 자격의 자체 승인 거부 | `recovery::need_approver`: `roles:control` + 저장소 범위 + `kind == user`. 서비스 → 403 `HUMAN_APPROVAL_REQUIRED`, 에이전트(그 역할의 supervisor·개발 에이전트 자격) → 403 `SCOPE_DENIED`. 표의 `approver_kind`는 DB CHECK로 `user`만 | 단계 3: 운영 서비스·supervisor 자격·개발 에이전트·범위 없는 사용자·다른 repo 사용자 거절, 기록 0 | B1 |
| 2.1 승인자·사유·시각, 대상 배정·세대·미완료 체크포인트를 중앙에 기록 | `role_recoveries`(추가 전용): 승인자, 자격 종류, 사유, 시각, 배정·세대, 체크포인트 순번·digest·단계, 상태, 보류 사유, 읽은 사실, GitHub 대조 | 단계 6: 기록 내용 전부 확인, 같은 키 재요청 200, 다른 내용 409, 조회 | B3 |
| 2.1 다음 전달은 승인된 대상에 한해 새 세션 시작. 재개와 구분 | 전달의 `recovery_id` → 판단 `recovery_allowed`, 호출은 늘 `start`(그 전달의 새 세션). 전달 결과와 배정 연결에 `recovery`, supervisor 체크포인트에 `recovery_id` | 단계 9: `recovery_allowed`·`start`·새 세션 s6(`mode: new`), 체크포인트의 `recovery_id`, 연결의 `recoveries`와 그 전달의 `recovery` | B4, B19, B20 |
| 2.2 단계 7의 GitHub head 대조를 재사용, 신뢰 입력인지 실제 수집인지 명시 | `github_head_check`를 승인과 사용에서 그대로 부름. 기록의 `source`가 “evidence:ingest (trusted service input; not a live GitHub read)” | 단계 5: GitHub 불일치 → 보류와 `source` 확인. 단계 6: `github.result: match` | B9 |
| 2.2 기존 실행이 진행 중이면 거부 | supervisor 상태의 `running`이 있으면 409 `RECOVERY_RUNNING`(기록 없음) | 단계 1: SLEEP 회차가 도는 동안 그 takeover에 대한 승인 거절 | B5 |
| 2.2 실행 종료나 외부 요청 결과를 확인할 수 없으면 “확인 필요”와 근거를 남기고 보류 | `needs_confirmation`으로 기록(쓸 수 없음): supervisor 미관측, 로컬≠중앙, 대기 외부 요청이 빈 목록이 아님(읽을 수 없음 포함), 그 역할에 결과 모르는 전달, GitHub head가 `match`가 아님. 근거는 `reasons`·`facts`·`github` | 단계 5: 미관측, GitHub 불일치, supervisor 대기 목록, 결과 모르는 전달 → 넷 모두 보류. 보류를 쓴 전달은 부르지 않음 | B6, B7, B8, B9, B13 |
| 2.2 승인 뒤 체크포인트·세대·head가 달라지면 다시 대조 | 전달의 판단마다(예산 잠금 뒤 포함) `recovery::usable`이 승인 때의 검사를 모두 다시 함. 사용 기록에 그때의 GitHub 대조와 승인 때의 head(`head_at_approval`) | 단계 7: 승인 뒤 head가 바뀜 → 부르지 않음·미사용. 단계 9: 같은 SHA의 새 증거로 다시 대조해 통과, 두 head 증거 id가 기록됨. 단계 4·6 뒤: 끝난 체크포인트·다른 순번·digest·세대·배정 → 409 | B10, B14, B15, B16 |
| 2.2 승인 하나는 한 실행에만. 중복 요청·응답 유실로 두 번 실행되지 않게 | 사용(`role_recovery_uses`, 승인당 하나·체크포인트당 하나)을 start를 보내기 직전에 커밋. 같은 전달 키는 저장된 전달(재조회는 부르지 않음). 쓰인 승인은 판단에서 거절 | 단계 10: 같은 요청 → 저장된 전달, 다른 전달 → “already used”, 실행 수 그대로. 단계 11: 답 유실 → `unknown`, 사용 기록됨, 재조회 → `accepted`(requeried), 코디네이터·supervisor 재시작 뒤 같은 답, 실행 수 그대로, 사용 둘 | B11, B17 |
| 2.3 이전 체크포인트·실험 증거 보존, 복구 이력 추가 | 두 표 모두 추가 전용(UPDATE·DELETE 금지 트리거). 체크포인트·전달·증거는 바꾸지 않음 | 단계 9: 충돌한 takeover 체크포인트가 연결에 그대로 | — |
| 2.3 누적 사용량·미계측 유지. 승인이 예산 초기화나 추가 호출 승인이 되지 않게 | 승인은 사용량 표와 원장을 건드리지 않음. 복구 전달은 배정 한도(미계측 → 409)와 공유 예산 검사를 그대로 거침. supervisor 보조 상한도 그대로 | 단계 6: 승인 전후 배정 사용량 같음. 단계 8: 미계측 기록 → `budget_refused`, 미사용. 단계 11 끝: 답 유실의 미계측 → 다음 전달 409 `ASSIGNMENT_USAGE_UNMEASURED`. supervisor 시험: 오류 회차 뒤 `recovery_id`가 있어도 `BUDGET_EXCEEDED` | — |
| 2.3 추가 청구 USD 0, 02-04 예비 5회 미사용 | 모델 호출 없음 | 전체 | — |
| 2.4 실제 DB와 가짜 런타임으로 정상 복구·권한 거부·실행 중 거부·대상 불일치·중복 요청·재시작·응답 유실 | §4 | 위 단계 1~11 | §5 |
| 2.4 새 모델 호출 없음 | — | — | — |
| 2.4 자동 복구 가능 조건과 사람 판단 조건을 후속 요구사항으로 | §6 | — | — |
| (지시) supervisor 직접 start는 그 복구 id 없이 거부, `local_only`도 고려 | `serve.rs` launch: 로컬 체크포인트가 takeover·error이면 `recovery_id` 없는 start는 409 `EXTERNAL_RECONCILIATION_REQUIRED`. `local_only`에도 적용하되 id를 대조할 코디네이터가 없음(§7) | `tests/serve.rs` `a_start_over_an_unfinished_run_needs_a_recovery`(local_only): takeover·error 위 거절, 잘못된 id 400, id가 있으면 새 세션과 체크포인트의 id, 끝난 회차 뒤에는 필요 없음. 종단 단계 2: 코디네이터 모드의 직접 start 거절 | B18, B19 |

## 2. 더한 작업

### 2.1 코디네이터(`crates/ieo-coord/src/recovery.rs` 새 파일, `delivery.rs`, `chain.rs`, `api.rs`, `schema.sql`)

- **approveRecovery** `POST /v0/repos/{repo}/roles/{role}/recoveries`(HTTP 전용, `Idempotency-Key`). 본문: `assignment_id`, `generation`, `checkpoint_seq`, `checkpoint_sha256`, `reason`. 순서: 역할 404 → 403(범위·저장소·사람) → 400 → 같은 키(저장된 승인 200, 다른 내용 409) → supervisor 상태(트랜잭션 밖) → 한 트랜잭션(체크포인트 잠금): 거절 409(기록 없음) 또는 `approved`·`needs_confirmation`(201).
- 대상 검사: 현재 배정·세대가 머리와 같고, 이름 댄 체크포인트가 중앙 최신이며 단계가 takeover·error이고, 그 체크포인트의 배정이 같고 세대가 머리 이하. Guardrail은 검토 대상이 현재 개발 target과 같아야 한다.
- **getRecovery** `GET …/recoveries/{id}`(`roles:control`): 승인과 그 사용.
- **deliverAssignment `recovery_id`**: §1의 다시 대조와 사용 기록. 쓸 수 없으면 판단은 compare의 것이고 첫 사유가 이유를 말하며 아무것도 부르지 않는다. 쓸 수 있으면 `recovery_allowed`.
- 스키마: `role_recoveries`, `role_recovery_uses`(추가 전용 트리거). 기동 때 만든다.
- 배정 연결: 확인 수준 `verified`에 `recoveries`를 더하고 각 전달에 `recovery`.

### 2.2 supervisor(`serve.rs`)

- start는 로컬 체크포인트가 takeover·error이면 `recovery_id`가 있어야 한다. 있으면 새 세션을 시작하고 체크포인트(takeover·after_run·error)에 `recovery_id`를 적는다. reconnect는 바꾸지 않았다(끝나지 않은 회차 위의 reconnect는 이미 거절).

### 2.3 단계 7·이전 잔여

- **검토 47e4e3f L-4.** 체크포인트에 `branch_ref`가 없으면 그 `pushed_sha`를 읽지 않고 target SHA를 기대한다(런타임 계약 항목 22의 문장에 코드를 맞춤). `delivery_e2e` 단계 12에 시험. 변이 B22.
- **CI에서 실패한 `a_statement_past_the_deadline_rolls_back_as_deadline_exceeded`.** 원인은 코드의 경쟁이다: 클라이언트 deadline이 없을 때 tonic의 시간 제한 계층이 서버 상한과 같은 시각(700 ms)에 처리 future를 버려 `CANCELLED`로 답했고, 서버의 `DEADLINE_EXCEEDED`(작업 기한 650 ms의 문장 시간 제한 → 롤백 → 답)는 부하에서 50 ms 안에 나가지 못하면 졌다. 고침: 계층을 서버 상한 + 1초로 두어 서버 자신의 답이 늘 먼저다(`grpc.rs` `TRANSPORT_SLACK`). 클라이언트 deadline이 있으면 계층은 그 deadline 그대로다. 시험은 코드뿐 아니라 메시지(“rolled back”, 서버의 답)도 확인한다. 내부 계약 §6에 반영.

### 2.4 계약

- 런타임 계약 **revision 15**: 머리 기록, 항목 22의 문장 하나(L-4), 새 §18 항목 25~29(승인하는 사람, 대상과 기록, 쓰기, supervisor, 사용량과 예산). **뜻이 바뀐 동작이 하나 있다**: supervisor start가 끝나지 않은 체크포인트 위에서 `recovery_id`를 요구한다.
- OpenAPI 0.15.0 → **0.16.0**: approveRecovery, getRecovery, `Recovery` 스키마, deliverAssignment `recovery_id`, 판단 값 `recovery_allowed`, 전달의 `recovery`, 배정 연결 설명. 머리 기록에 동작 변경. `openapi-spec-validator` OK.
- 내부 계약(`contracts/ieo-internal-v0.md`) §6: 시간 제한 계층은 “`grpc-timeout`과 서버 상한 + 1초 중 짧은 쪽”.
- proto는 바꾸지 않았다(복구는 HTTP 전용 운영 작업).
- **영속 DB**: 기동 때 두 표가 생긴다. 영속 PoC DB(55433)에는 적용하지 않았다.

## 3. 수용된 작업의 동작 변경

사용자에게 알릴 항목이다.

1. **supervisor start(02-04 수용).** 로컬 체크포인트가 takeover·error(끝나지 않은 회차)이면 `recovery_id` 없는 start는 409 `EXTERNAL_RECONCILIATION_REQUIRED`다. 02-04에서는 이것이 충돌 뒤의 설계된 복구(사람이 새 세션 start)였다. `local_only`(시험, 02-04 호스트 호출자)도 같다: 그 경로의 복구는 이제 `recovery_id`를 함께 보내야 하고, 코디네이터가 없어 그 id는 대조되지 않는다(§7). 답을 잃은 takeover가 대기 사본으로 채택된 뒤의 start도 여기에 든다(그 takeover는 끝나지 않은 회차다). 수용된 시험 넷의 start에 `recovery_id`를 넣었다: `a_run_error_leaves_an_error_checkpoint_and_stops_further_starts`(이제 먼저 409, `recovery_id`가 있어도 `BUDGET_EXCEEDED`), `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once`, `a_takeover_accepted_but_not_written_locally_is_adopted_next_time`, 단계 6의 `pending_external_requests_are_tracked_requeried_and_stop_a_reconnect`. 읽을 수 없는 체크포인트 위의 start는 바꾸지 않았다.
2. **gRPC 서버(02-04 수용, 내부 계약 §6).** 클라이언트 deadline이 없을 때 시간 제한 계층이 서버 상한 + 1초다. 서버의 답(`DEADLINE_EXCEEDED`, 롤백)은 전처럼 상한 50 ms 전이고, 같은 시각의 `CANCELLED`와 겨루지 않는다. 클라이언트 deadline이 있으면 바뀌지 않았다.
3. 02-05 진행 중 작업(수용 전)의 변경: compare의 GitHub 대조는 체크포인트에 브랜치가 없으면 `pushed_sha`를 읽지 않는다(L-4, 닫는 쪽). 배정 연결의 `evidence_levels.verified`에 `recoveries`가 늘었다(수용 전 시험 `claims_are_kept_apart_from_the_completion_judgment`의 기대를 바꿈). deliverAssignment는 `recovery_id`를 받는다(추가).

## 4. 시험

새 시험(3개):

| 시험 | 내용 |
| --- | --- |
| `ieo-coord` `tests/recovery_e2e.rs`(HTTP·gRPC) | 아래 표 |
| `ieo-supervisor` `tests/serve.rs` `a_start_over_an_unfinished_run_needs_a_recovery` | §1 마지막 행 |

넓힌 시험: `operations.rs`(두 작업의 경로와 실행 모양), `delivery_e2e`(L-4), `grpc.rs`(서버의 답 확인), 위 §3의 supervisor 시험 넷, `coordinator.rs`의 확인 수준 목록.

`recovery_e2e` 단계(충돌은 supervisor가 회차 전에 보고하는 takeover 체크포인트를 고정 전달과 함께 만들어 흉내 낸다. 그 회차의 예산 기록은 02-04처럼 사람이 정리한 것으로 둔다):

| 단계 | 확인 |
| --- | --- |
| 1 | 회차가 도는 동안 그 takeover에 대한 승인 → 409 `RECOVERY_RUNNING`, 기록 0 |
| 2 | 충돌 뒤 compare 외부 대조, 새 세션 전달 `not_called`, 코디네이터 모드의 supervisor 직접 start 409 |
| 3 | 서비스·supervisor·개발 에이전트·범위 없는 사용자·다른 repo 사용자 → 403. 키 없음 400 |
| 4 | 다른 순번·digest·세대·배정 → 409 `RECOVERY_TARGET_MISMATCH`, 기록 0 |
| 5 | 보류 넷(미관측, GitHub 불일치, supervisor 대기 목록, 결과 모르는 전달) `needs_confirmation`과 근거. 보류를 쓴 전달 → `not_called`(“not approved (needs_confirmation)”) |
| 6 | 승인 기록(승인자 alice-ops·`user`, 사유, 시각, 배정 A1/0, 순번·digest·takeover, GitHub `match`), 같은 키 200, 다른 내용 409, 조회, 승인 전후 사용량 같음 |
| 7 | 승인 뒤 head 변경 → 다시 대조, `not_called`, 미사용 |
| 8 | 미계측 기록 → `recovery_allowed`·`budget_refused`, 미사용 |
| 9 | 사용: `recovery_allowed`·`start`·`accepted`, 새 세션, 승인자와 head 두 증거 id, 보낸 본문의 `recovery_id`, after_run 체크포인트의 id, 끝난 체크포인트 승인 409, 사용 기록, 연결의 `recoveries`·전달의 `recovery`·충돌 체크포인트 보존 |
| 10 | 같은 전달 키 → 저장된 전달, 다른 전달 → “already used”, 실행 수 그대로 |
| 11 | 두 번째 충돌, 승인, 답 유실(`unknown`), 사용 기록, 재조회 `accepted`, 코디네이터·supervisor 재시작 뒤 같은 답, 다음 전달은 미계측으로 409, 사용 둘, 실행 수 그대로 |

결과([evidence/test.log](evidence/test.log), 커밋 180a2a4):
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음.
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **395 통과, 0 실패, 0 무시**. 기준(78d7fe5) 392에 새 시험 3개.
- 공유 대상 디렉터리의 다른 작업 트리 바이너리를 피하려고 작업 공간 일곱 패키지를 `cargo clean -p` 한 뒤 증거를 만들었다.

## 5. 변이 시험

각 변이를 하나씩 3481111 코드에 넣고(180a2a4는 확인 하나만 더함) 대상 시험을 돌린 뒤 파일을 되돌렸다(매번 `git status` 깨끗, [evidence/mutants.txt](evidence/mutants.txt)). **19개 모두 잡혔다.**

| id | 변이(조건) | 잡은 시험 |
| --- | --- | --- |
| B1 | 서비스 자격이 승인할 수 있음(2.1) | `recovery_e2e` |
| B3 | 승인자가 다른 값으로 기록됨(2.1) | 같음 |
| B4 | 복구가 재개로 감(2.1) | 같음 |
| B5 | 실행 중에도 승인(2.2) | 같음 |
| B6 | 대기 외부 요청이 보류가 아님(2.2) | 같음 |
| B7 | 결과 모르는 전달이 보류가 아님(2.2) | 같음 |
| B8 | supervisor 미관측이 보류가 아님(2.2) | 같음 |
| B9 | GitHub 불일치가 보류가 아님(2.2, I-3) | 같음 |
| B10 | 사용 때 다시 대조하지 않음(2.2) | 같음 |
| B11 | 쓰인 승인을 다시 씀(2.2) | 같음 |
| B13 | 보류를 쓸 수 있음(2.2) | 같음 |
| B14 | 아무 체크포인트나 이름 댈 수 있음 | 같음 |
| B15 | 끝난 회차도 복구 | 같음 |
| B16 | 다른 배정·세대를 이름 댈 수 있음 | 같음 |
| B17 | 사용을 호출 전에 기록하지 않음(2.2) | 같음 |
| B18 | supervisor가 id 없이 끝나지 않은 회차 위에서 start | serve `a_start_over_an_unfinished_run…` |
| B19 | 회차가 복구 id를 기록하지 않음 | 같음 |
| B20 | start 본문에 복구 id가 없음 | `recovery_e2e` |
| B22 | 브랜치 없는 체크포인트의 `pushed_sha`를 읽음(L-4) | `delivery_e2e` |

돌리지 않은 것: B12(사용 기록의 충돌 확인. 판단이 쓰인 승인을 먼저 거절하므로 동시 사용에서만 닿는다. DB의 기본 키와 체크포인트당 고유 제약이 같은 일을 한다), B21(gRPC 계층의 여유. 시간 경쟁이라 한 번 돌려서는 판정되지 않는다. 고친 시험이 서버의 답을 확인한다).

## 6. 정식 설계 후속: 자동 복구와 사람 판단

이번 구현은 명시적 승인 경로뿐이다(조건 2.4). 정식 설계에 넘기는 요구사항:

| 조건 | 자동 복구 후보 | 사람 판단이 필요한 이유 |
| --- | --- | --- |
| 회차가 끝났음이 확인됨(supervisor 관측, 실행 없음, 세션 잠금 없음) | 예 | — |
| 대기 외부 요청이 빈 목록이고 결과 모르는 전달이 없음 | 예 | 하나라도 있으면 결과를 모른다: 사람 |
| 회차가 push하지 않았음(GitHub head = 회차 전 기대 SHA) | 예 | — |
| 충돌한 회차가 push함(GitHub head ≠ 회차 전 기대 SHA, I-3) | 아니오 | 그 커밋을 받아들일지(새 세션의 시작점) 버릴지는 사람의 판단. 후보: 승인에 “확인한 head”를 적게 하고 그 SHA를 새 기대값으로 기록 |
| 예산 원장에 그 회차의 기록이 없거나 미계측 | 아니오 | 사용량을 0으로 셀 수 없다(02-04 규칙). 사람이 원장을 정리 |
| 자식·모델의 직접 네트워크 요청 | 아직 판단 불가 | 04-05의 자식 추적과 관측 범위가 정해진 뒤 |
| 같은 체크포인트의 반복 충돌 | 아니오 | 원인 조사(사람). 자동 복구에는 횟수 상한이 필요 |
| 승인자 신원 | — | 지금은 자격 설정의 `kind`. 실제 운영은 F4 신원 규칙(1b)과 다단계 확인 |

담당: 02-05 단계 9(수용 보고)가 이 표를 정식 설계 후속 목록에 올린다.

## 7. 결정한 것

- **사람의 구분은 자격의 `kind`다.** 이미 실행 예외 승인(`exceptions:approve`)이 `user`만 가능한 것과 같은 근거다. 범위 `roles:control`과 `kind == user`를 함께 본다. 새 범위는 만들지 않았다. 한계: `kind`는 자격 설정 파일이 정하고 토큰 하나로 확인한다(§8).
- **I-3(충돌한 회차가 push함): 닫는 쪽.** 끝나지 않은 체크포인트의 `pushed_sha`는 회차 전 값이라, push한 뒤 충돌하면 GitHub 대조는 `mismatch`다. 이것을 `needs_confirmation`(보류)으로 기록하고 쓸 수 없게 했다. 사람이 브랜치를 정리하고(또는 증거 서비스가 그 결과를 넣고) 다시 승인한다. “확인한 head로 기대값을 바꾸는 승인”은 사람 판단의 새 권한이라 이번에 넣지 않고 §6에 후속으로 적었다.
- **자식 미추적은 보류 사유가 아니다.** `active_children`은 04-05까지 늘 `not_tracked`라 보류로 하면 복구가 영영 안 된다. 사용자 결정 2026-11-15 항목 3과 같이 기록의 `facts.limits`에 남겼다.
- **사용은 호출 전에 커밋한다.** 사용을 기록하고 호출 전에 코디네이터가 멈추면 승인은 쓰인 것으로 남고 아무것도 실행되지 않는다. 다시 하려면 새 승인이 필요하다(닫는 쪽). 반대 순서는 두 번 실행될 틈을 남긴다.
- **`local_only`.** supervisor의 거절은 `local_only`에도 걸린다. 코디네이터가 없으므로 `recovery_id`는 대조되지 않는 어댑터 호출자의 기록된 주장이다. 어댑터 자격은 신뢰 경계 안이다.
- **Guardrail.** 같은 작업이 Guardrail에도 쓰인다. GitHub 대조 대신 검토 대상 = 현재 개발 target을 본다. 종단 시험은 개발 역할만 했다(§8).

## 8. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 사람의 신원 | `user` 자격 = 사람은 자격 설정의 선언이다. 토큰 하나, 다단계 확인 없음 | 1b(F4) |
| GitHub 입력 | 대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다 | 1b |
| 충돌한 회차의 push(I-3) | 보류. 확인한 head로 기대값을 바꾸는 승인은 없음 | 단계 9(정식 설계 후속, §6) |
| Guardrail 복구 | 구현했으나 종단 시험은 개발 역할만 | 단계 9 전 또는 다음 PR |
| 동시 사용 | 승인당·체크포인트당 고유 제약으로 막지만 동시 전달 시험은 없다(B12) | 기록 |
| 사용 뒤 호출 전 중단 | 승인은 쓰인 것으로 남고 실행은 없음. 새 승인 필요 | 기록(닫는 쪽) |
| 영속 DB | 새 두 표를 영속 PoC DB에 아직 적용하지 않음 | 병합 뒤 원장 담당 |
| 이월 I-8(첫 행동 조건) | 이번 범위에 맞지 않아 손대지 않음 | 단계 8 전 정리(기존 담당 그대로) |

## 9. 검토 잔여 처리

| 항목 | 처분 |
| --- | --- |
| PR #74 검토 fe924b7 I-4 | 닫음. 사용자 결정 2026-11-16 (나)의 공식 복구(§1~§2). 코디네이터 모드와 `local_only` 모두 끝나지 않은 회차 위의 start는 승인된 복구로만 |
| PR #79 검토 47e4e3f I-3 | 닫는 쪽으로 처리: 불일치 → `needs_confirmation`(보류, 근거 기록), 사람이 먼저 브랜치를 정리. 기대값 변경 승인은 §6 후속 |
| 47e4e3f L-4 | 교정(§2.3). 시험과 변이 B22 |
| `grpc.rs` `a_statement_past_the_deadline_rolls_back_as_deadline_exceeded`(CI의 `Cancelled`) | 코드 경쟁으로 확인하고 고침(§2.3, §3 2번). 시험은 서버의 답(“rolled back”)도 확인 |
| 이월 I-8 | 손대지 않음(§8) |

## 10. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험, 커밋 180a2a4), [evidence/mutants.txt](evidence/mutants.txt)(19개).
- 시험 DB(55432) 정리: 변이 실행이 남긴 `ieo_t_*` 38개(모두 이 작업의 끝난 프로세스)를 이름으로 지웠다. 종단 시험은 실패하면 자기 DB를 지우지 않는다(기존 동작). 끝난 뒤 `ieo_t_*`는 0개다. 55433·58090은 건드리지 않았다.
