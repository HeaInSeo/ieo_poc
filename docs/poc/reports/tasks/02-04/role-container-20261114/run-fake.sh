#!/bin/bash
# 02-04 role containers with a fake claude (no model calls). Usage: run-fake.sh <supervisor-binary> <workdir>
# Prints a log; never prints the adapter token or a credential.
set -euo pipefail
BIN=$1; W=$2
IMG=docker.io/library/golang:1.26.9-bookworm
REPO=sori
rm -rf "$W"; mkdir -p "$W/bin"
cp "$BIN" "$W/bin/ieo-supervisor"
cat > "$W/bin/fake-claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
cfg=$(ls -A "$CLAUDE_CONFIG_DIR" 2>/dev/null | tr '\n' ',')
printf '{"type":"system","subtype":"init","session_id":"%s","uid":"%s","cwd":"%s","config":"%s","prompt_seen":%s}\n' "$sid" "$(id -u)" "$(pwd)" "$cfg" "$(echo "$prompt" | grep -q '"type":"user"' && echo true || echo false)"
case "$prompt" in *SLEEP*) sleep 60;; esac
printf '{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n'
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","modelUsage":{"m":{"inputTokens":3,"outputTokens":7,"cacheReadInputTokens":20,"cacheCreationInputTokens":10}},"total_cost_usd":0.01}\n' "$sid"
FAKE
chmod 755 "$W/bin/"*
TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
setup_role() { # role uid port
  local r=$1 uid=$2
  mkdir -p "$W/$r/state" "$W/$r/etc/template" "$W/$r/cred"
  echo '{"autoMemoryEnabled": false}' > "$W/$r/etc/template/settings.json"
  printf '{"role":"%s","repo":"%s","state":"/state","etc":"/etc/ieo/role","cred":"/run/ieo/cred","claude":"/opt/ieo/fake-claude"}\n' "$r" "$REPO" > "$W/$r/etc/supervisor.json"
  echo "fake-$r-credential" > "$W/$r/cred/oauth_token"
  printf '%s' "$TOKEN" | sha256sum | cut -d' ' -f1 > "$W/$r/cred/adapter.sha256"
  chmod 0400 "$W/$r/cred/"*; chmod 0500 "$W/$r/cred"
  podman unshare chown -R "$uid:$uid" "$W/$r/state" "$W/$r/cred"
}
run_role() { # role uid port
  local r=$1 uid=$2 port=$3
  podman rm -f "ieo-ct-$REPO-$r" >/dev/null 2>&1 || true
  podman run -d --name "ieo-ct-$REPO-$r" --user "$uid:$uid" --security-opt no-new-privileges --cap-drop ALL \
    --label "ieo.repo=$REPO" --label "ieo.role=$r" \
    -v "$W/$r/state:/state:z" -v "$W/$r/etc:/etc/ieo/role:ro,z" -v "$W/$r/cred:/run/ieo/cred:ro,z" \
    -v "$W/bin:/opt/ieo:ro,z" -p "127.0.0.1:$port:8080" \
    "$IMG" /opt/ieo/ieo-supervisor serve --config /etc/ieo/role/supervisor.json --listen 0.0.0.0:8080 >/dev/null
  for i in $(seq 1 50); do curl -s -o /dev/null -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$port/v0/role/status" && return; sleep 0.2; done
  echo "role $r did not come up"; podman logs "ieo-ct-$REPO-$r"; exit 1
}
api() { # port method path [body]
  curl -s -w '\n%{http_code}' -X "$2" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' ${4:+--data-binary "$4"} "http://127.0.0.1:$1$3"
}
body() { # attempt prompt extra-json
  python3 -c 'import json,sys; v={"attempt_id":sys.argv[1],"prompt":sys.argv[2],"coordinator":{"generation":0,"assignment_id":"A1","sprint_id":"POCX","sprint_revision":3,"spec_digest":"dg","image_digest":"golang:1.26.9-bookworm","profile_id":"dev","profile_revision":1},"limits":{"timeout_ms":30000,"billable_room":1000000,"cache_read_room":1000000,"grace_ms":2000}}; v.update(json.loads(sys.argv[3])); print(json.dumps(v))' "$1" "$2" "$3"
}
wait_done() { # port attempt
  for i in $(seq 1 150); do
    s=$(curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$1/v0/role/status")
    if python3 -c 'import json,sys; s=json.loads(sys.argv[1]); sys.exit(0 if s["running"] is None and (s["last"] or {}).get("attempt_id")==sys.argv[2] else 1)' "$s" "$2"; then
      python3 -c 'import json,sys; l=json.loads(sys.argv[1])["last"]; print(json.dumps({k:l.get(k) for k in ["attempt_id","session_id","mode","stop","exit_code","stopped_by_guard","cleanup_failed","tamper"]}))' "$s"; return; fi
    sleep 0.2
  done; echo "timeout waiting for $2"; exit 1
}
init_of() { # port attempt
  curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$1/v0/role/runs/$2/stdout" | python3 -c 'import json,sys; print(json.loads(sys.stdin.read())["content"].splitlines()[0])'
}
echo "# 02-04 role containers, fake claude (no model calls) · $(date -u +%FT%TZ) · image $IMG"
setup_role development 20001; setup_role guardrail 20002
run_role development 20001 58201; run_role guardrail 20002 58202
D=58201; G=58202
echo "## U3 layout"
for r in development guardrail; do
  c="ieo-ct-$REPO-$r"
  echo "$r: pid1=$(podman exec $c cat /proc/1/cmdline | tr '\0' ' ' | cut -c1-60) uid=$(podman exec $c id -u) labels=$(podman inspect $c --format '{{index .Config.Labels "ieo.role"}}')"
  echo "  mounts: $(podman inspect $c --format '{{range .Mounts}}{{.Destination}}({{if .RW}}rw{{else}}ro{{end}}) {{end}}')"
done
echo "  shared host paths between roles: $(comm -12 <(podman inspect ieo-ct-$REPO-development --format '{{range .Mounts}}{{.Source}}{{"\n"}}{{end}}' | sort) <(podman inspect ieo-ct-$REPO-guardrail --format '{{range .Mounts}}{{.Source}}{{"\n"}}{{end}}' | sort) | grep -v '/bin$' | wc -l) (the read-only program dir /opt/ieo excluded)"
echo "## adapter auth"
echo "no token: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$D/v0/role/status)"
echo "## (a) cleanup before the first start: markers planted in the config dir"
podman unshare sh -c "mkdir -p '$W/development/state/claude/skills/x' && echo MARKER > '$W/development/state/claude/CLAUDE.md' && echo MARKER > '$W/development/state/claude/skills/x/SKILL.md' && echo '{}' > '$W/development/state/claude/.credentials.json' && chown -R 20001:20001 '$W/development/state/claude'"
api $D POST /v0/role/start "$(body d1 hello '{"session_id":"s-dev-1"}')" | tail -1
wait_done $D d1; echo "init: $(init_of $D d1)"
echo "## (c) reconnect without changes resumes the same session"
api $D POST /v0/role/reconnect "$(body d2 more '{"new_session_id":"s-dev-unused"}')" | tail -1
wait_done $D d2
echo "## persistent volume: container restart, then reconnect resumes from the stored checkpoint"
podman restart ieo-ct-$REPO-development >/dev/null; for i in $(seq 1 50); do curl -s -o /dev/null -H "Authorization: Bearer $TOKEN" http://127.0.0.1:$D/v0/role/status && break; sleep 0.2; done
echo "after restart: pid1=$(podman exec ieo-ct-$REPO-development cat /proc/1/cmdline | tr '\0' ' ' | cut -c1-40) checkpoint session=$(curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:$D/v0/role/status | python3 -c 'import json,sys; print(json.load(sys.stdin)["checkpoint"]["session_id"])')"
api $D POST /v0/role/reconnect "$(body d3 more '{"new_session_id":"s-dev-unused2"}')" | tail -1
wait_done $D d3
echo "## (b) tamper: a skill planted after the checkpoint"
podman unshare sh -c "mkdir -p '$W/development/state/claude/skills/evil' && echo MARKER > '$W/development/state/claude/skills/evil/SKILL.md' && chown -R 20001:20001 '$W/development/state/claude/skills'"
api $D POST /v0/role/reconnect "$(body d4 more '{"new_session_id":"s-dev-2"}')" | tail -1
wait_done $D d4
echo "## single run per session: a reconnect while running answers with status"
api $D POST /v0/role/reconnect "$(body d5 SLEEP '{"new_session_id":"s-dev-x"}')" | tail -1
sleep 1
api $D POST /v0/role/reconnect "$(body d6 more '{"new_session_id":"s-dev-y"}')" | python3 -c 'import sys; t=sys.stdin.read().rsplit("\n",1); import json; print(t[1], json.loads(t[0])["code"])'
echo "## stop"
api $D POST /v0/role/stop | tail -1
wait_done $D d5
sleep 1
echo "processes left in the container besides PID 1 (incl. zombies): $(podman exec ieo-ct-$REPO-development ps -eo pid=,stat=,args= | awk '$1!=1 && $3!="ps"' | wc -l)"
echo "## guardrail role: its own volume, uid and arguments"
api $G POST /v0/role/start "$(body g1 review '{"session_id":"s-gr-1"}')" | tail -1
wait_done $G g1; echo "init: $(init_of $G g1)"
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:$G/v0/role/runs/g1/outcome | python3 -c 'import json,sys; o=json.loads(json.load(sys.stdin)["content"]); a=o["argv"]; i=a.index("--allowedTools"); print("allowedTools", a[i+1:i+5]); print("Write( in argv:", any("Write(" in x for x in a)); print("env:", o["env_names"])'
echo "## credential values in the logs: $(podman logs ieo-ct-$REPO-development 2>&1 | grep -c 'fake-development-credential' || true) occurrences"
echo "## graceful stop: SIGTERM to PID 1"
for r in development guardrail; do
  t0=$(date +%s.%N); podman stop -t 10 ieo-ct-$REPO-$r >/dev/null; t1=$(date +%s.%N)
  echo "$r: podman stop took $(python3 -c "print(round($t1-$t0,1))") s, exit code $(podman inspect ieo-ct-$REPO-$r --format '{{.State.ExitCode}}')"
done
podman rm -f ieo-ct-$REPO-development ieo-ct-$REPO-guardrail >/dev/null
echo "# done"
