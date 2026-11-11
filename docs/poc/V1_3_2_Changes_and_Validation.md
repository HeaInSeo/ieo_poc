# Console 코디네이터 PoC · v1.3.2 변경과 검산

2026-11-11 KST · v1.3.1 → v1.3.2 · 제공된 독립 리뷰 1~6 한정 반영

**여섯 지시에 모두 동의하고 반영했다.** 주계획은 9개 스프린트·72개 작업·30개 실험, 후속은 별도 12개 작업·5개 후보다. 작업 ID는 유지하고 직접 선행은 `07-01 ← 04-14` 한 건만 추가했다. 현재 실제 개발·probe·새 PoC 실험은 미실행이며 두 진행률은 0/72·0/12다. 계획/생성기 자체 검산을 01-06이나 P07/P26의 완료로 올리지 않았다.

기준 입력은 사용자가 전달한 `Console_Coordinator_v1_3_1_Review_Package_2026-11-10.zip`과 [이번 리뷰 원문](01_plan_and_review/Independent_Review_v1.3.1_and_v1.3.2_Instructions.md)이다. 별도 문서의 최신 v1.3.1 바이트와 묶음의 대응 파일도 같음을 대조했다. 현재 운영 서버/예약·NPG 소유권·GitHub 제품 브랜치를 변경하지 않았다.

## 지시별 반영

| 지시 | 위치 | 변경과 이유 |
| --- | --- | --- |
| 1 · 실행 단위 U3 | 상세 1/1.2/1.5, P09/P11/P22/P28/P34/P35, 장부 01-02/02-04/04-13/08-03/08-05, 구조도·인계·probe | repo당 개발/Guardrail 두 Pod, 각 컨테이너 하나로 바꿨다. 같은 호스트 단계는 컨테이너 둘이다. UID·파일시스템·자격·PV를 분리하고 공유 RW 볼륨과 직접 통신을 두지 않는다. supervisor는 Pod별 PID 1이며 자기 역할 하나만 제어한다. 어댑터 필드는 유지하고 역할별 container_or_pod_ref가 다름을 명시했다. U1은 U3으로 대체됐다고 기록했다. |
| 1 · 역할별 수명·복원 | 상세 1.5/P28/P34, 결정 필요 목록 | 두 역할이 모두 일 없어야 한다는 조건을 삭제했다. 작업 보유 대기와 일 없음은 구별한다. 역할별 상시 유지/idle 정지는 미결정으로 남겼고 P34에 개발만/Guardrail만/둘 다 재생성과 역할별 복원 시간·자원 측정을 넣었다. 과거 상시 유지 결정을 몰래 뒤집지 않았다. |
| 2 · S0 전 독립성 | P07, 04-07·02-06, S0 선행 경로 | 개발·빌드·테스트·자식의 Guardrail 작업공간/세션/승인 입력 접근, 자격 획득, 명의 위조 성공 각 0건을 필수로 넣었다. 하나라도 성공하면 해당 구성의 독립 검토 FAIL이다. 04-07은 이미 S0 선행이므로 추가 작업/새 DAG 선행은 불필요하다. |
| 3 · 검토 중 변경 | 새 1.9절, P07·02-06·04-07, 인계, P26 | 작업당 PR 하나·repo당 열린 수정 PR 하나, REVIEW_PENDING 중 개발 대기, 반려 전달 후 교정 push만 허용한다. 새 head면 이전 배정 무효/새 SHA 재배정·늦은 원 결과 보존. 승인 head SHA를 서버 측 병합 조건에 지정하고 불일치 시 거부한다. 의존 작업은 병합/main CI 뒤 배정한다. 경쟁·강제 push·무허가 push·정상 교정 사례와 검토 대기 유휴 시간을 추가했다. |
| 4 · 보고 유지 | 상세 1.8/P33, 01-06/02-01/02-07/09-02, 인계·progress·reports | state.json만 진행 원본으로 두고 제공한 render_progress.py로 STATUS/TASKS/RESULTS를 생성한다. events/RESUME도 원장의 파생물이다. 회차 보고/로그는 불변 증거다. 화면 손편집·5분/30분 보고 타이머를 없애고 다음 의존 작업 전·제어 반환 시점 갱신으로 바꿨다. 합산 진행률을 삭제했다. |
| 5 · P26 선행 | 07-01, 기술 DAG P26, P26 결과표·07-02/07-03 | P35(04-14)를 먼저 요구한다. P05-Q는 일괄 선행으로 추가하지 않고 한도로 인한 정지 분류를 채택했다. 관측 불가 한도가 정상 산출까지 막지 않게 하되 원래 최초 N건·완료율·종료 시각을 그대로 보존한다. 한도로 미완료인 표본도 분모에 남는다. 실제 quota 재개 시험은 P05-Q의 해당 증거를 요구한다. |
| 6 · 버전 | 현재 장부·후속·기술/후속 JSON·구조도 파일명, 내용·명세·참조 | 현재 문서/그림은 v1.3.2다. SprintSpec/ProgressReport의 v0는 계약 버전이므로 유지한다. 이전 검산/설계 로그/논문/CI 파일은 역사적 입력으로 그대로 보존한다. |

## 의존관계 재검산

`scripts/validate_package.py`가 JSON과 사람용 실행 장부를 독립 파싱해 대조한다. 자기 작업을 제외한 서로 다른 모든 조상 작업 수이며 시간·공수 예측이 아니다.

| 그래프/지점 | v1.3.1 | v1.3.2 |
| --- | --- | --- |
| 주계획 노드 / 연결 | 72 / 184 | 72 / 185 |
| 주계획 순환 / 미정의 선행 | 0 / 0 | 0 / 0 |
| 04-03 첫 실제 PR 앞의 조상 | 14 | 14 |
| 05-05 최소 S0 앞의 조상 | 24 | 24 |
| 07-02 NodeKit 백로그 앞의 조상 | 35 | 36 |
| 기술 의존 노드 / 연결 | 49 / 214 | 49 / 215 |
| 기술 의존 순환 / 미정의 선행 | 이번에 재검산 | 0 / 0 |
| 후속 노드 / 연결 | 이번에 재검산 | 12 / 18 |
| 후속 순환 / 미정의 선행 | 이번에 재검산 | 0 / 0 |

S0의 조상에 04-07이 있고 03 스프린트 작업은 없다. 07-02/07-03의 조상에 04-14가 있고 04-15는 없다. 기존 선행 삭제는 없다. 작업의 수용 기준을 바꾼 것은 이번 여섯 지시와 직접 연결되는 15개 행이며 그 외 57개는 이전 원문과 같다. 01-06에 생성기 산출물 한 항목을 추가했고 다른 작업의 산출물 목록은 그대로다. [이전 작업 계약 비교 입력](validation/v1.3.1_task_contracts.json)을 검산에 포함했다.

## 명세 revision과 digest

명세 JSON의 9개 스프린트는 모두 **revision 3**, 검사 프로파일 `poc-acceptance-v1.3.2` revision 3이다. 사용자 결정의 참조·원본 해시와 plan_version을 갱신했으므로 내용이 그대로인 스프린트도 새 revision/digest를 받는다. 실제 repo/주체/예산/일정은 아직 미바인딩이며 registration_state=draft를 유지한다.

| 명세 | revision | SHA-256 digest |
| --- | --- | --- |
| POC-01 | 3 | `0bcef8ef86435c293c3b053954d849f7f526b7c313b1125ce920c1ac76a894f0` |
| POC-02 | 3 | `e1d1a5d3663725847b0aef0c53bd9c53edacc8c1f23458dbb2544affbb12de99` |
| POC-03 | 3 | `ee8ae6f6d1ec9995e8e4ecd2f613848897db0e14cd4839344d992b94783de668` |
| POC-04 | 3 | `5bcfe851d2d92d152a0f4b581948b338ad392d3fc7933651c89bccd684f02c62` |
| POC-05 | 3 | `74ddd127b335e844a7fde32de5a17c04c0958e4b6f669034cd8d588c0a5c7ddb` |
| POC-06 | 3 | `35123a11d1c7337472b1be82230b027717ef9bdbbf3ed87366d4e1dd796348e3` |
| POC-07 | 3 | `cb10aaf18e1eb5c3986dd097429c848092645460234cc42769e44f000e9fcd8d` |
| POC-08 | 3 | `b23a4423363cc2db5b79b0f29afa5b4d1957f540792c2edf785bafe2d95c12ad` |
| POC-09 | 3 | `cb5908a7000ae8341767ff209fe2965aaaa0d6ed679844b46682544e1ebe5069` |


키 정렬·공백 없는 UTF-8 JSON에서 digest 필드만 제외해 SHA-256을 재계산한다. 9개 명세의 JSON Schema, 프로파일 항목/수용 문구, 사람용 장부의 ID/선행/수용 문구와 출처 해시를 대조한다. 기계 결과는 [v1_3_2_validation.json](v1_3_2_validation.json)에 남긴다. 런타임 어댑터와 명세/보고 스키마의 필드는 바꾸지 않았다.

## 보고 생성기 확인

`python3 scripts/render_progress.py`는 원장을 읽어 화면을 생성하고 `--check`는 저장된 화면과 재생성 예상값을 대조한다. 회차·작업 ID 중복, 기준선 불일치, 증거 없는 검수 완료, 아직 없는 로컬 보고 링크 등은 화면 쓰기 전에 거부한다. 원본 증거 URL의 실제 내용이나 독립 검수는 이 도구가 대신 확인하지 않는다.

초기 0/72·0/12, 유효한 검수 1건 반영, 여러 회차 보존, 잘못된 완료/중복 회차/없는 링크 거부, 생성 중단 뒤 동일 원장 재생성을 임시 fixture에서 확인한다. 이는 **파일 생성기 자체 확인**이며 실제 PoC 작업/실험 PASS가 아니다. 결과는 [생성기 확인](validation/report_generation_check.json)을 따른다.

## 결정 필요·실측 미확인

| 항목 | 아직 필요한 것 | 처리 지점 |
| --- | --- | --- |
| 런타임 선택 | CLI/SDK·버전·모델·무인 기동/재개 계약과 실제 지원 여부 | 01-02 / 01-04 |
| 두 역할의 인증·동시 세션·한도 | 같은 계정으로 두 세션 동시 실행, 개발/Guardrail 자격·한도 분리 가능 여부. 프로세스 분리를 계정/공급자 한도 분리로 추정하지 않음 | 01-04의 첫 probe 두 항목 |
| 역할별 상시 유지/idle 정지 | idle 기간·각 역할 복원 시간/자원·미확정 효과와 운영 영향. U3이 정지 정책까지 확정한 것은 아님 | 01-03 / P34 / P28-M |
| 실제 repo·저장소·주체 | URL, 각 Pod UID/자격/PV 경로, 정책 승인 주체, 실제 사용자 열람 링크 | 01-02 / 01-04 / 01-06 |
| 표본·예산·일정 | 최초 표본·난도·N·수용/충분성 목표, 누적 시간/토큰/교정 상한. 근거 없는 예상 납기 없음 | 01-03 / 07-01 |
| 운영 이전·제품 권한 | 최신 설치/actor/lock/미확정 요청, 운영 창과 실제 인수 | P27의 정상 절차 |

probe 15개 항목은 여전히 미실행/미확인이다. 이번 검토자는 이전 리뷰가 읽지 않은 P 본문 전체나 실측을 대신 검증한 것이 아니다. 5.2 기술 의존과 후속 의존은 이번에 구조상 재계산했지만 런타임 동작은 아직 확인하지 않았다. 다음은 01-01부터 계약을 고정하고 런타임을 선택한 뒤 01-04 probe의 실제 결과를 얻는 것이다.

## 바뀐 파일

| 파일 | 변경 |
| --- | --- |
| `00_READ_FIRST.md` | 내용/참조/검산 갱신 |
| `06_remaining_checks/Runtime_and_Operational_Probe_Checklist.md` | 내용/참조/검산 갱신 |
| `Console_Coordinator_Architecture_v1.3.2.dot` | 이름 변경 (`Console_Coordinator_Architecture_v1.3.dot`) 및 내용/참조/검산 갱신 |
| `Console_Coordinator_Architecture_v1.3.2.png` | 이름 변경 (`Console_Coordinator_Architecture_v1.3.png`) 및 내용/참조/검산 갱신 |
| `Console_Coordinator_Architecture_v1.3.2.svg` | 이름 변경 (`Console_Coordinator_Architecture_v1.3.svg`) 및 내용/참조/검산 갱신 |
| `Console_Coordinator_Followup_Experiments_v1.3.2.md` | 이름 변경 (`Console_Coordinator_Followup_Experiments_v1.3.md`) 및 내용/참조/검산 갱신 |
| `Console_Coordinator_PoC_Execution_Ledger_v1.3.2.md` | 이름 변경 (`Console_Coordinator_PoC_Execution_Ledger_v1.3.md`) 및 내용/참조/검산 갱신 |
| `Console_Coordinator_PoC_Validation_Plan.md` | 내용/참조/검산 갱신 |
| `Console_Coordinator_Sprints_v0.json` | 내용/참조/검산 갱신 |
| `DEV_AGENT_HANDOFF.md` | 내용/참조/검산 갱신 |
| `Followup_Execution_v1.3.2.json` | 이름 변경 (`Followup_Execution_v1.3.json`) 및 내용/참조/검산 갱신 |
| `PAPER_REPO_MAPPING.md` | 내용/참조/검산 갱신 |
| `REPORTING_REVIEW_NOTES.md` | 내용/참조/검산 갱신 |
| `Technical_Dependencies_v1.3.2.json` | 이름 변경 (`Technical_Dependencies_v1.3.json`) 및 내용/참조/검산 갱신 |
| `User_Decisions_2026-11-11.md` | 이름 변경 (`User_Decisions_2026-11-10.md`) 및 내용/참조/검산 갱신 |
| `progress/README.md` | 내용/참조/검산 갱신 |
| `progress/RESULTS.md` | 내용/참조/검산 갱신 |
| `progress/RESUME.md` | 내용/참조/검산 갱신 |
| `progress/STATUS.md` | 내용/참조/검산 갱신 |
| `progress/TASKS.md` | 내용/참조/검산 갱신 |
| `progress/state.json` | 내용/참조/검산 갱신 |
| `reports/templates/CONVERSATION_UPDATE.md` | 내용/참조/검산 갱신 |
| `reports/templates/EXPERIMENT_REPORT.md` | 내용/참조/검산 갱신 |
| `reports/templates/TASK_REPORT.md` | 내용/참조/검산 갱신 |
| `reports/templates/result.template.json` | 내용/참조/검산 갱신 |
| `01_plan_and_review/Independent_Review_v1.3.1_and_v1.3.2_Instructions.md` | 신규 원문 보존 또는 생성/검산 산출물 |
| `V1_3_1_SOURCES.json` | 신규 원문 보존 또는 생성/검산 산출물 |
| `V1_3_2_Changes_and_Validation.md` | 신규 원문 보존 또는 생성/검산 산출물 |
| `scripts/render_progress.py` | 신규 원문 보존 또는 생성/검산 산출물 |
| `scripts/validate_package.py` | 신규 원문 보존 또는 생성/검산 산출물 |
| `v1_3_2_validation.json` | 신규 원문 보존 또는 생성/검산 산출물 |
| `validation/report_generation_check.json` | 신규 원문 보존 또는 생성/검산 산출물 |
| `validation/v1.3.1_task_contracts.json` | 신규 원문 보존 또는 생성/검산 산출물 |


원문 폴더의 과거 설계/리뷰·논문/repo·NPG CI는 제공 바이트 그대로다. 원 SOURCES.json은 V1_3_1_SOURCES.json으로 보존하고 현재 SOURCES.json/SHA256SUMS.txt는 v1.3.2로 다시 계산한다.

## 기술 근거의 해석 범위

U3은 이 PoC의 실행 결정으로 채택했다. Restricted의 비루트·권한 상승 금지·capabilities 제한 아래에서 기존 supervisor의 UID 전환에 의존하는 U1 구성을 승인할 근거가 없었다. 다만 Restricted가 모든 단일 컨테이너 격리 기법의 불가능성을 증명하는 것은 아니며, 두 Pod도 호스트/노드 장애까지 독립된다는 뜻은 아니다. 따라서 Pod 분리만으로 PASS를 주지 않고 P07/P11/P34의 실제 거부·복원 증거를 요구한다. 이 설명은 여섯 개정 지시의 거절이나 범위 변경이 아니다.

공식 근거(2026-11-11 조회): [Kubernetes Restricted](https://kubernetes.io/docs/concepts/security/pod-security-standards/), [NetworkPolicy](https://kubernetes.io/docs/concepts/services-networking/network-policies/), [GitHub head SHA 조건부 병합](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request). NetworkPolicy는 이를 집행하는 네트워크 구현과 실제 허용/거부 시험이 필요하다. 병합 전 단순 재조회 대신 GitHub의 head SHA 조건을 사용하는 이유도 이 계약에 따른다.
