#!/bin/bash
# One step of RUN_PLAN.md: call.sh <workdir> <attempt>. Runs ieo-role-call for the step in
# steps.tsv and writes a summary (no raw stream, no secret) to <workdir>/summaries/<attempt>.json.
set -uo pipefail
W=$1; A=$2
HERE=$(cd "$(dirname "$0")" && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
line=$(grep -P "^$A\t" "$HERE/steps.tsv") || { echo "no step $A"; exit 1; }
role=$(cut -f2 <<<"$line"); op=$(cut -f3 <<<"$line")
if [ "$op" = restart ]; then
  # a container restart between runs (no model call): the checkpoint must be restored from the
  # role volume (2026-11-14 pr30 §4 checkpoint restoration)
  c="${NAME_PREFIX:-ieo-rt}-fixture0204-$role"; port=$(podman port "$c" 8080 | head -1 | cut -d: -f2)
  # the same guards as a call (review f90f5b5 N2): a restart erases the service's in-memory last
  # attempt, so it never runs over a call in flight, a record that asks for a person, a running
  # attempt or a last attempt the caller does not know
  curl -s -H @"$W/adapter.header" "http://127.0.0.1:$port/v0/role/status" > "$W/.status.$A" || { echo "restart refused: status unavailable"; exit 1; }
  "$PY" - "$W" "$A" <<'PYEOF' || exit 1
import json, os, sys
w, a = sys.argv[1], sys.argv[2]
st = json.load(open(f"{w}/.status.{a}"))
recs = [json.loads(l) for l in open(f"{w}/ledger/records.jsonl")] if os.path.exists(f"{w}/ledger/records.jsonl") else []
known = {r.get("attempt_id") for r in recs}
why = []
if os.path.exists(f"{w}/ledger/inflight.json"): why.append("a call is in flight without a record")
why += [f"record {r.get('attempt_id')} asks for a person" for r in recs if r.get("stop_required")]
if st.get("running"): why.append("an attempt is running")
last = (st.get("last") or {}).get("attempt_id")
if last and last not in known: why.append(f"the service's last attempt {last} has no caller record")
cpa = (st.get("checkpoint") or {}).get("session_cumulative_attempt")
if cpa and cpa not in known: why.append(f"the checkpoint names attempt {cpa}, which has no caller record")
if why:
    print("restart refused: " + "; ".join(why)); sys.exit(1)
PYEOF
  rm -f "$W/.status.$A"
  before=$(curl -s -H @"$W/adapter.header" "http://127.0.0.1:$port/v0/role/status" | "$PY" -c 'import json,sys; c=json.load(sys.stdin)["checkpoint"]; print(c["session_id"], c["phase"], c.get("session_cumulative_attempt"))')
  t0=$(date +%s.%N); podman restart -t 15 "$c" >/dev/null; t1=$(date +%s.%N)
  for _ in $(seq 1 50); do curl -s -o /dev/null -H @"$W/adapter.header" "http://127.0.0.1:$port/v0/role/status" && break; sleep 0.2; done
  after=$(curl -s -H @"$W/adapter.header" "http://127.0.0.1:$port/v0/role/status" | "$PY" -c 'import json,sys; s=json.load(sys.stdin); c=s["checkpoint"]; print(c["session_id"], c["phase"], c.get("session_cumulative_attempt"), "last", s["last"])')
  mkdir -p "$W/summaries"
  printf '{"step": "%s", "restart_seconds": %s, "pid1": "%s", "before": "%s", "after": "%s"}\n' "$A" \
    "$("$PY" -c "print(round($t1-$t0,1))")" "$(podman exec "$c" cat /proc/1/cmdline | tr '\0' ' ' | cut -c1-40)" "$before" "$after" > "$W/summaries/$A.json"
  cat "$W/summaries/$A.json"; exit 0
fi
# the credential form again before each call (prefix only, never the value)
podman unshare head -c 10 "$W/$role/cred/oauth_token" | grep -q '^sk-ant-oat' || { echo "$role: credential is not the OAuth token form"; exit 1; }
if [ ! -e "$W/coordinator.json" ]; then
  "$PY" - "$W" <<'PYEOF'
import hashlib, json, sys
root = "/opt/rust/src/github.com/HeaInSeo/ieo_poc/docs/poc/Console_Coordinator_Sprints_v0.json"
d = hashlib.sha256(open(root, "rb").read()).hexdigest()
json.dump({"generation": 0, "assignment_id": "0204-RT", "sprint_id": "POC-02", "sprint_revision": 7,
           "spec_digest": "sha256:" + d,
           "image_digest": "docker.io/library/golang@sha256:d9c68c2c51161e12fd77e4c6320687c9cd86e1af1e3ad6e6cd63ff970641453c",
           "profile_id": "poc-acceptance-v1.3.3", "profile_revision": 7}, open(sys.argv[1] + "/coordinator.json", "w"))
PYEOF
fi
if [ "$A" = D6 ]; then
  # (b): a skill planted after the checkpoint (not under plugins/store/*_builtin-*.json)
  podman unshare sh -c "mkdir -p '$W/development/state/claude/skills/tamper0204' && printf -- '---\nname: tamper0204\ndescription: MARKER-TAMPER-0204\n---\nMARKER-TAMPER-0204\n' > '$W/development/state/claude/skills/tamper0204/SKILL.md' && chown -R 20001:20001 '$W/development/state/claude/skills'"
fi
mkdir -p "$W/summaries"
vol() { podman unshare ls -dZn "$W/$role/state" "$W/$role/state/claude" 2>&1 | sed "s|$W|<W>|"; }
{ echo "# before $A"; vol; } > "$W/summaries/$A.volume.txt"
sid=$(cat /proc/sys/kernel/random/uuid)
"$W/bin/ieo-role-call" --plan "$W/plan.json" --task 02-04 --role "$role" --op "$op" --attempt "$A" \
  --session "$sid" --prompt-file "$HERE/prompts/$A.txt" --coordinator-file "$W/coordinator.json"
rc=$?
{ echo "# after $A"; vol
  echo "# files"
  for f in work/fixture0204/HOOK-RAN guardrail-scratch/g1.txt review/fixture/g1-denied.txt inputs/0204-RT/g1-denied.txt claude/skills/tamper0204/SKILL.md; do
    p="$W/$role/state/$f"; if podman unshare test -e "$p"; then echo "$f exists: $(podman unshare head -c 40 "$p" | tr '\n' ' ')"; else echo "$f absent"; fi
  done; } >> "$W/summaries/$A.volume.txt"
"$PY" "$HERE/summarize.py" "$W/ledger/records.jsonl" "$A" > "$W/summaries/$A.json" 2>&1 || true
# exit: 0 recorded and nothing to look at; 2 not started; anything else (3, 1, …): stop
echo "exit $rc · result $("$PY" -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d["result"]["subtype"], "is_error", d["result"]["is_error"], "usage", (d["usage_run"] or {}).get("state"), "stop_required", d["stop_required"])' "$W/summaries/$A.json" 2>/dev/null || echo unknown) · summary $W/summaries/$A.json"
exit $rc
