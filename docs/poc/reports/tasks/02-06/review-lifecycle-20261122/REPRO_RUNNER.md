# 02-06 재현 실행기와 수용 기준 대조

## 실행 계약

Guardrail supervisor는 기존 adapter 인증과 외부 요청 목록을 재사용한다. 선택 설정 `repro_central: {url, token_file}`에는 같은 repo의 Guardrail `agent:repro` 자격을 지정한다. `POST /v0/role/repro-requests`가 original request ID와 JSON SHA-256을 볼륨에 먼저 기록한다. 응답 유실 뒤에는 같은 ID·내용의 영수증만 조회하고 확인할 수 없으면 보류한다. 이 조회는 시작·재전송을 하지 않는다. 상태와 원본 로그는 같은 자격으로 조회한다. supervisor의 proxy는 HTTP를 사용하며 기존 코디네이터 gRPC 접수·상태 조회는 유지한다.

코디네이터 소유 별도 프로세스 `ieo-coord repro-worker --config FILE`이 repo별 대기열을 처리한다. 설정의 repo ID마다 `repository`(코디네이터가 관리하는 Git 객체 저장소), `bundles`(파일 이름이 SHA-256인 입력 묶음 디렉터리), `state`(로컬 작업 디렉터리), `image`(로컬 승인 이미지의 전체 sha256 ID)를 지정한다. `--once`는 격리 점검에 쓸 한 회차 옵션이다. 기본 `serve`가 실행기를 시작하지는 않는다.

선택한 요청은 현재 Guardrail 배정·세대·SHA를 확인한 뒤 DB에 한 번만 `running`과 컨테이너 ID를 기록한다. supervisor의 재현 어댑터는 대상 commit과 묶음 digest를 확인하고 한 번만 시작한다. rootless Podman에서 승인 이미지, 비루트 UID, 네트워크 없음, capability 제거, 읽기 전용 루트, 제한된 작업용 tmpfs를 사용한다. 컨테이너가 받는 호스트 마운트는 준비한 입력 폴더 하나로 제한한다. 요청 기한은 기본 60초·최대 300초다. 프로세스 16개, 파일 디스크립터 128개, 출력 파일 32 MiB, 프로세스별 주소 공간 128 MiB를 제한한다. 컨테이너 로그는 상한이 있는 stdout·stderr 원본 바이트로 보존한다.

종료를 관측하면 같은 ID로 exit code·이미지·시각·로그를 원장에 옮긴다. 명령의 0·비영 종료는 각각 `finished`의 exit code로 구분한다. 실행기 준비 실패는 `runner_failed`, 낡은 배정은 `rejected_stale_generation`이다. 시작·종료 여부를 증명하지 못한 ID는 `running`으로 남기며 새 실행을 보내지 않는다. 원장 업데이트가 실패해도 컨테이너 증거를 유지하여 재시작 뒤 같은 ID를 확인한다. 외부 관리자가 직접 격리 컨테이너를 삭제하기 전에 원장과 대조해야 한다.

## 02-06.accept 및 이월 대조

| 범위 | 코드·검증 근거 |
| --- | --- |
| 개발·Guardrail 세션/주체·배정 분리 | 기존 역할별 supervisor와 `assignment_heads`, repo·role scope 재사용. `review_transitions_use_current_assignments_and_preserve_late_reports`, `repro_requests_new_must_be_current_replays_are_not_requeued`. 실제 역할 Pod·worktree·자격의 교차 거부 시험은 04-07/P07에서 실행한다. |
| REVIEW_PENDING·반려·교정 push·새 head | `review_store.rs`와 `github_write.rs`. `github_correction_transfers_new_objects_and_reconciles_lost_push_and_merge`에서 원격에 없던 Git 객체 전송, 단계별 거부, 새 head 검토와 exact SHA CI를 대조한다. |
| 대체 상한·timeout·회수·늦은 결과 | lifecycle 단위 시험과 `timeout_fences_the_old_assignment_and_replacement_is_spent_once`, `timed_out_review_can_invalidate_and_reassign_a_new_head`. 실제 GitHub 권한 회수·head 경쟁은 04-07/P07, 보호 규칙·App 설정은 05-03/P23. |
| 조건부 병합·main CI·후속 | PR #121의 조건부 SHA 및 원격 이력 대조 재사용. `prerequisites_need_a_verified_accepted_verdict`와 exact 병합 SHA CI 전 판정 거부 시험을 대조한다. |
| N5 | 별도 `pr_review_state` 신뢰 증거로 현재 후보와 별도로 PR 수준 변경 요청을 차단한다. 이전 SHA의 요청도 해당 PR의 최신 정확한 증거가 해소해야 한다. 실제 수집·GitHub 주체 확인 F4는 1b, G3 결과를 유지한 외부 상태 표시 문구는 02-07이다. |
| 02-03 재현 실행기·02-05 단계 6 외부 요청 추적 이월 | `repro_worker.rs`에서 로컬 일회성 실행·비영 종료·준비 실패·저장 실패 후 복원·불명확 요청의 0회 재전송·낡은 세대를 검증한다. supervisor의 `reproduction_receipt_after_restart_only_reads_the_original_request`는 응답 유실과 재시작 뒤 GET만 검증한다. |

실제 GitHub/역할 Pod 이월과 별도 Claude → Codex 전환 실측 조건 대기를 유지한다. 실제 모델 요청과 운영 자격을 사용하지 않았다.
