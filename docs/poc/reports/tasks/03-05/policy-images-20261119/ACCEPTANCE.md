# 03-05 수용 기록

`POC-REQ-03-05 / 03-05.accept / poc-acceptance-v1.3.3@4`. 03-04의 승인 자료와 02-04의 수용 기록을 선행 근거로 사용했다. 통합·Go·Rust 이미지의 digest, 공통 검증 실행기, 두 스택의 프로파일 및 CI 후보를 T04 산출물로 수용한다. [정책 이미지 보고서](REPORT.md) · [빌드 원본](evidence/build.log) · [격리 13/13](evidence/results.txt)

독립 검토를 승인받은 `6e39c42c8b2e4afc33e63881467827d875399988`에서 PR CI 4/4를 확인했다. 승인 SHA를 병합 조건으로 지정해 `ed15dabafc2c2cb55eb5ac7bbb3afa09a2263dae`에 병합했고, 같은 SHA의 main push 재실행 4/4를 확인했다. [PR #103](https://github.com/HeaInSeo/ieo_poc/pull/103) · [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44403435337) · [병합 커밋](https://github.com/HeaInSeo/ieo_poc/commit/ed15dabafc2c2cb55eb5ac7bbb3afa09a2263dae) · [main CI 재실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44404757078/attempts/2)

첫 main CI는 기존 정산 시험의 오류 문구 단정이 실패해 3/4였다. 같은 소스의 격리 재현에서 자식이 잠금 파일을 exec 전까지 상속해 잠금 단계에서 먼저 거부되는 경우를 확인했다. 잠금과 진행 중 표식 모두 기록을 보존했다. [첫 main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44404757078/attempts/1) · [원인 조사](MAIN_CI_DIAGNOSIS.md)

03-06은 02-02의 검수 완료 기록과 위 후보를 사용해 P22 기본 실행 범위를 검증한다. 역할별 권한·컨테이너 재생성과 입력 누락 거부가 이 후속 범위다. 실제 CI 보호와 운영 채택은 각각 해당 검증 절차에서 판단한다. [02-02 평가 기준](../../02-02/eval-20261112/REPORT.md)
