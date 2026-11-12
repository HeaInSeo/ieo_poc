<!-- 01-05 조항 위치 조사(개발 에이전트가 맡긴 읽기 전용 조사, 2026-11-12). 인용은 고정 커밋 e5075aff 원문. 로컬 사본 9개와 ci.yml은 고정 커밋과 바이트 동일(fixed-source-compare.txt). 이 파일은 분석이며 원본이 아니다 -->
# NPG clause map at e5075affbc87ed650a755558b8491828e6783862

Repo: `HeaInSeo/notion-projection-guard`. Commit: `e5075affbc87ed650a755558b8491828e6783862`.

How the sources were checked:
- I compared all 9 local docs in `ieo_poc-0105/docs/poc/05_npg_evidence/repo_documents/repo/` with the same files at the commit using `cmp`. They are byte-identical, and so is `ci.yml` against `.github/workflows/ci.yml`. Line numbers for these files therefore apply to both copies.
- Files marked **(remote)** were read from the commit through `gh api` and a read-only clone in the scratchpad. Their line numbers refer to the raw file at that commit.
- Quotes are verbatim. `…` marks a cut inside a quote.

## Responsibility map

| # | clause file:lines | quote | notes/gap |
|---|---|---|---|
| 1 Notion 본문 쓰기의 사전 이미지·readback·중단 | README.md:281-282; README.md:285-287; README.md:288-289 | 281: "Save the full raw preimage, body, properties, planned output and hashes with private permissions" / 286-287: "Require the entire expected body and all protected prefix/suffix/properties to match." / 288: "Any discrepancy or unknown result halts the batch." | All of these sit under "Enforced update sequence" (README.md:271-289). README.md:291 adds: "This is a check-and-detect barrier, not remote atomic CAS." |
| 2 Notion 사건별 reconcile 헬퍼 | README.md:52-54; README.md:477; templates/central-supervisor.md:45 (remote) | 52-53: "`reconcile_bori_prepared_mcp.cjs` is a separately reviewed helper for the exact `BORI-AUTH15-DRIFT-REPAIR-20261022` incident" / 477: "This entry point is never called automatically by `run` or `resume`." / CS:45: "복구 helper를 자동 호출하지 않는다." | Other single-incident helpers: README.md:119 (nodekit_pointer_partial), :246 (transport_preprepare), :441 (claim40_superseded), :537 (w02_prewrite), :550 (preflight_connection), :573 (router_stale_prewrite), :622 (v02_partial); AUTOMATIC_FOLLOWTHROUGH.md:99 (permit_expiry) and :163 (unpermitted_dispatch). Each helper is pinned to one incident and runs check-only by default. |
| 3 결과 미확인 쓰기의 무근거 재전송 금지 | README.md:289; README.md:362-363; AUTOMATIC_FOLLOWTHROUGH.md:159; templates/central-supervisor.md:29-30 (remote) | 289: "do not rollback, blindly retry, or proceed to a new batch." / 362-363: "Mutation calls, unknown acknowledgements and async failures are never retried." / AF:159: "A failed or uncertain mutation is never retried." / CS:30: "이 이후 응답 유실은 발송 불확실이며 재송신 금지다." | README.md:316 adds: "Never use a new batch ID to bypass unresolved state." The same rule for review requests is at OPERATIONS.md:144-145 ("INTENT/UNKNOWN 전송은 … `RECONCILE_DELIVERY_NO_RESEND`이며 새 ID로 재전송하지 않는다"). |
| 4 소유권·lease·이전 실행자 차단 | templates/central-supervisor.md:24-27 (remote); AUTOMATIC_FOLLOWTHROUGH.md:60-62; broker.cjs:455-458 (remote); REVIEW_TIMEOUT_OPERATIONS.md:12-13 (remote) | CS:24-25: "새 fenced runner에는 실제 중앙 실행·한정 소유권·필수조건을 새로 읽는 `assertOwnership({phase,batchId})`가 필수다." / AF:60-62: "The original broker lock must be held by the new executor, so a live prior executor cannot be adopted." / broker:458: "'EXECUTOR_FENCED', 'Original live executor, immutable plan and ticket required; no restart replay'" / RTO:12-13: "All mutations use the original flock, exact revision CAS, at-most-five-minute lease and monotonic fence." | Related: CENTRAL_SCHEDULER_MIGRATION.md:35-36 ("original prior-executor absence checks") and README.md:349-350 (a stale `broker.lock` may be removed only after proving its PID is absent). The lease in the RTO row applies to the review-timeout ledger, which is a candidate and not deployed. |
| 5 요청 전달과 실제 인수의 구분 | AUTOMATIC_FOLLOWTHROUGH.md:295-296; SOURCE_REVIEW_BOOTSTRAP.md:42; templates/central-supervisor.md:71-72 (remote); SCOPED_EXCEPTION.md:59-60 (remote) | AF:295-296: "A service start, configuration save, REQUESTED row or unchanged historical RELEASE is not RECEIVED." / SRB:42: "Saving request data or starting a process is not reviewer intake." / CS:71-72: "요청만으로 인수를 완료 처리하지 않는다." / SE:59-60: "A request, service start or hook test is not actual intake." | README.md:213-215: PROMOTE/RECIRCULATE requires both a real request receipt and an own receipt bound to task, session, time and input. |
| 6 지정한 PR·head/base/tree만 검토 | SOURCE_REVIEW_BOOTSTRAP.md:22-23; source_review_request.cjs:30 (remote); source_review_request.cjs:12-14 (remote) | SRB:22-23: "Bind the new request to the exact head/base/tree, Guardrail role, existing task, row and session." / SRR:30: "`Review only ${REPO} PR${r.pr} head ${r.head}, base ${r.base}, tree ${r.tree}, clean source ${r.sourceDirectory}.`" | Narrow scope: `REPO` is fixed to `HeaInSeo/notion-projection-guard` and the task to `GR-NPG-NORMAL-FRESHNESS-20261028` (source_review_request.cjs:5-6). This covers NPG's own Guardrail source review only, not product PRs in general. Related: OPERATIONS.md:163 ("이전 head/base clean은 현재 검토에 쓸 수 없다"); CENTRAL_SCHEDULER_MIGRATION.md:53 (scope is exact repo, PR 25, head, base and tree). |
| 7 출처 확인과 불변 설치 | README.md:329-331; AUTOMATIC_FOLLOWTHROUGH.md:190-193; CENTRAL_SCHEDULER_MIGRATION.md:47; OPERATIONS.md:39-40 | 329-331: "Hosted source reads use a verified full commit, never a mutable branch or tag." / AF:190-192: "Use one reviewed immutable source commit after normal CI, independent review, current-head Codex and expected-head landing." / AF:192: "Install the new files in a separate versioned directory" / CSM:47: "The directory must be 0555 and all installed files and this manifest 0444." | Also README.md:333-336: "Approved code/policy hashes are checked before starting the broker". OPERATIONS.md:39-40 says not to use a mutable branch name as the approved value. The provenance check runs in CI (ci.yml:39-45). |
| 8 저장소별 독립 진행 | templates/central-supervisor.md:31, 34-36 (remote); README.md:197; README.md:201-202 | CS:31: "저장소마다 complete entry를 따로 실행한다." / CS:34-35: "독립 사건을 body9/native4 결합 관측으로 묶어 다른 저장소의 worker 변경을 기다리지 않는다." / 197: "Use one repository per ordinary observation" / 201-202: "A busy unrelated worker does not defer these decisions." | Limit: the global canonical barrier still applies. README.md:202-203: "The canonical broker's global pending barrier continues to apply." CS:35-36: "canonical pending/HALTED 장벽은 저장소 분리로 우회할 수 없다." |
| 9 활성 수정 PR 하나 | AGENTS.md:5 | "Keep at most one ACTIVE modifying branch and PR for this repository, including directly requested operational fixes." | Applies to the NPG repo only. For worker repos, AUTOMATIC_FOLLOWTHROUGH.md:287-289 says the existing "single modifying PR gates remain" and does not define them. |
| 10 독립 검토·CI 이후 병합과 main CI 확인 | CENTRAL_SCHEDULER_MIGRATION.md:41-43; CENTRAL_SCHEDULER_MIGRATION.md:58-59; AUTOMATIC_FOLLOWTHROUGH.md:276-277; OPERATIONS.md:64-65 | CSM:41-43: "After genuine independent review, managed current-head review, CI, normal expected-head landing and main CI, the normal installer writes" / CSM:59: "mainCI: matching merged commit and conclusion success." / AF:276-277: "Return the repository's old landing hold only after fresh normal landing/main checks and slot reconciliation." | NPG code never merges. It only verifies the merge: AUTOMATIC_FOLLOWTHROUGH.md:211 ("actual GitHub merge"), :218-219 (the merged PR must target the default branch). The CSM clauses are about installation gating for PR25. OPERATIONS.md:64-65 asks to confirm tests, CI and the normal review gates on a fixed commit, then record the main merge and the server install separately. AGENTS.md:11 keeps the main landing gates in force. |
| 11 완료·RELEASE 뒤 직접 후속 전부의 처분과 이월 | templates/central-supervisor.md:65-66 (remote); templates/central-supervisor.md:72-73 (remote); README.md:220; README.md:232-233; AUTOMATIC_FOLLOWTHROUGH.md:270-271 | CS:65-66: "매 실제 RELEASE/landing 사건의 직접 후속 전체를 원 DAG에서 찾아 PROMOTE, RECIRCULATE, KEEP_GATED(미충족 조건·owner·trigger·source), DONE으로 처분한다." / CS:72-73: "반환 pending을 기존 canonical state의 private audit 영역에 보존하고 다음 회차 첫 입력으로 다시 읽는다." / 220: "An omitted prior pending event remains pending." / 232-233: "Persist the returned pending list in the existing private audit area and read it first next time." | Implemented in `supervision_gate.js` `assess` (README.md:206-234). Limit at README.md:233-234: "This check cannot discover omitted DAG successors or authenticate caller evidence". |
| 12 검토 timeout·대체 한도·늦은 결과 연결 | OPERATIONS.md:139; OPERATIONS.md:148-149; OPERATIONS.md:155-156; OPERATIONS.md:161-162; REVIEW_TIMEOUT_OPERATIONS.md:165-168 (remote) | 139: "30분 이상이면 WARN, 정확히 60분까지는 대기, 60분 **초과**면 timeout이다." / 148-149: "동일 head/base의 대체 검토 후보를 최대 1회만 낸다." / 155-156: "대체까지 timeout이면 `BLOCKED / REPLACEMENT_TIMEOUT`과 `nextOwner`, `next`, `nextTrigger`를 남긴다." / 161-162: "늦게 도착한 선행 결과와 새 head 이전의 결함도 삭제하지 않는다." / RTO:167-168: "A replacement timeout is BLOCKED with owner/next/trigger, never an automatic third request." | **Not adopted.** OPERATIONS.md:126 is headed "(미채택 정책)". deployment/review-timeout-operational.json:3 reads `"OPERATIONAL_CANDIDATE_NOT_DEPLOYED"` (remote). REVIEW_TIMEOUT_TRANSITION.md:126 reads "Safe adoption handoff — not performed" (remote). The budget is one replacement per repo/PR/head (OPERATIONS.md:181-182). |
| 13 사용량 게이트와 해소 후 재기동 | SOURCE_REVIEW_BOOTSTRAP.md:33; SOURCE_REVIEW_BOOTSTRAP.md:35-36; REVIEW_TIMEOUT_OPERATIONS.md:74-76 (remote); AUTOMATIC_FOLLOWTHROUGH.md:287-289 | SRB:33: "Executes the actual LIVE usage gate immediately before launch." / SRB:35-36: "Keep the existing usage watchdog and thresholds." / AF:287-289: "All existing usage/provider, model, HOLD, ownership, worktree and single modifying PR gates remain." | **GAP: 해소 후 재기동 (restart after the limit clears).** No clause says what happens once a usage limit clears. NPG defers to an existing gate it does not define, and it does not change that gate. README.md:268-269: "No Claude wrapper, timer, session, usage policy, or worker-owned proof is changed." SPRINT_U0.md:22 (remote) says no "usage gate is changed". The review-timeout gate list includes `usage` (RTO:75), but only as one gate to attest. |
| 14 읽기 전용 큐 감시와 코디네이터 정지 알림 | templates/queue-watch.md:1 (remote); templates/queue-watch.md:7-8 (remote); templates/queue-watch.md:16-17 (remote); AUTOMATIC_FOLLOWTHROUGH.md:200-201 | QW:1: "큐 감시 — 읽기 전용 공통 모듈 템플릿" / QW:7-8: "runtime·systemd를 읽지 못한 회차는 UNKNOWN을 보고하며 이전 정상값을 현재값으로 재사용하지 않는다." / QW:16: "실제 HALTED·불확실한 쓰기는 동일 batch와 보존 증거를 담당 중앙 owner에게 보고한다." / AF:200-201: "Keep the watch read only." | **Partial GAP: 코디네이터 정지 알림.** No clause tells the watch to alert anyone when the central coordinator itself is stopped or disabled, and no alert channel is named. QW:16 covers reporting a HALTED or uncertain write to the central owner. QW:17 lists what the watch must not do: re-request, raw Notion edits, a new batch, running a worker, restarting a service, or merging. REVIEW_TIMEOUT_TRANSITION.md:97 (remote): "no scheduler or notification is provided." SORI48_UNKNOWN_CLOSURE_PROPOSAL.md:91-92 (remote) calls the Watch a "read-only detector/reporter". |
| 15 사람이 보는 큐·허브·대시보드 | README.md:164-165; AUTOMATIC_FOLLOWTHROUGH.md:228-229 | 164-165: "It requires a fixed manifest: both native central metadata records, the repo index, both role queues, Router, hub and dashboard." / AF:228-229: "including the role queues, Router, hub and dashboard." | These are Notion body targets that NPG writes. No clause calls them human-facing or describes their content. The page IDs in policy.json:4-16 are not labelled. Per-repo counts are in README.md:197-199 and templates/central-supervisor.md:31-33. |
| 16 역할별 지침·템플릿 (latest evidence, allowed scope) | AGENTS.md:13; templates/central-supervisor.md:17-18 (remote); OPERATIONS.md:96-98; source_review_request.cjs:37-38 (remote) | AGENTS:13: "Dated branch inventories are evidence snapshots, not execution authority. Re-read GitHub before starting the next task." / CS:17-18: "현재 권한과 GitHub head·CI·worker 원본 증거를 새로 읽는다. 다른 worker의 CLAIM·lease·RELEASE·자기 증빙을 대신 작성하지 않는다." / OPS:96: "템플릿의 자리표시자는 실제 읽기로 확정한다." / SRR:37-38: "`No source edits, new task/session, central/product/queue/canonical writes, Codex request, thread action, merge, install, replay or synthetic ACK.`" | Only two templates exist: central-supervisor and queue-watch (OPERATIONS.md:92-94). **GAP:** no Operator or Guardrail worker-role instruction file is in the repo. The real prompts are private (OPERATIONS.md:14; templates/central-supervisor.md:3-4). The SRR payload is the only role-scoped instruction, and it covers just the Guardrail source review of NPG. The SRR allowedTools list is at source_review_request.cjs:40-42. |
| 17 브로커의 공통 권한 원칙 | broker.cjs:224-225 (remote); broker.cjs:242-244 (remote); policy.json:3 (remote); README.md:339-342; README.md:280; README.md:319-325; OPERATIONS.md:213-216 | broker:225: "'OWNERSHIP', 'Only explicit central-owned observation headings are editable'" / broker:244: "'PRODUCTION_HOLD', 'Production remains blocked until incident restoration'" / 339-340: "`productionEnabled:false` is the checked-in default. Only the two explicit private NON-EXECUTABLE canary pages are writable under that default." / 280: "No offsets, ellipsis selection, full replacement, append or replace-all exist." / 324-325: "Do not describe a prompt rule as capability isolation." / OPS:215-216: "allowlist 확대나 Notion 직접 쓰기로 우회하지 않는다." | Elements of the allow-list: policy.json `productionPages` (lines 4-16), `canaryPages` (17-20), `centralSections` (21-30) and `headingPrefixes` (31-35). broker.cjs:228-229 limits literals to 1024 characters on one line. broker.cjs:355-357 shows `update_content` with `allow_deleting_content: false` and `replace_all_matches: false`. **GAP:** no general external-write allow-list covers GitHub or other effects. The allow-list applies only to Notion body edits. The deployed policy is private and differs from the checked-in file (OPERATIONS.md:29-31). |

## (a) CI workflow (`ci.yml`, identical to `.github/workflows/ci.yml` at the commit)

- Triggers, ci.yml:3-7: push to `main`, `migration/**` and `fix/**`; `pull_request`; `workflow_dispatch`.
- Permissions, ci.yml:9-10: `contents: read`.
- Job `guard`, ci.yml:13-19: `runs-on: ubuntu-24.04`, `fail-fast: false`, matrix `node: [22, 24]`.
- Setup, ci.yml:21-26: `actions/checkout@v7` with `fetch-depth: 0`, then `actions/setup-node@v7` with `${{ matrix.node }}`.
- ci.yml:27-28, "Guard and installed-runtime regressions": `node --test test_guard.cjs test_runtime.cjs test_revision.cjs supervision_gate_cases.cjs serialized_format_cases.cjs test_nodekit_pointer_partial.cjs review_timeout_policy_cases.cjs review_timeout_ledger_cases.cjs review_timeout_operational_cases.cjs test_fenced_dispatch.cjs test_unwritten_recovery.cjs test_case_exception.cjs test_case_ack_bridge.cjs`
- ci.yml:29-30: `node verify_review_timeout_candidate.cjs`
- ci.yml:31-32, "Ordinary landing and current-task projection regressions": `node --test complete_projection_cases.cjs test_automatic_followthrough.cjs test_normal_live.cjs test_permit_expiry.cjs test_sori_control.cjs test_sori48_unknown_state.cjs test_sori48_unknown_closure.cjs test_captured_automation_run.cjs test_sori48_unknown_probe.cjs test_sori48_unknown_runtime.cjs test_sori48_unknown_evidence.cjs test_sori48_unknown_sources.cjs test_sori48_pair_provider.cjs test_sori48_pair_metadata.cjs test_normal_client_required.cjs`
- ci.yml:33-34, "Scoped source review bootstrap regressions": `node --test test_source_review_request.cjs test_sori48_pair_correction.cjs test_central_migration_artifacts.cjs test_central_migration_files.cjs test_central_migration_execution.cjs`
- ci.yml:35-36: `node verify_review_timeout_operational.cjs`
- ci.yml:37-38: `python3 -m unittest -v test_provenance.py`
- ci.yml:39-43: `git fetch` of `infra-lab` tag `archive/npg-before-split-20261020` and of `origin` tag `import/infra-lab-20261020`.
- ci.yml:44-45: `python3 verify_provenance.py`

The docs list shorter test commands than CI runs:
- README.md:151-152 gives a 6-file `node --test` command "on Node 22 or 24".
- OPERATIONS.md:103 gives a 2-file command.
- REVIEW_TIMEOUT_VERIFICATION.md:12 gives an 8-file command.

## (b) Statements about installation, runtime or operations state (treat all as past snapshots, not current evidence)

Docs (local copies are byte-identical):
- README.md:156-158: "The recorded installed broker/runner come from full commit `01b8a7897bc8709450f536744577313c805c66ca`"
- README.md:266-267: "The broker uses the existing server-direct Desktop Commander device and its fixed approved command."
- README.md:323-324: "the current recovery's enabled state is recorded separately in private execution evidence."
- README.md:332-333: "The runtime is `/home/heain/.local/share/notion-projection-guard` and durable state is `/home/heain/.local/state/notion-projection-guard`."
- README.md:334-335: "Runtime code is 0555; policy/manifest 0444; snapshots/journal 0600 within a 0700 directory."
- README.md:342-343: "The existing approved runtime policy has its own pinned hash and private activation record"
- README.md:349: "A crash can leave `broker.lock`: inspect its recorded PID on heain" (host `heain`; file `broker.lock`)
- README.md:667-669: "Its installation and real scheduled adoption require separate evidence; source availability is not a runtime completion claim."
- OPERATIONS.md:21-27, section "현재 설치 버전": "서버 설치 receipt는 2026-10-19 23:02:49 UTC의 `01b8a789…`를 가리키며, 2026-10-20 00:07:09 UTC에 broker·runner·policy·approved manifest를 읽어 확인했다."
- OPERATIONS.md:34: "검증 도구는 현재도 이 명세로 같은 설치 bytes를 대조한다."
- VERIFICATION_20261012.md:3: "중앙 감독도 재개하지 않았다."
- VERIFICATION_20261012.md:14: "중앙 설정 readback | NPG-20261012-v1 pinned runner 경로 저장 확인; is_enabled=false; 기존 schedule 동일"
- VERIFICATION_20261012.md:15: "Queue Watch | enabled=true, 기존 읽기 전용 prompt 동일"
- VERIFICATION_20261012.md:16: "runtime 무결성 | broker/policy/runner 승인 SHA256 일치; code0555, policy/runner/manifest0444; 활성 broker lock0"
- VERIFICATION_20261012.md:36-38: "중앙 automation: 6a742f6443e8819184310cd28e6162cd" plus runtime `/home/heain/.local/share/notion-projection-guard` and state `/home/heain/.local/state/notion-projection-guard`
- VERIFICATION_20261020_OPERATIONS.md:8: "실제 자동화 프롬프트·스케줄·서비스와 canonical state는 이번 작업으로 변경하지 않았다."
- VERIFICATION_20261020_OPERATIONS.md:16-27: "2026-10-20 00:11:07.262 UTC … 실제 서버에서 새 검증 모듈을 메모리로 실행했다." Result: `"status": "MATCH"`, `sourceCommit 01b8a789…`, `installedAt 2026-10-19T23:02:49.009Z`, `"sourceFilesChecked": false`.
- SOURCE_REVIEW_BOOTSTRAP.md:3-5: "The ordinary hourly Guardrail selector does not consume private request JSON. It selects eligible current Notion rows; the NPG row ended at RELEASE36."
- SOURCE_REVIEW_BOOTSTRAP.md:9-11: "the installed Claude binary, original Guardrail settings/hooks, existing role lock, and the existing Guardrail session/task/row."
- SOURCE_REVIEW_BOOTSTRAP.md:25: "Holds `/home/heain/.local/state/claude-guardrail/role.lock` with nonblocking `flock`"
- SOURCE_REVIEW_BOOTSTRAP.md:30: "Uses the original installed `case_gate.cjs launch` with `--role guardrail`"
- CENTRAL_SCHEDULER_MIGRATION.md:3-5: "preserves the original Sori48 incident owner automation:6a742f6443e8819184310cd28e6162cd. It admits the prospective automation:6ae6e6e4c880819192e89cd430271bfa executor"
- CENTRAL_SCHEDULER_MIGRATION.md:20-21: "The original capture directory is fixed beneath the existing Sori48 audit directory."
- CENTRAL_SCHEDULER_MIGRATION.md:42-43: "the normal installer writes PR25_IMMUTABLE_INSTALLATION_MANIFEST.json in the existing Sori48 audit directory." This describes the procedure. It does not record that the install happened.
- AUTOMATIC_FOLLOWTHROUGH.md:121-122: "The first scheduled adoption of PR16 halted at `SEND_INTENT` before a send permit existed."
- AUTOMATIC_FOLLOWTHROUGH.md:193-195: "retain the existing broker, runner, complete entry, policy, approved hashes, case controls, role hooks, timers, credentials and canonical journal."
- AUTOMATIC_FOLLOWTHROUGH.md:200-201: "Keep the existing central enabled state and hourly schedule. Keep the watch read only."
- AUTOMATIC_FOLLOWTHROUGH.md:293: "Use the existing worker service/timer and one real request receipt"
- AUTOMATIC_FOLLOWTHROUGH.md:315: "The original NPG/1 attempt is still HALTED at SEND-INTENT."
- REVIEW_TIMEOUT_VERIFICATION.md:3-4: "Baseline: `ac95acf6…` (remote main read back on 2026-10-25). GitHub open PR query returned no open PRs."
- REVIEW_TIMEOUT_VERIFICATION.md:46-47: "No push, PR creation/merge, remote server access, deployment, schedule change, real Codex request or cancel operation was performed."

Other files at the commit (remote):
- deployment/runtime-01b8a789.json:4, 15-16: `"sourceCommit": "01b8a789…"`, `"recordedInstallationAt": "2026-10-19T23:02:49.009Z"`, `"serverReadbackAt": "2026-10-20T00:07:09.227617Z"`.
- deployment/review-timeout-operational.json:3: `"status": "OPERATIONAL_CANDIDATE_NOT_DEPLOYED"`.
- deployment/review-timeout-candidate.json:4: `"status": "LOCAL_CANDIDATE_NOT_DEPLOYED"`.
- The other `deployment/runtime-*.json` files describe themselves in their `installationStatus` field as a "Candidate manifest" or "Candidate only; not deployed".
- SPRINT_R1.md:28: "Read-only server audit: fresh hourly role sessions; 479 inspected scheduled transcripts contained six child-agent calls"
- SPRINT_R1.md:35-39: "Last canonical read: `2026-10-26T23:03:48.158Z` … Active null; no broker lock; … both role tick services inactive at that instant."
- SPRINT_U0.md:21: "The supervisor remains disabled. Queue Watch remains read-only."
- SORI48_UNKNOWN_CLOSURE_PROPOSAL.md:89-92: "Existing central automation `6a742f6443e8819184310cd28e6162cd` … Existing Platform Queue Watch `6a7fea8fc3b481918a9aa102d21c4088` is the read-only detector/reporter. Their existing schedules stay unchanged."
- REVIEW_TIMEOUT_OPERATIONS.md:131: "The installed wrapper's contract command supplies private paths and this text."
- REVIEW_TIMEOUT_OPERATIONS.md:137: the run must "match the unchanged hourly minute/second".
- SCOPED_EXCEPTION.md:34-35: "The two existing role wrappers retain permissions, usage/auth gates, native timers, role locks and ordinary task preflight."
- SCOPED_EXCEPTION.md:50: "requesting the same existing systemd unit".
- Unit name in code, case_gate.cjs:105-110: `const unit='claude-'+role+'-tick.service'`, so `claude-<role>-tick.service` (e.g. guardrail, operator). The code runs `/usr/bin/systemctl --user show …` and `--user start --no-block`. No timer unit names appear anywhere. Timers are mentioned only generically.

Host paths hardcoded in code (remote):
- `/home/heain/.local/state/notion-projection-guard` and `/home/heain/.local/share/notion-projection-guard`: reconcile_*.cjs (for example reconcile_bori_prepared_mcp.cjs:4-5) and case_ack_bridge.cjs:6.
- reconcile_bori_prepared_mcp.cjs:7: `…/manual-audits/nodekit-pointer-bori-operator-20261021/install-P2-c4f77e1/installation.json`
- reconcile_w02_prewrite.cjs:8: `/home/heain/.local/share/notion-projection-guard/policy.json`
- central_migration_files.cjs:4: `/home/heain/.local/state/notion-projection-guard/manual-audits/sori48-closure-20261029`
- central_migration_files.cjs:5: `/home/heain/.local/share/review-timeout/config.json`
- central_migration_files.cjs:44-46: `…/review-timeout/runtime-approved.sha256`, `/home/heain/.local/bin/review-timeout-supervision`, `…/review-timeout/CONTRACT.md`
- central_migration_files.cjs:72: `/home/heain/.local/state/review-timeout-evidence`
- source_review_request.cjs:52, 54: `/home/heain/.local/bin/claude` and `/home/heain/.local/share/notion-projection-guard/case-guardrail-settings.json`
- source_review_request.cjs:42: `/home/heain/.local/bin/claude-role-alert.sh`
- sori_startup_control.cjs:12, 14: `/home/heain/work/packets/sori-w02-startup-20261028-391159f9/ADMISSION` and `/home/heain/work/packets/sori-w02-admin-tail-20261026/ADMISSION`

Notion pages named:
- policy.json:4-16: 11 `productionPages`.
- policy.json:17-20: 2 `canaryPages`.
- policy.json:21-30: 2 `centralSections`, the W02 pages `3f586e6dd1d181ebb43dd7de62aa2ca4` and `3f586e6dd1d181cd9f5bdcdab6290de7`.
- source_review_request.cjs:5: NPG Guardrail row `40786e6dd1d1818086faf517ccee6238`.
- SPRINT_R1.md:25-27: Notion agent rules `3b086e6dd1d181b8bc6ce70a509f9973`, runtime memory `3b286e6dd1d1818db922c7b1c31d7a14`, queues `3b286e6dd1d18194866dd8d7a28c7ab4`.
- No Notion database IDs are named as write targets.

"server SHA": no literal match. Server-side hash claims appear at OPERATIONS.md:27 ("검토된 이 커밋의 broker·runner 해시와 서버 파일이 일치했다") and VERIFICATION_20261012.md:16.

## (c) External effects NPG performs

1. **Notion page body writes.** These go only through the broker-generated request.
   - broker.cjs:355-357 (remote): `command: 'update_content', allow_async: false, allow_deleting_content: false … replace_all_matches: false`.
   - README.md:264-265: "`broker.cjs` now constructs the connector request. `runner.js` calls that request through the existing authenticated Notion connector."
   - README.md:283-284: "Save WRITE_PENDING before returning the one generated `update_content` request."
   - Targets are listed at README.md:164-165 (repo index, role queues, Router, hub, dashboard, central records).
2. **Notion native property writes.**
   - sori48_pair_metadata.cjs:113 (remote) issues `command:'update_properties'` for the Sori48 parent, child and pair rows.
   - AUTOMATIC_FOLLOWTHROUGH.md:225-226: "Reconcile native metadata through the existing minimal central-property path".
   - README.md:183-184 says the complete entry itself "does not write properties, worker proof or policy".
3. **GitHub issue comment (Codex review request).**
   - review_timeout_github.cjs:283-284 (remote): `api('POST','/repos/' + attempt.scope.repo + '/issues/' + attempt.scope.pr + '/comments',{body})`, sent exactly once with no retry (comment at :282).
   - It uses the token from `gh auth token` (:18).
   - REVIEW_TIMEOUT_OPERATIONS.md:100: "One review-dispatch POST embeds exact head/base, unique request UUID, predecessor and fence."
   - Status: candidate, not deployed (deployment/review-timeout-operational.json:3).
   - GitHub reads use GET and GraphQL POST queries (review_timeout_github.cjs:72-102).
4. **Starting a worker systemd unit.**
   - case_gate.cjs:110 (remote): `systemctl --user start --no-block` of `claude-<role>-tick.service`, after checking that the unit is inactive (:106-107).
   - SCOPED_EXCEPTION.md:48-50 describes it.
5. **Preparing a Claude CLI launch.** NPG builds the request but does not execute it.
   - source_review_request.cjs:2-3: "Never installs a candidate, starts a service, writes a worker event or grants role authority."
   - SOURCE_REVIEW_BOOTSTRAP.md:18-19: "It neither executes the CLI nor supplies permission or worker proof."
   - The prepared command (:52-55) gives the launched reviewer the tools `notion-update-page` and `claude-role-alert.sh` (:40-42).
6. **Local private state writes:** journal, snapshots, receipts and audits (README.md:281-282, OPERATIONS.md:18-19), plus the review-timeout ledger and outbound records (review_timeout_github.cjs:272).
7. **Notifications or alerts: none of NPG's own.**
   - REVIEW_TIMEOUT_TRANSITION.md:97: "no scheduler or notification is provided."
   - The Watch "reports" to the central owner (templates/queue-watch.md:16), but no channel is named.
   - The only alert artifact is the `claude-role-alert.sh` tool allowed to the launched reviewer.
8. **No merges, PR creation or pushes.** No code path does any of these; a grep for merge, push and PR-create API calls found nothing.
   - REVIEW_TIMEOUT_TRANSITION.md:162-164 says push, PR creation/merge, deployment, server commands, schedule changes and real review requests "remain unperformed".
   - queue-watch.md:17 forbids merges by the Watch.
