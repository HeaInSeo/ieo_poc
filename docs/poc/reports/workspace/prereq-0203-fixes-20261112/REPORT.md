# 02-03 선행 교정: 늦은 옛 배정 결과 보존 · 옛 revision 실행 등록/승격 거부

2026-11-12 KST · 계획 v1.3.3-A1 · 동결 대장 항목 **L08**([01-03 동결](../../tasks/01-03/staged-freeze-20261112/REPORT.md)) · 근거 [사용자 지시](../../../User_Decisions_2026-11-12_0103.md)
**작업 수에 더하지 않는다.** 01-07·01-08의 검수 기록은 그대로 두고, 이 교정으로 바뀐 계약 범위만 독립 검토에서 다시 검증한다.

## 1. 결함과 교정

| 출처 | 결함(교정 전) | 교정 | 계약 |
| --- | --- | --- | --- |
| 01-08 검수 후속 N1 | 합법적으로 발급됐던 옛 배정·세대의 늦은 claim을 409 `STALE_ASSIGNMENT`로 버렸다. 같은 ID 재전송도 배정 검사 뒤에 봐서, 배정이 바뀐 뒤의 재조회가 409가 됐다 | 같은 ID 재전송을 먼저 본다. 같은 내용이면 200 재조회(상태 변화 없음), 다른 내용이면 409 `EVENT_CONTENT_CONFLICT`. 새 claim이 현재 배정이면 202 `current`다. 같은 (repo, role)에 발급됐던 옛 배정이면 202 `preserved_late`로, 그 배정과 코디네이터 DB의 그 배정 SHA에 귀속해 보존하고 현재 판정에서 제외한다. 발급 이력이 없으면 409. 새 재현 요청은 실행을 새로 시작하므로 현재 배정과 그 SHA만 받는다. 같은 요청 재전송은 200 재조회이며 다시 대기열에 넣지 않는다 | [Report_API_Contract_v0.md](../../tasks/01-08/report-api-contract-20261112/Report_API_Contract_v0.md) rev 3 §1·§2, OpenAPI `submitAgentReport`·`submitReproRequest`·`ClaimReceipt` |
| 01-07 검수 후속 N1 | 더 새 revision이 등록된 뒤에도 draft로 저장된 옛 revision이 executable 등록을 통과했다(승격). Registry도 옛 revision을 다시 기록해 그 작업 집합을 되살렸다 | 활성 revision은 등록된 최신 하나다. 옛 revision의 신규 등록이나 executable 승격은 409 `REVISION_SUPERSEDED`. 옛 revision의 동일 재제출(저장 방식이 요청을 포괄)은 200 `SUPERSEDED`+`superseded_by`이며, 저장 결과의 역사적 조회일 뿐 재활성화하지 않는다. Registry는 옛 revision 기록을 거부하고, 새 revision 등록 때 그 스프린트의 작업 집합을 바꾼다 | [Registration_Contract_v0.md](../../tasks/01-07/spec-contract-20261112/Registration_Contract_v0.md) rev 3 §1·§2, OpenAPI `registerSprint`·`RegistrationResult`·`Rejection` |

**API 변경(호환성 주의).**
- `authorize_claim`·`authorize_repro`는 이제 현재 배정 하나 대신 (repo, role)의 발급 이력 `AssignmentBook`과 저장된 같은 ID 레코드를 받는다. 결과는 처분(`ClaimDisposition`/`ReproDisposition`)이다.
- `Registry::add_spec*`는 기록 여부를 `bool`로 돌려준다.
- 새 코드 `REVISION_SUPERSEDED`를 추가했다. 이전에 “낮은 revision”으로 `REVISION_CONFLICT`를 내던 경우가 이 코드로 바뀌었다.

## 2. 시험

- `cargo test`: 76건 통과(증거는 아래 §4). 01-07 등록 23건(2건 추가, 1건 수정), 01-08 계약 32건(교정 전 409 시험 1건을 대체하고 3건 추가). `cargo fmt --check`·`clippy -D warnings` 통과. `openapi-spec-validator` OK.
- 변이 시험(§4): 의미 있는 변이 11개 모두 검출했다. M7은 생존했지만 등가 변이다(저장된 claim은 항상 발급된 배정을 가리킨다). 그래서 더 날카로운 M7b로 대체했고 M7b는 검출됐다.

## 3. 범위 밖

- 실제 DB·HTTP 서버는 없다. 배정 이력과 저장 레코드의 영속화, 동시성은 02-03·02-04에서 구현한다. 이 교정은 참조 구현과 계약이다.
- 01-08 N2(`completion_summary`의 확인된 범위 전체 표시)와 D12의 권한·판정 의미(L07)는 이 PR에 넣지 않았다. 둘 다 02-03 착수 전에 따로 처리한다.
