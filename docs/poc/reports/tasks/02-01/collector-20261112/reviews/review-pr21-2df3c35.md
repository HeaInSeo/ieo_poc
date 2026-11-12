# PR #21 독립 검토 — 02-01 명령 실행·증거 수집·비용 계측기

| 항목 | 값 |
| --- | --- |
| 검토 대상 | HeaInSeo/ieo_poc PR #21, 브랜치 `task/02-01-evidence` |
| 검토 SHA | `2df3c35ed697b182c59058673087ed7712691aea` (로컬 worktree `git rev-parse HEAD`로 확인, worktree는 수정하지 않음) |
| 비교 기준 | `git diff origin/main...2df3c35` (커밋 2개: 91d570e 코드, 2df3c35 보고·증거·계약 rev 7) |
| 판정 | **reject (변경 요청)** — P1 없음, P2 4건(병합 전 수정 필요), P3 9건 |

## 1. 판정 요약

계측기의 핵심 설계(종료 코드 0만으로 성공 아님, 의도한 거부/중단의 기대 판정, `measured`/`unmeasured` 구분, 재개 회차 몫 = 세션 누적 − 기준 회차 누적, 미계측 합 null)는 올바르고 01-04 원본 stream으로 독립 확인했다. 재개 세션 `modelUsage` 누적 주장도 원본에서 그대로 성립한다(§3.3).
다만 (a) 교차 대조 스크립트의 세션 방식 판정 버그 때문에 P2c 재개 회차가 "새 세션"으로 재생되어, 보고서·요약 증거가 미계측이어야 할 회차를 **계측값 0**으로 적고 "35 중 33 계측"이라고 주장한다. (b) 출력 관을 잡은 분리 자손이 있으면 이미 받은 stdout이 **빈 파일로 대체되고 succeeded**로 판정된다(오류 은닉). (c) 시간 초과 시 TERM을 무시하는 그룹원이 KILL 없이 살아남는다. (d) JSON 이스케이프되는 문자를 가진 비밀은 stream-json에서 가려지지 않는다. 이 4건은 수용 기준("오류를 숨기지 않는다", "미계측은 0으로 대체하지 않는다")과 상시 요구("비밀 값 비기록")에 직접 닿으므로 병합 전 수정이 필요하다.

## 2. 발견 사항

### P2-1. 교차 대조의 세션 방식 판정 버그 → P2c 재개 회차가 "계측 0"으로 기록, 보고 수치 오류

- 위치: `docs/poc/reports/tasks/02-01/collector-20261112/crosscheck_0104.py:62-63`, 결과 `evidence/replay-0104/01-04-replay/04-05-P2c-development/replay/record.json`, `evidence/replay-0104-summary.json`, `evidence/crosscheck-0104.txt`, `REPORT.md:48`, `REPORT.md:52`, `REPORT.md:56`, `result.json`(observed)
- 원인: `argv = " ".join(str(a) for a in meta["argv"])`. first-probes(시도 01~09)의 `meta.json`에서 `argv`는 **리스트가 아니라 JSON 문자열**이다. 문자열을 순회하면 글자마다 공백이 끼어 `" --resume "`가 절대 일치하지 않는다.
- 확인: 35개 meta를 직접 읽어 보니 first-probes 9개는 `str`, remaining 26개는 `list`. `05-P2c-development`의 argv에는 `--resume 73305c11-…`(04-P1-guardrail 세션)이 있다. 스크립트 출력은 `{"new": 26, "resume": 9}`이지만 실제는 new 25 · resume 10.
- 결과: P2c 재생 기록은 `session_mode: "new"`, `run`=`measured` 0/0/0/0(출처 "새 세션: result.usage + 자식 … (modelUsage 비어 있음)"). 같은 stream을 올바르게 `--session resume --resume-base <04-P1 재생 기록>`으로 직접 돌리면 계측기는 `run`·`session` 모두 `unmeasured`("result.modelUsage 비어 있음; 새 세션이 아니어서 부모·자식 합으로 대신할 수 없음")를 낸다. 즉 계측기 로직은 맞고, 증거 생성 입력이 틀려서 **미계측이어야 할 회차가 0으로 기록**되었다.
- 보고서 주장과의 차이: REPORT §3 "세션 방식은 원래 argv의 `--resume`으로 정했다"(거짓, 9개 stream에서 판정 불능), "재생 합계 incomplete(35 중 33 계측, 미계측 N4b·L2)"(실제로는 32 계측, P2c 포함 3건 미계측), P2c 단락 "관측된 0이고 대체한 0이 아니다"(재개 회차이므로 계약 rev 7 자체 규칙상 미계측). 대조 스크립트의 기대값도 같은 잘못된 `mode`로 계산되어 35/35 "일치"가 이 오류를 잡지 못했다(자기 일관적 오류). CI에 같은 스크립트가 들어가므로 CI도 잡지 못한다.
- 요구: `meta["argv"]`가 문자열이면 `json.loads`로 풀어 판정, 재생·요약·대조표·REPORT §3·result.json 수치 재생성. P2c 단락은 "재개 회차, modelUsage 비어 있음 → 미계측"으로 정정(또는 사용자 결정으로 다른 규칙을 정할 것).

### P2-2. 분리된 자손이 출력 관을 잡고 있으면 수집된 stdout/stderr가 버려지고 succeeded로 판정

- 위치: `crates/ieo-evidence/src/run.rs:94-102`(`spawn_reader`가 `read_to_end` 완료 후에만 버퍼 전송), `run.rs:233-243`(`recv_timeout` 실패 시 `unwrap_or_default()` → 빈 Vec), `judge.rs` 전체(`stray_group_killed`·수집 손실을 판정에 반영하지 않음)
- 재현(스크래치 사본, 커밋 SHA 빌드):
  `ieo-evidence run … -- sh -c 'echo hello-marker; setsid sleep 20 & exit 0'`
  → `stdout.bytes = 0`(sha256 = 빈 문자열 해시), `stray_group_killed: true`, `kill_sent: true`, `judgment.status = succeeded`, `reasons = []`, CLI 종료 코드 0. 이미 출력된 `hello-marker`는 어디에도 남지 않았다.
- 영향: "원본 로그 수집"과 "오류를 숨기지 않는다" 위반. 원본 로그가 잘렸다는 사실이 기록 어디에도 없고, 빈 stdout의 SHA-256이 원본 digest처럼 기록된다. 모델 요청이면 결과 사건이 사라져 incomplete가 되긴 하지만 `partial_observations`까지 사라진다. Claude CLI가 남긴 백그라운드 셸이 setsid 등으로 그룹을 벗어나면 실제로 일어날 수 있다. REPORT §5-2는 "setsid 자손은 죽이지 못한다"만 적고 출력 손실·성공 판정은 적지 않았다.
- 요구: 읽은 바이트를 공유 버퍼에 점진 저장해 손실 없이 보존하고, EOF에 도달하지 못한 경우 `capture_complete: false` 같은 명시 필드(스키마 포함)와 판정 사유를 남기며 `Success` 기대에서는 성공으로 세지 않을 것. codex 봇 지적(run.rs:241)과 같은 결함이며 타당하다.

### P2-3. 시간 초과 때 그룹 리더가 TERM으로 죽으면 TERM을 무시한 그룹원에게 KILL이 가지 않음

- 위치: `crates/ieo-evidence/src/run.rs:190-214`(KILL은 리더가 유예 뒤에도 살아 있을 때만), `run.rs:231-243`(출력 관을 잡지 않은 그룹원은 정리 대상 아님)
- 재현: `--timeout-s 1 --grace-s 1 --expect timeout -- sh -c '(trap "" TERM; exec sleep 47) >/dev/null 2>&1 & sleep 60'` → `timed_out`, `TERM true · KILL false`, `expected_matched`, 그러나 수집기 종료 뒤 `pgrep`에 `sleep 47`이 **살아 있음**(같은 프로세스 그룹, setsid 아님). 기록에는 생존 프로세스 흔적이 없다.
- 영향: REPORT §1 "시간 상한에서 그룹에 TERM → 유예 뒤 KILL" 주장과 다르다. 중단 시험이 기대 일치로 판정되면서 중단되지 않은 프로세스가 남는다.
- 요구: TERM을 보냈다면 리더 종료와 무관하게 유예 뒤 그룹 KILL(또는 `kill -0 -PGID`로 생존 확인 후 KILL)하고 그 사실을 기록. 시험 `timeout_escalates_to_kill_when_term_ignored`(tests/collector.rs:143)는 리더 자신이 TERM을 무시하는 경우만 덮는다.

### P2-4. JSON 이스케이프되는 문자를 포함한 비밀은 stream-json에서 가려지지 않음

- 위치: `crates/ieo-evidence/src/redact.rs:424-445`(`Redactor::new`), `redact.rs:451-478`(`mask`: 원문 바이트 그대로 비교)
- 시나리오: `--secret-file`의 값에 `"`, `\`, 제어 문자(내부 개행 등)가 있으면 stream-json은 `\"`, `\\`, `\n`, `\u00XX`로 인코딩한다. 원문 바이트 일치가 없으므로 치환 0회, `secret_value_seen: false`인 채 stdout 파일에 저장되고, JSON 디코딩하면 비밀이 복원된다. 비밀 값과 SHA-256 형태 모두 가리지만 JSON 표현은 다루지 않는다.
- 평가: PoC의 실제 비밀(OAuth 토큰, `sk-ant-…` base64url 문자)에서는 일어나지 않으므로 현 증거에는 유출이 없다(커밋된 diff에서 `sk-ant-` 실값·토큰 형태 검색 결과 시험용 가짜 값뿐). 그러나 계측기는 범용 `--secret-file`을 받고 "비밀 값은 증거 파일에 닿지 않는다"(lib.rs 문서)를 무조건적으로 주장하며, 못 가린 사실도 숨긴다. codex 봇은 P1로 올렸으나 현 사용 범위에서 실제 유출이 없으므로 P2로 본다.
- 요구(가장 단순한 것): JSON 이스케이프가 필요한 문자(`"`·`\`·U+0000–U+001F)를 포함한 비밀은 `Redactor::new`에서 거부하거나, 그 JSON 이스케이프 표현도 함께 치환할 것. 시험 1건 추가.

### P3 (비차단, 기록)

| # | 위치 | 내용·시나리오 |
| --- | --- | --- |
| P3-1 | `run.rs:142-148` | codex 봇 지적(TOCTOU): `dir.exists()` 확인과 `create_dir_all` 사이에 같은 ID 동시 실행이 둘 다 통과해 같은 파일을 덮어쓸 수 있다. 계약 §5-5(세션당 단일 실행)·단일 supervisor 전제에서는 일어나지 않지만 "덮어쓰지 않는다" 주장은 무조건적이다. 마지막 디렉터리를 `fs::create_dir`(배타)로 만들면 해결. |
| P3-2 | `judge.rs:343-349` | 결과 사건의 `subtype`을 보지 않는다. `subtype`이 `success`가 아닌데 `is_error`가 false/없음이면 succeeded. 01-04 35개 stream에는 그런 사례가 없다(직접 확인). 최소한 사유로 남기기를 권함. |
| P3-3 | `judge.rs:385-391` | `nonzero_exit`·`no_result`는 무관한 실패(예: 명령 없음 127, 인증 실패로 결과 없음, exit 0 무출력)로도 `expected_matched`가 된다. 표식으로 구분해야 하는데 표식은 stdout만 검색한다(stderr의 거부 문구는 못 씀). rt1 `deny-write`는 셸이 stdout에 `WRITE_DENIED`를 찍어 우회했다. 문서화 또는 stderr 표식 지원 권함. |
| P3-4 | `judge.rs:353` | 모델 요청 + `--expect timeout`은 "필수 결과 사건 없음"이 blocking이라 항상 `expected_not_matched`. 모델 중단은 `no_result`를 써야 한다는 점을 CLI 도움말/보고에 명시 권함. |
| P3-5 | `run.rs:231-243`, `judge.rs` | 명령 종료 뒤 남은 그룹원을 죽인 경우(`stray_group_killed: true`)도 판정 사유에 나오지 않는다(기록 필드에는 있음). |
| P3-6 | `run.rs:196-204` | try_wait 직후 자연 종료와 마감이 겹치면 이미 끝난 명령에 TERM을 보내고 `timed_out`으로 기록할 수 있는 경쟁(20 ms 창). 실무 영향 작음. |
| P3-7 | `bin/ieo-evidence.rs:135-166`, `aggregate.rs:324` | `summarize`가 (task, attempt, command) 중복을 거르지 않는다. 디렉터리와 그 안의 record.json을 함께 주면 같은 회차가 두 번 합산된다. 중복 키 거부 권함. |
| P3-8 | `REPORT.md:48` | "도구·자식·압축이 없는 30개 회차"는 실제 검사(`crosscheck_0104.py:95`)가 Agent·압축만 제외한다. U·G 등 도구를 쓴 회차도 30개에 들어 있다. 문구 정정. |
| P3-9 | `Runtime_Contract_v0.md` §11 항목 7·§5-1 | rev 7이 02-04에 새 의무(기준 회차 전달, 체크포인트에 세션 누적 기록, 새→재개→/compact 연쇄 합 시험)를 추가한다. 01-04 원본으로 근거는 확인되며 수치 기준 추가는 없다. 02-04 착수 시 명세·원장에서 이 의무를 인지하도록 기록 권함. 또 `--env`로 넘긴 비밀 값은 `--secret-file`로도 주지 않으면 `sk-ant-` 형태 외에는 가려지지 않는다(운영 지침에 명시 권함). |

## 3. 검증한 것과 방법

### 3.1 빌드·시험
- `git archive 2df3c35`를 스크래치 디렉터리에 풀어 실행(worktree 무변경).
- `cargo test --offline --locked --workspace`: 전부 통과, 합계 106건(ieo-evidence 단위 4 + 통합 24 포함) — 보고 수치와 일치.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: 경고 없음.
- `python3.12 -I docs/poc/scripts/test_render_progress.py`: `RESULT PASS 0`(28건). 새 검사 `same_snapshot_and_digest_in_all_views`, `no_combined_progress_rate` 확인.
- 91d570e..2df3c35 사이 `crates/` 변경 없음 → build.txt의 소스 커밋(91d570e)과 검토 SHA의 코드가 같다.

### 3.2 교차 대조 재실행
- `crosscheck_0104.py <빌드한 ieo-evidence> <사본> <출력>`: `streams 35 · mismatches 0`, 통계 동일. 새로 만든 35개 record.json을 커밋된 replay 기록과 비교(시각·소요·argv·cwd 제외) → 전부 동일. `summarize` 결과도 `wall_ms_sum` 외 동일.
- 그러나 세션 방식 판정이 틀렸음을 별도로 확인(P2-1).
- 테스트 fixture 7개는 01-04 원본 stdout.jsonl과 SHA-256 바이트 동일.

### 3.3 재개 세션 `modelUsage` 누적 주장(원본 stream 직접 판독)
01-04 35개 stdout.jsonl을 독립 Python으로 읽어 `result.usage`와 `modelUsage` 합(input/output/cache_read/cache_creation)을 비교:
- N1 (2,135,2224,342) + N2 usage (4,12,2566,158) = N2 mU (6,147,4790,500) ✓; N2 mU + N3 usage (4,12,2724,37) = N3 mU (10,159,7514,537) ✓; N3 + N4a usage (8,342,13364,1528) = N4a mU (18,501,20878,2065) ✓.
- N4b(두 번째 결과): N3 + (4,12,2761,1078) = (14,171,10275,1615) ✓ → N4b는 N3에서 분기, N4a 기준이면 누적 감소(미계측 처리 맞음).
- L2: N4a + (4,12,2761,1042) = (22,513,23639,3107) ✓ → 실제 기준은 N4a. N4b 기준이면 (8,342,13364,1492) ≠ 자기 usage → 철회 맞음.
- L3: usage 0, mU − L2 mU = (2020,1354,2761,1019) = 압축 사용량 ✓, compact_boundary 1.
- G3a→G3b, G3a-rerun→G3b-rerun, R1→R2 모두 `이전 mU + 이번 usage = 이번 mU` ✓. 비용도 사슬을 따라 단조 증가(N4b < N4a로 분기 일관).
- T23(새 세션): mU (6,166,7907,3730) = usage (4,157,7907,2886) + 자식 (2,9,0,844) ✓.
→ 계약 rev 7 §11 항목 7의 사실 주장은 원본과 일치한다. P2c만 재개 회차인데 새 세션으로 처리되었다(P2-1).

### 3.4 코드 검토(판정·프로세스·가림·사용량·합산·스키마)
- 판정: `Success`는 실패 → failed, 필수 결과/산출물/표식 누락 → incomplete, exit 0만으로 성공 불가 확인. 한도 신호는 비차단 사유로 남김. 기대 판정은 기대 관측 + blocking 누락 없음.
- 프로세스: `process_group(0)`, `env_clear` 후 허용 이름만, 기동 실패도 기록 생성 확인(rt1 launch-fail). 결함은 P2-2, P2-3, P3-6.
- 가림: stdout·stderr·argv는 쓰기 전에 가림. 기록의 다른 문자열 필드(cwd, 산출물 경로, 표식 패턴, launch_error)는 운영자 입력·OS 문자열이며 비밀 경로가 보이지 않음. 결함은 P2-4. 커밋된 diff에서 실제 토큰 형태 검색: 시험용 가짜 값만 존재.
- 사용량: 네 항목 중 하나라도 없으면 unmeasured, 결과 없으면 run/parent/session 미계측·partial_observations 비합산, 재개 기준 없음/세션 불일치/누적 감소/압축 없는 불일치 → 미계측. 0 대체 경로는 "새 세션 + modelUsage 비어 있음 + CLI가 명시 보고한 usage 0" 하나뿐이며 출처 문자열로 구분된다(새 세션에 한해 타당).
- 합산: `tokens_measured_sum`은 계측 회차가 없으면 null, 미계측 목록 첨부, 비용 합은 모든 모델 회차가 회차 몫 비용을 가질 때만. 누적값은 회차 몫(`run`)만 합산하므로 재개 사슬·동시 분기에서 이중 합산 없음을 위 원본 수치로 확인. 남은 이중 합산 경로는 입력 중복(P3-7).
- 스키마-serde: 커밋된 43개 record.json을 Python `jsonschema` 4.26(Draft 2020-12)로 검증 → 오류 0. 스키마의 조건(`succeeded`⇒exited/0, 모델 요청⇒세션 방식 new|resume, new⇒resume_base null, measure oneOf)이 serde 출력과 맞음. `run_cost_usd` minimum 0은 코드의 `c >= bc` 조건과 일치.

### 3.5 보고 주장 대 증거
- 시험 수(28·106·28), 실제 실행 8건 판정(성공 4·기대 일치 2·실패 1·불완전 1), TERM 2,033 ms, 덮어쓰기 거부·세션 방식 없는 모델 요청 거부(exit 2) — 증거 파일과 일치.
- 모델 호출 0회: `claude --version`만 실행, 01-04 예산 미사용 — 증거와 일치.
- 고정 입력/실제 프로세스/실제 모델 실행 범위 구분(§4) 있음.
- 불일치: P2-1(세션 방식, 33/35, P2c), P3-8(30개 회차 문구).
- F05: 시작 11:34:57Z, 마지막 커밋 12:37:31Z(약 3,750 s) — 14,400 s 이내.

### 3.6 명세 초과 여부
- 수용 기준 밖의 수치 기준이나 기능 승격은 없다. `MIN_SECRET_LEN 8`, 기본 유예 10 s, 정리 대기 2 s/10 s는 구현 파라미터이며 판정 기준이 아님. `summarize`·`inspect-stream`·`partial_observations`·귀속 대조는 사용량 수집·비은닉의 범위 안이다. 토큰 총량 정의(L01/L02)는 정하지 않고 남겼다(적절).

### 3.7 봇 리뷰 평가(codex, 2df3c35 대상 3건)
- run.rs:148 원자적 디렉터리 예약(P2 표기) → 타당, 단 전제상 발생 가능성 낮아 **P3-1**.
- redact.rs:59 JSON 이스케이프 비밀(P1 표기) → 타당, 현 비밀 형태에서는 유출 없음 → **P2-4**.
- run.rs:241 출력 보존(P2 표기) → 직접 재현으로 확인, **P2-2**.
- 봇이 놓친 것: P2-1(교차 대조 버그), P2-3(시간 초과 후 생존 프로세스).

## 4. 병합 조건

P2-1~P2-4 수정 후 새 head에서 재검토. 최소 요구: crosscheck 세션 방식 수정과 증거·보고 재생성, 수집 손실의 명시 기록과 판정 반영, 시간 초과 시 그룹 KILL 보장, JSON 이스케이프 비밀 거부 또는 처리 — 각각 시험 1건 이상.
