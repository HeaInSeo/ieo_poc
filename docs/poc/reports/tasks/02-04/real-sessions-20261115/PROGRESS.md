# 02-04 진행 기록 4 · 예산을 집행하는 호출자와 실제 세션 시험 계획 (수용 아님)

| 항목 | 기록 |
| --- | --- |
| 구분 | 주계획 02-04의 중간 산출물. 완료 수 11/73 변동 없음 |
| 산출물 | `crates/ieo-supervisor/src/caller.rs`(호출자), `src/bin/ieo-role-call.rs`, `run::charge_reported`, 역할 서비스 수정(검토 475b409 R1·R2·R4, PID 1 전용 거두기), 시험 `tests/caller.rs` 7건·`tests/serve.rs` 4건 추가·단위 시험 보강, 실제 회차 계획 [RUN_PLAN.md](RUN_PLAN.md)와 도구([run-real.sh](run-real.sh), [call.sh](call.sh), [steps.tsv](steps.tsv), [summarize.py](summarize.py), [prompts/](prompts/), [role/](role/), [fixture/](fixture/)), 컨테이너 예행 [dry-run.sh](dry-run.sh)와 [기록](evidence/dry-run.log), 작업 공간 시험 [cargo-test.log](evidence/cargo-test.log) |
| 모델 호출 | 0회(02-04 예산 사용 0). 실제 회차는 이 PR의 검토·병합 뒤 RUN_PLAN 순서로 한다 |

## 1. 호출자(`ieo-role-call`, 컨테이너 밖)

진행 기록 3 §1의 실제 호출 선행 조건(“예산을 집행하는 호출자가 종단 시험을 통과하기 전에는 실제 자격을 넣지 않는다”)을 채우는 부분이다.

**기록**: 예산의 모든 회차(두 역할)를 저장소 밖 작업 디렉터리의 `records.jsonl`에 남기고 닫힌 쪽으로 읽는다. 읽을 수 없는 줄이 있으면 오류다. 형식 `ieo-role-run/v0`은 `budget::spent`가 읽는 필드(`model_request`, `duration_ms`, `usage.run`)를 가진다.

**호출 전**:
- `stop_required` 기록이 하나라도 있으면 아무것도 시작하지 않는다. 같은 attempt_id도 다시 쓰지 않는다.
- `Limits::for_next`(예산 결정과 무추가결제 관측) 뒤 역할 몫(회차·시간·토큰)과 역할별 회당 창 상한(`max_window_seconds`)으로 줄인다.
  - 회당 창 상한이 필요한 까닭: 역할 서비스는 내준 창으로 시간을 세므로, 1,800초씩 내주면 개발 몫 9,600초에 5회만 들어간다.
- 계획 파일의 역할 몫 합이 승인 예산을 넘으면 계획을 받지 않는다.

**무추가결제 대조**(사용자 결정 2026-11-14 pr30 §2, 과거 stream만으로 확정하지 않음):
1. 가장 새로운 관측 stream(첫 회차는 같은 계정의 저장된 stream, 7일 이내)의 모든 `rate_limit_event`가 rejected·false·allowed여야 한다.
2. 역할 환경: 실행이 보고한 변수 이름에 `CLAUDE_CODE_OAUTH_TOKEN`이 있고 `ANTHROPIC_*`가 없어야 한다.
3. 이번 실행의 신호: guard가 첫 위반에서 멈추고, 호출자가 실행 뒤 stream을 다시 검사한다.

2·3 중 하나라도 어긋나면 `stop_required`다. 자격 파일이 OAuth 토큰 형식인지는 컨테이너 준비(run-real.sh)에서 값 출력 없이 확인한다.

**실행 뒤**:
- stream을 02-01 계측기로 읽는다. 재개 회차의 기준은 호출자 자신의 기록 중 그 세션의 가장 새로운 회차다. 서비스 체크포인트가 그 회차를 기준으로 재개했을 때만 쓴다(§11 항목 7 “실제로 재개한 직전 회차”). 아니면 미계측이다.
- `run::charge_reported`로 반영한다. guard가 멈춘 회차는 미계측이고, 관측 하한보다 작은 계측값도 미계측이다.
- 정리 실패·실행 오류·응답 분실(시작 뒤 끝을 알 수 없음)은 `stop_required`이고 사용량은 미계측(시간 없음)이다.
- 원본 stream은 작업 디렉터리에 두고 기록에는 경로·크기·SHA-256만 남긴다.

**시험**(`tests/caller.rs`, 가짜 Claude, 세션 누적이 회차마다 늘어남):
- 새 세션 → 재개 → Guardrail의 공유 예산 기록과 측정, 재개 기준
- 관측 없음·allowed·allowed_warning이면 보류
- 실행 중 위반 신호면 guard 정지, 미계측, `stop_required`, 다음 호출 거부
- 정리 실패·실행 오류면 `stop_required`
- 재개 기준 불일치면 미계측, 예산 정지
- 계획·기록 검사
- 실제 HTTP(서비스 `serve`와 `ureq`)와 어댑터 토큰 불일치 401

## 2. 역할 서비스 수정(검토 475b409 이월)

| # | 처리 |
| --- | --- |
| R1 | 잠금 탐지를 단계 검사보다 먼저 한다. 살아 있는 보유자가 있으면 `takeover` 체크포인트여도 `ALREADY_RUNNING`(pid·상태)이다. 시험 있음, 변이로 확인 |
| R2 | 자기 결과는 `Exited`일 때만 재개 기준으로 쓴다. guard 정지 뒤 늦게 쓴 결과는 이전 기준을 바꾸지 않는다. 시험 있음, 변이로 확인 |
| R3 | 수동 대조 절차를 [RUN_PLAN.md](RUN_PLAN.md) §4에 적었다 |
| R4 | 회차의 끝 줄이 여럿이면 모두 읽을 수 있어야 하고 가장 큰 값을 센다. 시험 있음 |
| R5 | 실제 역할 컨테이너는 `--stop-timeout 15`(run-real.sh, 예행에서 15초 확인) |
| R6 | 회차 디렉터리 수 계산, 시작 전 정지의 0/0 끝 줄, 몫 범위 검사 시험 |

**더 고친 것**: 고아 거두기(`waitpid(-1)`)는 PID 1일 때만 한다. 시험 프로세스 안의 `serve`가 다른 시험의 자식 종료 상태를 가져가 `cleanup_failed`가 간헐적으로 났다(호출자 시험 5회 중 2회). 고친 뒤 10회 연속 통과했다. 컨테이너 안에서는 PID 1이므로 동작이 같다.

## 3. 컨테이너 예행

[evidence/dry-run.log](evidence/dry-run.log): 실제 회차와 같은 스크립트로 7회를 했다. 가짜 Claude와 가짜 OAuth 형식 문자열을 썼고 모델 호출은 0회다.

| 확인 | 결과 |
| --- | --- |
| 기록 | 7건, `stop_required` 0건, 모두 측정됨 |
| 재개 기준 | D2~D5가 D1 세션을 직전 회차 기준으로 재개함 |
| §11 항목 7 | D1~D5 몫 합 = D5 세션 누적 |
| (b) D6 | 변조 탐지, 새 세션 |
| (a) D1 | 심은 항목 정리 |
| 환경 | OAuth 변수만 있음 |
| 비밀 | 요약·로그에 비밀 문자열 0건 |
| 정지 | 정지 제한 15초, 0.2~0.5초에 종료 |

## 4. 아직 하지 않은 것

- RUN_PLAN의 실제 7회(예비 5회). 역할별 자격 확인은 D1·G1에서 한다.
- 진행 기록 3 §3의 나머지: 코디네이터 컨테이너, 역할 전용 이미지.
- 개발 역할의 허용 목록은 03-02 몫이다. 시험은 읽기 전용 settings로 한다(RUN_PLAN §1).
- PR #48 검토 3479675의 보고 문구 항목(F1 회당 시간 열, F2 회차 수 추정 표시, F3 정지 제한, F4 X-02 위치)은 다음 정기 보고에 반영한다. 회차 수는 RUN_PLAN이 정한다(7회, 예비 5회).
