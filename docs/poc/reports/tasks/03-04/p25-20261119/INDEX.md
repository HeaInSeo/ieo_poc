# P25 초기 사용 색인 v0.2.1

이 색인은 고정한 03-02 공통·저장소 정책과 03-03 Go·Rust 상세 문서를 찾기 위한 자료다. [색인 원본](catalog-use.json)은 항목 버전, 적용·제외 조건, 역할, 수용 프로파일과 원본을 기록한다. [동결 원본](inputs.sha256)과 [독립 기대값](EXPECTATIONS.md)을 같은 검증 회차에서 사용한다.

| 범위 | 선택할 항목 | 상세 |
| --- | --- | --- |
| 범용 | CAT-ROLE@0.2.1, CAT-RECOVERY@0.2.1 | [공통 검사·역할·재개](../../03-02/common-policy-20261118/COMMON_POLICY.md) |
| Go | CAT-GO@0.2.1 | [Go 정책](GO_POLICY.md) |
| Rust | CAT-RUST@0.2.1 | [Rust 정책](RUST_POLICY.md) |
| HeaInSeo/ieo_poc | CAT-GITHUB-IEO-POC@0.2.1 | [Actions·브랜치 정책](../../03-02/common-policy-20261118/IEO_POC_GITHUB_PROFILE.md) |

조회는 배정 역할, 대상 스택과 저장소의 정확한 조합으로 한다. 등록되지 않은 스택·대상은 별도로 판단한다. 승인된 스택 밖, 배정 역할 밖, 전용 정책의 다른 저장소는 색인 조회에서 제외한다. Go 셔플·레이스, Rust 기능/target·문서 시험 같은 조건부 검사는 적용 승인과 환경 근거가 필요하다. NPG의 역사적 항목은 기존 [카탈로그](../../03-01/catalog-contract-20261118/catalog.json)에 보존한다.

P25 대조에서 필수 시험의 `skip`·`ignored`를 확인하면 적용 요구 위반으로 FAIL을 기록한다. 필수 실행 결과의 존재나 범위를 확인할 수 없으면 INCONCLUSIVE다. 두 판정 모두 통과 증거가 부족해 준비 완료를 보류한다. 승인된 적용 제외만 N/A로 기록한다. 이 분류는 원본 도구의 종료 코드와 별개다.
