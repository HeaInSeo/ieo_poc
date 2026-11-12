# PR #21 독립 검토 — 02-01 · head 273907b

| 항목 | 기록 |
| --- | --- |
| 검토 대상 | HeaInSeo/ieo_poc PR #21, 브랜치 `task/02-01-evidence`, **head `273907b07c3da8d3fe86042c53cd0bd5bd1431c4`** |
| 비교 기준 | 이전 검토 head `2df3c35`(reject, P2-1~P2-4), 교정 `97b7231`, 증거 재생성 `a11f107`, 시험 수정 `273907b` |
| 검토 방법 | `git archive 273907b`를 scratchpad에 풀어 새로 빌드·실행(worktree 무수정). 비교용으로 `2df3c35`도 별도 빌드 |
| 수용 기준 | 원장 `Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md` 83행(02-01) |
| 판정 | **approve** — P1 없음, P2 없음, P3 3건(병합을 막지 않음, 기록 권고) |

## 1. 판정 요약

이전 검토의 P2 4건은 모두 이 head에서 고쳐졌음을 원래 실패 시나리오를 직접 재현해 확인했다. 교차 대조를 다시 실행한 결과는 커밋된 증거와 같다(35/35 일치, 32 계측, P2c·N4b·L2 미계측, 재생 기록 35개 의미 필드 동일, 요약 동일). `97b7231`과 `273907b` 사이 계측기 소스·스키마·Cargo 파일 차이는 0줄이라서, 97b7231로 만든 증거는 이 head의 코드와 같다. 작업공간 시험 110건, clippy `-D warnings`, fmt, 생성기 시험 28건 모두 통과했다. 새로 생긴 결함 중 성공을 잘못 보고하거나 미계측을 0으로 바꾸는 것은 없다. 아래 P3는 기록 정확도와 동작 일관성 문제다.

## 2. 이전 P2 교정 확인(직접 재현)

| 지적 | 재현 | 결과 |
| --- | --- | --- |
| P2-1 crosscheck argv | `crosscheck_0104.py`를 이 head 바이너리로 다시 실행(.venv-poc Python, `-I`) | 출력이 커밋된 `evidence/crosscheck-0104.txt`와 같음(소요 ms 제외). P2c 행: `resume`, 기준 `03-04-P1-guardrail`, `unmeasured`(“result.modelUsage 비어 있음; 새 세션이 아니어서…”). 통계 `new 25 · resume 10 · measured 32 · unmeasured 3 · semantic 30/30`, `mismatches 0`. 재생 기록 35개를 시각·경로·argv·소요를 뺀 나머지 필드로 비교해 **35개 모두 동일**. `summarize` 결과도 `replay-0104-summary.json`과 같음(`incomplete`, 32/35, 미계측 P2c·N4b·L2) |
| P2-2 그룹 밖 자손이 stdout 보유 | `-- sh -c 'echo hello-marker; setsid sleep 20 & sleep 1; exit 0'` 5회, `sleep 1` 없는 변형 5회 | 10회 모두 `incomplete`, `counts_as_success=false`, CLI 종료 1, stdout 13바이트(`hello-marker`) 보존, 표식 발견, `output_incomplete=true`. 소요 3.2 s / 2.2 s. 2df3c35에서는 빈 stdout + succeeded였음 |
| P2-3 TERM 무시 그룹원 | `--timeout-s 1 --grace-s 1 --expect timeout -- sh -c '(trap "" TERM; exec sleep 4747) >/dev/null 2>&1 & sleep 60'` 3회 + 고유 숫자(5858) 1회 | `timed_out`, TERM·KILL 모두 보냄, `stray_group_killed=true`, `expected_matched`, 약 2.02 s. 실행 0.2 s 뒤 `ps`로 `sleep 5858` **생존 0**(대조로 띄운 `sleep 5859`는 1로 잡힘). 변형: 관을 잡은 TERM 무시 그룹원, 지도자 자신이 TERM 무시(`trap "" TERM; sleep …`) — 모두 생존 없음 |
| P2-4 JSON 이스케이프 비밀 | `--secret-file`에 `"`, `\`, 탭, `\x01`, `\x7f`, 중간 개행, 중간 CRLF, 비UTF-8 값 | 모두 CLI 종료 2, 거부 메시지에 값 미출력, 증거 디렉터리 생성 전 거부(`Redactor::new`가 `create_dir`보다 앞, run.rs). 끝 개행·끝 CRLF는 잘라내고 정상 가림 확인. 비ASCII UTF-8 값은 stdout의 원문 JSON(`ensure_ascii=False`)과 argv에서 가려짐 |

## 3. 지적 사항

### P3-1. 정상 종료 뒤 남은 그룹원을 즉시 KILL해서 출력이 잘려도 `succeeded` (교정으로 생긴 동작 후퇴)

- 위치: `crates/ieo-evidence/src/run.rs:281-295`(시간 초과가 아니면 `group_alive` 즉시 `KILL`), `crates/ieo-evidence/src/judge.rs:70-77`(`stray_group_killed`는 비차단 사유)
- 시나리오: `-- sh -c '(sleep 0.3; echo late-line) & echo early'`
  - 273907b: 3회 모두 stdout `early`만 남음, `succeeded`, 사유 “명령 뒤에 남은 프로세스 그룹 구성원을 KILL함”.
  - 2df3c35(같은 명령): stdout `early late-line`. 이전에는 2초 동안 관이 닫히기를 기다렸다.
- 영향: 수집기가 직접 그룹원을 죽여 출력을 자른 뒤에도, 출력 관이 KILL로 닫혀 EOF가 되므로 `output_incomplete=false`가 되고 성공으로 판정된다. KILL 사실은 사유에 남아서 숨긴 것은 아니다. 그러나 기록만 보고는 출력이 잘렸는지 알 수 없다. 모델 요청은 결과 사건을 지도자(CLI)가 직접 쓰므로 영향 가능성이 낮다. 동시에 2df3c35도 2초 뒤에는 같은 방식으로 죽이고 성공으로 판정했으므로 차단 사유는 아니다.
- 권고(다음 계측기 수정 또는 02-02): KILL 전에 짧게(예: 기존 2 s) 관이 닫히기를 기다린다. 또는 KILL 시점에 리더 쪽 reader가 아직 EOF가 아니었으면 `output_incomplete`(또는 별도 필드)로 표시한다.

### P3-2. stdout과 stderr의 대기 시간이 달라서 같은 상황의 판정이 갈림

- 위치: `crates/ieo-evidence/src/run.rs:300-301`(stdout 최대 2 s, 그 뒤 stderr 최대 200 ms)
- 시나리오: 그룹 밖 자손이 0.5 s 뒤 한 줄을 쓰고 끝나는 경우
  - stderr로 쓰면 `incomplete`, `output_incomplete=true`, stderr 0바이트(줄 유실), 423 ms
  - stdout으로 쓰면 `succeeded`, stdout 9바이트 보존, 505 ms
- 영향: 보수적인 방향(성공을 거짓으로 내지 않음)이라 판정 안전성 문제는 없다. 다만 stdout은 즉시 EOF이고 stderr만 잡혀 있으면 stderr는 사실상 200 ms만 기다린다. 마감 하나를 두 reader가 함께 쓰도록(예: 2 s 마감 안에서 둘 다 기다리기) 하면 일관된다.

### P3-3. REPORT 머리표의 작성 시점이 rev 2에 맞게 갱신되지 않음

- 위치: `docs/poc/reports/tasks/02-01/collector-20261112/REPORT.md:9` “이 보고 작성 시점 12:36Z(약 3,700초)”
- rev 2 증거는 12:46Z에 생성됐고(`evidence/build.txt`) 보고는 그 뒤에 썼다. F05 상한 14,400 s 안이므로 판정에는 영향이 없다. 다음 갱신 때 rev 2 시점으로 고치기를 권한다.

### 참고(지적 아님)

- P2-4 거부 규칙은 Node `JSON.stringify` 방식의 이스케이프에 맞춰져 있다. ASCII로 이스케이프하는 생성기(예: Python 기본 `ensure_ascii`)가 비ASCII 비밀을 `ä`로 내면 가리지 못한다(직접 확인: 두 번째 줄 `{"t": "pässwörd-xyz"}`는 가려지지 않음). Claude CLI stream-json은 Node 방식이라 현재 대상에서는 문제가 아니다. 이후 다른 런타임을 감쌀 때 알아 둘 전제다.
- `group_alive`는 kill 신호 0을 쓰므로 그룹에 좀비만 남아도 살아 있다고 볼 수 있다. 정상 종료 15회, 지도자 TERM 무시 시간 초과 15회 실행에서 `stray_group_killed`가 거짓으로 켜진 경우는 0회였다. 대기 시간에 상한이 있어서 멈추지 않는다.
- 판정 비차단 사유를 문자열 앞부분(`"한도 신호"`, `"명령 뒤에 남은"`)으로 거른다(judge.rs:77). 지금은 맞게 동작하지만 문구를 바꾸면 깨지기 쉽다.

## 4. diff 검토(2df3c35..273907b)

- **Reader(Arc<Mutex<Vec>> + done 채널)**: 읽은 조각을 바로 공유 버퍼에 넣는다. EOF 뒤 `send`가 일어나고 `recv`가 그 뒤를 보장하므로, `done`이면 `take()`가 전체 바이트를 얻는다. EOF가 아니면 그 시점까지 읽은 것만 가져가고 `output_incomplete`를 표시한다. mutex 오염은 `into_inner`로 처리한다. EOF에 이르지 못한 reader 스레드는 남아 있지만 CLI는 곧 끝나므로 영향이 없다.
- **group_alive/KILL**: 시간 초과면 남은 유예 동안 20 ms 간격으로 확인한 뒤 KILL한다. 루프 안에서 이미 KILL했다면 바로 재확인한다. 정상 종료면 즉시 KILL한다(P3-1). 끝없이 기다리는 경로는 없다.
- **판정**: `output_incomplete`는 차단 사유다. Success 기대면 `incomplete`, 다른 기대면 `expected_not_matched`로 보수적이다. `stray_group_killed`는 비차단 사유로 나열한다(이전 P3-5 교정).
- **스키마**: `output_incomplete`가 required와 properties에 모두 추가됐다. 커밋된 증거 디렉터리의 기록(재생 35·실제 실행 10 포함)과 이 검토에서 만든 기록을 합한 101개를 jsonschema로 검증했고 실패는 0이다.
- **배타 디렉터리 생성(P3-1 교정)**: 부모는 `create_dir_all`, 마지막은 `create_dir`, `AlreadyExists`면 거부한다. 덮어쓰기 거부를 확인했다(`runs-cli.txt`).
- **시험 견고성**: P2-2 시험은 `sleep 1`로 setsid가 먼저 일어나게 해서 경쟁을 없앴다. 다만 sleep 1 없이도 10/10 incomplete였다. P2-3 시험은 POSIX sh만 쓰고 프로세스별 sleep 길이(4000+pid%1000)와 `pgrep -f`로 확인한다. 같은 호스트에서 다른 프로세스가 같은 문자열을 가질 가능성만 이론상 남는다. 두 시험 모두 이 호스트에서 통과했고 CI(run 43221466851)도 성공이다.
- **crosscheck 수정**: argv가 문자열이면 `json.loads`하고, 형식을 모르면 중단한다(조용히 넘어가지 않음).

## 5. 증거·보고 대조

- `evidence/build.txt` source commit `97b7231`: `git diff 97b7231 273907b -- crates/ieo-evidence/src contracts Cargo.toml Cargo.lock crates/ieo-evidence/Cargo.toml`가 0줄이다. 이 사이 변경은 `tests/collector.rs`와 REPORT §6 두 줄뿐이다. `git-head/stdout`도 97b7231이다.
- 실제 실행 10건: 커밋된 `runs/`를 이 head의 `summarize`로 다시 집계하면 `runs-summary.json`과 같다(성공 4·기대 일치 3·불완전 2·실패 1). `detached-holds-pipe`(sleep 1 없는 형태, incomplete 13바이트)와 `timeout-term-ignored`(TERM·KILL·stray, 2,027 ms)의 내용은 REPORT §4 서술과 맞는다. `timeout-abort`는 2,034 ms로 REPORT의 “TERM 2,034 ms”와 같다.
- 시험 수: ieo-evidence 32(단위 5·통합 27), 작업공간 110, 생성기 28 PASS로 REPORT·result.json과 같다.
- REPORT §3: P2c 재개·미계측 서술, “자식(Agent)·압축이 없는 30개”(P3-8 교정), 32/35를 확인했다. `result.json` observed·unresolved도 같은 수치다.
- REPORT §6 처분: 이전 검토 파일(`reviews/review-pr21-2df3c35.md`)의 P2 4건·P3 9건과 하나씩 대응한다. 교정(P2-1~4, P3-1, P3-5, P3-8)은 코드·시험·증거로 확인했고, 기록으로 넘긴 P3-2·3·4·6·7·9의 담당 지정(02-02/02-04/다음 수정)은 원래 지적 내용과 맞는다. P3-9의 `--env` 비밀 안내는 REPORT §1 비밀 행에 반영됐다. a11f107 CI 실패 원인(dash `exec -a`, setsid 경쟁)과 “계측기 코드·증거 불변” 서술은 diff와 일치한다.
- 범위: 원장 02-01 행을 넘는 수치 기준이나 기능 승격은 없다. 원장·progress 파일은 바뀌지 않았다(8→9/73은 보고 PR에서 할 일로 남김). 모델 호출은 0회다.

## 6. 실행한 검사

- 빌드·시험: `cargo test --workspace --offline --locked` → 110 passed. `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` → 통과. `cargo fmt --all -- --check` → 통과. `docs/poc/scripts/test_render_progress.py` → PASS 28.
- 시나리오: §2의 P2-2(10회)·P2-3(5회 + 변형 2)·P2-4(비밀 11종), P3-1·P3-2 재현, 좀비 오탐 확인(30회), 2df3c35 바이너리 비교.
- 교차 대조 재실행, 재생 기록 필드 비교, summarize 재집계, 스키마 검증 101건.
- PR 봇 의견(`gh api …/pulls/21/comments`): codex 지적 3건(배타 디렉터리, JSON 이스케이프 비밀, 출력 보존)은 모두 2df3c35에 대한 것이고, 작성자가 97b7231에서 고쳤다고 답했다. 이 검토에서 각각 확인했다. a11f107·273907b에 대한 새 봇 의견이나 리뷰는 없다. CI 3개 검사(docs/progress, toolchain pin, fmt·clippy·test)는 273907b에서 성공이다.

## 7. 결론

**approve (head 273907b07c3da8d3fe86042c53cd0bd5bd1431c4).** P2 4건 교정 확인, 새 P1/P2 없음. P3-1·P3-2(계측기 다음 수정 또는 02-02 외부 대조)와 P3-3(보고 머리표 시점)은 원장 미해결 또는 보고 PR에 기록하기를 권한다. head가 바뀌면 이 승인은 적용되지 않는다.
