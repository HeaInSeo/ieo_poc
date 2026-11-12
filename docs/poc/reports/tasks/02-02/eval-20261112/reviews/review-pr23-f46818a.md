# 독립 검토 — PR #23 (02-02 · 독립 기대값·모의 시계·평가 자료)

| 항목 | 값 |
| --- | --- |
| 검토 대상 head | `f46818a98c7b299b940fbfcef1bc86670979c8a6` (branch `task/02-02-eval`) |
| 기준 | 사용자 결정 `docs/poc/User_Decisions_2026-11-12_0202.md`(권위), 개정 A2, `poc-acceptance-v1.3.3` rev 6 `02-02.accept` |
| 비교 범위 | `1dd56f7..f46818a` (091e099 · 1785cb7 · 61a27aa · f46818a) |
| 실행 위치 | `git archive` 사본(head·1785cb7)을 scratchpad에 풀어 실행. worktree는 읽기만 했다 |
| 판정 | **approve** (P1 0 · P2 0 · P3 7). head가 바뀌면 이 승인은 적용되지 않는다 |

검토자의 독립성 한계: 이 검토자도 개발 세션이 띄운 하위 에이전트이며 같은 모델 계열, 같은 OS 계정(heain)이다. 다른 모델이나 사람의 검토는 아니다. 평가 세트·정답 파일도 이 검토자가 읽을 수 있는 위치에 있었다.

## 1. 확인한 것

### 1.1 개정 정합성(사용자 결정 대조)

- 실행: `validate_package.py` PASS(`amendments: [A1, A2]`, delivered 120, amended 12). `render_progress.py --check` PASS. `test_render_progress.py` RESULT PASS. `cargo run --bin ieo-spec -- check-manifest docs/poc/Console_Coordinator_Sprints_v0.json` rc 0, 9개 모두 201 `ACCEPTED_FOR_SEMANTIC_REVIEW`(POC-02 rev 6, 나머지 rev 5).
- 명세를 직접 대조했다(`1dd56f7` 대비).
  - 최상위에서 바뀐 키는 `inspection_profiles`, `sprints`, `amendments`뿐이다.
  - rev 4·5 프로파일은 바이트 그대로이고 rev 6이 추가됐다. rev 6과 rev 5의 차이는 `02-02.accept` 하나이고 키 집합이 같다.
  - 바뀐 스프린트는 POC-02 하나다(`revision`·`sources`·`tasks`·`digest`).
  - 바뀐 작업은 02-02 하나다(`acceptance`·`exit_condition`, profile_revision 4→6). 02-02는 A1에서 바뀌지 않았으므로 4→6은 맞다.
  - 새 출처 `User_Decisions_2026-11-12_0202.md`의 digest `37874278…`는 파일 SHA-256과 같다(validator가 확인).
- A2 JSON의 `files` 3개: `original_sha256`이 SHA256SUMS 값과 같고, 현재 SHA가 `sha256`과 같다(validator가 확인).
- 실행 장부 02-02 행이 rev 6 문구와 바이트 단위로 같다(validator `ledger_spec_match`). 04-07 행과 제품 런타임 역할·자격 격리 문구는 바뀌지 않았다.
- rev 6 문구와 사용자 결정 항목을 대조했다.
  - 유지된 것: 정상/실패/누락 외부 대조, 첫 평가 전 동결, 미확정 분리, 공개·작성·검증 세트, 현재 범위 누출 검사.
  - 제외·이월된 것: 강한 봉인·OS 권한 실증 → P07 등 착수 전.
  - 필수 문구: ‘개발 계정의 정답 접근 차단은 미검증’ 명시.
  - 보고 금지: Amendment §범위 한정에 “통과를 강한 봉인 검증·전체 정확성 보장으로 보고하지 않음”.
  - kind·sudo·클러스터 불변, 계정·sub-UID·VM 제외.
  - 봉인안: 이력 보존, 처분 “이번 단계에서는 보류”(PLAN.md:3).
  - 범위는 02-02 평가 환경에 한정하고, 제품 런타임 격리는 유지한다.
  - 완료 수에 넣지 않음: A2.md:5.
  - 기준 문구에 더하거나 빠뜨린 것은 없다. 다만 “수집기 이월 결함 교정”은 기준 문구에 없고 작업 항목으로만 수행됐다(P3-5).
- `validate_package.py` 일반화를 검토했다. A1 때 하던 검사는 모두 남아 있다.
  - 프로파일 집합 고정({4,5,6})
  - 이웃 revision 차이 = `changed_acceptance` = `changed_items`(이 등식은 새로 생겨 A1에도 더 엄격해졌다)
  - 작업별 profile_revision
  - 스프린트 revision
  - SHA256SUMS 대조
  - 같은 경로가 A1·A2에 모두 있으면 A2 값이 우선한다. 이 경우 A1의 중간 SHA는 더 이상 검사하지 않는데, 순차 개정이므로 당연하고 A2 JSON `note`에도 적혀 있다.
  - 약화는 발견하지 못했다. 사소한 공백은 P3-6에 적었다.
- 01-03 정정: `host-isolation-check.txt:4-5`는 `sudo -n true` → “a password is required”뿐이다. 정정 문구와 `host-check.txt`(wheel, `(ALL) ALL`, `NOPASSWD: /usr/local/bin/kind`)가 일치한다. 저장소의 다른 현행 보고·progress에는 “sudo 없음” 서술이 없다. 과거 검토 기록 review-pr7은 이력이라 고치지 않은 것이 맞다.

### 1.2 코드(`git diff 1dd56f7 f46818a -- crates/`, 1785cb7..f46818a 사이 crates 변경 0)

- `cargo test --workspace --offline --locked` 전부 통과. `cargo clippy --workspace --all-targets -D warnings` 통과. `cargo fmt --check` 통과.
- `run.rs` 배수 기간: 정상 종료 뒤 `group_alive && !pipes_done` 동안 최대 2초 기다린다. 그 뒤에도 그룹이 살아 있으면 KILL한다. KILL 직전 관이 열려 있었으면 `output_incomplete`이다.
  - `Reader::wait(ZERO)`의 반복 호출을 확인했다. 송신 뒤 sender가 drop되므로 두 번째부터는 `Disconnected`가 오고 true로 처리된다. 신호를 소비해 오판하는 문제는 없다.
  - 관 확인과 KILL 사이 경쟁: 그 사이에 관이 닫혀도 결과는 `incomplete` 쪽(보수적)이라 성공으로 잘못 세지는 않는다.
  - 공유 마감: stdout이 쓰고 남은 시간을 stderr가 쓰므로 두 스트림이 같은 2초 안에서 다뤄진다.
- 행동 변화: `sleep 30 & echo x`처럼 관을 잡은 채 아무것도 쓰지 않는 그룹원이 있으면, 이전에는 `succeeded`였고 이제는 `incomplete`다. 검토 273907b P3-1의 처분과 일치하고 REPORT:19에 적혀 있다.
- `summarize` 중복 거부: (task, attempt, command) 키로 거부하고 종료 2. 버그는 없다(시험 공백은 P3-4).
- `clock.rs`: `MockClock`은 `advance`로만 움직인다. 독(poison)에 걸려도 회복한다. 아직 수집기 경로에 주입되지 않아 시험에서만 쓰이며, REPORT:20이 그대로 밝힌다.
- 버그는 발견하지 못했다.

### 1.3 평가 무결성(`docs/poc/reports/tasks/02-02/eval-20261112/`)

- 순서(git·reflog):

  | 사건 | 시각(UTC) | 출처 |
  | --- | --- | --- |
  | 1785cb7 커밋 | 13:40:27Z | 커밋 시각 |
  | manifest `created_at` | 13:53:52Z | manifest.json |
  | 61a27aa 커밋 | 13:54:14Z | 커밋 시각 |
  | 61a27aa push | 13:54:16Z | 로컬 reflog `origin/task/02-02-eval` |
  | 평가 시작 | 13:55:01.235Z | eval-times.txt |
  | 평가 끝 | 13:55:19.399Z | eval-times.txt |

  - 61a27aa의 push 시각은 로컬 reflog에만 있다. GitHub 쪽 증거는 공개 events API에 이 push가 없고, 61a27aa에 대한 CI run도 없다(P3-3).
  - 개발 scratchpad `0202-eval/`에서 가장 이른 파일 mtime은 22:41:42+09(oracle_stream.py)로 후보 커밋보다 늦다. 따라서 후보 코드가 세트를 보고 만들어졌을 수는 구조적으로 없다(로컬 mtime이라 약한 증거).
- 해시:
  - `freeze/manifest.json` SHA-256 = `ec486cc5…`. 61a27aa의 blob과 같다.
  - manifest의 42개 파일과 `manifest-results.json`의 16개 파일을 모두 직접 다시 해시했고 불일치는 0이다.
  - 1785cb7과 f46818a 사이에 crates 변경은 없다.
- 기대값 근거를 01-04 원본 stream에서 독립 계산으로 다시 확인했다. 기대값 작성자의 `oracle_stream.py`는 쓰지 않고 따로 작성한 리더(`myscripts/indep.py`)를 썼다.
  - fixture 14개는 SOURCES.json의 01-04 원본과 바이트가 같다.
  - V-12(T23): parent `(4,157,7907,2886)`, session `(6,166,7907,3730)`, 자식 1개 `af06f3de…` `(2,9,0,844)`, parent+자식 = session → `consistent`, 비용 0.0161167. 모두 일치한다.
  - V-13(N1→N2→N3→N4a→L2→L3 재개 연쇄): 6회차의 parent·session·run share·run cost를 모두 다시 계산했고 일치한다. L3는 압축 1회이고 run `(2020,1354,2761,1019)`이며, parent 0 ≠ share이므로 `mismatch`다. 02-01 REPORT 수치와도 같다.
  - V-01·V-30·V-31: stdout/stderr 바이트·SHA가 명령이 출력하는 바이트에서 바로 나온다(실측 PASS로 함께 확인).
- 미확정 처리: validation 필드 52개 가운데 36개는 `by_model`·`runtime_version`(규칙에 정의 없음)이다. 나머지 16개를 하나씩 보고 후보의 실제 값도 열어 확인했다.
  - V-14·V-20: 미계측 사용량인데 `succeeded`. 승인 규칙에 차단 효과가 정의되어 있지 않다.
  - V-33: `error_max_turns`인데 is_error=false이고 후보는 `succeeded`. P3-2 미결.
  - V-34: 후보는 `expected_not_matched`. P3-4 미결.
  - V-07: `stray_group_killed` false.
  - 모두 승인 규칙에 정해지지 않은 지점이라 실패를 숨기려고 제외했다고 보지 않는다. 다만 V-33에서 후보가 `succeeded`를 내는 점은 P3-2 규칙을 정할 때 참고할 정보다(P3-2 처분에 반영 권장).
  - `score.py` 비교기는 엄격하다. 누락은 실패, 부동소수 오차 허용은 1e-12, bool과 int를 구분한다.

### 1.4 재채점(1785cb7 새 빌드)

- `git archive 1785cb7`을 `cargo build --release --offline --locked -p ieo-evidence`로 빌드했다. 바이너리 SHA-256 `8125cc4a370e…ce16`으로 build.txt와 **같다**(재현 빌드).
- 동결 사본의 `score.py`로 세 세트를 재실행했다(산출은 scratchpad `rr-*`, 동결 파일은 수정하지 않음): public 5/5, authoring 14/14, **validation 33/34**, 미확정 사례 1, 필드 3·19·52. 보고값과 같다.
- **V-22 분류는 맞다(사례 작성 오류, 후보 결함 아님).**
  - 승인 규칙(02-01 REPORT:21)은 `--secret-file` 값, 그 SHA-256, `sk-ant-…` 형태를 stdout·stderr·argv에서 가린다고 정한다.
  - argv[2]의 스크립트 원문에는 `k="sk-""ant-oat01-Qw3r…"`처럼 나뉜 글자열이 있어 `sk-ant-` 형태가 아니다.
  - 같은 사례의 작성자 기대 `argv_replacements = 2`(값 1개 + SHA 1개, basis에 “sk-ant literal split in script”라고 명시)가 이 꼬리가 argv에 남는다는 것을 이미 전제한다. `$tree_not_contains`의 셋째 문자열은 같은 사례 안의 다른 기대와 모순된다.
  - 재실행 record에서 argv는 값·SHA만 가려졌고, stderr의 실제 토큰은 `sk-ant-[MASKED]`로 가려졌다.
  - 위장(분할) 문자열까지 가리는 것은 규칙 밖이며, 패턴 기반 가림의 알려진 한계다(P3-7에 기록 권장).
- 공식 결과를 33/34로 두고 재채점하지 않은 처리는 맞다.

### 1.5 보고 주장

- REPORT.md:12와 result.json `observed`에 **‘개발 계정의 정답 접근 차단은 미검증’**이 있다. leak-check.md §d에도 있다.
- 통과를 강한 봉인 검증이나 전체 정확성 보장으로 보고하지 않는다(REPORT:12, leak-check §d).
- 독립성 한계를 정직하게 밝혔다: “같은 모델의 별도 하위 에이전트(맥락 비공유). 다른 모델·사람 담당이 아니다”(REPORT:46), 사용량 미계측(REPORT:9·56). 작성자의 “src를 읽지 않았다”는 주장은 검증할 수 없지만 claim으로 다뤄졌다.
- F05: 동결 대장(01-03 REPORT:21)은 설계 3,600초·구현 14,400초와 외부 절차 몇 개만 정한다. 02-02(준비)에 맞는 상한이 없다는 서술은 정확하다. 실측 약 2,350초는 더 엄격한 3,600초 안이다.
- 사소한 불일치는 P3-2에 적었다.
- 수치(5/5, 14/14, 33/34, 필드 68·234·628, 미확정 74, 동결 파일 42, 바이너리 SHA, 시각)는 모두 재현되거나 원본과 일치한다. 시험은 3건 추가(clock 1, collector 2)·1건 기대 변경으로, REPORT:19와 맞다.

### 1.6 PR 봇 의견

- `gh api repos/HeaInSeo/ieo_poc/pulls/23/comments`: 0건.
- `gh pr view 23 --comments`: chatgpt-codex-connector 요약 1건. `f46818a`에 대한 code review가 completed이고 `has_any_findings: false`다. 처분할 의견은 없다.
- CI run 43232495921(f46818a, pull_request) success.

## 2. 발견

P1 없음. P2 없음.

### P3-1. 이월 항목과 02-02 소유 미결 항목의 원장 연결이 아직 완성되지 않았다

- 위치: `docs/poc/Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md:119`(04-07 행 불변), `docs/poc/progress/state.json`(`unresolved` 불변), `docs/poc/reports/tasks/02-02/eval-20261112/REPORT.md:54`.
- 시나리오:
  - 사용자는 “이월 항목은 담당 작업과 처리 시점을 정해 원장에 연결”하라고 했다.
  - 담당(04-07)과 시점은 A2 JSON `deferred`, 명세 `amendments[].reading_rule`, 장부 02-02 행의 “P07 등 착수 전”에만 있다. 04-07 행을 읽는 사람은 이 선행 조건을 보지 못한다. 명세 최상위 `amendments`는 스프린트 digest에 묶이지도 않는다(01-03 검토 N2).
  - 02-01 REPORT:89가 02-02에 넘긴 P3-2·3·4·6 가운데 P3-2·4·6은 “미확정, 규칙을 정하면 새 세트”로만 남았고 새 담당·시점이 없다. P3-3(무관한 실패의 expected_matched, stdout 전용 표식)은 02-02 보고에 처분이 없다.
- 권고: 병합 뒤 원장 갱신(state.json `unresolved`, 진행 원장) 때 다음 두 가지를 반드시 넣는다.
  1. “강한 봉인·정답 접근 차단 실증 → 담당 04-07, P07 숨긴 결함 회차 착수 전”
  2. P3-2·3·4·6 각각의 새 담당·시점
  - 04-07 행 자체를 고치려면 수용 기준 개정이 필요하므로, 원장 연결로 충분하다고 본다.

### P3-2. result.json `cost.elapsed_seconds`가 null인데 REPORT에는 실측값이 있다

- 위치: `eval-20261112/result.json:34`, `REPORT.md:8`.
- REPORT는 시작 13:18:45Z와 보고 13:58Z(약 2,350초)를 적었는데 result.json은 null이다. 측정 경계(시작 13:18:45Z)를 정해 기계 판독본에도 같은 값을 넣거나, null의 이유를 적기를 권한다.

### P3-3. 동결 커밋이 평가 전에 push됐다는 증거가 로컬에만 있다

- 위치: `results/leak-check.md` §b, REPORT.md:25.
- push 시각 13:54:16Z는 로컬 reflog에만 있다. GitHub에는 61a27aa의 CI run도 공개 PushEvent도 없고, PR은 13:58:45Z에 열렸다.
- 커밋 시각은 같은 계정이 정할 수 있는 값이다. 다만 세트 파일 mtime이 후보 커밋보다 늦어, 후보가 세트를 볼 수 없었다는 결론에는 영향이 없다.
- 다음 동결부터는 push 뒤 원격 ref 조회 결과(`git ls-remote` 출력·시각)나 동결 커밋의 CI run을 증거로 남기기를 권한다.

### P3-4. `summarize` 중복 거부에 crate 시험이 없다

- 위치: `crates/ieo-evidence/src/bin/ieo-evidence.rs:167-186`.
- 이 동작은 평가 세트(A-12, V-24, V-27)로만 확인된다. 회귀 방지용 시험 1건(같은 기록 경로 두 번 → 종료 2, 메시지에 두 경로)을 다음 계측기 수정 때 추가하기를 권한다.

### P3-5. 일부 기대값은 개발 측 “task brief” 서술에 근거하고, brief 원문은 남아 있지 않다

- 위치: `eval-20261112/tools/gen_sets.py:23,29`, `freeze/manifest.json` `rules_used`.
- 확정 필드 930개 가운데 59개의 근거가 brief 5·6항뿐이다. 내용은 CLI 종료 코드 0/1/2, 2초 배수 기간, 관이 열린 채 KILL되면 incomplete, 공유 마감, 중복 거부 종료 2다.
- brief 내용은 gen_sets.py 상수에 요약돼 동결됐지만 원문은 저장소에 없다. 이 필드들은 개발자가 구현을 설명한 것에서 나왔으므로 다른 필드보다 독립성이 약하다. REPORT:22가 “이월 교정 서술, CLI 인터페이스”를 입력으로 밝혔으니 숨긴 것은 아니다.
- 다음 회차에는 brief 원문을 동결 파일에 넣고 REPORT에 이 약점을 한 줄로 적기를 권한다.
- 같은 맥락에서 “수집기 이월 결함 교정”은 사용자 결정 2항에 있지만 rev 6 기준 문구에는 없다. 작업으로는 수행됐다(091e099).

### P3-6. 검사기의 개정 순서 가정이 명시적으로 검사되지 않는다

- 위치: `docs/poc/scripts/validate_package.py:57`.
- `prev = r-1`이라고 가정하지만, 개정 목록의 revision이 정확히 5, 6 순서인지는 단언하지 않는다. 또 스프린트 revision이 그 스프린트 안 작업의 기준 변경과 함께 올라갔는지도 대조하지 않는다.
- 현재 데이터로는 문제가 없다. 다음 개정 때 `[pr['revision'] for am in amends] == [5, 6, …]`와 “바뀐 작업의 스프린트 ∈ sprint_revisions” 단언을 추가하기를 권한다.

### P3-7. 사소한 문구와 기록

- `reports/tasks/01-03/staged-freeze-20261112/REPORT.md:66`: 정정 문단 뒤에 원래 문장 “그래서 P2의 UID 경계를 시험할 수 있을 것으로 본다”가 이어진다. 이 문장의 근거는 정정문이 아니라 rootless podman·sub-UID라서, 읽는 순서가 어색하다.
- V-22가 드러낸 한계(분할·위장된 `sk-ant` 문자열, 스크립트 원문 안의 토큰 조각은 argv 기록에 남는다)는 규칙 범위 밖이지만, 02-04(`--secret-file` 사용 의무) 쪽 운용 주의로 남길 만하다.

## 3. 결론

- **approve, head `f46818a98c7b299b940fbfcef1bc86670979c8a6`.**
- 개정 A2는 사용자 결정을 더하거나 빠뜨리지 않고 반영했다.
- 문서 검사기, 생성 화면, 명세 검사, Rust 시험, clippy가 모두 통과했다.
- 동결 순서와 해시, 기대값의 원본 근거(01-04 stream 독립 재계산)를 확인했다.
- 재현 빌드와 재채점 결과 33/34가 같고, V-22를 사례 작성 오류로 본 분류도 맞다.
- “개발 계정의 정답 접근 차단은 미검증”이 명시되어 있고, 과대 주장은 없다.
- P3-1(이월 항목과 P3-2·3·4·6의 원장 연결)은 병합 뒤 원장 갱신에서 반드시 처리할 것을 조건으로 권한다.
- 이 승인은 강한 봉인 검증이 아니며, 같은 모델·같은 계정의 검토라는 한계가 있다.
