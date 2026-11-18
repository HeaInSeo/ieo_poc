# 09-01 구현 수용 근거

POC-09의 `09-01.accept` rev 4 구현 검수 근거다. 착수 전 원장에 02-01과 02-02 검수 완료가 기록돼 있었다. 결과와 도입 결정, 누적 예산, 자산 재사용 상태, 작업·진척표, 등록용 최종 스프린트 초안이 구현됐다. 합성 기대 출력과 CLI 대조는 구현 파일과 분리했다. [작업 보고](REPORT.md) · [기대 출력](fixtures/expected.json)

## 검토와 병합

| 근거 | 확인 결과 |
| --- | --- |
| 독립 검토 | `10a6878d2fb6fadc1f0d4a43bedca06583b27c82`은 최초 변환에서 교체된 원증거를 승인하는 문제로 반려됐다. 교정한 `9a6f48586451d7a6582879cc8dc9ba03fd0f692d`가 승인됐다. PR 본문의 독립 검토 기록에서 승인 SHA와 합성 CLI 22건의 독립 재실행을 확인했다. |
| 승인 head PR CI | `9a6f48586451d7a6582879cc8dc9ba03fd0f692d`의 PR CI 4/4 성공, 실행 44319728265 |
| 병합 | PR #98, 병합 SHA `fcf1060dd0307a6c2b1005cac1a07f25f99a582f` |
| 병합 SHA main CI | `main` push 실행 44321144168, 같은 병합 SHA에서 4/4 성공. GitHub 완료 기록 2026-11-18 23:17:14 KST |

[PR #98 독립 검토 기록](https://github.com/HeaInSeo/ieo_poc/pull/98) · [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44319728265) · [main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44321144168) · [main CI 조회 증거](evidence/pr98-main-ci.json)

교정 후 최초 변환에서도 승인 보고서가 지정한 경로별 SHA-256을 실제 바이트와 대조한다. 증거 교체·누락, 경로와 연결되지 않은 해시, 다른 경로의 동일 해시를 `INCONCLUSIVE/미판정`으로 처리했다. [교정 재시험 기록](evidence/pr98-review1-recheck.txt)

수용 범위는 09-01 구현이다. 09-02 독립 기대값과 P33 합성 판정은 별도 담당 범위이며, 09-05 실제 입력 연결과 제품 완료는 미검증이다.
