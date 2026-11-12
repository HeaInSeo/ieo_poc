# PR #9 독립 검토 (Guardrail) — head 2fc943c vs main 53d66d5

VERDICT: approve

검토 대상: `fix/validate-write-path` @ `2fc943ca86f3d64d374ca0c5b281188569d654f0` (9개 파일, +216/−4). 저장소·PR은 수정하지 않았다. 깨끗한 worktree에서 실행한 뒤 `--force`로 지웠다.

## 1. 교정의 정합성과 무결성 검사 약화 여부

- **결함은 유효하고, 재현된다.** main 4bd4eaf의 깨끗한 worktree에서 `--write`는 rc=0이었고, 이어서 실행한 검산은 `AssertionError: v1_3_3_validation.json`로 rc=1이었다. `evidence/write-repro.txt`와 같다. 원 지적은 `chatgpt-codex-connector[bot]` 댓글 4546491247(P2, `validate_package.py` line 101, commit 814a5fb, original 5c3af86)이며, REPORT와 PR 본문의 인용과 맞는다.
- **전달본은 바이트 단위로 같다.** `git diff 53d66d5 2fc943c -- docs/poc/v1_3_3_validation.json`은 비어 있다. 이 파일의 SHA는 `10cac447…1023`이고 `SHA256SUMS.txt` 값과 같다. `--write`를 두 번 실행한 뒤에도 바뀌지 않았다.
- **무결성 검사는 약해지지 않았다.** `validate()` 본문(`validate_package.py:29-102`)은 바뀌지 않았다. 바뀐 곳은 `__main__`의 쓰기 경로(`:106-112`)뿐이다. `SHA256SUMS.txt`(120줄)와 개정 색인의 대조 규칙도 그대로다. 새 파일 `validation/v1.3.3-A1_current_validation.json`은 SHA256SUMS에 없다. 무결성 루프는 SHA256SUMS에 있는 경로만 순회하므로 새 파일이 검사 대상을 바꾸지 않는다.
- **전달 경로 쓰기는 거부된다.** worktree의 SHA256SUMS에 출력 경로를 임시로 추가하고 `--write`를 실행했다. 결과는 `AssertionError: refusing to overwrite a delivered package file`였고, 파일은 쓰이지 않았다. 아래 P3-1의 한계가 있다.
- **개정 색인 갱신은 맞다.** 2fc943c의 `scripts/validate_package.py`를 직접 계산한 SHA-256은 `31c637ab4fe6667c3b595b5f76de4ee4a93dc456d43f56d362b86b0bc562d5e6`이다. `Amendment_v1.3.3-A1.json:148`과 같다. `original_sha256`은 `bd43577b…448c`이며 SHA256SUMS 값과 같다. `change`는 `amended`로 유지됐다.

## 2. 실행 확인 (깨끗한 worktree, 2fc943c)

| 단계 | rc | current_validation SHA | 전달본 SHA |
|---|---|---|---|
| `--write` | 0 | f5538c78…0a5697 | 10cac447…1023 |
| 검산 | 0 | — | — |
| `--write` | 0 | f5538c78…0a5697 | 10cac447…1023 |
| 검산 | 0 | — | — |
| `render_progress.py --check` | 0 (`"ok": true`) | | |

- 이 시퀀스 뒤 `git status --porcelain` 출력은 비어 있었다. 전달 파일과 커밋된 결과 파일 모두 바뀌지 않았다.
- 커밋된 `validation/v1.3.3-A1_current_validation.json`(f5538c78…)은 새로 `--write`한 출력과 바이트 단위로 같다.

## 3. 사용자 지시 원문 기록

`User_Decisions_2026-11-12_followup.md:7-19`를 이 세션 대화 기록(jsonl)의 사용자 메시지 원문과 `diff`로 비교했다. 빈 줄까지 **완전히 같다**(EXACT). 의역이나 추가는 없다. 머리말 `:1-3`은 출처와 반영 위치를 적은 편집 주석이며 `---`로 원문과 분리돼 있다. 원문 1번은 위임 메시지가 인용한 내용과 맞는다: 유효성 확인, 처분 기록, 원본 보존과 별도 경로 우선, 저장 뒤 재검산 연속 통과. PR 본문의 설명도 원문과 어긋나지 않는다.

## 4. L04 `shortfall_record` (`freeze_register.json:188`)

- 4번 지시와 맞는다. 6+6 유지, 07-01의 백로그·Notion 배정 대조, 독립 수용 가능성 확인, 그때도 부족하면 근거 있는 변경안 제시, 잘게 나누거나 약한 항목을 넣지 않는다는 내용이 모두 들어 있다.
- 5번 지시와 맞는다. 기록이 "01-04의 차단 조건이 아니다"를 명시한다.
- `adopted_now`(nodekit 6, sori 6, 8/12, 0, 0)와 `blocks: 07-02·07-03`은 바뀌지 않았다. `probe_gate_table.json`도 바뀌지 않았다. 01-04 차단 조건은 추가되지 않았고, 6+6 기준도 약해지지 않았다.
- 스냅샷 사실(NodeKit 1차 3건 중 2건은 #86 분할, 기능·실패 복구 후보 없음, sori 6건 네 종류 충족, N1·N2와 S2·S3 분할)은 `P26_Candidates.md:24-26,73,84,143,154,214`와 맞는다. 아래 P3-3을 보라.

## 5. 증거·REPORT 정확성과 비밀 정보

- REPORT의 표(유효성, 처분, 확인, 한계)는 위의 재현·재실행 결과와 맞는다.
- 비밀 정보는 없다. 토큰, 키, 비밀번호 패턴을 grep했고 검출되지 않았다.

## Findings

P1과 P2는 없다.

- **P3-1** `docs/poc/scripts/validate_package.py:111`: 거부를 `assert`로 구현했다. `python -O`로 실행하면 검사가 제거된다. worktree에서 출력 경로를 SHA256SUMS에 넣고 `-O --write`를 실행했더니 거부되지 않고 파일을 덮어썼다(rc=0). 출력 경로가 상수라서 현재 패키지에서는 실제로 발생하지 않는다. 그래도 거부를 보장하려면 `if …: sys.exit(...)`나 `raise`가 낫다.
- **P3-2** `docs/poc/reports/workspace/validate-write-20261112/evidence/write-fixed.txt:5,12`: 기록된 결과 파일 SHA `d584fd45…ed587`가 커밋된 파일과 깨끗한 재실행 결과(`f5538c78…0a5697`) 모두와 다르다. 증거는 최종 트리 이전 상태에서 만든 것으로 보인다(예: 링크 수 등 출력이 나중에 바뀜). REPORT도 이를 설명하지 않는다. 주장 자체(두 번 쓴 SHA가 같고 전달본은 그대로)는 참이고 재현된다. 다만 증거와 커밋 산출물을 연결하려면 최종 head에서 다시 만들거나 차이를 한 줄 적어야 한다.
- **P3-3** `docs/poc/reports/tasks/01-03/staged-freeze-20261112/freeze_register.json:188`: `shortfall_record`가 sori를 "1차 6건(네 종류 충족)"으로만 적는다. 스냅샷(`P26_Candidates.md:26`)은 S6가 약하다고 밝힌다. 사용자는 "약한 항목을 넣지는 마"라고 지시했으므로 sori도 07-01 재판정 대상이 될 수 있다. 괄호의 분할 후보 판정 목록에 S6 약함도 함께 적으면 더 정확하다.
- **P3-4** `validate_package.py:112`: `validation/` 디렉터리가 없으면 `write_text`가 실패한다(`mkdir` 없음). 현재는 디렉터리가 커밋돼 있어서 영향이 없다.

## 실행한 명령

```
git diff --stat 53d66d5 2fc943c ; git diff 53d66d5 2fc943c -- <변경 파일들>
gh api repos/HeaInSeo/ieo_poc/pulls/7/comments
gh pr view 9 --json body,headRefOid,title
git worktree add --detach <scratch>/wt-pr9 2fc943c
cd docs/poc
sha256sum scripts/validate_package.py v1_3_3_validation.json validation/v1.3.3-A1_current_validation.json
grep -E ' v1_3_3_validation.json$| scripts/validate_package.py$' SHA256SUMS.txt
for i in 1 2; do .venv-poc/bin/python scripts/validate_package.py --write; sha256sum ...; .venv-poc/bin/python scripts/validate_package.py; done   # 모두 rc=0
.venv-poc/bin/python scripts/render_progress.py --check   # ok
git status --porcelain   # 비어 있음
# 거부 시험: SHA256SUMS에 출력 경로를 임시 추가 → --write AssertionError; -O --write는 우회됨; 원복 후 git checkout
git worktree add --detach <scratch>/wt-4bd 4bd4eaf ; --write → 검산 rc=1 AssertionError: v1_3_3_validation.json (재현)
python3 -I (세션 jsonl에서 사용자 원문 추출) ; diff orig1.txt file_body.txt   # EXACT
grep P26_Candidates.md / state.json / probe_gate_table 대조 ; 비밀 패턴 grep
git worktree remove --force <scratch>/wt-4bd ; git worktree remove --force <scratch>/wt-pr9
```
