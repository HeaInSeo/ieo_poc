# 03-02 · 공통 정책·역할·GitHub 기준 문서

계획 v1.3.3. `POC-REQ-03-02`, `poc-acceptance-v1.3.3@4 / 03-02.accept`. 03-01 작성 계약은 독립 승인 head와 병합 SHA main CI를 확인해 검수 완료했다. [03-01 수용 기록](../../03-01/catalog-contract-20261118/ACCEPTANCE.md)

재사용 입력은 계약 v0.1.0의 `CAT-ROLE`, `CAT-RECOVERY`, `CAT-GITHUB` 작성 후보와 고정 원본이다. 구성 요소별 원본과 검증 기준을 먼저 대조했다. [03-01 색인](../../03-01/catalog-contract-20261118/catalog.json)

| 작성할 기준 | 범용 규칙과 대조할 원본 | 적용 범위와 기대 증거 |
| --- | --- | --- |
| 검사·실패 대응 | [02-01 증거 계약](../../02-01/collector-20261112/REPORT.md) §1·§3 | 명령, 도구 버전, 범위, 결과·필수 산출물·실패 이유를 함께 보존. 성공·실패·불충분·비적용 판정 기대값을 분리. Go/Rust 명령 세부 값은 03-03에서 채움. |
| 역할·인계 | [런타임 계약 rev 20](../../01-02/runtime-contract-20261112/Runtime_Contract_v0.md) §2·§4.1 · [인계](../../../../DEV_AGENT_HANDOFF.md) §3·§6 | 개발·Guardrail·코디네이터의 책임과 승인 head 조건, 전달·재개 증거를 기록. 현재 저장소/역할의 권한 바인딩은 각각 고정하며 NPG 규칙 이식의 동작은 P21에서 확인. |
| Actions·저장소 설정 | [ieo_poc workflow](../../../../../../.github/workflows/ci.yml) · [NPG CI 원본 범위](../../../../05_npg_evidence/CI_Evidence_Scope.md) | workflow 선언과 실제 브랜치 보호·필수 검사의 관측을 구분. 필수 검사명·대상 브랜치·관측 SHA/시각을 저장소 전용으로 기록. 실제 enforcement는 P23 검증. |

2026-11-18 21:03 KST에 `b384643` 시점의 GitHub main 보호 설정을 읽었다. 최신 base 검사와 관리자 적용이 설정돼 있었다. 필수 검사 세 이름은 `docs/poc package and progress validation`, `rust toolchain pin resolves`, `rust fmt clippy test`였다. proto 검사는 별도 workflow job이며 팀 절차에서 확인한다. 실제 기준 문서에는 제출 직전 다시 읽은 설정을 고정한다. 독립 검토는 저장소의 승인 수와 별개로 팀 절차에 따른다.

이 절은 착수 시점의 재사용 입력과 확인할 범위를 보존한다.

## 03-02 작성·검증 결과

- [공통 정책](COMMON_POLICY.md)에는 테스트·린트의 범위/판정·증거, 개발·Guardrail·코디네이터 책임, 인계 및 실패 뒤 재개를 정리했다.
- [저장소 전용 기준](IEO_POC_GITHUB_PROFILE.md)에는 GitHub workflow·명령 및 job, main 보호 관측과 현행 조건 병합을 연결했다. 보호 설정은 [관측 원본](github-main-observation.json)에 기록했다. 관측 전후 main은 동일한 `d27fa96504b568fd3156d6c5ac21e7d2f333ff9e`였다. 보호 설정은 시점 관측이며 workflow와 도구 pin 바이트는 구현 기준 `317ca966d0bee6223f63a396b0ac805c6e30b84a`와 관측 SHA에서 일치했다.

| 수용 항목 | 대조 결과 |
| --- | --- |
| 테스트·린트와 실패 대응 | 로컬 명령 수집기 5/5. 정상 성공, 비0 종료, 기동 실패, 시간 초과, 필수 증거 누락을 각각 대조했다. 현재 저장소 Rust job의 포맷 check 및 경고 실패 규칙을 확인했다. |
| 역할·인계 | 격리 코디네이터 4/4. 자격·범위 위반에서 DB 상태 불변, 독립·현재 세대 검토, 정확한 SHA의 CI·실행 증거, 미해결 반려 차단을 검증했다. 역할 인자 생성 시험 1/1로 절대형 Edit·Guardrail scratch와 Write 표기 배제를 대조했다. |
| Actions·저장소 설정 | workflow의 실제 선언과 main 보호 관측을 대조했다. 필수 check 세 개와 추가 proto job을 구분했다. 운영상 독립 검토는 GitHub 승인 요구 수와 별개로 적용한다. |
| 범용/저장소 전용 | 공통 기준의 승인·검사·실패 규칙과 ieo_poc 전용 명령, job 이름 및 브랜치 보호 관측을 별도 문서로 고정했다. P23 실제 집행과 P25 전체 준비 검증은 후속 범위다. |

### 재현 및 검증 범위

[격리 시험 로그](isolated-test.log)의 로컬 기준은 `317ca96`에서 시작한 03-02 후보 워크트리다. 실행 코드와 workflow 변경은 0건이다. 저장소 루트에서 [검증 스크립트](verify-isolated.sh)를 `bash`로 실행하면 수집기·역할 인자·코디네이터 표적 시험을 재현할 수 있다. 코디네이터는 CI와 같은 pin의 PostgreSQL 17 이미지에서 임시 DB를 사용했다. 데이터 디렉터리는 tmpfs, 포트는 임시 loopback 바인딩, 보호/CI 증거와 자격은 synthetic fixture다.

`CI=1`에서 DB 주소가 없을 때 실제 시험이 실패하는 대조군을 먼저 실행했다. 설정된 경로에서는 주소와 서버 준비 상태를 기록했고 네 테스트마다 서버의 서로 다른 CREATE/DROP을 일치시켰다. 종료 직전 잔여 시험 DB는 0개였고 컨테이너를 정지했다. [격리 시험 로그](isolated-test.log).

첫 독립 검토 `7b7f14196cb5d26b2099f0451cbae1cc102f0198`의 지적에 따라 런타임 계약 §10 항목 1과 6을 공통 정책에 연결했다. Claude의 경로 인자는 정해진 표기와 도구 범위를 확인하는 근거다. 개발 Bash 허용 목록은 판정·격리 근거에서 제외하며, 경계는 컨테이너·역할 자격 범위·코디네이터 판정에서 확인한다. 실제 역할 격리는 P07에서 검증한다. 실서버 작업과 PoC 모델 호출은 0건이다.

상태는 검토 후보 준비다. 최신 main 및 X-05의 원장 반영 순서를 대조한 뒤 공통 원장 push와 PR 제출을 진행한다.
