<!-- 보관본: PR #16 독립 검토 f64c413 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/16#issuecomment-6819078773 (머리말·끝 교정 안내 생략) -->
# PR #16 독립 검토 (Guardrail) — fix/01-04-followups @ f64c413

- 검토 대상: head `f64c413`, base main `2f3af51` (커밋 1개)
- 검토 방법: `git archive f64c413` 트리(스크래치패드)에서 원본 증거 재계산, 회귀 시험·검증 스크립트 실행, `gh api` 재조회, 비밀 값 대조. 모델 요청·probe 단계 실행 없음. 수정·커밋·push 없음.
- 검토일: 2026-11-12

VERDICT: reject

차단 사유는 P2 두 건이다. 하나는 하네스에서 "산출물 없음"을 성공으로 처리하는 경로가 남은 것이다(P2-1). 다른 하나는 계약 rev 6 항목 3의 규칙과 완료 조건이 서로 모순되는 것이다(P2-2). 영향 감사 결론("01-04 판정 영향 없음")은 독립 재계산으로 확인했고 유지된다.

## 확인한 것 (문제 없음)

1. **01-04 원본 증거 불변**: `git diff 2f3af51 f64c413 -- docs/poc/reports/tasks/01-04` 출력 0바이트.
2. **영향 감사 독립 재계산**(자체 스크립트로 `attempts/*/stdout.jsonl`·`meta.json`·`budget.json` 직접 집계):
   - 첫 run 9건과 나머지 run 26건을 합쳐 기동된 시도는 35건이다. 나머지 run은 예약 27건 중 시도 03(U-alt)이 `launched:false`다.
   - 35건 모두 `type=result` 사건이 1개 이상 있다. 종료 0인데 결과가 없는 시도는 0건이다.
   - 0이 아닌 종료는 `05-P2c-development`(rc=1, `error_during_execution`, `is_error:true`) 하나뿐이다. stdout의 `errors`에 "No conversation found with session ID: …"가 있어 REPORT §1.2의 기대 결과 서술과 맞는다.
   - 모델 없는 시험은 33건이다(P2 10, E 16, E1b 7). P2의 2c는 output이 없는 모델 시도라서 따로 셌다. 33건 모두 `DECISIVE` 표식이 있다. 표식은 각 시험의 기대값과 맞는다(E3: 201/200/409/`HTTP/1.1 200`/401/401, E1b-main-unchanged: `refs/heads/main 73ea669…`).
   - `audit_result_events.py`를 다시 실행한 결과가 `evidence/audit_result_events.txt`와 바이트 단위로 같다.
3. **회귀 시험**: 아카이브 트리에서 `.venv-poc/bin/python -I probes/01-04/test_result_check.py`를 실행했다. rc=0, `RESULT PASS 0`, 33건 확인. PASS/FAIL 줄은 `evidence/test_result_check.txt`와 같다. 출력에 `sk-ant` 문자열은 0건이다.
4. **하네스 핵심 교정**:
   - `probe2.stop_signals`(probe2.py:149-157)와 `probe.stop_on_signal`(probe.py:201-203)은 결과 사건이 없으면 종료 코드와 무관하게 정지한다.
   - `finish`(check_stop=False)는 `stop_batch`(C: probe2.py:603, N4: :563)로 모두 정지 검사를 받는다. `finish`가 넘기는 extra에는 `expected_outcome`이 없으므로 기본값(결과 필수) 쪽, 즉 안전한 쪽으로 동작한다.
   - P1 `ok`(probe.py:212)와 P3 `result_ok`(probe.py:323)는 결과 사건을 요구한다.
5. **계약 rev 6 범위**: diff hunk가 머리말에 적은 §2.1·§4.1·§4.2·§5·§7·§8·§10과 정확히 일치한다. 그 밖의 조항은 바뀌지 않았다.
6. **계약 항목별 근거 대조**(원본 기준):
   - U 01: `Write`·`Bash`·`Write` 거부 3건으로, 허용 대상까지 거부됐다.
   - U-alt 02·04: 거부 2건, ok.txt가 생성됐다.
   - G2 06: markers 없음, `NO_HOOK_FILE`.
   - G3a-rerun 09: marker 3종과 hook 파일 2개가 확인됐다.
   - G3b-rerun 10: 정리 뒤 marker가 없고 암호어를 기억했으며, 이전 CLAUDE.md 지시를 기록에서 인용했다.
   - AM 26·27: `memory_paths:null`.
   - N3 13: 같은 session_id로 암호어를 기억했다.
   - N4 14·15: 같은 session_id로 둘 다 rc 0이다.
   - N4a 14: `while pgrep -f "^sleep 45" …; echo done`이 실행됐고 거부는 0건이다.
   - 사용자 결정 원문의 "Edit 형식 채택"과 자동 기억 2회 확인도 맞다.
7. **자산 보완**:
   - `gh api`로 다시 조회한 값이 supplement 파일과 같다. secrets·variables·environments는 모두 0, contents 4개와 sha, tree 6개 항목(`truncated:false`), main `2ab9d87…`.
   - 착수 의존(04-07←02-06·02-02, 05-01←05-04·04-07, 05-02←05-01, 05-03←05-02)은 원장 표와 일치한다.
   - `repository-smoke`를 독립 판정과 구분했고, 준비 작업은 완료 수에서 제외했다.
8. **검증 스크립트**: `validate_package.py` rc=0, `render_progress.py --check` rc=0(`ok:true`).
9. **비밀 스캔**: 다음 16개 값을 대조했다.
   - 역할 토큰 2개(`/opt/rust/ieo_poc_secrets/*.token`)
   - E 비밀 5개(`/opt/rust/ieo_poc_probe/e-secrets/*`)
   - `~/.claude.json` oauthAccount 문자열 필드 9개

   대조 범위는 아카이브 트리 전체, 전체 diff, 커밋 메시지다. 일치는 0건이고, diff 안의 `sk-ant-` 토큰 형태도 0건이다. 값은 출력하지 않았다.

## 발견 사항

### P2-1 (차단) 모델 probe의 산출물 검사에는 "산출물 없음 = 관측 불가" 규칙이 적용되지 않았다. G1은 산출물이 없으면 자동으로 통과 쪽으로 분기한다

- 위치:
  - `probes/01-04/probe2.py:479`, `:484`
  - 같은 패턴: `:426`, `:444`(U·U-alt `files`), `:504`·`:529`·`:538`(G2·G3 `hook_files`), `:577`(L1 `podman diff`)
- 근거:
  - 사용자 지시 3은 "판정 기준에 필요한 결과 사건**이나 산출물**이 없으면" 성공 처리하지 말라고 한다.
  - 이번 교정은 결과 사건(모델 시도)과 모델 없는 시험(P2·E·E1b `t()`)에만 표식 판정을 넣었다.
  - 모델 probe에 딸린 산출물 검사는 모두 `unshare(..., check=False).stdout`을 그대로 기록한다. `podman unshare`가 실패하면 빈 문자열이 된다.
  - 특히 probe2.py:484 `g1_marker_fail = … or "HOOK-FIRED" in out["G1"]["files"]`는 `files`가 비어도 False가 된다. 그러면 "hook 미발동(합격)"으로 처리되어 G1-alt 대안 경로를 건너뛴다. 산출물 부재를 성공으로 처리하는 자동 분기다.
  - `BAD_WRITE_*`, `DENIED_FILE_*`, `NO_HOOK_FILE`/`HOOK-FIRED` 같은 결정 표식이 출력에 없는지는 어디서도 검사하지 않는다.
- 01-04 영향: 없음. 원본의 U·U-alt·U-alt-2 `files`에는 모두 `DENIED_FILE_ABSENT`가 있다. G1 `files`에는 `BAD_WRITE_ABSENT`가 있고, G2/G3 `hook_files`에는 `NO_HOOK_FILE` 또는 `HOOK-FIRED-*`가 있다. 따라서 영향 감사 결론은 유지된다. 다만 감사 범위(REPORT §1.2 표)에 이 산출물 검사들은 들어 있지 않으므로 그 행을 추가해야 한다.
- 요청:
  - 이 산출물 검사들에 `observe()`류 결정 표식 검사를 적용한다. 표식이 없거나 unshare rc≠0이면 `UNOBSERVABLE`로 기록하고 정지한다.
  - `g1_marker_fail` 분기는 관측 불가일 때 합격으로 진행하지 않게 한다.
  - `test_result_check.py`에 빈 `files` 사례를 추가한다.
  - REPORT §1.2·§1.3에 범위를 반영한다.

### P2-2 (차단) 계약 rev 6 항목 3: "변조 탐지 세션은 재개하지 않는다"는 규칙과 완료 조건이 모순되고, 새 규칙에는 완료 조건이 없다

- 위치:
  - `docs/poc/reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md:137`(§5-6)
  - 같은 파일 `:206`(§10 항목 3)
- 근거:
  - 항목 3과 §5-6은 "`CLAUDE_CONFIG_DIR` 변조가 탐지된 세션은 재개하지 않고 새 세션으로 시작"을 새로 정한다.
  - 그런데 항목 3의 완료 조건은 "표식 settings·CLAUDE.md·skills·`memory/` 파일을 넣고 정리하면 … **세션 재개는 성공한다**"이다. 표식을 넣는 것 자체가 변조이므로, 이 시험은 §5-6을 위반하는 동작을 합격 조건으로 요구한다.
  - "변조 탐지"의 방법(예: 정리 전 디렉터리 digest와 RO 템플릿 또는 직전 체크포인트 `settings_digest`의 대조)과 탐지 시 동작(재개 거부, 새 세션, 보고)에 대한 완료 조건은 없다. 사용자 지시 2는 항목별 완료 조건을 요구한다.
  - 이 규칙은 01-04 REPORT §1 항목 3("세션 기록 안의 이전 내용은 재개하면 남는다")에서 설계상 추론한 것이고, 사용자 결정 원문에는 없다. 이 점은 허용 범위지만, 그렇다면 더욱 완료 조건이 필요하다.
- 요청:
  - 완료 조건을 두 시험으로 나눈다. (a) 정리 효과: 표식이 없는 정상 세션에서, 정리 뒤 init에 표식·`memory_paths`가 없고 재개가 성공한다. (b) 변조 탐지: 표식을 주입하면 탐지가 기록되고, `--resume`을 시작하지 않으며, 새 session_id로 시작하고, 체크포인트와 코디네이터에 보고된다.
  - 탐지 기준을 한 줄로 명시한다.

### P3-1 결과 사건 선택이 "첫 번째" result이다. 동시 재개(N4b)에서는 첫 result가 0턴 빈 결과였다

- 위치: `probes/01-04/probe2.py:114`, `probes/01-04/probe.py:122`(`next(...)`)
- 근거:
  - `15-N4b-development/stdout.jsonl`에는 result 사건이 2개 있다. 첫째는 `num_turns:0`, `result:""`, `stop_reason:null`이고, 둘째는 `num_turns:1`, `result:"KESTREL-62F61D"`다.
  - N.json의 `N4.concurrent.result`는 `""`로 기록됐다.
  - 판정에 필요한 결과가 아닌 것을 "결과 있음"으로 받아들이는 경로다. 01-04 판정은 원본으로 했으므로 영향은 없다.
- 제안: 마지막 result를 쓰거나, result가 여러 개이면 모두 기록하고 `num_turns≥1`인 것을 판정용으로 지정한다.

### P3-2 `expected_outcome="no_result"`가 background/`finish` 경로에서 유실되고, 이 경로는 회귀 시험에 없다

- 위치: `probes/01-04/probe2.py:170-171`, `:187`
- `attempt(..., background=True, expected_outcome="no_result")`의 값이 반환 튜플에 없어서 `finish`의 extra로 전달되지 않는다.
- 결과적으로 안전한 쪽(정지)으로 동작하지만, 의도적 중단 시험을 묶음으로 돌리면 잘못 정지한다.
- `test_result_check.py`는 `finish`와 `stop_batch` 경로를 시험하지 않는다.

### P3-3 `DECISIVE`의 약한 패턴과 "표식 존재 = 관측"

- 위치: `probes/01-04/probe2.py:44-50`
- 맨 단어 `VISIBLE`·`ABSENT`, 그리고 줄 전체가 `200|201|401|409`인 패턴은 무관한 출력에도 걸릴 수 있다.
- `observed`는 표식이 하나라도 있는지만 보고 `expect`와 대조하지 않는다. 판정 자체는 여전히 사람이 보고서에서 한다.
- 시험별 기대 표식 집합(`expect_markers`)을 두는 쪽이 지시 취지("표식으로 판정")에 더 가깝다.

### P3-4 REPORT §1.3 서술이 probe.py 동작보다 넓다

- 위치: `docs/poc/reports/workspace/harness-result-check-20261112/REPORT.md:30`
- "`result_missing: true`, `judgeable: false`로 기록한다(`probe2.py stop_signals`, `probe.py stop_on_signal`)"라고 적었다.
- 그러나 `probe.py record_attempt`(probe.py:113-136)는 이 두 필드를 기록하지 않는다. 정지만 한다.

### P3-5 계약 안에 rev 6과 어긋나는 낡은 "가정·probe G에서 확인" 문구가 남았다

- 위치: `Runtime_Contract_v0.md`
  - `:3`: "동작은 01-04 probe 전까지 미확인"
  - `:102`: "probe G가 … 로드되는지 확인한다"
  - `:103` 끝: 자동 기억의 위치·비활성 방법은 "가정이며 probe G에서 확인"이라고 적혀 있다. 같은 줄 앞부분은 rev 6에서 `projects/*/memory/`로 고쳤다.
  - `:104`: 경로 한정 규칙은 "가정"
  - §4.2: 자식 사용량은 "별도 확인"
- 01-04에서 확인됐거나(G·U·AM·T) §10에서 확정한 내용이다. 수정하거나 "(01-04에서 확인, §10)"으로 정리하면 개정 범위 안에서 일관된다.

### P3-6 항목 1의 근거 "G1(Guardrail scratch 쓰기는 `Edit(//…)`로 성공)"은 과한 귀속이다

- 위치: `Runtime_Contract_v0.md:204`
- G1의 허용 인자에는 `Write(//state/guardrail-scratch/**)`와 `Edit(//state/guardrail-scratch/**)`가 함께 있었다(probe2.py `gargs`). G1만으로는 성공이 Edit 규칙 덕분인지 구분할 수 없다.
- 직접 근거는 U(01)/U-alt(02·04)의 대조다.
- rev 6의 Guardrail 규칙 집합(Edit만 있고 `Read(//state/inputs/**)`가 추가됨) 자체는 시험되지 않았다. 02-04/03-02의 완료 조건으로 다시 확인한다는 문구를 두는 것이 정확하다.

### P3-7 ASSETS §3의 "원장 각 작업의 `unresolved`에 같은 문구로 기록"은 아직 사실이 아니다. G 번호도 겹친다

- 위치: `docs/poc/reports/workspace/test-env-assets-20261112/ASSETS.md:55`
- 이 PR은 원장이나 `unresolved` 필드를 바꾸지 않았다. 저장소 어디에도 해당 문구가 없다.
- "다음 원장 갱신에서 기록한다"처럼 시점을 명시해야 한다.
- 이 문서의 G1~G5는 01-07 계약 공백 G1~G8, 01-04 probe G1/G2/G3와 이름이 겹친다. 접두사(예: GH1~GH5)를 쓰면 혼동이 줄어든다.

### P3-8 세션 잠금 위치(§5-5)

- 위치: `Runtime_Contract_v0.md:136`
- "역할 볼륨 안 supervisor 소유 파일" 잠금은 개발 컨테이너에서 개발 코드가 지울 수 있다(§2.1, 같은 컨테이너).
- 영향은 개발 역할 범위 안이라 계약 원칙과 충돌하지는 않는다. 다만 단일 실행 보장의 한계로 명시해 두면 좋다.

## 요약

- 영향 감사는 독립 재계산과 일치한다: 35/35에 result가 있고, 종료 0인데 결과가 없는 시도는 0건이며, 비0 종료는 P2c 하나(의도한 거부)다. 33/33이 표식 판정이다. 원본은 불변이다. 따라서 01-04의 판정과 accepted 상태에는 영향이 없다.
- 차단은 두 건이다.
  - P2-1: 모델 probe 산출물 검사에 관측 불가 규칙이 없다. G1 분기는 산출물이 없을 때 합격으로 진행한다.
  - P2-2: 계약 항목 3의 "변조 세션 재개 금지" 규칙과 완료 조건이 모순되고, 이 규칙의 완료 조건이 없다.
- 두 건 모두 작은 수정으로 해소할 수 있다. 01-04 재실험은 필요 없다.
