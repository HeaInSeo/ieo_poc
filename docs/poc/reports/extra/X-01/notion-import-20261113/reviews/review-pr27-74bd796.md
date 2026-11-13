# PR #27 독립 재검토 3 (Guardrail) · head 74bd796 · base main 2c86f3d

VERDICT: approve

## 요약

검토 대상은 X-01이다. 개정 v1.3.3-A3의 추가 작업이며 73·12 완료 수에 넣지 않는다.

- 대상 head는 `74bd7965f5692a6766cf414150e871ab14a8976e`, base는 `2c86f3d`다. `git ls-remote`와 `gh pr view 27`의 값도 같다(OPEN, MERGEABLE).
- 교정분 `git diff 7a48a4f 74bd796`을 읽었다. 별도 detached worktree에서 직접 재현했다.
- 검토 4/4, 교정 3/3을 다 쓴 마지막 검토다.

재현 결과:
- fmt, clippy `-D warnings`, `cargo test --locked --all-targets`가 통과했다. 시험은 165건이다(42+6+29+9+1+33+4+16+25). 시험 DB를 썼고 skip은 없다.
- 증거 변이 표본 8개(X1·X9·X12·X13~X17)는 모두 검출됐다. 실패 시험 목록도 증거 `mutants.txt`와 같다.
- `validate_package.py` EXIT 0, `render_progress.py --check` ok, `test_render_progress.py` PASS, openapi-spec-validator OK다.
- 정책 SHA-256은 `e19a54ad…`로 불변이다.

T1은 끝까지 해소됐다. 임시 probe로 다음 흐름이 실제로 동작함을 확인했다.
1. 가져오기를 한다.
2. 파생 명세를 executable로 등록한다(201).
3. bob이 의미 검토를 통과시킨다(201).
4. 배정이 `issued`로 나온다(201, ceiling 1).

재가져오기 뒤에도 같은 흐름이 동작한다. 그때 이전 가져오기의 작업으로 만든 명세는 409로 거부된다.

새 발견 U1~U5는 모두 P3다. 이 중 U1은 X-02로 넘길 잔여 조건이다. P0~P2는 없다. 따라서 approve다.

## 이전 지적 판정

| 지적 | 처분(주장) | 판정 | 근거 |
| --- | --- | --- | --- |
| R1 P2 비 Notion 스프린트 덮어쓰기, test_id = Notion ID | 409 / 400 | **유지** | `api.rs:833`은 변경이 없다. X9·X15 검출. |
| R2 P2 다른 repo replay | 저장 내용 없이 409 | **유지** | 코드 불변. X1 검출. X10은 이전 검토에서 확인했고 코드가 바뀌지 않았다. |
| R3 P2 pin과 최신 상한 | 상한 적용 | **유지** | 상한 코드는 이번 diff에서 바뀌지 않았다. X12·X16·X17 검출. |
| R4 P3 A→B→A replay | 문서화 | 수용 | 변경 없음. |
| R5 P3 경로별 digest | 문서 | 유지 | 변경 없음. |
| R6 P3 unresolved는 클라이언트 값 | 유지 | **수용, 다만 §6 근거 문장이 낡음(U1)** | REPORT:105의 "서버는 … 실행 가능으로 바꿀 수는 없다"는 가져오기 경로에만 맞는다. 이제는 `/v0/sprints`로 사유를 비우고 승격할 수 있다. |
| R7 P3 큐 상태 원문 | 요약 | 유지 | X-01 디렉터리 전체에서 토큰을 grep했고 0건이다. |
| S1 P2 pin 상한이 draft·미검토 revision에서 옴 | executable·검토 통과 revision만 | **유지** | 새 시험 `only_an_executable_reviewed_newer_revision_sets_a_pinned_ceiling`이 두 조건을 각각 고정한다(X16·X17 검출). |
| S2 P2 가져오기 스프린트 위 다른 경로 등록 | 409 | **작업 단위로는 유지. 스프린트 수준 필드는 열림(U1)** | 작업이나 출처 digest가 다르면 409다. 시험 X13·Z3·Z4로 확인했다. 변환 경로는 예약 규칙으로 막힌다(U3 참조). |
| S3 P3 다른 repo에 존재 노출 | 유지 | 수용 | T5로 출처 문구 노출은 없어졌다. |
| S4 P3 OpenAPI 머리말 | 교정 | 유지 | 머리말 :11-12를 새 동작으로 고쳤다. |
| S5 P3 §2 표현 | 교정 | 유지 | — |
| S6 P3 보관본 토큰 | 가림 | 유지 | b8b619a 보관본 :102의 가림이 그대로 있다. |
| S7 P3 정책 개정 선행 증거 | 사실 기재 | 수용 | digest 불변. |
| **T1** P2 가져오기 스프린트 실행 불가 | 같은 작업·같은 notion 출처 digest의 파생 명세는 `/v0/sprints`로 등록 가능 | **해소** | `api.rs:932-968`. 위 probe로 가져오기 → executable → 검토 → 배정 `issued`, 재가져오기 뒤 새 revision에서도 같음. 미해결 사유가 남은 초안의 승격은 422 `UNBOUND_DRAFT`다(기존 시험과 live-recheck3). X14 검출. |
| **T2** P3 형식보다 소유 409가 먼저 | 소유 검사는 형식 통과·같은 repo 명세에만 적용 | **동작은 해소, 시험은 미고정(U4)** | probe: 작업을 바꾸고 `title`을 뺀 명세 → 400 `MISSING_FIELD`. OpenAPI에 반영됐다. 그러나 시험의 `bad`는 파생 명세라 소유 검사를 통과한다. 변이 Z2(형식 fall-through 제거)가 생존한다. |
| **T3** P3 시험 부재 | 시험 추가, 변환 경로 소유 검사 제거 | **해소** | Y1→X15, Y3→X16, Y4→X17 모두 검출. Y2는 코드를 제거해 대상이 없어졌다(U3). |
| **T4** P3 보관본 토큰 재인용 | 가림 | **해소(형식 사소)** | 원본과 diff한 결과 그 셀만 다르고, 토큰은 0건이다. 7a48a4f 보관본은 검토자 원본과 바이트 단위로 같다. 가림 주석의 표 형식 문제는 U5에 적었다. |
| **T5** P3 다른 repo에 출처 노출 | repo 규칙 먼저 | **동작은 해소, 시험은 미고정(U4)** | probe: carol(repo-b)이 작업을 바꾼 명세를 등록 → 409 `SPRINT_REPO_CONFLICT`, 출처 문구 없음. 그러나 시험의 `theirs`는 파생 명세라 소유 검사를 통과한다. 변이 Z1(repo fall-through 제거)이 생존한다. |

## 새 발견 사항

### U1 · P3 · "파생 명세" 판정은 작업과 출처 digest만 본다. 제목·목표·위임·사람 검증·출처 메타데이터·큐 선행 사유는 바꿀 수 있다. 오류 힌트와 보고서의 의도 문구가 실제 판정보다 좁다

- **위치**:
  - `crates/ieo-coord/src/api.rs:951-957`: `same_source && spec["tasks"] == imported["tasks"]`.
  - `:966`: 힌트 "bind only limits/readiness".
  - REPORT.md:39, :124("한도·준비 상태를 바인딩해 executable로 올리는 경로"), :105(R6 근거).
- **재현**: 시험 DB에서 임시 probe를 실행했고, 실행 뒤 `git checkout`으로 되돌렸다.
  1. `POCX-T01`을 가져온다.
  2. `derived_executable`에서 다음을 바꾸고 digest를 다시 계산한다.
     - `title`을 다른 값으로 바꾼다.
     - `exit_condition`을 바꾼다. 이 값은 Notion의 "목표"에서 온 것이다.
     - `delegation`을 바꾼다: allowed `["anything"]`, forbidden 교체, `approval_scope_ref` 교체. 이 값은 승인 바인딩에서 온 것이다.
     - `human_validation.required`를 false로 바꾼다.
     - notion_page 출처의 `ref`를 `https://evil.invalid/other`로, `revision`을 1999년으로 바꾼다. digest는 유지한다.
     - 임의 출처 1건을 추가한다.
  3. `POST /v0/sprints?mode=executable`을 보낸다. 결과는 **201 executable, revision 2**다.
- 큐 선행 사유도 같다. 가져오기가 같은 digest의 원본에서 만든 "큐 구획·작업 상태·차단 대상 미해제" 사유를 `unresolved_bindings`에서 지우면 승격된다. 그런데 원본은 바뀌지 않았다(같은 digest가 그 증명이다). 따라서 장부 X-01의 "큐 선행·차단 미해제는 … 실행 가능 등록·배정을 거부"는 이제 서버가 아니라 두 가지에만 의존한다.
  - 작성자가 사유를 지우지 않는 것.
  - 독립 의미 검토(bob ≠ alice).
- **기대**: 둘 중 하나가 필요하다.
  - 코드를 좁힌다. 허용 목록(`revision`, `registration_state`, `limits`, `unresolved_bindings`의 한도·바인딩 사유, `target.repository_url`, `digest`) 밖의 필드는 최신 가져오기 revision과 같아야 하게 한다.
  - 아니면 보고서·힌트를 실제 판정("작업과 notion 출처 digest만 고정, 나머지는 작성자 권한, 큐 선행 사유 해제는 사람의 판단과 의미 검토에 의존")에 맞추고, X-02 선행 조건(§5 결정 3)에 명시한다.
- **실제**: OpenAPI(:20-21, :135-140)의 정의 문장 "same tasks and the same `notion_page` source digest"는 코드와 정확히 일치한다. 그러나 힌트와 §8은 "한도·준비만 바인딩"이라는 더 좁은 성질을 시사한다. R6의 근거 문장 "실행 가능으로 바꿀 수는 없다"도 이제 경로 하나에만 참이다.
- **P3로 본 이유**:
  - 실행 단위인 작업(요구 ID, 선행, 산출물, 검증, 수용)은 고정된다.
  - 같은 repo의 `sprints:register` 권한자는 원래 임의 명세를 등록할 수 있다.
  - 배정에는 다른 주체의 의미 검토 통과가 필요하다.
  - X-02는 큐 선행이 남은 실행에 사용자 결정을 요구한다.
  - 따라서 새 권한 상승은 없다. 남는 것은 출처 표기와 실제 내용이 어긋날 수 있다는 문제와 문서 정밀도다.

### U2 · P3 · 소유 검사가 01-07의 revision별 오류 코드와 digest·참조 400을 앞지른다

- **위치**: `api.rs:589`(`register_spec`·`check_registration` 앞), `openapi.yaml:37-38`("format → reference → binding").
- **재현**: 파생 rev 2를 등록한 뒤:
  - (a) 작업을 바꾼 rev 1(이미 낡은 revision) → 409 `EVENT_CONTENT_CONFLICT`. 비 가져오기 스프린트라면 `REVISION_SUPERSEDED`다.
  - (b) 작업을 바꾼 rev 2(같은 revision, 다른 digest) → 409 `EVENT_CONTENT_CONFLICT`.
  - (c) 같은 rev 2에서 제목만 바꾸면 409 `REVISION_CONFLICT`.
  - 같은 상황이 바뀐 필드에 따라 서로 다른 코드를 받는다. digest 불일치나 참조 오류(400)가 있어도 작업이 다르면 409가 먼저 나온다.
- **기대/실제**: 쓰기는 없다. 409 범주도 같다. OpenAPI는 "other content → 409 `EVENT_CONTENT_CONFLICT`"라고만 적고, 검사 순서 문장에는 이 검사의 자리가 없다. 순서 문장에 "format → (가져오기 소유) → reference → …"를 적거나, 소유 검사를 revision 검사 뒤로 옮기기를 권한다.

### U3 · P3 · 변환 경로 소유 거부 제거의 근거가 설정 불변식에 의존한다

- **위치**: `api.rs:632`(주석), `:821-833`.
- **논증**: 예약 검사는 **가져오기 시점의** `ctx.bindings`만 본다. 또 `imported_before`가 참이면 예약 검사를 건너뛴다.
  - 운영자가 나중에 어떤 repo 바인딩의 `sprint_id`를 이미 가져온 시험 ID(예: `POCX-SORI-OC08-T01`)로 바꾸고 재기동한다고 하자.
  - 그러면 `/v0/sprint-inputs`가 그 가져오기 소유 스프린트에 변환 결과 revision을 쓴다. `convert_and_register`는 `guard_import_owned`를 거치지 않는다.
  - 기동 시 바인딩과 `notion_imports.test_id`의 충돌을 검사하는 코드는 없다.
- 재현은 하지 않았다. 바인딩 변경과 재기동이 필요한 논증이다.
- 정상 설정에서는 주장대로 도달할 수 없다. 이전 검토도 이 검사를 "이중 방어로 무방"하다고만 했다.
- **권고**: 기동 시 충돌 검사를 두거나, 변환 경로에서도 `guard_import_owned`와 같은 검사를 하는 편이 안전하다.

### U4 · P3 · T2·T5의 교정과 "최신 가져오기" 선택이 시험으로 고정되지 않았다(변이 생존)

검토자가 추가한 변이 결과다. 시험 전체는 통과하고 변이는 살아남았다.

- **Z1**: repo fall-through 제거(`spec["target"]["repo_id"]… != Some(owner)` → `false`) → 생존.
- **Z2**: 형식 fall-through 제거 → 생존.
  - Z1·Z2의 원인: `an_imported_draft_can_be_promoted_only_with_its_own_tasks_and_source`(coordinator.rs:2594-2605)의 `bad`와 `theirs`가 둘 다 파생 명세(같은 작업·같은 출처)다. 그래서 소유 검사를 통과하고, 원래 400·`SPRINT_REPO_CONFLICT`가 그대로 나온다.
  - 동작 자체는 probe로 확인했다. 작업을 바꾸면 형식 오류는 400 `MISSING_FIELD`, 다른 repo는 409 `SPRINT_REPO_CONFLICT`다. REPORT §8 T2·T5의 "같은 시험"은 과장이다.
- **Z5**: `ORDER BY n.sprint_revision DESC` → `ASC`(최초 가져오기 기준) → 생존.
  - 재가져오기 뒤 이전 작업으로 만든 명세를 거부하는 시험이 없다.
  - 동작은 probe로 맞다. 재가져오기 rev 3 뒤, rev 1 작업으로 만든 rev 4는 409이고 rev 3에서 파생한 rev 4는 201이다.
- Z3(출처 미검사)와 Z4(작업 미검사)는 검출됐다.

### U5 · P3(형식) · T4 가림 주석이 표 밖 초과 셀이라 렌더링되지 않는다

- `reviews/review-pr27-38b671b.md:31`은 `| … |` 뒤에 `[보관 시 생략: …]`를 닫는 파이프 없이 붙였다.
- 이 행은 머리행보다 셀이 하나 많다. GFM은 초과 셀을 버린다. 그래서 GitHub 화면에는 "개발 측이 가렸다"는 고지가 나타나지 않는다. 원문 diff에서만 보인다.
- 가린 토큰 자리의 `[큐 상태 토큰 — 보관 시 생략]` 표기는 보이므로 정직성 문제는 아니다. 고지를 셀 안으로 옮기면 된다.

## 확인했고 문제없는 항목

- **guard 우회(작업 단위)**: 작업 한 필드 변경, notion 출처 제거, 이전 가져오기의 작업 사용은 모두 409이고 행 수가 불변이다(시험과 probe).
- **정상 승격**: 거부되지 않는다. draft 등록(`mode=draft`)도 허용된다.
- **반복 등록과 키 재사용**: 같은 revision의 동일 재제출은 정상 경로다. 같은 키에 다른 본문을 보내면 409 `IDEMPOTENCY_KEY_CONFLICT`다. idempotency 조회는 소유 검사보다 앞이다.
- **잠금**: guard는 `registry` 잠금 안에서 읽는다. 가져오기도 같은 잠금을 쓰므로 경합이 없다.
- **형식 판정 일치**: guard는 `check_registration`과 같은 validator를 쓴다. 그래서 fall-through로 형식 오류가 통과할 수 없다.
- **01-07 상호작용**: 파생 명세도 `check_registration`(digest, 참조, executable 바인딩)과 RG2 repo 규칙을 그대로 거친다. 미해결 사유가 남은 승격은 422 `UNBOUND_DRAFT`다. 동일 rev 재제출 중 비작업 필드 변경은 `REVISION_CONFLICT`다.
- **pin 상한 코드**: 이번 diff에서 바뀌지 않았다. 새 시험이 Y3·Y4를 고정한다.
- **보고서 수치**: 시험 165건, 코디네이터 42건, 변이 17/17, live-recheck3(재조회 200, executable 등록 422 `UNBOUND_DRAFT`, 초안 배정 422, 행 불변)가 재현 결과·코드와 일치한다. §3 #4의 422·400 문구도 시험과 맞다.
- **보관 검토문**: 7a48a4f 보관본은 검토자 원본과 같다. 38b671b 보관본은 그 셀만 다르다.
- **비밀**: diff에 토큰·비밀번호 값은 없다(`<scratchpad>/pgpw` 자리표시만 있다). 영속 DB `ieo-poc-db`와 `ieo_poc-data`는 쓰지 않았다.

## 실행한 명령

```
git ls-remote origin task/x01-notion-import main        # 74bd796…, 2c86f3d…
gh pr view 27 --json headRefOid,baseRefOid,state,mergeable   # OPEN, MERGEABLE, 같은 SHA
df -h /                                                  # 시작 2.4G, 빌드 뒤 1.5G
git worktree add --detach <scratchpad>/g4wt 74bd796
git diff 7a48a4f 74bd796; diff <scratchpad>/review-pr27-{7a48a4f,38b671b}.md vs 보관본
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratchpad>/g4wt/target CARGO_PROFILE_DEV_DEBUG=0
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratchpad>/pgpw)"
cargo fmt --all -- --check                              # 0
cargo clippy --locked --all-targets -- -D warnings      # 0
cargo test --locked --all-targets                       # 0, 165 passed
cargo test -q --locked -p ieo-coord --test coordinator probe_u|probe_v -- --nocapture
    # 임시 probe(U1·U2 재현, 끝까지 가는 승격·배정, 재가져오기, T2/T5 비파생 경우). 실행 뒤 git checkout으로 원복
.venv-poc/bin/python -I <scratchpad>/g4scripts/mut.py
    # evidence/mutants.py 사본: X1,X9,X12,X13–X17 killed 8/8; Z3,Z4 killed; Z1,Z2,Z5 survived; dirty none
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py          # EXIT 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -I docs/poc/scripts/test_render_progress.py      # RESULT PASS 0
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
sha256sum contracts/judgment-policy-v0.json             # e19a54ad…
git worktree remove --force <scratchpad>/g4wt; rm -rf <scratchpad>/g4scripts <scratchpad>/g4-*.log
```
