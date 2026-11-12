<!-- 보관본: PR #14 독립 재검토 a3d01ce 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/14#issuecomment-6818305666 (머리말 1줄 생략) -->
# PR #14 재검토 (Guardrail, 4번째 · F04 마지막) — 01-04 나머지 범위 probe

- 대상: HeaInSeo/ieo_poc PR #14, head `a3d01ce` (branch `task/01-04-remaining`), base `main`
- 직전 검토: `c20e7e3` reject (P2 3건). 이번 diff는 `c20e7e3..a3d01ce`로, 778ab90·bc09fe4·a711441·a3d01ce 네 커밋이다.
- 검토일: 2026-11-12 (KST)
- 검토 트리: worktree `/opt/rust/src/github.com/HeaInSeo/ieo_poc-rem` (HEAD = a3d01ce, 깨끗함)
- 범위: 사용자 결정 기록, ADDENDUM §6·§7, REPORT 변경, E1b·AM 증거, probe2.py 변경(e1b, am, stop_signals, 묶음 정지 검사), 01-04 전체 수용 판단
- 하지 않은 것: 모델 단계 실행, 저장소 수정, 비밀 값 출력

**VERDICT: approve**

직전 P2 3건은 모두 해소됐다. 새 P2 이상은 없다. 남은 것은 P3 권고 5건이며 수용을 막지 않는다. 이 검토로 01-04를 수용한다.

---

## A. 확인 요청 항목

### 1. a711441이 시도 26보다 앞서는가 — 예

| 사건 | 시각(UTC) |
| --- | --- |
| a711441 커밋(사용자 결정 원문 + ADDENDUM §7 고정) | 08:05:34Z |
| 시도 26 예약·시작(`budget.json` n=26) | 08:05:44.949Z |
| 시도 27 종료 | 08:05:53.021Z |
| a3d01ce 커밋(결과) | 08:06:19Z |

- §7의 고정 부분(조건·기대·판정 규칙·시도 수)은 a711441 이후 바뀌지 않았다. a711441→a3d01ce 사이 ADDENDUM diff는 "결과" 절 추가뿐이다.
- 간격은 10초다. 짧지만 순서는 원본으로 확인된다.
- 다만 하네스 `cmd_am`은 a3d01ce에서야 커밋됐다(P3-1).

### 2. AM 판정이 고정 §7에서 나오는가 — 예

- 고정 규칙: init 사건의 `memory_paths`에 `auto`가 없으면 비활성 확인, 그대로 있으면 미비활성, 결과 사건이 없으면 관측 불가다. 각 1회, 재시도 없음.
- 원본
  - `attempts/26-AM1-development/stdout.jsonl`·`27-AM2-…`: `"memory_paths"` 문자열 0건이다. 결과는 `success`, `is_error` false, 답은 `ok-am`, rate_limit `allowed`다.
  - AM1의 `#ENV_NAMES`에 `CLAUDE_CODE_DISABLE_AUTO_MEMORY`가 있다. argv에도 `export CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`이 있다.
  - AM2의 argv에 `--settings /etc/ieo/role/settings-am.json`이 있다. 호스트 파일 내용은 `{"autoMemoryEnabled": false}`이고 0444다.
- 대조군: 같은 공통 인자(`--tools ''`)를 쓴 N1(시도 11)·R2(시도 25) stdout에는 `memory_paths`가 1건씩 있다.
- 따라서 두 방법 모두 **비활성 확인**이다. 판정은 고정 규칙 그대로다. ADDENDUM §7은 "init 표시 기준이고 행동 시험은 아님"이라는 한계와 이중 방어(정리 대상에 `projects/*/memory/` 포함)를 함께 적었다. 적절하다.
- 예산: 27/30, 벽시계는 첫 시도부터 약 2,073초로 7,200초 안이다. 사용자 결정의 "2회 이내"를 지켰다.

### 3. E1b가 좁혀진 E 주장을 뒷받침하는가 — 예

구성(`probe2.py` `cmd_e1b`, 778ab90 커밋, E1b.json `at` 07:49:47Z, 모델 요청 없음):
- forge 컨테이너가 UID 20005로 `git daemon`을 실행한다. repo 디렉터리는 20005 소유, 0700이다.
- forge는 내부 네트워크 `ieo-probe-forge`에만 붙는다. 개발 컨테이너는 그 네트워크로 붙고 repo를 마운트하지 않는다(argv의 -v 목록에 repo 없음).

| 행 | 기대 | 원본 |
| --- | --- | --- |
| work/e1b push | 허용 | `* [new branch] work/e1b`, PUSH_WORK_OK |
| main push / 강제 push / 삭제 | 거부 | 셋 다 `pre-receive hook declined`, `*_REJECTED` |
| 개발 컨테이너의 repo 파일·hook 접근 | 거부 | `/srv/a.git`, `hooks/pre-receive` ABSENT, forge 마운트 0 (`/srv`는 이미지의 빈 기본 디렉터리) |
| Guardrail의 forge 접속 | 거부 | 이름 해석 실패, REACH_DENIED |
| forge main 불변 | seed SHA | `refs/heads/main 73ea6690…` = `E.json` `repo_a_main` |

- 이제 보호 장치(hook·refs)는 막아야 할 주체 밖에 있다. 개발 역할이 바꿀 수 있는 것은 push 프로토콜로 보내는 ref 갱신뿐이고, main 갱신은 그 경로에서 거부된다. 직전 P2-2의 우회 경로(`update-ref`, hook 수정)는 닫혔다.
- 한계도 E1b.json `limits`와 ADDENDUM §6에 적혀 있다. git daemon에는 인증이 없어 네트워크 소속과 서버 측 hook이 권한을 정한다. 로컬 대역으로서 수용 가능하다.

### 4. 직전 P2 해소 여부

| 직전 지적 | 교정 | 판단 |
| --- | --- | --- |
| P2-1 U 중단 조건·결정 누락 | REPORT 머리말·§1·§9를 정정했다("해당 없음"이 틀렸음을 명시). ADDENDUM §6이 생겼다. `docs/poc/User_Decisions_2026-11-12_0104.md`에 "Edit 형식 채택"을 원문으로 남겼다(a711441) | **해소.** 결정 문구가 실제 사용자에게서 왔는지는 검토자가 증명할 수 없다. 기록과의 정합성만 확인했다: 질문 3건이 직전 검토 P2-1·P2-3과 PR #12의 토큰 해시 노출 처리(first-probes ADDENDUM §3, 0303f3e)에 정확히 대응하고, 결정 뒤의 행동(AM 2회, 토큰 미교체, 계약 개정은 별도 작업)이 결정과 일치한다. 결정을 사용자가 한 것으로 간주하는 근거는 이 기록뿐이라는 점을 남긴다 |
| P2-2 E1 hook 우회 | E1b를 추가하고 REPORT §1 E 행에 E1의 한계와 E1b를 명시했다 | **해소**(§A-3) |
| P2-3 자동 기억 | 위치 확인(init `memory_paths`, 두 역할 모두). 계약 반영 #3을 "`projects/<cwd>/memory/`도 정리하거나 끈다"로 정정했다. 비활성 방법을 사용자 결정에 따라 AM 2회로 확인했다(§A-2) | **해소** |

직전 P3도 대부분 반영됐다: N4 편차, G3b 템플릿, G2 라벨, U-alt-2 출처, 환경 변수, `stop_signals` 무결과 정지, 계획 외 3회, 비용 0.2655, T의 `modelUsage`, 허용 목록 관측(계약 반영 #6), ADDENDUM §5 암호어 목록.

### 5. 하네스 변경

- `stop_signals`: 결과 사건이 없고 종료 코드가 0이 아니면 정지한다. 결과 사건 없이 종료 코드 0인 경우는 여전히 통과하지만, 실무상 드물어 P3 수준이다.
- bc09fe4: 묶음 시도(C, N4)는 `finish(check_stop=False)`로 모두 기록한 뒤 `stop_batch`로 정지를 검사한다. 자동 검토 4547641003의 지적(정지 시 나머지 시도 기록 누락)을 정확히 고쳤다. 이번 실행은 정지 신호 0이라 소급 영향은 없다.
- 자동 검토 4547640995(G3가 자동 기억을 보존함)는 P2-3과 같은 지적이며 함께 처리됐다.

### 6. 비밀·PII

- 재검사: 역할 토큰 2개(전체값, 부분 문자열, SHA-256 전체), e-secrets 5개(값, SHA-256 전체), `oauthAccount`의 식별 필드 6개를 브랜치 7개 커밋의 전체 트리·커밋 메시지·작성자와 대조했다.
- 결과: PR이 바꾼 파일에서 0건이다. displayName·fullName은 main에 이미 있던 `docs/poc/01_plan_and_review/Independent_Review_*.md` 4개에서만 나오고, 이 PR은 그 파일들을 바꾸지 않았다.
- 결정 문서는 0303f3e 이력에 남은 해시 전체값을 "교체하지 않음"으로 기록했다. 고엔트로피 토큰의 SHA-256은 역산 위험이 낮다. 이 위험 수용은 사용자 소관이며 기록돼 있다.

---

## B. 남은 지적 (P3, 수용을 막지 않음)

- **P3-1 AM 하네스 코드는 실행 뒤에 커밋됐다.** `probe2.py` `cmd_am`의 docstring은 "frozen at a711441"이라고 하지만, a711441에는 문서만 있고 코드는 a3d01ce에서 들어왔다. 실행 인자(시도 26·27 argv)는 §7 표와 일치하므로 판정에는 영향이 없다. docstring을 "조건은 a711441에 고정, 코드는 a3d01ce"로 고친다.
- **P3-2 REPORT:126 기동 횟수.** "실제로 기동한 것은 24회"는 25회까지 기준이다. 27회 기준으로는 26회 기동(03 미기동)이다.
- **P3-3 PR 본문이 낡았다.** 본문의 예산(25/30)과 F05(8,256초)가 a3d01ce 기준이 아니다. 05:22:39Z→08:06:19Z 연속 9,820초로 계산해도 14,400초 안이지만, 본문을 갱신한다.
- **P3-4 REPORT §2 본문에 E1b가 없다.** E1b는 §1 요약 행과 ADDENDUM §6에만 있다. §2 표에 E1b 행을 넣고, E1 main 거부를 "push 경로 한정(E1b가 대체)"으로 표시하면 독자가 오해하지 않는다.
- **P3-5 E1b 범위 밖의 경우.** `HEAD` 같은 symref 대상 push나 다른 ref 이름 표기는 시험하지 않았다. hook은 `refs/heads/main` 정확 일치만 거부한다. 실제 보호는 GitHub 보호 브랜치(사용자 결정 대상)로 넘어가므로 기록만 권한다.

---

## C. 01-04 전체 수용 판단 — 수용

| 수용 기준 문구 | 근거 | 판단 |
| --- | --- | --- |
| 기본 시험 repo | E1(push 경로) + E1b(forge 소유, 우회 불가) | 충족. GitHub 시험 repo와 GitHub 역할 자격은 05-03/P07 전에 결정할 미결 항목으로 남는다(REPORT §2) |
| DB·수신기 | E2·E3 | 충족 |
| 역할별 허용/거부 | E·E1b, PR #12 probe 2 | 충족 |
| 무인 기동/권한 확인 | U(고정 인자 미지원) → 사용자 결정으로 `Edit(//path)` 채택 → U-alt 2회 지원, G1 | 충족(결정 기록 포함). 계약 개정은 별도 작업 |
| 이름↔세션 재개 | N1~N4 | 충족 |
| 기록 위치·압축 감지 | L1~L3, 자동 기억 위치·비활성(AM) | 충족 |
| 동시 세션 수 | C(4), PR #12 probe 1 | 충족 |
| 공유 quota group / 한도 분리 | PR #12 probe 3 공유 확인 | 충족(분리 성공으로 쓰지 않음) |
| 세션/자식별 토큰 | T | 충족 |
| 런타임 갱신 전후 재개 | R | 충족 |
| 첫 세 probe 순서·A1-GATE | PR #12 수용 | 충족 |
| 원본 로그·오류·영향·대안·재개 조건 | REPORT·ADDENDUM §1~§7·evidence | 충족 |
| 비밀 값 비기록 | §A-6 | 충족 |

중단 조건도 남은 것이 없다.
- G 불합격(계약 §4.1): 해당 없음.
- 필수 계약: U는 사용자 결정으로 해소됐고, N2는 지원이다.
- 한도·인증 신호: 0.
- 예산: 27/30, 벽시계 안.
- F05: 연속 계산 9,820초 < 14,400초.
- F04: 이번이 4번째 검토로 상한에 도달했지만 승인이므로 보류 조건은 생기지 않는다.

후속(01-04 수용과 별개):
- 계약 개정 6건(REPORT §1)
- GitHub 시험 repo·역할 자격 결정(05-03/P07 전)
- supervisor의 세션당 단일 실행 보장(N4)
- 자동 기억 끄기 설정을 RO 설정에 넣기
