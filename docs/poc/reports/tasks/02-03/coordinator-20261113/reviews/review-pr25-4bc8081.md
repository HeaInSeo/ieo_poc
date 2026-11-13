# PR #25 독립 재검토 2 (Guardrail) · head 4bc8081 · base main c501a44

VERDICT: reject

- 검토 대상: `task/02-03-coordinator-db-api` 4bc8081. 0fa839d 이후 커밋은 세 개다.
  - eb7b0af: N1·N3 교정, 정책 0.3.0.
  - 02c61a0: assignment-id 잠금 제거, 변이 재지정.
  - 4bc8081: 보고·증거·검토 보관. `git diff --stat 02c61a0 4bc8081 -- crates contracts`는 비어 있다.
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다.
  - 검토용 detached worktree(`<scratch>/rv3-0203`)에서 임시 probe 시험을 붙여 실행했다.
  - 끝난 뒤 `git checkout -- crates`로 되돌리고 `worktree remove --force`로 지웠다. 내가 만든 target(`rv3-0203/target-rv3`)도 함께 지웠다.
- 시험 DB는 `ieo-poc-0203-pg`(127.0.0.1:55432)만 썼다.
  - 이관 확인용 `rv3_mig` DB를 만들었다가 지웠다.
  - 끝난 뒤 남은 시험 DB가 없음을 확인했다.

## 요약

- 재현 결과는 보고와 일치한다.
  - fmt, clippy(-D warnings), `cargo test --locked --all-targets`가 통과한다. 시험은 **141건**이다(27+6+29+1+33+4+16+25).
  - 변이 M17·M24·M27~M30을 포함해 14개를 다시 돌렸고 모두 검출됐다. 내 추가 변이 X1·X2도 검출됐다.
  - OpenAPI는 `openapi-spec-validator` OK다. `validate_package.py`와 `render_progress.py --check`는 EXIT=0이다.
  - 정책 SHA-256 `01acce79…`는 `PINNED_POLICY_DIGEST`와 같다.
- **N1(P2)은 해소됐다.** 다음 상한 순서를 probe로 돌렸고, 500은 한 번도 없었다.
  - 순서: 1 → 2 → 1 → 2 → 3 → 0 → 3.
  - 이미 물었던 값에서는 409 `LIMIT_REACHED`이고 상태 변화가 없다.
  - 새 값(3, 0)에서는 202와 새 decision_request가 나온다.
- **N3 교정이 새 P2 결함을 하나 만들어서 reject한다(R1).**
  - G1은 "작업 정의"가 같은지를 task JSON 객체 **전체**로 비교한다. 이 객체에는 진행 표시 필드인 `status`·`evidence`·`estimate`도 들어 있다.
  - 그래서 새 revision이 이 표시 필드만 바꿔도(예: `status` "검수 완료", 02-01 계측으로 `estimate` 갱신) 받아들여진 선행 판정이 무효가 된다. 후속 작업은 모두 409 `PREREQUISITE_UNMET`이 된다. 스프린트를 넘는 선행 관계에도 똑같이 적용된다.
  - 벗어나려면 이미 받아들여진 선행 작업을 새 revision에서 **다시 배정**해야 한다. 그러면 개발 머리를 되돌리게 되고 교정 시도 1회를 쓴다. 상한 0에서는 그마저 불가능하고, 실제로는 없었던 "교정 상한 도달" 결정 요청이 기록된다.
- 그 밖에 P3 두 건이 있다(R2·R3).

## 이전 지적 처분 검증

### 1차 검토(1e98f17) F1~F10

| 지적 | 판정 | 근거 |
|---|---|---|
| F1 P1 교차 repo 스프린트 탈취 | **유지(해소)** | `register_spec`은 바뀌지 않았다(0fa839d..4bc8081 diff에 없음). M21이 검출된다. |
| F2 P1 GitHub 검토가 머리 SHA에 묶이지 않음 | **유지(해소)** | 코드는 바뀌지 않았다. M22·M12·M26이 검출된다. |
| F3 P3 판정 대상 revision | **해소** | 판정 행에 `sprint_revision`(배정 revision)을 기록한다. 시험 `verdicts_judge_the_assigned_revision_…`가 있고, M30(최신 revision으로 되돌림)이 검출된다. |
| F4 P3 GitHub author 독립성 | **수용 유지** | §4 후속 그대로 |
| F5 P3 등록·배정 경쟁 창 | **해소(직접 시험 없음, 공개됨)** | 배정은 `registry` 잠금(`api.rs:1354`)을 잡은 뒤에 모든 revision·G1 조회를 한다. 경쟁 창을 직접 겨냥한 시험은 없다. §7에 "시간 창이 좁아 자동 시험 없음"으로 공개했고, 이를 수용한다. |
| F6 P3 상한 재도달·assignment_id 경쟁 | **해소** | 상한 쪽은 아래 N1을 본다. id 경쟁은 registry 잠금이 직렬화한다(아래 "확인했고 문제없는 항목"). M29를 시험 `one_assignment_id_raced_by_two_roles_never_errors`가 검출한다. |
| F7 P3 정책 해석 표시 | **유지(해소)** | G4·RG2 문구는 바뀌지 않았다. |
| F8 P3 보고 수치·깨끗한 트리 | **해소** | 141건·30/29 검출·커밋 02c61a0·porcelain 빈 목록이 재현과 같다. |
| F9 P3 reject 때 이전 SHA 승인 표시 사라짐 | **유지(해소)** | M26이 검출된다. |
| F10 P3 커서·500·재연결·승격자 검토 | **유지(해소)** | 코드는 바뀌지 않았다. M25가 검출된다. |

### 재검토(0fa839d) N1~N5

| 지적 | 보고 §7 처분 | 판정 | 근거 |
|---|---|---|---|
| N1 P2 상한을 낮췄다 되돌리면 500 | 교정: 그 상한 값의 decision 사건이 있으면 409 | **해소** | `api.rs:1538-1553`은 `events`에서 `(scope_repo, 'coordinator', limit-{sprint}-{task}-{role}-c{ceiling})`의 존재를 본다. 이 키는 삽입하는 `event_id`와 같은 문자열이므로 UNIQUE 위반 경로가 없다. probe `rv3_probe_ceiling_sequences`에서 상한을 1→2→1→2→3→0→3으로 바꿨다. 결과는 c1·c2·c3·c0마다 202가 한 번씩이고, 반복과 이미 물은 값은 409이며 행 수 변화가 없다(decision 사건 4개). 500은 없었다. 시험(1→2→1)이 있고 M28·M24가 검출된다. |
| N2 P3 sprint_id 선점·존재 노출 | §4 후속 | **수용** | §4 표와 result.json `unresolved`에 적혔다. |
| N3 P3 F3 시험 부재·바뀐 작업의 옛 판정 | 교정(G1 0.3.0) | **해소, 단 새 결함 R1(P2)** | 시험과 M27·M30이 있고, 내 X1·X2도 검출된다. 그러나 "정의"를 task 객체 전체로 잡아 표시 필드 변경까지 무효로 만든다(R1). |
| N4 P3 경쟁 교정·재연결 시험 부재, 전역 직렬화 서술 | 경쟁 시험 추가, §4 문구 교정, id 잠금 제거 | **해소** | M29를 경쟁 시험이 검출한다(내 실행에서도 검출). §4에 "등록·의미 검토·배정은 전역 registry 잠금으로 직렬화"라고 적혔다. 재연결은 이전 수동 재현을 근거로 둔다(수용). |
| N5 P3 이전 SHA changes_requested와 GitHub PR 상태 차이 | §4 후속 | **수용** | §4와 `unresolved`에 적혔다. |

## 선행 항목 재확인

| 항목 | 판정 | 근거 |
|---|---|---|
| L07 / D12 | **부분(R1)** | 정책 0.3.0의 digest 고정, 버전 상수, 7개 규칙 집합은 그대로 맞다. G5 문구와 코드도 일치한다. 그러나 G1의 "definition of that task"는 정의되지 않았고, 코드는 진행 표시 필드까지 정의로 취급한다(R1). |
| L08 | **충족** | `REVISION_SUPERSEDED`와 `already_issued`는 바뀌지 않았다. |
| 01-08 N2 | **해소** | 바뀌지 않았다. M20은 보고상 검출이다. |
| PR #10 N1~N5 | **해소** | 바뀌지 않았다. 141건이 통과한다. cargo-test.log 1행에 커밋 02c61a0과 빈 porcelain이 있다. |

## 새 지적

### R1 · P2 · G1이 진행 표시 필드(`status`·`evidence`·`estimate`) 변경까지 "작업 정의 변경"으로 보아, 받아들여진 선행 판정을 무효로 만든다
- **위치**
  - `crates/ieo-coord/src/api.rs:1494-1505`: `(SELECT t FROM jsonb_array_elements(jr.spec->'tasks') t …) = (SELECT t … lr.spec …)`로 task 객체 전체를 jsonb로 비교한다.
  - `contracts/judgment-policy-v0.json:14-16`(G1 0.3.0): "definition of that task equals its definition in the sprint's latest revision".
  - 스키마 `docs/poc/SprintSpec_v0.schema.json`의 task 필수 필드에는 `status`(미착수…검수 완료), `evidence`, `estimate`가 들어 있다.
- **재현**: probe `rv3_probe_status_label_only_change`.
  1. `ready_sprint(0)`을 만든다. A1(01-01)과 G1을 배정하고, guardrail approve와 전체 증거를 넣는다. 판정 → `accepted`(rev 5).
  2. 대조군: 작업 내용이 같은 rev 6(revision만 다름)을 등록하고 검토한다 → 01-02 배정 **201**.
  3. 새 DB에서 1을 반복한다. rev 6에서 `tasks[0].status`만 "검수 완료"로 바꿔 등록하고 검토한다 → 01-02 배정 **409 `PREREQUISITE_UNMET`**.
  4. 다시 판정받으려고 01-01 gen 1을 rev 6에서 배정한다 → **202 `limit_reached`**. 기록된 사건은 `limit-POC-01-01-01-development-c0`다. 교정 시도가 한 번도 필요 없었는데 "교정 상한 도달" 결정 요청이 남는다.
  5. 판정을 다시 해도 머리가 rev 5 배정이므로 `accepted`(rev 5)다. 01-02는 여전히 **409**다.
  6. 상한 3에서는 `estimate`만 `measured`로 바꾼 rev 6으로도 01-02 → **409 `PREREQUISITE_UNMET`**이 재현된다.
- **기대**
  - G1 0.3.0의 취지("a changed task needs a new verdict", N3)는 판정 입력(산출물·수용 기준·검증·종료 조건 등)이 바뀐 경우다.
  - 같은 정책은 "status labels in a spec never satisfy a prerequisite"라고 하여, 상태 표시를 판정과 무관한 표시로 다룬다.
  - 따라서 판정 입력이 아닌 표시 필드의 갱신은 선행 판정을 무효로 만들지 않아야 한다.
- **실제**
  - 진행 상태나 추정치 갱신처럼 정상적인 revision 갱신만으로 후속 작업 전체가 막힌다. 이 효과는 스프린트를 넘는다. 예를 들어 POC-02의 02-01은 01-03·01-06을, 02-03은 01-02·01-07·01-08을 선행으로 둔다.
  - 해제하는 방법은 이미 병합·수용된 선행 작업을 새 revision에서 재배정하는 것뿐이다.
    - (repo, role)당 머리가 하나이므로 진행 중인 개발 배정이 밀려난다.
    - 누적 교정 시도를 1회 쓴다.
    - 상한에 닿아 있으면 사실과 다른 decision_request가 기록되고, 사용자 결정 없이는 진행할 수 없다.
- **수정 방향**
  - 판정 입력에 해당하는 필드만 비교한다. 예: `kind`, `outputs`, `verification`, `acceptance`, `exit_condition`, `prerequisites`, `requirement_ids`. 또는 `status`·`evidence`·`estimate`(그리고 판정과 무관한 `references`·`legacy_v1_2_id`)를 뺀 사영을 비교한다.
  - 정책 G1에 "definition"의 필드 목록을 적는다(정책 버전을 올린다).
  - 위 재현(`status`만 변경 → 201)을 회귀 시험으로 추가한다. "필드 하나만 바꾸면 여전히 409"인 기존 N3 시험은 유지한다.

### R2 · P3 · OpenAPI `assign` 설명이 정책 0.3.0(G1 정의 비교, G5 값별 질의)을 반영하지 않음
- **위치**: `contracts/ieo-api-v0.openapi.yaml:384-389`(버전 0.6.0, 이번 교정에서 바뀌지 않음).
- **내용**
  - 설명은 "a ceiling **raised** by a newer revision and reached again records a new decision_request"라고 한다. 코드는 **낮춘** 새 값에서도 새 decision_request를 기록한다(probe에서 c0 → 202). 이미 물었던 값으로 되돌리면 409다.
  - G1 설명("Prerequisites need verified verdicts under the pinned policy")에는 "판정 revision의 작업 정의가 최신 revision과 같아야 한다"는 조건이 없다.
- **조치**: 설명을 정책 0.3.0 문구에 맞춘다. G5 정책 문구 "recorded once per (task, role, ceiling value), whenever that value was asked before"도 읽기 어렵다. "물은 적 없는 상한 값마다 한 번 기록하고, 이미 물은 값이면 409"처럼 다듬기를 권한다.

### R3 · P3 · `judgments.sprint_revision` 추가가 이관되지 않고, 외래 키가 없음
- **위치**: `crates/ieo-coord/src/schema.sql:156-157`, `:209`. `migrate`는 `CREATE TABLE IF NOT EXISTS`만 실행하고, `schema_version`은 `ieo-coord-v0` 그대로다.
- **재현**
  1. 새 DB에 0fa839d의 schema.sql을 적용한 뒤 4bc8081의 schema.sql을 적용한다(`migrate`와 같은 batch).
  2. 두 번 다 성공한다. `judgments`에 `sprint_revision` 열은 **0개**이고 `schema_version`은 `ieo-coord-v0`이다.
  3. 이 DB에서는 판정 INSERT가 "column does not exist"로 500이 된다.
- **영향**
  - 아직 병합 전이라 영향은 개발·기동 확인 DB에 한정된다. 다만 이관은 성공하는데 실행 중에 500이 나는 형태다.
  - 또 `assignments`와 달리 `(sprint_id, sprint_revision) REFERENCES sprint_revisions`가 없다. G1 조회가 내부 JOIN이라 지금 동작은 안전하다.
- **조치**: 외래 키를 추가한다. 스키마 변경 시 `schema_version`을 올리고, 기동 때 버전이 다르면 거부하는 것을 고려한다. 최소한 §4에 "v0 개발 중 스키마 변경은 DB를 다시 만든다"고 적는다.

## 확인했고 문제없는 항목

- **G1 SQL의 NULL 처리**
  - 판정한 revision이나 최신 revision에 그 작업이 없으면 스칼라 부질의가 NULL이 된다. `=`가 NULL이 되어 EXISTS는 거짓이다. 즉 미충족으로 닫힌다(fail closed).
  - 둘 다 없어도 `NULL = NULL`은 NULL이라 미충족이다.
  - 같은 task_id가 한 명세에 두 번 들어가 "more than one row"(500)가 나는 경로는 없다. draft 등록도 422 `DUPLICATE_TASK_ID`다(probe `rv3_probe_duplicate_task_draft`).
  - `tasks`가 없으면 `jsonb_array_elements(NULL)`이 0행이 되어 미충족이다.
  - `jr`은 내부 JOIN이라, 판정 revision이 없는 행은 셈에서 빠진다.
- **스프린트를 넘는 선행 관계**
  - 비교 대상은 *판정의* 스프린트(`j.sprint_id`)의 최신 revision이다. 따라서 POC-02가 POC-01 작업을 선행으로 둘 때도 POC-01 쪽 정의로 비교한다. 맞다.
  - task_id는 등록 검사 `TASK_ID_CONFLICT`로 스프린트 사이에서 고유하다. 그래서 `j.task_id`만으로 고르는 것이 다른 스프린트의 동명 작업과 섞이지 않는다.
  - 작업이 다른 스프린트로 옮겨 가면, 옛 스프린트의 판정은 최신 revision에 그 작업이 없어 NULL → 미충족이다.
  - `lr`은 draft를 포함한 최신 revision이다. 이는 L08의 `latest_revision`(draft 포함 시 `REVISION_SUPERSEDED`)과 같은 기준이다.
- **G1 조회의 안정성**: 모든 스프린트의 등록·변환·의미 검토는 같은 전역 `registry` 잠금을 잡는다. 배정도 그 잠금 아래에서 G1을 조회하므로, 조회 도중 최신 revision이 바뀌지 않는다.
- **assignment-id 잠금 제거**
  - `INSERT INTO assignments`와 `assignment_heads`를 쓰는 경로는 `assign` 하나다(`api.rs:1593`, `:1600`).
  - `assign`은 scope 잠금 → `registry` 잠금을 잡은 **뒤에** 재요청 조회, 미사용 id 확인, 삽입을 모두 한다.
  - 트랜잭션은 기본 READ COMMITTED이고 advisory 잠금은 `pg_advisory_xact_lock`이다. 그래서 잠금 뒤의 각 문장은 앞선 보유자가 커밋한 행을 본다. 두 역할이 같은 id로 경쟁하면 하나는 201, 하나는 409다.
  - 잠금 순서는 다음과 같고 순환은 없다.
    - assign: scope → registry → sprint
    - register·convert·semantic_review: registry → sprint
    - judge: judgment → scope:dev → scope:gr → sprint
    - claim·repro: scope → sprint
  - 내 변이 X3(assign의 scope 잠금 제거)는 살아남는다. registry 잠금이 상위 집합이라 **동등 변이**로 본다.
- **상한별 결정 확인**
  - 확인 키는 삽입 `event_id`와 같은 형식 문자열이다(`limit_decision_request`).
  - `scope_role='coordinator'` 사건은 코디네이터만 만든다. 에이전트 보고의 역할은 development·guardrail로 제한된다.
  - task_id 형식(NN-NN)상 sprint_id와 task_id를 이어 붙인 키가 다른 쌍과 겹칠 수 없다.
  - `task_attempts.reached_ceiling`은 이제 기록용일 뿐 판단에 쓰이지 않는다(무해).
- **정책 0.3.0과 코드**: G1·G5 문구는 코드 동작과 같다. 단 G1의 "definition" 범위는 R1에 적었다. version 상수 `0.3.0`, digest 고정, 규칙 7개는 그대로다.
- **변이 재지정**: M17("attempt ceiling ignored")과 M24(키에서 상한 제거)가 의미 있게 재지정됐다. 옛 M17의 의미(결정 요청 1회)는 M28이 덮는다.
- **보고 일관성**: result.json의 repo_sha 02c61a0, revision 3, 141건, 30/29가 재현과 같다. P04를 주장하지 않는다. §7의 처분 표는 위 검증과 같다. 단 N3은 R1이 남는다.

## 실행한 명령

```
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree add --detach <scratch>/rv3-0203 4bc8081
git log --oneline 0fa839d..4bc8081; git diff 0fa839d 4bc8081 -- crates contracts <evidence>/mutants.py
git diff --stat 02c61a0 4bc8081 -- crates contracts                 # empty
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/rv3-0203/target-rv3
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=<file>"
cargo fmt --all -- --check                                          # OK
cargo clippy --locked --all-targets -- -D warnings                  # OK
cargo test --locked --all-targets                                   # 27+6+29+1+33+4+16+25 = 141 passed
<scratch>/myscripts/rv3-mut.py (evidence/mutants.py의 M 목록 재사용, cargo test -p ieo-coord --all-targets)
  M01 M03 M04 M12 M17 M21 M22 M24 M25 M26 M27 M28 M29 M30: killed (mutants.txt와 동일)
  X1 G1 compares with the first revision (DESC→ASC): killed
  X2 G1 compares latest with latest (verdict revision ignored): killed
  X3 scope lock removed from assign: SURVIVED (registry 잠금이 상위 집합, 동등)
  dirty after run: none
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml       # OK
(cd docs/poc && .venv-poc/bin/python scripts/validate_package.py)           # EXIT=0
(cd docs/poc && .venv-poc/bin/python scripts/render_progress.py --check)    # ok, EXIT=0
sha256sum contracts/judgment-policy-v0.json                         # 01acce79… == PINNED_POLICY_DIGEST
probe tests (tests/coordinator.rs에 임시 추가 → git checkout으로 원복):
  rv3_probe_status_label_only_change   # R1: 대조군 201 / status만 변경 409 / 재배정 202 c0 / 재판정 후 409 / estimate만 변경 409
  rv3_probe_ceiling_sequences          # N1: 1→2→1→2→3→0→3, 500 없음, 409는 상태 불변, decision c1·c2·c3·c0
  rv3_probe_duplicate_task_draft       # draft 중복 task 422 DUPLICATE_TASK_ID
이관 확인: CREATE DATABASE rv3_mig; 0fa839d schema.sql → 4bc8081 schema.sql (psql, ON_ERROR_STOP)
  # 둘 다 성공, judgments.sprint_revision 열 0개, schema_version ieo-coord-v0; DROP DATABASE rv3_mig
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree remove --force <scratch>/rv3-0203
```
