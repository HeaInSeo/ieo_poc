# PR #25 독립 재검토 3 (Guardrail) · head 66958e2 · base main c501a44

VERDICT: approve

- 검토 대상: `task/02-03-coordinator-db-api` 66958e2(검토 4/4, 교정 3/3 소진 뒤 마지막 검토). 4bc8081 이후 커밋은 두 개다.
  - efad0ee: R1(G1 비교 필드 한정)·R2(OpenAPI·G5 문구)·R3(judgments 외래 키, schema.sql 머리 주석). 정책 0.4.0.
  - 66958e2: 보고·증거·검토 4bc8081 보관. `git diff --stat efad0ee 66958e2 -- crates contracts`는 비어 있다.
- 저장소는 수정하지 않았다. push·댓글도 하지 않았다.
  - 검토용 detached worktree(`<scratch>/rv4-0203`)에서 임시 probe 시험을 붙여 실행했다. 끝난 뒤 `git checkout -- crates`로 되돌렸다(porcelain은 내 target 디렉터리 외 빈 목록).
  - 끝난 뒤 `worktree remove --force`로 지웠다. 내가 만든 target(`rv4-0203/target-rv4`)도 함께 지워졌다.
- 시험 DB는 `ieo-poc-0203-pg`(127.0.0.1:55432)만 썼다.
  - 스키마 확인용 `rv4_fk`·`rv4_mig` DB를 만들었다가 지웠다.
  - 내 변이 실행 중(08:36Z) 남은 시험 DB `ieo_t_3330195_…` 하나를 지웠다. 그 밖에 08:29~08:35Z에 생긴 `ieo_t_*` 6개는 내 것이라고 확정할 수 없어 그대로 두었다(보통 실행에서는 남지 않음을 확인했다. 변이로 시험이 깨질 때 남는 것으로 보인다).

## 요약

- 재현 결과는 보고와 일치한다.
  - fmt, clippy(-D warnings), `cargo test --locked --all-targets`가 통과한다. 시험은 **142건**이다(28+6+29+1+33+4+16+25).
  - 변이 M03·M21·M22·M24·M26~M32 11개를 다시 돌렸고 모두 검출됐다. 내 추가 변이 Y1~Y7 중 Y1·Y2는 검출, Y3~Y7은 살아남았다(아래 S2·S3, Y6은 동등).
  - OpenAPI는 `openapi-spec-validator` OK다. `validate_package.py`와 `render_progress.py --check`는 EXIT=0이다.
  - 정책 SHA-256 `7b54ae55…`는 `PINNED_POLICY_DIGEST`와 같다. 기동 확인 기록도 같은 digest다.
- **R1(P2)은 해소됐다.** probe로 필드별로 확인했다.
  - status·estimate(회귀 시험), references·evidence·legacy_v1_2_id·kind만 바꾼 revision → 01-02 배정 **201**.
  - outputs·acceptance·exit_condition·verification·requirement_ids 중 하나를 바꾼 revision → **409 `PREREQUISITE_UNMET`**.
  - 코드·정책 0.4.0 G1·OpenAPI 문구가 서로 같다.
- R2·R3(P3)도 해소됐다.
- 새 지적은 P3 세 건뿐이다(S1~S3). P0/P1/P2는 남지 않았다.

## 이전 지적 처분 검증

### 1차 검토(1e98f17) F1~F10

| 지적 | 판정 | 근거 |
|---|---|---|
| F1 P1 교차 repo 스프린트 탈취 | **유지(해소)** | `register_spec`은 바뀌지 않았다. M21이 검출된다. |
| F2 P1 GitHub 검토가 머리 SHA에 묶이지 않음 | **유지(해소)** | 판정 코드는 바뀌지 않았다. M22·M26이 검출된다. |
| F3 P3 판정 대상 revision | **유지(해소)** | `judgments.sprint_revision`에 이제 외래 키까지 있다. M30이 검출된다. 개발 머리가 넘어간 뒤의 판정은 §4·`unresolved` 후속 그대로다. |
| F4 P3 GitHub author 독립성 | **수용 유지** | §4·`unresolved`에 남아 있다. |
| F5 P3 등록·배정 경쟁 창 | **유지(해소, 직접 시험 없음·공개)** | 배정은 `registry` 잠금 아래에서 G1을 조회한다. 바뀌지 않았다. |
| F6 P3 상한 재도달·assignment_id 경쟁 | **유지(해소)** | M24·M28·M29가 검출된다. |
| F7 P3 정책 해석 표시 | **유지(해소)** | G4 해석 문구는 0.4.0에서도 그대로다. |
| F8 P3 보고 수치·깨끗한 트리 | **해소** | 142건, 32개 중 31 검출(M13 동등), 커밋 efad0ee, porcelain 빈 목록이 재현과 같다. |
| F9 P3 reject 때 이전 SHA 승인 표시 | **유지(해소)** | M26이 검출된다. |
| F10 P3 커서·500·재연결·승격자 검토 | **유지(해소)** | 코드는 바뀌지 않았다. |

### 재검토(0fa839d) N1~N5

| 지적 | 판정 | 근거 |
|---|---|---|
| N1 P2 상한 되돌림 500 | **유지(해소)** | `api.rs:1539-1557` 상한 값별 확인은 바뀌지 않았다. M24·M28이 검출된다. |
| N2 P3 sprint_id 선점 | **수용 유지** | §4·`unresolved`에 남아 있다. |
| N3 P3 바뀐 작업의 옛 판정 | **해소** | 정의 비교는 이제 판정 관련 다섯 필드다(R1). 비교 전체를 지운 변이 Y1과 M27·M30이 검출된다. |
| N4 P3 경쟁 시험·직렬화 서술 | **유지(해소)** | M29가 경쟁 시험으로 검출된다. |
| N5 P3 이전 SHA changes_requested | **수용 유지** | §4·`unresolved`에 남아 있다. |

### 재검토 2(4bc8081) R1~R3

| 지적 | 보고 §8 처분 | 판정 | 근거 |
|---|---|---|---|
| R1 P2 G1이 task 객체 전체 비교 | 다섯 필드만 비교, 정책 0.4.0 | **해소** | `api.rs:1497-1511`. probe `rv4_probe_fields`(아래)에서 표시 필드 변경은 201, 정의 필드 변경은 409다. 회귀 시험이 status·estimate → 201과 verification → 409를 고정한다. M31·M32가 검출된다. |
| R2 P3 OpenAPI 문구 | 교정, G5 문장도 다시 씀 | **해소** | `ieo-api-v0.openapi.yaml:384-392`가 G1 다섯 필드와 G5 "위아래 어느 쪽이든 물은 적 없는 값이면 새 decision_request, 이미 물은 값이면 409"를 적는다. 코드(`api.rs:1539-1557`)와 같다. |
| R3 P3 이관 없음·외래 키 없음 | 외래 키 추가, schema.sql 머리 주석 | **해소** | 새 DB에 schema.sql을 적용하면 `judgments_sprint_id_sprint_revision_fkey`가 생기고, 없는 revision으로 INSERT하면 FK 위반이다. 4bc8081 스키마 위에 다시 적용하면 FK는 생기지 않는다(외래 키 1개). schema.sql 1-3행 주석이 이 동작("개발 DB는 다시 만든다")을 적는다. 병합 전이고 배포된 DB가 없으므로 수용한다. |

## 선행 항목 재확인

| 항목 | 판정 | 근거 |
|---|---|---|
| L07 / D12 | **충족** | 정책 0.4.0. 버전 상수, digest 고정, 규칙 7개 집합이 맞다. G1 "definition"이 필드 목록으로 정의됐고 코드와 같다. G7(필수 산출물만 판정)과도 어긋나지 않는다. 비교 필드가 판정이 실제로 읽는 `outputs`를 포함하고, 나머지는 더 엄격한 쪽이다(S1 참고). |
| L08 | **충족** | `REVISION_SUPERSEDED`·`already_issued`는 바뀌지 않았다. |
| 01-08 N2 | **해소** | 바뀌지 않았다. |
| PR #10 N1~N5 | **해소** | 바뀌지 않았다. 142건이 통과한다. |

## 새 지적

### S1 · P3 · `outputs`를 배열 전체로 비교해, 선택 산출물 설명이나 순서만 바뀌어도 선행 판정이 무효가 된다
- **위치**: `crates/ieo-coord/src/api.rs:1505-1508`(`'outputs', t->'outputs'`), `contracts/judgment-policy-v0.json` G1(0.4.0)·G7.
- **재현**: probe `rv4_probe_fields`. 01-01을 rev 5에서 accepted로 판정한 뒤 rev 6을 등록·검토하고 01-02를 배정한다.
  - `outputs` 두 항목의 순서만 바꿈 → **409 `PREREQUISITE_UNMET`**.
  - `required: false`인 `01-01-notes`의 `description`만 바꿈 → **409 `PREREQUISITE_UNMET`**.
- **기대**: G7은 선택 산출물을 "never verdict inputs"라고 한다. 판정(`api.rs:2135-2156`)도 필수 산출물의 `artifact_id`만 읽는다. 따라서 "fields a verdict depends on"을 엄밀히 따르면 이 두 변경은 판정을 무효로 하지 않아도 된다.
- **실제**: 무효가 된다. 다만 정책 0.4.0 G1 문장은 필드 단위("outputs")로 정의하므로 코드는 정책 문구와는 일치한다. 이런 변경은 계획 작성자가 산출물 목록을 고치는 드문 편집이고, 판정 쪽으로는 닫힌 실패(더 엄격)다. R1처럼 일상적인 진행 갱신이 막히는 경우가 아니어서 P3으로 둔다.
- **조치(후속)**: 다음 정책 버전에서 `outputs`를 "required=true 항목의 artifact_id 집합"처럼 판정 입력으로 정규화해 비교하는 것을 검토한다. 정책 문구가 `kind`·`prerequisites`·`legacy_v1_2_id`를 "무효로 하지 않는" 쪽 목록에 적지 않은 점도 함께 다듬는다(첫 문장이 정의를 다섯 필드로 한정하므로 의미는 모호하지 않다. probe에서 kind·legacy_v1_2_id 변경 → 201).

### S2 · P3 · 재지정된 M27이 이름과 다른 변이다(G1 술어 전체를 참으로 만듦)
- **위치**: `docs/poc/reports/tasks/02-03/coordinator-20261113/evidence/mutants.py:32`, `evidence/mutants.txt:33-34`.
- **내용**: 치환 결과는 `… AND (SELECT …jr…) IS NOT NULL OR true OR NULL = (SELECT …lr…))`이다. SQL에서 `OR`가 `AND`보다 우선순위가 낮다. 그래서 WHERE 전체가 `(…) OR true`가 되어, task·정책 digest·outcome과 상관없이 **아무 판정 행 하나만 있으면** 모든 선행 조건이 충족된다. 이름("prerequisite verdict of a changed task counts")과 mutants.txt 설명("comparison made always true")보다 넓다. 그래서 N3과 무관한 `prerequisites_need_a_verified_accepted_verdict`도 이 변이를 잡는다.
- **영향**: 증거 서술의 정확성 문제다. 원래 의도한 변이(비교만 제거, 내 Y1 `AND true)`)는 `verdicts_judge_the_assigned_revision_…`와 새 R1 시험이 검출하므로, 시험 공백은 없다.
- **조치(후속)**: 다음 증거 갱신 때 M27을 Y1 형태(정의 비교만 제거)로 고친다.

### S3 · P3 · 비교 필드 일부와 새 외래 키에 시험이 없다
- **내용**: 다음 변이가 살아남는다(`cargo test -p ieo-coord --all-targets`).
  - Y3 `acceptance`를 양쪽에서 뺌, Y4 `exit_condition`을 양쪽에서 뺌, Y5 `requirement_ids`를 양쪽에서 뺌.
  - Y7 `judgments`의 `(sprint_id, sprint_revision)` 외래 키 제거.
  - 회귀 시험은 `verification`(R1 시험)과 `outputs`(N3 시험)만 바꾼다.
- **동작 자체는 맞다**: probe에서 acceptance.criteria·exit_condition·requirement_ids 변경은 각각 409다. 외래 키는 psql로 확인했다(위 R3).
- **조치(후속)**: 필드별 표 시험 하나로 Y3~Y5를 잡을 수 있다. P3 결함이 아니라 회귀 방지 공백이다.

## 확인했고 문제없는 항목

- **G1 닫힌 실패(task 없음)**
  - 판정 revision이나 최신 revision에 그 task가 없으면 스칼라 부질의가 NULL이다. `jsonb_build_object` 비교는 NULL이 되고 EXISTS는 거짓이다. psql로 확인: `(NULL = obj) IS NULL` → t, `EXISTS(SELECT 1 WHERE NULL)` → f, `jsonb_array_elements(NULL)` 부질의 → NULL.
  - API 경로로는 이 상황을 만들 수 없다. 같은 스프린트에서 01-01을 지운 rev 6은 422 `UNDEFINED_PREREQUISITE`로 거부된다(probe). 판정은 배정 revision에 그 task가 있어야 하므로 `jr` 쪽은 항상 NULL이 아니다. 그래서 내 변이 Y6(`=` → `IS NOT DISTINCT FROM`)은 **동등 변이**다.
  - task 안에 필드가 없으면 `t->'x'`가 NULL이고 `jsonb_build_object`는 JSON `null` 값으로 넣는다. 양쪽이 같으면 같다고 본다. 스키마상 다섯 필드는 모두 필수라 실제로는 생기지 않는다.
  - 같은 task_id 중복은 등록에서 422 `DUPLICATE_TASK_ID`라 "more than one row"(500) 경로가 없다(재검토 2 확인 유지).
- **필드 선택(G1·G7)**: 판정 입력은 `outputs`(G7)뿐이고, `acceptance`·`exit_condition`·`verification`·`requirement_ids`는 사람이 보는 수용 기준이다. 이들을 포함하는 것은 더 엄격한 쪽이다. 진행 표시(`status`·`evidence`·`estimate`)와 서지(`references`·`legacy_v1_2_id`), `kind`, 선행 작업의 `prerequisites`는 판정을 무효로 하지 않는다. 선행 작업의 `prerequisites` 변경은 probe에서 순환(422 `DEPENDENCY_CYCLE`)으로 등록이 막혀 확인하지 못했지만, 비교식에 들어 있지 않음은 코드로 확인했다. task 배열 순서만 바꾼 revision → 201.
- **외래 키**: `sprint_revisions`의 PK가 `(sprint_id, revision)`이고 `judgments`의 두 열이 NOT NULL이라 FK가 항상 검사된다. 판정 INSERT는 배정 revision을 쓰므로 기존 시험이 모두 통과한다.
- **정책 0.4.0과 코드**: G1 문구와 SQL이 같다. G5 문구("위아래로 바뀐 새 값이면 새 decision_request, 물은 값이면 409")가 `api.rs:1539-1557`과 같다. 버전 상수 `0.4.0`, digest 고정, 규칙 7개는 그대로다. 기동 확인은 바뀐 정책 파일로 기동을 거부한다(exit 2, digest 7b54ae55 기대).
- **OpenAPI**: 설명만 바뀌었다(0.6.0 유지). 검증기 OK다. G1·G5 설명이 정책 0.4.0과 같다.
- **보고 일관성**: result.json의 repo_sha efad0ee, revision 4, 142건, 32/31, `previous_run_ref`가 재현과 같다. P04를 주장하지 않는다. §8 처분 표는 위 검증과 같다. §7의 "정책 G1 0.3.0"은 당시 처분 기록이라 그대로 두어도 된다.

## 실행한 명령

```
df -h /                                                              # 3.2G free at start
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree add --detach <scratch>/rv4-0203 66958e2
git log --oneline c501a44..HEAD; git diff 4bc8081 66958e2 -- contracts crates <report> <evidence>/mutants.py
git diff --stat efad0ee 66958e2 -- crates contracts                 # empty
export PATH=/opt/rust/cargo/bin:$PATH CARGO_TARGET_DIR=<scratch>/rv4-0203/target-rv4
export IEO_TEST_DB_URL="host=127.0.0.1 port=55432 user=ieo dbname=ieo password=<file>"
cargo fmt --all -- --check                                          # OK
cargo clippy --locked --all-targets -- -D warnings                  # OK
cargo test --locked --all-targets                                   # 28+6+29+1+33+4+16+25 = 142 passed
<scratch>/myscripts/rv4-mut.py (evidence/mutants.py의 M 목록 재사용, cargo test -p ieo-coord --all-targets)
  M03 M21 M22 M24 M26 M27 M28 M29 M30 M31 M32: killed (mutants.txt와 동일)
  Y1 definition comparison removed (AND true): killed
  Y2 outputs dropped from both sides: killed
  Y3 acceptance dropped / Y4 exit_condition dropped / Y5 requirement_ids dropped: SURVIVED (S3)
  Y6 '=' → IS NOT DISTINCT FROM: SURVIVED (동등: jr 쪽은 항상 non-NULL)
  Y7 judgments (sprint_id, sprint_revision) FK removed: SURVIVED (S3)
  dirty after run: none
probe test (tests/coordinator.rs에 임시 추가 → git checkout으로 원복): rv4_probe_fields
  control 201 | acceptance.criteria 409 | exit_condition 409 | requirement_ids 409
  outputs 순서 409 | 선택 산출물 description 409 (S1)
  references 201 | evidence 201 | legacy_v1_2_id 201 | kind 201 | task 배열 순서 201
  01-01 prerequisites 추가 → 등록 422 DEPENDENCY_CYCLE | 01-01 삭제 → 등록 422 UNDEFINED_PREREQUISITE
psql(podman exec ieo-poc-0203-pg): NULL 비교 의미 확인 (t|f|t)
psql: CREATE DATABASE rv4_fk; schema.sql 적용 → FK 2개, 없는 revision INSERT → FK 위반; 재적용 OK
psql: CREATE DATABASE rv4_mig; 4bc8081 schema.sql → 66958e2 schema.sql → 성공, FK 1개(주석대로); 두 DB DROP
cargo test -p ieo-coord 재실행 전후 ieo_t_* 목록 비교 → 차이 없음(정상 실행은 DB를 남기지 않음)
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml       # OK
(cd docs/poc && .venv-poc/bin/python scripts/validate_package.py)           # EXIT=0
(cd docs/poc && .venv-poc/bin/python scripts/render_progress.py --check)    # ok, EXIT=0
sha256sum contracts/judgment-policy-v0.json                         # 7b54ae55… == PINNED_POLICY_DIGEST
git -C /opt/rust/src/github.com/HeaInSeo/ieo_poc worktree remove --force <scratch>/rv4-0203
```
