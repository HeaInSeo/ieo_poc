# X-01 후속 교정 C1·C2 · Notion 원문 수집과 가져온 작업의 실행 통제

| 항목 | 기록 |
| --- | --- |
| 구분 | 추가 작업 X-01의 **후속 교정**(X-01의 검수 이력은 그대로 둔다). 73·12 완료 수 미산입 |
| 근거 | [사용자 결정 2026-11-13](../../../../User_Decisions_2026-11-13_x02.md) 보완 이슈 A·B, 결정 ③·⑤ |
| 산출물 | `tools/notion_from_transcript.py`(C1) · 코디네이터 실행 통제(C2): `notion:import`·`exceptions:approve` 범위, `notion_imports.gates/missing`, `exec_exceptions` 표, `POST /v0/coordinator/exceptions`, OpenAPI 0.8.0 |
| 모델 호출 | 0회 |
| 상태 | rev 1. 독립 검토 대기 |

## 1. C1 · Notion 원문을 옮겨 적지 않고 수집 (이슈 A, 결정 ⑤)

- **수집 경로.** 이 터미널의 Claude Code에 이미 연결된 Notion MCP 서버(`https://mcp.notion.com/mcp`, HTTP, 인증은 Claude Code가 보유)를 그대로 썼다. `claude mcp list`에서 Connected로 확인했다. 환경 변수에는 API 토큰이 없고, 기존 자격을 꺼내거나 새 인증을 만들지 않았다.
- **방법.** 세션에서 `notion-fetch`를 호출하면 Claude Code가 응답 원문을 세션 기록(JSONL)에 남긴다. `tools/notion_from_transcript.py`는 그 `tool_result` 텍스트를 그대로 비공개 파일(0600)로 복사하고 메타데이터를 남긴다: 기록 줄, tool_use_id, 기록 시각, 바이트 수, SHA-256, `page_last_edited_at`, 잘림 표지(`truncated`, `unknown_block_count`, 닫는 태그). 모델이 내용을 다시 쓰는 단계가 없다.
- **결과.** OC08 페이지를 두 번 수집했다(X-01 당시 호출과 이번 호출). 두 응답 원문은 같았다(17,203바이트, SHA-256 `0c2c7e82…`). 잘림 표지는 없었고 본문·페이지 닫는 태그가 모두 있었다. [c1-collection.txt](evidence/c1-collection.txt)
- **기존 사본과 비교.** 정규화 digest가 원문 두 건과 X-01 사본(옮겨 적고 뒷부분을 덧붙인 것) 모두 `3f8a300f…`로 같다. 속성 43개와 본문 5,034자도 같다. 목표·수용 기준(OC08-01~03)·선행조건·차단 사유·원 작업 ID도 같다. 따라서 **X-01 결과는 “당시 사본 기준 시험”으로 그대로 유효**하고, 다시 수행한 변환·등록·판정 시험은 없다. 기존 사본과 X-01 증거는 바꾸지 않았다.
- **한계.** 수집한 형태는 연결 도구가 그려 준 표현이다. REST API의 블록 JSON이 아니다. 두 경로의 digest는 원래 다르다(X-01 R5). REST 경로로 수집하려면 통합 토큰이 필요하지만 요청하지 않았다. 연결 경로가 동작하므로 결정 ⑤에 따라 새 발급을 요구하지 않는다. 기록 파일은 이 터미널의 세션 기록에 의존하므로, 다른 서버나 세션에서는 같은 도구를 해당 세션의 기록 경로로 돌려야 한다.

## 2. C2 · 등록자가 차단 사유를 지우는 경우의 실행 통제 (이슈 B, 결정 ③)

### 2.1 현재 한계 재현(교정 전 main 82c7db9)

큐 선행이 남은 가져오기 작업에서 일반 등록자가 미해결 사유를 모두 지우고 한도를 채운 파생 명세를 내면 이렇게 됐다. executable 등록 201, 의미 검토 201, 배정 201(`issued`). 서버가 강제한 것은 작업·Notion 출처의 동일성뿐이었고, 실행 허가는 등록자와 의미 검토자에게 달려 있었다: [c2-limit-reproduction.txt](evidence/c2-limit-reproduction.txt)

### 2.2 보완(최소, 기존 역할·범위 모델 재사용)

| 요소 | 동작 |
| --- | --- |
| 신뢰된 수집 기록 | `POST /v0/imports/notion`은 service 전용 범위 `notion:import`가 있어야 한다. `sprints:register`만 가진 등록자는 가져오기 기록이나 해제 기록을 쓸 수 없다(403). 가져오기 도구는 **큐 선행·차단(gates)**과 **필수 정보 누락(missing)**을 따로 보내고, 서버가 그 가져오기 행에 저장한다 |
| 실행 판정 | 가져온 스프린트의 executable 등록과 배정은 명세 문구가 아니라 **최신 가져오기 행**을 본다. missing이 있으면 409 `IMPORT_INCOMPLETE`(예외로도 풀리지 않음). gates가 있으면 409 `IMPORT_GATED`이고, 그 (시험 ID, repo)의 사용자 승인 예외가 있을 때만 통과한다. gates가 빈 새 가져오기가 신뢰된 해제 기록이다 |
| 사용자 승인 예외 | user 전용 범위 `exceptions:approve`로 `POST /v0/coordinator/exceptions`에 기록한다(추가 전용). 정확히 한 (시험 ID, repo)에만 적용되고 큐 선행만 푼다. 형식·바인딩·한도·의미 검토·필수 정보 검사는 그대로다. 가져오기 행의 gates는 그대로 남는다 |
| 이전 행 | C2 이전 가져오기 행은 gates/missing이 비어 있다(NULL). 이 경우 가져오기 서비스가 저장한 응답에서 가져오기 도구의 고정 사유 문형으로 나누고, 모르는 사유는 missing으로 본다(닫힌 실패) |

### 2.3 시나리오별 결과(시험·변이)

| 시험 상황 | 기대 | 결과 |
| --- | --- | --- |
| 선행 미해제 작업 | 실행 차단 | executable 등록 409 `IMPORT_GATED`, 배정 409 `IMPORT_GATED`(`c2_gated_…`, `an_imported_draft_…`). draft 등록은 허용 |
| 일반 등록자가 미해결 사유만 삭제 | 실행 가능해지지 않음 | 사유를 모두 지운 파생 명세도 409 `IMPORT_GATED`(교정 전 201 → 교정 후 409) |
| 근거 없이 해제됐다고 주장 | 신뢰 기록과 구별해 차단 | 등록자의 해제 가져오기는 403. 명세 제목의 “해제됨” 주장은 409. service·일반 등록자의 예외 기록은 403(`c2_claimed_…`) |
| 유효한 사용자 승인 PoC 예외 | 지정 시험 ID·repo에만 적용 | T02 예외로는 T01이 409. 다른 repo 예외는 403. T01 예외가 있으면 executable 201·의미 검토 201·배정 201. 가져오기 행의 gates는 유지(`c2_a_user_exception_…`) |
| 예외가 있어도 필수 기술 입력 부족 | 실행 차단 | 409 `IMPORT_INCOMPLETE`(`c2_an_exception_…`). 미바인딩 한도는 기존대로 422 |
| 정당한 해제·검토·필수 조건 충족 | 정상 실행 허용 | gates 없는 새 가져오기 → 파생 executable 201 → 의미 검토 201 → 배정 201(`c2_a_trusted_release_…`) |

항상 거부하는 구현이 아님을 위 마지막 두 허용 경로(예외, 신뢰된 해제)가 보인다. 변이 8개를 모두 검출했다(통제 제거, 배정 경로 누락, missing 무시, 예외 범위 확대, 등록자에게 가져오기 허용, 이전 행 닫힌 실패 제거, 가져오기 도구의 분류 오류, 해제 위조): [mutants.txt](evidence/mutants.txt). 시험 171건 통과, fmt·clippy·생성기 시험·패키지 검산·OpenAPI 통과: [cargo-test.log](evidence/cargo-test.log).

### 2.4 영속 DB에 적용(OC08 시험)

배포 전 `pg_dump`로 백업한 뒤 새 열을 이관했다. C2 이전 OC08 행 2건은 그대로 두었다. 결과는 [c2-live.txt](evidence/c2-live.txt)에 있다.
- 예외 없이 사유를 지운 파생 executable 명세를 내면 409 `IMPORT_GATED`이고 행은 바뀌지 않았다.
- 결정 ③을 user 승인 자격으로 별도 예외 기록 `EX-POCX-SORI-OC08-T01-20261113`(201)에 남겼다. 범위는 브랜치 `poc/POCX-SORI-OC08-T01`, 기준 SHA `2fa7f5e1…`, 기본 브랜치 병합·배포 없음, 원 작업 담당·상태·claim·차단 사유 유지다.
- 가져오기 service 자격은 예외를 기록할 수 없고, 승인 자격은 가져오기를 할 수 없다(각 403).
- Notion 원 작업은 바꾸지 않았다. 실행용 executable revision 등록은 X-02 준비 단계에서 한다.

**자격 보관의 한계.** 승인 자격과 가져오기 자격은 같은 호스트·같은 계정의 비공개 디렉터리에 있다. 서버는 “사용자 자격이 승인했는가”를 검증하지만, 그 자격이 사람에게만 있다는 보관 분리는 이 PoC 범위가 아니다(02-02 강한 봉인과 같은 종류의 미검증 항목, 04-07 담당).
