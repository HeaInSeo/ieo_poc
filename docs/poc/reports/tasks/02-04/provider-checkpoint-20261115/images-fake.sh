#!/bin/bash
# 02-04 decision items 4 and 9 in containers, no model call.
# Usage: images-fake.sh <tag> <scratch workdir>
# A separate test environment like images-20261115/images-fake.sh: its own podman network, a
# throwaway PostgreSQL (digest-pinned, random password, tmpfs), the coordinator image (UID 20000,
# PID 1) and the two role images (UID 20001/20002, PID 1, fake claude). New here: an executable
# sprint and current assignments in the coordinator, the role supervisors report every
# checkpoint to it (checkpoint:report credentials), the coordinator compares and judges resume,
# and the provider status / shared quota group is read through the coordinator — before and
# after restarting the coordinator container and a role container.
set -uo pipefail
TAG=$1; W=$(realpath -m "$2")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../../../../.." && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
NET=ieo-img-net; PG=docker.io/library/postgres@sha256:665dc7b37ac52a17e298511414c21d23ffa4e6a6ef6225b3e5d59c7afb35e994
SHA=1111111111111111111111111111111111111111; GROUP=anthropic-subscription-shared-0104
if [ -e "$W" ] && [ ! -e "$W/.images-fake" ]; then echo "$W is not this script's workdir"; exit 1; fi
cleanup() {
  for c in ieo-img-coordinator ieo-img-development ieo-img-guardrail ieo-img-db; do podman rm -f "$c" >/dev/null 2>&1; done
  podman secret rm ieo-img-db-url >/dev/null 2>&1; podman network rm "$NET" >/dev/null 2>&1
}
cleanup
trap 'cleanup; podman unshare rm -rf "$W/development/cred" "$W/guardrail/cred"; rm -f "$W/adapter.header" "$W/coord-etc/"*.header "$W/coord-etc/adapter.token"' EXIT
podman unshare rm -rf "$W"; mkdir -p "$W/coord-etc" "$W/fake"; touch "$W/.images-fake"
rnd() { head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
echo "# 02-04 items 4 and 9 in containers, no model call · $(date -u +%FT%TZ) · tag $TAG"
for i in coordinator role-development role-guardrail; do
  echo "$i: $(podman image inspect --format 'id={{.Id}} user={{.Config.User}} revision={{index .Labels "org.opencontainers.image.revision"}} binary={{index .Labels "ieo.binary.ieo-supervisor.sha256"}}{{index .Labels "ieo.binary.ieo-coord.sha256"}}' localhost/ieo-poc/$i:$TAG)"
done
podman network create "$NET" >/dev/null
PGPW=$(rnd)
podman run -d --name ieo-img-db --network "$NET" --tmpfs /var/lib/postgresql/data -e POSTGRES_USER=ieo -e POSTGRES_PASSWORD="$PGPW" -e POSTGRES_DB=ieo "$PG" >/dev/null
for _ in $(seq 1 100); do podman exec ieo-img-db pg_isready -U ieo -d ieo >/dev/null 2>&1 && break; sleep 0.2; done
sleep 1
printf 'host=ieo-img-db port=5432 user=ieo dbname=ieo password=%s' "$PGPW" | podman secret create ieo-img-db-url - >/dev/null
# credentials: each principal's token goes to a header file (removed at exit); the coordinator
# keeps digests only
declare -A TOK
for n in operator alice bob assigner sup-development sup-guardrail; do TOK[$n]=$(rnd); ( umask 077; printf 'Authorization: Bearer %s\n' "${TOK[$n]}" > "$W/coord-etc/$n.header" ); done
$PY - "$W/coord-etc/tokens.json" "${TOK[operator]}" "${TOK[alice]}" "${TOK[bob]}" "${TOK[assigner]}" "${TOK[sup-development]}" "${TOK[sup-guardrail]}" <<'PYEOF'
import hashlib, json, sys
h = lambda t: hashlib.sha256(t.encode()).hexdigest()
o, a, b, s, d, g = sys.argv[2:]
json.dump({"principals": [
  {"name": "operator", "token_sha256": h(o), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["img"]},
  {"name": "alice", "token_sha256": h(a), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["img"]},
  {"name": "bob", "token_sha256": h(b), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["img"]},
  {"name": "assigner", "token_sha256": h(s), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["img"]},
  {"name": "sup-development", "token_sha256": h(d), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["img"], "role": "development"},
  {"name": "sup-guardrail", "token_sha256": h(g), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["img"], "role": "guardrail"}]},
  open(sys.argv[1], "w"))
PYEOF
TOKEN=$(rnd)
( umask 077; printf 'Authorization: Bearer %s\n' "$TOKEN" > "$W/adapter.header"; printf '%s' "$TOKEN" > "$W/coord-etc/adapter.token" )
echo "{\"img\": {\"development\": {\"url\": \"http://ieo-img-development:8080\", \"adapter_token_file\": \"/etc/ieo/coord/adapter.token\", \"quota_group_id\": \"$GROUP\"}, \"guardrail\": {\"url\": \"http://ieo-img-guardrail:8080\", \"adapter_token_file\": \"/etc/ieo/coord/adapter.token\", \"quota_group_id\": \"$GROUP\"}}}" > "$W/coord-etc/roles.json"
cp "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" "$ROOT/contracts/judgment-policy-v0.json" "$W/coord-etc/"
# an executable sprint for repo img (the manifest's first sprint, digest as ieo_spec::spec_digest)
$PY - "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" "$W/coord-etc/spec.json" "$W/coord-etc/review.json" <<'PYEOF'
import hashlib, json, sys
m = json.load(open(sys.argv[1]))
s = m["sprints"][0]
s["registration_state"] = "ready"
s["target"] = {"repo_id": "img", "repository_url": "https://example.invalid/img"}
s["limits"] = {"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 3, "unresolved_reason": None}
s["unresolved_bindings"] = []
s["revision"] = 5
s["tasks"][0]["outputs"] = [{"artifact_id": "01-01-output", "description": "required output", "required": True}]
s.pop("digest", None)
s["digest"] = hashlib.sha256(json.dumps(s, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
json.dump(s, open(sys.argv[2], "w"), ensure_ascii=False)
json.dump({"decision": "passed", "reason": "container test", "digest": s["digest"]}, open(sys.argv[3], "w"))
PYEOF
SD=$($PY -c 'import json,sys; print(json.load(open(sys.argv[1]))["digest"])' "$W/coord-etc/spec.json")
for r in development guardrail; do
  echo "{\"repo_id\":\"img\",\"role\":\"$r\",\"assignment_id\":\"A-$r\",\"generation\":0,\"sprint_id\":\"POC-01\",\"sprint_revision\":5,\"task_id\":\"01-01\",\"target_sha\":\"$SHA\",\"expected_current\":null}" > "$W/coord-etc/assign-$r.json"
done
chmod 0644 "$W/coord-etc/"*
echo "## coordinator container (UID 20000, PID 1, its own test DB)"
start_coord() {
  podman run -d --name ieo-img-coordinator --network "$NET" --secret ieo-img-db-url,type=mount,uid=20000,mode=0400 -e IEO_DB_URL_FILE=/run/secrets/ieo-img-db-url \
    --cap-drop ALL --security-opt no-new-privileges --stop-timeout 15 -v "$W/coord-etc:/etc/ieo/coord:ro,z" \
    "localhost/ieo-poc/coordinator:$TAG" --roles /etc/ieo/coord/roles.json >/dev/null
  for _ in $(seq 1 50); do podman logs ieo-img-coordinator 2>&1 | grep -q listening && break; sleep 0.2; done
}
start_coord
echo "pid1: $(podman exec ieo-img-coordinator cat /proc/1/cmdline | tr '\0' ' ' | cut -c1-40) uid: $(podman exec ieo-img-coordinator id -u)"
cc() { podman exec ieo-img-coordinator curl -s -w ' %{http_code}' -X "$2" -H @"/etc/ieo/coord/$1.header" -H 'Content-Type: application/json' ${4:+-H "Idempotency-Key: img-0001"} ${5:+--data-binary "@/etc/ieo/coord/$5"} "http://127.0.0.1:8090$3"; }
code() { echo "$1" | awk '{print $NF}'; }
echo "register $(code "$(cc alice POST '/v0/sprints?mode=executable' k spec.json)") · semantic review $(code "$(cc bob POST /v0/sprints/POC-01/revisions/5/semantic-review '' review.json)") · assign development $(code "$(cc assigner POST /v0/coordinator/assignments '' assign-development.json)") guardrail $(code "$(cc assigner POST /v0/coordinator/assignments '' assign-guardrail.json)")"
# role containers report to the coordinator over the network
cat > "$W/fake/fake-claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
printf '{"type":"system","subtype":"init","session_id":"%s"}\n' "$sid"
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
case "$prompt" in *LIMIT*) printf '{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","overageStatus":"rejected","isUsingOverage":false}}\n';; esac
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"modelUsage":{"m":{"inputTokens":1,"outputTokens":1,"cacheReadInputTokens":0,"cacheCreationInputTokens":0}}}\n' "$sid"
FAKE
chmod 755 "$W/fake/fake-claude"
declare -A UIDS=([development]=20001 [guardrail]=20002) PORTS=([development]=58231 [guardrail]=58232)
for r in development guardrail; do
  mkdir -p "$W/$r/etc/template" "$W/$r/state" "$W/$r/cred"
  echo '{"autoMemoryEnabled": false}' | tee "$W/$r/etc/template/settings.json" > "$W/$r/etc/settings.json"
  echo '{"mcpServers": {}}' > "$W/$r/etc/mcp.json"; echo '{}' > "$W/$r/etc/agents.json"
  printf '{"role":"%s","repo":"img","state":"/state","etc":"/etc/ieo/role","cred":"/run/ieo/cred","claude":"/opt/ieo-fake/fake-claude","budget":"02-04","quota_group_id":"%s","quota_group_basis":"01-04 P3: shared (same account, same window resets)","checkpoint_central":{"url":"http://ieo-img-coordinator:8090","token_file":"/run/ieo/cred/coordinator_token"},"share":{"max_runs":4,"total_seconds":200,"billable_tokens":40,"cache_read_tokens":40}}\n' "$r" "$GROUP" > "$W/$r/etc/supervisor.json"
  echo "sk-ant-oat01-fake-not-a-credential" > "$W/$r/cred/oauth_token"
  printf '%s' "$TOKEN" | sha256sum | cut -d' ' -f1 > "$W/$r/cred/adapter.sha256"
  printf '%s' "${TOK[sup-$r]}" > "$W/$r/cred/coordinator_token"
  chmod 0400 "$W/$r/cred/"*; chmod 0500 "$W/$r/cred"
  podman unshare chown -R "${UIDS[$r]}:${UIDS[$r]}" "$W/$r/state" "$W/$r/cred"
  podman run -d --name "ieo-img-$r" --network "$NET" --cap-drop ALL --security-opt no-new-privileges --stop-timeout 15 \
    -v "$W/$r/state:/state:z" -v "$W/$r/etc:/etc/ieo/role:ro,z" -v "$W/$r/cred:/run/ieo/cred:ro,z" -v "$W/fake:/opt/ieo-fake:ro,z" \
    -p "127.0.0.1:${PORTS[$r]}:8080" "localhost/ieo-poc/role-$r:$TAG" >/dev/null
done
wait_role() { for _ in $(seq 1 50); do curl -sf -o /dev/null -H @"$W/adapter.header" "http://127.0.0.1:${PORTS[$1]}/v0/role/status" && break; sleep 0.2; done; }
for r in development guardrail; do wait_role $r; done
body() { $PY -c 'import json,sys; print(json.dumps({"attempt_id":sys.argv[1],"prompt":sys.argv[2],"target_sha":sys.argv[6],"coordinator":{"generation":0,"assignment_id":"A-"+sys.argv[5],"sprint_id":"POC-01","sprint_revision":5,"spec_digest":sys.argv[7],"image_digest":"i","profile_id":"p","profile_revision":1},"limits":{"timeout_ms":20000,"billable_room":5,"cache_read_room":5,"grace_ms":2000},sys.argv[3]:sys.argv[4]}))' "$@" "$SHA" "$SD"; }
api() { curl -s -w ' %{http_code}' -X "$2" -H @"$W/adapter.header" -H 'Content-Type: application/json' ${4:+--data-binary "$4"} "http://127.0.0.1:${PORTS[$1]}$3"; }
waitrun() { for _ in $(seq 1 100); do curl -s -H @"$W/adapter.header" "http://127.0.0.1:${PORTS[$1]}/v0/role/status" | $PY -c 'import json,sys; sys.exit(0 if json.load(sys.stdin)["running"] is None else 1)' && break; sleep 0.2; done; }
judge() { cc operator GET "/v0/repos/img/roles/$1/checkpoints/compare" | sed 's/ [0-9]*$//' | $PY -c 'import json,sys; b=json.load(sys.stdin); print(b["decision"], b["reasons"])'; }
latest() { cc operator GET "/v0/repos/img/roles/$1/checkpoints/latest" | sed 's/ [0-9]*$//' | $PY -c 'import json,sys; b=json.load(sys.stdin); print("seq", b.get("seq"), "phase", b.get("phase"), "session", b.get("session_id"), "generation", b.get("generation"), "target", (b.get("target_sha") or "")[:7], "tamper_events", b.get("tamper_events"))'; }
groups() { cc operator GET /v0/repos/img/roles | sed 's/ [0-9]*$//' | $PY -c 'import json,sys; b=json.load(sys.stdin); q=b["quota_groups"]; print({g["quota_group_id"]: (g["provider_status_or_unknown"], g["affected_roles"]) for g in q["groups"]}, "mismatches", q["mismatches"]); print("  per role:", {r: ((v.get("status") or {}).get("quota_group_id"), (v.get("status") or {}).get("provider_status_or_unknown")) for r,v in b["roles"].items()})'; }
echo "## item 9: checkpoints reported, stored and judged"
echo "before any run: development $(judge development) · guardrail $(judge guardrail)"
echo "development start: $(code "$(api development POST /v0/role/start "$(body d1 hello session_id s-dev-1 development)")") · guardrail start: $(code "$(api guardrail POST /v0/role/start "$(body g1 hello session_id s-gr-1 guardrail)")")"
waitrun development; waitrun guardrail
for r in development guardrail; do echo "$r central: $(latest $r) · judgment: $(judge $r)"; done
echo "## item 4: provider status and the shared quota group, through the coordinator"
groups
echo "## restart the coordinator container (its DB persists)"
podman stop ieo-img-coordinator >/dev/null; podman rm ieo-img-coordinator >/dev/null; start_coord
for r in development guardrail; do echo "$r after coordinator restart: $(judge $r)"; done
echo "## restart the development role container (PID 1 again)"
podman restart ieo-img-development >/dev/null; wait_role development
curl -s -H @"$W/adapter.header" "http://127.0.0.1:${PORTS[development]}/v0/role/status" | $PY -c 'import json,sys; s=json.load(sys.stdin); print("development after restart: provider", s["provider_status_or_unknown"], "· before start", (s.get("provider_status_before_start") or {}).get("observation",{}).get("value"), "current", (s.get("provider_status_before_start") or {}).get("current"), "· checkpoint seq", s["checkpoint_seq"])'
echo "development judgment after its restart: $(judge development)"
echo "development reconnect: $(api development POST /v0/role/reconnect "$(body d2 LIMIT x x development)" | $PY -c 'import sys; t=sys.stdin.read(); import json; b=json.loads(t.rsplit(" ",1)[0]); print(t.rsplit(" ",1)[1], "mode", (b.get("accepted") or {}).get("mode"), "central", (b.get("accepted") or {}).get("central"))')"
waitrun development
echo "development central: $(latest development) · judgment: $(judge development)"
echo "after development saw a rejected window:"; groups
echo "## a changed local checkpoint is refused by both sides"
podman unshare sh -c "sed -i 's/\"last_event_offset\": [0-9]*/\"last_event_offset\": 999/' '$W/development/state/checkpoint.json'"
echo "judgment: $(judge development)"
echo "development reconnect: $(api development POST /v0/role/reconnect "$(body d3 hello x x development)" | sed 's/.*"code":"\([A-Z_]*\)".* \([0-9]*\)$/\2 \1/')"
echo "## SIGTERM to PID 1"
for c in coordinator development guardrail; do
  t0=$(date +%s.%N); podman stop "ieo-img-$c" >/dev/null; t1=$(date +%s.%N)
  echo "$c: podman stop $($PY -c "print(round($t1-$t0,1))") s, exit $(podman inspect ieo-img-$c --format '{{.State.ExitCode}}')"
done
echo "rows in the coordinator DB: checkpoints $(podman exec ieo-img-db psql -U ieo -d ieo -tAc 'select count(*) from role_checkpoints') · tamper events $(podman exec ieo-img-db psql -U ieo -d ieo -tAc 'select count(*) from role_tamper_events')"
cleanup
echo "# done"
