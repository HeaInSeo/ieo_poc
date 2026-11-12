<!-- 보관본: PR #19 독립 검토 b301843 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/19#issuecomment-6819840011 (머리말·끝 안내 생략) -->
# 독립 검토: PR #19 (01-06 시험실 초기화·진행 화면 생성기) — head b301843

- 검토자 역할: Guardrail(독립 검토). 작성에 참여하지 않음. 01-06 검토 1회차(상한 4)
- 대상: `task/01-06-lab` b301843, 기준 main 417d6e3
- 수행 방식: `git archive b301843`/`417d6e3` 트리(스크래치)에서 시험·검증기 실행, `lab.py inventory`/`verify`(읽기 전용)만 실행. reset·init·archive는 실행하지 않음. 모델 요청 0회. 저장소·GitHub·systemd·podman·시험실에 쓰지 않음(인벤토리 출력은 스크래치에만 저장)
- 검토 시각: 2026-11-12

VERDICT: approve

P1·P2 없음. P3 7건(비차단, 후속 정리 권고).

## 1. 확인 결과 요약

### 1.1 생성기 변경(render_progress.py)
- 변경은 `main()`의 쓰기 경로뿐이다(`git diff 417d6e3 b301843`). `render()`·검증 규칙은 그대로라 화면 내용은 바뀌지 않는다.
- 이전 생성기(sha256 `7e1a152c…`)를 b301843 트리에 넣으면 `--check`가 통과한다. 같은 원장에서 바이트가 같은 화면을 만든다는 뜻이다.
- `test_render_progress.py`(아카이브 트리, 임시 사본만 사용): 새 생성기는 26 PASS, exit 0이고 증거 `test_render_progress.txt`와 PASS/FAIL 줄이 일치한다. 이전 생성기는 25 PASS 1 FAIL(`stray_tmp_flagged_by_check`), exit 1이고 `test_render_progress_old_renderer.txt`와 일치한다.
- REPORT §2의 비교 주장(“결함 교정이 아니라 탐지 범위 확대”)은 사실이다. 이전 생성기의 표기 `main 4bcba6a`는 417d6e3과 같은 파일이다(sha256 동일, 기준선 d92b414 이후 변경 없음).
- `render_progress.py --check`: ok, rc 0. `validate_package.py`: rc 0(delivered 120, amended 12). `validate_contracts.py`: rc 0.
- Amendment 항목: `original_sha256 7e1a152c…`는 `SOURCES.json:553`의 전달본과 같고, `sha256 6f2ac232…`는 b301843의 파일과 같다.

### 1.2 lab.py 안전성
- inventory: 읽기 전용(find/stat, podman ps·inspect·network ls, `systemctl --user show`)이다. 비밀은 파일 이름만 기록한다.
- reset: 삭제 대상은 `LAB/<상수 DATA_DIRS>`(guard_path)와 이름이 `ieo-probe-*`인 컨테이너·네트워크뿐이다. `bin`·`e-secrets`는 남긴다. 그 밖의 항목이 있으면 지우지 않고 오류로 끝낸다.
- archive: dest가 시험실 안이면 거부한다. 결과는 0600 파일, 0700 디렉터리다. 자격 경로 재검사도 한다.
- 실측 확인:
  - 보관본 sha256 `a26b7290…`와 크기 5228400이 MANIFEST와 같고, 저장소 `archive-manifest.json`과도 필드가 모두 같다. 모드는 0600이다.
  - tar 목록 564항목이다. inv-0의 9개 디렉터리 항목 수 568 − cred 4(역할 2개의 `cred/`와 `oauth_token`) = 564로 맞는다. `/cred` 경로는 0건이다.
  - 사용자 토큰 2개와 E 비밀 5개의 실제 값을 보관본 전체(압축 해제한 스트림)와 PR의 모든 01-06 산출물에서 대조했다. 일치 0건이다(값은 출력하지 않았다).
  - 호스트 `~/.claude.json` oauthAccount의 식별 필드(email, accountUuid, organizationUuid, displayName, fullName, organizationName)도 일치 0건이다. 일치한 것은 `hasExtraUsageEnabled`·`organizationType` 같은 일반 값 문자열뿐이다.
  - 보관본 안 역할 `.claude.json`에는 `oauthAccount`·`primaryApiKey`가 없고 `userID`·`machineID`만 있다. `sk-ant-` 패턴은 0건이다.
- 한계는 P3-3·P3-4·P3-5에 적었다.

### 1.3 재현 주장
- verify-1·verify-2: 각 25개 검사가 모두 ok다. 두 파일은 `at`·`inventory` 외에 같다.
- inv-2와 inv-3의 차이: `at`과 `names_digest` 4개(roles, snapshots, testrepos, forge)뿐이다. 모드·UID·항목 수·역할 볼륨·컨테이너·네트워크·timer는 같다.
  - digest 차이는 seed SHA로 설명된다. 역할 볼륨의 digest도 다른 이유는 E1이 `roles/development/state/work/e1`에 clone하기 때문이다(probe2.py:284).
- init-1·init-2 E(16)·E1b(7)
  - id 집합과 expect가 01-04 `remaining-probes-20261112`와 같다. 모두 rc 0이고 `observed: true`·`judgement_basis: markers`다.
  - 출력은 SHA·IP·Date를 정규화하면 01-04와 같다(E2-no-password는 컨테이너 IP, E3-get은 Date 헤더만 다르다).
  - E1b-main-unchanged의 SHA는 각 실행의 `seed_main`과 같다.
  - E 비밀 지문과 역할 토큰 지문은 01-04와 같다(e-secrets를 유지한다는 설명과 맞는다).
- 현재 시험실: 지금 `lab.py inventory` 결과를 inv-3과 비교하면 `at` 외에 모든 필드가 같다(digest 포함). `verify`는 all_ok다. 네트워크 2개는 `internal=true`, 컨테이너 3개는 exited다. 문서화된 초기 상태와 일치한다.

### 1.4 기록 위치 표(Lab_Procedure §3)
- 위치는 맞다: `attempts/NN-<probe>-<role>/{stdout.jsonl,stderr.txt,meta.json}`, `budget.json`, `setup.json`, `E.json info.images`, `P2.json`, `state.json`.
- 다만 필드 설명 일부가 01-04 실제 증거와 다르다(P3-2).

### 1.5 열람 경로
- 공유 작업공간 `/opt/rust/src/github.com/HeaInSeo/ieo_poc`는 지금 `main` 417d6e3이고 작업 트리가 깨끗하다(읽기만 했다).
- `STATUS.md`는 존재하고 0664, heain, 10296바이트, snapshot `v1.3.3-EXEC-0010`이다.
- 인증 없는 curl로 4개 URL(blob 3, raw 1)이 모두 200이었다. raw 머리말의 snapshot이 같고, `ls-remote` main은 417d6e3이다.
- “사용자 실제 열람 미확인”을 REPORT 첫 절, §1, result.json `unresolved`에 적었다. 작업 사본의 브랜치를 따른다는 한계(검사 시점에 보고 브랜치였다)도 Lab_Procedure §5에 정직하게 적었다.

### 1.6 “필요한 환경만”·한계
- 초기화는 01-04가 쓴 환경(역할 볼륨, repo·forge, DB, 수신기)으로 한정했다. probe 중간 산출물은 기대값에서 “없음”으로 검사한다. 클러스터·GHCR·GH1~GH5는 해당 실험으로 미뤘다. 수용 문구와 맞는다.
- 한계 절(호스트 로컬 보관본, 세션 0건 초기화, seed SHA 변동)은 사실과 맞는다.

## 2. 발견 사항

### P3-1 “보관 → 정리 → 초기화 → 대조 2회” 표현이 실행과 다르다
- 위치: `docs/poc/reports/tasks/01-06/lab-20261112/REPORT.md:15`
- 근거:
  - 보관본 디렉터리는 `/opt/rust/ieo_poc_archive/lab-20261112T091332Z` 하나뿐이다.
  - 시각 순서는 archive 09:13:34 → reset-1 09:13:35 → inv-2 09:14:27 → reset-2 09:14:43 → inv-3 09:15:18이다. 두 번째 회차에는 보관 단계가 없다.
- 영향: 두 번째 reset이 지운 것은 init-1이 만든 재생성 가능한 상태다(init-1 증거는 저장소에 있다). 증거 손실은 없다.
- 권고: “보관 1회, 정리→초기화→대조 2회”로 고친다.

### P3-2 기록 위치 표의 필드 설명이 01-04 실제 증거보다 넓다
- 위치: `docs/poc/reports/tasks/01-06/lab-20261112/Lab_Procedure.md:35`, `:38`, `:39`
- 근거:
  - 01-04 `attempts/*/meta.json` 35개 어디에도 `result_missing`·`judgeable` 필드가 없다. 이 필드는 후속 커밋 f64c413·d579f97에서 하네스에 추가됐고, `probes/01-04/test_result_check.py`에만 나온다.
  - 01-04 `E.json`·`E1b.json`에는 `markers`·`judgement_basis`가 없다(01-06 init 결과에는 있다).
  - `reports/tasks/01-04/*/result.json`이 없다. 01-04의 작업 결과는 REPORT.md와 state.json에 있다.
- 영향: 표는 “현재 하네스가 쓰는 형식”으로는 맞다. 하지만 01-04 기록을 찾는 사람에게는 없는 필드를 기대하게 한다.
- 권고: “01-04 후속 교정 이후 형식이며, 01-04 run에는 result.json·result_missing/judgeable·markers가 없다”는 주석을 단다.

### P3-3 archive의 자격 제외 목록이 알려진 위치에만 맞춰져 있다
- 위치: `probes/lab/lab.py:25`, `:102`
- 근거: `NEVER_ARCHIVE`는 `*/cred*`와 `e-secrets*`만 뺀다. 역할 `CLAUDE_CONFIG_DIR`(`state/claude`)에 `.credentials.json`이 생기면(예: 역할이 `/login`을 쓰는 경우) 제외되지 않는다. 사후 검사(`/cred/`, `e-secrets` 문자열)도 이를 잡지 못한다.
- 현재: 보관본에 해당 파일은 없다. 위 1.2에서 값 대조로 확인했다.
- 권고:
  - `.credentials.json`과 `*.token`을 제외 목록에 넣는다.
  - 또는 보관 뒤 `secrets/*.token` 값으로 내용 대조 검사를 추가한다.

### P3-4 archive 대상 경로에 저장소 거부가 없다
- 위치: `probes/lab/lab.py:92-94`
- 근거: dest 거부 조건은 “시험실 안”뿐이다. 보관본에는 역할 세션 기록 원문(jsonl)과 `.claude.json`(userID·machineID)이 들어 있다. dest를 공개 저장소 안으로 주면 그대로 커밋될 수 있다.
- 권고: `REPO`의 하위 경로도 거부한다.

### P3-5 범위 보호가 환경 변수와 실행 순서에 기대고 있다
- 위치: `probes/lab/lab.py:113-126`, `:136-139`; `probes/01-04/probe.py:16-17`
- 근거:
  - `init`이 부르는 probe.py·probe2.py는 `IEO_PROBE_BASE`·`IEO_PROBE_SECRETS` 환경 변수를 따른다(probe2 `ROOT = P.BASE.parent`). lab.py는 이 변수를 고정하거나 지우지 않는다. 그래서 상속된 환경에서는 `/opt/rust/ieo_poc_probe` 밖에 쓰거나 `rmtree`할 수 있다.
  - `reset`은 직전 archive가 있는지 확인하지 않는다. 절차 §4는 2→3 순서를 문서로만 요구한다.
- 권고:
  - init에서 두 변수를 고정한다.
  - reset에 `--archived <MANIFEST>` 같은 확인을 둔다.

### P3-6 verify의 한 검사가 항상 참이다
- 위치: `probes/lab/lab.py:183`
- 근거: `c("only ieo-probe-* touched …", True, …)`는 값과 무관하게 ok다. 25개 ok 중 1개는 정보 표시일 뿐이다.
- 권고:
  - 기준 목록(예: inv-0의 `other_containers_not_touched`)과 비교한다.
  - 또는 검사 목록에서 빼고 정보 필드로 둔다.

### P3-7 정리 뒤 자산 목록과 01-04 재현 명령이 사라진 경로를 가리킨다. 새 시험도 CI 밖에 있다
- 위치: `docs/poc/reports/workspace/test-env-assets-20261112/ASSETS.md:27-31`; `docs/poc/reports/tasks/01-04/first-probes-20261112/REPORT.md:125`; `.github/workflows/ci.yml:9-23`
- 근거:
  - ASSETS.md는 `roles-recreated`(“L2 증거용”)와 `snapshots/73ea669…`를 현재 자산으로 적는다. reset-1 뒤 이 경로는 없고 호스트 로컬 보관본에만 남는다. ASSETS.md는 01-04 원장 증거 목록에 들어 있다.
  - REPORT §3이 보관본에 남는다는 사실은 밝혔다. 하지만 자산 목록은 갱신하지 않았다.
  - CI는 `render_progress.py --check`만 돌리고 `test_render_progress.py`는 돌리지 않는다.
- 권고:
  - ASSETS.md에 “01-06 reset 뒤 보관본 `lab-20261112T091332Z`로 이동”을 적는다.
  - CI에 생성기 시험을 추가할지 검토한다(선택).

## 3. 종합 판정

수용 문구별 판단:
- **재현**: 정리→초기화→대조가 2회 모두 일치했다. 보관은 1회다(P3-1).
- **필요한 환경만**: 충족.
- **STATUS 고정 경로·결과 인덱스·보고 링크를 첫 응답으로 제공**: 충족.
- **state.json 단일 원본과 생성기 산출물 포함**: 충족.
- **손편집·타이머 불요**: 충족.
- **정상/실패·증거 링크·중단 뒤 재생성 확인**: 26개 시험을 재실행해 확인했다.
- **열람 미확인 공개**: 충족.

생성기 변경은 화면 내용과 검증 규칙을 바꾸지 않는다. Amendment sha256도 맞다. 비밀이나 식별자 유출은 찾지 못했다. P3 항목은 문서 정확성과 방어 심화에 관한 것이라 수용을 막지 않는다.

**01-06 수용 승인(approve)**: b301843 기준이며, CI 성공을 조건으로 한다.
