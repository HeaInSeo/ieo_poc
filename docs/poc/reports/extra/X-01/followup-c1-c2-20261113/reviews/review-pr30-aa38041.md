VERDICT: approve

# PR #30 독립 재검토 4 (Guardrail, 마지막) · head aa38041 · base main 3426c77

## 요약

- **대상.** `fix/x01-c1-c2-notion-gate`의 head는 `aa38041ef87bf41facfae235ccc1776d767a754f`이다. `gh pr view 30 --json mergeable,headRefOid`의 결과는 `MERGEABLE`이고, 원격 head도 이 커밋이다. 이번 교정분은 `git diff e908a8c aa38041`이다. 서버 코드는 `api.rs` 3곳과 `schema.sql` 주석만 바뀌었다. `mutants.txt` 머리의 커밋 `a33a8e2`와 head의 `crates/`는 같다(`git diff --stat a33a8e2 aa38041 -- crates`의 결과가 비어 있음).
- **main 이동.** main은 PR이 열린 뒤 #31·#32(`crates/ieo-supervisor`)로 `3426c77`까지 나아갔다. 충돌은 없다(`git merge-tree --write-tree`). 임시 worktree에서 병합한 상태로도 clippy `-D warnings`와 `cargo test --locked --all-targets`를 돌렸다. 202건이 통과했고 실패는 0이다. validate_package, render_progress `--check`, openapi-spec-validator도 통과했다.
- **검사 재실행(head).** 모두 통과했다.
  - fmt, clippy `-D warnings`
  - 시험 182건. 시험 DB에 연결했고 건너뛴 시험은 없다.
  - mutants.py 19개: 18개 검출, C4a 생존. 보고서와 같다.
  - validate_package(schema_errors 0), render_progress `--check`, test_render_progress, openapi-spec-validator
- **H1–H3는 지적한 경로에서 해소됐다.** F1–F6과 G1–G4도 회귀가 없다(아래 표). 허용 경로도 실제로 통과한다. 신뢰된 해제, 사용자 예외, 같은 내용을 가진 두 페이지(G2)는 모두 201·201·201이다. 따라서 항상 거부하는 구현이 아니다.
- **새 발견은 P3 셋이다.**
  - I1: 새 `carried` 판정은 가리킨 페이지의 **과거** 가져오기를 본다. 그래서 G2 상황(같은 내용을 가졌던 두 페이지)에서 다른 페이지가 해제된 뒤에는, 참조만 그 페이지로 바꾼 사본이 통과한다. 이때 문서의 "참조만 바꾼 사본도 판정" 서술이 이 경우에는 넓다.
  - I2: H2 시험은 실행 판정 쪽 정렬을 고정하지 못한다. `clock_timestamp()` 교정 자체를 검사하는 시험도 없다.
  - I3: 보고서 §2.4 문장 하나가 둘로 나뉘지 않고 이어져 있다.
- P2 이상이 없으므로 approve한다. I1은 한 줄짜리 교정이고 관련 시험은 아래에 적었다. 후속 작업으로 고치기를 권한다.

## 이전 지적 재판정

### F1–F6(검토 9a421b7)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| F1 P1: 다른 sprint_id 사본 실행 | **해소 유지**. 좁은 변형은 **I1** | `c2_a_copy_…`, `g1_…`, `h1_…` 통과. C1·C9·C9b·C13 변이 검출 |
| F2 P2: A→B→A replay | **해소 유지** | replay 질의는 판정과 같은 `ORDER BY imported_at DESC, sprint_revision DESC`를 쓴다. C10 변이 검출 |
| F3 P2: 보고서 과장 | **해소**. 좁은 예외 **I1** | §2.3 서버 행, 잔여 범위 행, OpenAPI 잔여 문장을 H1에 맞게 고쳤다. I1 경로에서만 서술이 넓다 |
| F4 P3: 예외가 이후 다른 차단에도 적용 | **해소 유지** | 예외는 (시험 ID, repo, 페이지, 내용 digest)에 묶인다. C11 변이 검출 |
| F5 P3: issueAssignment 설명 | **해소 유지** | 변경 없음 |
| F6 P3: 범위 규칙 시험, 대시 ID | **해소 유지** | 변경 없음. 시험 통과 |

### G1–G4(검토 7afd4d1)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| G1 P2: `kind`나 digest만 바꾼 사본 | **해소 유지** | `g1_…` 4개 변형 모두 409. C9 변이 검출 |
| G2 P3: 같은 내용의 다른 페이지 과차단 | **해소 유지** | `carried`가 참이면 digest로 다른 페이지를 끌어오지 않는다. `g2_…` 통과. 변이 C13b(항상 끌어오기) 검출. 검토 시험 b: P가 해제되고 Q는 같은 옛 내용으로 차단된 상태에서, P의 해제 파생은 201·201·201이고 Q의 자기 예외 경로는 201이다 |
| G3 P3: 한 페이지 한 test_id | **해소 유지** | `g3_…` 통과. C12 변이 검출 |
| G4 P3: 이관 | **해소 유지** | 이번 교정은 `schema.sql`의 주석만 바꿨다. DDL과 DO 블록은 그대로다. `imported_at`의 기본값 `now()`는 남았지만, 행을 넣는 경로는 `import_notion` 하나뿐이고 그 경로는 `clock_timestamp()`를 명시한다. `g4_…`(main 키, 7afd4d1 키에서 이관 후 재실행)가 통과하므로 멱등성도 유지된다 |

### H1–H3(검토 e908a8c)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| H1 P2: 참조를 해제된 다른 가져온 페이지로 바꾸고 digest를 남긴 사본 | **지적한 경로는 해소**. 좁은 변형 **I1** | `api.rs:1106-1128`. 출처가 가리킨 페이지 중 그 digest로 가져온 적이 있는 페이지가 없으면, 같은 digest를 가진 페이지도 판정 대상에 넣는다. `h1_…`는 지적한 재현 그대로 409이고 행 수가 바뀌지 않는다. C13 검출 |
| H2 P3: "최신 가져오기" 기준 불일치 | **해소**. 시험 범위 문제 **I2** | 기준 순서를 쓰는 판독기 다섯 곳이 모두 `imported_at DESC, sprint_revision DESC`를 쓴다: replay(820), `guard_import_owned`(1008), `notion_gate`(1135), `record_exception`(1298). `imported_at`은 `INSERT`(974)에서 `clock_timestamp()`로 찍는다. 그 `INSERT`는 `lock(tx,"registry")`(816) 뒤에 실행된다. 나머지 판독기(856, 869, 884, 1089, 1100, 1114, 1122)는 순서와 무관한 존재 검사나 DISTINCT 조회다 |
| H3 P3: C4b는 동등 변이가 아님 | **해소** | `h3_…`가 페이지 없는 이전 예외 행과 일치하지 않음을 고정한다. C4b 검출. "C4b 동등" 판정을 철회했다. C4a 동등 주장은 맞다. 새 데이터에서는 한 페이지가 한 시험 ID에만 속한다(C12). main 키에서는 (page, digest)가 유일했다. 그래서 다른 시험의 예외가 같은 페이지·같은 digest에 묶이고, 판정에서도 `test_id == sprint_id`를 통과하는 상태가 생길 수 없다 |

**`clock_timestamp()` 판단.** 건전하다. 같은 페이지를 가져오는 두 트랜잭션은 registry 자문 잠금에서 차례로 실행된다. 뒤 트랜잭션은 앞 트랜잭션이 커밋한 뒤에야 잠금을 얻는다. 그래서 뒤 트랜잭션의 `clock_timestamp()`는 앞 트랜잭션의 기록 시각보다 늦다. 따라서 새 행에서는 가져온 시각 순서가 revision 순서와 같다. 남는 경우는 벽시계가 뒤로 점프할 때뿐이다. 이때도 모든 판독기가 같은 행을 최신으로 보므로 서로 어긋나지는 않는다. 다만 그 행이 실제 최신 revision이 아닐 수 있다. 문서는 이 경우를 언급하지 않는다(I2 참고).

## 새 발견

### I1 · P3 · `carried` 판정이 가리킨 페이지의 과거 가져오기를 본다: 해제된 페이지가 예전에 같은 내용이었으면 참조만 바꾼 사본이 통과한다

- **위치.**
  - 판정: `crates/ieo-coord/src/api.rs:1111-1117`. `SELECT EXISTS (SELECT 1 FROM notion_imports WHERE page_id = ANY($1) AND content_digest = $2)`는 가리킨 페이지의 **모든** 가져오기 행을 본다.
  - 넓게 쓴 서술:
    - OpenAPI `contracts/ieo-api-v0.openapi.yaml:54-60`: "also when its reference is rewritten to another imported page"와 잔여 문장 "A copy whose sources cite neither the gated page's id nor its imported content digest … cannot be recognised"
    - REPORT §2.3 서버 행("참조만 다른 가져온 페이지로 바꾼 사본도 포함")
    - REPORT §3 H1 행
  - 규칙 자체의 서술은 정확하다. OpenAPI "unless a page that source names was itself imported with that digest"와 §2.2 ③ "그 digest로 가져온 적이 있으면"이 그렇다.
- **재현.** 검토용 임시 시험 `zz_probe_a`를 head aa38041과 시험 DB에서 돌렸고, 끝난 뒤 되돌렸다. 아래 1–2는 `g2_…`와 같은 준비 단계다.
  1. `importer`가 `POCX-T01`을 페이지 P로 가져온다. digest는 `a…`이고 `queue_gates()`가 있다.
  2. `importer`가 `POCX-T02`를 페이지 Q로 가져온다. 내용 digest는 같은 `a…`이고 gates가 있다. 페이지를 복제한 경우와 같다.
  3. 대조군: `copy_of_t01(POCX-Z20)`에서 출처의 `ref`만 Q로 바꾸면 **409 `IMPORT_GATED`**이다. Q 자신이 아직 차단 상태이기 때문이다.
  4. `importer`가 Q를 digest `c…`, gates 없음으로 다시 가져온다. Q가 해제된다.
  5. 3과 같은 사본 `POCX-Z21`(작업 `13-01`)을 등록한다. `kind`는 `notion_page`, digest는 P의 현재 차단 내용 `a…`이고, `ref`만 Q를 가리킨다.
  6. 결과: `alice` executable **201**, `bob` 의미 검토 **201**, `assigner` 배정 **201**이다.
  7. 같은 시점에 원래 `POCX-T01`의 파생 executable은 409 `IMPORT_GATED`이다.
- **원인.** Q가 예전에 digest `a…`로 가져온 행이 있으므로 `carried`는 참이다. 그래서 P를 판정에 넣지 않는다. 판정에는 Q의 **최신** 행(해제된 `c…`)만 쓰인다. 결국 사본이 인용한 내용 `a…`의 현재 상태는 아무 판정도 받지 않는다.
- **조건과 등급.** 이 경로가 열리려면 차단된 페이지와 정확히 같은 내용 digest(속성과 본문 전체)로 가져온 적이 있는 다른 페이지가 있어야 하고, 그 페이지가 이후 해제돼야 한다. H1은 "해제된 페이지가 하나만 있으면" 열렸다. 그에 비해 이 경로는 훨씬 좁다. 또 의도적인 등록자는 이미 문서화된 잔여 범위(참조와 digest를 둘 다 다른 가져온 페이지 값으로 바꾸기)로 전제 조건 없이 같은 결과를 얻는다. 따라서 이 경로가 우회 능력을 새로 주지는 않는다. 서버 강제 범위의 서술이 이 경우에 넓다는 점과, 같은 비용으로 막을 수 있다는 점 때문에 P3으로 둔다.
- **교정 확인(검토 시험).** `carried`가 가리킨 페이지의 **최신** 가져오기 digest만 보게 바꿔 보았다. 판정과 같은 정렬을 쓴다:
  ```sql
  SELECT EXISTS (SELECT 1 FROM (SELECT DISTINCT ON (page_id) content_digest FROM notion_imports
    WHERE page_id = ANY($1) ORDER BY page_id, imported_at DESC, sprint_revision DESC) l
    WHERE l.content_digest = $2)
  ```
  - 결과: 위 5의 사본이 409이다. coordinator 시험 58건과 검토 시험 b는 모두 그대로 통과한다. 검토 시험 b는 해제 경로, Q의 자기 예외, G2 상황을 다룬다.
  - 과차단도 생기지 않는다. 가져오기에서 나온 스프린트의 파생 명세는 반드시 최신 가져오기 digest를 인용해야 한다(`guard_import_owned`). 그래서 정당한 출처는 항상 가리킨 페이지의 최신 digest를 인용한다.
  - 이 교정을 하지 않는다면 §2.3·OpenAPI·`notion_gate` 주석의 잔여 범위에 이 경우를 적어야 한다. 이 경우는 "가리킨 다른 가져온 페이지가 예전에 같은 내용이었으면, 참조만 바꿔도 알아보지 못한다"는 것이다.

### I2 · P3 · H2 시험이 실행 판정·replay의 정렬과 `clock_timestamp()` 교정을 고정하지 못함

- **위치.**
  - `crates/ieo-coord/tests/coordinator.rs` `h2_every_reader_takes_the_same_latest_import`
  - REPORT §2.4 "등록 파생 판정, 실행 판정, 예외 승인이 같은 가져오기를 최신으로 본다(`h2_…`)"
  - `evidence/mutants.txt`(C14·C14b만 있음)
- **재현(검토용 추가 변이 `r30d-xmut.py`, coordinator 시험 전체).**

  | 변이 | 결과 |
  | --- | --- |
  | X1 `notion_gate`의 최신 행 질의를 `ORDER BY sprint_revision DESC`로 | **생존** |
  | X2 replay 부분 질의를 `ORDER BY sprint_revision DESC`로 | **생존** |
  | X3 `clock_timestamp()` → `now()` | **생존** |
  | X4 `carried`에서 digest 조건 제거(C13과 같은 효과) | 검출 |

- **원인.**
  - X1: h2 시험이 뒤바뀐 상태에서 실행 판정을 부르는 유일한 경로는 rev3에서 파생한 명세다. 이 명세는 그보다 먼저 `guard_import_owned`에서 409를 받는다. 그래서 판정의 정렬은 결과에 영향을 주지 않는다.
  - X3: 순차 가져오기에서는 `now()`도 순서가 [1,2,3]이다. 동시 실행을 재현하지 않으면 잡을 수 없다. 시험이 어렵다는 점은 이해할 만하다.
- **영향.** 현재 코드는 맞다. 판독기 다섯 곳의 정렬을 직접 읽어 확인했다. 다만 보고서가 `h2_…`를 실행 판정의 근거로 든 것은 사실보다 넓다. 이후 누가 판정 질의만 바꾸면 시험이 이를 잡지 못한다.
- **권고.**
  - 뒤바뀐 상태에서 판정 결과가 rev2와 rev3 중 어느 쪽을 보느냐에 따라 달라지는 단언을 추가한다. 예를 들어 rev2를 차단(a), rev3을 해제(b)로 두고 다른 sprint의 사본이 409인지 확인한다.
  - replay도 같은 방식으로 단언한다.
  - X3는 "시험으로 고정하지 않음"이라고 mutants.txt나 §2.4에 적는다.
  - 벽시계가 뒤로 점프할 때의 잔여 위험도 적는다. 이 위험은 새 데이터에서 페이지 안의 정렬을 `sprint_revision`으로 하면 없어진다.

### I3 · P3 · REPORT §2.4 "유효한 사용자 승인 PoC 예외" 행의 문장 연결

- **위치.** REPORT §2.4 셋째 행: "… 같은 가져오기를 최신으로 본다(`h2_…`), T01 예외가 있으면 201·201·201이고 …"
- **내용.** 교정 3에서 h3·h2 문장을 기존 문장 사이에 끼워 넣었다. 그래서 `h2_…` 문장과 원래의 "T01 예외가 있으면…" 문장이 쉼표로 이어졌다. 그 결과 h2 시험이 T01 예외 경로를 검사하는 것처럼 읽힌다. 서술 내용 자체는 틀리지 않았다.

## 확인했고 문제없는 항목

- **H1 판정의 과차단 검사.**
  - 해제(`c2_a_trusted_release_…`), 예외(`c2_a_user_exception_…`), 같은 내용을 가진 두 페이지(`g2_…`)가 모두 통과한다.
  - 검토 시험 b의 상태: P는 다른 내용으로 해제됐고, Q는 P의 옛 내용과 같은 내용으로 차단돼 있다. 이 상태에서 P의 해제 파생 명세(P 참조, digest `b…`)는 201·201·201이다. P의 과거 행이 `a…`로 Q와 겹쳐도 `carried`가 참이라 Q를 끌어오지 않는다. 같은 상태에서 Q의 자기 예외로 Q의 파생은 201이다.
  - 사람 입력 출처(`000…`)나 아직 가져오지 않은 페이지 ID는 `named`가 비고 digest 일치도 없으므로 영향이 없다.
- **차단 페이지 ID를 직접 인용한 사본.** 그 페이지가 `named`에 들어가므로 digest와 무관하게 항상 판정 대상이다. `g1_…`에서 확인했다.
- **이관.** G4 행과 같다. 이번 교정에는 DDL 변경이 없다.
- **문서.**
  - OpenAPI의 "Latest import means the newest by import time (stamped after the registry lock), then revision, for every check"는 코드와 같다.
  - §2.2 ③은 규칙을 정확히 적었다. 넓은 서술은 I1에서 지적한 요약 문장들뿐이다.
  - §2.5는 교정 2·3이 영속 코디네이터에 아직 배포되지 않았다고 정직하게 적었다.
- **변이 증거.**
  - 직접 다시 돌린 결과가 `mutants.txt`와 같다: 19개 중 18개 검출, C4a 생존, 실행 뒤 작업 트리 깨끗함. 검출한 시험 목록은 앞 4개만 보이므로 순서만 다르다.
  - C9b의 이름이 교정 3 이전의 동작을 가리킨다는 점은 결과 주석에 밝혀 두었다.
  - C4a 동등 주장은 맞다(H3 행).
  - 생존한 변이를 감추지 않았다. 다만 H2의 증거 범위는 I2처럼 넓게 적혀 있다.

## 실행한 명령

```
gh pr view 30 --json mergeable,headRefOid          # MERGEABLE, aa38041ef87bf41facfae235ccc1776d767a754f
git fetch -q origin; git merge-tree --write-tree origin/main aa38041   # 충돌 없음
git diff --stat a33a8e2 aa38041 -- crates           # 비어 있음
git worktree add --detach <scratch>/wt30d aa38041ef87bf41facfae235ccc1776d767a754f
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-r30d
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                          # 0
cargo clippy -q --locked --all-targets -- -D warnings   # 0
cargo test --locked --all-targets                   # 0, 182 passed
python3.11 -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 18/19, C4a survived, dirty none
python3.11 -I <scratch>/r30d-xmut.py                 # X1·X2·X3 survived, X4 killed (I2)
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py          # rc 0, schema_errors 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -B docs/poc/scripts/test_render_progress.py      # RESULT PASS 0
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
# 검토용 임시 시험(zz_probe_a·b, 저장소에 넣지 않음; diff는 <scratch>/r30d-probes.diff):
#   a: Q가 예전에 P와 같은 내용 → Q 해제 → ref만 Q로 바꾼 사본(I1) + 대조군
#   b: 해제·예외·같은 내용 두 페이지의 허용 경로(과차단 확인)
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
# I1 교정안(carried를 최신 digest로) 적용 후 coordinator 60건(검토 시험 포함) 통과, 사본 409
# 병합 상태: git merge --no-commit --no-ff origin/main (임시 worktree)
cargo clippy … ; cargo test --locked --all-targets  # 0, 202 passed; validate/render/openapi 통과; git merge --abort
```

정리:
- 임시 시험, 교정안, 병합은 `git checkout`과 `git merge --abort`로 되돌렸다. 이후 `git status`는 깨끗했다.
- worktree `wt30d`와 target `target-r30d`는 검토 뒤 삭제했다.
- 시험 DB는 하네스가 만든 임시 DB만 썼다.
- 비밀번호와 토큰은 어디에도 기록하지 않았다.
- 영속 DB, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다.
