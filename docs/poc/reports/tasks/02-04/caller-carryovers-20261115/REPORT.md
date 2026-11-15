# X-08 · 02-04 호출자 이월 교정(결정 항목 8)

| 항목 | 기록 |
| --- | --- |
| 작업 | X-08(개정 A6, 장부 §7). 02-04 수용 보고 §4 항목 8과 수용 기록 §4의 PR #58 정보성 지적 |
| 기준 커밋 | main 433ec4c에서 작성하고 fda0f2b 위로 다시 쌓았다. 교정 커밋 5ff5f49(코드·시험, 작성 때 4237abf), d59ff04(시험 보강, 작성 때 1feebb1). 시험 기록은 작성 때의 1feebb1에서 돌렸다(코드 같음) |
| 모델 호출 | 0회. 가짜 `claude`, 가짜 코디네이터(HTTP·gRPC), 가짜 `podman`과 상태 응답 서버만 썼다. 예비 회차·실제 중간 정지·컨테이너는 쓰지 않았다 |
| 예산 | 바뀌지 않았다. 기존 사용량·정정 이력·역할 몫의 의미는 그대로다. 실제 장부(`/opt/rust/ieo_poc_runs`)와 비밀 디렉터리는 읽지도 쓰지도 않았다 |
| 상태 | 구현과 시험 완료, **독립 검토 전**. 이 기록은 X-08 완료가 아니다. Q-3 하나는 사용자 결정이 필요하다(§4) |

## 1. 항목별 교정과 회귀 시험

시험 이름은 `crates/ieo-supervisor/tests/caller.rs`(caller), `tests/serve.rs`(serve), `tests/call_sh.rs`(call_sh), `crates/ieo-evidence/tests/collector.rs`(collector)의 것이다. “변이”는 교정을 하나씩 되돌린 코드로 그 시험을 돌린 결과다([evidence/mutants.txt](evidence/mutants.txt)).

| 항목 | 위치(현재 코드) | 교정 | 회귀 시험 | 결과 |
| --- | --- | --- | --- | --- |
| PR #51 71e408a **P1** 스트림에 없는 agent의 전사, 경로 문자가 든 id, 거절 뒤 남는 복사본 | `caller.rs` `remeasure`·`reevaluate`·`After::finish` | 전사 id는 그 회차 스트림의 `agentId`이고 영문·숫자·`-`·`_`만이어야 한다. 검사는 `remeasure` 첫머리에 있어 복사 전에 거절한다. `reevaluate`가 거절로 끝나면 이번에 만든 복사본과 디렉터리를 지운다. 회차 스트림 자체가 그런 id를 담으면 파일을 만들지 않고 자식을 미측정으로 둔다(`"refused": "unsafe agent id"`) | caller `transcripts_only_for_the_run_s_own_children_and_kept_privately`, `a_refused_correction_removes_the_copies_it_made`(자식 둘: 첫 복사 뒤 둘째가 거절되면 첫 복사본도 지움) | 통과, 변이 3개 잡힘 |
| P2 (C4) 다른 전사가 이미 복사 위치에 있음 | `caller.rs` `reevaluate` | 코드는 이미 거절했다. 시험이 없었다 | caller `a_correction_refuses_a_different_copy_and_reads_the_ledger_back`: 맞아떨어지는(다른 장부에서 정정이 성립함을 확인한) 다른 파일을 미리 두면 거절, 파일·장부 그대로 | 통과, 변이 잡힘 |
| P2 (C6) 추가 뒤 다시 읽기 | `caller.rs` `reevaluate` | 코드는 이미 다시 읽었다. 시험이 없었다 | 같은 시험: 마지막 줄의 줄바꿈이 없는 장부에 정정을 붙이면 두 줄이 합쳐지고, 다시 읽기가 곧바로 오류를 낸다 | 통과, 변이 잡힘 |
| P3 복사본 권한 | `caller.rs` `write_private` | 검토는 “조치 없음”이었지만 닫힌 쪽으로 바꿨다. 장부에 새로 만드는 디렉터리는 0700, 파일(스트림·자식 전사·정정 복사본)은 0600. 이미 있는 디렉터리·파일의 권한은 바꾸지 않는다 | caller `transcripts_only_for_…`(복사본 0600, 디렉터리 0700, 스트림 0600) | 통과, 변이 잡힘 |
| P4 장부가 절대 경로에 묶임 | `caller.rs` `moved_hint`(`observation`, `remeasure`에서 사용) | 이동을 지원하지는 않는다(기록 형식과 기존 기록·정정의 재측정 방식을 바꾸지 않음). 파일이 없을 때 거절 문구에 “장부 디렉터리를 옮기거나 이름을 바꾸면 거절된다, 되돌려 놓으라”를 붙인다 | caller `a_moved_ledger_is_refused_with_its_cause`(일반 회차와 정정 둘 다) | 통과, 변이 잡힘. §4 참고 |
| 12c9a21 **N6** `call.sh`가 정정 줄을 믿음 | `call.sh` R1, `caller::check`, `ieo-role-call check` | R1의 Python 검사를 없애고 읽기 전용 `ieo-role-call check`를 부른다. `check`는 호출과 같은 검사를 한다: 진행 중 표시, 장부 읽기(정정마다 다시 측정), 사람 확인 요청 기록, 모든 역할 서비스의 상태. 아무것도 쓰지 않는다 | caller `check_refuses_what_a_call_would_refuse`, call_sh `the_restart_step_runs_only_when_every_guard_holds`(성립하지 않는 정정 → 재시작 안 함) | 통과, 변이 2개 잡힘 |
| 608b367 **F8** 읽지 못한 `--child-transcript`가 “전사 없음”으로 | `ieo-evidence` `run.rs`, `stream.rs` | 읽기 결과를 그대로 넘기는 `parse_with_child_reads`를 더했다. 읽기 오류는 “자식 전사를 읽지 못함: <경로>: <오류>”로 미측정 사유에 남는다. 미측정(닫힌 쪽)은 그대로다 | collector `an_unreadable_transcript_is_reported_as_a_read_error` | 통과, 변이 잡힘 |
| PR #49 dc11612 **L-c3-2** 3초 잠금 대기 | `caller.rs` `LOCK_WAIT` | 검토의 두 선택지 중 “유지하고 적기”를 택했다. 이름 붙인 상수로 바꾸고, 실제 운용에는 필요 없고 같은 프로세스 안 시험용이라고 주석에 적었다 | caller `a_call_waits_briefly_for_a_released_lock`(1초 뒤 풀리는 잠금을 기다려 통과). 넘기면 거절은 기존 `a_second_caller_is_refused` | 통과, “대기 0초” 변이 잡힘 |
| **L-c3-3** 대상 역할의 서비스만 확인 | `caller.rs` `services_clear`(`call_with`·`check` 공용) | 호출 전에 계획의 **모든** 역할 서비스에 상태를 묻는다. 어느 역할이든 실행 중이거나, 마지막 시도나 체크포인트가 가리키는 시도가 장부에 없으면 거절한다 | caller `an_unknown_run_of_another_role_blocks_the_call`(검토의 재현: 장부에 없는 개발 D1 → G1 거절, 개발 서비스 재시작 뒤 체크포인트로도 거절), call_sh(Guardrail의 모르는 G9 → 개발 R1 거절) | 통과, 변이 2개 잡힘 |
| **L-c3-4** (a) `curl -s`, (b) 잠금 밖 검사 | `call.sh` R1 | (a) `curl -sf`, 재시작 전 상태가 비면 거절, 재시작 뒤 상태가 비면 종료 1. (b) `flock -w 5`로 `ledger/caller.lock`을 잡고 검사·재시작·재시작한 서비스의 응답까지 유지한다. 잠금 설명자(9)는 하위 명령에 넘기지 않는다(`9>&-`) | call_sh: 401 상태 → 재시작 안 함, 잠금을 다른 쪽이 잡음 → 대기 뒤 거절, 정상 재시작 중에는 잠금이 잡혀 있음 | 통과, 변이 2개 잡힘 |
| **마감 정지 시험**(dc11612 L-new-3) | `caller.rs` `After::finish` | 정지 동작은 그대로다. 호출자가 마감에 정지를 보냈으면 기록 `notes`에 “the caller sent a stop at its deadline”을 남긴다 | caller `the_caller_sends_a_stop_at_its_deadline`(창 7초 + 여유 0.3초 전에는 정지 없음, 지난 뒤 정확히 1번), `a_run_at_its_time_limit_is_stopped_and_recorded`(가짜 런타임이 60초 잠: 실행 감시가 2초에 `RunTimeout`으로 멈춤. 기록은 `stopped_by_guard`, 미측정, `stop_required`, 진행 중 표시 해제, 다음 호출 거절) | 통과, 변이 3개 잡힘 |
| PR #58 704330f **P-2** `tamper_recorded: false`에도 대기 변조 삭제 | `serve.rs` `write_checkpoint`(takeover) | 코디네이터가 있으면 응답이 `tamper_recorded: true`(또는 같은 takeover의 `duplicate`)일 때만 지운다. 그렇지 않으면 대기로 남아 다음 새 세션이 다시 보고한다. 코디네이터 없는 구성은 그대로다 | serve `a_tamper_the_coordinator_did_not_record_stays_pending`(HTTP·gRPC). 가짜 코디네이터가 실제처럼 새 세션 id마다 한 번만 기록하고 `tamper_recorded`를 답하게 고쳤다 | 통과, 변이 2개 잡힘 |
| ae65f0c **Q-1** 대기 사본 시험이 I-2 규칙을 고정하지 못함 | `tests/serve.rs` | 대기 사본을 “같은 seq, 코디네이터 최신 기록과 다른 바이트”로 바꿨다. 비교 뒤 거절되는 요청으로 사본 삭제와 `checkpoint.json` 불변을 바로 확인한다 | serve `a_detection_survives_refusals_and_a_refused_takeover_keeps_it_pending`(HTTP·gRPC) | 통과, 검토의 변이 M2 잡힘 |
| **Q-2** 손상된 대기 파일이 새 탐지를 막음 | `serve.rs` `launch` | “파일이 있음” 대신 “객체로 읽힘”을 본다. 읽히지 않는 대기 파일은 새 탐지로 덮어쓴다 | serve `an_older_pending_detection_is_kept_and_an_unreadable_one_is_replaced` | 통과, 변이 잡힘 |
| Q-2 오래된 대기 탐지가 앞섬, **“기존 대기 유지”·일치 분기 고정 시험** | `serve.rs` `launch`의 `(Some(p), false, _)` 갈래와 저장 조건 | 동작은 설계대로 둔다(먼저 보고되지 않은 탐지를 그대로 보고). 시험으로 고정했다 | 같은 시험: 첫 변조 → 거절 → 다른 변조 → 거절(대기 파일 바이트 그대로) → 새 세션의 `observed_digest`가 첫 탐지의 것 | 통과, 검토의 변이 M4·M5 잡힘 |
| **Q-3** `start`는 변조를 탐지하지 않음 | `serve.rs` `launch`(`op == "start"`) | **교정하지 않았다. 사용자 결정 필요(§4)** | 없음 | — |

## 2. 바꾼 파일과 동작 변화

- `crates/ieo-supervisor/src/caller.rs`: `services_clear`·`check`·`asks_for_a_person`·`write_private`·`safe_agent_id`·`stream_agents`·`moved_hint`·`LOCK_WAIT`. `remeasure`·`reevaluate`·`After::finish` 수정.
- `crates/ieo-supervisor/src/bin/ieo-role-call.rs`: `check` 하위 명령(종료 0 이상 없음, 2 거절, 1 오류).
- `crates/ieo-supervisor/src/serve.rs`: P-2, Q-2.
- `crates/ieo-evidence/src/stream.rs`·`run.rs`: F8.
- `docs/poc/reports/tasks/02-04/real-sessions-20261115/call.sh`: R1 단계(N6, L-c3-3, L-c3-4). Python 경로는 `IEO_POC_PY`로 바꿀 수 있게 했다(기본값은 그대로).
- 시험: `tests/caller.rs`(9개 추가, 가짜 `claude`의 둘째 자식 `AGENTTWO`), `tests/serve.rs`(2개 추가, 1개 강화, 가짜 코디네이터의 `tamper_recorded`), `tests/call_sh.rs`(새 파일), `ieo-evidence/tests/collector.rs`(1개).

**실제 회차 전에 알아 둘 동작 변화:**
- 모든 호출이 계획의 두 역할 서비스에 상태를 묻는다. 한 서비스라도 응답하지 않으면 호출은 시작 전에 오류(종료 1)로 끝난다. 이전에는 대상 역할만 물었다.
- `call.sh` R1에는 `bash`·`curl`·`flock`·Python과, `check` 하위 명령이 있는 `ieo-role-call`이 필요하다. 실제 작업 디렉터리의 `bin/ieo-role-call`은 병합본으로 바꿔야 한다. R1 거절 줄은 `restart refused: {"clear":false,"reason":…}` 꼴이다.
- 장부에 새로 생기는 파일은 0600이다. 이미 있는 실제 장부의 권한은 바꾸지 않는다.
- `dry-run.sh`는 컨테이너가 필요해 돌리지 않았다(이 작업의 금지 사항). 같은 R1 경로는 call_sh 시험이 가짜 `podman`으로 지난다.

## 3. 시험

- `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`: 통과.
- `cargo test --locked --all-targets`(시험 DB 포함): 346 통과, 0 실패, 0 무시. 기록은 [evidence/test.log](evidence/test.log)(커밋 1feebb1).
- 변이 24개: 24개 모두 잡혔다([evidence/mutants.txt](evidence/mutants.txt)). 교정을 하나씩 되돌리고 해당 시험만 돌린 뒤 원래 코드로 복구했다. 정리(cleanup) 변이는 처음에 이를 지나지 않는 시험으로 돌려 살아남았고, 1feebb1의 시험으로 다시 돌려 잡혔다.

## 4. 교정하지 않은 항목과 사용자 결정 요청

**Q-3 · `start`는 설정 변조를 탐지하지 않는다**(PR #58 검토 ae65f0c, 정보성).
- 근거: `serve.rs` `launch`에서 `op == "start"`는 `checkpoint::decide`를 부르지 않는다. 변조된 설정으로 `start`를 보내면 `tamper: null`로 시작하고 대기 파일도 만들지 않는다(검토 probe E). `start`는 매 회차 전 설정 정리를 거치는 명시적 새 세션이고, 런타임 계약 §13 항목 10은 탐지를 `reconnect`에 둔다. PR #58 이전부터의 동작이다.
- 교정하지 않은 까닭: `start`에 탐지를 더하면 계약 §13 항목 10과 코디네이터 변조 사건의 의미(“재개 거부로 생긴 새 세션”)가 바뀐다. X-08(호출자 이월) 범위 밖의 계약 변경이라 혼자 정하지 않았다.
- 선택지:
  - (가) 지금 동작을 계약의 알려진 한계로 기록한다(권장). `start`는 설정을 정리한 뒤 새 세션을 시작하므로 변조가 그 세션에 들어가지 않는다. 변조 사실의 보고만 빠진다.
  - (나) `start`도 체크포인트가 있으면 설정 digest를 비교하고, 다르면 `{old: 체크포인트 세션, new: start 세션}` 변조를 takeover에 실어 보고한다. 계약 §13 항목 10 개정과 독립 검토가 필요하다. 02-05 이후 작업으로 둘 수 있다.

**사용자 확인을 바라는 판단 세 가지**(교정은 했지만 범위를 정한 것):
- **Q-2 앞부분(오래된 대기 탐지가 앞섬):** 교정이 아니라 시험 고정으로 처리했다. 수용 기록 §4가 “기존 대기 유지를 고정하는 시험이 없다”고 적었으므로 그 동작을 유지하는 것으로 읽었다. 새 탐지를 우선하려면 결정이 필요하다. 어느 쪽이든 takeover 체크포인트는 현재 설정 digest를 따로 기록한다.
- **P4:** 원인을 알리는 거절로 처리했고, 장부를 옮겨도 읽히게 하지는 않았다. 02-04 장부 디렉터리는 예산이 열려 있는 동안 옮기지 않는다는 운용 규칙을 진행 기록에 적을지는 사용자가 정한다.
- **L-c3-2:** 잠금 대기를 없애지 않고 유지했다(시험 하네스 때문. 실제 운용에는 영향 없음).

**이번에 새로 본 잔여(교정하지 않음):** 대기 변조 파일이 손상돼 있고 새 탐지가 없으면(설정이 이미 되돌려진 경우) 지금처럼 “대기 없음”으로 읽혀 재개될 수 있다. Q-2 둘째 시나리오와 같은 뿌리다. 선택지: (가) 기록만 한다(파일은 원자적으로 쓰여 바깥 손상이 있어야 생김), (나) 읽히지 않는 대기 파일이 있으면 `EXTERNAL_RECONCILIATION_REQUIRED`로 거절한다.

## 5. 범위 밖으로 남긴 것

- 같은 검토들의 다른 지적(PR #49 L-c3-1·L-c3-5, PR #51 F9·F10)은 X-08 목록에 없어 다루지 않았다. PR #58 P-3의 4xx 부분은 PR #58에서 이미 시험이 있다.
- 실제 장부·실제 컨테이너에서는 확인하지 않았다. 다음 실제 호출 전에 실제 작업 디렉터리의 `ieo-role-call`을 병합본으로 바꾸는 일은 X-02 준비에서 한다.
