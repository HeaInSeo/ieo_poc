# 02-03 선행 교정: 늦은 옛 배정 결과 보존 · 옛 revision 실행 등록/승격 거부

2026-11-12 KST · 계획 v1.3.3-A1 · 동결 대장 항목 **L08**([01-03 동결](../../tasks/01-03/staged-freeze-20261112/REPORT.md)) · 근거 [사용자 지시](../../../User_Decisions_2026-11-12_0103.md)
**작업 수에 더하지 않는다.** 01-07·01-08의 검수 기록은 그대로 두고, 이 교정으로 바뀐 계약 범위만 독립 검토에서 다시 검증한다.

## 1. 결함과 교정

| 출처 | 결함(교정 전) | 교정 | 계약 |
| --- | --- | --- | --- |
| 01-08 검수 후속 N1 | 합법적으로 발급됐던 옛 배정·세대의 늦은 claim을 409 `STALE_ASSIGNMENT`로 버렸다. 같은 ID 재전송도 배정 검사 뒤에 봐서, 배정이 바뀐 뒤의 재조회가 409가 됐다 | 같은 ID 재전송을 먼저 본다. 같은 내용이면 200 재조회(상태 변화 없음), 다른 내용이면 409 `EVENT_CONTENT_CONFLICT`. 새 claim이 현재 배정이면 202 `current`다. 같은 (repo, role)에 발급됐던 옛 배정이면 202 `preserved_late`로, 그 배정과 코디네이터 DB의 그 배정 SHA에 귀속해 보존하고 현재 판정에서 제외한다. 발급 이력이 없으면 409. 새 재현 요청은 실행을 새로 시작하므로 현재 배정과 그 SHA만 받는다. 같은 요청 재전송은 200 재조회이며 다시 대기열에 넣지 않는다 | [Report_API_Contract_v0.md](../../tasks/01-08/report-api-contract-20261112/Report_API_Contract_v0.md) rev 3 §1·§2, OpenAPI `submitAgentReport`·`submitReproRequest`·`ClaimReceipt` |
| 01-07 검수 후속 N1 | 더 새 revision이 등록된 뒤에도 draft로 저장된 옛 revision이 executable 등록을 통과했다(승격). Registry도 옛 revision을 다시 기록해 그 작업 집합을 되살렸다 | 활성 revision은 등록된 최신 하나다. 옛 revision의 신규 등록이나 executable 승격은 409 `REVISION_SUPERSEDED`. 옛 revision의 동일 재제출(저장 방식이 요청을 포괄)은 200 `SUPERSEDED`+`superseded_by`이며, 저장 결과의 역사적 조회일 뿐 재활성화하지 않는다. Registry는 옛 revision 기록을 거부하고, 새 revision 등록 때 그 스프린트의 작업 집합을 바꾼다 | [Registration_Contract_v0.md](../../tasks/01-07/spec-contract-20261112/Registration_Contract_v0.md) rev 3 §1·§2, OpenAPI `registerSprint`·`RegistrationResult`·`Rejection` |

**API 변경(호환성 주의, OpenAPI 0.5.0).**
- `authorize_claim`·`authorize_repro`는 이제 현재 배정 하나 대신 (repo, role)의 발급 이력 `AssignmentBook`과 저장된 같은 ID 레코드를 받는다. 결과는 처분(`ClaimDisposition`/`ReproDisposition`)이다.
- `Registry::add_spec*`는 기록 여부를 `bool`로 돌려준다.
- 새 코드 `REVISION_SUPERSEDED`를 추가했다. 이전에 “낮은 revision”으로 `REVISION_CONFLICT`를 내던 경우가 이 코드로 바뀌었다.

## 2. 시험

- `cargo test`: 78건 통과. 내역은 01-07 등록 24건(3건 추가, 1건 수정), 01-08 계약 33건(교정 전 409 시험 1건 대체, 4건 추가), 그 밖 21건이다. `cargo fmt --check`·`clippy -D warnings`·`openapi-spec-validator` 모두 통과했다. 증거와 제출 SHA의 연결은 §4에 있다.
- 독립 검토 e691dfa(reject)에서 나온 지적을 이렇게 반영했다.
  - F1 P2: 본문 `body.target_sha`가 배정에 기록된 SHA와 다르면 409 `ASSIGNMENT_SHA_MISMATCH`를 낸다. 현재 배정과 옛 배정 모두에 적용한다.
  - F2: M7은 등가 변이가 아니었다. 같은 ID에 발급 이력이 없는 배정을 넣으면 `EVENT_CONTENT_CONFLICT`를 내야 한다는 시험을 추가했다.
  - F3: Registry는 같은 revision을 다른 digest로 덮어쓰지 않고, executable 저장 방식을 draft로 낮추지 않는다.
  - F4: OpenAPI를 0.5.0으로 올렸다. 0.4.0과 호환되지 않는다는 점을 명시했고, 재전송 영수증의 `counts_for_current_judgment`는 응답 시점에 다시 계산한다.
  - F5: Idempotency-Key 재전송도 현재 등록 상태로 다시 평가한다(`SUPERSEDED`).
  - F7: 재전송 조회는 (repo, role) 범위로 한정한다.
  - `Denied::ContentConflict`의 직렬화 코드를 계약과 같은 `EVENT_CONTENT_CONFLICT`로 맞췄다.
- 남은 한계(F6): `review_currency`는 처분을 모른다. 그래서 호출자는 SHA만으로 거르지 말고 `counts_for_current_judgment`가 참인 claim만 현재 판정에 넣어야 한다. 이 규칙은 02-03 구현이 지켜야 하며, 계약 01-08 §1에 적었다.

## 3. 범위 밖

- 실제 DB·HTTP 서버는 없다. 배정 이력과 저장 레코드의 영속화, 동시성은 02-03·02-04에서 구현한다. 이 교정은 참조 구현과 계약이다.
- 01-08 N2(`completion_summary`의 확인된 범위 전체 표시)와 D12의 권한·판정 의미(L07)는 이 PR에 넣지 않았다. 둘 다 02-03 착수 전에 따로 처리한다.

## 4. 증거와 제출 SHA

- 시험한 코드는 커밋 `8eb2f986c230b8afe6cb757e1c62955067b20385`이다. 이 PR의 다음 커밋은 증거만 더한다. `git diff 8eb2f98 HEAD -- crates contracts`가 비어 있으면 그렇다.
- [cargo-test.log](evidence/cargo-test.log): fmt·clippy 통과, 시험 **78건** 통과(`^test … ok` 78줄). 앞서 보고한 76건에 검토 반영 시험 2건(본문 SHA 대조, Registry 불변)을 더한 수다.
- [mutants.txt](evidence/mutants.txt): 같은 커밋에서 변이 **15개**를 돌려 모두 검출했다. 앞서 보고한 11개에, 등가로 오판했던 M7과 검토 반영 M12~M14를 더했다. 각 변이의 diff는 [mutant-diffs/](evidence/mutant-diffs/)에 있다. 실행 뒤 `crates/`의 변경은 0건이다.
