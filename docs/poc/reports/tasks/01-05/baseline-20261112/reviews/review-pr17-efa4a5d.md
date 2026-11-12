<!-- 보관본: PR #17 독립 검토 efa4a5d 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/17#issuecomment-6819395632 (머리말·끝 교정 안내 생략) -->
# PR #17 독립 검토 — 01-05 원본·기준선·운영 충돌 범위 기록

- 저장소: HeaInSeo/ieo_poc (public) · 브랜치 task/01-05-baseline · head `efa4a5d` · base main `cd19663`
- 검토자: Guardrail(독립 검토, 작성에 참여하지 않음) · 01-05 검토 1/4 · 2026-11-12 KST
- 수용 기준: poc-acceptance-v1.3.3 rev 4 `01-05.accept`
- 방법: 모두 읽기 전용. 모델 요청 0회. repo·GitHub·Notion·systemd에 쓰기 없음. 토큰·env 값·프롬프트·로그 본문은 읽거나 출력하지 않음.

VERDICT: reject

차단 사유는 P2 1건(현재 운영 기록의 사실 오류)뿐이다. 나머지 범위는 대부분 정확하고 정직하다. 수정량은 작다.

## 독립 확인 결과 요약

| 항목 | 결과 |
| --- | --- |
| 1. 고정 원본 | `gh api …contents/<path>?ref=e5075aff…` raw로 9개 문서 + `.github/workflows/ci.yml`을 다시 받아 sha256 전체를 대조했다. 10개 모두 `fixed-source-compare.txt`와 패키지 사본과 같다. tree blob 143개, 원격 main = `e5075aff…`. CI run 41469168311: push/main, head `e5075aff…`, attempt 1, 2026-11-02T15:11:40Z, completed/success. job 2개(Node 22·24 “tests and source provenance”) 모두 success. 기록과 일치한다 |
| 2. 조항 인용 | 17행 중 **16행**(1·2·3·4·5·6·7·8·9·10·11·12·13·14·16·17)의 `파일:줄`을 고정 커밋 원문과 줄 번호까지 대조했다. 원격 파일 포함(broker.cjs:224-229·242-244·355-357·455-458, policy.json:3-35, source_review_request.cjs:5-6·12-14·30·37-42, templates/central-supervisor.md·queue-watch.md, REVIEW_TIMEOUT_OPERATIONS.md:12-13·74-76·165-168, REVIEW_TIMEOUT_TRANSITION.md:97, deployment/review-timeout-operational.json:3). 어긋난 줄 번호와 잘못된 인용은 없다. 좁음/공백 판정(6·9·13·14·16·17)은 모두 원문과 맞다. 행 15의 분류만 P3-1 참고 |
| 독립 기대값 | 대응표가 새 기대값을 만들지 않는다. 행 1 “Notion 쓰기 자격·호출 0”은 계획 P21 통과 기준 문구다. 행 12의 30/60분·대체 1회는 계획 :328의 “제안값, T01에서 확정”을 그대로 표시했다. 행 13의 F06 값은 freeze_register `stage_0[5]`의 동결값(1800s·1800s·10회, provider_signal_first)과 같다. 행 14의 “T01·T05에서 고정”은 계획 :764와 같다. 나머지는 모두 L03(담당: 개발 맥락 없는 독립 담당, 시점: 해당 실행 전)으로 연결했다. 적절하다 |
| 행 구성 | 계획 §1.1의 15행과 처분·검증 P가 일치한다. P21 원본 영역 4개 중 2개(역할 지침, 브로커 공통 권한)를 16·17행으로 추가했고, 나머지 2개(source_review_request·timeout, Notion 브로커·헬퍼)는 6·12·1·2행에 흡수했다. 타당하다 |
| 3. 현재 운영 | timer 목록·schedule(07분/37분), unit의 Description/ExecStart, tick 스크립트 WORKDIR(`/opt/go/src/github.com/HeaInSeo`, 각 17행), 줄 수와 sha256-12(224cef2ee979·6a19c5eb0ed9), claude/codex 프로세스 8개의 cwd, NPG 작업 사본(HEAD `0d03bf6…`, 브랜치 fix/sori48-approved-unknown-closure-20261029, 미커밋 0, `merge-base --is-ancestor` rc=1, 38/2), runner(`actions.runner.HeaInSeo-mac-k8s-multipass-terraform.rocky-baremetal-01`, User=ghrunner, ieo_poc runner 0개), podman 이름(ieo-probe-pg/rx/forge exited, local-registry created, 네트워크 ieo-probe-coord/forge)을 확인했다. 모두 일치한다. **예외: report bridge 상태(P2-1)** |
| 4. quota 겹침 | 01-04 P3-identity.json에서 development·guardrail 둘 다 `sub==accountUuid: true`, `aud==organizationUuid: true`이고 두 역할의 claims 비교도 모두 same이다. tick 스크립트와 unit에는 `CLAUDE_CONFIG_DIR`·`CLAUDE_CODE_OAUTH_TOKEN`·`ANTHROPIC_API_KEY`·`EnvironmentFile` 이름이 없고(값은 보지 않음), 실행 파일은 `$HOME/.local/bin/claude`다. 따라서 “기존 tick도 호스트 로그인을 쓴다”는 주장은 근거가 있다. “기술적으로 분리할 수 없으니 D1에 따라 지원 구성으로 진행, B2·L01 한도, 04-15 측정”이라는 처리는 User_Decisions_2026-11-12_0103.md D1과 일치하고 정직하다 |
| 5. 고정 원본을 현재 증거로 쓰지 않음 | Source_Baseline 머리말, CI의 “뜻하지 않는 것”, 원격 main 동일성의 부정, NPG 작업 사본의 비조상 실측, 조사 원본 (b)의 과거 스냅샷 목록으로 잘 지켰다. 사소한 문구 1건(P3-4) |
| 6. 위생 | 토큰·JWT·이메일·UUID 패턴은 없다. 32자리 hex 9개(Notion 페이지·automation ID)는 모두 public NPG 저장소의 고정 커밋에 있는 값이다. `/home/heain/...` 경로도 NPG 공개 원문에 있다. 반복 게시는 수용할 만하다. source_review_request.cjs의 SESSION UUID는 옮기지 않았다(좋음). 새로 공개되는 호스트 세부는 P3-3 참고. `validate_package.py` rc=0, `render_progress.py --check` ok(`.venv-poc/bin/python -I`, docs/poc에서 실행). PR CI 3개 SUCCESS @efa4a5d. 작업 트리 변경 없음 |

## 지적 사항

### P2-1 · report bridge timer를 “비활성”으로 기록했으나 실제로는 enabled·active(elapsed)다. 겹침 분리에서 빠졌다

- 위치: `docs/poc/reports/tasks/01-05/baseline-20261112/Current_Operations_20261112.md:13`(“`claude-report-bridge.timer` | 비활성(마지막 2026-10-13) | 없음(현재)”), `REPORT.md:14`(“report bridge 비활성”), `result.json:26`(“report bridge 비활성”). 근거 파일 `evidence/current-operations.txt:6`은 `list-timers` 한 줄(NEXT n/a)뿐이며, enabled나 active 상태는 기록하지 않았다.
- 독립 확인(읽기 전용 `systemctl --user show/is-enabled`):
  - timer: `UnitFileState=enabled`, `ActiveState=active`, `SubState=elapsed`, `NextElapseUSecMonotonic=infinity`, `LastTriggerUSec=Tue 2026-10-13 07:23:19 KST`
  - timer 정의: `OnBootSec=1min`, `OnUnitActiveSec=5min`, `Persistent=true`, `WantedBy=timers.target`
  - service: `inactive/dead`, 이번 manager 수명에서 실행 기록 없음
- 의미: 지금 발화 예정이 없다는 점(NEXT n/a)은 맞다. 그러나 unit은 비활성화(disabled)된 것이 아니다. 활성화된 timer가 다음 계산 시점이 없는 elapsed 상태에 머물러 있을 뿐이다. 호스트 재부팅이나 user manager 재기동 뒤에는 `OnBootSec=1min`으로 다시 시작하고 5분 주기로 돌 수 있다. PoC 기간(01-06 초기화, 장기 실험) 중 재부팅은 충분히 일어날 수 있다. 그러면 Operator/Guardrail 보고를 스캔하는 actor가 예고 없이 다시 살아난다.
- 수용 기준 “현재 운영은 별도로 확인”에 비추면 현재 actor 하나의 상태를 실제와 다르게 기록한 것이다. §2 겹침 표도 이 actor의 재개 가능성을 다루지 않는다.
- 수정 제안:
  - (a) 근거 파일에 `is-enabled`와 `show -p UnitFileState,ActiveState,SubState,NextElapseUSecMonotonic,LastTriggerUSec` 결과, timer의 `On*`/`Persistent` 줄을 추가한다.
  - (b) 표의 상태를 “enabled · active(elapsed) · 다음 실행 없음 · 마지막 2026-10-13 · 재부팅 시 OnBootSec로 재개 가능”으로 고친다.
  - (c) §2에 처리를 한 줄 추가한다. 예: PoC는 재부팅과 user manager 조작을 하지 않는다. 01-06·각 실험 전에 이 timer 상태를 다시 확인한다. 외부 효과는 현재 미관측이다.
  - 같은 정정을 REPORT.md:14와 result.json:26에도 반영한다.

### P3-1 · 행 15가 “직접 근거”로 집계됐지만 내용상 “이름보다 좁음”이다

- 위치: `Policy_Responsibility_Map_v0.md:31`(행 15 공백·주의: “사람용 내용 정의가 없다”), `:39`(집계 “직접 11행”에 15 포함), `REPORT.md:28`, `result.json:45`(L03 인계 목록 6·9·13·14·16·17에 15 없음)
- 근거: README.md:164-165와 AUTOMATIC_FOLLOWTHROUGH.md:228-229는 이 페이지들을 NPG가 쓰는 Notion 대상으로 나열할 뿐이다. 사람이 보는 현황의 내용(현재 작업·대기 이유·다음 담당·최근 증거, 계획 §1.1)을 정의하지 않는다. 조사 원본 :29도 “No clause calls them human-facing or describes their content”라고 적었다.
- 영향: 행 자체의 주의 문구는 정직하다. 다만 집계와 L03 인계 목록에서 빠져 있어, P12·P13·P26 기대값 담당이 원본 근거가 있다고 오해할 수 있다.
- 제안: 행 15를 “좁음”으로 재분류한다(직접 10, 좁음 4). 인계 목록에도 넣는다.

### P3-2 · 겹침 분리가 한 방향만 다룬다(기존 actor → PoC 저장소)

- 위치: `Current_Operations_20261112.md:33`(GitHub 계정 공유 행)
- 내용: “PoC가 다른 저장소에 쓰지 않는다”는 방향만 적었다. 반대 방향은 다루지 않는다. 기존 Operator·Guardrail tick과 장기 claude/codex 세션은 같은 GitHub 사용자 자격(`gh`)으로 돌고, 다른 저장소의 PR 검토 대기도 처리한다고 기록했다. 이들이 `HeaInSeo/ieo_poc`·`ieo_poc_test`에 댓글·검토·병합을 낼 가능성은 언급이 없다.
- 영향: P21의 “범위 밖 효과 0건” 같은 계수가 외부 actor의 효과로 오염될 수 있다.
- 제안: 한 줄을 추가한다. 예: 기존 actor는 Notion 큐로 대상을 고르며 ieo_poc 계열은 등록되지 않았다(근거가 있으면 인용, 없으면 “미확인”). PoC 시험 계수는 PoC 역할 자격·actor 식별로 필터한다.

### P3-3 · 위생: 필요 이상의 호스트 세부가 공개 저장소에 새로 들어간다

- 위치: `evidence/current-operations.txt:39-42`(`/home/heain/openai-dc-additional-manager/manage.py _serve`, `openclaw … gateway --port 18789`, `openclaw-gateway (v2026.3.13)`), `:63`(같은 경로의 codex cwd), `:74`(runner agentName)
- `ieo_poc@cd19663`에서 `git grep`으로 찾은 결과와 NPG 고정 커밋 원문에는 이 값들이 없다. 즉 이번 PR에서 처음 공개된다. 비밀은 아니다. 그러나 원격 제어 게이트웨이의 버전·포트·관리 경로는 01-05 목적(겹침 분리)에 필요하지 않다.
- 제안: 서비스 이름과 “원격 명령 경로, PoC 미사용” 정도만 남긴다. ExecStart·포트·버전·관리 경로는 지운다.
- Notion 페이지·automation ID 9개와 `/home/heain/.local/...` 경로는 public NPG 원문에 이미 있으므로 그대로 두어도 된다. 다만 조사 원본 (b)의 “Notion pages named” 목록은 분석에 꼭 필요하지 않으니, 줄여도 무방하다.

### P3-4 · 고정 문서의 서술을 현재형으로 옮긴 문구, 근거 파일에 없는 관측

- 위치: `Policy_Responsibility_Map_v0.md:33`(행 17 “배치된 정책은 비공개이고 저장소 사본과 다르다(OPERATIONS.md:29-31)”)
  - 고정 문서가 당시 배포 상태를 서술한 것을 현재형 사실로 적었다. “고정 문서 기준(당시): 운영 정책은 비공개이며 저장소의 policy.json(productionEnabled:false)을 운영 파일에 쓰지 않는다”로 시점을 표시하면 원칙 5와 일관된다.
- 위치: `Current_Operations_20261112.md:11-12`(“Notion 일정·영수증 페이지 기록(tick 로그)”, “다른 저장소 PR의 검토 대기 처리(tick 로그)”), `:32`(“사용량 게이트(주간)”)
  - `evidence/current-operations.txt`에는 TICK_END 시각·rc만 있고, 이 주장들의 근거 줄이 없다. 이번 검토에서 사용량 게이트 스크립트에 weekly 관련 키워드가 있는 것은 이름 수준으로 확인했다.
  - REPORT.md:24는 로그 본문을 읽지 않았다고 한다. 그렇다면 이 주장들의 출처(어떤 패턴을 셌는지 등)를 근거 파일에 남기거나, “문서/구성 근거”로 바꿔 추적 가능하게 한다.

## 판정

- 고정 원본·CI·설계 버전 기록: 충족. 재조회와 sha256이 일치한다.
- 조항 출처·검증 범위·독립 기대값·공백 연결: 충족. 인용 정확도가 높다. 기대값을 새로 만들지 않았고, 미채택 후보(행 12)와 미배치 lease(행 4)도 정직하게 표시했다. P3-1은 분류 보정이다.
- 현재 운영 별도 확인: **미충족(P2-1)**. report bridge 하나를 제외한 모든 주장이 독립 실측과 일치한다.
- 겹치는 외부 효과 분리: quota 공유의 근거(P3 sub/aud)와 처리가 타당하고 정직하다. P2-1의 actor 누락과 P3-2의 역방향은 보완이 필요하다.
- 고정 원본을 현재 설치·소유권 증거로 쓰지 않음: 충족(P3-4 문구 1건).
- 위생과 도구 검사: 통과. P3-3은 권고다.

P2-1을 고치면(근거 파일 보강과 세 문서의 문구 정정, §2에 처리 한 줄) 승인할 수 있다. P3는 같은 수정에서 함께 반영하길 권한다.
