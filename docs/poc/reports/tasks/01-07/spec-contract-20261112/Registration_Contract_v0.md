# 01-07 · 스프린트 명세 v0 · 등록 검사 계약

2026-11-12 KST · 계획 v1.3.3 · contract_revision 4. 기존 산출물을 재사용하고 빠진 부분만 고정한다.

| 구성 | 정본 | 이번 회차의 처리 |
| --- | --- | --- |
| 필드·digest·대상 repo 하나·작업별 검증/산출물/종료 조건 | [SprintSpec_v0.schema.json](../../../../SprintSpec_v0.schema.json), 계획 §1.4 | 그대로 고정. 변경 없음 |
| 사람용 입력 양식·필수/선택·예시·담당·변환 규칙 | [HumanSprintInput_v0.schema.json](../../../../HumanSprintInput_v0.schema.json), [Human_Sprint_Authoring_v0.md](../../../../Human_Sprint_Authoring_v0.md) | 그대로 고정. 변환 결과 기록 형식과 거부 코드를 추가로 고정 |
| 일정(기한 set/none/undecided)·작업별 예상 범위(measured/unestimated) | 위 두 스키마, [Schedule_and_Role_Reports_v0.md](../../../../Schedule_and_Role_Reports_v0.md) | 그대로 고정 |
| 등록 거부 분류·필드 위치·교정 내용·HTTP 대응 | **이 문서 §2** | 신규 |
| 참조 구현·적합성 사례 | `crates/ieo-spec` (Rust) | 신규. 02-03 구현의 출발점 |

## 1. 검사 층과 등록 응답의 의미

1. **형식**: JSON Schema(2020-12, format 검증 포함). 실패하면 참조 검사를 하지 않는다(깨진 문서의 참조 판정은 신뢰할 수 없음).
2. **참조**: digest, 작업 ID 중복(명세 안·다른 등록 스프린트), 선행 해석(이 명세 안은 같은 plan_version만, 외부는 등록된 작업+요구 plan_version), 순환(등록 그래프 포함), 승인 프로파일·항목 존재, revision 동일성.
3. **바인딩**: 실행 가능 등록(`executable`)에서만 `registration_state=ready`와 `unresolved_bindings=[]`을 요구한다. `ready`인데 repository_url/한도가 비면 스키마의 조건부 규칙이 형식 층에서 거른다.
4. **의미 검토(P01)는 여기서 하지 않는다.** 통과 응답은 `ACCEPTED_FOR_SEMANTIC_REVIEW`이며 실행 인수·완료가 아니다. 승인 프로파일에 없는 완료 기준은 `UNKNOWN_PROFILE_ITEM`으로 막고 의미 검토/정책 승인으로 보낸다. 형식 검사로 요구의 일관성을 승인하지 않는다.

## 2. 거부 코드(안정 계약값)

모든 거부는 `{code, field, message, correction, retryable_after_edit}`이다. `field`는 제출 문서 기준 위치(`tasks[1].prerequisites[0].task_id`, 문서 전체는 `""`)다.

| 코드 | 층 | HTTP | 조건 | field 예 |
| --- | --- | --- | --- | --- |
| `MISSING_FIELD` | 형식 | 400 | 필수 필드 누락(필드 누락을 다른 형식 오류와 구분) | `tasks[0].verification` |
| `SCHEMA_VIOLATION` | 형식 | 400 | 타입·어휘·패턴·조건부(ready 바인딩, 기한 set/none/undecided)·추가 필드 | `tasks[0].kind`, `target.repository_url` |
| `DIGEST_MISMATCH` | 참조 | 422 | digest ≠ 정본 직렬화 SHA-256 | `digest` |
| `DUPLICATE_TASK_ID` | 참조 | 422 | 명세 안 중복 또는 다른 등록 스프린트의 ID와 충돌 | `tasks[8].task_id` |
| `UNDEFINED_PREREQUISITE` | 참조 | 422 | 선행이 이 명세(같은 plan_version)에도, 등록 목록(요구 plan_version)에도 없음 | `tasks[1].prerequisites[0].task_id` |
| `DEPENDENCY_CYCLE` | 참조 | 422 | 선행 순환(등록 그래프 포함). message에 경로 | `tasks[0].prerequisites` |
| `UNKNOWN_PROFILE_ITEM` | 참조 | 422 | 승인 프로파일(ID·revision) 없음, 또는 항목 ID가 그 프로파일에 없음 | `tasks[0].acceptance.item_ids[0]` |
| `UNBOUND_DRAFT` | 바인딩 | 422 | 실행 가능 등록인데 draft 또는 미바인딩 항목 존재(항목마다 1건) | `registration_state`, `unresolved_bindings[2]` |
| `REVISION_CONFLICT` | 충돌 | 409 | 같은 sprint_id·revision이 다른 digest로 이미 등록 | `revision` |
| `UNRESOLVED_PREREQUISITE` | 변환 | 422 | 사람 입력 `after`가 입력 안 이름·등록 작업 ID 어디에도 없음(Human_Sprint_Authoring 예시와 같은 코드) | `tasks[1].after[0]` |
| `DUPLICATE_TASK_NAME` | 변환 | 400 | 사람 입력 작업 이름 중복 | `tasks[1].name` |
| `ESTIMATE_RANGE_REVERSED` | 변환 | 422 | `min_minutes > max_minutes`(스키마로 표현 불가) | `tasks[1].expected_duration` |

- 같은 sprint_id·revision·digest 재제출은 거부가 아니라 **200 재조회**(멱등)다. 접수는 201.
- 응답 상태는 거부 중 400 → 409 → 422 순으로 가장 앞선 것을 쓴다.
- 인증/범위 거부(401/403, 다른 repo 무단 배정)는 주체 자격이 있어야 판정하므로 01-08 API 계약에서 고정한다.

## 3. 변환 결과 기록(사람 입력 → v0 draft)

변환기는 항상 `draft`를 만든다. 결과 기록: `input_digest`, `spec`, `name_to_task_id`(원 이름↔ID), `preserved_unrepresented`, `semantic_review_required`, `unresolved_bindings`.

- 작업 ID는 입력 순서대로 `NN-01…`(NN=스프린트 번호, 코디네이터가 부여). 같은 입력은 같은 ID·digest.
- 완료 기준 문구는 그대로 `acceptance.criteria`·`exit_condition`에 둔다. 항목 ID는 `<task_id>.accept` 규칙으로 변환기가 채우고, 승인 프로파일에 없으면 `semantic_review_required`에 올린다(변환기가 기준을 승인하지 않음).
- 예상 범위: 근거가 있으면 `measured`, `분×60`초, evidence 보존. 없으면 `unestimated`·`range_seconds=null`·이유. 0으로 대체하지 않음.
- 기한: set은 시각 그대로, none/undecided는 이유를 `unresolved_reason`에 보존. 서로 바꾸지 않는다.
- repository_url·한도·착수 조건이 없으면 null/미지정 + `unresolved_bindings`. 위임·출처·사람 확인 블록은 코디네이터의 승인된 repo 바인딩에서 받고 사용자에게 쓰게 하지 않는다.
- digest는 마지막에 계산한다.

## 4. 표현 공백(계약 갭)

| 갭 | 현재 처리 | 제안 |
| --- | --- | --- |
| 사람 입력의 `out_of_scope`, `decisions`가 SprintSpec v0에 대응 필드 없음 | 변환 결과 기록 `preserved_unrepresented`에 보존, 명세 digest에는 미포함 | 다음 contract revision(5)에서 `scope.excluded[]`, `decision_requests[]` 추가 여부를 결정. revision 4 변경 없음 |
| 사람 작업에는 `kind`가 없음 | 변환기가 `구현`으로 채움 | P36 실측 뒤 입력 양식에 선택 필드 추가 여부 결정 |
| 다른 repo 무단 배정 판정 | 주체 자격 필요 → 01-08 API(403) | 01-08에서 고정 |

## 5. 적합성 사례(참조 구현 `crates/ieo-spec`)

- 등록 16건(`tests/registration.rs`): 패키지 9개 명세 digest 재현(9/9), POC-01 기준선 draft 접수(201), 필드 누락, 그 밖의 형식 위반, digest 불일치, 명세 안 ID 중복, 등록 스프린트와 ID 충돌, 미정의 선행, 다른 plan_version 선행, 순환, 없는 프로파일 항목/프로파일, draft의 실행 가능 등록 거부, 바인딩 완료 ready 접수, ready인데 URL 없음(형식 층), revision 충돌 409·동일 재제출 200, 모든 거부의 field/correction 존재.
- 변환 11건(`tests/conversion.rs`): 유효 변환(ID·선행·초 환산·미산정·기한 none·digest·문구 보존·보존 필드·미바인딩)과 그 draft의 등록 검사 통과/실행 등록 거부, 프로파일 밖 기준의 의미 검토 회부, 문서 예시와 같은 `UNRESOLVED_PREREQUISITE`, 이름 중복, 역전 범위, 근거 없는 예상, 필수 누락, 기한 set 형식, undecided 이유 필수, 순환, 같은 입력의 같은 digest.
- CLI `ieo-spec check-manifest`: 9개 명세 draft 모드 전부 201, 실행 가능 모드 전부 422(UNBOUND_DRAFT만, 각 6건).

이 구현은 로컬 참조 검사다. 인증된 등록 API·DB 기록·의미 검토(P01)는 02-03/04-01의 범위다.
