# PR #10 독립 재검토 (Guardrail) · head ea902dd · base main 36bce10

VERDICT: approve

- 검토 대상: `fix/02-03-prereq-stale-revision` ea902dd. 코드 커밋은 8eb2f98, 그 뒤 커밋은 증거만 더한다.
- 직전 검토는 e691dfa를 reject했다(`review-pr10-e691dfa.md`).
- 저장소는 수정하지 않았고 push·댓글도 하지 않았다. 임시 worktree 2개는 `--force`로 제거했다.

## 요약

- P2였던 F1은 해소됐다. 본문 `body.target_sha`가 배정에 기록된 SHA와 다르면, 현재 배정이든 옛 배정이든 409 `ASSIGNMENT_SHA_MISMATCH`다.
- 403 → 같은 ID 재전송 → 배정 조회 → SHA 대조 순서가 유지된다.
- 아래 경로는 찾지 못했다.
  - 늦은 claim, stale claim, 다른 SHA의 claim이 현재 판정에 들어가는 경로
  - 옛 revision이 등록·승격·재활성화되는 경로
- 증거 연결을 확인했다.
  - `git diff 8eb2f98 ea902dd -- crates contracts`는 비어 있다.
  - 깨끗한 worktree에서 fmt·clippy가 통과하고 시험 78건이 통과한다.
  - 변이 5개(M7, M7b, M12, M13, M14)를 `git apply`로 적용해 모두 검출됐다.
  - `validate_package.py`가 EXIT=0이다.
- P1·P2는 없다. 남은 것은 P3 후속 항목뿐이다.

## 이전 지적 처리

| ID | 심각도 | 판정 | 근거 (e691dfa..ea902dd) |
|---|---|---|---|
| F1 | P2 | **해소** | `lib.rs:415-421`: 배정을 조회한 뒤, 본문 SHA가 문자열이고 `a.target_sha`와 다르면 `AssignmentShaMismatch`(409)를 낸다. 현재 배정과 옛 배정 모두에 적용된다. 시험의 book SHA를 예시 본문 SHA(CUR)로 맞췄다. `body_sha_must_match_the_assignment_sha` 시험이 두 경우를 고정한다: 현재 배정에 OLD0을 낸 approve, 옛 세대에 CUR를 낸 claim. 계약 §1(`Report_API_Contract_v0.md:23`)과 OpenAPI(`:210-211`)에도 반영했다. M12가 검출된다. |
| F2 | P3 | **해소** | `report_contract.rs`의 `same_id_replay_is_checked_before_assignment`에 반례를 추가했다. 같은 ID에 발급 이력 없는 배정을 넣으면 `ContentConflict`가 나와야 한다. REPORT의 "등가" 주장도 정정했다. M7(diff 그대로)이 검출됨을 재현했다. |
| F3 | P3 | **해소** | `check.rs:97-108`: 같은 revision에 다른 digest가 오면 `false`를 돌려주고 아무것도 바꾸지 않는다. executable로 저장된 방식은 draft로 낮추지 않는다. `registry_never_overwrites_a_revision_or_narrows_its_mode` 시험을 추가했다. M13·M14가 검출된다. 새로 생긴 경계 사례는 N2에 적었다. |
| F4 | P3 | **부분 해소** | OpenAPI를 0.5.0으로 올리고 비호환임을 명시했다(`:4,8-9`). 재전송 영수증의 `counts_for_current_judgment`는 "응답 시점 재계산"으로 명시했다(`:206-207,421-423`). 남은 것은 두 가지다(N3). 영수증을 만드는 코드나 시험이 여전히 없다. 코드 `Replay`의 판정 사용 여부는 항상 false다. |
| F5 | P3 | **해소(계약 수준)** | OpenAPI `:43-45`와 01-07 계약 §2에 적었다. Idempotency-Key 재전송도 현재 등록 상태로 다시 평가해, 대체된 revision이면 `SUPERSEDED`를 돌려준다. 참조 구현에는 이 경로가 없어 시험할 대상이 없다. |
| F6 | P3 | **부분 해소** | 실패할 수 없던 공허한 검사는 주석으로 대체했다. 계약 `:24`에 규칙을 추가했다: 판정 입력은 `counts_for_current_judgment`가 참인 claim뿐이고, SHA만으로 거르지 않는다. REPORT에는 남은 한계로 적었다. 그러나 `review_currency`(`lib.rs:793`)와 `completion_summary`(`:824`)는 여전히 처분(disposition)을 모른다. 02-03 후속으로 넘어간다(N4). |
| F7 | P3 | **해소(계약)** | 계약 `:22`, OpenAPI `:203,208`, `Denied::ContentConflict` 문서 주석에 재전송 조회 범위를 (repo, role)로 명시했다. 다만 `authorize_claim`의 `stored` 인자 주석(`lib.rs:382`)에는 범위가 빠져 있다. 사소하다. |
| F8 | P3 | **부분 해소** | `mutant-diffs/` 15개를 추가해, 변이를 diff 그대로 재현할 수 있다. 그러나 `cargo-test.log:1`의 "uncommitted changes before run: 1"이 무엇인지 설명이 없다. 깨끗한 worktree에서 78건을 독립 재현했으므로 증거 연결에는 영향이 없다(N5). |

## 새 지적 (모두 P3, 후속)

### N1 · P3 · 01-08 계약에서 `ASSIGNMENT_SHA_MISMATCH`가 검사 순서와 엔드포인트 표에서 빠짐
- `Report_API_Contract_v0.md:26`: 검사 순서가 "… → 배정 분류 → 400 → 422"로 적혀 있다. 코드는 배정 분류 직후, 400 앞에서 SHA를 대조한다(`lib.rs:415-421`). 이 단계를 순서에 넣어야 한다.
- `Report_API_Contract_v0.md:40`: `POST /v0/agent/reports`의 409 목록에 `ASSIGNMENT_SHA_MISMATCH`가 없다.
- `contracts/ieo-api-v0.openapi.yaml:42-43`: "… An\n An Idempotency-Key"로 단어가 중복됐다. 오타다.
- 본문 SHA가 `null`이면 "없음"으로 취급해 대조를 건너뛴다. 이것이 계약에 명시돼 있지 않다. `development_progress`는 스키마가 `null`+`sha_unavailable_reason`을 허용하므로 이 처리가 맞다. 계약 문구는 "있으면"뿐이다.

### N2 · P3 · `Registry::add_spec_with_mode`의 불변성이 호출자가 준 digest 문자열에 기대고 있음
- `check.rs:97`: `spec["digest"]`를 그대로 쓰고 `spec_digest`로 다시 계산하지 않는다.
- probe 1(digest는 같고 내용이 다른 경우): `tasks`를 비운 명세에 저장된 digest를 붙여 넣었다. `add_spec_with_mode`가 `true`를 돌려주고 작업 집합을 교체했다.
- probe 2(digest가 없는 경우):
  - digest 없는 명세가 `""`로 기록된다. 교정 전에는 기록하지 않았다.
  - 그 뒤 정상 제출이 409 `REVISION_CONFLICT`가 된다.
- 두 경우 모두 옛 revision의 재활성화는 아니다. `check_registration`이 digest 일치를 먼저 확인하는 호출 순서에서는 일어나지 않는다.
- 회귀가 아님을 확인했다. `ieo-spec check-manifest`(draft·executable)와 `ieo-spec check`(sprint 0~2)의 출력은 main 36bce10과 8eb2f98에서 바이트 단위로 같다. 매니페스트 명세는 모두 digest를 가진다.
- 02-03 영속화 때 Registry 안에서 digest를 다시 계산하거나, digest가 없으면 거부하도록 권고한다.

### N3 · P3 · 재전송 영수증과 코드의 의미 차이
- 계약: 재전송 영수증의 `counts_for_current_judgment`는 응답 시점에 다시 계산한다. 배정이 여전히 현재면 true일 수 있다.
- 코드: `ClaimDisposition::Replay.counts_for_current_judgment()`는 항상 false다(`lib.rs:376-378`). probe에서 아직 현재 배정인 claim을 재전송해도 `Replay counts=false`였다.
- 보수적인 방향(판정에 덜 넣는 쪽)이라 안전 문제는 아니다.
- `ClaimReceipt.disposition`은 재전송 때 원래 값(`current`)을 유지하는데, `counts`는 false가 될 수 있다. 두 필드가 어긋날 수 있다는 점을 스키마 설명에 적으면 좋다.

### N4 · P3 · 현재 판정 입력 규칙의 시점(F6 후속)
- 계약 `:24`는 "`counts_for_current_judgment`가 참인 claim만" 판정 입력으로 쓴다고 한다. 그 값이 접수 시점에 고정된 것인지, 판정 시점에 다시 계산한 것인지는 명시하지 않았다(재전송 영수증만 응답 시점 재계산으로 명시).
- 접수 시점 값을 저장해 쓰면 문제가 생긴다. gen 1에서 `current`로 받은 approve는, 배정이 gen 2(같은 SHA)로 넘어간 뒤에도 입력에 남을 수 있다.
- 02-03 계약이나 구현에서 판정 입력을 "판정 시점의 현재 (assignment_id, generation)과 같은 claim"으로 정의하고, 그 시험을 두도록 권고한다.

### N5 · P3 · 증거 로그 머리줄
- `evidence/cargo-test.log:1`의 "uncommitted changes before run: 1"이 어떤 변경이었는지 적혀 있지 않다(예: 추적되지 않는 증거 디렉터리였는지).
- 독립 재현으로 결과 자체는 확인됐다. 다음 보고서부터는 변경 목록을 함께 남기기를 권고한다.

## 새 변경 검토에서 확인한 사항 (문제 없음)

- **검사 순서**(`lib.rs:388-436`): RoleMismatch / RepoScopeDenied / VerifiedFromAgentDenied / RoleMismatch(역할·보고 유형)의 403 → `stored` 재전송(200 또는 409 `EVENT_CONTENT_CONFLICT`) → `book.lookup` → SHA 대조(409) → `Current`, `PreservedLate`, 또는 409 `STALE_ASSIGNMENT`. 403은 여전히 재전송보다 먼저다.
- **본문 SHA 경계값**(probe):

  | 경우 | `authorize_claim` 결과 | 이후 |
  |---|---|---|
  | guardrail의 `null`·숫자·누락 | `Current` | 스키마가 400으로 거부(`invalid()=true`). guardrail 스키마는 `target_sha`를 필수 문자열로 요구한다(`ProgressReport_v0.schema.json:740-753`). |
  | development의 `null` + `sha_unavailable_reason` | `Current`, 배정 SHA에 귀속 | 스키마 유효. 계약 의도에 맞는다. |
  | 대문자 SHA | 409 `AssignmentShaMismatch` | — |
  | 옛 세대에 현재 SHA를 쓴 claim | 409 | 시험으로 고정 |

- **serde 이름**: `ContentConflict`는 `"EVENT_CONTENT_CONFLICT"`로, `AssignmentShaMismatch`는 `"ASSIGNMENT_SHA_MISMATCH"`로 직렬화된다. 시험으로 고정돼 있다. 나머지 이름은 `SCREAMING_SNAKE_CASE` 그대로다.
- **본문 SHA가 없는 보고 유형**: 에이전트 경로에서 허용되는 유형은 `development_progress`와 `guardrail_review` 둘뿐이다. 둘 다 본문에 `target_sha`가 있다. 그 밖의 유형은 403으로 먼저 거부된다.
- **재현 요청**(`authorize_repro`): 바뀌지 않았다. 현재 배정과 SHA가 같을 때만 대기열에 넣는다. M8·M11 시험이 있다.
- **Registry**(probe):

  | 경우 | 결과 |
  |---|---|
  | 더 새 revision 뒤 옛 revision을 executable 또는 draft로 다시 기록 | `false` |
  | 옛 draft 저장분의 executable 승격 | 409 `REVISION_SUPERSEDED` |
  | executable 저장분 재제출 | 200 `SUPERSEDED`(`superseded_by=6`) |
  | 같은 내용을 draft → executable로 넓히기 | `AlreadyRegistered` |

- **OpenAPI 0.5.0**: `openapi-spec-validator` OK. `Rejection.code`에 `ASSIGNMENT_SHA_MISMATCH`가 들어 있다.
- **01-07 계약 rev 3**: 등록 시험 24건을 확인했다. 계약 문구는 코드와 일치한다.
- **REPORT.md 수치**: 78 = 24 + 33 + 21(1 + 4 + 16). 변이 15개는 11개에 M7과 M12~M14를 더한 수다. 시험한 SHA는 8eb2f98이다. 세 가지 모두 일치한다.

## 실행한 명령

```
cat review-pr10-e691dfa.md
git log --oneline -8 ea902dd
git diff e691dfa ea902dd --stat; git diff e691dfa 8eb2f98 -- crates
git diff e691dfa ea902dd -- contracts docs/poc/reports/tasks docs/poc/reports/workspace/prereq-0203-fixes-20261112/REPORT.md
git diff 8eb2f98 ea902dd -- crates contracts | wc -c                                    # 0
git worktree add --detach <scratch>/wt-8eb 8eb2f98
export PATH=/opt/rust/cargo/bin:$PATH; cargo fmt --all --check; cargo clippy -q --all-targets -- -D warnings; cargo test
  # FMT=0 CLIPPY=0; passed 1+33+4+16+24 = 78
for m in M7 M7b M12 M13 M14: git apply mutant-diffs/$m.diff; cargo test; git apply -R
  # M7 killed (same_id_replay_is_checked_before_assignment)
  # M7b killed (same_id_replay_is_checked_before_assignment)
  # M12 killed (body_sha_must_match_the_assignment_sha)
  # M13 killed (registry_never_overwrites_a_revision_or_narrows_its_mode)
  # M14 killed (registry_never_overwrites_a_revision_or_narrows_its_mode)
  # git status --porcelain crates → 0
probe tests (임시 추가 → git checkout -- crates로 원복): 본문 SHA null/숫자/누락/대문자, dev null, 현재 배정 재전송, Registry 위조 digest/digest 없음/승격/재기록
git worktree add --detach <scratch>/wt-main 36bce10
ieo-spec check-manifest [--executable], ieo-spec check sp{0,1,2} --manifest   # main == 8eb2f98 (동일 출력)
.venv-poc/bin/openapi-spec-validator contracts/ieo-api-v0.openapi.yaml           # OK
git checkout --detach ea902dd; .venv-poc/bin/python docs/poc/scripts/validate_package.py   # EXIT=0
grep -c '^test .* ok$' evidence/cargo-test.log                                     # 78
git worktree remove --force <scratch>/wt-8eb; git worktree remove --force <scratch>/wt-main
```
