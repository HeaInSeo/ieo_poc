VERDICT: reject
TARGET_SHA: bf854bb6f1914da6ec8aaa95dd37cdf86fb7c0b9

Independent Guardrail re-review: PR #2 (HeaInSeo/ieo_poc), task 01-02, run `runtime-contract-20261112` (contract revision 2). I reviewed the full diff b12bc73..bf854bb. The correction commit is bf854bb, on top of 7a39452, which I rejected earlier; that review is review-pr2-7a39452.md.

This review was read-only. I used a scratch worktree at `scratchpad/rev-pr2b`, and raw logs are in `scratchpad/rlog2/` (plus `scratchpad/rlog/` from the first review). Both are under /tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/. I made no sessions, model calls, logins, GitHub posts or merges.

## Commands run (exit codes)

| # | Command | Exit | Raw log / note |
|---|---|---|---|
| 1 | `git fetch origin; git cat-file -t bf854bb6…` | 0 | object is a commit |
| 2 | `git worktree add <scratch>/rev-pr2b bf854bb` | 0 | |
| 3 | `git log --oneline b12bc73..bf854bb`; `git diff --stat 7a39452 bf854bb` | 0 | 2 commits; 5 files changed. Added: evidence/commands.txt and evidence/codex-version.txt |
| 4 | `git diff --stat 7a39452 bf854bb -- evidence/claude-help.txt evidence/claude-auth-status.redacted.txt` | 0 | empty, so both files are unchanged from 7a39452. claude-help.txt was shown byte-identical to the installed 2.1.322 output in the first review (rlog/help.txt) |
| 5 | `grep -c -- <opt> evidence/claude-help.txt` for every option in §4.1 argv, §3 and §4.2 | 0 | rlog2/argv-check.txt: every option has 1 or more hits (`-p --input-format --output-format --verbose --session-id -n --permission-mode dontAsk --permission-prompts none --setting-sources --settings --strict-mcp-config --mcp-config --agents --allowedTools --disallowedTools --add-dir --disable-slash-commands`, plus `--safe-mode --bare apiKeyHelper --forward-subagent-text`) |
| 6 | `grep -n -- '--add-dir\|CLAUDE.md' evidence/claude-help.txt` | 0 | L56-57: "--add-dir (CLAUDE.md dirs)"; L214: safe-mode customization list |
| 7 | `python scripts/validate_package.py` (docs/poc, .venv-poc) | 0 | rlog2/validate_package.txt |
| 8 | `python scripts/render_progress.py --check` | 0 | rlog2/render_check.txt (`ok: true`) |
| 9 | python: compare result.json keys with the template | 0 | missing [] / extra [] |
| 10 | `git diff b12bc73 bf854bb \| grep -Ei 'email\|sk-ant\|oat01\|uuid\|token":'` | 1 | no matches |
| 11 | `gh run watch 43124180490 --exit-status`; `gh run view 43124180490 --json headSha,conclusion,jobs` | 0 / 0 | rlog2/ci-watch.txt. headSha bf854bb6…; conclusion success; both jobs ("docs/poc package and progress validation", "rust toolchain pin resolves") succeeded |

## Prior findings: resolved or not

| Prior | Status at bf854bb | Basis |
|---|---|---|
| F1 (P1) settings trust | **Partly resolved.** The repo `.claude/` settings, hooks and `.mcp.json` path is closed by `--setting-sources user` plus `--settings` (RO) plus `--strict-mcp-config`, and probe G was added. Two gaps remain: the Guardrail argv's `--add-dir` is documented as a CLAUDE.md source (N2), and the override covers only `settings.json` and only at start (N3). | §4.1, §8.5 |
| F2 (P2) same UID / adapter auth | **Partly resolved.** §2.1 now states the limitation and the adapter caller auth (coordinator-only credential, supervisor holds only a verification key). Its mitigation argument is wrong for the Guardrail role (N1). The checkpoint digest is reported with a credential the agent can read (N5). | §2.1 |
| F3 (P2) quota group pre-decided | **Resolved.** §3 item 3 now says the group is decided in 01-04 as shared, separate or unobservable. Table A says "공유 가능성이 높다(판정 아님)". REPORT is hedged. | §3, REPORT |
| F4 (P2) owner and route of the auth choice | **Resolved.** §9, REPORT and result.json all say: the user decides, before the first 01-04 probe, and the question is asked alongside 01-03 but "01-03 수용 기준 자체의 항목은 아님". | §9, REPORT, result.json |
| F5 (P3) overstatements | **Resolved.** (a) §4.2 says text/thinking only, no usage. (b) §3 C says "API key 또는 apiKeyHelper". (c) For OAUTH_TOKEN it says "우선순위·적용 방식은 미확인". (d) codex-version.txt was added. One new unsupported claim: N6. | §3, §4.2 |
| F6 (P3) coverage | **Mostly resolved.** The checkpoint now records version and digest fields; repo-scope stop, child records and parent-abort handling, and the error and limit mapping were added. Still missing: §1.2 "개발과 Guardrail은 직접 통신하지 않는다" (network) is not stated. The new error mapping has a defect (N4). | §5, §6, §7 |
| F7 (P3) privacy | **Acknowledged, not changed.** commands.txt documents the username, expiry and plan tier as deliberate non-secret exposure. Acceptable. | evidence/commands.txt L8 |
| F8 (P3) provenance | **Resolved.** result.json now has every template key, and commands.txt lists the generating commands. A small revision-label mismatch remains (N7). | command 9 |
| F9 (P3) env inheritance | **Resolved.** §4.3 starts the process from an empty environment plus an allowlist and logs the variable names, for both the supervisor and 01-04 probes. | §4.3 |

## Findings (new or residual)

### N1 · P1 · The Guardrail runtime may run code from the reviewed tree under a UID that can read the Guardrail credentials (introduced by §2.1 together with §4.1)
- **Violation condition:** §1.2 requires that P07 see 0 successes of "개발 코드·빌드·테스트·자식의 … 자격 획득/Guardrail 명의 검토 발행 시도" before S0. 01-02.accept requires the contract to fix the per-role credential boundary. A contract that fixes a configuration in which this is achievable by construction does not meet that requirement.
- **Reproduction:** Read three places together:
  1. §4.1 Guardrail argv: "허용 도구는 읽기·**검사 실행**만", with the reviewed SHA attached through `--add-dir`. Running the checks means executing development-authored code: tests, build scripts, Makefiles and similar.
  2. §2.1: "같은 UID의 에이전트가 supervisor의 파일·환경을 읽거나 로컬 파일을 바꿀 수 있다고 가정". The supervisor's coordinator credential can submit "그 (repo, role)의 claim", which here means Guardrail claims.
  3. §4.3: the role's provider credential variable is in the process environment, and child processes such as the Bash tool inherit it. `/run/ieo/cred/` is readable by that same UID.
- **Expected:** One of the following.
  - (a) The Guardrail process never executes code from the reviewed tree. Checks come from CI or a coordinator-owned runner, and Guardrail reads only their results.
  - (b) Execution happens in a separate sandbox that cannot read `/run/ieo/cred`, the supervisor credential or the Guardrail `CLAUDE_CONFIG_DIR`.
  - (c) The contract records this as an open blocking design gap with a decision request, rather than as a bounded limitation.
- **Observed:** §2.1 argues that a same-UID agent "그 역할이 원래 할 수 있는 claim 이상은 못 한다". For Guardrail, that bounded capability is exactly the harm. Development-authored test code run by the Guardrail agent can read the Guardrail claim credential and submit a Guardrail-named approval, and it can also use the Guardrail model token. §2.1 defers verification to P07, but the design as written is expected to fail P07.
- **Certainty:** supported (from the document; nothing was executed).
- **Raw log:** Runtime_Contract_v0.md L32-39 (§2.1), L82-83 (§4.1 Guardrail argv), L108 (§4.3); Validation_Plan §1.2 "역할별 Pod 격리" row.

### N2 · P2 · The chosen Guardrail argv contradicts committed help evidence on CLAUDE.md loading through `--add-dir`
- **Violation condition:** §4.1 requires that the Guardrail not apply CLAUDE.md from the reviewed tree. The argv still has to be consistent with the committed evidence.
- **Reproduction:** evidence/claude-help.txt L55-57 (the `--bare` text) lists "--add-dir (CLAUDE.md dirs)" among the explicit context sources. In other words, the help documents directories passed with `--add-dir` as CLAUDE.md sources. §4.1 attaches the reviewed tree to the Guardrail with exactly `--add-dir /state/work/<repo>@<sha>`.
- **Expected:** The contract acknowledges this documented behaviour and picks a mechanism that keeps the reviewed tree readable without making it a memory source. Examples: read-only file access only through a non-`--add-dir` path, or a flag or setting that disables CLAUDE.md discovery for added dirs. That choice is then made a primary probe-G case.
- **Observed:** The help line is not mentioned. Probe G would probably catch it, and §4.1 L87 correctly treats any application of the file as failure. But the fixed argv is the configuration the evidence already flags as the risk.
- **Certainty:** supported (help text). Whether CLAUDE.md is actually loaded without `--bare` is unconfirmed.
- **Raw log:** evidence/claude-help.txt L41-58; command 6.

### N3 · P2 · The settings-trust enforcement covers only `settings.json`, and only before start
- **Violation condition:** §4.1 rule: config from agent-writable locations, "`CLAUDE_CONFIG_DIR` 안의 settings" and plugins included, is not applied.
- **Reproduction:**
  - §4.1 L86: the supervisor overwrites `CLAUDE_CONFIG_DIR/settings.json` only "매 기동 전". Probe G (L153) tests only whether tampering "다음 기동에 반영되지 않는지".
  - evidence/claude-help.txt L213-218 (`--safe-mode`) shows that further user-scope customization sources exist: CLAUDE.md, skills, plugins, hooks, MCP servers, custom commands and agents, output styles, workflows. All of these live under the RW `CLAUDE_CONFIG_DIR` or HOME.
  - The claim that the user source is `CLAUDE_CONFIG_DIR/settings.json` is not in the help (see N6).
- **Expected:** Either the whole user-scope config tree is supervisor-owned, read-only or regenerated, or user scope is disabled. Probe G should also test (i) mid-session edits to `settings.json`, `agents/` and user `CLAUDE.md` while a session runs (live reload), and (ii) user-level `agents/`, `skills/`, `commands/` and `CLAUDE.md` planted under `CLAUDE_CONFIG_DIR`, not only `settings.json`. `--disable-slash-commands` is applied to Guardrail only; for the development role, user skills and commands remain an open path.
- **Observed:** Only the start-time `settings.json` overwrite is specified.
- **Certainty:** supported (help text plus the contract text); the live-reload behaviour is unconfirmed.
- **Raw log:** Runtime_Contract_v0.md L64, L86, L153; evidence/claude-help.txt L195-223.

### N4 · P2 · The §7 error mapping files auth/billing errors and usage limits under `TRANSIENT_FAILURE`
- **Violation condition:** §1.2 requires distinguishing context limits, provider usage limits, rate limits and auth/billing errors. It also requires that recovery from a limit go through `next_check_at` and a resume probe with a fixed interval and cap, and that duration alone not be taken as recovery. "Transient" implies the request may be retried.
- **Reproduction:** §7 L145: "일시적 네트워크·공급자 오류(재시도 가능), 한도/rate limit 신호 → `TRANSIENT_FAILURE` + … 원 오류 분류(컨텍스트/사용량 한도/rate limit/인증·과금)".
- **Expected:**
  - Runtime auth or billing failures (expired or revoked token) are non-retryable. They map to a distinct non-transient outcome, for example `UNAUTHORIZED` with a runtime-credential subtype or a separate classification, plus a decision request.
  - Usage limits map to a quota-wait state governed by `next_check_at` and the 01-03 caps.
  - Context limits map to compaction handling, not retry.
- **Observed:** All of these share the "재시도 가능" code. A generic retry policy keyed on `TRANSIENT_FAILURE` would loop on a dead credential or spend model calls against an exhausted quota group, which conflicts with the probe caps 01-03 is supposed to freeze. The sub-classification field reduces the risk but does not remove it, because the code itself says "retryable".
- **Certainty:** reproduced (text).
- **Raw log:** Runtime_Contract_v0.md L136-145; Validation_Plan §1.2 rows "한도 종류와 공유" and "한도 해소".

### N5 · P3 · The checkpoint digest mitigation uses a credential the agent can read
- §2.1 L36 says the supervisor reports the checkpoint digest and sequence number to the coordinator, so the local file is not trusted. Under §2.1's own assumption, the agent can read the supervisor credential and so can report a forged digest too.
- This protects against accidental corruption, not against a malicious same-role agent. The contract should say so. The impact is limited to the agent's own role, except as covered by N1.
- **Certainty:** supported. **Raw log:** L34-37.

### N6 · P3 · One new statement is not backed by the committed evidence
- §4.1 L86: "`--setting-sources user`의 user 출처는 `CLAUDE_CONFIG_DIR/settings.json`". The help (L226-227) lists only the source names user/project/local and does not give a path.
- §3 L43 cites "검토 세션 확인" for the empty-`CLAUDE_CONFIG_DIR` logged-out result. That check came from my scratch log, which is not committed. Either commit a redacted equivalent or label it as an external observation to be repeated in 01-04; the 01-04 part is already stated.
- **Certainty:** reproduced (grep). **Raw log:** evidence/claude-help.txt L226-227.

### N7 · P3 · Residual and housekeeping items
- REPORT's header still says "run … **rev 1**", while result.json says `revision: 2` and the contract says "문서 revision 2".
- The development role applies worktree CLAUDE.md, including uncommitted and unreviewed edits. §1.2 treats repo knowledge as changed through PR and review. Consider loading it from the approved base SHA.
- The rule that the two roles do not communicate directly (network) from §1.2 is still absent.
- **Raw log:** REPORT.md L5; Runtime_Contract_v0.md L68.

## Summary

The correction is real for F3, F4, F5, F8 and F9, and for most of F6. CI on bf854bb succeeded in run 43124180490, and both local validators pass. Every option in the §4.1 argv exists in the committed `claude --help`. No secrets appear in the diff.

F1 is only partly resolved: the repo settings, hook and MCP path is closed by argv, but CLAUDE.md through `--add-dir` (N2) and user-scope and live-reload config (N3) remain.

The new §2.1 introduces a P1 (N1). The contract now explicitly accepts that a same-UID process can read the supervisor's Guardrail claim credential and the Guardrail model token, while §4.1 lets the Guardrail run checks of the development-authored tree. Together these give development code a designed path to issue Guardrail-named claims, which §1.2/P07 requires to be zero.

N4 is a P2: the §7 mapping marks auth/billing and usage-limit errors as retryable `TRANSIENT_FAILURE`.

To fix N1:
- Forbid executing the reviewed tree inside the Guardrail process, and source the check results from CI or a coordinator-owned runner.
- Or sandbox execution away from the credentials.
- Or record the gap as a blocking decision.

Also fix N2 through N4.
