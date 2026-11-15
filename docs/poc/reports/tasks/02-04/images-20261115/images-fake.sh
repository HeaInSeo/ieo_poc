#!/bin/bash
# 02-04 coordinator and role images in containers, no model call.
# Usage: images-fake.sh <tag> <scratch workdir>
# A separate test environment: its own podman network, a throwaway PostgreSQL (digest-pinned,
# random password, tmpfs), the coordinator image (UID 20000, PID 1, DB URL from a mounted secret
# file) and the two role images (UID 20001/20002, PID 1, fake claude). The coordinator controls
# the roles over the network: repo-scope status and stop (runtime contract §7).
set -uo pipefail
TAG=$1; W=$(realpath -m "$2")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../../../../.." && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
NET=ieo-img-net; PG=docker.io/library/postgres@sha256:665dc7b37ac52a17e298511414c21d23ffa4e6a6ef6225b3e5d59c7afb35e994
if [ -e "$W" ] && [ ! -e "$W/.images-fake" ]; then echo "$W is not this script's workdir"; exit 1; fi
cleanup() {
  for c in ieo-img-coordinator ieo-img-development ieo-img-guardrail ieo-img-db; do podman rm -f "$c" >/dev/null 2>&1; done
  podman secret rm ieo-img-db-url >/dev/null 2>&1; podman network rm "$NET" >/dev/null 2>&1
}
cleanup
trap 'cleanup; rm -f "$W/adapter.header" "$W/coord-etc/adapter.token" "$W/coord-etc/operator.header"' EXIT
podman unshare rm -rf "$W"; mkdir -p "$W/coord-etc" "$W/fake"; touch "$W/.images-fake"
echo "# 02-04 images, no model call · $(date -u +%FT%TZ) · tag $TAG"
echo "## images (labels, user, binaries inside)"
for i in coordinator role-development role-guardrail; do
  echo "$i: $(podman image inspect --format 'id={{.Id}} digest={{.Digest}} user={{.Config.User}} revision={{index .Labels "org.opencontainers.image.revision"}}' localhost/ieo-poc/$i:$TAG)"
done
lab() { podman image inspect --format "{{index .Labels \"$2\"}}" "localhost/ieo-poc/$1:$TAG"; }
in_img() { podman run --rm --user 0 --entrypoint sha256sum "localhost/ieo-poc/$1:$TAG" "$2" | cut -c1-64; }
[ "$(in_img coordinator /usr/local/bin/ieo-coord)" = "$(lab coordinator ieo.binary.ieo-coord.sha256)" ] && echo "coordinator: ieo-coord in image = label" || echo "coordinator: ieo-coord MISMATCH"
for r in development guardrail; do
  [ "$(in_img role-$r /usr/local/bin/ieo-supervisor)" = "$(lab role-$r ieo.binary.ieo-supervisor.sha256)" ] && [ "$(in_img role-$r /opt/claude/claude)" = "$(lab role-$r ieo.binary.claude.sha256)" ] \
    && echo "role-$r: supervisor and claude in image = labels" || echo "role-$r: binary MISMATCH"
done
podman network create "$NET" >/dev/null
# throwaway database
PGPW=$(head -c 18 /dev/urandom | od -An -tx1 | tr -d ' \n')
podman run -d --name ieo-img-db --network "$NET" --tmpfs /var/lib/postgresql/data -e POSTGRES_USER=ieo -e POSTGRES_PASSWORD="$PGPW" -e POSTGRES_DB=ieo "$PG" >/dev/null
for _ in $(seq 1 100); do podman exec ieo-img-db pg_isready -U ieo -d ieo >/dev/null 2>&1 && break; sleep 0.2; done
sleep 1
printf 'host=ieo-img-db port=5432 user=ieo dbname=ieo password=%s' "$PGPW" | podman secret create ieo-img-db-url - >/dev/null
# role containers
TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
( umask 077; printf 'Authorization: Bearer %s\n' "$TOKEN" > "$W/adapter.header"; printf '%s' "$TOKEN" > "$W/coord-etc/adapter.token" )
cat > "$W/fake/fake-claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
printf '{"type":"system","subtype":"init","session_id":"%s"}\n' "$sid"
case "$prompt" in *SLEEP*) sleep 60;; esac
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"modelUsage":{"m":{"inputTokens":1,"outputTokens":1,"cacheReadInputTokens":0,"cacheCreationInputTokens":0}}}\n' "$sid"
FAKE
chmod 755 "$W/fake/fake-claude"
declare -A UIDS=([development]=20001 [guardrail]=20002) PORTS=([development]=58231 [guardrail]=58232)
for r in development guardrail; do
  mkdir -p "$W/$r/etc/template" "$W/$r/state" "$W/$r/cred"
  echo '{"autoMemoryEnabled": false}' | tee "$W/$r/etc/template/settings.json" > "$W/$r/etc/settings.json"
  echo '{"mcpServers": {}}' > "$W/$r/etc/mcp.json"; echo '{}' > "$W/$r/etc/agents.json"
  printf '{"role":"%s","repo":"img","state":"/state","etc":"/etc/ieo/role","cred":"/run/ieo/cred","claude":"/opt/ieo-fake/fake-claude","budget":"02-04","share":{"max_runs":4,"total_seconds":200,"billable_tokens":40,"cache_read_tokens":40}}\n' "$r" > "$W/$r/etc/supervisor.json"
  echo "sk-ant-oat01-fake-not-a-credential" > "$W/$r/cred/oauth_token"
  printf '%s' "$TOKEN" | sha256sum | cut -d' ' -f1 > "$W/$r/cred/adapter.sha256"
  chmod 0400 "$W/$r/cred/"*; chmod 0500 "$W/$r/cred"
  podman unshare chown -R "${UIDS[$r]}:${UIDS[$r]}" "$W/$r/state" "$W/$r/cred"
  podman run -d --name "ieo-img-$r" --network "$NET" --cap-drop ALL --security-opt no-new-privileges --stop-timeout 15 \
    -v "$W/$r/state:/state:z" -v "$W/$r/etc:/etc/ieo/role:ro,z" -v "$W/$r/cred:/run/ieo/cred:ro,z" -v "$W/fake:/opt/ieo-fake:ro,z" \
    -p "127.0.0.1:${PORTS[$r]}:8080" "localhost/ieo-poc/role-$r:$TAG" >/dev/null
done
for r in development guardrail; do for _ in $(seq 1 50); do curl -sf -o /dev/null -H @"$W/adapter.header" "http://127.0.0.1:${PORTS[$r]}/v0/role/status" && break; sleep 0.2; done; done
# coordinator: an operator credential (roles:control for repo img) and the roles config
OPTOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
( umask 077; printf 'Authorization: Bearer %s\n' "$OPTOKEN" > "$W/coord-etc/operator.header" )
$PY -c 'import hashlib,json,sys; json.dump({"principals":[{"name":"operator","token_sha256":hashlib.sha256(sys.argv[1].encode()).hexdigest(),"kind":"service","scopes":["roles:control"],"repo_ids":["img"]}]}, open(sys.argv[2],"w"))' "$OPTOKEN" "$W/coord-etc/tokens.json"
echo '{"img": {"development": {"url": "http://ieo-img-development:8080", "adapter_token_file": "/etc/ieo/coord/adapter.token"}, "guardrail": {"url": "http://ieo-img-guardrail:8080", "adapter_token_file": "/etc/ieo/coord/adapter.token"}}}' > "$W/coord-etc/roles.json"
cp "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" "$ROOT/contracts/judgment-policy-v0.json" "$W/coord-etc/"
chmod 0644 "$W/coord-etc/"*
echo "## coordinator container (UID 20000, PID 1, its own test DB)"
podman run -d --name ieo-img-coordinator --network "$NET" --secret ieo-img-db-url,type=mount,uid=20000,mode=0400 -e IEO_DB_URL_FILE=/run/secrets/ieo-img-db-url \
  --cap-drop ALL --security-opt no-new-privileges --stop-timeout 15 -v "$W/coord-etc:/etc/ieo/coord:ro,z" \
  "localhost/ieo-poc/coordinator:$TAG" --roles /etc/ieo/coord/roles.json >/dev/null
for _ in $(seq 1 50); do podman logs ieo-img-coordinator 2>&1 | grep -q listening && break; sleep 0.2; done
echo "pid1: $(podman exec ieo-img-coordinator cat /proc/1/cmdline | tr '\0' ' ' | cut -c1-40) uid: $(podman exec ieo-img-coordinator id -u)"
echo "request without credential: $(podman exec ieo-img-coordinator curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8090/v0/repos/img/roles)"
echo "DB password in inspect env: $(podman inspect ieo-img-coordinator --format '{{range .Config.Env}}{{println .}}{{end}}' | grep -c 'password=' || true) · in argv: $(podman exec ieo-img-coordinator cat /proc/1/cmdline | tr '\0' ' ' | grep -c password || true) · in PID 1 environment: $(podman exec ieo-img-coordinator sh -c 'tr "\0" "\n" < /proc/1/environ' | grep -c 'password=' || true)"
echo "schema migrated (tables): $(podman exec ieo-img-db psql -U ieo -d ieo -tAc "select count(*) from information_schema.tables where table_schema='public'")"
coord() { podman exec ieo-img-coordinator curl -s -X "$1" -H @/etc/ieo/coord/operator.header "http://127.0.0.1:8090$2"; }
echo "## repo-scope control through the coordinator"
body() { $PY -c 'import json,sys; print(json.dumps({"attempt_id":sys.argv[1],"prompt":sys.argv[2],"coordinator":{"generation":0,"assignment_id":"A","sprint_id":"S","sprint_revision":1,"spec_digest":"d","image_digest":"i","profile_id":"p","profile_revision":1},"limits":{"timeout_ms":20000,"billable_room":5,"cache_read_room":5,"grace_ms":2000},sys.argv[3]:sys.argv[4]}))' "$@"; }
api() { curl -s -o /dev/null -w '%{http_code}' -X "$2" -H @"$W/adapter.header" -H 'Content-Type: application/json' ${4:+--data-binary "$4"} "http://127.0.0.1:$1$3"; }
echo "development start $(api ${PORTS[development]} POST /v0/role/start "$(body d1 hello session_id s-dev-1)") · guardrail start (long run) $(api ${PORTS[guardrail]} POST /v0/role/start "$(body g1 SLEEP session_id s-gr-1)")"
sleep 1.5
coord GET /v0/repos/img/roles | $PY -c 'import json,sys; b=json.load(sys.stdin); print("status via coordinator:", {r: (v.get("reachable"), (v.get("status") or {}).get("running") is not None) for r,v in b["roles"].items()}, "(reachable, running)")'
t0=$(date +%s.%N); out=$(coord POST /v0/repos/img/stop); t1=$(date +%s.%N)
echo "$out" | $PY -c 'import json,sys; b=json.load(sys.stdin); print("repo stop:", {r:(v.get("stop_http_status"), v.get("running_after"), (v.get("safe_shutdown") or {}).get("checkpoint_saved")) for r,v in b["roles"].items()}, "(stop status, running after, checkpoint saved)"); print("safe_shutdown_confirmed:", b["safe_shutdown_confirmed"]); [print("  unconfirmed:", u) for u in b["unconfirmed"]]'
echo "repo stop took $($PY -c "print(round($t1-$t0,1))") s"
podman unshare cat "$W/guardrail/state/runs/g1/outcome.json" | $PY -c 'import json,sys; o=json.load(sys.stdin); print("guardrail g1 outcome:", o.get("stop"), "cleanup_failed", o.get("cleanup_failed"))'
echo "processes left in the guardrail container besides PID 1: $(podman exec ieo-img-guardrail ps -eo pid=,args= | awk '$1!=1 && $2!="ps"' | wc -l)"
echo "## SIGTERM to PID 1"
for c in coordinator development guardrail; do
  t0=$(date +%s.%N); podman stop "ieo-img-$c" >/dev/null; t1=$(date +%s.%N)
  echo "$c: podman stop $($PY -c "print(round($t1-$t0,1))") s, exit $(podman inspect ieo-img-$c --format '{{.State.ExitCode}}')"
done
echo "coordinator log: $(podman logs ieo-img-coordinator 2>&1 | grep -E 'stopping|stopped' | tr '\n' ' ')"
cleanup
echo "# done"
