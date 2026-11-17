# 02-05 단계 8 실행안 · 한 번의 승인으로 하는 재배포와 R1→R2→G1

| 항목 | 기록 |
| --- | --- |
| 근거 | [사용자 결정 2026-11-17 단계 8 L01](../../../../User_Decisions_2026-11-17_step8-l01.md)(L01 값 승인, 실행안 요청, 묶음 승인), [단계 8 절차서](../step8-runbook-20261117/RUNBOOK.md), [예산 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md) §2.1, [예비 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md), [복구 결정 2026-11-16](../../../../User_Decisions_2026-11-16_recovery.md) |
| 기준 | main 05cf16e(PR #85 병합). 실행은 이 실행안을 담은 PR의 병합 SHA(아래 **M**)의 빌드로 한다 |
| 만든 방법 | 코드·문서와 원장 담당이 읽기 전용으로 확인한 사실(아래 “확인된 사실”)에서만 끌어냈다. 58090·55433·비밀·실행 기록은 보지 않았다. sprint 명세는 로컬 `ieo-spec check --executable`로 검사했고(§1.2), 역할 실행 스크립트([launcher/](launcher/))와 이 실행안의 §5.2~§6 전체를 가짜 `claude`·가짜 자격·시험 DB(55432)의 시험 코디네이터로 예행했다(§4, 21개 확인 모두 통과) |
| 표기 | **[읽기]** 읽기만. **[변경]** 실 시스템을 바꿈. **[모델]** 모델 호출. “실행 시 확인”은 실행 첫 단계에서 읽기 전용으로 확인하고, 다르면 멈추고 보고한다 |
| 승인 | §8의 묶음 하나를 사용자가 승인하면 그 안의 단계는 다시 묻지 않는다. 멈춤 조건(§6.3)에 걸리면 멈추고 보고한다. 예비는 묶음 밖이다(건별 승인) |

**확인된 사실(원장 담당, 읽기 전용).** 대상 저장소 `HeaInSeo/ieo_poc_test`(공개 합성 sandbox, `SANDBOX.json`의 `role_credentials` = `NOT_CONFIGURED`). `main` = `2ab9d8760012516f37674a9446ca2fb2699973c0`, 커밋 하나(“chore: bootstrap…”), 파일 `.github/workflows/bootstrap.yml`·`.gitignore`·`README.md`·`SANDBOX.json`. 58090 코디네이터는 **호스트 프로세스**(`/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/bin/ieo-coord`, pid 파일 `ieo_poc-data/coord.pid`, 2026-11-14 21:06 기동, 778ee47 빌드, 인자 `--listen --tokens --manifest --profiles --bindings --policy --workers`, 환경 `IEO_DB_URL`, `--roles`·`--grpc-listen` 없음). 영속 DB는 podman 컨테이너 `ieo-poc-db`(postgres:17-bookworm, 127.0.0.1:55433, 데이터 약 680M, `/home` 아래). 여유: `/` 11G(`/opt`·`/tmp`), `/home` 2.5G. 시험 DB 55432는 쓰지 않는다. 02-04 역할 컨테이너 `ieo-rt-fixture0204-*`는 멈춰 있고 그 볼륨은 다시 쓰지 않는다.

## 1. 값

### 1.1 대상과 식별자

| 항목 | 값 | 근거·비고 |
| --- | --- | --- |
| repo_id(코디네이터) | `ieo-poc-test` | **제안값.** 코디네이터의 repo id는 소문자·하이픈 관례(`console-coordinator`, 시험의 `repo-a`). 역할 설정 `repo`도 같은 값이라 개발 작업 디렉터리는 `/state/work/ieo-poc-test` |
| 저장소 | `https://github.com/HeaInSeo/ieo_poc_test` | 확인된 사실 |
| target_sha | `2ab9d8760012516f37674a9446ca2fb2699973c0` | `main` 머리. 실행 시 확인(`git ls-remote` [읽기]). 다르면 멈춤 |
| branch_ref | `refs/heads/main` | 단계 8은 push가 없으므로 R2의 GitHub 대조 대상은 `target_sha`. `branch_head` 증거 = `target_sha` |
| sprint_id·revision | `POC-S8-01`, 1 | 새 sprint. 비공개 바인딩·Notion 가져오기와 겹치지 않음을 실행 시 확인(§5.1) |
| task_id | `90-01` | 형식 `^\d{2}-\d{2}$`, manifest의 73개 작업과 겹치지 않는 값(로컬 검사가 `02-05`를 `TASK_ID_CONFLICT`로 거절해 고름) |
| 배정 | 개발 `S8-DEV-A1`, Guardrail `S8-GR-A1`, 둘 다 generation 0 | `expected_current`는 실행 시 `assignment_heads`를 읽어 정함(없으면 `null`) |
| 시도 id | `s8-r1`, `s8-r2`, `s8-g1` | 예비가 승인되면 `s8-sp1`(묶음 밖) |
| 멱등 키 | `s8-dlv-r1-0001`, `s8-dlv-r2-0001`, `s8-dlv-g1-0001`, 배정 `s8-asg-dev-0001`·`s8-asg-gr-0001`, 등록 `s8-reg-0001` | 8~128바이트. 시간 초과면 같은 키·같은 본문으로만 다시 보냄 |
| session_id | 회차마다 새 소문자 UUID: `cat /proc/sys/kernel/random/uuid`(소문자) | 보내기 전 `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$` 확인. R1·R2·G1 모두 다름 |
| 한도(L01, 사용자 승인) | sprint revision `limits` = `elapsed_seconds` 1,800·`total_tokens` 150,000·`correction_attempts` 0. 개발 배정 `limits` 1,800·150,000, Guardrail 배정 600·50,000 | 배정 한도는 revision 값을 넘지 못한다. `correction_attempts` 0: 단계 8은 세대 0만 쓴다(새 세대 없음) |
| 예산 | `02-05`(총 4회·회당 600초·합계 2,400초·200,000·cache_read 2,000,000, 역할 몫 개발 3회·Guardrail 1회), 추가 청구 USD 0 | 결정 2026-11-15 §2.1, 코드 `BUDGET_0205` |

### 1.2 sprint 명세 초안

`POC-S8-01` r1의 전체 명세는 별도 파일 `sprint-POC-S8-01-r1.json`(이 실행안과 같은 디렉터리)이다. 요점: `registration_state: ready`, `target {repo_id: ieo-poc-test, repository_url}`, 위 `limits`, `unresolved_bindings: []`, 작업 하나(`90-01`, 산출물 `90-01-answers` “README.md·SANDBOX.json·CI 워크플로의 사실에 대한 짧은 답(쓰기 없음)”, 수용 `poc-acceptance-v1.3.3` r7 항목 `02-05.accept`). 나머지 필드(`sources`·`schedule`·`delegation` 등)는 manifest의 POC-01에서 가져왔다. 로컬 검사: `ieo-spec check sprint-POC-S8-01-r1.json --manifest docs/poc/Console_Coordinator_Sprints_v0.json --executable` → **201 `ACCEPTED_FOR_SEMANTIC_REVIEW`**, digest `bbd2cfbd5d0d8388856ceb02ee21c455d650d522a6e5c0a661e01cc75d7d327c`. 예행의 시험 코디네이터도 같은 명세를 201로 등록했다. 수용 항목은 프로파일 r7에 단계 8 전용 항목이 없어 `02-05.accept`를 쓴다: 단계 8은 02-05 수용의 일부이므로 원장 담당이 이 재사용을 받아들였다. 실행 시 확인: 58090이 쓰는 `--manifest`·`--profiles` 파일이 저장소의 manifest와 같은지(다르면 그 파일로 다시 검사).

### 1.3 지시문(짧게, 읽기·답하기만)

공통: 파일 수정·명령 실행·네트워크(모델 외) 없음, 자식 에이전트 없음(02-04의 자식 회차가 미계측이 된 적이 있음), 답은 몇 줄. 02-04의 비슷한 회차는 회당 3~10초, 과금 토큰(input+output+cache_creation) 약 2,600~9,000이었다(공개 기록 `real-sessions-20261115/evidence/real/records.jsonl`). 그래서 600초·150,000 안에 넉넉하다. 가드 멈춤(시간·토큰·overage)은 미계측이 되고 예비로도 풀 수 없으므로 지시를 작게 둔다.

- **R1(개발, 새 세션 start, cwd `/state/work/ieo-poc-test`):**
  `Use only the Read tool. Read README.md and SANDBOX.json in the current directory. Reply in exactly three lines: 1) the first line of README.md, 2) the value of role_credentials in SANDBOX.json, 3) the names of the top-level keys of SANDBOX.json, comma-separated. Do not modify any file and do not run any command.`
- **R2(개발, 같은 세션 reconnect):**
  `Continue in the same session. Use only the Read tool. Read .github/workflows/bootstrap.yml. Reply in exactly three lines: 1) the workflow name, 2) its trigger events, comma-separated, 3) its job ids, comma-separated. Then add one line that repeats your answer 2) from the previous turn. Do not modify any file and do not run any command.`
  (마지막 줄은 재개가 R1 세션을 이었는지 답으로도 보이게 한다)
- **G1(Guardrail, start, cwd `/state/guardrail-cwd`, 허용 읽기 `/state/review/**`):**
  `Use only the Read tool. The files of the reviewed commit are in /state/review/ieo-poc-test. Below are numbered claims made by the development role. For each claim, read the file it is about and reply with one line: <number> CONFIRMED or <number> CONTRADICTED <file>. Do not write any file.` 뒤에 R1·R2 결과 문장(장부 기록의 결과 사건 `result` 문자열)을 번호를 붙여 그대로 붙인다. 실행 시 그 문장을 자동으로 넣는다(사람이 고치지 않는다).

## 2. 주체와 자격

| 주체(이름 제안) | kind | 범위 | repo | 누가 만들고 쥠 | 쓰임 |
| --- | --- | --- | --- | --- | --- |
| `s8-registrant` | service | `sprints:register`, `sprints:read` | `ieo-poc-test` | 에이전트가 생성(§2.1) | sprint 등록 |
| `<사용자 이름>`(예: `heain`) | **user** | `review:semantic`, `sprints:read`, `roles:control` | `ieo-poc-test` | **사용자 본인**(§2.2) | 독립 의미 검토(G3: 등록자와 다른 사람), 예비(건별 승인 때만) |
| `s8-assigner` | service | `coordinator:assign` | `ieo-poc-test` | 에이전트 | 배정 |
| `s8-operator` | service | `roles:control` | `ieo-poc-test` | 에이전트 | R1·R2·G1 전달(서비스 자격은 예비를 보낼 수 없음) |
| `s8-ingest` | service | `evidence:ingest` | `ieo-poc-test` | 에이전트 | `branch_head` 입력 |
| `s8-sup-dev` | agent | `checkpoint:report`, role `development` | `ieo-poc-test` | 에이전트(개발 볼륨의 `cred/coordinator_token`) | supervisor 보고 |
| `s8-sup-gr` | agent | `checkpoint:report`, role `guardrail` | `ieo-poc-test` | 에이전트(Guardrail 볼륨) | supervisor 보고 |

`tokens.json`에는 토큰 값이 아니라 SHA-256(`hex(sha256(토큰 문자열))`, 끝 공백 없이)만 들어간다(`auth::token_digest`). 새 `tokens.json` = 지금 58090이 쓰는 파일의 주체 전부 + 위 주체(실행 시 확인: 현재 파일 경로와 내용의 이름·종류·범위만 읽음).

### 2.1 에이전트가 만드는 것(값 출력 없음)

`/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/`(0700)에, 주체마다:
```
( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > s8-<name>.token )
printf '%s' "$(cat s8-<name>.token)" | sha256sum | cut -d' ' -f1   # 이 digest만 tokens.json에
```
요청 헤더는 주체마다 0600 파일로 한 번 만든다: `( umask 077; printf 'Authorization: Bearer %s\n' "$(cat s8-<name>.token)" > s8-<name>.header )`, 요청은 `curl -H @s8-<name>.header …`. 값이 명령줄·화면·기록에 남지 않는다(셸 함수 안의 `<(…)`는 curl이 읽기 전에 닫혀 401이 되는 것을 예행에서 확인했다). supervisor 토큰은 역할 볼륨의 `cred/coordinator_token`(0400, 그 역할 UID)으로 복사한다. 역할의 OAuth 자격은 02-04와 같이 `/opt/rust/ieo_poc_secrets/<역할>.token`을 볼륨의 `cred/oauth_token`(0400)으로 **복사만** 하고 앞부분 형식(`sk-ant-oat`)만 확인한다(값 출력 없음, 02-04 `run-real.sh`와 같음). 어댑터 토큰도 같은 방식으로 새로 만든다.

### 2.2 사용자 본인이 하는 것

1. 본인 토큰 만들기(에이전트는 이 파일을 읽지 않는다. 같은 OS 계정이라 파일 권한으로 막히지는 않으므로 이것은 절차상의 분리다):
   ```
   mkdir -p -m 700 /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user
   ( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/heain.token )
   printf '%s' "$(cat /opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/heain.token)" | sha256sum | cut -d' ' -f1
   ```
   마지막 줄이 출력하는 **digest만** 에이전트에게 준다(digest는 비밀이 아니다).
2. 의미 검토(재배포·등록 뒤, 에이전트가 revision digest를 알려 준다):
   ```
   T=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/heain.token
   curl -sS --max-time 30 -H @<(printf 'Authorization: Bearer %s\n' "$(cat $T)") -H 'Content-Type: application/json' \
     -X POST http://127.0.0.1:58090/v0/sprints/POC-S8-01/revisions/1/semantic-review \
     --data '{"decision": "passed", "reason": "<본인이 명세를 읽고 쓴 이유>", "digest": "<에이전트가 알려 준 digest>"}'
   ```
   201이면 끝. 다른 답이면 그대로 에이전트에게 준다.
3. (예비가 필요할 때만, 묶음 밖) 건별 승인 뒤 같은 방식으로 예비 전달을 보낸다(§6.4).

## 3. 백업 [변경: 파일 쓰기만, DB는 읽기]

위치: `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/backups/<UTC 시각>-pre-step8/`(`/opt`, `/home` 아님). DB 이름·사용자는 실행 시 확인(58090 `IEO_DB_URL`의 `user`·`dbname` 필드만 읽고 암호는 출력하지 않음, 아래 `<db>`·`<user>`).
```
B=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/backups/$(date -u +%Y%m%dT%H%M%SZ)-pre-step8; mkdir -p -m 700 "$B"
df -h /opt                                                     # 여유 확인(DB 약 680M, 덤프는 그보다 작음)
podman exec ieo-poc-db psql -U <user> -d <db> -Atc "SELECT pg_size_pretty(pg_database_size('<db>'))"
podman exec ieo-poc-db pg_dump -U <user> -d <db> -Fc > "$B/ieo.dump"
sha256sum "$B/ieo.dump" > "$B/ieo.dump.sha256"
podman exec -i ieo-poc-db pg_restore --list < "$B/ieo.dump" > "$B/ieo.dump.list"; grep -c "TABLE DATA" "$B/ieo.dump.list"
```
확인: 덤프 크기 > 0, `pg_restore --list`가 오류 없이 표 목록을 냄(`sprint_revisions`·`notion_imports`·`assignments`·`evidence` 포함). 아니면 멈춤.

복원(되돌리기 때만, §5.4): 새 코디네이터를 멈춘 뒤
```
podman exec ieo-poc-db dropdb -U <user> <db>
podman exec ieo-poc-db createdb -U <user> <db>
podman exec -i ieo-poc-db pg_restore -U <user> -d <db> --exit-on-error < "$B/ieo.dump"
```
그 뒤 778ee47 바이너리로 기동(§5.4).

## 4. 준비(승인 전, 실 시스템 변경 없음)

- **역할 실행 스크립트(완료).** [launcher/run-step8-roles.sh](launcher/run-step8-roles.sh)와 역할 설정 [launcher/role/](launcher/role/). 02-04 `run-real.sh`를 단계 8에 맞게 바꿨다: `checkpoint_central` → 코디네이터(supervisor `agent` 토큰은 `cred/coordinator_token`), 예산 `02-05`(몫 없음), `QUOTA_GROUP_ID` 필수, 대상 커밋의 파일만(`git archive <SHA>`, `.git`·자격 없음) 개발 `/state/work/ieo-poc-test`와 Guardrail `/state/review/ieo-poc-test`에 두고 쓰기 금지, 개발 허용 `Read`·`Grep`·`Glob`만(Guardrail은 supervisor 고정 규칙), 새 작업 디렉터리·볼륨만(이미 있으면 거절), 포트 58221/58222. 끝에 코디네이터 `--roles`의 repo 항목(`roles-entry.json`)과 어댑터 토큰을 남긴다.
- **호스트 연결(확정).** 컨테이너 네트워크 `slirp4netns:allow_host_loopback=true`, 컨테이너 안에서 코디네이터 주소 `http://10.0.2.2:58090`. 코디네이터는 계속 `127.0.0.1`에만 열린다(밖으로 열지 않음). 예행에서 두 supervisor가 이 주소로 takeover·체크포인트·첫 행동을 보고했다.
- **예행(완료).** [launcher/dry-run-step8.sh](launcher/dry-run-step8.sh): 같은 스크립트로 역할 컨테이너 둘(가짜 `claude`·가짜 자격), 이 브랜치로 만든 시험 코디네이터(시험 DB 55432의 새 데이터베이스, 끝나면 지움), 이 실행안의 주체 7개·명세 등록·사람 주체의 의미 검토·배정 둘·`branch_head` 입력·R1→R2→G1을 그대로 지났다. **21개 확인 모두 통과**([evidence/dry-run.log](evidence/dry-run.log)): R1 `no_checkpoint`→`start`, R2 `resume_allowed`→`reconnect`(R1 세션 재개), G1 `no_checkpoint`→`start`, 세 기록 모두 02-05 장부·계측됨·`stop_required` 없음, 두 supervisor 모두 중앙 모드, 연결 조회의 takeover·첫 행동, 코디네이터 SIGTERM에 0으로 끝남. 58090·55433·비밀·실행 기록·실제 `claude`는 쓰지 않았다.
- **실행 때 빌드.** 이 실행안의 PR 병합 뒤 그 SHA(**M**)의 깨끗한 작업 트리에서 `cargo build --release -p ieo-coord -p ieo-supervisor`(대상 디렉터리는 `/home` 밖). 바이너리 SHA-256 기록.

## 5. 재배포

### 5.1 사전 확인 [읽기](약 10분)

RUNBOOK §1을 그대로, 지금의 사실에 맞춰:
1. 빌드 M이 main CI 성공. 바이너리가 M에서 만들어짐.
2. 진행 중 회차 없음: 02-04 장부(`/opt/rust/ieo_poc_runs/02-04-*/ledger`, 실행 시 경로 확인)에 `inflight.json` 없음, 그 `caller.lock`을 연 프로세스 없음(`fuser`/`lsof`, 보기만), `ieo-role-call` 프로세스 없음(`ps`).
3. 55433 읽기 전용 질의(RUNBOOK §1 항목 3~5): 새 표 11개가 없음, `POC-S8-01`이 `sprint_revisions`·`notion_imports.test_id`에 없음, 비공개 바인딩의 `sprint_id`가 다른 repo와 겹치지 않음. `role_checkpoints` 질의는 표가 없어 오류 = 정상.
4. 첫 관측: 가장 새 실제 stream은 02-04 D6(종료 **2026-11-14T23:14:03Z**, `ended_unix` 1794698043, 공개 기록). 7일 한도라 **R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 보내야 한다. 실행하는 사람이 읽기 전용으로 확인한다(stream에는 자격이 없다. 내용은 출력하지 않고 판정만):
   ```
   ls -l /opt/rust/ieo_poc_runs/*/ledger/streams/D6.jsonl
   python3 -I -c 'import json,sys; ev=[json.loads(l)["rate_limit_info"] for l in open(sys.argv[1]) if "\"rate_limit_event\"" in l]; print(len(ev), all(e.get("status")=="allowed" and e.get("overageStatus")=="rejected" and e.get("isUsingOverage") is False for e in ev))' <그 경로>
   ```
   결과가 `<1 이상> True`이면 그 파일을 02-05 관측 경로로 복사한다. 경로가 하나가 아니거나 `False`면 멈춤.
5. 디스크: `/opt` 여유 ≥ 3G, `/home` 쓰기 없음.

### 5.2 순서와 시간 [변경]

| # | 단계 | 예상 | 확인 [읽기] |
| --- | --- | --- | --- |
| 1 | 백업(§3) | 2~3분 | 덤프 목록 |
| 2 | 778ee47 정지: `kill -TERM $(cat ieo_poc-data/coord.pid)`. 778ee47에는 신호 처리기가 없고 호스트 프로세스(PID 1 아님)라 SIGTERM의 기본 동작으로 곧 끝난다. 30초 안에 끝나지 않으면 `kill -KILL`. 바이너리를 `bin/ieo-coord.778ee47`로 보존 | 1분 | 프로세스 없음, 58090 닫힘 |
| 3 | 새 파일 놓기: `bin/ieo-coord`(M), 새 `tokens.json`(기존 + §2), `roles.json`(§5.3), 장부 `ieo_poc-data/ledger-0205/`(새 디렉터리, 0700), 관측 사본 `ieo_poc-data/ledger-0205-obs/D6.jsonl` | 3분 | 파일·권한 |
| 4 | 새 코디네이터 기동: 기존 인자 + `--roles <roles.json>`, 새 `--tokens`. 같은 `IEO_DB_URL`. `migrate`가 기동 때 돈다(새 표 11·열 4·제약 2, 모두 추가) | 1분(마이그레이션 수 초) | `GET /v0/health` 200, `schema_version: ieo-coord-v0`, 새 표 11개, `evidence_kind_check`에 `branch_head`, 로그에 바인딩 오류 없음 |
| 5 | 역할 컨테이너 기동(§5.3) | 3분 | 두 서비스 `GET /v0/role/status`: `central_checkpoint.mode`가 `local_only` 아님, `capability_gaps`에 local_only 없음, `running: null`, `quota_group_id` |
| 6 | sprint 등록(`s8-registrant`, `POST /v0/sprints?mode=executable`, 키 `s8-reg-0001`, 본문 §1.2) | 1분 | 201, digest = §1.2 값 |
| 7 | **사용자의 의미 검토**(§2.2 2) | 사용자 | 201 `passed` |
| 8 | `assignment_heads` 읽기 → 개발·Guardrail 배정(`s8-assigner`) → `branch_head` 입력(`s8-ingest`, `{"evidence_id": "s8-gh-head-1", "repo_id": "ieo-poc-test", "kind": "branch_head", "sha": "2ab9d876…", "ref": "refs/heads/main", "source_ref": "github:HeaInSeo/ieo_poc_test@refs/heads/main (git ls-remote <시각>)"}`) | 2분 | 배정 201(한도 그대로), 연결 조회 200 |

재배포 시간 창(1~5): 약 15분. 6~8은 사용자 검토 시간에 따름. R1~G1(§6): 약 15분.

### 5.3 설정

- **코디네이터 `roles.json`**:
  ```json
  {"ieo-poc-test": {
    "development": {"url": "http://127.0.0.1:58221", "adapter_token_file": "<작업 디렉터리>/adapter.token",
                    "quota_group_id": "<§9 질문 2>", "image_digest": "<기반 이미지 digest>",
                    "profile_id": "s8-dev", "profile_revision": 1},
    "guardrail":   {"url": "http://127.0.0.1:58222", "adapter_token_file": "<작업 디렉터리>/adapter.token",
                    "image_digest": "<기반 이미지 digest>", "profile_id": "s8-gr", "profile_revision": 1},
    "budget_plan": {"budget": "02-05", "ledger_dir": "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205",
                    "first_observation": {"stream": "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205-obs/D6.jsonl", "observed_unix": 1794698043},
                    "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}}}
  ```
  몫(`share`)은 쓰지 않는다(승인값).
- **역할 컨테이너**([launcher/run-step8-roles.sh](launcher/run-step8-roles.sh), 예행 통과): `QUOTA_GROUP_ID=<§9 질문 2> SUP_DEV_TOKEN_FILE=…/s8-sup-dev.token SUP_GR_TOKEN_FILE=…/s8-sup-gr.token bash run-step8-roles.sh <M의 ieo-supervisor가 있는 디렉터리> /opt/rust/ieo_poc_runs/02-05-s8-<날짜>`(기본값: 실제 CLI `claude-2.1.322`, 자격 `/opt/rust/ieo_poc_secrets`, 네트워크 `slirp4netns:allow_host_loopback=true`, 코디네이터 `http://10.0.2.2:58090`, 대상 `ieo_poc_test@2ab9d876…`). 작업 디렉터리 `/opt/rust/ieo_poc_runs/02-05-s8-<날짜>`(새것, 저장소 밖), 새 볼륨, 기반 이미지 `golang@sha256:d9c68c2c…`(02-04와 같음), 바이너리 M의 `ieo-supervisor` 읽기 전용 마운트, CLI `claude-2.1.322` 읽기 전용, UID 20001/20002, `--cap-drop ALL`, `no-new-privileges`, `--stop-timeout 15`, 포트 127.0.0.1:58221/58222. 설정:
  `{"role": "<역할>", "repo": "ieo-poc-test", "state": "/state", "etc": "/etc/ieo/role", "cred": "/run/ieo/cred", "claude": "/opt/claude/claude", "budget": "02-05", "quota_group_id": "<§9 질문 2>", "checkpoint_central": {"url": "http://10.0.2.2:58090", "token_file": "/run/ieo/cred/coordinator_token", "transport": "http"}}`. 개발 `settings.json`의 허용은 `Read`·`Grep`·`Glob`만(02-04의 `Edit`·`Agent` 없음), `agents.json`은 빈 목록. 대상 저장소 사본을 개발 `/state/work/ieo-poc-test`와 Guardrail `/state/review/ieo-poc-test`에 두고 쓰기를 막는다(`chmod -R a-w`). GitHub 자격은 넣지 않는다.

### 5.4 되돌리기와 판단 지점

| 지점 | 실패 | 대응 |
| --- | --- | --- |
| 5.2-1 | 덤프 실패·목록 오류 | 멈춤. 아무것도 바뀌지 않음. 보고 |
| 5.2-2 | 778ee47이 30초 뒤에도 남음 | SIGKILL. 그래도 남으면 멈춤·보고(아직 아무것도 바뀌지 않음) |
| 5.2-4 | 기동 실패(바인딩 충돌, 마이그레이션 오류, 포트) | 로그 확인 → **되돌리기**: 새 프로세스 정지, §3 복원, `bin/ieo-coord.778ee47`을 옛 인자·옛 `tokens.json`으로 기동, `GET /v0/health` 200 확인, 보고 |
| 5.2-5 | 역할 서비스가 뜨지 않음, 중앙 연결 실패 | 컨테이너만 지우고(볼륨은 둠) 원인 보고. 코디네이터는 새 빌드로 둠(778ee47로 되돌릴지는 사용자 결정) |
| 5.2-6/7 | 등록·의미 검토 거절 | 멈춤·보고(명세 수정은 새 revision, 사용자 확인) |
| 5.2-8 | 배정·입력 거절 | 멈춤·보고 |

되돌리기는 위 5.2-4에서만 자동으로 한다(DB를 덤프 시점으로 되돌리는 것이 그 단계까지는 잃는 것이 없기 때문). 5.2-5 이후에는 새 기록(등록·배정)이 생기므로 되돌리지 않고 멈춰 보고한다.

## 6. 회차

### 6.1 순서

R1 → R2 → G1. 전달은 `s8-operator`로 `POST /v0/repos/ieo-poc-test/roles/<역할>/deliveries`, 새 UUID 세션, 키 §1.1, `--max-time 150`. 본문은 `{"assignment_id", "generation": 0, "attempt_id", "session_id", "prompt"}`뿐.

| 회차 | 기대 판단·연산 | 결과 |
| --- | --- | --- |
| R1 | `no_checkpoint` → `start` | `accepted` |
| R2 | `resume_allowed` → `reconnect`(R1 세션, `github` 일치) | `accepted` |
| G1 | Guardrail 첫 전달, 검토 대상 = 개발 `target_sha` → `start` | `accepted` |

### 6.2 회차마다 자동 확인 [읽기]

1. 전달 응답이 위 표와 같음.
2. 역할 `GET /v0/role/status`가 `running: null`이 되고 `last.attempt_id`가 이 회차.
3. 02-05 장부의 이 시도 기록: `stop_required: false`, `usage.run.state: measured`, 결과 `success`·`is_error: false`, 실행 신호 `overageStatus: rejected`·`isUsingOverage: false`, 환경에 `CLAUDE_CODE_OAUTH_TOKEN` 있고 `ANTHROPIC_*` 없음. `inflight.json` 없음.
4. 연결 조회(`GET /v0/coordinator/assignments/<aid>/generations/0/chain`): 전달 → takeover(그 전달에 연결) → 첫 행동(`recordFirstAction`, 그 시도) → after_run. R2는 R1과 같은 세션의 resume.
5. 역할 상태의 `safe_shutdown.pending_external_requests`가 비어 있음(체크포인트 안의 같은 이름 목록은 그 체크포인트를 쓸 때의 사본이다).
6. 누적 사용량이 예산·역할 몫·배정 한도 안.
7. 답 내용(사람이 볼 기록용): R1·R2의 세 줄이 파일과 맞는지, G1의 판정. 답이 틀려도 회차는 멈추지 않는다(사실 확인은 수용 기록에 적음). 다만 지시를 어긴 도구 사용(쓰기·명령)이 기록에 있으면 멈춤.

### 6.3 멈춤 조건(RUNBOOK §7·§9, 묶음 안에서도 곧 멈추고 보고)

미계측 회차, 상한 도달, 허용되지 않은 한도 신호, `stop_required`, 예상과 다른 판단(`not_called`, `external_reconciliation_required`, `stale_target`, `refused`, `unknown`, 예상 밖 `budget_refused`), 확인되지 않은 외부 효과(`pending_external_requests`), `inflight.json`이 남음, 장부와 연결 조회 불일치, 첫 행동 미연결(409), 비밀 출력, G1 실패(다시 하지 않음). 자동 재시도 없음.

### 6.4 예비(묶음 밖)

R1 또는 R2가 실패하면 멈추고 보고한다: 실패 기록(사용량 계측 여부, `stop_reason`), 체크포인트 단계, 세션 유무, RUNBOOK §6 항목 2의 전제 충족 여부, 원인 판단과 증거. 전제를 채우고 사용자가 그 건(원인 보고, `replaces`, 연산: R1이면 `new_session: true` 권함, R2이면 reconnect 기본)을 승인하면, **사용자 본인의 자격**으로 예비 전달을 보낸다(에이전트는 사람 자격을 쓰지 않는다). 전제를 채우지 못하면 예비는 불가능하고 사용자가 정한다.

## 7. 실패 대응

| 실패 | 대응 | 되돌리기 | 보고 |
| --- | --- | --- | --- |
| 사전 확인 하나라도 아님 | 시작하지 않음 | 없음(바뀐 것 없음) | 항목과 근거 |
| 백업 실패 | 멈춤 | 없음 | 오류 |
| 새 코디네이터 기동 실패 | §5.4 되돌리기 | DB 복원 + 778ee47 | 로그, 복원 확인 |
| 역할 컨테이너 실패 | 컨테이너 제거(볼륨 유지) | 코디네이터는 그대로 | 로그 |
| 등록·검토·배정 거절 | 멈춤 | 없음 | 응답 본문 |
| R1 `not_called`/`budget_refused` | 멈춤(모델 호출 없음) | 없음 | 판단 이유 |
| R1·R2 실패(결과 오류, 계측됨) | 멈춤, 예비 판단 자료 보고 | 없음 | §6.4 |
| 미계측 회차(가드 멈춤·답 잃음·오류 체크포인트) | 멈춤. 예산은 코드가 멈춤. 예비 불가 | 없음 | 미계측으로(0이 아님), 사람의 대조 필요 |
| overage 등 한도 신호 | 멈춤 | 없음 | 신호 원문 |
| G1 실패 | 멈춤, 다시 하지 않음 | 없음 | 기록 |
| 실행 중 코디네이터 재시작 필요 | 원칙상 하지 않음. 꼭 필요하면 RUNBOOK §8(새 빌드: SIGTERM 뒤 진행 중 회차 기다림, 호스트 프로세스이므로 `kill -TERM` 후 최대 900초, 그 뒤 SIGKILL) — 묶음 밖, 사용자 승인 | 없음 | 이유 |
| 비밀이 출력됨 | 멈춤, 그 자격 폐기·교체는 사용자 결정 | 없음 | 사실만(값 없이) |

## 8. 승인 요청(묶음)

**아래를 한 번에 승인해 주십시오.** 승인된 정상 절차 안에서는 단계마다 다시 묻지 않고, 멈춤 조건이면 멈추고 보고합니다.

**포함:**
1. 사전 확인(§5.1): 55433 읽기 전용 질의(새 표·sprint·바인딩·체크포인트), 02-04 장부·프로세스 확인(보기만), D6 관측 stream 확인과 복사.
2. 백업(§3): `ieo_poc-data/backups/<시각>-pre-step8/`에 `pg_dump -Fc`, 검증.
3. 재배포(§5.2 1~5): 778ee47 정지(`kill -TERM`, 30초 뒤 필요하면 SIGKILL), 바이너리 M으로 교체(778ee47 보존), 새 `tokens.json`·`roles.json`·장부·관측 디렉터리, 기동 시 55433 마이그레이션(추가만), 실패 시 §5.4 되돌리기(DB 복원 + 778ee47).
4. 자격·설정 파일(§2.1, §5.3): 에이전트가 주체 토큰을 만들고(값 출력 없음) digest를 `tokens.json`에, OAuth 자격 파일을 역할 볼륨으로 복사(값 출력 없음).
5. 역할 컨테이너 둘(§5.3): 새 작업 디렉터리·볼륨, 대상 저장소 읽기 전용 사본, `checkpoint_central` → 58090, 예산 02-05.
6. sprint `POC-S8-01` r1 등록(§1.2), 배정 둘, `branch_head` 입력(§5.2 6·8).
7. 실제 회차 **R1, R2, G1**(§1.3 지시문, §6), 각 회차 뒤 자동 확인.

**제외(별도 승인 필요):** 예비 회차(건별 승인, §6.4), 어떤 재시도·다시 하기, 실행 중 코디네이터 재시작, GitHub 쓰기(push·PR·댓글), 이 실행안 밖의 모든 것, 02-04·X-02 예산 사용, 추가 결제.

**사용자 또는 실행하는 사람이 실행 전에 확인할 것(읽기 전용, 값 출력 없음):**
1. **관측 마감: R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전.** 지나면 이 묶음으로는 R1을 보내지 않고 멈춘다(§9 질문 3).
2. D6 관측 stream의 경로와 판정(§5.1 항목 4의 명령).
3. `quota_group_id` 값(§9 질문 2).

**사용자 본인이 할 일:**
1. 본인 토큰 만들기와 digest 전달(§2.2 1) — 재배포 전.
2. 의미 검토 요청(§2.2 2) — 등록 뒤, 에이전트가 digest를 알려 주면.
3. (실패가 있으면) 예비 건별 판단.

## 9. 사용자 확인이 필요한 것

1. **repo_id `ieo-poc-test`와 sprint `POC-S8-01`·task `90-01`**을 이대로 쓸지(제안값).
2. **`quota_group_id` 값.** 02-04 실제 회차에서는 설정되지 않아 `unknown`이었다(02-04 수용 보고). 그래서 옛 값이 없다. 개발·Guardrail 자격(`/opt/rust/ieo_poc_secrets/<역할>.token`)이 **같은 구독 계정**이면 두 역할에 같은 값을, 다르면 서로 다른 값을 정해 주십시오(형식 1~128자 `[A-Za-z0-9._:-]`). 제안: 같은 계정이면 `claude-sub-shared`. 이 값은 비밀이 아니며 코디네이터 `roles.json`과 두 역할 설정에 그대로 들어간다.
3. **관측 마감.** R1은 **2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 보내야 한다. 그 전에 실행하지 못하면 새 무추가청구 관측을 어떻게 얻을지 사용자가 정한다(실제 stream만이 출처다).

**해결된 공백(실행안 v1의 §9).** 역할 실행 스크립트는 만들어 예행했다(§4). 호스트 연결은 `slirp4netns:allow_host_loopback=true` + `10.0.2.2`로 정했다(코디네이터는 loopback에만). 수용 항목 `02-05.accept` 재사용은 원장 담당이 받아들였다(§1.2).
