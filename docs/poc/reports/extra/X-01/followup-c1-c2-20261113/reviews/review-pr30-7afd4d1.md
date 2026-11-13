# PR #30 독립 재검토 (Guardrail) · head 7afd4d1 · base main 82c7db9

VERDICT: reject

## 요약

- **대상.** `fix/x01-c1-c2-notion-gate`의 head는 `7afd4d1b3ac13acef7ae41f07e5393189a0996fb`이다. `git ls-remote`로 원격 head와 같음을 확인했다. base는 main `82c7db9`이고, 교정분은 `git diff 9a421b7 7afd4d1`이다.
- **검사 재실행.** 다음을 모두 통과했다.
  - fmt, clippy `-D warnings`
  - `cargo test --locked --all-targets` 175건(시험 DB 연결 확인)
  - mutants.py 11개: 10개 검출, C4 생존(보고서와 같음)
  - validate_package(schema_errors 0), render_progress `--check`, test_render_progress, openapi-spec-validator
- **F1·F2·F4·F5·F6은 해소됐다.** 아래 시나리오는 검토용 임시 시험에서 모두 기대대로 동작했다.
  - 같은 출처를 그대로 인용한 다른 sprint_id 사본은 409이다.
  - A→B→A 되돌림은 새 revision이 되고 409이다.
  - 예외는 승인한 내용 digest에만 적용된다.
  - 해제 뒤 만든 사본은 201이고, 다시 차단되면 다음 세대가 409이다.
  - draft를 executable로 올릴 때 예외가 없으면 409, 있으면 201·201·201이다.
- **새 판정도 우회할 수 있다(G1, P2).** `notion_gate`는 `kind == "notion_page"`이고 digest가 정확히 같은 출처만 본다. 그런데 `kind`는 스키마상 자유 문자열이다. 따라서 Notion `ref`를 그대로 두고 다음 중 하나만 바꾼 사본은 executable 등록·의미 검토·배정이 모두 201이 된다.
  - `kind`를 `"notion"`으로 바꾼다(ref·digest는 그대로).
  - digest만 다른 값으로 바꾼다.
- **보고서가 서버 강제 범위를 다시 넓게 적었다.** §2.3은 "같은 Notion 출처를 인용한 다른 스프린트"를 **서버** 강제 범위로 적었다. 또 잔여 범위는 "출처를 **빼고**" 등록하는 경우뿐이라고 적었다. 위 우회는 출처를 빼지 않으므로 이 표는 정확하지 않다. F3이 지적한 과장이 다른 형태로 남았다.
- **그 밖의 발견(P3).**
  - C4 "동등 변이" 주장에는 반례가 있다(G2).
  - 새 규칙 "한 페이지는 하나의 test_id"를 검사하는 시험이 없다(G3).
  - 이 규칙 이전에 만든 DB에 한 페이지가 두 test_id로 가져와져 있으면 이관이 실패한다(G4).
- P2가 있으므로 reject한다.

## 이전 지적 F1–F6 재판정

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| F1 P1: 다른 sprint_id 사본 실행 | **해소**(지적한 그대로의 경로). 인접 변형은 **G1로 잔존** | 출처 `kind`·`ref`·digest를 그대로 둔 `POCX-Z02` 사본은 executable 409 `IMPORT_GATED`이고 행 수가 변하지 않았다. T01 예외가 있어도 409이다(기존 시험 + 검토 시험 대조군). C9 변이를 검출했다. 해제 뒤 사본은 201이고, 다시 차단되면 그 사본의 다음 세대 배정이 409이다(검토 시험) |
| F2 P2: A→B→A replay | **해소** | replay는 페이지의 최신 가져오기일 때만 일어난다(`api.rs:817-821`). A→B→A는 revision 3이 되고 409이다. C10 변이를 검출했다. 이관 뒤 기본 키는 (page_id, sprint_revision)이다 |
| F3 P2: 보고서 과장 | **부분 해소 → G1** | §2.3 강제 주체 표를 추가했다. scope는 기록만 하고 보관 분리는 검증하지 않았다고 정직하게 적었다. 그러나 "같은 Notion 출처를 인용한 다른 스프린트 = 서버"라는 행은 G1 때문에 성립하지 않는다 |
| F4 P3: 예외가 이후의 다른 차단에도 적용 | **해소** | 예외에 `content_digest`를 두고, 승인 시점 최신 가져오기의 digest를 기록한다. 차단 내용이 바뀐 가져오기는 409이고 새로 승인한 뒤에 201이다. C11 변이를 검출했다. scope를 서버가 강제하지 않는다는 점은 §2.3과 OpenAPI에 적혀 있다 |
| F5 P3: issueAssignment 설명 | **해소** | `IMPORT_INCOMPLETE`/`IMPORT_GATED`와, 고정된 진행(pinned continuation)도 멈춘다는 점이 적혔다. 다만 문장 중간에 절이 끼어 읽기 어렵다(`openapi.yaml:503-510`, 참고 사항) |
| F6 P3: 범위 규칙 시험, 대시 ID | **해소** | `credentials_refuse_inconsistent_principals`에 user의 `notion:import`, service의 `exceptions:approve`를 거부하는 경우가 추가됐다. 도구는 URL·대시 UUID에서 32-hex를 뽑고 tool_use 입력도 대시 제거·소문자로 비교한다 |

## 새 발견

### G1 · P2 · Notion 출처를 유지한 채 `kind` 또는 digest만 바꾼 사본은 서버 판정을 통과함 (§2.3 강제 범위 과장)

- **위치.**
  - `crates/ieo-coord/src/api.rs:1054-1069`(`notion_gate`): `.filter(|x| x["kind"] == "notion_page")` 뒤 `content_digest = ANY($2)`
  - `docs/poc/SprintSpec_v0.schema.json`: `sources[].kind`는 `minLength: 1`인 자유 문자열이다
  - `REPORT.md:39`, `:42`(§2.3), `contracts/ieo-api-v0.openapi.yaml:53-55`
- **재현.** 검토용 임시 시험이며 head 7afd4d1과 시험 DB에서 돌렸고, 끝난 뒤 되돌렸다.
  1. `importer`가 `POCX-T01`을 가져온다(digest `a…`, `queue_gates()` 2건, missing 없음).
  2. (a) `derived_executable(POCX-T01)`을 바탕으로 `sprint_id=POCX-Z02`, `revision=1`, 작업 ID `13-01`인 사본을 만든다. `notion_page` 출처의 `kind`만 `"notion"`으로 바꾼다. `ref`(`https://app.notion.com/p/0123…`)와 digest(`a…`)는 그대로 두고, spec digest를 다시 계산한다.
  3. 결과: `alice` executable **201**, `bob` 의미 검토 **201**, `assigner` 배정 **201**.
  4. (b) 새 환경에서 `kind`는 `notion_page`, `ref`는 그대로 두고 digest만 `9…`로 바꾼다(`POCX-Z03`, 작업 `14-01`). 결과는 **201 / 201 / 201**이다.
  5. 대조군: 출처를 그대로 둔 사본(`POCX-Z04`)은 409 `IMPORT_GATED`이다.
- **기대.**
  - 이슈 B는 등록자가 고칠 수 있는 내용만으로 실행 허가가 나지 않게 하라고 요구한다. 서버는 가져오기 기록에 `url`(=`ref`)·`page_id`·`content_digest`를 갖고 있다. 그러므로 다음 둘 중 하나는 차단해야 한다.
    - 차단된 페이지의 ref나 이미 가져온 digest를 출처로 인용한 명세. 이때 `kind`와 무관해야 한다.
    - 또는 `kind`가 Notion ref를 가리키면서 digest가 가져오기 기록과 맞지 않는 명세.
  - 그렇게 하지 않는다면 보고서 §2.3과 OpenAPI가 "출처를 뺀 경우"만이 아니라 다음 경우도 등록자와 의미 검토에 남는다고 밝혀야 한다.
    - `kind` 문자열이 `notion_page`가 아닌 경우
    - digest가 가져오기 기록과 다른 경우
- **실제.**
  - 출처를 빼지 않고 필드 하나만 바꿔도 통제를 우회한다.
  - 의미 검토자는 Notion 출처가 붙은 명세를 보게 된다. 그래서 서버가 이미 판정했다고 오인할 수 있다.
  - §2.3 1행("같은 Notion 출처를 인용한 다른 스프린트 → 서버")과 잔여 범위 행("출처를 **빼고**")은 이 경우를 잘못 분류한다.
  - 이 경로를 다루는 시험이나 변이가 없다.

### G2 · P3 · C4 "동등 변이" 주장에 반례가 있음

- **위치.** `api.rs:1109-1115`, `evidence/mutants.txt:14`, `REPORT.md:57`.
- **원인.** 보고서는 "다른 시험의 예외는 일치할 수 없다"고 주장한다. 그러나 `content_digest`는 속성과 본문만으로 만든다(`crates/ieo-notion/src/lib.rs:121`, page_id 미포함). 서버도 서로 다른 페이지가 같은 digest를 갖는 것을 막지 않는다. 그래서 다른 test_id의 예외가 같은 digest를 가질 수 있다.
- **반례 재현(검토 시험).**
  1. 페이지 P를 `POCX-T01`로 가져온다(digest `a…`, 차단).
  2. 페이지 Q(`fedcba…`)를 `POCX-T02`로 가져온다(같은 digest `a…`, 차단). 결과는 201이다.
  3. `uma`가 `EX-T02`를 기록한다. 응답의 `approved_content_digest`는 `a…`이다.
  4. Q를 차단 없는 내용 `b…`로 다시 가져온다(해제). 결과는 201이다.
  5. T01의 파생 executable을 등록한다.
     - 원래 코드: **409 `IMPORT_GATED`**
     - C4 변이(`(test_id = $1 OR true)`): **201**, 의미 검토 201, 배정 201
- **결론.** C4는 동등 변이가 아니다. SQL의 `test_id` 조건은 이중 방어가 아니라 실제로 필요한 조건이다. 이 경우를 다루는 시험을 추가하고 보고서의 동등 판정을 고쳐야 한다.
- **같은 원인의 과차단.** 위 1~3 단계 직후(Q를 아직 해제하기 전)에는 T02가 자기 예외 `EX-T02`를 갖고도 409 `IMPORT_GATED`이다. T02 명세의 digest `a…`가 페이지 P를 함께 끌어오기 때문이다. digest가 같은 서로 다른 페이지는 드문 경우이므로 P3로 둔다.

### G3 · P3 · 새 가져오기 규칙 "한 페이지는 하나의 test_id"를 검사하는 시험·변이가 없음

- **위치.** `api.rs:854-866`.
- **재현.** `WHERE page_id = $1 AND test_id <> $2 LIMIT 1`을 `… AND false LIMIT 1`로 바꾸고 `cargo test --locked --all-targets`를 돌렸다. 결과는 **rc 0, 모두 통과**(변이 생존)였다.
- **동작 자체는 정상이다.** 검토 시험에서 같은 페이지를 `POCX-T09`로 가져오면 409 `EVENT_CONTENT_CONFLICT`이다.
- **영향.** 이 규칙은 판정의 전제다. 아래 두 가지가 "페이지별 최신"을 `sprint_revision` 순서로 고르는데, 이 순서는 한 test_id 안에서만 의미가 있다. 그런데도 회귀를 잡는 시험이 없다.
  - F2 replay 판정의 `max(sprint_revision)`
  - `notion_gate`의 `ORDER BY sprint_revision DESC`
- **참고.** 이 규칙 때문에 같은 Notion 작업을 새 test_id로 다시 시험할 수 없다. OpenAPI에는 "A page belongs to one test_id"로 적혀 있다. 보고서 §2.2·§3에는 이 규칙이 언급되지 않는다.

### G4 · P3 · 이전 DB에 한 페이지가 두 test_id로 가져와져 있으면 이관이 실패함

- **위치.** `crates/ieo-coord/src/schema.sql:222-228`(DO 블록).
- **원인.** main 82c7db9와 9a421b7은 digest가 다르면 같은 페이지를 다른 test_id로 가져오는 것을 허용했다. 이 경우 두 행 모두 `sprint_revision`이 1이 될 수 있다.
- **재현(검토 시험).** main 스키마와 9a421b7 스키마로 각각 DB를 만들고 아래 행을 넣은 뒤 `migrate`를 두 번 돌렸다.
  - 넣은 행: `(P,a,T01,1)`, `(P,b,T01,2)`, `(P,c,T02,1)`
  - 결과: 두 번 모두 `could not create unique index "notion_imports_pkey"`로 실패했다.
  - 실패 뒤에도 기본 키는 원래의 `(page_id, content_digest)`로 남았다. `batch_execute`가 하나의 암묵 트랜잭션이라 DROP도 롤백되기 때문이다.
  - 충돌이 없는 데이터에서는 두 스키마 모두 성공했고, 다시 돌려도 아무것도 바뀌지 않았다.
- **영향.** 데이터는 손상되지 않고 닫힌 실패이다. 하지만 그런 DB에서는 서버가 기동하지 않는다. 영속 DB는 OC08 하나뿐이라 이미 이관됐다(`c2-live-r2.txt`).
- **필요한 조치.** 다음 중 하나가 필요하다.
  - 이관 전에 충돌을 검사해 명확한 메시지를 내는 것
  - 이 전제를 보고서에 적는 것

## 확인했고 문제없는 항목

- **판정 위치(register).**
  - `guard_import_owned` 다음, `register_spec` 이전에 판정한다. 형식 검사가 통과할 때만 판정하므로 형식 오류는 400 `SCHEMA_VIOLATION`을 유지한다(검토 시험).
  - idempotent replay는 판정 전에 200을 돌려주지만 상태를 바꾸지 않는다.
  - 판정이 `register_spec` 안의 409/422(`TASK_ID_CONFLICT`, `REVISION_SUPERSEDED`, 바인딩 사유 등)보다 먼저 409 `IMPORT_GATED`를 낸다. OpenAPI는 우선순위를 정하지 않으므로 불일치는 아니다.
- **판정 위치(assign).**
  - `UNKNOWN_SPRINT_REVISION`/`REPO_MISMATCH`/`REVISION_SUPERSEDED`/`UNBOUND_DRAFT`/`SEMANTIC_REVIEW_NOT_PASSED` 다음, `UNKNOWN_TASK` 이전에 판정한다. 대상은 배정하는 revision의 저장된 spec이다.
  - 고정된 진행도 판정한다(`c2_a_re_gating_…`, 검토 시험의 사본 다음 세대도 409).
  - 상태 코드와 문형은 OpenAPI 0.8.0과 일치한다. `every_emitted_code_is_declared_in_the_openapi_contract`가 통과한다.
- **정당한 흐름(과차단 없음).**
  - 해제된 가져오기 뒤의 사본은 201·201·201이다.
  - draft(ready 내용, mode=draft)를 executable로 올릴 때 예외가 없으면 409, 예외가 있으면 201·201·201이다.
  - 출처가 없는 일반 스프린트(human_input digest `000…`)는 모든 기존 시험이 통과한다. 이런 스프린트는 관련 없는 digest와 일치할 가져오기 행이 없다.
  - 예외는 `test_id == sprint_id`이고 repo와 digest가 같을 때만 적용된다. 다른 repo의 예외는 403이다.
- **이관.**
  - 새 DB의 기본 키는 `(page_id, sprint_revision)`이고, `exec_exceptions`에 `content_digest`가 있다. `migrate`를 다시 돌려도 아무것도 바뀌지 않는다.
  - main·9a421b7 스키마에서 이관하면 기본 키가 바뀌고, 두 번째 실행은 아무것도 바꾸지 않는다.
  - `content_digest`가 NULL인 이전 예외는 `= $3`과 일치하지 않는다(닫힌 실패).
- **변이.** C1, C2, C3, C5~C11은 검출됐고, 실행 뒤 작업 트리는 깨끗했다. C4는 생존했다(G2).
- **보고서 §2.5(live).** `c2-live.txt`, `c2-live-r2.txt`와 X-01 증거를 대조했다.
  - rev1 digest는 `3f8a300f…`(실제 내용), rev2는 `20181522…`이다. rev2는 X-01 R3의 "모의 변경"이다(X-01 REPORT:49).
  - 실제 내용을 다시 가져오면 A→B→A 규칙에 따라 rev3이 되고, gates 3·missing 0이 저장된다. 이는 X-01 rev1의 미해결 5건(차단 3건, 한도·바인딩 2건)과 맞는다.
  - 예외 3건과 각 digest(NULL, `20181522…`, `3f8a300f…`)도 서술과 같다. "앞의 두 예외는 효력이 없다"는 서술도 정확하다.
  - 재확인을 실행한 커밋은 `9cb0e3a`이다. 이 커밋과 head의 서버 코드 차이는 예외 응답과 이력에 `approved_content_digest`를 추가한 것뿐이다. 판정 동작은 같다. 영속 서버가 head로 배포됐다는 주장은 하지 않으므로 과장이 아니다.
  - 영속 DB 자체는 규칙에 따라 열람하지 않았다.
- **C1 도구.** URL·대시 UUID 정규화는 정확하다. 출력 권한(0600/0700)은 9a421b7에서 바뀌지 않았다.

## 실행한 명령

```
df -h /                                                    # 2.1G → 작업 중 3.4~4.4G(다른 세션의 정리)
git ls-remote origin refs/heads/fix/x01-c1-c2-notion-gate refs/heads/main   # 7afd4d1…, 82c7db9…
git worktree add --detach <scratch>/wt30 7afd4d1b3ac13acef7ae41f07e5393189a0996fb
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/wt30-target CARGO_PROFILE_DEV_DEBUG=0
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                                 # 0
cargo clippy -q --locked --all-targets -- -D warnings      # 0
cargo test --locked --all-targets                          # 0, 175 passed
.venv-poc/bin/python -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 10/11 killed, C4 survived, dirty none
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py      # rc 0, schema_errors 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -B docs/poc/scripts/test_render_progress.py      # RESULT PASS 0
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
# 검토용 임시 시험(zz_probe*, 저장소에 넣지 않음): G1 kind/digest 변형, C4 반례(원래/변이), 해제 뒤 사본·재차단,
#   draft→executable, 형식 오류 순서, 페이지-test_id 규칙, main·9a421b7 스키마 → migrate ×2(충돌 유무), 새 DB
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
# 추가 변이: api.rs:855 "AND test_id <> $2 AND false" → cargo test --all-targets rc 0(생존, G3)
```

정리:
- 임시 시험과 변이는 `git checkout`으로 되돌렸다.
- 직접 만든 이관 DB(`ieo_mig_r30_*`)는 삭제했다.
- worktree, target, 임시 파일은 삭제했다.
- 시험 PG에 `ieo_t_3396381_*` 3개와 `ieo_t_3399090_*` 4개가 남아 있다. 두 PID는 이미 종료됐다. 하지만 이 검토가 만든 것인지 확인할 수 없어 건드리지 않았다.
- 비밀번호와 토큰은 어디에도 기록하지 않았다.
- 영속 DB `ieo-poc-db`, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다.
