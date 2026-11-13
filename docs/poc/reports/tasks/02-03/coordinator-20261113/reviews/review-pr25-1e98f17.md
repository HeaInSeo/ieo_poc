# PR #25 독립 검토 (Guardrail) · head 1e98f17 · base main c501a44

VERDICT: reject

- 검토 대상: `task/02-03-coordinator-db-api` 1e98f17. 코드 커밋은 0850eaf와 3e24286(시험만 추가)이다. 1e98f17은 보고·증거만 더한다(`git diff 3e24286 1e98f17`에 crates·contracts 변경 없음).
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다. 검토용 detached worktree(`<scratch>/rv-0203`)에서 임시 probe 시험을 추가해 실행했고, `git checkout`으로 되돌린 뒤 `worktree remove --force`로 지웠다.
- 시험 DB는 `ieo-poc-0203-pg`(127.0.0.1:55432)만 썼다. 시험마다 자기 DB를 만들고 지운다. 다른 컨테이너와 설정은 건드리지 않았다.

## 요약

- 재현 결과는 보고와 일치한다.
  - fmt, clippy(-D warnings), `cargo test --locked --all-targets`가 통과한다. 시험은 135건이다.
  - 변이 20개를 전부 다시 돌렸다. 19개가 검출됐고, M13만 살아남았다. mutants.txt와 같다.
  - OpenAPI 0.6.0은 `openapi-spec-validator` OK다.
  - `validate_package.py`와 `render_progress.py --check`는 모두 EXIT=0이다.
  - 정책 파일의 SHA-256은 고정값 `f616d526…`과 같다.
- 형식 검사와 의미 검토의 분리, 소유권 CAS, 세대 규칙, 늦은 claim 보존, 옛 세대 승인 제외(N4), 거부 시 무상태, 결함 주입 롤백, G4/G5 누적 상한, liveness와 P04의 구분은 코드와 시험으로 확인했다.
- **P1 두 건 때문에 reject한다.**
  - **F1**: 다른 repo의 클라이언트가 남의 스프린트에 새 revision을 등록할 수 있다. 등록되면 그 스프린트를 빼앗아 사건을 읽고, 원래 repo의 배정과 판정을 막는다.
  - **F2**: GitHub 검토 입력을 검토자별 "repo 전체 최신 1건"으로 고른다. 그래서 현재 SHA의 changes_requested가 같은 검토자의 뒤 검토에 가려진다. 뒤 검토는 commented일 수도 있고, 다른 PR 승인일 수도 있다. 이렇게 가려지면 verified `accepted` 판정이 기록된다.
- 그 밖에 P3 후속 항목이 있다(F3~F10).

## 선행 항목 검증

| 항목 | 출처 | 판정 | 근거 |
|---|---|---|---|
| L07 / D12 | 01-03 REPORT §3 L07, `User_Decisions_2026-11-12_0103.md` D12, 01-07 §4 G1~G8, 01-08 §5 RG1~RG3 | **부분 충족(F1·F2로 결함)** | 최소 방식으로 "버전·digest 고정 기계 판독 정책"을 택했다. `policy.rs`가 digest와 id/version을 확인하고, 규칙 집합이 구현과 같은지 본다. 하나라도 다르면 기동을 거부한다. 기동 확인에서 변조 파일은 exit 2였다. D12가 열거한 다섯 의미는 다음과 같이 대응한다. **선행 증거**는 G1이다. **SHA 귀속**은 G2이다. **독립 판정**은 G3이다. **누적 재시도/한도**는 G4·G5이다. **승인 및 제외 범위**는 RG2·G7이다. 다섯 의미 모두 강제 위치가 코드에 있다. 다만 두 군데가 정책 문구와 어긋난다. 하나는 G3(현재 SHA의 reject는 승인을 막는다)이고 F2에서 깨진다. 다른 하나는 RG2(승인은 그 대상에만 적용된다)이고 F1에서 다른 repo가 남의 스프린트 revision을 대체할 수 있다. 정책 해석상의 선택 두 가지는 F7에 적었다. 재시도를 교정 상한에 합산한 것과, RG2를 결정 답변 범위가 아닌 대상 범위로 해석한 것이다. |
| L08 | PR #10, 리뷰 `review-pr10-ea902dd.md` | **충족(재사용)** | 영속 구현이 `authorize_claim`/`authorize_repro`와 DB 발급 이력을 그대로 쓴다. 옛 revision 배정은 409 `REVISION_SUPERSEDED`이고, 같은 배정 재요청은 200 `already_issued`일 뿐 재활성화가 아니다. 시험과 변이 M06·M15로 확인했다. 등록과 배정 사이의 경쟁 창은 F5(P3)에 적었다. |
| 01-08 N2 | `review-pr5-f75ea87.md` N2 | **해소** | `completion_summary`가 확인된 범위와 미완료 범위를 모두 쓴다(`ieo-report/src/lib.rs` filter). 계약 rev 4 문구도 맞췄다. 변이 M20이 검출된다. 판정에서 reject가 막을 때 `stale_approvals`를 비우는 문제는 F9(P3)에 적었다. |
| PR #10 N1 | 리뷰 N1 | **해소** | 01-08 계약의 검사 순서에 "본문 SHA 대조"를 넣었다. 409 목록에 `ASSIGNMENT_SHA_MISMATCH`가 들어갔고, 본문 SHA가 null일 때의 처리도 명시했다. OpenAPI의 중복 단어 "An"도 지웠다. |
| PR #10 N2 | 리뷰 N2 | **해소** | `Registry::add_spec_with_mode`가 digest를 다시 계산한다. digest가 없거나 다르면 false를 돌려준다. 변이 M19가 검출된다. |
| PR #10 N3 | 리뷰 N3 | **해소** | `api.rs:980-994`의 `receipt`가 응답 시점의 head로 `counts_for_current_judgment`를 다시 계산한다. 계약에 disposition과 counts가 다를 수 있다고 적었다. 변이 M06이 검출된다. |
| PR #10 N4 | 리뷰 N4 | **해소** | `api.rs:1986-2000`은 판정 시점의 guardrail head (assignment, generation)와 그 작업의 claim만 입력으로 쓴다. 변이 M07이 검출된다. |
| PR #10 N5 | 리뷰 N5 | **부분** | 로그 첫 줄에 커밋 SHA와 porcelain 출력이 함께 있다. 그런데 그 출력이 `?? docs/poc/reports/tasks/02-03/`이어서 빈 목록이 아니다. 보고의 "깨끗한 트리" 서술과 맞지 않는다(F8, P3). |

## 지적

### F1 · P1 · 다른 repo 클라이언트가 남의 스프린트를 새 revision으로 빼앗을 수 있음(교차 repo 노출·차단)
- **위치**
  - `crates/ieo-coord/src/api.rs:526-544`(`register`): `target.repo_id`만 자격 범위와 대조한다.
  - `crates/ieo-spec/src/check.rs:186-300`(`check_registration`): 같은 `sprint_id`의 이전 revision과 `repo_id`가 같은지 보지 않는다.
  - `api.rs:692-705`(`visible_sprint`): **최신 revision**의 repo로 가시성을 정한다.
- **재현**: probe 시험 `probe_cross_repo_sprint_hijack`. 사용한 fixture는 alice(repo-a)와 carol(repo-b, `sprints:register`)이다.
  1. `ready_sprint`로 repo-a의 POC-01 rev 5를 executable 등록하고 의미 검토를 통과시킨다. dev-a에게 배정한 뒤 claim 1건을 저장한다.
  2. carol `GET /v0/sprints/POC-01/events` → 404(정상).
  3. carol `POST /v0/sprints?mode=executable`로 `spec("repo-b", 6, 3)`, 즉 POC-01 rev 6(target repo-b)을 보낸다 → **201 `ACCEPTED_FOR_SEMANTIC_REVIEW`**.
  4. carol `GET /v0/sprints/POC-01/events` → **200, repo-a의 claim 사건(`repo_id: "repo-a"`)이 보인다**.
  5. alice `GET /v0/sprints/POC-01/state` → **404**. 자기 스프린트가 보이지 않는다.
  6. repo-a 배정(A1 gen 1) → **409 `REVISION_SUPERSEDED`**. repo-a 판정 → **422 `REPO_MISMATCH`**.
- **기대**: 같은 `sprint_id`의 새 revision은 같은 `repo_id`여야 한다. 그렇지 않으면 거부해야 한다(403 또는 409/422). 다른 repo의 사건·명세는 읽을 수 없어야 한다. 보고 §2.2도 "다른 repo 스프린트 조회는 404"라고 주장한다.
- **실제**: 다른 repo 자격만으로 남의 스프린트를 대체할 수 있다. 그 뒤로는 사건과 옛 revision 명세(`GET …/revisions/5`)를 읽을 수 있고, 원래 repo의 진행을 막는다. 정책 RG2(승인이 그 대상을 넘지 않음)와 보고의 교차 repo 격리 주장에 어긋난다.
- **수정 방향**: revision을 등록할 때 이전 revision과 `repo_id`가 같은지 확인한다(`register_spec` 또는 `check_registration`). 가시성은 revision마다의 repo가 아니라 스프린트 단위로 고정된 repo로 판단한다. 회귀 시험을 추가한다.

### F2 · P1 · 현재 SHA의 changes_requested가 같은 검토자의 뒤 검토(다른 PR 포함)에 가려져 verified `accepted`가 기록됨
- **위치**: `crates/ieo-coord/src/api.rs:2001-2018`.
  - `SELECT DISTINCT ON (body->>'reviewer') … WHERE repo_id = $1 AND kind = 'review' … ORDER BY body->>'reviewer', seq DESC`는 SHA나 작업을 제한하지 않고, 검토자마다 **repo 전체에서 가장 최근 1건**만 고른다.
  - 그 1건이 `commented`나 `dismissed`이면 `continue`로 버린다.
- **재현**: probe 시험 `probe_github_reject_hidden_by_later_review`. 공통 준비로 dev A1을 SHA1에 배정하고 `full_evidence(SHA1)`를 넣는다.
  1. r1이 SHA1을 approved, r2가 SHA1을 changes_requested → judge `not_accepted`(정상).
  2. r2가 SHA1에 **commented** 검토를 하나 더 남긴다 → judge **`accepted`**.
  3. 별도 사례: r2가 **다른 PR의 SHA3**을 approved → judge **`accepted`**.
- **기대**: 보고 §2.3과 OpenAPI `recordVerdict`는 "머리 SHA에 현재 reject가 있으면 승인으로 보지 않는다"고 적었다. GitHub 의미에서도 changes_requested는 그 검토자가 같은 PR에서 승인하거나 dismiss될 때까지 남는다. 뒤의 COMMENTED 검토나 다른 PR의 검토로 풀리지 않는다.
- **실제**: 미해결 changes_requested가 있는데도 verified `coordinator_verdict`가 `accepted`로 기록된다. 이 판정은 G1 선행 충족에도 쓰이므로(`api.rs:1463-1469`) 후속 작업 배정까지 열린다. 이전 증거나 무관한 증거가 현재 판정에 들어가는 경로다.
- **비고**: 기존 시험은 reject를 guardrail claim으로만 검사한다(M12). GitHub 검토의 reject 경로는 시험이 없다.
- **수정 방향**: 검토 증거를 머리 SHA(또는 그 PR)로 한정한다. 검토자별 상태는 그 SHA·PR 안에서 approved, changes_requested, dismissed만으로 계산한다. commented는 상태를 바꾸지 않는다. 위 두 사례를 회귀 시험으로 추가한다.

### F3 · P3 · 판정은 개발 head가 그 작업에 있을 때만 가능함
- **위치**: `api.rs:1960-1977`. head는 (repo, role)마다 하나다.
- **문제**: 개발 head가 다른 작업으로 넘어가면 앞 작업은 판정할 수 없다(409 `NO_CURRENT_ASSIGNMENT`). 선행 관계가 없는 두 작업이라면 첫 작업이 영원히 판정되지 않을 수 있다. 판정의 대상 revision도 배정 당시가 아니라 **최신 revision**의 task·outputs다(`api.rs:1927-1958`).
- **조치**: 02-04·02-07에서 정할 것을 권한다. 판정의 head를 그 작업의 마지막 개발 배정 SHA로 할지, 대상 revision을 배정 당시의 revision으로 할지다. 보고 §4에 한계로 적어 두는 정도면 충분하다.

### F4 · P3 · GitHub 검토 author의 독립성은 수집기가 준 문자열에만 기댐
- **위치**: `api.rs:2003`의 `reviewer <> author`.
- **문제**: author가 개발 에이전트 신원이나 PR 작성자와 실제로 같은지 코디네이터가 확인하지 않는다. 수집기는 신뢰 서비스이므로 이번 범위에서는 허용한다.
- **조치**: 실제 수집기(02-04/02-05)에서 PR 작성자를 GitHub에서 직접 얻도록 할 것을 후속으로 남긴다.

### F5 · P3 · 등록과 배정 사이의 경쟁 창(L08 경계)
- **문제**: `assign`은 `scope:` 잠금만 잡고 `latest_revision`을 읽는다(`api.rs:1368`). `register`가 잡는 `registry` 잠금은 잡지 않는다. READ COMMITTED에서는 새 revision 커밋이 배정의 검사와 커밋 사이에 끼어들 수 있다. 그러면 대체된 revision으로 배정이 커밋된다. 재현하지는 않았다(창이 좁다).
- **조치**: 배정에서도 `registry`를 공유 잠금으로 잡거나, `sprint_revisions` 행을 `FOR SHARE`로 잠근다.

### F6 · P3 · G5 이후 재도달과 assignment_id 경쟁의 처리
- **G5 재도달**
  - `limit_reached`는 다시 내려가지 않는다.
  - 상한을 올린 새 revision으로 재개한 뒤 다시 상한을 넘으면 decision_request 없이 곧바로 409 `LIMIT_REACHED`가 된다.
  - 같은 `event_id`(`limit-{sprint}-{task}-{role}`)는 다시 쓸 수 없다.
  - 정책 문구("once per task")와는 일치한다. 다만 사용자에게 다시 묻는 경로가 없으므로 결정 API 작업에서 정해야 한다.
- **assignment_id 경쟁**: 같은 `assignment_id`의 gen 0을 다른 role로 동시에 보내면 서로 다른 잠금을 잡는다. 이번 probe에서는 409로 끝났다. 순서가 맞물리면 PK 위반이 500이 될 수 있다.

### F7 · P3 · 정책 해석 두 가지의 표시
- **재시도와 교정 상한의 합산**: G4의 공백은 "재시도 누적 상한(교정과 **별도**)"이다. 정책은 변경 없는 재시도도 `limits.correction_attempts`에 합산한다. 새 숫자를 만들지 않고 더 엄격하게 가는 선택이라 수용한다. 다만 사용자 결정 기록에 "재시도와 교정을 한 상한으로 센다"는 해석을 드러내야 한다.
- **RG2의 의미 변경**: 01-08 §5의 RG2는 "결정 요청 답변이 허용하는 행동 범위"다. 정책의 RG2는 의미 검토·배정·판정의 대상 범위로 바뀌어 있다. 결정 답변 API가 이번 범위 밖이므로 지금 쓰이는 의미는 없다. 그래도 보고 §1의 "RG2 승인 범위" 표기는 이 차이를 적어야 한다.

### F8 · P3 · 보고 수치·표기
- **시험 수 내역**: REPORT §3의 "ieo-report 33+4·ieo-spec 16+25+1"은 실제와 다르다. 실제는 ieo-report 1+33, ieo-spec 4+16+25다. 합계 135는 맞다.
- **"깨끗한 트리"**: `cargo-test.log:1`의 porcelain 출력은 `?? docs/poc/reports/tasks/02-03/`이다. 다음 줄의 "(empty above = clean tree)"와 맞지 않는다. 미추적 보고 디렉터리일 뿐이라 빌드에는 영향이 없다. 그래도 "보고 디렉터리 외 깨끗함"으로 고쳐야 한다.
- **기동 확인 커밋**: startup-smoke는 0850eaf에서 했다. 0850eaf→3e24286은 시험만 바뀌었으므로 문제는 없다. 이 사실을 보고에 명시하면 좋다.

### F9 · P3 · 판정 요약에서 이전 SHA 승인 정보가 사라짐
- **위치**: `api.rs:2019-2023`.
- **문제**: reject가 막는 경우 `stale_approvals: vec![]`로 대체한다. 그러면 "이전 SHA 승인 …은 현재 판정에 쓰지 않음" 문구가 빠진다. 01-08 N2가 의도한 "범위 숨김 없음"과 결이 다르다.

### F10 · P3 · 사소한 계약·견고성 항목
- **cursor 검사**: 범위 안의 숫자이기만 하면 서버가 발급하지 않은 cursor(`s3` 등)도 받는다(`api.rs:906-919`). 결과는 정상 페이지다.
- **500 응답 선언**: DB 오류 500(`INTERNAL`)은 health 외의 엔드포인트 OpenAPI 응답에 선언돼 있지 않다.
- **DB 연결 끊김**: `http.rs`에서 DB 연결이 끊긴 worker는 받은 요청에 응답하지 않고 종료한다. 모든 worker가 종료되면 서버는 응답 없이 멈춘다.
- **의미 검토 가능 주체**: `semantic_review`의 자기 검토 판정은 `registered_by`만 본다. 그래서 draft를 등록한 주체와 다른 주체가 executable로 승격한 경우, 승격한 주체가 그 revision을 검토할 수 있다.

## 확인했고 문제없는 항목

- **인증**: 토큰은 SHA-256만 보관한다. 자격 파일은 다음 경우를 기동 단계에서 거부한다(시험 `credentials_refuse_inconsistent_principals`).
  - 에이전트가 repo 하나·역할 하나·`agent:*` 범위만 갖지 않은 경우
  - `judge:verify`가 service가 아닌 주체에 있는 경우
  - `agent:repro`가 guardrail이 아닌 에이전트에 있는 경우
- **에이전트 차단**: probe로 확인했다. 에이전트의 verified 보고는 403 `VERIFIED_FROM_AGENT_DENIED`이고, `coordinator_verdict` 제출은 403이다. 에이전트는 `/v0/sprints*`와 coordinator 엔드포인트에서 403이다.
- **거부 시 무상태**: 한 요청이 한 트랜잭션이고 2xx만 커밋한다. 쓰기 뒤 실패하면 결함 주입 시험이 전부 롤백됨을 확인한다. M13이 동등 변이라는 설명도 맞다. 실패 없이 쓰기 뒤 거부하는 경로는 내부 500(저장 직후 재조회 실패)밖에 없다.
- **G1**: 판정은 같은 정책 digest, `accepted` 또는 `result_recorded`(아무 verified 판정)여야 한다. `result_recorded`는 장부 정의와 맞다. probe로 선행 미판정 작업의 배정이 409 `PREREQUISITE_UNMET`임을 확인했다.
- **G4/G5**: 상한 c이면 첫 발급과 c회, 모두 c+1회를 발급한다. 그다음 요청은 202 `limit_reached`이고 decision_request와 outbox를 기록한다. 그 뒤로는 409이고 행 수가 바뀌지 않는다.
- **판정의 원자 기록과 append-only**: 판정 사건·outbox·judgment가 한 트랜잭션이다. append-only 트리거가 있다(M18).
- **잠금 순서**: scope(dev→guardrail)→sprint 순서이고 registry는 따로다. 순환 대기는 찾지 못했다.
- **health**: 응답 본문에 "not a P04 result"가 있다. 보고와 result.json도 P04를 주장하지 않는다. "fixture"와 "실제 GitHub 미연결"도 명시했다.
- **CI**: rust-check에 PostgreSQL 17 서비스가 digest로 고정돼 있다. `CI`가 설정돼 있는데 DB 변수가 없으면 시험이 실패한다. 건너뛰지 않는다.
- **OpenAPI**: 서버가 내는 코드가 모두 열거에 있다(시험 `every_emitted_code_is_declared_in_the_openapi_contract`).

## 실행한 명령

```
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree add --detach <scratch>/rv-0203 1e98f17
git diff --stat c501a44..HEAD; git diff 0850eaf 3e24286 --stat   # tests only
git diff 3e24286 1e98f17 --stat                                   # report/evidence only
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-rv
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=<file>"
cargo fmt --all -- --check                                        # OK
cargo clippy --locked --all-targets -- -D warnings                # OK
cargo test --locked --all-targets                                 # 21+6+29+1+33+4+16+25 = 135 passed
.venv-poc/bin/python docs/poc/reports/tasks/02-03/coordinator-20261113/evidence/mutants.py
  # 20개 전부: M01-M12, M14-M20 killed, M13 SURVIVED (mutants.txt와 동일), dirty after run: none
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml       # OK
(cd docs/poc && .venv-poc/bin/python scripts/validate_package.py)           # EXIT=0
(cd docs/poc && .venv-poc/bin/python scripts/render_progress.py --check)    # ok, EXIT=0
sha256sum contracts/judgment-policy-v0.json                       # f616d526… == PINNED_POLICY_DIGEST
probe tests (tests/coordinator.rs에 임시 추가 → git checkout으로 원복):
  probe_cross_repo_sprint_hijack            # F1
  probe_github_reject_hidden_by_later_review # F2
  probe_agent_verified_claim_and_misc       # 403 VERIFIED_FROM_AGENT_DENIED, 403 ROLE_MISMATCH, cursor s0, PREREQUISITE_UNMET
  probe_same_aid_two_roles_concurrently     # 201 + 409
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree remove --force <scratch>/rv-0203
```
