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

## Authorized public VM run

The user designated the standard public GitHub Actions Ubuntu 24.04 runner as the isolated VM and authorized one branch-scoped push. The probe was replayed over main `36e8ebc9b0f3c5dd0cb9be8576a60e91279495fb` on the separate branch `verify/0804-p10-isolated-0d15659`. Exact run SHA `955244841295d515a49adc2d1252d45f03ac2130` concluded **FAIL during fixture setup**. Cilium and the fixture endpoints reached readiness. The open-policy comparison aborted before the first P10 attempt with `unexpected active NetworkPolicy in open`. [Run 44519648688](https://github.com/HeaInSeo/ieo_poc/actions/runs/44519648688) · [Exact run metadata](evidence/44519648688-run.json)

Kubernetes omits empty rule arrays in API responses. The pushed P10 comparison compared the original explicit `egress: []` on the receiver with the API's omitted field. The previously archived P11 API response reproduced that mismatch locally. The unpushed correction at `2c1a8bb2789a2b1d3f4967abe589ac596063d0d3` shares the existing 08-02 normalizer and adds a regression that distinguishes omitted empty rules from incorrectly omitted nonempty allow rules. Local checks passed 5/5. It has no live run.

The archive confirms the exact source commit, fixture setup, and cleanup with zero remaining nodes. Its receiver logs contain no accepts or received markers, and no P10 attempt files exist. This absence occurred before the attempted connection and does not qualify as an enforcement result. P10 observation and blocking therefore remain **unverified**. Archive SHA-256 `32457d176371c24726d7d36d95d867c1c2a3afbe32b8fb33f0bd958068ded0ee` matches the job log. [Raw archive](evidence/44519648688-evidence.tar.gz) · [Cleanup](evidence/44519648688-cleanup.txt) · [Receiver logs](evidence/44519648688-receivers-final.txt)

One push was made. The correction and evidence report remain local. No merge, PR, shared ledger edit, production target, product credential, or model call was involved.
