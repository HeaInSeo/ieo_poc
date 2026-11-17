#!/bin/bash
# Dry run of EXECUTION_PLAN.md §5.2–§6 end to end with a fake claude and fake credentials: the
# same run-step8-roles.sh, a TEST coordinator (built from this branch) on a new database of the
# test PostgreSQL (55432), the plan's sprint, principals, assignments, branch_head input and the
# deliveries R1 → R2 → G1. No model call, nothing of 58090/55433, no secrets, no run records.
# Usage: dry-run-step8.sh <dir with ieo-supervisor and ieo-coord> <scratch workdir>
# Env: TEST_DB_PASSWORD_FILE (the test database's password, never printed); optional
#      TEST_PG_CONTAINER (default ieo-poc-0203-pg), TEST_PG_PORT (default 55432), COORD_PORT (58290)
set -uo pipefail
BIN_DIR=$(realpath "$1"); W=$(realpath -m "$2")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../../../../../.." && pwd)
PGC=${TEST_PG_CONTAINER:-ieo-poc-0203-pg}; PGPORT=${TEST_PG_PORT:-55432}; CPORT=${COORD_PORT:-58290}
: "${TEST_DB_PASSWORD_FILE:?}"
[ "$PGPORT" = 55433 ] && { echo "refusing the persistent database port"; exit 1; }
[ "$CPORT" = 58090 ] && { echo "refusing the live coordinator port"; exit 1; }
if [ -e "$W" ] && [ ! -e "$W/fake/claude" ]; then echo "$W is not a dry-run workdir: refusing to delete it"; exit 1; fi
for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
podman unshare rm -rf "$W"; mkdir -p "$W/fake/secrets" "$W/tok" "$W/out"; chmod 700 "$W"
DB=ieo_t_s8dry_$$
cleanup() {
  [ -n "${CPID:-}" ] && kill -KILL "$CPID" 2>/dev/null
  for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
  podman exec "$PGC" dropdb -U ieo --if-exists --force "$DB" >/dev/null 2>&1 || true
}
trap cleanup EXIT
pass=0; fail=0
ok() { if [ "$1" = 0 ]; then pass=$((pass+1)); echo "PASS $2"; else fail=$((fail+1)); echo "FAIL $2"; fi; }
echo "# 02-05 step 8 dry run (fake claude, fake credentials, test coordinator on $PGPORT/$DB, no model call) · $(date -u +%FT%TZ)"
echo "# branch $(git -C "$ROOT" rev-parse --short HEAD) · supervisor $(sha256sum < "$BIN_DIR/ieo-supervisor" | cut -c1-12) · coordinator $(sha256sum < "$BIN_DIR/ieo-coord" | cut -c1-12)"
# fake credentials and fake claude (stream-json like the CLI: init, an assistant text, the no-overage
# signal, a measured result; session totals grow per session on the role volume)
for r in development guardrail; do echo "sk-ant-oat01-fake-$r-not-a-credential" > "$W/fake/secrets/$r.token"; chmod 600 "$W/fake/secrets/$r.token"; done
cat > "$W/fake/claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 65536)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
f="/state/fake-cum-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10))
printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"%s","tools":["Read","Grep","Glob"],"memory_paths":[],"skills":[]}\n' "$sid" "$(pwd)"
printf '{"type":"assistant","message":{"id":"m-%s-%s","content":[{"type":"text","text":"fake answer"}],"usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "$sid" "$i"
echo "$i $o $r $c" > "$f"
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","result":"fake answer","usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20},"modelUsage":{"m":{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}},"total_cost_usd":0.01}\n' "$sid" "$i" "$o" "$r" "$c"
FAKE
chmod 755 "$W/fake/claude"
echo '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}' > "$W/fake/first.jsonl"
# principals (EXECUTION_PLAN.md §2): token files 0600, only their SHA-256 in tokens.json
mk() { ( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > "$W/tok/$1.token"
         printf 'Authorization: Bearer %s\n' "$(cat "$W/tok/$1.token")" > "$W/tok/$1.header" ); }
dg() { printf '%s' "$(cat "$W/tok/$1.token")" | sha256sum | cut -d' ' -f1; }
for p in s8-registrant user-dry s8-assigner s8-operator s8-ingest s8-sup-dev s8-sup-gr; do mk "$p"; done
cat > "$W/tokens.json" <<JSON
{"principals": [
 {"name": "s8-registrant", "token_sha256": "$(dg s8-registrant)", "kind": "service", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["ieo-poc-test"]},
 {"name": "user-dry", "token_sha256": "$(dg user-dry)", "kind": "user", "scopes": ["review:semantic", "sprints:read", "roles:control"], "repo_ids": ["ieo-poc-test"]},
 {"name": "s8-assigner", "token_sha256": "$(dg s8-assigner)", "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["ieo-poc-test"]},
 {"name": "s8-operator", "token_sha256": "$(dg s8-operator)", "kind": "service", "scopes": ["roles:control"], "repo_ids": ["ieo-poc-test"]},
 {"name": "s8-ingest", "token_sha256": "$(dg s8-ingest)", "kind": "service", "scopes": ["evidence:ingest"], "repo_ids": ["ieo-poc-test"]},
 {"name": "s8-sup-dev", "token_sha256": "$(dg s8-sup-dev)", "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["ieo-poc-test"], "role": "development"},
 {"name": "s8-sup-gr", "token_sha256": "$(dg s8-sup-gr)", "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["ieo-poc-test"], "role": "guardrail"}]}
JSON
# the role containers (the real launcher, fake claude and credentials, test ports)
CLAUDE_BIN="$W/fake/claude" SECRETS_DIR="$W/fake/secrets" NAME_PREFIX=ieo-s8dry QUOTA_GROUP_ID=dry-run-quota \
  SUP_DEV_TOKEN_FILE="$W/tok/s8-sup-dev.token" SUP_GR_TOKEN_FILE="$W/tok/s8-sup-gr.token" COORD_PORT="$CPORT" \
  bash "$HERE/run-step8-roles.sh" "$BIN_DIR" "$W/run"; ok $? "run-step8-roles.sh starts both role containers"
mkdir -p "$W/ledger-0205"
python3 -I - "$W" "$(($(date +%s) - 60))" <<'PY'
import json, sys
w, at = sys.argv[1], int(sys.argv[2])
e = json.load(open(w + "/run/roles-entry.json"))
e["budget_plan"] = {"budget": "02-05", "ledger_dir": w + "/ledger-0205",
                    "first_observation": {"stream": w + "/fake/first.jsonl", "observed_unix": at},
                    "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}
json.dump({"ieo-poc-test": e}, open(w + "/roles.json", "w"), indent=1)
PY
# the test coordinator on a new test database (never 55433, never 58090)
podman exec "$PGC" createdb -U ieo "$DB"; ok $? "test database $DB created on $PGPORT"
IEO_DB_URL="host=127.0.0.1 port=$PGPORT user=ieo dbname=$DB password=$(cat "$TEST_DB_PASSWORD_FILE")" \
  "$BIN_DIR/ieo-coord" serve --listen "127.0.0.1:$CPORT" --tokens "$W/tokens.json" --roles "$W/roles.json" \
  --manifest "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" --policy "$ROOT/contracts/judgment-policy-v0.json" \
  2> "$W/out/coordinator.log" &
CPID=$!
for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$CPORT/v0/health" && break; sleep 0.2; done
st=$(curl -s -o "$W/out/health.json" -w '%{http_code}' "http://127.0.0.1:$CPORT/v0/health"); [ "$st" = 200 ]; ok $? "test coordinator healthy ($st), coordinator listens on 127.0.0.1 only"
# one request: who method path [body file] [idempotency key] → status, body in $W/out/<tag>.json
req() { local tag=$1 who=$2 m=$3 p=$4 body=${5:-} key=${6:-}
  # the header from a 0600 file: the token is in no argv and never printed
  local args=(-s -o "$W/out/$tag.json" -w '%{http_code}' --max-time 150 -X "$m" -H @"$W/tok/$who.header")
  [ -n "$body" ] && args+=(-H 'Content-Type: application/json' --data @"$body")
  [ -n "$key" ] && args+=(-H "Idempotency-Key: $key")
  curl "${args[@]}" "http://127.0.0.1:$CPORT$p"; }
j() { python3 -I -c "import json,sys; d=json.load(open(sys.argv[1])); print(eval(sys.argv[2]))" "$W/out/$1.json" "$2"; }
SPEC="$HERE/../sprint-POC-S8-01-r1.json"
st=$(req reg s8-registrant POST '/v0/sprints?mode=executable' "$SPEC" s8-reg-0001); [ "$st" = 201 ]; ok $? "sprint POC-S8-01 r1 registered ($st)"
DIG=$(j reg "d.get('digest') or d.get('revision',{}).get('digest') or d.get('registration',{}).get('digest')")
echo "{\"decision\": \"passed\", \"reason\": \"dry run\", \"digest\": \"$DIG\"}" > "$W/out/review-body.json"
st=$(req review user-dry POST /v0/sprints/POC-S8-01/revisions/1/semantic-review "$W/out/review-body.json"); [ "$st" = 201 ]; ok $? "semantic review by the user principal ($st)"
TSHA=2ab9d8760012516f37674a9446ca2fb2699973c0
asg() { echo "{\"repo_id\": \"ieo-poc-test\", \"role\": \"$1\", \"assignment_id\": \"$2\", \"generation\": 0, \"sprint_id\": \"POC-S8-01\", \"sprint_revision\": 1, \"task_id\": \"90-01\", \"target_sha\": \"$TSHA\", \"expected_current\": null, \"limits\": {\"elapsed_seconds\": $3, \"total_tokens\": $4}$5}" > "$W/out/asg-$1.json"; }
asg development S8-DEV-A1 1800 150000 ', "branch_ref": "refs/heads/main"'; asg guardrail S8-GR-A1 600 50000 ''
st=$(req asg-dev s8-assigner POST /v0/coordinator/assignments "$W/out/asg-development.json" s8-asg-dev-0001); [ "$st" = 201 ]; ok $? "development assignment S8-DEV-A1 ($st)"
st=$(req asg-gr s8-assigner POST /v0/coordinator/assignments "$W/out/asg-guardrail.json" s8-asg-gr-0001); [ "$st" = 201 ]; ok $? "guardrail assignment S8-GR-A1 ($st)"
echo "{\"evidence_id\": \"s8-gh-head-1\", \"repo_id\": \"ieo-poc-test\", \"kind\": \"branch_head\", \"sha\": \"$TSHA\", \"ref\": \"refs/heads/main\", \"source_ref\": \"dry-run fixture\"}" > "$W/out/gh.json"
st=$(req gh s8-ingest POST /v0/coordinator/evidence "$W/out/gh.json"); [ "$st" = 201 ]; ok $? "branch_head input ($st)"
role_wait() { local port=$1 a=$2; for _ in $(seq 1 150); do
    curl -s -H @"$W/run/adapter.header" "http://127.0.0.1:$port/v0/role/status" > "$W/out/status-$a.json"
    j "status-$a" "d['running'] is None and (d.get('last') or {}).get('attempt_id') == '$a'" | grep -q True && return 0; sleep 0.2; done; return 1; }
settled() { for _ in $(seq 1 300); do [ -f "$W/ledger-0205/records.jsonl" ] && grep -q "\"attempt_id\":\"$1\"" "$W/ledger-0205/records.jsonl" && [ ! -e "$W/ledger-0205/inflight.json" ] && return 0; sleep 0.2; done; return 1; }
dlv() { local tag=$1 role=$2 aid=$3 att=$4 key=$5 sid; sid=$(cat /proc/sys/kernel/random/uuid)
  echo "{\"assignment_id\": \"$aid\", \"generation\": 0, \"attempt_id\": \"$att\", \"session_id\": \"$sid\", \"prompt\": \"dry run $att\"}" > "$W/out/$tag-body.json"
  req "$tag" s8-operator POST "/v0/repos/ieo-poc-test/roles/$role/deliveries" "$W/out/$tag-body.json" "$key"; }
for step in "r1 development S8-DEV-A1 s8-r1 s8-dlv-r1-0001 58221 no_checkpoint start" \
            "r2 development S8-DEV-A1 s8-r2 s8-dlv-r2-0001 58221 resume_allowed reconnect" \
            "g1 guardrail S8-GR-A1 s8-g1 s8-dlv-g1-0001 58222 no_checkpoint start"; do
  set -- $step
  st=$(dlv "$1" "$2" "$3" "$4" "$5")
  got=$(j "$1" "(d['result']['decision'], d['result']['op'], d['result']['outcome'])")
  [ "$st" = 201 ] && [ "$got" = "('$7', '$8', 'accepted')" ]; ok $? "$4: $got"
  role_wait "$6" "$4"; ok $? "$4: the role finished the run"
  settled "$4"; ok $? "$4: ledger record written, no in-flight marker"
done
python3 -I - "$W" <<'PY'
import json, sys
w = sys.argv[1]
recs = [json.loads(l) for l in open(w + "/ledger-0205/records.jsonl") if l.strip()]
by = {r["attempt_id"]: r for r in recs}
ok = lambda c, m: print(("PASS " if c else "FAIL ") + m)
ok(sorted(by) == ["s8-g1", "s8-r1", "s8-r2"] and all(r["budget"] == "02-05" for r in recs), "three 02-05 ledger records")
ok(not any(r["stop_required"] for r in recs), "none asks for a person")
ok(all(r["usage"]["run"]["state"] == "measured" for r in recs), "every run measured")
ok(by["s8-r2"]["session"]["mode"] == "resume" and by["s8-r2"]["session"]["session_id"] == by["s8-r1"]["session"]["session_id"], "R2 resumed R1's session")
st = {a: json.load(open(f"{w}/out/status-{a}.json")) for a in ("s8-r2", "s8-g1")}
ok(all(s["central_checkpoint"]["mode"] != "local_only" for s in st.values())
   and not any("local_only" in g for s in st.values() for g in s["capability_gaps"]), "both supervisors report to the coordinator (not local_only)")
ok(all(s["safe_shutdown"]["pending_external_requests"] == [] for s in st.values()), "no pending external requests (safe_shutdown)")
PY
for a in S8-DEV-A1 S8-GR-A1; do
  st=$(req "chain-$a" s8-operator GET "/v0/coordinator/assignments/$a/generations/0/chain")
  n=$(j "chain-$a" "sum(1 for l in d.get('links', []) if l.get('taken_over') and l.get('first_action'))")
  [ "$st" = 200 ] && [ "$n" -ge 1 ]; ok $? "$a chain: $n delivery(ies) taken over with a first action"
done
kill -TERM "$CPID"; for _ in $(seq 1 150); do kill -0 "$CPID" 2>/dev/null || break; sleep 0.2; done
wait "$CPID"; code=$?; CPID=; [ "$code" = 0 ]; ok $? "the test coordinator stops on SIGTERM (exit $code)"
grep -q "ieo-coord: stopped" "$W/out/coordinator.log"; ok $? "coordinator log: stopped"
echo "## result: $pass passed, $fail failed"
[ "$fail" = 0 ]
