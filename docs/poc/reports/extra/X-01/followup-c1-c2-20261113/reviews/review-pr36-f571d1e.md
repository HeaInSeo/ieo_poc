VERDICT: approve

# PR #36 독립 검토 1 (Guardrail) · head f571d1e · base main 50f407e

## 요약

- **대상.** `fix/x01-c2-k1-k2`의 head는 `f571d1ebfec21dcac1046088a83d683f946f5864`이다. 검토를 시작할 때 `gh pr view 36 --json headRefOid,mergeStateStatus`의 결과는 이 커밋과 `BLOCKED`였다. 그때는 CI "rust fmt clippy test"가 pending이었다. 검토가 끝날 때는 CI 세 건(rust fmt clippy test, docs/poc 검증, toolchain pin)이 모두 pass였고, 상태는 `CLEAN`이었다. head는 그대로였다. origin/main은 `50f407e`이고, `git merge-tree`에서 충돌은 없다.
- **변경.**
  - 서버 코드: `notion_gate`의 페이지별 판정 앞에서 `count(DISTINCT test_id)`를 센다. 그 값이 1보다 크면 409 `IMPORT_GATED`로 거부한다. 함수 doc 주석도 함께 바꿨다.
  - 시험: `i2_…` 두 개의 이름과 주석을 고쳤다. `k1_a_page_imported_under_two_test_ids_fails_closed`를 새로 넣었다.
  - 문서: REPORT §2.2·§2.4·§3(G4·H2·K1·K2 행), mutants.py(C19), mutants.txt, cargo-test.log.
- **검사 재실행(head, 임시 worktree).**
  - fmt는 통과했다.
  - `cargo test --locked --all-targets`: 207건 통과, 실패 0, 건너뛴 시험 0. 시험 DB에 연결했다(coordinator 63건).
  - clippy는 로컬에서 돌리지 않았다. 디스크 여유가 2–5 GB뿐이었다. 같은 head의 CI가 pass이다.
  - mutants.py 24개를 head에서 **한 번에** 다시 돌렸다. 23개가 검출됐고, C4a만 생존했다. 실행 뒤 작업 트리는 깨끗했다. mutants.txt의 최종 결과와 같다.
- **K1·K2 판정.**
  - K1은 해소됐다. 6352413 검토 시험 B의 경로가 이제 409이다. base에서는 201이다. 허용 경로도 그대로 열려 있다.
  - K2는 대부분 해소됐다. 다만 고친 문장에 작은 결함이 둘 남았다(L1).
- **허용 경로.** 아래 경로가 head에서 실제로 통과한다. 그래서 항상 거부하는 구현이 아니다.
  - 신뢰된 해제, 사용자 예외, G2(같은 내용 두 페이지)를 고정하는 기존 시험이 모두 통과한다.
  - 이전 데이터 페이지가 DB에 **있는** 상태에서, 그 페이지와 같은 내용을 가진 다른 페이지도 통과한다. 예외 경로에서는 등록·의미 검토·배정이 201·201·201이다. 해제 경로에서는 등록·의미 검토가 201·201이다(검토 시험 D).
- **새 발견은 P3 셋이다.** P2 이상이 없으므로 approve한다.

## 이전 지적 재판정(검토 6352413)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| K1 P3: 한 페이지에 두 test_id가 있는 이전 데이터에서 revision 우선 정렬이 열린 실패를 만듦 | **해소** | 아래 "K1 검토". 시험 B가 head에서 409이고 base에서 201이다. C19가 검출된다 |
| K2 P3: 시험 이름·주석, §3 G4·H2 행 | **해소**(문장 결함 L1은 남음) | 시험 이름이 `…_highest_revision_whatever_the_clock`으로 바뀌었다. 주석 "revision 2 (gated), not the later-stamped revision 1 (released)"는 `inverted_latest`와 맞다. G4·H2 행에는 J2 변경이 적혔다 |

J1–J3 이전의 지적(F·G·H·I)도 회귀가 없다. 해당 시험이 모두 통과하고, 대응 변이 C1–C18도 C4a를 빼고 모두 검출된다.

## K1 검토

**코드.** 검사는 `notion_gate`의 `for page in pages` 안에 있다. 그래서 대상 페이지 ①·②·③ 모두에 적용된다. ①은 그 스프린트로 가져온 페이지, ②는 출처가 이름을 댄 페이지, ③은 digest로 끌려온 페이지이다.
- 페이지별 최신 행을 고르는 질의보다 앞에서 거부하므로, revision 숫자를 비교하는 일이 생기지 않는다.
- 예외 확인보다도 앞이다. 그래서 사용자 예외로도 풀리지 않는다. 오류 문구는 "API 밖에서 정리하라"이다.
- `notion_gate`는 executable 등록(`api.rs:604`)과 배정(`api.rs:2212`) 둘 다에서 불린다.

**검토 시험.** 저장소에 넣지 않았다(`<scratch>/r36-probes.rs`, `r36-probe-e.rs`). head와 base(`50f407e`의 `crates/ieo-coord/src`)에서 각각 돌렸다.

| 시험 | head | base 50f407e |
| --- | --- | --- |
| **B(6352413 재현)**: P를 T01로 a(차단)·b(해제), Q를 T02로 d(차단)로 가져온 뒤 Q 행을 P로 바꿈. T02 파생 executable | **409 `IMPORT_GATED`**(두 test id 문구) | 201(열린 실패) |
| B: T01 파생(해제 b) | 409 | 201 |
| B: P를 인용한 Z13 사본 | 409 | 201 |
| B: 거부 뒤 행 수 | 불변 | 바뀜 |
| B: T02 예외 승인 뒤 T02 파생 | 예외 201, 파생 **409** | 예외 201, 파생 201 |
| B: T02 draft 등록 | 201 | (같음) |
| B2: 두 test id가 모두 해제(차단 없음)인 이전 페이지 | 409 | 201 |
| C: T01 해제 → 등록 201 → 의미 검토 201 → 이전 상태로 바꿈 → 배정 | **409** | 201 |
| D: P에 이전 데이터 행 삽입(T01 + `POCX-LEG`) → T01 파생 | 409 | 201 |
| D: 다른 페이지 R(T02), P와 **같은 내용** b, 차단 → 예외 없이 | 409(일반 gates 문구) | 409 |
| D: R에 예외 → 파생·의미 검토·배정 | **201·201·201** | 201·201·201 |
| D: R 해제(c) → 파생·의미 검토 | **201·201**(배정은 같은 역할에 앞 배정이 있어 409 `OWNERSHIP_CONFLICT`. 게이트와는 무관하다) | 같음 |
| D: 페이지 ID를 지우고 digest b만 남긴 사본 | 409(두 test id. ③으로 P가 끌려옴) | 201 |

- **닫힌 실패.** 두 test id를 가진 페이지는 인용 방식과 상관없이 거부된다. 인용 방식은 직접, 사본, digest 사본, 배정 시점의 재검사를 모두 확인했다. 예외로도 풀리지 않는다. 행도 바뀌지 않는다.
- **과차단.**
  - 정상 데이터에서는 한 페이지가 test id 하나만 가진다. 두 번째 test id로 가져오면 856행에서 409이고, C12가 검출된다. 그래서 새 검사는 정상 데이터에서 발동하지 않는다.
  - 같은 내용을 가진 다른 페이지(G2)는 `carried` 덕분에 이전 데이터 페이지를 끌어오지 않는다. 시험 D에서 확인했다.
  - B2에서 "두 test id가 모두 해제된 이전 페이지"도 거부된다. 이것은 의도한 일괄 닫힘이고, REPORT §2.2와 오류 문구에 적혀 있다. 영속 DB에 해당 페이지가 0건이라는 보고가 맞다면(L2) 실제 영향은 없다.

**다른 읽기 지점(이전 데이터에서 열린 실패가 남는가).**

| 읽기 지점 | 이전 데이터에서의 동작 | 판정 |
| --- | --- | --- |
| replay(`api.rs:820`) | revision이 큰 T01 rev2(b)를 최신으로 본다. T01로 b를 다시 가져오면 200 `replay: true`이고 행이 생기지 않는다. T02로 d를 다시 가져오거나, 어느 쪽으로든 새 내용을 가져오면 409 `EVENT_CONTENT_CONFLICT`이다 | 실행을 허용하는 경로가 아니다. 문제없음 |
| `guard_import_owned`(`1006`) | test_id 단위로 판정하므로 T02 rev1 파생을 통과시킨다. 그러나 바로 뒤의 `notion_gate`가 ①(T02의 페이지 = P)에서 거부한다 | 닫힘 |
| `record_exception`(`1319`) | T02의 최신(P, d)으로 예외를 201로 기록한다. 하지만 게이트가 예외 확인보다 먼저 거부하므로 효과가 없다 | 닫힘. 효과 없는 예외가 기록되는 점은 아래 "확인한 항목"에 적었다 |
| `carried`(`1118`) | 이전 데이터 페이지가 ②로 지목되면 그 페이지 자체가 `pages`에 들어가 거부된다. 그래서 `carried`의 값과 상관없이 결과가 같다 | 닫힘 |

## 새 발견

### L1 · P3 · 고친 문서 문장의 인용 오류와 문장 이음새

- **위치와 내용.**
  - REPORT §2.2 "새 가져오기는 두 번째 시험 ID를 거부한다(검토 f0c4e87 J2)": 두 번째 test id를 거부하는 규칙은 J2와 무관하다. J2는 revision 우선 정렬이다. 그 규칙의 시험은 G3(검토 7afd4d1)에서 고정됐다. 원래 이 괄호는 앞 문장(정렬)에 붙어 있었는데, 문장을 나누면서 엉뚱한 문장으로 옮겨졌다.
  - REPORT §3 G4 행 "…`g4_…`가 시험한다 이 순서는 두 번째 후속 PR(J2)에서 …", H2 행 "…C14·C14b 검출 이 순서는 …": 앞 문장과 덧붙인 문장 사이에 마침표가 없다.
  - OpenAPI(`contracts/ieo-api-v0.openapi.yaml` 49–60행)의 게이트 설명에는 "두 test id를 가진 페이지는 409 `IMPORT_GATED`"라는 새 동작이 없다. 코드는 이미 문서화된 오류 코드를 쓰므로 계약을 깨지는 않는다.
- **권고.**
  - 괄호를 "(검토 7afd4d1 G3)"로 고치거나 지운다.
  - G4·H2 행에 마침표를 넣고, 덧붙인 문장을 괄호로 감싼다.
  - OpenAPI의 "Latest import means …" 문장 뒤에 한 문장을 더한다.

### L2 · P3 · "영속 DB 0건"을 뒷받침하는 기록이 저장소에 없음

- **위치.** REPORT §3 K1 행의 "영속 DB에는 그런 페이지가 0건이다(읽기 전용 조회)". 질의문은 PR 본문에만 있다.
- **내용.** 6352413 검토의 권고 1은 "결과는 보고 PR에 남긴다"였다. 그런데 REPORT에는 질의문, 실행 시각, 대상 DB, 출력이 없다. evidence 폴더에도 해당 파일이 없다. 영속 DB는 이번 검토 범위 밖이어서 나는 이 주장을 확인하지 않았다.
  - K1의 코드 교정이 닫힌 실패이므로, 이 숫자가 틀려도 열린 실패는 생기지 않는다. 그래서 P3이다.
  - 다만 0건이 아니면 해당 페이지를 인용한 실행이 모두 409가 된다. X-02 재적용 때 확인할 근거가 필요하다.
- **권고.** 질의문과 결과(시각, 행 수)를 REPORT §2.5 또는 evidence에 한 줄로 남긴다. 아래 L3의 비정규 page_id 조회도 함께 남긴다.

### L3 · P3 · 서버가 page_id 형식을 정규화하지 않아 "한 페이지 두 test id"를 새로 만들 수 있음(기존 문제, 이번 diff 밖)

- **위치.**
  - `api.rs:757`: `page_id`는 `str_field(…, 64)`로만 확인한다. 32자리 소문자 hex라는 형식 검사가 없다.
  - `api.rs:856`의 G3 검사와 K1의 `count(DISTINCT test_id) … WHERE page_id = $1`은 모두 문자열이 정확히 같을 때만 같은 페이지로 본다.
  - 반면 출처 인용(`page_ids_in`)은 대시를 지우고 소문자로 바꿔 비교한다.
  - 주석과 REPORT §2.2는 "only possible before that was refused"라고 적었다.
- **재현(검토 시험 E, head).**
  - T01로 P(`0123…cdef`)를 가져왔다(차단).
  - 같은 페이지를 대시 표기(`01234567-89ab-cdef-0123-456789abcdef`)로 T02에 가져왔다. 결과는 **201**이었다. 이제 두 행이 같은 Notion 페이지를 서로 다른 문자열로 가리킨다. K1 검사는 이 상태를 보지 못한다.
  - 이 경우 T02 파생은 409였다. ②가 T02 URL 속 ID를 정규화해서 T01의 차단 행을 찾았기 때문이다. 이번 시험에서 열린 실패는 재현하지 못했다.
- **조건과 등급.**
  - 가져오기는 service 전용 `notion:import` 범위가 필요하다. 가져오기 도구는 `page_id_from`으로 정규화해서 보낸다.
  - 다만 `ieo-notion/src/api.rs:141`의 실패 대체 경로와 스냅샷 파일 경로(`lib.rs:358`)는 받은 값을 그대로 쓴다.
  - 신뢰된 서비스가 비정규 값을 보내야만 생기는 상태이고, 실제 열린 실패도 재현하지 못했다. 그래서 P3이다. 그래도 "이전 데이터에서만 가능하다"는 전제는 엄밀하지 않다.
- **권고.** X-02 전에 하나를 고른다.
  1. 서버가 `page_id`를 32자리 소문자 hex로 검증해 아니면 400을 주거나, 정규화해서 저장한다.
  2. 또는 영속 DB에서 `SELECT count(*) FROM notion_imports WHERE page_id !~ '^[0-9a-f]{32}$'`가 0인지 확인하고 L2의 기록에 함께 남긴다.

## 확인했고 문제없는 항목

- **신뢰 기록만으로 판정.** 새 검사는 서버의 가져오기 행만 센다. 명세 문구는 보지 않는다. 판정을 푸는 방법은 여전히 둘뿐이다. 신뢰된 해제 행(service `notion:import`)과 사용자 예외 행(user `exceptions:approve`)이다. 이전 데이터 페이지에는 둘 다 통하지 않는다(닫힘).
- **효과 없는 예외.** 이전 데이터 페이지를 가진 test id에 대해서도 `record_exception`은 201로 예외를 기록한다. 하지만 게이트가 그 예외를 쓰지 않는다. 오해의 여지는 있지만, 게이트의 오류 문구가 "API 밖에서 정리"를 안내하므로 지적으로 올리지 않는다.
- **`k1_…` 시험.**
  - T01과 T02 양쪽 모두 409 `IMPORT_GATED`이고, 행 수가 바뀌지 않음을 단언한다.
  - 단언은 오류 코드만 본다. 하지만 검사가 없으면 두 쪽 모두 201이므로(base 결과, C19 검출) 판별력이 있다.
  - base 소스에 이 시험을 돌리면 3749행에서 실패한다.
- **변이 증거의 정직성.**
  - mutants.txt는 실행 이력을 커밋별로 나눠 적었다. 본 실행 9ec5eb7, C14 재실행 929e611, C19 단독 2765f57이다.
  - C1–C18은 K1 코드가 들어가기 전의 api.rs에서 돌린 것이다. 그래서 "24개 중 23개"는 서로 다른 커밋의 합산이다. 다만 각 줄에 커밋이 적혀 있다.
  - 내가 head에서 24개를 한 번에 다시 돌린 결과도 23/24이다. C4a만 생존했고 dirty는 none이었다. 그러므로 결과 줄은 사실이다.
  - C19의 패턴(`if test_ids > 1 {` → `if false {`)은 K1 검사만 정확히 끈다.
  - C4a 동등 판단은 이전 검토와 같다. K1 검사는 "한 페이지 = 한 test id" 전제를 이전 데이터에서도 강제하므로, 동등 근거를 약하게 만들지 않는다.
- **cargo-test.log.** 머리 커밋은 `c547f78`이다. `2765f57`부터 head까지 `crates/`는 바뀌지 않았다. 로그의 207건은 내가 다시 돌린 수(207 통과, coordinator 63)와 같다.
- **schema.sql 주석.** 바뀌지 않았지만 지금 동작과 어긋나지 않는다.
- **§2.5·§2.6.** 이번 교정도 영속 코디네이터에 배포되지 않았다. X-02 전에 재적용해야 한다는 기존 서술은 그대로 유효하다. 재적용할 때 L2·L3의 확인을 함께 할 것을 권고한다.

## 실행한 명령

```
gh pr view 36 --json headRefOid,mergeStateStatus   # f571d1e…, BLOCKED(CI pending) → 끝날 때 CLEAN
gh pr checks 36                                     # 끝날 때 3건 pass
gh pr diff 36; git diff 50f407e f571d1e; git log --oneline 50f407e..f571d1e
git diff --stat 2765f57 f571d1e -- crates           # 비어 있음
git fetch -q origin; git merge-tree --write-tree origin/main f571d1e   # 충돌 없음(main = 50f407e)
git worktree add --detach <scratch>/wt36 f571d1ebfec21dcac1046088a83d683f946f5864
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-r36
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                          # 0
cargo test -q --locked --all-targets                # 0, 207 passed, 0 failed, 0 ignored
# 검토용 임시 시험 zz_probe_b·b2·c·d·e(저장소에 넣지 않음; <scratch>/r36-probes.rs, r36-probe-e.rs)
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
git checkout 50f407e -- crates/ieo-coord/src; cargo test … -- zz_probe k1_ (base 비교); git checkout f571d1e -- crates/ieo-coord/src
python3.11 -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 23/24, C4a 생존, dirty none
```

정리:
- 임시 시험은 `git checkout`으로 되돌렸다. 이후 `git status`는 깨끗했다.
- worktree `wt36`과 target `target-r36`은 검토 뒤 삭제했다.
- 시험 DB는 하네스가 만든 임시 DB만 썼다.
- 비밀번호는 출력하거나 기록하지 않았다.
- 영속 DB, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다.
