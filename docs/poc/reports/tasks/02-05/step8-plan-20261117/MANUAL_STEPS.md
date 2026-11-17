# 02-05 단계 8 실행 절차 · 사용자 세 가지와 에이전트 단계

| 항목 | 기록 |
| --- | --- |
| 근거 | 사용자 승인: 병합된 [실행안](EXECUTION_PLAN.md)(main 46799b0, main CI 44033234326 success)의 §8 묶음, id는 계획대로, 예산·멈춤 규칙 그대로. 예비·DB 복원·회차 중 재시작은 묶음 밖. 자동 되돌리기 = 옛 바이너리를 DB 그대로 |
| 나눔 | 사용자 결정: **사용자는 세 가지만** 한다. ① 본인 토큰 생성과 digest(덮어쓰기 없음) ② 등록 뒤 독립 의미 검토 ③ 자격 계정이 같은지 답. 나머지는 모두 에이전트가 한다. 민감한 경로(`/opt/rust/ieo_poc_secrets`, `/opt/rust/ieo_poc_runs`, `/proc/<pid>/environ`, 캡처한 `env0`)를 건드리는 일은 아래 [권한 설정](#권한-설정사용자-시작-전-한-번)의 좁은 허용 규칙에 맞는 세 스크립트로만 한다 |
| 표기 | **U**n = 사용자 단계(명령을 그대로 붙여 넣음), **A**n = 에이전트 단계(에이전트가 Bash 도구로 그대로 실행). 어떤 명령도 비밀 값(토큰, OAuth 자격, DB 암호)을 출력하지 않는다. 출력되는 것은 개수, True/False, 이름, 경로, 비밀이 아닌 것의 SHA-256뿐이다 |
| 실행본 | 46799b0의 스크립트·명세 사본 `$D/step8-46799b0/`(16개 파일, 46799b0과 SHA-256이 같음을 확인), 도우미 `$D/step8-ops/obs-check.sh`(이 브랜치의 [launcher/obs-check.sh](launcher/obs-check.sh)와 같음), 목록 `$D/step8-ops/SHA256SUMS`. 둘 다 쓰기 금지(`chmod -R a-w`). 새 빌드 M(46799b0, release)은 `$D/bin/ieo-coord-M`과 `$D/bin/m-46799b0/`, 옛 `$D/bin/ieo-coord`(778ee47)는 그대로 |
| 마감 | **R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 받아들여져야 한다(첫 관측 D6의 7일 한도, 구동 스크립트가 10분 여유를 두고 확인) |

이 문서에서 `$D` = `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data`다. 다만 **민감 명령(아래 다섯 형식)에는 `$D`를 쓰지 않고 절대 경로를 글자 그대로 쓴다.** 허용 규칙이 명령의 글자를 맞추기 때문이다. 어느 단계에서든 예상과 다른 출력이 나오면 다음 단계로 가지 않는다.

## 권한 설정(사용자, 시작 전 한 번)

프로젝트의 `/opt/rust/src/github.com/HeaInSeo/ieo_poc/.claude/settings.local.json`에서 `permissions`에 아래 `allow`·`deny`를 합친다(기존 `additionalDirectories`는 그대로).
```json
{
  "permissions": {
    "allow": [
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture *)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop *)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start *)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh *)",
      "Bash(bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh)"
    ],
    "deny": [
      "Read(//opt/rust/ieo_poc_secrets/**)",
      "Read(//opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/**)"
    ]
  }
}
```
명령 형식의 약속(에이전트):
- 민감 명령은 언제나 **따로 선 하위 명령** `bash <절대 경로> <인자>`다. `sh`, `./`, 변수(`$D`), 명령 치환(`$(…)`)을 쓰지 않는다.
- 환경 변수는 같은 하위 명령 앞에 붙이지 않는다(`X=1 bash …` 금지). 같은 Bash 호출 안에서 `export X=…;`를 별도 하위 명령으로 먼저 둔다(호출 사이에는 셸 상태가 이어지지 않으므로 같은 호출 안이어야 한다).
- 하위 명령은 `&&` 또는 `;`로만 잇는다. 이어진 각 하위 명령이 저마다 규칙에 맞아야 한다.

허용 규칙이 있어도 어떤 민감 명령이 막히면 에이전트는 **멈추고**, 그 명령을 글자 그대로(같은 줄의 `export`까지) 사용자에게 주고 예상 출력과 함께 사용자 터미널에서 실행해 달라고 한다. 58090이 멈춘 뒤라면(A6) 기동 또는 되돌리기 명령을 바로 준다.

**단계 8이 끝나면**(A8 보고 뒤, 또는 되돌리기 뒤) 사용자가 위 `allow` 다섯 줄을 지운다. `deny` 두 줄은 남겨도 된다. 사본 두 디렉터리는 증거로 쓰기 금지인 채 둔다.

## 실행본 확인(에이전트, 민감 스크립트를 쓰기 직전마다)

A1·A2·A4·A5·A6(되돌리기 포함)·A7·A8에서 사본의 스크립트를 실행하기 직전에 같은 Bash 호출의 앞에 둔다.
```
cd /opt/rust/src/github.com/HeaInSeo/ieo_poc-data && sha256sum step8-ops/SHA256SUMS && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && echo "sums ok" && find step8-46799b0 step8-ops -perm /222 | wc -l
```
예상 출력: `0445557d19b83a621b52a2129b727d5566c5943401bb12b7b582e18f3549a640  step8-ops/SHA256SUMS`, `sums ok`, `0`(쓰기 가능한 항목 없음). 하나라도 다르면 멈추고 보고한다. 목록의 17개 중 민감 명령에 쓰는 셋:

| 파일 | SHA-256 |
| --- | --- |
| `step8-46799b0/launcher/coord-switch.sh` | `6e0245f2750c2db4c9a9fafda114d981537273fee617b76dcf6eb6f5de5b4b06` |
| `step8-46799b0/launcher/run-step8-roles.sh` | `b07cde82fa947785529488bd887b1a448e20c0ba3bf10d4fb044feccfdd14ac2` |
| `step8-ops/obs-check.sh` | `f5086109c117bb3272f1b59d47d9b6e159a67d58dfd04d24e1076efa4a0e3edc` |

## 순서 한눈에

| # | 누가 | 무엇 | 라이브 변경 |
| --- | --- | --- | --- |
| U1 | 사용자 | 본인 토큰과 digest(A3 전까지) | 없음 |
| U2 | 사용자 | 두 역할 자격이 01-04와 같은 구독 계정인지(A4 전까지) | 없음 |
| A1 | 에이전트 | `obs-check.sh`: D6 관측 확인·복사, 02-04 진행 중 회차 없음 | 없음(파일 복사) |
| A2 | 에이전트 | 옛 코디네이터 캡처, DB user·dbname 확인 | 없음(파일 쓰기) |
| A3 | 에이전트 | 주체 토큰·새 `tokens.json`, 55433 읽기 전용 질의, 백업, 시험 DB 마이그레이션 예행 | 없음(백업 파일, 시험 DB) |
| A4 | 에이전트 | 역할 컨테이너 둘, 헤더·역할 항목 복사 | 새 컨테이너만 |
| A5 | 에이전트 | `roles.json`, 장부 디렉터리, 정지 직전 재확인 | 없음 |
| A6 | 에이전트 | 옛 코디네이터 정지 + 새 빌드 기동, 실패 시 되돌리기 | **58090, 55433 마이그레이션** |
| A7 | 에이전트 | sprint 등록 → revision digest 알림 | 55433 기록 |
| U3 | 사용자 | 독립 의미 검토(본인 토큰) | 55433 기록 |
| A8 | 에이전트 | 배정·`branch_head`·R1→R2→G1과 회차마다 확인 | **모델 호출 3회** |

U1·U2는 처음에 함께 물어도 된다. A1·A2는 그 답을 기다리지 않는다.

## U1 · 본인 토큰과 digest(사용자)

이미 있으면 **덮어쓰지 않고** digest만 보여 준다. 없으면 새로 만든다(`set -C`: 덮어쓰기 금지). 에이전트는 이 파일을 읽지 않는다(`deny` 규칙).
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
보내 줄 것: “같다”(그러면 두 역할 모두 `QUOTA_GROUP_ID=claude-sub-shared`) 또는 “다르다”(그러면 A4 전에 멈추고 G1 전 관측을 어떻게 할지 묻는다; 실행안 §9 Q2).

## A1 · 첫 관측과 02-04 확인(에이전트)

```
bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-ops/obs-check.sh
```
`obs-check.sh`는 인자가 없고 경로가 모두 고정이다. 하는 일:
- [1] D6 stream이 정확히 하나, 그 안의 `rate_limit_event`가 모두 `allowed`·초과 사용 `rejected`·`isUsingOverage` false, D6 기록의 `ended_unix`가 1794698043. 맞을 때만 `$D/ledger-0205-obs/D6.jsonl`(0600)로 복사하고(이미 있으면 같은 내용이어야 함) SHA-256을 출력.
- [2] 02-04 장부마다 `inflight.json` 없음, `caller.lock`을 연 프로세스 없음(`fuser` exit 1), `ieo-role-call` 프로세스 0.
- [3] 캡처한 `env0`이 있으면 `IEO_DB_URL`의 user·dbname·host·port와 암호가 있는지(True/False)만. 없으면 건너뜀.
- [4] 역할 작업 디렉터리가 있으면 헤더·역할 항목 복사(A4). 없으면 건너뜀.

예상 출력: `D6 streams: 1`과 경로, `rate_limit_events <n>; all allowed, overage rejected, not in use: True`, `D6 records 1; ended_unix 1794698043; equals 1794698043: True`, `installed …/ledger-0205-obs/D6.jsonl (0600)`과 SHA-256, `02-04 ledgers: <n>`과 경로들, `inflight.json absent: True`, `processes holding caller.lock: none (fuser exit 1)`(또는 `caller.lock absent`), `ieo-role-call processes: 0`, [3]·[4] `skipped`, 끝 줄 `## obs-check: PASS`(exit 0). `FAIL:`이 있으면 exit 1이고 에이전트가 멈추고 보고한다.

## A2 · 옛 코디네이터 캡처(에이전트, 읽기만)

pid가 778ee47 `ieo-coord`이고 `127.0.0.1:58090`인지, 부모·cgroup·클라이언트·경로 인자를 확인하고, 인자·환경·작업 디렉터리를 소유자 전용 파일에 담는다. 환경 값은 출력하지 않는다. 디렉터리를 먼저 만든다(민감 명령이 아님).
```
mkdir -p -m 700 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8
```
```
bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh capture /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord 58090 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47
```
예상 출력:
- `pid <n>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord; parent 1 (systemd)`(부모가 1이거나 셸이면 정상).
- `cgroup: 0::/user.slice/user-1001.slice/session-<n>.scope` 같은 줄. `.service`로 끝나는 단위면 스크립트가 `STOP`으로 끝난다.
- `established clients on :58090: 0`.
- 경로 인자마다 한 줄: `--tokens …`, `--manifest …`, `--profiles …`, `--bindings …`, `--policy …`(상대 경로면 `(relative to …)`). A3이 이 경로들을 쓴다.
- `captured <n> argv entries, <n> environment entries (<k> exported shell functions, dropped at start; IEO_DB_URL present; values not shown), cwd <경로>`.

그다음 A1의 명령을 다시 실행한다. [3]이 `form …; user <이름> dbname <이름> host <…> port <…> ; password set: <True|False>`를 낸다(**암호 값은 출력되지 않는다**). 에이전트는 `env0`·`argv0`을 직접 열지 않는다. 결과(경로 인자, user·dbname)를 보고에 적는다.

## A3 · 준비(에이전트, 민감 경로 없음)

U1의 digest를 받은 뒤 한다.
1. 빌드 확인: `$D/bin/ieo-coord-M`, `$D/bin/m-46799b0/{ieo-coord,ieo-supervisor}`와 SHA-256(`d6a742145ea1…928f`, `5fca01584a09…0cc3`), 기반 이미지 `golang@sha256:d9c68c2c…` 있음.
2. 주체 토큰·헤더(0600)를 `$D/private/s8/`에 만든다(값 출력 없음): `s8-registrant`, `s8-assigner`, `s8-operator`, `s8-ingest`, `s8-sup-dev`, `s8-sup-gr`. 새 `tokens.json`을 **새 경로** `$D/private/s8/tokens-s8.json`에 쓴다: A2의 `--tokens` 파일의 주체 전부(이름·종류·범위·digest) + 단계 8 주체 + 사용자 주체(U1의 digest, `kind: user`, `review:semantic`·`sprints:read`·`roles:control`). 옛 `tokens.json`은 그대로 둔다(되돌리기용).
3. 55433 읽기 전용 질의(`podman exec ieo-poc-db psql -U <A2의 user> -d <A2의 dbname>`): 새 표 11개 없음, `POC-S8-01`이 `sprint_revisions`·`notion_imports`에 없음, 바인딩의 sprint 소유 충돌 없음, `assignment_heads`(배정의 `expected_current`).
4. 백업: `$D/backups/<UTC>-pre-step8/`에 `pg_dump -Fc`, SHA-256, `pg_restore --list`, 라이브 행 수(`live-counts.txt`).
5. 시험 DB(55432) 마이그레이션 예행(같은 Bash 호출 안에서 `export` 뒤 실행): `TEST_DB_PASSWORD_FILE`(시험 DB 암호 파일, 출력 안 함), `TOKENS_FILE=$D/private/s8/tokens-s8.json`, `BINDINGS_FILE`·`MANIFEST`·`POLICY`(A2의 `--bindings`·`--manifest`·`--policy` 절대 경로)를 `export`하고 `bash $D/step8-46799b0/launcher/rehearse-migration.sh <덤프> $D/bin/ieo-coord-M 58291 <live-counts.txt>` → `## rehearsal: PASS`.

`pg_dump`나 `psql`이 암호를 요구하거나 분류기가 옛 `tokens.json`·바인딩 파일 읽기를 막으면 멈추고, 그 명령을 사용자에게 글자 그대로 준다.

## A4 · 역할 컨테이너 둘(에이전트)

U2가 “같다”일 때만 한다. 스크립트가 OAuth 자격을 역할 볼륨으로 **복사만** 한다(앞부분 형식만 확인, 값 출력 없음). 작업 디렉터리는 새것이어야 한다(있으면 스크립트가 거절). `<YYYYMMDD>`는 실행하는 날의 UTC 날짜를 에이전트가 **글자로** 적는다(`$(date …)` 금지).
```
export QUOTA_GROUP_ID=claude-sub-shared; export SUP_DEV_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-dev.token; export SUP_GR_TOKEN_FILE=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/s8-sup-gr.token; bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8-roles.sh /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/m-46799b0 /opt/rust/ieo_poc_runs/02-05-s8-<YYYYMMDD>
```
예상 출력:
- `# role containers up: ieo-s8-ieo-poc-test-{development,guardrail} on 127.0.0.1:58221/58222 · image docker.io/library/golang@sha256:d9c68c2c… · network slirp4netns:allow_host_loopback=true · coordinator http://10.0.2.2:58090`
- `# target https://github.com/HeaInSeo/ieo_poc_test @ 2ab9d8760012516f37674a9446ca2fb2699973c0 (read-only copies) · credentials: OAuth form (values not printed) · roles entry: …/roles-entry.json`

`… credential is not the OAuth token form`, `role … did not come up`, `already holds role volumes`가 나오면 멈추고 보고한다(컨테이너만 지우고 볼륨은 둔다).

그다음 A1의 명령을 다시 실행한다. [4]가 작업 디렉터리 `W=/opt/rust/ieo_poc_runs/02-05-s8-<YYYYMMDD>`를 하나만 찾고, 두 역할의 `quota_group_id`(`claude-sub-shared`)와 `adapter_token_file`이 `W/adapter.token`인지, 헤더 형식을 확인한 뒤 `adapter.header`(0600)·`roles-entry.json`(0644)을 `$D/private/s8/`로 복사한다. 어댑터 토큰 자체는 `W`에 남는다(코디네이터 프로세스가 읽음). 에이전트는 `W` 안을 직접 읽지 않는다. 마지막으로 `podman ps --format '{{.Names}} {{.Status}} {{.Ports}}' | grep ieo-s8`로 두 컨테이너가 `Up`이고 `127.0.0.1:58221->8080/tcp`, `127.0.0.1:58222->8080/tcp`인지 본다.

## A5 · 기동 준비(에이전트)

1. `$D/private/s8/roles.json` = `roles-entry.json`(두 역할, `quota_group_id`, `adapter_token_file` `W/adapter.token`) + `budget_plan`(`02-05`, `ledger_dir` `$D/ledger-0205`, `first_observation` `{stream: $D/ledger-0205-obs/D6.jsonl, observed_unix: 1794698043}`, 두 역할 `max_window_seconds` 600).
2. `$D/ledger-0205/`(0700), 기록 디렉터리 `$D/s8-out/`.
3. 정지 직전 재확인: A1의 명령 다시(02-04 진행 중 없음, `PASS`), `ss`로 58090 연결 0, `coord.pid`의 프로세스가 아직 그 `ieo-coord`, 두 역할 서비스 `GET /v0/role/status`(`$D/private/s8/adapter.header`로) `running: null`, `/opt` 여유.

## A6 · 옛 코디네이터 정지와 새 빌드 기동(에이전트)

**라이브가 멈추는 시간은 약 2분이다.** 한 Bash 호출에서 정지와 기동을 잇는다. 기동은 캡처한 환경(`env0`, 셸 내장으로만 되살림: 값은 어떤 명령줄에도 나오지 않음; 기동 전에 그 셸의 환경을 모두 지우므로 에이전트 세션의 환경은 코디네이터에 들어가지 않음)·작업 디렉터리·인자 + 새 `tokens.json`·`--roles`로 한다. 기동 때 55433 마이그레이션이 한 트랜잭션으로 돈다.
```
bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30 && bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-M.log --tokens /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/tokens-s8.json -- --roles /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/roles.json
```
예상 출력:
- `pid <옛 pid> stopped`(778ee47은 신호 처리기가 없어 SIGTERM에 곧 끝난다. 30초를 넘으면 스크립트가 SIGKILL을 보내고 그렇다고 적는다)
- `pid <새 pid>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M healthy on 127.0.0.1:58090 in <캡처한 cwd> (log …/coord-M.log)`

그다음 **별도 Bash 호출에서**(기동한 호출이 끝난 뒤에도 프로세스가 사는지) 확인한다: `GET /v0/health` = 200, `coord.pid`의 pid가 살아 있고 `/proc/<pid>/exe`가 `$D/bin/ieo-coord-M`, 부모가 1, `ss -Htlnp`의 58090 수신자가 그 pid, 로그 끝 `ieo-coord listening on 127.0.0.1:58090; policy …`(바인딩 오류 없음). 기동한 호출이 끝나며 프로세스가 사라졌다면 아래 되돌리기를 에이전트가 하지 않고(같은 일이 반복됨) 사용자에게 되돌리기 명령을 준다.

**기동이 실패하면**(`STOP: … exited at start` 또는 `not healthy after 30 s`, exit 3, 또는 위 확인 실패) 아래 되돌리기를 **바로** 한다. DB는 건드리지 않는다(마이그레이션이 실패했다면 DB는 그대로이고, 성공했다면 778ee47이 추가된 표를 무시하고 돈다; 예행에서 확인함). 옛 `tokens.json`과 옛 인자를 그대로 쓴다. 앞의 `stop`은 M이 아직 떠 있으면 끄고, 이미 끝났으면 `STOP: pid … is not running`을 내고 넘어간다(`;`).
```
bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh stop /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid 30; bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/coord-switch.sh start /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/coord-778ee47 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/coord.pid /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/logs/coord-778ee47-rollback.log
```
예상 출력: `pid <n>: …/bin/ieo-coord healthy on 127.0.0.1:58090 …`. 그다음 별도 호출에서 `health 200`과 실패한 M의 로그 끝 20줄(실패 원인)을 보고한다. 되돌리기도 실패하면(`health`가 200이 아님) 더 손대지 않고 보고한다.

**막히면.** 정지가 막혔다면 라이브는 그대로다. 멈추고 위 정지·기동 명령을 사용자에게 준다. 정지 뒤 기동이 막혔다면(58090이 멈춘 상태) 에이전트는 곧바로 사용자에게 위 기동 명령(`coord-switch.sh start … ieo-coord-M …`, 앞의 `stop … &&` 없이)과, 그것도 실패할 때의 되돌리기 명령을 함께 준다. 사용자는 같은 줄을 자기 터미널에 붙여 넣으면 된다.

## A7 · sprint 등록(에이전트)

```
export TOKDIR=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8; export OUT=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/s8-out; export LEDGER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205; export ADAPTER_HEADER=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/adapter.header; bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh register
```
예상 출력: `registered POC-S8-01 r1 (201), digest a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7: the user's semantic review is next …`. 에이전트가 사용자에게 알림: sprint `POC-S8-01`, revision `1`, digest **`a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7`**(명세 파일과 같음을 스크립트가 확인), 명세 파일 `$D/step8-46799b0/sprint-POC-S8-01-r1.json`.

## U3 · 독립 의미 검토(사용자 본인)

명세(`$D/step8-46799b0/sprint-POC-S8-01-r1.json`)를 읽고, 등록한 주체(`s8-registrant`)와 다른 사람으로서 통과를 남긴다(정책 G3). 토큰은 파일에서 읽고 출력하지 않는다.
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

`run-step8.sh`는 `TOKDIR`(에이전트가 만든 헤더), `ADAPTER_HEADER`(A4의 사본), `LEDGER`(코디네이터가 쓰는 장부), `OUT`만 읽고 쓴다. 역할 볼륨(`W`), 자격, `/proc`의 환경은 읽지 않는다. 각 호출 앞에 A7과 같은 `export` 네 개를 둔다.
1. `… ; bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh prepare`: 검토 통과, 두 역할 도달·중앙 모드·유휴·quota 일치 → 배정 `S8-DEV-A1`·`S8-GR-A1`(A3의 `expected_current`) → `branch_head`(`2ab9d876…`, `refs/heads/main`).
2. `… ; export OBS_UNIX=1794698043; bash /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/step8-46799b0/launcher/run-step8.sh runs`: 관측 마감 확인 → R1(start) → R2(reconnect) → G1(start), 회차마다 실행안 §6.2의 모든 확인. 도구 한 호출의 600초를 넘을 수 있으므로 배경으로 돌리고 끝까지 지켜본다. 하나라도 어긋나면 `STOP`으로 끝나고 더 보내지 않는다(묶음 안에서 다시 묻지 않음; 멈춤 보고 뒤 사용자 결정).
3. 결과 보고: 회차마다 판단·연산·결과, 계측 사용량, 도구, 누적과 남은 예산, 연결 조회, 추가 청구 0의 근거. 미계측은 미계측으로 적는다. 그 뒤 사용자에게 `allow` 다섯 줄을 지워 달라고 알린다.

## 실패하면

| 어디서 | 하는 일 |
| --- | --- |
| 실행본 확인 | 멈추고 보고. 사본을 고치거나 다시 복사하지 않는다(검토된 해시와 다르기 때문) |
| 허용된 명령이 막힘 | 멈추고 그 명령을 글자 그대로 사용자에게 준다(A6의 정지 뒤라면 기동·되돌리기 명령을 바로) |
| U1·U2, A1~A3 | 멈추고 보고. 라이브는 바뀌지 않았다 |
| A4 | 멈추고 보고. 컨테이너만 지우는 명령을 알린다(볼륨은 둠) |
| A6 기동 실패 | 위 되돌리기(옛 바이너리, DB 그대로) 후 보고 |
| A7~A8 | 멈추고 보고. 등록·배정·회차 기록이 생긴 뒤라 자동으로 되돌리지 않는다 |
| 회차 중 무엇이든 | 재시작·재시도·예비는 하지 않는다(묶음 밖). 예비는 실패 원인과 증거 보고 뒤 건별 승인 |
