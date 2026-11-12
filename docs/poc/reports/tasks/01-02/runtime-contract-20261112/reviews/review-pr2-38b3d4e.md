VERDICT: approve
TARGET_SHA: 38b3d4e3d4e206da938cf74aae755909b2c52307

Independent Guardrail re-review of PR #2 (HeaInSeo/ieo_poc), task 01-02, run `runtime-contract-20261112`, contract revision 4.

- **Scope:** the full diff b12bc73..38b3d4e. The correction under review is commit 38b3d4e on top of 2e5a5ef, which I rejected in review-pr2-2e5a5ef.md.
- **Method:** read-only. I worked in a scratch worktree at `scratchpad/rev-pr2d`, and raw logs are in `scratchpad/rlog4/`. Both are under /tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/.
- **Not done:** no sessions, model calls, logins, GitHub posts or merges.

## Commands run (exit codes)

| # | Command | Exit | Raw log / note |
|---|---|---|---|
| 1 | `git fetch origin; git cat-file -t 38b3d4e3…` | 0 | object is a commit |
| 2 | `git worktree add <scratch>/rev-pr2d 38b3d4e` | 0 | |
| 3 | `git log --oneline b12bc73..38b3d4e`; `git diff --stat 2e5a5ef 38b3d4e`; `git diff 2e5a5ef 38b3d4e` | 0 | 4 commits. The correction touches only REPORT.md, Runtime_Contract_v0.md and result.json |
| 4 | `git diff --stat 7a39452 38b3d4e -- evidence/`; `git diff --stat 2e5a5ef 38b3d4e -- evidence/` | 0 / 0 | Since 7a39452 only codex-version.txt and commands.txt were added (in bf854bb), and there is no evidence change in rev 4. claude-help.txt is unchanged, and it was earlier shown byte-identical to installed 2.1.322 |
| 5 | python: print 01-08/02-06/05-03/03-07.accept and the task records in Sprints_v0.json | 0 | quoted in S4 |
| 6 | `grep -n '워크플로를 등록\|workflows' Validation_Plan.md` | 0 | L577 (P23: "개발 에이전트가 워크플로를 등록") |
| 7 | `grep -n '쓰기 도구가 없어\|읽기 전용 도구 밖\|02-06\|05-03\|필수 체크' Runtime_Contract_v0.md` | 0 | L37, L39, L96, L100 (see S5) |
| 8 | `python scripts/validate_package.py` | 0 | rlog4/validate_package.txt |
| 9 | `python scripts/render_progress.py --check` | 0 | rlog4/render_check.txt |
| 10 | `git diff b12bc73 38b3d4e \| grep -Ei 'email\|sk-ant\|oat01\|ghs_\|uuid'` | 1 | no matches |
| 11 | `gh run list --commit 38b3d4e…`; `gh run watch 43125310678 --exit-status`; `gh run view 43125310678 --json headSha,conclusion,jobs` | 0 / 0 / 0 | rlog4/ci-watch.txt. headSha 38b3d4e3…; conclusion success; both jobs succeeded |

## Prior findings: resolved or not

| Prior | Status at 38b3d4e | Basis |
|---|---|---|
| R1 (P1): read-only Guardrail cannot reproduce or re-run | **Resolved in substance.** The Guardrail still never executes reviewed code in its own container. It writes counterexamples and tests in `/state/guardrail-scratch/` and submits a repro request (SHA, bundle digest, command, expected value). A coordinator-owned, one-shot, credential-free runner with no network by default and resource caps runs it and returns raw logs, exit code and runner image digest to the Guardrail's input path. This covers P07's "고정 반례 재현", P20/P24 local re-execution and P22's writable build/test space, while keeping the N1 property. Work is assigned: API to 01-08, runner to 02-06, verification to P07 (04-07), with the probe-G entry "재현 요청이 격리 실행기(자격 0)에서 돌고 원본 로그가 돌아오는지". The GitHub-side duties are not covered (S3) | §2.1 L37, §4.1, §8.5 |
| R2 (P2): push not limited to the right phase | **Mostly resolved.** The push credential is a short-lived GitHub App installation token (one repo, ≤1 hour), issued only in the `개발`/`교정` phases and revoked on `REVIEW_PENDING`. main is protected. Other branches are handled by detection plus invalidation, re-assignment and conditional merge on the approved head (§1.9). Verification is assigned to 05-03/P07. Two permission gaps remain: S1 and S2 | §2.1 L39 |
| R3 (P3): credential files and auto-memory in `CLAUDE_CONFIG_DIR` | **Resolved.** No credential files are allowed in `CLAUDE_CONFIG_DIR`, and any found are deleted and logged. Auto-memory is not preserved; knowledge lives only in repo files. The auto-memory location and how to disable it are marked as assumptions for probe G | §4.1 L96 |
| R4 (P3): read rules, credential paths, input locations | **Resolved.** `/run/ieo`, `/etc/ieo`, `/state/claude`, `/state/home` and `/proc` are now on an explicit deny list. The read allowlist now includes `/state/inputs/<assignment_id>/` (spec, profile, protected check results, repro output). Path-rule syntax and effect are marked as assumptions, with probe G and fallbacks (`--restricted`, mount namespace). Claims are scanned for credential fingerprints before they are stored or published | §4.1 argv and bullets 3-4, §8.5 |

Contradictions with §1.2, §1.7, §1.9, U3 and 01-02.accept: none at P0 or P1. The runner is coordinator-owned and is not a writable volume shared between the two role Pods. No admin rights go to either role. The Guardrail's inputs remain the approved spec, profile, fixed SHA and protected results. The two roles do not communicate directly; the repro path runs through the coordinator.

## Findings (none P0 or P1)

### S1 · P2 · The development push token's `contents:write` also allows merging, but the main protection listed does not require a verdict that development cannot produce
- **Violation condition:** §1.9 says merging happens only through the coordinator's server-side conditional merge on the approved head SHA. P07 requires 0 wrong passes of B.
- **Reproduction:** §2.1 L39: the token has "contents/pull_requests 쓰기", and the protection is "main은 보호 설정(PR 필수·필수 체크·관리자 포함)". On GitHub, `contents:write` lets the holder merge a PR through the API once the protection conditions are met. If "필수 체크" means CI only, development code holding the token can merge its own PR after CI passes and before the Guardrail review.
- **Expected:** main protection requires something only the coordinator can produce. For example: a required status check published by the coordinator's verdict path; or a required approving review from a non-development identity; or a ruleset that limits merging to the coordinator App. The 05-03 or P07 case should include "development token attempts merge before verdict".
- **Observed:** Not specified. Verification is assigned to 05-03/P07, and the fix is a settings-level change, so this is not a reject.
- **Certainty:** supported (document plus GitHub permission semantics; not executed).
- **Raw log:** Runtime_Contract_v0.md L39.

### S2 · P2 · The phase token has no `workflows` permission, but the plan has development register workflows
- **Violation condition:** P23 (L577): "개발 에이전트가 워크플로를 등록". P20 has development add the first CI setup.
- **Reproduction:** §2.1 L39 grants only "contents/pull_requests 쓰기". GitHub App tokens need the `workflows` permission to create or update files under `.github/workflows/`; without it, the push is rejected.
- **Expected:** add `workflows: write` in the phases that need it, noting the risk that development can change CI. 05-03 and P23 already test workflow and Actions-permission variants. Alternatively, state that workflow files go through a separate approved path.
- **Observed:** Missing. P20/P23 setup would fail at push time.
- **Certainty:** supported (GitHub App permission model; not executed).
- **Raw log:** Runtime_Contract_v0.md L39; Validation_Plan L577.

### S3 · P2 · The Guardrail's GitHub-side duties are not covered by the new repro path
- **Violation condition:** P23 (L586) has the Guardrail run an intentionally failing input "같은 GitHub Actions 이벤트, 필수 작업, 명령 및 환경으로". P24 (L643) has the Guardrail "GitHub 설정을 다시 조회". The rev 4 Guardrail has no Bash, network or GitHub write access, and the isolated runner has network off by default.
- **Reproduction:** §2.1 L37 and the §4.1 Guardrail tools (Read, Grep, Glob, plus Write/Edit in scratch only). There is no GitHub read MCP or input for settings, and no way to trigger GitHub Actions.
- **Expected:** name two extensions of the same coordinator-mediated path: (a) the coordinator fetches the actual GitHub settings and drops them into `/state/inputs/<assignment_id>/`, or the role gets a read-only GitHub MCP from `/etc/ieo/role/mcp.json`; (b) a repro request with target "GHA", which the coordinator executes on a scratch branch or through `workflow_dispatch` under its own identity, returning the run ID and conclusion. Assign this to 05-03/P23.
- **Observed:** Not named. These are later experiments and the R1 pattern extends to them naturally, so this is not a reject.
- **Certainty:** supported (document).
- **Raw log:** Runtime_Contract_v0.md L37, §4.1 Guardrail argv; Validation_Plan L586, L643.

### S4 · P3 · The runner is assigned to a task whose acceptance does not mention it, and the network-off default is unaddressed
- "실행기 구현은 02-06". 02-06.accept covers review, correction, REVIEW_PENDING and merge wiring, and P07 isolation. It does not mention an execution runner. 03-07 ("승인 실행") is a closer match.
- The repro-request API is added to 01-08's scope, but 01-08.accept does not include it.
- Record these as explicit scope additions in the ledger or acceptance, so 01-08 and 02-06 are not reviewed without them.
- The network-off default needs a dependency strategy, such as a pre-fetched or vendored cache or an allowlisted registry mirror. Otherwise Rust and Go builds fail inside the runner.
- **Raw log:** command 5.

### S5 · P3 · Stale wording after the Guardrail gained Write/Edit
- L96 still says "실행 중 변조는 Guardrail은 쓰기 도구가 없어 불가". The Guardrail now has Write/Edit. Tampering is impossible only if the path-scoped write rule holds, and that rule is itself a probe-G assumption.
- L100 still says the pass criterion is "읽기 전용 도구 밖의 실행". It should read "the fixed tool set (Read, Grep, Glob, Write/Edit in scratch only)".
- **Raw log:** command 7.

## Summary

Rev 4 resolves R1 through R4 in substance:
- The Guardrail does not execute reviewed code in its own container.
- Independent reproduction and re-execution go through a coordinator-owned, credential-free isolated runner, with the API, implementation and verification assigned to 01-08, 02-06 and P07.
- Push is limited to the right phases by a short-lived App token, protected main, and detection plus conditional merge, verified in 05-03/P07.
- `CLAUDE_CONFIG_DIR` holds no credentials or auto-memory.
- Credential paths are denied explicitly, and claims are scanned for secrets before being stored or published.

I found no new contradiction with §1.2, §1.7, §1.9, U3 or 01-02.accept at P0 or P1. CI on 38b3d4e succeeded (run 43125310678), both validators pass, the help evidence is unchanged and there are no secrets.

**Approve.** Fix the remaining items before or in their assigned tasks:
- S1: main protection must require a verdict that development cannot produce, because `contents:write` can merge.
- S2: add `workflows` permission for P20/P23.
- S3: cover the Guardrail's GitHub-side duties for P23/P24.
- S4: anchor the runner and API scope additions in 02-06 or 03-07 and in 01-08.
- S5: correct the stale wording.
