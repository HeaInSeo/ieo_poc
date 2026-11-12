<!-- 보관본: PR #12 독립 검토(Guardrail) 0303f3e 원문과 게시 댓글 끝줄(교정 커밋 안내). 게시 댓글의 머리말 1줄(검토 주체·SHA·판정)은 생략 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/12#issuecomment-6817381159 -->
# PR #12 독립 검토 (Guardrail) · 01-04 첫 세 probe

- 검토 SHA: `0303f3e32dfcd6112eb8438a07c08b004da2a2b8` (branch `task/01-04-probes`)
- base: `main` `c729474`
- 날짜: 2026-11-12
- 범위: `docs/poc/reports/tasks/01-04/first-probes-20261112/` 전체(REPORT.md, PROBE_PLAN.md §5, evidence/), `probes/01-04/probe.py`, `probes/01-04/identity_claims.py`. 기준은 사용자 결정 D1·D2·D6, 후속 지시 5, `probe_gate_table.json`(A1-GATE-01-04), `Probe_Budget_Rules.md`, a321cd2·93a44ad 시점 PROBE_PLAN.md, `Console_Coordinator_Sprints_v0.json`의 01-04 수용 문구.
- 검토자는 `claude -p`와 probe.py p1/p2/p3를 실행하지 않았다(예산 9/30 그대로). 비밀 값은 출력하지 않고 동등 비교·해시로만 대조했다. 저장소는 바꾸지 않았다.
- CI: head `0303f3e`에서 세 job 모두 SUCCESS(gh 확인).

VERDICT: reject

차단 사유는 P2 한 건(공개 증거에 토큰 전체 SHA-256 기록)이다. 세 판정(P1 지원, P2 성립 확인, P3 공유 확인)은 검토자가 다시 계산하고 독립적으로 대조한 결과 원본 증거와 동결 기준에서 따라 나온다. 이 P2를 처리하면 승인할 수 있다.

---

## 1. 재계산 결과 (질문 1)

| 항목 | REPORT 주장 | 검토자 재계산 | 일치 |
| --- | --- | --- | --- |
| P1 1회차 겹침 | 3.811초 | min(145.2568, 145.2885) − max(141.4456, 141.4004) = 3.8112 | 예 |
| P1 2회차 겹침 | 3.352초 | min(149.9906, 149.5479) − max(146.1961, 146.1785) = 3.3518 | 예 |
| 겹침 시각 출처 | 컨테이너 안 시각 | stderr `#T_START/#T_END`가 meta·P1.json 값과 같음 | 예 |
| P1 session_id | 발급 UUID와 같음 | 01~04 모두 `system/init`·`result`의 session_id가 `issued_session_id`·argv `--session-id`와 같음, `result` = `ok-<role>` | 예 |
| 시도 수 | 9/30 (P1 4, P2 1, P3 4) | budget.json attempts n=1..9, attempts/ 디렉터리 9개, 수동 재시도 없음 | 예 |
| 벽시계 | 69초 | 07:05:40.941Z → 07:06:50.184Z = 69.243초 | 예 |
| tool_use / parent_tool_use / 시간 초과 | 0 / 0 / 0 | stdout 재파싱: 0 / 0 / 0. init의 `tools=[]`, `mcp_servers=[]`, `apiKeySource=none` | 예 |
| rate_limit_event | P1 0.11/0.29, P3 0.12/0.29, resetsAt 1794479400/1794729600, 모두 allowed | stdout 재파싱 결과 같음. meta.json의 rate_limit_events와 stdout이 완전히 같음 | 예 |
| 관측 쌍 수 | "6쌍" | **4회차 × 두 역할 = 4쌍(사건 8개)**, 창 기준으로는 8쌍 | **아니오** (P3-1) |
| 비용 합 | 0.0175 | 0.0175041 | 예 |
| 2c | rc 1, `error_during_execution`, 0 토큰 | 같음. 대상 `73305c11…`은 시도 04(P1 2회차 Guardrail)의 발급 ID | 예 |
| 2c 대조 | Guardrail 볼륨 1개 31,167 bytes, 개발 0개 | 같음(같은 `projects/-state-work/` 경로라 프로젝트 경로 차이 때문에 실패한 것도 아님) | 예 |
| 2d | 파일 12개, TOKEN_ABSENT | 같음 | 예 |
| 볼륨 소유 | 185536/185537, 0700 | setup.json `stat`과 같고 subuid 165536 + (20001−1)과 맞음 | 예 |
| 모델 없는 실행 | "컨테이너 실행 10건과 setup" | P2 시험 10건 중 컨테이너 실행은 7건(2a, 2b×2, 2p×2, 2d-control, 2d), 3건은 호스트(2a-host는 `podman unshare`, 2e×2는 bash) | 표현 부정확 (P3-6) |

## 2. 판정 타당성

### P1 — 지원 (동의)
PROBE_PLAN §1의 조건은 실행 구간 겹침, 두 역할 모두 오류 없는 result, 발급 session_id 일치, 2회 반복이다. 이 조건을 모두 만족한다. 한계(공급자 쪽 동시 처리를 직접 관측한 것은 아님)도 REPORT §3에 적혀 있다.

### P2 — 성립 확인 (동의, 보완 권고 있음)
- **가용성과 차단 성공 분리(후속 지시 5)**: 지킨다. setup.json, host-isolation-check.txt, container-smoke.txt는 가용성 증거로 표시했다. dry-run은 판정 증거가 아니라고 README에 적었다. 차단 판정은 실제 자격으로 한 교차 시도(P2.json)에만 근거한다.
- **양성 대조**: 2p(소유 UID가 같은 `:z` 라벨·같은 이미지·같은 옵션으로 자기 볼륨에 LIST/READ/WRITE_OK)와 2d-control(같은 검사가 자기 토큰을 ENV_MATCH·FILE_MATCH로 찾음)이 있다. dry-run에서 SELinux 때문에 자기 볼륨도 거부된 결함을 고친 뒤의 대조라서, 2b의 거부가 SELinux가 아니라 DAC(UID·0700)에서 나왔다고 보는 근거로 충분하다. 다만 2b 출력은 `2>/dev/null`로 errno를 버린다. 그래서 EACCES와 ENOENT가 원문으로 구분되지 않는다(P3-4).
- **2d 검사의 범위**: 2d는 파일 전체(또는 끝 개행을 뺀 내용)의 해시를 비교한다. 그래서 다른 파일 안에 토큰이 박혀 있으면 찾지 못한다(P3-3). 검토자가 `podman unshare`로 두 역할 볼륨의 모든 파일(개발 14개, Guardrail 15개)을 부분 문자열로 검사했다. 각 토큰은 자기 `cred/oauth_token`에만 있고, 교차 존재는 0건이다. 결론은 유지된다.
- **2c**: 같은 프로젝트 경로(`-state-work`)에서 대상 세션 기록이 Guardrail 볼륨에만 있고, 개발 프로세스는 "No conversation found"로 실패한다(모델 사용 0). 대조와 함께 보면 이 실패는 볼륨 분리 때문이다. 존재하지 않는 ID나 경로 차이 때문이 아니다. 실질적으로는 2a(마운트 없음)의 CLI 수준 귀결이며, 공급자 쪽 재개 경로를 시험한 것은 아니다. REPORT §4가 이 한계를 정확히 적었다. 모델 요청이 실제로 나가지 않았는데도 1회로 센 것은 예산 규칙 §2(기동 순간 1회)에 맞는 보수적 집계다.
- **2e 한계**: `podman unshare`로 heain이 두 볼륨을 읽을 수 있다는 점(검토자도 그렇게 읽었다)을 역할 간 격리와 따로 적었다. 적절하다.

### P3 — 공유 확인 (동의, 근거 기술 교정 필요)
**사후 식별 출처가 기준 변경인가.** 기준 변경이 아니라 측정 보강으로 판단한다. 새 revision이나 사용자 결정은 필요하지 않다. 근거는 다음과 같다.
1. 동결 기준 문구는 "두 토큰의 계정·org 식별이 같고 + 같은 창의 resetsAt가 같다"이다. 이 문구는 바뀌지 않았다. 바뀐 것은 식별을 얻는 관측 경로다. 계획한 경로(`auth status`)는 토큰 인증에서 식별 필드를 내지 않았다(P3.json `fields_present`). 계획은 이 경우를 예상하지 못했다.
2. "관측 불가 → 중단"은 동결 문구상 해당하지 않는다. §3의 관측 불가는 "한도 정보가 출력에 없고, **그리고** 계정 식별도 얻을 수 없다"이다. 한도 정보(rate_limit_event, 두 창)는 8개 사건 모두에 있다. 식별 출처를 더하지 않았다면 결과는 "공유도 분리도 확정되지 않음(예산 21회 남음)"이지 관측 불가가 아니다. 사후 출처는 결과를 뒤집은 것이 아니라 계획이 비워 둔 틈을 메웠다.
3. 사후 선택 편향 위험이 낮다. 결과가 엇갈리는 여러 출처 중 하나를 고른 것이 아니다. 단일 출처이고, 공급자가 서명한 문서다. 검토자가 독립적으로 확인한 내용은 다음과 같다.
   - **서명 검증**: 두 `policy-limits.json.signature.json`의 ES256 서명이 각 x5c leaf 인증서 공개키로 `openssl dgst -verify` → **Verified OK**. leaf CN `claude-code-managed-settings-signing`, 발급자 "Claude Code Signed Cache Issuing CA", 그 상위는 "Anthropic Apps Signing Root"다. 루트 신뢰 앵커와는 대조하지 않았다.
   - **각자 받은 문서**: 두 서명 파일의 SHA-256이 다르고 leaf 인증서·`kid`도 다르다. 두 문서가 각각 서명됐다는 뜻이다. payload는 같고 `iat`=1794467204(07:06:44, P3 2회차 동시 기동 시각)이다.
   - **클레임의 의미**: 호스트 heain의 대화형 로그인(`~/.claude.json` `oauthAccount`)과 동등 비교만 했다. `sub` == `accountUuid`이고, **`aud` == `organizationUuid`**이며, `ws`는 둘 다 아니다. 따라서 계정은 `sub`, org는 `aud`로 식별되고 두 역할 모두 같다(P3-identity.json `comparison.aud = same`). 기준의 "계정·org 식별이 같다"를 충족한다. 이 확인으로 두 역할 토큰이 개발 대화 자체와 같은 계정이라는 것도 드러난다.
4. PROBE_PLAN §5는 이 보강을 실행 뒤 추가라고 밝혔고, 판정을 독립 검토에 맡겼다. 정직하게 공개했다.

따라서 `shared_confirmed → proceed_record_supported_shared`는 정당하다. 다만 REPORT가 판정 근거로 든 클레임(`sub`·`ws`·`plan`)은 org 식별을 잘못 짚었다(P3-2). "공유 확인을 한도 분리 성공으로 쓰지 않는다"와 수용 항목 "한도 분리 → 분리되지 않음" 기록은 판정표 note와 맞다. 두 번째 분기(utilization 동조)를 쓰지 않은 판단도 옳다. 0.11→0.12는 두 역할과 개발 대화가 모두 쓴 구간이라 구별이 안 된다.

---

## 3. 발견 사항

### P2-1 (차단) 공개 증거에 두 역할 토큰의 전체 SHA-256이 기록됨 — 동결된 기록 규칙 위반
- 위치: `docs/poc/reports/tasks/01-04/first-probes-20261112/evidence/P2.json:260`(2d-control argv, `G=` + development 토큰 64자 해시), `:300`(2d argv, guardrail 토큰 64자 해시). 생성 코드는 `probes/01-04/probe.py:252-256`(`token_scan(hashlib.sha256(...).hexdigest())`)이고, 그 argv가 `probe.py:216`에서 그대로 저장된다.
- 근거: 검토자가 커밋 트리 전체를 비교한 결과, 두 토큰의 `sha256(token)` 64자 전체가 P2.json과 커밋 `0303f3e` diff에 나타난다(값은 출력하지 않고 일치 여부만 확인).
- 위반하는 규칙:
  - PROBE_PLAN.md:11(a321cd2부터 동결): "값을 출력·기록하지 않고 **SHA-256 앞 12자 지문만** 남긴다".
  - `probe.py:6` 자체 주석: "only SHA-256 prefixes".
  - REPORT §2는 지문을 12자로만 제시하므로, 독자는 전체 해시가 공개된 사실을 알 수 없다.
- 영향: 고엔트로피 토큰이라 역산 위험은 실질적으로 낮다. 그래도 공개 repo에 장기(setup-token) 자격의 완전한 확인 오라클이 남는다. PR 브랜치가 이미 공개라서 브랜치 수정만으로 이력에서 지워지지 않는다.
- 요구:
  - (a) P2.json에서 두 `G=` 값을 12자 지문 + 가림 표시로 바꾸고, 바꾼 사실을 증거에 주석으로 남긴다.
  - (b) probe.py가 기록용 argv에서 전체 해시를 가리거나, 해시를 argv 대신 컨테이너 안의 읽기 전용 파일로 넘기도록 고친다. 이후 01-04 나머지 probe에서 재발하지 않게 하기 위해서다.
  - (c) 이미 공개된 이력의 처분(이력 재작성 안 함을 기본으로, 토큰 재발급 여부는 사용자 판단)을 REPORT에 기록한다.
  - 기존 증거를 다시 실행할 필요는 없다. 판정은 영향을 받지 않는다.

### P3-1 rate_limit_event 관측 수 "6쌍"이 원본과 다름
- 위치: REPORT.md:13, REPORT.md:66.
- 근거: P1 2회차 + P3 2회차 = 4회차 × 두 역할 = 역할 쌍 4개(사건 8개)다. 창(five_hour, seven_day) 기준으로는 8쌍이다. "6"에 해당하는 집합이 없다. "4쌍(사건 8개, 창 비교 8쌍)"으로 고친다.

### P3-2 P3 판정 근거의 org 식별 클레임 오기
- 위치: REPORT.md:13, :78, :88. PROBE_PLAN.md §5(59행).
- 근거: 검토자의 동등 비교 결과 `sub`=계정 UUID, `aud`=org UUID이고, `ws`는 둘 다 아니다. 기준은 "계정·org 식별"인데, REPORT는 `sub`·`ws`·`plan`을 근거로 들고 `aud`는 언급하지 않는다. 결론(같음)은 P3-identity.json `comparison.aud = same`으로 이미 뒷받침되므로 판정은 유지된다. 근거 문구를 "`sub`(계정)·`aud`(org) 같음, `ws`·`plan`도 같음"으로 고치고, 의미 확인 방법(호스트 로그인 계정과 동등 비교, 값 미기록)을 덧붙인다.

### P3-3 2d 검사는 파일 전체 일치만 탐지
- 위치: `probes/01-04/probe.py:244-250`, REPORT.md 2d 행.
- 근거: 파일 해시가 토큰 해시와 같은 경우만 찾는다. 설정 JSON 같은 다른 파일 안에 박힌 토큰은 놓친다. 2d-control도 토큰만 담은 파일로만 성공을 보였다. 검토자의 부분 문자열 검사(위 §2 P2)로 이번 결론은 유지된다. 이후 probe에서는 부분 문자열 검사(예: `grep -F -f` 를 컨테이너 안 읽기 전용 파일로)로 바꾸고, 대조도 "다른 파일 안에 박힌 토큰"으로 만드는 것을 권한다.

### P3-4 2b 출력에 오류 원문(errno)이 없음
- 위치: `probe.py:230-233`(`>/dev/null 2>&1`), P2.json 2b 출력.
- 근거: `LIST_DENIED`는 EACCES와 ENOENT를 구분하지 않는다. 예산 규칙 §5는 "거부 오류 원문"을 남기라고 한다. podman은 없는 원본 경로를 마운트하면 실패하고 rc=0이므로 실제 위험은 낮다. 그래도 `stat -c '%u %a' /x/state`와 stderr 원문을 남기면 같은 컨테이너 안에서 마운트 존재를 증명할 수 있다.

### P3-5 동결 계획 문서에 사후 절을 덧붙임
- 위치: PROBE_PLAN.md:1(제목 "실행 전 고정"), §5(55행 이후).
- 근거: 기준은 그대로이고 §5 제목에 "실행 뒤 추가"라고 적었으며, git으로 a321cd2/93a44ad와 구별된다. 그래도 HEAD만 읽으면 "실행 전 고정" 문서에 사후 내용이 섞여 있다. 사후 측정 보강은 별도 addendum 파일로 옮기고 PROBE_PLAN.md는 93a44ad 내용 그대로 두기를 권한다. 기준 변경이 아니므로 사용자 결정은 필요하지 않다고 판단한다(§2 P3 참조).

### P3-6 기록 세부가 계획·예산 규칙과 어긋남
- PROBE_PLAN.md:53은 `evidence/budget.log`라고 했지만 실제는 `budget.json`이다.
- 예산 규칙 §3이 요구하는 시도별 "누적 시도 수와 남은 벽시계"가 meta.json에 없다. budget.json의 `first_attempt_at`으로 계산할 수는 있다.
- 시도 05(2c)는 다른 시도와 달리 stderr에 `#ENV_NAMES`·`#UID`·`#T_START/#T_END`가 없다(`probe.py:271-274`). argv로 대체할 수는 있다.
- `auth status` 2회는 rc와 파싱한 필드만 남았고 원문은 없다(예산 규칙 §2 "원본 로그를 남긴다", `probe.py:300`은 파싱 실패 때만 원문을 남김).
- REPORT.md:100의 "컨테이너 실행 10건"은 실제로 컨테이너 7건, 호스트 3건이다.

### P3-7 P3-identity.json의 출처 기록이 약함
- 위치: `evidence/P3-identity.json`, `probes/01-04/identity_claims.py`.
- 근거: 읽은 시각, 원본 파일(서명 파일)의 SHA-256·mtime, 실행 명령·rc가 없다. 캐시 파일은 다음 CLI 실행 때 덮어써지므로 지금 남겨야 재현할 수 있다. 서명은 `openssl`(호스트에 있음)로 x5c leaf 대비 검증할 수 있다(검토자 결과 두 역할 모두 Verified OK). 따라서 REPORT.md:87 "서명은 검증하지 않았다"는 검증 기록으로 바꿀 수 있다.
- 추가: 계정·org UUID의 12자 해시 공개는 후보 UUID를 아는 사람에게 확인 수단이 된다. 위험이 낮으므로 기록만 권한다.

---

## 4. 비밀·PII 점검 (질문 4)
- 검토자가 `0303f3e` 트리의 모든 파일과 a321cd2·93a44ad·cf767e5·0303f3e 커밋 diff·메시지를 대상으로 다음 값을 동등·부분 문자열로 대조했다(값은 출력하지 않음).
  - 두 토큰 원문, 접두 뒤 본문, 중간 32자
  - `sk-ant-` 패턴
  - 두 역할 `.claude.json` userID
  - JWS `sub`·`ws`·`aud` 원문(대시 포함·제외)
  - 호스트 로그인의 이메일·accountUuid·organizationUuid·organizationName
- 결과: 위 값은 모두 0건이다. **예외는 P2-1의 토큰 전체 SHA-256 2건**이다.
- 호스트 표시 이름(displayName/fullName)과 같은 문자열이 이 PR 밖의 기존 문서 4개(`docs/poc/01_plan_and_review/Independent_Review_*.md`)에 있다. 이 PR이 바꾼 파일이 아니고 repo 소유자 표기로 보인다. 이 PR의 범위가 아니다.

## 5. 승인 조건
P2-1의 (a)·(b)·(c)를 반영한 새 head에서 P2.json의 해시 가림, probe.py 수정, 처분 기록을 확인하면 승인한다. P3-1·P3-2는 같은 수정에서 함께 고치기를 권하지만 차단 사유는 아니다. 나머지 P3은 01-04 나머지 probe 계획을 고정할 때 반영해도 된다.

---
교정 커밋: `4ada612` (ADDENDUM.md §2·§3). 재검토 예정.
