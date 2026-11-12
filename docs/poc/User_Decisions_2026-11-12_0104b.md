# 사용자 결정·지시 2026-11-12 · PR #14 병합 방식과 GitHub 시험 저장소 (원문)

## 1. PR #14 base 갱신 처리 (F04 상한 도달 뒤 branch protection의 최신 base 요건)

맥락: PR #14는 01-04의 4번째(상한) 검토에서 head a3d01ce로 승인됐다. 그 뒤 main 보호의 최신 base 요건 때문에 병합이 거부됐다. 개발 에이전트는 세 선택지를 제시했다. ① base 갱신 후 차이 대조 ② 5번째 검토 허용 ③ 보류. 사용자 원문은 다음과 같다.

> 이번 건에 한해 1번으로 진행해. 독립 검토자가 추가 변경이 PR #13과 동일함을 확인하고, 새 head에서 필수 CI가 모두 통과한 뒤 그 SHA를 조건으로 병합해. 다른 변경이나 충돌이 발견되면 중단하고, 이번 처리 근거는 원장에 남겨줘.

## 2. GitHub 시험 저장소

> GitHub 시험용 저장소는 이미 생성했으니 이 저장소를 사용해줘.
>
> - 저장소: https://github.com/HeaInSeo/ieo_poc_test
> - 원격 서버 경로: `/opt/rust/src/github.com/HeaInSeo/ieo_poc_test`
> - 공개 저장소이며 합성 시험 파일만 등록했음.
> - 기본 GitHub Actions CI 성공: https://github.com/HeaInSeo/ieo_poc_test/actions/runs/43180843445
> - `main` 보호: PR 필수, 최신 base 반영과 필수 CI 요구, 관리자 포함 적용, force push·브랜치 삭제 금지.
> - Actions 기본 토큰은 읽기 전용이며 PR 작성·승인 권한은 꺼져 있음.
>
> 역할별 GitHub App 자격과 독립 판정 체크는 아직 연결하지 않았어. 현재 CI는 저장소 연결 확인용이므로 P07/P23의 독립 판정이 구성됐다고 간주하지 말아줘.
>
> 현재 설정을 다시 조회한 뒤 시험 환경 자산 목록과 원장에 기록하고, 05-03/P07 전에 필요한 자격·독립 판정 연결 작업에 연계해줘. 저장소 준비는 PoC 작업·실험 완료 수에 포함하지 않아.
