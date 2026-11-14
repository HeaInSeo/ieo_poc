#!/bin/bash
# 02-04 real-session role containers (RUN_PLAN.md). Sets up the two role containers and the
# caller plan; it makes NO model call (calls are made one by one with ieo-role-call).
#
# Usage: run-real.sh <ieo-supervisor binary> <workdir outside the repo>
# Overrides for the container dry run with a fake claude (dry-run.sh):
#   CLAUDE_BIN, SECRETS_DIR, NAME_PREFIX, PORT_DEV, PORT_GR, NETWORK
#
# Never prints a credential or the adapter token. The workdir holds the credentials (0400,
# owned by the role UIDs), the ledger and the raw streams: keep it outside the public repo.
set -euo pipefail
BIN=$1; W=$2
CLAUDE_BIN=${CLAUDE_BIN:-/opt/rust/ieo_poc_probe/bin/claude-2.1.322}
SECRETS_DIR=${SECRETS_DIR:-/opt/rust/ieo_poc_secrets}
NAME_PREFIX=${NAME_PREFIX:-ieo-rt}
PORT_DEV=${PORT_DEV:-58211}; PORT_GR=${PORT_GR:-58212}
NETWORK=${NETWORK:-slirp4netns}   # 01-04 used slirp4netns for the API
IMG=docker.io/library/golang:1.26.9-bookworm
REPO=fixture0204
HERE=$(cd "$(dirname "$0")" && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
case "$W" in /opt/rust/src/*) echo "workdir must be outside the repositories"; exit 1;; esac
[ -e "$W/ledger/records.jsonl" ] && { echo "a ledger already exists in $W: refusing to reset it"; exit 1; }
podman unshare rm -rf "$W/development" "$W/guardrail" "$W/bin"
mkdir -p "$W/bin" "$W/ledger"; chmod 700 "$W"
cp "$BIN" "$W/bin/ieo-supervisor"; cp "$(dirname "$BIN")/ieo-role-call" "$W/bin/ieo-role-call"; chmod 755 "$W/bin/"*
# credential form only (subscription OAuth token, never an API key); the value is never printed
for r in development guardrail; do
  head -c 10 "$SECRETS_DIR/$r.token" | grep -q '^sk-ant-oat' || { echo "$r: credential is not the OAuth token form"; exit 1; }
done
head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$W/adapter.token"; chmod 600 "$W/adapter.token"
# per-role shares of the 02-04 budget (sum = the approval: 12 runs, 14,400 s, 1.5 M, 15 M)
declare -A SHARE=([development]='{"max_runs":8,"total_seconds":9600,"billable_tokens":1000000,"cache_read_tokens":10000000}'
                  [guardrail]='{"max_runs":4,"total_seconds":4800,"billable_tokens":500000,"cache_read_tokens":5000000}')
declare -A UIDS=([development]=20001 [guardrail]=20002)
for r in development guardrail; do
  e="$W/$r/etc"; s="$W/$r/state"; c="$W/$r/cred"
  mkdir -p "$e/template" "$s" "$c"
  echo '{"autoMemoryEnabled": false}' > "$e/template/settings.json"
  echo '{"mcpServers": {}}' > "$e/mcp.json"
  cp "$HERE/role/$r/settings.json" "$e/settings.json"
  cp "$HERE/role/$r/agents.json" "$e/agents.json"
  extra=""
  if [ "$r" = development ]; then
    cp "$HERE/role/development/CLAUDE.md" "$e/CLAUDE.md"
    extra=',"append_system_prompt_file":"/etc/ieo/role/CLAUDE.md"'
  fi
  printf '{"role":"%s","repo":"%s","state":"/state","etc":"/etc/ieo/role","cred":"/run/ieo/cred","claude":"/opt/claude/claude","budget":"02-04","share":%s%s}\n' \
    "$r" "$REPO" "${SHARE[$r]}" "$extra" > "$e/supervisor.json"
  install -m 0400 "$SECRETS_DIR/$r.token" "$c/oauth_token"
  sha256sum < "$W/adapter.token" | cut -d' ' -f1 > "$c/adapter.sha256"; chmod 0400 "$c/adapter.sha256"; chmod 0500 "$c"
done
# development: the fixture repository (§10 item 2: repo CLAUDE.md and repo hook must not apply)
fx="$W/development/state/work/$REPO"; mkdir -p "$fx/.claude"
cp "$HERE/fixture/README.md" "$fx/README.md"
echo "MARKER-REPO-ONLY-0204: this repository CLAUDE.md must not be loaded." > "$fx/CLAUDE.md"
printf '{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"touch /state/work/%s/HOOK-RAN"}]}]}}\n' "$REPO" > "$fx/.claude/settings.json"
git -C "$fx" init -q && git -C "$fx" add -A && git -C "$fx" -c user.name=fixture -c user.email=fixture@invalid commit -q -m fixture
# development: §10 item 3 (a) markers in the config dir before the first start
cd="$W/development/state/claude"; mkdir -p "$cd/skills/marker0204" "$cd/projects/-state-work-$REPO/memory"
echo '{"autoMemoryEnabled": true, "env": {"MARKER_SETTINGS_0204": "1"}}' > "$cd/settings.json"
echo "MARKER-CONFIGDIR-0204: this config-dir CLAUDE.md must be removed." > "$cd/CLAUDE.md"
printf -- '---\nname: marker0204\ndescription: MARKER-SKILL-0204\n---\nMARKER-SKILL-0204\n' > "$cd/skills/marker0204/SKILL.md"
echo "MARKER-MEMORY-0204" > "$cd/projects/-state-work-$REPO/memory/MEMORY.md"
# guardrail: review snapshot, inputs, scratch, cwd
g="$W/guardrail/state"; mkdir -p "$g/review/fixture" "$g/inputs/0204-RT" "$g/guardrail-scratch" "$g/guardrail-cwd"
cp "$HERE/fixture/README.md" "$g/review/fixture/README.md"
for r in development guardrail; do podman unshare chown -R "${UIDS[$r]}:${UIDS[$r]}" "$W/$r/state" "$W/$r/cred"; done
run_role() { # role port
  local r=$1 port=$2 name="$NAME_PREFIX-$REPO-$1"
  podman rm -f "$name" >/dev/null 2>&1 || true
  podman run -d --name "$name" --user "${UIDS[$r]}:${UIDS[$r]}" --network "$NETWORK" \
    --security-opt no-new-privileges --cap-drop ALL --stop-timeout 15 \
    --label "ieo.repo=$REPO" --label "ieo.role=$r" \
    -v "$W/$r/state:/state:z" -v "$W/$r/etc:/etc/ieo/role:ro,z" -v "$W/$r/cred:/run/ieo/cred:ro,z" \
    -v "$W/bin:/opt/ieo:ro,z" -v "$CLAUDE_BIN:/opt/claude/claude:ro,z" -p "127.0.0.1:$port:8080" \
    "$IMG" /opt/ieo/ieo-supervisor serve --config /etc/ieo/role/supervisor.json --listen 0.0.0.0:8080 >/dev/null
  for _ in $(seq 1 50); do
    curl -s -o /dev/null -H "Authorization: Bearer $(cat "$W/adapter.token")" "http://127.0.0.1:$port/v0/role/status" && return
    sleep 0.2
  done
  echo "role $r did not come up"; podman logs "$name" 2>&1 | tail -5; exit 1
}
run_role development "$PORT_DEV"; run_role guardrail "$PORT_GR"
# the caller plan: first observation = the newest stored 01-04 stream of the same account
OBS=/opt/rust/src/github.com/HeaInSeo/ieo_poc/docs/poc/reports/tasks/01-04/remaining-probes-20261112/evidence/attempts/27-AM2-development/stdout.jsonl
OBS=${FIRST_OBSERVATION:-$OBS}; OBS_UNIX=${FIRST_OBSERVATION_UNIX:-1794470753}  # 2026-11-12T08:05:53Z
"$PY" - "$W" "$PORT_DEV" "$PORT_GR" "$OBS" "$OBS_UNIX" <<'EOF'
import json, sys
w, pd, pg, obs, at = sys.argv[1:]
shares = {"development": {"max_runs": 8, "total_seconds": 9600, "billable_tokens": 1000000, "cache_read_tokens": 10000000},
          "guardrail": {"max_runs": 4, "total_seconds": 4800, "billable_tokens": 500000, "cache_read_tokens": 5000000}}
plan = {"budget": "02-04", "ledger_dir": w + "/ledger",
        "first_observation": {"stream": obs, "observed_unix": int(at)},
        "roles": {r: {"url": f"http://127.0.0.1:{p}", "adapter_token_file": w + "/adapter.token",
                      "share": shares[r], "max_window_seconds": 600}
                  for r, p in (("development", pd), ("guardrail", pg))}}
json.dump(plan, open(w + "/plan.json", "w"), indent=1)
EOF
echo "# role containers up: $NAME_PREFIX-$REPO-{development,guardrail} on 127.0.0.1:$PORT_DEV/$PORT_GR · image $IMG · network $NETWORK"
echo "# plan: $W/plan.json · ledger: $W/ledger · credentials: OAuth form (values not printed)"
