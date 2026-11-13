# X-01 · Notion 작업 가져오기·초안 등록

| 항목 | 기록 |
| --- | --- |
| 계획·작업 | v1.3.3-A3 · 추가 작업 X-01(구현, 선행 02-03) · 73·12 완료 수 미산입 · 근거 [사용자 지시](../../../../User_Decisions_2026-11-13_notion.md), [개정 A3](../../../../Amendment_v1.3.3-A3.md) |
| 수용 기준 | 장부 §7 X-01 행 |
| 산출물 | crate [`crates/ieo-notion`](../../../../../../crates/ieo-notion)(라이브러리 + CLI `ieo-notion snapshot|import`) · 코디네이터 `POST /v0/imports/notion`(OpenAPI 0.7.0) · `notion_imports` 표 · 진행 화면의 추가 작업 표시 |
| 모델 호출 | 0회 |
| 상태 | rev 4. 독립 검토 [b8b619a reject](reviews/review-pr27-b8b619a.md)(P2 R1~R3) → 교정(§6) → 재검토 [38b671b reject](reviews/review-pr27-38b671b.md)(P2 S1·S2) → 교정(§7) → 재검토 2 [7a48a4f reject](reviews/review-pr27-7a48a4f.md)(P2 T1) → 교정(§8). X-01 검토 3/4·**교정 3/3(상한 도달)**. 마지막 검토(4/4) 대기 |

## 1. 사용한 Notion 원본과 선정

조회는 이 대화의 Notion 연결로 했다. **서버에는 재사용할 Notion API 자격이 없다**: 환경 변수·설정에 Notion 토큰이 없고, 홈 디렉터리의 `notion-recovery-*`는 2026-10-12의 페이지 사본뿐이다. 그래서 실제 페이지는 대화 연결로 읽어 비공개 스냅샷(공개 저장소 밖, 권한 0600)으로 저장하고, 서버의 가져오기 도구가 그 스냅샷을 읽었다. 같은 도구는 Notion REST API(통합 토큰 파일)로 직접 읽는 경로도 가지며, 이 경로는 모의 서버로만 시험했다(§3). 서버 직접 조회는 자격 발급 뒤의 일이다.

| 읽은 것 | 범위 |
| --- | --- |
| 큐 진입점 `3ed86e6dd1d18118b0b8d98bb91892fb` | 현재 관측 절과 조회 계약 절만 |
| 작업 DB 데이터소스 `collection://34105eb7-…` | 후보·선행 행의 속성 조회(작업 ID·구획·상태·담당·repo), 현재 구획 행 목록 |
| `OP-OCI-RAC-OC08-20261018`(`3fd86e6dd1d181a3a9cace91920adac0`) | 전체(속성·본문) |
| `OP-NODEKIT-CLI-USABILITY-20261021`(`40086e6dd1d1811db360fe7c2619b4cc`) | 속성과 차단·재개 지시 |

**판단.**
- **OC08을 선정했다. 초안 등록만 하고 실행 가능으로 보지 않는다.** 원본의 큐 구획·작업 상태·차단 대상이 실행 대기 상태가 아니다. 직접 선행 OC01·OC02는 수용·merged 인계 전이고, Sori 단일 수정 슬롯은 PR #48이 쓰고 있다. 코드 범위는 겹치지 않는다(PR #48은 `internal/admininit`·`jobledger`, OC08은 `chunked/validate.go`). 작은 순수 부품이고 수용 기준 OC08-01~03이 표로 고정돼 있어 변환 시험에 적합하다.
- **NodeKit CLI 후속은 제외했다.** 다음 하위 행이 정해지지 않았고 Operator 인수도 없다. 기존 서버 작업공간도 다른 브랜치에서 수정 중이다. 실행 중인 운영 흐름과 겹칠 위험이 크다.
- **더 작은 후보는 없었다.** 큐의 `현재` 구획 행은 기존 담당이 진행 중인 행, 설계·검토 전용 행, 보류 행뿐이었다.
- Notion에는 아무것도 쓰지 않았다. 원 작업의 담당·상태·claim은 그대로다.

## 2. 구현

| 요소 | 동작 |
| --- | --- |
| 읽기(`ieo-notion`) | 연결 결과 스냅샷 또는 REST API(`/v1/pages`, `/v1/blocks/*/children` 쪽 나눔 따름)를 속성 문자열 + 본문으로 정규화한다. 두 경로의 본문 표현은 다르다(REST는 제목·문단·목록·코드·표만 읽고 중첩·기타 블록은 생략). 그래서 같은 페이지도 경로가 바뀌면 digest가 달라 새 revision이 된다. 토큰은 Authorization 헤더로만 보내며 오류 메시지에 넣지 않는다. 쓰기 호출은 없다 |
| 내용 digest | 속성과 본문의 정본 JSON SHA-256이다. 같은 경로의 반복 읽기는 같은 값이다. 스냅샷 파일은 기록된 digest와 내용이 다르면 거부한다 |
| 추출 | 작업 ID, 제목, 대상 repo(`저장소 식별자`의 owner/name), 목표(`## 목표` 절 첫 문단 또는 `내용 요약`), 수용 기준(`## 필수 인수 판정` 표 → 판정 ID·입력·기대·증거), 선행 절 |
| 미해결 사유 | 큐 구획이 `현재`가 아님, 작업 상태가 `실행재개`가 아님, `차단 대상`이 남음, 필수 속성·목표·수용 기준을 읽지 못함. 값을 지어내지 않는다(기한은 `undecided`와 이유) |
| 변환·등록 | 사람 입력 v0 → 기존 01-07 변환기 → 기존 등록 검사. 서버가 repo의 승인 바인딩을 적용하고, 원본과 다른 **시험 ID**를 스프린트 ID로 쓴다. 출처에 `notion_page`(URL·수정 시각·digest)를 더하고 미해결 사유를 `unresolved_bindings`에 넣은 뒤 digest를 계산한다. 항상 draft다 |
| 중복·개정 | (페이지, 내용 digest)가 같으면 200 재조회이며 새 행이 없다. 내용이 바뀌면 다음 revision이 되고 이전 revision은 그대로 남는다. 같은 내용을 다른 시험 ID로, 다른 페이지를 같은 시험 ID로 보내면 409. `notion_imports`는 추가 전용(트리거)이다 |
| 실행 차단 | draft·미바인딩 한도·미해결 사유가 있으면 executable 등록과 배정이 거부된다(01-07·02-03 규칙 그대로) |
| 실행 고정 | 실행 중인 배정의 다음 세대는 자기 revision에 머물 수 있다. 그 상한은 더 새로운 revision 중 executable이고 의미 검토를 통과한 것의 `correction_attempts`이고, 그런 revision이 없으면 자기 revision의 값이다. draft(예: 다시 가져온 Notion 작업)는 실행 중인 배정을 바꾸지 않는다. 새 배정은 최신 revision만 쓴다. Notion 가져오기가 만든 스프린트에 `/v0/sprints`로 등록할 수 있는 것은 최신 가져오기에서 파생한 명세(같은 작업과 같은 `notion_page` 출처 digest)뿐이다. 이것이 한도를 바인딩하고 미해결 사유를 정리해 executable로 올리는 경로이고, 다른 내용은 409다. 바인딩의 `sprint_id`는 시험 ID로 쓸 수 없어 `/v0/sprint-inputs`는 가져온 스프린트에 쓰지 않는다. 따라서 원본이 바뀌어 새 revision이 생겨도 진행 중인 실행은 자동으로 바뀌지 않는다 |
| 작업 번호 | 새 시험 스프린트는 바인딩의 `import_sprint_numbers`에서 쓰이지 않은 첫 번호를 받고, 이후 revision도 그 번호를 유지한다 |
| 함께 고친 결함 | 같은 스프린트의 이전 revision을 다른 스프린트로 보고 `TASK_ID_CONFLICT`를 내던 변환 경로(기존 `/v0/sprint-inputs`에도 있었다)를 고쳤다 |

## 3. 검증

| # | 사용자 검증 항목 | 결과 | 근거 |
| --- | --- | --- | --- |
| 1 | 실제 Notion 작업 1건 조회 → 명세 변환 → DB 초안 등록 | OC08 스냅샷(digest `3f8a300f…`)을 영속 DB의 코디네이터에 등록했다. HTTP 201, 시험 ID `POCX-SORI-OC08-T01` revision 1, draft·의미 검토 대기, 미해결 사유 5건(큐 구획·작업 상태·차단 대상 3건과 한도·바인딩 미정 2건). 작업 91-01~03이 OC08-01~03에 대응한다 | [live-check.txt](evidence/live-check.txt) §1 |
| 2 | 같은 입력 재등록의 중복 방지 | HTTP 200 `replay=true`, revision 1, 행 수 불변 | 같은 파일 §2, 시험 `notion_import_registers_a_draft_once_and_revises_on_change` |
| 3 | 원본 변경 시 새 revision, 기존 기록 보존 | **모의 변경**(비공개 스냅샷 복사본의 속성 1개 수정, Notion은 수정 안 함)으로 revision 2(digest `e5516fc8…`)를 만들었다. revision 1(`91ed78dd…`)은 그대로다 | 같은 파일 §3 |
| 4 | 필수 조건이 빠진 작업의 실행 차단 | 초안 배정 422 `UNBOUND_DRAFT`, 행 수 불변. 시험에서는 미해결 사유가 남은 초안의 executable 승격(422 `UNBOUND_DRAFT`)과 ready 전환(400)도 거부 | 같은 파일 §4, `an_imported_draft_with_unresolved_reasons_cannot_execute` |
| 5 | Notion 연결 실패 시 DB 보존 | REST API에 도달하지 못하면 exit 2이고 코디네이터에 아무것도 보내지 않는다. 행 수 불변. 모의 서버의 500·404도 오류이며 토큰은 오류에 나오지 않는다 | 같은 파일 §5, `notion_failures_are_errors_and_never_leak_the_token` |
| 6 | 02-04 준비 후 실제 개발 세션 | **미실행.** X-02로 분리했다. 선행 02-04 미수용, 모델 예산 미승인, 원 작업 큐 선행 미해제 | §5 |

- 시험 162건 통과(ieo-notion 9건, 코디네이터 39건 중 X-01 관련 11건), 커밋 68be8db·깨끗한 트리, fmt·clippy 통과, 생성기 시험 PASS(추가 작업 4건 포함), `validate_package.py` EXIT 0, OpenAPI OK: [cargo-test.log](evidence/cargo-test.log).
- 변이 13개를 모두 검출했다(검토 반영 X9~X13 포함): [mutants.txt](evidence/mutants.txt).
- 검토 반영 뒤 같은 영속 DB에서 재확인: 재조회(200, 행 불변), 초안 배정 거부(422), test_id = Notion 작업 ID 거부(400, 행 불변): [live-recheck.txt](evidence/live-recheck.txt). 재검토 반영 뒤 다시 확인: 재조회 200, 가져오기 스프린트 위 `/v0/sprints` 등록 409, 초안 배정 422, 모두 행 불변: [live-recheck2.txt](evidence/live-recheck2.txt).
- 시험은 모의 데이터만 쓴다. 공개 저장소에는 큐 원문을 넣지 않았고, 증거 파일에는 ID·digest·상태 코드만 남겼다.
- DB 등록 성공은 개발 완료가 아니다. OC08 자체는 구현되지 않았다.

## 4. DB와 대상 repo

- **이전 시험 DB `ieo-poc-0203-pg`.** tmpfs 위의 일회성 컨테이너로, cargo 시험 전용이며 데이터를 남기지 않는 설계다. 그대로 두었다.
- **영속 PoC DB `ieo-poc-db`.** PostgreSQL 17, 이미지 digest는 위와 같고 127.0.0.1:55433에서 동작한다. 데이터는 저장소 밖 디렉터리의 bind mount에 있다. 보관은 `pg_dump -Fc`, 복원은 `pg_restore`로 하며, 복원한 DB의 행 수·digest가 원본과 같음을 확인했다: [db-backup-restore.txt](evidence/db-backup-restore.txt). 코디네이터 서비스(127.0.0.1:58090)와 자격(토큰 digest만)·시험 바인딩도 같은 비공개 디렉터리에 있다.
- **시험 바인딩(HeaInSeo/sori).** 승인된 수용 프로파일이 아니라 시험 항목(`x01-notion-import-test`)이다. 한도는 바인딩하지 않아 실행 가능 등록이 원천적으로 막힌다.
- **대상 repo HeaInSeo/sori.**
  - 기본 브랜치는 `master`이고 기준 SHA는 `2fa7f5e12fa4474b194acf0d6e1a0273666642c1`이다.
  - 기존 작업공간은 조사만 했고, 새로 clone한 사본에서 `poc/POCX-SORI-OC08-T01`을 만들어 push했다. 기준 SHA 그대로이고 추가 커밋은 0이다: [target-branch.txt](evidence/target-branch.txt).
  - **PR은 만들지 않았다.** 큐 규칙상 repo당 수정 PR은 하나이고, PR #48이 Sori 슬롯을 쥐고 있다.
  - 기본 브랜치 병합은 하지 않는다.

## 5. 남은 조건과 결정 (X-02, 02-04)

**모델 호출 예산안(한 번에 제시, 승인 전 호출 없음).** 02-04의 실제 세션 수용 시험과 X-02를 함께 덮는 안이다. 01-04 예산은 쓰지 않는다.

| 항목 | 02-04 실제 세션 시험 | X-02 시험 개발 세션 |
| --- | --- | --- |
| 범위 | 런타임 계약 rev 6 §10 항목 1~5(정상 재개(c): 도구 없음·허용 도구·자식·`/compact`, 세션당 단일 실행, 재개 키), rev 7 §11 항목 7(새 세션→재개→`/compact` 재개의 회차 몫 합 = 마지막 세션 누적) | 고정 명세 `POCX-SORI-OC08-T01`로 `poc/` 브랜치에서 개발 세션 기동·중단·재개, 독립 검토 1회와 재검토 1회 |
| 시도 수 상한 | 모델 회차 12 | 개발 회차 3 + 검토 회차 2 + 교정 회차 1 = 6 |
| 시간 상한 | 회차당 1,800초, 합 14,400초(F05) | 개발 회차당 3,600초, 합 14,400초(F05) |
| 토큰 산정 | 02-01 계측기의 회차 몫(`run` = 세션 누적 − 재개 기준) 합. **청구 토큰 = input + output + cache_creation**, cache_read는 따로 기록해 별도 상한. 미계측 회차가 생기면 남은 예산을 0으로 보고 다음 호출을 하지 않는다 | 같음 |
| 토큰 상한(제안) | 청구 1,500,000 · cache_read 15,000,000 | 청구 3,000,000 · cache_read 30,000,000. EARLY-01 임시 예산(01-03 채택 3,000,000)을 넘지 않는 선 |
| 비용 | CLI `total_cost_usd`(회차 몫)를 기록한다. 계정 방식에 따라 실제 청구와 다를 수 있어 금액 상한은 사용자가 정해 주기 바란다 | 같음 |
| 정지 규칙 | 상한 하나라도 도달하거나 다음 회차가 넘길 것으로 예상되면 호출하지 않고 보고한다. `rate_limit_event`가 allowed가 아니면 대기 없이 정지하고 보고한다. 자동 재시도는 없다 | 같음 |

**필요한 사용자 결정.**
1. 위 예산안의 승인 또는 수정.
2. 02-04·X-02에 쓸 역할 자격(개발·Guardrail)의 확인 또는 재발급. 01-04에서 쓴 자격을 계속 쓸지 정해야 한다.
3. **OC08의 큐 선행이 남아 있는 상태에서 PoC 시험으로 개발 세션을 돌려도 되는지.** 원 작업의 claim·상태를 바꾸지 않고, 기본 브랜치 병합 없이, 결과를 반영 후보로만 남기는 조건이다. 승인하지 않으면 OC01·OC02 수용과 Sori 슬롯 반환까지 X-02를 미룬다.
4. Sori에 시험 PR을 열지. 열면 큐의 "repo 수정 PR 1" 규칙과 충돌한다. 열지 않으면 브랜치 비교 링크로 diff를 남긴다.
5. 서버 직접 Notion 조회용 통합 토큰을 발급할지. 발급하면 그 경로의 실제 조회를 다시 시험한다. 쓰기 권한은 필요 없다.

**후속(담당).**
- Notion 원본 상태 자동 갱신: 별도 후속이며 이번 범위 밖이다.
- 02-04의 모델 없는 부분(supervisor 골격, 체크포인트 형식, 계측기 연결, 이월 판정 규칙 정리): 02-04에서 다음에 진행한다. 이번 회차에는 진행하지 않았다.

## 6. 독립 검토 반영(rev 2)

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| R1 P2 Notion 가져오기가 만든 적 없는 기존 스프린트 ID를 test_id로 써 덮어쓸 수 있음, test_id = Notion 작업 ID 허용 | 교정. 앞의 경우 409, 뒤의 경우 400 | `a_test_id_cannot_take_over_an_existing_sprint_or_the_notion_id`, X9 |
| R2 P2 replay가 다른 repo 호출자에게 저장 spec 전체를 돌려줌 | 교정. replay는 이전 가져오기의 repo에서만 응답하고, 다른 repo는 저장 내용 없이 409 | `an_import_replay_answers_only_within_its_repo`, X10 |
| R3 P2 pin 예외가 최신 revision의 시도 상한을 무시함, 문서 미갱신 | 교정. pin된 이어 쓰기도 최신 revision의 `correction_attempts`를 따른다. OpenAPI issueAssignment와 판정 정책 0.5.0(RG2 예외 문장, 근거 사용자 지시 2026-11-13)에 적었다 | `a_pinned_continuation_obeys_the_latest_attempt_ceiling`, X11 |
| R4 P3 A→B→A 되돌림은 새 revision이 아니라 replay | 유지하고 OpenAPI에 명시. 되돌림을 새 revision으로 만들려면 가져오기 순번을 digest에 넣어야 하는데, 같은 내용 중복 방지 요구와 충돌한다 | — |
| R5 P3 REST 경로와 연결 경로의 digest 차이, 미지원 블록 생략 | 주석을 사실대로 고쳤다(경로를 바꾸면 새 revision). 서버 직접 조회가 열리면 한 경로로 통일한다 | — |
| R6 P3 `unresolved`는 클라이언트 값, 번호 풀과 바인딩 번호 중복 가능 | 유지. 가져오기 도구가 원본에서 만든 사유를 보내며, 서버는 사유를 더 붙일 수만 있고 실행 가능으로 바꿀 수는 없다. 번호가 겹쳐도 나중 변환이 거부돼 오염은 없다 | — |
| R7 P3 보고서의 큐 상태 원문 표기 | 교정. §1을 요약 표현으로 바꿨다 | — |

## 7. 재검토 반영(rev 3)

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| S1 P2 pin된 이어 쓰기의 상한을 draft를 포함한 최신 revision에서 가져옴(상한 상향 draft로 G5 초과, null 상한 draft로 실행 중단) | 교정. 더 새로운 revision 중 executable·의미 검토 통과한 것의 상한만 쓰고, 없으면 자기 revision 값. 정책 0.6.0, OpenAPI 문구 갱신 | `a_draft_revision_never_changes_a_running_execution`, `a_draft_with_unbound_limits_does_not_block_a_running_execution`, X12 |
| S2 P2 순서를 뒤집으면 R1 우회(가져오기 스프린트 위에 `/v0/sprints`·`/v0/sprint-inputs`) | 교정. 가져오기가 만든 스프린트는 다른 등록 경로에서 409. 바인딩의 `sprint_id`는 시험 ID로 쓸 수 없다 | `an_import_owned_sprint_changes_only_through_imports`, `a_test_id_cannot_take_over_…`, X13 |
| S3 P3 다른 repo 호출자의 409가 존재·출처를 드러냄 | 유지. 저장 내용은 나가지 않는다. 같은 페이지 ID·digest를 아는 호출자에게만 드러나며, 스프린트 ID 이름 공간은 02-03부터 전역이다(재검토 0fa839d N2, 02-05 이월과 같은 범위) | — |
| S4 P3 OpenAPI 머리글의 "additive" | 교정(배정 동작 변경 명시) | — |
| S5 P3 §2의 경로 무관 표현 | 교정 | — |
| S6 P3 보관한 검토 파일의 큐 상태 표기 | 보관본의 그 인용 한 줄을 가리고 가린 사실을 그 자리에 적었다 | — |
| S7 P3 정책 개정 뒤 이전 digest의 판정은 선행 증거가 아님 | 사실을 적어 둔다. 영속 DB에는 판정·배정이 0건이라 지금 영향은 없다. 정책을 바꿀 때마다 기존 판정을 새 digest로 다시 받아야 선행 조건이 열린다(G1 그대로) | — |

## 8. 재검토 2 반영(rev 4)

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| T1 P2 가져오기 스프린트가 executable이 될 길이 없음(가져오기는 draft만, `/v0/sprints`는 전부 409) | 교정. 최신 가져오기에서 파생한 명세(같은 작업·같은 `notion_page` 출처 digest)는 `/v0/sprints`로 등록할 수 있다. 한도·준비 상태를 바인딩해 executable로 올리는 경로다. 작업이나 출처가 다르면 409 | `an_imported_draft_can_be_promoted_only_with_its_own_tasks_and_source`, X13·X14 |
| T2 P3 형식 오류보다 소유 409가 먼저, 문서 미반영 | 교정. 소유 검사는 형식 검사를 통과한 같은 repo 명세에만 적용된다(형식 오류 400). OpenAPI registerSprint·import 설명과 보고 §3 문구를 고쳤다 | 같은 시험 |
| T3 P3 예약 sprint_id·상한 조건의 시험 부재 | 시험 추가. 변환 경로의 소유 검사는 예약 규칙 때문에 도달할 수 없어 제거하고, 그 사실을 코드 주석에 적었다 | `a_binding_sprint_id_is_reserved_against_imports`, `only_an_executable_reviewed_newer_revision_sets_a_pinned_ceiling`, X15~X17 |
| T4 P3 새로 보관한 검토의 큐 상태 토큰 인용 | 그 셀의 토큰 인용만 가리고 가린 사실을 그 자리에 적었다 | — |
| T5 P3 다른 repo에 가져오기 출처가 드러남 | 교정. 다른 repo의 명세는 소유 검사 전에 `SPRINT_REPO_CONFLICT`로 끝난다 | 같은 시험 |
