# Claude and Codex live fixture trial, approval required

This is a proposed follow-up to the isolated PoC. Use a new sandbox repository, assignment, budget and quota-group bindings. Keep the 02-05 step 8 environment, approvals and ledger out of the trial.

## Prerequisites

1. Obtain supported read-only subscription observations for each actual account, including per-window limits, timestamp, account/group and confirmation that usage credits or any metered fallback are disabled. Collect sanitized real Codex samples and validate the mapping. Claude's current classifier is based on runtime events, so after restart it does not establish fresh available status on its own. If either provider lacks a safe read-only observation path, keep its status unknown and block the corresponding trial. Do not spend quota or send probes just to manufacture an exhausted or recovered state.
2. Bind each runtime to an authenticated role target and demonstrate equivalent enforcement for filesystem scope, network access, tool allow/deny, identity and independent review. Exercise those restrictions using harmless repository fixtures. Validate actual CLI session creation and resume inputs. The portable checkpoint includes task, branch/head, verified stage and evidence references. Reconcile working tree or staged changes as well as pushed effects; an unknown side effect is a hard stop.
3. Integrate with the coordinator's current assignment generation, semantic and evidence review, shared budget lock, role ceilings, durable attempt/delivery ledger and trusted measured usage. Confirm non-overage credentials, reserve the remaining budget before dispatch and enforce time/token limits with the runtime guard. Correlate provider usage, UTC timestamps and effects to the exact delivery; an HTTP success or a local process exit does not establish acceptance or completion.
4. Prepare a separate run request with the exact settings and digests, test repo and role, snapshot/rollback, number of authorized model calls, common time/token limits, evidence retention and additional charges capped at USD 0. Get explicit approval before creating any live dispatch. No 02-05 permissions or configuration are copied into this trial.

## Proposed first trial after separate approval

The first trial is **Claude → Codex only**, on one disposable repository and its development role. Use a known checkpoint from a verified, already completed read-only preparation stage. Allow at most **one** actual model dispatch, on Codex for the remaining stage. Claude performs no trial model request. The proposed ceilings are 120 seconds including stop grace, 4,000 billable tokens, 20,000 cache-read tokens, and **USD 0** in additional charges. Use only the separately approved account/role bindings and snapshot. Reverse direction and a post-recovery invocation require a different trial approval.

1. Bind the exact approved subscription accounts and model/bucket applicability to the two role targets. Apply and verify equivalent policy digests and effective permissions in the trial sandbox.
2. After the quota event occurs naturally, an operator opens Claude Code `/usage` in the bound account. Record sanitized applicable window, exhaustion evidence, reset timestamp/time zone, usage credits off, actual role binding, observation time and verifier. This is manual evidence, distinct from automatic quota detection. If the display is stale, ambiguous or for another account, stop. No prompt to force a quota event.
3. For the bound Codex subscription, call the documented App Server `account/read` and `account/rateLimits/read` without starting a thread or turn. Match identity and quota group, check the bucket/window coverage for the specific model, its fresh available state and reset. Independently verify from the bound workspace’s Usage & Billing or enforceable administrator policy that paid fallback, credit spend and auto-reload cannot charge for the trial. A zero credit flag alone is insufficient. Preserve only sanitized evidence. An unconfirmed window or billing control is `UNKNOWN` and blocks dispatch.
4. Revalidate the same review, development-role authorization, assignment generation, branch/head, checkpoint and actual stage evidence. Under shared admission, reserve the bounded allowance and persist the delivery/attempt ID before sending the one Codex new-session POST. Require verified usage and repository effect under that same ID. If either quota condition never occurs, record zero dispatches and report the trial as unexercised.
5. Stop on unknown acceptance or effects, quota or role drift, unconfirmed usage, missing evidence or an unexpected cost. Preserve the ledger. No re-POST, paid reset, test call to probe a window, or second model call is authorized by this proposal.

## Read-only findings and proposed fixture binding

See [the quota-read record](QUOTA_READ.md). Codex has a supported machine read for quota windows. Claude has a verified operator screen. Keep live dispatch disabled until the trial accounts are explicitly bound to their provider observations and both USD 0 settings are verified. The existing developer-host Claude.ai Max and ChatGPT Codex logins are candidates for read-only verification only. They have no approved runner credential binding. An operator-assisted trial could use fresh Claude screen evidence with account, window, reset, credits-off and timestamp attestation. An unattended trial needs a supported machine-readable Claude source. An account with non-disableable paid fallback is ineligible.

The separate approval packet must pin the disposable repository, branch and SHA, approved account references, quota groups, effective policy digests and current review generation. Use subscription authentication without API-key fallback. The developer-host snapshots in the quota-read record are historical and must not be reused for this trial.

## Coordinator integration to prepare in isolation

- **Role parity fixture.** Use one canonical read-only role policy. Pin equivalent repository roots, branch/head, tool allowlist, network denial, no push, no unapproved external operation, and the independent review generation. Run equivalent harmless access and denial fixtures for each candidate policy. An adapter must attest the effective restrictions and runtime/account identity. A matching string by itself is insufficient. The current tests cover role mismatch, revision revocation during observation, and checkpoint drift.
- **Shared lock and accounting.** Keep runtime selection and provider observation read-only. Re-enter coordinator admission through the existing `delivery` flow for the same repo, assignment generation and policy. Under the budget's existing `caller.lock`, read effective records, settle assignment usage, recompute role/common room, lower the run window/token limits, and persist the intent or in-flight marker for the coordinator delivery key before handing it to the selected adapter. The same admission path must cover every caller using that budget. Do not run the fixture journal alongside the live ledger as a second authority. The isolated PoC uses the existing `budget::spent` accounting and demonstrates combined ceilings; it does not connect a live adapter to `caller::Admission`.
- **Submission and reconciliation.** Fence authorization and effective runner identity against concurrent changes at submission, with the approval revision and checkpoint in the request. Use the coordinator delivery ID end to end; use separate provider session IDs. After a timeout, consult the delivery row, adapter attempt status and actual repo state. A confirmed no-send closes the intent without charging a model run; a send with unmeasured usage or unverifiable effects blocks the next one. Preserve observations and timestamps apart from credentials. Before live integration, rerun the same two-runtime fixture through a disposable database and ledger to prove lock ownership, a persisted delivery ID, combined accounting, the authorization fence and no duplicate POST after lost acknowledgment.

[The isolated delivery record](DELIVERY_INTEGRATION.md) exercises common admission, row-lock approval, supervisor HTTP, shared accounting and no-replay reconciliation with both fakes. The draft live plan still requires an approved real router and evidence mapping. Bubblewrap parity for the fakes is evidence about the test boundary only.

[첫 실제 시험 준비 및 병합 원장](TRIAL_READINESS.md)에 병합 후 확인한 근거와 미충족 조건을 기록한다.

## 실제 첫 시험의 전송·요청 한도

상태는 `WAITING_FOR_PREREQUISITE`다. 시험 계정은 사용자가 지정한 개발 호스트의 기존 Claude 구독 로그인과 현재 Codex CLI의 ChatGPT 계정·워크스페이스다. 역할 자격의 사용 범위는 별도 운영 승인에 따른다.

| 한도 | 집행·검증 기준 |
| --- | --- |
| 코디네이터 → 역할 실행기 | 다음 미완료 단계의 durable delivery ID 한 건으로 **Codex 전송 최대 1회**. 응답 유실 시 동일 ID를 조회하고 자동 재전송하지 않는다. |
| 역할 실행기 → 공급자 | Claude 모델 요청 **0회**, Codex 실제 모델 요청 **최대 1회**. 단일 delivery나 `turn/start`는 이 횟수의 증거가 아니다. 내부 도구 후속 요청, 재시도, 재연결을 포함한 공급자 요청 수의 사전 집행과 독립 대조 근거가 필요하다. |
| 시간·토큰·청구 | 정지 유예 포함 120초, 과금 토큰 4,000, 캐시 읽기 20,000, 추가 USD 0. 시간은 감독자가 집행한다. 사용량 관측 뒤 정지로는 공급자 측 토큰 상한을 보장할 수 없어 지원되는 집행 근거를 요구한다. |

승인안에는 두 횟수의 집행 지점, 각각의 증거와 실패 시 처분을 포함한다. 자연 소진과 동일 시점의 Codex 가용성, 유료 대체·자동 충전·크레딧 지출 차단, 역할·권한·장부 결합 및 상한 집행이 모두 확인된 뒤에만 승인 요청을 올린다.
