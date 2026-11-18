# CAT-RUST@0.2.0 · Rust 검사 정책과 실행 예

`POC-REQ-03-03 / poc-acceptance-v1.3.3@4 / 03-03.accept`. [작성 계약](../../03-01/catalog-contract-20261118/CATALOG_CONTRACT.md)의 Rust 항목과 [03-02 공통 기준](../../03-02/common-policy-20261118/COMMON_POLICY.md)을 구체화한다. Rust 패키지·워크스페이스 변경에 적용한다. 대상 프로파일에서 패키지·feature·target·문서 시험과 외부 서비스 시험 범위를 지정한다.

격리 예는 **rustc/Cargo 1.99.1**, **rustfmt 1.10.0-stable**, **Clippy 0.1.99**, **Linux amd64**를 사용한다. 저장소의 pin을 유지하며 외부 의존성이 없는 격리 크레이트로 기본 동작을 확인했다. [대상 도구 pin](../../../../../../rust-toolchain.toml) · [ieo_poc Actions의 기존 검사 방식](../../03-02/common-policy-20261118/IEO_POC_GITHUB_PROFILE.md)

| 검사 | 고정 예시와 중요 옵션 | 기대 결과와 한계 |
| --- | --- | --- |
| 형식 | `cargo fmt --all -- --check` | 연결된 형식 대상의 차이 출력과 비0 종료를 대조한다. `--check`는 형식을 쓰지 않는다. 별도 설정·워크스페이스 밖 파일은 프로파일에서 범위를 확인한다. |
| 린트 | `cargo clippy --locked --all-targets -- -D warnings` | 현재 대상 컴파일 및 Clippy 경고를 실패로 처리한다. 샘플에서는 `len_zero` 경고와 비0 종료를 확인한다. feature 선택·빌드 플랫폼이 다른 실행은 별도 근거가 필요하다. |
| 기능 | `cargo test --locked --all-targets` | 선언한 패키지의 실제 테스트 이름과 결과, 실패·ignored 수를 확인한다. 예시의 기본 feature와 실제 활성화 target은 대조해야 한다. `--all-targets`는 별도 doctest나 모든 feature 조합을 증명하지 않는다. |

`--locked`는 잠금이 없는 상태 또는 변경이 필요한 해석을 거부한다. 예행은 `--offline`과 격리된 Cargo 홈·target 경로를 추가했다. 대상에 따라 승인된 `--workspace`·`-p`·`--features` 설정을 명시하고 doctest가 필수이면 해당 패키지의 `cargo test --locked --doc` 증거를 별도로 받는다. DB나 네트워크가 필요한 검사는 승인된 환경과 설정을 확인한다. 실행 스크립트의 CI 생략 분기, 필수 `ignored` 또는 테스트 0건은 종료 코드 0이어도 INCONCLUSIVE다. 승인된 비적용은 사전 사유를 기록한다. 결함·설정 위반은 FAIL로 기록하고, 의도한 음성 대조는 오류와 종료가 기대값에 맞을 때 대조 시험 PASS를 부여한다.

[격리 실행 기록](evidence/isolation.txt)에 명령별 wall time과 peak RSS를 남겼다. 캐시를 분리한 작은 크레이트의 관측으로, 전체 제품 빌드나 외부 DB의 시간·메모리 추정은 별도 근거가 필요하다. 모델 요청은 0회다. [02-01 증거 계약](../../02-01/collector-20261112/REPORT.md)
