#!/bin/bash
# 02-05 step 8: the two role containers (EXECUTION_PLAN.md §5.3), the 02-04 run-real.sh adapted to
# the coordinator path: central checkpoints (`checkpoint_central` → the coordinator, a supervisor
# agent token), the approved 02-05 budget without shares, an explicit quota_group_id, a read-only
# copy of the target commit at /state/work/<repo> and /state/review/<repo>, Read/Grep/Glob only,
# fresh volumes. No ieo-role-call plan: deliveries come from the coordinator only.
# Usage: run-step8-roles.sh <dir with ieo-supervisor> <fresh workdir outside the repositories>
# Required env: QUOTA_GROUP_ID, SUP_DEV_TOKEN_FILE, SUP_GR_TOKEN_FILE (the supervisors' coordinator
# tokens, `agent` principals with checkpoint:report). Never prints a credential or a token.
# Writes <workdir>/roles-entry.json: the repo entry of the coordinator's --roles file (without
# budget_plan) and <workdir>/adapter.token (0600, for the coordinator's adapter_token_file).
set -euo pipefail
BIN_DIR=$1; W=$2
: "${QUOTA_GROUP_ID:?QUOTA_GROUP_ID is required (the quota group of the subscription account)}"
: "${SUP_DEV_TOKEN_FILE:?}" "${SUP_GR_TOKEN_FILE:?}"
CLAUDE_BIN=${CLAUDE_BIN:-/opt/rust/ieo_poc_probe/bin/claude-2.1.322}
SECRETS_DIR=${SECRETS_DIR:-/opt/rust/ieo_poc_secrets}
NAME_PREFIX=${NAME_PREFIX:-ieo-s8}
REPO_ID=${REPO_ID:-ieo-poc-test}
REPO_SRC=${REPO_SRC:-https://github.com/HeaInSeo/ieo_poc_test}
TARGET_SHA=${TARGET_SHA:-2ab9d8760012516f37674a9446ca2fb2699973c0}
PORT_DEV=${PORT_DEV:-58221}; PORT_GR=${PORT_GR:-58222}
COORD_PORT=${COORD_PORT:-58090}
# the containers reach the host's loopback through slirp4netns at 10.0.2.2: the coordinator keeps
# listening on 127.0.0.1 only (EXECUTION_PLAN.md §5.3, settled by the dry run)
NETWORK=${NETWORK:-slirp4netns:allow_host_loopback=true}
COORD_URL=${COORD_URL:-http://10.0.2.2:$COORD_PORT}
IMG=docker.io/library/golang@sha256:d9c68c2c51161e12fd77e4c6320687c9cd86e1af1e3ad6e6cd63ff970641453c
HERE=$(cd "$(dirname "$0")" && pwd)
W=$(realpath -m "$W")
case "$W" in /opt/rust/src/*) echo "workdir must be outside the repositories"; exit 1;; esac
if [ -e "$W/development" ] || [ -e "$W/guardrail" ]; then
  echo "$W already holds role volumes: refusing to reset them (use a new workdir)"; exit 1
fi
for r in development guardrail; do
  podman container exists "$NAME_PREFIX-$REPO_ID-$r" && { echo "container $NAME_PREFIX-$REPO_ID-$r exists: refusing to replace it"; exit 1; }
done
case "$TARGET_SHA" in *[!0-9a-f]*|"") echo "TARGET_SHA must be lowercase hex"; exit 1;; esac
mkdir -p "$W/bin"; chmod 700 "$W"
cp "$BIN_DIR/ieo-supervisor" "$W/bin/ieo-supervisor"; chmod 755 "$W/bin/ieo-supervisor"
# credential form only (subscription OAuth token, never an API key); the value is never printed
for r in development guardrail; do
  head -c 10 "$SECRETS_DIR/$r.token" | grep -q '^sk-ant-oat' || { echo "$r: credential is not the OAuth token form"; exit 1; }
done
head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$W/adapter.token"; chmod 600 "$W/adapter.token"
( umask 077; printf 'Authorization: Bearer %s\n' "$(cat "$W/adapter.token")" > "$W/adapter.header" )
# the target commit's files only (no .git, no credentials): git archive of the exact SHA
src="$W/target-src"; git clone -q --no-checkout "$REPO_SRC" "$src"
[ "$(git -C "$src" rev-parse --verify "$TARGET_SHA^{commit}")" = "$TARGET_SHA" ] || { echo "target $TARGET_SHA not in $REPO_SRC"; exit 1; }
declare -A UIDS=([development]=20001 [guardrail]=20002)
declare -A SUPTOK=([development]="$SUP_DEV_TOKEN_FILE" [guardrail]="$SUP_GR_TOKEN_FILE")
for r in development guardrail; do
  e="$W/$r/etc"; s="$W/$r/state"; c="$W/$r/cred"
  mkdir -p "$e/template" "$s" "$c"
  echo '{"autoMemoryEnabled": false}' > "$e/template/settings.json"
  echo '{"mcpServers": {}}' > "$e/mcp.json"
  cp "$HERE/role/$r/settings.json" "$e/settings.json"
  cp "$HERE/role/$r/agents.json" "$e/agents.json"
  printf '{"role":"%s","repo":"%s","state":"/state","etc":"/etc/ieo/role","cred":"/run/ieo/cred","claude":"/opt/claude/claude","budget":"02-05","quota_group_id":"%s","checkpoint_central":{"url":"%s","token_file":"/run/ieo/cred/coordinator_token","transport":"http"}}\n' \
    "$r" "$REPO_ID" "$QUOTA_GROUP_ID" "$COORD_URL" > "$e/supervisor.json"
  install -m 0400 "$SECRETS_DIR/$r.token" "$c/oauth_token"
  install -m 0400 "${SUPTOK[$r]}" "$c/coordinator_token"
  sha256sum < "$W/adapter.token" | cut -d' ' -f1 > "$c/adapter.sha256"; chmod 0400 "$c/adapter.sha256"; chmod 0500 "$c"
done
d="$W/development/state/work/$REPO_ID"; g="$W/guardrail/state"
mkdir -p "$d" "$g/review/$REPO_ID" "$g/inputs" "$g/guardrail-scratch" "$g/guardrail-cwd"
git -C "$src" archive "$TARGET_SHA" | tar -x -C "$d"
git -C "$src" archive "$TARGET_SHA" | tar -x -C "$g/review/$REPO_ID"
chmod -R a-w "$d" "$g/review/$REPO_ID"
for r in development guardrail; do podman unshare chown -R "${UIDS[$r]}:${UIDS[$r]}" "$W/$r/state" "$W/$r/cred"; done
run_role() { # role port
  local r=$1 port=$2 name="$NAME_PREFIX-$REPO_ID-$1"
  # no --init: the supervisor must be PID 1, which reaps orphans (review c4620d2 L10)
  podman run -d --name "$name" --user "${UIDS[$r]}:${UIDS[$r]}" --network "$NETWORK" \
    --security-opt no-new-privileges --cap-drop ALL --stop-timeout 15 \
    --label "ieo.repo=$REPO_ID" --label "ieo.role=$r" --label "ieo.step=02-05-s8" \
    -v "$W/$r/state:/state:z" -v "$W/$r/etc:/etc/ieo/role:ro,z" -v "$W/$r/cred:/run/ieo/cred:ro,z" \
    -v "$W/bin:/opt/ieo:ro,z" -v "$CLAUDE_BIN:/opt/claude/claude:ro,z" -p "127.0.0.1:$port:8080" \
    "$IMG" /opt/ieo/ieo-supervisor serve --config /etc/ieo/role/supervisor.json --listen 0.0.0.0:8080 >/dev/null
  for _ in $(seq 1 50); do
    curl -s -o /dev/null -H @"$W/adapter.header" "http://127.0.0.1:$port/v0/role/status" && return
    sleep 0.2
  done
  echo "role $r did not come up"; podman logs "$name" 2>&1 | tail -5; exit 1
}
run_role development "$PORT_DEV"; run_role guardrail "$PORT_GR"
cat > "$W/roles-entry.json" <<JSON
{"development": {"url": "http://127.0.0.1:$PORT_DEV", "adapter_token_file": "$W/adapter.token",
                 "quota_group_id": "$QUOTA_GROUP_ID", "image_digest": "${IMG#*@}",
                 "profile_id": "s8-dev", "profile_revision": 1},
 "guardrail": {"url": "http://127.0.0.1:$PORT_GR", "adapter_token_file": "$W/adapter.token",
               "image_digest": "${IMG#*@}", "profile_id": "s8-gr", "profile_revision": 1}}
JSON
echo "# role containers up: $NAME_PREFIX-$REPO_ID-{development,guardrail} on 127.0.0.1:$PORT_DEV/$PORT_GR · image $IMG · network $NETWORK · coordinator $COORD_URL"
echo "# target $REPO_SRC @ $TARGET_SHA (read-only copies) · credentials: OAuth form (values not printed) · roles entry: $W/roles-entry.json"
