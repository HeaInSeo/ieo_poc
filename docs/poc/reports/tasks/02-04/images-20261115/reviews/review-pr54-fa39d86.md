# Review of PR #54, head fa39d86 (review 2 of at most 4; follows correction 1 of at most 3)

- **PR:** HeaInSeo/ieo_poc #54, "02-04: coordinator PID 1, repo-scope role control, role images, carry-over triage"
- **Head reviewed:** fa39d86f4c59dcb787e36aa18e5631d9cf20befd (confirmed with `gh pr view 54 --json headRefOid,statusCheckRollup`). Base: main c5e9af0. Correction: code commit 8fca0a0, docs commit fa39d86 (`git diff 8fca0a0 fa39d86` touches only docs/evidence).
- **CI at fa39d86:** all three checks COMPLETED SUCCESS (docs/poc package and progress validation; rust toolchain pin resolves; rust fmt clippy test).
- **Reviewer:** independent review agent (Claude Code). No model calls. Did not touch the persistent DB/coordinator (55433/58090), /opt/rust/ieo_poc_runs or the secrets directory. No push, comment, merge or branch change.
- **Verdict: APPROVE.** Both blocking findings of review adf760e (B1, B2) are resolved, and M-1, M-2, L-1, L-3 are resolved; L-2 is mostly resolved. No new blocking defect. The non-blocking findings below (one medium about the §7 `provider_status_or_unknown` shape and the "충족(모의)" label) should be fixed or stated in the 02-04 acceptance report.

## What I ran

In worktree `<scratchpad>/rv54b` at fa39d86 with its own target dir (`CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`); `df -h /` was 8.3 GB free before and after. Worktree, target dir and the scratch workdir were removed afterwards.

- `cargo fmt --all -- --check`: OK.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: OK.
- `cargo test --locked -p ieo-coord -p ieo-supervisor` (test DB on 55432): **165 passed, 0 failed** (coordinator 66, roles 6, shutdown 2, caller 23, run_guard 27, serve 19, supervisor 20, lib 1+1). That is review 1's 161 plus the 4 new tests (3 roles, 1 serve; the coordinator test was extended in place).
- **Evidence arithmetic:** `evidence/cargo-test.log` header is commit 8fca0a0 with a clean tree; `test result` lines sum to 298 passed / 0 failed, 0 SKIP lines (= review 1's 294 + 4). `evidence/build-images.txt` shows revision 8fca0a0, profile debug, CLI hash bfb3703a…, three images with the user IDs 20000/20001/20002.
- **`images-fake.sh` re-run** against the existing `localhost/ieo-poc/*:8fca0a0` images (no rebuild). Before: 0 `ieo-img-*` containers, no `ieo-img-net`, no `ieo-img` secret, ports 58231-58232 free. Result identical to `evidence/images-fake.log` (same image IDs/digests/labels, PID 1 `ieo-coord` UID 20000, 401 without credential, DB password 0/0/0, 15 tables, repo stop dev 200 / Guardrail 202 both stopped with checkpoint saved, `safe_shutdown_confirmed` false with the 4 `not_tracked` reasons, 0.5 s, Guardrail `Requested`, 0 leftover processes, SIGTERM 0.3/0.5/0.3 s exit 0, "stopping (signal)"/"stopped"). Exit 0. Afterwards: 0 containers, 0 networks, 0 secrets; the new EXIT trap removed `adapter.header`, `coord-etc/adapter.token`, `coord-etc/operator.header` (only the hash file `tokens.json` remained). I removed the workdir.
- **Mutation tests** (each `timeout 900`, file restored after each, worktree verified clean):

| # | Mutation | Result |
| --- | --- | --- |
| N1 | repo check reverted to `!repos.is_empty() && !contains` (B1 fail-open) | killed (`repo_scope_role_control_…`) |
| N2 | refused (non-200/202) stop is polled for the full wait | killed (`a_refused_stop_is_reported_without_waiting`) |
| N3 | unreachable role is polled (review-1 M5) | killed (`an_unreachable_role_is_not_waited_for`) |
| N4 | sequential stop (send + wait per role, review-1 M-1) | killed (`both_roles_get_their_stop_before_either_is_waited_for`) |
| N5 | unreadable checkpoint counted as saved | killed (`status_reports_the_section_7_fields_…`) |
| N6 | any parseable checkpoint counted as saved (takeover/error phase) | **survived** (L-a) |
| N7 | `tracked` not written to the checkpoint | killed |
| N8 | `provider_status` always null | killed |
| N9 | `active_children: 0` instead of `not_tracked` (review-1 M7) | killed |
| N10 | `restored_checkpoint_ref` never found | killed |
| N11 | `quota_group_id` config ignored | **survived** (L-a) |
| N12 | wait budget reset per role (each role gets the full 30 s) | **survived** (L-a; behavioural, low) |
| N13 | `ended_at` dropped (`last_actual_action_at`) | killed |

Review-1 survivors M5, M7, M9, M11 are all now killed (N3, N9, N5, N2).

## Review-1 findings

| Finding | Status | Evidence |
| --- | --- | --- |
| B1 empty `repo_ids` fails open | **Resolved** | `api.rs` uses `who.in_repo(repo)` (`auth.rs`: `repos.contains`, empty = none). Test principal `operator-norepo` gets 403 SCOPE_DENIED on GET and POST; N1 killed |
| B2 triage incomplete / owner changes | **Resolved** | See "Triage completeness" below |
| M-1 sequential stop, unchecked stop status, 30 s spin on 401 | **Resolved** | Both stops sent first, shared deadline; non-200/202 and unreachable reported at once; three timing tests; N2/N3/N4 killed. The SIGTERM interaction line was not added to PROGRESS, but with the new early returns the worst case is now ~2×10 s (send) + 30 s + 10 s, still above `--stop-timeout 15`; see L-b |
| M-2 no safe_shutdown test; unreadable checkpoint; `[]` in checkpoint | **Resolved** | New serve test; `(Some(_), None) => false`; checkpoint `tracked` = all false; N5/N7/N9 killed |
| L-1 untested claims | **Resolved** | Agent (`dev-a`) refusal now tested; "처리 중인 요청을 마치는 동작은 시험하지 않았다" stated in §1; unreachable not waited for is tested |
| L-2 images and scripts | **Mostly resolved** | Containerfile comment fixed; `CLAUDE_SHA256` check in `build-images.sh`; EXIT trap with token deletion (verified on re-run). The weak `grep -c 'password='` check was not changed and §6 does not say it was declined (L-c) |
| L-3 §5 labels | **Resolved** | §5 has rows for coordinator start/reconnect (미충족, 02-05 proposal) and §7 status fields — but see M-a on the latter's label |

## Triage completeness (against `unresolved` on main c5e9af0)

I went through the old `unresolved` text item by item against PROGRESS §4 (closed / remaining), §5 and the new `state.json` `unresolved`. Every item from review 1's B2(a) list is now present: §7 status fields (closed, with `not_tracked` remainder in the remaining table), stop grace 10→6, 23fee80 PROGRESS §2 charge note, f73ac13 P3 memory half (remaining, owner "다음 supervisor 정비"), Read last-line observation, start/lock serialisation, debug log, first SHA256SUMS, D3 answer (remaining table and `state.json` now agree). All other old items (PR #52 L1–L5, PR #51 P1–P4/N6/F8, PR #49 L-c3-1..5 and the deadline-stop test, PR #48 F1–F4, N2, PR #47 R1–R6, 23fee80 five items, rev 6 §10 1–5, rev 7 §11 item 7, P3-9/P3-7, P3-2/3/4/6, 02-03 items, X-01 U3, in-run stop, rebuild reproducibility, same-UID limit, allowlist 03-02) are either closed with a pointer I could verify or listed with owner, timing and acceptance impact. Two tiny old notes are not named (L-c).

The 02-03 items (assignment-level `elapsed_seconds`/`total_tokens`, real GitHub collector with F4/N5, F3) are now marked **제안**, cite that the accepted 02-03 report and review pr26-84a123a fixed them as "02-04 수용 전", and say they count as unmet for 02-04 acceptance until the acceptance review/user confirms. `state.json` `unresolved` repeats this and the coordinator start/reconnect move as proposals. This is honest and matches the user instruction. Completion count 11/73 and 02-04 "진행" unchanged; spares dev 2 / Guardrail 3 and budget unchanged; no model calls claimed or observed.

## New findings

### M-a (medium, non-blocking): `provider_status_or_unknown` does not have the contract's shape; §5 "충족(모의)" overstates the §7 status row

- **Where:** `crates/ieo-supervisor/src/serve.rs` `provider_status()` and `status()`; PROGRESS §1 "supervisor 상태의 §7 항목" and §5 row "supervisor 상태의 §7 항목 → 충족(모의)".
- **What:** Runtime contract v0 §7 (line 165) and §8 (lines 177-178) define `provider_status_or_unknown` as a classification that the coordinator acts on: `usage_limit` → QUOTA_WAIT, `auth_or_billing` → 사람 결정 대기, `unknown` when not observable. The implementation returns the raw last `rate_limit_event` object `{status, overageStatus, isUsingOverage}` or the string `unknown`. A consumer following the contract (`== "usage_limit"`) never fires; a rejected rate-limit event or an auth/billing error result is not mapped. In addition:
  - it reflects only the last *finished* run (`st.last`), so it is stale during a run and becomes `unknown` after a service restart even though the volume has the stream;
  - `last_actual_action_at` likewise is null after a restart and is not updated while a run is in progress;
  - `quota_group_id` is read from config, but no config in the repo (role configs, `images-fake.sh`) sets it, so it is always `unknown`.
- **Why it matters:** the field set is present, but "충족" for this row implies the contract semantics. "부분 충족" (or "충족(모의), 값 분류는 계약과 다름") is the accurate label.
- **Fix:** either map to the contract vocabulary (e.g. `rate_limit_info.status == "rejected"` → `usage_limit`, a result with an auth/billing error → `auth_or_billing`, `allowed*` → `ok`, else `unknown`) and keep the raw object under a separate key, or keep the code and relabel the §5 row and note the gap (and the restart/staleness behaviour, and that `quota_group_id` is never configured) in the 02-04 acceptance report's remaining table.

### L-a (low): three surviving mutations

- **N6:** no test pins that a `takeover` or `error` checkpoint gives `checkpoint_saved: false` (the test covers no checkpoint, `after_run`, unreadable). Review 1 asked for `takeover`/`error`; add one write of each phase and assert false.
- **N11:** `quota_group_id` from config is never tested (only the `unknown` default).
- **N12:** the 30 s budget is shared across roles ("합계 최대 30초" in §1), but nothing pins it; a per-role reset would double the worst case. Low; a timing test with two never-stopping fakes and `wait = 1 s` asserting < 2 s total would pin it.

### L-b (low): repo-stop report details

- **Where:** `crates/ieo-coord/src/roles.rs` `stop()`.
- A refused stop is reported as `{"http_status": st, "body": b}` without `stop_http_status`, and the reason text "stop or status answered {st}" does not say which call refused. Since the code knows (refused stop never polls), say "stop refused {st}" vs "status answered {st}".
- When the stop was delivered (200/202) but every status poll then fails at the transport level, the role is reported as `reachable: false` / "unreachable", hiding that the stop reached it. Include `stop_http_status` in that branch too.
- Stops are still *sent* sequentially, so one role whose connect hangs delays the other role's stop by up to the 10 s call timeout. Acceptable for the PoC; worth one line, together with review-1 M-1's note that a repo stop in flight can outlive `--stop-timeout 15` on coordinator SIGTERM (not yet in PROGRESS).

### L-c (low): small record gaps

- Two old `unresolved` notes are not named in §4: the pre-swap binary `652fc14f…` preserved after the fact (`requery.txt`), and supervisor-core PROGRESS §2 "실제 세션 시험 전부 미실행" (de facto closed by 진행 기록 6). Both are informational; one closed row each would make the table exhaustive.
- §6 L-2 does not mention the password check (`grep -c 'password='` rather than the generated `$PGPW`), which was neither changed nor declined.
- `images-fake.sh` does not read the new §7 status fields through the container, so the "충족(모의)" for that row rests on the cargo test only; fine, but the acceptance-report re-run could print them.

## Verified as correct

- `in_repo` semantics now match the other 9 call sites and `ieo_report` (empty = no repo).
- Stop ordering: both POSTs before any poll; deadline measured once from after the sends; the first-arm report only fires on stop OK + status 200, so a refused stop can never be counted as stopped.
- `checkpoint_saved` is false while running, false for an unreadable file, false for any phase other than `after_run`, true with no checkpoint (idle role that never ran).
- `restored_checkpoint_ref` is computed once at startup from the volume (session, phase, base attempt, SHA-256; `readable: false` on parse failure); no secret material in it. `tracked` in the checkpoint is additive; I found no strict-schema consumer of the checkpoint in `ieo-role-call` or the supervisor.
- `build-images.sh` refuses a CLI whose SHA-256 differs from bfb3703a… before anything else.
- Evidence files (`images-fake.log`, `build-images.txt` at 8fca0a0, `cargo-test.log` 298 passed) match the claims and my re-runs.
