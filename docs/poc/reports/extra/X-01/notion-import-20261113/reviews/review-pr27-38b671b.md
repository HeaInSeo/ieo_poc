# PR #27 독립 재검토 (Guardrail) · head 38b671b · base main 2c86f3d

VERDICT: reject

## 요약

검토 대상은 X-01(개정 v1.3.3-A3의 추가 작업, 73·12 완료 수 미산입)이다. 브랜치 `task/x01-notion-import` head `38b671be18df608d4f22e0d856a755204295bb40`를 base `2c86f3d`와 비교했고, 교정분 `git diff b8b619a 38b671b`를 따로 읽었다. `gh pr view 27`의 head·base가 위 값과 같다. 별도 detached worktree에서 직접 재현했다.

재현 결과는 모두 통과다.
- fmt, clippy `-D warnings`, `cargo test --locked --all-targets`: 159건(36+6+29+9+1+33+4+16+25)으로 보고서와 같다.
- 변이 X1·X2·X5·X9·X10·X11과 검토자 추가 변이 Y1(`test_id == source_task_id` 검사 제거)을 돌렸다. 7/7 검출됐고, 실패 시험은 증거 `mutants.txt`와 같다.
- `validate_package.py` EXIT 0, `render_progress.py --check` ok, `test_render_progress.py` PASS, openapi-spec-validator OK.
- 정책 파일 SHA-256 `470914ab…`는 `PINNED_POLICY_DIGEST`와 같다.

R2는 해소됐다. R1은 원래 재현 경로가 막혔다. 하지만 R3 교정에 새 P2가 생겼다(S1). pin된 이어 쓰기가 "최신 revision"의 상한을 따르는데, 그 revision이 의미 검토 전이나 draft여도 그대로 쓴다. 그래서 다음 두 일이 생긴다.
- 검토되지 않은 draft가 상한을 올리면, G5 한도에 도달한 실행이 사용자 결정 없이 계속된다.
- 상한이 null인 draft(바인딩에 한도가 없는 Notion 재가져오기 draft가 이 경우다)가 등록되면, 진행 중인 실행이 422로 멈춘다.

어느 쪽이든 사용자 지시 "이후 Notion 수정은 현재 실행을 자동 변경하지 않도록"과 보고서 §2 "실행 고정" 문장에 어긋난다. 또 R1은 등록 순서를 뒤집으면 여전히 재현된다(S2, P2). 그래서 reject다.

## R1~R7 처분 확인

| 지적 | 주장된 처분 | 판정 | 근거 |
| --- | --- | --- | --- |
| R1 P2 기존 비 Notion 스프린트 덮어쓰기, test_id = Notion ID | 409 / 400 | **부분 해소 (S2 참조)** | `api.rs:818-833`의 순서는 replay → 다른 페이지 test_id → 비 Notion 기존 스프린트 409다. 원래 재현(`/v0/sprint-inputs`로 HUM-01 생성 → import test_id HUM-01)은 409이고 행 수가 불변이다. `api.rs:744-752`의 400도 확인했다. 시험·X9·Y1 검출. 다만 검사가 import 시점에 "그 test_id가 notion_imports에 한 번이라도 있었는가"만 본다. 그래서 import가 먼저 오고 비 Notion 등록이 뒤에 오면 막지 못한다(S2). |
| R2 P2 replay가 다른 repo에 저장 내용 반환 | 저장 내용 없이 409 | **해소** | `api.rs:784-799`. carol(repo-b)이 같은 page·digest·test_id를 보내면 409 `EVENT_CONTENT_CONFLICT`가 나고 본문에 conversion·registration이 없다. 같은 carol의 GET은 404다. 스프린트 repo는 `SPRINT_REPO_CONFLICT`로 고정되므로 `LIMIT 1`도 안전하다. X10 검출. 존재 노출은 S3(P3) 참조. |
| R3 P2 pin 예외가 최신 상한 무시, 문서 미갱신 | 최신 revision의 `correction_attempts` 적용, OpenAPI·정책 0.5.0 | **교정이 새 결함을 만듦 (S1)** | "최신 revision"에 mode·review 조건이 없다. latest == pinned이면 `ceiling_from_latest`가 None이 되어 pinned spec 값을 쓴다. 이것은 맞다. 상한을 낮추는 방향(시험 `a_pinned_continuation_obeys_the_latest_attempt_ceiling`, X11)도 동작한다. 하지만 올리는 방향과 null인 경우가 S1이다. issueAssignment·정책 RG2 문구는 코드와 같다. OpenAPI 머리말의 "0.7.0 (X-01, additive)"는 그대로다(S4). |
| R4 P3 A→B→A replay | 유지, OpenAPI 명시 | **수용** | OpenAPI importNotionTask에 명시돼 있다. CLI 경고는 추가하지 않았다. 문서화된 동작이라 P3 유지로 충분하다. |
| R5 P3 경로별 digest 차이 | 주석 수정 | **부분 (S5 P3)** | `crates/ieo-notion/src/lib.rs:115-119` 주석은 사실대로 고쳐졌다. 그러나 REPORT §2 표(:32 "같은 형태로 정규화", :33 "읽은 경로와 시각은 넣지 않는다")는 그대로다. |
| R6 P3 unresolved 클라이언트 값, 번호 풀 중복 | 유지, 사유 기재 | **수용** | §6에 사유가 있다. 다만 풀·`sprint_number` 중복(11)은 S2의 재현 경로에서 그대로 쓰인다. |
| R7 P3 보고서 §1 큐 상태 원문 | §1 요약으로 교체 | **보고서는 해소, 같은 PR에 재노출 (S6 P3)** | REPORT §1에는 상태 토큰이 없다. 그러나 이번 PR이 보관한 `reviews/review-pr27-b8b619a.md:102-103`에 `[큐 상태 토큰 — 보관 시 생략]`, `[큐 상태 토큰 — 보관 시 생략]`, PR #48 슬롯, 26행이 그대로 있다. | [보관 시 생략: 이 셀의 큐 상태 토큰 인용만 개발 측이 가렸다, 2026-11-13 재검토 2 T4]

## 새 발견 사항

### S1 · P2 · pin된 이어 쓰기의 상한을 검토되지 않은 draft revision이 정한다. 올리면 G5 한도를 넘어 계속되고, null이면 실행이 멈춘다

- **위치**: `crates/ieo-coord/src/api.rs:1719-1750`. `latest_revision`은 mode·review_state와 무관하게 최대 revision을 고른다. 그 spec의 `limits.correction_attempts`를 그대로 `ceiling_from_latest`에 넣는다. `:1876-1879`가 이를 우선 적용한다. `contracts/judgment-policy-v0.json` RG2 예외 문장과 `contracts/ieo-api-v0.openapi.yaml` issueAssignment 설명도 "latest revision"이라고만 적는다.
- **재현**: 시험 DB `ieo-poc-0203-pg`에서 임시 probe를 `coordinator.rs`에 붙여 실행했고, 실행 뒤 `git checkout`으로 되돌렸다.
  - (a) 상한 올리기
    1. `ready_sprint(ceiling 0)`(rev 5, executable, 의미 검토 통과)에서 A1 gen 0을 발급한다(201).
    2. rev 6을 `?mode=draft`로 등록한다. 상한은 5이고 결과는 201 `ACCEPTED_FOR_SEMANTIC_REVIEW`다. 의미 검토는 없다.
    3. A1 gen 1·2·3 on rev 5 → 모두 **201 issued**, 응답 `"ceiling":5`.
    - 대조군: rev 6 없이 gen 1 → 202 `limit_reached`(ceiling 0). 이 PR의 이전 head(b8b619a)도 pinned spec 값 0을 써서 limit_reached였다. 즉 이번 교정이 이 경로를 새로 열었다.
  - (b) 상한 null
    1. `ready_sprint(ceiling 3)`에서 A1 gen 0을 발급한다.
    2. rev 6을 draft로 등록한다. limits는 모두 null이다(바인딩에 `limits`가 없을 때 변환기가 만드는 형태, `ieo-spec/src/convert.rs:239-242`). 결과는 201이다.
    3. A1 gen 1 on rev 5 → **422 `UNBOUND_DRAFT` "the attempt ceiling is not bound"**.
    - 시험 바인딩(repo-a)에는 `limits`가 없다. 그래서 Notion 재가져오기로 생기는 draft는 바로 이 형태다.
- **기대**:
  - 승인 범위(RG2)는 승인된 객체에만 미친다. 의미 검토를 통과하지 않은 draft의 값이 실행 상한을 넓혀서는 안 된다.
  - G5는 "Nothing continues automatically until a user decision is recorded"다.
  - 사용자 지시는 "이후 Notion 수정은 현재 실행을 자동 변경하지 않도록"이다. 보고서 §2 "실행 고정" 행은 "원본이 바뀌어 새 revision이 생겨도 진행 중인 실행은 자동으로 바뀌지 않는다"라고 적는다.
  - 예를 들어 다음 중 하나로 정의하고, 그 정의를 문서와 시험에 넣어야 한다.
    - pinned 값과 최신 값 중 작은 값을 쓴다. 최신 값이 null이면 pinned 값을 쓴다.
    - "최신 executable·검토 통과 revision"의 값만 상한을 바꾼다.
- **실제**: 검토 전 draft 하나로 상한 0이 5가 된다. 반대로 Notion 재가져오기 draft 하나로 진행 중인 배정이 멈춘다. 이 PR의 시험(상한 0으로 낮추는 rev 6)도 의미 검토 전 revision의 값을 쓰는 경우라, 이 성질을 오히려 굳힌다.

### S2 · P2 · R1 우회: Notion 가져오기가 먼저 test_id를 차지하면, 비 Notion 경로가 그 위에 revision을 쌓는다. 이후 재가져오기가 그것을 다시 덮는다

- **위치**: `api.rs:818-833`. `imported_before`가 참이면 latest revision이 누가 만든 것이든 통과한다. 반대 방향인 `/v0/sprint-inputs`·`/v0/sprints`는 해당 sprint_id가 Notion 시험 스프린트인지 보지 않는다(`convert_and_register`는 자기 스프린트를 충돌 집합에서 뺀다).
- **재현**: 임시 probe `probe_s1_order`.
  1. `POST /v0/imports/notion` test_id `HUM-01`(repo-a 바인딩 sprint_id, 아직 미사용) → **201 rev 1**. 풀의 첫 번호 11은 바인딩 `sprint_number`와 같다(R6).
  2. alice `POST /v0/sprint-inputs` → **201 rev 2**. 최신 revision의 sources는 `human_input`뿐이다.
  3. 같은 페이지의 바뀐 내용으로 재가져오기 → **201 rev 3**. 사람 입력 스프린트가 다시 Notion draft로 바뀐다(sources = human_input + notion_page).
  - 보조 probe: Notion import `POC-01` 다음 `/v0/sprints` executable rev 2도 201이었다. 그 뒤 재가져오기는 시험 프로파일에 `01-01.accept`가 없어 우연히 422 `UNKNOWN_PROFILE_ITEM`이었다(이전 R1의 `POC-01` 경우와 같은 우연).
- **기대**: X-01 기준 "원본과 구분되는 시험 ID"와 OpenAPI 0.7.0 "the test sprint"에 따르면, Notion 시험 스프린트와 사람 입력·운영 스프린트는 섞이지 않아야 한다. 방법은 둘 중 하나다.
  - 가져오기 시 test_id가 어느 바인딩의 `sprint_id`와도 같으면 거부한다.
  - 비 Notion 등록 경로가 Notion 시험 스프린트 ID에 revision을 더하는 것을 거부한다. 또는 `imported_before`일 때 latest revision이 Notion 가져오기 revision인지 확인한다.
- **실제**: 순서만 바꾸면 R1과 같은 혼합이 201로, 경고 없이 일어난다. 운영자가 test_id를 고를 때 하는 실수 한 번이 원인이라는 점도 R1과 같다.

### S3 · P3 · 가져오기 거부가 다른 repo의 존재를 알려 준다

- **위치**: `api.rs:772-783`(다른 test_id 409가 repo 검사보다 앞), `:784-799`(R2 409), `:818-833`(R1 409).
- **재현**: carol(repo-b, 바인딩 없음)이 보냈다.
  - 같은 page·digest·다른 test_id → 409 "imported under another test_id"
  - 같은 test_id → 409 "imported for another repo"
  - test_id `POC-01`(repo-a의 비 Notion 스프린트) → 409 "names an existing sprint that was not created by a Notion import"
  - 같은 carol의 GET은 404다.
  - b8b619a에서 마지막 경우는 바인딩이 없어 422 `UNBOUND_DRAFT`였다. 지금은 409로 존재와 출처를 알려 준다.
- **평가**: sprint_id 존재 노출은 `/v0/sprints`의 `SPRINT_REPO_CONFLICT`로 이미 설계상 허용돼 있다(정책 RG2). 내용 유출은 막혔으므로 P3다. 권고: repo·바인딩·범위 검사를 이 세 검사보다 앞에 두거나, 다른 repo 경우를 하나의 일반 문구로 합친다.

### S4 · P3 · OpenAPI 0.7.0 머리말이 여전히 "additive"다

- **위치**: `contracts/ieo-api-v0.openapi.yaml:10`.
- **문제**: issueAssignment는 이제 "be the latest" 예외(pin)와 상한 출처 변경을 적는다. 배정 의미가 바뀐 것인데, 머리말은 "0.7.0 (X-01, additive): Notion task import"만 적는다. 이것은 이전 검토 R3 문제 3의 남은 부분이다.
- 설명 문구 자체(test_id 400·409, replay repo 한정, A→B→A, 번호 풀 422)는 코드와 일치한다. 확인 순서 문구(403/409 → 400 → 409 → 422)와도 맞는다.

### S5 · P3 · REPORT §2가 여전히 경로 무관 digest를 암시한다

- **위치**: REPORT.md:32-33("같은 형태로 정규화", "읽은 경로와 시각은 넣지 않는다").
- **문제**: R5 처분은 주석만 고쳤다. 보고서 본문도 "같은 경로 안에서만 같은 digest, 경로가 바뀌면 새 revision"으로 맞춰야 한다.

### S6 · P3 · R7 처분 뒤에도 같은 PR의 보관 검토문에 큐 상태 토큰이 남아 있다

- **위치**: `docs/poc/reports/extra/X-01/notion-import-20261113/reviews/review-pr27-b8b619a.md:102-103`.
- **문제**: R7의 목적(공개 repo에서 큐 상태 원문 인용 축소)은 보관본에서 다시 무너진다. b8b619a 커밋 이력에도 남는다.
- **권고**: 보관본은 원문 보존이 원칙이므로, 해당 두 줄만 "[큐 상태 토큰 생략 — R7]"로 가리는 편집 주석을 둘지 사용자에게 확인한다.

### S7 · P3 · 정책 0.5.0으로 바뀌어 0.4.0에서 기록된 판정이 G1 선행 증거로 인정되지 않는다. 이 사실이 기록에 없다

- **위치**: `api.rs`의 G1 SQL이 `j.policy_digest = ctx.policy.digest`로 비교한다. 정책 digest는 7b54ae55에서 470914ab로 바뀌었다.
- **영향**: 영속 DB의 재확인 기록에는 배정 0건이 있어 현재 영향은 없다. 그러나 정책 판 올림이 선행 증거를 무효로 만든다는 점을 REPORT §6 또는 정책 변경 사유에 한 줄 적어야 한다. 02-03 기록(STATUS·TASKS의 "0.4.0, digest 7b54ae55")은 당시 기록이므로 바꿀 필요 없다.

## 확인했고 문제없는 항목

- **R2 replay 범위**: 같은 repo의 replay는 200이고 `reevaluate`(SUPERSEDED)가 동작한다. 다른 repo는 저장 내용 없이 409다. 시험 `an_import_replay_answers_only_within_its_repo`와 X10으로 확인했다.
- **R1 원 경로**: 비 Notion 스프린트 HUM-01을 먼저 만든 뒤 가져오기를 보내면 409이고 counts가 불변이다. test_id = source_task_id이면 400이다. 이 검사는 replay·DB 조회보다 먼저 일어나므로, 같은 페이지 replay도 이 경우 400이 된다. 문서와도 맞다.
- **R3 기본 성질**: latest == pinned이면 pinned spec 값을 쓴다. 새 assignment_id와 다른 revision으로 넘어가는 세대는 여전히 409 `REVISION_SUPERSEDED`·`REPORT_SEMANTIC_VIOLATION`이다(X2). 상한을 낮추는 방향은 202 `limit_reached` 뒤 409 `LIMIT_REACHED`다(X11).
- **정책 0.5.0**:
  - `version` 0.5.0이 `POLICY_VERSION`과 같다.
  - SHA-256 `470914abbf5f…a37a0`이 `PINNED_POLICY_DIGEST`와 같고, 재계산 시험도 통과한다.
  - 규칙 7개 집합은 그대로다.
  - 바뀐 것은 RG2 문장 끝의 예외 한 문장뿐이고, 근거(사용자 지시 2026-11-13)를 적었다. 코드와 문구가 일치한다. S1은 문구가 가리키는 대상 자체의 문제다.
- **증거**: `live-recheck.txt`에는 health digest, 상태 코드, 행 수만 있다. digest가 470914ab로 새 정책과 같다.
- **진행 화면·검산기**: 10/73 그대로이고 추가 작업은 별도 표에 있다. `validate_package.py` EXIT 0.
- **대상 repo**: `git ls-remote https://github.com/HeaInSeo/sori`에서 `poc/POCX-SORI-OC08-T01` = `master` = `2fa7f5e1…`이다. 보고서와 일치한다.
- **비밀**: diff에서 토큰 패턴은 보관 검토문의 설명 문구에만 나온다. 실제 값은 없다.
- **영속 DB `ieo-poc-db`와 `ieo_poc-data`**: 사용하지 않았다.

## 실행한 명령

```
git fetch origin; git rev-parse origin/main (2c86f3d); git ls-remote origin task/x01-notion-import (38b671b); gh pr view 27 --json headRefOid,baseRefOid,state
df -h /   # 시작 2.4G, 빌드 중 최소 1.6G, 정리 후 2.4G
git worktree add --detach <scratchpad>/wt-r27 38b671b…
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratchpad>/t27r CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratchpad>/pgpw)"
cargo fmt --all -- --check                               # 0
cargo clippy --locked --all-targets -- -D warnings       # 0
cargo test --locked --all-targets                        # 0, 159 passed
cargo test --locked -p ieo-coord --test coordinator probe_ -- --nocapture --test-threads=1   # 임시 probe(S1a/b·S2·S3), 실행 뒤 git checkout으로 원복
.venv-poc/bin/python -I <scratchpad>/r27b-scripts/mut_subset.py   # evidence/mutants.py 사본에서 X1,X2,X5,X9,X10,X11 + Y1만 실행: 7/7 killed, dirty none
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py          # EXIT 0
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -I docs/poc/scripts/test_render_progress.py      # RESULT PASS
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml   # OK
sha256sum contracts/judgment-policy-v0.json              # 470914ab… == PINNED_POLICY_DIGEST
git ls-remote https://github.com/HeaInSeo/sori refs/heads/poc/POCX-SORI-OC08-T01 refs/heads/master   # 둘 다 2fa7f5e1…
grep (큐 상태 토큰, 토큰 패턴) docs/ 및 diff
git worktree remove --force <scratchpad>/wt-r27; rm -rf <scratchpad>/t27r <scratchpad>/r27b-scripts
```
