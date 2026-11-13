VERDICT: approve

# PR #37 독립 검토 1 (Guardrail) · head 784d60e · base main 0086b0d

## 요약

- **대상.** `fix/x01-c2-l1-l3`의 head는 `784d60efae799718cd8615fd03a1228efe50c950`이다. 검토를 시작할 때 `gh pr view 37 --json headRefOid,mergeStateStatus`의 결과는 이 커밋과 `BLOCKED`였다. 그때 CI "rust fmt clippy test"는 pending이었고, 나머지 두 건(docs/poc 검증, toolchain pin)은 pass였다. 검토가 끝날 때는 CI 세 건이 모두 pass였고, 상태는 `CLEAN`이었다. head는 그대로였다. origin/main은 `0086b0d`이고, `git merge-tree`에서 충돌은 없다.
- **변경.**
  - 서버 코드: `import_notion`이 `source.page_id`를 32자리 소문자 hex로만 받는다. 그 밖은 400 `SCHEMA_VIOLATION`이다. K1 doc 주석도 함께 고쳤다.
  - 시험: `l3_page_ids_are_accepted_only_in_canonical_form`(대시, 대문자, 31자, hex 아님 → 400·행 불변; 정규형 → 201).
  - 계약: OpenAPI `page_id`에 `pattern: '^[0-9a-f]{32}$'`를 넣었다. 게이트 설명에 "두 test id 페이지는 409 `IMPORT_GATED`"를 더했다.
  - 문서: REPORT §2.2·§2.4·§3(G4·H2 행, L1–L3 행), `evidence/persistent-db-checks.txt`(새 파일), mutants.py(C20), mutants.txt, cargo-test.log.
  - `5277e6c`부터 head까지 `crates/`와 `contracts/`는 바뀌지 않았다. 뒤의 두 커밋은 증거 파일만 바꿨다.
- **검사 재실행(head, 임시 worktree).**
  - fmt는 통과했다.
  - `cargo test --locked --all-targets`: **208건 통과**, 실패 0, 건너뛴 시험 0. 시험 DB에 실제로 연결했다(coordinator 64건). cargo-test.log의 208건과 같다.
  - clippy는 로컬에서 돌리지 않았다. 디스크 여유가 2–4.7 GB뿐이었다. 같은 head의 CI가 pass이다.
  - mutants.py 25개를 head에서 **한 번에** 다시 돌렸다. 24개가 검출됐고, C4a만 생존했다. 실행 뒤 작업 트리는 깨끗했다. mutants.txt와 같다.
- **L1–L3 판정.**
  - L3은 해소됐다. f571d1e 검토 시험 E(대시 표기를 두 번째 test id로 가져오기)가 이제 **400이고 행이 바뀌지 않는다**. base에서는 201이었다. 정규형 가져오기·재조회·해제·파생은 그대로 통과한다.
  - L1은 대부분 해소됐다. §2.2에 새로 생긴 문장 순서 결함 하나가 남았다(M1).
  - L2는 해소됐다. 다만 기록된 질의문이 실제 실행한 SQL이 아니라 요약 표기이다(M2).
- **새 발견은 P3 셋이다.** P2 이상이 없으므로 approve한다.

## 이전 지적 재판정(검토 f571d1e)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| L1 P3: §2.2 근거 표기 오류, G4·H2 행 마침표, OpenAPI 누락 | **해소**(새 문장 결함 M1) | §2.2 괄호가 "(검토 7afd4d1 G3)"이다. G4·H2 행에 마침표가 들어갔다. OpenAPI 49–60행에 "A cited page with imports under more than one test id … is refused (409 `IMPORT_GATED`)"가 있다. 코드(`notion_gate`의 `test_ids > 1`)와 맞다 |
| L2 P3: "영속 DB 0건"의 기록 없음 | **해소**(표기 결함 M2) | `evidence/persistent-db-checks.txt`에 시각, DB 이름, 열 이름, 결과 한 줄이 있다. REPORT §2.2와 §3 K1·L2 행이 이 파일을 가리킨다 |
| L3 P3: 서버가 page_id를 정규화하지 않음 | **해소** | 아래 "L3 검토" |

이전 지적(F–K)도 회귀가 없다. 해당 시험이 모두 통과하고, 대응 변이 C1–C19도 C4a를 빼고 모두 검출된다.

## L3 검토

**코드.** 검사는 `api.rs:757–772`에 있다. `str_field(…, 64)` 바로 뒤, 첫 DB 질의(`835`행 재조회)보다 앞이다. 거부는 `Err`이므로 `run`이 트랜잭션을 롤백한다. 그래서 거부된 요청은 어떤 행도 남기지 않는다. 바이트 단위로 `0-9`와 `a-f`만 허용하고 길이는 정확히 32이다. 그래서 대문자, 공백, 개행, 전각 숫자, URL 같은 우회 표기가 모두 걸린다.

**검토 시험 E와 변형.** 저장소에 넣지 않았다(`<scratch>/r37-probes.rs`). head에서 돌렸다. 먼저 T01로 P(`0123…cdef`, 차단)를 가져왔다.

| 요청 | head | base 0086b0d(f571d1e 검토 기록) |
| --- | --- | --- |
| **E: P를 대시 표기로 T02에** | **400 `SCHEMA_VIOLATION`** | 201 |
| P를 대문자로 T03에 | 400 | (미시험) |
| 뒤 공백, 앞 공백, 뒤 개행, `{…}`, 전각 숫자, URL 전체 | 모두 400 | (미시험) |
| P를 대시 표기로 **같은 T01에** | 400 | — |
| 위 거부 9건 뒤 행 수 | **불변** | — |
| P(정규형)를 T02에 | 409 `EVENT_CONTENT_CONFLICT`(G3 규칙) | 같음 |
| 다른 페이지(정규형)를 T02에 | **201** | 201 |
| T01 같은 내용 재조회 | **200 `replay: true`** | 같음 |
| T01 해제(gates 빈 새 내용) → 파생 executable | **201 → 201** | 같음 |

- **나쁜 입력은 거부된다.** 시험 E가 만들던 "한 페이지, 두 문자열"의 상태가 새 데이터에서는 생기지 않는다. 이제 G3 검사(`page_id = $1`)와 K1 검사(`count(DISTINCT test_id)`)의 문자열 비교가 같은 페이지를 놓치지 않는다.
- **좋은 입력은 통과한다.** 정규형 가져오기, 재조회, 해제, 파생 등록이 그대로 통과한다. 그래서 항상 거부하는 구현이 아니다.
- **출처 쪽과 일치.** 출처 인용(`page_ids_in`)은 원래 대시를 지우고 소문자로 바꾼 32자 후보만 만든다. 저장 값이 정규형으로 고정되었으므로, 이제 이 둘이 같은 표기로 비교된다. 예전에는 대시로 저장된 행을 출처 인용이 영영 찾지 못할 수 있었는데, 그 틈도 닫혔다.

**신뢰된 가져오기 도구(`crates/ieo-notion`)가 정규형을 보내는가.**

| 경로 | page_id의 출처 | 판정 |
| --- | --- | --- |
| `page_id_from`(`lib.rs:58`) | URL 또는 대시 UUID에서 hex 32자를 뽑아 `to_lowercase()` | 결과는 항상 정규형이거나 `None`이다. `None`이면 호출자가 오류를 낸다 |
| `--page`(REST, `bin/ieo-notion.rs:66` → `api::fetch_page`) | `page_id_from(--page)`로 정규화한 값 → `fetch_page`가 다시 `page_id_from` | 정규형. `unwrap_or_else` 대체 경로는 정규형 입력에서는 실행되지 않는다. 다른 호출자가 비정규 값을 넘기면 서버가 400이다(닫힌 실패) |
| connector 스냅샷 만들기(`from_connector_fetch`, `lib.rs:100`) | `page_id_from(url)` | 정규형. URL에 ID가 없으면 오류 |
| `--snapshot`(`from_snapshot`, `lib.rs:358`) | 파일의 `page_id`를 그대로 사용 | 도구가 만든 스냅샷은 위 경로를 거치므로 정규형이다. 손으로 고쳤거나 오래된 파일이 비정규면 서버가 400이다(닫힌 실패). 관련 참고는 M3 |

`importer.rs`의 `page_ids_from_urls`가 URL과 대시 UUID에서 정규형이 나옴을 고정한다. 그러므로 정당한 가져오기는 깨지지 않는다.

**page_id를 받는 다른 진입점.**

- `POST /v0/sprints`(등록): 출처의 page_id는 저장하지 않고 `page_ids_in`으로 정규화된 후보만 만들어 비교한다. 문제없음.
- `POST /v0/coordinator/exceptions`: 요청에서 page_id를 받지 않는다. 서버가 최신 가져오기 행에서 읽는다(`api.rs:1335`). 문제없음.
- `notion_imports`에 쓰는 곳은 `import_notion`뿐이다(`INSERT INTO notion_imports`가 한 곳). 다른 라우트(`api.rs:289–313`)에는 page_id 입력이 없다.

**시험과 변이.** `l3_…`는 대시, 대문자, 31자, hex 아님 네 경우를 400·행 불변으로 단언하고, 정규형 201로 허용 경로도 단언한다. C20(`if false && !(…)`)은 검사 전체를 끈다. 대문자 경우가 있으므로 `is_ascii_hexdigit`로 약하게 바꾼 변이도 걸린다.

## L2 검토(파일만으로 판단)

`persistent-db-checks.txt`의 결과 줄은 `2026-11-13 17:05:41.852346+00|3|0|0|1`이다.

- **내부 일관성.** 가져오기 3건, 서로 다른 페이지 1개, 두 test id 페이지 0건이다. 이것은 §2.5·§2.6의 기록(`POCX-SORI-OC08-T01` 한 시험 ID에 revision 1·2·3, 3번째가 C1 원문의 재가져오기)과 맞는다. 비정규 ID 0건도 영속 DB의 행이 정규형 도구 경로로만 들어왔다는 점과 맞는다.
- **시각.** 질의 시각(17:05:41Z)은 L3 커밋 `5277e6c`(17:05:50Z)보다 9초 앞이다. 순서상 모순은 없다.
- 그래서 REPORT §2.2의 "그런 페이지와 비정규 ID는 0건"과 K1 행의 "0건"은 이 파일로 뒷받침된다. 다만 기록된 질의 줄에는 결함이 있다(M2).

## 새 발견

### M1 · P3 · §2.2 "따라서"가 새로 끼워 넣은 문장 뒤로 밀림

- **위치.** REPORT §2.2 "실행 판정" 행: "… [영속 DB 확인] (링크 대상 생략)에서 그런 페이지와 비정규 ID는 0건이다. **따라서** 다른 sprint_id로 만든 사본도 판정 대상이다."
- **내용.** "따라서"는 원래 ②·③의 대상 페이지 설명을 받는 말이다. 이번에 영속 DB 문장을 그 앞에 넣으면서, "DB에 0건이므로 사본도 판정 대상"이라는 엉뚱한 인과로 읽힌다. L1과 같은 종류의 문장 이음새 결함이다.
- **권고.** 영속 DB 문장을 행 끝으로 옮기거나 "따라서" 문장을 ②·③ 설명 바로 뒤로 옮긴다.

### M2 · P3 · 증거 파일의 질의 줄이 실행한 SQL이 아님

- **위치.** `evidence/persistent-db-checks.txt` 3행: `SELECT now(), count(imports), count(page_id !~ '^[0-9a-f]{32}$'), count(pages with count(DISTINCT test_id) > 1), count(DISTINCT page_id)`.
- **내용.**
  - `count(imports)`, `count(pages with …)`는 SQL이 아니다. 실행할 수 없는 요약 표기이다.
  - 특히 `count(page_id !~ '…')`를 글자 그대로 실행하면 0이 아니라 **3**이 나온다. PostgreSQL의 `count(expr)`는 NULL이 아닌 값을 세므로, 불리언 식은 참·거짓 모두 센다. 실제로는 `count(*) FILTER (WHERE …)` 같은 다른 질의를 썼을 것이다. 결과(0)는 아마 맞지만, 적힌 질의와 결과가 서로 맞지 않는다.
  - 접속 계정·읽기 전용 여부(트랜잭션 모드 등)도 적혀 있지 않다.
  - L2의 목적은 X-02 재적용 때 같은 확인을 다시 하는 것이다. 지금 기록으로는 그대로 재실행할 수 없다.
- **등급.** 결과가 다른 기록과 일관되고, K1·L3 코드가 닫힌 실패이므로 이 숫자가 틀려도 열린 실패는 생기지 않는다. 그래서 P3이다.
- **권고.** 실제로 실행한 SQL 전문(예: `SELECT now(), count(*), count(*) FILTER (WHERE page_id !~ '^[0-9a-f]{32}$'), (SELECT count(*) FROM (SELECT page_id FROM notion_imports GROUP BY page_id HAVING count(DISTINCT test_id) > 1) x), count(DISTINCT page_id) FROM notion_imports;`)과 실행 방식(`psql` 명령, `default_transaction_read_only` 등)을 남긴다. X-02 재적용 때 같은 질의를 다시 돌려 붙인다.

### M3 · P3 · 영속 코디네이터는 아직 c5a3d3a 코드이고, 재적용 의무가 REPORT에 명시되어 있지 않음

- **내용.**
  - §2.6에 따르면 영속 코디네이터는 main `c5a3d3a` 빌드이다. 후속 PR 1–4(I1–L3)는 배포되지 않았다. 그래서 영속 서버는 지금도 비정규 page_id를 받아들이고(L3 이전), 두 test id 페이지를 거부하는 K1 검사도 없다. 즉 L3이 고친 "대시 표기로 두 번째 test id" 상태를 영속 서버에서는 아직 만들 수 있다.
  - 코드 주석과 OpenAPI의 "possible only in data from before that was refused"는 **이 저장소의 코드**에 대해서는 맞다. 그러나 영속 DB에 대해서는 재배포 전까지 성립하지 않는다. `persistent-db-checks.txt`는 그 시점의 사진일 뿐이다.
  - §2.6 "이월"은 I1–I3를 "후속 PR에서 처리했다"로 끝나고, 후속 PR의 코드를 영속 코디네이터에 다시 적용해야 한다는 문장이 없다. f571d1e 검토는 이것을 "기존 서술이 유효"하다고 봤지만, 다시 읽으면 §2.5의 "다시 적용한다"는 교정 2에 대한 문장이고, 그것은 c5a3d3a에서 이미 끝났다.
  - 참고(같은 항목): `from_snapshot`은 파일의 `page_id`를 URL이나 digest와 대조하지 않는다. 스냅샷의 `page_id`만 다른 정규형 ID로 바꾸면 digest 검사를 통과하고, 서버는 그 페이지로 기록한다. 스냅샷은 신뢰된 가져오기 운영자 쪽 파일이므로 이번 PR 범위 밖이고 등급을 올리지 않는다. 서버가 `page_id`가 `url` 안의 ID와 같은지 확인하면(이미 `page_ids_in`이 있음) 값싼 보강이 된다.
- **등급.** 아직 X-02 실행 전이고, 영속 DB에 실행 가능한 등록이 없으므로 P3이다.
- **권고.** §2.6 또는 §3 끝에 "후속 PR 1–4는 영속 코디네이터에 아직 적용되지 않았다. X-02 전에 main 빌드를 재적용하고, 같은 날 `persistent-db-checks` 질의를 다시 실행해 붙인다"를 적는다. 이 일은 사용자 결정의 X-02 준비 순서에 넣는다.

## 확인했고 문제없는 항목

- **신뢰 기록만으로 판정.** 새 검사는 신뢰된 service(`notion:import`)의 요청 형식을 좁힐 뿐이다. 판정을 푸는 방법은 여전히 둘뿐이다. 신뢰된 해제 행과 사용자 예외 행(user `exceptions:approve`)이다. 명세 문구는 판정에 쓰이지 않는다.
- **OpenAPI.** `page_id`의 `pattern`은 코드 검사와 같다(소문자 hex 32자). 오류 코드 `SCHEMA_VIOLATION`은 이미 문서화된 코드다. 새 게이트 문장은 코드와 맞다.
- **변이 증거의 정직성.** mutants.txt는 이제 한 커밋(`5277e6c`)에서 한 번에 돌린 결과만 적는다. 이전 검토가 지적한 "서로 다른 커밋의 합산" 문제가 없어졌다. 내가 head(`crates/`가 `5277e6c`와 같음)에서 25개를 한 번에 다시 돌린 결과도 **24/25**이다. C4a만 생존했고 dirty는 none이었다. 검출 시험 이름도 mutants.txt와 같다. C4a 동등 판단은 이전 검토와 같고, 정규형 강제는 "한 페이지 = 한 test id" 전제를 더 강하게 만들 뿐이다.
- **208건.** 이전 207건에 `l3_…` 하나가 더해졌다. 내가 다시 돌린 수(208 통과, coordinator 64)와 같다. cargo-test.log의 머리 커밋은 `6d5b204`이고 그 dirty 항목은 log 자신뿐이다. `6d5b204`와 head의 `crates/`는 같다.
- **REPORT §2.4.** "25개 중 24개를 한 번의 실행에서 검출(C15~C20 추가)"과 "208건"은 증거 파일과 맞다.

## 실행한 명령

```
gh pr view 37 --json headRefOid,mergeStateStatus   # 784d60e…, BLOCKED(CI pending) → 끝날 때 CLEAN
gh pr checks 37; gh pr diff 37; git diff 0086b0d 784d60e; git log 0086b0d..784d60e
git diff --stat 5277e6c 784d60e -- crates contracts   # 비어 있음
git fetch -q origin; git merge-tree --write-tree origin/main 784d60e   # 충돌 없음(main = 0086b0d)
git worktree add --detach <scratch>/wt37 784d60efae799718cd8615fd03a1228efe50c950
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-r37
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
# 검토용 임시 시험 zz_probe_e(저장소에 넣지 않음; <scratch>/r37-probes.rs)
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
git checkout -- crates                               # 임시 시험 되돌림, status 깨끗
cargo fmt --all -- --check                           # 0
cargo test -q --locked --all-targets                 # 0, 208 passed, 0 failed, 0 ignored
python3.11 -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 24/25, C4a 생존, dirty none
```

정리:
- worktree `wt37`과 target `target-r37`은 검토 뒤 삭제했다.
- 시험 DB는 하네스가 만든 임시 DB만 썼다.
- 비밀번호는 출력하거나 기록하지 않았다.
- 영속 DB, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다(이 검토 파일만 공용 git 디렉터리의 pending-reviews에 썼다).

<!-- 보관 주: 88행 인용 속 링크의 대상이 "…"(생략 부호)라 패키지 검증기가 거부한다. 그래서 링크 괄호를 "(링크 대상 생략)"으로 바꿨다. 그 밖에는 원본과 같다. -->
