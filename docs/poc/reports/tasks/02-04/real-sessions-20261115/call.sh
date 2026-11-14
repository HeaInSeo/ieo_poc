#!/bin/bash
# One step of RUN_PLAN.md: call.sh <workdir> <attempt>. Runs ieo-role-call for the step in
# steps.tsv and writes a summary (no raw stream, no secret) to <workdir>/summaries/<attempt>.json.
set -uo pipefail
W=$1; A=$2
HERE=$(cd "$(dirname "$0")" && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
line=$(grep -P "^$A\t" "$HERE/steps.tsv") || { echo "no step $A"; exit 1; }
role=$(cut -f2 <<<"$line"); op=$(cut -f3 <<<"$line")
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
echo "exit $rc · summary $W/summaries/$A.json"
exit $rc
