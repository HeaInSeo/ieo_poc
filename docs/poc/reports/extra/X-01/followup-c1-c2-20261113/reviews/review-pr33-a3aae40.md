# PR #33 독립 검토 (Guardrail, 보고 PR) · head a3aae40 · base main c5a3d3a

VERDICT: APPROVE

- **대상.** `report/x01-c12-0204-core-20261113`의 head는 `a3aae409cb5e494929b91f436878eb36a0da374f`이다(`gh pr view 33 --json headRefOid`). 처음에는 9286d86을 검토했다. 이후 head가 a3aae40으로 바뀌었고, 바뀐 부분(`git diff 9286d86 a3aae40`: REPORT §2.6, X-01-C12 event, 새 unresolved 1건, 생성 화면)도 검토했다. 쓰기 작업은 이 파일 하나만 했다.

## 검사

1. **사실 대조(state.json·PROGRESS.md·REPORT §2.6).**
   - PR #30: commits 9a421b7 → 7afd4d1 → e908a8c → aa38041 → 7111a73. merge c5a3d3a(부모 3426c77·7111a73)이며 MERGED 상태다.
   - PR #31: 8cc5c30 → 260089f → 7f06184 → bc55edf. merge 0f446d6.
   - PR #32: 119ab95 → aeab3d9. merge 3426c77.
   - CI는 모두 success였다. main: 0f446d6 run 43441103331, 3426c77 run 43442655596, c5a3d3a run 43445817957. PR #30 head 7111a73: 필수 검사 3개 pass(run 43445430351).
   - 검토·교정 수가 보관 검토문과 맞는다. #30은 4/4·3/3, #31은 3/4·2/3, #32는 2/4·1/3이다.
   - supervisor 시험 20건(3426c77의 `#[test]` 20개). 3426c77에서 X-01 쪽 182건 + 20건 = 병합 상태 202건이다. aa38041 검토문 8행·156행과도 맞는다.
   - 02-04 예산 수치(12회·14,400초·1.5M·15M)와 X-02 수치(6회·3,600초·14,400초·3M·30M)가 User_Decisions_2026-11-13_x02.md와 맞는다.
   - 모델 호출 0이라는 주장과 어긋나는 근거는 찾지 못했다.
2. **완료 수.** state.json 기준 주계획은 73건 중 검수 완료 11·진행 1(02-04)·미착수 61이다. 후속 실험은 12건 모두 미착수다. 생성된 STATUS는 11/73·0/12다. 02-04는 "진행"이며 "진행 기록 1(수용 아님)"으로 적혀 있다. X-01·X-02는 extra_tasks에만 있다. 부풀린 수는 없다.
3. **PR #30 병합 경위(독립 사후 확인).**
   - `git rev-parse 7111a73^{tree}` = `de2c64a092e0104b5058c596aa0aa05f4bd54757`이다. `git merge-tree --write-tree 3426c77 aa38041`의 결과도 같은 값이다.
   - 7111a73의 부모는 aa38041·3426c77이다. committer는 GitHub이다(브랜치 최신화 기능).
   - PR 파일 20개에서 `git diff aa38041 7111a73 -- <PR files>`의 결과는 비어 있다. 따라서 PR 파일의 변경은 0이다.
   - c5a3d3a의 tree도 de2c64a로 같다.
   - 보고는 이 처리를 절차 이탈로 투명하게 밝힌다. User_Decisions_2026-11-12_0104b.md §1 원문은 "이번 건에 한해", 그리고 "독립 검토자가 확인"이라고 적는다. 보고는 이 원문과 맞는 범위만 주장한다. 갱신 전에는 최종 검토자가 같은 병합 상태를 시험했고, 갱신 뒤의 확인은 이 검토가 한다. 추인 여부는 사용자 결정으로 남기며, state.json의 unresolved와 event에도 기록되어 있다.
   - 이 검토가 tree 일치를 독립적으로 확인했다. 다른 변경이나 충돌은 없다. 추인은 여전히 사용자 몫이다.
4. **보관 검토문.**
   - pending-reviews에 원본이 있는 6개 파일은 byte 단위로 같다: review-pr28-97ce523, review-pr31-8cc5c30/260089f/bc55edf, review-pr32-119ab95/aeab3d9.
   - 각 파일의 VERDICT·head 줄은 PR 이력과 맞는다. 260089f 파일은 검토한 head가 7f06184라고 스스로 밝히고, PROGRESS §4도 같은 내용을 적는다.
   - review-pr30-aa38041.md는 아래 지적 1을 본다.
5. **비밀.** 추가 줄에서 `sk-ant|ghp_|github_pat|secret_|ntn_|token=|Bearer|password` 패턴을 찾았다. 실제 값은 0건이다. 걸린 것은 `password=$(cat <scratch>/pgpw)` 자리표시와 검사 명령 문자열뿐이다. c2-live-r3의 digest와 page id는 잘린 접두어(`3f8a300f…`·`3fd86e6d`)다. Notion 큐 원문은 없다.
6. **생성 화면과 검사.** `render_progress.py --check`는 ok였다. `validate_package.py`는 rc 0, `test_render_progress.py`는 RESULT PASS였다. worktree는 깨끗하다.
7. **c2-live-r3와 §2.6.**
   - 다음 사항이 증거와 맞는다: 백업, PK (test_id, sprint_revision), 행 수 불변, r1~r3의 page 열이 NULL이라 불일치하는 점, r4 201(page·digest에 묶임), 최신 가져오기 3|3f8a300f|3|0, 양방향 403.
   - 한계도 숨기지 않았다. 1번 확인은 `IMPORT_GATED`가 아니라 `EVENT_CONTENT_CONFLICT`(409)로 거부됐다. 원인은 저장 명세가 revision 2에서 파생된 것이었기 때문이다. §2.6에 이 사실이 명시되어 있다. 판정의 근거는 시험과 c2-live-r2로 돌렸다.

## 지적

1. **P3, 출처 확인 불가.** `reviews/review-pr30-aa38041.md`는 `$(git rev-parse --git-common-dir)/ieo-dev-workspace/pending-reviews/`에 원본이 없다. 다른 worktree나 git 이력에도 이 PR 이전의 사본이 없다. 그래서 byte 단위로 대조하지 못했다. 내용은 확인 가능한 사실과 모두 맞는다(head aa38041, base main 3426c77, approve, 202건, 18/19). 차단 사유는 아니다. 앞으로는 approve 검토문도 pending-reviews에 원본을 남기기를 권한다.
2. **P3, 낡은 문장.** X-01 `next_action`이 아직 "X-02 착수 조건 충족 시(02-04 수용·모델 예산 승인·큐 선행 처분)"이다. 예산과 큐 선행은 2026-11-13 결정으로 처분되었다(X-02 unresolved에 기록됨). 다음 보고 PR에서 X-02 next_action과 맞추면 된다.
3. **P3, 일관성.** 02-04 result_state와 event는 "main CI success"라고만 적는다. 이전 항목들처럼 run ID(0f446d6 43441103331, 3426c77 43442655596)를 적으면 대조가 쉽다. X-01-C12 event에도 c5a3d3a run 43445817957을 함께 적을 수 있다.

P1·P2 지적은 없다. 완료 수, 수용 상태, SHA, 판정, 병합 경위가 모두 정확하다.
