# PR #30 독립 검토 (Guardrail) · head 9a421b7 · base main 82c7db9

VERDICT: reject

## 요약

- 대상: `fix/x01-c1-c2-notion-gate` head `9a421b7e9b3e025950117ff468d07b207cd08669`. `git ls-remote`로 원격 head와 일치함을 확인했다. base는 main `82c7db9`이다. X-01의 이전 승인과는 별개로, 이 코드를 처음부터 다시 검토했다.
- 다시 돌린 검사는 모두 통과했다. fmt, clippy `-D warnings`, `cargo test --locked --all-targets` 171건, 변이 8/8 검출, validate_package, render_progress `--check`, test_render_progress, openapi-spec-validator.
- C1 수집 도구는 정확하다. 세션 기록에서 직접 다시 계산해 보니 보고서 수치와 같았다. 두 응답 모두 17,203바이트, SHA-256 `0c2c7e82…`이고, 정규화 digest는 `3f8a300f…`이다.
- 허용 경로는 실제로 동작한다. 사용자 예외와 신뢰된 해제 모두 executable·의미 검토·배정이 201이다. 이전(NULL) 행에 예외를 더한 경우도 201이다. 따라서 "항상 거부하는 구현" 문제는 없다.
- 그러나 C2 통제는 `sprint_id` 하나에만 걸려 있다. 그래서 등록자가 같은 Notion 작업을 다른 sprint_id로 등록하면 실행·배정할 수 있다(F1). 이때 차단된 가져오기의 `notion_page` 출처 digest를 그대로 인용해도 막히지 않는다.
- Notion 내용이 이전의 차단 상태로 되돌아가면 재가져오기는 replay가 된다. 그 결과 해제 상태가 그대로 유지된다(F2).
- 보고서는 두 경계를 적지 않았다. 또한 "등록자가 사유를 지워도 실행 불가"를 무조건 성립하는 것처럼 썼다(F3). P1·P2가 있으므로 reject한다.

## 발견 사항

### F1 · P1 · 다른 sprint_id로 등록하면 차단된 가져오기 작업을 실행할 수 있음
- 위치: `crates/ieo-coord/src/api.rs:968-1010`(`guard_import_owned`), `:1015-1070`(`import_gate`), `:1987`(assign).
- 원인: 통제는 `notion_imports.test_id = spec.sprint_id`일 때만 동작한다. 다른 sprint_id를 쓰면 가져오기 행이 없다. 그래서 `guard_import_owned`와 `import_gate`가 모두 `Ok(())`를 돌려준다. 명세의 `sources`에 차단된 가져오기와 같은 `notion_page` digest가 있어도 서버는 보지 않는다.
- 재현(검토용 임시 시험, head 9a421b7, 시험 DB):
  1. `importer`가 `POCX-T01`을 가져온다. queue gates 2건, missing 없음.
  2. `derived_executable(POCX-T01)`을 받아 `sprint_id="POCX-Z02"`, `revision=1`, `tasks[0].task_id="13-01"`로 바꾼다. `notion_page` 출처(차단된 digest)는 그대로 두고 digest를 다시 계산한다.
  3. `alice`가 `POST /v0/sprints?mode=executable`를 보낸다. 결과는 **201**이다. 이어서 `bob`의 의미 검토가 **201**, `assigner`의 배정(`task 13-01`)이 **201**이다.
  4. 작업 ID까지 그대로 두면 409 `TASK_ID_CONFLICT`가 난다. 하지만 이것은 작업 ID 중복 검사 때문이지 C2 통제 때문이 아니다.
  5. 같은 시나리오에서 `notion_page` 출처를 빼고 작업 ID를 `14-01`로 바꾼 경우도 executable **201**이다.
- 기대와 실제:
  - 이슈 B는 등록자가 고칠 수 있는 내용만으로 실행 허가가 나지 않게 하라고 요구한다. 그렇다면 서버가 이미 아는 사실, 즉 "이 명세는 차단 상태로 기록된 Notion 가져오기 페이지·digest에서 왔다"는 사실로 실행을 막아야 한다. 하지만 실제로는 필드 두 개만 고치면 통제를 완전히 우회한다.
  - 최소한 차단된(또는 이전에 가져온) `notion_page` ref·digest를 출처로 인용하는 명세는 막아야 한다. 이런 명세가 가져오기가 소유하지 않은 sprint_id로 executable 등록되면 409를 내야 한다. 또는 같은 `import_gate` 판정을 적용해야 한다.
  - 출처를 뺀 복사본(5번)은 서버가 판별할 수 없다. 이것은 등록자와 독립 의미 검토에 의존하는 범위로 남는다. 그렇다면 보고서가 그렇게 밝혀야 한다(F3).
- 기존 시험과 변이 8개에는 이 경로가 없다.

### F2 · P2 · 내용이 A→B→A로 되돌아가면 다시 차단되지 않음 (replay가 해제 상태를 유지)
- 위치: `crates/ieo-coord/src/api.rs:797-830`(같은 (page, digest)면 replay 200, 새 행 없음), `:1017-1020`(`ORDER BY sprint_revision DESC LIMIT 1`).
- 재현:
  1. `importer`가 A(digest `a…`, gates 2건)를 가져온다. 결과는 201, rev 1이다.
  2. B(digest `b…`, gates 없음, 신뢰된 해제)를 가져온다. 결과는 201, rev 2이다.
  3. A를 다시 가져온다(gates 2건). 결과는 **200 `replay=true`**이고 새 행이 없다.
  4. rev 2에서 파생한 rev 3을 executable로 등록하면 **201**이다. 의미 검토도 **201**, 배정도 **201**이다.
- 기대와 실제:
  - 가져오기 service는 원본이 지금 다시 차단 상태라고 보고했다. 그러므로 최신 판정은 차단이어야 한다.
  - 실제로는 rev 2(해제)가 계속 "최신 가져오기"로 남는다. 그래서 실행과 배정이 허용된다. 응답도 replay라서 가져오기 도구는 차단이 반영되지 않았다는 사실을 알 수 없다.
  - content digest는 속성과 본문만으로 만든다(`crates/ieo-notion/src/lib.rs:120`). 따라서 상태나 차단 대상 속성이 이전 값으로 돌아가면 digest가 같아진다. 상태가 이전 값으로 돌아가는 것은 현실적인 일이다.
- 이 동작은 X-01 R4에서 P3("A→B→A는 replay")로 유지됐다. C2 이후에는 해제 기록이 이 동작에 의존한다. 그래서 다시 차단해야 할 때 그러지 못하는 결함이 됐다.
- 보고서 §2.2는 "최신 가져오기 행을 본다", "gates가 빈 새 가져오기가 해제 기록"이라고만 쓰고 이 예외를 언급하지 않는다.
- 해결 방향: replay이더라도 그 내용이 최신 가져오기가 아니면 관측 기록을 추가해야 한다. 예를 들어 관측 순번 행을 두거나 `last_observed` 표를 쓸 수 있다. 그리고 `import_gate`가 "마지막 관측"을 보게 해야 한다. 또는 최신이 아닌 digest의 replay를 409로 거절해 가져오기 도구가 알게 해야 한다.

### F3 · P2 · 보고서가 서버 강제 범위를 과장하고 잔여 범위를 적지 않음
- 위치: `docs/poc/reports/extra/X-01/followup-c1-c2-20261113/REPORT.md` §2.2·§2.3. 특히 "일반 등록자가 미해결 사유만 삭제 → 실행 가능해지지 않음" 행과 "gates가 빈 새 가져오기가 신뢰된 해제 기록이다" 문장이다.
- 이슈 B는 "서버 강제 범위와 등록자·독립 의미 검토 의존 범위를 재현"하라고 요구한다. 보고서는 같은 sprint_id 경로만 재현했다. 다음 내용이 없다.
  - F1: 다른 sprint_id. 출처를 유지한 경우와 뺀 경우 모두 해당한다.
  - F2: A→B→A 되돌림.
  - 예외의 범위 한계(F4).
- 기대: 서버가 막는 경로와 의미 검토에만 의존하는 경로를 표로 나누어 적어야 한다.

### F4 · P3 · 사용자 예외가 기록 당시의 차단 상태나 범위에 묶이지 않음
- 위치: `api.rs:1051-1057`(EXISTS 조회는 test_id·repo만 본다), `:1102-1205`.
- 재현: T01에 예외를 기록한 뒤 새로운 차단 사유("새 선행 Q9")를 담은 가져오기를 한다. 결과는 201(rev 2)이다. 그런데 파생 executable, 의미 검토, 배정이 모두 **201**이다. 즉 승인 이후에 생긴 새 차단도 기존 예외가 무기한 풀어 준다.
- `scope`(branch, 기준 SHA, merge_to_default=false)는 저장만 된다. 배정의 `target_sha`나 브랜치와 대조하지 않는다.
- 보고서 §2.4는 "범위는 브랜치 …, 기준 SHA …, 기본 브랜치 병합·배포 없음"이라고 쓴다. 이것이 서버가 강제하지 않는 기록 내용이라는 점을 밝혀야 한다.
- 다음 중 하나가 필요하다. 예외를 승인 시점의 gates나 가져오기 revision에 묶는 것, 또는 이 한계를 결정 ③ 범위 해석으로 명시하는 것이다.

### F5 · P3 · OpenAPI issueAssignment 설명에 C2 동작이 없음
- 위치: `contracts/ieo-api-v0.openapi.yaml:491` 이하.
- 409 `IMPORT_GATED`/`IMPORT_INCOMPLETE` 코드는 Problem enum(:756)에 선언돼 있다. 409 응답도 선언돼 있다. 하지만 배정 설명은 이 코드들을 언급하지 않는다. 다시 차단하는 가져오기가 고정된 진행(pinned continuation)의 다음 세대까지 멈춘다는 점도 없다. 검토 시험에서 확인한 결과는 다음과 같다.
  - 예외가 없으면 409 `IMPORT_GATED`이다.
  - 예외가 있으면 201이다.
- X-01 설명은 "a newer draft changes nothing for a running execution"이라고 되어 있다. 이 문장과 C2 동작의 관계를 적어야 한다.

### F6 · P3 · 범위 규칙과 C1 도구의 시험·견고성 공백
- `auth.rs:139-146`의 user·service 전용 범위 거부에는 단위 시험이 없다. 변이 목록에도 없다. 코드는 맞고, `record_exception`이 `Kind::User`를 한 번 더 검사한다.
- `tools/notion_from_transcript.py:28`은 `page_id in json.dumps(input)`로 매칭한다. 그래서 하이픈이 들어간 ID나 URL로 호출한 기록은 찾지 못한다. 이 경우 "no result"로 크게 실패하므로 잘못된 결과가 조용히 나오지는 않는다.
- 세션 기록 자체가 큰 도구 결과를 줄여서 저장하는지는 검사하지 않는다. 연결 도구의 `truncated` 표지만 본다. 이번에는 두 번의 수집이 같고 닫는 태그가 있어 문제가 없다.

## 확인했고 문제없는 항목

- **같은 sprint_id에서 사유 삭제와 문구 주장.** 결과는 409 `IMPORT_GATED`이고 행이 늘지 않는다. draft 등록은 허용된다.
- **convert 경로(`/v0/sprint-inputs`).** 바인딩 sprint_id만 쓰고 draft만 만든다. 가져오기 test_id는 바인딩 sprint_id와 겹칠 수 없다(기존 R1/Y1). executable로 올리는 다른 경로는 `/v0/sprints?mode=executable`뿐이고, 이 경로에서 통제가 동작한다. 단 F1은 예외다.
- **다시 차단하는 가져오기 뒤 이전 executable revision 배정.** 새 배정(gen 0)은 409 `IMPORT_GATED`, 고정된 진행(gen 1)도 409 `IMPORT_GATED`이다. 예외가 있으면 고정된 진행은 201이다. 최신 가져오기만 보는 방식은 새 내용이 들어오는 경우에는 맞다. F2의 replay 경우는 예외다.
- **예외 범위.** T02의 예외로는 T01이 409이다. 다른 repo의 예외는 403(`REPO_SCOPE_DENIED`)이다. 가져온 적 없는 test_id는 422 `UNKNOWN_TASK`이다. 같은 exception_id에 같은 내용이면 200, 다른 내용이면 409이다. `exec_exceptions`는 추가 전용 트리거가 있다.
- **예외를 기록할 수 있는 주체.** alice(등록자), bob, assigner, importer(service)는 403 `SCOPE_DENIED`이다. agent `dev-a`도 403 `SCOPE_DENIED`이다.
- **가져오기를 할 수 있는 주체.** alice, uma(승인자), assigner, bob은 403 `SCOPE_DENIED`이다. agent는 403 `AGENT_READ_DENIED`이다. 해제를 위조하는 경로가 없다. 자격 로더도 user에게 `notion:import`를, user가 아닌 주체에게 `exceptions:approve`를 주지 않는다.
- **이전(NULL) 행.** 가져온 행의 gates와 missing을 NULL로 바꾸면 응답의 `conversion.unresolved_bindings`에서 다시 나눈다. 결과는 409 `IMPORT_GATED`이다. 예외를 더하면 201·201·201이다. 모르는 사유는 missing으로 분류되어 닫힌 실패가 된다(단위 시험과 C6 변이). 응답에 그 키가 없으면 열린 실패가 되지만, 응답은 서버가 쓴 값이라 실질 위험은 낮다.
- **`gates`가 배열이 아닐 때.** 400 `SCHEMA_VIOLATION`이다.
- **이관.** main 82c7db9 스키마로 DB를 만들고 C2 이전 행을 넣은 뒤 `migrate`를 두 번 돌렸다. 둘 다 성공했고 기존 행의 gates·missing은 NULL로 남았다. 기본값 없이 nullable 열을 `ADD COLUMN IF NOT EXISTS`로 추가하면 테이블을 다시 쓰지 않는다. 그래서 UPDATE 트리거가 동작하지 않는다. 새 DB와 이관한 DB의 열 순서는 다르지만, INSERT가 열 이름을 명시하므로 영향이 없다.
- **C1.** 도구는 세션 기록을 읽기만 하고 자격 파일이나 네트워크를 쓰지 않는다. 출력은 0600, 디렉터리는 0700이다. 독립 재계산 결과는 다음과 같다(출력은 검토 뒤 삭제했다).
  - 기록 줄 9704와 10919: 17,203바이트, `0c2c7e822c6cbcb8…`, `page_last_edited_at 2026-10-18T14:08:08.303Z`. truncated 없음, `</content>`·`</page>` 있음.
  - `ieo-notion snapshot` 정규화 digest: 두 건 모두 `3f8a300fcae5d319…`.
  - X-01 사본은 비공개 위치에 있어 직접 대조하지 않았다. X-01 live-check의 digest와 같은 값이다.
- **OpenAPI 0.8.0.**
  - importNotion: scope, 에이전트 403, `gates`·`unresolved` 설명이 코드와 일치한다.
  - recordExecutionException: 필수 필드, `lifts` const, 422 `UNKNOWN_TASK`가 일치한다. 코드가 내는 400 `MISSING_FIELD`·`SCHEMA_VIOLATION`, 403, 409 `EVENT_CONTENT_CONFLICT`, 422가 모두 선언돼 있다.
  - registerSprint의 409 `IMPORT_GATED`/`IMPORT_INCOMPLETE` 설명이 있다. 새 코드는 enum에 선언돼 있다.
- **보고서의 정직성.** 자격 보관의 한계가 적혀 있다. 승인 자격과 가져오기 자격이 같은 호스트·계정에 있다는 점, 서버는 user 자격을 검증할 뿐 사람이 보유했는지는 검증하지 않는다는 점, 이것이 04-07 담당이라는 점이다. live 예외 기록(c2-live.txt)과 이전 행 2건의 NULL 상태(`1|t|t`, `2|t|t`)는 보고서 내용과 일치한다.
- **변이 재실행.** 8/8 검출. 보고서의 목록과 같고, 실행 뒤 작업 트리가 깨끗했다.

## 실행한 명령

```
df -h /                                  # 2.7G → 작업 중 최소 1.8G
git ls-remote origin fix/x01-c1-c2-notion-gate main   # 9a421b7…, 82c7db9…
git worktree add --detach <scratch>/wt30 9a421b7…
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/wt30-target CARGO_PROFILE_DEV_DEBUG=0
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=$(cat <scratch>/pgpw)"
cargo fmt --all -- --check                                  # 0
cargo clippy --locked --all-targets -- -D warnings          # 0
cargo test --locked --all-targets                           # 0, 171 passed
python -I docs/poc/reports/extra/X-01/followup-c1-c2-20261113/evidence/mutants.py   # 8/8 killed, dirty none
python -I docs/poc/scripts/validate_package.py              # rc 0, schema_errors 0
python -I docs/poc/scripts/render_progress.py --check       # ok
python -B docs/poc/scripts/test_render_progress.py          # RESULT PASS
openapi-spec-validator contracts/ieo-api-v0.openapi.yaml    # OK
# 검토용 임시 시험(zz_probe*, 저장소에 넣지 않음): 다른 sprint_id, A→B→A, 이전 revision·pinned, NULL 행,
#   주체별 403, 예외 뒤 새 gate, 이전 스키마 → migrate ×2
cargo test --locked -p ieo-coord --test coordinator zz_probe -- --nocapture --test-threads=1
# C1 재계산: tools/notion_from_transcript.py --all → ieo-notion snapshot --from-connector (출력 삭제)
```

정리: 임시 시험은 되돌렸다. 직접 만든 시험 DB(`ieo_mig_*`)는 삭제했고, 다른 pid의 `ieo_t_*`는 건드리지 않았다. worktree, target, 임시 로그는 삭제했다. 비밀번호와 토큰은 어디에도 기록하지 않았다. 영속 DB `ieo-poc-db`, `ieo_poc-data`, `ieo_poc_secrets`는 사용하지 않았다.
