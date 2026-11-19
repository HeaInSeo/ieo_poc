# 08-04 / P10 defensive network checks

**Local preparation verified. VM trial unexecuted.** The user supplied the START signal and confirmed the exact #112 main merge `31267c169f14f4103ef73560c1747b4d1605f7f6`, CI run 44517248720, 4/4. This worktree was created from that locally recorded main head. Criteria were committed before test implementation at `22a099c00bbcdcda65f8c3b1cf6f734c9612de87`. The marker grammar was aligned to the existing receiver before any tests at `865529950a3e97791681b381cedae82e1f1370da`. [Frozen criteria](../../../../../../probes/0804-policy-block/CRITERIA.md)

The runner reuses the 08-02 pinned kind/Cilium/Tetragon setup, Restricted fixture Pods and echo receiver. It reuses P09's process, Pod, destination and loss checks and P11's same-name egress replacement. The client is fixed to a numeric Pod IPv4 address, TCP 18080 and the single benign marker `0802-p10-fixture`. The runner refuses an occupied fixture namespace. The 08-02 launcher refuses an existing cluster with that name and cleans up only the cluster it creates in the authorized disposable VM.

| Check | Expected live evidence | Status |
| --- | --- | --- |
| Open-path observation | Attributed attempt, receiver acceptance and echo, live allowed control, stable observer and no loss. Independent violation verdict excludes completion. | Unexecuted |
| NetworkPolicy | Connect-stage failure for the development actor and zero receiver accepts/marker receipts, with an allowed path and the unaffected control reaching the same receiver on both sides of the blocked case. | Unexecuted |
| Restoration | Original egress rule restored and previously prohibited receiver again reachable. | Unexecuted |
| Local judge | Missing or foreign event cannot prove a violation, failure after TCP connect cannot prove a block, and any receiver accept defeats the blocked verdict. | PASS, four tests |

The local judgment tests used in-memory records and emitted no traffic. Python and shell syntax checks passed. No reachable existing disposable VM was identified through the current inventory and local permissions. The available host does not have the required Docker engine command. No replacement VM was provisioned and no other worktree or shared ledger was modified. [Target discovery](evidence/target-discovery.txt) · [Local checks](evidence/local-checks.txt)

On the designated disposable VM with the existing pinned tooling, the existing launcher accepts `bash probes/0802-cluster-baseline/run-lab.sh <fresh-absolute-output-directory> --p10`. It keeps the raw attribution and client/receiver deltas under `evidence/p10`. No product credentials, production targets, model calls, or remote publication were used.
