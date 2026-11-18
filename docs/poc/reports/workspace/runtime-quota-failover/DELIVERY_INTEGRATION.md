# Isolated coordinator delivery integration

Base is the PR #92 merge at `4e04893b4e010ccae05642e648838dc7494d0315`. The integration tests use a disposable PostgreSQL database, loopback coordinator and supervisor, and two fake executables behind one logical development role. The fixture invokes the PoC quota classifiers before setting its runner routing. The existing HTTP delivery path remains the only admission and accounting path. There is no separately budgeted PoC journal in the DB fixture, no production runtime broker, and no 02-05 step 8 service or ledger is used.

## Boundaries exercised

| Scenario | Evidence |
| --- | --- |
| Approval and authority | HTTP checks caller scope, repository/role and assignment generation. Existing supervisor verifies the assigned runtime profile, spec digest and generation. The coordinator compares the checkpoint and branch/head under the budget lock; fixture rechecks the same branch and head sent with the continuation. |
| Withdrawal before sending | The fixture holds the budget lock, waits for the old approved delivery intent to exist in PostgreSQL, and withdraws review in the fixture DB. When the lock is released, the final row-lock check returns `not_called`. Both supervisor POST count and model-run ledger count remain unchanged. No dispatch hash or in-flight reservation is issued for that delivery. The normal APIs still treat a review decision as immutable. |
| Prior stage and concurrency | Claude executes `PREPARE`; the supervisor and coordinator retain its checkpoint, and a matching ingested branch observation allows resume. The Codex fake receives only `VERIFY` with the predecessor delivery reference in a new session. Two concurrent callers submit one continuation key; there is one DB creation and one supervisor start. Role, profile, generation and branch/head checked against the actual request. |
| Budget contention | Both runtime runs share the existing `caller.lock`, outer run ledger and role share. The live lock and in-flight delivery ID are inspected during the Codex fake run. A competing request routed toward Claude submits nothing while that run is active. The two measured rows consume the two-run role share and later requests are refused. Their aggregate measured usage is 40 billable and 40 cache-read tokens. |
| Lost response and restart | Drop an accepted supervisor reply, preserve its unmeasured ledger entry and delivery ID, restart the real coordinator process on the same fixture DB. The same request only re-queries the supervisor (digest match). A new delivery gets `ASSIGNMENT_USAGE_UNMEASURED` and sends nothing. Acceptance evidence does not settle usage. |
| Effective permission comparison | Both fakes verify the same supervisor read-only tool envelope, including `Read,Grep,Glob`, repository scope, credential read deny and strict MCP config. With `IEO_TEST_FIXTURE_BWRAP=1`, both also run in identical read-only fixture mounts and separate network namespaces. The probes verify an allowlisted read, an outside-root read denied, an attempted source write denied and separation from the host network namespace. Only synthetic run evidence is writable. |

## Reproduction

With `IEO_TEST_DB_URL` pointing to a disposable PostgreSQL instance, run:

```bash
cargo test --locked -p ieo-coord --test delivery_e2e
IEO_TEST_FIXTURE_BWRAP=1 cargo test --locked -p ieo-coord --test delivery_e2e failover_delivery
```

The second variant requires a Linux host that allows Bubblewrap namespaces. GitHub CI runs the standard database cases. Local runs passed 4/4 with the standard fixtures and 2/2 with kernel isolation enabled. Provider quotas, runtime credentials, and the fixture's normalized usage stream are synthetic. Independent runtime identity binding, live policy parity, provider quota and billing gates, and an authorized method to settle usage when an actual reply is lost remain separate prerequisites. See [the one-direction live proposal](LIVE_PLAN.md).

## Independent review correction

The first integration head `32e93cd` was rejected on the final submission ordering and one test gap. Ordinary delivery now holds the requested assignment and review rows, the development head when acting as guardrail, and branch/CI evidence locks through the bounded acceptance POST. Checkpoint observation is validated with its lock at the final fence and bound into the request by digest. A new test blocks the final row-lock query, ingests a different branch head after the previous judgment, and checks for zero supervisor POSTs. Another distinct-key HTTP case reaches the shared budget admission gate while idle and asserts the exact `another caller holds the ledger` refusal, with no additional POST or ledger row.

The separate one-shot recovery endpoint still commits use before a possible POST to survive a process crash. It checks the final fence inside the same transaction *before* recording the use. A pre-send fence refusal leaves the same approval available. The recovery fixture proves that the same approval works after review is restored and that an ambiguous POST retains its committed use across restart. Its post-commit handoff was corrected as described below.

The second review at `27ffc9f` rejected the recovery commit-to-POST gap. The corrected recovery path commits its one-shot use on a second, bounded connection while the primary connection retains the recovery, assignment, approval and evidence locks through supervisor acceptance. It checks shutdown and the send deadline again after the use commits. A refusal before that commit leaves the approval reusable. A shutdown or expiry after the durable commit records no dispatch and consumes no model run; the durable recovery-use history is kept. The synchronized recovery fixture pauses the supervisor before acceptance, observes the committed recovery use, and confirms that competing evidence and assignment writers cannot acquire their locks.

The first full CI on `8fd91e7` exposed a circular wait with the delayed-reply transport fixture. The supervisor had accepted the HTTP request and needed to report takeover before releasing the fixture reply. Holding the checkpoint lock while waiting for that reply blocked the report. The final coordinator check therefore snapshots the checkpoint under its lock, binds its SHA-256 into the dispatch, then releases only that lock before POST. The supervisor compares its checkpoint digest and checks the central record before starting. Assignment, approval, recovery and evidence locks remain held. In the synchronized test the checkpoint lock is available while the other decision locks and committed recovery use remain observable.

Review of `6239b76` found that the supervisor used a second local checkpoint read to select the session. The corrected supervisor chooses from the same validated bytes, checks the on-disk digest again at acceptance and before takeover, and reports the pin with the takeover. The coordinator compares the pin to the last stored checkpoint under its existing report lock before accepting the takeover. A deterministic supervisor test mutates the file during the central comparison and verifies that neither acceptance nor a run is created.
