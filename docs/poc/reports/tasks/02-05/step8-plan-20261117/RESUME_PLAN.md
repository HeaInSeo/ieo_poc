# 02-05 단계 8 재개안 · R1 멈춤 뒤 정산, 회차 집계, 승인, 배포

| 항목 | 기록 |
| --- | --- |
| 근거 | [사용자 결정 2026-11-17 경고](../../../../User_Decisions_2026-11-17_warning.md) (다)의 §1 항목 4~6, [부록 A](#부록-a-한도-경고-판정의-근거-정책-전후-시험), [실행안](EXECUTION_PLAN.md) §12, [예산 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md) §2.1, [예비 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md) |
| 이 문서가 하는 일 | 한 번에 검토할 재개안이다. **승인을 대신하지 않는다.** 실제 회차, 실서버 재배포·재시작, 예비 사용, 정정 기록 추가는 아래 §6의 승인 뒤에만 한다 |
| 상태 | **재개 묶음**: 스크립트·예행 완료(재개 PR, 검토·병합 전; §9). **정책 교정**: 코드·계약·시험 완료, PR #89 병합(main 90ffef5; 근거·정책 전후·시험은 [부록 A](#부록-a-한도-경고-판정의-근거-정책-전후-시험)). **정산 형식**: 코드·시험 완료(정산 PR, 검토·병합 전; [부록 B](#부록-b-정산-형식ieo-role-run-settlementv0)). **사용자 결정**: [사용자 결정 2026-11-17 정산](../../../../User_Decisions_2026-11-17_r1-settlement.md)로 §2.1 (가)와 §3.1 (A)를 조건부 승인. **독립 검산**: 진행 중(별도 검토자). **실제 장부 정산·운영 변경·재시험**: 하지 않음(최종 묶음 승인 뒤) |
| 만든 방법 | 보관된 R1 증거만 읽었다: 02-05 장부(`records.jsonl`, `streams/s8-r1.jsonl`), 구동 기록(`s8-out/`), 역할 상태 볼륨의 세션 transcript(`state/claude/projects/…`; `cred/`는 읽지 않음). 58090·55433에 쓰지 않았고 모델 호출 없음. 숫자는 [evidence/r1-reconciliation.json](evidence/r1-reconciliation.json) |

## 1. R1에서 실제로 확인된 것과 아닌 것

| 구분 | 내용 | 근거 |
| --- | --- | --- |
| 전달·인수·첫 행동 연결 | **확인됨.** 전달 `D-c2d9…`(판단 `no_checkpoint` → `start`, `accepted`), takeover 체크포인트 seq 1, 첫 행동 `tool_use`(사건 위치 2, 12:19:48.596Z), after_run 체크포인트 seq 2. 연결 `taken_over: true`, 첫 행동 충돌 없음. 명세 digest `0e4d471d…`(r6) | `s8-out/chain-S8-DEV-A1-s8-r1.json` |
| 도구 제한 | **확인됨.** `init.tools` = `Glob, Grep, Read`, 쓴 도구 `Read` 두 번(README.md, SANDBOX.json, 대상 사본 안) | 보관 stream |
| 경고로 인한 중단 | `rate_limit_event` `allowed_warning`(seven_day 0.86, 경고 기준 0.75, 재설정 2026-11-22T08:00:00Z, five_hour 0.04 allowed), `overageStatus: rejected`(`out_of_credits`), `isUsingOverage: false`. 당시 규칙이 멈춤(exit 143) | 보관 stream 6행, 장부 기록 |
| 사용량 | 장부: **미계측**(가드 멈춤, 결과 사건 없음). stream의 관측 하한 billable 1,429, cache_read 3,223(총량 아님) | 장부 기록 |
| R2·G1 | **실행하지 않음** | `runs-console.log`(STOP) |
| 구동 스크립트 출력의 오류 | `check-run.py`가 미계측 회차를 `measured, … billable 0`으로 출력했다(요약 줄만, 판정은 실패로 맞음). 이 PR에서 고침 | `runs-console.log` |

## 2. R1 사용량 대조

| 자료 | 내용 |
| --- | --- |
| 보관 stream(7행) | `assistant` 두 줄은 **한 메시지**(`msg_011Ch8VK…`)의 스트리밍 중간값이다(output 16). `result` 사건 없음 → 하한만 나온다: input 2 + cache_creation 1,411 + output 16 = 1,429, cache_read 3,223 |
| 세션 transcript(25항목, 49,297바이트, SHA-256 `806746fd…dc68`) | 같은 메시지의 **최종** 사용량: input 2, output **130**, cache_creation 1,411, cache_read 3,223. `assistant` 16·18행은 그 한 메시지의 **내용 블록별 항목**이다(`apiBlockIndex` 0·1, 도구 사용 하나씩, 둘 다 같은 최종 사용량; 스트리밍 중간값이 아님). `api-request` 항목은 **하나**(14행, 12:19:47.084Z)이고 그 `id`(`36a2d590…`)가 두 `assistant` 항목의 `requestRef`와 같다(요청 1 : 메시지 1). 끝부분의 종료 기록(`last-prompt`·`custom-title`·`agent-name`·`atis-latch`·`cost-state`)에 CLI의 세션 집계 `cost-state.modelUsage`가 있고 값이 위와 같다(input 2, output 130, cache_read 3,223, cache_creation 1,411; `totalAPIDuration` 1,756 ms, 보고 금액 0.00727 USD는 환산값). 그 뒤 24행 둘째 도구 결과(12:19:48.852Z), 25행 `prompt_snapshot` 첨부(12:19:48.855Z: CLI가 다음 요청을 조립할 때 쓰는 항목, 11행에서는 21 ms 뒤 `api-request`가 따랐다). 그 뒤 `api-request`는 없다. 파일 마지막 쓰기 12:19:48.953Z, 장부 끝 12:19:49.763Z(SIGTERM, exit 143, 4,119 ms) |
| 판단 | 두 번째 요청은 기록되지 않았다: 요청마다 남는 `api-request` 항목이 하나뿐이고, 종료 때 쓴 CLI의 집계가 그 한 요청의 최종값과 같다. **CLI 자신의 종료 집계로는 billable 1,543(2 + 130 + 1,411), cache_read 3,223이다.** 관측 하한(1,429)보다 114 많다. 이 값의 이름은 **“CLI 종료 집계(클라이언트 쪽)”**이다. 공급자 쪽 청구가 아니다. 남는 불확실성(PR #89 검토 91e87ce L-5): transcript의 순서가 종료 기록(19~23행, 23행 `cost-state`, 시각 없음) → 둘째 도구 결과(24행, 12:19:48.852Z) → 첨부(25행, 12:19:48.855Z)다. 즉 CLI의 반복은 종료 집계를 쓴 뒤에도 이어졌고, 도구 결과 다음의 자연스러운 단계는 둘째 요청이다. 그 요청이 항목이 쓰이기 전에 보내졌다가 끊겼다면 클라이언트 쪽 자료로는 볼 수 없다(구독 사용량일 뿐 추가 청구 USD 0에는 영향 없음). 14행 뒤에 `api-request` 항목이 없으므로 기록된 요청은 하나다. 0이라고 증명하지는 못한다 |
| 지금 코드로 정정할 수 있나 | 기존 정정(`ieo-role-run-correction/v0`, `reevaluate`)으로는 없다: **보관 stream의 결과 사건**으로 다시 계측하는데 R1 stream에는 결과 사건이 없다. 가드가 멈춘 회차는 멈춘 뒤의 결과를 믿지 않는다는 규칙은 그대로다. 그래서 정산 PR이 별도 형식 `ieo-role-run-settlement/v0`를 더했다([부록 B](#부록-b-정산-형식ieo-role-run-settlementv0)) |

### 2.1 정산 결정: (가) 조건부 승인([사용자 결정 2026-11-17 정산](../../../../User_Decisions_2026-11-17_r1-settlement.md))

사용자는 (가)를 조건부로 승인했다. 조건: 별도 검토자의 **독립 검산**(같은 해시의 사본으로 세션·시도 일치, 같은 `message.id` 중복 제거, 네 토큰, API 요청 기록 수, `cost-state` 전후 항목과 시각)이 먼저이고, 검산 대상 값 billable 1,543·cache_read 3,223은 아직 확정값이 아니다. 근거가 부족하면 미계측을 유지한다. 원본 기록은 어떤 경우에도 그대로 두고, 정산은 최종 묶음 승인 뒤에만 추가 기록으로 쓴다. 독립 검산용 사본: `$D/s8-evidence/s8-r1-transcript.jsonl`(0400, 저장소 밖, SHA-256 `806746fd…dc68`, 원본과 같음, `MANIFEST.txt`).

**독립 검산 결과: CONFIRMED WITH CAVEAT**([evidence/r1-usage-verification.md](evidence/r1-usage-verification.md), 전사 본문 없음). 별도 검토자가 사본·원본 해시, 세션·시도·시간 창, 메시지별 최종 사용량과 `cost-state`(두 관점)에서 billable 1,543·cache_read 3,223을 그대로 재현했다. 단서: 이 값은 **기록된 요청 하나**의 사용량이다. 25행의 `prompt_snapshot`은 다음 요청이 준비되고 있었음을 보이며, SIGTERM 직전에 보내졌으나 기록이 남지 않은 요청은 클라이언트 쪽 자료로 배제할 수 없다. 공급자 쪽 청구는 보이지 않는다. 그래서 정산 기록은 이 단서를 `verification.uncovered: unlogged_in_flight_request_at_sigterm`과 `residual` 문장으로 함께 싣는다(부록 B).

| 선택 | 내용 | 남는 예산(02-05 전체 / 개발 몫) | 결과 |
| --- | --- | --- | --- |
| **(가) transcript 집계로 정산(추천)** | 새 정산 기록 형식(예: `ieo-role-run-settlement/v0`): transcript 사본을 장부(`children/s8-r1/`)에 두고 SHA-256과 `cost-state.modelUsage`·`api-request` 수를 장부를 읽을 때마다 다시 확인한다. R1 사용량 = billable 1,543, cache_read 3,223, 시간 5초(4,119 ms 올림). 출처를 `transcript_exit_accounting`, 위 불확실성을 기록에 적는다. 멈춤 사유 중 “미계측”과 “당시 규칙의 신호”만 해소하고 “결과 사건 없음”은 남긴다(R1은 실패 회차로 남음) | 회차 3/4 남음, 2,395초, billable 198,457, cache_read 1,996,777 / 개발 2회, 1,795초, 148,457, 1,496,777. 배정 `S8-DEV-A1`: 1,795초, 148,457 | 재개 가능. 코드 PR 하나 더 필요(형식·재검증·시험, 모델 호출 없음) |
| (나) 관측 하한을 “하한” 표시로 기록 | billable 1,429, cache_read 3,223에 `lower_bound: true` | 위보다 114 많이 남음 | 사용자 지시(하한을 총량으로 보지 않음)와 맞지 않아 **권하지 않음** |
| (다) 상한으로 청구 | R1의 남은 몫 전부(개발 150,000·1,800초)를 쓴 것으로 | 개발 몫 0 | 개발 회차를 더 할 수 없음(사실상 종료) |
| (라) 미계측 유지 | 그대로 | 계산 불가 | 공유 예산 검사와 배정 정산(409 `ASSIGNMENT_USAGE_UNMEASURED`)이 다음 호출을 모두 막는다: **단계 8은 R1에서 끝난다** |

## 3. 회차 집계와 재실행 분류

| 항목 | R1 뒤 지금 | 비고 |
| --- | --- | --- |
| 02-05 회차 | 1/4 사용(R1) | 실패 회차도 센다(삭제·초기화 없음) |
| 개발 몫 | 1/3 사용 | 표식 없는 계획 회차 1/2 사용 |
| Guardrail 몫 | 0/1 | |
| 예비 | 미사용 1회 | 자동으로 쓰지 않는다 |

R1을 대신할 개발 회차(아래 **R1b**)와 R2를 모두 하면 개발 회차가 3개(R1·R1b·R2)다. 개발 몫 3회 안이지만 **표식 없는 계획 회차는 2회까지**라 셋째는 예비 표식이 있어야 한다. 그런데 예비는 실패한 회차를 “결과로 인한 멈춤”일 때만, 그리고 사용량이 계측됐을 때만 대체한다(`spare_releasable`). R1은 지금 미계측이고 멈춤 사유에 한도 신호가 있어 예비로 대체할 수 없다.

### 3.1 재실행 분류: (A) 조건부 승인([사용자 결정 2026-11-17 정산](../../../../User_Decisions_2026-11-17_r1-settlement.md))

| 선택 | 내용 | 결과 |
| --- | --- | --- |
| **(A) 예비를 R1 대체에 명시적으로 씀(승인, 조건부)** | 이것은 두 해석에 기댄다(PR #89 검토 91e87ce L-4): (i) 정산이 R1의 신호를 규칙 rev 19로 다시 판정해 “미계측”과 “당시 규칙의 신호” 멈춤을 해소한다. (ii) 가드가 일으킨 “결과 사건 없음”을 예비 결정 §4.3의 “결과로 인한 멈춤”처럼 해제할 수 있다. 사용자는 이 둘을 **이번 s8-r1에 한정한 특례**로 승인했다(정산 결정 §2.3). 코드는 그 범위를 강제한다: 가드가 멈춘 회차의 “결과 사건 없음”은 이 정책의 정산이 있을 때만 예비가 해제하고, 실제 거부·추가 과금 신호·시간·토큰 상한·다른 미계측 회차는 정산되지 않으며 예비도 해제하지 못한다(부록 B). 사람(`kind: user`)이 R1b 전달에 예비 표식(사유, `replaces: s8-r1`, `evidence_ref`: 이 재개안·정산 결정)을 붙여 새 세션으로 시작한다. 예산 숫자는 그대로다 | 개발 R1(실패)·R1b(예비)·R2, Guardrail G1 = 4/4. **더 남는 예비 없음**: R1b·R2·G1 중 하나라도 실패하면 멈추고 보고하고 단계 8은 끝난다 |
| (B) 계획 회차 수 변경 | 02-05의 개발 표식 없는 계획 회차를 2 → 3으로(예산 결정 변경). 총 4회 상한은 그대로라 예비는 사실상 쓸 수 없다 | (A)와 같은 회차 수. 예산 결정을 바꾸는 것이라 별도 승인 |
| (C) R2·G1 없이 종료 | R1까지로 단계 8을 닫고 보고 | 실제 재시험 없음 |

“실행 환경 결함”(예비 결정 §4.2)에 R1의 원인(우리 실행 가드 규칙의 결함)이 해당하는지는 표식을 붙이는 사람이 판단한다. 코드는 판단하지 않는다.

## 4. 중앙 체크포인트와 공식 복구

- `s8-r1`의 중앙 체크포인트: seq 1 `takeover`, seq 2 `after_run`(12:19:49.649Z 수신). 최신이 `after_run`(끝난 회차)이라 판단은 `resume_allowed`다. **공식 복구(단계 7b)는 필요 없다**: 복구는 끝나지 않은 체크포인트(`takeover`·`error`) 위의 새 세션 승인이다.
- R1b는 R1 세션을 이어 쓰지 않고 **새 세션**이어야 한다(R1의 대화는 경고 멈춤으로 잘렸다). 전달 본문에 `new_session: true`를 주면 `resume_allowed`에서 `start`가 된다(`choose_op`). R2는 그 뒤 최신 체크포인트(R1b의 `after_run`)의 세션을 `reconnect`로 잇는다.
- 배정 사용량: chain의 `usage`는 `runs 0`이었다. 코디네이터는 장부를 **다음 전달 때** 정산하기 때문이다(설계대로). 지금 다음 전달을 보내면 R1이 미계측이라 409 `ASSIGNMENT_USAGE_UNMEASURED`로 막힌다. **§2.1 정산이 R1b보다 먼저**여야 한다.
- GitHub head 대조(단계 7)는 R2의 `reconnect`에서 다시 한다(대상 `2ab9d876…`는 그대로).

## 5. 코드, 배포, 재시작, 허용 규칙

| 항목 | 내용 |
| --- | --- |
| 적용할 코드 | 이 PR(경고 판정)의 병합 SHA **+** §2.1 (가)를 고르면 정산 형식 PR의 병합 SHA. 두 PR 모두 독립 검토·필수 CI·승인 head 병합·main CI 확인 뒤. 빌드는 그 main SHA에서 release로 새 디렉터리(`$D/bin/m-<SHA>/`) |
| 코디네이터(58090) 재시작 | 지금 M(46799b0)을 멈추고 새 빌드로 기동(공유 예산 검사·회차 기록이 코디네이터 프로세스 안에 있음: 경고 기록과 정산 형식 읽기). 캡처한 환경·인자 그대로, `--tokens`·`--roles` 그대로. 마이그레이션: 두 PR 모두 스키마를 바꾸지 않으면 없음(병합 전 확인) |
| 역할 컨테이너 | 개발·Guardrail 모두 실행 가드가 컨테이너 안의 `ieo-supervisor`라 **새 바이너리가 필요하다**(옛 것은 R1b에서 같은 경고로 다시 멈춘다). `run-step8-roles.sh`는 기존 작업 디렉터리를 거절하고, 새 볼륨은 개발 역할의 로컬 체크포인트(R1 `after_run`)를 잃어 중앙과 어긋난다. 그래서 **같은 볼륨에서 바이너리만 바꾸고 재시작**하는 새 스크립트가 필요하다(예: `launcher/restart-roles.sh`: 두 컨테이너 정지 → `W/bin/ieo-supervisor` 교체·SHA-256 확인 → 시작 → 상태 확인). 다음 재개 문서 PR에서 쓰고 시험한다 |
| 실행본 사본 | 새 `$D/step8-resume/`(launcher 전체 + `rate_limit_policy.py` + 명세, 병합 SHA의 blob과 같음), `$D/step8-ops/`에 새 `obs-check.sh`와 `rate_limit_policy.py`, 새 `SHA256SUMS`(자기 해시가 바뀜), 둘 다 쓰기 금지. 옛 `step8-46799b0`은 증거로 둔다 |
| 구동 스크립트 | 지금 `run-step8.sh runs`는 R1(`s8-r1`, `no_checkpoint`) → R2 → G1 고정이고 첫 관측 마감(D6)을 본다. 재개용 단계가 필요하다: R1b(`s8-r1b`, 키 `s8-dlv-r1b-0001`, `new_session: true`, 예비 표식은 사람 토큰으로) → R2(R1b 세션 `reconnect`) → G1, `check-run.py`의 R2 세션 비교와 G1 지시문 수집을 R1b 기준으로 |
| 예비 표식을 보내는 쪽 | 예비 표식은 `user` 자격만 붙일 수 있다(403 `HUMAN_APPROVAL_REQUIRED`). R1b 전달은 **사용자 본인이 자기 토큰으로** 보낸다(의미 검토와 같은 방식, 명령 하나). 기다림과 확인, R2·G1은 에이전트가 한다 |
| 허용 규칙 변경 | 정확한 줄로 다시 만든다: 확인 줄의 목록 해시, 새 바이너리의 코디네이터 `start` 줄, 역할 재시작 스크립트 줄, 재개 구동 단계의 `export`·호출 줄. 옛 줄(46799b0 사본·M의 `start`·`run-step8-roles.sh`)은 지운다. 사용자 명령 하나(백업 → 바꿈 → JSON 확인 → 개수 출력)로 준다 |

## 6. 필요한 승인(한 번에 검토)

1. §2.1 (가)와 §3.1 (A)는 조건부 승인됨. 독립 검산: CONFIRMED WITH CAVEAT(§2.1). 남은 조건: 정산 형식 PR의 검토·병합·main CI.
2. 최종 묶음에서: 정산 기록을 실제 장부에 쓰는 것(`ieo-role-call settle`, 부록 B), 사람이 R1b에 예비 표식을 붙이는 것.
3. 경고 정책 PR(이것)과 정산 PR의 병합, 그 SHA로 빌드.
4. 실서버 변경: 58090 재시작(새 빌드), 역할 컨테이너 바이너리 교체·재시작, 허용 규칙 교체.
5. 정정 기록 추가(정산 기록, 원본 보존).
6. 실제 회차 R1b·R2·G1(모델 호출 3회). 회차 중 재시작·재시도는 계속 묶음 밖이다.

## 7. 관측 유효 기간과 재실행 전제

- **관측**: 다음 호출의 관측은 장부의 가장 새 기록의 stream, 즉 R1 stream이다(`caller::observation`). D6은 더 쓰이지 않는다. 새 규칙에서 R1의 사건은 경고로 통과하므로 **R1 stream이 유효한 관측**이고 7일 한도는 **2026-11-24T12:19:49Z**(R1 끝 1794917989 + 7일)다. 옛 마감(D6, 2026-11-21T23:14Z)은 R1 기록이 생긴 뒤로 다음 호출을 정하지 않는다. 재개 구동 단계는 이 시각으로 마감을 확인해야 한다(지금 스크립트는 D6 시각을 본다).
- **7일 창**: R1의 사건은 seven_day 0.86, 재설정 **2026-11-22T08:00:00Z**(KST 17:00)다. 그 전에 하면 R1b·R2·G1도 경고를 받을 수 있고(기록·보고), 사용량이 경고 기준을 넘어 `rejected`가 되면 그 회차는 멈추고 미계측이 된다(추가 사용은 거부 상태라 과금은 없다). 회차가 작아(R1 billable 1,543) 그럴 가능성은 낮다. 재설정 뒤에 하면 경고 없이 할 수 있다. 어느 쪽이든 관측 한도(11-24) 안이다.
- **전제(모두 확인 뒤 R1b)**: 두 PR 병합·main CI 성공, 새 빌드 SHA-256 기록, §2.1 정산 기록이 장부에 있고 다시 읽혀 계측(코디네이터의 공유 예산 검사가 시작을 허용), `assignment_heads`가 `S8-DEV-A1`·`S8-GR-A1`(배정은 그대로), 두 역할 서비스 도달·중앙 모드·유휴·quota 일치·새 바이너리, 02-05 장부에 `inflight.json` 없음, 58090 연결 0에서 재시작, 허용 규칙 교체 확인, 디스크 여유(루트 파일 시스템이 시험 중 가득 찬 적이 있음: 재시작 전 확인).

## 8. 보고 구분

보고는 다음을 따로 적는다: 정책 교정 완료(이 PR, 검토·병합 상태), R1의 확인된 연결과 도구 제한, 경고로 인한 R1 중단, R1 사용량(미계측, 정산 전·후), 미실행 R2·G1, 실제 재시험 완료 여부(재개 승인 뒤 별도).

## 9. 최종 재개 묶음(사용자 결정 2026-11-17 정산 §2.5)

이 절은 실제 정산 반영·운영 변경·모델 호출 **직전에 한 번에 승인받을 묶음**이다. 아래 명령은 모두 시험 시스템에서 처음부터 끝까지 예행했다([evidence/dry-run-resume.log](evidence/dry-run-resume.log), 34/34). 그 예행은 46799b0 바이너리로 2026-11-17의 R1 멈춤을 재현한 뒤 재개했다. `<SHA>`는 정산 PR과 재개 PR이 병합된 main의 SHA, `<SHA12>`는 그 앞 12자, `<SUMS>`·`<SUP>`는 A1이 출력하는 해시다. `$D` = `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data`(규칙 명령에서는 글자 그대로 쓴다).

### 9.1 근거와 수치

| 항목 | 내용 |
| --- | --- |
| 독립 검산 | **CONFIRMED WITH CAVEAT**([evidence/r1-usage-verification.md](evidence/r1-usage-verification.md), SHA-256 `28b2f04e…9267`): billable 1,543, cache_read 3,223을 두 관점(메시지별 최종 사용량, `cost-state`)에서 그대로 재현. 단서: 기록된 요청 하나의 사용량이며, SIGTERM 직전에 보내졌으나 기록이 사라진 요청은 클라이언트 쪽에서 배제할 수 없고 공급자 쪽 청구는 보이지 않는다 |
| 정산 후보 기록 | `ieo-role-run-settlement/v0`, `settles: s8-r1`, `source: transcript_exit_accounting`, `usage_run.tokens` input 2·output 130·cache_creation 1,411·cache_read 3,223, `evidence.sha256` `806746fd…dc68`(= 원본), `verification` API 요청 1·메시지 1·`uncovered: unlogged_in_flight_request_at_sigterm`, `independent_check`(보고서 경로, `28b2f04e…`, CONFIRMED WITH CAVEAT), `decision` 정산 결정. 장부 사본에서 `--dry-run`으로 만든 후보와 같다(아무것도 쓰지 않음). R1은 실패(“결과 사건 없음”)로 남는다 |
| 남는 예산(정산 뒤) | 02-05 전체: 회차 3/4, 2,395초, billable 198,457, cache_read 1,996,777. 개발 몫: 2회, 1,795초, 148,457, 1,496,777. Guardrail 몫: 1회, 600초, 50,000, 500,000. 배정 `S8-DEV-A1`: 1,795초, 148,457. 추가 청구 USD 0 |
| 회차 집계 | 실패 R1 + 예비 R1b + R2 + G1 = 4/4. 추가 예비·자동 재시도 없음. R1b·R2·G1 중 하나라도 멈추면 단계 8은 거기서 끝나고 보고한다 |
| 특례 범위 | 가드가 당시 규칙의 경고로 멈춘 s8-r1 하나. 코드가 강제한다: 정산은 그런 회차만(현재 규칙으로 모든 신호 통과, 경고 하나 이상), 예비는 그 정산이 있을 때만 그 “결과 사건 없음”을 해제. 실제 거부·추가 과금 신호·시간·토큰 상한·다른 미계측 회차는 해당 없음(부록 B 시험) |
| 관측 | 다음 호출의 관측은 R1 stream(새 규칙에서 경고로 통과). 7일 한도 **2026-11-24T12:19:49Z**(구동 스크립트는 10분 여유로 확인). 7일 창 재설정 2026-11-22T08:00:00Z: 그 전이면 R1b·R2도 경고를 받을 수 있다(기록·보고, 멈춤 아님). `rejected`가 되면 그 회차는 멈추고 미계측이 되며 단계 8은 끝난다(추가 사용은 거부 상태라 과금 없음) |

### 9.2 코드와 바이너리

| 항목 | 내용 |
| --- | --- |
| 코드 | 정산 PR(#90: 정산 형식, `ieo-role-call settle`)과 재개 PR(`ieo-role-call records`, `check-run.py`의 유효 기록, `restart-roles.sh`, `run-step8-resume.sh`, `make-resume-copies.sh`, `apply-perm-resume.py`, 예행)이 병합된 main `<SHA>`. 두 PR 모두 독립 검토·필수 CI·승인 head 병합·main CI 확인 뒤 |
| 빌드(A0) | `<SHA>`에서 release 빌드(debuginfo 없음, 대상 디렉터리 하나, 루트 여유 8G 이상 확인) → `$D/bin/m-<SHA12>/`의 `ieo-coord`·`ieo-supervisor`·`ieo-role-call`, 각 SHA-256 기록. 옛 `bin/ieo-coord-M`(46799b0)·`bin/m-46799b0/`은 되돌리기용으로 그대로 |

### 9.3 순서(A = 에이전트, U = 사용자)

1. **A0** 빌드(위).
2. **A1** 사본과 규칙: `bash <checkout>/docs/poc/reports/tasks/02-05/step8-plan-20261117/launcher/make-resume-copies.sh <SHA의 checkout> $D $D/bin/m-<SHA12> $D/private/s8/rules-resume.json`. 결과: `$D/step8-resume/`(커밋의 파일만, blob과 같음), `step8-ops/`의 `obs-check.sh`·`rate_limit_policy.py`, 새 `SHA256SUMS`(자기 해시 `<SUMS>`), 둘 다 쓰기 금지, 규칙 파일(옛 허용 28줄 빼기, 새 32줄 넣기). 옛 `step8-46799b0`은 증거로 둔다.
3. **A2** `$D/private/s8/plan-0205.json`(장부·역할 URL·어댑터 토큰 파일 경로만, 비밀 없음): `{"budget": "02-05", "ledger_dir": "$D/ledger-0205", "first_observation": {"stream": "$D/ledger-0205-obs/D6.jsonl", "observed_unix": 1794698043}, "roles": {"development": {"url": "http://127.0.0.1:58221", "adapter_token_file": "/opt/rust/ieo_poc_runs/02-05-s8-20261118/adapter.token"}, "guardrail": {"url": "http://127.0.0.1:58222", "adapter_token_file": "/opt/rust/ieo_poc_runs/02-05-s8-20261118/adapter.token"}}}`(경로는 글자로).
4. **U1** 허용 규칙 교체(사용자 터미널, 명령 하나): `python3 -I $D/step8-resume/launcher/apply-perm-resume.py $D/private/s8/rules-resume.json`. 백업을 만들고, 정확한 줄만 바꾸고, JSON을 확인한다. 예상 출력 `allow 28 -> 32 … new lines all present once: True`. 다시 실행하면 `already applied`. deny 7줄과 다른 설정은 그대로다.
5. **A3** 사전 확인(읽기): 확인 줄 `cd $D && echo '<SUMS>  step8-ops/SHA256SUMS' | sha256sum --quiet --strict -c - && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && find step8-resume step8-ops -perm /222 | wc -l | grep -qx 0 && echo "copies ok"`(아래 `<확인>`); 58090 연결 0, 02-05 장부에 `inflight.json` 없음, 두 역할 유휴, 루트 여유 8G 이상.
6. **A4** 역할 바이너리 교체: `<확인> && bash $D/step8-resume/launcher/restart-roles.sh swap /opt/rust/ieo_poc_runs/02-05-s8-20261118 $D/bin/m-<SHA12>/ieo-supervisor <SUP>` → 두 역할 `up, idle, /opt/ieo/ieo-supervisor <SUP>`. 같은 컨테이너·볼륨(개발 역할의 s8-r1 `after_run` 체크포인트 유지), `cred/`는 건드리지 않는다. **되돌리기**: `<확인> && bash $D/step8-resume/launcher/restart-roles.sh rollback /opt/rust/ieo_poc_runs/02-05-s8-20261118`(보관한 `.prev`로).
7. **A5** 코디네이터 교체(약 2분 멈춤): `<확인> && bash $D/step8-resume/launcher/coord-switch.sh stop $D/coord.pid 900 && bash $D/step8-resume/launcher/coord-switch.sh start $D/bin/m-<SHA12>/ieo-coord $D/private/s8/coord-778ee47 $D/coord.pid $D/logs/coord-<SHA12>.log --tokens $D/private/s8/tokens-s8.json -- --roles $D/private/s8/roles.json`. 별도 호출에서 생존 확인(health 200, pid·exe·수신자). **되돌리기**: `stop … 900` 뒤 `start $D/bin/ieo-coord-M …`(같은 캡처·인자, 로그 `coord-M-rollback.log`). 스키마 변경 없음(마이그레이션 없음).
8. **A6** 정산(최종 승인 뒤): `PLAN`·`ROLE_CALL`·`TRANSCRIPT`·`ORIGINAL_PATH`·`ORIGINAL_SHA256`·`DECISION`·`CHECK_REPORT`·`CHECK_VERDICT`·`COORD_ROLES`를 `export`한다. 먼저 `export DRY=1`과 함께 `run-step8-resume.sh settle` → `dry run: s8-r1 still unmeasured`(아무것도 쓰지 않음). 이어서 `DRY` 없이 `run-step8-resume.sh settle` → `s8-r1 counts: measured {… 'output': 130 …} stop_required True no result event | settlement guard_stop_on_old_rule_warning CONFIRMED WITH CAVEAT`. 원본 줄은 그대로이고 한 줄이 더해진다. 이어서 같은 단계가 **코디네이터 자신의 예산 계획**(`$D/private/s8/roles.json`의 `budget_plan`, 코디네이터가 장부를 읽는 바로 그 경로)으로 장부를 다시 읽어 s8-r1이 정산으로 세어지는지 확인한다(`coordinator's reading: s8-r1 measured | settlement guard_stop_on_old_rule_warning`, PR #90 검토 7b904de N-2). 아니면 R1b로 가지 않는다. 정산은 장부를 읽을 때마다 장부 안의 두 사본(`$D/ledger-0205/children/s8-r1/settlement-transcript.jsonl`과 검산 보고서 사본 `independent-check.md`)을 다시 확인하고, 고정한 s8-r1(예산·시도·세션·stream 해시)에만 적용된다. 그래서 두 사본은 지우거나 옮기지 않는다(없어지면 장부를 쓸 수 없게 되어 멈춘다). 보고서의 원래 경로(`$D/step8-resume/evidence/…`)는 출처 기록이며 다시 읽지 않는다.
9. **A7** `run-step8-resume.sh r1b-body` → `$D/s8-out/r1b-body.json`(새 세션, `new_session: true`, 예비 표식 `replaces: s8-r1`, 사유·근거)을 쓰고 **U2** 명령을 출력한다.
10. **U2** R1b 전달(사용자 본인 토큰, 출력 없음): A7이 출력한 `curl … -H 'Idempotency-Key: s8-dlv-r1b-0001' -X POST http://127.0.0.1:58090/v0/repos/ieo-poc-test/roles/development/deliveries --data @$D/s8-out/r1b-body.json` → `http 201`, `resume_allowed start accepted`. 코디네이터가 사람(`kind: user`)과 시각을 표식에 적는다.
11. **A8** `run-step8-resume.sh r1b-check` → 기다림, 장부 기록, 유효 기록으로 §6.2 모든 확인(s8-r1은 정산으로 센다), `s8-r1b spare: replaces s8-r1 by user heain`.
12. **A9** `run-step8-resume.sh r2g1`(배경 실행): 관측 마감(R1 끝 + 7일) → R2(R1b 세션 `reconnect`) → G1(`start`), 회차마다 모든 확인. 끝 줄 `R1b, R2 and G1 recorded and checked`.
13. **보고**(정책 교정·정산·재시험을 따로) 뒤 사용자가 단계 8 허용 규칙(새 32줄)을 지운다.

모든 규칙 명령은 확인 줄(`<확인> &&`)로 시작한다. 규칙 명령은 그 단계에서만 실행하고, 끝났거나 멈춘 뒤에는 사용자의 명시 지시 없이 다시 실행하지 않는다. 허용된 명령이 막히면 멈추고 그 줄을 사용자에게 준다.

### 9.4 되돌리기와 그 뒤의 상태

| 시점 | 되돌리기 | 결과 |
| --- | --- | --- |
| A4 실패 | `restart-roles.sh rollback` | 옛 바이너리로 같은 컨테이너. 아무 기록도 바뀌지 않음 |
| A5 실패(정산 전) | 코디네이터를 M으로 | 지금과 같은 상태(R1 미계측으로 막힘) |
| A6 뒤에 M으로 되돌림 | 역할 `rollback` + 코디네이터 M | **M은 정산 형식을 모른다**: 정산 줄을 회차가 아닌 줄로 보고 R1을 원본대로 미계측으로 센다. 그래서 다음 전달을 409 `ASSIGNMENT_USAGE_UNMEASURED`로 거절하고 아무것도 시작하지 않는다(예행에서 확인, fail closed). 정산 줄과 경고 기록(`rate_limit_warnings`)은 장부에 남고, 새 빌드로 돌아오면 다시 확인되어 적용된다. 옛 역할 바이너리는 경고에서 다시 멈추므로 되돌린 채로 회차를 보내지 않는다 |
| 회차 중 | 하지 않음 | 재시작·재시도는 묶음 밖. 코디네이터 정지는 진행 중 회차를 기다린다(900초) |

### 9.5 사용자에게 요청할 최종 승인(한 번에)

1. 두 PR의 병합 SHA와 빌드 해시.
2. 허용 규칙 교체(U1).
3. 역할 바이너리 교체(A4)와 코디네이터 교체(A5), 각 되돌리기.
4. 정산 기록 추가(A6, 원본 보존).
5. 사용자 본인의 R1b 전달(U2: 예비 표식)과 그 뒤 R2·G1(모델 호출 3회).

## 부록 A. 한도 경고 판정의 근거, 정책 전후, 시험

### A.1 근거 자료

| 자료 | 확인한 것 |
| --- | --- |
| 공식 문서 | Agent SDK TypeScript 참조의 `SDKRateLimitEvent`: `status: "allowed" \| "allowed_warning" \| "rejected"`, `allowed_warning`일 때 `overageEnabled`는 사용 크레딧이 켜져 있는지. `overageStatus`·`isUsingOverage`·`unifiedWindows`는 공개 형에 없다 |
| CLI 2.1.322 실행 파일의 형 정의 | `status`, `overageStatus`, 각 창의 `status`가 모두 `allowed \| allowed_warning \| rejected`, `isUsingOverage`·`overageInUse`·`overageEnabled` 불리언, `rateLimitType` 6값, `errorCode`. `unifiedWindows` 설명(원문): 창의 status는 “allowed_warning once past a warning threshold, rejected at or past its limit. A rejected window alone does not mean requests are refused; the top-level status says whether they are.” 헤더 대응: `isUsingOverage` = 통합 상태 `rejected`이고 추가 사용 상태가 `allowed`/`allowed_warning` |
| R1 실제 사건 | `allowed_warning`, `rateLimitType: seven_day`, `utilization: 0.86`, `surpassedThreshold: 0.75`, `overageStatus: rejected`, `overageDisabledReason: out_of_credits`, `isUsingOverage: false`, 창 five_hour 0.04 `allowed`·seven_day 0.86 `allowed_warning` |

원문 발췌와 해시: [evidence/warning/rate-limit-sources.txt](evidence/warning/rate-limit-sources.txt).

### A.2 정책 전후

| 신호 | 전(46799b0까지) | 후(이 PR) |
| --- | --- | --- |
| `status: allowed`, `overageStatus: rejected`, `isUsingOverage: false` | 진행 | 진행 |
| `status: allowed_warning`, `overageStatus: rejected`, `isUsingOverage: false` | **멈춤**(실행 가드·관측·회차 기록·`check-run.py`), 상태 표시는 `available`(불일치) | **진행 + 경고 기록**(`rate_limit_warnings`, 상태 표시 `warning`, `check-run.py` `WARNING`) |
| 창 하나가 `allowed_warning`(최상위 `allowed`) | 보지 않음 | 진행 + 경고 |
| `status: rejected` | 멈춤 | 멈춤 |
| `overageStatus` 없음·`allowed`·`allowed_warning` | 멈춤 | 멈춤 |
| `isUsingOverage` 없음·`true`·불리언 아님 | 멈춤(상태 표시는 없음을 `unknown`) | 멈춤 |
| `overageInUse` 참, `overageEnabled` 참(사용 크레딧 켜짐), `errorCode` 있음 | 보지 않음 | 멈춤 |
| `rateLimitType`이 모르는 값, 또는 `overage`·`seven_day_overage_included`(추가 사용 거부와 모순) | 보지 않음 | 멈춤 |
| `unifiedWindows`가 객체 아님, 창이 객체 아님·상태 없음·모르는 값·`rejected` | 보지 않음 | 멈춤(거부된 창은 보수적으로: CLI는 최상위가 거부를 정한다고 하지만 다 쓴 창을 경고로 덮지 않는다. 상태 없는 창은 CLI가 읽지 못한 값일 수 있어 미확인) |
| 한 stream 안 경고 뒤 멈춤, 멈춤 뒤 경고 | 멈춤 | 멈춤(관측·기록·상태 표시 모두, 나중 경고가 지우지 못함) |
| `status` 없음·모르는 값, 읽을 수 없는 사건 줄 | 멈춤 | 멈춤 |
| 가드가 멈춘 회차의 사용량 | 미계측 | 미계측(그대로) |
| 미계측 회차 뒤 다음 호출 | 막힘 | 막힘(그대로) |
| 관측 유효 기간 | 7일 | 7일(그대로) |
| 시간·토큰·회차 상한, 추가 청구 | 승인값, USD 0 | 그대로 |

### A.3 판정이 쓰이는 곳

`allowed`·overage·`rate_limit`을 판정하는 곳을 모두 찾았다(`grep`). 하나의 판정 `budget::rate_limit_verdict`를 모두가 쓴다.

| 경로 | 바꾼 것 |
| --- | --- |
| 실행 가드 | `run.rs`의 `stream_stop` → `overage_check` → 판정: 경고는 멈추지 않음, 멈춤은 그 사건에서 프로세스 그룹 정지 |
| 사전 관측 검사 | `observation_check`: 관측 stream의 모든 사건이 판정 통과(경고 포함), 7일 |
| 공유 예산 검사·역할 서비스 상한 | `budget::next`·`Limits::for_next`(위 관측 검사; 역할 서비스 `serve.rs`도 이것). 예비 해제 규칙(`spare_releasable`)은 그대로: 한도 신호 멈춤은 예비로 풀리지 않음 |
| 회차 기록 | `caller`: 모든 사건을 판정, 멈춤은 `stop_required`, 경고는 `rate_limit_warnings` 배열 |
| 상태 표시 | `provider::classify`: `available`은 판정 통과일 때만, 경고는 `warning`, 다른 멈춤은 `unknown`으로 남아 나중 사건이 지우지 못함. 상태 응답 상세에 `warning` |
| 회차 후 검사 | `launcher/rate_limit_policy.py`(같은 규칙의 Python 사본), `check-run.py`가 그것을 씀, 기록의 경고와 stream의 경고가 같아야 함. 미계측 회차를 `measured`로 출력하던 요약 줄 고침 |
| 관측 사본 검사 | `obs-check.sh` [1]이 같은 Python 규칙을 씀(같은 디렉터리의 `rate_limit_policy.py`) |
| 바꾸지 않음 | `ieo-evidence/judge.rs`(한도 상태를 비차단 메모로만 남김), `dry-run-step8.sh`(일부러 깨는 시험 사건) |
| 계약·문서 | [런타임 계약](../../01-02/runtime-contract-20261112/Runtime_Contract_v0.md) revision 19(§13 항목 9, 새 §20 항목 31), [절차서](../step8-runbook-20261117/RUNBOOK.md) 첫 관측·멈춤 조건, [실행안](EXECUTION_PLAN.md) §6.2 5번·§12, [수동 절차](MANUAL_STEPS.md) A1 |

**배포된 사본은 아직 옛 규칙이다**(`$D/step8-46799b0`, `$D/step8-ops`, 58090의 M, 역할 컨테이너의 `ieo-supervisor`). 바꾸는 순서와 허용 규칙은 §5.

### A.4 시험(모델 호출 없음)

| 시험 | 내용 |
| --- | --- |
| `tests/rate_limit_policy.rs` | 공유 사례 31개(`tests/fixtures/rate_limit_cases.json`: 실제 R1 사건 포함)를 Rust 판정과 세 경로(가드·관측·기록 경고)로, 그리고 `python3`로 돌린 `rate_limit_policy.py`로 판정해 종류와 문구가 같음. 실제 R1 사건이 모든 경로에서 경고(옛 규칙은 멈춤), 7일 넘은 관측은 보류. 경고 앞뒤의 거부·추가 사용·누락은 관측 보류·상태 표시 유지. 보관 R1 stream 전체 재생(`IEO_S8R1_STREAM`; 원본은 저장소 밖 장부에 있고 사건 한 줄만 저장소에 둠) |
| `tests/caller.rs`(가짜 런타임) | 경고 회차: 결과까지 진행·계측·`stop_required: false`·경고 기록 1개·다음 호출 시작. 경고 뒤 `rejected`·`isUsingOverage: true`·`isUsingOverage` 누락: 가드 멈춤·미계측·`stop_required`·다음 호출 막힘(경고는 기록에 남음) |
| `tests/run_guard.rs` | 실제 R1 사건은 가드를 멈추지 않음, 그 뒤의 거부 사건은 4초 안에 멈춤 |
| 기존 시험 | `overageStatus: allowed_warning`(추가 사용이 덮는 경우)은 계속 멈춤. `status: allowed_warning`을 멈춤으로 보던 시험 한 줄은 새 결정에 맞춰 통과로 바꿈 |
| 재생 기록 | [evidence/warning/replay.txt](evidence/warning/replay.txt): 보관 stream 재생, 새·옛 `check-run.py`를 R1 기록에 읽기 전용으로 |
| 전체 | `cargo test --workspace --locked` **420 통과, 0 실패**([evidence/warning/test.txt](evidence/warning/test.txt)), `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과 |
| 변이 | 규칙마다 하나씩 22개(판정 15, 회차 기록 2, 상태 표시 3, Python 2), **22/22 죽음**([evidence/warning/mutants.txt](evidence/warning/mutants.txt)) |

시험 중 루트 파일 시스템이 가득 차(링커 SIGBUS) 한 번 거짓 실패가 났다. 내 빌드 디렉터리와 오래된 빌드 캐시를 지우고 debuginfo 없이 다시 빌드해 전부 통과했다.

## 부록 B. 정산 형식(`ieo-role-run-settlement/v0`)

정산 결정 §2.2~§2.4의 구현(정산 PR). 원본 기록은 바꾸지 않고 장부에 **별도 줄**을 더한다. 장부를 읽을 때마다(`caller::records`) 그 줄을 파일에서 다시 확인하고 대상 회차에 적용한다.

| 항목 | 내용 |
| --- | --- |
| 기록 | `format`, `settles`(대상 시도), `source: transcript_exit_accounting`, `policy: guard_stop_on_old_rule_warning`, `budget`·`role`·`session_id`, `original_stream_sha256`, `evidence`(장부 안 사본 `children/<시도>/settlement-transcript.jsonl`의 절대 경로·SHA-256·크기, 원본 경로·원본 SHA-256), `verification`(API 요청 수, 메시지 수, 항목 수, 종료 집계 일치, 현재 규칙으로 다시 판정한 신호, `uncovered: unlogged_in_flight_request_at_sigterm`과 `residual`: “transcript로 정산; 기록된 api-request 하나(requestRef = api-request id, 메시지와 1:1); 끝에 다음 요청의 prompt_snapshot이 있고 그 뒤 api-request 없음; SIGTERM 직전에 보내졌으나 기록이 사라진 요청은 클라이언트 쪽에서 배제할 수 없음; 공급자 쪽 청구는 보이지 않음”), `usage_run`, `lower_bound_not_used`(가드 하한, 쓰지 않음), `rule`, `decision`, `independent_check`(`{evidence, sha256, verdict}`: 검산 보고서 경로·SHA-256·판정, 판정은 `CONFIRMED` 또는 `CONFIRMED WITH CAVEAT`만), `settled_at`. `model_request: false`(회차가 아님) |
| 대상 조건(**이번 s8-r1만**) | 코드에 고정한 하나의 회차만 정산된다(정산 결정 §2.3, PR #90 검토 d971f59 L-4): 예산 `02-05`, 시도 `s8-r1`, 세션 `5c991c71-b6e2-4cfb-bd76-6bb18a36e090`, 보관 stream SHA-256 `252609c7…4c94`(`settlement::PIN_*`). 그 위에 구조 조건: 장부의 모델 회차, 가드가 한도 신호(`Overage`)로 멈춤, 미계측, 결과 사건 없음, 자식 회차 없음, 예비 아님, 멈춤 사유가 “결과 사건 없음”·“사용량 미계측”·“이 회차의 신호”뿐, 보관 stream 파일이 기록의 SHA-256과 같고 그 안의 한도 사건이 현재 규칙으로 모두 통과하며 하나 이상이 경고. 다른 회차는 구조가 같아도 정산되지 않고 예비도 해제하지 못한다. (시험용 합성 장부는 debug 빌드에서만 받는 시험 고정값을 쓴다. 배포하는 release 빌드는 위 하나만 안다) |
| 사용량 계산(`transcript_accounting`) | 모든 줄이 JSON이고 같은 세션, 자식(side-chain) 항목 없음, 같은 `message.id`는 마지막 항목 하나로(중복 제거), 서로 다른 메시지 수 = `api-request` 항목 수(1 이상), 마지막 `cost-state`의 `modelUsage` 합이 메시지 합과 네 항목 모두 같음. 가드의 관측 하한보다 작으면 거절(하한은 쓰지 않음). 계산은 넘침 검사(`checked_add`)를 하고, 넘치면 거절(L-7). 정산 사용량이 그 회차 자신의 한도(`limits.billable_room`·`cache_read_room`)를 넘으면 거절(L-6) |
| 적용 결과 | 회차는 **한 번** 센다(정산 줄은 회차가 아님). `usage.run`은 계산값(`measured`, `source`), 이전 값은 `before_settlement`. `stop_required`는 그대로 true, 사유는 “결과 사건 없음”만(R1은 실패로 남음) |
| 거절(장부 사용 불가) | 사본 변조(해시 다름), 같은 회차의 둘째 정산, 없는·나중의 회차, 다른 세션·stream·사용량·요청 수, 결정·독립 검산 참조 없음, 검산 판정이 확인이 아님, 단서(`uncovered`·`residual`) 누락, 다른 출처·정책, 상대 경로, 정정된 회차의 정산과 정산된 회차의 정정, 정산 필드를 스스로 가진 회차 줄. 사본은 장부 안에만 둔다: `<장부의 정규 경로>/children/s8-r1/settlement-transcript.jsonl`과 검산 보고서 사본 `independent-check.md`(검토 7b904de N-1). 장부 디렉터리는 정규 경로로 비교해 별칭·`..`로 적은 계획도 같은 판정(N-2), 장부 안(`children`, `children/s8-r1`, 파일)의 심볼릭 링크는 거절(L-1, N-3). 읽을 때마다 `evidence.original_sha256` = `evidence.sha256` = 사본의 실제 해시, 보고서는 장부의 사본이 기록된 SHA-256과 같아야 하며(없거나 다르면 거절; 원래 경로는 출처 기록일 뿐 다시 읽지 않음), `decision`은 `docs/poc/User_Decisions_2026-11-17_r1-settlement.md`만(L-2) |
| 예비 해제 범위 | `spare_releasable`: 가드가 멈춘 회차의 “결과 사건 없음”은 이 정책의 정산이 있을 때만 해제. 정산 없는 가드 멈춤, 실제 거부, 추가 과금, 상한, 다른 미계측은 계속 막음. 정산이 있어도 그 회차가 고정한 s8-r1이 아니면 해제하지 않는다 |
| 쓰는 명령 | `ieo-role-call settle --plan <plan.json> --attempt s8-r1 --transcript $D/s8-evidence/s8-r1-transcript.jsonl --original-path <역할 볼륨의 원본 경로> --original-sha256 806746fd…dc68 --decision docs/poc/User_Decisions_2026-11-17_r1-settlement.md --independent-check docs/poc/reports/tasks/02-05/step8-plan-20261117/evidence/r1-usage-verification.md --independent-verdict "CONFIRMED WITH CAVEAT" [--dry-run]`(보고서 SHA-256은 명령이 계산해 기록). 장부 잠금 아래, 진행 중 호출 없음, 사본 해시가 원본과 같을 때만, 다시 읽어 적용될 때만 한 줄을 더한다. 어떤 거절에도, `--dry-run`에도 아무것도 쓰지 않는다(드라이런은 주어진 증거 사본을 직접 읽고 임시 파일도 만들지 않는다: L-3). 두 사본(전사·보고서)은 새로 만들 때만 쓰고(`create_new`, 0600), 그 경로나 그 위 디렉터리가 심볼릭 링크이거나 다른 파일이면 거절하며, 실패하면 만든 사본을 지운다. **실제 장부에는 아직 실행하지 않았다**(장부 사본에서 `--dry-run`만: 쓴 것 없음) |
| 재개 스크립트 | `check-run.py`는 `EFFECTIVE_RECORDS`(`ieo-role-call records`가 출력한 유효 기록)로 집계해 정산된 s8-r1을 정산으로 센다(장부 줄 그대로 읽으면 R1 원본을 미계측 실패로 센다: 예행에서 확인). 재개 순서와 명령은 §9. 정산 줄에는 `attempt_id`가 없으므로 장부 줄을 직접 읽는 곳(`check-run.py`, `dry-run-step8.sh`의 변조 사본)은 회차 줄(`format: ieo-role-run/v0`)만 센다(PR #90 검토 d971f59 L-5). 정산 줄에는 `attempt_id`가 없으므로 장부 줄을 직접 읽는 곳(`check-run.py`, `dry-run-step8.sh`의 변조 사본)은 회차 줄(`format: ieo-role-run/v0`)만 센다(PR #90 검토 d971f59 L-5) |
| 시험 | `tests/settlement.rs`: 계산(중복 제거·종료 집계 불일치·요청 없음·다른 세션·side-chain·깨진 줄·숫자 아님), 정산 뒤 한 번 집계·실패 유지·예비 해제, 변조·중복·다른 시도·세션·stream·참조 누락·출처·사본 해시·상대 경로·요청 수, 근거 부족(아무것도 쓰지 않음), 특례 범위(실제 거부, 경고 뒤 거부, 추가 사용, 경고 없음, 시간·토큰·cache_read 상한, 가드 아님, 다른 사유, 계측됨, 결과 있음, 자식, 예비, 다른 stream: 정산 안 되고 예비도 해제 못함), CLI(드라이런·해시 다름·정상). PR #89 검토 L-1~L-3 보강(stream 단위 Rust=Python, 숫자 0/1, 나중 `allowed`가 경고 지움). 독립 검산 판정·SHA-256·근거 경로와 단서(`uncovered`·`residual`)가 없거나 바뀌면 거절. 전체 428 통과·0 실패, 변이 36개 중 35 죽음·1 동등(W02: 쓰기 전 원본 해시 확인은 `build`가 같은 조건을 다시 확인해 중복). 검토 d971f59 정정: 고정값(예산·시도·세션·stream 해시 각각 다르면 거절, 다른 시도·세션은 정산도 예비 해제도 안 됨), 사본 위치·심볼릭 링크(읽기와 쓰기), 사본·원본 해시 불일치, 보고서 변경·삭제, 다른 결정, 회차 한도 초과·한도 없음, 넘침, 바뀐 stream 파일, 드라이런이 장부에 아무것도 만들지 않음. 변이: 정정 10개 모두 죽음, 옮겨진 코드의 기존 변이 다시 실행해 죽음(V03·W02는 동등: 실제 파일 해시와 비교하므로 형식 검사·쓰기 전 해시 검사가 중복) |
