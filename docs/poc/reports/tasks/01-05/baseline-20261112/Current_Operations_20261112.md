# 01-05 현재 운영 확인과 외부 효과 분리 (2026-11-12)

조회 원본: [evidence/current-operations.txt](evidence/current-operations.txt)(2026-11-12T09:03:56Z 재조회. 첫 조회 뒤 검토 efa4a5d의 지적으로 timer 상태를 넣고, 01-05에 필요 없는 세부를 줄여 다시 찍었다). 조회는 모두 읽기 전용이었다. systemd·프로세스·git 상태는 바꾸지 않았다. 비밀·로그 본문·환경 값은 기록하지 않았다.

**이 문서는 고정 원본([Source_Baseline.md](Source_Baseline.md))과 별개로 조회 시점의 호스트를 관측한 것이다.** 고정 원본 문서에 나오는 설치·운영 서술은 과거 스냅샷이다. 예: OPERATIONS.md:21-27의 설치 커밋 `01b8a789…`, VERIFICATION_20261012.md의 enabled 상태. 그런 서술은 현재 증거로 쓰지 않는다. 목록은 [조사 원본 (b)](evidence/npg-clauses-e5075aff.md)에 있다.

## 1. 관측한 현재 actor

| actor | 관측 | 외부 효과(관측·문서 근거) |
| --- | --- | --- |
| `claude-operator-tick.timer` → `.service` | enabled·active(waiting), 매시 07분. 실행 파일 `~/.local/bin/claude-operator-tick.sh`(208줄, `224cef2ee979`). 작업 디렉터리 `/opt/go/src/github.com/HeaInSeo`. 공유 사용량 게이트 `claude-usage-gate.sh`(스크립트 주석: 주간 사용량만 차단 조건). 마지막 `TICK_END` 2026-11-12T08:09:05Z rc=0 | 호스트 `~/.local/bin/claude`(호스트 claude.ai 로그인)로 역할 작업. tick 로그 마지막 40줄에 notion 1회·receipt 2회·PR 2회 언급(개수만 기록, 본문 미기록) |
| `claude-guardrail-tick.timer` → `.service` | enabled·active(waiting), 매시 37분. `claude-guardrail-tick.sh`(251줄, `6a19c5eb0ed9`). 같은 작업 디렉터리·게이트. 마지막 `TICK_END` 08:38:36Z rc=0 | 같은 로그인. tick 로그 마지막 40줄에 notion 3회·receipt 4회·PR 5회 언급(개수만) |
| `claude-report-bridge.timer` | **enabled·active, SubState=elapsed**(다음 실행 없음). 마지막 트리거 2026-10-13. 단위 설정 `OnBootSec=1min`, `OnUnitActiveSec=5min`, `Persistent=true`. 재부팅이나 사용자 systemd 관리자가 재시작되면 5분 주기로 다시 돌 수 있다. 첫 조회에서 “비활성”으로 적은 것은 잘못이었다(list-timers의 NEXT n/a만 봄) | `claude-report-bridge.py --scan`(Operator/Guardrail 보고 스캔) |
| NPG 작업 사본 `/opt/javascript/src/github.com/HeaInSeo/notion-projection-guard` | HEAD `0d03bf6…`, 브랜치 `fix/sori48-approved-unknown-closure-20261029`, 미커밋 0, 커밋 2026-11-01. **고정 `e5075aff`는 HEAD의 조상이 아니다**(HEAD에만 38, 고정 쪽에만 2). 이 디렉터리에서 codex 프로세스 2개 실행 중 | NPG 개발·운영 작업(이 run에서 내용은 조회하지 않음) |
| 그 밖의 장기 claude 세션 | cwd `/opt/go/src/github.com/HeaInSeo`(2), `…/bori`, `/opt/dotnet/src/github.com/HeaInSeo/NodeKit`. 이 PoC 대화 세션(`/opt/rust/src/github.com/HeaInSeo/ieo_poc`)도 있다 | 각 프로젝트 작업. NodeKit은 P26 표본 후보 repo다 |
| 원격 제어 서비스 | 실행 중인 사용자 서비스 3개(원격 명령·MCP 터널 계열, 이름은 원본) | 원격 명령 경로. PoC는 쓰지 않는다 |
| GitHub self-hosted runner | 등록 저장소 `HeaInSeo/mac-k8s-multipass-terraform` | 그 저장소의 CI. PoC CI는 GitHub 호스트 러너(`ubuntu-latest`)만 쓴다. ieo_poc·ieo_poc_test의 self-hosted runner는 0개다 |
| podman(rootless, heain) | PoC: `ieo-probe-pg`·`ieo-probe-rx`·`ieo-probe-forge`(정지), 네트워크 `ieo-probe-coord`·`ieo-probe-forge`. PoC 아님: `local-registry`(Created) | — |

`/home/heain/.local/share|state/notion-projection-guard` 같은 NPG runtime 경로의 현재 설치 상태는 이 run에서 조회하지 않았다. 실제 전환 전 조사는 P27의 범위다. 여기에는 “확인하지 않음”으로 남긴다.

## 2. PoC 외부 효과와 겹침 분리

PoC가 이 호스트와 외부에 내는 효과는 다음으로 한정한다.
- GitHub `HeaInSeo/ieo_poc`·`HeaInSeo/ieo_poc_test`의 브랜치·PR·댓글·병합
- `/opt/rust/ieo_poc_probe`·`/opt/rust/ieo_poc_secrets` 아래 파일
- 이름이 `ieo-probe-*`인 podman 자원
- 역할 자격(방식 A)으로 하는 모델 요청

| 겹침 | 근거 | 분리·처리 |
| --- | --- | --- |
| **공급자 quota group 공유** | 01-04 P3: 역할 토큰의 `sub`(계정)·`aud`(org)가 호스트 로그인과 같다. 기존 Operator·Guardrail tick도 호스트 로그인을 쓴다. 따라서 PoC 모델 요청, 기존 tick, 이 개발 대화가 같은 5시간·7일 창을 쓴다 | 기술적으로 분리할 수 없다(D1: 공유 확인이면 지원 구성으로 기록하고 진행). PoC 모델 요청은 B2·L01 누적 한도 안에서만 하고, 시도 수와 한도 신호를 기록한다. 기존 tick의 사용량 게이트(주간)를 PoC가 바꾸지 않는다. PoC가 기존 tick을 한도 대기로 밀어낼 수 있다는 것은 알려진 영향이다. 04-15에서 측정한다 |
| GitHub 계정 공유 | PoC와 기존 actor가 같은 GitHub 사용자(HeaInSeo) 아래 저장소를 쓴다 | PoC는 위 두 저장소 밖에 쓰지 않는다. NPG·bori·NodeKit·Sori·mac-k8s 저장소에는 쓰지 않는다. NPG는 고정 커밋을 API로 읽기만 한다 |
| Notion | 기존 tick이 Notion에 쓴다. 계획은 코디네이터의 Notion 쓰기를 금지한다 | PoC 경로·자격에 Notion 쓰기가 없다(01-04 역할 컨테이너에 Notion 자격 없음, MCP 비어 있음). P21에서 Notion 쓰기 0건을 확인한다 |
| NodeKit·Sori 실제 백로그(P26) | NodeKit 디렉터리에서 기존 claude 세션이 작업 중이다 | 07-01에서 현재 백로그·담당·권한을 다시 대조한 뒤 배정한다. 기존 담당자가 진행 중인 작업은 표본에 넣지 않는다(L04) |
| 호스트 자원 | 같은 호스트의 rootless podman | PoC 자원 이름을 `ieo-probe-*`로 구별한다. 다른 컨테이너(`local-registry`)를 건드리지 않는다. 정리·초기화 절차는 01-06 |
| report bridge timer 재기동 | 재부팅·사용자 관리자 재시작 시 5분 주기로 다시 돈다(위 §1) | PoC는 재부팅하지 않고 사용자 systemd 관리자를 건드리지 않는다. 01-06과 각 실험 첫 실행 전에 timer 상태를 다시 조회한다 |
| 기존 actor가 PoC 저장소에 쓰는 것 | ieo_poc의 PR 댓글·검토·PR 작성자: `chatgpt-codex-connector[bot]` 26건, `HeaInSeo` 57건. ieo_poc_test는 0건(원본 마지막 절) | codex 자동 검토는 actor가 구별되며 PoC 기록에서 자동 검토로 처분한다. `HeaInSeo` 쓰기에는 이 개발 세션의 쓰기가 들어 있다. 같은 계정을 쓰는 다른 actor와 계정만으로는 구별할 수 없다. 기존 tick은 작업 디렉터리가 `/opt/go/...`라 PoC 저장소를 대상으로 하지 않는다. P21의 범위 밖 효과 집계는 actor와 브랜치·시각으로 대조한다 |
| 원격 제어 서비스 | 호스트에서 원격 명령을 받는다 | PoC가 쓰지 않는다. 런타임 계약 §1에 따라 이 대화 세션의 자격·환경을 역할 프로세스에 넘기지 않는다 |
