# 02-07 완료 판정·보고 대기·현황 조회

## 기준

`02-07.accept`, 검증 계획 T06·P13, `DEV_AGENT_HANDOFF.md` §3–7을 적용한다. 02-05 수용과 PR #121의 02-06 수용 증거를 선행 근거로 연결한다. [02-06 코드 PR](https://github.com/HeaInSeo/ieo_poc/pull/121), [병합 SHA main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/45166161403), [수용 원장 PR](https://github.com/HeaInSeo/ieo_poc/pull/122)을 대조했다.

## 첫 구현 범위

- 02-03 판정의 재사용 근거는 `crates/ieo-coord/src/api.rs`의 판정·outbox 트랜잭션과 `tests/coordinator.rs`의 롤백·재생·현황 조회 시험이다. PostgreSQL이 판정과 전송 상태의 정본이다.
- F3의 남은 경우는 개발 머리가 다음 작업으로 넘어간 뒤 앞 작업을 판정하는 것이다. 현재 머리가 다른 작업이면 원장에 남은 해당 작업의 최신 개발 배정·세대가 고정한 SHA를 후보로 대조한다. 정확한 현재 검토 계보, PR과 승인 SHA, 병합, 해당 병합의 main CI 및 실행 증거를 함께 확인한다. 오래된 승인이나 모호한 후보로 완료시키지 않는다. 실패·불일치 사례를 먼저 고정한 뒤 판정 경로에 연결한다.
- 에이전트 완료 claim 없이도 확인된 외부 증거로 판정·보고가 이뤄지는 경우와, claim만 있고 확인 증거가 없는 경우를 대조한다. 판정·사건·outbox 원자성과 pending/sent/unknown의 읽기 경계를 확인한다. 수신자가 확인되지 않으면 추측에 의한 재전송은 0건이다.
- PR 수준 changes_requested는 현재 SHA G3 입력과 별도인 외부 상태 불일치로 표시한다. 실제 수집은 기존 1b 범위를 따른다.
- G1의 acceptance·exit_condition·requirement_ids를 각각 제거하는 변이를 검출한다. 02-03의 M27은 G1 비교만 바꾸도록 좁혀 다시 확인한다.
- 진행 원장과 생성된 화면의 snapshot, 중단·재개 및 전달 상태는 기존 렌더러와 계약으로 검증한다.

격리 DB와 시험 수신기를 사용한다. 실제 제품 계정·모델 호출·GitHub 설정 변경은 별도 범위다.
