# 01-06 시험실 초기 상태·기록 위치·정리 절차

계획 v1.3.3-A1 · 01-06 run `lab-20261112` · 도구 [probes/lab/lab.py](../../../../../../probes/lab/lab.py)(모델 요청 없음)

## 1. 범위 보호

- **건드리는 것**: `/opt/rust/ieo_poc_probe` 아래 경로와 이름이 `ieo-probe-*`인 podman 컨테이너·네트워크뿐이다. `guard_path`가 그 밖의 경로를 거부하고, podman 자원은 이름 접두로 거른다.
- 그 밖의 컨테이너(예: `local-registry`)는 목록에만 적고 건드리지 않는다.
- systemd timer는 읽기만 한다.
- **자격**:
  - `/opt/rust/ieo_poc_secrets`(사용자 발급 토큰)는 경로·모드·파일 이름만 본다.
  - 역할 `cred/` 사본과 `e-secrets/`는 보관본에서 빠진다(`--exclude`). 보관 뒤 목록을 다시 검사하고, 자격 경로가 있으면 중단한다.
  - `reset`은 `bin/`(CLI 사본)과 `e-secrets/`를 남긴다.
- 재부팅이나 사용자 systemd 관리자 조작은 하지 않는다(01-05 현재 운영 §2).

## 2. 사례별 초기 상태

`lab.py init`(= `probe.py setup` → `probe2.py e` → `probe2.py e1b`)이 만드는 상태다. `lab.py verify`의 기대값은 `EXPECTED`다.

| 사례 | 초기 상태 | 확인 |
| --- | --- | --- |
| 역할 볼륨(개발·Guardrail) | `roles/<role>/{state,cred,etc}`. 컨테이너 UID 20001/20002 소유(호스트 185536/185537). `state`·`cred` 0700, `cred/oauth_token` 0400(사용자 토큰 사본). 세션 기록 0건 | verify: 모드·UID·세션 0 |
| 시험 repo A·B(E1) | `testrepos/{a,b}.git` bare. `main` 보호 `pre-receive` hook. 새 seed 커밋(실행마다 SHA가 다름) | E.json E1 4건 |
| forge(E1b) | `forge/a.git` UID 20005 소유 0700. 컨테이너 `ieo-probe-forge`(git daemon)는 시험 뒤 정지 | E1b.json 7건 |
| DB(E2) | 컨테이너 `ieo-probe-pg`(postgres:17), 역할 `ieo_coord`·`ieo_status_reader`, 스키마 `ieo`. 시험 뒤 정지 | E.json E2 6건 |
| 보고 수신기(E3) | 컨테이너 `ieo-probe-rx`(소스 `probes/01-04/receiver`), 데이터 `rx-data/`. 시험 뒤 정지 | E.json E3 6건 |
| 네트워크 | `ieo-probe-coord`, `ieo-probe-forge`(둘 다 internal) | verify |
| 없는 것 | `roles-recreated`, `snapshots-g*`, `g2-src`, `g3-cfg`(probe 중간 산출물) | verify |

## 3. 원본 로그·버전·시각·사용량 기록 위치

| 기록 | 위치 | 담는 것 |
| --- | --- | --- |
| 모델 요청 시도 원본 | `reports/tasks/<task>/<run>/evidence/attempts/NN-<probe>-<role>/` | `stdout.jsonl`(stream-json 원문, 토큰·해시 가림), `stderr.txt`(환경 변수 이름, UID, 컨테이너 안 시각 `#T_START/#T_END`), `meta.json` |
| 시도별 요약 | 같은 폴더의 `meta.json` | argv, UTC 시작·종료, 종료 코드, 시간 초과, 결과 사건(`usage`·`modelUsage`·`total_cost_usd`·`num_turns`), `rate_limit_event`, 토큰 지문, 누적 시도·남은 벽시계(01-04 나머지 run부터). `result_missing`·`judgeable`은 PR #16 이후 하네스부터 있다(01-04 원본에는 없음) |
| 누적 예산 | `evidence/budget.json` | 시도 예약·기동 여부, 첫 시도 시각, CLI 기동(모델 없음) 목록 |
| 버전 | `evidence/setup.json`, `E.json` `info.images` | podman rootless·subuid, 볼륨 소유, 이미지 ID·digest, CLI 사본 sha256, 수신기 소스 sha256 |
| 모델 없는 시험 | `P2.json`, `E.json`, `E1b.json` | argv, rc, 출력. 판정 표식(`markers`)·`judgement_basis`는 PR #16 이후 하네스부터 있다(01-04 원본에는 없고, 01-06 init-1·2에는 있음) |
| 작업 결과 | `reports/tasks/<task>/<run>/result.json` | 입력 버전, 기대·관측, 비용 필드(미계측이면 이유를 적고 0으로 쓰지 않음). 01-04 run에는 없고 01-01·01-03·01-05·01-06에는 있다 |
| 진행 원장 | `progress/state.json`(유일 원본) → `render_progress.py`가 STATUS·TASKS·RESULTS·RESUME·events.jsonl 생성 | 작업 상태, 사건 `observed_at`, 재개 정보 |
| 시험실 상태 | `lab.py inventory` JSON(이 run의 `evidence/inv-*.json`) | 디렉터리 모드·소유·항목 수·이름 digest, 역할 볼륨 상태, `ieo-probe-*` 자원, 기존 timer 상태 |
| 시험실 보관본 | 저장소 밖 `/opt/rust/ieo_poc_archive/<시각>/lab-state.tar.gz`(0600, 디렉터리 0700)와 `MANIFEST.json` | 정리 전 전체 상태(자격 제외). sha256은 이 run의 [archive-manifest.json](evidence/archive-manifest.json) |
| 미계측 | 개발 대화 세션 자체의 사용량 | 수단이 없다. result.json `unmeasured_reason` |

## 4. 정리 절차

1. `lab.py inventory --out <before>`
2. `lab.py archive --dest /opt/rust/ieo_poc_archive/lab-<UTC>`. 정리 전 증거를 보존한다. 01-04의 세션 기록 원문도 여기 남는다.
3. `lab.py reset --yes --archived <2의 MANIFEST.json>`(다시 만들 수 있는 상태만이면 `--no-archive-needed`). `ieo-probe-*` 컨테이너·네트워크와 시험실 데이터 디렉터리를 지운다. 남는 것은 `bin`·`e-secrets`뿐이다.
4. `lab.py init --evidence <dir>`. 모델 요청 없이 초기 상태를 만들고 E·E1b 허용/거부를 다시 시험한다. 하위 probe의 시험실 경로를 고정하고, 결과를 `check_init`으로 판정한다. 하나라도 어긋나면 실패다.
5. `lab.py inventory --out <after>` → `lab.py verify --inventory <after>`. 기대 초기 상태와 대조한다.
6. 공유 작업공간(`/opt/rust/src/github.com/HeaInSeo/ieo_poc`)은 병합마다 `main`으로 되돌린다(아래 §5).

실험 첫 실행 전에는 1·5(inventory·verify)를 실행한다. 실험 사이에 깨끗한 상태가 필요하면 2~5를 실행한다. 타이머나 상주 프로세스는 필요 없다.

## 5. 열람 경로

| 경로 | 상태 |
| --- | --- |
| 호스트 공유 작업공간 고정 경로 `/opt/rust/src/github.com/HeaInSeo/ieo_poc/docs/poc/progress/STATUS.md` | 존재(0664, heain). 이 경로의 내용은 그 작업 사본의 브랜치를 따른다. 확인 시점에는 보고 브랜치에 남아 있었고, `main`으로 되돌려 같은 snapshot을 확인했다(viewing-paths.txt). 병합 뒤 `main`으로 되돌리는 것을 절차에 넣었다 |
| GitHub `main` STATUS·RESULTS·TASKS(blob)·raw STATUS | 익명 HTTP 200. raw 머리말 snapshot `v1.3.3-EXEC-0010`이 원장과 같다 |
| 결과 인덱스 | [RESULTS.md](../../../../progress/RESULTS.md)(실험 회차), [TASKS.md](../../../../progress/TASKS.md)(작업·최근 보고 링크) |
| 사용자의 실제 열람 | **미확인**. 접근 가능함만 확인했다 |
