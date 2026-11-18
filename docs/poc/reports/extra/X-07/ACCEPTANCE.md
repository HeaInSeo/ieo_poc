# X-07 수용 기록

Cilium 1.20.5의 격리 kind 4노드에서 실측 PASS를 확인했다. L4 대조군 30건은 한 Pod에 도착했다. L7은 TCP 연결 하나에서 1,500/1,500건이 정확히 균등 분산됐다. 변경 중 일반 호출 성공 사이의 최대 간격은 299 ms였다. 스트림은 순번 14부터 재개됐고, 보류 단항의 자동 재실행은 0건이었다. 명시 재요청 및 서버 처리 시작 기록은 각각 1건이었다. 시험 노드 4개를 제거했다. [기준](criteria.md) · [실측 보고와 원본](REPORT.md) · [격리 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44346080838)

| 근거 | 결과 |
| --- | --- |
| 독립 승인 | 실측 후보 `4468b9cf1e18b9eae1af1114d358d31de27ee53b`와 통합 후보 `a6d8955a6614eb0f979112f9044cc131c03e623e`를 각각 승인했다. 통합 후보에서 실측 산출물 20개가 바이트 단위로 일치했다. |
| 승인 head PR CI | `44408383281` 실행 1회차 4/4 성공. |
| 조건 병합 | PR #104에서 승인 SHA를 조건으로 `caad62d80b6b1255637a6dd2d71e1288d4f556ea`에 병합했다. 병합 트리와 승인 head 트리가 일치했다. |
| 병합 SHA main CI | `44409828333` 실행 1회차 4/4 성공. 완료 시각 `2026-11-19T07:23:03+09:00`. |

[PR #104](https://github.com/HeaInSeo/ieo_poc/pull/104) · [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44408383281/attempts/1) · [main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44409828333/attempts/1) · [PR CI 근거](evidence/pr104-pr-ci.json) · [main CI 근거](evidence/pr104-main-ci.json)

X-07은 추가 작업으로 검수 완료다. 주계획 18/73, 후속 0/12다. 수용 범위는 고정 조합의 시험용 읽기·스트림이다. 운영 변경과 실제 모델 호출은 0건이다.
