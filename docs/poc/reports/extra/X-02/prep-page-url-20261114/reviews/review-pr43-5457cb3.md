VERDICT: approve

# 리뷰 · PR #43 (fix/x02-page-id-url) · head 5457cb3f52769416439bc3c2320e6a6087dabda2

리뷰어: 독립 리뷰어(Guardrail, 읽기 전용) · 2026-11-14 · 리뷰 1
- head 확인: `gh pr view 43 --json headRefOid,mergeStateStatus` = 5457cb3f52769416439bc3c2320e6a6087dabda2. 리뷰를 시작할 때 BLOCKED(CI 진행 중)였고, 끝날 때 CLEAN이었습니다.
- base: main 86aed04. 커밋 1개입니다.
- CI 3개 job이 모두 pass입니다. rust job 로그의 `test result` 합계는 235 passed로, 본문의 "235 workspace tests"와 맞습니다.

## 결론

사용자 결정 2026-11-14 §5의 요구("가져온 페이지 ID와 URL 안의 페이지 ID 일치 검사, 불일치 거부 시험")는 충족됩니다.
- 서버 `POST /v0/imports/notion`은 정규형 `page_id` 검사 다음에 `url_page_id(url) == page_id`를 강제합니다.
- 불일치하면 400 `SCHEMA_VIOLATION`(field `source.url`)을 반환하고, 어떤 행도 쓰지 않습니다. 시험이 `e.counts()` 불변으로 이를 확인합니다.
- 실제 Notion URL 형태는 모두 받아들여집니다.
- 우회 시도(대소문자, 퍼센트 인코딩, 유니코드 유사 문자, 후행 슬래시, query나 fragment 속 id)로 **다른 페이지 id가 일치로 통과하는 경우는 찾지 못했습니다.**

아래 지적은 P2 1건과 P3 5건입니다. 차단 사유는 없습니다.

## 수행한 점검

### 실행 환경

scratch worktree(head 5457cb3)를 쓰고, CARGO_TARGET_DIR는 전용 디렉터리로 두었습니다. DB는 IEO_TEST_DB_URL(127.0.0.1:55432)입니다. 끝난 뒤 worktree와 target 디렉터리를 모두 지웠습니다.

### 시험 실행

`cargo test -p ieo-coord -p ieo-notion` 결과는 coordinator 66, importer 9, lib 1이며 모두 통과했습니다.

### 경계값 프로브

`ieo_coord::api::url_page_id`, `ieo_notion::url_page_id`, `page_id_from`을 28개 URL에 적용했습니다. 이를 위해 ieo-coord에 dev-dependency를 임시로 추가했습니다(scratch에서만).
- 두 `url_page_id` 사본의 결과는 28건 모두 같았습니다.
- 받아들여진 형태:
  - `app.notion.com/p/<id>`
  - `www.notion.so/Title-<id>`
  - `www.notion.so/acme/Title-<id>`(워크스페이스 경로)
  - 위 형태에 `?pvs=4`나 `#abc`를 붙인 것
  - 하이픈 UUID
  - 대문자 id(소문자로 바뀜)
  - 후행 `/` 또는 `//`
- 거부된 형태(None):
  - `<id>/sub`, `/p/<id>/x`(id가 마지막 세그먼트에 없음)
  - 전각 숫자 `０`
  - `<id>%2F`, `<id>\`, `<id> `(후행 공백)
  - 32자 비-hex 세그먼트
- query나 fragment에 들어 있는 id는 무시됩니다. 예를 들어 `notion.so/<O>?p=<id>`와 `notion.so/<O>#<id>`는 모두 `<O>`로 읽힙니다.
- 아래 형태는 모두 tail이 `<id>`로 읽힙니다. `<id>` 앞부분은 제목 slug로 취급되며, Notion이 slug를 읽는 방식과 같아서 불일치 통과가 아닙니다.
  - `<O>%2F<id>`
  - `<O>\<id>`
  - `<O><id>`(64 hex)
  - `x‐<id>`(U+2010)
  - `é<id>`
- `%30123…`처럼 퍼센트 인코딩된 hex는 그대로 hex 검사를 받습니다. 그래서 디코딩해서 다른 id가 되는 경로가 없습니다.

### 변이 시험

scratch 사본에서 변이 22개를 만들었습니다.

**ieo-coord**(시험 대상: coordinator 시험 파일)
- 서버 검사 제거: killed(`an_import_whose_url_names_another_page_is_refused`)
- `split(['?','#'])`를 없앰, `'?'`만, `'#'`만으로 바꿈: 모두 killed
- `trim_end_matches('/')` 제거: killed(`url_page_ids_are_read_strictly`)
- `rsplit('/')` 제거(trim은 유지): **survived**
- trim과 rsplit을 함께 제거: killed
- dash filter 제거: killed
- 소문자화 제거: killed
- 길이 31: killed
- `is_ascii_hexdigit` 검사를 `true`로 바꿈: **survived**

**ieo-notion**(시험 대상: importer 시험)
- snapshot 검사 제거: killed(`snapshot_round_trips_and_detects_tampering`)
- 길이 31: killed
- 나머지 9개(split 3종, trim, rsplit, trim+rsplit, dash, hex, lower): **모두 survived**

### 본문의 "동등 변이" 주장 검증

rsplit만 제거하고 trim은 유지하는 변이를 봤습니다.
- 마지막 세그먼트가 32자 이상이면 tail이 원본과 같습니다.
- 32자 미만이면 tail이 `/`를 포함해 hex 검사에서 실패합니다. 원본은 `checked_sub`에서 None을 냅니다. 결과는 둘 다 None입니다.
- dash 제거 범위 차이와 `get`의 char boundary 차이도 결과를 바꾸지 않습니다.

따라서 **주장은 정확합니다**(동등 변이 맞음).

### 그 밖의 입력 경로

- 서버 `notion_gate`/`page_ids_in`은 source 텍스트에서 후보 id를 수집하는 함수일 뿐, page_id와 URL을 함께 받는 입력 경로가 아닙니다. 예외 등록(line ~1362)은 page_id를 서버 레코드에서 가져옵니다. 따라서 page_id와 URL을 함께 받는 서버 입력은 `import_notion` 하나입니다.
- 클라이언트 경로는 다음과 같습니다.
  - `ieo-notion import --snapshot` → `from_snapshot`. 검사가 있습니다.
  - `ieo-notion import --page` → `api::fetch_page`. page_id는 `--page` 인자에서, url은 Notion의 `page["url"]`에서 옵니다. 클라이언트 쪽 검사는 없고, 서버 검사에 의존합니다.
  - `ieo-notion snapshot --from-connector` → `from_connector_fetch`. page_id를 `page_id_from(url)`로 구하고, 검사 없이 snapshot을 씁니다.
  - `import_request`에는 검사가 없습니다. 위 경로의 결과를 그대로 직렬화합니다.

### 영속 DB

본문은 "영속 DB의 import 3건이 모두 `https://app.notion.com/p/<page_id>`"라고 합니다. 저는 지시에 따라 영속 DB에 접근하지 않았으므로 **미검증**입니다.

## 지적 사항

1. **P2: ieo-notion의 `url_page_id` 사본은 시험이 사실상 없습니다(변이 11개 중 9개 생존).**
   - 사본은 "Same rule as the coordinator's import check"라고 주석을 달고 있습니다. 그러나 이를 잡는 시험은 snapshot 왕복 시험의 불일치 1건(`app.notion.com/p/<other>`)뿐입니다.
   - 다음을 바꾸는 변이가 모두 통과합니다: split(? / #), 후행 슬래시 trim, 마지막 세그먼트 선택, dash 제거, hex 검사, 소문자화.
   - 두 사본이 갈라지면 어떤 영향이 있을까요? 클라이언트가 느슨해지면 서버가 막아 줍니다. 반대로 클라이언트가 엄격해지면, 정상 URL(예: `?pvs=`, 하이픈 UUID, 워크스페이스 경로)이 전송 전에 거부됩니다. 그래도 아무것도 드러나지 않습니다.
   - 권고는 둘 중 하나입니다.
     - (a) `url_page_ids_are_read_strictly`의 사례를 `ieo-notion/tests/importer.rs`에도 같은 표로 넣습니다.
     - (b) 두 crate가 공유하는 작은 함수로 합칩니다. 예를 들어 `ieo-spec`에 두거나, coord가 쓸 수 있는 공통 위치에 둡니다.
   - 이번 PR의 핵심인 서버 검사에는 영향이 없으므로 차단하지 않습니다.

2. **P3: 본문의 변이 보고가 불완전합니다.**
   - 본문은 생존 변이로 `rsplit` 제거 1개만 적었습니다.
   - 실제로는 coord의 `is_ascii_hexdigit` 검사를 `true`로 바꾼 변이도 생존합니다.
     - 서버 수준에서는 동등합니다. `page_id`가 먼저 32자 소문자 hex로 강제되므로, 비-hex tail이 같다고 판정될 수 없습니다.
     - 그러나 공개 함수 `url_page_id` 수준에서는 동등하지 않습니다. 예: `notion.so/ghijklmnopqrstuvwxyzghijklmnopqr`에 대해 `Some(non-hex)`를 반환합니다.
   - ieo-notion의 생존 변이 9개(지적 1)도 본문에 없습니다.
   - `url_page_ids_are_read_strictly`에 `("https://www.notion.so/ghijklmnopqrstuvwxyzghijklmnopqr", None)` 한 줄을 추가하면 hex 변이를 잡을 수 있습니다.

3. **P3: 본문의 "the importer fails before sending"은 `--snapshot` 경로에만 맞습니다.**
   - `ieo-notion import --page`(REST) 경로는 `fetch_page`가 page_id(`--page` 인자에서)와 url(Notion 응답에서, 없으면 빈 문자열)을 따로 가져옵니다. 이 경로에는 클라이언트 쪽 일치 검사가 없습니다. 서버가 400으로 막으므로 안전합니다(fail-closed).
   - 본문 문구를 "snapshot 경로에서는 전송 전에 실패"로 고치거나, `import_request`에 같은 검사를 두어 모든 경로를 덮는 것을 권합니다.

4. **P3: connector snapshot 경로는 다른(느슨한) 파서 `page_id_from`을 씁니다.**
   - 본문은 "The connector-fetch path already derives page_id from the URL"이라고 하지만, 그 파서는 `url_page_id`와 결과가 다릅니다. 프로브 결과:

     | URL | `page_id_from` | `url_page_id` |
     |---|---|---|
     | `/p/<id>/x` | `<id>` | None |
     | `<id>/sub` | `123…defb`(한 글자 밀린 잘못된 id) | None |
     | `<id>%2F` | `23…ef2f`(잘못된 id) | None |
     | `<id>\` | `<id>` | None |

   - 그 결과 `ieo-notion snapshot`이 snapshot을 성공적으로 써 놓고, 나중에 `import --snapshot`의 `from_snapshot`이 그것을 거부할 수 있습니다.
   - 모두 fail-closed라 안전성 문제는 아닙니다. 그래도 `from_connector_fetch`(그리고 CLI `--page` 인자)가 `url_page_id`를 쓰거나, snapshot을 쓰기 전에 같은 검사를 하도록 일관화하는 것을 권합니다. `page_id_from`의 밀린-id 동작은 이 PR 이전부터 있던 문제입니다.

5. **P3: URL의 scheme과 host는 검사하지 않습니다.**
   - 다음 값이 모두 page_id와 일치하는 것으로 통과합니다: `https://evil.example/<id>`, 맨 id 문자열 `<id>`, `javascript:<id>`.
   - 요구사항은 "페이지 ID 일치"이므로 범위 밖입니다. 다만 이 URL은 `notion_imports.url`에 저장되고, 보고서나 화면에 링크로 쓰일 수 있습니다. 그래서 `https://`와 Notion 호스트(`www.notion.so`, `notion.so`, `app.notion.com`, `*.notion.site`)를 허용 목록으로 두는 것을 후속 과제로 고려할 만합니다.
   - 관련 사례가 하나 더 있습니다. `notion.so/<db>?p=<page>`(peek) 형태는 `<db>`로 읽힙니다. 브라우저에서는 다른 페이지가 열리지만, 경로가 가리키는 페이지 기준으로 일관되므로 문제로 보지 않습니다.

6. **P3: 시험이 요청된 URL 형태 가운데 일부를 다루지 않습니다.**
   - 다음 형태는 프로브에서 정상 동작을 확인했지만, 저장소 시험에는 없습니다.
     - 워크스페이스 접두 경로 `www.notion.so/<workspace>/<Title>-<id>`
     - 대문자 id를 실은 서버 경유 import
   - `an_import_whose_url_names_another_page_is_refused`의 수락 목록에 워크스페이스 경로 1건을 추가하면, 실제 공유 URL 형태 회귀를 잡을 수 있습니다.

## 본문 정확성 요약

| 주장 | 판정 |
|---|---|
| 서버 400 `SCHEMA_VIOLATION`(`source.url`), 행 기록 없음 | 정확 |
| 엄격 규칙(쿼리·fragment 제거, 마지막 세그먼트, dash 제거, 32 hex로 끝남, 소문자) | 코드와 일치 |
| 세 Notion 형태 수락 | 정확(시험 포함) |
| 기존 시험 3개(4곳)에 URL 보정 | 정확 |
| 서버 검사 제거와 snapshot 검사 제거 변이가 killed | 정확(재현함) |
| `rsplit` 제거는 동등 변이 | 정확. 단, 다른 생존 변이 누락(지적 2) |
| importer가 전송 전 실패 | `--snapshot` 경로만 해당(지적 3) |
| connector 경로는 URL에서 page_id 도출 | 다른 파서 사용(지적 4) |
| 235 tests pass | CI 로그 합계 235로 일치 |
| 영속 DB 3건이 규칙 충족 | 미검증(접근 금지) |
| Model calls: 0 | 확인 불가 항목 아님(코드 변경만) |
