# 03-04 수용 기록

`POC-REQ-03-04 / 03-04.accept`, 계획 v1.3.3, `poc-acceptance-v1.3.3@4`. 범용·Go·Rust·`HeaInSeo/ieo_poc` 전용 카탈로그 자료의 P25를 수용한다. [고정 기대값](EXPECTATIONS.md) · [독립 대조 기록](REVIEW.md) · [검증 보고서](REPORT.md)

네 번째 독립 검토는 교정 3/3 뒤 `2aac476e7fad82f3b36fedb14dee1ffdd4eed508`를 승인했다. 오류 변형 20/20, 기존 Python 13/13, 입력 해시 16/16, 산출물 해시 26/26, 공통 격리 10/10을 대조했다. Go·Rust 실행 19건의 명령과 stdout/stderr 38개를 각각 원본과 대조했다. [오류 변형](structural-checks-r4.txt) · [실제 명령과 원본 출력](evidence/review1-rerun/actual-commands.json)

PR #102에서 정확히 승인된 head의 CI 4/4를 확인하고 그 SHA를 조건으로 `0d8d74b0360854839b74f49d95459764d9a14c80`에 병합했다. 이 병합 SHA의 main push CI 4/4를 확인했다. [PR #102](https://github.com/HeaInSeo/ieo_poc/pull/102) · [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44342564299) · [병합 커밋](https://github.com/HeaInSeo/ieo_poc/commit/0d8d74b0360854839b74f49d95459764d9a14c80) · [main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44344204377)

실험 원장에 넣을 P25 FAIL 2회와 고정 범위 PASS 1회는 별도 변경안으로 보존했다. 보존본의 SHA-256은 `ee8fb928f433b16c6fc9573029cd8fa69d6e02409a9e1e2a38dd550425e7e13f`다. 현행 `validate_package.py`가 `experiments[].runs`에 항목이 있으면 실패함을 격리 사본에서 재현했다. 작업과 이벤트 원장에 판정을 기록하고 실험 원장의 구조화된 회차와 전체 평가 종료는 검증기가 지원할 때까지 보류한다. [P25 회차 변경안](evidence/p25-run-delta.json) · [패키지 검사기](../../../../scripts/validate_package.py)

03-05는 위 검증 범위의 자료를 이미지와 CI 후보의 입력으로 사용한다. 이미지 재현·역할 권한 실행과 실제 CI 보호는 각각 P22·P23에서 검증한다.
