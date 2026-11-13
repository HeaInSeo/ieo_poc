# PR #27 독립 재검토 2 (Guardrail) · head 7a48a4f · base main 2c86f3d

VERDICT: reject

## 요약

검토 대상은 X-01(개정 v1.3.3-A3의 추가 작업, 73·12 완료 수 미산입)이다. 브랜치 `task/x01-notion-import` head `7a48a4f625e5739b2a497b32b6ca4ac583cae4a9`를 base `2c86f3d`와 비교했다. 교정분 `git diff 38b671b 7a48a4f`는 따로 읽었다. `gh pr view 27`의 head·base도 위 값과 같다(OPEN, MERGEABLE). 별도 detached worktree에서 직접 재현했다.

재현 결과는 모두 통과다.
- fmt, clippy `-D warnings`, `cargo test --locked --all-targets`: 162건(39+6+29+9+1+33+4+16+25). 보고서와 같다.
- 변이 X1·X2·X5·X9·X11·X12·X13: 7/7 검출. 실패 시험 목록도 증거 `mutants.txt`와 같다.
- `validate_package.py` EXIT 0, `render_progress.py --check` ok, `test_render_progress.py` PASS, openapi-spec-validator OK.
- 정책 파일 SHA-256 `e19a54ad…e8e41`은 `PINNED_POLICY_DIGEST`와 같다.

S1은 해소됐다. 상한 계산의 경계 사례도 probe로 확인했고 모두 의도대로 동작한다. S2도 혼합 경로 자체는 닫혔다. 그러나 S2 교정이 지나치게 넓다(T1, P2). 가져오기 경로는 항상 draft만 등록한다. 그런데 이제 `/v0/sprints`는 같은 revision을 executable로 승격하는 요청까지 거부한다. 그래서 Notion 가져오기로 만든 시험 스프린트는 어떤 API 경로로도 실행 가능 상태가 될 수 없다. 이 결과는 다음과 어긋나며, 보고서·계약 어디에도 밝혀져 있지 않다.
- 사용자 지시: "선행조건이 남은 후보는 초안 등록과 실행 가능 여부를 구분", "실행이 시작되면 명세 revision을 고정".
- 장부 X-01 수용 기준(실행 고정)과 X-02 정의("X-01로 등록한 시험 작업의 고정 명세로 … 개발 세션").
- 보고서 §5의 X-02 계획("고정 명세 `POCX-SORI-OC08-T01`로 … 기동").

나머지 새 발견은 P3다. 그래서 reject다. 다만 T1은 문서만 고쳐도 해소할 수 있다(아래 기대 (b)). 남은 교정 1회 안에서 끝낼 수 있는 범위다.

## R1~R7·S1~S7 판정

| 지적 | 주장된 처분 | 판정 | 근거 |
| --- | --- | --- | --- |
| R1 P2 비 Notion 스프린트 덮어쓰기, test_id = Notion ID | 409 / 400 | **유지(해소)** | `api.rs:821-841`. 원 경로(HUM-01을 먼저 만든 뒤 가져오기)는 409이고 행 수가 불변이다. 400 검사(`:745-753`)도 그대로다. X9(재조준판) 검출. |
| R2 P2 다른 repo replay | 저장 내용 없이 409 | **유지(해소)** | `api.rs:786-801` 변경 없음. X1 검출. |
| R3 P2 pin 예외와 최신 상한 | 상한 적용 | **유지(S1로 정밀화)** | 낮추는 방향 시험·X11 검출. |
| R4 P3 A→B→A replay | 문서화 | 수용 | 변경 없음. |
| R5 P3 경로별 digest | 주석·보고서 | **해소** | REPORT §2(:32-33)가 이제 "경로가 바뀌면 digest가 달라 새 revision"이라고 적는다(S5). |
| R6 P3 unresolved·번호 풀 | 유지 | 수용 | 변경 없음. |
| R7 P3 큐 상태 원문 | §1 요약 | **보고서는 해소, 보관본에 재노출(T4)** | S6 참조. |
| S1 P2 pin 상한이 draft·미검토 revision에서 옴 | executable·의미 검토 통과한 더 새 revision의 상한, 없으면 자기 revision 값 | **해소** | `api.rs:1772-1784`. probe로 확인: (a) 상한 5인 draft rev 6 → gen 1은 202 `limit_reached`(ceiling 0). (b) null 상한 draft → 201(자기 상한 3). (c) 여러 개: rev 6 executable·통과 c5, rev 7 executable·**기각** c0 → gen 1은 201 ceiling 5(기각된 rev 7 무시). rev 8 executable·**검토 대기** c1 → gen 2는 201 ceiling 5(대기 무시). (d) pinned 상한 null은 pinned revision이 executable·통과여야 하므로 바인딩 검사상 생길 수 없다. 생기더라도 이전과 같이 422 `UNBOUND_DRAFT`다. 새 revision이 생기면 이전 revision은 의미 검토를 받을 수 없다(`:1166`). 그래서 대기 상태가 나중에 바뀌어 상한이 뒤늦게 달라지는 경우도 없다. 정책 0.6.0 RG2 문구, OpenAPI issueAssignment 문구, 코드가 일치한다. X12 검출. 다만 조건 두 개 각각은 시험되지 않는다(T3). |
| S2 P2 역순 R1 우회 | 가져오기 소유 스프린트는 다른 경로 409, 바인딩 sprint_id 예약 | **혼합 경로는 해소, 교정이 정상 흐름을 막음(T1)** | `refuse_import_owned`(`api.rs:928-945`)는 register에서 idempotent replay 뒤·`register_spec` 앞(`:589`)에, convert에서는 replay 뒤(`:633`)에 있다. 재현: 가져오기 POCX-T01 → `/v0/sprints` executable·draft 둘 다 409, 행 수 불변. 예약 검사(`:827-833`): 시험 ID = 바인딩 `sprint_id`(HUM-01, 스프린트 미생성)이면 409이고 행 수 불변이다. replay 배치는 안전하다. 가져오기는 이미 revision이 있는 sprint_id를 쓸 수 없으므로, 이전 비 가져오기 등록의 replay는 소유 검사를 만나지 않는다. probe에서도 비 가져오기 등록 201 → 같은 키 replay 200이다. replay는 쓰기가 없으므로 우회 수단도 아니다. X13 검출. 예약 검사와 convert 쪽 검사는 시험되지 않는다(T3). |
| S3 P3 다른 repo에 존재 노출 | 유지(사유 기재) | 수용 | 사유가 타당하다. 새 409 문구가 출처("created by a Notion import")를 다른 repo 호출자에게도 알려 주는 점은 같은 범주다(T5). |
| S4 P3 OpenAPI "additive" | 교정 | **해소** | `openapi.yaml:10-12`가 issueAssignment 동작 변경과 등록 거부를 명시한다. |
| S5 P3 §2 경로 무관 표현 | 교정 | **해소** | REPORT :32-33. |
| S6 P3 보관 검토문의 큐 상태 토큰 | 한 줄 가림 + 가림 주석 | **부분(T4)** | 가림 주석은 정직하다. 개발 측이 인용만 가렸고 근거·날짜·검토 ID를 적었다. 원본과 비교한 diff는 그 한 줄뿐이다. 지적 내용(R7 P3, 원문 전재 아님, 인용 축소 권고)은 그대로이고, 검토자 판정도 바뀌지 않았다. 그러나 이번에 보관한 `review-pr27-38b671b.md:31`이 같은 두 토큰을 다시 인용한다. |
| S7 P3 정책 개정과 G1 선행 증거 | 사실 기재 | 수용 | REPORT §7에 적혀 있다. 0.6.0 판 올림으로 digest가 다시 바뀌었다(470914ab → e19a54ad). 영속 DB의 판정·배정은 0건이라 지금은 영향이 없다. |

## 새 발견 사항

### T1 · P2 · Notion 가져오기 스프린트가 영구히 실행 불가가 됐다(같은 revision의 executable 승격까지 409). 보고서·계약·장부의 X-02 전제와 어긋나며, 이 사실이 밝혀져 있지 않다

- **위치**:
  - `crates/ieo-coord/src/api.rs:589`: register가 sprint_id만 보고 무조건 거부한다.
  - `:1054`(convert_and_register): 가져오기는 항상 `CheckMode::Draft`로 등록한다.
  - `contracts/ieo-api-v0.openapi.yaml:136-137`.
  - REPORT.md:39(§2 실행 고정), :50(§3 #4), :64, §5 X-02 행.
  - 시험 `an_imported_draft_with_unresolved_reasons_cannot_execute`. 이 시험은 이전에는 "미해결 사유 때문에 승격 거부(400/422)"를 확인했다. 지금은 "가져오기 소유라서 409"로 바뀌었다.
- **재현**: 시험 DB에서 임시 probe를 실행했고, 실행 뒤 `git checkout`으로 되돌렸다.
  1. `POST /v0/imports/notion` test_id POCX-T01 → 201, mode `draft`.
  2. 응답의 spec을 실행 가능 형태로 만든다. `registration_state=ready`, 한도 바인딩, `unresolved_bindings=[]`, rev 2, digest 재계산.
  3. `POST /v0/sprints?mode=executable` → **409 `EVENT_CONTENT_CONFLICT`**. `mode=draft`도 409다.
  - 가져오기 경로 자체도 draft만 만든다. 그래서 원본의 선행이 해제되고 바인딩에 한도가 생겨도, 그 시험 스프린트를 executable로 등록할 길이 없다. `live-recheck2.txt`의 영속 DB 409도 같은 사실이다.
  - 실행하려면 다른 sprint_id로 복사해야 한다(시험의 `COPY-OF-IMPORT`처럼). 그러면 "원본에 연결"된 시험 ID와 실행 객체가 분리된다. 또 "재가져오기가 실행을 바꾸지 않는다"는 고정 성질도 사소하게만 참이 된다.
- **기대**:
  - 사용자 지시는 "필수 정보가 부족하면 … 실행 가능 상태로 올리지 마세요"이다. 반대로 부족하지 않으면 올릴 수 있다는 뜻이다. 장부 X-01도 "필수 정보 부족과 큐 선행·차단 미해제는 … 실행 가능 등록·배정을 거부"라고 조건부로 적는다.
  - X-02와 REPORT §5는 X-01이 등록한 시험 ID의 고정 명세로 배정·세션을 연다고 전제한다.
  - S2가 요구한 것은 "다른 경로가 새 revision을 덧쌓아 출처를 섞지 않는 것"이었다. 가져오기가 만든 revision 그대로를 승격하는 것까지 막으라는 요구는 아니었다.
  - 다음 중 하나가 필요하다.
    - (a) 가져오기 소유 스프린트에 한해, 최신 가져오기 revision과 같은 내용의 executable 승격은 허용한다. 예: sprint_id·revision·digest가 `notion_imports`의 최신 행과 일치할 때. 바인딩 검사는 그대로 적용하고 시험을 추가한다. 또는 가져오기 요청에 executable 승격 수단을 둔다.
    - (b) X-01 범위에서는 가져오기 스프린트가 실행 불가임을 결정으로 명시한다. 이때 REPORT §2·§3 #4·§5, OpenAPI importNotionTask, 장부 X-02 행(선행에 "승격 경로 구현" 추가)에 적고, 사용자 결정 사항으로 올린다.
- **실제**: 거부는 fail-closed라 안전성 결함은 아니다. 그러나 X-01의 "실행 고정" 수용 기준이 Notion 작업에는 도달할 수 없는 상태로만 충족된다. 보고서 §2 실행 고정 행은 그 성질이 실제로 동작하는 것처럼 적고, §5는 실행할 수 없는 계획을 제시한다.

### T2 · P3 · 등록 거부의 문서·순서

- `openapi.yaml:34-53`(registerSprint)과 `:85-90`(convertHumanInput) 설명에 가져오기 소유 409가 없다. 머리말과 importNotionTask에만 있다.
- registerSprint는 "Checks run in the 01-07 order (format → …)"라고 적는다. 그런데 소유 검사가 형식 검사보다 앞이다. probe: 본문이 `{"sprint_id":"POCX-T01","target":{"repo_id":"repo-a"}}`뿐인 요청 → 400이 아니라 409다.
- REPORT :50 "executable 승격과 ready 전환도 거부(400/422)"는 현재 시험(409, ready 전환은 다른 ID의 복사본)과 맞지 않는다.

### T3 · P3 · 교정 조건 일부가 시험으로 고정되지 않음(변이 생존)

검토자 추가 변이로 확인했다. 시험 전체는 통과하고 변이는 살아남았다.
- Y1 예약 검사 제거(`(reserved || …)` → `(false || …)`) → 생존. §7은 `a_test_id_cannot_take_over_…`를 근거로 들지만, 그 시험은 HUM-01을 먼저 만들어 두므로 예약 검사를 거치지 않는다.
- Y2 convert의 `refuse_import_owned` 제거 → 생존. 예약 검사 때문에 정상 경로로는 도달할 수 없다. 이중 방어로는 무방하다.
- Y3 상한 질의에서 `review_state` 조건 제거 → 생존. "executable이지만 기각·검토 대기"인 더 새 revision의 시험이 없다. 동작 자체는 probe (c)로 맞다.
- Y4 `mode` 조건 제거 → 생존. "draft이지만 의미 검토 통과" 경우의 시험이 없다.

### T4 · P3 · 이번에 보관한 `review-pr27-38b671b.md:31`이 S6에서 가린 큐 상태 토큰 2건을 다시 인용한다

- b8b619a 보관본에서 가린 바로 그 두 토큰이 38b671b 보관본의 R7 행에 남아 있다. 공개 repo에 다시 노출된다. 원인은 검토자 문장이다.
- 같은 방식의 가림 주석으로 처리하면 된다. 이번 검토문을 보관할 때에도 같은 원칙을 적용해야 한다. 이 검토문에는 토큰 원문을 적지 않았다.

### T5 · P3 · 새 409 문구가 다른 repo 호출자에게 출처를 알려 준다

- carol(repo-b)이 `/v0/sprints`로 가져오기 소유 sprint_id를 등록하면, `SPRINT_REPO_CONFLICT`보다 먼저 "created by a Notion import" 409가 난다.
- 존재 노출은 S3 처분(전역 이름 공간)의 범위 안이다. 출처 노출은 한 단계 더 나간다. repo 검사를 소유 검사보다 앞에 두거나 문구를 일반화하기를 권한다.

## 확인했고 문제없는 항목

- **상한 질의**: `revision > pinned`, `mode='executable'`, `review_state='semantic_review_passed'`, 최신순 1건이다.
  - pinned 자신은 이미 executable·통과이므로 `>`와 `>=`는 동치다.
  - latest == pinned이면 이 분기에 들어가지 않고 자기 값을 쓴다.
  - 새 assignment_id로 rev 5 배정 → 409 `REVISION_SUPERSEDED`. pin 범위는 넓어지지 않았다.
  - `let _ = latest;`는 무의미한 줄이지만 해는 없다.
- **정책 0.6.0**: `version`·`POLICY_VERSION`·digest pin이 일치하고 재계산 시험도 통과한다. 규칙 7개 집합은 불변이다. 바뀐 것은 RG2 예외 문장뿐이며 코드와 같다. OpenAPI `info.version` 0.7.0과 머리말도 맞다.
- **replay와 소유 검사의 순서**: 정상 replay를 깨지 않는다. replay로 우회할 수도 없다(쓰기가 없다).
- **증거**: `cargo-test.log` 162건, `mutants.txt` 13/13, `live-recheck2.txt`(health digest e19a54ad, 상태 코드·행 수만 기록)는 재현과 일치한다.
- **보관 검토문**: 38b671b 보관본은 검토자 원본과 바이트 단위로 같다. b8b619a 보관본은 가림 한 줄만 다르다.
- **비밀**: diff에 토큰·비밀번호 값은 없다. 영속 DB `ieo-poc-db`와 `ieo_poc-data`는 사용하지 않았다.

## 실행한 명령

```
git fetch origin; git ls-remote origin task/x01-notion-import main   # 7a48a4f…, 2c86f3d…
gh pr view 27 --json headRefOid,baseRefOid,state,mergeable          # OPEN, MERGEABLE, 같은 SHA
df -h /   # 시작 2.4G, 빌드 중 최소 1.7G, 정리 후 2.4G
git worktree add --detach <scratchpad>/wt3 7a48a4f…
git diff 38b671b 7a48a4f (crates, contracts, docs); diff <scratchpad>/review-pr27-{b8b619a,38b671b}.md vs 보관본
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratchpad>/t27c CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratchpad>/pgpw)"
cargo fmt --all -- --check                               # 0
cargo clippy -q --locked --all-targets -- -D warnings    # 0
cargo test --locked --all-targets                        # 0, 162 passed (coordinator 39, DB 사용·skip 없음)
cargo test -q --locked -p ieo-coord --test coordinator probe_t -- --nocapture --test-threads=1
    # 임시 probe 2건(상한 경계 a–d, 예약·승격·replay·형식 순서). 실행 뒤 git checkout으로 원복
.venv-poc/bin/python -I <scratchpad>/r27c-scripts/mut.py
    # evidence/mutants.py 사본: X1,X2,X5,X9,X11,X12,X13 killed 7/7; Y1–Y4 survived; dirty none
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py          # EXIT 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -I docs/poc/scripts/test_render_progress.py      # RESULT PASS 0
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
sha256sum contracts/judgment-policy-v0.json              # e19a54ad… == PINNED_POLICY_DIGEST
git worktree remove --force <scratchpad>/wt3; rm -rf <scratchpad>/t27c <scratchpad>/r27c-scripts (+ 로그)
```
