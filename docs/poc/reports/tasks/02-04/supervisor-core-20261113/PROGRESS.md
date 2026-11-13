# 02-04 진행 기록 1 · 모델 호출 없는 supervisor 핵심(수용 아님)

| 항목 | 기록 |
| --- | --- |
| 구분 | 주계획 02-04의 **중간 산출물**. 02-04 수용 기준(런타임 계약 rev 6 §10 항목 1~5, rev 7 §11 항목 7)의 실제 세션 시험은 아직 하지 않았다. 완료 수 11/73 변동 없음 |
| 근거 | [런타임 계약 v0 rev 6/7](../../01-02/runtime-contract-20261112/Runtime_Contract_v0.md), [사용자 결정 2026-11-13 ①](../../../../User_Decisions_2026-11-13_x02.md) |
| 산출물 | `crates/ieo-supervisor`: `args`·`env`·`configdir`·`lock`·`checkpoint`·`budget` |
| PR | [#31](https://github.com/HeaInSeo/ieo_poc/pull/31): 8cc5c30 changes requested → 교정 1(260089f, CI 고침 7f06184) → 7f06184 changes requested → 교정 2 → bc55edf **approve**(검토 3/4·교정 2/3) → 조건부 병합 0f446d6 · main CI success. [#32](https://github.com/HeaInSeo/ieo_poc/pull/32)(검토 R1~R3 후속): 119ab95 changes requested → aeab3d9 **approve**(검토 2/4·교정 1/3) → 조건부 병합 3426c77 · main CI success |
| 모델 호출 | 0회. 02-04 예산(12회·14,400초·1.5M·15M) 사용 0 |

## 1. 한 것

| 모듈 | 내용 | 계약 |
| --- | --- | --- |
| `args` | 역할별 고정 인자. 파일 쓰기는 `Edit(//…)`만, `Write(` 없음. 재개 키는 session_id만. 개발 역할 CLAUDE.md 공급은 `--append-system-prompt-file`. Guardrail 규칙은 argv 한 칸에 하나 | §4.1, §10 항목 1·2·5 |
| `env` | 허용 목록 5개(HOME·PATH·LANG·CLAUDE_CONFIG_DIR·CLAUDE_CODE_OAUTH_TOKEN). `apply()`가 상속 환경을 비운 뒤 설정. `ANTHROPIC_API_KEY`는 거부(유료 API 전환 차단) | §4.3 |
| `configdir` | 지시성 항목 digest, 변조 탐지, 정리. 링크는 따라가지 않고 어디에 있든 변조로 본다. 특수 파일은 열지 않는다. 읽기 오류는 닫힌 실패(변조). 자격 파일 삭제를 기록. 템플릿은 `autoMemoryEnabled:false` 필수 | §4.1, §5-6, §10 항목 3 |
| `lock` | 세션당 단일 실행을 OS 파일 잠금(flock)으로 지킨다. 프로세스가 죽으면 커널이 푼다 | §5-5, §10 항목 4 |
| `checkpoint` | §5-1 기록과 재연결 결정(새 세션·재개·변조·낡은 세대·실행 중·외부 대조). 대조 항목: 배정·스프린트·revision·명세/이미지 digest·프로파일·열린 자식/외부 요청(§6) | §5-1·2·5·6, §6 |
| `budget` | 결정 ① 상한. 세지 못한 회차, 필드 누락, 넘침은 0이 아니라 중지. 추가 과금 없음 조건: 관측 stream의 모든 `rate_limit_event`가 `overageStatus:"rejected"`·`isUsingOverage:false`·`status:"allowed"`이고 7일 이내일 때만 시작, 아니면 그 호출만 보류. 실행 중 `stream_stop()` | 결정 ① |

시험 20건(모델 호출 없음). 저장된 01-04 실제 stream으로 추가 과금 없음 조건을 확인했다(`rejected`·`out_of_credits`·`isUsingOverage:false`).

## 2. 아직 하지 않은 것(02-04 수용 전)

- U3 Pod·PID 1 supervisor·런타임 어댑터 연결, 실제 `claude` 프로세스 기동.
- 역할 자격 유효성 확인(값 출력 없이). 첫 예산 호출과 함께 한다.
- 실제 세션 시험: §10 항목 1~5(Edit 인자 실행, CLAUDE.md 공급, (a) 정리·(b) 변조 탐지·(c) 정상 재개, 단일 실행, session_id 재개)와 §11 항목 7(새 세션→재개→`/compact` 재개 연쇄의 회차 몫 합 = 마지막 세션 누적).
- 02-04 `unresolved`의 이월 항목(계측기 판정 연결, elapsed/token 집행, `--secret-file` 전달 등).

## 3. 연결 단계에 넘기는 주의(검토 기록에서)

- 토큰 상한은 **회차 사이**에만 확인한다(지금까지의 소비 + 가장 큰 회차 ≤ 상한일 때 시작). 첫 회차는 시간 제한으로만 묶인다. 회차 도중 토큰 상한 정지는 어댑터 단계에서 `stream_stop()`과 함께 만든다.
- 역할 프로세스 기동(fork)과 세션 잠금 해제·재획득을 다른 스레드에서 동시에 하면, fork~exec 사이 자식이 잠금 기술자를 잠깐 공유해 해제된 잠금이 잠시 `Running`으로 보인다(안전 쪽 실패). 어댑터는 기동과 잠금 해제를 직렬화한다(PR #32 검토 P1·aeab3d9 F1).
- 잠금 기술자는 O_CLOEXEC라 `claude`에 상속되지 않는다. supervisor가 죽으면 잠금이 풀린다(Pod의 PID 1이므로 컨테이너 종료와 같다).
- 정리와 `observed()`는 역할 프로세스가 실행 중이 아닐 때를 전제로 한다.
- CLI 디버그 로그를 켜면 `debug/latest` 링크 때문에 모든 재연결이 변조로 판정된다(닫힌 실패). 디버그 로그는 끈다.
- 범위 밖의 읽을 수 없는 일반 파일을 열지 않는다는 점은 시험으로 고정하지 않았다(살아남은 변이, bc55edf R5).
- 시험의 chmod 000 사례는 비루트 실행을 전제로 한다(CI ubuntu-latest는 `runner`).

## 4. 검토 기록

- PR #31: [8cc5c30](reviews/review-pr31-8cc5c30.md) · [7f06184](reviews/review-pr31-260089f.md)(파일 이름은 교정 커밋, 검토한 head는 7f06184) · [bc55edf approve](reviews/review-pr31-bc55edf.md)
- PR #32: [119ab95](reviews/review-pr32-119ab95.md) · [aeab3d9 approve](reviews/review-pr32-aeab3d9.md)
