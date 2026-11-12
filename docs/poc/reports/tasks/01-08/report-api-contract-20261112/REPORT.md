# 01-08 · 진행 보고 v0·등록/조회 API 계약 · report-api-contract-20261112

| 필드 | 기록 |
| --- | --- |
| 계획·작업·회차 | v1.3.3 · 01-08 · run `report-api-contract-20261112` rev 1 · 이전 회차 없음 |
| 선행 | 01-07(검수 완료 후에만 병합·수용) |
| 담당·단계 | 개발 에이전트(설계+참조 구현) → 독립 검토 · 2026-11-12 KST |
| 수용 기준 | `poc-acceptance-v1.3.3` rev 4 `01-08.accept` |
| 산출물 | [Report_API_Contract_v0.md](Report_API_Contract_v0.md) · `contracts/ieo-api-v0.openapi.yaml` · 참조 구현 `crates/ieo-report` · 재사용: ProgressReport_v0 스키마, Schedule_and_Role_Reports_v0.md, 역할 예시 4개 |

## 수용 기준 대조

| 기준 요소 | 근거 |
| --- | --- |
| 주장/판정·단계·운영 상태·사유·증거·다음 담당 고정 | 기존 스키마 유지 + 계약 §1 경로별 근거 수준 |
| 설계 로그의 5부 보고·재알림·완료 표현·4개 시험 입력 보존 | 계약 §3, `render_human`/`notification`/`review_currency`, 사례 `input1~4` |
| 인증된 등록·현재 상태·사건 조회와 CLI 계약 | OpenAPI(외부 `$ref` 해석 검증), 계약 §2·§4. 01-02 계약이 요구한 Guardrail 재현 요청 경로 포함 |
| actor 역할/이름 분리, 네 역할 본문 | 기존 스키마·역할 예시 4개 유효, 이름 비신뢰(`claim_authorization`) |
| 개발/검토 claim, 코디네이터/결정 verified 유지 | 스키마 조건 + `authorize_claim`(403 3종) |
| 일정 상태·남은 작업 수·지연 사유/영향·근거 있는 예상 범위 또는 미산정 | 기존 스키마 + 의미 검사(역전 범위·기한 전 late), 5부 보고의 일정 줄 |

## 실행·실측 (원본: evidence/)

| 명령 | 결과 |
| --- | --- |
| `cargo test --locked -p ieo-report` | exit 0 · 단위 1 + 계약 21 passed ([log](evidence/cargo-test-ieo-report.log)) |
| `cargo test --locked` (작업공간) | exit 0 · ieo-spec 41 + ieo-report 22 = 63 passed ([log](evidence/cargo-test-workspace.log)) |
| `cargo clippy --all-targets --locked -- -D warnings` / `cargo fmt --check` | exit 0 / exit 0 |
| `openapi-spec-validator contracts/ieo-api-v0.openapi.yaml` | OK, exit 0; 깨진 `$ref` 사본은 exit 2 ([validate](evidence/openapi-validate.log) · [negative](evidence/openapi-negative.txt)) |
| 수동 변이 3건 | 3/3 검출 ([mutation-check.txt](evidence/mutation-check.txt)) |

`validate_contracts.py`의 무효 사례 12건을 같은 이름으로 Rust에 이식했고 둘 다 거부한다(Python은 `validate_package.py` 안에서 계속 실행).

## 판정·한계

계약·참조 구현 완료(claim). 검수는 독립 검토로 정한다. 실제 HTTP 서버·인증·DB·발송은 구현하지 않았다(02-03/02-05/02-07). 사람 이해도는 04-09(P12).
비용: 시간·토큰 미계측(대화 세션의 역할별 원시 사용량 수단 없음).
