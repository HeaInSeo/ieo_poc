# 01-07 · 스프린트 명세 v0·등록 검사 계약 · spec-contract-20261112

| 필드 | 기록 |
| --- | --- |
| 계획·작업·회차 | v1.3.3 · 01-07 · run `spec-contract-20261112` rev 1 · 이전 회차 없음 |
| 선행 | 01-01 검수 완료(원장) |
| 담당·단계 | 개발 에이전트(설계+참조 구현) → 독립 검토 · 2026-11-12 KST |
| 수용 기준 | `poc-acceptance-v1.3.3` rev 4 `01-07.accept` |
| 산출물 | [Registration_Contract_v0.md](Registration_Contract_v0.md) · 참조 구현 `crates/ieo-spec`(Rust, 같은 PR) · 재사용: SprintSpec/HumanSprintInput 스키마, Human_Sprint_Authoring_v0.md, Schedule_and_Role_Reports_v0.md |

## 수용 기준 대조

| 기준 요소 | 근거 |
| --- | --- |
| 1.4절 필드·digest·대상 repo 하나·작업별 검증/산출물/종료 조건·JSON Schema 고정 | 기존 스키마 유지 + 9개 명세 digest를 독립 구현으로 9/9 재현 |
| 필드 누락·미정의 선행·순환·없는 프로파일 항목·미바인딩 draft 거부 구분 | 계약 §2 코드표, 적합성 사례(각각 다른 코드·필드 위치) |
| P01 의미 검토와 연결 | 통과 상태 `ACCEPTED_FOR_SEMANTIC_REVIEW`, 프로파일 밖 기준의 의미 검토 회부(계약 §1·§3) |
| 사람용 양식 필수/선택·예시·작성/변환 검토 담당·v0 변환 규칙 | 기존 Human_Sprint_Authoring_v0.md + 계약 §3 변환 결과 기록 |
| digest·항목 ID는 변환기가 채움, 거부는 필드 위치·수정 내용 | 변환 사례, `every_rejection_has_field_and_correction` |
| 작업별 근거 있는 예상 범위, 기한/기한 없음/미확정 구별 | 변환 사례(measured/unestimated, none/undecided/set) |
| 표현 불가 필드는 공백 목록 | 계약 §4 |

## 실행·실측 (원본: evidence/)

| 명령 | 결과 |
| --- | --- |
| `cargo test --locked` | exit 0 · 단위 4 + 변환 11 + 등록 16 = 31 passed, 0 failed ([cargo-test.log](evidence/cargo-test.log)) |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 ([cargo-clippy.log](evidence/cargo-clippy.log)) |
| `cargo fmt --all -- --check` | exit 0 ([cargo-fmt-check.log](evidence/cargo-fmt-check.log)) |
| `ieo-spec check-manifest Console_Coordinator_Sprints_v0.json` | exit 0 · 9/9 201, computed_digest = 패키지 digest ([draft](evidence/check-manifest-draft.txt)) |
| `… --executable` | exit 1 · 9/9 422, 각 UNBOUND_DRAFT 6건만 ([executable](evidence/check-manifest-executable.txt)) |
| 수동 변이 3건 | 3/3 검출 ([mutation-check.txt](evidence/mutation-check.txt)) |
| 환경 | rustc 1.99.1, cargo 1.99.1 ([toolchain.txt](evidence/toolchain.txt)); 빌드 산출물 target 896 MiB, 레지스트리 56 MiB(`/opt`) |

## 판정·한계

설계 계약·참조 구현 완료(claim). 검수는 독립 검토로 정한다.
인증된 등록 API·DB·의미 검토(P01)·다른 repo 무단 배정(403)은 이 작업에서 구현/판정하지 않았다(01-08 계약, 02-03/04-01 구현·검증).
비용: 시간·토큰 미계측(대화 세션의 역할별 원시 사용량 수단 없음).
