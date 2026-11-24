# 02-07 검증 체크포인트

기준은 `666f089`과 최신 main `744e0a2`다. 앞선 F3 포함 통합 83/83 근거를 재사용하고 현 변경에서 코디네이터 83건과 검토 수명주기 5건을 재실행했다.

| 수용 기준 및 이월 | 근거 |
| --- | --- |
| 보고 v0의 사건·현재 상태와 claim 분리 | `claims_are_kept_apart_from_the_completion_judgment`와 `verdict_uses_github_and_run_evidence_for_the_recorded_sha_only`. 완료 claim 없이 판정하며 다른 SHA의 증거는 채택하지 않는다. |
| 판정·사건·outbox 원자성 및 전달 | 기존 PostgreSQL 롤백 주입과 재생 시험을 재사용했다. 같은 outbox ID를 `unknown`으로 기록하면 pending에서 빠지고, 같은 unknown 요청은 상태를 바꾸지 않으며 이후 수신 확인은 같은 ID를 sent로 바꾼다. 이 시험은 API와 DB의 경계를 확인한다. |
| F3 | `completed_evidence_stays_with_its_task_after_development_moves_on`에서 새 배정으로 이전한 뒤 앞 작업의 SHA를 판정한다. |
| G1, S3, D1/M27 | acceptance, exit_condition, requirement_ids의 독립 변경마다 종속 배정을 거부한다. 좁힌 M27과 필드별 비교 제거 3종을 표적 시험으로 각각 검출했다. |
| N5 | 기존 G3 승인과 PR 수준 변경 요청을 별도로 읽어 외부 상태 불일치와 근거 ID를 보고에 표시한다. 판정 결과는 기존 G3와 수용 조건으로 계산한다. 실제 수집은 1b로 이월한다. |
| 갱신·중단/재개·혼합 snapshot | 기존 `DEV_AGENT_HANDOFF.md`와 원장·렌더러를 근거로 동일 snapshot와 SHA, 중단 복구 및 산출물 링크 제한 시험을 재실행했다. |

## 재개 조건

#122 `3ad8bf8`은 필수 CI 4/4를 통과했다. 독립 검토에서 supervisor를 통한 로그 URL 경로와 이미지 설정 변경 뒤 재현 결과 복원이 발견돼 02-06 최종 수용이 보류됐다. #123 `744e0a2`의 병합/main CI 증거와 02-07 로컬 변경을 보존한다. 02-06 수용 뒤 최신 main을 다시 대조하고 02-07 독립 검토를 요청한다.
