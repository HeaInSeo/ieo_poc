# X-06 수용 기록

실제 `ieo-coord`와 전용 PostgreSQL을 rootless Podman 기본 kind 3노드에서 실행했다. 사전 동결 기준 `e95bd8e`의 제품 HTTP/gRPC 대응 요청 157/157건과 요청별 DB 상태가 일치했다. 인증·역할 거부, 중복 배정과 멱등 재생, 옛 세대 보존과 현재 판정 제외, 커밋 전 롤백, 커밋 응답 유실 뒤 같은 키 확인과 명시 재요청 1회를 검증했다. 재요청 뒤 최종 DB 행은 1건이었다. [시험 기준](criteria.md) · [시험 보고와 원본](REPORT.md) · [kind 실측](https://github.com/HeaInSeo/ieo_poc/actions/runs/44321595437)

| 근거 | 확인 결과 |
| --- | --- |
| 독립 검토 | `42d467f`가 03-02 원장 불일치로 반려됐고, 반영한 `b25b24e05ca36d128093d328996d722409930489`가 승인됐다. X-06의 승인 산출물 15개는 이전 승인 head `7dc0571`과 동일했다. |
| 승인 head PR CI | 44325388603, 4/4 성공 |
| 조건 병합 | PR #100, 승인 head로 고정하여 `298053bba4156342da0ae008a381f12417c72f1b`에 병합했다. 병합 커밋의 트리는 승인 head와 동일하다. |
| 병합 SHA main CI | 실행 44326951060, `main` push에서 같은 병합 SHA의 4/4 성공. 완료 시각 `2026-11-18T23:48:44+09:00`. |

[PR #100 독립 검토·병합](https://github.com/HeaInSeo/ieo_poc/pull/100) · [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44325388603) · [main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44326951060) · [PR CI 원본](evidence/pr100-pr-ci.json) · [main CI 원본](evidence/pr100-main-ci.json)

X-06은 추가 작업으로 검수 완료다. 주계획 73과 후속 12의 완료 수는 유지한다. 이 제품 경로 결과는 04-03·04-04의 근거로 연결한다. 개별 수용 조건, 실제 물리 장애와 운영 처리량은 별도다.
