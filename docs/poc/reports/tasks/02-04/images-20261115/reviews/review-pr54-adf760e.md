# Review of PR #54, head adf760e (review 1 of at most 4)

- **PR:** HeaInSeo/ieo_poc #54, "02-04: coordinator PID 1, repo-scope role control, role images, carry-over triage"
- **Head reviewed:** adf760e19539f3b471931c69cd9e9b50ea979377 (confirmed with `gh pr view 54 --json headRefOid`). Base: main c5e9af0. Code commit: ba33f19. adf760e changes only docs; `git diff ba33f19 adf760e -- crates tools contracts` is empty.
- **CI at review time:** docs validation and toolchain pin passed. "rust fmt clippy test" was still IN_PROGRESS.
- **Reviewer:** independent review agent (Claude Code). No model calls. Did not touch the persistent DB, the coordinator, /opt/rust/ieo_poc_runs or the secrets.
- **Verdict: CHANGES REQUESTED.** There are 2 blocking findings: B1 (repo scope fails open) and B2 (carry-over triage drops items and reassigns owners without saying so). Everything else is non-blocking.

## What I ran

All runs were in worktree `<scratchpad>/rv54a` with its own target directory. Both were removed afterwards.

- **Format and lint:**
  - `cargo fmt --all -- --check`: OK.
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`: OK.
- **Tests:** `cargo test --locked -p ieo-coord -p ieo-supervisor` with `IEO_TEST_DB_URL` set to the test DB on 55432. All passed, 161 tests:

  | Test binary | Passed |
  | --- | --- |
  | coordinator | 66 |
  | roles | 3 |
  | shutdown | 2 |
  | caller | 23 |
  | run_guard | 27 |
  | serve | 18 |
  | supervisor | 20 |
  | lib | 1 + 1 |

  The disk filled up during the first build, so I re-ran with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`. Same code, no debuginfo.
- **Evidence log arithmetic:** `cargo-test.log` sums to 294 passed and 0 failed, with 0 SKIP lines. The header shows commit ba33f19 and a clean tree.
- **`build-images.sh` refusal paths:** run from the worktree.
  - `HEAD~1` → "HEAD is not HEAD~1", exit 1.
  - An untracked file → "the worktree has local changes", exit 1.
  - I did not do a full rebuild because only about 1 GB of disk was free.
- **`images-fake.sh` re-run:** run against the existing `localhost/ieo-poc/*:ba33f19` images. Before running, I checked that nothing was using the `ieo-img-*` names, the `ieo-img-net` network or ports 58231-58232. The results match `evidence/images-fake.log`:
  - Label hashes equal the files in the images.
  - PID 1 is ieo-coord at UID 20000. An unauthenticated request gets 401. The DB password appears 0 times in inspect, argv and environ. 15 tables.
  - Status goes through the coordinator. On repo stop, development returns 200 and Guardrail 202, both with checkpoints saved. The response has `safe_shutdown_confirmed` false with 4 `not_tracked` reasons.
  - The Guardrail run ended `Requested` with 0 leftover processes.
  - SIGTERM: all three containers stopped in 0.3–0.4 s with exit 0.
  - Afterwards there were 0 leftover containers, networks or secrets, and the output contained no tokens or passwords. I removed the scratch directory.
- **Mutation tests:** each run under `timeout 900`.

| # | Mutation | Result |
| --- | --- | --- |
| M1 | signal handler not registered | killed (`sigterm_stops_the_coordinator_with_exit_zero`) |
| M2 | startup clash check disabled | killed (`a_binding_on_an_imported_test_id_stops_the_start`) |
| M3 | `roles:control` scope check removed | killed (coordinator `repo_scope_…`) |
| M4 | repo restriction removed | killed (coordinator `repo_scope_…`) |
| M5 | unreachable role is waited for (no early return) | **survived** (only costs time; no test bounds it) |
| M6 | `not_tracked` accepted as safe | killed (`repo_stop_sends_a_role_stop…`) |
| M7 | supervisor reports `active_children: 0` instead of `not_tracked` | **survived** (no cargo test reads `safe_shutdown` from the real supervisor; only `images-fake` covers it) |
| M8 | non-object charge check removed | killed (`charge_compares_every_part…`) |
| M9 | supervisor `checkpoint_saved` always true | **survived** |
| M10 | repo stop sends no stop | killed |
| M11 | stop HTTP status ignored (all treated as 200) | **survived**. There is no such check; see M-1 below |

## Findings

### B1 (blocking, medium): an empty `repo_ids` grants repo-scope control over every repo

- **Where:** `crates/ieo-coord/src/api.rs` (new route block), the check `if !who.repos.is_empty() && !who.repos.contains(*repo)`.
- **What happens:** A service or user credential with `roles:control` and no `repo_ids` (or `[]`) passes the check. It can read status and **stop both roles of every configured repo**.
- **Why this is wrong:** Everywhere else in the coordinator (`Caller::in_repo`, used at 9 sites, and `ieo_report` `Principal::Client`), an empty `repos` set means access to no repo. `Credentials::from_json` accepts service and user principals without `repo_ids`. So this route is the only fail-open interpretation, and it sits on the most disruptive endpoint added so far.
- **Claims that do not match:** PROGRESS §1 says "자격의 repo 안에서만" ("only within the credential's repos"). The OpenAPI text says "`roles:control` for `repo_id`".
- **Fix:**
  - Use `if !who.in_repo(repo) { 403 }`.
  - Add a coordinator test with a `roles:control` principal that has no `repo_ids` and expects 403. The current M4 test only covers a credential that does list repos.

### B2 (blocking, process): the carry-over triage drops items and silently changes owners and timing

The user instruction (2026-11-15, item 5) asks to close resolved items with evidence and to list **the rest** with owner, timing and impact. I compared PROGRESS §4 and the new `state.json` `unresolved` against the old `unresolved` on main.

**(a) Old items that appear in neither the "closed" nor the "remaining" table:**
- **Contract §7 status fields.** The old wording was "PROGRESS §1 한계 … §7 확장 코드·없는 상태 항목". The supervisor status still has no `quota_group_id`, `provider_status_or_unknown` or `last_actual_action_at`. Acceptance names adapter `status`, so this has a plausible effect on 02-04 acceptance and must be listed.
- **The stop grace change.** The old wording was "stop grace 10→6초 변경(진행 기록 2의 값은 고치지 않음)".
- **A PROGRESS §2 note from PR #40 review 23fee80.** The old wording was "PROGRESS §2 마지막 항목에 charge 미계측 명시". The closed row covers Y1/Y4/Y7 and the non-object case, not this item.
- **The first half of f73ac13 P3.** The old wording was "projects/*/memory 빈 디렉터리의 digest 처리". Only the `subagents` half is closed.
- **One item from RESULTS §5.** The old wording was "Read의 빈 마지막 줄 표시는 한 번의 관측".
- **Two items from supervisor-core PROGRESS §2·§3.** The old wording was "기동과 잠금 해제 직렬화, 디버그 로그 끔".
- **A preserved-evidence note.** The old wording was "첫 SHA256SUMS는 자기 해시가 낡음".
- **The D3 answer expectation.** It was not met. It is still in the new `state.json`, but missing from the §4 "남음" table. Put it in the table so that the two places agree.

**(b) Owner and timing changes presented as "no impact":**
- **The 02-03 carried items:**
  - Enforcing the `elapsed_seconds`/`total_tokens` limits.
  - The real GitHub evidence collector, including F4 and N5.

  The accepted 02-03 REPORT (lines 104/107/110/113) assigned these to 02-04. Review pr26-84a123a then fixed their timing as **"02-04 수용 전"** ("before 02-04 acceptance"). The PR moves them to 02-05/EARLY-01 and says the 02-04 acceptance text does not mention them. That may be the right call: "한도 검증은 P05-Q로 분리" supports it for the limits. But it changes an owner and deadline that an accepted report and review set. The table should say so explicitly ("이전 기록: 02-04 수용 전 → 제안: 02-05, 수용 검토/사용자 확인 필요"), not present it as settled.
- **F3:** the owner changes from "02-04·02-07" to 02-07 only, and should be called out the same way.

**Fix:**
- Add the missing rows, each with owner, timing and impact.
- Mark the reassignments as proposals that the acceptance review must confirm.
- Regenerate the ledger snapshot.

### M-1 (medium, non-blocking): repo-scope stop is sequential and ignores the stop response status

- **Where:** `crates/ieo-coord/src/roles.rs` `stop()`.
- **Sequential stop:**
  - The code sends development's stop, waits until development stops (up to 30 s), and only then sends Guardrail's stop.
  - Scenario: both roles are mid-run, and development takes its full guard grace. Guardrail keeps running, and keeps spending budget, for that whole time after the operator asked to stop the repo. The evidence did not show this because development was idle.
  - Fix: send both stops first, then poll both.
- **Stop status not checked:**
  - The response status of `POST /v0/role/stop` is never checked (M11). A 401, 404, 409 or 5xx still counts as "stopped" if the later status call shows `running: null` and the checkpoint saved.
  - Today `not_tracked` keeps the result unconfirmed anyway, but once tracking lands this becomes fail-open.
  - A token mismatch also makes the status poll spin for the full 30 s, because it gets 401 every time. That holds one coordinator worker (4 by default) for up to about 2 × (10 + 30 + 10) s.
  - Fix: if the stop status is not 200 or 202, push a reason and skip the wait.
- **Shutdown interaction:** with these worst-case durations, a repo stop in flight during a coordinator SIGTERM outlives podman's `--stop-timeout 15`. The coordinator is then SIGKILLed mid-request. Worth one line in PROGRESS.

### M-2 (medium, non-blocking): the supervisor's `safe_shutdown` block has no unit test, and one value fails open

- **Where:** `crates/ieo-supervisor/src/serve.rs` `status()`.
- **No test:** M7 (`active_children: 0`) and M9 (`checkpoint_saved: true` always) survive `cargo test -p ieo-supervisor`. The "never 0" property is the PR's main safety claim, and only the podman script covers it.
- **Fail-open value:** `checkpoint_saved` is true when `phase` is None. That covers no checkpoint, but also a checkpoint file that exists and fails to parse. A corrupt checkpoint should be "not saved".
- **Contradicting record (existing code):** `write_checkpoint` still records `active_children: []` and `pending_external_requests: []` in the checkpoint. That contradicts the new `not_tracked` report: a checkpoint consumer would read "0".
- **Fix:**
  - Add a serve test that asserts `not_tracked` and `checkpoint_saved` for these states: no checkpoint, `takeover`, `error`, `after_run`, and an unreadable checkpoint.
  - Treat a present-but-unparseable checkpoint as not saved.
  - Either note the checkpoint fields as a known limitation, or write `"not_tracked"` there too.

### L-1 (low): test and claim mismatches

- **Agent refusal is claimed but not tested.**
  - The `repo_scope_role_control…` comment says "without the scope, as an agent, or for another repo". PROGRESS §1 says the coordinator API test covers "범위·에이전트·다른 repo 거부" (scope, agent and other-repo refusal).
  - In fact the test uses `uma` (a user without the scope). No agent credential is exercised.
  - The refusal does hold structurally: agents cannot load `roles:control`, and `need_client_scope` rejects `Kind::Agent`. Add one agent call or reword the claim.
- **"A request in progress is answered first" is not tested.** The `tests/shutdown.rs` header and PROGRESS §1 claim it, but the test sends SIGTERM only after its request has finished. The code path (`unblock` once per worker after `serve_one`) looks right, but it is untested.
- **M5:** nothing bounds the "unreachable is reported at once, not waited for" behaviour. A timing assertion would do it, for example that `stop` with a 5 s wait on an unreachable role returns in under 2 s.

### L-2 (low): images and scripts

- **Stale comment:** `tools/containers/Containerfile.coordinator` says the DB URL comes from "IEO_DB_URL, given by an env file". The PR deliberately moved to `IEO_DB_URL_FILE` (a mounted secret) because the env form leaked into `podman inspect`. Update the comment.
- **CLI not pinned:** `build-images.sh` records the Claude CLI hash but does not compare it with the expected `bfb3703a…`. A different `CLAUDE_BIN` would be built and labelled without any warning. Consider a `CLAUDE_SHA256` check.
- **No cleanup trap in `images-fake.sh`:** the script has no `trap cleanup EXIT`. If it is interrupted, the containers, the network and the DB secret stay until the next run. The scratch directory also keeps the adapter and operator tokens and `coord-etc/adapter.token` at mode 0644. These are test tokens, but delete them at the end.
- **Weak password check:** the password check greps only for the literal `password=`. Grepping for `$PGPW` itself would be a stronger test.

### L-3 (low): PROGRESS §5 labels

"충족(모의)" ("met, mock") for coordinator PID 1 and role images is honest: fake Claude and a throwaway DB, and the build came from the PR commit rather than the merge commit. "부분 충족" ("partly met") for repo-scope stop is right. Two things should be stated next to these rows:
- The coordinator-side start and reconnect are deferred to 02-05.
- The §7 status fields are still missing (see B2a).

Without them, the acceptance reader might take "어댑터 start/reconnect/stop/status 연결" ("adapter start/reconnect/stop/status connected") as fully met through the coordinator. 11/73 and "진행" (in progress) are unchanged, which is correct.

## Verified as correct

- **Security:**
  - Adapter tokens are read per call, sent only in the Authorization header, and never logged or returned. Errors report only `e.kind()`.
  - The DB URL parse error does not echo the value.
  - Role URLs come only from the operator-owned `--roles` file, and the repo path segment only indexes that map. There is no request-controlled URL, so no SSRF.
  - The per-call timeout is 10 s.
- **Startup check:** it queries `notion_imports.test_id`, refuses a clash with exit 2, and is tested both ways (M2 killed).
- **SIGTERM handling:** exits 0 and stops listening (M1 killed). It is confirmed in the container by my re-run.
- **`charge_reported`:** a non-object value is unmeasured with a reason. Y1, Y4 and Y7 are pinned (M8 killed).
- **Closed items checked against the repo:**
  - PR #47 R1–R6 are in real-sessions PROGRESS §2.
  - D5 `/compact` resume is measured (RESULTS.md D5).
  - §10 items 1–3 are real and items 4–5 mock (RESULTS.md table).
  - Credentials are passed through the env (`env::build`) and the argv tests exist.
- **Images:** built from one clean commit. The labels match the binaries, and `build-images.sh` refuses a dirty tree or the wrong commit.
