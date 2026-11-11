# Console 코디네이터 PoC · v1.3.3

2026-11-12 KST · v1.3.2 기준선 위에 표준화/사람 평가 리뷰 다섯 항목 반영

주계획 **9개 스프린트·73개 작업·31개 실험**, 별도 후속 **12개 작업·5개 후보**다.
이전 72/30 기준선과 변경 이유를 보존했다. 새 작업은 04-16/P36 하나이며 04-09 뒤, 07-01 전에 연결한다. S0 선행에는 넣지 않는다.
실제 진척은 [현재 상태](progress/STATUS.md)를 본다. 계획/명세의 미착수 값은 등록 전 기준선이며 실행 진척 원본은 progress/state.json 하나다.
내려받은 ZIP은 전달 시점의 스냅샷이다. 운영/제품의 자동 현황이 구현됐다는 뜻은 아니다.

1. [변경·검산·결정 필요](V1_3_3_Changes_and_Validation.md)
2. [실행 장부](Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md) · [실험 상세](Console_Coordinator_PoC_Validation_Plan.md)
3. [사람용 입력](Human_Sprint_Authoring_v0.md) · [일정과 역할별 보고](Schedule_and_Role_Reports_v0.md) · [P36 절차](P36_Protocol_and_Measurement.md)
4. [개발 인계](DEV_AGENT_HANDOFF.md) · [01-01 범위 산출물](reports/tasks/01-01/scope-20261112/REPORT.md)
5. [01-02 런타임 비교·권고안](reports/tasks/01-02/runtime-comparison-20261112/REPORT.md)
6. [후속 실험](Console_Coordinator_Followup_Experiments_v1.3.3.md) · [구조도](Console_Coordinator_Architecture_v1.3.3.svg)

U3의 두 역할 Pod/각 컨테이너 하나와 SHA 조건부 병합·단일 진행 원장·한도 정지 분류는 유지한다.
01-02는 사용자 선택 전 계약 확정으로 기록하지 않는다. 실제 01-04 probe는 미실행이다.
선택 뒤에는 01-03 기준 동결을 거쳐 01-04에서 같은 계정 동시 두 세션과 자격/한도 분리부터 확인한다. 하나라도 불가/관측 불가이면 이후 작업을 멈추고 영향·대안을 보고한다.
01 스프린트가 끝나면 산출물과 probe 원본을 독립 검토에 넘긴다. 문서 개정·생성기 검산을 그 스프린트 완료로 세지 않는다.

재검산: `python3 scripts/validate_package.py`(jsonschema 필요), `python3 scripts/render_progress.py --check`.
명세/보고는 v0 contract_revision=4, 명세 9개는 revision 4의 draft다. 구 actor 문자열은 명시적 새 revision 변환 없이 수용하지 않는다.
루트의 V1_2/V1_3/V1_3_2 기록과 validation/v1.3.2_task_contracts.json, 01_plan_and_review/02_design_sources, 논문/코드/NPG CI는 역사적 원문이다. 현재 검산은 v1_3_3_validation.json이다.
