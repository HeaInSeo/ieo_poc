# 01-04 후속: 하네스의 결과 사건 요구 · 런타임 계약 revision 6 · 기록 보완

2026-11-12 KST · 계획 v1.3.3-A1 · 근거: 사용자 지시 2026-11-12(01-05 진행 지시의 보완 2~5). **작업 수에 더하지 않는다.** 01-04 원본 증거는 고치지 않고, 분석·정정 기록만 여기에 더한다.

## 1. 종료 코드 0·결과 사건 누락 (보완 3)

### 1.1 결함

PR #14 검토 a3d01ce §5가 남긴 잔여 위험이다.
- 하네스의 정지 검사(`probe2.py stop_signals`, `probe.py stop_on_signal`)는 결과 사건이 없어도 종료 코드가 0이면 시도를 통과시켰다.
- 모델 없는 시험의 기록에는 판정 근거 표식을 따로 남기는 필드가 없었다. 그래서 종료 코드 0만으로 성공처럼 보일 여지가 있었다.

### 1.2 01-04 판정에 대한 영향: 없음

원본 로그를 먼저 대조했다. 원본은 바꾸지 않았다.

| 대상 | 대조 방법 | 결과 |
| --- | --- | --- |
| 모델 요청 시도 35건(첫 run 9건, 나머지 run 26건 = 예약 27 중 미기동 1 제외) | 각 `attempts/*/stdout.jsonl`에서 `type=result`·`init` 사건 수와 종료 코드를 셌다([audit_result_events.py](../../../../../probes/01-04/audit_result_events.py)) | 35건 모두 결과 사건 ≥1. 종료 코드 0인데 결과가 없는 시도 0건. 0이 아닌 종료는 P2c(시도 05, 종료 1, `error_during_execution`) 하나뿐이고, 이것은 의도한 거부 시험이다. 판정은 기대 결과(“No conversation found”)로 했다 |
| 미기동 예약 1건(나머지 run 시도 03) | budget.json `launched: false` | 판정에 쓰지 않았다(ADDENDUM §2) |
| 모델 없는 시험 33건(P2 10, E 16, E1b 7) | 각 출력에 판정 표식(`*_OK`/`*_DENIED`/`ABSENT`/HTTP 코드/ref SHA 등)이 있는지 | 33건 모두 표식이 있다. 판정은 표식으로 했고 종료 코드만으로 판정한 시험은 없다 |

원본: [evidence/audit_result_events.txt](evidence/audit_result_events.txt), [evidence/test_result_check.txt](evidence/test_result_check.txt)(아래 §1.3 시험의 33건 회귀 포함).

**결론.** 이 결함은 01-04의 어떤 판정에도 영향을 주지 않았다. 판정과 검수 완료 상태는 그대로다.

### 1.3 교정 (하네스를 다시 쓰기 전)

- **모델 요청 시도**:
  - 결과 사건이 없으면 종료 코드와 무관하게 `result_missing: true`, `judgeable: false`로 기록한다. 그 시도는 관측 불가로 두고 새 시도를 시작하지 않는다(`probe2.py stop_signals`, `probe.py stop_on_signal`).
  - 의도적인 중단 시험은 `attempt(..., expected_outcome="no_result")`로 표시한다. 이런 시험은 정지하지 않고 자기 기대 결과로 판정한다. 의도적인 거부 시험(예: P2c)은 오류 결과 사건이 기대 결과다.
- **모델 없는 시험**: 출력에서 판정 표식(`DECISIVE`)을 찾아 `markers`와 `judgement_basis`를 기록한다. 표식이 없으면 `UNOBSERVABLE`이고 종료 코드는 쓰지 않는다(`probe2.py` E·E1b, `probe.py` P2).
- **회귀 시험**: [test_result_check.py](../../../../../probes/01-04/test_result_check.py). 모델·podman·토큰 출력 없이 실행한다.
  - 합성 시도: 종료 0·결과 없음은 정지·판정 불가, 의도적 무결과는 판정 가능·비정지, 정상은 통과.
  - `probe.py` 회차 검사의 두 경우.
  - 빈 출력·무관한 출력은 관측 불가.
  - 기록된 01-04 모델 없는 시험 33건 모두 관측됨.
  - 결과: `RESULT PASS 0`.
- 추가 모델 요청은 없다(01-04 B2 27/30 그대로).

## 2. 런타임 계약 revision 6 (보완 2)

[Runtime_Contract_v0.md](../../tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md)를 revision 6으로 개정했다.
- 01-04 실측 6건을 §2.1·§4.1·§4.2·§5·§7에 반영했다.
- 새 §10 표에 항목별 근거(시도 번호·결정 원문), 적용 위치, 완료 조건을 적었다.
- 01-04 실험은 다시 하지 않는다.
- 완료 조건: 이 revision이 검수 완료되는 것. 그 위에 항목별 구현 시험이 붙는다.
  - 02-04(supervisor) 착수 전: 항목 1~5.
  - 03-02(정책 문서): 항목 1·6.
  - P07: 항목 6.
- §8의 “이후 probe”는 01-04에서 실행·검수 완료됐음을 적었다.

## 3. 기록 보완 (보완 4·5)

- **base 갱신 head CI run**: `7c365d1`의 필수 CI는 run 43181686192(3 job success, 08:17:57Z)였다. 다음 원장 갱신 event에 연결한다(이 PR 병합 뒤 원장 보고 PR).
- **자산 목록 근거**: Actions secret·variable·environment 개수와 전체 tree를 보완 조회했다. [github-ieo_poc_test-supplement.txt](../test-env-assets-20261112/evidence/github-ieo_poc_test-supplement.txt)는 값 없이 개수와 이름만 담는다. ASSETS.md §1에 연결했다. 원래 조회 원본은 그대로 둔다.
- **G1~G5 착수 조건**: ASSETS.md §3에 작업별 착수 조건 표를 넣었다(04-07: G1·G2·G3, 05-01: G3·G5, 05-02: G3, 05-03: G1·G2·G4·G5). 연결 확인용 CI와 독립 판정을 구분했고, 준비·보완 기록은 완료 수에 넣지 않는다.
- **하네스 잔여 위험의 처분**은 §1이다. 원장 이월 목록에는 “교정·영향 없음”으로 반영한다.
