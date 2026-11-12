# 02-01 · 명령 실행·증거 수집·비용 계측기

| 항목 | 기록 |
| --- | --- |
| 계획·작업·회차 | v1.3.3-A1 · 02-01(구현, T03) · run `collector-20261112` |
| 수용 기준 | `poc-acceptance-v1.3.3` rev 4 `02-01.accept`(장부 02-01 행과 같음) |
| 선행 | 01-03·01-06 검수 완료(원장 snapshot `v1.3.3-EXEC-0011`, main `c0c5dc4`에서 착수 확인) |
| 산출물 | Rust crate [`crates/ieo-evidence`](../../../../../../crates/ieo-evidence)(라이브러리 + CLI `ieo-evidence`) · 기록 형식 [`contracts/evidence-record-v0.schema.json`](../../../../../../contracts/evidence-record-v0.schema.json) · 생성기 시험 보강 [`scripts/test_render_progress.py`](../../../../scripts/test_render_progress.py) · 런타임 계약 revision 7(§11 항목 7) |
| 상한 | F05 구현 14,400초. 시작 2026-11-12T11:34:57Z, 이 보고 작성 시점 12:36Z(약 3,700초). 검토·교정은 F04(4회·3회) |
| 모델 호출 | **0회**. 02-01에 동결된 모델 요청 예산이 없고 01-04 예산은 전용하지 않는다. 실제 모델 출력은 01-04에 보존된 실제 stream으로 대조했다(§3) |
| 상태 | rev 2. 독립 검토 2df3c35 reject(P2-1~P2-4) 교정 → 재검토 대기(02-01 검토 1/4·교정 1/3). 이 보고는 claim이다 |

## 1. 구현 결과

명령 1회 = 증거 디렉터리 1개 `<out>/<task_id>/<attempt_id>/<command_id>/` (`stdout`, `stderr`, `record.json`). 같은 ID로 다시 실행하면 거부하고 덮어쓰지 않는다.

| 요소 | 동작 |
| --- | --- |
| 실행(`run.rs`) | 빈 환경 + 허용한 변수 **이름**만 전달(값은 기록하지 않음), 별도 프로세스 그룹, 시간 상한에서 그룹에 TERM → 유예 뒤 그룹에 남은 구성원이 있으면(지도자가 이미 끝났어도) KILL. 시간 초과 없이 끝났는데 그룹원이 남아 있으면 바로 KILL하고 `stray_group_killed`로 기록(판정 사유에 나열, 성공은 막지 않음). 출력은 읽는 대로 모아 두므로, 그룹 밖 자손(`setsid`)이 출력 관을 계속 잡고 있어도 이미 읽은 바이트를 보존하고 `output_incomplete`로 표시해 `incomplete`로 판정한다. 증거 디렉터리는 배타 생성한다. 기동 실패도 기록을 남긴다 |
| 원본 로그·digest | stdout·stderr를 **비밀 가림 뒤** 저장하고 크기·SHA-256 기록. 필수 산출물은 존재·크기·SHA-256. 시각은 UTC 밀리초, 소요는 단조 시계 |
| 비밀 | `--secret-file`의 값(메모리에만)과 그 SHA-256, `sk-ant-…` 형태를 stdout·stderr·argv에서 가린다. 가린 횟수와 “비밀 값이 보였는지”만 기록. 8바이트 미만, UTF-8이 아닌 값, JSON이 이스케이프하는 문자(제어 문자·`"`·`\`)를 가진 값은 stream-json 안에서 가림을 보장할 수 없어 거부한다. `--env`로만 넘긴 값은 비밀로 알려 주지 않으면 `sk-ant-` 형태만 가린다 |
| 판정(`judge.rs`) | **종료 코드 0만으로 성공이 아니다.** 결과 사건(모델 요청 시)·필수 산출물·필수 표식 중 하나라도 없으면 `incomplete`. 기동 실패·신호·시간 초과·비0 종료·`is_error`는 `failed`. 의도한 거부/중단 시험은 기대(`nonzero_exit`·`timeout`·`no_result`·`result_error`)와 필수 표식으로 `expected_matched`/`expected_not_matched`. 사유는 모두 나열하고 한도 신호(`rate_limit_event`가 allowed 아님)도 사유에 남긴다. CLI 종료 코드: 성공 0, 그 밖 1, 요청·기록 오류 2 |
| 사용량(`stream.rs`) | 결과 사건 = num_turns ≥ 1인 마지막, 없으면 마지막(01-04 PR #16 규칙). `parent`=`result.usage`, `session`=`result.modelUsage` 합(자식 포함), 자식 = Agent tool_use마다 `tool_use_result.usage`, **`run`(회차 몫, 합산 대상)** = 새 세션이면 `session`, 재개면 `session` − 재개 기준 회차의 `session`(§3). 네 항목(input·output·cache_read·cache_creation) 중 하나라도 없으면 그 값은 `unmeasured`+사유. 결과 사건이 없으면 회차·부모·끝나지 않은 자식이 모두 미계측이고, 메시지별 스트리밍 사용량은 `partial_observations`로 남기되 합산하지 않는다. CLI 내부·공급자 재시도는 `internal_retries: unmeasured` |
| 귀속 대조 | `run` = 부모 + 자식이면 `consistent`, 다르면 `mismatch`와 차이(압축 등 귀속 안 된 호출)를 기록. 재개 회차가 압축 사건 없이 다르면 기준 회차가 잘못된 것으로 보고 `run`을 미계측으로 철회 |
| 합산(`aggregate.rs`) | 부모·자식·실패·중단·재시도(`retry_of`)·검토(`role=review`) 회차를 모두 포함. `completeness`: 모든 모델 회차가 계측이면 `complete`, 하나라도 미계측이면 `incomplete`(합은 “계측된 회차의 합”이며 총량이 아님, 미계측 회차 목록 첨부), 모델 회차가 없으면 `no_model_runs`. 계측된 회차가 없으면 합은 **null**(0 아님). 역할별 합도 같은 규칙. 비용 합은 모든 모델 회차가 회차 몫 비용을 가질 때만 |
| 형식 | `ieo-evidence-record/v0` JSON Schema. 스키마 자체가 “succeeded인데 exit≠0”, “unmeasured인데 tokens 있음”, 모델 요청인데 세션 방식 없음을 거부한다 |
| 재시도 | 계측기는 자동 재시도하지 않는다. 재시도는 새 attempt이고 `retry_of`로 잇는다 |

재사용: 01-04 하네스(`probes/01-04/probe2.py`)의 결과 사건 선택 규칙·결과 누락 정지·표식 판정·비밀 가림 방식을 Rust로 옮겼다. digest 직렬화가 필요한 곳이 없어 `ieo-spec` digest는 쓰지 않았다(파일 SHA-256만). 진행 원장·화면은 01-06 생성기를 그대로 쓰고 시험만 보강했다.

**정하지 않은 것.** 토큰 “총량”을 네 항목 중 무엇의 합으로 셀지(L01 누적 한도·L02 EARLY-01 3,000,000)는 이 작업이 정하지 않는다. 계측기는 네 항목을 따로 낸다. 정의는 L01/L02 동결 때 결정할 항목으로 남긴다. `cost_usd_reported`는 CLI가 보고한 추정값이며 구독 자격이라 청구액이 아니다.

## 2. 수용 기준별 검증

| 수용 기준(02-01.accept) | 근거 | 범위 |
| --- | --- | --- |
| 명령·종료 코드·원본 로그·SHA/digest·모델 사용량을 정해진 형식으로 수집 | 스키마 + `every_record_matches_the_schema`(정상·실패·시간 초과·실제 stream·잘린 stream 기록 모두 스키마 통과, 위조 기록 거부), `normal_success_records_everything`(stdout·stderr·산출물 SHA-256 대조). 실제 실행 10건 [runs/](evidence/runs/02-01/rt1) | 고정 입력 + 실제 프로세스 |
| 오류를 숨기지 않는다 | `launch_failure_still_writes_record`, `nonzero_exit_is_failure_with_stderr_kept`, `timeout_*`, `exit_zero_without_required_evidence_is_incomplete`(사유 2개 모두), `model_run_exit_zero_without_result_event_is_incomplete`, `result_is_error_is_failure_unless_expected`, `stray_background_process_does_not_hang_the_collector`. 실제 실행: 기동 실패·산출물 누락이 `failed`·`incomplete`로 남고 CLI가 1을 반환([runs-cli.txt](evidence/runs-cli.txt)) | 고정 입력 + 실제 프로세스 |
| 부모·자식의 실패/중단/검토 사용량 합산 | `aggregate_includes_children_failures_retries_and_reviews`(성공·실패·재시도·검토 4회차, 자식 4/4 계측, 합 = 4 × 회차값), `model_run_real_stream_measures_parent_child_session`(01-04 T 시도 23: 세션 = 부모 4/157/7907/2886 + 자식 2/9/0/844), 재생 요약 [replay-0104-summary.json](evidence/replay-0104-summary.json) | 실제 stream 고정 입력 |
| 미계측을 0으로 대체하지 않음 | `result_without_usage_fields_is_unmeasured`(합 null), `model_run_exit_zero_without_result_event_is_incomplete`, `intentional_model_abort_expects_no_result`, `resumed_session_usage_is_split_from_the_cumulative_total`(기준 없는 재개·다른 세션 기준·누적 감소 → 미계측), `wrong_resume_base_is_withdrawn_and_compaction_is_counted` | 실제 stream 고정 입력 + 변형 |
| 진행 원장은 state.json, 01-06 생성기로 같은 snapshot_id/원장 digest의 STATUS/TASKS/RESULTS | `test_render_progress.py` 새 검사 `same_snapshot_and_digest_in_all_views`(STATUS·TASKS·RESULTS·RESUME 모두 같은 snapshot·digest), 기존 `committed_views_match_ledger`·`render_idempotent` 등 28건 PASS. 이 작업의 검수 기록은 보고 PR에서 state.json에 남기고 생성기로 화면을 만든다 | 임시 사본 + CI |
| 주계획 73·후속 12를 따로, 합산 진행률 없음 | 기존 `main_and_followup_counted_separately` + 새 `no_combined_progress_rate`(85 분모·합산 행 없음) | 임시 사본 + CI |

시험 수: `ieo-evidence` 32건(단위 5·통합 27), 작업공간 전체 `cargo test` 110건 통과, clippy `-D warnings` 통과, 생성기 시험 28건 PASS. CI에 01-04 실제 stream 재생 대조를 추가했다.

## 3. 실제 데이터 대조와 발견

**01-04 실제 stream 재생.** 01-04에 보존된 35개 실제 stream(가림 적용본)을 계측기 `run --model-request -- cat stdout.jsonl`로 재생하고 별도 Python 판독([crosscheck_0104.py](crosscheck_0104.py))과 대조했다: **35/35 일치**([crosscheck-0104.txt](evidence/crosscheck-0104.txt)). 재생 기록의 stdout SHA-256은 35개 모두 01-04 원본 파일과 같다(가림 0회). 세션 방식은 원래 argv의 `--resume`으로(새 25·재개 10), 재개 기준은 같은 세션의 직전 재생 회차로 정했다. rev 1은 first-probes의 argv가 JSON 문자열로 저장된 것을 읽지 못해 P2c를 새 세션으로 재생했다(검토 P2-1, 스크립트 교정 후 재생성). 자식(Agent)·압축이 없는 30개 회차는 회차 몫이 자기 `result.usage`와 정확히 같았다(뺄셈과 독립적인 의미 대조).

**발견: 재개한 세션의 `modelUsage`는 세션 누적이다.** 첫 구현은 회차마다 `modelUsage`를 더했는데 재생 대조가 9개 재개 회차에서 어긋났다. 원본을 보면 N2의 `modelUsage`(6/147/4790/500) = N1(2/135/2224/342) + N2의 `result.usage`(4/12/2566/158)이고, R1→R2, G3a→G3b, N2→N3도 같다. 회차마다 `modelUsage`를 더하면 재개할 때마다 앞 회차를 다시 센다. 또 L3(`/compact`)는 `result.usage`가 모두 0이고 압축 사용량(2020/1354/2761/1019)은 누적 증가분에만 있다. 그래서 회차 몫을 “새 세션 = 누적, 재개 = 누적 − 실제 재개 기준 회차 누적”으로 바꾸고 세션 방식을 모델 요청의 필수 입력으로 했다. 이 규칙을 런타임 계약 revision 7 §11 항목 7로 넣었다(02-04 supervisor가 기준 회차를 넘겨야 함).

**기준 회차를 정할 수 없는 경우.** 01-04 N4a·N4b는 같은 세션을 일부러 동시에 재개했다. N4b는 직전 시각 회차(N4a)를 기준으로 하면 누적이 줄어 미계측이다. L2는 N4b를 기준으로 하면 값이 커지지만 실제로는 N4a 분기를 이었기 때문에 L2 자기 사용량과 다르다. 압축 사건 없이 회차 몫이 부모+자식과 다르므로 미계측으로 철회한다(N4a를 기준으로 주면 자기 사용량과 일치, 시험으로 고정). 재생 합계는 그래서 `incomplete`(35 중 32 계측, 미계측 P2c·N4b·L2 명시)이다. 계약 §5-5(세션당 단일 실행)를 지키면 기준은 하나로 정해진다.

**부동소수 정확도.** 첫 재생에서 비용 4건이 1 ulp 달랐다(serde_json 기본 파서). `float_roundtrip`을 켜 35건 모두 원문 값과 같다.

**P2c(01-04 시도 05).** 재개(`--resume`) 회차이고 요청 전 실패(`error_during_execution`, num_turns 0)라 `modelUsage`가 비어 있다. 재개 회차는 `result.usage`(이번 회차 부모분)로 세션 누적을 대신할 수 없으므로 **미계측**이다. rev 1 보고의 “관측된 0” 서술은 세션 방식을 잘못 읽은 결과였고 철회한다. 새 세션에서 `modelUsage`가 비고 CLI가 `result.usage`를 0으로 명시한 경우만 출처 문자열(“result.usage + 자식 … modelUsage 비어 있음”)을 단 계측값이 된다(01-04에는 이런 회차가 없다).

**01-04 판정 영향: 없음.** 01-04 예산은 시도 수·시간으로 셌고 토큰 합을 판정에 쓰지 않았다. T의 “결과 modelUsage는 자식을 포함한다”는 새 세션(시도 23)에서 그대로 성립한다. 01-04 증거는 바꾸지 않았다.

## 4. 고정 입력과 실제 런타임의 구분

| 범위 | 확인한 것 | 확인하지 않은 것 |
| --- | --- | --- |
| 고정 입력(시험) | 01-04 실제 stream 7개 고정본(T23·R1·R2·N4a·N4b·L2·L3)과 변형(결과 누락·usage 누락·modelUsage 제거·is_error·귀속 불일치), 합성 셸 명령 | — |
| 실제 프로세스(이 호스트) | `git rev-parse`, `cargo test`(32건), 생성기 시험, `claude --version`(모델 요청 아님), 실제 쓰기 거부(`/proc/version`, 기대 일치), 실제 시간 초과(TERM 2,034 ms, 기대 일치), TERM을 무시하는 그룹원(유예 뒤 KILL, 남은 프로세스 없음), 그룹 밖 자손이 출력 관을 잡은 경우(이미 읽은 출력 보존·`incomplete`), 기동 실패, 산출물 누락, 세션 방식 없는 모델 요청 거부, 덮어쓰기 거부([runs-summary.json](evidence/runs-summary.json)) | — |
| 실제 모델 실행 | 01-04의 실제 출력 35개를 재생해 형식·값 대조 | **이 계측기로 새 `claude -p`를 실행하지 않았다**(모델 호출 0). 실시간 stream의 관 처리·TERM 시 CLI 동작·자식 중단 중 계측은 실제 실행으로 확인하지 않았다. 02-04 supervisor 시험 또는 승인된 예산의 회차에서 확인한다 |

## 5. 알려진 한계

1. 재개 기준 오류 탐지는 “누적 감소” 또는 “압축 없이 부모+자식과 불일치”일 때만 된다. 압축이 있는 재개에서 기준이 틀리면 귀속 불일치(`mismatch`)로만 남는다. 세션당 단일 실행(계약 §5-5)이 전제다.
2. `setsid`로 그룹을 벗어난 자손은 죽이지 못한다. 출력 관을 잡고 있으면 `output_incomplete`로 드러나지만, 관을 닫고 떠난 자손은 기록에 남지 않는다(역할 컨테이너 경계에서 처리할 일).
3. 산출물은 존재·SHA-256만 본다. 내용 판정은 표식 또는 각 실험의 독립 기대값(02-02, L03)으로 한다.
4. 표식은 가림 뒤 stdout에서 찾는다. 비밀 값과 같은 표식은 찾지 못한다(의도).
5. 증거 기록의 `cwd`·argv에는 호스트 경로가 남는다(비밀 아님).
6. 비용은 CLI 보고 추정이다. 공급자 쪽 요청 수와 CLI 내부 재시도는 관측하지 못한다(미계측으로 표시).

## 6. 검토 처분

[검토 2df3c35](reviews/review-pr21-2df3c35.md) reject: P2 4건 교정, P3 9건 중 2건 교정·7건 기록.

| 지적 | 처분 |
| --- | --- |
| P2-1 교차 대조 스크립트가 first-probes argv(문자열)를 못 읽어 P2c를 새 세션으로 재생 | 스크립트 교정, 증거·보고·result.json 재생성(32/35 계측, P2c 미계측) |
| P2-2 그룹 밖 자손이 출력 관을 잡으면 출력을 버리고 성공 | 읽은 바이트 보존, `output_incomplete` → `incomplete`. 시험 `output_held_by_detached_descendant_is_kept_and_flagged`, 실제 실행 `detached-holds-pipe` |
| P2-3 시간 초과 뒤 TERM 무시 그룹원 생존 | 지도자 종료 뒤에도 유예 끝에 그룹 KILL. 시험 `group_member_ignoring_term_is_killed_after_grace`, 실제 실행 `timeout-term-ignored` |
| P2-4 JSON 이스케이프 문자를 가진 비밀 미가림 | 그런 값 거부(+UTF-8 아님 거부). 시험 `rejects_secrets_that_json_would_escape` |
| P3-1 증거 디렉터리 확인·생성 사이 경쟁 | 배타 생성으로 교정 |
| P3-5 남은 그룹원 KILL이 사유에 없음 | 사유에 나열(비차단). 시험 `leftover_member_after_normal_exit_is_killed_and_recorded` |
| P3-2 subtype 무시 · P3-3 nonzero/no_result의 무관한 실패 일치·표식 stdout만 · P3-4 모델 요청 timeout 기대 불가 · P3-6 마감 직후 종료 경쟁 · P3-7 summarize 중복 입력 · P3-9 계약 rev 7의 02-04 의무·`--env` 비밀 | 원장 미해결로 기록. 담당 02-02(외부 대조에서 판정 규칙 보강: P3-2·3·4·6), 02-04(P3-9, 기준 회차 전달·`--secret-file` 사용), 09-01 또는 계측기 다음 수정(P3-7) |
| P3-8 “도구·자식·압축이 없는 30개” 문구 | 문구 교정(자식·압축) |

## 7. 다음

1. 독립 검토 → 필수 CI → 승인 head 조건 병합 → main CI → 원장 보고 PR(02-01 검수 시에만 8→9/73).
2. 이 계측기를 쓰는 다음 작업: 02-02(독립 기대값으로 수집기·판정기 외부 대조), 03-01, 02-04(계약 revision 7 항목 7의 기준 회차 전달).
