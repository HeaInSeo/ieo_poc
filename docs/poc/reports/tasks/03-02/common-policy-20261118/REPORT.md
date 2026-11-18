# 03-02 · 공통 정책·역할·GitHub 기준 문서 준비

계획 v1.3.3. `POC-REQ-03-02`, `poc-acceptance-v1.3.3@4 / 03-02.accept`. 03-01 작성 계약은 독립 승인 head와 병합 SHA main CI를 확인해 검수 완료했다. [03-01 수용 기록](../../03-01/catalog-contract-20261118/ACCEPTANCE.md)

재사용 입력은 계약 v0.1.0의 `CAT-ROLE`, `CAT-RECOVERY`, `CAT-GITHUB` 작성 후보와 고정 원본이다. 구성 요소별 원본과 검증 기준을 먼저 대조했다. [03-01 색인](../../03-01/catalog-contract-20261118/catalog.json)

| 작성할 기준 | 범용 규칙과 대조할 원본 | 적용 범위와 기대 증거 |
| --- | --- | --- |
| 검사·실패 대응 | [02-01 증거 계약](../../02-01/collector-20261112/REPORT.md) §1·§3 | 명령, 도구 버전, 범위, 결과·필수 산출물·실패 이유를 함께 보존. 성공·실패·불충분·비적용 판정 기대값을 분리. Go/Rust 명령 세부 값은 03-03에서 채움. |
| 역할·인계 | [런타임 계약 rev 20](../../01-02/runtime-contract-20261112/Runtime_Contract_v0.md) §2·§4.1 · [인계](../../../../DEV_AGENT_HANDOFF.md) §3·§6 | 개발·Guardrail·코디네이터의 책임과 승인 head 조건, 전달·재개 증거를 기록. 현재 저장소/역할의 권한 바인딩은 각각 고정하며 NPG 규칙 이식의 동작은 P21에서 확인. |
| Actions·저장소 설정 | [ieo_poc workflow](../../../../../../.github/workflows/ci.yml) · [NPG CI 원본 범위](../../../../05_npg_evidence/CI_Evidence_Scope.md) | workflow 선언과 실제 브랜치 보호·필수 검사의 관측을 구분. 필수 검사명·대상 브랜치·관측 SHA/시각을 저장소 전용으로 기록. 실제 enforcement는 P23 검증. |

2026-11-18 21:03 KST에 `b384643` 시점의 GitHub main 보호 설정을 읽었다. 최신 base 검사와 관리자 적용이 설정돼 있었다. 필수 검사 세 이름은 `docs/poc package and progress validation`, `rust toolchain pin resolves`, `rust fmt clippy test`였다. proto 검사는 별도 workflow job이며 팀 절차에서 확인한다. 실제 기준 문서에는 제출 직전 다시 읽은 설정을 고정한다. 독립 검토는 저장소의 승인 수와 별개로 팀 절차에 따른다.

다음은 범용·저장소 전용 상세 문서 작성, 긍정·실패·비적용 대조 및 독립 검토다. 03-02는 준비됨이다. 모델 요청·운영 변경은 0건이다.
