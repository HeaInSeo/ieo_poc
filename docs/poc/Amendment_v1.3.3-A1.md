# 계획 개정 v1.3.3-A1 (2026-11-12)

근거: [사용자 결정 원문](User_Decisions_2026-11-12_0103.md)(01-03 결정안 D1~D12 응답). 기계 판독 색인: [Amendment_v1.3.3-A1.json](Amendment_v1.3.3-A1.json).
**작업 수·실험 수는 바뀌지 않는다**: 주계획 73·실험 31, 후속 12·후보 5. 이 개정은 실행·검수 결과가 아니며 어떤 작업의 완료로 세지 않는다.

## 1. 바뀐 것

| # | 변경 | 위치 |
| --- | --- | --- |
| 1 | **01-03 단계별 동결.** 01-03은 01-04 첫 세 probe 조건, 작업별 초기 상한, 재개 probe, 최소 P12를 동결한다. 나머지 실험의 표본·예산·기대값은 담당·시점·차단 조건만 인수한다. 각 항목은 해당 첫 실행 전에 동결 증거가 있어야 시작한다 | 장부·명세 `01-03.accept`, 계획 끝의 A1 문단, [동결 대장](reports/tasks/01-03/staged-freeze-20261112/freeze_register.json) |
| 2 | **01-04 판정표 A1-GATE-01-04.** 자격 방식 A에서 공급자 한도 공유가 확인되면 지원 구성으로 기록하고 진행한다. 동시 실행 불가·역할 자격 격리 실패/미확인·한도 구조 관측 불가는 중단이다 | 장부·명세 `01-04.accept`, [런타임 계약 rev 5 §8](reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md), [probe_gate_table.json](reports/tasks/01-03/staged-freeze-20261112/probe_gate_table.json), READ_FIRST·HANDOFF·probe 확인표의 안내 문장 |
| 3 | 명세 개정: 수용 프로파일 `poc-acceptance-v1.3.3` **revision 5**를 추가했다(rev 4는 보존, `01-03.accept`·`01-04.accept` 두 항목만 다름). `POC-01`을 **revision 5**로 올리고 01-03·01-04가 rev 5 항목을 참조한다. 출처에 결정 원문을 추가했고 `limits.unresolved_reason`을 갱신했다. 한도 값은 여전히 `null`(draft)이다. 다른 스프린트는 revision 4 그대로다 | `Console_Coordinator_Sprints_v0.json` |
| 4 | 검증기: 프로파일·스프린트 revision을 이 개정 색인에서 읽는다. 전달 패키지 `SHA256SUMS.txt`의 각 파일이 원본과 같거나 이 색인에 개정 후 SHA-256과 함께 올라 있는지 확인한다 | `scripts/validate_package.py` |
| 5 | **D11.** 제3자 논문 PDF를 공개 저장소의 현재 파일 목록에서 삭제하고 원문 링크로 바꿨다. 공개 URL의 파일은 삭제한 사본과 SHA-256이 같다([대조 기록](reports/tasks/01-03/staged-freeze-20261112/evidence/d11-link-check.txt)). 커밋 `d92b414`부터 이 개정 직전까지의 이력에는 남아 있으며, 이력 재작성은 하지 않았다 | `07_supporting_research/References.md`, `SOURCES.json`, `.gitignore` |

## 2. 바뀌지 않은 것

- U3, SHA 조건부 병합, 단일 진행 원장, 한도 정지 분류, 자격 격리와 quota 구조의 별도 판정, 비밀 값 비기록.
- 01-02·01-07·01-08의 검수 기록. 런타임 계약은 §3·§8·§9만 rev 5로 바뀌었고 그 범위는 01-03 독립 검수에서 재검증한다.
- 과거 결정·검토 문서(`User_Decisions_2026-11-11.md`, `User_Review_Decisions_2026-11-12.md`, `01_plan_and_review/`, 이전 `V1_*` 색인). 이들은 당시 기록이므로 수정하지 않았다. 충돌하면 이 개정이 우선한다.

## 3. 읽는 법

계획·장부에서 “01-03에서 동결/고정한다”고 쓴 수치·표본·기대값(04-16의 P36 기준, 07-04 재측정, EARLY-01, P26 표본, 누적 한도 등)은 A1 이후 **해당 첫 실행 전 동결**로 읽는다. 동결 증거가 없으면 그 실행은 시작하지 않는다. 담당·시점·차단 조건은 동결 대장의 `later` 항목에 있다.
