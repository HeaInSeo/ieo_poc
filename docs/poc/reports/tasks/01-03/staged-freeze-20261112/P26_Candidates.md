# P26 표본 후보 목록 — 후보 스냅샷 (동결 표본 아님)

> **이 문서는 후보 스냅샷이다.** 관측 시점 이후 이슈·PR·브랜치가 바뀌면 그대로 낡는다.
> 실제 실행 전에 사용자가 목록·난도·처리시간·비용·보류 한도·공통 관측 기간을 동결해야 하며,
> 이 문서는 그 값 중 시간·비용·보류 한도·관측 기간을 **제안하지 않는다.**

## 0. 관측 정보

| 항목 | 값 |
|---|---|
| 관측 시각 (UTC) | 2026-11-12T05:03:30Z (수집 시작) ~ 2026-11-12T05:08Z (보조 조회 종료) |
| NodeKit HEAD (main) | `0e99c37c01981019e801eec826b29da22b3447c1` (2026-10-28 커밋, PR #150) |
| NodeKit 기타 브랜치 | `ux/console-avalonia-v16-spike-20260818` @ `3e7a49e91cd644e18713d1112ad2fa4874ecd142` (열린 PR #102) |
| sori HEAD (master) | `2fa7f5e12fa4474b194acf0d6e1a0273666642c1` (2026-10-28 커밋, PR #47) |
| sori 기타 브랜치 | `w021b-i-startup-classifier` @ `b26246e4ab3870bf6f7516b08906791570b57163` (열린 PR #48) |
| 원본 근거 | `evidence/p26-raw/` 아래 `gh` JSON (이슈·PR·브랜치·커밋·이슈별 timeline·sori PR #48 상세) |
| 문서 근거 | 위 HEAD의 shallow clone (`p26/nodekit`, `p26/sori`)에서 읽은 backlog·sprint 문서 |
| 작업 방식 | 읽기 전용. GitHub에 생성·수정·댓글·라벨·담당 지정 없음. ieo_poc 저장소 변경 없음. |

D8 원문: "P26은 NodeKit 6건·Sori 6건, 무개입 완료 목표 8/12, 잘못된 완료·권한 밖 효과 0건으로 한다. 현재 백로그에서 기존 담당 작업을 제외한 후보와 선정 근거는 에이전트가 준비해줘. 실제 실행 전 목록·난도·처리시간·비용·보류 한도와 공통 관측 기간을 동결한다."

## 1. 요약과 부족분 (먼저 읽을 것)

- **NodeKit은 정당한 1차 후보 6건을 채우지 못한다.** 열린 이슈 8건 중 진행 중이거나 다른 저장소·실서버·정책 결정에 묶이지 않은 것은 사실상 #86(일부)과 #94(정책 결정 조건부)뿐이다. backlog 문서는 대부분 완료 처리됐거나 GUI·실 NodeVault 범위다. 1차 후보 **3건**(그중 2건은 같은 이슈 #86을 나눈 것), 대체 3건(모두 무개입 표본으로는 약함)만 제시한다.
- **NodeKit에는 저장소 안에서 끝나는 미담당 `기능`·`실패 복구` 항목이 없다.** 계획서의 "저장소마다 네 종류 각 1건 이상"을 NodeKit은 현재 백로그로 충족할 수 없다.
- **sori는 1차 후보 6건을 채우지만 그중 1건(S6)은 수락 기준이 주관적이라 약하다.** 네 종류는 모두 1건 이상이다. 다만 #11에서 2건을 나눴고, `기능` 1건은 오래된 backlog 문서(p3-backlog.md)에서 왔다.
- **두 저장소 모두 실행 전 사용자 결정이 필요한 항목이 있다** (NodeKit #94 정책, #86 계약 형태, sori #8 처분, sori 저장소 슬롯). 결정 없이 에이전트가 선택하면 권한 밖 효과가 될 수 있다.
- **보이지 않는 담당**: 이슈 본문들은 Notion Catalog·중앙 배정(own CLAIM, W-unit owner)을 가리킨다. 이 조사는 GitHub와 저장소 문서만 봤으므로 Notion 쪽 CLAIM은 확인하지 못했다. 동결 전에 Catalog 대조가 필요하다.

### 종류별 집계 (1차 후보 기준)

| 저장소 | 기능 | 결함 | 계약·통합 | 실패 복구 | 합계 | 계획 요구(각 종류 ≥1) |
|---|---|---|---|---|---|---|
| NodeKit | 0 | 1 (조건부) | 2 | 0 | 3 | **미충족** (기능·실패 복구 없음) |
| sori | 2 (S6는 약함) | 2 | 1 (조건부) | 1 | 6 | 충족 |

대체 후보: NodeKit 3건(실패 복구 1, 계약·통합 1, 정리 1), sori 2건(결함 1, 실패 복구 1).

## 2. 제외 규칙과 해석

규칙("기존 담당 작업"): 담당자(assignee)가 있음, 해당 항목을 다루는 열린 PR·브랜치가 있음, 또는 최근 활동으로 누군가 진행 중임이 드러남 → 제외. 아래도 함께 적용했다.

- **assignee**: 두 저장소의 열린 이슈 13건 모두 assignee 없음 (`evidence/p26-raw/*_issues_open.json`). 이 조건으로 제외된 항목은 없다.
- **본문 지정 담당**: 이슈 본문에 "후속 담당: …담당"처럼 역할이 적혀 있고 착수 조건이 아직 충족되지 않은 경우는 *진행 중*이 아니므로 자동 제외하지 않았다. 대신 후보마다 "담당 역할 지정됨"을 위험으로 적고, Catalog 확인을 동결 전 조건으로 남겼다.
- **다른 저장소·실서버·자격증명 필요**: 규칙상 제외 대상은 아니지만, 무개입 실행에서 권한 밖 효과 위험이 커서 1차 후보에서 뺐다. 표시는 "부적합(외부 의존)"으로 따로 했다.
- **이미 해결된 것으로 보이는 열린 이슈·문서 항목**: 표본으로 쓰면 "잘못된 완료" 판정이 어려워 제외했다.
- **사소한 보고 필드 추가·로그 한 줄**: 계획서 지시에 따라 제외했다.

## 3. NodeKit

### 3.1 제외 항목과 사유

| 출처 | 제목 (요약) | 제외 사유 |
|---|---|---|
| [#96](https://github.com/HeaInSeo/NodeKit/issues/96) | CLI 국제화 ko/en | **진행 중.** PR #144~#150이 2026-10-21~10-28에 연달아 머지됐다(UF01a/UF01b). 본문에 headless Operator→paired Guardrail 개발·검토 체계가 명시돼 있다. |
| [#97](https://github.com/HeaInSeo/NodeKit/issues/97) | GUI ReactiveUI 재설계 | GUI 설계/구현 담당 지정, 착수 조건 미충족. 관련 GUI spike 브랜치와 열린 PR #102가 있다. 규모도 저장소 전체 아키텍처 변경이다. |
| [PR #102](https://github.com/HeaInSeo/NodeKit/pull/102) | Avalonia v16 spike | 열린 PR·브랜치(`ux/console-avalonia-v16-spike-20260818`). 이 영역(GUI) 전체를 제외했다. |
| [#95](https://github.com/HeaInSeo/NodeKit/issues/95) | Catalog 잔여 계약 추적 | owner: 중앙 설계 담당. NodeVault·NodePalette 저장소 변경이 필요하고, 본문이 "구현을 새로 허가하지 않는다"고 명시한다. |
| [#79](https://github.com/HeaInSeo/NodeKit/issues/79) | AdminToolList revision/Make Active | GUI·NodeVault 계약 담당 지정. NodeVault API가 아직 없다(본문 "블로커"). |
| [#74](https://github.com/HeaInSeo/NodeKit/issues/74) | `--idle-timeout`+heartbeat+reconnect | NodeVault heartbeat 프로토콜 추가가 먼저 필요하다. 연동 계약 담당 지정. → 대체 후보 NA1로만 기록(부적합 표시). |
| [#59](https://github.com/HeaInSeo/NodeKit/issues/59) | samtools 실제 제출 테스트 | 실제 NodeVault 환경·자격증명·승인된 관측 기준이 필요하다. 연동 검수 담당 지정. → 대체 후보 NA2로만 기록(부적합 표시). |
| [#86](https://github.com/HeaInSeo/NodeKit/issues/86) 3항 | request ID로 서버 상태 조회 | NodeVault API·보존·멱등성 계약이 필요하다(다른 저장소). 1·2항만 후보로 썼다. |
| `docs/ARCHITECTURE.md`@0e99c37c §알려진 미완료 | DataRegisterRequest UI 연결/gRPC 전송 | GUI 범위이고 NodeVault P3 TODO-12에 묶여 있다. |
| `docs/NODEKIT_CLI_UX_IMPROVEMENT_SPRINT_PLAN.md`@0e99c37c U5-2 | 폐쇄망 live 재검증 | live seoy Harbor·infra-lab#35에 막혀 있다(외부 인프라). |
| `docs/NODEKIT_CLI_FIRST_SPRINT_PLAN.md`@0e99c37c Sprint 7 Task 2 | 전체 wizard 수동 테스트 | 사람이 직접 하는 수동 시험이다. |
| `docs/NODEKIT_CLI_RELEASE.md`@0e99c37c §8·§9 | release checklist, 실 NodeVault 연결 | release 기록 작업이거나 실서버 범위다. |
| `docs/NODEKIT_SOURCEBUILD_STRUCTURED_INTENT_DESIGN.md` Phase B~D | SourceBuild 구조화 | 문서 상태는 "구현 미착수"지만 코드에 이미 구현돼 있다(`SourceBuildProfileCatalog.cs` 등, AGENTS.md "R22 … done"). Phase E는 NodeVault, Phase F는 결정 대기다. |
| `docs/NODEKIT_IMAGEURI_SEMANTICS_REPORT.md` §6 | L1-IMG-006, 모든 FROM pin 검사 | 이미 구현됨(`ImageUriValidator.cs:103`, `DockerfileStructureValidator.ValidateAllFromInstructionsPinning`). |
| `docs/NODEVAULT_LIVE_RECIPE_REPRO_IMPROVEMENT_NODEKIT.md` pin-mode | `--pin-mode` UX | Sprint R19(`--strict-reproducible`, L1-RCP-016)로 다른 형태로 처리됐다. 남은 차이를 표본으로 삼으려면 설계 결정부터 새로 해야 한다. |

### 3.2 1차 후보

#### N1. #86 1항 — 사전 watch 실패의 구조화된 실패 정보

- **source**: https://github.com/HeaInSeo/NodeKit/issues/86 (본문 "이 이슈에 남기는 범위" 1항), 코드 기준 `0e99c37c`
- **title**: `BuildEvent`/submit 결과에 `FailurePhase`·`GrpcStatusCode`·`FailureKind` 같은 구조화된 실패 정보 보존·노출
- **kind**: 계약·통합 (CLI 내부 계약 + `nodekit.submit.v1` JSONL의 추가 전용 필드)
- **difficulty 제안**: medium — 손댈 곳은 `GrpcToolSpecClient.cs`의 catch 지점(현재 L183·L235·L269 근처)과 `BuildEvent.cs`, `SubmitJsonlRecord.cs`로 좁다. 다만 "어느 계층이 보존·노출할지"를 정해야 하고, 문자열 추정 분류 금지, 기존 JSONL 스키마 하위 호환, 언어 불변성(#96 테스트, `186a80a6`)을 모두 지켜야 한다.
- **rough scope**: `src/Grpc/BuildEvent.cs`, `src/Grpc/GrpcToolSpecClient.cs`, `src/NodeKit.Cli/SubmitJsonlRecord.cs`, submit 결과 매핑, `docs/NODEKIT_CLI_USAGE.md` JSONL 표, `tests/NodeKit.Cli.Tests/`(loopback HTTP/2 fake, `ToolSpecWireP05Tests` 계열)
- **acceptance evidence**: Verify workflow `Build · Test`(dotnet format/build/test, coverage baseline, CLI test report gate). 기존 loopback fake gRPC 서버로 Unavailable·거부·내부오류를 재현할 수 있다(`cf62fe07`). 실서버는 필요 없다.
- **risks**: 본문 지정 후속 담당("NodeKit/NodeVault 연동 계약 담당")이 있다. 본문이 "최신 서버 API와 기대 결과를 고정한 뒤"를 조건으로 건다. 필드 이름·enum 값을 에이전트가 정하면 계약 결정을 대신하게 되므로 **실행 전 사용자가 기대 필드·값을 동결해야 한다.** 외부 자격증명은 필요 없다.
- **무개입 표본 적합성**: 조건부 적합. 범위가 CLI 안에서 닫히고 fake 서버로 검증할 수 있다. 계약을 미리 동결하지 않으면 "잘못된 완료"(임의 계약) 위험이 있다.

#### N2. #86 2항 — 첫 ToolSpec 제출 request ID를 기계 판독 결과에 노출

- **source**: https://github.com/HeaInSeo/NodeKit/issues/86 (본문 2항), 코드 기준 `0e99c37c`
- **title**: `nodekit submit --format jsonl` 결과에 첫 `SubmitToolBuild` request ID 노출 (`registration_request_id`와 구분)
- **kind**: 계약·통합 (결과 불명 `PRE_WATCH_FAILED`의 수동 복구를 돕는 실패 복구 성격도 있다)
- **difficulty 제안**: small~medium — `BuildEvent.RequestId`는 이미 있지만(JobCreated 이벤트만) `SubmitJsonlRecord`에는 `registration_request_id`만 있다. build ID를 받기 전 실패 경로에서도 request ID를 확정적으로 전달하려면 생성 위치(호출자 측 생성, P04 receipt와의 일치)를 정해야 한다.
- **rough scope**: `src/NodeKit.Cli/SubmitJsonlRecord.cs`, submit 명령의 결과 조립, `src/Grpc/GrpcToolSpecClient.cs`(필요 시), receipt와의 일치 테스트, `docs/NODEKIT_CLI_USAGE.md`
- **acceptance evidence**: Verify CI. P04/P05 receipt 테스트(`ReceiptReentryP05Tests`, `SubmitCancelTimeoutP05Tests`)와 같은 fake로 "build ID 없음 + request ID 있음"을 검증할 수 있다.
- **risks**: N1과 같은 이슈·같은 지정 담당이다. N1과 같은 파일을 건드리므로 **두 건을 동시에 실행하면 충돌하고, 독립 표본으로 세기 어렵다.** 필드 이름(`request_id` 등)은 사용자가 동결해야 한다. 서버 상태 조회(3항)는 범위 밖임을 명시해야 한다.
- **무개입 표본 적합성**: 적합한 편. 범위가 작고 검증이 쉽다. N1과 순서를 정하거나 둘 중 하나만 고르는 편이 낫다.

#### N3. #94 — DockerfileFallback의 버전 미고정 apt/apk/yum 설치가 L1을 통과

- **source**: https://github.com/HeaInSeo/NodeKit/issues/94, 코드 기준 `0e99c37c` (`src/Validation/PackageVersionValidator.cs`)
- **title**: `PackageVersionValidator.ValidateDockerfile`가 `apt-get install`/`apk add`/`yum install` 미고정을 검사하지 않음
- **kind**: 결함 (검증 공백)
- **difficulty 제안**: small(advisory 선택 시)~medium(hard block 선택 시) — 파서·validator 한 곳과 pass/block 테스트만 바꾸면 된다. 다만 CLAUDE.md §9에 따라 L1 규칙을 바꿀 때는 통과·차단 사례를 직접 테스트해야 하고, 명령 구문 변형(`apt-get -y install`, 줄 이음, `&&` 연결 등)을 다뤄야 한다.
- **rough scope**: `src/Validation/PackageVersionValidator.cs`(또는 advisor 신규 파일), `tests/NodeKit.Tests` validator 테스트, 규칙 ID·안내 문구(ko/en 리소스, #96 구조와 맞춤), `docs/NODEKIT_CLI_USAGE.md` 규칙 표
- **acceptance evidence**: Verify CI의 단위 테스트. 외부 의존이 없다.
- **risks**: **본문 처분이 "정책 결정 대기"**다. block/advisory/허용 중 무엇을 고를지, 지원 명령·구문, 기대 입력·출력을 설계/정책 결정권자가 정해야 한다. 결정 없이 실행하면 에이전트의 정책 선택 자체가 권한 밖 효과다. 2026-10-09 처분에서 P03(#114/#115, 닫힘)과 연관 있다고 적혀 있다.
- **무개입 표본 적합성**: **사용자가 정책(선택지 1~3)과 양·음 사례를 먼저 동결하면** 좋은 표본이다(작고 결정적이며 테스트로 판정 가능). 동결 전에는 부적합하다.

### 3.3 대체 후보 (모두 무개입 표본으로는 약함)

| ID | source | title | kind | difficulty 제안 | 판단 |
|---|---|---|---|---|---|
| NA1 | [#74](https://github.com/HeaInSeo/NodeKit/issues/74) | `--idle-timeout` + heartbeat + reconnect | 실패 복구 | large | **부적합(외부 의존).** NodeVault `WatchToolBuild` heartbeat가 먼저 필요하다. NodeKit만 구현하면 정상 빌드를 timeout으로 잘못 분류한다고 본문이 경고한다. NodeKit 쪽 유일한 실패 복구 항목이라 기록만 한다. |
| NA2 | [#59](https://github.com/HeaInSeo/NodeKit/issues/59) | samtools 실제 제출 테스트 | 계약·통합 | medium | **부적합(실서버·자격증명).** 실제 NodeVault endpoint, 승인된 환경·sample, 서버 로그 관측이 필요하다. 실행하면 저장소 밖(실 클러스터 빌드·Harbor push)에 효과가 생긴다. |
| NA3 | `docs/ARCHITECTURE.md`@0e99c37c §레거시 클래스 | 미사용 `GrpcToolRegistryClient` 정리 | 네 종류 중 해당 없음(정리) | small | 약함. 문서가 "삭제 검토 가능"으로만 적어 결정되지 않았다. 테스트 2개(`GrpcToolRegistryClientMappingTests`, `ToolRegistryClientTests`)가 참조하고, GUI 영역(#97 담당·PR #102)과 겹칠 수 있다. |

## 4. sori

### 4.1 제외 항목과 사유

| 출처 | 제목 (요약) | 제외 사유 |
|---|---|---|
| [PR #48](https://github.com/HeaInSeo/sori/pull/48) / 브랜치 `w021b-i-startup-classifier` | W02.1b-I start classifier | **진행 중**(열린 PR). `internal/admininit/*`, `jobledger/initstate.go`, `jobledger/startfacts.go`를 바꾼다. W02.1b 트랙(PR #41~#47, 2026-10-19~10-28) 전체를 기존 담당 작업으로 보고 제외했다. |
| [#45](https://github.com/HeaInSeo/sori/issues/45) | GO-2026-6809, go1.26.10 필요 | **이미 해결된 것으로 보인다.** master `go.mod`가 `go 1.26.10`이고, 이 변경은 PR #44(`aadcd13c`, 2026-10-25)에서 들어왔다(`evidence/p26-raw/sori_commits_gomod.json`, timeline에 10-25 referenced). 이슈만 열려 있다. 남은 일은 `make vuln` 재확인과 이슈 종료뿐이라 표본이 될 수 없다. |
| [#11](https://github.com/HeaInSeo/sori/issues/11) 잔여 3 | in-fetch digest mismatch 분기 증명 | `chunked/failure_test.go`의 `TestFetch_ChunkDigestMismatch`(PR #13 "isolate per-chunk digest-mismatch branch", 2026-08-02)가 이미 이를 다루는 것으로 보인다. 남은 일은 증거 판정(보고)에 가까워 코드 표본으로 쓰기 어렵다. 사람의 확인이 필요하다. |
| `p8-sprint.md`@2fa7f5e1 | SkipIfCurrent | 문서는 ⏳지만 이미 구현됐다(`options.go:84`, `client.go:189`, `skip_if_current_test.go`). |
| `docs/stub-status.md`@2fa7f5e1 | stub 제거 | 문서 결론: Sprint 20에서 제거 완료. |
| `docs/v1-readiness.md`@2fa7f5e1 | GHCR/Harbor smoke·real dataset·release binary 결과 기록 | 외부 registry 자격증명과 실 업로드가 필요하다(저장소 밖 효과). |
| `p5-sprint.md`@2fa7f5e1 P5-6/P5-7 | 1~50GiB 벤치 gate, ECR OQ-1 | 수동 대규모 실행, AWS 자격증명이 필요하다. |
| `p5-sprint.md`@2fa7f5e1 P5-3 | `Log.Infof` at chunk boundaries | 사소한 로그 추가(계획서의 "사소한 필드 추가" 제외 취지). |
| `docs/followup-sprint-plan.md`·`docs/v1-readiness.md` §API freeze 등 | stable 승격·registry 지원 범위·오류/옵션 freeze 결정 | 결정·문서 작업이다. 현재 sori 정본(authority·W-unit 체계)보다 앞선 시기의 계획이라 현행성도 불분명하다. |

### 4.2 1차 후보

#### S1. #38 — authority/sqlitestore의 connect 시점 busy wait 상한 시험 누락

- **source**: https://github.com/HeaInSeo/sori/issues/38, 코드 기준 `2fa7f5e1`
- **title**: `authority/sqlitestore/open_test.go`에 connect 시점 busy-wait 상한 시험 추가 (`acquisition/sqlitecheckpoint`의 `TestOpenCapsBusyWaitWhileConnecting`과 같은 방식)
- **kind**: 결함 (회귀 방지 시험. 본문은 "알려진 동작 버그 없음, 커버리지 공백"으로 적는다)
- **difficulty 제안**: small — 참조할 시험이 이미 있다(PR #37). 대상 파일 하나와 시험 하나다. 본문이 mutant(DSN `_busy_timeout` 고정 10s)로 실패를 확인하는 방법까지 제시한다.
- **rough scope**: `authority/sqlitestore/open_test.go`(필요 시 `export_test.go`)
- **acceptance evidence**: Tests workflow `Run tests`(make test), lint, vuln guardrail. "mutant에서 실패, 수정본에서 통과"를 증거로 요구할 수 있다.
- **risks**: 타이밍 의존 시험이라 CI에서 flaky할 수 있다(300ms 예산). 외부 의존은 없다. 저장소 슬롯 문제는 §4.4를 본다.
- **무개입 표본 적합성**: 적합. 범위·판정 기준이 명확하다. 다만 쉬운 쪽에 속한다.

#### S2. #11 잔여 2 — 무조건 `t.Skip` 시험의 거짓 커버리지 표시

- **source**: https://github.com/HeaInSeo/sori/issues/11 (Residual 2), 코드 기준 `2fa7f5e1`
- **title**: `chunked/fetch_test.go`의 `TestFetch_IntegrityFailure`·`TestFetch_SchemaVersionMismatch`(L67·L71), `chunked/e2e_test.go`의 `TestE2E_TC05_IntegrityFailure`(L83) 무조건 skip을 실행 가능한 시험으로 바꾸거나 정직한 근거로 정리
- **kind**: 결함 (시험 거버넌스)
- **difficulty 제안**: small — 같은 분기를 이미 다루는 실행 시험이 있다(`TestFetch_ChunkDigestMismatch`, `TestE2E_TC11_SchemaVersionMismatch`). 주된 일은 실제로 같은 분기를 타는지 확인한 뒤 skip을 대체·삭제·재명명하는 것이다.
- **rough scope**: `chunked/fetch_test.go`, `chunked/e2e_test.go`(필요 시 `chunked/failure_test.go`)
- **acceptance evidence**: Tests workflow. 결과에서 해당 skip이 사라지거나 정확한 사유로 바뀐 것을 확인할 수 있다(`go test -v` 출력).
- **risks**: 본문 "Implementation gate": 수정 PR 전에 저장소 슬롯과 현행 Sori authority를 확인해야 한다. 같은 이슈(#11)에서 S3와 함께 나왔으므로 둘 다 고르면 이슈 종료 판정이 겹친다. 시험 메타데이터만 바꾸고 "완료"를 주장하는 잘못된 완료 위험이 있어, 분기별 증거를 요구해야 한다.
- **무개입 표본 적합성**: 적합. 작고 판정 가능하다. 사소한 변경으로 끝나지 않도록 "같은 분기를 실제로 실행"하는지를 수락 기준에 넣어야 한다.

#### S3. #11 잔여 1 — 중단된 부분 push와 재시도 의미 증명

- **source**: https://github.com/HeaInSeo/sori/issues/11 (Residual 1, 수락 기준 1·2·5), 코드 기준 `2fa7f5e1`
- **title**: 원격에 부분 효과가 남은 뒤 실패한 push에 대해 transport 수준의 결과와 재시도 동작을 시험으로 고정하거나, 현행 처분을 명시
- **kind**: 실패 복구
- **difficulty 제안**: medium — `push_partial_failure_test.go`(`blobFailAfterNRegistry`, PR #13)가 이미 "manifest를 commit하지 않는다"를 증명한다. 남은 것은 *재시도*가 결정적이고, 나중에 성공한 transport가 새 권위 Revision처럼 보이지 않는다는 점이다. fake registry 확장과 재시도 시나리오 설계가 필요하다.
- **rough scope**: `push_partial_failure_test.go`, 필요 시 `volume_publish_fetch.go`의 push 경로, `docs/test-strategy.md`
- **acceptance evidence**: Tests workflow(httptest fake registry, 외부 registry 불필요). CI의 `Registry Push/Pull Smoke`(registry:2 서비스)는 보조 증거다.
- **risks**: **본문이 "Sori publication semantics 재설계 허가가 아니다"라고 못 박고 있고**, SORI-I1M/I2R authority 경계를 지켜야 한다. 에이전트가 authority 의미를 건드리면 권한 밖 효과다. 정본 문서가 Notion에 있어 에이전트가 읽지 못할 수 있다. S2와 같은 이슈다.
- **무개입 표본 적합성**: 조건부 적합. "transport 수준 시험만, authority 코드 변경 금지"로 범위를 동결해야 한다. 난도가 있는 실패 복구 표본으로 쓸 만하다.

#### S4. #8 — NodeVault와 중복된 죽은 Tool/Volume proto 정의

- **source**: https://github.com/HeaInSeo/sori/issues/8, 코드 기준 `2fa7f5e1` (루트 `tool_metadata.proto`, `volume_resource.proto`)
- **title**: 필드 번호가 충돌하는 중복 proto 정의를 제거하거나 정본 소유자를 지정
- **kind**: 계약·통합
- **difficulty 제안**: small(선택지 1 "제거")~large(선택지 2 "정본 이전 + descriptor 호환 CI") — 현재 두 파일은 루트에 있고 `buf.yaml` 모듈(`api/`) 밖이며 생성 코드·소비자가 없다(본문, 관측 시 확인). 제거만 하면 작고, 선택지 2는 NodeVault와의 조율이 필요하다.
- **rough scope**: 선택지 1: 루트 `*.proto` 2개 삭제, 참조 문서 정리, 활성 NodeVault 연동(Go 패키지) 설명 추가
- **acceptance evidence**: Tests·Proto workflow(`proto.yml`, buf lint/breaking은 `api/`만 대상). 수락 기준 6개 중 선택지 1에 해당하는 항목만 검증할 수 있다.
- **risks**: **본문이 "Required decision"으로 선택지 1·2를 사람이 고르도록 요구한다.** 사용자가 선택지 1로 동결하지 않으면 부적합하다. 선택지 2는 NodeVault 저장소 변경이 필요해 권한 밖이다. CODEOWNERS는 `*`에 @HeaInSeo다.
- **무개입 표본 적합성**: 선택지 1로 동결하면 적합(작고, 계약 부채를 실제로 해소한다). 동결하지 않으면 부적합하다.

#### S5. p3-backlog P3-1 Step 2 — 대용량 layer 디스크 사전 점검과 disk-full 오류

- **source**: `p3-backlog.md`@`2fa7f5e12fa4474b194acf0d6e1a0273666642c1` §P3-1 "⏳ Step 2 — spooling and disk-safety improvement", "Open items"
- **title**: 큰 layer를 쓰기 전에 가용 공간을 점검하고, temp 쓰기가 디스크 부족으로 실패할 때 명확한 오류를 내기
- **kind**: 기능 (실패 사전 차단 성격도 있다)
- **difficulty 제안**: medium — `buildAndPushTempLayer` 경로에 statfs 기반 점검(OS별 build tag)을 넣고, 오류 분류(`ErrValidation`/`ErrTransport` 등 기존 `Err*` 계약)를 맞추고, 디스크 부족을 결정적으로 시험할 주입 지점을 만들어야 한다.
- **rough scope**: `volume_publish_fetch.go`(temp layer 경로), `errors.go`, 신규 시험, `p3-backlog.md` 상태 갱신
- **acceptance evidence**: Tests workflow. 디스크 부족은 주입 seam이나 작은 tmpfs 없이는 재현하기 어렵다. CI에서는 seam 기반 단위 시험으로 판정해야 한다.
- **risks**: 문서가 "future, low priority"로 적었고 현재 sori 정본(authority·W-unit) 이전 시기의 backlog라 **현행 우선순위인지 확인해야 한다.** #32(fetch 디스크가 L에 비례)의 설계 예산(D02.1)과 겹치는 영역이라 중앙 결정과 충돌할 수 있다. 외부 의존은 없다.
- **무개입 표본 적합성**: 조건부. sori에서 유일하게 저장소 안에서 끝나는 미담당 `기능` 항목이라 넣었다. 현행 우선순위를 사용자가 확인해야 한다.

#### S6. v1-readiness — push auth hint·오류 문구의 초보자 직접성 (약한 후보)

- **source**: `docs/v1-readiness.md`@`2fa7f5e12fa4474b194acf0d6e1a0273666642c1` L122 "- [ ] auth hint와 error message가 GHCR/Harbor 초보자에게 충분히 직접적인지 확인한다."
- **title**: `sorictl` push 실패 시 auth hint(`cmd/sorictl/main.go` `authHint`)와 오류 문구 개선
- **kind**: 기능 (CLI UX)
- **difficulty 제안**: small — `authHint`는 자격증명이 비었을 때만 안내한다. 401/403, 잘못된 registry 호스트 같은 실패 유형별 안내를 넣는 정도다.
- **rough scope**: `cmd/sorictl/main.go`, `cmd/sorictl` 시험, `docs/sorictl-quickstart.md`
- **acceptance evidence**: Tests workflow(httptest로 401/403 재현). GHCR/Harbor 실계정은 필요 없다.
- **risks**: **수락 기준이 "충분히 직접적인지"라는 주관적 표현**이라 무개입 판정이 어렵다. 동결 시 구체적인 실패 유형과 기대 문구 조건을 정해야 한다. 오래된 v1 계획 문서의 항목이다. 사소한 문구 수정으로 끝날 위험이 있다(계획서의 "사소한 추가" 경계).
- **무개입 표본 적합성**: 약함. 6건을 채우려고 넣었다. 수락 기준을 구체화하지 않으면 대체 후보 SA1·SA2와 바꾸는 편이 낫다.

### 4.3 대체 후보

| ID | source | title | kind | difficulty 제안 | 판단 |
|---|---|---|---|---|---|
| SA1 | [#32](https://github.com/HeaInSeo/sori/issues/32) | fetch RSS가 in-flight chunk × chunk 크기로 커지고, 디스크가 L에 비례 (`chunked/fetch.go:258` `io.ReadAll`, `volume_publish_fetch.go:965-980` 전체 temp OCI 복사) | 결함 | large | 측정 근거가 탄탄하고 범위가 저장소 안이다. 그러나 본문이 "Central assigns the W-unit owner"라고 적어 **담당 배정 대기**이고, D02.1 설계 예산(RSS ≤ 768MiB, 디스크 = staging L + journal)이라는 설계 결정과 묶여 있다. 수 GiB fixture 측정은 CI 범위를 넘는다. 중앙 배정 여부를 확인한 뒤에만 쓴다. |
| SA2 | `p5-sprint.md`@2fa7f5e1 P5-3 "Test: 5xx mock → backoff → ErrTransport (deferred)" | chunk push 재시도·backoff 시험 | 실패 복구 | small~medium(범위 재정의 후) | **문서 전제가 현재 구조와 맞지 않는다.** `chunked.Publish`의 store는 로컬 `*oci.Store`(`chunked/publish.go:49`)라 HTTP 5xx가 생기지 않는다. 재시도 판정도 오류 문자열에 `"50"`이 들어 있는지로 한다(`chunked/publish.go:416`). 미등록 관찰이며 이 목록에서 별도 결함으로 세지 않았다. 표본으로 쓰려면 사용자가 "재시도 분기의 실제 도달 조건 시험 또는 처분"으로 범위를 다시 정해야 한다. |

### 4.4 sori 실행 시 공통 주의

- **저장소 슬롯**: #38 본문("Keep it out of #37 so the repo stays at one modifying PR")과 #11 본문("Fresh repo slot … must be checked before any modifying PR")을 보면 sori는 *수정 PR을 한 번에 하나만* 운영한다. 관측 시점에 PR #48이 열려 있다. P26 실행이 이 운영 규칙과 부딪히면 권한 밖 효과가 될 수 있으므로, 슬롯 배정 방식을 사용자가 정해야 한다.
- **CODEOWNERS**: `*`, `/go.mod`, `/go.sum`, `/.github/`이 모두 @HeaInSeo다. 후보 중 go.mod나 `.github`를 건드릴 필요가 있는 것은 없지만, 실행 범위에서 금지 경로로 명시하는 것이 안전하다.

## 5. 동결 전에 사용자가 정할 것 (값은 제안하지 않음)

1. **NodeKit 부족분 처리**: 6건을 채울 수 없다. 표본 크기를 NodeKit 3건 내외로 줄일지, 다른 저장소로 옮길지, 외부 의존 항목(NA1·NA2)을 별도 조건으로 넣을지 정해야 한다. 이렇게 되면 D8의 "NodeKit 6건·8/12 목표"와 계획서의 "각 종류 ≥1"이 그대로는 성립하지 않는다.
2. **정책·계약 동결**: #94 처분(hard block/advisory/허용)과 양·음 사례, #86 1·2항의 필드·enum 이름, sori #8 선택지.
3. **이슈 분할 인정 여부**: #86→N1·N2, #11→S2·S3를 각각 독립 표본으로 셀지.
4. **sori 저장소 슬롯**: PR #48 진행 중에 실행할지, 슬롯을 어떻게 배정할지.
5. **Catalog(Notion) 대조**: 본문 지정 담당 역할(#86 연동 계약 담당, #32 중앙 W-unit 배정 등)에 실제 CLAIM이 있는지.
6. 난도·처리시간·비용·보류 한도·공통 관측 기간 값.

## 6. 근거 파일 (`evidence/p26-raw/`)

- `observed_at_utc.txt` — 수집 시작 시각
- `NodeKit_issues_open.json`, `sori_issues_open.json` — 열린 이슈(assignees, labels, closedByPullRequestsReferences, body)
- `*_issues_closed_recent.json`, `*_prs_open.json`, `*_prs_merged_recent.json`, `*_branches.json`, `*_commits.json`, `*_repo.json`
- `NodeKit_issue{59,74,79,86,94,95,96,97}_timeline.json`, `sori_issue{8,11,32,38,45}_timeline.json` — 교차 참조·담당 지정 이벤트
- `sori_commits_gomod.json` — #45 해결 근거(go.mod 변경 이력)
- `sori_pr48.json` — PR #48의 변경 파일·본문
