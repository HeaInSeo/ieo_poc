# 시험 환경 자산 목록 (2026-11-12 기준)

계획 v1.3.3-A1 · T05 시험 환경 · 01-04 검수 완료 시점의 목록이다. **환경 준비와 자산 등록은 PoC 작업·실험 완료 수에 넣지 않는다.** 비밀 값은 적지 않고 지문(SHA-256 앞 12자)만 적는다.

## 1. GitHub 시험 저장소 (사용자 생성)

사용자 원문: [User_Decisions_2026-11-12_0104b.md](../../../User_Decisions_2026-11-12_0104b.md). 아래 값은 개발 에이전트가 GitHub API로 다시 조회한 것이다. 원본은 [evidence/github-ieo_poc_test-settings.txt](evidence/github-ieo_poc_test-settings.txt)에 있고, 조회 시각은 그 파일 첫 줄이다. Actions secret·variable·environment 개수와 파일 목록(전체 tree)은 보완 조회 [evidence/github-ieo_poc_test-supplement.txt](evidence/github-ieo_poc_test-supplement.txt)에 있다(2026-11-12T08:39:59Z, PR #15 검토 P3-2). secret·variable·environment는 각 0개이고, tree는 `.github/workflows/bootstrap.yml`·`.gitignore`·`README.md`·`SANDBOX.json`이다.

| 항목 | 조회 값 | 사용자 진술과의 대조 |
| --- | --- | --- |
| 저장소 | `HeaInSeo/ieo_poc_test`, public, 기본 브랜치 `main`, 생성 2026-11-12T08:10:42Z | 일치 |
| 원격 서버 클론 | `/opt/rust/src/github.com/HeaInSeo/ieo_poc_test`, HEAD `2ab9d87`(`main...origin/main`) | 일치 |
| 내용 | `.github/workflows/bootstrap.yml`, `.gitignore`, `README.md`, `SANDBOX.json` | 합성 시험 파일만(일치) |
| CI | workflow `sandbox-bootstrap`, job `repository-smoke`. run 43180843445는 push·main·`2ab9d87`·success. 권한 `contents: read`, `persist-credentials: false`. 정확한 SHA를 checkout하는지 대조한다 | 일치 |
| `main` 보호 | 필수 상태 검사 `repository-smoke`(app_id 15368 = GitHub Actions), `strict: true`(최신 base). PR 필수, 필요한 승인 수 **0**, stale 승인 해제. `enforce_admins: true`. force push 금지, 삭제 금지. 대화 해결 필수. 서명 요구 없음, 선형 이력 요구 없음 | 일치. 다만 **필요 승인 수가 0**이다. 독립 판정은 아직 강제되지 않는다(아래 §3) |
| Actions 권한 | 기본 토큰 `read`, PR 승인 불가(`can_approve_pull_request_reviews: false`). 허용 액션 `all` | 일치. 허용 액션이 전체라는 점은 05-03 대조 대상이다 |
| ruleset / webhook / deploy key / Actions secret | 0 / 0 / 0 / 0 | — |
| 협력자 | 저장소 소유자 1명(admin) | — |
| 역할별 GitHub App 자격 | **없음**(연결 안 됨) | 일치 |
| 독립 판정 체크 | **없음**. 현재 필수 검사는 저장소 연결 확인용이다 | 일치. **P07/P23의 독립 판정이 구성됐다고 간주하지 않는다** |

## 2. 로컬 시험 자산 (01-04에서 준비)

| 자산 | 위치·식별 | 상태 |
| --- | --- | --- |
| 역할 자격(방식 A) | `/opt/rust/ieo_poc_secrets/{development,guardrail}.token`(0700/0600, 사용자 발급). 지문 `2456b85e9111` / `da968d393412` | 사용 중. 교체하지 않음(사용자 결정) |
| 역할 볼륨 | `/opt/rust/ieo_poc_probe/roles/{development,guardrail}/{state,cred,etc}`. 컨테이너 UID 20001/20002(호스트 185536/185537) | 01-04 probe 기록 포함 |
| 재생성 모사 볼륨 | `/opt/rust/ieo_poc_probe/roles-recreated/development/state` | L2 증거용 |
| CLI | `/opt/rust/ieo_poc_probe/bin/claude-2.1.322`(`bfb3703a…`), `claude-2.1.316`(`fd13c700…`) | 0755 사본 |
| 기반 이미지 | `golang:1.26.9-bookworm`(`6880d90c0623…`), `postgres:17-bookworm`(`018dbecf6a31…`) | 로컬 |
| 로컬 시험 repo | `/opt/rust/ieo_poc_probe/testrepos/{a,b}.git`(main 보호 hook), 스냅샷 `/opt/rust/ieo_poc_probe/snapshots/73ea669…` | E1 |
| forge | `/opt/rust/ieo_poc_probe/forge/a.git`(UID 20005 소유), 컨테이너 `ieo-probe-forge`(git daemon), 네트워크 `ieo-probe-forge`(internal) | E1b. 컨테이너 정지 |
| DB | 컨테이너 `ieo-probe-pg`, 역할 `ieo_coord`·`ieo_status_reader`, 스키마 `ieo`, 네트워크 `ieo-probe-coord`(internal) | E2. 컨테이너 정지 |
| 보고 수신기 | 원본 `probes/01-04/receiver/main.go`(`741895648379`), 컨테이너 `ieo-probe-rx`, 데이터 `/opt/rust/ieo_poc_probe/rx-data` | E3. 컨테이너 정지 |
| E 시험 비밀 | `/opt/rust/ieo_poc_probe/e-secrets/{pg_super,pg_coord,pg_reader,rx_token,rx_other}`(0700 디렉터리). 지문은 01-04 [E.json](../../tasks/01-04/remaining-probes-20261112/evidence/E.json) | 실행 때 생성 |
| 하네스 | `probes/01-04/probe.py`, `probe2.py`, `identity_claims.py` | main |

재생성: `probe2.py e`(E1~E3), `probe2.py e1b`(forge). 정리·초기화 절차는 01-06의 산출물이다.

## 3. 05-03/P07 전에 필요한 연결 작업 (미완, 사용자 자격 발급 포함)

**연결 확인용 CI와 독립 판정을 구분한다.**
- 현재 필수 검사 `repository-smoke`는 저장소 연결과 정확한 SHA checkout만 확인한다. 독립 판정이 아니다.
- 아래 G1~G5가 구성되기 전에는 P07/P23의 독립 판정 근거로 쓰지 않는다.
- 이 표의 작업과 저장소 준비는 PoC 작업·실험 완료 수에 넣지 않는다.

| # | 작업 | 근거 |
| --- | --- | --- |
| G1 | 개발 역할 GitHub App: 대상 repo 하나, contents/pull_requests 쓰기, 만료 ≤ 1시간의 단기 설치 토큰. 배정 단계가 `개발`/`교정`일 때만 발급하고 `REVIEW_PENDING`에 폐기 | 런타임 계약 §2.1(push 시점 제한) |
| G2 | Guardrail 역할 자격: push 없음. 고정 SHA 읽기와 독립 판정 발행 경로만 | 계약 §2·§2.1 |
| G3 | 독립 판정 체크: Guardrail·코디네이터가 승인 head SHA에 발행하는 필수 상태 검사. `repository-smoke`와 구별하고, 개발 역할은 발행할 수 없어야 한다 | 계획 P07·P23, 01-08 계약 |
| G4 | `main` 보호에 G3 체크를 필수로 추가하고, 필요 승인 수 0과 허용 액션 `all`의 처분을 정한다 | 05-03 승인값↔적용값 대조 |
| G5 | 설정 승인 주체(사람 정책 소유자)와 제안자 분리 기록 | 05-01·05-03 수용 기준 |

**착수 조건 연결**(원장 각 작업의 `unresolved`에 같은 문구로 기록):

| 작업 | 착수 전 필요 | 이유 |
| --- | --- | --- |
| 04-07 (P07 현재 SHA·숨긴 결함·검토 정지) | G1·G2·G3 | 개발 역할의 Guardrail 자격 획득·명의 발행 거부와 독립 판정 경로를 실제 GitHub에서 시험한다 |
| 05-01 (보호된 CI 실행·검사 발행 후보) | G3·G5 | 보호된 검사 발행 경로와, 승인 주체·제안자 분리를 구성한다 |
| 05-02 (Actions 정상/거짓 성공 대조, P23) | G3(05-01 경유) | 동일 이름 체크·이전 SHA 대조에는 독립 판정 체크가 있어야 한다 |
| 05-03 (저장소 설정·브랜치·App·권한) | G1·G2·G4·G5 | 승인값과 적용값의 대조, App 권한, 보호 변형·복원 |

- App 생성과 자격 발급은 사용자가 한다(F01·계약 §3 원칙 2). 개발 에이전트는 만들지 않는다.
- 위 작업들은 의존관계상 아직 착수할 수 없다. 04-07은 02-06·02-02 뒤, 05-01은 05-04·04-07 뒤다. 그래서 G1~G5는 현재 01-05~02의 차단 조건이 아니다.
