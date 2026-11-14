# 02-04 진행 기록 2 · 실행 중 예산 집행(run guard), 모델 호출 없음(수용 아님)

| 항목 | 기록 |
| --- | --- |
| 구분 | 주계획 02-04의 중간 산출물이다. [사용자 결정 2026-11-14 §3](../../../../User_Decisions_2026-11-14_pr30.md): 실제 모델 호출 전에 예산 집행 연결을 모델 없이 시험한다. 완료 수 11/73 변동 없음 |
| 산출물 | `crates/ieo-supervisor/src/run.rs`(`run::run`, `Limits::for_next`), `tests/run_guard.rs` 17건 |
| 검토 | 3103933 changes requested(High 3·Medium 4·Low 3) → 교정 1 → d2989f7 changes requested(Medium 2·Low 1) → 교정 2(§3) |
| 모델 호출 | 0회(가짜 `claude` 셸 스크립트만 사용) |

## 1. 사전 검사와 실행 중 정지의 구분

- **사전 검사**는 [`budget::next`](../supervisor-core-20261113/PROGRESS.md)가 한다. 회차를 시작할지 정한다: 회차 수, 시간, 토큰(지금까지 + 가장 큰 회차), 추가 과금 없음 관측, 미계측.
- **실행 중 정지**는 이번 `run::run`이 한다. 시작한 회차를 아래 조건에서 멈춘다.
- 사전 검사를 통과했다는 것만으로 실행 중 집행이 검증된 것은 아니다. 둘은 따로 시험했다.

| 연결(사용자 결정 §3) | 동작 | 시험 |
| --- | --- | --- |
| 회차별·전체 시간 상한에 따른 종료와 하위 프로세스 정리 | 회차는 자기 프로세스 그룹에서 돈다. 정지를 시작하는 시점은 min(회차당, 남은 전체 시간) − 유예(10초)다. 그래서 정지(SIGTERM, 유예 뒤 SIGKILL)가 상한 안에 끝난다. 끝난 뒤에는 정상 종료여도 그룹에 SIGKILL을 보낸다. 감시 중 오류가 나도 같은 정리를 한다. 그룹 신호가 실패하면 대표 프로세스를 직접 죽이고 `cleanup_failed`로 보고한다. 대표 프로세스가 끝난 뒤 남은 출력은 최대 2초만 읽고, 그 줄도 검사한다 | `the_per_run_time_limit_…`(자식까지 종료), `the_total_time_left_…`, `a_process_ignoring_sigterm_…`, `a_stop_with_sigterm_first_…`, `children_left_behind_by_a_normal_exit_…`, `the_drain_after_exit_is_bounded_…`, `lines_after_the_leader_exits_…`, `an_unwritable_evidence_path_…`(기동 전에 실패) |
| 관측 가능한 사용량이 상한에 도달했을 때 중단 | stream의 `assistant` 메시지마다 `usage`를 메시지 ID별 최댓값으로 합산한다(부모·자식 모두, ID 없는 줄은 합치지 않음). 청구 대상(input+output+cache_creation) 또는 cache_read가 남은 몫에 닿으면 그룹을 멈춘다. 관측값은 **하한**이다(§2). stdout이 닫히거나 읽을 수 없게 되었는데 프로세스가 계속 돌면 관측이 불가능하므로 멈춘다(`StreamLost`). UTF-8이 아닌 바이트는 손실 변환으로 읽고 버리지 않는다 | `observed_tokens_reaching_…`(남은 몫과 같으면 중단, 1 작으면 허용), `observed_cache_read_…`, `a_repeated_message_counts_once_…`, `usage_lines_without_an_id_…`, `a_closed_or_unreadable_stream_…` |
| 비허용 한도·추가 사용량 신호에 따른 중단 | 모든 `rate_limit_event`를 [`budget::stream_stop`]으로 검사한다. `overageStatus` ≠ `rejected`, `isUsingOverage` ≠ false, `status` ≠ `allowed`, 읽을 수 없는 사건 중 하나면 곧바로 멈춘다 | `an_extra_usage_or_limit_signal_stops_the_run`(5가지) |
| 사용량 누락·계측 실패 때 다음 호출 차단 | 회차 몫의 권위 있는 값은 02-01 계측기(`ieo-evidence`)가 `result` 사건으로 계산한다. guard가 멈춰 `result`가 없으면 계측기는 그 회차를 미계측으로 기록한다. 그러면 `budget::next`와 `Limits::for_next`가 다음 호출을 막는다. 실행 중 관측 합계로 대신하지 않는다. `Limits::for_next`는 `budget::next`가 `Start`일 때만 한도를 준다 | `a_guard_stop_leaves_the_run_unmeasured_…`(guard 정지 → 실제 계측기 `stream::parse` → 미계측 → 다음 호출 차단, 대조: 01-04 실제 stream은 계측됨 → 허용), `limits_for_the_next_run_come_from_the_pre_run_decision` |
| 정상 범위 실행 허용(양성 대조) | 상한 안의 실행은 끝까지 돌고 stdout이 그대로 복사된다 | `a_normal_run_is_allowed_and_measured` |

변이 19개를 모두 이름 있는 시험이 검출했다([mutants.txt](evidence/mutants.txt)). 시험의 감시 장치가 멈춤을 실패로 바꾼다. 반복 실행 20회에서 실패 0이었다.

## 2. 한계(관측 후 정지이며 공급자 측 제한이 아니다)

- **관측값은 하한이다.** `assistant` 줄의 `usage`는 메시지가 **시작될 때** 아는 값이다. 입력·캐시 값은 완전하다. 그러나 `output_tokens`는 시작 시점 값이다(02-01 fixture r1-new: stream 5, `result` 612; n4a: 관측 청구 대상 1,567, 부모 `result.usage` 1,878, 계측기가 쓰는 세션 `modelUsage` 2,584). CLI가 스스로 하는 모델 호출(압축·부가 호출)은 `assistant` 줄에 나오지 않는다(l3-compact: stream 0, `modelUsage`의 청구 대상 8,035·cache_read 26,400). 그래서 실행 중 정지는 실제 사용량이 남은 몫을 넘은 뒤에 일어날 수 있다. 넘친 양은 `result`가 있을 때만 알 수 있다.
- **최종값은 끝에서만 안다.** 회차 몫은 `result` 사건으로 계측기가 계산한다. guard가 회차를 멈추면 `result`가 없고, 그 회차는 미계측이 되어 같은 예산의 다음 호출을 막는다. 이 경우 실제 사용량은 확정되지 않는다. 그 사실을 보고하고, 사용자 결정 없이 다음 호출을 하지 않는다.
- 세션의 첫 `rate_limit_event`는 첫 요청 뒤에야 나온다(01-04 stream에서는 첫 `assistant` 다음, l3-compact에서는 `assistant` 줄 없이). 그래서 첫 요청 전에 추가 과금 없음을 확인하는 것은 실행 전 관측(`observation_check`)과 구독 자격·환경 대조의 몫이다. 실행 중 정지는 첫 요청 뒤부터 작동한다.
- 공급자(Anthropic) 쪽의 사용량 한도나 추가 사용량 차단은 계정 설정이다. 이 guard는 그것을 바꾸거나 보장하지 않는다.
- 프로세스 그룹을 떠난 후손(`setsid` 등)은 그룹 신호로 멈추지 않는다. 그런 후손이 stdout을 열어 두면 읽기 스레드를 기다리지 않고 떼어 낸다. 역할 컨테이너 안에서는 컨테이너 종료가 마지막 정리다(어댑터 단계).
- 그룹 신호는 `kill` 명령(`kill -s SIG -- -PGID`)으로 보낸다(작업 공간이 `unsafe`를 금지). `kill`을 실행하지 못하면 대표 프로세스만 직접 죽이고 `cleanup_failed`로 보고한다. 이 경로는 PATH를 바꾸는 시험이 다른 시험에 영향을 주므로 자동 시험하지 않았다.
- 시간 상한은 guard가 측정한 벽시계다. 정지는 상한 안에 끝나지만 폴링과 `kill` 호출로 수십 ms 넘을 수 있다(계측기의 회차 시간은 초 단위 올림으로 기록되어 보수적이다).
- 정지 결정 뒤에도 출력은 복사한다(유예 동안과 그 뒤 최대 2초). 그래서 종료하며 쓴 `result`가 남으면 계측기가 그 회차를 계측할 수 있다. 그 시간 뒤에 온 줄은 남지 않는다.

## 3. 검토 반영(3103933 → 교정 1)

| 지적 | 처분 |
| --- | --- |
| 1 High: 기동 뒤 오류 시 자식이 남음 | 증거 파일을 기동 전에 만든다. 감시 중 오류도 그룹 정지·최종 SIGKILL을 거친다 |
| 2 High: 종료 뒤 읽기가 무한 | 최대 2초로 묶었다. 그 줄도 검사한다. 종료 확인을 매 반복 한다(계속 쓰는 후손이 종료를 가리지 못함) |
| 3 High: `assistant` 사용량이 최종값이 아님 | §2 첫 두 항목에 하한·끝에서만 아는 값으로 적었다 |
| 4 Medium: UTF-8 아님·닫힌 stdout에 눈이 멂 | 바이트로 읽고 손실 변환한다. 실행 중 stdout이 닫히면 `StreamLost`로 멈춘다 |
| 5 Medium: `kill` 없음 → 무한 대기 | 대표 프로세스를 직접 죽인다. `cleanup_failed`로 보고한다 |
| 6 Medium: 시험 공백 | 시험과 변이를 추가했다(SIGTERM 먼저·유예, 닫힌 stream, 종료 뒤 줄, `timeout_is_total` 경계, 실제 계측기 연결) |
| 7 Medium: `Limits::for_next`가 `budget::next`와 어긋남 | `budget::next`를 감싸 `Start`일 때만 한도를 준다. 남은 몫 0이면 거부한다 |
| 8 Low: 유예로 시간 초과 | 정지 시작을 유예만큼 앞당겼다 |
| 9 Low: ID 없는 줄 합쳐짐·누락 0 | ID 없는 줄은 따로 센다. 필드 누락은 `incomplete_lines`로 센다 |
| 10 Low: `budget.rs` 문서 | 고쳤다 |
| 11 정보: 실패한 시험이 프로세스를 남김 | 시험 정리에서 pid 파일의 프로세스를 죽인다 |

## 4. 검토 반영(d2989f7 → 교정 2)

| 지적 | 처분 |
| --- | --- |
| 1 Medium: 정지 뒤 줄(`result` 포함)이 버려짐 | 정지 결정 뒤에도 유예 동안과 그 뒤 최대 2초 동안 줄을 복사한다(검사는 하지 않음). 정지시킨 줄과 그 뒤 `result`, TERM 처리기가 쓴 `result`가 남는지 시험한다 |
| 2 Medium: `kill` 실패를 성공으로 셈 | `kill`이 실패하면 `/proc/*/stat`에서 그 그룹의 살아 있는 프로세스를 찾는다. 남아 있으면 실패로 보고한다. `kill` 프로그램을 주입할 수 있게 했다(`run_with_kill`). 항상 실패하는 `kill`로 시험한다: 살아남은 자식은 `cleanup_failed`로 보고하고, 대표 프로세스는 직접 죽인다 |
| 3 Low: 실패한 시험이 프로세스를 남김 | 시험이 실패할 때만, 가짜 대표 프로세스의 그룹 전체와 기록된 자식을 죽인다 |
| 4 정보: 시험 공백 | 변이 R20~R24를 추가했다(E1·E2·E4·E5 해당). E3(닫힌 stream 대기가 마감을 2초 넘김)·E6(남은 몫 0 검사, 실제로는 동등)은 그대로 둔다 |
| 5·6 정보: 한계 문장의 정확도, 시간 정밀도 | 모듈 문서와 §2를 고쳤다 |
