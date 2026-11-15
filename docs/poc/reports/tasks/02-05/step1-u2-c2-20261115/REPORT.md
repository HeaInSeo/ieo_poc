# 02-05 단계 1 · 등록 검사 순서(U2)와 sprint_id 바인딩 예약(C2)

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 1([계획](../plan-20261115/PLAN.md) §3 단계 1, §2.4 U2, §2 표 C2). 이월 C6·C2를 닫는다 |
| 착수 근거 | 계획 §2.6. [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md)(F4·S1은 권고대로, 02-05 바로 시작) |
| 검토 반영 | PR #66 검토 3c017a4([보관본](../plan-20261115/reviews/review-pr66-3c017a4.md)) N-1·N-2 |
| 기준 커밋 | 작성 때 main c5464f5(구현 커밋 c61708f), PR #72에서 386a223 위로 다시 쌓음(구현 커밋 1ad0a29, 코드 같음). 병합 089fb95 |
| 모델 호출 | 0회. 컨테이너·실제 회차 없음. 영속 PoC DB는 쓰지 않았다(일회성 시험 DB만) |
| 상태 | 병합(PR #72, 검토 [23877c1 승인](reviews/review-pr72-23877c1.md)). 원장 snapshot 0045가 기록. 시험: 작성 때 356, 다시 쌓은 뒤 357 통과(evidence/test.log 끝) |

## 1. 바꾼 것

**U2 · 등록 검사 순서.** `crates/ieo-coord/src/api.rs`
- `register_spec`를 검사(`check_spec`)와 저장(`save_spec`)으로 나눴다. `check_spec`는 sprint의 repo 검사(RG2, C2 예약)와 01-07 검사(`check_registration`)를 하고, 거부면 오류를 돌려준다. 쓰기는 하지 않는다.
- `registerSprint`의 새 순서: 멱등 재조회 → sprint repo(409 `SPRINT_REPO_CONFLICT`) → 01-07(형식 400 → digest → revision → 참조 → executable 바인딩) → 가져오기 소유(409 `EVENT_CONTENT_CONFLICT`) → 가져오기 출처 대조(executable, 409 `IMPORT_INCOMPLETE`·`IMPORT_GATED`) → 저장(201) 또는 저장된 결과 조회(200).
- 소유 검사와 출처 대조는 `check_spec` 바로 뒤에 있다. 01-07이 거부하지 않은 **모든** 결과(새 revision, `AlreadyRegistered`, `Superseded`)에 적용한다(검토 N-2). 그래서 바뀌지 않은 재제출도 새 제출이 거부될 때는 거부된다.
- RG2와 C2 예약은 지금처럼 형식 검사보다 앞이다. 다른 repo의 sprint_id를 쓴 형식 오류 본문은 예전처럼 400이 아니라 409다(검토 N-2 문제 2).
- 변환 경로(`convertHumanInput`, 가져오기)는 `register_spec`(= `check_spec` + `save_spec`)을 그대로 쓴다. 변환의 sprint_id는 그 repo 바인딩의 것이고 가져오기는 예약 ID를 이미 거부하므로, 두 경로의 답은 바뀌지 않는다.
- `guard_import_owned`의 형식 검사 분기는 지웠다(이제 형식이 맞은 본문만 온다). 쓰지 않던 `mode` 인자도 지웠다.

**C2 · 바인딩 sprint_id 예약.**
- `reserved_for(ctx, sprint_id)`: 그 ID를 이름 붙인 바인딩의 repo. 가져오기의 예약 검사도 이 함수를 쓴다(규칙 한 곳).
- `guard_sprint_repo`: DB에 등록된 repo가 있으면 그것(RG2), 없으면 바인딩 예약 repo가 sprint의 주인이다. 다른 repo면 409 `SPRINT_REPO_CONFLICT`. `/v0/sprints`와 변환 경로가 같이 쓴다.
- 기동 검사 `check_bindings`(`api.rs`로 옮김, `bin/ieo-coord.rs`가 부름). 바인딩의 sprint_id가 (1) Notion 가져오기 test id이면(X-01 U3, 그대로), (2) 다른 repo가 이미 등록한 sprint이면, (3) 두 바인딩이 같은 ID를 이름 붙이면 기동을 거부한다(종료 2). (2)가 없으면 RG2는 바인딩 repo를, 예약은 나머지 repo를 거부해 아무도 등록할 수 없게 된다.

**계약.** `contracts/ieo-api-v0.openapi.yaml` 0.8.0 → 0.9.0. registerSprint 설명에 위 순서 전체와 바뀐 첫 코드 목록을 적었다. convertHumanInput·importNotionTask 설명에 같은 순서를 가리키는 문장을 더했다. 새 코드·필드는 없다. 계획 §3의 “OpenAPI 0.8.0 → 0.9.0”을 이 단계에서 올렸고, 뒤 단계의 추가는 같은 0.9.0 설명에 더한다.

**시험.** `crates/ieo-coord/tests/coordinator.rs`
- 기존 시험 하나의 기대가 바뀌었다(검토 N-1): `an_imported_draft_with_unresolved_reasons_cannot_execute`. 저장된 가져오기 draft rev 1을 `mode=executable`로 다시 보내면 409 `IMPORT_GATED` → 422 `UNBOUND_DRAFT`. 주석을 고치고 행 수 불변 검사를 더했다. 다른 기존 시험의 기대는 바뀌지 않았다(코드를 바꾼 직후 기존 시험을 돌려 실패가 이것 하나임을 확인했다).
- 새 시험 7개: `u2_an_older_revision_of_an_imported_sprint_gets_the_revision_code_first`, `u2_an_imported_sprint_with_a_01_07_error_gets_the_01_07_code_first`, `u2_ownership_covers_every_outcome_the_01_07_checks_accept`, `u2_the_import_gate_follows_the_01_07_checks_and_the_repo_rule`, `n2_a_re_gated_page_keeps_an_unchanged_executable_copy_refused`, `c2_a_binding_sprint_id_is_reserved_for_its_repo`, `c2_the_startup_check_refuses_a_binding_on_another_repos_sprint`. 모든 거부는 `refused()`로 보내 14개 표의 행 수가 그대로인지 확인한다.
- `tests/shutdown.rs` `a_binding_on_an_imported_test_id_stops_the_start`: 실제 바이너리가 다른 repo 소유 sprint를 이름 붙인 바인딩으로 기동을 거부하는지(종료 2와 문구) 더했다.

## 2. 첫 코드가 바뀌는 요청 (입력 → 이전 → 이후)

“이전”은 같은 시험 입력을 기준 코드(c5464f5의 `api.rs`)로 돌린 답이다([evidence/old-codes.txt](evidence/old-codes.txt)). “이후”는 c61708f의 답이다([evidence/test.log](evidence/test.log) 끝). 모두 거부이고, 거부에서 행 수는 바뀌지 않았다.

| # | 입력 | 이전 | 이후 | 시험(사례 키) |
| --- | --- | --- | --- | --- |
| 1 | 가져온 sprint, 최신 가져오기에서 파생되지 않은 내용, 최신보다 낮은 새 revision(executable·draft) | 409 `EVENT_CONTENT_CONFLICT` | 409 `REVISION_SUPERSEDED` | u2-a-0001, u2-a-0002 |
| 2 | 같은 종류, 저장된 revision과 같은 번호·다른 digest | 409 `EVENT_CONTENT_CONFLICT` | 409 `REVISION_CONFLICT` | u2-b-0001 |
| 3 | 같은 종류, digest 오류 | 409 `EVENT_CONTENT_CONFLICT` | 422 `DIGEST_MISMATCH` | u2-c-0001 |
| 4 | 같은 종류, 참조 오류(정의되지 않은 선행) | 409 `EVENT_CONTENT_CONFLICT` | 422 `UNDEFINED_PREREQUISITE` | u2-c-0002 |
| 5 | 같은 종류, executable 바인딩 미충족(draft 상태) | 409 `EVENT_CONTENT_CONFLICT` | 422 `UNBOUND_DRAFT` | u2-d-0001 |
| 6 | 출처 대조에 걸리는 executable 사양 + digest 오류 | 409 `IMPORT_GATED` | 422 `DIGEST_MISMATCH` | u2-g-0002 |
| 7 | 필수 사실이 빠진 가져오기의 executable 사양 + digest 오류 | 409 `IMPORT_INCOMPLETE` | 422 `DIGEST_MISMATCH` | u2-g-0004 |
| 8 | 저장된 가져오기 draft(미해결 사유 있음)를 그대로 `mode=executable`로(검토 N-1) | 409 `IMPORT_GATED` | 422 `UNBOUND_DRAFT` | 기존 시험 `an_imported_draft_with_unresolved_reasons_cannot_execute` |
| 9 | 출처 대조에 걸리는 executable 사양, sprint_id가 다른 repo 소유(RG2) | 409 `IMPORT_GATED` | 409 `SPRINT_REPO_CONFLICT` | u2-g-0005 |
| 10 | 출처 대조에 걸리는 executable 사양, sprint_id가 다른 repo 바인딩의 예약 ID | 409 `IMPORT_GATED` | 409 `SPRINT_REPO_CONFLICT` | u2-g-0006 |

9는 계획 §2.4 표에 없던 행이다. 기준 코드에서는 출처 대조가 RG2보다 앞이었다(검토 N-2 문제 2가 지적한 RG2 위치). 10은 C2와 겹치는 경우다(9와 같은 까닭이고, 예약 자체는 새 규칙).

바뀌지 않은 것(같은 시험에서 확인):
- 01-07을 통과한 파생 아닌 새 revision → 409 `EVENT_CONTENT_CONFLICT`(u2-o-0001). 형식 오류 → 400(u2-f-0001). 다른 repo → 409 `SPRINT_REPO_CONFLICT`(u2-r-0001).
- 01-07 오류 없는 출처 대조 → 409 `IMPORT_GATED`·`IMPORT_INCOMPLETE`(u2-g-0001, u2-g-0003).
- 이전 가져오기 revision의 같은 내용 재제출 → 409 `EVENT_CONTENT_CONFLICT`, 200 아님(u2-m1-0001). 새 가져오기 뒤 옛 파생 revision 재제출 → 409(u2-m1-0005).
- 다시 막힌 페이지를 인용하는 바뀌지 않은 실행 사본 재제출(`AlreadyRegistered`·`Superseded`) → 409 `IMPORT_GATED`, 200 아님(n2-copy-04·05, 검토 N-2).
- 최신 가져오기 저장본 재제출과 최신 파생 재제출 → 200 `ALREADY_REGISTERED`. 정상 파생 새 revision → 201.
- 멱등 재조회는 지금처럼 맨 앞이다. 출처가 다시 막힌 뒤에도 같은 키·같은 본문의 재조회는 저장된 200을 돌려준다(n2 시험 끝, 기존 동작).

## 3. 불변 조건: 거부 집합

- **U2는 거부 집합을 바꾸지 않는다.** 모든 검사는 같은 트랜잭션, 같은 registry 잠금 안의 읽기이고 저장 전에는 쓰기가 없다. 소유 검사와 출처 대조의 판단은 01-07 결과에 기대지 않는다. 그리고 01-07이 거부하지 않은 세 결과 모두에 두 검사를 적용한다. 그래서 요청은 어느 검사든 하나가 거부하면 거부되고, 순서와 무관하다. §2의 1~9는 기준 코드에서도 거부였다(old-codes.txt). 409가 200이 된 경우는 없다.
- **C2는 거부를 새로 더한다(의도된 변경).** 아직 등록되지 않은 예약 sprint_id를 다른 repo가 등록하면 기준 코드는 201이었다(c2n2-0001·0002). 그 뒤에는 원래 repo가 막히고 다른 repo가 계속 revision을 더할 수 있었다(c2n2-0006도 기준 코드에서 201). 이제는 409 `SPRINT_REPO_CONFLICT`다. 예약 ID를 쓴 형식 오류 본문도 400 대신 409다(c2n2-0003, RG2와 같은 자리). 이것은 계획 §2 표의 C2 결정이고, U2 불변 조건의 예외가 아니다.
- U2에서 불변 조건을 지키지 못한 곳은 없다.

## 4. 시험 결과

- `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`: 통과.
- `cargo test --locked --all-targets`(시험 DB 포함): 356 통과, 0 실패, 0 무시(커밋 c61708f). 그중 coordinator 75개(기존 68 + 새 7), shutdown 2개. test.log에는 이어서 새 시험 6개를 `--nocapture`로 다시 돌린 사례 출력이 있어, 기록 안의 `test result` 합은 362다.
- 이전 코드 대조: [evidence/old-codes.txt](evidence/old-codes.txt). 기준 `api.rs`에 빈 `check_bindings`만 붙여 같은 시험 입력을 돌렸다(커밋하지 않은 임시 변경, 끝난 뒤 복구).
- 변이 4개, 모두 잡혔다([evidence/mutants.txt](evidence/mutants.txt)).
  - M1 소유 검사·출처 대조를 저장할 때만(새 revision만) 적용 → `u2_ownership_…`, `n2_…` 실패.
  - M2 소유 검사를 01-07 앞(예전 자리)으로 → `u2_an_older_revision_…`, `u2_an_imported_sprint_with_a_01_07_error_…` 실패.
  - M3 바인딩 예약 제거 → `c2_a_binding_sprint_id_is_reserved_for_its_repo`, `u2_the_import_gate_…` 실패.
  - M4 기동 검사에서 다른 repo 소유 검사 제거 → `c2_the_startup_check_…` 실패.

## 5. 한계와 남는 것

- 바인딩에 없는 sprint_id는 예전처럼 첫 등록 repo가 소유한다. 그 ID의 존재가 다른 repo에 409와 201의 차이로 드러나는 것은 남는다(계획 §3 단계 1에 적은 한계).
- 기동 검사는 기동할 때만 돈다. 바인딩 파일은 기동할 때만 읽으므로 실행 중에 바뀌는 경로는 없다.
- gRPC에는 등록 작업이 없어(운영 주체 작업은 HTTP 전용) 바뀐 것이 없다.
- 이 보고는 원장(`docs/poc/progress/*`)을 바꾸지 않는다. 원장 갱신은 별도다.

## 6. 사용자 결정이 필요한 것

없다. U2·C2는 계획으로 정한 개발자 권한이다. §2의 9·10행(출처 대조와 RG2·예약의 순서)은 계획 표에 없던 첫 코드 변경이라 독립 검토에서 확인을 받는다.

## 검토 23877c1 뒤

- L-1: 위 머리 표를 다시 쌓은 뒤의 사실로 고쳤다.
- L-2: 영속 코디네이터(58090)를 이 코드 이후 빌드로 바꾸면 시작 검사(`api::check_bindings`)가 돈다. 비공개 바인딩이 다른 repo가 등록한 sprint_id를 가리키거나, 두 바인딩이 같은 id를 가리키면 시작이 거부된다(두 번째는 계획의 C2 문장보다 넓다). 그래서 **재배치 전에** 비공개 바인딩 파일과 영속 DB의 `sprints` 소유를 읽기 전용으로 대조한다. 충돌이 있으면 바꾸지 않고 보고한다. 이 확인은 02-05 단계 8 준비(재배치)에 넣는다.
- I-1: `checkpoint_e2e …_over_grpc`의 500ms 대기는 다른 시험이 함께 돌 때 한 번 실패했다(재실행·CI 통과). 이 단계와 무관하며 다시 보이면 고친다.
- I-2: draft 상태 C2 사례는 단계 2에서 더했다. I-3: 원장 문구 정리.
