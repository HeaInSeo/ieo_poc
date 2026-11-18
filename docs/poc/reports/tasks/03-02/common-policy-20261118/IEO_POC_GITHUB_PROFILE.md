# ieo_poc 저장소 전용 GitHub 기준 v0.1.0

`CAT-GITHUB@0.1.0`의 ieo_poc 적용 문서다. [03-01 색인](../../03-01/catalog-contract-20261118/catalog.json)의 저장소 전용 NPG 항목은 역사적 예시로 유지한다. ieo_poc의 적용 대상은 `HeaInSeo/ieo_poc`, 통합 브랜치는 `main`이다. 다른 repo는 자체 필수 검사, 권한, 브랜치 보호와 merge 방식을 관측해 별도 프로파일로 만든다.

## Actions 선언과 도구 pin

[Workflow](../../../../../../.github/workflows/ci.yml)는 PR 및 `main` push에서 실행된다. 기본 `contents: read`와 고정된 checkout/setup action을 사용한다. 작업별 적용 내용은 아래와 같다.

| Job 이름 | 실행 범위 | 보호 설정 관측 |
| --- | --- | --- |
| `docs/poc package and progress validation` | 패키지·원장 뷰·report 동작 시험·OpenAPI 검사 | 필수 |
| `rust toolchain pin resolves` | `rust-toolchain.toml` 및 Rust·rustfmt·Clippy 버전 pin 대조 | 필수 |
| `rust fmt clippy test` | 포맷 check, Clippy 경고 실패, 전체 target 테스트, 격리 PostgreSQL, 보존 stream 재생과 manifest 대조 | 필수 |
| `proto buf lint and breaking` | 고정 buf lint/build, main 대비 WIRE_JSON 호환 검사 | 추가 CI job. 현재 팀 절차에서 4/4로 함께 확인 |

예를 들어 Rust job의 선언은 `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked --all-targets`다. 버전과 권한은 workflow 및 [도구 pin](../../../../../../rust-toolchain.toml)으로 확인한다. 문서 변경의 개발자 검산에는 문서·계약 표적 검사를 먼저 사용하고, 병합은 현재 head의 전체 지정 CI를 확인한다. 기존 Actions의 보존 stream 재생은 공급자 호출 없이 수행한다.

## 보호 설정 관측과 병합 확인

[읽기 전용 관측](github-main-observation.json)은 관측 전후 같은 main SHA를 확인했다. GitHub branch protection은 최신 base 검사(strict), 관리자 적용, PR 보호 객체와 stale review dismissal을 표시했다. 설정상 GitHub 승인 요구 숫자는 0이다. 팀 운영상 별개로 독립 승인 증거를 확보한다. 강제 push와 삭제 허용은 꺼져 있었으며 merge 방식은 merge commit으로 제한됐다.

병합 담당은 승인과 각 검사에서 정확히 같은 head를 대조한다. 누락·진행 중·실패·미계측·현재 head가 아닌 결과와 미해결 반려는 보류하고, 변경된 head는 다시 검토한다. 현재 저장소의 승인 SHA 조건 병합은 `gh pr merge --merge --match-head-commit <approved-sha>`를 사용한다. 병합 확인에는 PR head, 조건에 사용한 SHA, merge SHA와 같은 SHA의 main check/run 결과를 기록한다.

관측 파일은 설정의 당시 스냅샷이다. 준비된 사례를 실행하거나 제출할 때 최신 상태를 다시 읽는다. P23에서 실제 환경의 설정 집행 효과를 검증하며, 이 작업에서는 설정 변경 없이 위 선언·관측과 격리 판정 근거를 대조한다.
