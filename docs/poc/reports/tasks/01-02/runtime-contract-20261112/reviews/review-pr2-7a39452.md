VERDICT: reject
TARGET_SHA: 7a39452ac0a514fd90efa431f47a38451ef5013d

Independent Guardrail review: PR #2 (HeaInSeo/ieo_poc), task 01-02, run `runtime-contract-20261112`, base main b12bc73.
Scope was read-only. I used a scratch worktree at `scratchpad/rev-pr2` and kept raw logs in `scratchpad/rlog/`. Both scratch paths are under /tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/. I made no model calls, started no sessions, did not log in, and did not issue tokens.

## Commands run (exit codes)

| # | Command | Exit | Raw log / note |
|---|---|---|---|
| 1 | `git fetch origin; git cat-file -t 7a39452ac0a5…` | 0 | object is a commit |
| 2 | `git worktree add <scratch>/rev-pr2 7a39452` | 0 | HEAD 7a39452 |
| 3 | `git diff --stat b12bc73 7a39452` and `git log --oneline b12bc73..7a39452` | 0 | 8 files, 1 commit; docs only |
| 4 | `claude --version` and diff against evidence/claude-version.txt | 0 / diff 0 | rlog/ver.txt (`2.1.322 (Claude Code)`) |
| 5 | `claude --help` and diff against evidence/claude-help.txt (minus the trailing `exit=0` line) | 0 / diff 0 | rlog/help.txt; byte-identical |
| 6 | `claude auth --help` and diff against evidence | 0 / diff 0 | rlog/authhelp.txt |
| 7 | `claude auth status`, piped through a redacting sed | 0 | rlog/authstatus.red.txt matches evidence apart from the redaction tokens |
| 8 | `claude auth status --help` | 0 | rlog/authstatushelp.txt |
| 9 | `claude setup-token --help` (help only, not run) | 0 | rlog/st.txt: "Set up a long-lived authentication token (requires Claude subscription)" |
| 10 | `codex --version` | 0 | `codex-cli 0.166.0` (/usr/bin/codex) |
| 11 | `strings <claude 2.1.322 binary> \| grep -oE '<5 vars>' \| sort \| uniq -c` | 0 | rlog/strings.txt; identical counts to evidence/env-var-strings.txt |
| 12 | `CLAUDE_CONFIG_DIR=<scratch>/cfgprobe/claude claude auth status` | 1 | rlog/cfgdir-status.txt: loggedIn false, configDirectory/projectsDirectory relocated to the scratch dir |
| 13 | `git diff b12bc73 7a39452 \| grep -Ei 'email\|sk-ant\|oat01\|uuid\|heain\|knou\|token":'` | 0 | only hits: `/home/heain/.claude…` (2 lines) |
| 14 | `python scripts/validate_package.py` (docs/poc, .venv-poc) | 0 | rlog/validate_package.txt |
| 15 | `python scripts/render_progress.py --check` | 0 | rlog/render_check.txt (`ok: true`) |
| 16 | `gh pr view 2 --json headRefOid,baseRefOid,state,statusCheckRollup` | 0 | head = 7a39452…, base = b12bc73…; both CI checks SUCCESS (run 43123246234) |
| 17 | python: compare result.json keys with reports/templates/result.template.json; check that evidence_refs exist | 0 | 11 template keys missing (see F8); all refs exist |
| 18 | `env \| grep -oE '^(CLAUDE…\|ANTHROPIC…)'` (names only) | 0 | the reviewer's own shell has CLAUDECODE, CLAUDE_CODE_SESSION_ID, CLAUDE_CODE_MESSAGING_SOCKET/TOKEN, … |

## What holds up (no finding)

- Every option the contract cites appears in the committed `claude --help`, and the committed help output reproduces byte-identically on the installed 2.1.322 binary: `-p`, stream-json in/out, `--permission-mode dontAsk`, `--permission-prompts none`, `--session-id <uuid>`, `-n/--name`, `-r/--resume`, `--fork-session`, `--no-session-persistence`, `--autocompact`, `--json-schema`, `--max-budget-usd`, `--agents`, `--forward-subagent-text`, `setup-token`. REPORT and contract both state that this proves the options exist, not that the behaviour works.
- `CLAUDE_CONFIG_DIR` relocation is supported beyond the string count. Command 12 shows that `auth status` follows the variable, and that the host login does not leak into an empty config dir; it reports loggedIn false.
- The display name is explicitly not the resume key, and resume success is judged by the session_id of the first result event. This is consistent with §1.2. The adapter op/error vocabulary matches §1.2. There is no shared volume, the supervisor has no admin rights over the other Pod, and the Guardrail role has no push. Credential option D (sharing the developer's `~/.claude`) is forbidden. The 01-04 stop rule (§8.4) matches 01-04.accept.
- The committed evidence contains no secrets or tokens. Email, orgId and orgName are redacted.

## Findings

### F1 · P1 · The trust boundary for runtime configuration is not fixed, so the Guardrail runtime would load and run development-authored repo config
- **Violation condition:** 01-02.accept requires the contract to fix the per-role filesystem and credential boundary. §1.2 and U3 forbid any path by which development code obtains Guardrail credentials or issues a review in Guardrail's name (P07 requires 0 successes). The contract instead leaves the launch configuration as "역할별 고정 인자 집합" (§7) and only lists `--setting-sources`, `--settings` and `--strict-mcp-config` as available means (§4), without fixing their values.
- **Reproduction:** Read `Runtime_Contract_v0.md` §4 row "무인 기동" and §7 row "start". Grep for setting/hook/CLAUDE.md/project/restricted/bare/trust (command list, grep step). The only hits are the option names in §4. Compare with evidence/claude-help.txt: lines 171-180 say `-p` skips the workspace trust dialog ("Only use this in directories you trust"). Lines 226-227 show that setting sources default to user/project/local. Lines 41-58 (`--bare`) show that hooks from settings and plugins, and CLAUDE.md auto-discovery, are on by default. Lines 195-209 (`--restricted`) show that user/project/local settings are otherwise honoured and that the agent can otherwise write settings and tool-config files.
- **Expected:** For the Guardrail role in particular, the contract fixes the following: no project or local settings, hooks, `.mcp.json`, plugins, agents or CLAUDE.md from the reviewed SHA's tree; MCP only from an approved config; settings supplied only by the supervisor through a read-only `--settings`; and an agent-writable `CLAUDE_CONFIG_DIR` that cannot carry permission rules or hooks into the next start or resume. The development role needs the same protection against self-escalating permissions persistently through `/state/claude/settings.json`. A 01-04/P07 probe should check each of these.
- **Observed:** None of this is fixed. Under the contract as written, the Guardrail runtime is started in `/state/work` on the reviewed SHA in `-p` mode, with no trust prompt. Development-authored hooks or settings in that tree would execute in the Guardrail Pod under the Guardrail UID and token. Development-authored CLAUDE.md would also be injected into the reviewer's context, which §1.2 forbids. `CLAUDE_CONFIG_DIR` lives on the agent-writable RW volume, so an agent can change its own allow rules or hooks and those changes survive a `--resume`. The §8 probe 2 tests only process-to-process access, not this repo-content path, so the 01-04 role allow/deny probe would be unsound.
- **Certainty:** supported. The default load behaviour is taken from the installed help text. I did not run a session to show a hook executing, because model calls are out of scope.
- **Raw log:** evidence/claude-help.txt L41-58, L171-180, L195-209, L226-231; rlog/help.txt (identical).

### F2 · P2 · The privilege boundary inside the Pod between the supervisor and the agent, and authentication of the adapter's callers, are undefined
- **Violation condition:** 01-02.accept: "HTTP/JSON 런타임 어댑터와 Pod별 PID 1 supervisor의 자기 역할만 … 권한 경계를 정하고". §1.2 says the supervisor receives the coordinator's *authenticated* assignment.
- **Reproduction:** Contract §2 gives one UID per Pod (20001 and 20002). That UID covers both PID 1 (the supervisor) and the agent, which has Bash tools. §5 puts checkpoints in `/state/checkpoints` on the same RW volume. §7 does not say how the supervisor authenticates callers of its HTTP/JSON API, or where its credential for the coordinator API lives. The Guardrail role's "코디네이터 API로 claim 제출" also does not name a credential.
- **Expected:** Either the supervisor and agent run under separate UIDs, with checkpoints and the supervisor credential unreadable and unwritable by the agent, or the contract records why sharing a UID is acceptable. It should also name the caller-authentication mechanism and state that the agent cannot call its own supervisor's start/stop.
- **Observed:** Not specified. As written, the agent can forge or alter checkpoints, which the reconnect path then trusts, and can use the supervisor's coordinator credential.
- **Certainty:** supported (from the document).
- **Raw log:** Runtime_Contract_v0.md §2 table rows "실행 UID/GID" and "볼륨 내부 배치"; §5 step 1; §7.

### F3 · P2 · `quota_group_id` for options A and D is asserted ahead of the 01-04 judgement
- **Violation condition:** Check 4 and 01-04.accept: provider quota sharing or separation is decided only by the 01-04 probe.
- **Reproduction:** Contract §3, last bullet: "A/D이면 두 역할이 같은 group이다." The §8 probe 3 then asks "두 역할의 quota group이 같은지".
- **Expected:** Either a provisional value labelled as such (for example "가정: 동일, 01-04에서 확정"), or `unknown` until the probe runs.
- **Observed:** The text states it as fact, which makes probe 3 tautological for option A. Elsewhere the wording is correctly hedged: §3 table "공유가 예상됨 → 01-04 판정", and REPORT "가능성이 높다…판정 아님".
- **Certainty:** reproduced (text).
- **Raw log:** Runtime_Contract_v0.md §3 bullet 3; §8 item 3.

### F4 · P2 · The "인증" element is left open, and the owner and route for closing it contradict each other
- **Violation condition:** 01-02.accept lists 인증 as something the contract must fix.
- **Reproduction:** REPORT "01-04에 미리 드러난 위험" says the A/B/C choice is "01-03 결정안에 포함". Contract §9 says it is a "사람 정책 소유자 결정. 01-04 첫 probe 전 필요". result.json `unresolved[0]` matches the contract. 01-03.accept (Sprints_v0.json) contains nothing about choosing a credential method.
- **Expected:** One stated decision request with a single owner, a deadline ("before 01-04 probe 1"), the options, and their impact. The request should not be attached to a task whose acceptance does not cover it.
- **Observed:** The two documents name different routes. It is acceptable to defer the choice to the user, but the PR should resolve the inconsistency before 01-02 is closed.
- **Certainty:** reproduced (text).
- **Raw log:** REPORT.md "01-04에 미리 드러난 위험"; Runtime_Contract_v0.md §9; Console_Coordinator_Sprints_v0.json `01-03.accept`.

### F5 · P3 · Some "설치 버전 근거" claims slightly overstate the evidence
- (a) §6: "사용량: `--forward-subagent-text`로 … 자식 사건을 부모에 귀속". The help text (L107-110) says only that this flag forwards subagent *text and thinking blocks* with `parent_tool_use_id`. It says nothing about usage or token records, so it supports attribution of events, not usage.
- (b) §3 C: "`--bare`는 이것만 사용" (ANTHROPIC_API_KEY). The help text (L48-50) says "ANTHROPIC_API_KEY or apiKeyHelper via --settings".
- (c) §3 A: the evidence for `CLAUDE_CODE_OAUTH_TOKEN` is a string count (198) only. Nothing shows that the variable is read as auth, or how it ranks against a stored login in `CLAUDE_CONFIG_DIR`. The contract labels this as string presence, which is correct, but A's "자격 분리" column should also mark auth precedence as a probe item.
- (d) §1 and result.json: "codex-cli 0.166.0 설치 관측" has no evidence file in this run. The previous run's codex-version.txt is 0.167.0-alpha.4 from a different environment. I confirmed 0.166.0 on this host (command 10), so the claim is true but not evidenced in the PR.
- **Certainty:** reproduced. **Raw log:** evidence/claude-help.txt L41-58, L107-110; evidence/env-var-strings.txt; runtime-comparison-20261112/evidence/codex-version.txt.

### F6 · P3 · Coverage gaps against §1.2 and §1.7 (not acceptance blockers)
- The checkpoint fields (§5) leave out the image digest, profile version and spec version/digest. §1.2 requires these on the volume and requires them to be checked on reconnect ("문서 버전").
- The contract does not cover a repo-scope stop. §1.2 requires that each role's safe-stop conditions be checked individually.
- §6 does not record the child ID, parent ID or generation, and does not describe recovery after the parent aborts ("무조건 새 자식을 띄우지 않는다"). §1.7 requires both. This may belong to P35, but the contract should say so.
- §4 has no row for limit signals in the installed version. The help text has no flag for limit or quota signals, and the contract should state that and mark it as a probe. There is also no mapping from runtime auth, limit or transient errors to the adapter error codes, and no row for "개발과 Guardrail은 직접 통신하지 않는다" (network).
- **Certainty:** supported. **Raw log:** Validation_Plan §1.2 table and the adapter paragraph; §1.7 "자식 경계".

### F7 · P3 · Privacy in the public evidence
- `claude-auth-status.redacted.txt` keeps the absolute path `/home/heain/.claude`, which exposes the host username. It also keeps `loginExpiresAt`, `planTier max_5x` and `analyticsDisabled:false`. Email, orgId and orgName (which contained the email) are correctly redacted. No tokens, keys or session IDs are present.
- Exposure is low, because the username matches the public GitHub owner. Consider replacing the paths with `~/.claude`, as REPORT already does, and dropping `loginExpiresAt`.
- **Certainty:** reproduced. **Raw log:** command 13; evidence/claude-auth-status.redacted.txt L5-7.

### F8 · P3 · Provenance and result-record completeness
- result.json omits these template keys: `started_at`, `finished_at`, `next_owner`, `resume_condition`, `reuse_assets`, `operational_report_ref`, `schedule_report_ref`, and `cost.quota_*` / `review_pending_dev_idle_seconds`.
- The evidence files do not record the exact command line or timestamp; for example, nothing says how `env-var-strings.txt` was generated. I reproduced it with `strings | grep -oE | sort | uniq -c`.
- CI is green and both validators pass.
- **Certainty:** reproduced. **Raw log:** command 17; rlog/strings.txt.

### F9 · P3 · 01-04 probes could inherit the developer session's environment
- §1 excludes `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID` and `CLAUDE_CODE_MESSAGING_*` from role processes. However, §8 does not require 01-04 probes to run inside the role containers or under a cleared environment (`env -i` plus an allowlist).
- These variables, including `CLAUDE_CODE_MESSAGING_TOKEN`, are present in a shell spawned from the developer's Claude Code session (command 18). A probe started from that session could inherit the parent session's identity and messaging channel, or be confounded by it.
- **Certainty:** unconfirmed (effect not tested). **Raw log:** command 18 (names only).

## Summary

The contract is well structured, and the static evidence holds up: every cited option reproduces byte-identically on Claude Code 2.1.322, and `CLAUDE_CONFIG_DIR` isolation of `auth status` is confirmed. It distinguishes "option exists" from "behaviour verified" almost everywhere, it does not use the display name as a resume key, and it contains no secrets.

I am rejecting it for one P1. The contract leaves the role launch configuration unfixed, and with the selected runtime's defaults (`-p` skips trust; project/local settings, hooks and CLAUDE.md load automatically; the config dir is agent-writable), the Guardrail Pod would run development-authored repo configuration under Guardrail credentials. That breaks the U3/§1.2 credential boundary that 01-02.accept asks the contract to fix, and it makes the 01-04 allow/deny probe and P07 unsound.

To fix it, fix the per-role argument set and settings trust: for Guardrail, exclude project/local sources, hooks, plugins and auto-discovered CLAUDE.md from the reviewed tree; supply settings read-only from the supervisor; and add the matching 01-04/P07 probe. Also address:
- F2: separate the supervisor and agent privileges inside the Pod, and define how the adapter authenticates callers.
- F3: stop asserting the quota group for option A as fact.
- F4: give a single owner and route for the credential-method decision.
