# 02-03 · DB 스키마·API·소유권·사건 기록

| 항목 | 기록 |
| --- | --- |
| 계획·작업·회차 | v1.3.3-A2 · 02-03(구현, T06) · run `coordinator-20261113` |
| 수용 기준 | `poc-acceptance-v1.3.3` rev 4 `02-03.accept`(장부 02-03 행과 같음): 명세 v0·보고 v0·인증된 HTTP/JSON API를 따르는 배정·권한 세대·사건 ID·이력·outbox·조회 모델 구현, 형식 검사와 의미 검토 분리, GitHub/실행 증거를 완료 판정 입력으로, 기동 확인과 P04 통과 구분 |
| 선행 | 02-01·01-02·01-07·01-08 검수 완료(원장 snapshot `v1.3.3-EXEC-0013`, main `c501a44`에서 착수 확인) |
| 산출물 | Rust crate [`crates/ieo-coord`](../../../../../../crates/ieo-coord)(라이브러리 + `ieo-coord migrate|serve`) · DB 스키마 [`schema.sql`](../../../../../../crates/ieo-coord/src/schema.sql) · 판정 정책 [`contracts/judgment-policy-v0.json`](../../../../../../contracts/judgment-policy-v0.json)(L07) · OpenAPI 0.6.0 · 01-08 계약 rev 4 |
| 상한 | F05 구현 14,400초. 측정 시작 2026-11-13T07:25:10Z(선행 항목 읽기 몇 분은 이 시각 앞이라 포함하지 않음), 코드 커밋 0850eaf 07:44:10Z, 시험 보강 3e24286 07:47:36Z(약 1,350초), 검토 반영 dfac60a 08:00Z경(약 2,150초), 재검토 반영 eb7b0af·02c61a0 08:17Z경(약 3,100초), 재검토 2 반영 efad0ee 08:29Z경(약 3,800초). 검토·교정은 F04(4회·3회) |
| 모델 호출 | 0회. 실제 에이전트·GitHub 연결 없음(§5) |
| 상태 | rev 4. 독립 검토 [1e98f17 reject](reviews/review-pr25-1e98f17.md)(P1 F1·F2) → 교정 dfac60a(§6) → 재검토 [0fa839d reject](reviews/review-pr25-0fa839d.md)(P2 N1) → 교정 eb7b0af·02c61a0(§7) → 재검토 2 [4bc8081 reject](reviews/review-pr25-4bc8081.md)(P2 R1) → 교정 efad0ee(§8). 02-03 검토 3/4·**교정 3/3(상한 도달, 더 교정할 수 없음)**. 마지막 검토(4/4) 대기 |

## 1. 선행 항목 처분

| 항목 | 착수 시 상태 | 처분 | 근거 |
| --- | --- | --- | --- |
| L08 두 결함(01-08 N1·01-07 N1) | PR #10에서 교정·검토 승인(ea902dd)·병합(066f5fb) | **재사용**. 영속 구현이 같은 규칙을 그대로 쓴다(`ieo_report::authorize_claim`/`authorize_repro`, `Registry`) | 시험 `claims_late_duplicate_stale_and_mismatched`·`repro_requests_new_must_be_current_replays_are_not_requeued`·`draft_and_superseded_revisions_are_not_assignable` |
| L07 권한·판정 의미 공백(D12) | 미처리 | **이번에 고정**. 최소 방식 중 “버전·digest가 고정된 기계 판독 정책 참조”를 택했다. 명세 스키마(r4)와 9개 명세 digest를 바꾸지 않기 위해서다. 정책 파일(현재 0.4.0, digest `7b54ae55…`)은 G1 선행 증거·G2 SHA 귀속·G3 독립 판정·G4 누적 시도·G5 한도 도달·RG2 승인 범위·G7 제외 범위를 규칙으로 적는다. 숫자 값은 새로 정하지 않는다(상한 값은 명세 `limits`에서만). 코디네이터는 고정 digest와 다른 바이트, 또는 구현과 다른 규칙 집합이면 기동하지 않는다. 배정·판정마다 정책 digest를 기록한다 | §2.4, 시험 `policy_is_pinned_and_fails_closed`, 기동 확인 [startup-smoke.txt](evidence/startup-smoke.txt) 마지막 항목 |
| 01-08 N2 | 미처리 | **교정**. `completion_summary`가 확인된 범위 전부와 미완료 범위 전부를 쓴다. 현재 SHA 승인 없는 병합도 숨기지 않는다. 계약 01-08 rev 4 | `report_contract.rs::completion_summary_scopes`, 변이 M20 |
| PR #10 N1 | 문서 공백 | **교정**. 01-08 계약 검사 순서에 본문 SHA 대조 단계, `POST /v0/agent/reports` 409 목록에 `ASSIGNMENT_SHA_MISMATCH`, 본문 SHA `null` 처리 명시, OpenAPI 단어 중복 제거 | 계약 rev 4 §1·§2 |
| PR #10 N2 | Registry가 호출자 digest 문자열을 신뢰 | **교정**. `Registry::add_spec_with_mode`가 digest를 다시 계산해 없거나 다르면 기록하지 않는다 | `registry_recomputes_the_digest_and_refuses_missing_or_forged_ones`, 변이 M19 |
| PR #10 N3 | 재전송 영수증의 `counts_for_current_judgment`가 항상 false | **교정**(영속 구현). 응답 시점에 그 (배정, 세대)가 여전히 현재인지로 다시 계산한다. `disposition`은 저장값 그대로이므로 둘이 다를 수 있음을 계약에 적었다 | 시험 `claims_late…`, 변이 M06 |
| PR #10 N4 | 현재 판정 입력의 시점 미정 | **교정**. 판정 시점의 현재 Guardrail (배정, 세대)의 claim만 검토 입력이다. 같은 SHA라도 세대가 바뀌면 옛 승인은 입력이 아니다 | `review_inputs_are_current_independent_and_for_the_head`, 변이 M07 |
| PR #10 N5 | 증거 로그 머리줄 설명 부족 | **반영**. 증거 로그 첫 줄에 커밋 SHA와 실행 전 `git status --porcelain`을 기록한다. rev 1 로그는 보고 디렉터리가 이미 있어 빈 목록이 아니었다(검토 F8). rev 2 로그는 저장소 밖에 쓴 뒤 복사해 실제로 빈 목록이다 | [cargo-test.log](evidence/cargo-test.log) 첫 줄 |

## 2. 구현 결과

### 2.1 구성

외부 PostgreSQL 17과 인증된 HTTP/JSON(계획 §1.2의 첫 PoC 시험 구성)이다. 요청 하나가 DB 트랜잭션 하나다. 2xx만 커밋하고 그 밖은 롤백하므로 거부된 요청은 행을 남기지 않는다(OpenAPI “Every non-2xx response leaves server state unchanged”). DB 접속 정보는 환경 변수 `IEO_DB_URL`로만 받고 argv에 두지 않는다. 자격은 토큰의 SHA-256만 보관한다.

| 요소 | 구현 |
| --- | --- |
| 인증·권한 | Bearer 토큰 → 서버 쪽 주체 {kind user/service/agent, scopes, repos, role}. 에이전트는 repo 하나·역할 하나·`agent:*` 범위만 갖는다. `judge:verify`는 service만 갖는다. 어긋난 자격 파일은 기동을 거부한다. 본문의 이름은 권한 근거가 아니다 |
| 명세 등록(01-07) | `POST /v0/sprints`·`/v0/sprint-inputs`. 01-07 검사(형식→참조→바인딩)와 멱등 키(주체·엔드포인트별), L08 revision 규칙을 그대로 쓴다. sprint_id는 첫 revision의 repo에 속하며 다른 repo를 지정한 revision은 409 `SPRINT_REPO_CONFLICT`(검토 F1). 등록 성공은 **의미 검토 대기 접수**일 뿐이다 |
| 의미 검토(분리) | `POST …/revisions/{rev}/semantic-review`. 그 revision을 등록하거나 executable로 승격한 주체는 검토할 수 없고(G3, 검토 F10), 검토는 그 revision의 digest를 명시한다(RG2). revision마다 한 번 기록한다. **형식 검사를 통과해도 의미 검토가 `passed`가 아니면 배정하지 않는다** |
| 배정·권한 세대·소유권 | `POST /v0/coordinator/assignments`. (repo, role)마다 현재 머리(head) 하나를 둔다. `expected_current`로 비교 후 교체(CAS)하며, 다르면 409 `OWNERSHIP_CONFLICT`. 세대 n은 같은 배정의 n−1이 머리일 때만 발급한다. 같은 요청을 다시 보내면 200 역사적 조회일 뿐 재활성화하지 않는다. 대상 revision은 최신·executable·의미 검토 통과여야 한다 |
| 사건 ID·이력 | 보고 v0 사건을 (scope repo, role, `event_id`, `event_revision`)으로 유일하게 저장한다. 스프린트마다 커밋 순서 번호를 매긴다(스프린트별 잠금을 커밋까지 쥐므로 뒤 번호가 앞 번호보다 먼저 보이지 않는다). `assignments`·`events`·`evidence`·`judgments`·`semantic_reviews`·`history`는 DB 트리거로 UPDATE/DELETE를 거부한다 |
| 에이전트 경로(01-08) | `POST /v0/agent/reports`·`repro-requests`. 01-08/L08 규칙을 DB의 발급 이력과 저장 레코드로 실행한다. claim은 그 배정에 기록된 sprint/revision/task와 같아야 한다(다르면 422) |
| 증거 입력 | `POST /v0/coordinator/evidence`(`evidence:ingest`): `ci_run`·`run_record`(02-01 기록, `judgment.counts_as_success`만 통과로 봄, 종료 코드만으로 판단하지 않음)·`review`·`merge`·`main_ci`·`output`·`followups` |
| 완료 판정 | `POST /v0/coordinator/judgments`(service + `judge:verify`). 아래 §2.3. 판정 사건(verified)·outbox·판정 행을 한 트랜잭션에 기록한다 |
| outbox·전달 | `GET /v0/coordinator/outbox`, `POST …/outbox/{id}/delivery`. pending → sent 또는 unknown, unknown → sent, sent는 최종 상태다. 전달 상태는 작업 판정과 별개다 |
| 조회 모델 | `GET …/state`: 작업별 최신 verified와 최신 claim을 **다른 필드**로 돌려준다. 늦게 도착한 claim은 이력일 뿐 상태가 아니다. `delivery`는 최신 verified의 outbox 상태이며, verified가 아직 없으면 `unknown`이다. `GET …/events`: 커서 페이지(`s<번호>`). 서버가 발급하지 않은 커서는 400 `INVALID_CURSOR`. 조회는 행을 만들지 않는다 |
| 기동 확인 | `GET /v0/health`: 프로세스 응답과 DB 도달만 본다. **P04 통과가 아니다**(본문 `scope`에 명시) |

### 2.2 경계 조건(기존 계약)

| 조건 | 동작 | 시험 |
| --- | --- | --- |
| 소유권 충돌(두 발급자가 같은 머리를 교체) | 하나만 201, 나머지 409 `OWNERSHIP_CONFLICT`. 8개 동시 요청에서 정확히 1건 | `ownership_head_is_compare_and_swap`, `concurrent_duplicates_and_competing_issuers` |
| 중복 요청(같은 claim·평가·등록) | 같은 내용이면 200 재조회, 다른 내용이면 409. claim 8개 동시 전송: 202 1건, 200 7건, 행 1개. 같은 revision을 서로 다른 키로 6개 동시 등록하면 201 1건과 200 5건 | 같은 두 시험 |
| 늦게 도착한 결과 | 발급 이력이 있는 옛 세대의 claim은 202 `preserved_late`. 그 세대의 SHA에 귀속하고 현재 판정에서 제외한다. 발급 이력이 없으면 409 | `claims_late_duplicate_stale_and_mismatched` |
| 옛 세대 승인(같은 SHA) | 판정 시점에 현재 세대가 아니면 검토 입력이 아니다 | `review_inputs_are_current_independent_and_for_the_head` |
| 재전송 뒤 새 stale 재현 요청 | 같은 ID 재전송은 200(다시 대기열에 넣지 않음), 새 요청은 현재 배정·SHA만 받는다 | `repro_requests_new_must_be_current_replays_are_not_requeued` |
| 거부의 무상태성 | 모든 거부 사례에서 12개 표 행 수가 바뀌지 않는다. 첫 쓰기 뒤 실패(결함 주입)도 전부 롤백된다 | `authentication_and_scope_refusals_change_nothing`, `a_failure_after_the_first_write_rolls_back_the_whole_request` |
| 서로 다른 역할의 동시 기록 | 개발·Guardrail claim 24건 동시: 모두 202, 순서 번호 1..24 빈틈 없음 | `development_and_guardrail_claims_race_into_one_ordered_stream` |
| 다른 repo | 다른 repo 에이전트의 claim은 403, 다른 repo 대상 배정은 422 `REPO_MISMATCH`, 다른 repo 스프린트 조회는 404. 다른 repo가 같은 sprint_id의 새 revision을 등록하면 409 `SPRINT_REPO_CONFLICT`이고 원 repo의 조회·배정은 그대로다(sprint_id 이름 공간은 전역이라 그 ID가 쓰인다는 사실은 드러난다) | `independent_repos_progress_separately`, `a_sprint_cannot_be_moved_to_another_repo` |

### 2.3 완료 판정의 입력(GitHub·실행 증거)

머리 SHA는 현재 개발 배정에 코디네이터가 기록한 `target_sha`다(G2). 에이전트 본문의 SHA는 귀속 근거가 아니다. 작업·산출물은 그 배정이 발급된 revision의 것을 쓴다(검토 F3).

| 범위 | 입력 |
| --- | --- |
| 현재 SHA CI | 머리 SHA의 `ci_run`·`run_record`. `source_ref`마다 최신 것만 보며, 하나 이상 있고 모두 통과해야 한다 |
| 현재 SHA 독립 검토 | 판정 시점의 현재 Guardrail (배정, 세대)이 이 작업에 낸 최신 `guardrail_review` claim과, **머리 SHA에 대한** GitHub 검토(작성자와 다른 검토자마다 approved·changes_requested·dismissed 중 최신 하나. 뒤의 commented는 changes_requested를 풀지 않고 dismissed는 푼다. 다른 SHA·PR의 검토는 세지 않는다, 검토 F2). 머리 SHA에 현재 reject·needs_confirmation·changes_requested가 있으면 승인으로 보지 않으며, 이때도 이 작업의 이전 SHA 승인은 “현재 판정에 쓰지 않음”으로 표시한다(F9) |
| 병합 | `head_sha`가 머리 SHA인 `merge` |
| main CI | 그 병합 SHA의 `main_ci` |
| 필수 산출물 | `required=true` 산출물만 본다. 선택 산출물은 “판정 제외(G7)”로 나열한다 |
| 후속 인수 | 병합 SHA의 `followups` accepted |

결과는 verified `coordinator_verdict`이며 `evidence_comparison`에 범위별 완료 문구(01-08 N2 교정본)를 쓴다. 모든 범위가 참일 때만 `accepted`다. 에이전트의 완료 claim만으로는 선행 조건을 충족하지 않는다(시험 `prerequisites_need_a_verified_accepted_verdict`).

### 2.4 판정 정책(L07) 적용 위치

| 규칙 | 강제 위치 |
| --- | --- |
| G1 선행 증거 | 배정 발급: 같은 정책 digest의 verified 판정(`accepted` 또는 `result_recorded`)이 없으면 409 `PREREQUISITE_UNMET`. 판정이 본 revision과 스프린트 최신 revision에서 그 작업의 **판정 관련 필드**(outputs·acceptance·exit_condition·verification·requirement_ids)가 같아야 한다. 이 필드가 바뀌면 새 판정이 필요하고, status·evidence·estimate·references 변경은 판정을 무효로 하지 않는다(재검토 N3, 재검토 2 R1) |
| G2 SHA 귀속 | 판정: 머리 SHA·병합 SHA로만 증거를 고른다. claim 저장: 배정의 SHA에 귀속 |
| G3 독립 판정 | `judge:verify`는 service만 가진다. 자기 검토(의미 검토 등록자, GitHub 작성자=검토자)는 제외한다. 현재 세대 claim만 검토 입력이다 |
| G4 누적 시도 | (sprint, task, role)별로 배정 ID와 세대를 가로질러 센다. 첫 발급 뒤 세대마다 1회를 쓰며, 상한은 명세 `limits.correction_attempts`. **해석**: G4 공백은 교정과 별도의 재시도 상한이지만 그 값이 결정된 적이 없어, 재시도와 교정을 한 상한으로 센다(더 엄격한 쪽, 새 숫자 없음, 검토 F7). 별도 상한이 필요하면 사용자 결정 뒤 정책 새 버전에서 나눈다 |
| G5 한도 도달 | 상한을 넘는 첫 요청: 새 세대 없이 verified `decision_request`와 outbox를 한 번 기록하고 202 `limit_reached`(현재 담당 유지, 기본 조치 “정지 유지”). 이미 물었던 상한 값이면(언제 물었든) 409 `LIMIT_REACHED`이며 상태 변화가 없다. 새 revision이 아직 묻지 않은 값으로 상한을 바꾼 뒤 도달하면 새 decision_request를 기록한다(검토 F6, 재검토 N1) |
| RG2 승인 범위 | **대상 승인 범위**로 해석했다(검토 F7). 01-08 §5의 RG2(결정 답변이 허용하는 행동 범위)는 02-03이 결정 답변을 기록하지 않으므로 쓰이지 않으며, 필드화는 다음 contract revision 후보로 남는다. sprint_id는 한 repo에 속하고, 의미 검토는 digest를 명시하며, 배정은 (배정, 세대, SHA), 판정은 (작업, 배정 revision, 머리 SHA, 병합 SHA)에만 적용한다 |
| G7 제외 범위 | 선택 산출물·`out_of_scope`·`preserved_unrepresented`는 판정 입력이 아니다 |

## 3. 검증

| 확인 | 결과 | 증거 |
| --- | --- | --- |
| fmt·clippy(-D warnings)·전체 시험 | 통과. 시험 **142건**: ieo-coord 28(PostgreSQL 실DB·실HTTP)·ieo-report 1+33·ieo-spec 4+16+25·ieo-evidence 6+29. 커밋 efad0ee, 깨끗한 트리(실행 전 porcelain 빈 목록) | [cargo-test.log](evidence/cargo-test.log) |
| 변이 | 32개 중 31개 검출(검토 반영 변이 M21~M32 포함, 커밋 efad0ee). M13(거부 시 롤백 대신 커밋)은 **동등 변이**다. PostgreSQL은 실패한 트랜잭션의 COMMIT을 롤백으로 처리하고, 현재 코드에는 실패 없이 쓰기 뒤 거부하는 경로가 없다. 결함 주입 시험이 같은 성질(첫 쓰기 뒤 실패 시 전부 롤백)을 직접 고정한다 | [mutants.txt](evidence/mutants.txt), [mutants.py](evidence/mutants.py) |
| OpenAPI 0.6.0 | `openapi-spec-validator` OK. 서버가 내는 모든 오류 코드가 OpenAPI 열거에 있다(시험 `every_emitted_code_is_declared_in_the_openapi_contract`) | [openapi-validate.log](evidence/openapi-validate.log) |
| 문서 정합 | `validate_package.py` EXIT=0, `render_progress.py --check` EXIT=0. 문서 정합 확인일 뿐 런타임 PASS가 아니다 | [validate.log](evidence/validate.log) |
| 기동 확인(P04 아님) | 실제 바이너리를 띄워 health 200(`scope`에 P04 아님 명시), 자격 없는 요청 401, 다른 repo 스프린트 404, 이관 뒤 13개 표를 확인했다. 변경한 정책 파일로는 기동을 거부한다(exit 2). 토큰·DB 비밀번호는 기록에 없다 | [startup-smoke.txt](evidence/startup-smoke.txt) |
| CI | `rust-check`에 PostgreSQL 17 서비스(로컬과 같은 이미지 digest)를 붙였다. CI에서 DB가 없으면 시험이 실패하고 건너뛰지 않는다 | `.github/workflows/ci.yml` |

**모의·고정 입력 범위와 실제 실행 범위.** DB·HTTP·트랜잭션·잠금·동시성은 실제 PostgreSQL과 실제 서버로 시험했다. GitHub 증거와 에이전트 claim은 시험이 만든 고정 입력이다. 실제 GitHub API·실제 에이전트·실제 재현 실행기는 연결하지 않았다.

## 4. 범위 밖과 후속

| 항목 | 담당·시점 |
| --- | --- |
| P04(배정·DB 기록 시험) 통과 판정 | 04-03·04-04 등 해당 검증 작업. 이 보고의 기동 확인은 P04가 아니다 |
| 실제 GitHub 증거 수집기(폴링·웹훅)와 실제 에이전트 연결 | 02-04(supervisor)·02-05(문서 버전·배정 연결)·EARLY-01. 실제 GitHub 연결은 GH1~GH5 충족 뒤에만(04-07·05-01~05-03 착수 조건 그대로) |
| 재현 실행기 | 02-06. 이 작업은 요청 접수·상태 조회까지만 한다(`queued`) |
| 개발 머리가 다른 작업으로 넘어간 뒤 앞 작업의 판정(현재는 409 `NO_CURRENT_ASSIGNMENT`, 검토 F3) | 02-04·02-07에서 판정 머리를 “그 작업의 마지막 개발 배정”으로 할지 정한다 |
| GitHub 검토 작성자 신원 확인(현재는 수집 서비스가 준 `author` 문자열, 검토 F4) | 실제 수집기(02-04·02-05)가 GitHub에서 PR 작성자를 직접 얻는다 |
| 판정과 outbox의 원자 기록 검증, 보고 대기, 에이전트 완료 메시지 없는 정상 회차 판정 | 02-07. 이 작업은 판정·outbox·판정 행을 한 트랜잭션에 기록하는 모델과, 실패 주입 시 함께 롤백되는 시험까지 한다 |
| 사용자 결정 기록 경로(G5 뒤 재개, RG1·RG2 필드화) | 결정 API는 이번 범위가 아니다. G5 뒤에는 자동으로 계속하지 않는다. 필드화는 정책상 다음 contract revision 후보로 둔다(D12의 “표현 개선은 후속”) |
| 시간·토큰 한도(`elapsed_seconds`·`total_tokens`) 집행 | 02-04 supervisor(02-01 계측기 연결). 토큰 총량 정의는 L01/L02 |
| 쓰기 처리량·잠금 경합의 성능 특성 | 측정하지 않았다. 등록·의미 검토·**배정**은 전역 `registry` 잠금 하나로 직렬화된다(배정은 검토 F5 교정 뒤부터, 재검토 N4). 사건은 스프린트별 잠금을 쓴다. 독립 repo의 동시 진행 시험은 04-04 |
| sprint_id 이름 공간(첫 등록 repo가 가짐, 다른 repo의 draft로 선점 가능하고 409가 존재를 드러냄, 재검토 N2) | repo별 승인된 sprint 바인딩(01-07 §3의 바인딩, 02-05 문서 버전·배정 연결)에서 sprint_id를 repo에 미리 묶는다 |
| 새 push 뒤 이전 SHA의 changes_requested를 무시하는 규칙과 GitHub PR 상태의 차이(재검토 N5) | 실제 수집기(02-04·02-05)에서 PR 수준 검토 상태를 함께 수집할지 정한다 |

## 5. 비용·한도

- 모델 호출 0회. 02-03에 동결된 모델 예산은 쓰지 않았고 다른 작업의 예산도 쓰지 않았다.
- 대화 세션의 토큰 사용량은 계측 수단이 없어 **미계측**이다(0이 아님).
- 시험 DB는 tmpfs 위의 일회성 컨테이너(`ieo-poc-0203-pg`)였다. 기존 컨테이너·클러스터·kind/sudo 설정은 바꾸지 않았다. 시험이 만든 DB는 시험마다 삭제했다(남은 시험 DB 0개 확인).

## 6. 독립 검토 반영(rev 2)

[검토 1e98f17](reviews/review-pr25-1e98f17.md): reject(P1 2건). 재현한 증거(135건·변이 19/20·M13 동등 판단·OpenAPI·문서 정합)는 보고와 같았다.

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| F1 P1 다른 repo가 같은 sprint_id의 새 revision을 등록해 스프린트를 가로챔 | 교정. sprint_id는 첫 revision의 repo에 고정된다. 다른 repo의 revision은 409 `SPRINT_REPO_CONFLICT`(등록·변환 공통) | `a_sprint_cannot_be_moved_to_another_repo`, M21 |
| F2 P1 GitHub 검토를 SHA 구분 없이 검토자별 최신으로 골라, 뒤의 comment나 다른 PR 승인이 현재 changes_requested를 지움 | 교정. 머리 SHA의 검토만, 검토자별 approved·changes_requested·dismissed 중 최신 하나 | `github_reviews_count_only_at_the_head_and_comments_do_not_clear_changes_requested`, M22·M23 |
| F3 P3 판정 대상 revision이 최신 revision | 교정: 배정이 발급된 revision으로 판정. 머리가 다른 작업으로 넘어간 경우는 §4 후속 | — |
| F4 P3 GitHub author 독립성은 수집기 문자열에 의존 | 정책 0.2.0에 명시, §4 후속 | — |
| F5 P3 배정이 registry 잠금 없이 revision 확인 | 교정: 배정이 `registry` 잠금(순서 scope → assignment-id → registry)을 잡는다 | — |
| F6 P3 상한 재도달 시 결정 요청 없음, 역할 간 같은 assignment_id 경쟁의 500 | 교정: (작업, 역할, 상한)마다 한 번 묻는다. assignment_id 잠금 추가 | `a_raised_ceiling_reached_again_asks_again`, M24 |
| F7 P3 정책 해석(재시도·교정 합산, RG2 의미) | 정책 0.2.0과 §2.4에 해석을 드러냈다 | — |
| F8 P3 보고 수치·깨끗한 트리 표기 | 교정(§3 내역, 로그 재생성) | — |
| F9 P3 reject가 막을 때 이전 SHA 승인 표시가 사라짐 | 교정 | 위 F2 시험, M26 |
| F10 P3 커서·500 선언·DB 끊김·승격자 검토 | 500을 모든 엔드포인트에 선언, worker가 DB에 다시 연결하고 실패하면 500으로 응답, 승격자도 검토 불가(`whoever_registered_or_promoted_a_revision_cannot_review_it`, M25). 커서는 유지: 계약상 커서는 “위치”이며 기존 사건 뒤의 위치(`s3`)는 유효한 위치라 정상 페이지를 돌려준다 | — |

## 7. 재검토 반영(rev 3)

[재검토 0fa839d](reviews/review-pr25-0fa839d.md): reject(P2 1건). P1 F1·F2는 해소로 판정됐다.

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| N1 P2 상한을 낮췄다가 이전 값으로 되돌리면 같은 decision 사건을 다시 넣어 매번 500 | 교정. 마지막 값만 기억하지 않고, 그 상한 값의 decision 사건이 이미 있는지로 판단한다. 있으면 409 `LIMIT_REACHED`(상태 변화 없음) | `a_raised_ceiling_reached_again_asks_again`(1→2→1), M28 |
| N2 P3 sprint_id 선점·존재 노출 | §4 후속(repo별 sprint 바인딩) | — |
| N3 P3 F3에 시험 없음, 바뀐 작업의 옛 판정이 선행 조건을 채움 | 교정. 판정 행에 판정한 revision을 기록하고, 선행 조건은 그 작업 정의가 최신 revision과 같을 때만 충족(정책 G1 0.3.0) | `verdicts_judge_the_assigned_revision_and_a_changed_task_needs_a_new_verdict`, M27·M30 |
| N4 P3 경쟁 교정·재연결에 시험 없음, 배정 전역 직렬화 미기재 | 역할 간 같은 assignment_id 경쟁 시험(10회, 500 없음) 추가, §4 문구 교정. 배정이 잡는 `registry` 잠금이 모든 배정을 직렬화하므로 별도 assignment-id 잠금은 중복이라 제거했다(그 잠금 제거 변이 M29가 살아남은 이유). `registry` 잠금 제거 변이(M29 재지정)는 경쟁 시험이 검출한다. 재연결은 검토자 수동 재현으로 확인됨, 등록·배정 경쟁(F5)은 시간 창이 좁아 자동 시험을 두지 않았다 | `one_assignment_id_raced_by_two_roles_never_errors`, M29 |
| N5 P3 이전 SHA changes_requested 무시와 GitHub PR 상태 차이 | §4 후속 | — |

## 8. 재검토 2 반영(rev 4)

[재검토 2 4bc8081](reviews/review-pr25-4bc8081.md): reject(P2 1건). N1·N3·N4는 해소, N2·N5는 후속으로 수용됐다. F1~F10은 유지됨으로 판정됐다.

| 지적 | 처분 | 시험·변이 |
| --- | --- | --- |
| R1 P2 G1이 작업 객체 전체를 비교해 상태 라벨·추정치만 바뀌어도 선행 판정이 무효가 됨 | 교정. 판정 관련 필드(outputs·acceptance·exit_condition·verification·requirement_ids)만 비교한다. 정책 0.4.0 G1에 필드를 나열했다. 어느 쪽 revision에 작업이 없으면 NULL이라 미충족(닫힌 실패)이다 | `label_and_estimate_changes_keep_a_prerequisite_verdict_but_definition_changes_void_it`, M31·M32, M27(재지정) |
| R2 P3 OpenAPI 문구(상한을 낮춘 새 값, G1 정의 조건) | 교정. 정책 G5 문장도 다시 썼다 | — |
| R3 P3 이전 커밋의 개발용 DB에 새 스키마를 적용하면 `judgments.sprint_revision`이 없음, 외래 키 없음 | 외래 키 `(sprint_id, sprint_revision) → sprint_revisions`를 추가했다. 병합 전 스키마라 이관을 두지 않고 schema.sql 머리에 “이전 커밋의 개발 DB는 다시 만든다”고 적었다. 배포된 DB는 없다 | — |
