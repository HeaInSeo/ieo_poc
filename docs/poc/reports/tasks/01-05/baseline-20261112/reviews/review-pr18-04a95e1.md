<!-- 보관본: PR #18 검토 04a95e1 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/18#issuecomment-6819540679 (머리말 생략) -->
# PR #18 독립 검토 · 01-05 검수 완료(PR #17) · 01-04 후속 보완(PR #16)

- 검토 SHA: `04a95e1438ec9f47eb426bb7e44b5d66846fa95a` (branch `report/01-05-accepted`)
- base: `main` `4bcba6ac2151d94e846143e87f51797c056262fc` (PR #17 병합)
- 날짜: 2026-11-12
- 범위: diff `4bcba6a..04a95e1` 12개 파일(state.json, 생성 화면 5개, 보관 검토문 6개)
- 제약: 저장소를 바꾸지 않았다. 비밀 값은 출력하지 않았다. 모든 검사는 `git archive 04a95e1`로 scratchpad에 푼 트리에서 했다.
- CI: head `04a95e1` run 43188897273 success. codex는 검토 시점에 진행 중(👀)이었다.

VERDICT: approve

P1·P2는 없다. P3 한 건(계약 rev 6 항목 5의 02-05 연결 누락)만 있다.

---

## 1. 검사 결과

### 1.1 render·validate
- `render_progress.py --check`: `{"ok": true, …}` rc=0
- `validate_package.py`: rc=0
  - progress: accepted 7, main 73, follow-up 12
  - generated_views_match true, schema_errors 0, ledger_spec_match true, 링크 285
- 원장 SHA-256 `bd9abbe7b444e0f2…`가 네 화면 머리말과 같다.

### 1.2 집계(부풀림 없음)
- tasks를 다시 셌다. main은 검수 완료 7·미착수 66이고, followup은 미착수 12다.
- 새로 수용된 것은 01-05 하나다. 근거는 00a7566 검토의 "01-05의 수용 문장 네 개를 모두 충족한다"(댓글 6819432787 :47)다.
- PR #16은 사건 `01-04-FOLLOWUPS-20261112`(task_id null, "작업 수 아님")로만 기록됐다.
- 01-06·02-04·03-02·04-07·05-01·05-02·05-03은 모두 `미착수`이고, `unresolved` 문구만 붙었다. experiments·milestones는 바뀌지 않았다.

### 1.3 PR #16·#17 기록
| 주장 | 근거 | 결과 |
| --- | --- | --- |
| PR #16 f64c413·d579f97·8cd805e reject → f73ac13 approve | 댓글 6819078773(08:49:30Z)·6819149410(08:53:08Z)·6819195113(08:54:55Z)·6819231087(08:56:45Z) | 같음 |
| 병합 cd19663 | MERGED 08:56:47Z, head f73ac13 | 같음 |
| main CI 43187135040 | push/main, cd19663, 3 job success, 08:57:36Z | 같음 |
| PR #17 efa4a5d reject(1/4) → 00a7566 approve(2/4) | 댓글 6819395632(09:05:02Z)·6819432787(09:06:53Z), 머리말 "(01-05 검토 n/4)" | 같음 |
| 병합 4bcba6a | MERGED 09:06:56Z, head 00a7566 | 같음 |
| main CI 43188585139 | push/main, 4bcba6a, 3 job success, 09:07:49Z | 같음 |
| 영향 감사 "모델 시도 35건 모두 결과 사건, 모델 없는 시험 33건 표식" | `audit_result_events.txt:470` "launched 35 · missing [] · exit0 without result [] · non-zero 05-P2c 1건". f73ac13 검토 :25 독립 집계와 같음. 35 = 9 + 26(B2 27회 중 미기동 1) | 같음 |
| 회귀 시험 PASS | `test_result_check.txt` PASS 줄 | 같음 |
| codex | PR #16·#17 모두 `has_any_findings: false`, 인라인 지적 0, 👍 | 처분할 것 없음 |

### 1.4 관측 시각
- snapshot과 새·개정 사건 3개는 18:08:43+09:00(09:08:43Z)이다.
- 이 시각은 main CI 43188585139 종료(09:07:49Z)와 43187135040 종료(08:57:36Z)보다 뒤이고, 04a95e1 커밋(09:08:56Z)보다 앞이다.
- `01-04-ACCEPTED-20261112` revision 2가 덧붙인 run 43181686192(08:17:57Z)보다도 뒤다. 역전 없음.

### 1.5 증거 경로
- 01-04·01-05 evidence, 새·개정 사건 4개의 evidence, resume.saved_reports를 합친 로컬 경로 37개가 모두 04a95e1 트리에 있다(`git cat-file -e`).

### 1.6 보관 검토문
- 6개 모두 게시 댓글과 비교했다. 머리말 1줄(5개는 끝 "교정 커밋 … 재검토 예정" 3줄도) 생략 말고는 본문이 같다.
- 생략은 각 머리 주석에 "(머리말·끝 교정 안내 생략)"으로 밝혔다.
- f73ac13·00a7566은 게시본에 끝 안내가 없으니 주석의 "끝 교정 안내 생략"은 해당 사항이 없을 뿐 사실과 어긋나지 않는다.

### 1.7 사용자 후속 지시 4·5 충족 여부
| 요구 | 반영 | 판단 |
| --- | --- | --- |
| base 갱신 head CI run 43181686192 연결 | 사건 `01-04-ACCEPTED-20261112` revision 2의 state("run 43181686192(3 job success, 08:17:57Z)")와 evidence(actions/runs URL). unresolved "PR #14 병합 처리 근거" 줄(`state.json:1859`) 끝. 값은 PR #15 검토 때 확인한 실제 run과 같다 | 충족 |
| 하네스 잔여 위험 처분을 이월 목록에 | `state.json:1861` "하네스 잔여 위험(결과 사건 없이 종료 0, 산출물 빈 출력) — 처분: 교정(PR #16 cd19663), 01-04 판정 영향 없음, 회귀 시험 PASS". f73ac13 P3-C·P3-D는 다음 줄로 02-04에 이월 | 충족 |
| GH1~GH5를 04-07·05-01·05-02·05-03 착수 조건으로 | 각 `unresolved`가 "착수 조건"으로 시작한다. 04-07 GH1·GH2·GH3, 05-01 GH3·GH5, 05-02 GH3(05-01 경유), 05-03 GH1·GH2·GH4·GH5. ASSETS.md §3 "착수 조건 연결" 표와 정확히 같다 | 충족 |
| 연결 확인 CI와 독립 판정 구분 | 네 작업 모두 "repository-smoke는 연결 확인용이며 독립 판정 아님". ASSETS §3 머리 문단도 같다 | 충족 |
| 저장소 준비 미산입 | 네 작업 모두 "저장소 준비는 완료 수에 넣지 않음". 집계 변화 없음 | 충족 |
| 계약 rev 6 완료 조건을 02-04·03-02·04-07에 | `Runtime_Contract_v0.md` §10 표와 대조했다. 02-04: 항목 1~5 시험((a)(b)(c) 포함)과 f73ac13 P3-C·D. 03-02: 항목 1·6. 04-07: 항목 6(P07 위협 모형). 완료 조건 열과 같다 | 충족(아래 P3-1) |

### 1.8 비밀·식별자
- 두 역할 토큰의 원문, 중간 32자, `sha256` 전체·앞 32자가 04a95e1 트리 전체에서 0건이다.
- E 시험 비밀 5개의 원문·sha256이 diff에서 0건이다.
- 호스트 `oauthAccount` 값 5종이 diff와 01-05·harness-result-check·01-02 트리에서 0건이다.
- diff 추가 줄: UUID 0, 이메일 0, GitHub 토큰 형식 0. `sk-ant` 7건은 모두 보관 검토문의 "`sk-ant`는 0건" 같은 설명 문구다. 64자 hex 4건은 원장 SHA다.

---

## 2. 발견 사항

### P3-1 계약 rev 6 항목 5의 완료 조건이 02-05에 연결되지 않음
- 위치: `state.json:246`(02-04 unresolved에만 "재개 키 session_id만"), 02-05 항목(`state.json:249`)에는 unresolved가 없다.
- 근거: `Runtime_Contract_v0.md` §10 항목 5의 완료 조건은 "**02-04·02-05**의 매핑 레코드가 재개에 session_id만 쓴다(코드 검토)"다. 지시 범위(02-04·03-02·04-07)는 충족했다. 하지만 계약이 지목한 02-05는 원장에서 추적되지 않는다.
- 권고: 다음 원장 갱신 때 02-05 `unresolved`에 항목 5를 연결한다.

---

## 3. 결론
head `04a95e1`을 승인한다. 승인은 이 SHA에만 유효하다.
