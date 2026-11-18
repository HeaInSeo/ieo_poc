# 09-01 · 결과 처분·자산 목록·스프린트 변환기

`task/0901-result-export` · 2026-11-18 KST · 로컬 구현 및 합성 대조 완료 · 독립 검수 대기

[09-01.accept](../../../../Console_Coordinator_Sprints_v0.json)과 [T09·§2.3·§5.3·§5.6·P33](../../../../Console_Coordinator_PoC_Validation_Plan.md)을 적용했다. 원장 `v1.3.3-EXEC-0058`에 02-01·02-02가 모두 `검수 완료 / verified`다. 02-01은 PR #21의 검토 승인과 병합 `a0c59fd`, 02-02는 PR #23의 검토 승인과 병합 `dca7911`을 양쪽 보고에서 확인했다. 02-02의 개발 계정 정답 접근 차단은 미검증으로 이월됐다. [진행 원장](../../../../progress/state.json) · [02-01 검수 보고](../../02-01/collector-20261112/REPORT.md) · [02-02 검수 보고](../../02-02/eval-20261112/REPORT.md)

## 산출물

[변환기](../../../../scripts/export_results.py)는 로컬 자료에서 다음 산출물을 만든다.

| 파일 | 내용 |
| --- | --- |
| `export.json` | 회차·후보와 대상 버전, 원 판정과 증거 대조 결과, 제안 처분과 유효 처분, 목표·범위·작업·자산 연결, 누적 예산, 출처 SHA-256과 변경 영향 |
| `DRAFT.md` · `sprint.json` | 사람이 검토할 최종 개발 스프린트 초안과 등록 계약 v0 명세. 수용 기준은 기존 승인 프로파일에서 가져오며 등록 형식·참조 검사를 통과해야 한다. |
| `input.json` · `progress/` | 정렬한 입력과 기존 생성기로 만든 원장 스냅샷. 주계획 73개·후속 12개와 역사적 v1.2 0/76을 별도로 기록한다. |

예제 명세와 출처 사본은 [sprint.json](evidence/sprint.json)과 [input.json](evidence/input.json)에 보존했다. 전체 산출물은 아래 명령으로 재생성한다. 같은 입력을 다시 변환하면 기존 디렉터리를 돌려주고 변경 시 새 `rev-N`을 만든다. 이전 결과·작업 ID와 현재 리비전의 재검증 범위를 함께 기록한다. 출력 디렉터리는 한 스프린트·실행 종류에 사용한다.

## 입력과 판정

- [입력 묶음](fixtures/input.json)은 대상과 시각, 등록 명세의 원 작업·범위·예산, 회차 보고·처분·계측 파일, 자산, 후속 작업을 연결한다. 작업 ID는 명시적으로 배정하며 같은 ID를 다른 목표·범위에 쓰면 거부한다. 필수 범위에 작업이 빠졌거나 채택 증거가 쌓이지 않은 경우 목표 대응표에 남긴다. 선행 누락·순환·ID 충돌은 `ieo-spec check`가 거부한다.
- 회차의 원본은 `console.poc.run.v1`이다. 원 보고와 판정을 보존하고, 원증거·대상 버전·독립 근거에 공백이 있으면 대조 판정을 `INCONCLUSIVE`로 남긴다. 처분 제안은 별도 파일이다. 신뢰 출처에서 제공하는 `--authority`는 모의/실제 종류와 허용한 보고 v0의 경로·SHA-256만 담는다. 대조에는 검토 주체·원 보고 digest·처분 digest가 모두 맞아야 한다. 이 입력을 지정하지 않으면 승인된 처분이 없다. 로컬 대조의 신뢰 경계는 호출자가 해당 승인 목록의 출처를 확인했다는 데 있다.
- 검증 대상과 정확히 일치하는 범위만 채택한다. 개별 모듈의 검증 근거와 파일 digest가 있으면 방안 기각 뒤에도 재사용할 수 있다. 목표 환경 통합은 별도 작업에 둔다. 보류에는 담당·재개 조건·재확인 시각을 붙인다. 부족한 증거를 보완하거나 처분을 변경할 때 새 결과 revision으로 연결한다.
- 예산은 동일 목표에 연결된 모든 회차를 같은 한도로 집계한다. 후보 ID 변경이나 보고 revision 증가로 초기화되지 않는다. `ieo-evidence summarize`의 부모·자식 포함 회차 사용량을 개발·실패·중단·검토에 적용하며 같은 기록의 중복 입력을 거부한다. 승인한 토큰 필드만 더하고, 미계측 또는 누락이 있으면 계측 부분합을 별도로 보존한 채 전체를 `null`로 둔다. `automatic_retry`는 이 예산에서 반복을 막는 조건이 있는지를 나타낸다. 변환기는 실행을 배정하지 않는다. 남은 작업 공수는 별도 근거가 없으면 `unestimated`다.
- 직접 작성한 결과 입력은 `execution_kind`에 `synthetic` 또는 `actual`을 표시한다. 이전 결과의 값과 버전을 그대로 읽고, 현재 종류·범위가 입증되기 전에는 채택에 쓰지 않는다. 첫 입력의 목표 범위는 독립 검토가 대조할 대상이다. 실제 자료와 승인 범위 대조는 09-05에서 증거를 확보한 뒤 수행한다.

## 재현과 검증

저장소 루트에서 프로젝트의 Python 3.12 및 `tools/requirements-poc.txt` 환경을 사용한다.

```sh
cargo build --locked -p ieo-evidence -p ieo-spec
report=docs/poc/reports/tasks/09-01/result-export-20261118
python "$report/check_export.py" --collector "$PWD/target/debug/ieo-evidence" --spec "$PWD/target/debug/ieo-spec"
python docs/poc/scripts/export_results.py \
  --input "$report/fixtures/input.json" --authority "$report/fixtures/authority.json" \
  --out target/09-01-example --collector "$PWD/target/debug/ieo-evidence" --spec "$PWD/target/debug/ieo-spec"
```

입력과 기대값을 구현과 분리하고 [해시 목록](fixtures.sha256)으로 고정했다. 기대 처분과 기준은 계획의 P33 표에서 구현 코드와 별도로 옮겨 작성했다. 합성 승인·관측 기록은 고정 픽스처다. 구현 작성자가 CLI를 대조했으며 별도 담당의 기대값 승인과 P33 독립 판정은 09-02에서 수행한다. 초기 픽스처의 보고 시각을 회차 이후로 정정한 뒤 외부 CLI 대조를 실행했다. [기대값](fixtures/expected.json) · [대조 결과](evidence/targeted-checks.txt)

공유 `state.json`과 생성 화면에 쓰지 않았다. 모델 요청 0회, 신규 과금 0, 실서버 변경 0이다. #96은 병합됐고 마지막 조회에서 작업 공간의 활성 수정 PR은 X-05 #97이었다. 09-01은 로컬 커밋으로 보존하고 PR은 작업 소유권과 활성 수정 PR 순서에 따라 넘긴다. 제품 상태와 09-05 실제 연결은 미검증이다.
