# X-08 · 02-04 호출자 이월 교정(결정 항목 8)

| 항목 | 기록 |
| --- | --- |
| 작업 | X-08(개정 A6, 장부 §7). 02-04 수용 보고 §4 항목 8과 수용 기록 §4의 PR #58 정보성 지적 |
| 기준 커밋 | main 433ec4c에서 작성하고 fda0f2b 위로 다시 쌓았다. 교정 커밋 5ff5f49(코드·시험, 작성 때 4237abf), d59ff04(시험 보강, 작성 때 1feebb1). 시험 기록은 작성 때의 1feebb1에서 돌렸다(코드 같음). PR #67로 병합(main 6e5e502). 검토 681c1d3 후속 교정은 9b63ce8(§6). 사용자 결정 반영은 18d5d9e(§4) |
| 모델 호출 | 0회. 가짜 `claude`, 가짜 코디네이터(HTTP·gRPC), 가짜 `podman`과 상태 응답 서버만 썼다. 예비 회차·실제 중간 정지·컨테이너는 쓰지 않았다 |
| 예산 | 바뀌지 않았다. 기존 사용량·정정 이력·역할 몫의 의미는 그대로다. 실제 장부(`/opt/rust/ieo_poc_runs`)와 비밀 디렉터리는 읽지도 쓰지도 않았다 |
| 상태 | **검수 완료**(§8). PR #67(6e5e502)·#68(e0a7f8a)·#70(1c21177) 병합, 수용 기록 PR #71(386a223, 검토 [b92a398 승인](reviews/review-pr71-b92a398.md)). 남은 사용자 결정은 없다 |

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
| PR #58 704330f **P-2** `tamper_recorded: false`에도 대기 변조 삭제 | `serve.rs` `write_checkpoint`(takeover 응답), `local_checkpoint`(대기 사본 채택) | 처음 교정(PR #67)은 직접 응답 경로만 고쳤다(`tamper_recorded: true` 또는 `duplicate`일 때만 삭제). 나머지 두 경로(중복 응답, 응답을 잃은 takeover의 채택)는 검토 681c1d3 L-1의 지적으로 9b63ce8에서 고쳤다: 코디네이터가 중복 응답과 `latest`에 “그 체크포인트의 보고가 변조 사건을 기록했는가”(`tamper_recorded`)를 답하고, supervisor는 세 경로 모두 그것이 `true`일 때만 지운다(§6) | serve `a_tamper_the_coordinator_did_not_record_stays_pending`, `an_unrecorded_tamper_stays_pending_after_a_duplicate_or_an_adopted_takeover`(HTTP·gRPC). 가짜 코디네이터가 실제처럼 새 세션 id마다 한 번만 기록하고 `tamper_recorded`를 답한다 | 통과, 변이 4개 잡힘 |
| ae65f0c **Q-1** 대기 사본 시험이 I-2 규칙을 고정하지 못함 | `tests/serve.rs` | 대기 사본을 “같은 seq, 코디네이터 최신 기록과 다른 바이트”로 바꿨다. 비교 뒤 거절되는 요청으로 사본 삭제와 `checkpoint.json` 불변을 바로 확인한다 | serve `a_detection_survives_refusals_and_a_refused_takeover_keeps_it_pending`(HTTP·gRPC) | 통과, 검토의 변이 M2 잡힘 |
| **Q-2** 손상된 대기 파일이 새 탐지를 막음 | `serve.rs` `launch` | “파일이 있음” 대신 “객체로 읽힘”을 본다. 읽히지 않는 대기 파일은 새 탐지로 덮어쓴다 | serve `an_older_pending_detection_is_kept_and_an_unreadable_one_is_replaced` | 통과, 변이 잡힘 |
| Q-2 오래된 대기 탐지가 앞섬, **“기존 대기 유지”·일치 분기 고정 시험** | `serve.rs` `launch`의 `(Some(p), false, _)` 갈래와 저장 조건 | 동작은 설계대로 둔다(먼저 보고되지 않은 탐지를 그대로 보고). 시험으로 고정했다 | 같은 시험: 첫 변조 → 거절 → 다른 변조 → 거절(대기 파일 바이트 그대로) → 새 세션의 `observed_digest`가 첫 탐지의 것 | 통과, 검토의 변이 M4·M5 잡힘 |
| **Q-3** `start`는 변조를 탐지하지 않음 | `serve.rs` `launch`(`op == "start"`) | 코드 교정 없음. 사용자 결정(가)에 따라 런타임 계약 §13 항목 10에 알려진 한계로 기록했다(§4) | 없음 | 결정 반영 |

## 2. 바꾼 파일과 동작 변화

- `crates/ieo-supervisor/src/caller.rs`: `services_clear`·`check`·`asks_for_a_person`·`write_private`·`safe_agent_id`·`stream_agents`·`moved_hint`·`LOCK_WAIT`. `remeasure`·`reevaluate`·`After::finish` 수정.
- `crates/ieo-supervisor/src/bin/ieo-role-call.rs`: `check` 하위 명령(종료 0 이상 없음, 2 거절, 1 오류).
- `crates/ieo-supervisor/src/serve.rs`: P-2, Q-2(후속 L-1·I-4는 §6).
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
- `cargo test --locked --all-targets`(시험 DB 포함): 346 통과, 0 실패, 0 무시(커밋 1feebb1, PR #67). [evidence/test.log](evidence/test.log)는 그 뒤 후속(§6, 9b63ce8, 349 통과)과 사용자 결정 반영(§4, 350 통과) 때 새 실행으로 바꿨다. 지금 파일은 §4의 실행이다. 이전 기록은 git 이력(PR #67의 이 파일)에 있다. 후속 검토는 [review-pr68-3724bc9.md](review-pr68-3724bc9.md)(제안 문구 안의 상대 링크가 풀리도록 보고와 같은 폴더에 보관).
- 변이 24개: 24개 모두 잡혔다([evidence/mutants.txt](evidence/mutants.txt)). 교정을 하나씩 되돌리고 해당 시험만 돌린 뒤 원래 코드로 복구했다. 정리(cleanup) 변이는 처음에 이를 지나지 않는 시험으로 돌려 살아남았고, 1feebb1의 시험으로 다시 돌려 잡혔다.

## 4. 사용자 결정 반영

근거: [사용자 결정 2026-11-15 · 02-05 착수 전 규칙·실제 회차 예산, X-08 남은 항목](../../../../User_Decisions_2026-11-15_0205-x08.md). 반영 커밋 18d5d9e. 시험 350 통과, 0 실패, 0 무시([evidence/test.log](evidence/test.log), 커밋 18d5d9e).

| 항목 | 사용자 결정 | 한 일 |
| --- | --- | --- |
| 새 탐지 없이 손상됐거나 객체가 아닌 대기 변조 파일(Q-2 둘째 시나리오, 검토 681c1d3 I-4) | **재개 거절** | `serve.rs` `launch`: 대기 파일을 읽을 수 없거나, 파싱되지 않거나, JSON 객체가 아니면 start·reconnect를 409 `EXTERNAL_RECONCILIATION_REQUIRED`로 거절하고 파일을 지우지 않는다. 새 탐지가 있으면 지금처럼 그 파일을 새 탐지로 덮어쓰고 새 세션과 함께 보고한다(보고되지 않은 탐지를 잃지 않고, 더 새로운 정보로 바뀌므로 그대로 두었다). start도 거절한다: start는 대기 탐지를 실어 보고하는 길이기도 해서, 읽지 못한 대기 파일을 둔 채 시작하면 그 탐지가 보고되지 않는다. 시험 serve `an_unreadable_pending_file_stops_until_a_person_reconciles_it`(HTTP·gRPC; 손상 바이트·배열·문자열, reconnect·start 거절과 파일 유지, 새 탐지로 덮어쓴 뒤 새 세션). 이전 동작(“대기 없음”으로 읽음)으로 되돌리면 두 시험 모두 실패한다. 9b63ce8의 `a_pending_file_that_is_not_an_object_does_not_panic`은 이 시험으로 대체했다. 런타임 계약 §13 항목 10에 거절 문장을 더했다(추가 설명만) |
| Q-3 · `start`는 변조를 탐지하지 않음 | **(가) 계약의 알려진 한계로 기록** | 코드 변경 없음. 런타임 계약 §13 항목 10의 “완료 조건”(한계) 칸에 한계 문장을 더했다: 탐지는 reconnect에서만 하며, start 전에 생긴 변조는 설정 정리로 그 세션에 적용되지 않지만 탐지·보고되지 않는다 |
| Q-2 앞부분 · 오래된 대기 탐지가 새 탐지보다 앞섬 | 유지 확정(**사용자 확인 2026-11-15**) | 코드 변경 없음. 시험 `an_older_pending_detection_is_kept_and_an_unreadable_one_is_replaced`가 고정한다 |
| L-c3-2 · 3초 잠금 대기 | 유지 확정(**사용자 확인 2026-11-15**) | 코드 변경 없음(`LOCK_WAIT`, 시험 `a_call_waits_briefly_for_a_released_lock`) |
| P4 · 장부 이동 | 진단 문구만 확정(**사용자 확인 2026-11-15**) | 코드 변경 없음(`moved_hint`, 시험 `a_moved_ledger_is_refused_with_its_cause`). 장부 디렉터리는 예산이 열려 있는 동안 옮기지 않는다 |

함께 반영한 것:
- 런타임 계약 §13 항목 10의 “대기 사본을 채택하면 그 변조 보고도 보고된 것으로 본다”에, 코디네이터가 `tamper_recorded`로 기록을 확인할 때만이라는 조건을 더했다(9b63ce8 검토 681c1d3 L-1의 계약 문구 반영, 추가 설명만).
- 검토 3724bc9 I-1: coordinator 시험 `central_checkpoints_keep_sequence_identity_and_tamper_reports`가 seq 2 takeover 뒤 `latest.tamper_recorded == true`도 확인한다.

남은 사용자 결정은 없다.

### 4.1 손상된 대기 파일의 사람 대조 절차(PR #70 검토 4e42bef L-2, 8e1ecf7 L-1로 보완)

역할이 start·reconnect에서 409 `EXTERNAL_RECONCILIATION_REQUIRED`(“the pending tamper file /state/pending-tamper.json cannot be read …”)로 멈추면 사람이 다음을 한다. 실제 호출을 다시 하기 전에 이 절차를 끝낸다.
1. 역할 볼륨의 `/state/pending-tamper.json`을 바이트를 바꾸지 않고 보존 사본으로 복사한다. 보존 사본 없이 지우지 않는다.
2. 내용을 읽을 수 있고 탐지로 알아볼 수 있으면(옛 세션 id·digest가 보임), 먼저 역할의 체크포인트 보관 사본(`/state/checkpoints/`)에서 그 탐지를 실은 takeover가 이미 보고됐는지 본다(보고됐으면 다시 쓰면 같은 탐지가 두 번 기록된다, 검토 b92a398 I-2). 보고되지 않았으면 그 내용으로 올바른 JSON 객체를 다시 써서 대기로 남긴다. 코디네이터가 받는 필드는 비어 있지 않은 `old_session_id`·`checkpoint_digest`·`observed_digest`다. 하나라도 비면 다음 takeover가 400으로 거절되고 역할이 다시 멈춘다. 다음 새 세션의 takeover가 이 탐지를 보고하고, 코디네이터가 `tamper_recorded: true`로 답하면 supervisor가 파일을 지운다.
3. 내용을 알아볼 수 없으면 탐지가 있었는지 바이트로 확인할 수 없다. `latest`의 `tamper_events`는 개수일 뿐이라 특정 탐지가 기록됐는지 보여 주지 않는다. 이때는 보존 사본을 남긴 채 파일을 지우고 **reconnect**를 한다. reconnect는 설정을 정리하기 전에 체크포인트의 설정 digest와 현재 digest를 비교하므로, 설정이 아직 변조된 채로 남아 있으면 새 탐지를 만들고 새 세션과 함께 보고한다(앞선 시도의 정리가 이미 설정을 지웠다면 다시 탐지하지 못한다, 검토 b92a398 I-1). start는 탐지하지 않으므로(Q-3) 이 단계에 쓰지 않는다. 손상 전의 탐지가 이미 기록됐는지는 알 수 없는 채로 남으며, 이것을 진행 기록에 적는다.
4. 무엇을 보고 어떻게 했는지 진행 기록에 남긴다. 대기가 있으면 reconnect에는 새 세션 id가 필요하다(없으면 400).

새 탐지가 손상 파일을 덮어쓸 때는 손상 사실이 남지 않는다(검토 4e42bef I-1). 사본을 남기는 기능은 넣지 않았고 한계로 둔다.

## 5. 범위 밖으로 남긴 것

- 같은 검토들의 다른 지적(PR #49 L-c3-1·L-c3-5, PR #51 F9·F10)은 X-08 목록에 없어 다루지 않았다. PR #58 P-3의 4xx 부분은 PR #58에서 이미 시험이 있다.
- 실제 장부·실제 컨테이너에서는 확인하지 않았다. 다음 실제 호출 전에 실제 작업 디렉터리의 `ieo-role-call`을 병합본으로 바꾸는 일은 X-02 준비에서 한다.

## 6. 검토 681c1d3 후속

PR #67 검토 681c1d3(승인, 낮음 2·정보 5)의 지적과 처리. 커밋 9b63ce8. 시험 349 통과, 0 실패, 0 무시([evidence/test.log](evidence/test.log), 커밋 9b63ce8). 변이 7개 모두 잡힘([evidence/mutants.txt](evidence/mutants.txt) 끝 부분).

| 지적 | 처리 | 시험 |
| --- | --- | --- |
| L-1 P-2의 남은 두 경로(중복 응답의 `duplicate` 갈래, `local_checkpoint`의 대기 사본 채택)가 코디네이터가 기록하지 않은 변조도 지움 | **교정.** 닫힌 쪽 선택지 가운데 “코디네이터가 기록했다고 말할 때만 지운다”를 택했다. 코디네이터는 같은 seq·같은 바이트의 중복 보고(200)에 `tamper_recorded`를 더하고, `latest`에도 `tamper_recorded`(그 체크포인트의 seq로 저장된 변조 사건이 있는가)를 더한다. 사건은 체크포인트와 같은 트랜잭션에서 그 seq와 함께 저장되므로 이 값은 정확하다. supervisor는 직접 응답·중복 응답·채택 세 경로 모두 이 값이 `true`일 때만 대기 파일을 지운다. `false`이거나 값이 없으면(이전 코디네이터) 대기로 남고, 다음 새 세션이 다시 보고한다. “채택 때는 늘 남긴다”는 선택지는 버렸다: 흔한 경우(응답만 잃고 사건은 저장됨)에 같은 탐지를 두 번 보고하게 된다. API 변경은 더하기만이다(gRPC `GetLatestCheckpointResponse.tamper_recorded = 18`, `ReportCheckpointResponse.tamper_recorded`의 중복 응답 의미, OpenAPI 200 중복 응답 스키마). `buf lint`·`buf breaking`(6e5e502 기준)·OpenAPI 검증 통과 | serve `an_unrecorded_tamper_stays_pending_after_a_duplicate_or_an_adopted_takeover`(HTTP·gRPC, 두 경로. 교정 없이는 실패), 기존 `a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once`(기록된 경우는 그대로 지움), coordinator `central_checkpoints_keep_sequence_identity_and_tamper_reports`(중복 true/false, latest false) |
| L-2 장부의 실제 호출 해제 시점 문구 | 조정자가 장부에서 처리한다 | — |
| I-1 `podman restart`의 `9>&-` 시험 없음 | **시험 추가.** 가짜 `podman restart`가 `/proc/self/fd/9`의 유무를 기록하고, 시험이 “fd9-closed”를 확인한다 | call_sh `the_restart_step_runs_only_when_every_guard_holds`(`9>&-`를 빼면 실패) |
| I-2 `curl -f`만의 시험 없음 | **시험 보강.** 가짜 상태 서버의 401 응답 본문에도 상태 항목을 넣어, HTTP 상태(`-f`)로만 거절이 갈리게 했다 | 같은 시험(`-sf`를 `-s`로 바꾸면 실패) |
| I-3 장부 세부(02-05 `next_action` 등) | 조정자가 장부에서 처리한다 | — |
| I-4 (가) 객체가 아닌 대기 JSON에서 패닉 | **교정.** 두 번째 대기 읽기에도 객체만 받도록 걸렀다. 새 탐지가 없으면 손상 파일과 같게 “대기 없음”, 있으면 새 탐지로 덮어쓴다. 손상·비객체 파일의 처리는 뒤에 사용자가 “재개 거절”로 정했다(§4, 18d5d9e) | serve (당시) `a_pending_file_that_is_not_an_object_does_not_panic`·— 이 시험은 §4의 `an_unreadable_pending_file_stops_until_a_person_reconciles_it`로 대체됐다 |
| I-4 (나) 정정 정리 뒤 빈 `children/`이 남음 | **교정하지 않음.** 복사 전 검사(`remeasure`)를 통과한 뒤의 쓰기 실패에서만 생긴다. 이 경로를 시험으로 재현할 값싼 방법이 없어(쓰기 오류 주입 필요), 시험 없는 정리 코드를 더하지 않았다. 빈 디렉터리는 장부 읽기에 쓰이지 않고 다음 정정이 그대로 쓴다 | — |
| I-5 X-02 준비: 새 장부로 기존 역할 볼륨을 쓰면 모든 역할 확인 때문에 거절됨 | 운용 메모로 받는다. X-02 준비에서 새 볼륨을 쓰거나 조정 단계를 계획한다. 코드 변경 없음 | — |

**사용자 결정 목록(§4) 갱신:** L-1은 교정했으므로 결정 항목이 아니다. 남은 결정은 Q-3, 손상·비객체 대기 파일의 처리(거절 또는 기록만), §4의 확인 세 가지다. 다음 실제 호출 전에는 `ieo-coord`도 9b63ce8 이후 병합본으로 바꿔야 L-1 교정이 효과를 낸다. 이전 코디네이터는 이 값을 주지 않으므로 대기가 남는 쪽(다시 보고)으로 동작한다. (이 결정들은 2026-11-15 사용자 결정으로 반영했다. §4)

## 7. PR #70 검토 4e42bef 교정

- M-1: 결정 기록에 계획 §4.3 표 전체(역할 몫·예산 검사·R2)를 옮겼다. 원장 resume.budget과 02-05 항목에도 역할 몫을 적었다.
- M-2: 런타임 계약을 revision 10으로 올리고 머리말에 바뀐 점을 적었다. 원장 plan_and_spec도 revision 10.
- L-1: 결정 기록 §2.2에 start도 거절하는 이유를 적었다.
- L-2: 거절 사유에 파일 경로와 이 보고 §4를 넣었고, §4.1에 사람의 대조 절차를 적었다.
- L-3: 원장의 낡은 “사용자 결정 대기” 문구와 X-08 next_action을 고쳤다.
- I-1: 새 탐지가 손상 파일을 덮어쓸 때 사본을 남기지 않는 것은 한계로 둔다(§4.1).
- I-2: §3·§6의 시험 수와 바뀐 시험 이름을 고쳤다. I-3: EARLY-01에 F4 규칙. I-4·I-5: 기록.
- 시험: 바뀐 코드는 거절 사유 문자열뿐이다. ieo-supervisor 122 통과, 0 실패(evidence/test.log 끝). 전체는 새 head의 CI로 확인한다.

## 8. 수용(검토 8e1ecf7 승인)

- X-08의 PR: #67(교정, 검토 681c1d3 승인) 병합 6e5e502, #68(후속, 3724bc9 승인) 병합 e0a7f8a, #70(사용자 결정 반영, 4e42bef 변경 요청 → 교정 1 → [8e1ecf7 승인](reviews/review-pr70-8e1ecf7.md)) 병합 1c21177, main CI 43718393840 success. 각 PR의 검토·교정 수는 4·3 상한 안이다.
- 장부 §7 X-08 수용 기준: 항목마다 교정과 회귀 시험이 있다. 교정하지 않은 항목(Q-3)과 유지한 동작(Q-2·L-c3-2·P4)은 사용자 결정(2026-11-15)을 받았다. 모델 호출·예비 회차·실제 중간 정지·추가 결제는 쓰지 않았다. 독립 검토 승인·필수 CI·승인 head 병합·main CI가 모두 있다. X-08을 **검수 완료**로 기록한다. 추가 작업이므로 73·12 완료 수에 넣지 않는다.
- **차단의 해제 기준.** 개정 A6은 “X-08 병합 전”을 적었지만, 원장은 PR #67 검토 681c1d3 L-2에 따라 **검수 완료**까지 실제 호출·X-02를 막았다. 이 기록으로 그 조건 하나가 채워진다. X-02는 executable 등록·의미 검토 등 남은 조건이, 02-05 단계 8은 예산 이름·상한 코드 PR과 단계 3·4 병합 등 그 단계의 전제가 여전히 필요하다.
- **검토 8e1ecf7의 남은 지적:** L-1은 §4.1을 보완했다. L-2(revision 10은 02-05 계획이 쓰려던 번호): 02-05의 계약 변경은 **revision 11**부터 쓴다(원장 02-05 항목에 적음). I-1(§6 표 행)·I-4(시험 수 350) 정리. I-2·I-3 기록.
- **실제 호출 전 운영 준비(검토 3724bc9 I-5):** 실제 회차 전에 `ieo-role-call`·`ieo-coord`(영속 코디네이터 58090 포함)를 1c21177 이후 빌드로 바꾼다.

- 수용 기록 PR #71 검토 b92a398(승인): L-1 머리 상태 행, I-1·I-2 §4.1 보완(02-05 단계 1 PR에서). I-3~I-5 기록.
