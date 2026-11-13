VERDICT: approve

# PR #34 독립 검토 1 (Guardrail) · head f0c4e87 · base main db18e97

## 요약

- **대상.** `fix/x01-c2-i1-i3`의 head는 `f0c4e872a1efe7da3de6411fe0f9b31fbdbdcb19`이다. `gh pr view 34 --json headRefOid,mergeStateStatus`의 결과는 이 커밋과 `BLOCKED`이다. CI 세 건(rust fmt clippy test, docs/poc 검증, toolchain pin)은 모두 pass이므로, BLOCKED는 필수 검토가 아직 없기 때문으로 본다. origin/main은 아직 `db18e97`이고 `git merge-tree`에서 충돌은 없다.
- **변경.** 서버 코드는 `api.rs`의 `carried` 질의 하나와 주석만 바뀌었다. 시험은 `i1_…` 1건과 `i2_…` 3건이 추가됐다. 문서는 OpenAPI 한 문장, REPORT §2.2·§2.4·§3, mutants.py·mutants.txt·cargo-test.log가 바뀌었다. `mutants.txt` 머리의 커밋 `92473bb`와 head의 `crates/`는 같다(`git diff --stat 92473bb f0c4e87 -- crates`의 결과가 비어 있음).
- **검사 재실행(head, 임시 worktree).**
  - fmt 통과.
  - `cargo test --locked --all-targets`: 206건 통과, 실패 0. 시험 DB에 연결했고 건너뛴 시험은 없다. 이 중 coordinator는 62건이다.
  - clippy는 디스크 여유가 부족해 로컬에서 돌리지 않았다. 같은 head의 CI "rust fmt clippy test"가 pass이다.
  - mutants.py 23개: 22개 검출, C4a 생존. C13·C13b도 새 패턴으로 한 번에 검출됐다. 실행 뒤 작업 트리는 깨끗했다. mutants.txt와 결과가 같다.
  - `i2_…` 반복 실행: `i2_` 3건을 30회 단독으로 돌려 30/30 통과했다. coordinator 전체(62건, 병렬)도 10회 돌려 10/10 통과했다. C18 변이(`now()`)를 넣고 잠금 시험을 15회 돌리자 15/15 실패했다(변이 검출).
- **I1–I3 판정.**
  - I1은 해소됐다. aa38041 재현 경로가 409이고 행 수가 바뀌지 않는다.
  - I2도 해소됐다. 세 시험이 각각 판정 정렬, 재조회 정렬, `clock_timestamp()`를 고정한다. 대응 변이 C16·C17·C18이 모두 검출된다.
  - I3는 일부만 해소됐다(J1).
- **허용 경로.** 신뢰된 해제, 사용자 예외, G2(같은 내용 두 페이지)의 페이지별 판정이 모두 실제로 통과한다. 항상 거부하는 구현이 아니다(아래 검토 시험 a·b).
- **새 발견은 P3 셋이다.** P2 이상이 없으므로 approve한다.

## 이전 지적 재판정(검토 aa38041)

| 지적 | 이번 판정 | 근거 |
| --- | --- | --- |
| I1 P3: `carried`가 가리킨 페이지의 과거 가져오기 전체와 비교 | **해소** | `api.rs:1110-1121`. 각 지목 페이지의 최신 행(`DISTINCT ON (page_id) … ORDER BY page_id, imported_at DESC, sprint_revision DESC`)의 digest와만 비교한다. 정렬은 판정·재조회·등록 파생 판정·예외 승인과 같다. aa38041의 교정안과 글자 그대로 같다. `i1_…`는 aa38041 재현 경로(P 차단 a → Q가 a로 차단 → Q를 c로 해제 → ref만 Q로 바꾼 사본)가 409이고 행 수가 그대로임을 고정한다. C15(과거 전체 비교로 되돌림) 검출 |
| I2 P3: 판정·재조회 정렬과 `clock_timestamp()`를 시험이 고정하지 못함 | **해소** | 아래 "I2 시험 검토" 참고. C16·C17·C18 검출 |
| I3 P3: §2.4 예외 행의 문장 연결 | **일부 해소**(J1) | h2 문장은 새 행 "최신 가져오기의 순서"로 옮겼다. 그러나 남은 행에 "(`h3_…`)., T01 예외가 있으면 …"이 남았다 |

F1–F6, G1–G4, H1–H3은 회귀가 없다. 해당 시험이 모두 통과하고 대응 변이(C1–C14b)도 모두 검출된다.

## I1 회귀 검토

검토용 임시 시험(`zz_probe_a`·`b`·`c`)을 head에서 돌렸고, 끝난 뒤 되돌렸다. 저장소에는 넣지 않았다.

**a: aa38041 I1 재현, 대조군, 허용 경로**

| 단계 | 결과 |
| --- | --- |
| P(T01) 차단 a, Q(T02) 차단 a. ref만 Q로 바꾼 사본(대조군) | 409 `IMPORT_GATED` |
| Q를 c로 해제한 뒤 같은 사본(I1 경로) | **409 `IMPORT_GATED`**, 행 불변(aa38041에서는 201·201·201) |
| Q 자신의 해제 파생(rev 2→3) | 등록 201, 의미 검토 201, Q의 작업으로 배정 201 |
| P 자기 예외 → P 파생 | 예외 201(`approved_page_id`=P), 등록 201, 의미 검토 201 |
| P 예외가 있는 상태에서 다른 sprint의 I1 사본 | 409(예외는 자기 시험 스프린트만 푼다) |
| ref와 digest를 모두 Q의 최신 값(c)으로 바꾼 사본 | 201. 문서화된 잔여 범위(§2.3 "둘 다 바꾸고")와 일치 |

배정 칸의 422·409는 검토 시험의 배정 인자(작업 ID, repo당 개발 배정 하나) 때문에 생긴 것이다. 판정과는 관계없다. P 예외 경로의 201·201·201은 기존 `c2_a_user_exception_…`가 고정한다.

**b: G2 반대 방향.** P와 Q가 같은 내용 a로 차단된 뒤 P만 b로 해제했다.
- P의 해제 파생은 201·201·201이다.
- Q의 파생은 예외가 없으면 409이다. Q 자기 예외 뒤에는 등록 201, 의미 검토 201이다.
- P의 과거 행 a가 Q와 겹쳐도 Q의 판정에 P가 끌려오지 않는다. 과차단이 없다.

**c: 행동 변화의 범위.** I1 교정으로 바뀌는 것은 하나다. 출처가 지목한 페이지의 **과거** digest를 인용하면, 이제 그 digest를 가진 다른 페이지도 판정 대상에 들어간다. 정당한 파생은 `guard_import_owned`가 최신 digest 인용을 강제하므로 영향이 없다. 오래된 digest를 인용한 사람 작성 명세는 더 엄격해질 뿐이다(닫힌 쪽). 따라서 과차단 회귀로 보지 않는다.

**문서 일치.** 규칙 문장은 이제 코드와 같다: OpenAPI "unless the latest import of a page that source names carries that digest", §2.2 ③ "최신 가져오기가 그 digest이면", `notion_gate` 주석. §2.3 서버 행의 "참조만 다른 가져온 페이지로 바꾼 사본도 포함"도 이제 성립한다. 예외가 하나 있다. 바꾼 대상 페이지의 **현재** 내용이 인용 digest와 같으면 판정 대상에서 빠진다. 그러나 그것은 "ID와 digest가 모두 다른 가져온 페이지의 값"인 경우이므로, §2.3 잔여 행과 OpenAPI 잔여 문장에 이미 포함된다.

## I2 시험 검토

- **`i2_the_gate_…`.** `inverted_latest`로 rev1(차단 a)의 시각을 rev2(해제 b)보다 늦게 바꾼다. 그다음 rev2에서 파생한 사본(digest b, 다른 sprint)을 등록한다.
  - P의 최신 행이 a이므로 `carried`는 거짓이다. digest b로 끌려오는 페이지도 P 자신이다.
  - 판정 질의가 시각 순이면 rev1(차단)이 최신이어서 409이다. revision 순이면 rev2(해제)여서 201이다.
  - 다른 sprint이므로 `guard_import_owned`를 거치지 않는다. 따라서 aa38041 X1이 생존한 이유(파생 경로가 guard에서 먼저 막힘)가 사라졌다. C16 검출.
- **`i2_a_replay_…`.** 같은 반전 상태에서 a를 다시 가져오면 replay(200, `replay: true`)여야 한다. revision 순이면 최신이 b이므로 새 revision이 된다. C17 검출.
- **`i2_an_import_is_stamped_…`.** 별도 연결에서 `pg_advisory_xact_lock(hashtextextended('registry', 0))`를 잡는다.
  - `lock(tx, "registry")`(`api.rs:176-181`)의 `hashtextextended($1, 0)`에 `'registry'`를 넣은 것과 같은 키이다.
  - 자문 잠금은 데이터베이스 단위이다. `e.db()`와 서버는 같은 시험 DB(`cfg`)에 붙고, 시험마다 DB가 따로이므로 병렬 시험끼리 간섭하지 않는다.
  - 판정이 결정적이다. 올바른 코드에서 `imported_at`(잠금 획득 뒤 `clock_timestamp()`)은 같은 PostgreSQL 서버 시계로 잰 `released_at`(커밋 직전) 이후일 수밖에 없다. DB 시계가 뒤로 가지 않는 한 거짓 실패가 없다.
  - 멈출 위험도 낮다. 단언이 실패해 panic이 나도 `tx`가 drop되면서 롤백되고, 롤백이 잠금을 풀므로 요청 스레드는 끝난다. 서버 작업자는 3개이고 시험이 막는 것은 하나뿐이다.
  - 약점은 J3이다.
- **반복.** 30회 단독 실행, 전체 병렬 10회 실행 모두 통과했다. C18은 15/15 검출됐다.

## 새 발견

### J1 · P3 · I3가 남음: §2.4 예외 행의 "(`h3_…`)., T01 예외가 있으면"

- **위치.** REPORT §2.4 "유효한 사용자 승인 PoC 예외" 행(REPORT.md 53행): "… 일치하지 않는다(`h3_…`)., T01 예외가 있으면 201·201·201이고 …"
- **내용.** h2 문장을 새 행으로 옮기면서 그 뒤의 쉼표가 남았다. 그래서 ".," 구두점이 생겼고, 문장도 여전히 쉼표로 이어진다. §3 I3 행은 "순서 시험을 별도 행으로 옮겼다"고 해소를 선언했지만, 지적한 연결 문제는 형태만 바뀌어 남아 있다.
- **권고.** "(`h3_…`). T01 예외가 있으면 …"으로 고친다.

### J2 · P3 · 시계가 뒤로 갈 때의 잔여 위험은 "열리는" 방향인데, 방향과 위치가 적혀 있지 않음

- **위치.**
  - REPORT §2.4 새 행 "최신 가져오기의 순서": "시계가 거꾸로 가는 경우는 잔여 위험이다."
  - §2.3 강제 범위 표, OpenAPI, `notion_gate` 주석에는 이 경우가 없다.
- **재현(검토 시험 c).** P를 a(차단)로 가져온 뒤 b(해제)로 가져왔다. rev2의 `imported_at`을 1시간 뒤로 옮겨 시계가 뒤로 간 상황을 흉내 냈다. 이어서 c(재차단)를 가져왔다.
  - c의 가져오기는 201(새 revision 3)이다.
  - 그러나 모든 판독기가 최신을 rev2(해제)로 본다. 그래서 rev2에서 파생한 executable 등록이 **201**이다.
  - 즉 새 차단 기록이 무시된다(fail-open). 반대로 해제가 무시되면 닫힌 쪽이다. 두 방향이 모두 가능하다.
- **조건과 등급.** 시각은 PostgreSQL 서버의 `clock_timestamp()` 하나에서 나온다. 그래서 DB 호스트 시계가 앞 가져오기 시각보다 뒤로 점프해야만 생긴다. 판독기끼리 어긋나지도 않는다. 가능성이 낮아 P3이다.
- **권고.** 둘 중 하나를 한다.
  - §2.3 잔여 행이나 §2.4에 "DB 시계가 뒤로 가면 그 사이의 새 가져오기(재차단 포함)가 최신으로 인정되지 않을 수 있다(열리는 방향 포함)"고 적는다.
  - 또는 aa38041 I2의 권고대로, 새 데이터에서 페이지 안의 정렬을 `sprint_revision` 기준으로 한다. 같은 페이지의 revision은 등록 잠금 아래에서 단조 증가한다.

### J3 · P3 · 잠금 시험이 고정 대기 500ms에 기대므로, 부하에서는 C18을 놓칠 수 있음(거짓 실패는 아님)

- **위치.** `coordinator.rs` `i2_an_import_is_stamped_after_the_registry_lock`의 `std::thread::sleep(500ms)`
- **내용.** 요청 트랜잭션이 500ms 안에 잠금 대기에 들어가지 못하면, 요청은 잠금이 풀린 뒤에 시작된다. 그러면 `now()` 변이도 `stamped >= released_at`을 만족해 생존한다.
  - 정상 코드에서는 이 경우에도 통과한다. 그래서 거짓 실패(flake)는 없고, 검출력만 약해진다.
  - 이번 반복에서는 15/15 검출됐다.
- **권고(선택).** 고정 대기 대신 `pg_locks`에서 `locktype = 'advisory' AND NOT granted`인 행이 나타날 때까지 폴링한다. 그러면 요청이 잠금을 기다리고 있음을 확인한 뒤 해제할 수 있다.

## 확인했고 문제없는 항목

- **신뢰 기록만으로 판정.** 판정과 허용은 모두 서버의 가져오기 행(service `notion:import`)과 예외 행(user `exceptions:approve`)으로 정한다. 명세 문구는 쓰지 않는다. 이번 변경으로 바뀐 점은 없다.
- **변이 증거의 정직성.**
  - 본 실행(92473bb)에서는 C13·C13b가 `PATTERN COUNT 0`(실행 안 됨)으로 그대로 기록됐다. 패턴을 고친 뒤(9250980) 따로 다시 돌렸다는 주석과 그 결과도 함께 실었다. 9250980과 92473bb·head의 `crates/`는 같고, 바뀐 것은 mutants.py뿐이다.
  - 이번 head에서 한 번에 다시 돌린 결과도 23개 중 22개 검출로 같다.
  - C15–C18은 각각 I1, X1, X2, X3를 그대로 겨냥한다. C15는 aa38041 원래 질의와 효과가 같은 변이(과거 행 전체)이다.
  - C9b의 검출 시험에 `i1_…`가 추가된 것도 맞다.
  - C4a 동등 판단은 이전 검토와 같다.
- **cargo-test.log.** 머리 커밋은 `73e74e9`이고, 그 커밋과 head의 `crates/`는 같다. 206건은 내가 다시 돌린 수와 같다. "before run" 상태에 로그 파일 자신만 `M`으로 찍혔다. 이는 출력 대상 파일 때문이고 코드의 더러움은 아니다.
- **문서.**
  - §3 I1·I2 행이 변경 내용과 맞다.
  - §2.4 "변이 23개 중 22개", "작업 공간 전체 206건"이 재실행 결과와 맞다.
  - §2.5는 이번 교정이 영속 코디네이터에 배포되지 않았음을 바꾸지 않았다. X-02 전에 재적용이 필요하다는 점은 기존 서술 그대로다.

## 실행한 명령

```
gh pr view 34 --json headRefOid,mergeStateStatus   # f0c4e872a1efe7da3de6411fe0f9b31fbdbdcb19, BLOCKED
gh pr checks 34                                     # 3건 pass
git diff db18e97..f0c4e87; git diff --stat 92473bb f0c4e87 -- crates   # 비어 있음
git fetch -q origin; git merge-tree --write-tree origin/main f0c4e87   # 충돌 없음(main = db18e97)
git worktree add --detach <scratch>/wt34 f0c4e872a1efe7da3de6411fe0f9b31fbdbdcb19
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-r34
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                          # 0
cargo test -q --locked --all-targets                # 0, 206 passed (coordinator 62)
for i in 1..30: cargo test -p ieo-coord --test coordinator i2_     # 30/30 통과
for i in 1..10: cargo test -p ieo-coord --test coordinator         # 10/10 통과
python3.11 -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 22/23, C4a 생존, dirty none
# C18(now()) 수동 적용 후 i2_an_import_is_stamped_… 15회 → 15/15 실패(검출), git checkout으로 복구
# 검토용 임시 시험 zz_probe_a·b·c(저장소에 넣지 않음; <scratch>/r34-probes.diff)
cargo test -q --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
```

정리:
- 임시 시험과 변이는 `git checkout`으로 되돌렸다. 이후 `git status`는 깨끗했다.
- worktree `wt34`와 target `target-r34`는 검토 뒤 삭제했다.
- 시험 DB는 하네스가 만든 임시 DB만 썼다.
- 비밀번호는 출력하거나 기록하지 않았다.
- 영속 DB, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
- 저장소, GitHub, Notion에는 아무것도 쓰지 않았다.
