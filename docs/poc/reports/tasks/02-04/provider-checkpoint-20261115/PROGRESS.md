# 02-04 진행 기록 9 · 공급자 상태·공유 한도 그룹(항목 4)과 중앙 체크포인트(항목 9)

2026-11-15 KST · 계획 v1.3.3 · 근거 결정: [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0204-acceptance.md)(수용 보고 §4 선택지 (나))

**이 기록은 02-04 수용이 아니다.** 사용자 결정에 따라 항목 4·9를 02-04 수용 전에 구현하고 시험했다. 독립 검토, CI, 승인 head 병합, main CI를 거친 뒤 수용 기록에서 상태를 바꾼다. 완료 수는 바꾸지 않았다(main 11/73, 후속 0/12).

- **모델 호출:** 0회. 시험은 가짜 `claude`, 실제 PostgreSQL(시험 DB·컨테이너 안 일회용 DB), 저장된 원본 형 정의만 썼다. 예비 회차(개발 2·Guardrail 3)는 쓰지 않았고 실제 중간 정지도 하지 않았다.
- **예산:** 바뀌지 않았다. 7/12회, 40.666초(집계 45초), billable 33,948, cache_read 121,189.
- **코드:** 커밋 5f38ab9, 교정 1 ec64d19, 교정 2 a1cfe78. 바꾼 곳은 다음과 같다.
  - supervisor: 새 모듈 `provider`·`central`, `serve` 연결
  - coordinator: 스키마 표 2개, API 3개, `roles` 공유 그룹
  - OpenAPI
  - 런타임 계약 revision 9(§13 항목 9·10)
- **이미지:** a1cfe78(교정 2)에서 릴리스 빌드했다([build-images-a1cfe78.txt](evidence/build-images-a1cfe78.txt)).
  - `ieo-coord`: c69c49a4…
  - `ieo-supervisor`: ce807e09…
  - CLI: bfb3703a…(2.1.322, 바뀌지 않음)

## 1. 결과 요약

| 항목 | 상태 | 구분 | 근거 |
| --- | --- | --- | --- |
| 4. 원시 공급자 신호 → §7 값 | 구현·시험 | 모의(가짜 런타임)·형 정의 | §2 |
| 4. `quota_group_id` 명시 설정, 공유 그룹 연결 | 구현·시험 | 모의 | §2 |
| 4. 출처·관측 시각·신선도, 실행 중·재시작 뒤 미확인 | 구현·시험 | 모의 | §2 |
| 9. 체크포인트 순번·digest·변조의 코디네이터 보고·저장 | 구현·시험 | 모의·실제 DB | §3 |
| 9. 로컬↔중앙 대조, 불일치·누락·낡은 세대·순번 간격의 조치 | 구현·시험 | 모의·실제 DB | §3 |
| 9. 코디네이터·역할 프로세스 재시작 뒤 대조 | 구현·시험 | 모의·실제 DB·컨테이너 | §3, [컨테이너 기록](evidence/images-fake-a1cfe78.log) |
| 9. 재개 판단에 사용(코디네이터 판정 API, supervisor의 기동 전 대조) | 구현·시험 | 모의·실제 DB | §3 |

“모의”는 가짜 `claude`로 실행했다는 뜻이다. 실제 Claude 회차에서 이 경로를 지난 적은 없다. 실제 공급자의 거부·인증 오류 신호는 관측한 적이 없다(§2 한계).

## 2. 항목 4: 공급자 상태와 공유 한도 그룹

**값 대응**(런타임 계약 §13 항목 9, `crates/ieo-supervisor/src/provider.rs`):

| §7 값 | 원시 신호(CLI 2.1.322 SDK 형 정의) |
| --- | --- |
| `auth_or_billing` | `assistant` 사건의 최상위 `error`가 다음 중 하나: `authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `verification_required`, `billing_error`, `cloud_credential_error` |
| `usage_limit` | `rate_limit_event`의 `status: rejected`, 또는 `isUsingOverage: true` |
| `available` | 마지막 `rate_limit_event`가 `allowed`/`allowed_warning`이고 `isUsingOverage: false`이며, 그 뒤에 위 오류가 없다 |
| `unknown` | 그 밖 전부. 신호 없음, 읽을 수 없는 행, 모르는 값, 일시 오류(`rate_limit`·`overloaded`·`server_error`), 신선하지 않은 관측이 여기에 든다 |

- **근거와 가정.** 열거값은 실행 파일 안의 형 정의에서 확인했다. 확인한 것은 셋이다. `assistant.error`의 13개 값, `rate_limit_info.status`의 3개 값, `assistant` 사건의 최상위 `error` 위치다. 형 정의는 실행 파일을 문자열로 읽어 확인했고, 실행은 하지 않았다.
  - 실제로 관측한 것은 `allowed`·`overageStatus: rejected`·`isUsingOverage: false`뿐이다(01-04, 02-04 D1~D6).
  - `rejected`와 인증 오류가 실제로 어떤 모양으로 출력되는지는 **가정**이다. 실제 한도나 오류가 생기면 원본과 대조한다(04-15, 아래 남은 한계).
- **우선순위와 시각.**
  - 우선순위는 한 회차 안에서 `auth_or_billing` > `usage_limit` > 마지막 가용 신호다.
  - stream 사건에는 시각이 없다. 그래서 관측의 나이를 회차 **시작**부터 센다(보수적 쪽).
  - 최대 나이(설정 `provider_status_max_age_seconds`, 기본 600초)를 넘은 관측은 `unknown`으로 내고, 마지막 관측은 `provider_status_detail`에 남긴다.
  - 실행 중에는 그 회차의 stream만 읽는다.
  - 재시작 전의 관측은 `provider_status_before_start`(`current: false`)에만 보인다.
- **`quota_group_id`.** 역할 설정의 필수 값이다. 빠졌거나, 빈 값이거나, `unknown`이거나, 형식이 틀리면 기동을 거부한다.
  - 01-04 P3(같은 계정, 같은 창의 `resetsAt` 일치)에 따라 두 역할은 같은 그룹이다.
  - 컨테이너 시험에서는 `anthropic-subscription-shared-0104`로 설정했다. 이 이름은 예시다. 실제 배치 설정의 값은 배치할 때 정한다.
  - 코디네이터 `GET /v0/repos/{repo}/roles`는 `quota_groups`를 함께 낸다. 판정 규칙은 다음과 같다.
    - 한 구성원이 `usage_limit`이면 그룹 전체가 `usage_limit`이고, 모든 구성원이 영향 역할(`affected_roles`)이다.
    - `available`은 구성원 하나가 관측했고 `usage_limit`이 없을 때만이다.
    - `auth_or_billing`은 자격 하나의 상태이므로 그룹에 퍼뜨리지 않는다.
    - `--roles`에 적은 기대 그룹과 보고가 다르면(보고 없음 포함) `mismatches`에 남긴다.
- **시험**(모델 호출 0회):
  - `provider` 단위 시험: 값 대응 전체, 한 회차 안 우선순위(뒤의 `allowed`가 앞의 거부·인증 오류를 지우지 않음), 잘린 행, 모르는 값, 시각 계산, 나이 초과, 재시작
  - `tests/serve.rs::provider_status_maps_raw_signals_and_never_reuses_an_old_value`: 역할 서비스와 가짜 런타임 5회차. 다룬 것은 다음과 같다.
    - 실행 중인 회차의 거부 → `usage_limit`. 이전 회차의 `available`을 쓰지 않는다
    - 같은 볼륨의 재시작 → `unknown`
    - 인증 오류 → `auth_or_billing`
    - 신호 없음 → `unknown`
    - 나이 초과 → `unknown`
    - 그룹 설정 거부 4가지
  - `tests/serve.rs::status_reports_the_section_7_fields_and_an_honest_safe_shutdown`: 계약 값으로 보고하고, 재시작 뒤에는 `unknown`과 이력만 낸다.
  - `ieo-coord tests/roles.rs::a_shared_quota_group_carries_a_usage_limit_to_every_member`: 그룹 전파, 관측 없음, 인증 오류 비전파, 그룹 분리, 기대 그룹 불일치, 연결 불가
  - 컨테이너([기록](evidence/images-fake-a1cfe78.log)): 두 역할 모두 `available` → 그룹 `available`. 개발이 거부된 창을 본 뒤에는 그룹 `usage_limit`, 영향 역할은 개발·Guardrail이다. Guardrail 자신의 최근 관측은 `available` 그대로다. 개발 컨테이너를 재시작하면 `unknown`(이전 `available`은 `current False`)이다.
- **수용 보고의 한계(검토 fa39d86 M-a) 처분.**
  - 계약 값으로 바꾸지 않던 것은 해소했다.
  - 실행 중 낡은 값은 해소했다. 실행 중에는 그 회차를 읽는다.
  - 재시작 뒤 `unknown`은 의도대로이고 이력을 함께 낸다.
  - `quota_group_id` 미설정은 해소했다(필수).
  - `last_actual_action_at`은 여전히 끝난 회차의 종료 시각이다. 실행 중 마지막 행동 시각은 남은 한계다(§5).

## 3. 항목 9: 중앙 체크포인트 보고·저장·대조

**경로**(런타임 계약 §13 항목 10). 바꾼 곳은 supervisor `central.rs`·`serve.rs`, coordinator `api.rs`·`schema.sql`·`roles.rs`다.
1. supervisor가 체크포인트를 쓸 때마다 다음 순서로 처리한다.
   - 체크포인트에 `seq`(1부터 연속), `repo_id`, `role`, `config_tamper_detected`를 넣는다.
   - 순번은 보관 사본과 로컬 체크포인트 중 큰 값의 다음이다.
   - `checkpoint.json`을 쓴 뒤 사본을 `/state/checkpoints/<seq>.json`에 둔다(교정 2).
   - 파일 바이트를 그대로 코디네이터에 보고한다. takeover는 볼륨에 대기 사본을 둔 뒤 **먼저** 보고하고, 받아들여진 뒤에만 로컬에 쓴다. after_run·error는 로컬에 먼저 쓰고 보고한다. 로컬 쓰기는 `checkpoint.json` 다음 보관 사본 순서다(교정 1·2).
   - 보고 자격은 supervisor 전용 `checkpoint:report`(agent 종류, repo·역할 하나)다. 모델의 `agent:report` 자격과는 다르다.
2. 코디네이터는 받은 바이트의 SHA-256을 직접 계산한다. 다음 순번만 `role_checkpoints`(추가 전용)에 저장한다.
   - 배정·세대가 (repo, 역할)의 현재 배정 머리와 같아야 한다. 그 배정의 sprint revision, 명세 digest, 대상 SHA와도 같아야 한다. 세대는 저장된 마지막 값보다 낮으면 안 된다.
   - 같은 순번·같은 바이트는 200 중복이다.
   - 같은 순번·다른 바이트는 409 `EVENT_CONTENT_CONFLICT`다.
   - 다음 순번을 건너뛰면 409 `CHECKPOINT_SEQUENCE_GAP`이다.
   - 낮은 세대는 409 `STALE_ASSIGNMENT`다.
   - 다른 배정이나 sprint revision은 `CHECKPOINT_MISMATCH`, 명세 digest는 `DIGEST_MISMATCH`, 대상 SHA는 `ASSIGNMENT_SHA_MISMATCH`, 배정 없음은 `NO_CURRENT_ASSIGNMENT`다.
   - 예외 하나: 회차 도중 배정이 바뀌면, 같은 세션·배정·세대로 저장된 takeover를 잇는 after_run·error는 `current_assignment=false`로 저장한다. takeover는 언제나 현재 배정이어야 한다(교정 1).
   - 변조 탐지는 새 session_id 하나에 한 번 `role_tamper_events`에 기록한다.
   - 거부된 보고는 행을 남기지 않는다.
3. supervisor는 start·reconnect **전에** 코디네이터의 최신 기록을 받아 로컬 체크포인트의 순번·digest와 맞춘다.
   - 로컬이 앞서면 보관 사본을 순서대로 보내고 다시 맞춘다.
   - 다음 경우는 `EXTERNAL_RECONCILIATION_REQUIRED`다. 중앙이 앞섬, digest가 다름, 로컬이 없는데 중앙에 기록이 있음, 보관 사본이 없거나 다름.
   - 코디네이터에 닿지 않으면 `NOT_OBSERVED`이고 아무것도 시작하지 않는다.
   - takeover가 명확히 거부되면 대기 사본을 지우고 로컬에 아무것도 쓰지 않으며 실행하지 않는다. 시도는 실행되지 않은 것으로 닫힌다(예산 원장 끝 행 0/0). 다음 기동은 막히지 않는다(교정 1).
   - 답을 받지 못했거나(연결 실패·시간 초과), 받아들여진 뒤 로컬 쓰기가 실패했거나, 그 사이에 멈추면 실행하지 않는다. 대기 사본은 남는다. 다음 기동은 코디네이터의 최신 기록이 정확히 그 사본(같은 순번·digest)이면 채택하고, 코디네이터에 없으면 버린다(교정 2, 검토 accd6c8 M-1r).
   - 탐지된 설정 변조는 그것을 담은 takeover가 받아들여질 때까지 볼륨(`pending-tamper.json`)에 남는다. 그동안 reconnect는 재개하지 않고 새 세션을 요구한다(교정 2, M-2).
   - 회차 뒤 보고가 실패하면 상태의 `checkpoint_reported: false`로 드러난다. `checkpoint_reported`는 중앙 기록이 이 체크포인트(순번·digest)를 가진 것으로 알려졌을 때만 true다(받아들여진 보고나 성공한 대조). 이 사실과 마지막 보고 결과는 볼륨(`checkpoint-report.json`)에 남아 재시작 뒤에도 같다. repo 범위 stop은 명시적인 `true`만 보고됨으로 본다.
   - 보관 사본이 `checkpoint.json`보다 정확히 하나 뒤면(두 파일 쓰기 사이의 중단) 빠진 사본을 `checkpoint.json`에서 쓴다. `checkpoint.json`이 보관 사본보다 뒤인 경우(되돌림)는 되살리지 않는다(교정 2, L-3).
4. 코디네이터의 재개 판단 `GET /v0/repos/{repo}/roles/{role}/checkpoints/compare`는 역할 상태의 로컬 순번·digest를 중앙 기록·현재 배정과 비교한다.
   - 결과는 `resume_allowed` / `no_checkpoint` / `already_running` / `stale_generation` / `external_reconciliation_required` / `not_observed` 중 하나다.
   - 마지막 기록이 `after_run`이 아니면 재개하지 않는다.

session_id만 저장하지 않는다. 저장하는 식별자는 다음과 같다.
- 순번, digest, phase, session_id
- 배정, 세대, sprint, revision, 명세 digest, 대상 SHA
- 이미지 digest, 프로파일, 설정 digest
- 원본 체크포인트(jsonb), 보고자, 받은 시각

**시험**(실제 DB = 시험 PostgreSQL, 가짜 런타임, 모델 호출 0회):

| 사례 | 시험 | 결과 |
| --- | --- | --- |
| 정상 보고·저장·digest | `ieo-coord tests/coordinator.rs::central_checkpoints_keep_sequence_identity_and_tamper_reports` | 201. digest는 보고 바이트의 SHA-256 |
| 중복 | 같은 시험 | 같은 바이트 200(행 불변), 다른 바이트 409 |
| 역순·간격 | 같은 시험 | 첫 보고 전의 순번 2 → `CHECKPOINT_SEQUENCE_GAP`. 낮은 순번은 저장된 것과 비교만 한다 |
| 식별자 불일치 | 같은 시험 | 대상 SHA, 명세 digest, sprint revision, 다른 세대, 다른 배정, 다른 repo, 자기 seq와 다른 seq, 모르는 phase, 형식. 모두 거부되고 행 수가 그대로다 |
| 자격 | 같은 시험 | 모델의 `agent:report`, 다른 역할의 supervisor, client 자격 → 403 |
| 변조 | 같은 시험, e2e 7 | 한 번만 기록된다. new_session_id가 체크포인트와 다르면 거부된다 |
| 낡은 세대 | 같은 시험, e2e 8 | 재배정 뒤 이전 세대 보고 → `STALE_ASSIGNMENT`. 판단 `stale_generation`. supervisor reconnect → `STALE_GENERATION` |
| 추가 전용 | 같은 시험 | UPDATE·DELETE 거부 |
| 재개 판단 조합 | `resume_judgment_compares_local_central_and_assignment` | 13가지 조합 |
| 정상 회차 → 재개 허용 | `tests/checkpoint_e2e.rs` 2 | 순번 1(takeover)·2(after_run). 중앙 digest = 로컬 파일 digest |
| 코디네이터 재시작 | e2e 3, 컨테이너 | `resume_allowed` 유지 |
| 역할 프로세스 재시작 | e2e 4, 컨테이너 | 같은 볼륨의 새 서비스가 대조한 뒤 재개(순번 3·4) |
| 로컬 불일치·누락·되돌림 | e2e 5, 컨테이너 | 세 경우 모두 409 `EXTERNAL_RECONCILIATION_REQUIRED`, 실행 없음. 코디네이터 판단도 같다 |
| 거부된 takeover(잘못된 대상 SHA) | e2e 10 | 202 뒤 시도는 `CentralRefused`·실행 안 됨으로 닫힌다. 로컬 체크포인트·중앙 순번 그대로. 다음 올바른 기동 202(교정 1) |
| 회차 도중 배정 세대 변경 | e2e 11 | 회차 뒤 체크포인트가 `current_assignment=false`로 기록된다(보고됨 true). 판단 `stale_generation`. 새 세대의 새 세션 기동 202, 순번 18(교정 1) |
| 쓰기 중단·되돌림 | `tests/serve.rs::a_missing_archive_copy_is_written_and_a_rollback_is_not_hidden` | 빠진 보관 사본은 `checkpoint.json`에서 쓰고 재개, 한 단계 되돌린 `checkpoint.json`은 거부되고 그대로 남음(교정 2) |
| 변조 뒤 takeover 거부 | e2e 12 | 거부 뒤 새 세션 없는 reconnect는 400, 새 세션 reconnect는 변조를 담아 시작, 변조 사건 2건(교정 2) |
| takeover 답 유실 | e2e 13 | 코디네이터에 기록된 대기 사본을 다음 reconnect가 채택(“older”가 아님). 그 체크포인트가 takeover라 재개는 “did not finish”로 거부, 실행 없음(교정 2) |
| 재시작 뒤 보고 상태 | e2e 4 | 재시작한 서비스도 `checkpoint_reported: true`(교정 1) |
| 회차 중 코디네이터 중단 | e2e 6 | 회차 뒤 보고 실패 → `checkpoint_reported: false`. 중단 중 재개 → 503 `NOT_OBSERVED`, 실행 없음. 복구 뒤 판단 → “아직 기록 안 됨”. 다음 재개가 보관 사본 1건을 먼저 보낸 뒤 실행(순번 8) |

컨테이너([기록](evidence/images-fake-a1cfe78.log), 이미지 a1cfe78, UID 20000/20001/20002, PID 1) 순서:
1. 등록·의미 검토·두 역할 배정
2. 두 역할 첫 회차 → 중앙 순번 2·`resume_allowed`
3. 코디네이터 컨테이너 재시작 → `resume_allowed`
4. 개발 컨테이너 재시작 → `resume_allowed`, reconnect가 대조 뒤 재개(`sent 0`), 순번 4
5. 로컬 체크포인트를 바꾸면 판단과 reconnect 모두 거부
6. SIGTERM 종료 0
7. DB에 체크포인트 6행

모든 시험 결과는 다음과 같다(시험 DB 연결, `cargo test --workspace`). 실패는 0이다.
- `ieo-coord`: 68 + e2e 1 + roles 7 + shutdown 2
- `ieo-supervisor`: 단위 3, caller 23, run_guard 27, serve 21, supervisor 20

`cargo clippy --workspace --all-targets -D warnings`도 통과했다.

## 4. 나머지 결정 항목의 인계(이월 대응표)

이월은 완료가 아니다. 수용 보고 §1·§2의 미충족·부분 충족 기록은 바꾸지 않고 그대로 둔다. 아래는 승인된 이동과 후속 인수 조건이다.

| 결정 항목 | 이전 상태(보존) | 승인된 이동(담당 작업) | 완료 시점·인수 조건 | 차단 조건 |
| --- | --- | --- | --- | --- |
| 1a. 배정 수준 시간·토큰 한도 집행 | 미충족(02-03 이월) | 02-05 | 배정이 supervisor에 닿을 때, 배정의 한도로 회차를 막고 기록하는 시험 | 02-05 수용 전 필수 |
| 1b. 실제 GitHub 증거 수집·PR 작성자 신원(F4)·PR 수준 검토 상태(N5) | 미충족(02-03 이월) | GH1~GH5 충족 뒤 해당 연결 작업(GH 착수 조건을 가진 04-07·05-01~05-03)과 EARLY-01 | GH1~GH5가 갖춰진 시험 repo에서 실제 수집·신원·검토 상태 대조 | GH1~GH5 미충족이면 그 연결 작업을 착수하지 않음 |
| 1c. 판정 머리(F3) | 미충족(02-03 이월) | 02-07 | 완료 판정이 판정 머리를 쓰는 시험 | 02-07 수용 전 필수 |
| 2. 코디네이터의 start·reconnect 호출 | 부분 충족 | 02-05 | 배정·실제 인수 연결에서 코디네이터가 부르고, 이번 §3의 판단(`compare`)을 거친다 | 02-05 수용 전 필수 |
| 3. repo 범위 안전 종료 | 미확인 | 자식 추적 04-05, 외부 요청 추적 02-05 | 둘이 추적되기 전에는 `safe_shutdown_confirmed: false`를 유지한다(지금 동작) | 확인 전 “안전 종료 확인” 표기 금지 |
| 5. D3 답 불충족 | 실패 유지 | 이동 없음 | 수용 기록에 계약 조건 충족과 분리해 남긴다. 기대값 3과 답 4는 바꾸지 않는다 | 재실행하지 않음 |
| 6. 실행 중 실제 정지 | 미실행 | 04-05 | 실제 실행 **전에** 결정안(시험 계획, 미측정 사용량 처리, 재개 조건)을 보고하고 승인받는다. 초안은 진행 기록 7 §5 | 승인 전 실제 정지 금지 |
| 7. 예비 회차 | 미사용 | 유지(개발 2·Guardrail 3) | 사용하지 않는다 | 새 사용 결정 전 사용 금지 |
| 8. 호출자 이월 | 열림 | 다음 실제 호출·X-02 착수 **전** | 수용 보고 §4 항목 8의 목록(PR #51 P1~P4·N6·F8, PR #49 L-c3-2~4, 마감 정지 시험)을 처리하고 검토한다 | **차단 조건**: 완료 전에는 다음 실제 호출과 X-02를 시작하지 않는다 |
| (수용 보고 §7) PR #54 검토 fa39d86 L-a | 열림 | 이번 PR에서 일부 처리 | `quota_group_id` 설정은 처리했다. takeover·error 체크포인트의 미저장 시험과 30초 공동 대기 시험은 남는다 | 다음 supervisor·coordinator 정비 |
| fa39d86 L-b·L-c, adf760e L-2 일부, 그 밖의 열린 이월 | 열림 | 수용 보고 §7 표대로 | 표의 담당·시점 그대로 | 02-04 수용 영향 없음으로 본다(§7) |

## 5. 남은 한계

- 실제 Claude 회차는 이 경로(항목 4·9)를 지나지 않았다. 모두 모의다. 추가 실제 호출은 승인 범위 밖이다.
- 공급자 거부·인증 오류의 실제 출력 모양은 형 정의로만 정했다(가정). 실제 사건이 생기면 원본과 대조한다(04-15 한도 영향 측정, 런타임 갱신 시).
- 개발 컨테이너 안의 체크포인트와 보고 자격은 같은 UID의 개발 코드가 바꿀 수 있다(계약 §2.1, 검토 N5). 중앙 기록은 바꾼 사실을 드러낼 뿐이다. 재개의 최종 근거인 GitHub SHA·CI 대조는 02-05에서 연결한다.
- 체크포인트의 `worktree_head`·`diff_sha256`·활성 자식·외부 요청은 여전히 추적하지 않는다(`tracked` false). 각각 02-05와 04-05에서 다룬다.
- `last_actual_action_at`은 끝난 회차의 종료 시각이다. 실행 중의 마지막 행동 시각은 없다.
- `local_only`는 시험용이며 설정에 명시해야 한다. 쓰면 상태의 능력 공백에 나온다.
- 공유 그룹 판단은 각 역할의 최근 관측을 모은 것이다. 구성원들의 관측 시각을 서로 맞추지는 않는다. 한 구성원의 `available`이 다른 구성원의 미관측을 덮는다. 한도는 하나라서 그렇게 보지만, 그 관측의 신선도는 구성원마다 따로다.

## 6. 재발 방지

원인별 대응표는 [PREVENTION.md](PREVENTION.md)에 있다. 이번 작업에서 구현한 것과 정식 설계·개발로 넘길 것을 구분했다.

## 7. 교정 1(검토 48b1d0e)

| 지적 | 처분 |
| --- | --- |
| M-1 거부된 보고가 역할과 예산을 영구히 막음 | takeover는 보고가 받아들여진 뒤에만 로컬에 쓴다. 거부되거나 닿지 않으면 시도를 실행 안 됨(0/0)으로 닫는다. 회차 도중 배정이 바뀌면 회차 뒤 체크포인트를 `current_assignment=false`로 기록하고, 판단은 `stale_generation`, 복구는 새 세대의 새 세션이다. 시험: e2e 10·11 |
| L-1 repo 정지가 명시적 false만 막음 | 명시적인 `true`만 보고됨으로 본다 |
| L-2 재시작 뒤 `checkpoint_reported: false` | 마지막 보고 결과를 볼륨에 남기고 기동 때 읽는다. 시험: e2e 4 |
| L-3 순번을 보관 사본만으로 정함 | 보관 사본과 로컬 중 큰 값의 다음. 사본이 정확히 하나 앞서면 되살린다. 시험: serve 새 시험 |
| L-4 거부 이유가 “409 null” | `rejections[0].code`도 읽는다 |
| L-5 OpenAPI 누락 | `quota_groups`, 정지 사유, 모르는 역할 404, `current_assignment`, 예외 규칙을 적었다 |
| I-1 `unix_of` 뺄셈 | 0년을 거부한다(시험) |
| I-2 거래 안 원격 호출, 두 읽기 | supervisor 상태를 거래 전에 받는다. 두 읽기는 보고와 같은 잠금 아래에서 한다 |
| I-3 상태 잠금 안 파일 읽기 | 상태를 복사한 뒤 잠금을 풀고 읽는다 |
| I-4 신선도 경계 | 미래의 시작 시각은 신선하지 않다(시험). 최대 나이보다 긴 회차는 실행 중 `unknown`(보수적, 모듈 문서에 적음) |

## 8. 교정 2(검토 accd6c8)

| 지적 | 처분 |
| --- | --- |
| M-1r 결과를 알 수 없는 takeover가 역할을 영구히 막음 | takeover를 볼륨에 대기 사본으로 둔 뒤 보고한다. 명확한 거부는 지운다. 답 유실·로컬 쓰기 실패·중단은 남기고, 다음 기동이 중앙 최신 기록과 같은지(순번·digest) 확인해 채택한다. 로컬 쓰기 실패는 실행 안 됨으로 닫는다. 이전 문서의 “닿지 않으면 다음 기동이 막히지 않는다”는 과장이었다. 시험: e2e 13 |
| M-2 거부된 takeover가 변조 탐지를 지움 | 탐지된 변조를 `pending-tamper.json`에 두고, 그것을 담은 takeover가 받아들여질 때 지운다. 그동안 reconnect는 재개하지 않고 새 세션에 변조를 실어 보고한다. 기동 전 정지 경로도 같다. 시험: e2e 12 |
| L-1 CI 실패(경쟁) | 거부된 창은 예산 감시가 곧바로 회차를 멈춘다. 실행 중 관측은 회차를 멈추지 않는 인증 오류와 3초 대기로 시험하고, 거부는 회차 뒤 값으로 확인한다. 6회 반복과 병렬 실행에서 통과 |
| L-2 거부된 takeover 기록이 이전 체크포인트를 미보고로 바꿈 | `checkpoint_reported`는 중앙이 가진 것으로 알려진 순번·digest(`accepted`)로 판단한다. 대조로 다시 보낸 보고도 반영한다. 기록에 digest를 넣었다 |
| L-3 되살리기가 한 단계 되돌림을 숨김 | 쓰기 순서를 `checkpoint.json` → 보관 사본으로 바꾸고, 빠진 보관 사본만 `checkpoint.json`에서 쓴다. 되돌림은 거부된다(e2e 5의 한 단계 되돌림, serve 시험). 쓰기는 원자적(임시 파일·fsync·rename) |
| Info: 열 추가 | `ADD COLUMN IF NOT EXISTS`를 넣었다 |
| Info: 다른 배정의 판단 | 문서를 고쳤다. 같은 배정의 새 세대면 `stale_generation`, 다른 배정이면 `external_reconciliation_required`다 |
| Info: L-1(`!= true`) 시험 없음 | roles 시험이 필드 없는 역할을 “보고되지 않음”으로 보고하는지 확인한다 |

## 9. 병합 뒤 후속(검토 802e4a1의 비차단 지적)

PR #57은 검토 802e4a1(approve)의 head 802e4a1에서 병합했다(12bf34f, main CI 43679480078 success). 비차단 지적은 후속 PR에서 처리했다.

| 지적 | 처분 |
| --- | --- |
| N-1 변조 하나가 두 번 기록됨 | takeover가 받아들여지는 즉시 대기 변조를 지운다(로컬 쓰기 실패 경로 포함). 변조를 담은 대기 사본을 채택할 때도 지운다. 시험: `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once`(변조 보고 1건) |
| N-2 대기 변조가 있을 때 변조된 session_id 재사용 | 변조 뒤 새 세션의 session_id가 변조된 세션과 같으면 400. 시험: `a_pending_tamper_needs_a_session_other_than_the_tampered_one` |
| N-3 문서의 쓰기 순서·“사용량 0” 과장 | 계약 §13 항목 10과 §3을 고쳤다. 대기 사본을 쓰기 전이나 보고 도중에 멈추면 끝 행이 없고, 백스톱은 대조 전까지 다음 시작을 막는다 |
| I-1 4xx가 아닌 답도 대기 사본을 지움 | 4xx만 지운다. 5xx와 답 없음은 남긴다. 시험: 위 첫 시험(502 뒤 보존·채택) |
| I-2 채택이 더 새 로컬을 덮을 수 있음 | 대기 사본의 순번이 로컬보다 클 때만 채택하고, 아니면 버린다 |
| I-3 동시 기동 | `launch`를 한 번에 하나로 묶는다 |
| I-4 표시·시험 | 로컬 쓰기 실패는 `LocalWriteFailed`로 표시한다. 5xx 보존과 로컬 쓰기 실패 경로를 실제 실행 경로로 시험한다(가짜 코디네이터): `a_takeover_accepted_but_not_written_locally_is_adopted_next_time` |

PR #58 검토 704330f(approve)의 지적 처분(교정 1):

| 지적 | 처분 |
| --- | --- |
| P-1 새로 탐지한 변조가 거부된 요청에서 사라짐(PR #58에서 생긴 회귀) | 탐지는 새 세션 id 확인 같은 거부보다 먼저 `pending-tamper.json`에 저장한다. 이미 대기 중인 탐지가 있으면 그것을 유지한다. 시험: `a_detection_survives_refusals_and_a_refused_takeover_keeps_it_pending` |
| P-2 코디네이터가 `tamper_recorded: false`라고 답해도 대기 변조를 지움 | 이월(정보). 코디네이터는 새 세션 id마다 한 번 기록한다. 이전에 쓴 새 세션 id를 다시 쓰는 호출자 오류에서만 생긴다 |
| P-3 I-2·4xx 경로의 전용 시험 없음 | 같은 시험에서 4xx 거부(대기 사본 삭제, 변조 대기 유지)와 로컬보다 새롭지 않은 대기 사본의 삭제를 확인한다 |

