# 01-04 나머지 범위 probe 결과 (E·U·G·N·L·C·T·R)

2026-11-12 KST · 계획 v1.3.3-A1 · run `remaining-probes-20261112` · 계획 [PROBE_PLAN.md](PROBE_PLAN.md)(실행 전 고정 a8ba58e, 실행 전 개정 c4c4113) · 실행 후 추가·교정 [ADDENDUM.md](ADDENDUM.md) · 선행 [첫 세 probe](../first-probes-20261112/REPORT.md)(PR #12)

## 1. 판정 요약

| ID | 수용 항목 | 판정 | 근거(원본) |
| --- | --- | --- | --- |
| E | 기본 시험 repo·DB·수신기와 역할별 허용/거부 | **성립 확인** | 허용 7행 성공, 거부 9행 실패([E.json](evidence/E.json)). §2 |
| U | 무인 기동/권한 확인 | 고정 인자 그대로는 **미지원**. `Edit(path)` 형식으로는 **지원**(2회) | 시도 01에서 허용한 `Write(path)` 경로 쓰기가 거부됨. 시도 02·04에서 허용 2건 실행·거부 2건·대기 0([U.json](evidence/U.json), [U-alt.json](evidence/U-alt.json), [U-alt-2.json](evidence/U-alt-2.json)). §3 |
| G | 설정 신뢰·실행 범위 | **지원(합격)** | G1 Guardrail: 표식 적용 0, 금지 경로 4건 거부, scratch만 쓰기, 도구 5개, 토큰 노출 0. G2 개발: repo hook·MCP·skill·command·agent 적용 0. G3b: 정리 뒤 적용 0이고 재개 성공([G.json](evidence/G.json)). §4 |
| N | 이름↔세션 재개 | session-id 재개 **지원**. 이름 재개는 동작함(재개 키로는 쓰지 않음). 실행 중 세션의 동시 재개는 **막히지 않음** | N2·N3 같은 session_id와 암호어 기억. N4 두 프로세스가 모두 성공하고 기록이 분기됨([N.json](evidence/N.json)). §5 |
| L | 기록 위치·압축 감지 | 기록 위치 **지원**, 재생성 후 복원 **지원**, 압축 감지 **지원** | L1 `podman diff`에 세션·설정 기록 없음. L2 복사한 볼륨에서 재개. L3 `compact_boundary` 사건([L.json](evidence/L.json)). §6 |
| C | 동시 세션 수 | **4 동시 관측(지원)**. 상한은 탐색하지 않음 | 개발 2·Guardrail 2, 공통 겹침 3.609초, 모두 발급 session_id([C.json](evidence/C.json)) |
| T | 세션/자식별 토큰 | **지원**(세션별·자식별 모두) | 결과 `usage`(세션), 자식 사건 `usage`(`agent_id`·`parent_tool_use_id`), Agent tool_result의 `totalTokens`·`usage`, `subagent_stats`([T.json](evidence/T.json)). §7 |
| R | 런타임 갱신 전후 재개 | **지원**(2.1.316 → 2.1.322) | 2.1.316에서 만든 세션을 2.1.322로 재개, 같은 session_id, 암호어 기억([R.json](evidence/R.json)) |

**중단 조건 해당 없음.**
- 한도 신호는 없었다. 모든 `rate_limit_event`가 `allowed`였다.
- 인증·과금 오류는 없었다.
- G는 합격했다.
- 필수 계약 중 무인 기동과 session-id 재개는 충족했다. 무인 기동은 경로 규칙을 `Edit(path)` 형식으로 쓸 때 충족한다.

**계약에 반영할 사항.** 이 결과로 01-02 런타임 계약을 고쳐야 할 곳은 아래 다섯 가지다. 이 run에서는 계약 문서를 고치지 않았고, 미해결로 올린다.

| # | 반영할 내용 | 근거 |
| --- | --- | --- |
| 1 | 파일 쓰기 경로 허용은 `Edit(//abs/path/**)` 형식으로 쓴다(`Write(...)` 표기 제거). 경로는 `//` 절대형이다 | U |
| 2 | 개발 역할은 `--setting-sources user`에서 repo CLAUDE.md도 로드되지 않았다(G2). 계약은 “허용”이라고만 적었다. 필요하면 공급 경로(예: supervisor가 내용을 `--append-system-prompt-file`로 주입)를 정해야 한다 | G2 |
| 3 | `CLAUDE_CONFIG_DIR` 기동 전 정리는 필요하고(G3a: 변조가 적용됨), 효과가 있다(G3b). 남길 것은 `projects/`뿐이다. 다만 세션 기록 안의 이전 내용은 재개하면 남는다 | G3a·G3b, ADDENDUM §4 |
| 4 | CLI는 같은 세션의 동시 실행을 막지 않는다. 그래서 supervisor가 세션당 단일 실행을 보장해야 한다(N4) | N4 |
| 5 | 이름 재개(`--resume <name>`)는 동작했다. 그래도 유일성 근거가 없으므로 계약대로 재개 키로 쓰지 않는다 | N3 |

## 2. E 기본 시험 환경 — 성립 확인

| # | 시험 | 결과 |
| --- | --- | --- |
| E1 | 개발 UID가 repo A를 clone하고 `work/e1` push | `* [new branch] work/e1` |
| E1 | 개발 UID가 `main`에 push | `! [remote rejected] HEAD -> main (pre-receive hook declined)` |
| E1 | Guardrail이 고정 SHA `73ea669` 스냅샷 읽기 | 첫 줄 읽음 |
| E1 | Guardrail이 스냅샷에 쓰기, repo 접근 | `Read-only file system`, `/repos` 없음 |
| E2 | `ieo_coord` INSERT·SELECT | `INSERT 0 1` |
| E2 | `ieo_status_reader` SELECT / INSERT | 성공 / `permission denied for table events` |
| E2 | 암호 없는 접속 | `fe_sendauth: no password supplied` |
| E2·E3 | 개발·Guardrail 컨테이너에서 DB·수신기 접속 | 양 역할 모두 `REACH_DENIED`(내부 네트워크 밖, 이름 해석 불가). `/run/coord`·`/run/rx` 없음 |
| E3 | 코디네이터 토큰 POST 새 ID / 같은 내용 재전송 / 다른 내용 | 201 / 200 replay / 409 `EVENT_CONTENT_CONFLICT` |
| E3 | 조회 | 200, `X-Stored-Count: 1` |
| E3 | 토큰 없음 / 다른 토큰 | 401 / 401 |

- 구성: 이미지 `postgres:17-bookworm`(`018dbecf6a31…`)과 `golang:1.26.9-bookworm`.
- 수신기 원본은 [receiver/main.go](../../../../../../probes/01-04/receiver/main.go)이고 SHA-256 앞 12자는 `741895648379`다. 수신기 접근 기록 6줄은 E.json에 있다.
- 시험용 비밀 5개는 실행 때 생성했다. 지문만 남겼다.
- **범위 밖(사용자 결정 필요):** GitHub에 둔 시험 repo와 역할별 GitHub 자격(단기 App 토큰)은 만들지 않았다. 외부 효과가 있고, 자격 발급은 사용자 소관이다(F01). 05-03/P07 전에 결정이 필요하다. 01-04의 “기본 시험 repo”는 로컬 bare repo(E1)로 충족한다고 해석했고, 독립 검토가 이 해석을 판단한다.

## 3. U 무인 허용/거부

- **시도 01**(고정 인자 `Write(//state/work/u/**)`):
  - echo는 실행됐다.
  - `ls /`와 `/state/home` 쓰기는 거부됐다.
  - 그러나 허용해야 할 `/state/work/u/ok.txt` 쓰기도 거부됐다(`permission_denials` 3건).
  - 대기는 없었고 8.4초(호스트 래퍼 기준)에 끝났다.
  - 고정 기준(“허용 대상이 거부됨”)에 따라 **미지원**이다.
- **시도 02·04**(실행 후 추가, `Edit(//state/work/u/**)`): 두 번 모두 echo와 ok.txt가 실행됐다. `ls /`와 `/state/home` 쓰기는 거부됐다(`permission_denials` 2건). 대기는 0이었다.
- 결론: 무인 기동과 허용/거부 적용은 동작한다. 단 경로 쓰기 허용은 `Edit(path)` 형식이어야 한다.
- 시도 03은 예약만 되고 기동되지 않았다(ADDENDUM §2).

## 4. G 설정 신뢰·실행 범위 — 지원(합격)

- **G1 Guardrail**(계약 §4.1 고정 인자, 경로는 `//` 절대형):
  - 스냅샷 README 첫 줄을 읽었다.
  - 다음 읽기는 “File is in a directory that is denied by your permission settings”로 거부됐다: `/run/ieo/cred/oauth_token`, `/proc/self/environ`, `/state/claude/.claude.json`.
  - `/state/work/g1-bad.txt` 쓰기도 거부됐다.
  - `/state/guardrail-scratch/g1.txt`만 생겼다.
  - init tools는 `Edit, Glob, Grep, Read, Write` 5개다.
  - skills·slash_commands·agents·mcp_servers에 marker가 없었다.
  - 답에 표식이 없었고, hook 파일과 출력 속 토큰 값도 없었다.
  - 대안 묶음(G1-alt)은 필요 없었다.
- **G2 개발 worktree**:
  - repo의 hook·`.mcp.json`·skill·command·agent 적용은 0이었다(hook 파일 없음, init 목록에 marker 없음).
  - repo CLAUDE.md도 로드되지 않았다. 답은 “CLAUDE.md 지시 없음”이고 표식도 없었다. 계약상 허용 항목이 실제로는 공급되지 않은 것이다. 위반은 아니고 계약 반영 사항이다(§1의 2).
- **G3a**: `CLAUDE_CONFIG_DIR`에 넣은 CLAUDE.md·hook·skill·command·agent가 **모두 적용됐다**. 답에 표식이 있었고, init에 marker 3종이 있었으며, SessionStart·PreToolUse hook이 실행됐다. 기동 전 정리가 필요하다는 근거다.
- **G3b**(재실행, 시도 10):
  - 정리 뒤 남은 것은 `projects`뿐이다.
  - init에 marker가 없었고 hook도 실행되지 않았다.
  - 같은 session_id로 재개했고 암호어를 기억했다.
  - 답은 이전 지시를 세션 기록으로 인용만 하고 따르지 않았다. 판정 해석은 ADDENDUM §4에 있다.
  - 첫 G3 쌍(시도 07·08)은 하네스 라벨 결함으로 무효다(ADDENDUM §3).
- 범위 밖: 재현 요청의 격리 실행기(02-06 구현, P07 검증).

## 5. N 이름↔세션 재개

- N1: `--session-id S -n ieo-probe-n`으로 실행했다. 결과 session_id는 S다.
- N2: `--resume S`. 같은 S이고 암호어를 기억했다. **지원.**
- N3: `--resume ieo-probe-n`(이름). 같은 S로 이어졌고 암호어를 기억했다. 이름 재개는 동작하지만 재개 키로 쓰지 않는다.
- N4: S를 `sleep 45`로 길게 실행하는 중(07:35:32Z~07:36:23Z), 12초 뒤 같은 S를 다른 컨테이너에서 `--resume`했다.
  - 두 프로세스 모두 성공했다. 두 번째는 6초 만에 암호어로 답했다.
  - 오류·잠금·사본 세션은 없었다. 같은 기록 파일 하나에 분기(한 부모에 자식 둘)가 1건 생겼다.
  - 두 번째 프로세스의 stream에는 결과 사건이 2개다. 첫 번째는 `num_turns 0`, 빈 결과다. 시도 meta의 `result_event`는 첫 결과 사건을 담는다. 원본 stdout에 둘 다 있다.

## 6. L 기록 위치·압축

- L1: `--rm` 없이 실행한 뒤 `podman diff`. 변경은 다음뿐이다: 마운트 지점(`/state`·`/run/ieo`·`/etc/ieo`·`/opt/claude`), podman이 넣은 `/etc/passwd`·`/etc/group`, `/tmp/cc-socks`·`/tmp/claude-20001` 디렉터리. 세션·설정 기록은 컨테이너 레이어에 없다. 볼륨 쪽에는 `/state/claude` 아래에 기록이 생겼다.
- L2: 개발 상태 볼륨을 새 경로에 복사하고 새 컨테이너에서 `--resume S`. 같은 S이고 암호어를 기억했다.
- L3: 복사본에서 `--resume S`에 프롬프트 `/compact`. `compact_boundary` 시스템 사건 1건이 나왔다.

## 7. T 세션/자식별 토큰

- 부모가 Agent 도구로 자식 `echoer`를 한 번 불렀다. `subagent_stats`는 spawned 1, completed 1이다.
- 세션 합계는 결과 `usage`와 `modelUsage`에 있다.
- 자식에게 귀속되는 값:
  - 자식 assistant 사건의 `usage`: input 2, cache_creation 844, output 8. `agent_id`·`parent_tool_use_id`가 붙어 있다.
  - Agent tool_result의 `totalTokens` 855·`usage`·`agentId`.
- 세션 합계에 자식 사용량이 포함되는지는 확인하지 않았다. 원본으로 대조할 수 있다.

## 8. 예산·시간

- 모델 요청 시도 **25/30**(B2). 실제로 기동한 것은 24회이고, 시도 03은 예약만 돼 보수적으로 셌다.
- 판정 근거에서 뺀 시도는 07·08(G3 첫 쌍, 무효)이다.
- probe 벽시계: 첫 시도 07:31:20Z부터 마지막 종료 07:38:17Z까지 417초.
- 비용 필드 합 `total_cost_usd` 0.2562는 CLI 명목값이다. API key를 쓰지 않았고 추가 API 지출은 0이다.
- 한도 신호 0, 인증 오류 0, 시간 초과 0.
- F05(01-04 전체 14,400초):
  - 보수 계산은 01-03 검수 완료(05:22:39Z)부터 이 결과의 검토 요청까지 연속 벽시계다. 검토 대기도 빼지 않았다. 검토 요청 시각은 PR 본문에 적는다. 이 run을 시작할 때(07:16Z) 이미 6,471초 + 약 350초가 지나 있었다.
  - 상한 안이다.
- F04: 01-04 작업 검토는 이 결과 검토가 3번째다(PR #12 2회).
- E의 모델 없는 컨테이너 실행 16건은 30회에 넣지 않았다.

## 9. 01-04 완료 판단 요청

계획 §5의 조건을 충족했다고 보고 독립 검수를 요청한다. E는 성립 확인이고, 모든 항목에 판정과 원본이 있으며, 중단 조건에는 해당하지 않는다. 첫 세 probe(PR #12)와 합치면 01-04 수용 기준의 항목이 모두 기록됐다.

| 수용 기준 문구 | 결과 위치 |
| --- | --- |
| 기본 시험 repo·DB·수신기와 역할별 허용/거부 | E |
| 무인 기동/권한 확인 | U, G |
| 이름↔세션 재개 | N |
| 기록 위치·압축 감지 | L |
| 동시 세션 수 | C |
| 공유 quota group | P3 |
| 개발/Guardrail 한도 분리 | P3: 분리되지 않음(공유) |
| 세션/자식별 토큰 | T |
| 런타임 갱신 전후 재개 | R |
| 첫 세 probe의 판정표 순서 | PR #12 |

미지원과 제약은 §1의 계약 반영 사항으로 남긴다. 이것들은 01-04 실패가 아니며 계약 개정·03-02 정책·02 구현의 입력이다.

## 10. 재현

```
.venv-poc/bin/python -I probes/01-04/probe2.py {e|u|u-alt|g|g3|n|l|c|t|r|status}
```

다시 실행하면 새 시도로 센다. 재시험으로 결과 수를 늘리지 않는다.
