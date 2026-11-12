# PR #10 독립 검토 (Guardrail) · head e691dfa · base main 36bce10

VERDICT: reject

검토 대상: `fix/02-03-prereq-stale-revision` e691dfa (코드 커밋 e9b312f + 증거 커밋). 저장소는 수정하지 않았고, push·댓글도 하지 않았다. 임시 worktree는 `--force`로 제거했다.

## 요약

- 01-07 N1(옛 revision 승격·재활성화)은 해소됐다. `check_registration`에서도 `Registry::add_spec_with_mode`에서도 우회 경로를 찾지 못했다.
- 01-08 N1(늦은 옛 배정 결과 보존, 재조회와 새 stale 요청 구분)도 대부분 해소됐다. 403이 먼저 판정되고, 그다음 같은 ID 재전송, 그다음 배정 분류 순서다. 다른 principal은 재전송할 수 없고, 귀속 SHA는 코디네이터 기록에서 가져온다.
- 남은 문제 하나는 P2다. **현재 배정으로 낸 claim의 본문 `body.target_sha`가 그 배정의 SHA와 달라도 `current`로 판정된다.** 그 claim은 현재 판정에 쓰이고 코디네이터 SHA에 귀속된다(F1). 사용자 요구는 "옛 SHA의 결과는 현재 판정에 쓰지 않는다"인데, 이 구멍이 그 요구를 우회한다. 고정 시험도 이 동작을 정답으로 박아 두고 있다.
- 증거 연결은 확인됐다. diff가 비어 있고, 76건이 통과하며, 변이 5개를 재실행해 모두 검출됐다. 다만 M7은 등가 변이가 아니다(F2, P3).

## 발견

### F1 · P2 · 현재 배정 claim의 본문 SHA와 배정 SHA가 달라도 현재 판정에 들어간다

- `crates/ieo-report/src/lib.rs:409`: `Some((a, true)) => Ok(ClaimDisposition::Current { target_sha: a.target_sha })`. 본문 `body.target_sha`와 대조하지 않는다.
- 재현 요청은 대조한다(`lib.rs:459`). 비대칭이다.
- 재현 probe: 고정 예시 `guardrail_review`의 `body.target_sha`를 `ffff…`로 바꾸고 book의 현재 SHA는 `c1`로 두면 결과가 `Ok(Current { target_sha: "c1" })`, `counts_for_current_judgment=true`다.
  - 즉 SHA X를 검토한 approve가 현재 SHA `c1`의 승인으로 귀속돼 현재 판정에 쓰인다.
  - 같은 claim을 사람에게 보여 주는 `render_human`은 본문 SHA X를 표시한다(`lib.rs:666`). 영수증의 `attributed_target_sha`는 `c1`이다. 한 claim에 SHA가 둘이 된다.
- `crates/ieo-report/tests/report_contract.rs:204,208`: 고정 예시의 본문 SHA `aaaa…`와 배정 SHA `c1`이 이미 다른데, 시험은 `current()`를 기대한다. 불일치 수용이 시험에 정답으로 고정돼 있다.
- 왜 문제인가: "본문 SHA를 믿지 않는다"는 지켜졌다. 하지만 본문이 다른 SHA의 결과임을 스스로 밝혀도 현재 판정에 넣는다. 이것은 사용자 요구 1("원 SHA·배정에 보존하되 현재 판정에 사용하지 않는다")과 01-08의 "이전 SHA 승인은 현재 판정에 쓰지 않음"(`completion_summary`)을 우회한다.
- 교정안:
  1. 새 claim의 `body.target_sha`가 있으면 귀속 배정의 SHA와 같아야 한다. 다르면 409 `STALE_ASSIGNMENT`로 거부하거나, 현재 판정에서 빼고(예: `preserved_late`와 같이 취급) 보존한다. 어느 쪽이든 계약에 명시한다.
  2. 고정 예시 또는 시험의 book SHA를 일치시킨다.
  3. 불일치 시험을 추가한다.
  4. Report_API_Contract §1과 OpenAPI `submitAgentReport` 설명에 반영한다.

### F2 · P3 · M7은 등가 변이가 아니다(보고서 주장 정정 필요, 시험 공백)

- `docs/poc/reports/workspace/prereq-0203-fixes-20261112/REPORT.md:21,32`, `evidence/mutants.txt:9,21`
- 반례: 저장된 claim `E`(발급된 배정)가 있다. 같은 `event_id`·`event_revision`에 `assignment_id="never-issued"`로 바꾼 본문을 보낸다.
  - 원 코드: `Err(ContentConflict)`, 즉 409 `EVENT_CONTENT_CONFLICT`
  - M7: `Err(StaleAssignment)`, 즉 409 `STALE_ASSIGNMENT`
  - 응답 코드가 달라 관측 가능하고, 계약 §1(rev 3)의 "재전송을 배정 검사보다 먼저" 순서를 위반한다.
  - probe 시험으로 M7이 검출됨을 확인했다.
- "저장된 claim은 항상 발급된 배정을 가리킨다"는 맞다. 하지만 **들어온 본문**은 다른 배정을 가리킬 수 있다.
- 교정: 위 반례를 `same_id_replay_is_checked_before_assignment`에 추가하고, 보고서의 "등가" 표현을 정정한다.

### F3 · P3 · `Registry::add_spec_with_mode`의 같은 revision 처리

- `crates/ieo-spec/src/check.rs:86-118`
  - 옛 revision은 막는다. 하지만 같은 revision에 다른 digest를 넣으면 `true`를 돌려주고 digest와 작업 집합을 덮어쓴다. probe에서 그 뒤 원래 내용 재제출이 409 `REVISION_CONFLICT`가 됐다.
  - executable로 저장된 최신 revision을 `add_spec`(draft)로 다시 넣으면 mode가 draft로 내려간다. probe에서 그 뒤 executable 재제출이 200이 아니라 201로 다시 접수됐다.
- 옛 revision의 재활성화는 아니다. 호출자가 검사를 통과한 뒤에만 기록한다는 전제에 기대는 부분이다. 02-03 영속화 때 같은 revision 다른 digest의 덮어쓰기와 mode 하향을 거부하도록 권고한다.
- `revision`이 없는 명세는 `rev=0`으로 취급돼 `false`를 돌려준다(probe). 문제없다.
- 호출자 영향: `crates/ieo-spec/src/bin/ieo-spec.rs:42,67`이 `bool`을 무시한다. check-manifest가 서로 다른 스프린트를 넣는 용도라 영향은 없다. `convert.rs`는 `add_spec`을 호출하지 않는다.

### F4 · P3 · OpenAPI 호환성과 재전송 영수증의 의미

- `contracts/ieo-api-v0.openapi.yaml:395-413`의 `ClaimReceipt`
  - `additionalProperties: false`인 스키마에 필수 필드 5개(`disposition`, `assignment_id`, `generation`, `attributed_target_sha`, `counts_for_current_judgment`)가 추가됐다.
  - `RegistrationResult.state`에 `SUPERSEDED`가 추가됐다. 낮은 revision의 코드도 `REVISION_CONFLICT`에서 `REVISION_SUPERSEDED`로 바뀌었다.
  - 기존 v0 클라이언트나 검증기와 호환되지 않는 변경인데 `info.version`은 `0.4.0` 그대로다(`:4`). 버전 올림이나 변경 기록이 필요하다.
- 200 재전송은 "the stored receipt"를 돌려준다. 그런데 배정이 바뀐 뒤 재전송하면 저장된 영수증에 `counts_for_current_judgment=true`/`disposition=current`가 그대로 나온다. 코드의 `Replay`는 판정에 세지 않는다. 그 값이 접수 시점의 값임을 명시해야 한다.
- `ClaimReceipt` 생성 코드나 시험이 없어 스키마와 코드의 일치가 기계적으로 고정돼 있지 않다.

### F5 · P3 · Idempotency-Key 재전송이 옛 revision을 활성처럼 보일 수 있음

- `contracts/ieo-api-v0.openapi.yaml:41`: "returns the stored result with the current `superseded_by`"
- 저장된 결과가 `ACCEPTED_FOR_SEMANTIC_REVIEW`(201) 또는 `ALREADY_REGISTERED`라면, 대체된 뒤의 키 재전송이 활성 상태를 뜻하는 state를 돌려줄 수 있다.
- 이 경로는 참조 구현에 없고 시험도 없다. state를 `SUPERSEDED`로 바꾸는지 명시하도록 권고한다.

### F6 · P3 · 시험이 요구를 간접적으로만 고정함

- `report_contract.rs:272-275`의 "늦은 claim이 현재 배정을 바꾸지 않음" 검사는 공허하다. book을 불변 참조로 넘기므로 실패할 수 없다.
- "현재 판정에 사용하지 않는다"는 `counts_for_current_judgment` 플래그로만 고정돼 있다. `review_currency`(`lib.rs:779`)와 `completion_summary`는 disposition을 입력으로 받지 않는다.
  - probe: 옛 세대가 현재와 같은 SHA(`c1`)를 가지면 `PreservedLate { target_sha: "c1" }`가 된다.
  - 호출자가 SHA만으로 `review_currency`를 채우면 이 늦은 claim이 현재 승인으로 셀 수 있다.
  - 02-03에서 판정 입력을 disposition으로 거르는 시험을 요구한다.
- 요구 2는 시험으로 잘 고정돼 있다. 승격 거부(draft/executable), Registry 재기록 거부, 200 `SUPERSEDED`, 빠진 작업 미해석, 현재 revision의 `ALREADY_REGISTERED`가 모두 포함된다.
- 다음 경우는 probe로 동작만 확인했고 시험은 없다.
  - superseded + 다른 내용: 409 `REVISION_CONFLICT`
  - draft로 저장된 옛 revision의 draft 재제출: 200 `SUPERSEDED`(`superseded_by=6`)

### F7 · P3 · 재전송 조회 범위가 명시되지 않음

- `authorize_claim`/`authorize_repro`의 `stored` 인자, 그리고 계약 §1 `Report_API_Contract_v0.md:22`.
- `event_id`/`request_id` 저장 조회가 (repo, role) 범위인지 정해져 있지 않다. 전역이면 다른 repo의 자격이 같은 ID로 409 `EVENT_CONTENT_CONFLICT`를 받아 존재를 알 수 있다.
- 다른 principal의 재전송 성공은 불가능함을 확인했다. 본문 repo/role이 자격과 같아야 하고 저장본과 완전히 같아야 하기 때문이다.

### F8 · P3 · 증거 표기

- `evidence/cargo-test.log:1`의 "git status clean: 1 changes"는 앞뒤가 모순된다.
- `mutants.txt`에는 변이 설명만 있고 실제 diff가 없다. 그대로 재현할 수 없어 이번 검토에서는 변이를 독자적으로 구성했다.

## 확인한 사항(문제 없음)

- 발급된 적 없는 배정이나 세대, 다른 role의 book, 빈 book은 409 `STALE_ASSIGNMENT`다.
- 옛 발급 배정은 `preserved_late`로 그 배정의 SHA에 귀속되고, 현재 판정에 세지 않는다.
- 재전송은 403 검사 뒤에 판정된다. 다른 repo는 `REPO_SCOPE_DENIED`다.
- 재현 요청:
  - 옛 세대의 새 요청과 다른 SHA의 새 요청은 409다.
  - 동일 재전송은 `Replay`이며 다시 대기열에 넣지 않는다.
  - 판정 순서는 403(role → repo) → 재전송 → 배정이다.
- 등록 검사:
  - 더 새 revision 뒤의 옛 revision 신규 등록(draft·executable)은 409 `REVISION_SUPERSEDED`다.
  - draft로 저장된 옛 revision의 executable 승격도 409다.
  - 옛 revision의 동일 재제출은 200 `SUPERSEDED`이며 `superseded_by`가 붙는다.
  - `add_spec_with_mode`는 옛 revision을 `false`로 거부하고 상태를 바꾸지 않는다.
  - 새 revision을 등록하면 작업 집합이 교체된다.
- 계약 문서 rev 3과 OpenAPI의 코드·상태·순서가 코드와 일치한다. F1의 본문 SHA 규칙은 코드와 문서 모두에 없다.

## 실행한 명령

```
git diff 36bce10 e691dfa --stat; git diff 36bce10 e691dfa -- crates contracts docs/poc/reports/tasks
git diff e9b312f e691dfa -- crates contracts | wc -c                       # 0
git worktree add --detach <scratch>/wt-e9b e9b312f
export PATH=/opt/rust/cargo/bin:$PATH; cargo fmt --all --check; cargo clippy -q --all-targets -- -D warnings; cargo test
  # FMT=0 CLIPPY=0 TEST=0; passed 1+32+4+16+23 = 76
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml      # OK (e9b312f == e691dfa)
python3 -I <scratch>/mut/run.py <wt> M1 M3 M6 M10 M9 M7                    # 한 번에 하나 적용 → cargo test → git checkout
  # M1 killed, M3 killed, M6 killed, M10 killed, M9 killed, M7 SURVIVED (기존 시험으로)
  # M7 + probe zz_probe_m7: killed (ContentConflict → StaleAssignment), 비등가
probe 시험(임시, 원복): body SHA 불일치(F1), 같은 SHA 옛 세대(F6), 등록 edge(F3/F6)
git checkout --detach e691dfa; .venv-poc/bin/python docs/poc/scripts/validate_package.py   # EXIT=0
git worktree remove --force <scratch>/wt-e9b
```

→ 교정 커밋 8eb2f98(코드)·ea902dd(증거). 새 head를 다시 독립 검토한다. (PR #10 댓글 끝줄)
