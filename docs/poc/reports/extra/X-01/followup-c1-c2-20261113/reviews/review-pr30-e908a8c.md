VERDICT: reject

# PR #30 독립 재검토 3 (Guardrail) · head e908a8c · base main 82c7db9

## 요약

- **대상.** `fix/x01-c1-c2-notion-gate`의 head는 `e908a8cde285c18f6cd187ed10eae334e2b4444c`이다. `gh pr view 30 --json headRefOid`로 원격 head와 같음을 확인했다. base는 main `82c7db9`이다. 이번 교정분은 `git diff 7afd4d1 e908a8c`이다. 서버 코드는 `8d76ee7` 이후 바뀌지 않았다(`git diff --stat 8d76ee7 e908a8c -- crates` 비어 있음).
- **검사 재실행.** 다음을 모두 통과했다.
  - fmt, clippy `-D warnings`
  - `cargo test --locked --all-targets` 179건(시험 DB 연결 확인, 건너뜀 0)
  - mutants.py 15개: 13개 검출, C4a·C4b 생존(보고서와 같음)
  - validate_package(schema_errors 0), render_progress `--check`, test_render_progress, openapi-spec-validator
- **G1–G4는 지적한 그대로의 경로에서 해소됐다.** F1–F6도 회귀가 없다(아래 표).
- **새 판정에도 우회가 있다(H1, P2).** 출처의 페이지 참조를 **다른 가져온 페이지**(해제된 페이지)의 ID로 바꾸고 차단된 내용 digest는 그대로 둔 사본은 executable 등록·의미 검토·배정이 모두 201이다. 이 출처는 "가져온 페이지를 가리키는" 출처이므로 digest 대체 판정을 건너뛴다. 보고서 §2.3과 OpenAPI 요약 문장은 "참조와 digest를 **둘 다** 바꾼 사본"만 서버가 알아보지 못한다고 적었으므로, 서버 강제 범위를 다시 넓게 적었다.
- **그 밖의 발견(P3).**
  - "페이지의 최신 가져오기"를 `imported_at`(=트랜잭션 시작 시각) 순으로 고른다. 그런데 revision 순서는 registry 잠금 순서로 정해지고, 예외 기록은 `sprint_revision` 순으로 고른다. 동시 가져오기에서 둘이 어긋날 수 있다(H2).
  - C4b "겹치는 이중 방어(동등)" 주장에 반례가 있다. 페이지가 NULL인 이전 예외 행은 C4b에서 일치한다. "NULL은 일치하지 않는다"는 주장을 검사하는 시험이 없다(H3).
- P2가 있으므로 reject한다.

## 이전 지적 재판정

### F1–F6(검토 9a421b7)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| F1 P1: 다른 sprint_id 사본 실행 | **해소 유지**. 인접 변형은 **H1** | `c2_a_copy_…` 통과. 페이지 참조나 digest가 남은 사본은 409이다(`g1_…`, 검토 시험 대조군). C9·C9b 변이 검출 |
| F2 P2: A→B→A replay | **해소 유지** | replay 질의가 "페이지의 최신 가져오기"를 `(test_id, sprint_revision)`과 `imported_at` 순으로 고른다. 판정 질의와 같은 정렬이다. C10 변이 검출. 정렬의 동시성 문제는 H2 |
| F3 P2: 보고서 과장 | **부분 해소 → H1** | G1 경로에 대해서는 정확해졌다. H1 경로 때문에 §2.3 서버 행과 잔여 범위 행이 다시 맞지 않는다 |
| F4 P3: 예외가 이후 다른 차단에도 적용 | **해소 유지** | 예외는 (시험 ID, repo, 페이지, 내용 digest)에 묶인다. C11 변이 검출 |
| F5 P3: issueAssignment 설명 | **해소 유지** | OpenAPI 503-516 문장을 둘로 나눠 읽기 쉬워졌다. 내용은 서버 동작과 같다 |
| F6 P3: 범위 규칙 시험, 대시 ID | **해소 유지** | `credentials_refuse_inconsistent_principals` 통과. 도구 쪽 변경 없음 |

### G1–G4(검토 7afd4d1)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| G1 P2: `kind`나 digest만 바꾼 사본 통과 | **지적한 경로는 해소**. 인접 변형 **H1** | `page_ids_in`이 출처의 모든 문자열에서 대시를 지우고 32-hex 창을 모두 뽑아, 가져온 페이지 ID와 맞춘다. `kind`를 `notion`으로 바꾼 사본, digest만 바꾼 사본, 다른 필드에 대시 ID만 남긴 사본, 참조를 빼고 digest만 남긴 사본은 모두 409이다(`g1_…`). 대문자 대시 ID와 제목이 붙은 slug URL(`…/Fade-Bad-Task-<id>?pvs=4`)도 409이다(검토 시험) |
| G2 P3: C4 동등 판정 반례, 같은 내용의 다른 페이지 과차단 | **해소**. 관련 증거 문제 **H3** | `exec_exceptions.page_id`를 추가하고 승인 시점 최신 가져오기의 페이지를 기록한다. 페이지 ID가 있는 출처는 digest로 다른 페이지를 끌어오지 않는다. T02는 자기 예외로 201, T01은 계속 409이다(`g2_…`). C4 변이 검출 |
| G3 P3: "한 페이지는 한 test_id" 시험 없음 | **해소** | `g3_…`(같은 내용·다른 내용 모두 409, 행 수 불변). C12 변이 검출. REPORT §2.2에 규칙을 적었다 |
| G4 P3: 두 test_id로 가져온 이전 데이터에서 이관 실패 | **해소** | 기본 키를 `(test_id, sprint_revision)`으로 바꿨다. 가져오기 한 건이 revision 하나를 만들고, main 이후 모든 스키마에서 같은 (test_id, revision)을 가리키는 가져오기 행은 둘일 수 없다. 그래서 이전 데이터와 충돌하지 않는다. DO 블록은 다음 상태를 모두 처리한다: `content_digest`가 든 키(main, 9a421b7), `page_id`가 든 키(7afd4d1), 키 없음(추가), 이미 새 키(변경 없음). `g4_…`가 main 키와 7afd4d1 키에서의 이관과 재실행을 검사한다 |

## 새 발견

### H1 · P2 · 페이지 참조를 해제된 다른 가져온 페이지로 바꾸고 차단된 digest를 그대로 둔 사본이 서버 판정을 통과함 (§2.3·OpenAPI 강제 범위 과장)

- **위치.**
  - `crates/ieo-coord/src/api.rs:1090-1117`(`notion_gate`): `named`가 비어 있을 때만 `content_digest = $1` 대체 판정을 한다.
  - `api.rs:1066-1075`(같은 함수의 문서 주석) "Only a copy that drops or rewrites both the page reference and the digest is not recognised"
  - `contracts/ieo-api-v0.openapi.yaml:53-57`(56행) "A copy that drops or rewrites both the page reference and the digest cannot be recognised"
  - `REPORT.md` §2.3 서버 행("가져온 내용 digest가 남은 다른 스프린트") · 잔여 범위 행("페이지 참조와 digest를 **둘 다** 바꾸고")
- **재현.** 검토용 임시 시험이며 head e908a8c와 시험 DB에서 돌렸고, 끝난 뒤 되돌렸다.
  1. `importer`가 `POCX-T01`을 페이지 P(`0123…`)로 가져온다. digest `a…`, `queue_gates()` 2건, missing 없음.
  2. 대조군: `copy_of_t01(POCX-Z05)`에서 `notion_page` 출처의 `ref`만 `https://app.notion.com/p/fedcba9876543210fedcba9876543210`(Q, 아직 가져오지 않음)으로 바꾼다. 결과는 **409 `IMPORT_GATED`**이다(digest 대체 판정이 P를 찾음).
  3. `importer`가 `POCX-T02`를 페이지 Q로 가져온다. digest `c…`, gates 없음(해제 상태). 결과는 201이다.
  4. 2와 같은 사본(`POCX-Z06`, 작업 `13-01`)을 만든다. `kind`는 `notion_page`, digest는 `a…` 그대로이고 `ref`만 Q를 가리킨다.
  5. 결과: `alice` executable **201**, `bob` 의미 검토 **201**, `assigner` 배정 **201**.
  6. 같은 시점에 원래 `POCX-T01`의 파생 executable은 409 `IMPORT_GATED`이다.
- **원인.** 출처가 가져온 페이지 하나라도 가리키면 그 출처의 digest는 판정에 쓰이지 않는다. 그래서 이 출처는 "Q의 내용"이 아니라 "P의 차단된 내용"을 인용하는데도 Q의 최신 행(해제)만 보고 통과한다. 이 경로를 다루는 시험이나 변이가 없다.
- **조건.** 같은 코디네이터에 gates·missing이 없는 최신 가져오기를 가진 페이지가 하나라도 있으면 된다. 페이지 목록 질의는 repo로 거르지 않으므로 다른 repo의 페이지도 된다. 작업 하나가 해제되면 바로 생기는 상태다.
- **기대.**
  - 이슈 B는 등록자가 고칠 수 있는 필드 하나로 실행 허가가 나지 않게 하라고 요구한다. 서버는 출처의 digest가 가리키는 가져오기 행을 알고 있다.
  - 예: 출처 digest와 같은 내용 digest를 가진 가져온 페이지 중 **그 출처가 가리키는 페이지가 없으면**, 그 페이지들도 판정 대상에 넣는다. G2 경우(Q의 자기 출처가 Q를 가리키고 digest가 P와 같음)는 Q가 그 digest를 가지므로 P를 끌어오지 않는다. 이렇게 하면 G2의 과차단을 되살리지 않고 H1을 막을 수 있다.
  - 그렇게 하지 않는다면 §2.3·OpenAPI·주석에 "참조를 가져온 다른 페이지로 바꾼 사본은 digest가 남아도 알아보지 못한다"고 밝혀야 한다.
- **실제.**
  - 출처를 빼지 않고 `ref` 하나만 바꿔도 통제를 우회한다.
  - 의미 검토자는 실제로 존재하는 Notion 페이지(Q)를 가리키는 출처를 보게 된다. 그래서 서버가 이미 판정했다고 오인할 수 있다.
  - §2.3 서버 행("가져온 내용 digest가 남은 다른 스프린트")과 잔여 범위 행("둘 다 바꾸고")은 이 경우를 잘못 분류한다. OpenAPI 첫 문장("for a source naming no imported page")은 정확하지만, 같은 단락의 마지막 문장이 이를 넓게 요약한다.

### H2 · P3 · "최신 가져오기" 정렬이 트랜잭션 시작 시각을 쓰고, 예외 기록과 다른 기준을 씀

- **위치.**
  - `schema.sql:212` `imported_at timestamptz NOT NULL DEFAULT now()`: PostgreSQL의 `now()`는 트랜잭션 **시작** 시각이다.
  - `api.rs:816`: `lock(tx, "registry")`는 트랜잭션 시작 뒤에 잡는다. 그 앞에 본문 파싱과 검사가 있다. revision 번호는 이 잠금 순서로 정해진다.
  - `api.rs:820-822`(replay), `api.rs:1122`(판정): `ORDER BY imported_at DESC, sprint_revision DESC`
  - `api.rs:1285`(`record_exception`), `api.rs:1008`(`guard_import_owned`): `ORDER BY n.sprint_revision DESC`
- **원인.** 같은 페이지의 가져오기 두 건이 겹치면, 먼저 시작했지만 잠금을 늦게 잡은 트랜잭션이 더 큰 revision과 더 이른 `imported_at`을 갖는다. 이때 다음 셋이 서로 다른 행을 "최신"으로 본다.
  - 판정: `imported_at`이 큰 행
  - 스프린트의 최신 revision: `sprint_revision`이 큰 행
  - 예외 승인과 import 소유 스프린트의 파생 명세 검사(`guard_import_owned`): `sprint_revision`이 큰 행
- **결과 재현(검토 시험).** 동시 실행 자체는 결정적으로 재현하기 어렵다. 그래서 그 결과 상태를 직접 만들었다.
  1. T01을 a(gates)→b(해제)→c(gates) 순으로 가져온다. revision 1·2·3이 된다.
  2. 트리거를 잠시 끄고 rev3의 `imported_at`을 rev2보다 1ms 이르게 바꾼다. 겹친 가져오기가 남기는 상태다.
  3. rev3(차단 내용 c)에서 파생한 executable rev4: **201 / 201 / 201**. 판정이 rev2(해제)를 최신으로 본다.
  4. 이 rev4는 `guard_import_owned`가 rev3 기준으로 받아들이고, 판정은 rev2 기준으로 통과시킨다. 같은 상태에서 예외를 승인하면 `approved_content_digest`는 `c…`(rev3)이다. 판정이 보는 행과 다르다.
- **영향.** 가져오기 서비스는 service 전용이고, 같은 페이지를 동시에 두 번 가져오는 일은 드물다. 하지만 HTTP 앞단은 작업자 스레드마다 연결을 따로 쓰므로(`http.rs:1`) 동시 실행은 가능하다. 시계가 되돌아가는 경우에도 같은 일이 생긴다. 보고서 §3 G4 행은 "'페이지의 최신 가져오기'는 `imported_at` 순으로 정한다"고 적었지만, 이 순서가 revision 순서와 같다는 보장은 없다.
- **필요한 조치.** 다음 중 하나가 필요하다.
  - 한 페이지는 한 시험 ID에 속하므로 페이지 안에서는 `sprint_revision`으로 정렬한다. 시험 ID가 둘인 이전 데이터만 시각으로 나눈다.
  - 또는 잠금 뒤에 `clock_timestamp()`로 `imported_at`을 기록한다.
  - 어느 쪽이든 replay·판정·예외 승인이 같은 정렬을 쓰게 한다.

### H3 · P3 · C4b "겹치는 이중 방어(동등)" 주장에 반례가 있음: 페이지가 NULL인 이전 예외

- **위치.** `api.rs:1160-1165`, `evidence/mutants.txt:22`(결과 주석), `REPORT.md` §2.4 마지막 단락, `schema.sql:253`("NULL never matches"), `REPORT.md` §2.5.
- **원인.** 보고서는 C4a·C4b가 "한 조건만 빼면 결과가 같다"고 주장한다. C4a(시험 ID 조건 제거)는 맞다. 다른 시험의 예외가 같은 페이지·같은 digest에 묶일 수 없다. 이는 main 이후 모든 스키마에서 성립한다. 하지만 C4b(페이지 조건 제거)는 교정 2 이전에 기록된 예외 행, 즉 `page_id`가 NULL인 행과 일치하게 만든다. §2.5는 OC08 영속 DB의 기존 예외가 "닫힌 실패로 효력을 잃는다"고 적었다. 이 주장은 바로 `page_id = $3` 조건에 기대고 있다.
- **반례 재현(검토 시험).**
  1. T01을 가져온다(digest `a…`, gates).
  2. `exec_exceptions`에 `page_id` 없이 `(EX-legacy, POCX-T01, repo-a, queue_gates, content_digest=a…)`를 넣는다. 이관 전 행과 같은 형태다.
  3. T01 파생 executable:
     - 원래 코드: **409 `IMPORT_GATED`**
     - C4b 변이(`(page_id = $3 OR true)`): **201**, 의미 검토 201, 배정 201
- **결론.** C4b는 동등 변이가 아니다. "페이지가 NULL인 예외는 일치하지 않는다"를 검사하는 시험을 추가하고, mutants.txt와 §2.4의 판정을 고쳐야 한다. 같은 형태의 "`content_digest` NULL은 일치하지 않는다"(검토 9a421b7 F4) 주장도 시험이 없다면 함께 다루는 것이 좋다.

## 확인했고 문제없는 항목

- **페이지 ID 인식(`page_ids_in`).**
  - 대시를 지운 뒤 hex 구간마다 32자 창을 모두 뽑는다. 그래서 slug URL 안의 ID도 찾는다. 제목 글자가 hex여서 구간이 길어지는 경우도 포함한다. 대문자도 찾는다.
  - 다음 표기는 찾지 못한다(검토 시험, digest도 바꾼 사본 → 201): `%2D` 인코딩, en dash(U+2013), 공백 구분, 밑줄 구분. 객체 키는 보지 않지만 출처 스키마가 `additionalProperties: false`이고 값이 모두 문자열이라 해당하지 않는다.
  - OpenAPI는 "dashed or plain"으로 적었고 REPORT §2.2도 "대시 표기와 URL 안의 ID"로 적었으므로 범위 서술은 정확하다. 이 표기들은 §2.3의 "참조를 바꾼" 잔여 범위에 들어간다. digest를 그대로 두면 대체 판정이 409로 잡는다.
- **과차단.**
  - 과포함 창은 가져온 페이지 ID와 같을 때만 효과가 있다. 64-hex digest와 40-hex SHA의 창이 우연히 같을 가능성은 무시할 만하다.
  - 사람 입력 출처(`000…`)는 일치하는 가져오기 행이 없다. 기존 시험 179건이 모두 통과한다.
  - 해제 경로(`c2_a_trusted_release_…`)와 예외 경로(`c2_a_user_exception_…`, `g2_…`)가 201·201·201로 통과하므로 항상 거부하는 구현이 아니다.
  - 참고: 다른 스프린트의 출처가 차단된 페이지의 ID를 배경 자료로만 언급해도 409이고, 예외는 가져오기 자신의 시험 스프린트에만 적용되므로 풀 방법이 없다. 이는 F1 교정의 의도된 보수적 동작으로 보며 결함으로 세지 않는다.
- **digest 대체 규칙.** 가져온 페이지를 가리키지 않는 출처에 대해, 출처 digest와 같은 내용 digest의 모든 페이지를 대상으로 삼는다. `kind`와 무관하다(`g1_…` "reference dropped, digest kept", 검토 대조군). 결함은 가져온 페이지를 가리키는 출처에서 이 규칙을 건너뛰는 부분뿐이다(H1).
- **예외 바인딩.** 판정은 `test_id == sprint_id`, `repo_id`, `page_id`, `content_digest`를 모두 요구한다. 승인 응답과 이력에 `approved_page_id`가 남는다. 한 시험 ID는 한 페이지에만 속하므로(`api.rs:856-880`의 두 규칙) 승인 시점 최신 가져오기의 페이지가 곧 그 시험의 페이지이다.
- **이관.** 위 G4 행. 새 DB에서 기본 키는 `(test_id, sprint_revision)`이고 `exec_exceptions.page_id`가 생긴다. `migrate`를 다시 돌려도 아무것도 바뀌지 않는다(`g4_…`). 참고: 기본 키가 더 이상 `page_id`로 시작하지 않으므로 페이지별 질의(replay, 판정, 규칙 검사)는 색인 없이 훑는다. PoC 규모에서는 문제가 아니다.
- **동률.** 같은 트랜잭션에서 가져오기를 두 번 하는 경로는 없다. 그래서 `imported_at` 동률은 사실상 생기지 않고, 생겨도 replay와 판정이 같은 `ORDER BY`를 쓴다. 남는 문제는 정렬 기준 자체(H2)이다.
- **문서.**
  - §2.2 ①~③, §2.4 표, §3 G1–G4 행은 H1·H2·H3을 빼면 서버 동작과 같다.
  - OpenAPI `recordException`의 "page and content digest of that test sprint's latest import" 서술은 서버 동작과 같다(H2의 동시성 예외 제외).
  - §2.5는 교정 2가 영속 코디네이터에 아직 배포되지 않았고, 배포 뒤 기존 예외가 효력을 잃어 다시 기록해야 한다고 정직하게 적었다.
- **변이 증거.**
  - 직접 다시 돌린 결과가 `mutants.txt`와 같다: 15개 중 13개 검출, C4a·C4b 생존, 실행 뒤 작업 트리 깨끗함.
  - `mutants.txt` 머리의 커밋 `8d76ee7`과 head의 `crates/`는 같다.
  - 첫 실행에서 C9가 빌드 실패로 "killed"된 사실과, 컴파일되게 고친 뒤 a57ba9d에서 다시 돌린 사실이 기록돼 있다. 정직하다.
  - 동등 주장 중 C4a는 맞고, C4b는 H3의 반례가 있다.

## 실행한 명령

```
gh pr view 30 --json headRefOid,baseRefOid                # e908a8c…, 82c7db9…
git diff --stat 8d76ee7 e908a8c -- crates                 # 비어 있음
git worktree add --detach <scratch>/wt30c e908a8cde285c18f6cd187ed10eae334e2b4444c
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-c12
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                                 # 0
cargo clippy -q --locked --all-targets -- -D warnings      # 0
cargo test --locked --all-targets                          # 0, 179 passed, 0 failed
python3.11 -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 13/15 killed, C4a·C4b survived, dirty none
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py      # rc 0, schema_errors 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -B docs/poc/scripts/test_render_progress.py      # RESULT PASS 0
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
# 검토용 임시 시험(zz_probe_a~d, 저장소에 넣지 않음):
#   a: ref → 해제된 다른 가져온 페이지, digest 유지(H1) + 대조군(Q 미가져오기)
#   b: page_id NULL 예외 행(원래 코드 / C4b 변이, H3)
#   c: ID 표기 6종 + digest 변경
#   d: imported_at 역전 상태(H2)
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
```

정리:
- 임시 시험과 변이는 `git checkout`으로 되돌렸다. 처음 python3.6으로 mutants.py를 실행했다가 `capture_output` 오류로 C1 변이가 남은 것도 즉시 되돌렸다. 이후 `git status`는 깨끗했다.
- worktree는 검토 뒤 삭제했다. 공유 target(`target-c12`)은 남겼다.
- 시험 DB는 하네스가 만든 임시 DB만 썼다.
- 비밀번호와 토큰은 어디에도 기록하지 않았다.
- 영속 DB `ieo-poc-db`, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다.
