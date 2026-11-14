VERDICT: approve

# 리뷰 · PR #44 (fix/x02-page-url-shared) · head 65d56ce3b6384e0575c2427b10428b42e29fc3d6

리뷰어: 독립 리뷰어(Guardrail, 읽기 전용) · 2026-11-14 · 리뷰 1
- head 확인: `gh pr view 44 --json headRefOid,mergeStateStatus` = 65d56ce3b6384e0575c2427b10428b42e29fc3d6. 시작할 때 BLOCKED(리뷰 대기), 끝날 때 CLEAN.
- base: main 42b435e. 커밋 1개, 8개 파일(+89/−55).
- CI 3개 job 모두 pass. rust job 로그의 `test result` 합계는 236 passed로 본문의 "236 workspace tests"와 맞습니다(235 − coord 단위 시험 1 + spec 1 + notion 1).
- 선행 리뷰: review-pr43-5457cb3.md(P2 1건, P3 5건).

## 결론

사용자 요구("가져온 페이지 ID는 URL 안의 페이지 ID와 같아야 하고, 다르면 거부")는 서버와 importer 양쪽에서 같은 단일 규칙으로 충족됩니다. 리뷰 5457cb3의 P2(사본 두 개, importer 쪽 시험 없음)는 해소되었습니다. 차단 사유는 없고, 아래 P3 2건은 시험 보강 권고입니다.

## 수행한 점검

### 단일 구현
- `ieo_spec::notion_url::page_id`가 유일한 구현입니다. 본문은 PR #43의 두 사본과 글자 단위로 같습니다(규칙 변경 없음).
- `ieo_coord::api::url_page_id`와 `ieo_notion::url_page_id`는 모두 한 줄 위임입니다. 저장소 전체에서 다른 사본은 없습니다(`grep`).
- 서버(`import_notion`, api.rs ~780)는 `url_page_id`를 그대로 씁니다.

### 의존성
- `ieo-spec`의 dependencies에는 내부 crate가 없습니다(jsonschema, serde, serde_json, sha2, hex). 따라서 ieo-notion → ieo-spec 추가로 순환이 생기지 않습니다.
- Cargo.lock에 `ieo-spec` 1줄이 추가되었고, `cargo test --locked`가 통과합니다.
- 부수 효과: ieo-notion이 ieo-spec을 통해 jsonschema를 함께 빌드합니다. 빌드 시간만 늘고 동작 영향은 없습니다(지적 아님).

### importer 경로
- `import_request` 맨 앞에서 `url_page_id(p.url) == p.page_id`를 검사합니다. CLI `import`는 `--snapshot`과 `--page`(REST) 모두 `import_request`를 거쳐 전송하므로, 모든 전송 경로가 전송 전에 검사됩니다. `--dry-run`도 같습니다.
- `from_connector_fetch`는 page_id를 엄격 규칙(`url_page_id`)으로 구합니다. 느슨한 `page_id_from`은 CLI `--page` 인자 해석과 REST `fetch_page`의 page_id 정규화에만 남아 있습니다. 이는 본문 설명과 같습니다.
- REST 경로: `fetch_page`는 url을 Notion 응답 `page["url"]`에서, 없으면 빈 문자열로 가져옵니다. 이제 빈 url이면 클라이언트가 전송 전에 거부합니다(전에는 서버가 400). fail-closed이며 Notion API page 객체는 항상 `url`을 줍니다.

### 정상 URL 형태 회귀
저장된 OC08 데이터(ieo_poc-data, 읽기만 함)로 head 바이너리를 돌렸습니다. 출력은 scratchpad에만 썼습니다.
- connector fetch 원본 5개(`oc08-connector-fetch-20261113.json`, `notion-raw/*mcp-fetch*.json`)에 `ieo-notion snapshot --from-connector`: 5개 모두 성공, page `3fd86e6dd1d181a3a9cace91920adac0`.
- 저장된 snapshot 3개(`oc08-snapshot.json`, `oc08-snapshot-simulated-edit.json`, `notion-raw/oc08-mcp-fetch-20261113T131656.snapshot.json`)에 `import --snapshot --dry-run`: 3개 모두 rc=0.
- 저장소와 데이터에 나오는 URL 형태는 `https://app.notion.com/p/<id>`, `…?pvs=204`뿐이고 모두 받아들여집니다. 단위 표는 이에 더해 제목 접두, 워크스페이스 경로, `?pvs=`, `#fragment`, 하이픈 UUID, 대문자, 후행 `/`를 받아들입니다.
- 영속 DB는 지시에 따라 접근하지 않았습니다. 저장 import 3건의 URL 형태는 위 snapshot 파일과 같은 형태로 보이지만 DB 자체는 미검증입니다.

### 시험 실행
scratch worktree(head 65d56ce), 전용 CARGO_TARGET_DIR, IEO_TEST_DB_URL(127.0.0.1:55432). 디스크가 부족해 `-p ieo-spec -p ieo-notion -p ieo-coord`만 debug=0으로 돌렸습니다. coordinator 65, importer 10, spec lib 16 등 모두 통과. 전체 236은 CI 로그로 확인했습니다.

### 변이 시험(19개, 직접 작성)

`ieo-spec --lib`만으로(본문 S1–S5 주장 검증):

| 변이 | 결과 |
|---|---|
| S1 hex 검사 → true | killed |
| S2 query/fragment split 제거, `?`만, `#`만 | 3개 모두 killed |
| S3 trim 제거 | killed |
| S4 dash filter 제거 | killed |
| S5 소문자화 제거 | killed |
| S6 rsplit 제거 | **survived** |
| 길이 31 | killed |
| tail 대신 세그먼트 전체 | killed |
| rsplit을 `/`와 `-`로 | killed |
| trim에 `%2F`도 제거 | killed |

S6은 본문 설명대로 동등 변이입니다. 마지막 세그먼트가 dash 제거 후 32자 이상이면 tail이 같고, 짧으면 tail에 `/`가 들어가 hex 검사에서 None이 됩니다. 원본도 None입니다.

ieo-notion / ieo-coord:

| 변이 | 결과 |
|---|---|
| N1 `import_request` 검사 제거 | killed (`nothing_is_sent_for_a_page_whose_url_names_another_page`) |
| N2 `import_request`에서 URL에 id가 없으면 통과 | killed (같은 시험) |
| N3 `from_connector_fetch`를 `page_id_from`으로 되돌림 | **survived** |
| N4 `from_snapshot` 검사 제거 | killed (`snapshot_round_trips_and_detects_tampering`) |
| N5 `ieo_notion::url_page_id`를 `page_id_from`으로 | **survived** |
| C1 서버 검사 제거 | killed (`an_import_whose_url_names_another_page_is_refused`) |
| C2 서버에서 URL에 id가 없으면 통과 | killed (같은 시험) |

본문의 변이 주장(S1–S5 killed, S6 동등, N1·C1 killed)은 모두 재현됩니다.

## 지적 사항

1. **P3: connector 경로가 엄격 규칙을 쓴다는 점을 잡는 시험이 없습니다(N3 생존).**
   - `from_connector_fetch`를 PR 이전의 `page_id_from`으로 되돌려도 모든 시험이 통과합니다. connector 시험 픽스처가 두 파서가 같은 답을 내는 URL만 쓰기 때문입니다.
   - 되돌아가도 결과는 fail-closed입니다. 예를 들어 `…/p/<id>/sub`에서 밀린 id로 snapshot을 쓰더라도 `from_snapshot`/`import_request`가 거부합니다. 그래서 안전성 문제는 아니고, "connector 경로는 엄격 규칙" 주장이 회귀에 대해 고정되지 않았다는 점만 남습니다.
   - 권고: connector fetch 시험에 `<page url="https://app.notion.com/p/<id>/sub">` 같은 1건을 넣어 `from_connector_fetch`가 Err인지 확인.

2. **P3: ieo-notion이 공유 규칙에 위임한다는 점을 잡는 시험이 없습니다(N5 생존).**
   - `ieo_notion::url_page_id`를 느슨한 `page_id_from`으로 바꿔도 통과합니다. 새 시험 `nothing_is_sent_…`의 거부 사례(다른 id, id 없음)는 두 파서 모두 거부하기 때문입니다.
   - 서버가 같은 규칙으로 막으므로 안전성 영향은 없습니다. 위임이 한 줄이라 갈라질 위험도 작습니다.
   - 권고: 그 시험에 `p.url = format!("https://app.notion.com/p/{PAGE}/x")` → `is_err()` 1줄을 추가하면 N3·N5 모두 잡힙니다(`p.url`은 `import_request`가, `from_connector_fetch`는 별도 1건).

## 선행 리뷰(5457cb3) 지적 처리

| 지적 | 처리 |
|---|---|
| P2 importer 사본 시험 없음 | 해소. 사본 제거, `ieo-spec` 단위 표로 고정 |
| P3 hex 변이 생존·변이 보고 누락 | 해소. `ghij…` 사례 추가로 S1 killed. 본문 변이 보고 정확 |
| P3 "전송 전 실패"는 snapshot 경로만 | 해소. `import_request`에서 모든 경로 검사 |
| P3 connector가 느슨한 파서 사용 | 해소(코드). 시험 고정은 지적 1 |
| P3 scheme/host 미검사 | 본문과 모듈 doc에 범위 밖으로 명시. 동의 |
| P3 워크스페이스 경로·대문자 서버 시험 | 해소. coordinator 시험에 2건 추가 |

## 본문 정확성

| 주장 | 판정 |
|---|---|
| `ieo_spec::notion_url::page_id` 단일 구현, coord·notion 둘 다 사용 | 정확 |
| ieo-notion에 ieo-spec 의존 추가 | 정확. 순환 없음, lockfile 갱신 |
| 수락·거부 목록 | 단위 표와 일치 |
| `import_request`가 connector·snapshot·REST 모두 전송 전 검사 | 정확 |
| connector 경로는 엄격 규칙, `page_id_from`은 `--page` 인자용으로만 유지 | 정확(REST `fetch_page`의 page_id 정규화에도 쓰이나 같은 인자에서 온 값) |
| 서버 수준 시험에 워크스페이스·대문자 URL 추가 | 정확 |
| S1–S5 killed by `notion_url::tests` | 정확(재현) |
| N1, C1 killed | 정확(재현) |
| S6 생존, 동등 변이 | 정확 |
| host allow-list는 범위 밖 | 정확. 모듈 doc에도 명시 |
| 236 tests pass | CI 로그 합계 236 |
| Model calls: 0 | 코드 변경만이라 해당 없음 |

## 정리
scratch worktree와 전용 target 디렉터리는 삭제했습니다. 저장소, GitHub, Notion, 영속 DB, ieo_poc-data, secrets에는 쓰지 않았습니다.
