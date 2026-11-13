# PR #25 독립 재검토 (Guardrail) · head 0fa839d · base main c501a44

VERDICT: reject

- 검토 대상: `task/02-03-coordinator-db-api` 0fa839d. 1e98f17 이후 코드 커밋은 dfac60a 하나다. 0fa839d는 보고·증거·검토 보관만 바꾼다(`git diff --stat dfac60a 0fa839d`에 crates·contracts 변경 없음).
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다. 검토용 detached worktree(`<scratch>/rv2-0203`)에서 임시 probe 시험을 추가해 실행했고, `git checkout -- crates`로 되돌린 뒤 `worktree remove --force`로 지웠다.
- 시험 DB는 `ieo-poc-0203-pg`(127.0.0.1:55432)만 썼다. 시험은 DB마다 만들고 지운다. 재연결 확인용 `ieo_rv2_smoke` DB도 만든 뒤 지웠다. 다른 컨테이너와 설정은 건드리지 않았다.
- 작업 중 `/tmp` 볼륨이 가득 차서(남은 공간 0) 첫 변이 실행이 링크 단계에서 실패했다. 그 결과는 버렸다. 내가 만든 두 번째 target 디렉터리를 지운 뒤 다시 실행했다. 아래 변이 결과는 모두 다시 실행한 것이다.

## 요약

- 재현 결과는 보고와 일치한다.
  - fmt, clippy(-D warnings), `cargo test --locked --all-targets`가 통과한다. 시험은 139건이다(25+6+29+1+33+4+16+25).
  - 변이는 M08·M09·M12·M17·M21~M26 10개를 다시 돌렸고 모두 검출됐다. mutants.txt와 같다.
  - OpenAPI 0.6.0은 `openapi-spec-validator` OK다.
  - `validate_package.py`와 `render_progress.py --check`는 모두 EXIT=0이다.
  - 정책 파일의 SHA-256 `6d44407e…`는 `PINNED_POLICY_DIGEST`와 같다.
- P1 두 건은 해소됐다.
  - **F1**: 다른 repo의 revision은 409 `SPRINT_REPO_CONFLICT`로 거부된다. 이 검사는 등록과 변환에 공통으로 적용되며, 이전 probe의 탈취 경로는 막혔다.
  - **F2**: GitHub 검토는 머리 SHA의 것만 쓴다. 검토자마다 approved·changes_requested·dismissed 중 최신 1건을 고르므로, commented나 다른 PR의 승인으로는 changes_requested가 풀리지 않는다.
- **교정이 새 P2 결함을 하나 만들어서 reject한다(N1)**. F6 교정으로 결정 요청을 "(작업, 역할, 상한)마다 한 번" 기록하게 했다. 그런데 코드는 마지막 상한 하나만 기억한다.
  - 상한을 낮췄다가 이전 값으로 되돌리면, 같은 `event_id`의 결정 요청을 다시 넣으려다 UNIQUE 위반이 난다.
  - 그 결과 배정 요청이 매번 500 `INTERNAL`이 된다.
  - 정책 0.2.0 G5 문구와 OpenAPI가 약속한 202/409와 다르다.
- 그 밖에 P3 항목이 있다(N2~N5).

## 이전 지적 처분 검증

| 지적 | 보고 §6 처분 | 판정 | 근거 |
|---|---|---|---|
| F1 P1 교차 repo 스프린트 탈취 | 교정, `SPRINT_REPO_CONFLICT` | **해소** | `api.rs:414-430` `register_spec`은 첫 revision의 repo와 다르면 409를 낸다. 등록(`register`)과 변환(`convert`)이 같은 함수를 쓰고, 둘 다 `registry` 잠금 아래에서 실행되므로 경쟁도 없다. 시험 `a_sprint_cannot_be_moved_to_another_repo`가 있고 M21이 검출된다. 이전 probe 순서(carol rev 6 executable)는 409이고 행 수 변화가 없다. 남은 선점·존재 노출 문제는 N2(P3)에 적었다. |
| F2 P1 GitHub 검토가 머리 SHA에 묶이지 않음 | 교정, 머리 SHA만 | **해소** | `api.rs:2054-2069`는 `sha = head_sha`이고, 상태를 approved·changes_requested·dismissed로 제한한 뒤 검토자별 최신을 고른다. commented는 상태를 바꾸지 않고, 다른 SHA의 검토는 쓰지 않는다. dismissed는 changes_requested를 풀고 승인도 지운다(GitHub 의미와 같다). M22·M23과 내 변이 X6(dismissed 제외)이 검출된다. author가 없는 review 증거는 수집 단계에서 400이라 `<>` NULL 문제는 생기지 않는다(probe). 같은 PR의 이전 SHA에 남은 changes_requested를 무시하는 문제는 N5(P3)에 적었다. |
| F3 P3 판정 대상 revision | 교정, 배정의 revision | **부분(P3, N3)** | 판정은 이제 배정 당시 revision의 task·outputs를 쓴다. probe에서 verdict 사건의 `sprint_revision`은 5다. 다만 이 교정을 검사하는 시험이 없다. 내 변이 X1(최신 revision으로 되돌림)이 살아남는다. 또 G1과 결합하면 새 revision에서 추가된 필수 산출물이 선행 조건 판단을 우회한다(N3). 머리가 다른 작업으로 넘어간 경우는 §4에 후속으로 적었고, 이것은 수용한다. |
| F4 P3 GitHub author 독립성 | 정책 0.2.0 명시, §4 후속 | **수용** | G3 문구에 "author string is supplied by the evidence service; verifying it against GitHub is the real collector's job"를 넣었다. §4에 02-04·02-05 후속으로 적었다. |
| F5 P3 등록·배정 경쟁 창 | 교정, assign이 `registry` 잠금 | **해소(시험 없음, N4)** | `api.rs:1350-1354`의 잠금 순서는 scope → assignment-id → registry → (사건 삽입 시) sprint다. 다른 경로의 잠금 순서와 대조했고 순환 대기는 없다. register·convert·semantic_review는 registry → sprint, judge는 judgment → scope:dev → scope:guardrail → sprint, claim은 scope → sprint, evidence는 evidence → sprint다. 동시 스트레스 probe(48요청)에서 500·교착은 없었다. 그러나 변이 X2(잠금 제거)가 살아남는다. 또 배정이 전역 잠금을 잡게 되어 모든 repo의 배정이 직렬화되는데, §4의 "등록은 전역 잠금 하나로 직렬화" 서술에는 이 사실이 빠져 있다. |
| F6 P3 상한 재도달·assignment_id 경쟁 | 교정 | **결함 도입(N1, P2)** | 상한을 올려 다시 도달하면 다시 묻는 기본 경우는 동작한다(`a_raised_ceiling_reached_again_asks_again`, M24). 그러나 낮췄다가 되돌리는 경우는 500이 된다(N1). assignment-id 잠금 자체는 맞지만 변이 X3(잠금 제거)이 살아남아 시험이 없다. |
| F7 P3 정책 해석 표시 | 정책 0.2.0·§2.4 | **해소** | G4에 "retries and corrections share one ceiling … (interpretation, stricter …)"를 넣었다. RG2에는 "Object approval scope (not the decision-answer scope named RG2 in the 01-08 contract §5 …)"를 넣었다. |
| F8 P3 보고 수치·깨끗한 트리 | 교정 | **해소** | REPORT §3 내역(ieo-coord 25·ieo-report 1+33·ieo-spec 4+16+25·ieo-evidence 6+29=139)이 재현과 같다. `cargo-test.log` 1행 porcelain도 빈 목록이다. 기동 확인의 커밋 dfac60a도 명시돼 있다. |
| F9 P3 reject 때 이전 SHA 승인 표시 사라짐 | 교정 | **해소** | `api.rs:2071-2097`는 이 작업 개발 배정들의 이전 SHA 승인을 `stale_approvals`로 유지한다. M26과 내 X4(질의 제거)가 검출된다. |
| F10 P3 커서·500 선언·DB 끊김·승격자 검토 | 교정 / 커서 유지 | **해소** | 500 `Internal`은 모든 경로에 선언됐다. 승격자 검토는 history 기반으로 403이다(M25). 커서 유지 사유("위치")도 수용한다. 재연결은 실서버로 확인했다(`--workers 1`, pg_terminate_backend, DB 이름 변경). 결과는 다음과 같다. ① 끊긴 직후 첫 요청은 500 "connection closed"다. ② 다음 요청은 재연결 후 정상(404)이다. ③ DB를 쓸 수 없으면 500 "database unavailable"로 응답하고 침묵하지 않는다. ④ DB가 복구되면 200이다. 자동 시험은 없다(N4에 함께 적음). |

## 선행 항목 재확인

| 항목 | 판정 | 근거 |
|---|---|---|
| L07 / D12 | **부분(N1)** | 정책 0.2.0의 digest 고정, id/version 검사, 규칙 집합 대조는 그대로다. F1·F2로 G3·RG2 문구와 코드는 이제 일치한다. G5 문구 "once per (task, role, ceiling)"는 코드와 다르다(N1). |
| L08 | **충족** | 옛 revision 배정 409 `REVISION_SUPERSEDED`와 재요청 `already_issued`는 그대로다. 등록과 배정 사이 경쟁 창도 registry 잠금으로 닫혔다. |
| 01-08 N2 | **해소** | 이전 판정과 같다. reject로 막힐 때도 이전 SHA 승인을 표시한다(F9). |
| PR #10 N1~N4 | **해소** | 이전 판정과 같다. 바뀐 코드가 이 경로에 영향을 주지 않는다. M06·M07 등 관련 시험이 통과한다. |
| PR #10 N5 | **해소** | `cargo-test.log` 1행에 커밋 dfac60a와 빈 porcelain 출력이 있다. |

## 새 지적

### N1 · P2 · 상한을 낮췄다가 되돌리면 배정이 매번 500(정책 G5 "(task, role, ceiling)마다 한 번"과 코드 불일치)
- **위치**
  - `crates/ieo-coord/src/api.rs:1512-1563`: `task_attempts.reached_ceiling`에 **마지막** 상한 하나만 기록하고, `reached == Some(ceiling)`일 때만 409를 낸다.
  - `api.rs:1672`: `event_id = limit-{sprint}-{task}-{role}-c{ceiling}`.
  - `schema.sql`의 events `UNIQUE (scope_repo, scope_role, event_id, event_revision)`.
- **재현**: probe 시험 `rv2_probe_ceiling_lowered_then_raised_back`.
  1. `ready_sprint(1)` 뒤 A1 gen 0·1을 발급한다.
  2. gen 2 @rev 5 → 202 `limit_reached`, `limit-POC-01-01-01-development-c1`.
  3. rev 6(상한 2)을 등록하고 검토 → gen 2 @6 → 201. gen 3 @6 → 202 `limit_reached`, `…-c2`.
  4. rev 7(상한 1, 사용자가 상한을 다시 낮춤)을 등록하고 검토 → gen 3 @7 → **500 `{"code":"INTERNAL","message":"database error: db error"}`**. 다시 보내도 **500**이다.
  5. rev 8(상한 2) → 409 `LIMIT_REACHED`.
- **기대**
  - 정책 0.2.0 G5는 "The decision_request is recorded once per (task, role, ceiling)"라고 적었다. c1에서는 이미 물었으므로 409 `LIMIT_REACHED`이어야 한다.
  - 다르게 해석해 다시 묻는다고 해도 202이고 새 decision_request가 기록돼야 한다.
  - OpenAPI `assign` 설명도 202 또는 409만 약속한다.
- **실제**: 같은 `event_id`의 결정 요청을 다시 넣다가 UNIQUE 위반이 나고, 트랜잭션은 롤백되며 500이 된다. 기록되지 않은 상한 값으로 새 revision을 등록하기 전까지 그 작업·역할의 배정은 계속 500이다. 데이터는 손상되지 않지만, 정상 사용자 결정 경로(상한 조정)에서 선언된 의미 대신 내부 오류가 나고 진행이 막힌다.
- **수정 방향**
  - "(task, role, ceiling)마다 한 번"을 그대로 구현한다. 예: 해당 상한의 decision_request 사건이 이미 있으면 409. 또는 물어본 상한들의 집합을 저장한다.
  - 또는 event_id에 revision을 넣고 정책 문구를 그에 맞춘다.
  - 위 재현을 회귀 시험으로 추가한다.

### N2 · P3 · `SPRINT_REPO_CONFLICT`로 다른 repo의 sprint_id 선점과 존재 확인이 가능
- **위치**: `api.rs:414-430`.
- **재현**: probe `rv2_probe_repo_conflict_oracle_and_squat`.
  1. carol(repo-b)이 POC-01을 repo-b의 **draft**로 먼저 등록 → 201.
  2. alice(repo-a)가 POC-01 rev 5를 등록 → **409 `SPRINT_REPO_CONFLICT`**.
  3. alice `GET /v0/sprints/POC-01/state` → 404.
- **문제**
  - sprint_id의 소유가 "먼저 등록한 repo"로 정해진다. 그래서 다른 repo의 `sprints:register` 자격으로 sprint_id를 선점할 수 있다. 검증되지 않은 draft로도 된다.
  - 반대로, 남의 sprint_id를 보내면 409와 201이 갈리므로 존재 여부가 드러난다. 이는 `visible_sprint`가 404로 숨기는 원칙과 결이 다르다.
  - 탈취(F1)와 달리 사건 노출은 없고, 등록 주체는 신뢰된 사람이라 P3로 둔다.
- **조치 제안**
  - sprint_id와 repo의 대응을 승인된 결합(binding·manifest)에서 정한다. 또는 sprint를 (repo, sprint_id)로 이름 공간화한다.
  - 최소한 §4 한계와 정책 RG2 비고에 적는다.

### N3 · P3 · F3 교정의 검증 부재와 G1 결합 효과
- **위치**: `api.rs:2014-2030`(배정 revision으로 판정), `api.rs:1486-1500`(G1은 판정의 task_id·policy_digest·outcome만 본다).
- **재현**: probe `rv2_probe_judge_uses_assigned_revision`.
  1. A1을 rev 5에서 배정한다.
  2. rev 6에서 01-01에 필수 산출물 `01-01-extra`를 더하고 의미 검토를 통과시킨다.
  3. rev 5 기준 증거만 넣고 판정 → **accepted**. verdict 사건의 sprint_revision은 5다.
  4. rev 6에서 01-02(선행 01-01 accepted) 배정 → **201**.
- **문제**
  - 정책 G2 문구("judges … the sprint revision that assignment was issued from")와는 일치한다.
  - 그러나 의미 검토를 통과한 rev 6이 요구한 산출물 없이도 rev 6의 선행 조건이 충족된다. RG2의 "never carries over to another revision"과 긴장 관계다.
  - 또 이 교정을 검사하는 시험이 없다. 변이 X1(`rev`를 최신 revision으로 되돌림)이 살아남는다.
- **조치 제안**
  - F3 교정에 회귀 시험을 추가한다.
  - G1이 판정 revision과 현재 revision에서 그 작업의 정의(task·outputs)가 같은지 확인할지 02-07에서 정한다. 그 전까지는 §4에 한계로 적는다.

### N4 · P3 · 경쟁 교정(F5·F6)과 재연결의 시험 부재, 전역 직렬화 서술 누락
- **시험 부재**
  - 변이 X2(배정의 `registry` 잠금 제거)와 X3(`assignment-id` 잠금 제거)이 모두 살아남는다.
  - DB 재연결 경로(`http.rs:79-90`)에도 자동 시험이 없다. 수동으로는 정상 동작을 확인했다(위 F10).
- **전역 직렬화 서술 누락**: 이제 배정이 전역 `registry` 잠금을 잡으므로, 모든 repo의 배정과 등록이 서로 직렬화된다. REPORT §4 마지막 행은 등록만 직렬화한다고 적었다.
- **조치**: 결함 주입이나 지연 주입으로 경쟁 시험을 추가하거나, 변이 목록에 "시험 없음"을 명시한다. §4 서술을 고친다.

### N5 · P3 · 같은 PR의 이전 SHA에 남은 changes_requested는 새 push 뒤 무시됨
- **재현**: probe `rv2_probe_reject_on_previous_sha_of_same_pr`.
  1. r2가 SHA1에 changes_requested를 남긴다.
  2. A1 gen 1을 SHA2로 발급한다.
  3. r1이 SHA2를 approved하고 SHA2의 전체 증거를 넣는다 → 판정 **accepted**.
- **문제**
  - 정책 0.2.0 G3("reviews of other SHAs never count")과 일치하고, guardrail claim의 옛 SHA reject를 제외하는 기존 규칙과도 대칭이다.
  - 그러나 GitHub의 PR `reviewDecision`은 r2가 승인하거나 dismiss될 때까지 CHANGES_REQUESTED로 남는다. 따라서 실제 GitHub 상태와 코디네이터 판정이 갈릴 수 있다.
- **조치**: 실제 수집기(02-04·02-05)에서 PR 단위 미해결 changes_requested를 입력으로 줄지 정한다. §4의 F4 행과 같은 후속으로 적는다.

## 확인했고 문제없는 항목

- **잠금 순서**: 경로별 advisory 잠금 순서를 위 F5 행처럼 대조했다. 순환 대기는 찾지 못했다. 같은 assignment_id를 두 역할이 쓰더라도 scope(역할별) → id 순서라 순환이 없다. 키는 `hashtextextended`(64비트)를 쓴다. 동시 스트레스 probe(배정 dev/guardrail·판정·draft 재등록 48요청)에서 500이 없었다.
- **semantic_review 자기 검토 판정**: draft 등록자와 executable 승격자 모두 `sprint.registered` history로 걸러진다. `rev`는 i64로 파싱된 뒤 쓰이므로 bigint 비교가 맞다.
- **판정 경로**: 최신 revision에 그 작업이 없으면 422 `UNKNOWN_TASK`다. repo 검사는 최신 revision으로 하는데, F1로 repo가 고정되므로 문제없다.
- **재연결 경로의 응답 본문**: `{"code":"INTERNAL","message":…}`는 OpenAPI `Problem` 스키마(필수 code·message, 추가 속성 없음)에 맞는다.
- **OpenAPI 0.6.0과 코드**: `SPRINT_REPO_CONFLICT`가 Rejection 열거에 추가됐다. 시험 `every_emitted_code_is_declared_in_the_openapi_contract`가 통과한다. 검토 의미·상한 재질의·판정 revision 설명이 코드와 같다. 단, 상한 재질의는 N1의 경우를 빼고 같다.
- **정책 0.2.0**: version 상수, digest 고정, 규칙 집합은 변경 없이 7개다. G2·G3·G4·RG2 문구는 코드와 같다. 다른 것은 G5뿐이다(N1).
- **보고 일관성**: result.json의 repo_sha dfac60a, 139건, 25/26 검출(M13 동등)이 재현과 같다. P04를 주장하지 않는다. 기동 확인은 liveness only다.

## 실행한 명령

```
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree add --detach <scratch>/rv2-0203 0fa839d
git log --oneline 1e98f17..0fa839d; git diff 1e98f17 0fa839d -- crates contracts
git diff --stat dfac60a 0fa839d                                   # report/evidence only
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/target-rv
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=<file>"
cargo fmt --all -- --check                                        # OK
cargo clippy --locked --all-targets -- -D warnings                # OK
cargo test --locked --all-targets                                 # 25+6+29+1+33+4+16+25 = 139 passed
<scratch>/rv2-mut.py  (evidence/mutants.py의 M 목록 재사용 + 추가 변이, cargo test -p ieo-coord --all-targets)
  M08 M09 M12 M17 M21 M22 M23 M24 M25 M26: killed (mutants.txt와 동일)
  X1 judge uses latest revision spec (F3): SURVIVED
  X2 assign registry lock removed (F5):     SURVIVED
  X3 assignment-id lock removed (F6):       SURVIVED
  X4 stale approvals query dropped (F9):    killed
  X6 dismissed not considered (F2):         killed
  dirty after run: none
  (첫 실행은 /tmp 볼륨 가득 참으로 링크 실패 → 결과 폐기, 재실행)
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml       # OK
(cd docs/poc && .venv-poc/bin/python scripts/validate_package.py)           # EXIT=0
(cd docs/poc && .venv-poc/bin/python scripts/render_progress.py --check)    # ok, EXIT=0
sha256sum contracts/judgment-policy-v0.json                       # 6d44407e… == PINNED_POLICY_DIGEST
probe tests (tests/coordinator.rs에 임시 추가 → git checkout으로 원복):
  rv2_probe_ceiling_lowered_then_raised_back      # N1: 500 INTERNAL ×2
  rv2_probe_judge_uses_assigned_revision          # N3: accepted @rev5, 01-02 @rev6 201
  rv2_probe_repo_conflict_oracle_and_squat        # N2: carol 201 → alice 409
  rv2_probe_reject_on_previous_sha_of_same_pr     # N5: accepted; author 없는 review 증거 400
  rv2_probe_lock_stress                           # 48요청, 500 없음
재연결 확인: ieo-coord serve --workers 1 (DB ieo_rv2_smoke, 임시 토큰 파일)
  health 200 → pg_terminate_backend → 500 "connection closed" → 404(재연결)
  → DB 이름 변경 → 500, 500 "database unavailable" ×2 → 복구 → 200 ×2; DB·파일 삭제
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree remove --force <scratch>/rv2-0203
```
