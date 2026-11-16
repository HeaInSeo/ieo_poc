# 02-05 단계 7 · GitHub SHA·CI 대조를 재개의 최종 근거로(C10)

| 항목 | 기록 |
| --- | --- |
| 작업 | 02-05 구현 단계 7([계획](../plan-20261115/PLAN.md) §3.2 단계 7). 이월 C10(GitHub SHA·CI 대조를 재개의 최종 근거로)을 닫는다. C14의 `worktree_head`도 여기서 채운다 |
| 착수 근거 | 단계 6 병합(PR #78, main 9b66300, 검토 6e0ad85 승인) |
| 검토 반영 | PR #78 검토 6e0ad85 L-a·I-a·I-b·I-c와 실행되지 않은 회차의 원장 기록, PR #74 검토 fe924b7 I-4 잔여(§9) |
| 기준 커밋 | main 9b66300. 구현 c9be60f, 재사용 검사로 정리 f7cfb93(사용자 결정 2026-11-16 (나)의 단계 7b 준비), 증거 cb9366b(evidence/test.log 첫 줄이 f7cfb93, 작업 트리 깨끗). 6a2d615는 c9be60f의 증거였고 cb9366b가 대신한다 |
| 모델 호출 | 0회. 실제 `claude`·컨테이너·실제 회차 없음. 실제 GitHub도 읽지 않았다. 영속 PoC DB(55433)·58090 코디네이터는 쓰지 않았다(일회성 시험 DB 55432만) |
| 상태 | 구현과 시험 완료, **독립 검토 전**. fe924b7 I-4 잔여는 사용자 결정 2026-11-16 (나)에 따라 02-05 단계 7b 별도 PR로 넘김(§9). 원장(`state.json`)은 고치지 않았다(원장 담당이 기록) |

## 1. 무엇을 대조하는가(먼저 정함)

**C10: 대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다.** GitHub head와 CI는 증거 입력 서비스(`evidence:ingest` 범위의 서비스 주체)가 넣은 신뢰 입력이다. 이 단계의 시험에서는 고정 입력이다(02-03과 같은 구분). GitHub에서 head·CI를 실제로 가져오는 수집기, 신원 규칙(F4), PR 수준 상태(N5)는 1b 작업이고, 그 작업이 이 입력 자리에 실제 수집 결과를 넣는다.

| 구분 | 무엇 | 출처·신뢰 수준 |
| --- | --- | --- |
| GitHub 브랜치 head | 새 증거 종류 `branch_head` {sha, ref(`refs/heads/<name>`), source_ref}. 같은 브랜치의 가장 늦게 들어온 기록이 현재 head | 증거 입력(신뢰 서비스 입력) |
| CI | 기존 `ci_run` {sha, conclusion, source_ref}. head SHA의 것만 읽는다 | 증거 입력 |
| 배정 브랜치 | 배정 요청의 새 선택 필드 `branch_ref` | 코디네이터 기록(배정 주체) |
| 작업 트리 HEAD·push SHA | supervisor가 체크포인트에 적는 `worktree_head`·`pushed_sha`·`branch_ref` | supervisor 보고(claim, 같은 UID의 개발 코드가 바꿀 수 있음) |
| CI 주장 | 그 배정의 최신 `development_progress` 보고의 `internal_checks` | 에이전트 보고(claim) |

체크포인트에는 CI 주장이 없다. supervisor는 CI를 관측하지 않는다. 그래서 비교하는 CI 주장은 에이전트 보고의 것뿐이다.

## 2. 더한 작업과 바뀐 동작

### 2.1 supervisor(`crates/ieo-supervisor/src/gitref.rs` 새 파일, `serve.rs`)

- 모든 체크포인트에 `worktree_head`(작업 디렉터리 `HEAD`의 SHA, 읽을 수 없으면 `"not_observed"`), `branch_ref`(start·reconnect 요청의 배정 브랜치, 없으면 null), `pushed_sha`(`refs/remotes/origin/<name>`의 SHA, push가 없거나 읽을 수 없으면 null)를 적는다. `tracked.worktree_head`는 `true`가 됐다.
- **`git`을 실행하지 않는다.** 작업 디렉터리의 저장소는 모델의 것이고(같은 UID), 저장소 설정(hook, `core.fsmonitor`)이 코드를 실행할 수 있다. `.git`의 ref 파일만 읽는다: `HEAD`, 느슨한 ref, `packed-refs`, 연결된 작업 트리의 `gitdir`·`commondir`. 소문자 16진 SHA(40·64자)만 받고, 1 MiB보다 큰 파일과 `..`가 든 ref 이름은 읽지 않는다.
- push SHA는 `git push`가 `origin`으로 보낼 때 갱신하는 원격 추적 ref다. 원격 이름 `origin`은 고정이다.
- start·reconnect는 선택 필드 `branch_ref`를 받는다. `refs/heads/<name>`이 아니면 400 `SCHEMA_VIOLATION`.
- 상태의 능력 공백 목록에서 “worktree_head not tracked”를 뺐다.

### 2.2 코디네이터(`crates/ieo-coord/src/api.rs`, `delivery.rs`, `schema.sql`)

- **증거 입력**: `branch_head` 종류. `ref`는 `refs/heads/<name>`(256바이트 이하, 공백·제어 문자·`..` 없음). `evidence:ingest` 범위만 넣을 수 있다. 스키마의 `evidence.kind` CHECK를 넓혔다(기존 DB는 기동 때 제약을 바꿔 단다).
- **배정**: issueAssignment가 선택 필드 `branch_ref`를 받는다(같은 검사). 배정 요청 기록에 그대로 남는다. 전달의 `coordinator_record`에 `branch_ref`가 늘었다.
- **재개 판단(compareCheckpoint, 개발 역할).** §13 항목 10의 대조가 `resume_allowed`일 때만 더 본다:
  - 배정에 `branch_ref`가 없으면 `not_observed`(GitHub head를 대조할 수 없음).
  - 기대 SHA: 중앙 체크포인트의 `pushed_sha`, null이면 배정 `target_sha`. `pushed_sha`가 SHA도 null도 아니면 `external_reconciliation_required`.
  - 그 브랜치의 `branch_head` 기록이 없으면 `not_observed`. 최신 기록이 기대 SHA와 다르면 `external_reconciliation_required`(사유에 브랜치·GitHub head·기대 SHA와 그 출처). 같으면 `resume_allowed`.
  - 기대 SHA는 중앙 기록(로컬과 digest까지 같은 것)에서 읽는다. 로컬 체크포인트를 고치면 digest 대조에서 먼저 외부 대조가 된다.
  - 새 세션(start)의 판단은 바꾸지 않았다. 계획의 규칙은 재개에 대한 것이다.
- **CI**: head 기록이 있으면 그 SHA의 `ci_run`만 읽는다(`source_ref`마다 최신. 모두 성공 → `success`, 하나라도 아니면 `failure`, 없으면 `none`). 다른 SHA의 CI는 쿼리에 들어오지 않는다. CI 결과는 재개를 막지 않는다. 기록만 한다.
- **CI 주장**: 현재 개발 배정의 최신 `development_progress` 보고(처분 `current`) 가운데 `target_sha`가 그 head인 것의 `internal_checks`(FAIL 하나 → 실패, 모두 PASS → 성공, 그 밖은 주장 아님). GitHub 결과와 다르면 `ci_claim.matches_github: false`로 기록하고 GitHub 결과를 쓴다.
- 답의 새 필드 `github`: `source`(증거 입력이라는 표시), `branch_ref`, `worktree_head`·`pushed_sha`(체크포인트의 주장), `expected_sha`·`expected_from`(`pushed_sha`/`target_sha`), `head`(증거 id·SHA·source_ref), `matches`, `ci` {sha, result, evidence}, `ci_claim`. 개발 역할의 `resume_allowed` 판단이 아니면 null.
- **Guardrail**: Guardrail 배정의 `target_sha`가 현재 개발 배정의 `target_sha`가 아니면(개발 배정이 없을 때 포함), 판단이 `resume_allowed`·`no_checkpoint`·`stale_generation`이었어도 새 값 `stale_target`이다. 사유: 새 검토 배정이 필요하다. 계획은 “재개하지 않는다”만 적었다. 낡은 대상을 새 세션으로 검토하는 것도 쓸모없어서 실행할 수 있는 세 판단을 모두 막았다(§7).
- **재사용 검사(f7cfb93).** 이 대조는 함수 하나 `github_head_check`(배정·세대와 체크포인트 → 판단, 거절 사유, 대조 기록)다. 대조 기록의 `result`는 `match`/`mismatch`/`not_observed`이고 `source`는 입력이 증거 입력 기록(실제 GitHub 읽기가 아님)임을 밝힌다. `assignment_id`·`generation`, head의 증거 id, 그 head의 CI와 다른 CI 주장도 담는다. compare가 이 함수를 쓰고, 단계 7b의 공식 복구 점검(사용자 결정 조건 §2.2 “단계 7의 GitHub head 대조를 재사용”)이 같은 함수를 쓴다.
- **전달(deliverAssignment)**: 판단의 `github`를 결과 행(`role_delivery_results.github`, 새 열)에 기록하고 결과에 싣는다. start·reconnect 본문에 `branch_ref`를 싣고, 재개이면 `github` {branch_ref, head_sha, ci}도 싣는다(보낸 본문 digest에 포함). `stale_target`은 아무것도 부르지 않는다(`not_called`).

### 2.3 단계 6 잔여(검토 6e0ad85)

- **L-a.** supervisor는 start·reconnect를 받기 전에(실행 관문 안, 중앙 대조·재조회 뒤) 대기 목록을 읽고 그대로 다시 써 본다. 실패하면 바로 409 `EXTERNAL_RECONCILIATION_REQUIRED`(사유와 `file`에 목록 파일 경로)로 거절하고 시도 id를 쓰지 않는다. 그래서 목록이 깨진 상태의 start가 202를 받은 뒤 `CentralRefused`로 끝나 호출자 원장에 실행되지 않은 회차를 남기는 일이 없다. `EXTERNAL_REQUEST_NOT_TRACKED`의 사유와 답의 `file`, 첫 행동 `not_sent` 답의 `file`, 상태의 `safe_shutdown.pending_external_requests_file`이 파일을 가리킨다. `local_only`는 확인하지 않는다(아무것도 보내지 않는다).
- **I-a.** `limits_e2e` (i)의 고정 200 ms 대기를 “장부 잠금을 잡았다”는 신호로 바꿨다.
- **I-b.** supervisor 시험이 `not_sent` 답의 `ok: false`, `not_sent: true`, `file`을 확인한다. 회차 중에 목록이 깨지는 새 단계 8에서다(아래).
- **I-c.** 런타임 계약 항목 18에 한 문장: 중앙 대조가 보관본에서 보내는 누락 체크포인트 보고는 목록 밖이다(대조가 곧바로 코디네이터 기록을 다시 읽어 결론을 낸다).

### 2.4 계약

- 런타임 계약 **revision 14**: 머리 기록, §16 항목 18(받기 전 확인, 파일 경로, 보관본 전송), 새 §17 항목 21~24(supervisor 기록, 개발 재개, CI와 CI 주장, Guardrail 대상). **뜻이 바뀐 필드가 하나 있다**: `worktree_head`는 늘 `"none"`(관측 아님)이었고 이제 실제 SHA다. 머리 기록과 §17에 “대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다”를 적었다.
- OpenAPI 0.14.0 → **0.15.0**: `branch_head` 종류, issueAssignment `branch_ref`, compareCheckpoint `github`·`stale_target`, 전달 결과 `github`, `GithubComparison` 스키마, 머리 기록의 동작 변경. `openapi-spec-validator` OK.
- proto·내부 계약은 바꾸지 않았다(supervisor의 start 본문은 HTTP 어댑터 JSON이다).
- **영속 DB**: 기동 때 이관이 돈다(`evidence_kind_check` 교체, `role_delivery_results.github` 열 추가). 영속 PoC DB(55433)에는 적용하지 않았다. 병합 뒤 적용은 따로 기록한다.

## 3. 수용된 작업의 동작 변경

사용자에게 알릴 항목이다.

1. **compareCheckpoint(02-04 수용).** 개발 역할의 `resume_allowed`는 이제 배정 `branch_ref`가 있고 GitHub head 기록이 기대 SHA와 같을 때만이다. 그렇지 않으면 `not_observed`(브랜치·기록 없음) 또는 `external_reconciliation_required`(다른 head, SHA가 아닌 `pushed_sha`)다. `branch_ref`가 없는 기존 배정은 이제 재개가 `not_observed`다. 닫는 쪽이다. 답에 `github`가 늘었다.
2. **compareCheckpoint, Guardrail(02-04 수용).** 새 판단 값 `stale_target`. 검토 대상이 현재 개발 대상이 아니면 `resume_allowed`·`no_checkpoint`·`stale_generation`이 `stale_target`이 된다. 판단 값의 열거가 늘었으므로 값을 열거하는 호출자는 영향을 받는다.
3. **supervisor 체크포인트(02-04 수용).** `worktree_head`의 뜻(`"none"` → SHA 또는 `"not_observed"`), `tracked.worktree_head: true`, 새 키 `branch_ref`·`pushed_sha`. 바이트가 달라지지만 digest는 늘 바이트 기준이다.
4. **supervisor 상태(02-04 수용).** 능력 공백에서 “worktree_head not tracked”가 빠졌다(3개). `safe_shutdown.pending_external_requests_file`이 늘었다. 수용된 시험 `status_reports_the_section_7_fields_and_an_honest_safe_shutdown`의 기대(4 → 3, 없어진 문구 확인)를 바꿨다.
5. **supervisor start·reconnect(02-04 수용).** 대기 목록을 읽거나 쓸 수 없으면 받기 전에 409 `EXTERNAL_RECONCILIATION_REQUIRED`다. 단계 6(수용 전)까지 start는 202 뒤 `CentralRefused`였고, reconnect는 읽을 수 없을 때만 409였다(쓸 수 없지만 읽히면 진행). 새 선택 필드 `branch_ref`가 잘못되면 400이다.
6. **ingestEvidence·issueAssignment(02-03 수용).** 추가만: 새 종류 `branch_head`, 선택 필드 `branch_ref`(잘못되면 400). 기존 요청의 결과는 같다.
7. 02-05 진행 중 작업(수용 전)의 변경: deliverAssignment의 `coordinator_record`에 `branch_ref`, 결과에 `github`, 보낸 본문에 `branch_ref`(와 재개이면 `github`)가 늘어 보낸 본문 digest가 달라진다. 단계 6의 `EXTERNAL_REQUEST_NOT_TRACKED` 답과 `not_sent` 답에 `file`이 늘었다.

## 4. 시험

새 시험(4개)과 넓힌 시험:

| 시험 | 내용 |
| --- | --- |
| `ieo-coord` 단위 `the_github_head_is_the_final_basis_of_a_development_resume` | 판단 표: 브랜치 없음·기록 없음 → `not_observed`, push 없음이면 target SHA와 같아야 함(다르면 사유에 “nothing pushed”), push 있으면 push SHA와 같아야 함, 다른 head·다른 push 주장 → 외부 대조, SHA가 아닌 `pushed_sha`(문자열·숫자·대문자) → 외부 대조 |
| `ieo-coord` `tests/coordinator.rs` `branch_heads_and_assignment_branches_are_checked_refs` | `branch_head` 입력(201, 같은 본문 200), 잘못된 ref·SHA·남는 키 400, 증거 범위가 없는 자격 403, 배정 `branch_ref` 검사(400 셋, 201) |
| `ieo-supervisor` 단위 `gitref::tests::head_and_pushed_sha_are_read_from_the_ref_files_only` | 저장소 없음, 해석 안 되는 심볼릭 HEAD, 느슨한 ref, `packed-refs`, 느슨한 ref 우선, SHA가 아닌 내용, 브랜치 ref가 아닌 이름, `..`, 분리된 HEAD, 연결된 작업 트리(`gitdir`·`commondir`) |
| `ieo-supervisor` `tests/serve.rs` `a_run_records_its_worktree_head_and_the_pushed_sha` | 저장소·브랜치 없음 → `not_observed`·null, 커밋·push한 회차(가짜 런타임이 ref 파일을 씀) → HEAD·push SHA·브랜치, 다른 브랜치 → null, 잘못된 `branch_ref` 셋 → 400 |
| `delivery_e2e`(두 전송, 넓힘) | 아래 표 |
| `tests/serve.rs` `a_refused_first_action_is_settled_and_an_untracked_request_is_not_sent`(두 전송, 넓힘) | 단계 7: 목록을 쓸 수 없으면 reconnect·start 모두 받기 전 409, 답과 사유에 파일 경로, 시도·실행 디렉터리·보고 없음, 상태에 파일 경로. 단계 8(새): 회차 중에 개발 코드가 `pending-requests.tmp` 자리에 디렉터리를 만듦 → 첫 행동 `{ok: false, not_sent: true, file}`, after_run 보고 `EXTERNAL_REQUEST_NOT_TRACKED`(`file` 포함), takeover만 코디네이터에 감 |
| `chain_e2e`·`checkpoint_e2e`·`limits_e2e`(두 전송) | 배정에 `branch_ref`, 증거 서비스 자격으로 `branch_head`(target SHA) 하나를 넣는 고정 입력만 더했다. 다른 확인은 그대로다. `limits_e2e` (i)는 신호로 기다린다(I-a) |

`delivery_e2e`의 단계 7 확인(가짜 런타임, 실제 PostgreSQL, 코디네이터 바이너리):

| 위치 | 확인 |
| --- | --- |
| 단계 2 | `coordinator_record.branch_ref`(배정의 브랜치) |
| 단계 4 전 | **GitHub 기록 없음 → `not_observed`**(사유에 브랜치, `expected_from: target_sha`, 검사 `result: not_observed`, `source`가 증거 입력임을 밝힘). 증거 서비스가 브랜치를 target SHA로 기록 → `resume_allowed`, head 일치, CI `none` |
| 단계 4 | **head 일치 → 재개**(reconnect 수락). 결과의 `github.head.sha`, supervisor가 받은 본문의 `github` {branch_ref, head_sha, ci: none}과 `branch_ref` |
| 단계 4 뒤 | **다른 SHA의 CI 무시**: 다른 SHA의 실패 기록 뒤에도 CI `none`. head SHA의 성공 기록 → 단계 6의 재개 전달 결과 `ci` {result: success, evidence: [ci-head]}, 받은 본문 `ci: success` |
| 단계 12 | **CI 주장 불일치 기록**: 개발 에이전트가 같은 head에 FAIL을 주장 → 판단은 `resume_allowed`, CI는 GitHub의 `success`, `ci_claim` {claimed: failure, matches_github: false} |
| 단계 12 | **push 뒤 head 불일치 → 외부 대조**: after_run이 push SHA를 보고(supervisor가 보고하는 대로 만든 체크포인트) → GitHub head가 옛 SHA라 외부 대조(사유에 두 SHA), `expected_from: pushed_sha`. 증거 서비스가 push를 기록 → 재개, 옛 head의 CI를 쓰지 않음(`none`), CI 주장도 옛 SHA 것은 안 씀. 새 head의 성공·실패 두 기록 → `failure` |
| 단계 12 | **로컬 체크포인트 변조 → 외부 대조**: 보고 뒤 로컬 파일의 push SHA만 바꿈 → digest 불일치 |
| 단계 12 | **로컬 ref만 옮긴 push 주장 → 외부 대조**: 다른 브랜치의 head가 그 SHA여도 쓰지 않음 |
| 단계 12 | **Guardrail 대상 낡음 → 재개 안 함**: 대상이 개발 target과 같으면 `no_checkpoint`, 다른 SHA로 새 배정 → `stale_target`(사유 “새 검토 배정”), 그 전달 `not_called`·supervisor 호출 없음, 다시 맞춘 배정 → `no_checkpoint` |

결과([evidence/test.log](evidence/test.log), 커밋 f7cfb93):
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: 통과.
- `openapi-spec-validator`: OK. proto 변경 없음.
- `cargo test --workspace --locked`(시험 DB, `RUST_TEST_THREADS=4`): **392 통과, 0 실패, 0 무시**. 기준(9b66300) 388에 새 시험 4개.
- 공유 대상 디렉터리의 다른 작업 트리 바이너리를 피하려고 작업 공간 일곱 패키지를 `cargo clean -p` 한 뒤 증거를 만들었다.

## 5. 변이 시험

각 변이를 하나씩 f7cfb93 코드에 넣고(c9be60f에서도 같은 23개가 모두 잡혔고, 재사용 검사로 옮긴 뒤 다시 돌렸다) 대상 시험을 돌린 뒤 파일을 되돌렸다(매번 `git status` 깨끗, [evidence/mutants.txt](evidence/mutants.txt)). **25개 모두 잡혔다.**

| id | 변이 | 잡은 시험 |
| --- | --- | --- |
| S7-G1 | 개발 재개를 GitHub로 막지 않음 | `delivery_e2e` |
| S7-G2 | 다른 head여도 재개 | 단위 판단 표 |
| S7-G3 | push SHA를 무시(target 기대) | 단위 |
| S7-G4 | SHA가 아닌 `pushed_sha`를 push 없음으로 봄 | 단위 |
| S7-G5 | 기록이 없어도 재개 | 단위 |
| S7-G6 | 다른 SHA의 CI를 읽음 | `delivery_e2e` |
| S7-G7 | CI 하나만 성공해도 성공 | `delivery_e2e` |
| S7-G8 | 다른 CI 주장을 일치로 기록 | `delivery_e2e` |
| S7-G9 | 다른 SHA의 CI 주장을 씀 | `delivery_e2e` |
| S7-G10 | Guardrail 대상을 보지 않음 | `delivery_e2e` |
| S7-G11 | 다른 브랜치의 head를 씀 | `delivery_e2e` |
| S7-G12 | 전달 결과에 `github`를 기록하지 않음 | `delivery_e2e` |
| S7-G13 | 재개 본문에 `github`를 싣지 않음 | `delivery_e2e` |
| S7-G14 | 본문에 `branch_ref`를 싣지 않음 | `delivery_e2e` |
| S7-G15 | supervisor가 push SHA를 기록하지 않음 | serve `a_run_records…` |
| S7-G16 | supervisor가 HEAD를 기록하지 않음 | 같음 |
| S7-G17 | 받기 전 목록 확인을 뺌(L-a) | serve `a_refused_first_action…` |
| S7-G18 | 배정 `branch_ref` 검사를 뺌 | coordinator `branch_heads…` |
| S7-G19 | `branch_head` ref 검사를 뺌 | 같음 |
| S7-G20 | SHA가 아닌 ref 파일을 받음 | 단위 `gitref` |
| S7-G21 | 검사가 not_observed를 mismatch로 보고 | `delivery_e2e` |
| S7-G22 | 검사가 match를 mismatch로 보고 | `delivery_e2e` |
| S7-N1 | 목록에 넣지 못한 첫 행동을 보냄(단계 6 C1-L1a) | serve `a_refused_first_action…`(단계 7의 재조회) |
| S7-N2 | 목록에 넣지 못한 체크포인트 보고를 보냄(C1-L1b) | 같음(새 단계 8. 받기 전 확인 뒤로는 takeover 경로로 닿지 않아 회차 중 경로로 확인) |
| S7-N3 | `not_sent` 답이 `ok: true`(검토 6e0ad85 I-b의 N3) | 같음 |

## 6. 이월 항목

- **C10 닫음(입력의 한계와 함께).** 개발 재개의 최종 근거는 배정 브랜치의 GitHub head 기록이고, CI는 그 head SHA의 기록만 본다. **대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다.** 실제 수집기·F4·N5는 1b가 이 입력 자리에 넣는다.
- **C14 일부.** `worktree_head`를 실제로 기록한다(push SHA·브랜치와 함께). 남은 C14 항목은 없다고 보지만 단계 9 수용 보고에서 문장별로 확인한다.

## 7. 계획과 다른 점, 결정한 것

- **Guardrail은 재개뿐 아니라 새 세션도 막는다(`stale_target`).** 계획은 “재개하지 않는다(새 검토 배정이 필요)”다. 낡은 대상을 새 세션으로 검토해도 쓸 데가 없고, 새 검토 배정을 하면 판단이 다시 정상으로 돌아온다(시험). 새 판단 값을 쓴 것은 `stale_generation`(새 세션 요청이면 start)이나 `external_reconciliation_required`(사람이 대조)와 뜻이 달라서다.
- **배정 브랜치는 배정의 선택 필드다.** 계획은 “배정 브랜치”를 말하지만 배정에 브랜치가 없었다. 필수로 하면 기존 배정 요청이 모두 깨지므로 선택으로 했고, 없으면 개발 재개는 `not_observed`(닫는 쪽)다.
- **push SHA는 원격 추적 ref다.** supervisor가 push를 직접 관측할 길이 없어 `refs/remotes/origin/<name>`을 읽는다. 개발 코드가 그 ref를 옮길 수 있지만, 그 주장은 GitHub 기록과 다르면 외부 대조가 된다(시험). `git`을 실행하지 않는 것은 보안 결정이다(§2.1).
- **CI는 재개를 막지 않는다.** 계획대로 결과를 판단 기록과 전달 본문에 싣기만 한다. 실패한 CI를 고치는 것이 재개의 이유일 수 있다.
- **새 세션(start)의 판단은 GitHub를 보지 않는다.** 계획의 규칙이 재개에 대한 것이고, start는 새 세션이 target SHA부터 시작한다.
- fe924b7 I-4 잔여는 사용자 선택지로 올렸고, 사용자가 (나) 공식 복구 작업을 골랐다(§9).

## 8. 한계와 넘기는 것

| 항목 | 내용 | 담당 |
| --- | --- | --- |
| 실제 GitHub 수집 | head·CI는 증거 입력 기록이다. 실제 수집기, 신원(F4), PR 수준 상태(N5) | 1b 작업 |
| 원격 이름 | push SHA는 `origin`의 원격 추적 ref만 본다. 다른 원격으로 push하면 null(→ target SHA 기대 → GitHub head가 다르면 외부 대조, 닫는 쪽) | 단계 9에 기록 |
| 같은 UID | `worktree_head`·`pushed_sha`·`branch_ref`는 개발 코드가 바꿀 수 있는 claim이다. 최종 근거는 GitHub 기록과의 대조이고, 다르면 외부 대조다 | 기록(§2.1 N5와 같음) |
| `branch_ref` 없는 배정 | 개발 재개가 `not_observed`다. 실제 운영 배정은 브랜치를 넣어야 한다 | 단계 8 전 확인 |
| 실행되지 않은 회차의 원장 기록 | L-a의 받기 전 확인으로 “목록이 깨진 start”의 경우는 실제로 없어졌다. 다른 원인(코디네이터가 takeover를 거부하거나 답을 잃음, 세션 잠금·설정 정리 실패 등)으로 실행되지 않은 회차는 여전히 전달 경로 원장에 `unmeasured`(사람 확인)로 남는다. 닫힌 쪽이다 | 02-05 단계 9 |
| 영속 DB 이관 | 기동 때 도는 이관(제약 교체, 열 추가)을 영속 PoC DB에는 아직 적용하지 않았다 | 병합 뒤 원장 담당 |
| fe924b7 I-4 잔여 | §9 | 02-05 단계 7b(공식 복구 PR) |

## 9. 검토 잔여 처리

| 항목 | 처분 |
| --- | --- |
| PR #74 검토 fe924b7 I-4 잔여(전달 없는 직접 start가 끝나지 않은 체크포인트 위에서 새 세션을 시작) | **사용자 결정 2026-11-16 (나): 공식 복구 작업을 02-05 단계 7b 별도 PR로 추가**([결정 기록](../../../../User_Decisions_2026-11-16_recovery.md), 조건은 그 문서 §2). 이 단계는 그 복구 점검이 재사용할 GitHub head 대조를 함수 하나(`github_head_check`: 배정·세대·체크포인트 → `match`/`mismatch`/`not_observed`와 증거, f7cfb93, §2.2)로 제공한다. 결정 전 확인한 사실: 중앙 기록이 코디네이터이면 직접 start는 takeover가 `DELIVERY_REQUIRED`로 거부되어 실행되지 않고(chain_e2e), 전달 경로의 start는 외부 대조로 막힌다. 즉 코디네이터 모드의 구멍은 이미 닫혀 있다. 대신 그 모드에는 충돌 뒤 **복구 경로가 없다**(새 세대도 외부 대조). `local_only`(시험·02-04 호스트 호출자)의 직접 start는 02-04가 설계한 복구다. 선택지: (가) 기록만(코디네이터 모드 닫힘, `local_only`는 설계된 복구, 코디네이터 모드 복구 부재를 단계 9의 한계로), (나) 운영자만 쓰는 기록되는 복구 작업(GitHub head 대조를 조건으로, 그 뒤 새 세션만 허용, supervisor 직접 start도 그 복구 id 없이는 거부), (다) supervisor가 늘 거부하고 사람이 볼륨에서 체크포인트를 치움. 작성자 권고는 (가)였고 사용자는 (나)를 골랐다. 이 단계는 이 항목의 코드를 바꾸지 않았다 |
| PR #78 검토 6e0ad85 L-a(파일 이름, 202 뒤 실패) | 교정. 사유·답·상태에 파일 경로, 받기 전 읽기·쓰기 확인으로 바로 409(§2.3). 시험 단계 7·8, 변이 S7-G17 |
| 6e0ad85 I-a(고정 200 ms) | 교정. 잠금을 잡았다는 신호를 기다린다 |
| 6e0ad85 I-b(N3 생존) | 교정. `ok`·`not_sent`·`file` 확인. 변이 S7-N3 잡힘 |
| 6e0ad85 I-c(보관본 전송 예외) | 교정. 런타임 계약 항목 18에 한 문장 |
| 6e0ad85 I-d(원장 문구) | 원장 담당 몫(손대지 않음) |
| 실행되지 않은 회차의 `unmeasured` 기록(옆 발견) | L-a로 목록이 깨진 경우는 실제로 없어졌다. 다른 원인은 남으므로 단계 9로 넘긴다(§8) |

## 10. 검증 명령과 정리

- 로그: [evidence/test.log](evidence/test.log)(fmt·clippy·openapi·전체 시험, 커밋 f7cfb93), [evidence/mutants.txt](evidence/mutants.txt)(25개).
- 시험 DB(55432) 정리: 처음 실패한 `chain_e2e` 실행과 두 차례 변이 실행이 남긴 `ieo_t_chain_*`·`ieo_t_dlv_*` 42개(22개, 그 뒤 20개, 모두 이 작업의 끝난 프로세스)를 이름으로 지웠다. 이 종단 시험들은 실패하면 자기 DB를 지우지 않는다(기존 동작). 끝난 뒤 `ieo_t_*`는 0개다. 55433·58090은 건드리지 않았다.
