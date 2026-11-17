# 02-05 단계 8 수동 절차 · 사용자 단계와 에이전트 단계의 순서

| 항목 | 기록 |
| --- | --- |
| 근거 | 사용자 승인: 병합된 [실행안](EXECUTION_PLAN.md)(main 46799b0, main CI 44033234326 success)의 §8 묶음, id는 계획대로, 예산·멈춤 규칙 그대로. 예비·DB 복원·회차 중 재시작은 묶음 밖. 자동 되돌리기 = 옛 바이너리를 DB 그대로 |
| 왜 나누나 | 에이전트의 자동 모드 분류기가 민감한 경로를 막는다: `/opt/rust/ieo_poc_secrets`, 그리고 `/opt/rust/ieo_poc_runs`, `/proc/<pid>/environ`, 캡처한 `env0`도 그렇다고 본다. 사용자 선택 (나): **그 경로를 건드리는 단계는 사용자가 따로 연 터미널에서** 하고, 나머지는 에이전트가 한다 |
| 표기 | **U**n = 사용자 단계(아래 명령을 그대로 붙여 넣음), **A**n = 에이전트 단계. 각 U 단계 끝의 “보내 줄 것”만 에이전트에게 주면 된다. 어떤 명령도 비밀 값(토큰, OAuth 자격, DB 암호)을 출력하지 않는다. 출력되는 것은 개수, True/False, 이름, 경로, 비밀이 아닌 것의 SHA-256뿐이다 |
| 실행본 | 스크립트·명세는 46799b0의 것을 에이전트가 `$D/step8-46799b0/`에 그대로 복사해 두었다(SHA-256이 46799b0과 같음을 확인). 새 빌드 M(46799b0, release)은 `$D/bin/ieo-coord-M`과 `$D/bin/m-46799b0/`에 있다. 옛 `$D/bin/ieo-coord`(778ee47)는 손대지 않았다 |
| 마감 | **R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 받아들여져야 한다(첫 관측 D6의 7일 한도, 구동 스크립트가 10분 여유를 두고 확인) |

모든 블록 앞에 같은 변수 정의가 있다. 블록 하나씩 붙여 넣는다. 어느 단계에서든 예상과 다른 출력이 나오면 **다음 단계로 가지 말고** 그 출력을 그대로 에이전트에게 보낸다.

## 순서 한눈에

| # | 누가 | 무엇 | 라이브 변경 |
| --- | --- | --- | --- |
| U1 | 사용자 | 본인 토큰 digest, D6 관측 확인·복사, 02-04 진행 중 회차 없음, 옛 코디네이터 캡처, 자격 계정 확인 | 없음(파일 쓰기만) |
| A2 | 에이전트 | 주체 토큰·새 `tokens.json`, 55433 읽기 전용 사전 질의, 백업·라이브 행 수, 시험 DB 마이그레이션 예행 | 없음(백업 파일, 시험 DB) |
| U2 | 사용자 | 역할 컨테이너 둘(OAuth 자격 복사) | 새 컨테이너만 |
| A3 | 에이전트 | `roles.json`, 장부 디렉터리, 정지 직전 재확인 | 없음 |
| U3 | 사용자 | 옛 코디네이터 정지 + 새 빌드 기동(캡처한 환경으로), 실패 시 되돌리기 | **58090, 55433 마이그레이션** |
| A4 | 에이전트 | sprint 등록 → revision digest 알림 | 55433 기록 |
| U4 | 사용자 | 의미 검토(본인 토큰) | 55433 기록 |
| A5 | 에이전트 | 배정·`branch_head`·R1→R2→G1과 회차마다 확인 | **모델 호출 3회** |

## U1 · 시작 전 확인과 캡처(사용자)

변수(모든 U 블록 공통):
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; LAUNCH=$D/step8-46799b0/launcher
```

### U1(a) 본인 토큰과 digest

이미 있으면 **덮어쓰지 않고** digest만 보여 준다. 없으면 새로 만든다(`set -C`: 덮어쓰기 금지). 에이전트는 이 파일을 읽지 않는다.
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; T=$D/private/user/heain.token
mkdir -p -m 700 $D/private/user
if [ -e "$T" ]; then echo "exists (not overwritten)"; else ( set -C; umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > "$T" ) && echo "created"; fi
stat -c '%a %U' "$T"; printf '%s' "$(cat "$T")" | sha256sum | cut -d' ' -f1
```
예상 출력: `exists (not overwritten)` 또는 `created`, 그다음 `600 heain`(또는 `400 heain`), 그다음 64자리 hex 한 줄(digest).
보내 줄 것: **그 64자리 digest**(비밀이 아니다).

### U1(b) 첫 관측 D6 확인과 복사

```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data
ls -1 /opt/rust/ieo_poc_runs/*/ledger/streams/D6.jsonl | tee /dev/stderr | wc -l
P=$(ls -1 /opt/rust/ieo_poc_runs/*/ledger/streams/D6.jsonl | head -1)
python3 -I -c 'import json,sys; ev=[json.loads(l)["rate_limit_info"] for l in open(sys.argv[1]) if "\"rate_limit_event\"" in l]; print(len(ev), all(e.get("status")=="allowed" and e.get("overageStatus")=="rejected" and e.get("isUsingOverage") is False for e in ev))' "$P"
python3 -I -c 'import json,sys; r=[json.loads(l) for l in open(sys.argv[1]) if l.strip()]; d=[x for x in r if x.get("attempt_id")=="D6"]; print(len(d), d[0].get("ended_unix") if d else None, (d[0].get("ended_unix") if d else None)==1794698043)' "$(dirname "$P")/../records.jsonl"
mkdir -p -m 700 $D/ledger-0205-obs && install -m 0600 "$P" $D/ledger-0205-obs/D6.jsonl && sha256sum $D/ledger-0205-obs/D6.jsonl | cut -c1-64
```
예상 출력: 경로 한 줄과 `1`(경로가 정확히 하나), `<1 이상> True`, `1 1794698043 True`, 64자리 hex(복사본의 SHA-256; stream에는 자격이 없다).
보내 줄 것: 위 네 줄 전부(경로 포함). `False`이거나 경로가 하나가 아니면 여기서 멈추고 그대로 보낸다.

### U1(c) 02-04 진행 중 회차 없음

```
ls /opt/rust/ieo_poc_runs/02-04-*/ledger/inflight.json 2>&1 | grep -c 'No such file'
ls /opt/rust/ieo_poc_runs/02-04-*/ledger/ -d
fuser /opt/rust/ieo_poc_runs/02-04-*/ledger/caller.lock 2>/dev/null; echo "fuser exit $?"
echo "ieo-role-call processes: $(pgrep -xc ieo-role-call)"
```
예상 출력: `1`(inflight.json 없음), 02-04 장부 경로 한 줄, `fuser exit 1`(잠금 파일을 연 프로세스 없음), `ieo-role-call processes: 0`.
보내 줄 것: 네 줄 전부.

### U1(d) 옛 코디네이터 캡처(읽기만)

pid가 778ee47 `ieo-coord`이고 `127.0.0.1:58090`인지, 부모·cgroup·클라이언트·경로 인자를 확인하고, 인자·환경·작업 디렉터리를 소유자 전용 파일에 담는다. 환경 값은 출력하지 않는다.
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; LAUNCH=$D/step8-46799b0/launcher
mkdir -p -m 700 $D/private/s8
bash $LAUNCH/coord-switch.sh capture $D/coord.pid $D/bin/ieo-coord 58090 $D/private/s8/coord-778ee47
ls -l $D/private/s8/coord-778ee47
python3 -I -c '
import sys, urllib.parse
env = dict(kv.split("=", 1) for kv in open(sys.argv[1], "rb").read().decode(errors="replace").split("\0") if "=" in kv)
u = env.get("IEO_DB_URL", "")
if "://" in u:
    p = urllib.parse.urlparse(u); print("form url", "user", p.username, "dbname", p.path.lstrip("/"), "host", p.hostname, "port", p.port)
else:
    kv = dict(x.split("=", 1) for x in u.split() if "=" in x); print("form key=value", "user", kv.get("user"), "dbname", kv.get("dbname"), "host", kv.get("host"), "port", kv.get("port"))
' $D/private/s8/coord-778ee47/env0
```
예상 출력:
- `pid <n>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord; parent 1 (systemd)` 같은 줄(부모가 1이거나 셸이면 정상).
- `cgroup: 0::/user.slice/user-1001.slice/session-<n>.scope` 같은 줄. `.service`로 끝나는 단위면 스크립트가 `STOP`으로 끝난다.
- `established clients on :58090: 0`.
- 경로 인자마다 한 줄: `--tokens …`, `--manifest …`, `--profiles …`, `--bindings …`, `--policy …`(상대 경로면 `(relative to …)`).
- `captured <n> argv entries, <n> environment entries (<k> exported shell functions, dropped at start; IEO_DB_URL present; values not shown), cwd <경로>`.
- `ls`에 `argv0`·`env0`(`-r--------`)·`cwd`(`-rw-------`).
- 마지막 줄: `form … user <이름> dbname <이름> host <…> port <…>`(**암호는 출력되지 않는다**).

보내 줄 것: `STOP`이 없었다면 위 출력 전부(값이 없는 줄들이다). 특히 경로 인자 줄들(`--tokens`·`--bindings`·`--manifest`·`--profiles`·`--policy`)과 마지막 줄의 user·dbname.

### U1(e) 자격 계정 확인

질문: 지금 두 역할 자격(`/opt/rust/ieo_poc_secrets/development.token`, `guardrail.token`)이 01-04에서 쓴 것과 **같은 구독 계정**인가?
보내 줄 것: “같다”(그러면 두 역할 모두 `QUOTA_GROUP_ID=claude-sub-shared`) 또는 “다르다”(그러면 G1 전 관측을 어떻게 할지 함께; 실행안 §9 Q2).

## A2 · 준비(에이전트)

U1의 답을 받은 뒤 에이전트가 한다. 민감 경로는 쓰지 않는다.
1. 빌드 확인: M(46799b0, release)의 `$D/bin/ieo-coord-M`, `$D/bin/m-46799b0/{ieo-coord,ieo-supervisor}`와 SHA-256, 기반 이미지 `golang@sha256:d9c68c2c…` 있음(준비 때 확인함).
2. 주체 토큰·헤더(0600)를 `$D/private/s8/`에 만들고(값 출력 없음), 새 `tokens.json`을 **새 경로** `$D/private/s8/tokens-s8.json`에: U1(d)의 `--tokens` 파일의 주체 전부(이름·종류·범위·digest) + 단계 8 주체 + 사용자 주체(U1(a)의 digest, `kind: user`, `review:semantic`·`sprints:read`·`roles:control`). 옛 `tokens.json`은 그대로 둔다(되돌리기용).
3. 55433 읽기 전용 질의(`podman exec ieo-poc-db psql -U <user> -d <db>`): 새 표 11개 없음, `POC-S8-01`이 `sprint_revisions`·`notion_imports`에 없음, 바인딩의 sprint 소유 충돌 없음, `assignment_heads`(배정의 `expected_current`).
4. 백업: `$D/backups/<UTC>-pre-step8/`에 `pg_dump -Fc`, SHA-256, `pg_restore --list`, 라이브 행 수(`live-counts.txt`).
5. 시험 DB(55432) 마이그레이션 예행: `TOKENS_FILE=$D/private/s8/tokens-s8.json BINDINGS_FILE=<U1(d)의 --bindings 절대 경로> MANIFEST=<--manifest> POLICY=<--policy> rehearse-migration.sh <덤프> $D/bin/ieo-coord-M 58291 <live-counts.txt>` → `## rehearsal: PASS`.

`pg_dump`나 `psql`이 암호를 요구하거나, 분류기가 `tokens.json`·바인딩 파일 읽기를 막으면 에이전트는 그 명령을 사용자 단계로 넘겨 같은 형식(명령·예상 출력·보내 줄 것)으로 다시 적어 보낸다.

## U2 · 역할 컨테이너 둘(사용자)

OAuth 자격을 역할 볼륨으로 **복사만** 한다(앞부분 형식만 확인, 값 출력 없음). 작업 디렉터리는 새것이어야 한다(있으면 스크립트가 거절).
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; LAUNCH=$D/step8-46799b0/launcher
W=/opt/rust/ieo_poc_runs/02-05-s8-$(date -u +%Y%m%d)
QUOTA_GROUP_ID=claude-sub-shared \
SUP_DEV_TOKEN_FILE=$D/private/s8/s8-sup-dev.token SUP_GR_TOKEN_FILE=$D/private/s8/s8-sup-gr.token \
  bash $LAUNCH/run-step8-roles.sh $D/bin/m-46799b0 "$W"
echo "exit $?"
install -m 0600 "$W/adapter.token" "$W/adapter.header" $D/private/s8/ && install -m 0644 "$W/roles-entry.json" $D/private/s8/roles-entry.json
podman ps --format '{{.Names}} {{.Status}} {{.Ports}}' | grep ieo-s8
echo "W=$W"
```
(`QUOTA_GROUP_ID`는 U1(e)의 답. 다르다고 답했다면 에이전트가 알려 준 값으로 바꾼다.)
예상 출력:
- `# role containers up: ieo-s8-ieo-poc-test-{development,guardrail} on 127.0.0.1:58221/58222 · image docker.io/library/golang@sha256:d9c68c2c… · network slirp4netns:allow_host_loopback=true · coordinator http://10.0.2.2:58090`
- `# target https://github.com/HeaInSeo/ieo_poc_test @ 2ab9d8760012516f37674a9446ca2fb2699973c0 (read-only copies) · credentials: OAuth form (values not printed) · roles entry: …/roles-entry.json`
- `exit 0`
- `ieo-s8-ieo-poc-test-development Up … 127.0.0.1:58221->8080/tcp`, `ieo-s8-ieo-poc-test-guardrail Up … 127.0.0.1:58222->8080/tcp`
- `W=/opt/rust/ieo_poc_runs/02-05-s8-<날짜>`

`… credential is not the OAuth token form`, `role … did not come up`, `already holds role volumes`가 나오면 멈추고 그대로 보낸다.
보내 줄 것: 출력 전부와 `W=` 줄. (에이전트는 `$D/private/s8/`의 `adapter.header`·`roles-entry.json` 사본만 쓴다. `W` 안은 읽지 않는다.)

## A3 · 기동 준비(에이전트)

1. `$D/private/s8/roles.json` = `roles-entry.json`(두 역할, `quota_group_id`) + `budget_plan`(`02-05`, `ledger_dir` `$D/ledger-0205`, `first_observation` `{stream: $D/ledger-0205-obs/D6.jsonl, observed_unix: 1794698043}`, 두 역할 `max_window_seconds` 600). `adapter_token_file`은 `W/adapter.token`(코디네이터 프로세스가 읽음).
2. `$D/ledger-0205/`(0700), 기록 디렉터리 `$D/s8-out/`.
3. 정지 직전 재확인[읽기]: `ss`로 58090 연결 0, `coord.pid`의 프로세스가 아직 그 `ieo-coord`, 두 역할 서비스 `GET /v0/role/status`(복사한 헤더로) `running: null`, `/opt` 여유.
4. U3의 명령에 넣을 경로를 확정해 알린다(아래 기본값과 같으면 “그대로”).

## U3 · 옛 코디네이터 정지와 새 빌드 기동(사용자)

**라이브가 멈추는 시간은 약 2분이다.** 블록 하나로 정지와 기동을 이어서 한다. 기동은 캡처한 환경(`env0`, 셸 내장으로만 되살림: 값은 어떤 명령줄에도 나오지 않음)·작업 디렉터리·인자 + 새 `tokens.json`·`--roles`로 한다. 기동 때 55433 마이그레이션이 한 트랜잭션으로 돈다.
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; LAUNCH=$D/step8-46799b0/launcher
bash $LAUNCH/coord-switch.sh stop $D/coord.pid 30 \
 && bash $LAUNCH/coord-switch.sh start $D/bin/ieo-coord-M $D/private/s8/coord-778ee47 $D/coord.pid $D/logs/coord-M.log \
      --tokens $D/private/s8/tokens-s8.json -- --roles $D/private/s8/roles.json
echo "exit $?"
curl -s -o /dev/null -w 'health %{http_code}\n' http://127.0.0.1:58090/v0/health
tail -5 $D/logs/coord-M.log
```
예상 출력:
- `pid <옛 pid> stopped`(778ee47은 신호 처리기가 없어 SIGTERM에 곧 끝난다. 30초를 넘으면 스크립트가 SIGKILL을 보내고 그렇다고 적는다)
- `pid <새 pid>: /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord-M healthy on 127.0.0.1:58090 in <캡처한 cwd> (log …/coord-M.log)`
- `exit 0`, `health 200`
- 로그 끝: `ieo-coord listening on 127.0.0.1:58090; policy …`(바인딩 오류 없음)

보내 줄 것: 출력 전부.

**기동이 실패하면**(`STOP: … exited at start` 또는 `not healthy after 30 s`, `exit 3`) 아래 되돌리기를 **바로** 한다. DB는 건드리지 않는다(마이그레이션이 실패했다면 DB는 그대로이고, 성공했다면 778ee47이 추가된 표를 무시하고 돈다; 예행에서 확인함). 옛 `tokens.json`과 옛 인자를 그대로 쓴다.
```
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data; LAUNCH=$D/step8-46799b0/launcher
[ -d "/proc/$(cat $D/coord.pid)" ] && bash $LAUNCH/coord-switch.sh stop $D/coord.pid 30
bash $LAUNCH/coord-switch.sh start $D/bin/ieo-coord $D/private/s8/coord-778ee47 $D/coord.pid $D/logs/coord-778ee47-rollback.log
echo "exit $?"
curl -s -o /dev/null -w 'health %{http_code}\n' http://127.0.0.1:58090/v0/health
tail -20 $D/logs/coord-M.log
```
예상 출력: `pid <n>: …/bin/ieo-coord healthy on 127.0.0.1:58090 …`, `exit 0`, `health 200`, 그리고 실패한 M의 로그 끝 20줄(실패 원인).
보내 줄 것: 출력 전부. 되돌리기도 실패하면(`health` 200이 아님) 더 손대지 말고 그대로 보낸다.

## A4 · sprint 등록(에이전트)

`run-step8.sh register`(`s8-registrant`, 키 `s8-reg-0001`, 명세 `$D/step8-46799b0/sprint-POC-S8-01-r1.json`) → 201. 에이전트가 사용자에게 알림: sprint `POC-S8-01`, revision `1`, digest **`a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7`**(명세 파일과 같음을 스크립트가 확인), 명세 파일 경로.

## U4 · 의미 검토(사용자 본인)

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

## A5 · 배정과 회차(에이전트)

민감 경로가 필요 없음을 확인했다: `run-step8.sh`는 `TOKDIR=$D/private/s8`(에이전트가 만든 헤더), `ADAPTER_HEADER=$D/private/s8/adapter.header`(U2의 사본), `LEDGER=$D/ledger-0205`(코디네이터가 쓰는 장부와 그 stream 사본), `OUT=$D/s8-out`만 읽고 쓴다. 역할 볼륨(`W`), 자격, `/proc`의 환경은 읽지 않는다.
1. `run-step8.sh prepare`: 검토 통과, 두 역할 도달·중앙 모드·유휴·quota 일치 → 배정 `S8-DEV-A1`·`S8-GR-A1`(A2의 `expected_current`) → `branch_head`(`2ab9d876…`, `refs/heads/main`).
2. `OBS_UNIX=1794698043 run-step8.sh runs`: 관측 마감 확인 → R1(start) → R2(reconnect) → G1(start), 회차마다 실행안 §6.2의 모든 확인. 하나라도 어긋나면 `STOP`으로 끝나고 더 보내지 않는다(묶음 안에서 다시 묻지 않음; 멈춤 보고 뒤 사용자 결정).
3. 결과 보고: 회차마다 판단·연산·결과, 계측 사용량, 도구, 누적과 남은 예산, 연결 조회, 추가 청구 0의 근거. 미계측은 미계측으로 적는다.

## 실패하면

| 어디서 | 하는 일 |
| --- | --- |
| U1 어느 확인이든 | 멈추고 출력을 보낸다. 라이브는 바뀌지 않았다 |
| A2(백업·예행) | 에이전트가 멈추고 보고. 라이브는 바뀌지 않았다 |
| U2 | 멈추고 출력을 보낸다. 에이전트가 컨테이너만 지우는 명령을 알려 준다(볼륨은 둠) |
| U3 기동 실패 | 위 되돌리기(옛 바이너리, DB 그대로) 후 출력을 보낸다 |
| A4~A5 | 에이전트가 멈추고 보고. 등록·배정·회차 기록이 생긴 뒤라 자동으로 되돌리지 않는다 |
| 회차 중 무엇이든 | 재시작·재시도·예비는 하지 않는다(묶음 밖). 예비는 실패 원인과 증거 보고 뒤 건별 승인 |
