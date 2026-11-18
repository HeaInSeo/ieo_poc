# Claude and Codex live fixture trial, approval required

This is a proposed follow-up to the isolated PoC. Use a new sandbox repository, assignment, budget and quota-group bindings. Keep the 02-05 step 8 environment, approvals and ledger out of the trial.

## Prerequisites

1. Obtain supported read-only subscription observations for each actual account, including per-window limits, timestamp, account/group and confirmation that usage credits or any metered fallback are disabled. Collect sanitized real Codex samples and validate the mapping. Claude's current classifier is based on runtime events, so after restart it does not establish fresh available status on its own. If either provider lacks a safe read-only observation path, keep its status unknown and block the corresponding trial. Do not spend quota or send probes just to manufacture an exhausted or recovered state.
2. Bind each runtime to an authenticated role target and demonstrate equivalent enforcement for filesystem scope, network access, tool allow/deny, identity and independent review. Exercise those restrictions using harmless repository fixtures. Validate actual CLI session creation and resume inputs. The portable checkpoint includes task, branch/head, verified stage and evidence references. Reconcile working tree or staged changes as well as pushed effects; an unknown side effect is a hard stop.
3. Integrate with the coordinator's current assignment generation, semantic and evidence review, shared budget lock, role ceilings, durable attempt/delivery ledger and trusted measured usage. Confirm non-overage credentials, reserve the remaining budget before dispatch and enforce time/token limits with the runtime guard. Correlate provider usage, UTC timestamps and effects to the exact delivery; an HTTP success or a local process exit does not establish acceptance or completion.
4. Prepare a separate run request with the exact settings and digests, test repo and role, snapshot/rollback, number of authorized model calls, common time/token limits, evidence retention and additional charges capped at USD 0. Get explicit approval before creating any live dispatch. No 02-05 permissions or configuration are copied into this trial.

## Proposed trial after approval

1. With a read-only fixture task, verify status and role identity from both accounts. Pin the branch and starting commit and record a verified first-stage checkpoint.
2. If the approved primary is already demonstrably exhausted and the alternate is available, authorize one remaining-stage delivery. Verify the new session, recorded predecessor and common-budget deduction. Run the opposite direction only with the reciprocal observed quota conditions and remaining authorized budget.
3. If both are exhausted, capture zero deliveries. Observe recovery without starting a model. Recheck approval, common balance and actual repository state before one dispatch. If real quota conditions never occur, report the scenario as unexercised.
4. In the event of uncertainty, wait for independent reconciliation. Stop on any unrecognized quota signal, mismatched role or checkpoint, missing usage, unexpected cost, or unverified effect. Preserve the ledger and revert the trial configuration separately from any source change.
