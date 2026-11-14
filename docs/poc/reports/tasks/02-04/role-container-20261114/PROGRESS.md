# 02-04 진행 기록 3 · 역할 컨테이너의 supervisor(PID 1)를 가짜 Claude로 검증 (수용 아님)

| 항목 | 기록 |
| --- | --- |
| 구분 | 주계획 02-04의 중간 산출물이다. 완료 수 11/73 변동 없음 |
| 산출물 | `crates/ieo-supervisor/src/serve.rs`(역할 서비스), `src/bin/ieo-supervisor.rs`(`serve --config --listen`), `run::run_with`(stdin 프롬프트·외부 정지), 시험 `tests/serve.rs` 6건, 컨테이너 시험 스크립트 [run-fake.sh](run-fake.sh)와 [실행 기록](evidence/run-fake.log) |
| 모델 호출 | 0회. Claude 대신 가짜 셸 스크립트를 쓴다 |
| 재개 지점 | 2026-11-14 사용자 지시 “작업을 이어서 진행”: main d3e28df, 열린 PR은 #29(별도 DF-02)뿐, 실행 중 Claude·supervisor 없음, 이전 실제 호출 0회 |

## 1. 역할 서비스(어댑터 start/reconnect/stop/status, 런타임 계약 §7)

- 역할 컨테이너의 PID 1로 돈다. 작은 HTTP 끝점이다. 인증은 `Authorization: Bearer`이고, supervisor는 검증용 해시(`/run/ieo/cred/adapter.sha256`)만 가진다(§2.1). 해시 파일이 없으면 모두 거부한다.
- `start`: 새 session_id로 시작한다. `reconnect`: 체크포인트 결정(§5)에 따라 처리한다.
  - 같으면 `--resume <session_id>`로 재개한다.
  - 변조면 정리 뒤 새 session_id로 시작하고 `config_tamper_detected`를 보고한다.
  - 실행 중이면 시작하지 않고 409 `ALREADY_RUNNING`과 상태를 돌려준다(§5-5).
  - 낡은 세대·기록 불일치는 409로 거부한다.
- 매 실행 전에 `CLAUDE_CONFIG_DIR`을 정리하고 템플릿을 설치한다(§5-6). 자격 파일 삭제는 기록한다.
- 고정 인자(§4.1)와 허용 목록 환경(§4.3)으로 실행한다. 프롬프트는 stream-json으로 stdin에 넣는다. 실행 중 예산 집행은 `run::run_with`가 맡는다(진행 기록 2).
- 예산은 정하지 않는다. 호출자가 회차마다 한도(`Limits::for_next`)를 넘기고, 끝난 뒤 `run::charge`로 반영한다.
- 실행 뒤 §5-1 체크포인트를 원자적으로 쓴다(임시 파일 → 이름 바꾸기). 세션 누적값(`modelUsage`·`total_cost_usd`)도 남긴다(§11 항목 7의 다음 재개 기준).
- PID 1의 일:
  - SIGTERM/SIGINT를 받으면 실행 중 회차를 guard로 멈추고 끝낸다.
  - 실행이 없을 때만, 새 실행을 막는 같은 잠금 아래에서 고아 프로세스를 거둔다(`waitpid(-1, WNOHANG)`, `nix`). 실행 중인 자식의 종료 상태를 빼앗지 않는다.
  - 작업 공간이 `unsafe`를 금지하므로 신호·`waitpid`는 안전 래퍼(`signal-hook`, `nix`)를 쓴다.
- 자격 값은 응답·상태·로그 어디에도 나오지 않는다(시험으로 확인).

## 2. 컨테이너 시험(가짜 Claude, rootless podman)

[실행 기록](evidence/run-fake.log). 이미지는 01-04와 같은 `golang:1.26.9-bookworm`이고, 프로그램 디렉터리를 읽기 전용으로 마운트한다.

| 확인 | 결과 |
| --- | --- |
| U3 배치 | 개발 PID 1 = ieo-supervisor, UID 20001. Guardrail PID 1 = ieo-supervisor, UID 20002. 쓰기 가능한 것은 `/state`뿐이고 `/etc/ieo/role`·`/run/ieo/cred`는 읽기 전용이다. 두 역할이 공유하는 호스트 경로는 0개다(읽기 전용 프로그램 디렉터리 제외). `--cap-drop ALL`, `no-new-privileges` |
| 어댑터 인증 | 토큰 없음 401 |
| (a) 첫 기동 전 정리 | 심어 둔 CLAUDE.md·skill·`.credentials.json`이 지워지고, 프로세스가 본 설정 디렉터리는 `settings.json`뿐이다. 프롬프트는 stdin으로 도착했다 |
| (c) 변경 없는 재연결 | 같은 session_id로 `--resume` |
| 영속 볼륨 복원 | 컨테이너 재시작 뒤 저장된 체크포인트로 같은 세션을 재개했다 |
| (b) 변조 탐지 | 체크포인트 뒤 skill을 심으면 새 session_id로 시작하고 `config_tamper_detected`를 보고한다. 심은 skill은 정리된다 |
| 세션당 단일 실행 | 실행 중 두 번째 reconnect는 409 `ALREADY_RUNNING`이고 아무것도 시작하지 않는다 |
| stop | guard 경로로 정지(`Requested`). 남은 프로세스(좀비 포함) 0개, `cleanup_failed` false |
| Guardrail 인자 | `Edit(//state/guardrail-scratch/**)` 포함, `Write(` 없음. 환경 변수 이름 5개(허용 목록) |
| 정상 종료 | SIGTERM으로 `podman stop` 0.4~0.5초, 종료 코드 0 |

이 시험에서 찾아 고친 문제:
1. 컨테이너의 procps-ng 4.0.2 `kill`은 `kill -s SIG -- -PGID`를 거부한다. 그래서 그룹 정리가 실패했고 `cleanup_failed`로 보고됐다. 호스트(util-linux)와 CI는 받아들였다. 두 구현이 모두 받는 `kill -SIG -- -PGID`로 바꿨다.
2. PID 1이 SIGTERM을 처리하지 않아 `podman stop`이 10초 뒤 SIGKILL로 끝났다. 신호 처리를 넣었다.
3. 고아 프로세스가 PID 1 아래에 남았다. 거두기를 넣었다.

## 3. 아직 하지 않은 것(02-04 수용 전)

- **실제 Claude 시험(모델 호출)**:
  - 역할별 자격 유효성(첫 호출).
  - §10 항목 1: Guardrail 규칙으로 scratch 쓰기 허용·그 밖 거부.
  - 항목 2: CLAUDE.md 공급(`append_system_prompt_file`)과 repo hook 미적용.
  - 항목 3 (a)·(b)·(c): 실제 CLI 동작. (c)는 도구 없음·허용 도구·자식·`/compact`.
  - §11 항목 7: 새 세션 → 재개 → `/compact` 재개의 회차 몫 합 = 마지막 세션 누적.
  - 모두 02-04 예산(12회) 안에서 한다.
- **호출자(예산 기록·회차 몫 반영)**: 증거 기록을 읽어 `Limits::for_next`로 한도를 정하고, 끝난 뒤 02-01 계측기와 `run::charge`로 기록을 남기는 실행기. 진행 기록 2 §3의 이월 항목(정리 실패·오류는 정지·보고, charge 값을 기록에 쓰는지 종단 시험)을 여기서 처리한다.
- **코디네이터 컨테이너(UID 20000, PID 1)**: 지금 코디네이터는 호스트 프로세스다.
- **역할 전용 이미지**: 지금은 범용 이미지에 프로그램을 마운트한다.
