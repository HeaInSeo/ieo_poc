# 02-05 단계 8 실행안 · 한 번의 승인으로 하는 재배포와 R1→R2→G1

| 항목 | 기록 |
| --- | --- |
| 근거 | [사용자 결정 2026-11-17 단계 8 L01](../../../../User_Decisions_2026-11-17_step8-l01.md)(L01 값 승인, 실행안 요청, 묶음 승인), [단계 8 절차서](../step8-runbook-20261117/RUNBOOK.md), [예산 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md) §2.1, [예비 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md), [복구 결정 2026-11-16](../../../../User_Decisions_2026-11-16_recovery.md) |
| 기준 | main 05cf16e(PR #85 병합). 실행은 이 실행안을 담은 PR의 병합 SHA(아래 **M**)의 빌드로 한다 |
| 만든 방법 | 코드·문서와 원장 담당이 읽기 전용으로 확인한 사실(아래 “확인된 사실”)에서만 끌어냈다. 58090·55433·비밀·실행 기록은 보지 않았다. sprint 명세는 로컬 `ieo-spec check --executable`로 검사했고(§1.2), 실행에 쓰는 스크립트 전부([launcher/](launcher/): 역할 실행, 코디네이터 교체, 마이그레이션 예행, 회차 구동과 확인)를 시험 시스템에서만 처음부터 끝까지 예행했다: 778ee47 빌드의 “옛” 코디네이터와 고정 행 (로그인 셸처럼 내보낸 두 줄 셸 함수와 상대 경로 인자를 지닌 채) → 캡처와 백업·마이그레이션 예행 → 교체 → R1→R2→G1 → 멈춤 확인의 음성 시험 → 되돌리기(§4, 23개 확인 모두 통과) |
| 개정 | v5: 명세의 수용 프로파일 r7 → r6(라이브 58090에 r7이 없음, 사용자 결정 (가), §11). v4: PR #86 검토 a5f9402 반영(§10.2): 옛 프로세스의 환경을 NUL 경계·작업 디렉터리째 그대로 되살림(셸 함수 제외, 값은 어떤 명령줄에도 없음), 02-04에서 쓴 규칙 형식, 실제 바인딩·새 `tokens.json`으로 하는 예행, cgroup 확인, §5.2 순서. v3: PR #86 검토 3e4e3d6 반영(§10.1). 개발 역할의 도구 제한을 supervisor가 강제(코드 750e904), 자동 되돌리기에서 DB 복원 제거와 마이그레이션 예행, 옛 프로세스의 인자·환경 캡처와 정확한 정지·기동 명령, 실제 회차 구동 스크립트 |
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

`POC-S8-01` r1의 전체 명세는 별도 파일 [sprint-POC-S8-01-r1.json](sprint-POC-S8-01-r1.json)이다. 요점: `registration_state: ready`, `target {repo_id: ieo-poc-test, repository_url}`, 위 `limits`, `unresolved_bindings: []`, 작업 하나(`90-01`, 산출물 `90-01-answers` “README.md·SANDBOX.json·CI 워크플로의 사실에 대한 짧은 답(쓰기 없음)”, 수용 `poc-acceptance-v1.3.3` **r6** 항목 `02-05.accept`(§11: r7에서 바꿈)). 나머지 필드(`schedule`·`delegation` 등)는 manifest의 POC-01에서 가져왔고, `sources`에 이 단계의 근거 결정 둘(예산 2026-11-15, L01 값 2026-11-17, 파일 SHA-256)을 더했다(검토 3e4e3d6 L-7). 로컬 검사: `ieo-spec check sprint-POC-S8-01-r1.json --manifest docs/poc/Console_Coordinator_Sprints_v0.json --executable` → **201 `ACCEPTED_FOR_SEMANTIC_REVIEW`**, digest `0e4d471dc996dd9f7dfea269975bc5cf2491eaf85f6e6836e1526a1d961d0124`(§11; r7 판의 옛 digest `a8d97f12…7bb7`). 라이브 58090의 효과 manifest(manifest + `--profiles`)로도 201·같은 digest다(§11). 예행의 새 빌드 코디네이터는 r7 판을 그 digest(`a8d97f12…`)로 등록했다(예행 기록은 다시 만들지 않음, §11). 근거 결정 파일이 나중에 고쳐지면 그 SHA-256은 등록 때의 내용을 가리키는 기록으로 남는다(명세 검사는 파일과 대조하지 않는다). 수용 항목은 프로파일에 단계 8 전용 항목이 없어 `02-05.accept`를 쓴다: 단계 8은 02-05 수용의 일부이므로 원장 담당이 이 재사용을 받아들였다. 실행 시 확인: 58090이 쓰는 `--manifest`·`--profiles` 파일이 저장소의 manifest와 같은지(다르면 그 파일로 다시 검사).

### 1.3 지시문(짧게, 읽기·답하기만)

지시문은 [launcher/prompts/](launcher/prompts/)의 파일이고 구동 스크립트가 그대로 보낸다. 공통: 파일 수정·명령 실행·네트워크(모델 외) 없음, 자식 에이전트 없음(02-04의 자식 회차가 미계측이 된 적이 있음), 답은 몇 줄. 지시만이 아니라 도구 자체가 제한된다(§4: 개발 `Read`·`Grep`·`Glob`만, Guardrail은 고정 규칙). 02-04의 비슷한 회차는 회당 3~10초, 과금 토큰(input+output+cache_creation) 약 2,600~9,000이었다(공개 기록 `real-sessions-20261115/evidence/real/records.jsonl`). 그래서 600초·150,000 안에 넉넉하다. 가드 멈춤(시간·토큰·overage)은 미계측이 되고 예비로도 풀 수 없으므로 지시를 작게 둔다.

- **R1(개발, 새 세션 start, cwd `/state/work/ieo-poc-test`):**
  `Use only the Read tool. Read README.md and SANDBOX.json in the current directory. Reply in exactly three lines: 1) the first line of README.md, 2) the value of role_credentials in SANDBOX.json, 3) the names of the top-level keys of SANDBOX.json, comma-separated. Do not modify any file and do not run any command.`
- **R2(개발, 같은 세션 reconnect):**
  `Continue in the same session. Use only the Read tool. Read .github/workflows/bootstrap.yml. Reply in exactly three lines: 1) the workflow name, 2) its trigger events, comma-separated, 3) its job ids, comma-separated. Then add one line that repeats your answer 2) from the previous turn. Do not modify any file and do not run any command.`
  (마지막 줄은 재개가 R1 세션을 이었는지 답으로도 보이게 한다)
- **G1(Guardrail, start, cwd `/state/guardrail-cwd`, 허용 읽기 `/state/review/**`):**
  `Use only the Read tool. The files of the reviewed commit are in /state/review/ieo-poc-test. Below are numbered claims made by the development role. For each claim, read the file it is about and reply with one line: <number> CONFIRMED or <number> CONTRADICTED <file>. Do not write any file.` 뒤에 R1·R2의 답(각 회차 stream의 `result` 문자열)을 **줄마다 번호를 붙여**(빈 줄 제외, R1 다음 R2) 그대로 붙인다. 구동 스크립트가 자동으로 만들고(`check-run.py --g1-prompt`), 비었거나 2,048바이트를 넘으면 멈춘다(검토 3e4e3d6 L-6).

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

`/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/s8/`(0700)에, 주체마다 토큰과 요청 헤더 파일(둘 다 0600):
```
( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > s8-<name>.token
  printf 'Authorization: Bearer %s\n' "$(cat s8-<name>.token)" > s8-<name>.header )
printf '%s' "$(cat s8-<name>.token)" | sha256sum | cut -d' ' -f1   # 이 digest만 tokens.json에
```
요청은 `curl -H @s8-<name>.header …`로 보낸다(구동 스크립트 [launcher/lib-step8.sh](launcher/lib-step8.sh)). 값은 명령줄·화면·기록에 남지 않는다. 셸 함수 안의 `<(…)`는 curl이 읽기 전에 닫혀 401이 되는 것을 예행에서 확인해 헤더 파일로 바꿨다. agent 주체는 repo 하나에만 묶인다(코디네이터 규칙). supervisor 토큰은 역할 볼륨의 `cred/coordinator_token`(0400, 그 역할 UID)으로 복사한다. 역할의 OAuth 자격은 02-04와 같이 `/opt/rust/ieo_poc_secrets/<역할>.token`을 볼륨의 `cred/oauth_token`(0400)으로 **복사만** 하고 앞부분 형식(`sk-ant-oat`)만 확인한다(값 출력 없음). 어댑터 토큰도 같은 방식으로 새로 만든다. 새 `tokens.json`은 지금 58090이 쓰는 파일의 주체 전부(이름·종류·범위만 읽음) + §2의 주체다.

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

## 3. 백업과 마이그레이션 예행 [변경: 파일 쓰기와 시험 DB만, 영속 DB는 읽기]

DB 이름·사용자(`<db>`·`<user>`)는 §5.2 단계 1에서 캡처한 `IEO_DB_URL`의 `user`·`dbname` 필드에서 읽는다(암호는 출력하지 않음).

**백업**(§5.2 단계 2). 위치 `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/backups/<UTC 시각>-pre-step8/`(`/opt`, `/home` 아님):
```
B=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/backups/$(date -u +%Y%m%dT%H%M%SZ)-pre-step8; mkdir -p -m 700 "$B"
df -h /opt
podman exec ieo-poc-db psql -U <user> -d <db> -Atc "SELECT pg_size_pretty(pg_database_size('<db>'))"
podman exec ieo-poc-db pg_dump -U <user> -d <db> -Fc > "$B/ieo.dump"
sha256sum "$B/ieo.dump" > "$B/ieo.dump.sha256"
podman exec -i ieo-poc-db pg_restore --list < "$B/ieo.dump" > "$B/ieo.dump.list"
podman exec ieo-poc-db psql -U <user> -d <db> -At -c "SELECT relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE relkind='r' AND nspname='public' ORDER BY 1" \
  | while read -r t; do echo "$t=$(podman exec ieo-poc-db psql -U <user> -d <db> -Atc "SELECT count(*) FROM \"$t\"")"; done > "$B/live-counts.txt"
```

**마이그레이션 예행**(§5.2 단계 4, 검토 3e4e3d6 M-2). 라이브를 바꾸기 전에 그 덤프와 새 빌드 M으로 시험 DB(55432 컨테이너)에서만:
```
TEST_DB_PASSWORD_FILE=<시험 DB 암호 파일> TOKENS_FILE=<새 tokens.json> BINDINGS_FILE=<캡처한 --bindings 파일(절대 경로)> \
  launcher/rehearse-migration.sh "$B/ieo.dump" <M의 ieo-coord> 58291 "$B/live-counts.txt"
```
덤프를 새 시험 데이터베이스에 `pg_restore --exit-on-error`로 복원(백업이 실제로 복원됨을 확인) → 모든 표의 행 수가 `live-counts.txt`(백업 때 라이브 행 수)와 같음(스크립트가 비교) → M의 `ieo-coord migrate` → 행 수 다시(있던 표는 모두 같아야 함) → 새 표 11개와 `evidence.kind`의 `branch_head` → M의 `serve`가 그 복사본으로, **새 `tokens.json`과 실제 바인딩 파일로** 상태 조회 200(병합한 주체와 `check_bindings`를 라이브 기동 전에 확인), SIGTERM에 0으로 끝남 → 시험 데이터베이스 삭제(검토 a5f9402 L-9). 하나라도 아니면 멈춘다(아직 라이브는 바뀌지 않음).

**복원은 묶음 밖이다.** 마이그레이션은 한 문(`batch_execute(schema.sql)`)이라 PostgreSQL이 하나의 암묵 트랜잭션으로 돌린다: 실패하면 DB는 그대로다. 그리고 바뀐 것이 모두 추가라 778ee47은 마이그레이션된 DB에서도 돈다(예행에서 확인, §4). 그래서 자동 되돌리기는 DB를 건드리지 않는다(§5.4). 덤프에서 되살리는 것이 필요하면 **사용자의 별도 결정**으로, 지금 DB를 지우지 않고 **새 이름**의 DB에 복원한 뒤 바꾼다:
```
podman exec ieo-poc-db createdb -U <user> <db>_restore_<시각>
podman exec -i ieo-poc-db pg_restore -U <user> -d <db>_restore_<시각> --exit-on-error < "$B/ieo.dump"
# 확인 뒤 코디네이터의 IEO_DB_URL dbname만 그 이름으로 바꿔 기동(옛 DB는 그대로 둠)
```

## 4. 준비와 예행(승인 전, 실 시스템 변경 없음)

**스크립트**([launcher/](launcher/), 02-04 `run-real.sh`와 같은 자리):

| 파일 | 하는 일 |
| --- | --- |
| [run-step8-roles.sh](launcher/run-step8-roles.sh) | 역할 컨테이너 둘. `checkpoint_central` → 코디네이터(supervisor `agent` 토큰), 예산 `02-05`(몫 없음), `QUOTA_GROUP_ID` 필수(두 역할 항목 모두), 대상 커밋 파일만(`git archive <SHA>`, `.git`·자격 없음)을 개발 `/state/work/ieo-poc-test`·Guardrail `/state/review/ieo-poc-test`에 **`:ro` 바인드 마운트**, 개발 역할은 supervisor의 읽기 전용 모드(`"tools": "read_only"`)와 설정의 `deny` 목록, 새 작업 디렉터리·볼륨만, 포트 58221/58222 |
| [coord-switch.sh](launcher/coord-switch.sh) | 코디네이터 호스트 프로세스: `capture`(pid가 그 바이너리이고 `--listen 127.0.0.1:<port>`인지, 부모와 **cgroup**(systemd `*.service` 단위면 멈춤), 연결된 클라이언트 수, 경로 인자 `--tokens`·`--manifest`·`--profiles`·`--bindings`·`--policy`·`--roles`가 그 작업 디렉터리에서 있는지, `/proc`의 인자·환경을 **NUL 경계 그대로** `argv0`·`env0`로(소유자 읽기 전용)와 작업 디렉터리 `cwd`; 이름과 경로만 출력), `stop`(SIGTERM, 기다림, 시간이 넘으면 SIGKILL), `start`(지금 환경을 비우고 캡처한 환경을 셸 내장 `export`로만 되살림: **값이 어떤 프로세스의 명령줄에도 나오지 않음**, 내보낸 셸 함수 `BASH_FUNC_*`와 식별자가 아닌 이름은 버림; 캡처한 작업 디렉터리에서 `setsid nohup`, 인자는 캡처 그대로에 `--tokens` 바꿈·`--roles` 더함, pid 파일, 로그, 상태 조회 200까지) |
| [rehearse-migration.sh](launcher/rehearse-migration.sh) | §3의 마이그레이션 예행(시험 DB만) |
| [run-step8.sh](launcher/run-step8.sh) + [lib-step8.sh](launcher/lib-step8.sh) + [check-run.py](launcher/check-run.py) | 실제 회차 구동: `register` → (사용자 의미 검토) → `prepare`(검토 통과, 두 역할 도달·중앙 모드·유휴·quota 불일치 없음, 배정 둘, `branch_head`) → `runs`(관측 마감 → R1 → R2 → G1, 회차마다 §6.2의 모든 확인; 하나라도 어긋나면 `STOP`으로 끝나고 아무것도 더 보내지 않음) |
| [dry-run-step8.sh](launcher/dry-run-step8.sh) | 위 전부를 시험 시스템에서 예행 |

**개발 역할의 도구 제한(검토 3e4e3d6 M-1).** 02-04의 실제 CLI는 도구 옵션이 없으면 `Task`·`Bash`·`Write`·`WebFetch`·`Workflow`·`RemoteTrigger`·`SendMessage` 등을 내놓았다(02-04 D1 `init.tools`). 그래서 supervisor에 **읽기 전용 개발 모드**를 더했다(코드 750e904·d0ad250, 역할 설정 `"tools": "read_only"`): `--tools Read,Grep,Glob`(CLI가 다른 도구를 아예 내놓지 않음), `--allowedTools Read(//state/work/<repo>/**) Grep Glob`(02-04가 이 CLI에 준 규칙 형식: 범위 지정 `Read(//…)`와 그냥 `Grep`·`Glob`, 검토 a5f9402 L-8), `--disallowedTools Read(//run/ieo/**)·Read(//etc/ieo/**)·Read(//state/claude/**)·Read(//state/home/**)·Read(//proc/**)·Bash`, `--disable-slash-commands`. Guardrail이 02-04부터 쓰는 방식(`--tools`로 제한, G1의 `init.tools`가 정확히 그 목록이었음)과 같다. 옵션이 없는 개발 역할의 인자는 바뀌지 않는다(시험으로 고정). 설정의 `allow`도 같은 형식이고 `deny` 목록(위 도구 이름 전부와 같은 경로들)은 둘째 겹이다. 회차 뒤 `check-run.py`가 stream의 `init.tools`가 허용 집합 안이고(개발 `Read`·`Grep`·`Glob`, Guardrail에 `Write`·`Edit` 더함) 모든 `tool_use`가 `Read`·`Grep`·`Glob`인지 보고, 아니면 멈춘다. **실제 CLI에서 개발 역할의 `--tools`는 아직 관측되지 않았다**(가짜 `claude`는 받은 `--tools`를 그대로 내놓도록 해 supervisor가 옵션을 넘기는 것까지만 예행이 확인한다). R1의 `init.tools`가 첫 실제 관측이고, 다르면 R1 뒤에 멈춘다.

**네트워크 노출(검토 3e4e3d6 L-1).** 컨테이너 네트워크 `slirp4netns:allow_host_loopback=true`는 컨테이너에서 `10.0.2.2`로 **호스트 loopback의 모든 포트**에 닿게 한다(58090만이 아니라 55433·55432·58221/58222와 다른 로컬 서비스도; 검토자가 컨테이너에서 55432 연결을 확인함). 02-04의 그냥 `slirp4netns`보다 넓다. 이것이 필요한 것은 supervisor가 58090에 보고하기 때문이다. 모델 쪽 도구는 연결을 열 수 없고(셸·웹 도구 없음, 위), DB는 컨테이너에 없는 암호가 있어야 한다. 코디네이터 자신은 계속 `127.0.0.1`에만 열린다.

**읽기 전용 대상(L-3).** 대상 파일은 역할 볼륨 밖에 풀고 `:ro`로 마운트한다(역할 UID가 볼륨을 소유하므로 파일 권한만으로는 막지 못함). 예행의 컨테이너는 이 마운트로 떴다. 실제 CLI를 쓰기 불가 cwd에서 돌린 적은 없다: 시작이 실패하면 계측된 결과 실패라 예비 경로다(§6.4).

**예행(완료)**: `dry-run-step8.sh <M 바이너리 디렉터리> <778ee47 ieo-coord> <scratch>`, [evidence/dry-run.log](evidence/dry-run.log)(최종 스크립트 커밋에서), **23개 확인 모두 통과**. 시험 PostgreSQL(55432)의 새 데이터베이스(끝나면 지움)에서:
1. 778ee47 빌드 코디네이터를 호스트 프로세스로(`setsid`, pid 파일) 띄운다. 라이브처럼 따로 둔 작업 디렉터리에서 **상대 경로** `--tokens tokens-old.json --bindings bindings.json`, 환경에 **내보낸 두 줄 셸 함수**(`BASH_FUNC_…%%`, 이 호스트의 로그인 셸이 넣는 `which` 함수와 같은 모양; 검토 a5f9402 N-1). 그 API로 고정 행(draft sprint).
2. `coord-switch.sh capture`: cgroup(세션 scope), 상대 경로 둘을 작업 디렉터리에서 확인, 인자 14개·환경 89개(셸 함수 1개, 값 출력 없음), `env0`가 NUL 경계 그대로(DB URL과 두 줄 함수가 온전함), 소유자 읽기 전용.
3. 백업(`pg_dump -Fc`, `--list`, 라이브 행 수) → **마이그레이션 예행 통과**(복원 행 수 = 라이브 행 수, 15개 표 그대로, 새 표 11개, `branch_head`, M이 새 `tokens.json`·캡처한 바인딩으로 서비스, SIGTERM에 0).
4. 역할 컨테이너(실제 스크립트, 가짜 `claude`·자격; 가짜는 받은 `--tools`를 그대로 내놓음).
5. `stop` → `start`(M, 새 `tokens.json`, `--roles`): 새 프로세스의 환경에 DB URL이 있고 셸 함수가 없으며, 작업 디렉터리는 캡처한 곳, 바이너리는 M.
6. `run-step8.sh register`(digest `a8d97f12…`, r7 판; 지금 명세는 §11의 `0e4d471d…`) → 사람 주체의 의미 검토 → `prepare` → `runs`: R1 `no_checkpoint`→`start`, R2 `resume_allowed`→`reconnect`(R1 세션), G1 `start`, 회차마다 모든 확인 통과(개발 `init.tools` = `Glob, Grep, Read`, Guardrail = `Edit, Glob, Grep, Read, Write`, 사용 `Read`).
7. 멈춤 확인의 음성 시험: 고친 사본에서 `check-run.py`가 overage 신호와 `Bash` 사용에 각각 멈춤.
8. 되돌리기: M을 `stop`(정상 종료) → 캡처한 인자·환경·작업 디렉터리로 778ee47 `start`(**상대 경로 인자, 셸 함수가 있던 환경**)가 마이그레이션된 DB에서 상태 조회 200, 단계 8 sprint를 읽음(200) → `stop`.
58090·55433·비밀·실행 기록·실제 `claude`는 쓰지 않았다. 컨테이너·시험 DB·scratch는 지웠다.

**실행 때 빌드.** 이 실행안의 PR 병합 뒤 그 SHA(**M**)의 깨끗한 작업 트리에서 `cargo build --release -p ieo-coord -p ieo-supervisor`(대상 디렉터리는 `/home` 밖). 바이너리 SHA-256 기록.

## 5. 재배포

이하 경로: `D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data`, `LAUNCH=<M 작업 트리>/docs/poc/reports/tasks/02-05/step8-plan-20261117/launcher`.

### 5.1 사전 확인 [읽기], 약 10분

1. 빌드 M이 main CI 성공. 바이너리가 M에서 만들어짐.
2. 진행 중 회차 없음: 02-04 장부(`/opt/rust/ieo_poc_runs/02-04-*/ledger`)에 `inflight.json` 없음, 그 `caller.lock`을 연 프로세스 없음(`fuser`/`lsof`, 보기만), `ieo-role-call` 프로세스 없음(`ps`).
3. **58090의 다른 클라이언트**(검토 3e4e3d6 L-4): `ss -Htnp state established '( sport = :58090 )'`이 비어 있음. 아니면 기다리거나 보고.
4. 55433 읽기 전용 질의(RUNBOOK §1 항목 3~5): 새 표 11개가 없음, `POC-S8-01`이 `sprint_revisions`·`notion_imports.test_id`에 없음, 비공개 바인딩의 `sprint_id`가 다른 repo와 겹치지 않음. `role_checkpoints` 질의는 표가 없어 오류 = 정상.
5. 첫 관측: 가장 새 실제 stream은 02-04 D6(종료 **2026-11-14T23:14:03Z**, `ended_unix` 1794698043). 7일 한도라 **R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 받아들여져야 한다(구동 스크립트가 10분 여유를 두고 확인). 읽기 전용으로(stream에는 자격이 없다. 내용은 출력하지 않고 판정만):
   ```
   ls -l /opt/rust/ieo_poc_runs/*/ledger/streams/D6.jsonl
   python3 -I -c 'import json,sys; ev=[json.loads(l)["rate_limit_info"] for l in open(sys.argv[1]) if "\"rate_limit_event\"" in l]; print(len(ev), all(e.get("status")=="allowed" and e.get("overageStatus")=="rejected" and e.get("isUsingOverage") is False for e in ev))' <그 경로>
   ```
   결과가 `<1 이상> True`이면 그 파일을 `$D/ledger-0205-obs/D6.jsonl`로 복사한다. 경로가 하나가 아니거나 `False`면 멈춤.
6. 디스크: `/opt` 여유 ≥ 3G, `/home` 쓰기 없음.

### 5.2 순서와 시간 [변경]

| # | 단계(명령) | 예상 | 확인 [읽기] |
| --- | --- | --- | --- |
| 1 | **캡처**[읽기]: `$LAUNCH/coord-switch.sh capture $D/coord.pid $D/bin/ieo-coord 58090 $D/private/s8/coord-778ee47` | 수 초 | pid가 778ee47 `ieo-coord`이고 `--listen 127.0.0.1:58090`; 부모와 cgroup 출력, systemd `*.service` 단위면 멈춤(그 단위로 멈춰야 하므로 보고); 클라이언트 0; 경로 인자가 모두 작업 디렉터리에서 있음; `argv0`·`env0`·`cwd`(값 출력 없음). DB 이름·사용자는 여기서 읽음(§3) |
| 2 | 백업(§3) | 2~3분 | 덤프 목록, `live-counts.txt` |
| 3 | 자격 파일과 새 `tokens.json`(§2.1, **새 경로**: 옛 파일은 되돌리기에 쓰므로 그대로 둠) | 3분 | 권한, digest만 |
| 4 | 마이그레이션 예행(§3, 시험 DB만; 새 `tokens.json`과 단계 1이 출력한 `--bindings` 파일, `live-counts.txt` 비교) | 1~2분 | `## rehearsal: PASS` |
| 5 | 역할 컨테이너: `QUOTA_GROUP_ID=<§9 Q2> SUP_DEV_TOKEN_FILE=$D/private/s8/s8-sup-dev.token SUP_GR_TOKEN_FILE=$D/private/s8/s8-sup-gr.token $LAUNCH/run-step8-roles.sh <M의 ieo-supervisor 디렉터리> /opt/rust/ieo_poc_runs/02-05-s8-<날짜>`(코디네이터는 아직 옛 것; 역할은 회차가 올 때까지 보고하지 않음) | 3분 | 두 서비스 기동, `roles-entry.json` |
| 6 | `roles.json`(§5.3, 단계 5의 `roles-entry.json` + `budget_plan`), `$D/ledger-0205/`(0700), 관측 사본, M 바이너리를 `$D/bin/ieo-coord-M`에(옛 `$D/bin/ieo-coord`는 그대로) | 2분 | 파일·권한 |
| 7 | **정지**: `$LAUNCH/coord-switch.sh stop $D/coord.pid 30`. 778ee47은 신호 처리기가 없는 호스트 프로세스라 SIGTERM의 기본 동작으로 곧 끝난다 | 1분 | 프로세스 없음, 58090 닫힘 |
| 8 | **기동**: `$LAUNCH/coord-switch.sh start $D/bin/ieo-coord-M $D/private/s8/coord-778ee47 $D/coord.pid $D/logs/coord-M.log --tokens <새 tokens.json> -- --roles <roles.json>`(캡처한 환경·작업 디렉터리·인자 + `--roles`, `setsid nohup`, pid 파일 갱신). `migrate`가 기동 때 돈다 | 1분 | 상태 조회 200(스크립트가 기다림), 새 표 11개, 로그에 바인딩 오류 없음 |
| 9 | `COORD=http://127.0.0.1:58090 TOKDIR=$D/private/s8 OUT=<기록 디렉터리> LEDGER=$D/ledger-0205 ADAPTER_HEADER=<작업 디렉터리>/adapter.header $LAUNCH/run-step8.sh register` | 1분 | 201, digest = §1.2 |
| 10 | **사용자의 의미 검토**(§2.2 2) | 사용자 | 201 `passed` |
| 11 | `assignment_heads` 읽기 → `EXPECTED_CURRENT_DEV/GR` → 같은 환경으로 `run-step8.sh prepare` | 2분 | 검토 통과, 역할 도달·중앙·유휴·quota 일치, 배정 둘, `branch_head` |
| 12 | `OBS_UNIX=1794698043 … run-step8.sh runs` | 약 15분 | §6 |

라이브가 멈추는 시간(7~8): 약 2분. 1~6은 라이브를 바꾸지 않는다(5는 새 컨테이너만). 9~11은 사용자 검토 시간에 따름(검토 a5f9402 L-11: 역할 컨테이너가 `roles.json`과 기동보다 먼저).

### 5.3 설정

- **코디네이터 `roles.json`**: 단계 5가 쓴 `<작업 디렉터리>/roles-entry.json`(두 역할 모두 `quota_group_id`, 검토 3e4e3d6 L-2)에 `budget_plan`을 더한 것:
  ```json
  {"ieo-poc-test": {<roles-entry.json의 development·guardrail>,
    "budget_plan": {"budget": "02-05", "ledger_dir": "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205",
                    "first_observation": {"stream": "/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/ledger-0205-obs/D6.jsonl", "observed_unix": 1794698043},
                    "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}}}
  ```
  몫(`share`)은 쓰지 않는다(승인값).
- **역할 컨테이너**: 기본값 실제 CLI `claude-2.1.322`, 자격 `/opt/rust/ieo_poc_secrets`, 네트워크 `slirp4netns:allow_host_loopback=true`(§4의 노출), 코디네이터 `http://10.0.2.2:58090`, 대상 `ieo_poc_test@2ab9d876…`(`:ro`), 기반 이미지 `golang@sha256:d9c68c2c…`, UID 20001/20002, `--cap-drop ALL`, `no-new-privileges`, `--stop-timeout 15`. GitHub 자격은 넣지 않는다. 대상 저장소는 호스트에서 공개 `git clone`으로 읽는다(쓰기 없음).

### 5.4 되돌리기와 판단 지점

| 지점 | 실패 | 대응 |
| --- | --- | --- |
| 1 | pid가 그 바이너리가 아님, systemd 단위, 클라이언트 있음, 경로 인자 없음 | 멈춤. 보고 |
| 2·4 | 덤프·예행 실패(행 수 불일치 포함) | 멈춤. 라이브는 바뀌지 않음. 보고 |
| 5 | 역할 컨테이너 실패 | 컨테이너만 지우고(볼륨은 둠) 보고. 코디네이터는 옛 것 그대로 |
| 7 | 30초 뒤에도 남음 | SIGKILL(스크립트). 그래도 남으면 멈춤·보고 |
| 8 | 기동 실패(바인딩 충돌, 마이그레이션 오류, 포트) | **되돌리기(자동, DB는 그대로)**: `coord-switch.sh stop $D/coord.pid 30`(M이 떠 있으면) → `coord-switch.sh start $D/bin/ieo-coord $D/private/s8/coord-778ee47 $D/coord.pid $D/logs/coord-778ee47.log`(옛 바이너리, 캡처한 인자·환경·작업 디렉터리 그대로, 옛 `tokens.json`) → 상태 조회 200. 마이그레이션이 실패했다면 DB는 바뀌지 않았고, 성공했다면 778ee47이 추가된 표를 무시하고 돈다(예행 확인). 로그와 함께 보고. **DB 복원은 하지 않는다**(§3, 별도 결정) |
| 9~11 | 등록·의미 검토·배정 거절 | 멈춤·보고 |

8 이후에는 새 기록(등록·배정·회차)이 생기므로 자동으로 되돌리지 않고 멈춰 보고한다.

## 6. 회차

### 6.1 순서

`run-step8.sh runs`가 R1 → R2 → G1을 보낸다. 전달은 `s8-operator`로 `POST /v0/repos/ieo-poc-test/roles/<역할>/deliveries`, 회차마다 새 소문자 UUID 세션(정규식 확인), 키 §1.1, `--max-time 150`. 본문은 `{"assignment_id", "generation": 0, "attempt_id", "session_id", "prompt"}`뿐이고 한 번 쓰면 바꾸지 않는다.

| 회차 | 기대 판단·연산 | 결과 |
| --- | --- | --- |
| R1 | `no_checkpoint` → `start` | `accepted` |
| R2 | `resume_allowed` → `reconnect`(R1 세션, `github` 일치) | `accepted` |
| G1 | Guardrail 첫 전달, 검토 대상 = 개발 `target_sha` → `start` | `accepted` |

**같은 키로 다시 보내기는 재시도가 아니다**(검토 3e4e3d6 I-1). 답을 받지 못한 요청(연결 실패)만 같은 키·같은 본문 바이트로 다시 보내고(최대 3번), 그것은 저장된 답을 읽을 뿐 새 회차를 시작하지 않는다. 새 키로 다시 보내는 일은 없다. 멈춘 뒤 `runs`를 다시 돌리면 이미 기록된 회차는 같은 키로 저장된 답만 읽고 기다리지 않는다(검토 a5f9402 I-3); 이어서 할지는 멈춤 보고 뒤 사용자 결정이다.

### 6.2 회차마다 자동 확인 [읽기], `check-run.py`, 하나라도 어긋나면 `STOP`

1. 전달 응답이 위 표와 같음.
2. 역할 `GET /v0/role/status`가 `running: null`이 되고 `last.attempt_id`가 이 회차.
3. 02-05 장부의 이 시도 기록: `budget: 02-05`, `stop_required: false`, `usage.run.state: measured`, 결과 `success`·`is_error: false`. `inflight.json` 없음.
4. 환경 이름에 `CLAUDE_CODE_OAUTH_TOKEN` 있고 `ANTHROPIC_*` 없음.
5. 이 회차 stream의 `rate_limit_event`가 하나 이상이고 모두 `allowed`·`rejected`·`isUsingOverage: false`.
6. **도구**: stream의 `init.tools`가 허용 집합 안(개발 `Read`·`Grep`·`Glob`, Guardrail `Read`·`Grep`·`Glob`·`Write`·`Edit`), 모든 `tool_use` 이름이 `Read`·`Grep`·`Glob`(검토 3e4e3d6 M-1).
7. R2는 R1과 같은 세션의 resume.
8. 누적(02-05 장부의 계측된 모델 회차 전부): 예산(4회·2,400초·200,000·2,000,000), 역할 몫(개발 3회·1,800초·150,000·1,500,000 / Guardrail 1회·600초·50,000·500,000), 배정 한도(개발 1,800초·150,000 / Guardrail 600초·50,000) 안. 미계측 기록이 하나라도 있으면 멈춤(0으로 세지 않음).
9. 역할 상태의 `safe_shutdown.pending_external_requests`가 빈 목록.
10. 코디네이터 `GET /v0/repos/ieo-poc-test/roles`의 `quota_groups.mismatches`가 빈 목록(검토 3e4e3d6 L-2).
11. 연결 조회: 이 시도의 전달이 takeover됐고 첫 행동이 있음.
12. (사람이 볼 기록용) R1·R2·G1의 답 내용. 답이 틀려도 회차는 멈추지 않고 수용 기록에 적는다.

### 6.3 멈춤 조건(RUNBOOK §7·§9, 묶음 안에서도 곧 멈추고 보고)

§6.2의 어느 하나, 미계측 회차, 상한 도달, 허용되지 않은 한도 신호, `stop_required`, 예상과 다른 판단(`not_called`, `external_reconciliation_required`, `stale_target`, `refused`, `unknown`, 예상 밖 `budget_refused`), `inflight.json`이 남음, 첫 행동 미연결(409), quota 불일치, 허용 밖 도구, 관측 마감 지남, G1 지시문이 비었거나 2 KB를 넘음, 비밀 출력, G1 실패(다시 하지 않음). 자동 재시도 없음.

### 6.4 예비(묶음 밖)

R1 또는 R2가 실패하면 멈추고 보고한다: 실패 기록(사용량 계측 여부, `stop_reason`), 체크포인트 단계, 세션 유무, RUNBOOK §6 항목 2의 전제 충족 여부, 원인 판단과 증거. 전제를 채우고 사용자가 그 건(원인 보고, `replaces`, 연산: R1이면 `new_session: true` 권함, R2이면 reconnect 기본)을 승인하면, **사용자 본인의 자격**으로 예비 전달을 보낸다(에이전트는 사람 자격을 쓰지 않는다). 전제를 채우지 못하면 예비는 불가능하고 사용자가 정한다.

## 7. 실패 대응

| 실패 | 대응 | 되돌리기 | 보고 |
| --- | --- | --- | --- |
| 사전 확인 하나라도 아님 | 시작하지 않음 | 없음(바뀐 것 없음) | 항목과 근거 |
| 백업·예행 실패 | 멈춤 | 없음 | 오류 |
| 캡처 확인 실패(pid·재시작기·클라이언트) | 멈춤 | 없음 | 출력 |
| 새 코디네이터 기동 실패 | §5.4 되돌리기(778ee47을 캡처한 인자·환경·작업 디렉터리로, DB 그대로) | 자동, DB 복원 없음 | 로그, 상태 조회 |
| 역할 컨테이너 실패 | 컨테이너 제거(볼륨 유지) | 코디네이터는 그대로 | 로그 |
| 등록·검토·배정 거절 | 멈춤 | 없음 | 응답 본문 |
| R1 `not_called`/`budget_refused` | 멈춤(모델 호출 없음) | 없음 | 판단 이유 |
| R1·R2 실패(결과 오류, 계측됨) | 멈춤, 예비 판단 자료 보고 | 없음 | §6.4 |
| 미계측 회차(가드 멈춤·답 잃음·오류 체크포인트) | 멈춤. 예산은 코드가 멈춤. 예비 불가 | 없음 | 미계측으로(0이 아님), 사람의 대조 필요 |
| 허용 밖 도구가 제공·사용됨 | 멈춤 | 없음 | `init.tools`·`tool_use` |
| overage 등 한도 신호, quota 불일치 | 멈춤 | 없음 | 신호·응답 원문 |
| G1 실패 | 멈춤, 다시 하지 않음 | 없음 | 기록 |
| 회차 중 코디네이터가 죽음 | 그 회차의 기록을 쓰던 스레드가 사라져 `inflight.json`이 남고 회차는 **사람이 대조할 때까지 미계측**이다. 코드가 다음 호출을 막는다. 멈춤 | 없음 | 사실과 로그(검토 3e4e3d6 I-2) |
| 실행 중 코디네이터 재시작 필요 | 원칙상 하지 않음. 꼭 필요하면 RUNBOOK §8(`coord-switch.sh stop … 900`: 진행 중 회차를 기다림) — 묶음 밖, 사용자 승인 | 없음 | 이유 |
| 비밀이 출력됨 | 멈춤, 그 자격 폐기·교체는 사용자 결정 | 없음 | 사실만(값 없이) |

## 8. 승인 요청(묶음)

**아래를 한 번에 승인해 주십시오.** 승인된 정상 절차 안에서는 단계마다 다시 묻지 않고, 멈춤 조건이면 멈추고 보고합니다.

**포함:**
1. **사전 확인**(§5.1, 읽기 전용): 55433 질의(새 표·sprint·바인딩·체크포인트), 02-04 장부·프로세스 확인, 58090의 다른 클라이언트 확인, D6 관측 stream 판정과 복사.
2. **캡처**(§5.2 1): pid·바이너리·리슨 주소·cgroup·클라이언트·경로 인자 확인, **그 프로세스의 인자·환경(DB 암호 포함)·작업 디렉터리를 출력하지 않고 소유자 전용 파일로 읽어 둠**.
3. **백업과 예행**(§3, §5.2 2·4): `$D/backups/<시각>-pre-step8/`에 `pg_dump -Fc`와 라이브 행 수, 시험 DB(55432)에서 복원·행 수 비교·M 마이그레이션·새 `tokens.json`과 실제 바인딩으로 서비스 예행(끝나면 지움).
4. **옛 코디네이터 정지와 새 빌드 기동**(§5.2 7·8): SIGTERM(30초 뒤 필요하면 SIGKILL), 옛 바이너리·`tokens.json`은 그대로 두고 M을 `$D/bin/ieo-coord-M`에서 캡처한 환경(셸 내장으로만 되살림, 값은 명령줄에 없음)·작업 디렉터리·인자 + 새 `tokens.json`·`--roles`로 `setsid nohup` 기동(로그 `$D/logs/coord-M.log`, `coord.pid` 갱신), 기동 때 55433 마이그레이션(추가만, 한 트랜잭션). 실패하면 자동 되돌리기 = 778ee47을 캡처한 그대로 **DB 그대로** 다시 기동.
5. **자격·설정 파일**(§2.1, §5.2 3·6): 에이전트가 주체 토큰·헤더를 만들고(값 출력 없음) digest를 `tokens.json`에, OAuth 자격을 역할 볼륨으로 복사(값 출력 없음).
6. **역할 컨테이너 둘**(§5.2 5, §5.3): 새 작업 디렉터리·볼륨, 대상 저장소의 공개 `git clone`(읽기)과 `:ro` 사본, 개발 역할 읽기 전용 도구, `checkpoint_central` → 58090, 예산 02-05, **네트워크 `allow_host_loopback`(컨테이너에서 호스트 loopback 전체에 닿음, §4)**.
7. **등록·배정·입력**(§5.2 9·11): sprint `POC-S8-01` r1 등록(§1.2), 배정 둘, `branch_head` 입력.
8. **실제 회차 R1, R2, G1**(§1.3, §6): `run-step8.sh runs`, 회차마다 §6.2 자동 확인, 어긋나면 멈춤.

**제외(별도 승인 필요):** 예비 회차(건별, §6.4), 어떤 재시도·다시 하기, **DB 복원**(새 이름으로, §3), 실행 중 코디네이터 재시작(RUNBOOK §8), GitHub 쓰기(push·PR·댓글), 이 실행안 밖의 모든 것, 02-04·X-02 예산 사용, 추가 결제(USD 0).

**사용자 또는 실행하는 사람이 실행 전에 확인할 것(읽기 전용, 값 출력 없음):**
1. **관측 마감: R1은 2026-11-21T23:14Z(한국 2026-11-22 08:14) 전.** 지나면 이 묶음으로는 R1을 보내지 않고 멈춘다(§9 Q3).
2. D6 관측 stream의 경로와 판정(§5.1 항목 5의 명령).
3. `quota_group_id` 값과 두 자격의 계정(§9 Q2).

**사용자 본인이 할 일:**
1. 본인 토큰 만들기와 digest 전달(§2.2 1) — 재배포 전.
2. 의미 검토 요청(§2.2 2) — 등록 뒤, 에이전트가 digest를 알려 주면.
3. (실패가 있으면) 예비 건별 판단.

## 9. 사용자 확인이 필요한 것

1. **repo_id `ieo-poc-test`와 sprint `POC-S8-01`·task `90-01`**을 이대로 쓸지(제안값).
2. **`quota_group_id` 값과 두 자격의 계정.** 02-04 실제 회차에서는 설정되지 않아 `unknown`이었다(02-04 수용 보고). 그래서 옛 값이 없다. 개발·Guardrail 자격(`/opt/rust/ieo_poc_secrets/<역할>.token`)이 **같은 구독 계정**이면 두 역할에 같은 값을, 다르면 서로 다른 값을 정해 주십시오(형식 1~128자 `[A-Za-z0-9._:-]`). 제안: 같은 계정이면 `claude-sub-shared`. 이 값은 비밀이 아니다. **다른 계정이면** G1의 무추가청구 근거가 문제다: G1을 받아들이는 관측은 02-05 장부의 가장 새 기록(R2, 개발 자격의 stream)이라 Guardrail 계정에 대해서는 아무것도 말하지 않는다(검토 3e4e3d6 L-5). 그 경우 G1 전에 Guardrail 계정의 관측을 따로 얻을지, 실행 중 신호 멈춤만을 보호로 받아들일지 정해 주십시오.
3. **관측 마감.** R1은 **2026-11-21T23:14Z(한국 2026-11-22 08:14) 전에** 받아들여져야 한다. 그 전에 실행하지 못하면 새 무추가청구 관측을 어떻게 얻을지 사용자가 정한다(실제 stream만이 출처다).

## 10. 검토 반영

### 10.1 PR #86 검토 3e4e3d6

| 항목 | 처분 |
| --- | --- |
| **M-1**(개발 역할의 도구 제한이 강제되지 않음) | **교정.** 코드 750e904: supervisor의 읽기 전용 개발 모드(`"tools": "read_only"` → `--tools Read,Grep,Glob`, 범위 지정 허용, supervisor 경로 거부, 슬래시 명령 끔). 옵션 없는 개발 인자는 그대로(시험). 시험 `supervisor.rs`의 두 시험(인자, 설정; 잘못된 값·Guardrail의 `tools` 거절). 역할 설정의 `deny` 목록(둘째 겹). 회차 뒤 `init.tools`·`tool_use` 확인과 멈춤(§6.2 6). 실제 CLI 관측은 R1이 처음(§4) |
| **M-2**(자동 되돌리기가 영속 DB를 지움) | **교정.** 자동 되돌리기 = 778ee47을 캡처한 인자·환경으로 DB 그대로 다시 기동(§5.4). DB 복원은 별도 결정으로 새 이름에(§3). 라이브 정지 전 마이그레이션 예행(§3, `rehearse-migration.sh`)을 묶음에 넣음. 지금 시험 DB에서 778ee47 스키마 + 고정 행으로 예행(§4 예행 1·2·7: 복원, 15개 표 행 수 그대로, 새 표 11개, M 서비스, 그리고 778ee47이 마이그레이션된 DB에서 돎) |
| **M-3**(정지·기동 명령이 실행 가능하지 않음) | **교정.** `coord-switch.sh`: pid·바이너리·리슨 주소·부모·클라이언트 확인, 인자·환경을 0600 파일로(값 출력 없음), 정확한 정지·기동·되돌리기 명령(`setsid nohup`, 로그, pid 파일). 예행에서 캡처 → 정지 → 새 빌드 기동 → 되돌리기 기동까지 확인 |
| **M-4**(실제 회차 구동 스크립트 없음) | **교정.** `run-step8.sh`(register·prepare·runs) + `lib-step8.sh` + `check-run.py`: 모든 §6.2·§6.3 조건의 강제 멈춤(신호, 환경, 도구, 누적, 미계측, 대기 요청, quota), R1 전 관측 마감, G1 지시문 조립, 같은 키 다시 보내기만. 예행이 같은 스크립트를 돌림(21/21), 고친 사본으로 멈춤 확인의 음성 시험 둘 |
| L-1(loopback 노출) | §4·§5.3·§8에 적음 |
| L-2(Guardrail 항목의 quota) | 교정: 두 항목 모두 `quota_group_id`, 불일치는 멈춤(§6.2 10) |
| L-3(읽기 전용이 권한 비트뿐) | 교정: 대상 파일을 볼륨 밖에 두고 `:ro` 마운트(예행에서 기동 확인). 실제 CLI에서는 미관측(§4) |
| L-4(58090의 다른 클라이언트) | §5.1 항목 3, 캡처 단계에서도 셈 |
| L-5(다른 계정이면 G1의 관측 근거) | §9 Q2 |
| L-6(G1 지시문의 상한·번호) | 교정: 줄마다 번호, 2,048바이트 상한, 비었거나 넘으면 멈춤 |
| L-7(sprint 근거) | 교정: `sources`에 결정 둘, 새 digest `a8d97f12…`, `ieo-spec check` 201 |
| I-1(같은 키 다시 보내기) | §6.1 문장 |
| I-2(회차 중 코디네이터가 죽음) | §7 행 |

### 10.2 PR #86 검토 a5f9402

| 항목 | 처분 |
| --- | --- |
| **N-1**(되살린 환경이 깨지고 값이 명령줄에 나옴, 작업 디렉터리 없음) | **교정**(d0ad250·58819ea). `capture`는 `/proc/<pid>/cmdline`·`environ`을 NUL 경계 그대로 `argv0`·`env0`로 복사하고 `cwd`를 기록하며, 경로 인자를 그 작업 디렉터리에서 확인한다. `start`는 하위 셸에서 지금 환경을 비우고 `env0`를 `read -d ''`로 읽어 셸 내장 `export`로만 되살린다(`env -i VAR=…` 없음: 값이 어떤 프로세스의 명령줄에도 없다). `BASH_FUNC_*`와 식별자가 아닌 이름은 버리고(오류 문구로 값이 새지 않음), 캡처한 작업 디렉터리에서 `exec setsid nohup`. 예행: 내보낸 두 줄 셸 함수와 상대 `--tokens`/`--bindings`를 지닌 옛 프로세스에서 캡처 → M 기동(환경에 셸 함수 없음, 작업 디렉터리·바이너리 확인) → **되돌리기 기동 성공**(§4 예행 1·2·5·8) |
| L-8(규칙 형식) | 교정: `Read(//state/work/<repo>/**)`와 그냥 `Grep`·`Glob`(인자·설정·시험). 변이 4/4 잡힘([evidence/mutants-readonly.txt](evidence/mutants-readonly.txt)) |
| L-9(예행의 바인딩·주체, 행 수 비교) | 교정: `TOKENS_FILE`·`BINDINGS_FILE`, `live-counts.txt` 자동 비교. §5.2에서 새 `tokens.json`(3)을 예행(4) 앞에 둠 |
| L-10(재시작기 판단) | 교정: cgroup 출력, `*.service` 단위(사용자 관리자 `user@<uid>.service` 제외)면 멈춤 |
| L-11(§5.2 순서) | 교정: 캡처 → 백업 → 자격 → 예행 → 역할 컨테이너 → `roles.json`·파일 → 정지 → 기동 → 등록 → 검토 → 준비 → 회차 |
| I-3(`runs` 다시 돌리기) | 교정: 장부에 이미 있는 회차는 기다리지 않음(§6.1) |
| I-4(로그의 커밋) | 최종 스크립트 커밋 58819ea에서 다시 만든 [evidence/dry-run.log](evidence/dry-run.log)(23/23) |

**시험(코드가 바뀌었으므로 전체):** `cargo test --workspace --locked` **412 통과, 0 실패**(커밋 58819ea, fmt·clippy `-D warnings` 통과), `docs/poc` 검사기 통과.

## 11. 변경 이력 · 명세의 수용 프로파일 r7 → r6(사용자 결정 2026-11-17, 선택 (가))

**무엇이 문제였나.** 라이브 58090(778ee47)이 쓰는 manifest는 `/opt/rust/src/github.com/HeaInSeo/ieo_poc-c12/docs/poc/Console_Coordinator_Sprints_v0.json`(SHA-256 `45d78f28486013d0d7ac9348427267124f9e56c14d4cd84358251240a247c287`)이고 `--profiles`는 `/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/profiles.json`(`6eb8ff33bb436be572b1b694cc445953f19cf6ced1dd2a3805dce7c013e0a560`)이다. 효과 프로파일은 `poc-acceptance-v1.3.3` r4·r5·r6과 `x01-notion-import-test` r1이고 **r7이 없다.** 그래서 r7을 가리키던 명세는 라이브 코디네이터에서 `422 UNKNOWN_PROFILE_ITEM`(“revision 7이(가) 없습니다”)으로 거절된다. 저장소 manifest(46799b0, r4~r7)로만 검사해서 이것을 놓쳤다.

**사용자 결정 (가).** 단계 8 명세의 수용 프로파일 참조만 r7 → r6으로 바꾼다. 수용 기준·지시문·예산·권한 범위는 그대로다.

| 항목 | 기록 |
| --- | --- |
| 같은 기준인가 | `02-05.accept`는 r6과 r7에서 같다(저장소 manifest에서 비교; r6과 r7이 다른 항목은 `02-08.accept`·`04-11.accept`·`07-05.accept`뿐). 라이브의 r6 프로파일 전체가 저장소의 r6과 같다 |
| 바뀐 파일 | [sprint-POC-S8-01-r1.json](sprint-POC-S8-01-r1.json): `tasks[0].acceptance.profile_revision` 7 → 6과 `digest`만. 구동 스크립트의 기본 경로(`run-step8.sh`의 `SPEC`)가 이 이름이라 이름을 유지한다 |
| 옛 판 | [sprint-POC-S8-01-r1-profile-r7.json](sprint-POC-S8-01-r1-profile-r7.json)(이력, 쓰지 않음), digest `a8d97f129dca590e5d93fb8ddb6d9aa1fbf2fcaa14d72b152e384ca3a71a7bb7` |
| 새 digest | **`0e4d471dc996dd9f7dfea269975bc5cf2491eaf85f6e6836e1526a1d961d0124`** |
| 검사 | 46799b0의 `ieo-spec`(release)으로 `check --executable`: 효과 라이브 manifest(manifest의 `inspection_profiles` 뒤에 `--profiles` 배열을 붙임, `ieo-coord serve`와 같은 방식) → **201 `ACCEPTED_FOR_SEMANTIC_REVIEW`**, 저장소 manifest → 201, 옛 판 + 효과 라이브 manifest → 422. 기록 [evidence/spec-r6-check.txt](evidence/spec-r6-check.txt) |
| 실행본 사본 | `$D/step8-46799b0/sprint-POC-S8-01-r1.json`을 새 명세로 바꿨다. 이제 그 사본은 **이 파일 하나만** 46799b0과 다르다(이 수정 브랜치의 blob과 같음, SHA-256 `f28996d64d667b91936429afee9b56ea0296b0145338be4108daf566e939ea19`). 나머지 15개는 46799b0과 같다(사본의 `EXECUTION_PLAN.md`도 46799b0 판 그대로) |
| 목록과 권한 | `$D/step8-ops/SHA256SUMS`를 다시 만들었다(17개). 목록 자체의 해시가 `d91de424…`에서 `50507966…`으로 바뀌어 [MANUAL_STEPS](MANUAL_STEPS.md)의 확인 줄과 허용 규칙 **한 줄**(`echo '<해시>  step8-ops/SHA256SUMS'`)이 바뀐다. 다른 규칙은 그대로다 |
| 예행 기록 | [evidence/dry-run.log](evidence/dry-run.log)(23/23)는 r7 판(`a8d97f12…`)과 저장소 manifest로 한 것이다. 다시 만들지 않았다: 바뀐 것은 명세의 수용 revision 숫자뿐이고 새 명세는 위 검사로 확인했다 |

**원인과 재발 방지.** 실행안은 시험 명세를 저장소의 manifest(46799b0)로만 미리 검사했고, 라이브 배포가 실제로 쓰는 manifest·profiles로는 검사하지 않았다. 앞으로의 규칙: **실행안을 확정하기 전에, 시험 명세를 라이브 배포의 효과 manifest·profiles(읽기 전용)로 `ieo-spec check --executable` 해 본다.** 라이브 프로세스의 `--manifest`·`--profiles` 경로는 인자에서 읽고, 효과 manifest는 코디네이터와 같은 방식(프로파일 배열을 `inspection_profiles` 뒤에 붙임)으로 만든다.
