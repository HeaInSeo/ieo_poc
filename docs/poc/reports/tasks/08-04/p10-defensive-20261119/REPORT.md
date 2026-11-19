# 08-04 / P10 defensive network checks

**P10 scoped VM trial PASS on the retry.** The first VM run stopped during setup.

## Local preparation before the VM runs

The #112 main merge `31267c169f14f4103ef73560c1747b4d1605f7f6` passed all four jobs in CI run 44517248720. The initial worktree used that main head. Criteria were committed before test implementation at `22a099c00bbcdcda65f8c3b1cf6f734c9612de87`. The marker grammar was aligned to the existing receiver before any tests at `865529950a3e97791681b381cedae82e1f1370da`. [Frozen criteria](../../../../../../probes/0804-policy-block/CRITERIA.md)

The runner reuses the 08-02 pinned kind/Cilium/Tetragon setup, Restricted fixture Pods and echo receiver. It reuses P09's process, Pod, destination and loss checks and P11's same-name egress replacement. The client is fixed to a numeric Pod IPv4 address, TCP 18080 and the single benign marker `0802-p10-fixture`. The runner refuses an occupied fixture namespace. The 08-02 launcher refuses an existing cluster with that name and cleans up only the cluster it creates in the authorized disposable VM.

| Check | Expected live evidence | Pre-run status |
| --- | --- | --- |
| Open-path observation | Attributed attempt, receiver acceptance and echo, live allowed control, stable observer and no loss. Independent violation verdict excludes completion. | Unexecuted |
| NetworkPolicy | Connect-stage failure for the development actor and zero receiver accepts/marker receipts, with an allowed path and the unaffected control reaching the same receiver on both sides of the blocked case. | Unexecuted |
| Restoration | Original egress rule restored and previously prohibited receiver again reachable. | Unexecuted |
| Local judge | Missing or foreign event cannot prove a violation, failure after TCP connect cannot prove a block, and any receiver accept defeats the blocked verdict. | PASS, four tests |

The local judgment tests used in-memory records and emitted no traffic. Python and shell syntax checks passed. No reachable existing disposable VM was identified through the current inventory and local permissions. The available host does not have the required Docker engine command. No replacement VM was provisioned and no other worktree or shared ledger was modified. [Target discovery](evidence/target-discovery.txt) · [Local checks](evidence/local-checks.txt)

On the designated disposable VM with the existing pinned tooling, the existing launcher accepts `bash probes/0802-cluster-baseline/run-lab.sh <fresh-absolute-output-directory> --p10`. It keeps the raw attribution and client/receiver deltas under `evidence/p10`. No product credentials, production targets, model calls, or remote publication were used.

## Authorized public VM run

The isolated run used the standard public GitHub Actions Ubuntu 24.04 runner via a branch-scoped workflow. The probe was replayed over main `36e8ebc9b0f3c5dd0cb9be8576a60e91279495fb` on the separate branch `verify/0804-p10-isolated-0d15659`. Exact run SHA `955244841295d515a49adc2d1252d45f03ac2130` concluded **FAIL during fixture setup**. Cilium and the fixture endpoints reached readiness. The open-policy comparison aborted before the first P10 attempt with `unexpected active NetworkPolicy in open`. [Run 44519648688](https://github.com/HeaInSeo/ieo_poc/actions/runs/44519648688) · [Exact run metadata](evidence/44519648688-run.json)

Kubernetes omits empty rule arrays in API responses. The pushed P10 comparison compared the original explicit `egress: []` on the receiver with the API's omitted field. The previously archived P11 API response reproduced that mismatch locally. The unpushed correction at `2c1a8bb2789a2b1d3f4967abe589ac596063d0d3` shares the existing 08-02 normalizer and adds a regression that distinguishes omitted empty rules from incorrectly omitted nonempty allow rules. Local checks passed 5/5. It has no live run.

The archive confirms the exact source commit, fixture setup, and cleanup with zero remaining nodes. Its receiver logs contain no accepts or received markers, and no P10 attempt files exist. This absence occurred before the attempted connection and does not qualify as an enforcement result. P10 observation and blocking therefore remain **unverified**. Archive SHA-256 `32457d176371c24726d7d36d95d867c1c2a3afbe32b8fb33f0bd958068ded0ee` matches the job log. [Raw archive](evidence/44519648688-evidence.tar.gz) · [Cleanup](evidence/44519648688-cleanup.txt) · [Receiver logs](evidence/44519648688-receivers-final.txt)

The initial attempt used one branch-scoped push. The scoped retry below supplies the final verification evidence.

## Scoped retry and final evidence

The exact retry `b0ddefaf7882118a1fa1e69e22d38b2a46fb3c12` passed on the standard public Ubuntu 24.04 runner. The original failed archive above is retained unchanged. It contains the raw intended fixture, but the old runner raised before saving the active policy. The overlapping P11 raw API response differs only by omission of the empty egress array. The retry normalizes only empty rule arrays, and saves raw active and raw intended policy objects before comparison. All three live comparisons (open, enforced and restored) matched with selectors, peers, ports and nonempty rules compared exactly. [CI run 44520476538](https://github.com/HeaInSeo/ieo_poc/actions/runs/44520476538) · [Comparison of prior raw evidence](evidence/retry-policy-comparison.txt)

- **Observation.** With the restrictive rule unapplied, the designated receiver accepted and echoed the single marker. Tetragon's TCP event matched the development Pod, process and destination. The observer control matched and all four available loss metric deltas were zero. The required detection verdict was `VIOLATION`, with completion ineligible.
- **Blocking.** With the same-name restrictive rule applied, the denied case timed out at TCP connect. The dedicated receiver logged **zero accepts and zero markers** for that case. The permitted receiver succeeded, and the unaffected control reached the denied receiver immediately before and after the blocked attempt.
- **Restoration.** Removing the restriction restored the original rule and successful reachability. All seven case judgments matched recomputation from the archived client and receiver deltas. Cleanup reported zero remaining kind nodes.

The retry archive's SHA-256 is `90e2d03aea116e19dae941903d4ea6295effff5ddb76ac6fb674319d9d9790c3`, matching the runner log. Its internal source stamp and GitHub metadata match the exact tested SHA. [Raw retry archive](evidence/44520476538-evidence.tar.gz) · [Recheck](evidence/44520476538-recheck.json) · [Run metadata](evidence/44520476538-run.json) · [Cleanup](evidence/44520476538-cleanup.txt)

The submission candidate carries the same runnable probe over main `a65f820341f2abf8d2b1e9ec83800748eeb33e39` with both archives and the replay record. The temporary Actions workflow is specific to the test branch and is excluded from the submission. The live evidence stays attributed to its tested SHA. The shared ledger is untouched.
