# 01-06 시험실 초기화·증거 보존 경로

**열람 경로**(수용 기준의 “첫 응답”):
- 호스트 공유 작업공간: `/opt/rust/src/github.com/HeaInSeo/ieo_poc/docs/poc/progress/STATUS.md`
- GitHub: https://github.com/HeaInSeo/ieo_poc/blob/main/docs/poc/progress/STATUS.md
- 결과 인덱스: [RESULTS.md](../../../../progress/RESULTS.md), 작업·보고 링크 [TASKS.md](../../../../progress/TASKS.md)
- 사용자의 실제 열람은 확인하지 않았다(접근 가능함만 확인).

2026-11-12 KST · 계획 v1.3.3-A1 · run `lab-20261112` · 선행 01-04·01-05 검수 완료 · 수용 프로파일 rev 4 `01-06.accept` · 모델 요청 0회

## 1. 수용 기준 대응

| 수용 기준 문구 | 결과 | 근거 |
| --- | --- | --- |
| 사례별 초기 상태, 원본 로그·버전·시각·사용량 기록 위치와 정리 절차를 **재현**한다 | 보관(1회, 01-04 상태 보존) → 정리 → 초기화 → 대조를 실행했고, 이어 정리 → 초기화 → 대조를 한 번 더 했다(두 번째 정리는 init-1이 다시 만들 수 있는 상태만 지웠다). 두 번 모두 기대 초기 상태와 일치했다. 두 초기화 결과는 구조가 같다. 디렉터리·모드·항목 수·역할 볼륨·컨테이너·네트워크가 같고, 이름 digest만 새 seed 커밋 SHA 때문에 다르다. 초기화마다 E 16건·E1b 7건 허용/거부가 기대대로였다 | [Lab_Procedure.md](Lab_Procedure.md) §2~§4. evidence/`inv-0..3`, `archive-manifest.json`, `reset-1/2.json`, `init-1/2/`, `verify-1/2.json` |
| 후속 실험에 필요한 환경만 준비 | 01-04에서 쓴 환경(역할 볼륨, repo·forge, DB, 수신기)의 초기화만 만들었다. 클러스터·관측기(08-02), GHCR, 역할별 GitHub 자격(GH1~GH5, 사용자 발급)은 쓰는 실험이 준비한다 | Lab_Procedure §2. 시험 환경 자산 목록(test-env-assets) |
| STATUS 고정 경로와 결과 인덱스/보고 링크를 첫 응답으로 제공 | 이 문서 첫 절, Lab_Procedure §5 | evidence/[viewing-paths.txt](evidence/viewing-paths.txt): 호스트 경로 존재, GitHub 4개 URL 익명 200 |
| state.json만 진행 원본으로 갱신하고 render_progress.py로 STATUS/TASKS/RESULTS 및 파생 이력/재개 화면 생성 | 생성기 `scripts/render_progress.py`를 이 작업의 산출물로 포함한다. 01-01부터 써 왔고, 이번에 쓰기 방식을 보강했다(§2) | 원장 갱신 PR들(#8~#18)이 모두 이 생성기로 화면을 만들었다 |
| 손편집·타이머를 요구하지 않음 | 생성기에 시간·스케줄·네트워크 import가 없다(시험 7). 손편집은 `--check`가 잡는다(시험 2). 시험실 절차도 타이머 없이 명령으로 실행한다 | [test_render_progress.txt](evidence/test_render_progress.txt) |
| 정상/실패·증거 링크·중단 뒤 재생성 확인 | `scripts/test_render_progress.py` 26건 PASS. 정상과 멱등, 손편집 탐지, 증거 없는 검수 완료 거부(쓰기 없음), 링크 규칙(없는 파일·http·상위 경로·절대 경로 거부, https 허용), 기준선 수 불일치 거부, 중단 모사 뒤 `--check` 탐지와 재생성, 남은 임시 파일 탐지·제거 | 같은 파일 |
| 사용자 열람 경로 미확인을 숨기지 않음 | 사용자의 실제 열람은 미확인이라고 적었다. 호스트 경로 내용이 그 작업 사본의 브랜치를 따른다는 한계와 처리도 적었다 | Lab_Procedure §5 |

## 2. 생성기 보강과 그 근거

- **변경**: 이전 생성기는 화면마다 임시 파일을 쓰고 곧바로 교체했다. 이제는 모든 화면을 임시 파일로 먼저 쓴 뒤 한꺼번에 교체하고, 시작할 때 이전 중단이 남긴 `*.tmp`를 지운다. `--check`는 남은 `*.tmp`도 재생성 필요로 보고한다.
- **정직한 비교**: 같은 시험을 이전 생성기로 돌린 결과는 [test_render_progress_old_renderer.txt](evidence/test_render_progress_old_renderer.txt)에 있다. 중단 모사 뒤 재생성은 이전 생성기도 통과한다. 화면마다 교체하므로 다시 실행하면 복구된다. 차이는 한 건이다. 모든 화면이 최신인데 임시 파일만 남은 경우를 이전 `--check`는 통과시킨다(`stray_tmp_flagged_by_check` FAIL). 이 보강은 결함 교정이 아니라 탐지 범위를 넓힌 것이다.

## 3. 보관과 정리에서 지운 것

- 정리 전 보관본은 `/opt/rust/ieo_poc_archive/lab-20261112T091332Z/lab-state.tar.gz`다. sha256 `a26b7290ee0d…`, 564항목, 자격 제외. 01-04의 역할 볼륨 세션 기록 원문과 probe 중간 산출물이 여기 남는다.
- 저장소의 01-04 증거(`reports/tasks/01-04/`)는 그대로다.
- 정리 대상은 `ieo-probe-*` 컨테이너 3·네트워크 2와 시험실 데이터 디렉터리 9개다. `bin`·`e-secrets`는 남겼다.
- 01-05가 넘긴 report bridge timer는 inventory마다 다시 조회한다. 이번 상태는 enabled·active·elapsed다(`inv-*.json` `timers`).

## 4. 검토 반영(b301843: 독립 검토 approve, 자동 검토 P1·P2 2건)

- **자동 검토 P1**(상속된 `IEO_PROBE_BASE`/`IEO_PROBE_SECRETS`가 init의 하위 probe를 시험실 밖으로 보낼 수 있음): `child_env()`가 두 변수를 시험실 값으로 고정한다. 다른 값을 상속하면 거부한다.
- **자동 검토 P2**(상대 경로 `--dest`): 처음에 한 번 절대 경로로 풀고, 셸 인자는 따옴표로 감싼다. 저장소 안 목적지도 거부한다(독립 검토 P3-4).
- **자동 검토 P2**(init이 E/E1b 결과를 보지 않고 성공 보고): `check_init()`이 시험마다 필요한 표식과 금지 표식, DB 스키마 rc를 판정한다. 맞지 않으면 init이 실패한다. 기존 init-1·2 증거에 사후로 실행해 둘 다 통과했다(각 `init-check.json`).
- 독립 검토 P3 처리:
  - reset은 직전 보관 MANIFEST(`--archived`, sha256 확인)를 요구한다(P3-5).
  - 보관 제외에 `*credentials*`를 더했다(P3-3).
  - 항상 참이던 검사를 지웠다(P3-6).
  - CI가 `test_render_progress.py`를 실행한다(P3-7).
  - 기록 위치 표에 형식이 생긴 시점을 적었다(P3-2).
  - 시험 환경 자산 목록과 01-04 재현 명령이 가리키던 상태가 정리됐음을 적었다(P3-7).
- 검사 원본은 [lab-guard-checks.txt](evidence/lab-guard-checks.txt)다. init-1·2 통과, 음성 대조 2건 실패 검출, 거부 3건, 상대 경로 보관 정상이 들어 있다. 모델 요청은 없다.

## 5. 한계

- 보관본은 이 호스트에만 있다. 호스트 장애에는 보존되지 않는다. 보관본의 sha256은 저장소에 남는다.
- 모델 요청이 필요한 사례(역할 세션 재개 등)의 초기화는 세션 기록 0건 상태로 끝난다. 세션을 만드는 시험은 각 실험 예산으로 한다.
- 초기화 결과의 git 객체 SHA는 실행마다 다르다. 같은 SHA가 필요한 실험은 seed를 고정한 별도 절차가 필요하다.
- 01-04 증거가 가리키던 역할 볼륨(예: 첫 세 probe REPORT §8의 `identity_claims.py` 재현 명령, 시험 환경 자산 목록의 `roles-recreated`·`snapshots/73ea669…`)은 정리됐다. 그 상태는 보관본 `/opt/rust/ieo_poc_archive/lab-20261112T091332Z`에만 있다. 재현하려면 보관본을 시험실 밖에 풀어 쓴다.
