# Runtime quota failover PoC acceptance

Base is main ce0f852. This PoC runs through a separate coordinator module with a private fixture journal and injected adapters. Its tests use two in-process fake runtimes and a fake authority. Deployment, live credentials, provider calls, and the 02-05 step 8 services and ledger are outside this scope.

| Case | Acceptance |
| --- | --- |
| Primary exhausted | Claude → Codex and Codex → Claude, only with fresh available evidence and eligible role/permissions |
| Portable resume | A new runtime session receives the same assignment, role, branch, head, last completed stage and verification reference. Only the next stage is submitted |
| Uncertain delivery | Persist the intent before dispatch. Check both authoritative ledger and actual effects before advancing. Unresolved intent, usage, or checkpoint discrepancy yields zero further sends, including after restart |
| Both exhausted | Zero dispatches. After a fresh recovery observation, revalidate approval, current checkpoint and remaining budget. Concurrent or repeated resumes cannot repeat a completed stage |
| Common budget and approval | Prior dispatches and usage remain charged across runtimes. Deny exhausted budgets, stale review or assignment, and runtime/role/permission mismatches. Require explicit zero-extra-charge evidence |
| Unknown | Missing, stale, account or quota-group mismatch, auth/billing errors, and ambiguous signals do not prove availability or trigger a switch |

## Reuse and integration boundary

- Claude uses the existing supervisor `provider::classify` and its rate-limit policy. Codex uses a versioned **fixture** input. Production Codex signal mapping will require verified samples and a read-only source; this fixture defines no live Codex protocol.
- Reuse the quota-group distinction between shared limits and credential failures. PoC observations are bound to the configured runtime and group, and one adapter represents each group. Observations are freshness-bounded and neither recovering a timer nor querying an adapter can dispatch a model request.
- Reuse the checkpoint identity and reconciliation principles. A provider session ID cannot serve as a cross-provider reconnect key. The portable checkpoint carries a pinned branch/head and a verified stage prefix; the authority supplies current approval/permission and worktree evidence before every dispatch.
- Reuse `budget::spent` to aggregate measured fixture rows using the existing run, elapsed-time and token accounting. A separate, durable fixture journal stores one intent per dispatch and a common budget for one assignment. The coordinator passes remaining ceilings to the adapter. Actual wall-time, token and billing enforcement for live runners remains an adapter integration requirement.
- Live integration needs authenticated read-only quota evidence for each subscription (including paid usage disabled), an independent worktree/PR reconciliation source and trusted result/usage ingestion, mapping of effectively equivalent role restrictions, existing assignment and review checks, integration with the shared production budget lock and ledger, and a separately approved execution plan. Synthetic role permissions, usage and evidence cannot authorize live execution.

## Isolated evidence

Run `cargo test --locked -p ieo-coord --test runtime_failover`. The harness stores fake dispatches separately from the coordinator journal and reconciles each exact attempt against the fake worktree checkpoint.

| Scenario | Assertion |
| --- | --- |
| Claude → Codex and Codex → Claude | Two total sends in each direction. Second send is only `verify` in a new session, with the verified `prepare` checkpoint and reduced time/token ceilings |
| Ambiguous predecessor | Reopening the journal yields zero further sends until evidence agrees. Once reconciled, the already completed stage is skipped |
| Both exhausted → recovery | Zero sends while both are exhausted. Older availability does not advance. Fresh recovery permits one send, even if the other side's observation has aged out |
| Concurrent recovery | Two calls racing on the same journal produce one send for the remaining stage |
| Confirmed no effect | Charges the sent attempt, waits for evidence of quota exhaustion and recovery before resubmitting on the same runtime, and keeps a new attempt/session ID |
| Other gates | Run, time, billable and cache ceilings; revoked approval; changed worktree head; role mismatch; stale quota; and missing proof of disabled paid usage all block the successor. Observed nonzero cost blocks any continuation |

The journal is local to one authorized assignment. One OS file lock protects the transition and dispatch. The intent is atomically persisted before submission; a crash before send is intentionally ambiguous until the authority proves whether it was sent and what effects it had. Recovery after both quotas were confirmed exhausted relies on a newer available observation for the selected runtime and rechecks approval. Before that waiting state, an unknown primary does not independently authorize a switch. See [the separately approved live trial proposal](LIVE_PLAN.md) for missing integration work.

## Independent review follow-up

The initial head `8d555d126f90ebce4f818705a5b7205e4d2336a0` was rejected after the ten existing tests passed. Four regressions now cover approval revoked during quota observation, an older or same-time conflicting exhaustion signal, measured per-run time beyond its recorded allowance, and a crash before send. The corrected coordinator revalidates approval at the submission boundary and carries that approval revision in the dispatch. An independently proven no-send consumes no run; a lost or uncertain result remains pending. The production authority still needs to implement the authorization fence against real assignment/review changes.

Head `32853b6e31e41659a4853f85632c218f7e62b416` passed 14 tests but was independently rejected on two recovery boundaries. The next correction preserves the pair-exhausted waiting state through a pending intent until an actual send is independently established. It also invalidates same-time disputed exhaustion as evidence for recovery. The suite now has 16 cases, including the alternate-only no-send recovery and both orderings of a same-time quota conflict.
