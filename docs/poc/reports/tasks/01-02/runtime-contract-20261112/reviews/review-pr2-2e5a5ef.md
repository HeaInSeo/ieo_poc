VERDICT: reject
TARGET_SHA: 2e5a5efccf44fa2c996a4f98446e582a6235ac61

Independent Guardrail re-review of PR #2 (HeaInSeo/ieo_poc), task 01-02, run `runtime-contract-20261112`, contract revision 3.

- **Scope:** the full diff b12bc73..2e5a5ef. The correction under review is commit 2e5a5ef on top of bf854bb, which I rejected in review-pr2-bf854bb.md.
- **Method:** read-only. I worked in a scratch worktree at `scratchpad/rev-pr2c`, and raw logs are in `scratchpad/rlog3/`. Both are under /tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad/.
- **Not done:** no sessions, model calls, logins, GitHub posts or merges.

## Commands run (exit codes)

| # | Command | Exit | Raw log / note |
|---|---|---|---|
| 1 | `git fetch origin; git cat-file -t 2e5a5efc…` | 0 | object is a commit |
| 2 | `git worktree add <scratch>/rev-pr2c 2e5a5ef` | 0 | |
| 3 | `git log --oneline b12bc73..2e5a5ef`; `git diff --stat bf854bb 2e5a5ef`; `git diff bf854bb 2e5a5ef` | 0 | 3 commits. The correction changes REPORT.md, Runtime_Contract_v0.md and result.json; there is no evidence change |
| 4 | `git diff --quiet 7a39452 2e5a5ef -- evidence/claude-help.txt` | 0 | help evidence unchanged. It was earlier shown byte-identical to installed 2.1.322 (rlog/help.txt) |
| 5 | `grep -c` in claude-help.txt for `--tools`, `--disable-slash-commands`, `--allowedTools`, the `Bash(git *)` rule example, and `Read(` | 0 / `Read(` → 1 | rlog3/argv-check.txt. Every new argv option exists. No path-scoped `Read(...)` rule syntax appears in the help (see R4) |
| 6 | `sed`/`grep` of the plan for P07 and Guardrail execution duties; `Sprints_v0.json` 03-07/04-07/02-04.accept | 0 | quoted in R1 |
| 7 | `grep -rn 'QUOTA_WAIT\|작업을 보유한 대기\|사람 결정 대기'` in docs/poc | 0 | the vocabulary exists in plan §1.5 (L125, L136-137) and ProgressReport_v0.schema.json |
| 8 | `python scripts/validate_package.py` | 0 | rlog3/validate_package.txt |
| 9 | `python scripts/render_progress.py --check` | 0 | rlog3/render_check.txt |
| 10 | `git diff b12bc73 2e5a5ef \| grep -Ei 'email\|sk-ant\|oat01\|uuid'` | 1 | no matches |
| 11 | `gh run watch 43124804700 --exit-status`; `gh run view 43124804700 --json headSha,conclusion,jobs` | 0 / 0 | rlog3/ci-watch.txt. headSha 2e5a5efc…; conclusion success; both jobs succeeded |

## Prior findings: resolved or not

| Prior | Status at 2e5a5ef | Basis |
|---|---|---|
| N1 (P1): Guardrail runs reviewed code under its own credentials | **The specific path is closed.** The Guardrail never executes the reviewed tree; it has `--tools "Read,Grep,Glob"` only, with no Bash, Write or Edit. But the fix removes Guardrail execution capability that the plan requires, which is new problem R1 | §2.1 L37, §4.1 table and argv |
| N2 (P2): CLAUDE.md through `--add-dir` | **Resolved in the contract.** `--add-dir` is no longer used. The snapshot sits outside cwd and is read through path-scoped allow rules, with a fallback (a renamed-instruction bundle) and a stop condition. Whether a CLAUDE.md in the snapshot still gets loaded is a probe-G (01-04) question | §4.1 bullet 1 |
| N3 (P2): settings-only override, applied only before start | **Mostly resolved.** The whole `CLAUDE_CONFIG_DIR` is rebuilt from a read-only template, keeping only session records and credentials. Tampering during a run is impossible for the Guardrail, which has no write tools, and is accepted as within the development role's own scope. Probe G now covers CLAUDE.md, agents and skills. Residuals are in R3 | §4.1 bullet 2, §8.5 |
| N4 (P2): retryable `TRANSIENT_FAILURE` for limits and auth | **Resolved.** Adapter codes are now used only for failures of the call itself. Usage limits report through `status.provider_status_or_unknown=usage_limit` and are handled with `작업을 보유한 대기`/`QUOTA_WAIT` plus `next_check_at` and the 01-03 caps. Auth/billing errors are not retried and go to `사람 결정 대기`. A context limit is not treated as an error, and a short rate limit gets bounded backoff. This matches §1.2's "한도 종류와 공유"/"한도 해소" and the §1.5 state vocabulary (command 7) | §7 table |
| N5 (P3): checkpoint digest | **Resolved.** The development checkpoint is declared non-integrity-protected, and resume decisions are reconciled against GitHub branch/SHA and CI | §2.1 bullet 4 |
| N6 (P3): unsupported statements | **Resolved.** They are now labelled as assumptions or as uncommitted observations to be confirmed in 01-04 | §3, §4.1 |
| N7 (P3): residuals | **Resolved.** REPORT now says rev 3. The development role's CLAUDE.md is explained as working notes, with main as the approved knowledge. "직접 통신하지 않는다" is stated | REPORT L5, §2.1 bullet 3, §4.1 table |

## Findings

### R1 · P1 · The read-only Guardrail contradicts Guardrail execution duties the plan requires (P07, P20, P22, P24)
- **Violation condition:** 01-02.accept fixes each role's filesystem and execution scope for later tasks, and that scope has to let each role do what the plan assigns it. Rev 3 instead fixes the Guardrail as read-only (Read, Grep and Glob only). Its only access to execution is receiving results that CI or a coordinator-owned runner has already published, and it has no way to request execution of its own counterexample or check.
- **Reproduction** (Validation_Plan, command 6):
  - **L25 (role definition):** Guardrail "반례와 재현 가능한 판정을 제출한다".
  - **P07 procedure (S0 prerequisite, 04-07):** "검토자가 정상 CI에 드러나지 않는 고정 반례를 B에서 재현한다". The evidence must include "재현 입력·기대값과 실측값·…·원본 로그". By definition, CI does not reveal this counterexample, so receiving CI results cannot reproduce it.
  - **P20 (L430, L442):** "Guardrail은 … 검사를 재현한다"; "Guardrail이 셋업 후보의 실제 검사를 독립 실행한 뒤 … 새 SHA로 다시 실행한다".
  - **P22 (L544):** each role's execution space includes "쓰기 가능한 빌드·테스트 공간과 결과 출력".
  - **P24 (L643-644):** Guardrail "현재 후보의 필수 검사와 P23 정상·의도적 실패 대조를 실행한다", and "Guardrail은 새 커밋과 현재 설정에서 관련 검사를 다시 실행한다".
  - **The contract:**
    - §2.1 L37 says the Guardrail "코드를 읽기만 하고, 실행 결과는 CI 또는 코디네이터 소유 실행기가 … 발행한 결과를 … 받는다".
    - §4.1 gives it no Write tool, so it cannot even author a repro file.
    - The §1.2 citation ("완료와 필수 검사") describes the coordinator's judgment path. It is not a substitute for Guardrail-driven execution.
- **Expected:** keep the N1 property (no reviewed code runs where the Guardrail's model, claim or GitHub credentials are readable), and fix a Guardrail-driven execution path. For example:
  1. The Guardrail submits a repro request (counterexample test or patch, command, target SHA, profile) through the coordinator.
  2. A coordinator-owned, credential-free, isolated runner, with its own Pod or container, UID and no role Secrets, runs it against the fixed SHA. The P23 workflow-input case runs through GitHub Actions instead.
  3. Raw logs and exit codes come back to the Guardrail and into the evidence.
  4. The Guardrail gets a write-capable scratch area outside the snapshot for writing repro material, or the repro travels in the request.

  The contract should name this path and add a probe or acceptance hook for it: P07 "고정 반례 재현", with the runner's credential isolation tested.
- **Observed:** No such path exists, so the Guardrail role as fixed cannot satisfy P07 (an S0 prerequisite), P20 or P24. Adding execution later would reopen the N1 boundary outside this reviewed contract. This is a design contradiction, not something a 01-04 probe can settle.
- **Certainty:** supported (plan text against contract text).
- **Raw log:** Console_Coordinator_PoC_Validation_Plan.md L25, P07 procedure (L323), L430, L442, L544, L643-644; Runtime_Contract_v0.md §2.1 L37, §4.1 table and Guardrail argv.

### R2 · P2 · The development push credential is not phase-gated, although rev 3 now says the container is not a permission boundary
- **Violation condition:** §1.9 says that only after the coordinator moves the task to the correction phase "해당 브랜치 push를 허용한다". P07/04-07 injects a "교정 전 push" and records "push 거부/위반".
- **Reproduction:** §2 says the development role's GitHub credential is "작업 브랜치 push·PR 생성만". Rev 3 §2.1 bullet 2 adds that the development container's Claude permission rules "보안 경계가 아니다" and that its push credential is exposed to development code. A GitHub token by itself is not scoped to one branch or one phase. That needs rulesets or branch protection, or issuing and revoking the token per phase.
- **Expected:** say where the gate sits. For example: the coordinator issues a short-lived push credential only in the correction phase and revokes it on `REVIEW_PENDING`, plus rulesets limited to the work branch. Alternatively, state explicitly that §1.9 relies on detection (a new head invalidates the review assignment) together with server-side conditional merge.
- **Observed:** Neither is stated. The credential is static, branch scoping is asserted without a mechanism, and push during `REVIEW_PENDING` is unprevented and undocumented.
- **Certainty:** supported (document).
- **Raw log:** Runtime_Contract_v0.md §2 "GitHub 자격" row, §2.1 L38; Validation_Plan §1.9 L183, P07 "검토 중 변경" paragraph.

### R3 · P3 · Residual issues in the `CLAUDE_CONFIG_DIR` rebuild
- The rebuild preserves "자격 파일" in `CLAUDE_CONFIG_DIR`. §3 item 1 says each role receives credentials *only* through `/run/ieo/cred/`, so no credential files should exist on the RW volume. Either drop that exception or explain it.
- The preserved `projects/` tree may also hold runtime auto-memory. The `--bare` help lists "auto-memory" as on by default. For the development role, memory written there survives the rebuild and loads at the next start. That is within the development role's own scope, but probe G's "재개에 필요한 경로 목록" should also record what else under `projects/` gets loaded.
- **Certainty:** supported (help L41-47) / unconfirmed (exact paths).

### R4 · P3 · The Guardrail's path-scoped read rules are not in the evidence and do not deny credential paths
- The help shows rule syntax only for Bash (`Bash(git *)`); there is no `Read(<path>)` example (command 5). The contract correctly leaves effectiveness to probe G.
- The read deny list should also be fixed explicitly: `/run/ieo/cred/**`, `CLAUDE_CONFIG_DIR`, `/state/home`, `/etc/ieo`. Otherwise a prompt-injected Guardrail could copy its own token into review text that is persisted, and those reports go to a public repo.
- The read allowlist must also cover the approved spec/profile and the protected check results the Guardrail is supposed to read, and the contract does not locate them.
- Probe G checks for "Bash/쓰기/실행 시도 거부" but not for denial of reads outside the snapshot.
- **Certainty:** supported. **Raw log:** rlog3/argv-check.txt; Runtime_Contract_v0.md §4.1 and §8.5.

## Summary

Rev 3 is a substantive correction:
- N2 through N7 are resolved or are reduced to proper 01-04 probe items.
- The new §7 error table matches §1.2 and the §1.5 operational-state vocabulary.
- Every new argv option exists in the committed help.
- CI on 2e5a5ef succeeded (run 43124804700), both validators pass, and there are no secrets in the diff.

I am still rejecting it for one P1, R1. N1 was closed by making the Guardrail purely read-only and limiting it to consuming published CI or runner results. The plan, however, requires the Guardrail to reproduce counterexamples that CI does not reveal (P07, an S0 prerequisite) and to re-run checks independently (P20, P22, P24). This is a design contradiction, not a probe question.

The fix is narrow: keep "no reviewed code where Guardrail credentials are readable", and add a named, Guardrail-requested execution path on a credential-free, coordinator-owned isolated runner, with logs returned, plus the matching probe or acceptance hook. In the same pass, also address R2 (gating of the development push credential, against §1.9), R3 and R4.
