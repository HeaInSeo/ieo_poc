# PR #27 독립 검토 (Guardrail) · head b8b619a · base main 2c86f3d

VERDICT: reject

## 요약

검토 대상은 X-01(개정 v1.3.3-A3의 추가 작업, 73·12 완료 수 미산입)이다. 브랜치 `task/x01-notion-import` head `b8b619a050dd6ad12ecf7232c65748c53682a269`를 base `2c86f3d`와 비교했다. 별도 detached worktree에서 직접 재현했다.

재현 결과:
- fmt·clippy(-D warnings)·`cargo test --locked --all-targets`가 모두 통과했다. 시험은 156건이다.
- 변이 8개를 모두 검출했다.
- `validate_package.py`(EXIT 0), `render_progress.py --check`, `test_render_progress.py`(PASS), openapi-spec-validator(OK)가 모두 통과했다.
- 대상 repo 브랜치 주장도 일치한다.

기록과 정직성은 양호하다.
- DB 초안 등록을 개발 완료로 쓰지 않았다.
- 모의 원본 변경에는 SIMULATED 표시가 있다.
- 커넥터 스냅샷 경로를 서버 측 Notion 접근으로 포장하지 않았다.
- 비밀값은 없다.
- A3는 기준선·수용 기준을 바꾸지 않는다.

reject 사유는 코디네이터 쪽의 P2 3건이다. 모두 시험 DB 위의 probe 시험으로 재현했다.
- **R1** `test_id`가 Notion 가져오기가 아닌 기존 스프린트 ID와 같아도 거부하지 않는다. 그 스프린트에 Notion 내용의 새 revision을 덧씌운다.
- **R2** 같은 (page, digest, test_id)의 replay가 호출자의 repo 범위를 보지 않는다. 다른 repo 자격에게 저장된 응답(다른 repo의 spec)을 돌려준다.
- **R3** 배정 고정(pin) 예외에 범위 제한이 없다. 대체된 revision에서 새 세대가 계속 발급되고, 그동안 최신 revision의 상한은 무시된다. 배정 계약(OpenAPI issueAssignment)·정책·L08 문서와도 불일치한다.

P3 4건은 권고다.

## 발견 사항

### R1 · P2 · 기존(비 Notion) 스프린트 ID를 test_id로 쓰면 그 스프린트를 덧씌운다

- **위치**: `crates/ieo-coord/src/api.rs:762-793`(충돌 검사가 `notion_imports`만 본다), `:796-803`(`existing`이 `sprint_revisions` 전체에서 번호를 읽는다), `:909`·`:913-916`(자기 스프린트 제외).
- **재현**: probe 시험 `probe_r1`을 `coordinator.rs`에 임시로 붙여 시험 DB `ieo-poc-0203-pg`에서 실행했고, 실행 뒤 되돌렸다.
  1. `/v0/sprint-inputs`로 `HUM-01` rev 1을 만든다. 이것은 바인딩 스프린트이고 Notion과 무관하다.
  2. `POST /v0/imports/notion`에 `test_id: "HUM-01"`과 다른 페이지를 보낸다.
  3. 결과는 **201, revision 2**다. HUM-01의 최신 revision이 Notion 내용으로 바뀌었다.
- **같은 경로의 다른 경우**: 실행 가능·의미 검토 통과 스프린트 `POC-01`을 test_id로 줬을 때는 422 `UNKNOWN_PROFILE_ITEM`이 났다. 이는 시험 바인딩 프로파일에 01-01 항목이 없어서 우연히 막힌 것이다.
  - 프로파일이 맞으면 같은 경로로 draft rev N+1이 등록된다.
  - 그러면 그 스프린트의 새 배정은 `REVISION_SUPERSEDED`로 막힌다.
- **기대**: X-01 수용 기준은 "원본과 구분되는 시험 ID"이고, OpenAPI는 "uses `test_id` as the sprint ID (distinct from the Notion task ID)" 및 "the test sprint"라고 적는다. Notion 가져오기로 만든 적 없는 기존 스프린트 ID는 409로 거부해야 한다. 예를 들어 `sprint_revisions`에 그 ID가 있는데 `notion_imports`에 없으면 거부한다. `test_id == source_task_id`도 거부하는 편이 기준에 맞다. 지금은 서버와 CLI 어디서도 막지 않는다.
- **실제**: 오타 하나로 운영 스프린트가 Notion 내용의 draft로 대체된다. 결과는 201이고 경고는 없다.

### R2 · P2 · replay가 repo 범위를 넘어 다른 repo의 기록을 돌려준다

- **위치**: `api.rs:762-778`. (page_id, content_digest)로 찾은 행에 대해 test_id만 비교하고, 저장된 응답을 그대로 반환한다. 저장 행의 repo·호출자는 보지 않는다.
- **재현**: `probe_r2`
  1. alice(repo-a)가 `POCX-T01`을 가져온다(201).
  2. carol(repo-b 자격만 있음)이 같은 page·digest·test_id에 `input.repo="repo-b"`를 넣어 보낸다.
  3. 결과는 **200 `replay=true`**다. 응답의 `conversion.spec.target.repo_id`는 `"repo-a"`이고, spec 전체와 import 정보가 들어 있다.
  4. 같은 carol이 `GET /v0/sprints/POCX-T01/revisions/1`을 부르면 **404**다.
- **기대**: 다른 repo의 스프린트는 드러내지 않는다. 이는 `api.rs`의 "a sprint of another repo is not revealed" 규칙이고, RG2 범위다. replay는 저장 행의 repo가 호출자 범위 안이고 요청 `input.repo`와 같을 때만 허용해야 한다. 그렇지 않으면 404 또는 409로 답해야 한다. 기존 `/v0/sprint-inputs` replay는 `idempotent_store(who, …)`로 호출자에 묶여 있어 이런 문제가 없다.
- **비고**: 이용하려면 page_id·digest·test_id를 알아야 한다. 하지만 digest는 Notion 본문만 있으면 계산할 수 있다. 같은 이유로 `:780-793`의 409 "belongs to another Notion page"도 다른 repo의 test_id가 있는지 드러낸다(P3 수준).

### R3 · P2 · 배정 pin 예외가 대체된 revision의 새 작업을 무기한 허용하고, 최신 revision의 상한을 무시한다. 계약도 갱신하지 않았다

- **위치**: `api.rs:1678-1699`. pin 조건은 `generation>0`과 "직전 세대가 같은 revision"뿐이다. `:1824`의 상한은 pin된 옛 spec의 `limits`에서 읽는다. `contracts/ieo-api-v0.openapi.yaml:433-434`.
- **재현**: `probe_r3`
  1. `ready_sprint(ceiling 3)`(rev 5)에서 A1 gen 0을 발급한다(201).
  2. rev 6을 executable로 등록한다(201). **`correction_attempts=0`**이다.
  3. A1 gen 1 on rev 5 → **201 issued**.
  4. A1 gen 2 on rev 5 → **201 issued**.
  - 이 PR 전에는 3단계가 409 `REVISION_SUPERSEDED`였다.
- **문제 1 · 범위**: 코디네이터에는 "실행 중" 상태가 없다. head는 판정이 끝난 뒤에도 남는다. 그래서 대체된 revision에서 새 target_sha의 새 세대가 옛 상한까지 계속 발급된다. 01-07 rev 3 "한 스프린트에서 활성인 것은 최신 revision 하나"와 L08 "새 stale 실행 요청 구분"이 02-03 검토(PR #25)에서 "옛 revision 배정 409"로 충족 확인됐다. 이 PR은 그 성질을 결정 기록 없이 넓혔다.
- **문제 2 · 정책**: G5는 "newer revision changes the ceiling (up or down) … a new decision_request"라고 한다. 즉 새 revision의 상한 변경이 다음 세대에 적용된다고 전제한다. pin된 사슬은 새 revision이 상한을 낮추거나 0으로 해도 영향을 받지 않는다.
- **문제 3 · 계약**: issueAssignment 설명은 여전히 "be the latest (409 `REVISION_SUPERSEDED`)"다. 예외는 importNotionTask 설명의 마지막 문장에만 있다. 0.7.0은 "additive"라고 적었지만, 실제로는 배정 의미가 바뀌었다. judgment-policy-v0.json과 01-07·01-08 계약 문서에도 반영이 없다.
- **기대**: X-01 기준 "실행이 시작된 배정은 명세 revision을 고정"은 지키되 아래를 함께 한다.
  - (a) 예외를 issueAssignment 계약·정책(G4/G5)·L08 해석에 명시한다.
  - (b) 상한은 최신 revision의 값(또는 둘 중 작은 값)을 적용한다. 혹은 새 revision 등록 뒤에는 pin 세대 수를 제한한다.
  - (c) 그 정의를 시험한다. 예: 새 revision이 상한을 0으로 낮추면 pin 세대도 `LIMIT_REACHED`.
- **영향받지 않는 부분**: 옛 revision의 재등록·승격, 의미 검토 이월(RG2)은 열리지 않는다. 새 assignment_id는 계속 409다. 다른 revision으로 넘어가는 세대도 불가다(시험과 X2 변이로 확인).

### R4 · P3 · A→B→A 되돌림은 새 revision이 되지 않는다

- **재현**: `probe_r4`. A 201, B 201, A 다시 → **200 replay, rev 1, `SUPERSEDED`(superseded_by 2)**. 최신 revision(B)이 원본(A)과 달라진 채 남고, CLI는 exit 0이다.
- OpenAPI에 "re-evaluated: SUPERSEDED"로 적혀 있어 의도된 동작으로 볼 수 있으므로 P3로 둔다.
- **권고**:
  - CLI가 `SUPERSEDED` replay를 exit 0으로 처리하지 말고 경고한다.
  - 보고서 §2 "내용이 바뀌면 다음 revision"에 이 예외를 적는다.

### R5 · P3 · REST 경로와 커넥터 경로는 같은 페이지라도 digest가 다르다

- **위치**: `crates/ieo-notion/src/lib.rs:115-120`의 문서 주석("the read path … do not")과 보고서 §2("같은 형태로 정규화").
- **문제**:
  - 커넥터 본문에는 Markdown 이스케이프(`\[`, `\_`), 서식, `<table header-row=…>`가 들어간다.
  - `api.rs:render`는 plain_text만 쓴다. 중첩 자식 블록(토글·열 등)과 알 수 없는 블록은 조용히 버린다(`api.rs:124`).
  - 그래서 자격이 발급돼 REST로 처음 읽으면, Notion이 바뀌지 않았어도 새 revision이 생긴다. 일부 내용이 빠질 수도 있다.
- 보고서는 REST 경로를 "모의 서버로만 시험"이라고 정직하게 적었다. 그래서 P3로 둔다.
- **권고**:
  - "경로 무관" 표현을 "같은 경로 안에서"로 고친다.
  - 버리는 블록 종류는 unresolved나 경고로 남긴다.

### R6 · P3 · 미해결 사유와 번호 풀은 클라이언트와 설정에 의존한다

- 서버는 `unresolved`를 클라이언트가 보낸 대로 받는다. 큐 선행이 남은 페이지라도 사유를 비워 보내면 사유 없는 draft가 된다. 실행 차단은 미바인딩 한도와 독립 의미 검토가 맡는다.
- `import_sprint_numbers` 풀은 바인딩의 `sprint_number`나 주계획 스프린트 번호를 제외하지 않는다. 시험 설정에서 11이 겹친다. 겹쳐도 나중 변환이 `TASK_ID_CONFLICT`로 거부되므로 데이터가 오염되지는 않는다.
- **권고**: 보고서 한계에 적고, 바인딩 검사에서 풀과 `sprint_number`가 겹치지 않게 한다.

### R7 · P3 · 보고서 §1에 Notion 큐 관측 내용이 요약돼 있다

보고서 §1에는 다음이 들어 있다.
- 행 상태 토큰 `KEEP_GATED / NOT ACTIVE`, `NEXT_NODEKIT_CHILD_ROW_UNRESOLVED`
- 담당 상황, PR #48 슬롯, 26행 관측

원문 전재는 아니고 선정 근거 요약이다. 그래도 공개 repo이고 지시가 "큐 원문 공개 금지"이므로, 상태 토큰 원문 인용은 줄이기를 권한다. 증거 파일 4개에는 ID·digest·상태 코드·행 수만 있다. 이는 확인했다.

## 확인했고 문제없는 항목

- **dedup·revision**:
  - (page, digest) PK와 append-only 트리거가 있다.
  - 같은 내용은 replay이고 행 수가 불변이다.
  - 변경은 rev+1이고 rev 1 digest는 불변이다.
  - 같은 내용을 다른 test_id로, 또는 다른 페이지를 같은 test_id로 보내면 409다.
  - 모두 시험과 변이 X1·X5로 확인했다.
- **거부 시 롤백**:
  - 번호 풀 소진은 422이고 counts가 불변이다(기존 시험).
  - 잘못된 input(tasks 빈 배열)은 400 SCHEMA_VIOLATION이고 counts가 불변이다(probe_r5).
  - 잘못된 test_id는 400이다.
  - 모든 쓰기는 `run()` 트랜잭션 안에 있다.
- **범위 검사**: 에이전트는 403 `AGENT_READ_DENIED`, 다른 repo 입력은 `REPO_SCOPE_DENIED`다. 단 replay 경로는 R2 참조.
- **draft 강제**: 미해결 사유를 digest 계산 전에 넣고 `registration_state=draft`, `CheckMode::Draft`로 등록한다.
  - executable 승격(400/422)과 ready 전환(400)은 거부된다.
  - draft 배정은 422 `UNBOUND_DRAFT`다.
- **변환 수정(항목 3)**:
  - 자기 스프린트의 이전 revision만 충돌 집합에서 뺀다.
  - 다른 스프린트의 최신 revision task는 계속 충돌로 잡는다. 01-07 `check.rs:302`의 같은 스프린트 제외 규칙과 일치한다.
  - 다른 스프린트와의 ID 재사용은 여전히 `TASK_ID_CONFLICT`다. 잘못된 재사용 경로는 찾지 못했다. 단 R1처럼 test_id 자체가 남의 스프린트 ID면 "자기 스프린트"로 취급된다.
- **pin이 열지 않는 것**: 옛 revision의 신규 등록·승격(01-07 rev 3)은 그대로 409다. 새 assignment_id도 409다. 의미 검토는 (sprint, revision, digest) 단위로 이월되지 않는다(RG2). G1 선행 판정은 최신 정의와 비교한다.
- **토큰**:
  - Notion 토큰은 `Authorization` 헤더에만 쓴다.
  - 오류 메시지는 path·HTTP 코드·Notion `code`·`ErrorKind`만 담고, URL과 토큰은 넣지 않는다.
  - 코디네이터 토큰도 헤더에만 쓴다.
  - 시험은 합성 픽스처만 쓴다.
  - diff에 토큰 패턴(`ntn_`, `secret_…`, `ghp_`, PEM 등)은 없다.
- **증거 파일**: `live-check.txt`, `db-backup-restore.txt`, `target-branch.txt`, `cargo-test.log`에는 ID·digest·상태 코드·행 수만 있다. 비밀 디렉터리는 `<private data dir>`로 가렸다.
- **A3·검산기**:
  - A3에는 `baselines_unchanged` 73/31/12/5가 있고, profile·sprint revision과 changed_acceptance는 비어 있다.
  - 검산기는 A1→A2→A3를 차례로 적용한다.
  - 프로파일 없는 개정은 위 조건을 assert한 뒤에만 건너뛴다. A1·A2 검사는 그대로다.
  - 파일 sha는 나중 개정이 우선이다. 장부·생성기·검산기 sha256이 A3 기재값과 일치한다.
  - `state.extra_tasks` ⊆ 개정 정의다.
  - `state.json`의 X-01·X-02 제목·선행은 A3와 일치한다.
- **진행 화면**:
  - 추가 작업은 별도 표에 나오고, 73·12 완료 수는 10/73 그대로다.
  - `--check` 통과.
  - 생성기 시험에 X-99 검수 완료가 주계획 수를 바꾸지 않음, ID 충돌, 증거 없는 완료 거부가 들어 있다.
- **과대 주장 없음**:
  - "DB 등록 성공은 개발 완료가 아니다"라고 적었고, OC08은 구현되지 않았다고 했다.
  - 모의 변경을 명시했다.
  - 서버에 Notion 자격이 없음과 대화 연결 스냅샷 사용을 명시했다.
  - 모델 호출은 0회다.
- **대상 repo**:
  - `git ls-remote https://github.com/HeaInSeo/sori`에서 `refs/heads/poc/POCX-SORI-OC08-T01`과 `refs/heads/master`가 모두 `2fa7f5e12fa4474b194acf0d6e1a0273666642c1`이다.
  - 보고서의 "기준 SHA 그대로, 추가 커밋 0, PR 없음"과 일치한다.
- **OpenAPI 0.7.0 importNotionTask 본문**: 코드와 일치한다. 확인한 항목은 필수 필드, 패턴(`test_id` `[A-Za-z0-9._-]{1,64}`, digest 64 hex), 오류 코드(400/403/409/422), 번호 풀 422 `UNBOUND_DRAFT`, replay·SUPERSEDED다. issueAssignment 쪽은 R3 참조.
- **시험 수**: 156 = 33+6+29+9+1+33+4+16+25. 보고서·증거와 일치한다.
- **영속 DB `ieo-poc-db`와 비공개 디렉터리**: 존재만 확인했다. 쓰지도 읽지도 않았다.

## 실행한 명령

```
df -h /                                    # 2.4G 여유에서 시작, 빌드 중 디스크 부족으로 첫 변이 실행 무효 → 내 target 삭제 후 debug=0으로 재빌드·재실행
git fetch origin task/x01-notion-import main; git rev-parse …   # b8b619a / 2c86f3d 확인
git worktree add --detach <scratchpad>/wt27 b8b619a…
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratchpad>/t27
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratchpad>/pgpw)"
cargo fmt --all -- --check                 # 0
cargo clippy --locked --all-targets -- -D warnings   # 0
cargo test --locked --all-targets          # 0, 156 passed
python -I docs/poc/reports/extra/X-01/notion-import-20261113/evidence/mutants.py   # (CARGO_INCREMENTAL=0, debug=0) 8/8 killed, 증거 mutants.txt와 같은 실패 시험, dirty none
cargo test -p ieo-coord --test coordinator probe_ -- --nocapture   # 임시 probe 5건(R1~R5), 실행 후 git checkout으로 원복
.venv-poc/bin/python -I docs/poc/scripts/validate_package.py      # EXIT 0 (amendments A1,A2,A3)
.venv-poc/bin/python -I docs/poc/scripts/render_progress.py --check   # ok
.venv-poc/bin/python -I docs/poc/scripts/test_render_progress.py      # RESULT PASS
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml  # OK
sha256sum (장부·render_progress.py·validate_package.py) vs Amendment A3 sha256    # 일치
git ls-remote https://github.com/HeaInSeo/sori refs/heads/poc/POCX-SORI-OC08-T01 refs/heads/master   # 둘 다 2fa7f5e1…
git diff 2c86f3d HEAD | grep (토큰 패턴)  # 없음
git worktree remove --force <scratchpad>/wt27; rm -rf <scratchpad>/t27
```
