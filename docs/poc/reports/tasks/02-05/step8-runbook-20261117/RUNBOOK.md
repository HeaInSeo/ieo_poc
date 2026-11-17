# 02-05 단계 8 절차서 · 실제 회차(R1·R2·G1, 예비 1회) 전후 확인과 요청

| 항목 | 기록 |
| --- | --- |
| 목적 | 02-05 단계 8 실제 회차의 사전 확인, 재배포, 설정, 요청, 예비 판단, 멈춤·보고를 한 곳에 모은다. [단계 8 전 보고](../pre-step8-20261117/REPORT.md) §6(검토 63fefc1·4531772 정정 포함)과 [예산 보고](../budget-0205-20261117/REPORT.md)를 합쳤다 |
| 근거 | [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md) §2.1(예산 (나)·멈춤 규칙), [사용자 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md)(예비), [02-05 계획](../plan-20261115/PLAN.md) §4, 코드(main 5d69102 + 후속 PR feat/0205-pre8b) |
| 만든 방법 | 코드와 문서에서만 끌어냈다. 영속 PoC DB(55433)·코디네이터(58090)·비밀·실행 기록은 보지 않았고 아무것도 실행하지 않았다 |
| 승인 | 이 절차서는 실제 호출·예비 사용·추가 결제를 승인하지 않는다. **[실 시스템 변경]** 표시 단계는 그때마다 사용자의 명시적 승인을 받는다. 예비는 사람(`kind: user`)이 붙이고 **그 건마다 사용자의 승인**을 받는다(사용자 결정 2026-11-17 §4.5). 02-04 남은 5회는 쓰지 않고 유지, 추가 청구 USD 0. 남은 사용자 결정·승인은 §10 |
| 개정 | PR #83 병합 뒤 검토 8336044 반영(§11): §1A의 순서(등록·의미 검토는 재배포 뒤), 의미 검토 본문, 바인딩은 선택, UUID로 `not_called`된 뒤의 새 시도 id. 그 전 PR #83 검토 67b5cae 반영: 세션 id는 새 소문자 UUID(§5·§6), 예비·재시작의 승인 표시(§6·§8), 정지 유예 900초, 778ee47의 멈춤, 준비 절(§1A), 보고 조건, 사용자 결정·승인 목록(§10) |

표기: **[읽기]** 읽기만 한다(잠금·쓰기 없음). **[실 시스템 변경]** 58090 코디네이터, 55433 영속 DB, 역할 컨테이너·볼륨, 모델 호출 중 하나를 바꾼다. 비밀 값(토큰·자격)은 어느 단계에서도 출력하지 않는다(파일의 존재·이름·종류만 본다).

## 1. 사전 확인(모두 [읽기], 재배포 전)

하나라도 아니면 멈추고 보고한다(§9). “재배포 뒤 적용”으로 적은 확인은 §3에서 한다.

1. **빌드.** 이 절차서를 담은 PR(feat/0205-pre8b)이 main에 병합됐고 그 main CI가 성공했다. 재배포할 `ieo-coord`·`ieo-role-call`·역할 이미지는 그 병합 SHA로 만든다(PLAN §4.2). 지금 58090은 main 778ee47 빌드다(RESUME.md).
2. **진행 중 회차 없음.** 지금의 58090(778ee47)에는 전달·역할 서비스·연결 조회가 없다. 실제 회차의 유일한 경로는 호스트의 `ieo-role-call`(02-04 장부)이다. 그래서 재배포 전에는:
   1. 02-04 장부 디렉터리(그리고 다른 장부)에 `inflight.json`이 없다.
   2. 그 장부의 `caller.lock`을 연 프로세스가 없다(`fuser`·`lsof`로 보기만 한다. `flock`으로 잠그지 않는다).
   3. 돌고 있는 `ieo-role-call` 프로세스가 없다(`ps`로 보기만).
   4. **재배포 뒤 적용**(§8의 재시작 때): 두 역할 서비스의 `GET /v0/role/status`(어댑터 자격)가 `running: null`이고, 최근 전달 중 결과가 `accepted`인데 장부에 그 시도의 기록이 없는 것이 없다(`GET /v0/coordinator/assignments/{aid}/generations/{g}/chain`).
3. **새 표가 미리 없음**(55433): `SELECT table_name FROM information_schema.tables WHERE table_name IN ('role_checkpoints','role_tamper_events','role_first_actions','role_session_status','role_deliveries','role_delivery_results','role_takeovers','assignment_run_usage','assignment_limit_events','role_recoveries','role_recovery_uses')` → 0행. 있으면 main이 아닌 빌드가 만든 것이다. 열 정의를 `crates/ieo-coord/src/schema.sql`과 대조하고 사람이 정한다(`migrate`는 `IF NOT EXISTS`라 그 모양을 그대로 둔다).
4. **바인딩과 sprint 소유**(55433): 비공개 바인딩 파일의 각 repo `sprint_id`에 대해 (a) 두 repo가 같은 id를 대지 않는다, (b) `SELECT 1 FROM notion_imports WHERE test_id = $1` → 0행, (c) `SELECT DISTINCT repo_id FROM sprint_revisions WHERE sprint_id = $1` → 없음 또는 그 repo뿐. 아니면 새 코디네이터가 기동하지 않는다(`check_bindings`).
5. **대상 repo·역할에 다른 배정의 중앙 체크포인트가 없다**(55433): `SELECT assignment_id, generation, seq, phase FROM role_checkpoints WHERE repo_id = '<repo>' AND role IN ('development','guardrail')` → 0행. 재배포 전에는 이 표가 없어 질의가 “relation does not exist” 오류로 끝난다: **그것이 정상**이다(항목 3과 같은 뜻). 재배포 뒤 §3 항목 5에서 다시 본다.
6. **sprint revision**(§1A 항목 4·5 뒤에 확인: 그 revision은 재배포 뒤에 등록된다). 단계 8 배정이 쓸 sprint revision이 `registration_state = ready`, `review_state = semantic_review_passed`이고 그 `limits`(`elapsed_seconds`·`total_tokens`)가 계획(개발 1,800초·150,000, Guardrail 600초·50,000)을 덮는다: `SELECT sprint_id, revision, repo_id, registration_state, review_state, spec->'limits' FROM sprint_revisions WHERE sprint_id = '<sprint>'`.
   - **사용자 결정 필요(이 절차서를 병합할 때 확인).** L01의 **값**은 아직 정해지지 않았다([사용자 결정 2026-11-16](../../../../User_Decisions_2026-11-16_l01-step5.md)은 셈법만 정함). 실제 sprint는 `total_tokens = null`인 draft로 남아 등록 검사 `UNBOUND_DRAFT`가 막는다. 그래서 위 조건을 채우는 sprint revision이 지금은 없을 수 있다. 단계 8 전에 사용자가 (가) 단계 8 전용 sprint revision의 한도 값(예: 계획과 같은 1,800초·150,000 / 600초·50,000)을 L01의 값으로 승인하거나, (나) 다른 방법을 정해야 한다. 이 확인 없이는 §4로 가지 않는다.
7. **첫 관측.** `first_observation`으로 쓸 stream의 모든 `rate_limit_event`가 `status: allowed`, `overageStatus: rejected`, `isUsingOverage: false`이고, R1을 보낼 때 그 관측이 **7일 이내**다(`OBSERVATION_MAX_AGE_SECONDS`). 아니면 R1은 보류된다. 후보는 가장 새 실제 stream(02-04 실제 회차, 2026-11-15)이다. 그 시각은 장부 담당이 그 stream의 기록으로 확인한다(이 절차서는 실행 기록을 읽지 않았다).
8. **자격**(재배포 뒤 적용: 새 `tokens.json`은 §1A에서 준비하고 778ee47은 그것을 읽지 못한다. §3 항목 6에서 확인).
9. **역할 볼륨.** 단계 8의 개발·Guardrail 볼륨(`state`·`etc`·`cred`)이 **새것**이다. 02-04 회차를 돈 볼륨이면 supervisor의 보조 상한이 이미 찼다고 보고 시작하지 않는다.
10. **장부·관측 경로.** 02-05 장부 디렉터리는 **새 디렉터리**(02-04·X-02 장부와 다름. 장부 하나는 예산 하나)이고, 코디네이터 프로세스가 읽고 쓸 수 있으며 재배포 뒤에도 남는 경로다(컨테이너 안 임시 경로가 아님). 관측 stream도 같다.

## 1A. 준비 **[실 시스템 변경 — 사용자 승인]**

단계 8에 필요하지만 아직 없는 것이다. 각 항목은 사용자의 승인을 받은 뒤에만 한다. 어느 것도 예산을 쓰지 않는다(빠지면 닫는 쪽으로 실패한다).

**순서**(검토 8336044 L-B): 항목 1~3과 6은 재배포 전에 준비한다. 항목 4·5(등록·의미 검토)는 새 `tokens.json`의 주체(`sprints:register`, `review:semantic`)가 필요하므로 **재배포(§2 항목 3) 뒤**에 한다. **재배포 뒤에 하는 것을 권한다**: 778ee47에서 하면 778ee47의 등록 검사로 들어간다. 꼭 778ee47에서 했다면(그 범위의 주체가 이미 있을 때만), 그 revision은 배정 때 새 빌드가 다시 검사한다(§4 항목 1의 배정이 거절되면 멈추고 보고, PR #84 검토 b291d7c I-4). 항목 7은 §4 바로 전이다. §1 항목 6의 확인은 항목 4·5 뒤에 한다.

1. **입력 고르기**(사용자 결정, §10 항목 3): repo, `task_id`, `target_sha`(40자 소문자 hex), `branch_ref`(`refs/heads/<이름>`), R1·R2·G1의 짧은 지시 셋(읽기·답하기 수준, GitHub 쓰기 없음).
2. **새 `tokens.json`** [실 시스템 변경: 파일]. 코디네이터 자격 파일에 주체를 더한다(값이 아니라 SHA-256 digest만 들어간다): 개발·Guardrail supervisor의 `agent` 주체(`checkpoint:report`, `role`, 그 repo), 배정용 `coordinator:assign`, 전달용 `roles:control`(서비스), 증거 입력용 `evidence:ingest`, 예비용 **사람** 주체(`kind: user`, `roles:control`, 그 repo), sprint 등록용 `sprints:register`와 의미 검토용 `review:semantic`(등록자와 **다른 사람**, 정책 G3). 778ee47은 `agent` 주체가 든 이 파일을 읽지 못하므로 새 빌드로 기동할 때(§2.3) 처음 쓴다.
3. **supervisor 자격 파일** [실 시스템 변경: 파일]. 두 역할 볼륨의 `cred/coordinator_token`(위 `agent` 주체의 토큰 값)과 `cred/adapter.sha256`, 어댑터 토큰 파일. 값은 출력하지 않는다.
4. **단계 8 sprint revision 등록** [실 시스템 변경: 55433, 재배포 뒤]. L01 값이 정해진 뒤(§1 항목 6의 사용자 결정, §10 항목 1) `POST /v0/sprints?mode=executable`(`sprints:register`, `Idempotency-Key`)로 등록한다. 그 revision의 `limits`는 개발 1,800초·150,000 / Guardrail 600초·50,000을 덮어야 한다.
5. **의미 검토** [실 시스템 변경: 55433, 재배포 뒤]. 등록자와 다른 사람이 `POST /v0/sprints/{sprint_id}/revisions/{revision}/semantic-review`(`review:semantic`)로 본문 `{"decision": "passed", "reason": "<검토 이유>", "digest": "<그 revision의 저장된 digest>"}`를 보낸다(정책 G3). `digest`는 등록 응답 또는 `SELECT digest FROM sprint_revisions WHERE sprint_id = '<sprint>' AND revision = <rev>`[읽기]의 값이다.
6. **비공개 바인딩(선택, C2 예약)** [실 시스템 변경: 파일]. 바인딩은 그 `sprint_id`를 그 repo 것으로 **예약**한다(다른 repo가 같은 id를 등록·변환하면 409 `SPRINT_REPO_CONFLICT`). 단계 8에 꼭 필요하지는 않다. 더하면 그 repo → `sprint_id`를 바인딩 파일에 넣고, §1 항목 4의 질의로 충돌이 없음을 먼저 본다(새 코디네이터는 충돌이 있으면 기동하지 않는다).
7. **현재 배정 head** [읽기, §4 전]. `SELECT repo_id, role, assignment_id, generation FROM assignment_heads WHERE repo_id = '<repo>'` → 없으면 §4의 `expected_current: null`, 있으면 그 값을 `expected_current`로 쓴다(그리고 §1 항목 5·§3 항목 5의 조건을 다시 본다).

## 2. 재배포 **[실 시스템 변경 — 사용자 승인]**

§1이 모두 참일 때만 한다.

1. **58090(778ee47) 멈춤** [실 시스템 변경]. 778ee47에는 신호 처리기가 없다(PID 1이 SIGTERM을 무시). 그래서 `podman stop -t N`은 **N초를 다 기다린 뒤 SIGKILL**한다. 이 멈춤은 §1 항목 2의 (1)~(3)이 참일 때만 안전하다(`ieo-role-call`이 돌지 않고 02-04 `caller.lock`이 비어 있음). 그때는 진행 중인 것이 없으므로 N은 멈춤 시간만 정한다(사용자가 정하는 재배포 시간 창). 새 빌드의 멈춤 규칙(§8)은 이 멈춤에 해당하지 않는다.
2. **55433 백업** [실 시스템 변경 아님, 그러나 쓰기 공간 필요]. 재배포 전 영속 DB를 백업한다(장부 담당의 기존 절차). `/home`은 쓰지 않는다.
3. **새 빌드로 58090 기동** [실 시스템 변경: 55433 스키마]. §1A의 새 `tokens.json`, (선택) 바인딩, `roles.json`(§3 항목 2)으로. 기동과 §3 항목 1의 확인 뒤 §1A 항목 4·5(등록·의미 검토)를 하고 §1 항목 6을 확인한다. `ieo-coord serve`는 기동 때 `migrate`(schema.sql)를 돈다. 778ee47 뒤 더해진 것: 새 표 11개(§1 항목 3), 열 4개(`assignments.limits`, `role_checkpoints.current_assignment`, `role_first_actions.source`, `role_delivery_results.github`), `evidence.kind`에 `branch_head`(제약을 지우고 다시 만듦), `role_recovery_uses`의 옛 유일 제약 삭제, 새 표마다 추가 전용 트리거. 지우거나 줄이는 것은 없다. 기동 인자에 `--roles <roles.json>`(§3)과 기존 `--tokens`·`--manifest`·`--policy`·`--bindings`.
4. **`ieo-role-call` 교체** [실 시스템 변경 아님]. 호스트의 `ieo-role-call`도 같은 병합 SHA 빌드로 바꾼다(단계 8에서는 쓰지 않는다. §3 항목 4).

## 3. 재배포 뒤 확인과 설정

1. **마이그레이션 확인** [읽기]. `GET /v0/health` → 200, `schema_version: ieo-coord-v0`(값은 바뀌지 않는다). §1 항목 3의 질의 → 11행. `SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = 'evidence_kind_check'`에 `branch_head`. 기동 로그에 바인딩 오류 없음.
2. **코디네이터 `roles.json`** [실 시스템 변경: 설정 파일]. 대상 repo에:
   - `development`·`guardrail`: `url`(역할 서비스), `adapter_token_file`, `quota_group_id`, `image_digest`·`profile_id`·`profile_revision`(병합 SHA 이미지).
   - `budget_plan`: `{"budget": "02-05", "ledger_dir": "<새 02-05 장부 디렉터리>", "first_observation": {"stream": "<관측 stream>", "observed_unix": <그 관측 시각>}, "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}`. 몫(`share`)은 쓰지 않는다(승인값이 정한다. 적으면 승인값과 같아야 한다). `max_window_seconds`는 600 이하.
3. **역할 서비스(개발·Guardrail) 설정** [실 시스템 변경: 새 컨테이너·볼륨]. 둘 다 `"budget": "02-05"`(몫 없음), 새 볼륨, `"checkpoint_central": {"url": "<58090>", "token_file": "<그 supervisor의 agent 자격 파일>", "transport": "http"}`(**`local_only`가 아님**: 중앙 모드에서만 전달 없는 start가 실행되지 않는다), 병합 SHA 이미지, 구독 자격만(`CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_*` 없음).
4. **시작 경로는 코디네이터 하나** [읽기]. 두 역할 서비스의 `GET /v0/role/status`에서 `central_checkpoint.mode`가 `local_only`가 아니고, `capability_gaps`에 `central checkpoint comparison off (local_only)`가 없다. 단계 8 동안 이 서비스들에 `ieo-role-call`을 쓰지 않는다(예비도 거절한다).
5. **재배포 뒤 중앙 체크포인트** [읽기]. §1 항목 5의 질의 → 0행.
6. **자격** **[읽기]**(값을 출력하지 않고 `tokens.json`의 주체 이름·종류·범위만): §1A 항목 2의 주체가 모두 있다. `roles:control` 보유 주체 목록(이름·종류)을 보고에 적는다. 예비용 사람 주체를 쓸 사람이 정해져 있다(§10 항목 5).

## 4. 배정과 입력 **[실 시스템 변경 — 55433에 기록]**

1. **개발 배정** `POST /v0/coordinator/assignments`(`coordinator:assign`):
   `{"repo_id": "<repo>", "role": "development", "assignment_id": "<DEV-A>", "generation": 0, "sprint_id": "<sprint>", "sprint_revision": <rev>, "task_id": "<task>", "target_sha": "<T>", "expected_current": <§1A 항목 7의 값, 없으면 null>, "branch_ref": "refs/heads/<branch>", "limits": {"elapsed_seconds": 1800, "total_tokens": 150000}}`.
   `branch_ref`가 없으면 R2의 GitHub 대조가 `not_observed`가 되어 재개하지 않는다. `limits`는 revision의 것을 넘지 못한다(422).
2. **Guardrail 배정**: 같은 모양에 `"role": "guardrail"`, `"assignment_id": "<GR-A>"`, `"target_sha": "<T>"`(검토 대상 = 현재 개발 배정의 `target_sha`. 아니면 `stale_target`으로 실행하지 않음), `"limits": {"elapsed_seconds": 600, "total_tokens": 50000}`.
3. **GitHub head 입력** `POST /v0/coordinator/evidence`(`evidence:ingest`): `{"evidence_id": "<id>", "repo_id": "<repo>", "kind": "branch_head", "sha": "<T>", "ref": "refs/heads/<branch>", "source_ref": "<출처>"}`. 단계 8은 GitHub 쓰기·push가 없으므로 R2의 비교 대상은 `target_sha`다. 신뢰 입력이고 실제 수집이 아니다.

## 5. 회차 요청 **[실 시스템 변경 — 모델 호출, 사용자 승인]**

모든 전달: `POST /v0/repos/<repo>/roles/<role>/deliveries`, `roles:control` 자격.
- **`session_id`는 회차마다 새 소문자 UUID**다(예: `uuidgen | tr A-Z a-z` 또는 `cat /proc/sys/kernel/random/uuid`). R1, R2, G1, 예비가 모두 다르다. 보내기 전에 `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`인지 본다. start는 `claude --session-id <id>`가 되고 CLI는 UUID만 받는다(아니면 곧 실패해 미계측이 되고 예비로도 풀 수 없다). 코디네이터도 start의 비-UUID를 `not_called`로 막는다(PR #83 검토 67b5cae M-1). 그렇게 `not_called`된 전달의 시도 id는 쓰인 것이다: 고친 뒤에는 **새 `attempt_id`와 새 멱등 키**로 보낸다(검토 8336044 I-C).
- **`Idempotency-Key`**: 8~128바이트, 회차마다 새 키. 전달은 진행 창(120초)까지 걸릴 수 있으니 클라이언트 시간 제한은 150초 이상으로 둔다. 시간이 넘으면 **같은 키·같은 본문으로** 다시 보낸다(새 키는 두 번째 전달이 된다).
- **토큰을 출력하지 않고 보내기**: `curl -sS -H @<(printf 'Authorization: Bearer %s\n' "$(cat <토큰 파일>)") -H 'Content-Type: application/json' -H 'Idempotency-Key: <키>' --data @<본문 파일> --max-time 150 <url>`(토큰이 명령줄·화면·셸 기록에 남지 않는다). 본문 키는 `assignment_id`·`generation`·`attempt_id`·`session_id`·`prompt`(그리고 선택 `logical_name`·`new_session`·`recovery_id`·`spare`)뿐이다. 지시(`prompt`)는 **읽기·답하기 수준으로 짧게**: 가드가 멈춘 회차(시간·토큰 상한, overage)는 미계측이 되고 예비로 풀 수 없다. GitHub 쓰기·push·PR 없음.

코디네이터가 하는 일: 재개 판단 → 배정 한도 → 공유 예산 검사(02-05 장부, 잠금) → 역할 서비스에 start 또는 reconnect. 역할 서비스로 가는 본문: `attempt_id`, `prompt`, `coordinator`(generation·assignment_id·sprint_id·sprint_revision·spec_digest·image_digest·profile_id·profile_revision), `limits`(`timeout_ms`·`grace_ms`·`billable_room`·`cache_read_room`·`timeout_is_total`), start면 `session_id`, reconnect면 `new_session_id`, `target_sha`·`delivery_id`·`branch_ref`, 재개면 `github`(branch_ref·head_sha·ci). 예비 표식은 역할 서비스로 가지 않는다.

| 회차 | 본문 | 기대 | 매 회차 뒤 [읽기] |
| --- | --- | --- | --- |
| **R1**(개발) | `{"assignment_id": "<DEV-A>", "generation": 0, "attempt_id": "<r1>", "session_id": "<새 UUID>", "prompt": "<짧은 지시>"}` | 중앙 체크포인트 없음 → `no_checkpoint` → 새 세션 **start**. 결과 `accepted`. 역할이 takeover(그 `delivery_id`·`attempt_id`)를 보고하고, 첫 행동을 보고하고(그 전달에 연결된 takeover일 때만 받음), 끝에 after_run | 장부의 `<r1>` 기록(`stop_required`, `usage.run.state`, `outcome.result`), 연결 조회(takeover·첫 행동·after_run, `usage`) |
| **R2**(개발) | `{"assignment_id": "<DEV-A>", "generation": 0, "attempt_id": "<r2>", "session_id": "<새 UUID>", "prompt": "<짧은 지시>"}`(reconnect에서는 쓰이지 않지만 UUID로) | after_run이 최신이고 GitHub head(§4 항목 3)가 `target_sha`와 같으면 `resume_allowed` → **reconnect**(R1 세션 재개, `github` 포함). R1이 실패했으면 R2는 시작하지 않는다(`budget_refused` “asks for a person”) → §6 | 같음 |
| **G1**(Guardrail) | `{"assignment_id": "<GR-A>", "generation": 0, "attempt_id": "<g1>", "session_id": "<새 UUID>", "prompt": "<짧은 지시>"}`(`/roles/guardrail/deliveries`) | Guardrail 배정의 첫 전달 → **start**(검토 대상이 현재 개발 `target_sha`). G1은 실패해도 다시 하지 않는다 | 같음 |

순서는 R1 → R2 → G1(계획 표). 각 회차의 기록과 연결을 확인한 뒤에만 다음을 보낸다. 각 회차는 **[실 시스템 변경 — 모델 호출, 사용자 승인]**이다.

## 6. 예비 결정 트리(개발만, 한 번, 사람 자격, 건별 사용자 승인)

R1 또는 R2가 실패했을 때만 본다(G1은 예비가 없다).

1. **원인 보고를 먼저.** 실패가 실행 환경 결함인지 사람이 판단하고 원인 보고(또는 증거)를 남긴다. 이 참조가 표식의 `evidence_ref`다. 사유 문자열만으로는 증거가 아니다. 환경 결함이 아니면 예비를 쓰지 않고 보고한다(§9).
2. **관문의 전제** [읽기]. 실패한 회차의 장부 기록이 모두 참이어야 한다(`caller::spare_gate`·`spare_releasable`):
   - 이 예산(02-05)의 개발 회차이고, 그 시도의 기록이 장부에 하나뿐이다.
   - 유효 기록(정정 반영)이 실패다: `stop_required: true`이거나 결과가 깨끗한 성공이 아니다.
   - **사용량이 계측됨**: `usage.run.state == "measured"`.
   - **멈춤 사유가 결과뿐**: `stop_reason`의 모든 조각이 `result …` 또는 `no result event`.
   - 예비를 아직 쓰지 않았다.
   **하나라도 아니면 예비를 쓸 수 없다.** 최신 체크포인트가 `error`·`takeover`인 회차(실행 실패·중간 끊김: 답을 잃은 기록으로 미계측), 가드가 멈춘 회차(시간·토큰 상한, overage: 미계측), 정리 실패·구독 전용이 아닌 환경으로 멈춘 회차가 그렇다. 예산은 멈춘 채다. 복구 승인(`recovery_id`)을 더해도 관문이 거절한다. 멈추고 보고한다(§9).
3. **세션이 있었나** [읽기]. 연결 조회의 최신 체크포인트가 `after_run`인지, 실패한 회차의 stream에 그 세션의 `system`/`init`이 있는지 본다.
4. **사용자의 건별 승인.** 보내기 전에 사용자가 이 건을 승인한다: 원인 보고(`evidence_ref`), 대체할 시도(`replaces`), 연산(아래). 승인을 보고에 적는다(사용자 결정 2026-11-17 §4.5: 그 결정은 건별 승인을 미리 주지 않는다). 연산의 기본값:
   - **R1을 다시 할 때, 세션이 있었다** → `"new_session": true`를 권한다(실행 환경 결함을 새 세션으로 다시 함, **start**). 표식만이면 **reconnect**로 그 세션을 이어 간다. 사용자가 고른다.
   - **R2를 다시 할 때, 세션이 있었다** → 기본은 **표식만(reconnect)**: R2의 목적이 코디네이터 경유 재개(R1 세션)를 시험하는 것이기 때문이다(검토 67b5cae L-5). `new_session: true`는 그 목적을 다시 시험하지 않는다. 사용자가 고른다.
   - **세션이 생기기 전에 실패했다** → **반드시 `"new_session": true`**. 표식만이면 reconnect의 `--resume`이 없는 세션을 찾다 실패해 예비가 쓰인다.
5. **요청** **[실 시스템 변경 — 모델 호출, 사용자의 건별 예비 승인]**(사람 자격, 새 멱등 키, **새 소문자 UUID** `session_id`): 본문에 `"spare": {"reason": "<사유>", "replaces": "<실패한 시도 id>", "evidence_ref": "<원인 보고 참조>"}`와 승인된 연산(`"new_session": true` 또는 없음)을 더한다. 승인자와 시각은 코디네이터가 적는다(`by`, `approved_at`: 코디네이터가 그 요청을 처리한 시각).
6. **뒤** [읽기]. 예비 회차 기록의 `spare`(사유·대체 시도·`evidence_ref`·`by {name, kind: user, via: deliverAssignment}`·`approved_at`), 실패한 회차의 기록은 그대로. R1을 다시 한 경우 R2는 표식 없이 세 번째 호출로 보낸다. **예비가 실패하면 다시 멈추고 보고한다**(두 번째 예비 없음).

같은 예비 요청을 같은 멱등 키로 다시 보내면 저장된 답이 오고 새 회차는 없다. 다른 키로 두 번째 예비를 보내면 `budget_refused` “used already”다.

## 7. 멈춤 규칙(사용자 결정 2026-11-15 §2.1, 2026-11-17)

다음이면 곧 멈추고 보고한다. 자동 재시도는 없다.

- 미계측 회차(코드도 다음 호출을 멈춘다).
- 상한 도달: 총 4회(계획 3 + 개발 예비 1), 회당 600초, 합계 2,400초, 과금 토큰 200,000, cache_read 2,000,000, 역할 몫(개발 3회·1,800초·150,000·1,500,000 / Guardrail 1회·600초·50,000·500,000).
- 허용되지 않은 한도 신호(`overageStatus`가 `rejected`가 아님, `isUsingOverage`가 `false`가 아님, `status`가 `allowed`가 아님).
- `stop_required`인 기록.
- 예상과 다른 경로: 전달 판단이 §5 표의 기대와 다름(`not_called`, `external_reconciliation_required`, `stale_target`, `refused`, `unknown`, `budget_refused`가 예상 밖).
- 확인되지 않은 외부 효과: 역할 상태의 `pending_external_requests`가 비어 있지 않음(사용자 결정 2026-11-17 §4.3).
- G1 실패(다시 하지 않음). 예비는 §6의 조건에서만, 한 번, 건별 승인으로.

## 8. 재시작 규칙(새 빌드) **[실 시스템 변경 — 사용자 승인]**

단계 8 동안 58090은 재시작·재배포하지 않는 것이 원칙이다. 꼭 해야 하면 사용자의 승인을 받고:
1. [읽기] §1 항목 2의 (1)~(4)를 02-05 장부와 두 역할 서비스에 대해 한다. 모두 참일 때만.
2. [실 시스템 변경] `podman stop -t 900` 이상. 산술: 멈춤 신호 뒤 코디네이터는 진행 중 회차를 기본 780초(회당 창 600 + 호출자 여유 60 × 2 + 60) 기다리고, 신호 직전에 멈춤 검사를 지난 전달은 진행 창 120초 안에 끝나거나 호출 직전 다시 보기로 `not_called`가 된다: 780 + 120 = 900. 더 긴 창의 회차(02-04 1,800초 등)가 있으면 그 한도 + 120 이상. SIGKILL·짧은 유예 금지.
3. [실 시스템 변경] 다시 띄운 뒤 [읽기] `GET /v0/health`와 연결 조회로 마지막 회차의 기록·체크포인트가 그대로인지 본다. 다음 전달이 “local checkpoint … is not yet recorded”로 막히면 멈추고 보고한다.

## 9. 중단·보고 조건

다음이면 아무것도 더 보내지 않고 보고하고 사용자의 결정을 기다린다.

- §1의 사전 확인 하나라도 아님(특히 새 표가 이미 있음, 바인딩 충돌, 다른 배정의 중앙 체크포인트, 7일 넘은 관측, 새 볼륨이 아님).
- 재배포 뒤 기동 실패·마이그레이션 오류·`branch_head` 제약 없음.
- §7의 멈춤 조건 하나라도.
- 예비의 전제(§6 항목 2)가 아님, 또는 원인이 실행 환경 결함이 아님.
- 장부에 `inflight.json`이 남음, 장부와 연결 조회가 다름(`ledger_mismatch`), 첫 행동이 연결되지 않음(409 `CHECKPOINT_MISMATCH`).
- 비밀이 출력되었거나 자격이 맞지 않음.
- 확인되지 않은 외부 효과(`pending_external_requests`가 비어 있지 않음).

보고에 넣을 것: 회차마다 전달 id·판단·결과, 장부 기록(사용량 네 항목, 시간, `stop_required`·`stop_reason`), 연결 조회(takeover·첫 행동·체크포인트), 예비를 썼으면 표식 전체와 사용자의 건별 승인, 누적 사용량과 남은 예산, 추가 청구 0 근거(관측). **미계측 회차는 미계측으로 적는다(0으로 적지 않는다).**

## 10. 사용자 결정·승인 목록

단계 8 전에 사용자가 정하거나 승인해야 하는 것이다(PR #83 검토 67b5cae).

1. **L01 값**(§1 항목 6): 단계 8 sprint revision의 한도 값으로 개발 1,800초·150,000 / Guardrail 600초·50,000을 승인할지, 다른 방법을 정할지.
2. **그 revision의 등록**(§1A 항목 4~6): executable 등록, 등록자와 다른 사람의 의미 검토(정책 G3), 비공개 바인딩에 그 `sprint_id` 연결. 각 쓰기와 바인딩 변경의 승인.
3. **단계 8 입력**(§1A 항목 1): repo, `task_id`, `target_sha`, `branch_ref`, R1·R2·G1의 짧은 지시 셋(읽기·답하기 범위, GitHub 쓰기 없음).
4. **관측 마감**(§1 항목 7): 첫 관측이 2026-11-15의 02-04 stream이면 R1은 그 `observed_unix`부터 7일 안(2026-11-22경)에 보내야 한다. 그 뒤에는 새 무추가청구 관측을 얻는 방법을 사용자가 정한다(실제 stream만이 출처다).
5. **예비의 사람과 건별 승인**(§6): `kind: user` 자격을 가진 사람. 실패가 생기면 원인 보고, `replaces` 시도, 연산(R2는 reconnect 또는 `new_session: true`, 검토 67b5cae L-5)의 건별 승인.
6. **각 [실 시스템 변경] 단계의 승인**: 재배포 시간 창(778ee47 멈춤 포함, §2.1), 백업을 뜬 뒤의 55433 마이그레이션(백업 위치는 `/home`이 아님), 새 `tokens.json`·`roles.json`·역할 컨테이너와 볼륨, R1·R2·G1 각각.

## 11. 병합 뒤 정정(PR #83 검토 8336044)

| 항목 | 처분 |
| --- | --- |
| L-B(§1A 항목 4·5의 순서와 본문) | 정정. 등록·의미 검토는 새 `tokens.json`의 주체가 필요하므로 재배포 뒤(§1A 순서 문단, §2 항목 3). §1 항목 6의 확인은 그 뒤. 의미 검토 본문 `{decision, reason, digest}`. 바인딩(§1A 항목 6)은 선택(C2 예약) |
| I-C(UUID로 `not_called`된 뒤) | 정정. 고친 뒤에는 새 `attempt_id`·새 멱등 키(§5) |
| I-E(§9 목록의 빈 줄) | 정정 |
| L-A, I-A, I-B, I-F(코드) | 후속 PR feat/0205-pre8c(79bf6a3). 멈춤 다시 보기를 복구 승인의 사용 앞으로 옮겨 멈춤이 승인을 쓰지 않는다. 새 `not_called`에는 `op`가 없다. 재개는 비-UUID 세션으로도 그대로임을 시험. 두 멈춤 기다림이 멈춤 시각부터 세어 합쳐 한 한도를 넘지 않는다. 운영 절차는 바뀌지 않는다(`podman stop -t 900`) |
| main CI 44003424073 조사(코드) | 후속 PR feat/0205-pre8c(7519738). 코디네이터의 gRPC 쪽이 멈춤 뒤 열린 연결을 끝없이 기다리던 것을, 진행 중 호출의 가장 긴 마감(16초)까지만 기다리게 고쳤다. `--grpc-listen`에 클라이언트가 붙어 있어도 §8의 멈춤은 진행 중 회차를 기다린 뒤 스스로 끝난다. `podman stop -t 900`은 그대로(16초는 그 안에 든다). 재시작 뒤 이전 프로세스가 포트·DB를 쥐고 남는 경로는 없다 |
| PR #84 검토 b291d7c 반영 | §1A 순서: 등록·의미 검토는 재배포 뒤를 권함(778ee47에서 했다면 배정 때 새 빌드가 다시 검사, I-4). 멈춤의 최악 시간(진행 중 회차 기다림 780초 + gRPC 유예 16초)은 마지막 회차 한도가 약 884초 이하일 때 `podman stop -t 900` 안에 든다. 단계 8(창 600초)은 해당한다(I-5). 시험 쪽 포트 고르기·gRPC 멈춤 시험은 운영 절차와 관계없다 |