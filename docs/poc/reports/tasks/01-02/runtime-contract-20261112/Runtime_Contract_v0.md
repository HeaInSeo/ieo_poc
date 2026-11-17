# 01-02 · 에이전트 런타임·역할·인증·재개 계약 v0

2026-11-12 KST · 계획 v1.3.3 · 이전 회차 [runtime-comparison-20261112](../runtime-comparison-20261112/REPORT.md)(후보 비교)를 이어받는 회차.
문서 **revision 20**(2026-11-17): 02-05 정산 PR([재개안 부록 B](../../02-05/step8-plan-20261117/RESUME_PLAN.md#부록-b-정산-형식ieo-role-run-settlementv0), [사용자 결정 2026-11-17 정산](../../../../User_Decisions_2026-11-17_r1-settlement.md))에서 새 §21 항목 32를 더했다: 가드가 당시 규칙의 경고로 멈춘 회차를 CLI의 종료 집계로 정산하는 별도 장부 기록(`ieo-role-run-settlement/v0`)과, 그런 회차에만 한정한 예비 해제. 다른 조항은 revision 19와 같다. revision 19(2026-11-17): 02-05 경고 정책 PR([재개안 부록 A](../../02-05/step8-plan-20261117/RESUME_PLAN.md#부록-a-한도-경고-판정의-근거-정책-전후-시험), [사용자 결정 2026-11-17 경고](../../../../User_Decisions_2026-11-17_warning.md) (다))에서 한도 신호 판정을 하나로 정했다(새 §20 항목 31). **뜻이 바뀐 동작이 하나 있다**: 구독 상태 `allowed_warning`은 추가 유료 사용이 꺼져 있음(`overageStatus: rejected`, `isUsingOverage: false`, 둘 다 있음)이 확인되면 더는 멈춤이 아니고 기록되는 경고다(실행 가드, 사전 관측 검사, 공유 예산 검사, 회차 기록, 상태 표시, 실행 스크립트에서 모두 같게). §13 항목 9의 `available`·`unknown` 문장을 그 판정에 맞췄다. 다른 조항은 revision 18과 같다. revision 18(2026-11-17): 02-05 단계 8 전 PR([보고](../../02-05/pre-step8-20261117/REPORT.md))에서 두 문장을 고쳤다. §15 항목 15: 직접 첫 행동 보고(`recordFirstAction`)도 채움(항목 17)과 같은 조건, 즉 그 세션의 takeover가 그 시도를 부른 전달에 연결돼 있어야 받는다(PR #75 검토 debf413 I-8). §19 항목 30: 예비의 `approved_at`은 사람이 보낸 `deliverAssignment` 요청을 코디네이터가 처리한 시각이다(PR #81 검토 291a9b0 N-3). 다른 조항은 revision 17과 같다. revision 17(2026-11-17): 02-05 예산 코드 PR([보고](../../02-05/budget-0205-20261117/REPORT.md))에서 02-05 예산의 예비 표식([사용자 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md) (나)와 그 추가 결정)을 새 §19 항목 30으로 적었다. 표식은 공유 예산 검사와 예산 장부의 것이고 역할 서비스로 보내지 않는다(PR #81 검토 b38e8df I-4). 역할 서비스의 계약과 다른 조항은 revision 16과 같다. revision 16(2026-11-17): 02-05 예산 코드 PR([보고](../../02-05/budget-0205-20261117/REPORT.md))에서 §18 항목 27·28의 문장을 보강했다. 끝나지 않은 회차 위의 takeover가 대는 복구는 바로 그 체크포인트(순번)와 그 배정·세대의 승인이어야 한다(PR #80 검토 d336849 L-a, 코드도 그렇게 고침). 사용 뒤 풀리지 않는 두 경우(받았으나 실행되지 않은 start의 `unknown`, takeover가 거절된 `accepted`)를 적었다(L-c). 다른 조항은 revision 15와 같다. revision 15(2026-11-17): 02-05 단계 7b(공식 복구, [사용자 결정 2026-11-16 (나)](../../../../User_Decisions_2026-11-16_recovery.md))를 새 §18 항목 25~29로 반영했다. 끝나지 않은 체크포인트(takeover·error) 위에서는 지금까지 재개도 새 세션도 없었고 충돌 뒤 복구 경로가 없었다(PR #74 검토 fe924b7 I-4). 이제 **사람**(`user` 자격, `roles:control`)이 그 체크포인트 하나에 대한 새 세션을 승인하고, 전달이 그 승인을 한 번 쓴다. **뜻이 바뀐 동작이 하나 있다**: supervisor의 start는 로컬 체크포인트가 끝나지 않은 회차이면 `recovery_id` 없이 거절한다(`local_only` 포함). 체크포인트에 `recovery_id`가 늘었다. 체크포인트에 브랜치가 없으면 `pushed_sha`를 읽지 않는다는 항목 22의 문장을 코드에 맞췄다(검토 47e4e3f L-4). 바꾼 곳은 §17 항목 22의 문장 하나와 새 §18이고, 다른 조항은 revision 14와 같다. revision 14(2026-11-16): 02-05 단계 7(GitHub SHA·CI 대조를 재개의 최종 근거로, [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 7, C10)을 새 §17 항목 21~24로 반영했다. **뜻이 바뀐 필드가 하나 있다**: 체크포인트의 `worktree_head`는 지금까지 늘 `"none"`(`tracked.worktree_head: false`, 관측 아님)이었다. 이제 작업 디렉터리의 `HEAD` SHA(읽을 수 없으면 `"not_observed"`)이고 `tracked.worktree_head`는 `true`다. 체크포인트에 `branch_ref`(배정 브랜치)와 `pushed_sha`(그 브랜치에 push된 SHA, 없으면 null)가 늘었다. 개발 역할의 재개는 이제 코디네이터가 증거 입력으로 받은 GitHub 브랜치 head가 그 push SHA(없으면 배정 `target_sha`)와 같을 때만이다(다르면 외부 대조, 기록이 없으면 `not_observed`). 같은 세대의 명시적 새 세션도 `resume_allowed`에서 고르므로 같은 대조를 거친다(새 세대의 새 세션과 `no_checkpoint`의 start는 아니다). Guardrail은 검토 대상 SHA가 현재 개발 배정의 `target_sha`가 아니면 아무것도 실행하지 않는다(`stale_target`). 단계 6 항목 18에 받기 전 목록 확인(PR #78 검토 6e0ad85 L-a)과 보관본 전송이 목록 밖이라는 문장(I-c)을 더했다. 바꾼 곳은 §16 항목 18과 새 §17이고, 다른 조항은 revision 13과 같다(§5-1·§5-2·§13 항목 10의 규칙은 §17이 덧붙인다). revision 13(2026-11-16): 02-05 단계 6(외부 요청 추적, [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 6)을 새 §16 항목 18~20으로 반영했다. **뜻이 바뀐 필드가 하나 있다**: `pending_external_requests`는 지금까지 상태에서 `"not_tracked"`, 체크포인트에서 빈 목록(`tracked.pending_external_requests: false`, 관측 아님)이었다. 이제 supervisor가 코디네이터에 보낸 요청 중 답을 모르는 것의 **실제 목록**(요청 id·종류·보낸 시각·대상)이고 체크포인트의 `tracked` 값은 `true`다. 모델이 직접 낸 네트워크 요청은 `direct_network_requests: "not_observable"`로 따로 보고하고 “없음”으로 쓰지 않는다. 목록이 비어 있지 않으면 reconnect는 외부 대조로 멈춘다(§5-1·§6 그대로). 코디네이터의 repo 범위 정지 보고는 결과를 모르는 전달도 사유로 적는다. `active_children`은 04-05까지 `not_tracked`라 안전 종료는 계속 미확인이다(사용자 결정 2026-11-15 항목 3). 바꾼 곳은 §7 status·stop(repo 범위) 행의 참조와 새 §16이고, 다른 조항은 revision 12와 같다. revision 12(2026-11-16): 02-05 단계 4(인수·첫 행동 연결, [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 4)를 새 §15 항목 15~17로 반영했다. 체크포인트에 필드 셋을 더한다: `delivery_id`(그 회차가 답하는 코디네이터 전달), `attempt_id`(그 전달이 부른 시도, PR #75 검토 5dbb3d8 M-1)와 `first_action`(회차의 첫 실제 행동, 없으면 `"none"`). 첫 행동의 정의(`init` 다음의 첫 `assistant` 사건 중 `tool_use` 또는 비지 않은 텍스트가 있는 것), after_run 체크포인트가 같은 값을 싣는다는 것, takeover가 전달에 묶인다는 것이다. 모두 추가이고 기존 필드·조항의 뜻은 바뀌지 않는다. 다만 코디네이터가 전달 없는 takeover를 거부하므로, 중앙 기록이 코디네이터인 supervisor에 전달 없이 직접 보낸 start·reconnect(02-04 호스트 호출자 경로)는 실행되지 않는다(§15 항목 16). 상태의 `last_actual_action_at`은 관측한 첫 행동의 시각이다(회차 끝 시각이 아니다, C14). 바꾼 곳은 §5-1 문장 하나의 참조, §7 status 행의 참조, 새 §15이고, 다른 조항은 revision 11과 같다. revision 11(2026-11-15): 02-05 단계 3(배정 전달, [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3 단계 3)을 새 §14 항목 11~14로 반영했다. start·reconnect는 같은 `attempt_id`·같은 내용이면 저장된 답을 돌려주고 새 실행을 하지 않는다(§7 오류 대응의 “같은 request_id·다른 내용만 충돌”에 코드를 맞춘 교정이며 뜻 변경이 아니다), supervisor는 시작 전 요청의 코디네이터 기록을 코디네이터의 현재 배정과 대조한다, 코디네이터가 배정 전달로 start·reconnect를 부른다(예산 검사는 컨테이너 밖 공유 라이브러리), 지시성 digest는 파일(과 링크) 기준이라 빈 디렉터리는 대상이 아니다. 재개 판단 하나도 고쳤다(PR #74 검토 c11e734 M-1): 같은 배정의 낮은 세대라도 마지막 기록이 `after_run`이 아니면 `stale_generation`이 아니라 `external_reconciliation_required`다(끝나지 않은 회차는 새 세션으로도 덮지 않는다). 바꾼 곳은 §7 오류 대응표의 한 행, §13 항목 10의 판단 문장 하나, 새 §14이고, 다른 조항은 revision 10과 같다. revision 10(2026-11-15): X-08 사용자 결정([2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md))과 X-08 교정을 §13 항목 10에 반영했다. 읽을 수 없거나 객체가 아닌 대기 변조 파일은 새 탐지가 없으면 start·reconnect를 거절한다(사람이 대조), 대기 사본 채택은 코디네이터의 `tamper_recorded`가 참일 때만 보고로 친다, start가 변조를 탐지하지 않는 것은 알려진 한계다. 다른 조항은 revision 9와 같다. revision 9(2026-11-15): 02-04 수용 결정 항목 4·9([사용자 결정](../../../../User_Decisions_2026-11-15_0204-acceptance.md))를 §13 항목 9·10으로 반영했다. 공급자 상태 값 대응과 `quota_group_id` 명시 설정, 중앙 체크포인트 보고·대조 절차다. 바꾼 곳은 §7 status 행의 참조와 새 §13이고, 다른 조항은 revision 8과 같다. revision 8(2026-11-15): 02-04 실제 회차 D3에서 확인한 자식 사용량 집계 규칙 1건을 §12 항목 8로 반영했다([사용자 결정](../../../../User_Decisions_2026-11-15_d3.md)). 다른 조항은 revision 7과 같다. revision 7(2026-11-12): 02-01 계측기가 01-04 원본 stream을 재생해 확인한 사용량 집계 규칙 1건을 §11 항목 7로 반영했다. 바꾼 곳은 §4.2 자식 사건 귀속 행·§5-1·§6 기록과 새 §11이고, 다른 조항은 revision 6과 같다. revision 6(2026-11-12): 01-04 실측(검수 완료, PR #12·#14)과 사용자 결정([01-04 결정](../../../../User_Decisions_2026-11-12_0104.md))을 §10 표의 6개 항목으로 반영했다. 바꾼 곳은 §2.1·§4.1·§4.2·§5·§7·§8과 새 §10이고, 다른 조항은 revision 5와 같다. revision 5(2026-11-12, [개정 v1.3.3-A1](../../../../Amendment_v1.3.3-A1.md)): 사용자 결정 D1·D2로 §3 자격 방식, §8 판정표, §9 미확정 목록만 바꿨다. 다른 조항은 revision 4(PR #2 병합 커밋 76ffd03에서 검수 완료)와 같다. revision 4: 독립 검토 7a39452(reject) F1~F9, bf854bb(reject) N1~N7, 2e5a5ef(reject) R1~R4 반영. 이 문서는 **설계 계약**이다. “근거”는 설치 버전 도움말/상태 출력에 옵션·문구가 **존재**함을 뜻하며, 동작은 01-04 probe 전까지 미확인이었다. revision 6에서 01-04 실측 결과를 반영했다(§10, 아래 각 조항의 “01-04:” 문장).

## 1. 선택과 적용 범위

| 항목 | 고정값 |
| --- | --- |
| 사용자 선택 | **Claude Code CLI 우선 검증** (2026-11-12 대화, 사용자 원문 “01-02는 기존 권고대로 Claude Code CLI를 우선 검증 대상으로 선택할게.”) |
| 대안 | Codex CLI(이 서버 설치 `codex-cli 0.166.0`, [codex-version.txt](evidence/codex-version.txt)). 01-04에서 필수 계약을 못 채우면 영향·전환 결정을 요청한다. 임의 전환 금지 |
| 적용 런타임 버전 | Claude Code `2.1.322` ([claude-version.txt](evidence/claude-version.txt)). 버전이 바뀌면 이 계약의 근거 표를 재확인하고 01-04 “런타임 갱신 전후 재개” probe 대상이 된다 |
| 두 역할 | 개발(`development`)·Guardrail(`guardrail`). 같은 런타임을 각각 격리해 쓴다. 세션·자격·볼륨·설정을 공유하지 않는다 |
| 제외 | 이 대화를 수행하는 Claude Code 세션(PoC 개발 도구)은 제품 런타임 인스턴스가 아니다. 그 자격·세션·환경 변수를 역할 프로세스·probe에 넘기지 않는다(§4.3) |

## 2. U3 배치 계약 (repo당 두 Pod, 각 컨테이너 하나)

| 항목 | 개발 Pod `<repo>-dev` | Guardrail Pod `<repo>-guardrail` |
| --- | --- | --- |
| 라벨 | `ieo.repo=<repo>`, `ieo.role=development` | `ieo.repo=<repo>`, `ieo.role=guardrail` |
| 컨테이너 | 1개. PID 1 = repo supervisor(development) | 1개. PID 1 = repo supervisor(guardrail) |
| 실행 UID/GID | 20001/20001 (root 아님, 추가 capability 없음) | 20002/20002 |
| 역할 전용 영속 볼륨(RW) | `ieo-<repo>-dev-state` | `ieo-<repo>-guardrail-state` |
| 볼륨 배치 | `/state/home`(HOME) · `/state/claude`(`CLAUDE_CONFIG_DIR`, 세션 기록) · `/state/work`(worktree) | 같은 구조, 별도 볼륨 |
| 읽기 전용 마운트 | `/etc/ieo/role/`(supervisor 제공 설정·MCP 목록·자식 정의), `/run/ieo/cred/`(Secret) | 같은 구조, 자기 역할 것만 |
| 공유 볼륨 | 없음. 상대 역할 볼륨·Secret·설정 마운트 금지 | 없음 |
| GitHub 자격 | 대상 repo 작업 브랜치 push·PR 생성만 | push 없음. 고정 SHA 읽기 |
| 같은 호스트 초기 단계 | podman 컨테이너 2개(위 값 동일), 볼륨 2개 | |

코디네이터는 별도 단일 활성 컨테이너(UID 20000)다. supervisor는 자기 Pod의 역할 하나만 start/reconnect/stop/status 하고 상대 Pod·컨테이너 런타임 admin 권한을 갖지 않는다. 배정·판정·정책 승인 권한도 없다.

### 2.1 같은 컨테이너 안의 supervisor와 에이전트(알려진 한계)

U3의 “Pod당 컨테이너 하나”와 S3의 PSA Restricted(추가 capability 금지)에서는 PID 1 supervisor와 에이전트 프로세스를 **다른 UID로 나눌 수 없다**. 같은 UID의 프로세스는 supervisor의 파일·환경(모델 자격, 코디네이터 claim 자격)을 읽을 수 있다고 가정한다. 따라서 경계는 “그 컨테이너 안에서 **누구의 코드가 실행되는가**”로 둔다.

- **Guardrail 컨테이너에서는 검토 대상(개발이 작성한) 코드·테스트·빌드 스크립트·hook을 실행하지 않는다**(검토 N1). Guardrail 컨테이너에서 실행되는 코드는 런타임·supervisor·Guardrail 모델의 도구 호출뿐이다.
- **Guardrail의 독립 재현·재실행은 격리 실행 경로로 한다**(검토 R1, P07의 “정상 CI에 드러나지 않는 고정 반례 재현”, P20/P24의 독립 재실행, P22의 역할별 쓰기 가능한 빌드/시험 공간): Guardrail은 자기 쓰기 공간 `/state/guardrail-scratch/`에 반례 입력·시험 파일·명령을 작성하고 **재현 요청**(claim: 대상 SHA, 패치/파일 묶음 digest, 명령, 기대값)을 코디네이터에 제출한다. 코디네이터는 **자격·Secret이 없는 일회성 격리 실행기**(코디네이터 소유, 대상 SHA 체크아웃+요청 묶음, 네트워크 기본 차단, 시간/자원 상한)에서 실행하고 원본 로그·종료 코드·실행기 이미지 digest를 Guardrail 입력 위치로 돌려준다. 필수 검사 결과는 CI/코디네이터 실행기가 보호된 경로로 발행한 것을 고정 SHA 기준으로 받는다(§1.2 “완료와 필수 검사”). 이 경로의 API는 01-08 계약에 추가하고, 실행기 구현은 02-06, 실제 검증은 P07(04-07)에서 한다.
- 개발 컨테이너는 개발 코드를 실행한다. 그 안의 자격(개발 모델 자격, 개발 claim 자격, push 자격)은 개발 코드에 노출된다고 보고 **그 범위가 개발 역할의 권한과 같도록** 둔다. 개발 컨테이너에서 Claude 권한 규칙은 보안 경계가 아니다. 경계는 컨테이너·자격 범위·코디네이터 판정이다. 01-04에서 실측으로 확인했다(§10 항목 6): `Bash(sleep:*)`만 허용했는데도 읽기 전용 복합 명령(`while pgrep …; echo done`)이 거부 없이 실행됐다. 그러므로 Bash 허용 목록을 판정·격리 근거로 쓰지 않는다.
- **push 시점 제한(§1.9, 검토 R2)**: push 자격은 코디네이터가 배정 단계가 `개발` 또는 반려 뒤 `교정`일 때만 발급하는 **단기 GitHub App 설치 토큰**(대상 repo 하나, contents/pull_requests 쓰기, 만료 ≤ 1시간)이며 `REVIEW_PENDING` 진입 시 폐기한다. 토큰은 브랜치를 한정하지 못하므로 main은 보호 설정(PR 필수·필수 체크·관리자 포함)으로 막고, 그 밖의 브랜치는 **탐지**로 다룬다: 코디네이터가 단계 밖 head 변경을 관측하면 정책 위반을 기록하고 검토 배정을 무효화·재배정하며, 병합은 승인 head SHA 서버 측 조건부 병합으로만 한다. 이 방식의 실제 거부·탐지는 05-03/P07에서 검증한다.
- 개발과 Guardrail은 **직접 통신하지 않는다**(§1.2). 서로의 네트워크 주소·자격·볼륨을 받지 않으며 Guardrail 입력은 승인 명세·프로파일·GitHub 고정 SHA·보호된 검사 결과뿐이다.
- **판정·체크포인트의 정본은 코디네이터 DB와 외부 상태(GitHub SHA·CI)**다. supervisor는 체크포인트 digest·순번을 코디네이터에 보고하고, 재개 시 코디네이터 기록·외부 상태와 다르면 `external_reconciliation_required=true`로 멈춘다. 개발 컨테이너의 체크포인트는 개발 코드가 바꿀 수 있으므로(검토 N5) 무결성을 보장하지 않으며, 재개 판단은 GitHub의 브랜치/SHA·CI 기록과의 대조로 한다.
- 코디네이터→supervisor 어댑터 호출은 코디네이터 전용 자격으로 인증하며 supervisor는 그 검증 키(공개 키/토큰 해시)만 가진다.
- 이 경계의 실제 위반 시도(개발 코드가 Guardrail 자격 획득·Guardrail 명의 발행 시도, 0건이어야 함)는 P07(04-07)에서 검증한다. Pod 분리 선언만으로 독립성 PASS를 주지 않는다.

## 3. 인증 계약

설치 관측([claude-auth-status.redacted.txt](evidence/claude-auth-status.redacted.txt)): 현 사용자에 **claude.ai 로그인 1개**(authMethod `claude.ai`, subscriptionType `max`, configDirectory `~/.claude`)만 있다. API key·별도 계정은 관측되지 않았다. `CLAUDE_CONFIG_DIR`을 빈 디렉터리로 지정하면 이 로그인이 보이지 않는다는 관측은 검토 세션의 미커밋 기록뿐이므로 01-04에서 원본 로그로 확인한다.

계약(방식과 무관하게 고정):

1. 역할마다 **별도 자격**을 자기 Secret(`/run/ieo/cred/`)으로만 받는다. 호스트 `~/.claude`·현재 대화 세션의 자격(D)은 금지한다.
2. 자격 획득·교체·폐기는 사람 정책 소유자(사용자)가 한다. 런타임/에이전트는 로그인·토큰 발급을 하지 않는다. 비밀 값은 보고·원장·로그에 넣지 않고 Secret 이름·지문만 남긴다.
3. `quota_group_id`는 공급자/계정 또는 org/모델 계열 단위로 기록한다. 두 역할이 같은 group인지는 **01-04에서 원본 로그로 공유 확인/분리 확인/관측 불가 중 하나로 판정**한다. 별도 컨테이너·`CLAUDE_CONFIG_DIR`·`--max-budget-usd`는 공급자 한도 분리의 근거가 아니다.

**선택(revision 5): A.** 사용자 결정 D1([원문](../../../../User_Decisions_2026-11-12_0103.md)): “우선 A: 같은 구독의 역할별 자격으로 검증한다.” A+C·추가 API 과금은 이번에 채택하지 않는다. 토큰 발급은 사용자가 하며(계약 2), 두 역할은 서로 다른 토큰을 받는다.

후보 방식(revision 4의 비교표, 기록용):

| 방식 | 설치 버전 근거 | 자격 분리 | 공급자 한도 | 확인할 것 |
| --- | --- | --- | --- | --- |
| A. 같은 구독 계정의 역할별 장기 토큰(`claude setup-token`) | `setup-token` 명령 존재(“requires Claude subscription”). `CLAUDE_CODE_OAUTH_TOKEN` 문자열은 실행 파일에 존재하나 우선순위·적용 방식은 미확인 | 토큰 단위 분리·폐기 가능 여부 probe | 같은 계정이므로 **공유 가능성이 높다**(판정 아님) | 토큰 주입 경로, 공유/분리 판정 |
| B. 역할별 별도 구독 계정 | 계정 미보유 | 계정 단위 | 계정 단위 분리 여부 probe | 계정 확보 |
| C. 역할별 API key 또는 `apiKeyHelper`(`--bare`는 이 둘만 사용) | `--bare` 도움말 | key 단위 | org/workspace 한도 구조 probe | key 발급·한도 구조 |
| D. 호스트·대화 세션 로그인 공유 | — | 불가 | — | **금지** |

## 4. 기동·설정 신뢰·재개 계약

### 4.1 역할별 설정 신뢰·실행 범위

**규칙**: 역할 프로세스는 **supervisor가 읽기 전용으로 제공한 설정만** 적용한다. repo worktree나 `CLAUDE_CONFIG_DIR` 아래 에이전트가 쓸 수 있는 위치에서 온 권한 규칙·hook·MCP·플러그인·skill·명령·자식 정의·사용자 CLAUDE.md는 적용하지 않는다.

| 역할 | repo CLAUDE.md | repo `.claude/`·hook·플러그인·`.mcp.json` | 도구 | 설정 출처 |
| --- | --- | --- | --- | --- |
| 개발 | 허용하지만 `--setting-sources user`에서는 **자동 로드되지 않는다**(01-04 G2, §10 항목 2). 공급 방식(예: supervisor가 main에 병합된 CLAUDE.md를 `--append-system-prompt-file`로 공급)은 02-04 전에 정한다. 승인된 누적 지식은 main에 병합된 CLAUDE.md다(검토 N7) | 적용 금지 | 쓰기·Bash 포함(컨테이너 안). 권한 규칙은 보안 경계 아님(§2.1) | `/etc/ieo/role/` RO |
| Guardrail | **적용 금지**. 검토 대상으로 읽기만 | **적용 금지** | Read·Grep·Glob(입력 경로 한정) + Write·Edit(`/state/guardrail-scratch/**`만). Bash·네트워크 게시 없음. 검토 대상 실행 금지, 재현은 격리 실행 요청(§2.1) | `/etc/ieo/role/` RO |

고정 인자 집합. 근거는 [claude-help.txt](evidence/claude-help.txt) 옵션 존재이며 각 인자가 규칙을 달성하는지는 §8 probe G로 확인한다:

```text
공통   claude -p --input-format stream-json --output-format stream-json --verbose
         --session-id <코디네이터 발급 UUID> -n <repo>-<role>
         --permission-mode dontAsk --permission-prompts none
         --setting-sources user --settings /etc/ieo/role/settings.json
         --strict-mcp-config --mcp-config /etc/ieo/role/mcp.json
         --agents /etc/ieo/role/agents.json
         --allowedTools <역할 허용 목록> --disallowedTools <역할 거부 목록>
         (경로 규칙은 `//절대경로` 형식. 파일 쓰기 허용은 `Edit(//path/**)`로 쓴다. `Write(...)` 표기는 쓰지 않는다 — §10 항목 1)
         /etc/ieo/role/settings.json 에 {"autoMemoryEnabled": false} 포함(§10 항목 3)
개발   cwd=/state/work/<repo>
Guardrail cwd=/state/guardrail-cwd(빈 디렉터리), --disable-slash-commands,
         --tools "Read,Grep,Glob,Write,Edit" (--add-dir는 쓰지 않음)
         허용 규칙: Read(//state/review/**) Read(//state/inputs/**) Read(//state/guardrail-scratch/**)
                    Edit(//state/guardrail-scratch/**)
         거부 규칙: Read(//run/ieo/**) Read(//etc/ieo/**) Read(//state/claude/**) Read(//state/home/**) Read(//proc/**) Bash
         읽기 허용: /state/review/<sha>/** (코드 스냅샷, RO)
                    /state/inputs/<assignment_id>/** (승인 명세·프로파일·보호된 검사 결과·재현 결과 원본, RO)
                    /state/guardrail-scratch/**
         쓰기 허용: /state/guardrail-scratch/**
         읽기 거부: /run/ieo/**, /etc/ieo/**, /state/claude/**, /state/home/**, /proc/**
```

- **--add-dir를 Guardrail에 쓰지 않는다**(검토 N2): 도움말이 `--add-dir`를 CLAUDE.md 출처로 적는다. 검토 대상은 cwd 밖 읽기 전용 스냅샷으로 두고 경로 한정 Read/Grep/Glob 허용 규칙으로 읽게 한다. probe G가 그 스냅샷의 표식 CLAUDE.md가 로드되는지 확인한다. 01-04: 로드되지 않았다(G1 시도 05). 대안은 필요 없었다. 로드됐다면 대안은 supervisor가 만든 검토 묶음(지시 파일 이름을 `*.reviewed`로 바꾼 사본+`review.patch`)이며, 그것도 실패하면 01-04 중단·결정 요청이다.
- **`CLAUDE_CONFIG_DIR` 보호**(검토 N3·R3): 자격은 §3대로 `/run/ieo/cred/`에서만 오며 `CLAUDE_CONFIG_DIR`에 자격 파일을 두지 않는다(기동 전 발견 시 삭제하고 기록). supervisor는 매 기동 전에 `CLAUDE_CONFIG_DIR` 아래에서 `--resume`에 필요한 세션 기록(`projects/<cwd>/*.jsonl`)만 **제외한 전부**(settings, CLAUDE.md, agents, skills, commands, plugins, `projects/*/memory/` 자동 기억 등)를 지우고 RO 템플릿으로 다시 만든 뒤 digest를 체크포인트에 남긴다. 실행 중 변조는 Guardrail은 쓰기 도구가 없어 불가하고, 개발은 가능하지만 개발 역할 범위 안의 영향이다(§2.1). 세션 기록 경로 안의 자동 기억(auto-memory) 같은 지시성 파일은 보존 대상이 아니다: 누적 지식은 repo 파일(PR·독립 검토)로만 남긴다(§1.2, Guardrail은 repo 검토 지식만). “user 출처 = `CLAUDE_CONFIG_DIR/settings.json`”, 재개에 필요한 경로 목록, 자동 기억의 위치·비활성 방법은 도움말에 없는 **가정**이며 probe G에서 확인한다(검토 N6·R3).
- **경로 한정 읽기/쓰기 규칙**(`Read(<path>)`류)의 문법·효과는 도움말에 `Bash(...)` 예만 있어 가정이었다(검토 R4). 01-04: `//` 절대형 경로 규칙이 동작한다. G1의 읽기 거부 4건, U-alt의 허용·거부가 근거다. 쓰기 허용은 `Edit(//…)` 형식이다(§10 항목 1). revision 6의 Guardrail 규칙 집합 그대로(`Write(…)` 없이 `Edit(…)`만)는 실행하지 않았으므로 02-04에서 시험한다.
- **비밀 유출 방지**(검토 R4): Guardrail 모델이 지시 주입으로 자기 자격을 검토문에 옮겨도 공개되지 않도록, 코디네이터는 claim을 저장·게시하기 전에 등록된 자격 지문·알려진 토큰 패턴을 검사하고 일치하면 게시를 막고 자격 교체 결정을 요청한다.
- user 출처를 완전히 끄는 방법(빈 출처 목록, `--safe-mode`와 `--agents` 동시 사용, `--bare`+`apiKeyHelper`)은 probe G에서 비교해 가장 좁은 것을 채택한다. 01-04: `--setting-sources user` + RO `--settings` + 기동 전 `CLAUDE_CONFIG_DIR` 정리 + `autoMemoryEnabled:false` 조합이 G1·G2·G3b·AM에서 기대대로 동작했으므로 이것을 채택한다. `--safe-mode`·`--bare`는 비교하지 않았다(채택 조합으로 충분해 필요 없음).
- **probe G에서 Guardrail이 검토 대상의 CLAUDE.md·settings·hook·MCP·플러그인을 하나라도 적용하거나, 읽기 전용 도구 밖의 실행이 일어나면** 그 방식은 불합격이다. 대안이 없으면 Guardrail 독립성(P07)이 성립하지 않으므로 01-04 결과로 중단·결정 요청 대상이다. 01-04: 합격(G1 시도 05, 표식 적용 0·금지 경로 거부).

### 4.2 식별·재개

| 계약 | 수단(설치 버전 근거) | 01-04 확인 |
| --- | --- | --- |
| 무인 기동 | `-p`, `--output-format stream-json`, `--input-format stream-json`, `--permission-mode dontAsk`, `--permission-prompts none`(“anything that would prompt is denied automatically”) | 허용/거부 실제 적용, 프롬프트 대기 0건 |
| session ID 사전 지정 | `--session-id <uuid>` | 결과 사건의 session_id가 발급 UUID와 같은지 |
| 논리 이름 | `-n/--name`(표시 이름) | **재개 키로 쓰지 않는다**(유일성 근거 없음). 01-04 N3: `--resume <이름>`은 동작했지만 계약을 바꾸지 않는다(§10 항목 5) |
| 재개 | `-r/--resume <session-id>`; 복제는 `--fork-session` | 01-04 N2·L2·R: 지원. N4: 실행 중인 세션을 다른 프로세스가 재개해도 CLI는 막지 않고 기록이 분기된다. supervisor가 세션당 단일 실행을 보장한다(§5·§7, §10 항목 4) |
| 기록 위치 | `CLAUDE_CONFIG_DIR`(실행 파일 문자열·`auth status`의 configDirectory) | 역할 볼륨 아래에만 기록, 재생성 후 복원 |
| 압축 | `--autocompact <auto|tokens>` | 압축 사건 감지 가능 여부 |
| 구조화 결과 | `--output-format json|stream-json`, `--json-schema` | 결과 사건의 session_id·usage·비용 필드 실제 존재 |
| 자식 사건 귀속 | `--forward-subagent-text`: 자식의 **텍스트·thinking**을 `parent_tool_use_id`와 함께 전달(사용량 전달은 도움말에 없음) | 01-04 T(시도 23): 자식 assistant 사건의 `usage`(`agent_id`), Agent tool_result의 `totalTokens`·`usage`로 자식별 사용량을 얻는다. 결과 `modelUsage`는 자식을 포함한다. 02-01(revision 7, §11 항목 7): 재개한 세션의 `modelUsage`·`total_cost_usd`는 **세션 누적**이고 `result.usage`는 부모의 이번 회차만이며 압축 호출을 뺀다 |
| 예산 | `--max-budget-usd`(print 전용, 로컬 상한) | 로컬 상한일 뿐 공급자 한도 아님 |
| 사용 안 함 | `--dangerously-skip-permissions`, `bypassPermissions`, `--bg/attach/agents`(프로세스 수명은 supervisor가 관리), `--cloud`, `--remote-control`, `--chrome` | — |

매핑 레코드(제품 DB, state.json 아님): `logical_name ↔ session_id ↔ role ↔ repo_id ↔ workspace_ref ↔ runtime_instance_id ↔ assignment_id/generation ↔ quota_group_id ↔ settings_digest`. 이름이 같다는 사실만으로 재개 성공을 판정하지 않는다.

### 4.3 환경 정리(검토 F9)

supervisor와 01-04 probe는 역할 프로세스를 **빈 환경에서 허용 목록만** 넣어 시작한다(`env -i` 상당): `HOME`, `PATH`, `LANG`, `CLAUDE_CONFIG_DIR`, 선택한 자격 변수 1개. 개발 대화 세션의 `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_MESSAGING_*`, `CLAUDE_CODE_CHILD_SESSION` 등은 넘기지 않는다. probe 원본 로그에 실제 전달한 변수 **이름** 목록을 남긴다(값 제외).

## 5. 체크포인트·재개 절차

1. supervisor는 단계 전환·인수/첫 행동·종료/한도 직전·예외 시 체크포인트를 쓴다: session_id, generation, assignment_id, 대상 SHA, **sprint_id/revision/digest, image_digest, profile ID/revision, settings_digest**, worktree HEAD·미커밋 diff 해시, 마지막 stream-json 사건 오프셋, 활성 자식 목록, 미확인 외부 요청 ID, 마지막 회차의 세션 누적 사용량(`modelUsage` 합·`total_cost_usd`, 다음 재개 회차 몫의 기준, §11 항목 7), 그 회차의 전달(`delivery_id`)·시도(`attempt_id`)와 첫 행동(`first_action`, revision 12, §15). digest·순번은 코디네이터에 보고한다(§2.1).
2. 재기동 시 `reconnect`: 체크포인트의 session_id로 `--resume`, 현재 배정 세대·코디네이터 기록 digest와 대조. 세대가 낡으면 `STALE_GENERATION`, digest가 다르면 외부 대조 필요로 중단.
3. repo 기록(승인된 명세·코드)이 세션 기억과 다르면 repo 기록이 우선이다(1.2절).
4. 재개 실패/관측 불가는 `NOT_OBSERVED`로 보고하고 새 세션을 조용히 대체하지 않는다.
5. **세션당 단일 실행**(revision 6, §10 항목 4): supervisor는 같은 session_id로 실행 중인 프로세스가 있으면 `--resume`을 시작하지 않는다(잠금은 역할 볼륨 안 supervisor 소유 파일). 실행 중 재개 요청은 `status`로 응답한다. 한계: 같은 컨테이너의 개발 코드는 이 잠금 파일을 지울 수 있다(§2.1과 같은 이유). 영향은 개발 역할 자신의 세션 범위에 그친다. Guardrail은 Bash·볼륨 쓰기 도구가 없어 지울 수 없다.
6. **변조 탐지와 재개**(revision 6, §10 항목 3): 기동 전 정리는 다음 기동의 설정 적용을 막는다. 그러나 이미 세션 기록에 들어간 내용은 재개하면 맥락으로 남는다(01-04 G3b). 그래서 정리 **전에** 변조를 탐지한다.
   - 방법: `CLAUDE_CONFIG_DIR`의 **지시성 항목만** digest한다. 그 값을 마지막 체크포인트의 `settings_digest`(정리 직후 템플릿 상태의 같은 범위)와 비교한다.
     - 대상: `settings*.json`, `CLAUDE.md`, `agents/`, `skills/`, `commands/`, `output-styles/`, `hooks/`, `plugins/`(단 CLI 내장 기록 `plugins/store/*_builtin-*.json` 제외), `.mcp.json`, `projects/*/memory/`.
     - `.claude.json`은 파일 전체가 아니라 지시성 키만 넣는다: 최상위 `mcpServers`, `projects.*.mcpServers`, `projects.*.allowedTools`, `projects.*.enabledMcpjsonServers`.
     - CLI가 정상 실행마다 쓰는 항목은 대상에서 뺀다. 01-04 L1·G3b와 두 역할 볼륨에서 관측한 것은 다음이다. `.claude.json`의 나머지 키, `backups/`, `policy-limits*`, `remote-settings*`, `sessions/`, `session-env/`, `shell-snapshots/`, `.last-cleanup`, `plugins/store/cc-plugin-agents-md_builtin-*.json`. 이 중 지시성 대상 안에 있는 것은 마지막 하나뿐이고, 그래서 위에서 따로 제외했다. 01-04의 정상 실행에서 `.claude.json` 지시성 키는 없었다.
     - 제외 목록은 관측 범위의 것이다. 시험 (c)가 다른 종류의 정상 실행에서 지시성 대상 안의 새 자동 기록을 찾으면, 02-04 검수 전에 그 항목을 원본 증거와 함께 제외 목록에 넣거나 지시성으로 분류한다.
     - 한계: CLI 버전이 새 지시성 위치를 추가하면 이 목록을 다시 확인한다(런타임 갱신 시 §1). 목록에 없는 위치의 변조는 탐지하지 못하지만, 정리 단계가 지시성 항목 밖의 파일도 지우므로 다음 기동에 적용되지 않는다.
   - 같으면 정리한 뒤 `--resume`한다.
   - 다르면 **변조 탐지**다. `--resume`하지 않는다. 정리한 뒤 새 session_id로 시작하고, `config_tamper_detected`(이전·새 session_id, digest 두 값)를 체크포인트와 코디네이터에 보고한다.
   - 체크포인트가 없는 첫 기동은 비교 대상이 없으므로 정리한 뒤 새 세션으로 시작한다.

## 6. 자식 에이전트 경계

- 정의: `/etc/ieo/role/agents.json`(RO)을 `--agents`로 준다. 자식 도구는 부모 역할 허용 목록의 부분집합이다. 자식은 별도 자격을 받지 않고 부모 프로세스 안에서만 동작한다. Guardrail 자식은 개발 맥락을 받지 않는다.
- 기록: supervisor는 stream-json에서 자식 사건(`parent_tool_use_id`)을 읽어 **자식 ID·부모 session_id·세대·부작업·외부 요청·관측 가능한 비용**을 체크포인트와 코디네이터 claim에 남긴다. 자식별 사용량을 얻지 못하면 “미계측”으로 남기고 0으로 쓰지 않는다(P35). 회차 사용량은 새 세션이면 세션 누적 그대로, 재개면 세션 누적에서 **실제로 재개한 직전 회차**의 누적을 뺀 값이다. 기준 회차가 없거나 맞지 않으면 미계측으로 둔다(§11 항목 7).
- 부모 중단: 재개 전에 체크포인트의 활성 자식 목록·미확인 외부 요청을 대조한다. 무조건 새 자식을 띄우지 않는다(§1.7).
- 코디네이터의 인수·보고 상대는 부모뿐이다.

## 7. 런타임 어댑터 v0 대응

1.2절의 start/reconnect/stop/status 요청·응답 필드를 그대로 쓴다.

| 동작 | supervisor 구현 |
| --- | --- |
| start | §4.1 고정 인자, `CLAUDE_CONFIG_DIR`/HOME=역할 볼륨, §4.3 환경. `capability_gaps`에 01-04 미확인 항목을 그대로 보고 |
| reconnect | `--resume <session_id>`; `restored_checkpoint_ref`·`external_reconciliation_required` 반환. 같은 session_id가 실행 중이면 재개하지 않는다(§5-5) |
| stop (role 범위) | 체크포인트 기한 내 SIGTERM→관측→필요 시 SIGKILL. 자기 Pod 프로세스만 |
| stop (repo 범위) | 코디네이터가 두 supervisor에 **각각** role 범위 stop을 보낸다. supervisor는 상대 역할을 멈추지 않으며, 각 역할의 안전 종료 조건(미확인 외부 효과·활성 자식·미저장 체크포인트 0)을 따로 보고한다. 미확인 외부 요청의 목록과 코디네이터 자신의 결과 불명 전달은 §16 항목 18·20 |
| status | 프로세스 생존·마지막 체크포인트·session_id·`quota_group_id`·`provider_status_or_unknown`(관측 불가면 unknown)·`last_actual_action_at`. 값 대응·신선도·공유 그룹은 §13 항목 9, 체크포인트 순번·digest·중앙 보고 여부는 §13 항목 10, `last_actual_action_at`은 §15 항목 15 |

오류 대응. 어댑터 오류 코드(§1.2, 6종)는 **호출 자체의 실패**에만 쓴다. 공급자 한도·인증/과금은 재시도 가능한 일시 오류가 아니므로 `status`의 분류와 운영 상태로 보고한다(검토 N4, §1.2 “한도 종류와 공유”). 실제 런타임 오류 문자열은 01-04 원본 로그로 채운다.

| 관측 | 어댑터 응답 | 재시도 |
| --- | --- | --- |
| 호출자 자격 불일치·범위 밖 요청 | `UNAUTHORIZED` | 없음 |
| 배정 세대가 현재보다 낡음 | `STALE_GENERATION` | 없음 |
| 같은 `request_id`·다른 내용 | `ID_CONTENT_CONFLICT` | 없음. 같은 `request_id`(start·reconnect의 `attempt_id`)·같은 내용은 저장된 답(revision 11, §14 항목 11) |
| 필요한 옵션/동작이 런타임에 없음 | `UNSUPPORTED_CAPABILITY` | 없음 |
| 프로세스/세션 상태·재개 결과를 관측할 수 없음 | `NOT_OBSERVED` | 외부 대조 후 판단 |
| 일시적 네트워크 오류·공급자 5xx·rate limit(짧은 대기) | `TRANSIENT_FAILURE` | 상한 있는 backoff |
| **공급자 사용량 한도** | 정상 응답 + `status.provider_status_or_unknown=usage_limit`, `quota_group_id`, 최초 오류 원문 참조 | 즉시 재시도 금지. 운영 상태 `작업을 보유한 대기`/`QUOTA_WAIT`, `next_check_at`에 01-03에서 정한 간격·상한의 재개 probe |
| **인증/과금 오류** | 정상 응답 + `provider_status_or_unknown=auth_or_billing` | 재시도 금지. `사람 결정 대기`로 결정 요청(자격 교체는 사람 정책 소유자) |
| 컨텍스트 한계·압축 | 오류 아님. 체크포인트 + 압축 사건 기록 | — |

## 8. 01-04 probe 순서(이 계약의 확인 계획)

1. **동시 두 세션**: 같은 계정의 역할별 자격(A)으로 개발·Guardrail 역할 프로세스를 실제로 겹쳐 실행(각자 `--session-id`, 각자 `CLAUDE_CONFIG_DIR`, §4.3 환경). 판정: 지원/미지원/관측 불가.
2. **역할별 자격·세션·저장소 접근 격리**: 한 역할이 다른 역할의 Secret·설정 디렉터리·세션 기록·역할 볼륨(작업 저장소·scratch)을 읽거나 쓸 수 있는지, 다른 역할 명의로 동작하거나 다른 역할 세션을 재개할 수 있는지 시험한다. 판정: **성립 확인**(모든 시도가 거부됨을 원본 로그로 확인) / **실패**(하나라도 가능) / **미확인**(시험할 수 없거나 결과를 관측할 수 없음). “지원/미지원” 어휘는 쓰지 않는다.
3. **공급자 한도 공유·분리**: 두 역할의 quota group이 같은지. 판정을 **공유 확인 / 분리 확인 / 관측 불가**로 나눈다. 로컬 예산·별도 컨테이너는 근거로 쓰지 않는다.
4. **판정표 A1-GATE-01-04**(revision 5, 사용자 결정 D1). 실행 장부·명세 01-04 수용/종료 조건과 같은 표다. 기계 판독본: [probe_gate_table.json](../../01-03/staged-freeze-20261112/probe_gate_table.json). 예산·집계: [Probe_Budget_Rules.md](../../01-03/staged-freeze-20261112/Probe_Budget_Rules.md)(총 30회·7,200초·시도당 300초·추가 API 지출 0).

   | probe | 결과 → 조치 |
   | --- | --- |
   | 1 동시 두 역할 세션 | 지원 → 진행 · 미지원 → **중단** · 관측 불가 → **중단** |
   | 2 역할별 자격·세션·저장소 접근 격리 | 성립 확인 → 진행 · 실패 → **중단** · 미확인(관측 불가 포함) → **중단** |
   | 3 공급자 한도 구조 | 공유 확인 → 공유 quota group의 **지원 구성으로 기록하고 진행** · 분리 확인 → 진행 · 관측 불가 → **중단** |

   예산 소진으로 판정하지 못한 행은 관측 불가(2는 미확인)로 기록한다. 중단이면 원본 로그·영향·대안·재개 결정을 보고하고 다음 probe/작업을 진행하지 않는다. 공유 확인을 한도 분리 성공으로 쓰지 않는다. 공유 구성에서는 한 역할의 한도 도달이 같은 group의 다른 역할·repo에 영향을 준다는 전제로 운영하며(§7, P05-Q), 그 실제 영향은 04-15에서 측정한다.
5. 이후: **G. 설정 신뢰·실행 범위**, 무인 허용/거부, 이름↔세션 재개, 기록 위치·압축 감지, 세션/자식별 토큰, 런타임 갱신 전후 재개 — **01-04에서 실행·검수 완료**([결과](../../01-04/remaining-probes-20261112/REPORT.md)). 계약에 반영할 결과는 §10이다. 재현 요청의 격리 실행기는 02-06 구현, P07 검증 범위로 남는다.

## 9. 미확정(이 계약이 정하지 않는 것)

- 역할별 자격의 실제 획득(A로 결정됨, revision 5) — 사용자가 01-04 첫 probe 전에 두 역할 토큰을 발급해 비밀 값이 기록되지 않는 경로로 넘긴다.
- 역할별 허용/거부 도구 목록의 구체값 — 03-02 정책 문서에서 확정, 01-04 probe는 최소 목록으로 시험.
- 역할별 상시 유지/idle 정지(P34 뒤 결정).

## 10. 01-04 실측 반영 (revision 6)

근거:
- 01-04 결과 [첫 세 probe](../../01-04/first-probes-20261112/REPORT.md)·[나머지 범위](../../01-04/remaining-probes-20261112/REPORT.md) §1과 [ADDENDUM](../../01-04/remaining-probes-20261112/ADDENDUM.md)
- 사용자 결정 [01-04](../../../../User_Decisions_2026-11-12_0104.md)

이 개정은 문서 개정이다. 01-04 실험을 다시 하지 않는다. 각 항목은 해당 동작을 쓰는 구현 작업이 **착수하기 전에** 이 revision이 검수 완료여야 한다.

| # | 항목 | 근거(원본) | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- | --- |
| 1 | 파일 쓰기 경로 허용은 `Edit(//절대경로/**)`로 쓰고 `Write(...)` 표기는 쓰지 않는다. 모든 경로 규칙은 `//` 절대형이다 | 직접 근거: U 시도 01(`Write(//…)`: 허용 대상까지 거부)과 U-alt 시도 02·04(`Edit(//…)`: 기대대로 2회)의 대조. 간접 근거: G1 시도 05(scratch 쓰기 성공. 단 `Write(…)`와 `Edit(…)`를 함께 허용했으므로 단독 근거는 아니다). 사용자 결정 “Edit 형식 채택” | §4.1 고정 인자(공통·Guardrail 블록) | 이 revision 검수 완료. 03-02 정책 문서가 같은 표기를 쓴다. 02-04 supervisor의 인자 생성 시험이 `Edit(//…)`를 출력하고 `Write(`를 출력하지 않는다. revision 6 Guardrail 규칙 집합 그대로 scratch 쓰기 허용·그 밖 거부를 02-04에서 실행한다 |
| 2 | 개발 역할의 repo CLAUDE.md는 `--setting-sources user`에서 **로드되지 않는다**. “허용”은 자동 적용을 뜻하지 않는다. 필요하면 supervisor가 main에 병합된 CLAUDE.md 내용을 별도 경로로 공급한다(예: `--append-system-prompt-file`). hook·MCP·skill·command·agent 같은 repo 설정은 계속 적용하지 않는다 | G2 시도 06(repo CLAUDE.md 미로드, repo 설정 적용 0) | §4.1 표의 개발 행 | 02-04 착수 전에 공급 방식을 정하거나 “공급하지 않음”을 기록한다. 공급 방식을 쓰면 02-04에서 표식 CLAUDE.md로 로드와 repo hook 미적용을 함께 시험한다 |
| 3 | `CLAUDE_CONFIG_DIR` 기동 전 정리는 필수다. 남길 것은 `projects/<cwd>/*.jsonl` 세션 기록뿐이고 `projects/*/memory/`는 지운다. 자동 기억은 RO `settings.json`의 `{"autoMemoryEnabled": false}`로 끈다(환경 변수 `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`도 동작하지만 §4.3 허용 목록을 넓히므로 쓰지 않는다). 정리 전에 변조를 탐지하고, 탐지된 세션은 재개하지 않는다(§5-6) | G3a 시도 09(변조가 모두 적용됨). G3b 시도 10(정리 뒤 적용 0, 재개 성공, 이전 지시를 기록으로 인용). AM1·AM2 시도 26·27(`memory_paths` 소멸). 첫 G3 쌍 무효의 원인(SELinux 라벨) | §4.1 `CLAUDE_CONFIG_DIR` 보호·고정 인자, §5-6 | 02-04에서 세 시험을 따로 통과한다. (a) **정리**: 체크포인트 없는 첫 기동에 표식 settings·CLAUDE.md·skills·`memory/` 파일을 넣는다. init에 표식·`memory_paths`가 없고, 볼륨의 SELinux 라벨·모드가 바뀌지 않는다. (b) **변조 탐지**: 체크포인트가 있는 세션 뒤 표식을 넣고 reconnect한다. `--resume`이 실행되지 않고, 새 session_id로 시작하며, `config_tamper_detected`가 보고된다. (c) **정상 재개**: 변조 없이 여러 종류의 정상 실행(도구 없음, 허용 도구 사용, 자식 에이전트 호출, `/compact`)을 각각 한 뒤 reconnect하면 같은 session_id로 재개되고 변조로 오판하지 않는다. (b)의 표식 주입은 제외 경로(`plugins/store/*_builtin-*.json`)를 쓰지 않는다 |
| 4 | CLI는 같은 세션의 동시 실행을 막지 않는다. supervisor가 세션당 단일 실행을 보장한다 | N4 시도 14·15(두 프로세스 모두 성공, 기록 파일 분기 1건) | §4.2 재개 행, §5-5, §7 reconnect | 02-04에서 실행 중 세션에 대한 두 번째 reconnect가 시작되지 않고 `status`로 응답하는 시험 |
| 5 | 이름(`-n`)은 재개 키가 아니다. `--resume <이름>`이 동작해도 쓰지 않는다 | N3 시도 13 | §4.2 논리 이름 행 | 02-04·02-05의 매핑 레코드가 재개에 session_id만 쓴다(코드 검토) |
| 6 | 개발 역할의 Bash 허용 목록은 완전한 경계가 아니다. 판정·격리 근거로 쓰지 않는다 | N4a 시도 14(`Bash(sleep:*)`만 허용했으나 `while pgrep …; echo done` 실행), U 시도 01(`ls /` 거부) | §2.1 | 03-02 정책 문서와 P07 위협 모형이 개발 Bash 규칙을 경계로 세지 않는다 |

## 11. 02-01 계측 반영 (revision 7)

근거: 02-01 [계측기 보고](../../02-01/collector-20261112/REPORT.md) §3. 01-04에 보존된 실제 stream 35개를 계측기로 재생하고 독립 Python 판독과 대조했다(모델 호출 0회, 01-04 판정 불변).

| # | 항목 | 근거(원본) | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- | --- |
| 7 | 재개한 세션의 `result.modelUsage`·`total_cost_usd`는 이전 회차를 포함한 **세션 누적**이다. 회차 사용량은 새 세션이면 그 값, 재개면 그 값에서 실제로 재개한 직전 회차의 누적을 뺀 값이다. `result.usage`는 부모의 이번 회차만이고 자식·압축 호출을 뺀다. 기준이 없거나 누적이 줄거나, 압축 사건 없이 회차 몫이 부모+자식과 다르면 그 회차는 **미계측**이며 0이나 누적값으로 대신하지 않는다 | 01-04 R1→R2(시도 24·25), N1→N2→N3(11~13), G3a→G3b(07·08, 09·10): 재개 회차 `modelUsage` = 이전 누적 + 그 회차 `result.usage`. L3(18, `/compact`): `result.usage` 0, 누적 증가분이 압축 사용량. N4a∥N4b(14·15)→L2(17): 동시 재개로 분기해 직전 회차를 시각 순서로 정할 수 없음 | §4.2 자식 사건 귀속 행, §5-1 체크포인트, §6 기록 | 02-01 계측기(`ieo-evidence`)가 이 규칙으로 계산한다(02-01 검수). 02-04 supervisor가 매 회차 세션 방식(새/재개)과 재개 기준 회차를 계측기에 넘기고 체크포인트에 세션 누적을 남긴다. 02-04에서 새 세션→재개→`/compact` 재개 연쇄의 회차 몫 합이 마지막 세션 누적과 같음을 시험한다 |

## 12. 02-04 실측 반영 (revision 8)

근거: 02-04 실제 회차 D3(2026-11-15, 자식 에이전트가 있는 재개 회차)의 보존 기록과 [사용자 결정](../../../../User_Decisions_2026-11-15_d3.md). 기록 대조로 확인했고 새 모델 호출은 없다. [02-04 진행 기록 5](../../02-04/child-usage-20261115/PROGRESS.md)

| # | 항목 | 근거(원본) | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- | --- |
| 8 | 자식 사용량의 근거는 자식 전사(`projects/<cwd>/<session>/subagents/agent-<agentId>.jsonl`)다. 각 모델 응답의 확정 사용량(`stop_reason`이 있는 항목)을 `message.id`마다 한 번 더한다. 이유는 둘이다. CLI 2.1.322의 `tool_use_result.usage`·`totalTokens`는 자식의 **마지막 응답**만이다. 실시간 stream의 자식 `assistant` 사용량은 중간 관측(하한)이다. 확정 항목이 없거나 서로 다른 응답, 읽을 수 없는 행, 다른 자식의 행이 있으면 그 자식은 미계측이다. 자식 전사가 없으면 자식은 미계측이고 보고된 마지막 응답 값은 참고로만 남긴다. 재개 회차의 몫은 압축 사건이 없으면 부모＋자식 합과 네 항목 모두 같아야 측정된다. 다르거나 대조할 수 없으면 미계측이다. supervisor는 회차가 시작한 자식의 전사를 회차 디렉터리에 보존한다(다음 기동의 정리 전) | D3: 자식 응답 2개(2/61/2,067/0, 2/3/90/2,067). `tool_use_result.usage` = 둘째 응답. 첫 응답 stream 출력 16 ≠ 전사 61. 부모(6/160/456/33,595)＋두 응답 합 = 세션 증가분(10/224/2,613/35,662) | §6 기록, §11 항목 7의 부모·자식 대조 | 02-01 계측기(`stream::parse_with_children`)와 02-04 호출자·역할 서비스가 이 규칙으로 집계한다. D3 비식별 기록 재생에서 네 항목이 일치한다. 중복·진행 중·마지막 응답만·잘못된 기준·누적 감소 시험이 있다(02-04 진행 기록 5) |

## 13. 02-04 수용 결정 반영 (revision 9, 항목 10은 revision 10에서 보완)

근거: [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0204-acceptance.md) 항목 4·9. 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다. [02-04 진행 기록 9](../../02-04/provider-checkpoint-20261115/PROGRESS.md)

| # | 항목 | 근거(원본) | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- | --- |
| 9 | **공급자 상태 값.** 한 회차 stream의 원시 신호를 값으로 바꾼다. `auth_or_billing`: `assistant` 사건의 최상위 `error`가 `authentication_failed`·`oauth_org_not_allowed`·`account_on_hold`·`verification_required`·`billing_error`·`cloud_credential_error`. `usage_limit`: `rate_limit_event`의 `status: rejected` 또는 `isUsingOverage: true`(구독 한도 도달, 이어 가면 추가 과금이라 허용하지 않음). `available`: 마지막 `rate_limit_event`가 항목 31의 판정을 통과하고(`allowed` 또는 `allowed_warning`, `overageStatus: rejected`, `isUsingOverage: false`) 그 회차의 어느 사건도 그 판정의 멈춤이 아니며 그 뒤 위 오류가 없음(revision 19). `allowed_warning`이면 경고를 옆에 보인다. 그 밖은 모두 `unknown`이다: 신호 없음, 읽을 수 없거나 모르는 값, 항목 31의 멈춤 중 사용 한도가 아닌 것(추가 사용 가능, 누락·모순), 일시 오류(`rate_limit`(거부된 창 없음)·`overloaded`·`server_error`), 신선하지 않은 관측. 한 회차 안의 우선순위는 `auth_or_billing` > `usage_limit` > 항목 31의 다른 멈춤 > 마지막 가용 신호다. **시각과 신선도**: stream 사건에는 시각이 없다. 관측은 회차 시작과 끝(실행 중이면 지금) 사이로만 알 수 있으므로, 나이를 회차 **시작**부터 센다. 설정한 최대 나이(기본 600초)를 넘으면 `unknown`으로 보고하고 마지막 관측은 옆에 남긴다. 실행 중에는 그 회차의 stream만 본다. 서비스 재시작 전 관측은 이력(`current: false`)이며 현재 값이 아니다. **`quota_group_id`**는 역할 설정에 명시하는 필수 값이다(빈 값·`unknown` 거부). 자격이 같은 공급자 한도를 공유하면 같은 값을 쓴다(01-04 P3 공유 확인). 코디네이터는 그룹별로 묶는다. 한 구성원이 `usage_limit`이면 그룹 전체를 `usage_limit`으로 보고 모든 구성원을 영향 역할로 보고한다. `available`은 한 구성원이 관측했고 `usage_limit`이 없을 때만이다. `auth_or_billing`은 자격 하나의 상태라 퍼뜨리지 않는다. 코디네이터 설정의 기대 그룹과 다른 보고는 불일치로 표시한다 | CLI 2.1.322 SDK 스키마(실행 파일의 형 정의: `assistant.error` 열거값 13개, `rate_limit_event.rate_limit_info.status` `allowed`/`allowed_warning`/`rejected`, `isUsingOverage`). 실제로 관측한 형태는 `allowed`·`overageStatus: rejected`·`isUsingOverage: false`뿐이다(01-04·02-04). `rejected`·인증 오류의 실제 출력은 관측하지 않았다(가정, 아래 한계) | §3 항목 3, §7 status 행·오류 대응표 | supervisor `provider` 모듈과 상태 응답, 코디네이터 `quota_groups`. 모델 호출 없는 시험: 값 대응 전체, 같은 회차 안 우선순위, 실행 중 관측, 재시작 후 `unknown`, 오래된 관측, 그룹 전파·분리·불일치(진행 기록 9 §2). 한계: 거부·인증 오류의 실제 출력 형태는 실제 한도·오류가 생길 때 원본으로 대조한다(04-15) |
| 10 | **중앙 체크포인트 기록.** supervisor는 쓰는 체크포인트마다 순번(`seq`, 역할 볼륨 안에서 1부터 연속)과 `repo_id`·`role`·`config_tamper_detected`를 넣는다. 순번은 보관 사본과 로컬 체크포인트 중 큰 값의 다음이다. `checkpoint.json`을 쓴 뒤 사본을 `/state/checkpoints/<seq>`에 보관한다. 파일 바이트 그대로 코디네이터에 보고한다. 회차 시작(takeover)은 볼륨에 대기 사본(staged)을 둔 뒤 **먼저** 보고하고, 받아들여진 뒤에만 로컬에 쓴다. 회차 뒤(after_run)·오류(error)는 로컬에 먼저 쓰고 보고한다. 로컬 쓰기 순서는 `checkpoint.json` 다음 보관 사본이다(supervisor 전용 `checkpoint:report` 자격, repo·역할 하나에 묶임). 코디네이터는 그 바이트의 SHA-256을 직접 계산해 다음 순번만 저장한다. 저장 조건은 셋이다. (a) 배정·세대가 (repo, 역할)의 현재 배정 머리와 같다. (b) 명세 revision·digest와 대상 SHA가 그 배정과 같다. (c) 세대가 저장된 마지막 값보다 낮지 않다. 처리는 다음과 같다. takeover는 언제나 현재 배정이어야 한다. 단 회차 도중 배정이 바뀌면, 같은 세션·배정·세대로 저장된 takeover를 잇는 after_run·error는 `current_assignment=false`로 저장한다(회차가 실제로 끝난 기록을 잃지 않게). 같은 순번·같은 바이트는 중복으로 그대로 둔다. 같은 순번·다른 바이트는 충돌이다. 다음 순번을 건너뛰면 간격이다. 낮은 세대는 `STALE_ASSIGNMENT`다. 변조 탐지는 새 세션 하나에 한 번 기록한다. 거부된 보고는 행을 남기지 않는다. **start·reconnect 전 대조(§5-2)**: supervisor는 코디네이터의 최신 기록을 받아 로컬 체크포인트와 순번·digest를 맞춘다. 로컬이 앞서면 보관 사본을 순서대로 보낸 뒤 다시 맞춘다. 중앙이 앞서거나 digest가 다르면 `EXTERNAL_RECONCILIATION_REQUIRED`다. 로컬이 없는데 중앙에 기록이 있거나 보관 사본이 없어도 같다. 코디네이터에 닿지 않으면 `NOT_OBSERVED`이고 아무것도 시작하지 않는다. takeover가 명확히 거부되면 대기 사본을 지우고, 로컬에 아무것도 쓰지 않으며, 실행하지 않는다. 답을 받지 못했거나(연결 실패·시간 초과), 받아들여진 뒤 로컬 쓰기가 실패했거나, 그 사이에 멈춘 경우도 실행하지 않는다. 이때 대기 사본은 남고, 다음 기동이 코디네이터의 최신 기록이 정확히 그 사본(같은 순번·digest)임을 확인하면 그것을 로컬 체크포인트로 채택한다. 코디네이터에 없는 대기 사본은 버린다. 명확한 거부, 답 없음, 받아들여진 뒤 로컬 쓰기 실패는 시도를 실행되지 않은 것으로(사용량 0) 닫는다. 4xx 거부만 대기 사본을 지우고, 서버 오류(5xx)와 답 없음은 남긴다. supervisor가 대기 사본을 쓰기 전이나 보고 도중에 멈추면 예산 원장에 그 시도의 끝 행이 없다. 이때 역할의 예산 백스톱은 사람이 대조할 때까지 다음 시작을 막는다(사용량 0으로 치지 않는다). 대기 사본을 채택하면 그 사본이 담은 변조 보고도 보고된 것으로 본다. 단 코디네이터가 그 체크포인트의 보고가 변조 사건을 기록했다고 답할 때만이다(`latest`·중복 응답의 `tamper_recorded`, X-08 검토 681c1d3 L-1). 그렇지 않으면 대기로 남는다. 변조 뒤의 새 세션은 변조된 세션과 다른 session_id여야 한다. 탐지된 설정 변조는 그것을 담은 takeover가 받아들여질 때까지 볼륨에 대기로 남는다. 그동안은 재개하지 않고 다음 새 세션과 함께 보고한다(§5-6). 대기 파일이 있는데 읽을 수 없거나 JSON 객체가 아니면, 새 탐지가 그것을 덮어쓰지 않는 한 start·reconnect를 `EXTERNAL_RECONCILIATION_REQUIRED`로 거절하고 파일은 사람이 대조할 때까지 남긴다(보고되지 않은 탐지를 잃지 않게. [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md), X-08). 회차 뒤 보고가 실패하면 상태의 `checkpoint_reported: false`로 드러난다(마지막 보고 결과는 볼륨에 남아 재시작 뒤에도 같다). 이것은 repo 범위 stop의 안전 종료 확인을 막는다(명시적인 true만 보고됨으로 본다). 보관 사본이 `checkpoint.json`보다 정확히 하나 뒤면(두 파일 쓰기 사이의 중단) 빠진 사본을 `checkpoint.json`에서 쓴다. `checkpoint.json`이 보관 사본보다 뒤인 경우(되돌림)는 되살리지 않고 대조에서 드러난다. 상태의 `checkpoint_reported`는 중앙 기록이 이 체크포인트(순번·digest)를 가진 것으로 알려졌을 때만 true다(받아들여진 보고나 성공한 대조). **코디네이터의 재개 판단**: 역할 상태의 로컬 순번·digest와 중앙 기록·현재 배정을 비교한다. 결과는 `resume_allowed` / `no_checkpoint` / `already_running` / `stale_generation` / `external_reconciliation_required` / `not_observed` 중 하나다. 같은 배정의 새 세대면 `stale_generation`, 다른 배정이면 `external_reconciliation_required`다(revision 11: 낮은 세대의 마지막 기록이 `after_run`이 아니면 `external_reconciliation_required`, §14 항목 13). 마지막 기록이 `after_run`이 아니면 재개하지 않는다. 테스트용 `local_only`는 설정에 명시해야 하고 상태에 능력 공백으로 보고한다 | §2.1 정본 원칙, §5-1·§5-2·§5-6, 02-04 수용 보고 §4 항목 9 | §2.1, §5-1·5-2·5-6, §7 reconnect·status | 코디네이터 `role_checkpoints`·`role_tamper_events`(추가 전용), API 3개(보고·최신·재개 판단). 실제 DB와 가짜 런타임 시험: 정상 보고, 중복, 순번 간격, 내용 충돌, 식별자 불일치, 낡은 세대, 변조, 코디네이터 재시작·역할 프로세스 재시작 뒤 대조, 회차 중 코디네이터 중단 뒤 늦은 보고(진행 기록 9 §3). 한계: 개발 컨테이너 안의 체크포인트와 보고 자격은 같은 UID의 개발 코드가 바꿀 수 있다(§2.1, 검토 N5). 그래서 재개의 최종 근거는 GitHub SHA·CI 대조이고, 그 연결은 02-05다. 배정·인수 흐름에서 코디네이터가 reconnect를 부르는 것도 02-05다. 알려진 한계(X-08 Q-3, [사용자 결정 2026-11-15](../../../../User_Decisions_2026-11-15_0205-x08.md)): 변조 탐지는 reconnect에서만 한다. start는 회차 전 설정 정리를 거치는 명시적 새 세션이므로 변조가 그 세션에 적용되지는 않지만, start 전에 생긴 변조는 탐지·보고되지 않는다 |

## 14. 02-05 배정 전달 반영 (revision 11)

근거: [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3 단계 3(C8·C1·C12), [단계 3 보고](../../02-05/step3-delivery-20261115/REPORT.md). 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 11 | **같은 시도의 재전송.** start·reconnect를 받아들이면(202) supervisor는 실행 전에 그 답과 요청 digest(파싱한 JSON 값의 SHA-256)를 역할 볼륨(`/state/attempts/<attempt_id>.json`)에 둔다. 같은 `attempt_id`로 같은 연산·같은 내용이 다시 오면 저장된 답을 200(`replayed: true`)으로 돌려주고 아무것도 시작하지 않는다. 다른 내용·다른 연산이면 409 `ID_CONTENT_CONFLICT`다. 거절된 요청은 저장하지 않는다(상태가 없으므로 다시 보내면 다시 판단한다). `GET /v0/role/attempts/<attempt_id>`는 저장된 답을 읽기만 한다(없으면 404). 답을 잃은 호출자는 이것으로 실행 여부를 확인하고, 실행을 일으킬 수 있는 요청을 다시 보내지 않는다 | §7 start·reconnect, 오류 대응표 | supervisor `serve` 시험(같은 바이트·같은 값의 다른 바이트 재전송 200, 다른 내용·다른 연산 409, 저장된 답 조회, 거절은 저장 안 함, 재시작 뒤 재전송) |
| 12 | **현재 배정과의 대조.** 중앙 기록이 코디네이터일 때 supervisor는 start·reconnect의 중앙 체크포인트 대조(§13 항목 10) **전에** 자기 배정 조회(`getOwnAssignment`/`GetOwnAssignment`)로 요청의 코디네이터 기록을 대조한다. 배정·세대·sprint revision·명세 digest가 현재 배정과 같아야 하고, 코디네이터가 그 배정의 전달을 기록했으면 그 전달이 기대하는 이미지 digest·프로파일 ID·revision과도 같아야 한다. 같은 배정의 낮은 세대는 `STALE_GENERATION`, 그 밖의 차이와 현재 배정 없음은 `EXTERNAL_RECONCILIATION_REQUIRED`, 코디네이터에 닿지 않으면 `NOT_OBSERVED`다. 어느 쪽도 실행하지 않는다. 대상 SHA는 이 대조에 넣지 않는다(takeover 저장 조건 (b)가 거부한다). `local_only`는 대조하지 않는다(능력 공백) | §5-2, §7 start·reconnect | supervisor `serve` 시험(두 전송, 항목별 불일치, 낡은 세대, 이미지·프로파일 기대값), 코디네이터 종단 시험 |
| 13 | **코디네이터의 배정 전달.** 코디네이터는 운영 주체 작업 `deliverAssignment`(HTTP 전용, `roles:control`)로 start·reconnect를 부른다. 순서: 전달 요청 기록(verified) → 재개 판단(`compare`: `no_checkpoint`→start, `resume_allowed`→reconnect, 명시적 새 세션 요청이면 `resume_allowed`와 마지막 회차가 끝난(`after_run`) 낮은 세대의 `stale_generation`에서도 start, 그 밖은 부르지 않음. revision 14부터 `resume_allowed`는 개발 역할에서 GitHub head 대조(§17 항목 22)를 통과한 뒤의 값이므로, 같은 세대의 명시적 새 세션도 그 대조를 통과해야 start다(통과하지 못하면 그 판단으로 부르지 않음). 새 세대(`stale_generation`)의 새 세션은 대조하지 않는다. 낮은 세대의 끝나지 않은 회차는 판단 자체가 `external_reconciliation_required`다) → 예산 검사 → 예산 잠금을 쥔 뒤 재개 판단을 다시 하고 달라졌으면 부르지 않음 → 호출 → 답 또는 `unknown` 기록. 예산 계획의 역할은 그 repo에 설정된 역할과 같아야 한다(다르면 거부). 요청의 코디네이터 기록·대상 SHA·한도는 코디네이터 DB와 설정에서 만든다. **예산 검사의 권한은 역할 컨테이너 밖에 있다**: 02-04 호스트 호출자와 같은 코드(`caller::admit`·`Admission::start`·`Running::finish`)로 원장 판독, 사람 확인 요청 기록, 미계측 회차, 추가 결제 없음 관측, 예산·역할 몫 한도, 모든 역할 서비스의 유휴·회차 인지, 회차 뒤 계측·과금 기록을 거친다. supervisor의 `budget`·`share`는 보조 상한으로 남는다(§2.1). 답을 잃으면 그 시도는 원장에 사람 확인 요청으로 남고(02-04 규칙 그대로), 같은 전달 요청을 다시 보내면 저장된 전달을 돌려준다. 저장된 결과가 `unknown`이거나 120초가 지나도록 결과가 없을 때만 항목 11의 조회를 하고(결과 없이 120초 안이면 진행 중으로 거절), 조회 답은 이 전달이 보낸 요청 digest와 같을 때만 `accepted`로 기록한다. 전달 응답은 인수가 아니다(인수는 supervisor의 takeover 보고, 02-05 단계 4) | §2.1, §5-2, §7, §13 항목 10의 “02-05” 한계 | 코디네이터 종단 시험(두 전송): 체크포인트 없음→start, after_run→reconnect, 실행 중·외부 대조·session_id 바꾼 기록·관측 불가·낡은 세대 → 부르지 않음, 예산 거부(미계측·역할 몫) → 부르지 않음, 답 유실 → `unknown` 뒤 재조회로 `accepted`(실행 1회), 코디네이터 재시작 뒤 같은 답 |
| 14 | **재개 키와 지시성 digest의 확인.** (a) 매핑 레코드(§4.2)의 논리 이름은 코디네이터의 전달 기록에만 남는다. reconnect는 체크포인트·중앙 기록의 session_id로만 재개한다(§10 항목 5). 논리 이름만 바꾼 전달도 같은 session_id로 재개되고, 이름이 같아도 session_id가 다른 로컬 기록은 중앙 기록과 digest가 달라 외부 대조로 멈춘다. (b) 지시성 digest(§5-6)는 파일과 링크로 계산한다. CLI가 스스로 만드는 빈 `projects/*/memory/` 디렉터리는 대상이 아니고(변조 아님, 정리가 지움), 그 안의 파일은 변조다. 지시성 대상 밖의 파일은 열지 않는다(읽을 수 없는 파일도 digest와 정리를 막지 않는다) | §4.2, §5-6, §10 항목 3·5 | (a) 코디네이터 종단 시험과 코드 검토(`checkpoint::decide`는 session_id만 재개 키로 씀, `args::build`의 `-n`은 표시 이름). (b) supervisor 시험(빈 memory 디렉터리, 권한 000 파일) |

## 15. 02-05 인수·첫 행동 반영 (revision 12)

근거: [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 4(수용 기준 본문, C1·C14), [단계 4 보고](../../02-05/step4-chain-20261116/REPORT.md). 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 15 | **첫 행동.** 정의: 회차 stream에서 `system`/`init` 사건 **다음**의 첫 `assistant` 사건(자기 세션의 것, `parent_tool_use_id` 없음) 중 `tool_use` 블록 또는 비지 않은 `text` 블록이 있는 것. 종류는 `tool_use` 블록이 하나라도 있으면 `tool_use`, 아니면 `text`다. `init` 앞의 사건, `init` 자체, 내용 없는·`thinking`만 있는 `assistant`, `user`·`result`·`rate_limit_event`, 자식 사건은 행동이 아니다. 오프셋은 회차 `stdout.jsonl`의 0부터 센 줄 번호, 시각은 supervisor가 그 줄을 읽은 시계 시각이다(사건에 시각이 없다). supervisor는 관측하자마자 `recordFirstAction`(HTTP·gRPC)을 다른 스레드에서 보낸다. 코디네이터는 그 세션의 takeover가 그 시도를 부른 전달에 연결돼 있을 때만 받는다(항목 17의 채움과 같은 조건, revision 18). 보고 실패는 회차를 멈추지 않는다. after_run·error 체크포인트의 `first_action`에 같은 값(`attempt_id`·`event_offset`·`observed_at`·`kind`)을 싣는다. 행동 없이 끝난 회차의 after_run은 `"none"`이다. 0이나 시작 시각으로 채우지 않는다. takeover와 행동을 관측하지 못한 error는 null(모름)이다. 상태의 `last_actual_action_at`은 이 서비스가 관측한(또는 볼륨의 회차 결과에 남은) 가장 새 첫 행동 시각이다. 관측이 없으면 null이다 | §5-1, §7 status | supervisor 단위 시험(정의의 각 경우, 실시간 관측과 사후 판독의 일치), 종단 시험(보고, 행동 없음, 보고 유실 뒤 after_run으로 채움) |
| 16 | **인수와 전달의 연결.** 코디네이터의 전달은 start·reconnect 본문에 `delivery_id`를 싣는다. supervisor는 그 값과 그 회차의 `attempt_id`를 회차의 모든 체크포인트에 쓴다. 코디네이터는 takeover 체크포인트를 저장할 때 §13 항목 10의 (a)~(c)에 더해 다음을 본다: 체크포인트가 전달을 이름 붙인다(없으면 409 `DELIVERY_REQUIRED`). 그 전달이 같은 repo·역할·배정·세대의 것이고, 그 전달이 부른 `attempt_id`가 체크포인트의 `attempt_id`와 같으며(takeover는 그 전달 자신의 호출에 답한다. 전달 id만 이름 붙인 다른 시도는 그 전달을 소비하지 못한다), 마지막 결과가 아직 없거나(supervisor는 코디네이터가 답을 기록하는 동안 takeover를 보고한다) `accepted`·`unknown`이며, 아직 인수되지 않았다(아니면 409 `DELIVERY_MISMATCH`). 인수 행(전달, 체크포인트 순번·digest, session_id, 적용한 sprint revision·digest, 수신 시각)은 체크포인트 행과 **같은 트랜잭션**에서 쓴다. 거부는 어느 행도 남기지 않는다. 전달 요청만으로는 인수가 아니다. 전달 없는 start·reconnect(02-04 호스트 호출자가 supervisor에 직접 보낸 것)는 중앙 기록이 코디네이터이면 takeover가 거부되어 실행되지 않는다(시도는 실행되지 않은 것으로 닫힘). `local_only`는 이 검사가 없다(시험용) | §5-1, §7 start·reconnect, §13 항목 10 | 코디네이터 종단 시험: 정상 연결, 전달만 있고 인수 없음, 전달 없는 takeover 거부(행 0), 다른 세대·아무것도 보내지 않은 전달·이미 인수된 전달 거부, 낡은 세대 전달 뒤 새 세대 인수 |
| 17 | **after_run의 첫 행동으로 채움.** after_run·error 체크포인트의 `first_action`이 객체이고 그 시도의 첫 행동 기록이 없고, 체크포인트의 `attempt_id`(있으면)가 첫 행동의 것과 같고, 그 세션의 저장된 takeover가 그 시도를 부른 전달에 연결돼 있으면, 코디네이터는 체크포인트를 저장하는 트랜잭션에서 같은 본문의 첫 행동을 기록한다(`source: checkpoint`). 이미 있는 기록은 바꾸지 않는다(다르면 연결 조회가 둘 다 보인다). 뒤늦게 온 supervisor 보고는 같은 본문이므로 200이다. 인수·첫 행동·체크포인트는 모두 claim이다(§2.1). 완료 판정의 입력이 아니다 | §5-1, §13 항목 10 | 코디네이터 종단 시험(보고 유실 → after_run으로 채움, 뒤늦은 같은 보고 200, 다른 내용 409) |

## 16. 02-05 외부 요청 추적 반영 (revision 13)

근거: [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 6(C9, 02-04 수용 항목 3), [단계 6 보고](../../02-05/step6-external-20261116/REPORT.md). 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 18 | **추적하는 외부 요청.** 역할이 supervisor를 거쳐 코디네이터에 보낸 요청 중 답을 모르는 것이다: 체크포인트 보고(`checkpoint_report`, 대기 사본으로 남은 takeover 포함)와 첫 행동 보고(`first_action_report`). 보내기 전에 목록(역할 볼륨 `/state/pending-requests.json`)에 넣고, 코디네이터가 답하면(저장·중복·4xx 거부) 지운다. 서버 오류·답 없음은 남는다. **목록에 넣지 못한 요청은 보내지 않는다**(파일을 읽거나 쓸 수 없을 때): 체크포인트 보고는 `EXTERNAL_REQUEST_NOT_TRACKED`(503)로 실패하고, takeover이면 그 회차는 실행되지 않는다(`CentralRefused`, 모델 호출 없음). 첫 행동 보고는 `not_sent`로 남고 회차는 계속된다(after_run 체크포인트가 같은 값을 싣는다). 거절 사유와 답의 `file`, 상태의 `safe_shutdown.pending_external_requests_file`이 목록 파일을 가리킨다(revision 14). **받기 전 확인(revision 14)**: supervisor는 start·reconnect를 받기 전에(실행 관문 안에서) 목록을 읽고 그대로 다시 써 본다. 실패하면 바로 409 `EXTERNAL_RECONCILIATION_REQUIRED`(사유와 `file`에 파일 경로)로 거절하고 시도 id를 쓰지 않는다. 그래서 실행되지 않은 회차가 호출자의 원장에 닿지 않고, 위의 “보내지 않음”은 받은 뒤 회차 중에 목록이 깨진 경우만 남는다(`local_only`는 확인하지 않는다). 중앙 대조가 보관본에서 보내는 누락 체크포인트 보고는 목록 밖이다: 대조가 곧바로 코디네이터 기록을 다시 읽어 결론을 내므로 목록이 필요 없다. **재조회**: start·reconnect 전 중앙 대조(§13 항목 10)가 로컬과 중앙을 맞추면 목록의 체크포인트 보고는 모두 결론이 난 것이다(반영됨, 또는 반영 안 됨이 확인되어 대기 사본을 버림). 첫 행동 보고는 같은 본문으로 다시 보낸다. 재조회 뒤에도 목록이 비어 있지 않거나 목록을 읽을 수 없으면 **reconnect는 `EXTERNAL_RECONCILIATION_REQUIRED`로 멈추고** 목록을 답에 싣는다(§5-1·§6). start는 남은 목록이 있어도 진행한다. 목록을 읽거나 쓸 수 없으면 start도 받지 않는다(받기 전 확인, 사람이 파일을 고칠 때까지). 상태의 `safe_shutdown.pending_external_requests`와 체크포인트의 `pending_external_requests`(쓸 때의 목록, `tracked.pending_external_requests: true`)는 이 목록이다. 체크포인트의 목록은 기록이고, 재개 판단은 재조회 뒤의 현재 목록으로 한다. 보고 v0의 진행 보고와 재현 요청은 지금 supervisor를 거치는 코드가 없어 목록에 생길 수 없다. 그 경로가 생기면 같은 목록에 든다. **한계(같은 UID)**: 목록 파일은 개발 작업 디렉터리와 같은 역할 볼륨에 있고 모델은 supervisor와 같은 UID로 돈다. 개발 코드가 목록을 지우거나 고칠 수 있으므로(§2.1의 체크포인트와 같은 N5 한계) 목록은 무결성 보장이 아니다. 재개의 최종 근거는 코디네이터 기록(전달·takeover·첫 행동)과 02-05 단계 7의 GitHub 대조다 | §5-1, §6, §7 reconnect·status | supervisor 시험(두 전송): 답 없는 첫 행동 보고가 목록에 남음, 재시작 뒤 같은 목록, 목록이 있는 reconnect 거부(실행 없음), 재조회로 지워진 뒤 재개, 답을 잃은 takeover 보고가 목록에 남았다가 다음 start의 대조로 지워짐, 4xx로 거부된 첫 행동 보고가 재조회로 지워지고 reconnect 재개, 목록에 넣을 수 없는 요청은 보내지 않음(재조회·takeover), local_only는 목록 없음 |
| 19 | **관측할 수 없는 것.** 모델이 Bash·도구로 직접 낸 네트워크 요청은 supervisor가 관측할 수 없다. 상태와 체크포인트에 `direct_network_requests: "not_observable"`로 따로 적고 “없음”으로 쓰지 않는다(능력 공백에도 적음). 안전 종료 확인의 사유에는 넣지 않았다. 자식 추적(04-05)이 생겨 다른 사유가 모두 풀릴 때 이것을 확인 조건에 넣을지는 04-05에서 정한다. GitHub push 단기 토큰 경로(GH1)는 05-03 범위다 | §7 status | supervisor 시험(상태·체크포인트 값) |
| 20 | **repo 범위 정지 보고.** 각 역할의 미확인 외부 요청은 목록이 비어 있지 않을 때 사유다. 코디네이터는 역할 정지 뒤 읽기 트랜잭션 하나로 그 repo의 **결과를 모르는 전달**(결과 없음 또는 `unknown`)을 `coordinator_pending_external_requests`에 싣고 사유로 적는다. 그 읽기가 실패해도 정지는 이미 보냈으므로 같은 정지 보고(200)를 돌려준다: 목록은 `"unreadable"`, 사유에 DB 오류, 확인은 `false`다(PR #78 검토 1b362ec M-1). `active_children`은 04-05까지 `not_tracked`이므로 `safe_shutdown_confirmed`는 외부 요청이 0이어도 `false`이고 사유에 자식 미추적이 남는다(사용자 결정 2026-11-15 항목 3) | §7 stop(repo 범위) | 코디네이터 시험: 역할 목록이 사유로 나타남, 빈 목록이면 자식 사유만 남음, 종단 시험에서 결과를 잃은 전달 둘이 정지 보고에 나타남(`no result recorded`, 재질의 뒤 `result unknown`), DB 연결이 끊긴 정지도 200·두 역할·`unreadable`·미확인 |

## 17. 02-05 GitHub 대조 반영 (revision 14)

근거: [02-05 계획](../../02-05/plan-20261115/PLAN.md) §3.2 단계 7(C10), [단계 7 보고](../../02-05/step7-github-20261116/REPORT.md). 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다. **대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다.** GitHub head·CI는 증거 입력 서비스(`evidence:ingest`)가 넣은 신뢰 입력이다. 실제 수집기·신원(F4)·PR 수준 상태(N5)는 1b 작업이 이 입력 자리에 넣는다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 21 | **supervisor가 기록하는 것.** 모든 체크포인트(after_run 포함)에 `worktree_head`(작업 디렉터리 `HEAD`의 SHA, 아니면 `"not_observed"`), `branch_ref`(start·reconnect 요청의 배정 브랜치 `refs/heads/<name>`, 없으면 null), `pushed_sha`(`refs/remotes/origin/<name>`의 SHA, push가 없거나 읽을 수 없으면 null)를 적는다. supervisor는 작업 디렉터리에서 `git`을 실행하지 않는다(저장소 설정이 코드를 실행할 수 있다). `.git`의 ref 파일(`HEAD`, 느슨한 ref, `packed-refs`, `gitdir`·`commondir`)만 읽고 소문자 16진 SHA만 받는다. 읽을 때 심볼릭 링크와 절대 경로 `gitdir:`·`commondir`를 따라간다. 이것이 안전한 근거는 같은 UID·같은 컨테이너 가정뿐이다: 모델이 읽을 수 있는 것 이상을 읽지 않고, 결과는 SHA 꼴만 받으며 GitHub 기록과의 대조를 거친다(PR #79 검토 f0e0ca9 I-2). 이 값들은 같은 UID의 개발 코드가 바꿀 수 있는 claim이다(§2.1) | §5-1 | supervisor 시험: 저장소 없음 → `not_observed`·null, push한 회차 → HEAD·push SHA·브랜치, 다른 브랜치 → null, 잘못된 `branch_ref` → 400; ref 파일 단위 시험 |
| 22 | **개발 역할의 재개(최종 근거).** 코디네이터의 재개 판단(compare)이 §13 항목 10의 대조로 `resume_allowed`가 될 때, 배정의 `branch_ref`가 있고 그 브랜치의 최신 `branch_head` 증거가 중앙 체크포인트의 `pushed_sha`(null이면 배정 `target_sha`)와 같아야 `resume_allowed`로 남는다. 다르면(로컬 ref를 옮긴 경우 포함) `external_reconciliation_required`, 배정에 브랜치가 없거나 기록이 없으면 `not_observed`, `pushed_sha`가 SHA도 null도 아니면 외부 대조다. 어느 쪽도 재개하지 않는다. 이 대조는 `resume_allowed`에서 고르는 모든 호출에 걸린다: reconnect와, 같은 세대의 명시적 새 세션(start)이다(같은 세대의 새 세션은 같은 브랜치·작업 디렉터리를 이어 쓰므로 확인된 head에서 시작해야 한다, PR #79 검토 f0e0ca9 M-1, 개발자 결정, 닫는 쪽). `no_checkpoint`의 start와 새 세대(`stale_generation`)의 새 세션은 대조하지 않는다. 체크포인트의 `branch_ref`가 배정의 것과 다르면 그 push SHA는 이 브랜치의 것이 아니므로 외부 대조다(f0e0ca9 L-2). 체크포인트에 브랜치가 없으면 push를 관측하지 않은 것으로 보고 그 `pushed_sha`가 있어도 읽지 않으며 target SHA를 기대한다(기록의 `push_provenance`, revision 15에서 코드를 맞춤, 검토 47e4e3f L-4). 이 대조는 하나의 검사(배정·세대·체크포인트 → `match`/`mismatch`/`not_observed`와 그 증거, 입력이 증거 입력 기록이라는 표시)로 판단 답의 `github`에 실리고, 공식 복구(02-05 단계 7b, [사용자 결정 2026-11-16](../../../../User_Decisions_2026-11-16_recovery.md))가 같은 검사를 쓴다 | §5-2, §13 항목 10 | 코디네이터 단위 시험(판단 표), 종단 시험: 기록 없음 → `not_observed`, head 일치 → 재개, push 뒤 head 불일치 → 외부 대조, 로컬 체크포인트 변조 → 외부 대조, 로컬 ref만 옮긴 push 주장 → 외부 대조 |
| 23 | **CI와 CI 주장.** CI는 그 head SHA의 `ci_run` 증거만 본다(`source_ref`마다 최신, 모두 성공 → `success`, 하나라도 아니면 `failure`, 없으면 `none`). 다른 SHA의 CI는 읽지 않는다. 결과는 판단 답의 `github`와 전달 결과에 기록하고, 재개 전달 본문에 `github` {branch_ref, head_sha, ci}로 싣는다. 체크포인트에는 CI 주장이 없다(supervisor는 CI를 관측하지 않는다). 그 배정의 최신 `development_progress` 보고(claim)가 그 head SHA에 대해 `internal_checks`로 CI를 주장하고(FAIL → 실패, 모두 PASS → 성공) GitHub 기록과 다르면 GitHub 기록을 쓰고 `ci_claim.matches_github: false`로 기록한다. CI 결과는 재개를 막지 않는다 | §5-2 | 종단 시험: 다른 SHA의 실패 무시, head SHA의 성공이 전달 결과와 본문에, 다른 CI 주장이 불일치로 기록됨, 새 head의 CI는 옛 head의 것을 쓰지 않음 |
| 24 | **Guardrail의 검토 대상.** Guardrail 배정의 `target_sha`가 현재 개발 배정의 `target_sha`가 아니면(개발 배정이 없을 때 포함) 판단이 `resume_allowed`·`no_checkpoint`·`stale_generation`이었어도 `stale_target`이다. 전달은 아무것도 부르지 않는다. 새 검토 배정이 필요하다 | §5-2 | 종단 시험: 낡은 대상 → `stale_target`, 전달 `not_called`, 새 배정 뒤 `no_checkpoint` |

## 18. 02-05 공식 복구 반영 (revision 15)

근거: [사용자 결정 2026-11-16 (나)](../../../../User_Decisions_2026-11-16_recovery.md) §2, [단계 7b 보고](../../02-05/step7b-recovery-20261117/REPORT.md). 모델 호출 없이 가짜 런타임과 실제 PostgreSQL로 시험했다. GitHub 대조는 단계 7의 것을 쓰고, **대조 대상은 증거 입력으로 받은 기록이며 실제 수집이 아니다.**

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 25 | **승인하는 사람.** 복구 승인(`approveRecovery`)은 `roles:control`을 가진 `user` 자격(사람)만 낸다. 서비스 자격은 범위가 있어도 403 `HUMAN_APPROVAL_REQUIRED`, 에이전트 자격(그 역할 자신의 개발·Guardrail 자격 포함)은 403이다. 사람과 서비스의 구분은 자격 설정의 `kind`다(실행 예외 승인과 같은 근거) | §3, §5-2 | 종단 시험: 서비스·supervisor·에이전트·범위 없는 사용자·다른 repo 사용자 거절, 기록 없음 |
| 26 | **승인의 대상과 기록.** 승인은 현재 배정·세대와 그 역할의 최신 중앙 체크포인트(순번·digest, 단계가 takeover 또는 error)를 이름으로 대고 사유를 단다. 다르면 409 `RECOVERY_TARGET_MISMATCH`(Guardrail은 검토 대상이 현재 개발 target이 아니어도). 회차가 실행 중이면 409 `RECOVERY_RUNNING`. 거절은 아무것도 기록하지 않는다. 받아들이면 승인자·자격 종류·사유·시각·대상·읽은 사실·GitHub 대조를 추가 전용으로 기록한다. 실행 종료나 외부 요청 결과를 확인할 수 없으면(supervisor 미관측, 로컬≠중앙, 대기 외부 요청이 빈 목록이 아님, 결과 모르는 전달, 개발 역할의 GitHub head가 `match`가 아님) `needs_confirmation`(보류)으로 근거와 함께 기록하고 쓸 수 없다. 자식은 04-05까지 추적하지 않으므로 보류 사유가 아니라 기록의 한계로 남긴다 | §5-2, §6 | 종단 시험: 실행 중 거절, 대상 넷 거절, 보류 넷(미관측·GitHub 불일치·대기 외부 요청·결과 모르는 전달), 승인 기록 내용, 같은 키 재요청 |
| 27 | **쓰기.** 전달(`deliverAssignment`)이 `recovery_id`를 대면 판단마다(예산 잠금 뒤 포함) 승인을 다시 대조한다(체크포인트·세대·head가 승인 뒤 바뀌었을 수 있다). 쓸 수 있으면 판단은 `recovery_allowed`, 호출은 그 전달의 새 세션 `start`이고 재개는 없다. 아니면 아무것도 부르지 않는다. 사용은 start를 보내기 직전에 커밋한다: 승인 하나에 사용 하나다(같은 요청 재전송·답 유실·재시작에도 그 승인으로 두 번째 실행은 없다). 체크포인트를 **소비하는** 것은 전달이 실행했을 수 있는 사용이다: 전달의 최신 결과가 `accepted`·`unknown`이거나 아직 없음. 결과가 실행하지 않았음을 말하면(`refused`·`not_called`·`budget_refused`, 또는 재조회가 supervisor가 그 시도를 모른다고 확인한 `not_accepted`) 소비하지 않는다(PR #80 검토 185d12b M-1). 소비된 체크포인트의 새 승인은 그 전달을 근거로 단 `needs_confirmation`(보류)이고, 쓰기는 복구 잠금 아래에서 다시 확인해 부르지 않는다: 체크포인트당 실행은 많아야 한 번이다. 사용이 기록된 뒤 호출 전에 코디네이터가 멈추면 그 전달은 결과가 없으므로 체크포인트를 소비한 채로 남고(닫는 쪽), 그 전달의 재조회가 `not_accepted`를 기록하면 풀린다. 풀리지 않는 경우가 둘 있다(닫는 쪽, 해제 경로는 02-05 단계 9로 넘김, PR #80 검토 d336849 L-c): supervisor가 start를 받았으나(202) 코디네이터가 결과를 기록하기 전에 멈추고 그 회차의 takeover 보고도 실패해 실행되지 않은 경우(재조회는 보낸 digest가 없어 늘 `unknown`), 그리고 `accepted`인데 takeover가 거절된 경우다. 둘 다 그 체크포인트의 새 승인은 그 전달을 근거로 단 보류로 남는다 | §5-2, §14 항목 13 | 종단 시험: 보류는 쓰이지 않음, 승인 뒤 head 변경 → 부르지 않음·미사용, 정상 사용(새 세션), 같은 요청 → 저장된 전달, 다른 전달 → 이미 사용, 답 유실·재조회·두 재시작 뒤에도 한 번 |
| 28 | **supervisor.** start는 로컬 체크포인트가 takeover·error이면 `recovery_id`가 없을 때 409 `EXTERNAL_RECONCILIATION_REQUIRED`다. `recovery_id`가 있으면 새 세션을 시작하고 그 회차의 체크포인트에 `recovery_id`를 적는다. supervisor는 id를 스스로 대조하지 않는다. 코디네이터 모드에서는 끝나지 않은 회차(최신 중앙 체크포인트가 takeover·error) 위의 takeover 보고가 그 전달이 쓴 승인과 같은 `recovery_id`를 대야 한다. 아니면 409 `RECOVERY_REQUIRED`이고 아무것도 기록되지 않으므로 그 회차는 실행되지 않는다(PR #80 검토 185d12b L-1). 그 사용은 바로 그 체크포인트(최신 중앙 체크포인트의 순번)의 것이어야 하고 승인의 배정·세대가 보고의 것과 같아야 한다: 다른 체크포인트의 사용은 이 체크포인트를 덮지 않는다(검토 d336849 L-a). 02-04 호스트 호출자 `ieo-role-call`은 `--recovery-id`로 id를 보낸다(L-2). `local_only`에는 코디네이터가 없어 id를 대조하지 않는다: 어댑터 호출자(신뢰 경계 안)의 기록된 주장이다 | §5-1, §7 start | supervisor 시험: takeover·error 위 start 거절, 잘못된 id 400, id가 있으면 새 세션과 체크포인트의 id, 끝난 회차 뒤에는 필요 없음 |
| 29 | **사용량과 예산.** 승인은 사용량을 바꾸지 않고 예산을 초기화하지 않으며 호출을 승인하지도 않는다. 복구 전달도 배정 한도(미계측이면 거절)와 공유 예산 검사를 그대로 거친다. 이전 체크포인트·전달·증거는 그대로 두고 복구 이력을 더한다(배정 연결에 `recoveries`와 각 전달의 `recovery`) | §13 항목 10, §14 항목 13 | 종단 시험: 승인 전후 사용량 같음, 미계측 기록 → 예산 거절·미사용, 답 유실 뒤 미계측으로 다음 전달 거절, 충돌한 체크포인트가 연결에 남음 |

## 19. 02-05 예산의 예비 표식 (revision 17)

근거: [사용자 결정 2026-11-17](../../../../User_Decisions_2026-11-17_spare.md) (나)와 그 추가 결정, [02-05 예산 보고](../../02-05/budget-0205-20261117/REPORT.md). 모델 호출 없이 가짜 런타임으로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 30 | **예비 표식.** 02-05 예산에서 개발은 표식 없이 최대 2회 실행하고, 예비 1회는 표식을 달고 이 예산의 실패한 개발 회차(R1 또는 R2)를 대체한다. 표식은 사유, 대체할 시도 id, 원인 보고·증거 참조(`evidence_ref`), 승인한 사람(`by`, `kind: user`), 승인 시각(`approved_at`)이다. 표식은 코디네이터의 `deliverAssignment`로만 붙는다: `user` 자격만(아니면 403 `HUMAN_APPROVAL_REQUIRED`), 사람과 시각은 코디네이터가 적는다(`approved_at`은 그 요청을 코디네이터가 처리한 시각이다: 별도의 승인 시각이 아니라 인증된 사람의 요청 시각). 호스트 호출자 `ieo-role-call`은 인증된 신원이 없어 예비 플래그(`--spare-reason`·`--replaces`·`--evidence-ref`·`--approver`)를 모두 거절한다(아무것도 읽거나 시작하기 전, exit 1). 표식은 이름 댄 실패 회차의 결과로 인한 멈춤만 풀고, 미계측 사용량·한도·overage 신호·정리 실패·구독 전용이 아닌 환경·다른 회차의 멈춤은 그대로 막는다. 표식은 그 예비 회차의 장부 기록에 더해지고 실패 기록과 사용량은 그대로다. 표식은 한 번만 쓰이고 실패한 예비는 다시 멈춘다. **역할 서비스로 가는 start 본문에는 표식이 없다**: 역할 서비스의 계약은 바뀌지 않는다 | 공유 예산 검사(`caller::spare_gate`, `spare_releasable`), 전달(`deliverAssignment`), `ieo-role-call`(거절) | `caller.rs` 예비 시험들(`ieo_role_call_refuses_a_spare_marker` 포함), `delivery_e2e` 8단계, `delivery.rs` 단위 |

## 20. 02-05 한도 경고 판정 (revision 19)

근거: [사용자 결정 2026-11-17 경고](../../../../User_Decisions_2026-11-17_warning.md) (다), [02-05 재개안 부록 A](../../02-05/step8-plan-20261117/RESUME_PLAN.md#부록-a-한도-경고-판정의-근거-정책-전후-시험). 공식 문서의 `SDKRateLimitEvent`(상태 `allowed | allowed_warning | rejected`)와 CLI 2.1.322 실행 파일의 형 정의(`overageStatus`·`isUsingOverage`·`unifiedWindows`), 02-05 `s8-r1`이 실제로 남긴 사건이 근거다. 모델 호출 없이 실제 사건 재생과 가짜 런타임으로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 31 | **한도 신호 판정(하나).** `rate_limit_event` 하나는 다음 중 하나라도 맞으면 **멈춤**이다(순서대로, 처음 맞은 것이 사유): `rate_limit_info`가 객체가 아님, `status`가 없거나 `allowed`·`allowed_warning`·`rejected`가 아님, `status: rejected`, `overageStatus`가 문자열 `rejected`가 아님(없음 포함), `isUsingOverage`가 불리언 `false`가 아님(없음 포함), `overageInUse`나 `overageEnabled`가 있고 `false`가 아님(사용 크레딧이 켜져 있으면 추가 유료 사용이 가능), `errorCode`가 있음, `rateLimitType`이 모르는 값이거나 `overage`·`seven_day_overage_included`(추가 사용 거부와 모순), `unifiedWindows`가 객체가 아니거나 그 안의 창이 객체가 아님·상태를 읽을 수 없음·`rejected`(CLI는 최상위 상태가 거부 여부를 정한다고 적지만, 다 쓴 창은 보수적으로 멈춤). 그 밖에 최상위나 창의 `allowed_warning`이 있으면 **경고**(진행, 기록), 아니면 **허용**이다. 이 판정을 실행 가드(그 사건에서 프로세스 멈춤), 사전 관측 검사(최신 관측 stream의 모든 사건, 7일 유효), 공유 예산 검사와 역할 서비스의 상한, 회차 기록(멈춤은 `stop_required`, 경고는 `rate_limit_warnings`), 상태 표시(`available` + `warning`), 실행 스크립트(`check-run.py`·`obs-check.sh`가 같은 규칙의 Python 사본)에 똑같이 쓴다. 한 stream 안에서 나중의 경고는 앞의 멈춤을 지우지 못한다. 가드가 멈춘 회차는 그대로 미계측이고 미계측 회차는 다음 호출을 막는다. 시간·토큰·회차 상한과 추가 청구 USD 0은 그대로다 | supervisor `budget::rate_limit_verdict`(`overage_check`·`observation_check`·`stream_stop`이 이것을 씀), `caller`의 회차 기록, `provider`, `launcher/rate_limit_policy.py`(`check-run.py`·`obs-check.sh`) | `tests/rate_limit_policy.rs`(공유 사례 31개를 Rust와 Python이 같게 판정, 실제 `s8-r1` 사건, 나중 경고가 멈춤을 못 지움, 보관 stream 재생), `caller.rs`(경고 회차 계측·기록·다음 호출, 경고 뒤 거부·추가 사용·누락은 가드 멈춤·미계측·다음 호출 막힘), `run_guard.rs`, 변이 22/22 |

## 21. 02-05 가드 멈춤 회차의 정산 (revision 20)

근거: [사용자 결정 2026-11-17 정산](../../../../User_Decisions_2026-11-17_r1-settlement.md) (가)·(A), [재개안 부록 B](../../02-05/step8-plan-20261117/RESUME_PLAN.md#부록-b-정산-형식ieo-role-run-settlementv0). 모델 호출 없이 합성 장부와 가짜 transcript로 시험했다.

| # | 항목 | 적용 위치 | 완료 조건 |
| --- | --- | --- | --- |
| 32 | **가드 멈춤 회차의 정산.** 결정이 정한 **하나의 회차**(예산 `02-05`, 시도 `s8-r1`, 세션 `5c991c71-…`, 보관 stream SHA-256 `252609c7…`; 코드의 고정값, 정산 결정 §2.3)만 정산할 수 있다. 그 회차는 실행 가드가 한도 신호로 멈춘 회차 중 그 stream의 한도 사건이 현재 판정(항목 31)으로 모두 통과하고 하나 이상이 경고인 것(당시 규칙의 경고 멈춤)만, 그 세션 transcript의 CLI 종료 집계로 정산할 수 있다. 정산은 장부의 **별도 줄**(`ieo-role-run-settlement/v0`, 출처 `transcript_exit_accounting`)이고 원본 기록은 바뀌지 않는다. 장부를 읽을 때마다 다시 확인한다: 장부 안 transcript 사본의 SHA-256, 같은 세션의 모든 줄, side-chain 없음, `message.id`별 마지막 사용량, 메시지 수 = `api-request` 수(1 이상), 마지막 `cost-state.modelUsage` = 메시지 합(네 항목), 진술된 사용량과 같음, 가드 관측 하한 이상, 결정 참조, 독립 검산(`{evidence, sha256, verdict}`, 판정은 `CONFIRMED` 또는 `CONFIRMED WITH CAVEAT`만), 클라이언트 쪽 자료가 덮지 못하는 단서(`verification.uncovered: unlogged_in_flight_request_at_sigterm`과 그 설명). 사본은 정확히 `<장부>/children/<시도>/settlement-transcript.jsonl`(심볼릭 링크 없음), 사본 해시 = 원본 해시, 독립 검산 보고서는 절대 경로에서 다시 읽어 기록된 해시와 같아야 함, 결정은 `docs/poc/User_Decisions_2026-11-17_r1-settlement.md`만, 정산 사용량은 그 회차의 한도 안, 넘침 없음. 한 회차에 하나, 정정과 함께 쓸 수 없음. 적용되면 그 회차는 한 번 그 사용량으로 세고 실패(`stop_required`, “결과 사건 없음”)로 남는다. 가드가 멈춘 회차의 “결과 사건 없음”은 이 정산이 있고 그 회차가 고정한 회차일 때만 사람의 예비 표식(항목 30)이 해제한다. 실제 거부·추가 과금 신호·시간·토큰 상한·다른 미계측 회차에는 쓰지 않는다. 쓰기는 `ieo-role-call settle`(장부 잠금, 진행 중 호출 없음, 원본 해시, 다시 읽어 적용될 때만; `--dry-run`은 쓰지 않음) | supervisor `settlement`(`eligible`·`transcript_accounting`·`apply`·`build`), `caller::records`(적용), `caller::settle`, `caller::spare_releasable`, `ieo-role-call settle` | `tests/settlement.rs`(계산, 한 번 집계·실패 유지·예비 해제, 변조·중복·다른 시도 거절, 근거 부족, 특례 범위, CLI), 변이 35/36(1 동등) |
