VERDICT: reject
TARGET_SHA: 66842d134114cc4468a262c325b3f372baf63544

Reviewer: independent Guardrail session (did not author the change). Base 0858fb0b52a38930d1f64377b396e3748cfa5b3e.

NOTE: while this review ran, PR #1's head moved to 9b372bdc4556f419a115b7f82fc874e1ebb37a58
("infra: record public visibility switch and main branch protection"). This verdict covers ONLY 66842d1.
It is not an approval of 9b372bd, and 9b372bd needs its own review at that SHA.

## Commands run (exit codes)

S=/tmp/claude-1001/-opt-rust-src-github-com-HeaInSeo-ieo-poc/48619655-6edf-4b63-83bc-01e6dfc724e5/scratchpad

| # | Command | Exit |
|---|---|---|
| 1 | `git diff --stat 0858fb0..66842d1`; `git merge-base 0858fb0 66842d1` (= 0858fb0) | 0 |
| 2 | `git diff 0858fb0..66842d1` (all 16 files read) | 0 |
| 3 | `git worktree add $S/rev-pr1 66842d1` | 0 |
| 4 | (docs/poc) `.venv-poc/bin/python scripts/validate_package.py` > $S/rev-validate.log (`"schema_errors": 0`) | 0 |
| 5 | (docs/poc) `.venv-poc/bin/python scripts/render_progress.py --check` > $S/rev-render.log (`{"ok": true, "mode": "check", ...}`) | 0 |
| 6 | venv package list: attrs 26.1.0, jsonschema 4.26.0, jsonschema-specifications 2025.9.1, referencing 0.37.0, rpds-py 2026.9.1, typing_extensions 4.16.0 (all match tools/requirements-poc.txt); Python 3.12.13 | 0 |
| 7 | (worktree) `rustup show active-toolchain` -> `1.99.1-x86_64-unknown-linux-gnu (overridden by .../rev-pr1/rust-toolchain.toml)` | 0 |
| 8 | (worktree) `rustc -V \| tee /dev/stderr \| grep -q '^rustc 1\.99\.1 '` | 0 |
| 9 | `cargo fmt --version` -> rustfmt 1.10.0-stable (58cf8f9c35 2026-10-13); `cargo clippy --version` -> clippy 0.1.99 (58cf8f9c35 2026-10-13) | 0 |
| 10 | `rustup toolchain list` -> only `1.99.1-x86_64-unknown-linux-gnu (active, default)`; `rustup component list --installed` -> cargo, clippy, rust-std, rustc, rustfmt | 0 |
| 11 | `rustup show home` -> /home/heain/.rustup; `ls -la ~/.rustup ~/.cargo` -> symlinks to /opt/rust/rustup, /opt/rust/cargo; `env \| grep RUSTUP_HOME\|CARGO_HOME` -> (none set) | 0 / grep 1 |
| 12 | `df -h /home /` -> rl-home 1.6G 96%, rl-root 15G 79%; `du -sh` -> 617M /opt/rust/rustup, 21M /opt/rust/cargo | 0 |
| 13 | Negative gate test: temp dir with `channel = "1.98.0"`, `RUSTUP_AUTO_INSTALL=0 rustup show active-toolchain` -> "error: toolchain '1.98.0-...' is not installed"; `bash -e -c "rustc -V \| tee /dev/stderr \| grep -q '^rustc 1\.99\.1 '"` | gate=1 (fails as expected) |
| 14 | `gh pr view 1 --json headRefOid,baseRefOid,state,statusCheckRollup` (at review start head = 66842d1; both checks SUCCESS, run 43121774966) | 0 |
| 15 | `gh run view 43121774966 --log` (rust job: auto-installed 1.99.1, printed rustc 1.99.1 / rustfmt 1.10.0 / clippy 0.1.99); `gh run view 43121774966 --json headSha,event` -> 66842d1 pull_request | 0 |
| 16 | `git grep -nIiE 'gh[pousr]_...\|github_pat_\|token\|password\|secret\|authorization' 66842d1 -- docs/poc/reports/workspace .github tools rust-toolchain.toml` -> only `Token: [REDACTED]` and token scopes in prep-20261112 (pre-existing) | 0 |
| 17 | `git ls-remote origin main` -> 0858fb0b52a38930d1f64377b396e3748cfa5b3e refs/heads/main | 0 |
| 18 | `gh api repos/HeaInSeo/ieo_poc` -> `"private":false, "visibility":"public"` (saved $S/rev-repo-live.json) | 0 |
| 19 | `gh repo view HeaInSeo/ieo_poc --json visibility,isPrivate` -> `PUBLIC`, `isPrivate:false` | 0 |
| 20 | `gh api repos/HeaInSeo/ieo_poc/branches/main/protection` -> 200, required checks (both CI jobs, strict), enforce_admins true (saved $S/rev-protection-live.json) | 0 |
| 21 | `gh api repos/HeaInSeo/ieo_poc/events` -> PublicEvent created_at 2026-11-11T23:40:07Z by HeaInSeo (saved $S/rev-public-event.json) | 0 |
| 22 | `git log -1 --format='%ci' 66842d1` -> 2026-11-12 08:41:38 +0900 (=23:41:38Z); PR createdAt 2026-11-11T23:41:41Z | 0 |
| 23 | `git worktree remove $S/rev-pr1` | 0 |

## Findings

### F1 (P1) - github-repo REPORT.md and PR body state PRIVATE / branch protection unavailable, but the repo was already PUBLIC before the commit and PR existed
- Violation condition: a committed record's factual claims must match observable state at the time of the record (DEV_AGENT_HANDOFF: report actual observations; Q1 of this review). At 66842d1 `docs/poc/reports/workspace/github-repo-20261112/REPORT.md` says `PRIVATE`, "브랜치 보호·rulesets **사용 불가**: HTTP 403", and "**한계**: 비공개+Free 계정이라 필수 체크·승인 강제·직접 push 차단을 서버가 강제하지 못한다". The PR body repeats "비공개 Free: 브랜치 보호/rulesets 403".
- Reproduction: `gh api repos/HeaInSeo/ieo_poc/events -q '.[]|select(.type=="PublicEvent")|.created_at'` -> `2026-11-11T23:40:07Z`; `git log -1 --format=%ci 66842d1` -> `2026-11-12 08:41:38 +0900` (23:41:38Z); `gh pr view 1 --json createdAt` -> `23:41:41Z`; `gh api repos/HeaInSeo/ieo_poc -q .visibility` -> `public`; `gh api repos/HeaInSeo/ieo_poc/branches/main/protection` -> HTTP 200 with required checks and enforce_admins.
- Expected: at the time 66842d1 was committed and the PR opened, the record reflects the actual visibility (or explicitly states it is a pre-switch snapshot), and the "server cannot enforce" limitation is not asserted as current.
- Observed: visibility switched to public ~91 s before the commit; the committed record and PR body still claim PRIVATE and unenforceable protection. The 403 evidence files are genuine but are pre-switch snapshots and REPORT does not say so. The "한계" section (no server enforcement, user decision needed for 05-01~05-03 among "Pro / 공개 전환 / 별도 repo") is stale: one of those options had already been executed.
- Severity: P1. Certainty: reproduced.
- Raw log: $S/rev-public-event.json, $S/rev-repo-live.json, $S/rev-protection-live.json (S as above). Untracked files in the main working tree at review time (`evidence/visibility-public.json` mtime 08:42:32 KST, `branch-protection-set.json` 08:42:56) and the later head 9b372bd show the author recorded the switch only after this SHA.

### F2 (P1, needs user confirmation) - Repository visibility changed from private to public; no user decision recorded at this SHA
- Violation condition: the user's instruction for this PR was "GitHub repo created private". Changing visibility (publishing the full history, including absolute host paths, username `heain`, gh token scopes in prep-20261112 evidence, and author email in commits) is an external, hard-to-reverse effect. At 66842d1 the only mention of going public is as an open **user decision** ("사용자 결정 대상 ... 공개 전환"). No evidence in this SHA that the user approved it.
- Reproduction: same commands as F1 (rows 18-21).
- Expected: either the repo stays private, or a recorded user decision (date, wording) authorizing the public switch is cited.
- Observed: repo public since 23:40:07Z; 66842d1 frames public switch as pending a user decision. I cannot see the conversation, so whether the user approved it is unconfirmed. The caller should confirm with the user directly; if not authorized, this is an undisclosed deviation (Q5).
- Severity: P1 (undisclosed external effect contradicting the stated instruction, if not authorized). Certainty: supported (visibility change reproduced; lack of authorization unconfirmed).
- Raw log: $S/rev-public-event.json; REPORT.md line 19 at 66842d1.

### F3 (P3) - "RUSTUP_HOME=/opt/rust/rustup, CARGO_HOME=/opt/rust/cargo" wording vs evidence
- Condition: REPORT says the homes are those env values; versions.txt shows `rustup home: /home/heain/.rustup`, and no RUSTUP_HOME/CARGO_HOME is set in the environment now (row 11). Storage really is /opt/rust via symlinks (verified), and install log shows cargo bin `/opt/rust/cargo/bin`, so CARGO_HOME was set at install time.
- Expected: say "env vars set only for the install; day-to-day resolution is via ~/.rustup, ~/.cargo symlinks". Observed: reads as if env vars are the persistent configuration. Not misleading about disk location. Severity P3, certainty reproduced. Raw: versions.txt line 5; row 11.

### F4 (P3) - versions.txt captured before the pin; post-pin component list not in evidence
- Condition: versions.txt lists `stable-x86_64-unknown-linux-gnu (active, default)` and components of stable; the REPORT's "설치 구성요소" row is attributed to the final 1.99.1 state. pin-1.99.1.txt has versions but no `rustup component list --installed`.
- Expected/Observed: I reproduced live (row 10): 1.99.1 has exactly cargo, clippy, rust-std, rustc, rustfmt, and stable is removed. So the claim is true but its cited evidence is the pre-pin state. Also pin output shows `default toolchain set to 1.99.1` (default changed from stable), only implied in REPORT. P3, reproduced.

### F5 (P3) - CI toolchain step relies on deprecated auto-install; gate is still meaningful
- `rustup show active-toolchain || rustup toolchain install`: in CI run 43121774966, the first command auto-installed 1.99.1 and warned "auto-installation is deprecated ... use `rustup install` instead", so the fallback never ran. Locally with auto-install off, `show active-toolchain` exits nonzero ("not installed"), so the `||` fallback is correct. If both fail, the step fails. The real gate is `rustc -V | tee /dev/stderr | grep -q '^rustc 1\.99\.1 '` under default `bash -e`: grep is the last pipeline element, so a wrong or missing rustc fails the step. Verified that it fails (row 13). `cargo fmt/clippy --version` fail if the components are missing, but their versions are not asserted. No `|| true`, `continue-on-error`, or `if:` conditions. The docs job's validators exit nonzero on failure (CI SUCCESS matches local exit 0).
- Suggestion: use plain `rustup toolchain install` (no-arg form reads rust-toolchain.toml) to avoid the deprecated path; optionally assert rustfmt/clippy versions. P3, reproduced. Raw: run 43121774966 log lines 23:41:52-23:42:01.

### F6 (P3) - Evidence hygiene: absolute scratchpad path; rustup-init hash not checked against upstream; odd cc-crate noise in install log
- rustup-init.sha256.txt records a session scratchpad path (`/tmp/claude-1001/.../rustup-init.sh`), and pin-1.99.1.txt records `/opt/rust/src/...` paths. These are evidence content, not report links, so they do not violate the relative-link rule. Note the repo is now public (F2).
- The SHA-256 is only recorded, not compared against an official checksum. REPORT does not claim verification, so not an overstatement.
- rustup-install.log contains `cargo:rerun-if-env-changed=...` / `CC = None` lines after the installer output. These are unexplained but harmless.
- Actions are tag-pinned (`@v4`/`@v5`), not SHA-pinned. P3, reproduced by reading the diff.

### Checked and OK
- Versions in the rust REPORT table (rustup 1.29.3, rustc 1.99.1 with commit 58cf8f9c35b3..., LLVM 23.1.1, cargo 1.99.1 5f94df478, rustfmt 1.10.0, clippy 0.1.99) match versions.txt/pin-1.99.1.txt and live output.
- Disk: before 1.6G/96% and 15G/78%; after 1.6G/96% and 15G/79%. Usage 617M+21M=638M. All match the evidence and are re-observed live.
- Install flags (`-y --no-modify-path --profile minimal --default-toolchain stable -c rustfmt -c clippy`) match the user instruction ("profile set to minimal", "This has not been done automatically" in the log). The relocation to /opt/rust because /home is 96% full is disclosed.
- rust-toolchain.toml pins 1.99.1, minimal, rustfmt and clippy.
- tools/requirements-poc.txt matches the venv exactly.
- github-repo evidence: repo-settings-after.json (merge-commit only, delete_branch_on_merge) and actions-permissions.json match the REPORT. The 403 bodies match the quoted message. `refs/heads/main` = 0858fb0 matches the "history preserved" claim.
- No tokens or secrets in the new files. The only token-related hit is the pre-existing `Token: [REDACTED]` line.
- validate_package.py and render_progress.py --check both exit 0 at 66842d1.
- CI ran at headSha 66842d1, and both jobs succeeded.

## Required to approve
1. Make the github-repo REPORT and the PR body match the actual visibility and protection state, or label the 403 records as pre-switch snapshots with a timestamp (F1). This needs a new head SHA and a new review.
2. Cite the user's decision that authorized making the repo public, or revert the repo to private (F2).
