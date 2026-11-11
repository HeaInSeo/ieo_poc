# 일정과 역할별 보고 표준 · v0 계약 revision 4

01-07/01-08 산출물 후보. 자동 발행·실제 사용자 평가 완료를 뜻하지 않는다.

| 구간 | 필수 내용 | 근거 수준 |
| --- | --- | --- |
| development_progress / 개발 | 단계·변경 요약·대상 SHA(미생성 시 이유)·내부 검사·막힌 이유 | claim |
| guardrail_review / Guardrail | 승인/반려/추가 확인·대상 SHA·발견 목록 | claim |
| coordinator_verdict / 코디네이터 | 증거 대조·영향 작업·다음 담당 | verified |
| decision_request / 코디네이터 | 질문 하나·선택지별 영향·권고와 이유·실제 시한/없음 이유·미응답 기본 조치 | verified |

actor는 {role,name}이다. 발행 자격의 principal/역할과 대조하며 이름 문자열을 신뢰하지 않는다.
Guardrail의 각 발견에는 위반 조건, 재현 명령/입력, 기대값/실측값, 심각도(P0~P3), 근거 확실성(reproduced/supported/unconfirmed), 원본 로그 참조가 필수다.
반려는 적어도 한 발견을, 승인은 해당 SHA의 필수 검사와 독립 대조를 요구한다. 빈 발견 목록만으로 자동 승인하지 않는다.
결정 질문은 공통 decision과 body가 같아야 하며 별개 질문을 두 곳에 넣지 않는다. 선택지·권고·시한·기본 조치도 같은 사건에서 일치해야 한다.

명세: schedule.deadline_state=set/none/undecided와 deadline_at/이유를 구분한다. 9개 초안의 기한은 undecided이며 기한 없음으로 바꾸지 않는다.
보고: 정상(on_track)·지연 위험(at_risk)·지연(late)을 표시한다. 기한 없음(no_deadline)·미확정/관측 부족(unassessed)을 별도 표시한다.
정상은 실제 기한·잔여 범위·현재 증거로 판단한다. 기한이 없거나 미확정이면 정상으로 단정하지 않는다.
남은 작업 수는 해당 sprint revision의 미검수 작업 수다. 범위 미확정이면 null+이유이며 0으로 바꾸지 않는다.
지연/위험이면 사유와 영향 작업을 적고 관측 시각·원 기준일을 보존한다. 지연 판정에 예상 완료 산정이 필수는 아니다.

예상 완료는 과거의 비교 가능한 실제 작업·소요/대기·잔여 물량을 근거로 earliest/latest 범위만 쓴다.
근거가 없으면 forecast.state=unestimated, completion_range=null, reason을 채운다. 다음 확인 시각을 예상 완료로 복사하지 않는다.
작업별 예상 범위도 측정 근거가 없으면 미산정이다. 임의의 0공수·동일 공수 가정은 허용하지 않는다.
시간 범위의 앞뒤 순서·관측일과 기한의 비교·출처 존재·인가는 JSON Schema 외 의미 검사에서 검증한다.

형식 변경: v1.3.2의 actor 문자열과 새 구조는 호환되지 않는다. 두 스키마에 contract_revision=4를 추가했다.
PoC 제품 발행 전 초안 변경이며 이미 접수된 구 revision/원본을 덮어쓰지 않는다. 구 입력은 명시적 변환 검토→새 revision/digest로만 전환한다.
구 형식을 몰래 수용하거나 Guardrail claim을 coordinator verified로 승격하는 마이그레이션을 금지한다.
사람용 출력은 공통 5부 구성(요약→사실/일정·영향→다음 행동→결정→근거)을 유지하고 원 구조의 역할·근거 수준·현재 SHA를 표시한다.

시각 검사기는 선택적 jsonschema 포맷 모듈 유무에 의존하지 않고 날짜의 유효성·시간대가 있는 RFC3339 형태를 명시적으로 대조한다. v0는 윤초의 60초 값을 받지 않는다. 스키마의 format 표기만으로 실제 시각 검증 성공을 선언하지 않는다.
