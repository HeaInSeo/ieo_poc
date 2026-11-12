# 검증기 `--write` 교정 (PR #7 자동 검토 P2 처분)

2026-11-12 KST · 계획 v1.3.3-A1 · 근거 [사용자 후속 지시](../../../User_Decisions_2026-11-12_followup.md) 1 · 작업 수에 넣지 않는다.

| 항목 | 내용 |
| --- | --- |
| 지적 | `chatgpt-codex-connector[bot]` PR #7 리뷰 댓글(814a5fb, `validate_package.py:101`) “Exclude the generated validation report from frozen checksums”, P2 |
| 유효성 | **유효.** main 4bd4eaf에서 `--write`가 전달본 `v1_3_3_validation.json`을 현재 결과로 덮어쓴다. 이 파일은 `SHA256SUMS.txt`에 있고 개정 색인에는 없어서, 다음 검산이 `AssertionError: v1_3_3_validation.json`로 실패한다([재현](evidence/write-repro.txt)) |
| 처분 | **교정.** 사용자가 우선 검토하라고 한 방식대로 했다. 과거 검산 원본(전달본)은 보존하고, 현재 결과는 별도 경로 `validation/v1.3.3-A1_current_validation.json`에 쓴다. 쓰려는 경로가 전달 패키지 파일이면 거부한다. 검증기 SHA가 바뀌므로 개정 색인 항목도 갱신했다 |
| 확인 | `--write` → 재검산 → `--write` → 재검산이 모두 rc=0이다. 두 번 쓴 결과 파일의 SHA-256이 같고, 전달본 SHA는 `SHA256SUMS.txt` 값 그대로다. `render_progress.py --check`도 통과한다([기록](evidence/write-fixed.txt)) |
| 한계 | 결과 파일은 그 시점의 사본이다. 원장이 바뀌면 진행 수치 등이 달라질 수 있으며, 정합 여부는 언제나 검증기를 다시 실행해 확인한다 |

같은 PR에서 사용자 후속 지시 4·5에 따른 P26 부족분 기록을 동결 대장 L04 `shortfall_record`에 추가했다(기준 6+6 유지, 01-04 차단 조건 아님).
