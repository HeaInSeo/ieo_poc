# 02-04 진행 기록 2 · 실행 중 예산 집행(run guard), 모델 호출 없음(수용 아님)

| 항목 | 기록 |
| --- | --- |
| 구분 | 주계획 02-04의 중간 산출물이다. [사용자 결정 2026-11-14 §3](../../../../User_Decisions_2026-11-14_pr30.md): 실제 모델 호출 전에 예산 집행 연결을 모델 없이 시험한다. 완료 수 11/73 변동 없음 |
| 산출물 | `crates/ieo-supervisor/src/run.rs`(`run::run`, `Limits::for_next`), `tests/run_guard.rs` 11건 |
| 모델 호출 | 0회(가짜 `claude` 셸 스크립트만 사용) |

## 1. 사전 검사와 실행 중 정지의 구분

- **사전 검사**는 [`budget::next`](../supervisor-core-20261113/PROGRESS.md)가 한다. 회차를 시작할지 정한다: 회차 수, 시간, 토큰(지금까지 + 가장 큰 회차), 추가 과금 없음 관측, 미계측.
- **실행 중 정지**는 이번 `run::run`이 한다. 시작한 회차를 아래 조건에서 멈춘다.
- 사전 검사를 통과했다는 것만으로 실행 중 집행이 검증된 것은 아니다. 둘은 따로 시험했다.

| 연결(사용자 결정 §3) | 동작 | 시험 |
| --- | --- | --- |
| 회차별·전체 시간 상한에 따른 종료와 하위 프로세스 정리 | 회차는 자기 프로세스 그룹에서 돈다. 마감은 min(회차당, 남은 전체 시간)이다. 마감에 그룹 전체에 SIGTERM을 보내고, 유예 뒤에도 남으면 SIGKILL을 보낸다. 끝난 뒤에는 정상 종료여도 그룹에 SIGKILL을 보낸다 | `the_per_run_time_limit_…`(자식까지 종료), `the_total_time_left_…`, `a_process_ignoring_sigterm_…`, `children_left_behind_by_a_normal_exit_…` |
| 관측 가능한 사용량이 상한에 도달했을 때 중단 | stream의 `assistant` 메시지마다 `usage`를 메시지 ID별 최댓값으로 합산한다(부모·자식 모두). 청구 대상(input+output+cache_creation) 또는 cache_read가 남은 몫에 닿으면 그룹을 멈춘다 | `observed_tokens_reaching_…`(남은 몫과 같으면 중단, 1 작으면 허용), `observed_cache_read_…`, `a_repeated_message_counts_once_…` |
| 비허용 한도·추가 사용량 신호에 따른 중단 | 모든 `rate_limit_event`를 [`budget::stream_stop`]으로 검사한다. `overageStatus` ≠ `rejected`, `isUsingOverage` ≠ false, `status` ≠ `allowed`, 읽을 수 없는 사건 중 하나면 곧바로 멈춘다 | `an_extra_usage_or_limit_signal_stops_the_run`(5가지) |
| 사용량 누락·계측 실패 때 다음 호출 차단 | 회차 몫의 권위 있는 값은 02-01 계측기(`ieo-evidence`)가 `result` 사건으로 계산한다. 정지로 `result`가 없으면 그 회차는 미계측이다. 그러면 `budget::next`가 다음 호출을 막는다. 실행 중 관측 합계로 대신하지 않는다 | `a_run_without_measurement_blocks_the_next_call` |
| 정상 범위 실행 허용(양성 대조) | 상한 안의 실행은 끝까지 돌고 stdout이 그대로 복사된다 | `a_normal_run_is_allowed_and_measured` |

변이 8개(최종 그룹 정리 제거, SIGKILL 제거, 추가 사용량 신호 무시, 토큰·cache_read 상한 제거, 메시지 합산 오류, 마감 제거, 전체 시간 보고 오류)를 모두 이름 있는 시험이 검출했다([mutants.txt](evidence/mutants.txt)). 시험의 감시 장치가 멈춤을 실패로 바꾼다. 반복 실행 20회에서 실패 0이었다.

## 2. 한계(관측 후 정지이며 공급자 측 제한이 아니다)

- 사용량은 그 사용량을 만든 모델 요청이 **끝난 뒤** stream에 나타난다. 그래서 정지는 남은 몫을 넘긴 요청이 끝난 뒤에 일어나고, 그 요청의 토큰은 이미 쓰였다. 넘친 양은 보고하고 다음 호출을 막는다. 회차 사이 사전 검사의 예비분(가장 큰 회차)이 이 넘침을 줄이지만 없애지는 못한다.
- 세션의 첫 `rate_limit_event`는 첫 요청 뒤에야 나온다(01-04 stream에서 첫 `assistant` 다음). 그래서 첫 요청 전에 추가 과금 없음을 확인하는 것은 실행 전 관측(`observation_check`)과 구독 자격·환경 대조의 몫이다. 실행 중 정지는 첫 요청 뒤부터 작동한다.
- 공급자(Anthropic) 쪽의 사용량 한도나 추가 사용량 차단은 계정 설정이다. 이 guard는 그것을 바꾸거나 보장하지 않는다.
- 프로세스 그룹을 떠난 후손(`setsid` 등)은 그룹 신호로 멈추지 않는다. 그런 후손이 stdout을 열어 두면 읽기 스레드를 기다리지 않고 떼어 낸다. 역할 컨테이너 안에서는 컨테이너 종료가 마지막 정리다(어댑터 단계).
- 시간 상한은 guard가 측정한 벽시계다. 계측기의 회차 시간은 별도로 기록된다.
