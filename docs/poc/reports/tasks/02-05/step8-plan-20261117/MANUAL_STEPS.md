# 02-05 단계 8 실행 절차 · 사용자 세 가지와 에이전트 단계

| 항목 | 기록 |
| --- | --- |
| 근거 | 사용자 승인: 병합된 [실행안](EXECUTION_PLAN.md)(main 46799b0, main CI 44033234326 success)의 §8 묶음, id는 계획대로, 예산·멈춤 규칙 그대로. 예비·DB 복원·회차 중 재시작은 묶음 밖. 자동 되돌리기 = 옛 바이너리를 DB 그대로 |
| 나눔 | 사용자 결정: **사용자는 세 가지만** 한다. ① 본인 토큰 생성과 digest(덮어쓰기 없음) ② 등록 뒤 독립 의미 검토 ③ 자격 계정이 같은지 답. 나머지는 모두 에이전트가 한다. 민감한 경로(`/opt/rust/ieo_poc_secrets`, `/opt/rust/ieo_poc_runs`, `/proc/<pid>/environ`, 캡처한 `env0`)를 건드리는 일은 아래 [권한 설정](#권한-설정사용자-시작-전-한-번)의 **글자 그대로 맞는** 허용 규칙(와일드카드 없음)의 명령으로만 한다 |
| 표기 | **U**n = 사용자 단계(명령을 그대로 붙여 넣음), **A**n = 에이전트 단계(에이전트가 Bash 도구로 그대로 실행). 어떤 명령도 비밀 값(토큰, OAuth 자격, DB 암호)을 출력하지 않는다. 출력되는 것은 개수, True/False, 이름, 경로, 비밀이 아닌 것의 SHA-256뿐이다 |
| 실행본 | 46799b0의 스크립트·명세 사본 `$D/step8-46799b0/`(16개 파일, 46799b0과 SHA-256이 같음을 확인), 도우미 `$D/step8-ops/obs-check.sh`(이 브랜치의 [launcher/obs-check.sh](launcher/obs-check.sh)와 같음), 목록 `$D/step8-ops/SHA256SUMS`. 둘 다 쓰기 금지(`chmod -R a-w`). 새 빌드 M(46799b0, release)은 `$D/bin/ieo-coord-M`과 `$D/bin/m-46799b0/`, 옛 `$D/bin/ieo-coord`(778ee47)는 그대로 |
| 역할 작업 디렉터리 | **`/opt/rust/ieo_poc_runs/02-05-s8-20261118`**로 고정(규칙이 글자를 맞추므로). A4를 다른 날(UTC) 하게 되면 사용자가 규칙과 이 문서의 A4 명령 둘 다에서 날짜를 바꾼다 |
| 마감 | **R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 받아들여져야 한다(첫 관측 D6의 7일 한도, 구동 스크립트가 10분 여유를 두고 확인) |

이 문서에서 `$D` = `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data`다. 다만 **규칙에 걸리는 명령에는 `$D`를 쓰지 않고 절대 경로를 글자 그대로 쓴다.** 어느 단계에서든 예상과 다른 출력이 나오면 다음 단계로 가지 않는다.

## 권한 설정(사용자, 시작 전 한 번)

프로젝트의 `/opt/rust/src/github.com/HeaInSeo/ieo_poc/.claude/settings.local.json`에서 `permissions`에 아래 `allow`·`deny`를 합친다(기존 `additionalDirectories`는 그대로). 모든 `allow`는 명령 한 줄과 **글자 그대로** 같아야 통과한다(`*` 없음).
```json
{
  "permissions": {
    "allow": [
      "Bash(cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data)",
      "Bash(echo 'd91de4245b3da246a390dacc5311d4304df39f5a27c84c9593fc0b26c2d50fa0  step8-ops/SHA256SUMS')",
      "Bash(sha256sum --quiet --strict -c -)",
      "Bash(sha256sum --quiet --strict -c step8-ops/SHA256SUMS)",
      "Bash(find step8-46799b0 step8-ops -perm /222)",
      "Bash(wc -l)",
      "Bash(grep -qx 0)",
      "Bash(echo \"copies ok\")",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord 58090 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-M.log --tokens /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json -- --roles /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/roles.json)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-778ee47-rollback.log)",
      "Bash(export QUOTA_GROUP_ID=claude-sub-shared)",
      "Bash(export SUP_DEV_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-dev.token)",
      "Bash(export SUP_GR_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-gr.token)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/m-46799b0 /opt/rust/ieo_poc_runs/02-05-s8-20261118)",
      "Bash(export COORD=http://127.0.0.1:58090)",
      "Bash(export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8)",
      "Bash(export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out)",
      "Bash(export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205)",
      "Bash(export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header)",
      "Bash(export EXPECTED_CURRENT_DEV=null)",
      "Bash(export EXPECTED_CURRENT_GR=null)",
      "Bash(export OBS_UNIX=1794698043)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh register)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh prepare)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh runs)"
    ],
    "deny": [
      "Read(//opt/rust/ieo_poc_secrets/**)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/**)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47/env0)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47/argv0)",
      "Read(//proc/*/environ)",
      "Read(//opt/rust/ieo_poc_runs/*/adapter.token)",
      "Read(//opt/rust/ieo_poc_runs/*/*/cred/**)"
    ]
  }
}
```
- 처음 여덟 줄은 [실행본 확인](#실행본-확인에이전트-규칙에-걸리는-명령마다) 한 줄의 하위 명령이다(모두 읽기 전용). 부정(`!`) 없이 `find … | wc -l | grep -qx 0`으로 쓴다: 파이프라인의 종료 상태는 `grep`의 것이라 쓰기 가능한 항목이 0개일 때만 성공한다. 그래도 그 줄이 확인 창을 띄우면 사용자가 승인한다(읽기만 함).
- `export QUOTA_GROUP_ID=claude-sub-shared`는 **U2에서 “같다”고 답했을 때만** 넣는다. “다르다”면 이 줄을 넣지 않는다. A4 전에 멈추고, 실행안 §9 Q2의 결정 뒤 그 값의 `export` 한 줄로 바꾼다. `obs-check.sh` [4]도 두 역할이 `claude-sub-shared`인지 확인하므로, 그때는 도우미·목록·이 문서의 해시도 새로 고쳐 검토를 받는다.
- `EXPECTED_CURRENT_DEV/GR`은 `null`만 규칙에 둔다. `ieo-poc-test`는 단계 8에서 처음 쓰는 repo라 `assignment_heads`가 비어 있어야 한다. 값이 있으면(앞선 `prepare`가 일부 진행됐거나 계획에 없는 배정) A8.1에서 멈추고 보고한다. 그래서 값이 있는 `export`를 승인할 일은 없다.
- `deny`는 `Read` 도구에만 걸린다. `ieo_poc_runs`·`coord-778ee47` 디렉터리 전체는 막지 않는다(A2·A4·A6 명령의 인자다).

명령 형식의 약속(에이전트):
- 규칙에 걸리는 명령은 언제나 **따로 선 하위 명령**이고 규칙과 글자가 같다. `sh`, `./`, 변수(`$D`), 명령 치환(`$(…)`)을 쓰지 않는다.
- 환경 변수는 같은 하위 명령 앞에 붙이지 않는다(`X=1 bash …` 금지). 같은 Bash 호출 안에서 `export X=…`를 별도 하위 명령으로 먼저 두고 `&&`로 잇는다(호출 사이에는 셸 상태가 이어지지 않는다; `;`는 앞이 실패해도 뒤를 실행하므로 쓰지 않는다).
- **규칙 명령은 그 단계에서만 실행한다.** 그 단계가 끝났거나 `STOP`(또는 실패)으로 끝난 뒤에는 **사용자가 명시적으로 지시하기 전에는 다시 실행하지 않는다.** 규칙은 단계 8 내내 살아 있어 확인 창 없이 다시 통과하므로, 이 약속이 시점을 지킨다. 예: 회차 중의 `stop`(회차 중 재시작은 묶음 밖), `STOP` 뒤의 `runs` 재호출(일시적인 멈춤 뒤에 다시 부르면 G1까지 갈 수 있음). 예외는 이 문서에 적힌 것뿐이다: `obs-check.sh`의 재실행(A2·A4·A5, 읽기와 같은 내용의 복사만), A6 실패 뒤 되돌리기의 `stop`·옛 바이너리 `start` 한 번씩.
- 사용자는 A6 확인이 끝나면 `capture`, `run-step8-roles.sh`, M의 `start` 줄을 먼저 지워도 된다(되돌리기용 `stop`·옛 `start`는 단계 8 끝까지 둔다).

**여전히 확인 창(또는 분류기 판단)을 거치는 호출**: A2의 `mkdir`, A3 전부(주체 토큰 만들기, 두 JSON 쓰기·요약, manifest 비교, 55433 읽기 질의, 백업, 시험 DB 예행), A4 뒤의 `podman ps`, A5의 `roles.json` 쓰기·재확인, A6의 생존 확인 읽기, A8.1의 `assignment_heads` 질의. 모두 민감한 경로를 건드리지 않는다.

허용 규칙에 맞는 명령이 그래도 막히면 에이전트는 **멈추고**, 그 Bash 호출을 글자 그대로 사용자에게 주고 예상 출력과 함께 사용자 터미널에서 실행해 달라고 한다. 58090이 멈춘 뒤라면(A6) 기동 또는 되돌리기 명령을 바로 준다.

**단계 8이 끝나면**(A8 보고 뒤, 또는 되돌리기 뒤) 사용자가 단계 8을 위해 넣은 `allow` 줄을 **모두** 지운다. `deny`는 남겨도 된다. 사본 두 디렉터리는 증거로 쓰기 금지인 채 둔다.

## 실행본 확인(에이전트, 규칙에 걸리는 명령마다)

규칙에 걸리는 명령(A1·A2·A4·A5·A6·A7·A8)을 실행하는 Bash 호출은 언제나 이 줄로 시작하고 `&&`로 잇는다. 목록 파일의 해시를 문서의 값과 비교하고, 목록의 17개 파일을 확인하고, 쓰기 가능한 항목이 있으면 실패한다. 실패하면 뒤의 명령은 실행되지 않는다.
```
cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data && echo 'd91de4245b3da246a390dacc5311d4304df39f5a27c84c9593fc0b26c2d50fa0  step8-ops/SHA256SUMS' | sha256sum --quiet --strict -c - && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && find step8-46799b0 step8-ops -perm /222 | wc -l | grep -qx 0 && echo "copies ok"
```
아래에서 이 줄을 `<확인> &&`로 줄여 적는다. 실제 호출에는 줄 전체를 쓴다. 예상 출력: `copies ok`. 그 밖의 출력이 나오면 멈추고 보고한다. 목록의 17개 중 규칙 명령에 쓰는 넷:

| 파일 | SHA-256 |
| --- | --- |
| `step8-46799b0/launcher/coord-switch.sh` | `6e0245f2750c2db4c9a9fafda114d981537273fee617b76dcf6eb6f5de5b4b06` |
| `step8-46799b0/launcher/run-step8-roles.sh` | `b07cde82fa947785529488bd887b1a448e20c0ba3bf10d4fb044feccfdd14ac2` |
| `step8-46799b0/launcher/run-step8.sh` | `26cfca17c80b236096c07202d66e12cdfadbc3ea6d49abbc2b0f908408e282c4` |
| `step8-ops/obs-check.sh` | `bb66054adb78689b87ca51c03f48e0ba166dce65c6ea2487767fe24598e6f9eb` |

## 순서 한눈에

| # | 누가 | 무엇 | 라이브 변경 |
| --- | --- | --- | --- |
| U1 | 사용자 | 본인 토큰과 digest(A3 전까지) | 없음 |
| U2 | 사용자 | 두 역할 자격이 01-04와 같은 구독 계정인지(A4 전까지) | 없음 |
| A1 | 에이전트 | `obs-check.sh`: D6 관측 확인·복사, 02-04 진행 중 회차 없음 | 없음(파일 복사) |
| A2 | 에이전트 | 옛 코디네이터 캡처, DB user·dbname 확인 | 없음(파일 쓰기) |
| A3 | 에이전트 | 주체 토큰·새 `tokens.json`, manifest 비교, 55433 읽기 전용 질의, 백업, 시험 DB 마이그레이션 예행 | 없음(백업 파일, 시험 DB) |
| A4 | 에이전트 | 역할 컨테이너 둘, 헤더·역할 항목 복사 | 새 컨테이너만 |
| A5 | 에이전트 | `roles.json`, 장부 디렉터리, 정지 직전 재확인 | 없음 |
| A6 | 에이전트 | 옛 코디네이터 정지 + 새 빌드 기동, 실패 시 되돌리기 | **58090, 55433 마이그레이션** |
| A7 | 에이전트 | sprint 등록 → revision digest 알림 | 55433 기록 |
| U3 | 사용자 | 독립 의미 검토(본인 토큰) | 55433 기록 |
| A8 | 에이전트 | `assignment_heads` 재확인, 배정·`branch_head`·R1→R2→G1과 회차마다 확인 | **모델 호출 3회** |

U1·U2는 처음에 함께 물어도 된다. A1·A2는 그 답을 기다리지 않는다.

## U1 · 본인 토큰과 digest(사용자)

실행안 §2.2의 1과 같다. 다만 이미 있으면 **덮어쓰지 않고** digest만 보여 준다. 없으면 새로 만든다(`set -C`: 덮어쓰기 금지). 에이전트는 이 파일을 읽지 않는다(`deny` 규칙).
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; T=$D/private/user/heain.token
mkdir -p -m 700 $D/private/user
if [ -e "$T" ]; then echo "exists (not overwritten)"; else ( set -C; umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > "$T" ) && echo "created"; fi
stat -c '%a %U' "$T"; printf '%s' "$(cat "$T")" | sha256sum | cut -d' ' -f1
```
예상 출력: `exists (not overwritten)` 또는 `created`, 그다음 `600 heain`(또는 `400 heain`), 그다음 64자리 hex 한 줄(digest).
보내 줄 것: **그 64자리 digest**(비밀이 아니다).

## U2 · 자격 계정 확인(사용자)

질문: 지금 두 역할 자격(`/opt/rust/ieo_poc_secrets/development.token`, `guardrail.token`)이 01-04에서 쓴 것과 **같은 구독 계정**인가?
보내 줄 것: “같다”(그러면 두 역할 모두 `QUOTA_GROUP_ID=claude-sub-shared`, 규칙 줄을 넣음) 또는 “다르다”(그러면 A4 전에 멈추고 G1 전 관측을 어떻게 할지 묻는다; 실행안 §9 Q2).

## A1 · 첫 관측과 02-04 확인(에이전트)

```
<확인> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh
```
`obs-check.sh`는 인자가 없고 경로가 모두 고정이다. 하는 일:
- [1] D6 stream이 정확히 하나, 그 안의 `rate_limit_event`가 모두 `allowed`·초과 사용 `rejected`·`isUsingOverage` false, D6 기록의 `ended_unix`가 1794698043. 맞을 때만 `$D/ledger-0205-obs/D6.jsonl`(0600)로 복사하고(이미 있으면 같은 내용이어야 함) SHA-256을 출력.
- [2] 02-04 장부마다 `inflight.json` 없음, `caller.lock`을 연 프로세스 없음(`fuser` exit 1), `ieo-role-call` 프로세스 0.
- [3] 캡처한 `env0`이 있으면 `IEO_DB_URL`의 user·dbname·host·port와 암호가 있는지(True/False)만. 해석이 안 되면 예외 종류만 출력하고 실패. `env0`이 없으면 건너뜀.
- [4] 역할 작업 디렉터리가 있으면 확인(두 역할의 `adapter_token_file`이 `W/adapter.token`, `quota_group_id`가 둘 다 `claude-sub-shared`, 헤더 형식) 뒤 헤더·역할 항목 복사(A4). 확인이 실패하면 아무것도 복사하지 않음. 없으면 건너뜀.

예상 출력: `copies ok`, `D6 streams: 1`과 경로, `rate_limit_events <n>; all allowed, overage rejected, not in use: True`, `D6 records 1; ended_unix 1794698043; equals 1794698043: True`, `installed …/ledger-0205-obs/D6.jsonl (0600)`과 SHA-256, `02-04 ledgers: <n>`과 경로들, `inflight.json absent: True`, `processes holding caller.lock: none (fuser exit 1)`(또는 `caller.lock absent`), `ieo-role-call processes: 0`, [3]·[4] `skipped`, 끝 줄 `## obs-check: PASS`(exit 0). `FAIL:`이 있으면 exit 1이고 에이전트가 멈추고 보고한다.

## A2 · 옛 코디네이터 캡처(에이전트, 읽기만)

실행안 §5.2 단계 1. pid가 778ee47 `ieo-coord`이고 `127.0.0.1:58090`인지, 부모·cgroup·클라이언트·경로 인자를 확인하고, 인자·환경·작업 디렉터리를 소유자 전용 파일에 담는다. 환경 값은 출력하지 않는다. 디렉터리를 먼저 만든다(규칙 밖, 민감하지 않음): `mkdir -p -m 700 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8`.
```
<확인> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord 58090 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47
```
예상 출력:
- `pid <n>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord; parent 1 (systemd)`(부모가 1이거나 셸이면 정상).
- `cgroup: 0::/user.slice/user-1001.slice/session-<n>.scope` 같은 줄. `.service`로 끝나는 단위면 스크립트가 `STOP`으로 끝난다.
- `established clients on :58090: 0`.
- 경로 인자마다 한 줄: `--tokens …`, `--manifest …`, `--profiles …`, `--bindings …`, `--policy …`(상대 경로면 `(relative to …)`). A3이 이 경로들을 절대 경로로 바꿔 쓴다.
- `captured <n> argv entries, <n> environment entries (<k> exported shell functions, dropped at start; IEO_DB_URL present; values not shown), cwd <경로>`.

그다음 A1의 호출을 다시 한다. [3]이 `form …; user <이름> dbname <이름> host <…> port <…> ; password set: <True|False>`를 낸다(**암호 값은 출력되지 않는다**). 에이전트는 `env0`·`argv0`을 직접 열지 않는다(`deny`). 아래에서 `<user>`·`<db>`는 이 user·dbname, `<OLD_TOKENS>`·`<MANIFEST>`·`<PROFILES>`·`<BINDINGS>`·`<POLICY>`는 캡처가 출력한 경로 인자의 절대 경로다.

## A3 · 준비(에이전트, 민감 경로 없음)

U1의 digest(`<UDIG>`)를 받은 뒤 한다. 모두 규칙 밖이라 확인 창을 거칠 수 있다.

1. **빌드 확인**: `sha256sum $D/bin/ieo-coord-M $D/bin/m-46799b0/ieo-coord $D/bin/m-46799b0/ieo-supervisor` → `d6a742145ea1…928f`(앞의 둘), `5fca01584a09…0cc3`. `podman image exists docker.io/library/golang@sha256:d9c68c2c…`.
2. **주체 토큰·헤더**(실행안 §2.1, 0600, 값 출력 없음; 이미 있으면 덮어쓰지 않음):
   ```
   cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8 && for p in s8-registrant s8-assigner s8-operator s8-ingest s8-sup-dev s8-sup-gr; do [ -e $p.token ] || ( set -C; umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > $p.token && printf 'Authorization: Bearer %s\n' "$(cat $p.token)" > $p.header ); done; stat -c '%a %n' s8-*.token s8-*.header
   ```
   예상 출력: `600 s8-<이름>.header`·`600 s8-<이름>.token` 열두 줄.
3. **새 `tokens.json`**(실행안 §2의 표, 드라이런 `pr()`와 같은 꼴): 옛 파일의 주체 전부 + 단계 8 주체 일곱. 클라이언트 주체는 `repo_ids: ["ieo-poc-test"]`, agent 주체는 repo 하나와 `role`. 옛 `tokens.json`은 그대로 둔다(되돌리기용). 이름이나 digest가 옛 주체와 겹치면 쓰지 않고 멈춘다.
   ```
   python3 -I - <OLD_TOKENS> <UDIG> <<'PY'
   import hashlib, json, os, re, sys
   S = "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8"
   old, udig = sys.argv[1], sys.argv[2]
   if not re.fullmatch(r"[0-9a-f]{64}", udig): sys.exit("user digest is not 64 hex")
   d = json.load(open(old))
   dg = lambda n: hashlib.sha256(open(f"{S}/{n}.token", "rb").read()).hexdigest()
   R = ["ieo-poc-test"]
   P = lambda n, kind, scopes, digest=None, **x: dict({"name": n, "token_sha256": digest or dg(n), "kind": kind, "scopes": scopes, "repo_ids": R}, **x)
   new = [P("s8-registrant", "service", ["sprints:register", "sprints:read"]),
          P("heain", "user", ["review:semantic", "sprints:read", "roles:control"], udig),
          P("s8-assigner", "service", ["coordinator:assign"]),
          P("s8-operator", "service", ["roles:control"]),
          P("s8-ingest", "service", ["evidence:ingest"]),
          P("s8-sup-dev", "agent", ["checkpoint:report"], role="development"),
          P("s8-sup-gr", "agent", ["checkpoint:report"], role="guardrail")]
   clash = {p["name"] for p in d["principals"]} & {p["name"] for p in new}
   clash |= {p["token_sha256"] for p in d["principals"]} & {p["token_sha256"] for p in new}
   if clash: sys.exit(f"clash with the old principals: {len(clash)} (names or digests)")
   d["principals"] = d["principals"] + new
   with os.fdopen(os.open(f"{S}/tokens-s8.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as f: json.dump(d, f, indent=1)
   print("tokens-s8.json:", len(d["principals"]) - len(new), "old +", len(new), "new principals")
   PY
   ```
   그다음 읽기 전용 요약(digest 없이): `python3 -I -c 'import json,sys; [print(p["name"], p["kind"], p["scopes"], p.get("repo_ids"), p.get("role")) for p in json.load(open(sys.argv[1]))["principals"]]' $D/private/s8/tokens-s8.json`. 예상: 옛 주체들(A2의 `--tokens` 파일과 같은 이름·종류·범위) 뒤에 위 일곱 줄.
4. **manifest·profiles 비교**(실행안 §1.2 끝 “실행 시 확인”): 58090이 쓰는 파일과 46799b0 저장소의 manifest(`docs/poc/Console_Coordinator_Sprints_v0.json`, SHA-256 `ae5c1e7f230b0101ccfa759ccd1050868d218cced28653a7b84c48a51bfe4e85`)를 비교하고, 효과 manifest(manifest + profiles)에 명세의 수용 프로파일이 있는지 본다(`--profiles`가 없으면 둘째 인자를 `-`로).
   ```
   python3 -I - <MANIFEST> <PROFILES 또는 -> <<'PY'
   import hashlib, json, sys
   m, p = sys.argv[1], sys.argv[2]
   h = lambda f: hashlib.sha256(open(f, "rb").read()).hexdigest()
   print("manifest", h(m), "equals 46799b0:", h(m) == "ae5c1e7f230b0101ccfa759ccd1050868d218cced28653a7b84c48a51bfe4e85")
   ps = json.load(open(m)).get("inspection_profiles", []) + (json.load(open(p)) if p != "-" else [])
   if p != "-": print("profiles", h(p))
   print("profiles in effect:", [(x["profile_id"], x["revision"]) for x in ps])
   print("poc-acceptance-v1.3.3 r7 with 02-05.accept:", any(x["profile_id"] == "poc-acceptance-v1.3.3" and x["revision"] == 7 and "02-05.accept" in x["items"] for x in ps))
   PY
   ```
   예상: `equals 46799b0: True`, 마지막 줄 `True`. manifest가 다르면 에이전트가 46799b0에서 `ieo-spec`을 빌드해(`cargo build --release --locked -p ieo-spec`, 대상 디렉터리 `$D/targets/m-46799b0`) 효과 manifest로 `ieo-spec check $D/step8-46799b0/sprint-POC-S8-01-r1.json --manifest <효과 manifest> --executable`를 돌려 `201 ACCEPTED_FOR_SEMANTIC_REVIEW`와 digest `a8d97f12…7bb7`을 확인한다. 아니면 멈춘다.
5. **55433 읽기 전용 질의**(`podman exec ieo-poc-db psql -U <user> -d <db> -Atc …`): 새 표 11개(`role_checkpoints` … `role_recovery_uses`, rehearse-migration.sh의 목록) 중 있는 것 0, `SELECT count(*) FROM sprint_revisions WHERE sprint_id = 'POC-S8-01'` = 0, `SELECT count(*) FROM notion_imports WHERE test_id = 'POC-S8-01'` = 0, `<BINDINGS>`에 `ieo-poc-test`가 없고 같은 `sprint_id`를 두 repo가 쓰지 않음, `SELECT role, assignment_id, generation FROM assignment_heads WHERE repo_id = 'ieo-poc-test'` = 행 없음.
6. **백업**(실행안 §3의 명령 그대로, `<user>`·`<db>`를 넣어): `$D/backups/<UTC>-pre-step8/`에 `ieo.dump`(`pg_dump -Fc`), `ieo.dump.sha256`, `ieo.dump.list`(`pg_restore --list`), `live-counts.txt`.
7. **시험 DB(55432) 마이그레이션 예행**(실행안 §3): 시험 DB 암호 파일은 `/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/pgpw`(0600, 출력 안 함).
   ```
   export TEST_DB_PASSWORD_FILE=/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/pgpw && export TOKENS_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json && export BINDINGS_FILE=<BINDINGS> && export MANIFEST=<MANIFEST> && export POLICY=<POLICY> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/rehearse-migration.sh <백업>/ieo.dump /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M 58291 <백업>/live-counts.txt
   ```
   예상 끝 줄 `## rehearsal: PASS`. (`--profiles`는 예행의 serve 인자에 없다. 바인딩의 수용 프로파일이 profiles 파일에만 있어 serve가 실패하면 멈추고 보고한다.)

`pg_dump`나 `psql`이 암호를 요구하거나 분류기가 옛 `tokens.json`·바인딩 파일 읽기를 막으면 멈추고, 그 명령을 사용자에게 글자 그대로 준다.

## A4 · 역할 컨테이너 둘(에이전트)

U2가 “같다”일 때만 한다(실행안 §5.2 단계 5, §5.3). 스크립트가 OAuth 자격을 역할 볼륨으로 **복사만** 한다(앞부분 형식만 확인, 값 출력 없음). 작업 디렉터리는 새것이어야 한다(있으면 스크립트가 거절). `git clone`·두 역할 기동 대기로 2분을 넘을 수 있으니 **Bash 도구의 timeout을 600000 ms로** 준다(중간에 끊기면 볼륨이 남아 같은 디렉터리를 다시 쓸 수 없다).
```
<확인> && export QUOTA_GROUP_ID=claude-sub-shared && export SUP_DEV_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-dev.token && export SUP_GR_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-gr.token && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/m-46799b0 /opt/rust/ieo_poc_runs/02-05-s8-20261118
```
예상 출력:
- `copies ok`
- `# role containers up: ieo-s8-ieo-poc-test-{development,guardrail} on 127.0.0.1:58221/58222 · image docker.io/library/golang@sha256:d9c68c2c… · network slirp4netns:allow_host_loopback=true · coordinator http://10.0.2.2:58090`
- `# target https://github.com/HeaInSeo/ieo_poc_test @ 2ab9d8760012516f37674a9446ca2fb2699973c0 (read-only copies) · credentials: OAuth form (values not printed) · roles entry: /opt/rust/ieo_poc_runs/02-05-s8-20261118/roles-entry.json`

`… credential is not the OAuth token form`, `role … did not come up`, `already holds role volumes`가 나오면 멈추고 보고한다(컨테이너만 지우고 볼륨은 둔다).

그다음 A1의 호출을 다시 한다. [4]가 `W=/opt/rust/ieo_poc_runs/02-05-s8-20261118` 하나를 찾고, 두 역할의 `quota_group_id`(`claude-sub-shared`)와 `adapter_token_file`이 `W/adapter.token`인지, 헤더 형식을 확인한 뒤에만(출력 `both claude-sub-shared: True ; adapter_token_file in W: True`, `adapter.header form ok: True`) `adapter.header`(0600)·`roles-entry.json`(0644)을 `$D/private/s8/`로 복사한다. 어댑터 토큰 자체는 `W`에 남는다(코디네이터 프로세스가 읽음). 에이전트는 `W` 안을 직접 읽지 않는다. 마지막으로 `podman ps --format '{{.Names}} {{.Status}} {{.Ports}}'`로 `ieo-s8-ieo-poc-test-development`·`…-guardrail`이 `Up`이고 `127.0.0.1:58221->8080/tcp`, `127.0.0.1:58222->8080/tcp`인지 본다.

## A5 · 기동 준비(에이전트)

1. `$D/private/s8/roles.json`(실행안 §5.3, 드라이런과 같은 코드에 실제 경로):
   ```
   python3 -I - <<'PY'
   import json, os
   D = "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data"
   e = json.load(open(D + "/private/s8/roles-entry.json"))
   assert set(e) == {"development", "guardrail"}
   e["budget_plan"] = {"budget": "02-05", "ledger_dir": D + "/ledger-0205",
                       "first_observation": {"stream": D + "/ledger-0205-obs/D6.jsonl", "observed_unix": 1794698043},
                       "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}
   with os.fdopen(os.open(D + "/private/s8/roles.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as f: json.dump({"ieo-poc-test": e}, f, indent=1)
   PY
   ```
   그다음 읽기 전용 요약(비밀 없음): `python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); e=d["ieo-poc-test"]; [print(r, e[r]["url"], e[r]["quota_group_id"], e[r]["profile_id"], e[r]["adapter_token_file"]) for r in ("development","guardrail")]; print(e["budget_plan"])' $D/private/s8/roles.json`. 예상: 두 역할이 `127.0.0.1:58221/58222`, `claude-sub-shared`, `s8-dev`/`s8-gr`, `/opt/rust/ieo_poc_runs/02-05-s8-20261118/adapter.token`, 그리고 위 `budget_plan`.
2. `mkdir -p -m 700 $D/ledger-0205 $D/s8-out`.
3. 정지 직전 재확인: A1의 호출 다시(02-04 진행 중 없음, `PASS`), `ss -Htn state established '( sport = :58090 )' | wc -l` = 0, `coord.pid`의 pid의 `/proc/<pid>/exe`가 아직 `$D/bin/ieo-coord`, 두 역할 서비스 `curl -s -H @$D/private/s8/adapter.header http://127.0.0.1:5822{1,2}/v0/role/status`가 `"running":null`, `df -h /opt`.

## A6 · 옛 코디네이터 정지와 새 빌드 기동(에이전트)

실행안 §5.2 단계 7~8, §5.4. **라이브가 멈추는 시간은 약 2분이다.** 한 Bash 호출에서 정지와 기동을 잇는다. 기동은 캡처한 환경(`env0`, 셸 내장으로만 되살림: 값은 어떤 명령줄에도 나오지 않음; 기동 전에 그 셸의 환경을 모두 지우므로 에이전트 세션의 환경은 코디네이터에 들어가지 않음)·작업 디렉터리·인자 + 새 `tokens.json`·`--roles`로 한다. 기동 때 55433 마이그레이션이 한 트랜잭션으로 돈다.
```
<확인> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30 && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-M.log --tokens /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json -- --roles /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/roles.json
```
예상 출력:
- `copies ok`
- `pid <옛 pid> stopped`(778ee47은 신호 처리기가 없어 SIGTERM에 곧 끝난다. 30초를 넘으면 스크립트가 SIGKILL을 보내고 그렇다고 적는다)
- `pid <새 pid>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M healthy on 127.0.0.1:58090 in <캡처한 cwd> (log …/coord-M.log)`

그다음 **별도 Bash 호출에서**(기동한 호출이 끝난 뒤에도 프로세스가 사는지) 확인한다: `GET /v0/health` = 200, `coord.pid`의 pid가 살아 있고 `/proc/<pid>/exe`가 `$D/bin/ieo-coord-M`, 그 부모가 Bash 도구의 프로세스 트리에 있지 않음(1이나 `systemd --user` 같은 subreaper면 정상), `ss -Htlnp '( sport = :58090 )'`의 수신자가 그 pid, 로그 끝 `ieo-coord listening on 127.0.0.1:58090; policy …`(바인딩 오류 없음).

**되돌리기는 다음 중 하나일 때만** 한다: 기동 실패(`STOP: … exited at start` 또는 `not healthy after 30 s`, exit 3), health가 200이 아님, pid가 죽음, exe가 다름, 58090 수신자가 다른 pid, 로그에 바인딩·기동 오류. 부모 pid만 다른 것은 되돌릴 이유가 아니다. 기동한 호출이 끝나며 프로세스가 사라졌다면 에이전트가 되돌리지 않고(같은 일이 반복됨) 사용자에게 아래 되돌리기 명령을 준다.

되돌리기는 두 호출이다. DB는 건드리지 않는다(마이그레이션이 실패했다면 DB는 그대로이고, 성공했다면 778ee47이 추가된 표를 무시하고 돈다; 예행에서 확인함). 옛 `tokens.json`과 옛 인자는 캡처한 `argv0` 그대로다. 첫 호출은 M이 아직 떠 있으면 끄고, 이미 끝났으면 `STOP: pid … is not running`(exit 3)을 낸다. 둘 다 정상이고 둘째 호출로 간다.
```
<확인> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30
```
```
<확인> && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-778ee47-rollback.log
```
예상 출력: `copies ok`, `pid <n>: …/bin/ieo-coord healthy on 127.0.0.1:58090 …`. 그다음 별도 호출에서 `health 200`과 실패한 M의 로그 끝 20줄(실패 원인)을 보고한다. 되돌리기도 실패하면(`health`가 200이 아님) 더 손대지 않고 보고한다.

**막히면.** 권한 확인은 Bash 호출 전체에 대해 실행 전에 이뤄지므로, 정지·기동 호출이 막히면 아무것도 실행되지 않은 것이고 라이브는 그대로다. 멈추고 그 호출을 사용자에게 준다. 되돌리기 둘째 호출이 막히면(58090이 멈춘 상태) 곧바로 그 호출을 사용자에게 준다. 사용자는 같은 줄을 자기 터미널에 붙여 넣으면 된다.

## A7 · sprint 등록(에이전트)

실행안 §5.2 단계 9.
```
<확인> && export COORD=http://127.0.0.1:58090 && export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8 && export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out && export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205 && export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh register
```
예상 출력: `copies ok`, `registered POC-S8-01 r1 (201), digest a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7: the user's semantic review is next …`. 에이전트가 사용자에게 알림: sprint `POC-S8-01`, revision `1`, digest **`a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7`**(명세 파일과 같음을 스크립트가 확인), 명세 파일 `$D/step8-46799b0/sprint-POC-S8-01-r1.json`.

## U3 · 독립 의미 검토(사용자 본인)

명세(`$D/step8-46799b0/sprint-POC-S8-01-r1.json`)를 읽고, 등록한 주체(`s8-registrant`)와 다른 사람으로서 통과를 남긴다(정책 G3, 실행안 §2.2의 2). 토큰은 파일에서 읽고 출력하지 않는다.
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; T=$D/private/user/heain.token
REASON='<명세를 읽고 쓴 이유: 예) 대상·한도·작업이 단계 8 결정과 맞음>'
DIG=a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7
python3 -I -c 'import json,sys; print(json.dumps({"decision": "passed", "reason": sys.argv[1], "digest": sys.argv[2]}))' "$REASON" "$DIG" > /tmp/s8-review.json
curl -sS --max-time 30 -o /tmp/s8-review-answer.json -w 'http %{http_code}\n' \
  -H @<(printf 'Authorization: Bearer %s\n' "$(cat "$T")") -H 'Content-Type: application/json' \
  -X POST http://127.0.0.1:58090/v0/sprints/POC-S8-01/revisions/1/semantic-review --data @/tmp/s8-review.json
python3 -I -c 'import json; d=json.load(open("/tmp/s8-review-answer.json")); print({k: d.get(k) for k in ("decision", "digest", "review_state", "reviewer", "code", "message")})'
rm -f /tmp/s8-review.json /tmp/s8-review-answer.json
```
예상 출력: `http 201`, 그다음 결정 `passed`와 digest가 위 값인 요약 한 줄.
보내 줄 것: 두 줄. `http 201`이 아니면 그 요약(코드·메시지)을 보낸다.

## A8 · 배정과 회차(에이전트)

`run-step8.sh`는 `TOKDIR`(에이전트가 만든 헤더), `ADAPTER_HEADER`(A4의 사본), `LEDGER`(코디네이터가 쓰는 장부), `OUT`만 읽고 쓴다. 역할 볼륨(`W`), 자격, `/proc`의 환경은 읽지 않는다.
1. **`assignment_heads` 재확인**(실행안 §5.2 단계 11, 읽기 전용, `prepare` 바로 전):
   ```
   podman exec ieo-poc-db psql -U <user> -d <db> -Atc "SELECT role, assignment_id, generation FROM assignment_heads WHERE repo_id = 'ieo-poc-test' ORDER BY role"
   ```
   예상 출력: **없음**(행 0). 그러면 `EXPECTED_CURRENT_DEV/GR`은 둘 다 `null`이다. 행이 하나라도 있으면 `prepare`를 하지 않고 멈추고 그 행을 보고한다(위 권한 설정의 설명).
2. **`prepare`**(헤드가 비었을 때만, 같은 호출에서 `null`을 글자로 export):
   ```
   <확인> && export COORD=http://127.0.0.1:58090 && export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8 && export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out && export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205 && export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header && export EXPECTED_CURRENT_DEV=null && export EXPECTED_CURRENT_GR=null && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh prepare
   ```
   예상 출력: `copies ok`, `roles ready: both reachable, central, idle, no quota mismatch`, `prepared: S8-DEV-A1, S8-GR-A1, branch_head 2ab9d8760012516f37674a9446ca2fb2699973c0`.
3. **`runs`**: 관측 마감 확인 → R1(start) → R2(reconnect) → G1(start), 회차마다 실행안 §6.2의 모든 확인. 도구 한 호출의 600초를 넘을 수 있으므로 **배경으로**(`run_in_background`) 돌리고 끝까지 지켜본다. 하나라도 어긋나면 `STOP`으로 끝나고 더 보내지 않는다(묶음 안에서 다시 묻지 않음; 멈춤 보고 뒤 사용자 결정).
   ```
   <확인> && export COORD=http://127.0.0.1:58090 && export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8 && export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out && export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205 && export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header && export OBS_UNIX=1794698043 && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh runs
   ```
   예상 출력: `copies ok`, `s8-r1: ('no_checkpoint', 'start', 'accepted')`, `s8-r2: ('resume_allowed', 'reconnect', 'accepted')`, `s8-g1: ('no_checkpoint', 'start', 'accepted')`와 회차마다 `check-run.py`의 확인 줄, 끝 줄 `R1, R2 and G1 recorded and checked`.
4. 결과 보고: 회차마다 판단·연산·결과, 계측 사용량, 도구, 누적과 남은 예산, 연결 조회, 추가 청구 0의 근거. 미계측은 미계측으로 적는다. 그 뒤 사용자에게 단계 8의 `allow` 줄을 모두 지워 달라고 알린다.

## 실패하면

| 어디서 | 하는 일 |
| --- | --- |
| 실행본 확인 | 멈추고 보고. 사본을 고치거나 다시 복사하지 않는다(검토된 해시와 다르기 때문) |
| 허용된 명령이 막힘 | 멈추고 그 호출을 글자 그대로 사용자에게 준다(58090이 멈춘 뒤라면 되돌리기 명령을 바로) |
| U1·U2, A1~A3 | 멈추고 보고. 라이브는 바뀌지 않았다 |
| A4 | 멈추고 보고. 컨테이너만 지우는 명령을 알린다(볼륨은 둠) |
| A6 기동 실패 | 위 되돌리기(옛 바이너리, DB 그대로) 후 보고 |
| A8.1에 헤드가 있음 | `prepare`를 하지 않고 멈추고 보고 |
| A7~A8 | 멈추고 보고. 등록·배정·회차 기록이 생긴 뒤라 자동으로 되돌리지 않는다 |
| 회차 중 무엇이든 | 재시작·재시도·예비는 하지 않는다(묶음 밖). 예비는 실패 원인과 증거 보고 뒤 건별 승인 |
