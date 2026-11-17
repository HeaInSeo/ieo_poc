#!/bin/bash
# Dry run of EXECUTION_PLAN.md §3–§6 end to end on test systems only, with the SAME scripts as the
# real run (run-step8-roles.sh, coord-switch.sh, rehearse-migration.sh, run-step8.sh, check-run.py):
#   1. an "old" coordinator (a 778ee47 build) as a host process on a test database with fixture rows
#   2. backup (pg_dump -Fc) and the migration rehearsal of the new build on a scratch copy
#   3. capture of the old process, stop, start of the new build with the captured argv/env
#   4. role containers (fake claude, fake credentials), register → the user's review → prepare → runs
#   5. the driver's stop checks on doctored copies (an overage signal, an extra tool)
#   6. the rollback: stop the new build, start the old one with the captured argv/env on the migrated DB
# No model call, nothing of 58090/55433, no secrets, no run records.
# Usage: dry-run-step8.sh <dir with the new ieo-supervisor and ieo-coord> <778ee47 ieo-coord> <scratch workdir>
# Env: TEST_DB_PASSWORD_FILE (never printed); optional TEST_PG_CONTAINER, TEST_PG_PORT, COORD_PORT (58290)
set -uo pipefail
BIN_DIR=$(realpath "$1"); OLD=$(realpath "$2"); W=$(realpath -m "$3")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../../../../../.." && pwd)
PGC=${TEST_PG_CONTAINER:-ieo-poc-0203-pg}; PGPORT=${TEST_PG_PORT:-55432}; CPORT=${COORD_PORT:-58290}
: "${TEST_DB_PASSWORD_FILE:?}"
[ "$PGPORT" = 55433 ] && { echo "refusing the persistent database port"; exit 1; }
[ "$CPORT" = 58090 ] && { echo "refusing the live coordinator port"; exit 1; }
if [ -e "$W" ] && [ ! -e "$W/fake/claude" ]; then echo "$W is not a dry-run workdir: refusing to delete it"; exit 1; fi
for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
podman unshare rm -rf "$W"; mkdir -p "$W/fake/secrets" "$W/tok" "$W/out" "$W/backup" "$W/private"; chmod 700 "$W"
DB=ieo_t_s8dry_$$
cleanup() {
  [ -s "$W/coord.pid" ] && kill -KILL "$(cat "$W/coord.pid")" 2>/dev/null
  for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
  podman exec "$PGC" dropdb -U ieo --if-exists --force "$DB" >/dev/null 2>&1 || true
}
trap cleanup EXIT
pass=0; fail=0
ok() { if [ "$1" = 0 ]; then pass=$((pass+1)); echo "PASS $2"; else fail=$((fail+1)); echo "FAIL $2"; fi; }
echo "# 02-05 step 8 dry run (fake claude, fake credentials, test PostgreSQL $PGPORT/$DB, test coordinator 127.0.0.1:$CPORT, no model call) · $(date -u +%FT%TZ)"
echo "# branch $(git -C "$ROOT" rev-parse --short HEAD) · new supervisor $(sha256sum < "$BIN_DIR/ieo-supervisor" | cut -c1-12) · new coordinator $(sha256sum < "$BIN_DIR/ieo-coord" | cut -c1-12) · old coordinator (778ee47) $(sha256sum < "$OLD" | cut -c1-12)"
for r in development guardrail; do echo "sk-ant-oat01-fake-$r-not-a-credential" > "$W/fake/secrets/$r.token"; chmod 600 "$W/fake/secrets/$r.token"; done
# fake claude: offers the tools of `--tools` (or a full unrestricted list without it, as the real CLI
# did in 02-04 D1), reads once, answers two lines; session totals grow on the role volume
cat > "$W/fake/claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 65536)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
tools=$(printf '%s\n' "$@" | awk 'prev=="--tools"{print; exit} {prev=$0}')
[ -n "$tools" ] || tools="Task,Bash,Glob,Grep,Read,Edit,Write,WebFetch,WebSearch,Workflow,RemoteTrigger,SendMessage"
tl=$(printf '%s' "$tools" | awk -F, '{for(i=1;i<=NF;i++) printf "%s\"%s\"", (i>1?",":""), $i}')
f="/state/fake-cum-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10))
printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"%s","tools":[%s],"memory_paths":[],"skills":[]}\n' "$sid" "$(pwd)" "$tl"
printf '{"type":"assistant","message":{"id":"m-%s-%s","content":[{"type":"tool_use","id":"t-%s","name":"Read","input":{"file_path":"README.md"}}],"usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "$sid" "$i" "$i"
echo "$i $o $r $c" > "$f"
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","result":"fake line one\\nfake line two","usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20},"modelUsage":{"m":{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}},"total_cost_usd":0.01}\n' "$sid" "$i" "$o" "$r" "$c"
FAKE
chmod 755 "$W/fake/claude"
echo '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}' > "$W/fake/first.jsonl"
# principals (EXECUTION_PLAN.md §2): token and header files 0600, only SHA-256 in tokens.json
mk() { ( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > "$W/tok/$1.token"
         printf 'Authorization: Bearer %s\n' "$(cat "$W/tok/$1.token")" > "$W/tok/$1.header" ); }
dg() { printf '%s' "$(cat "$W/tok/$1.token")" | sha256sum | cut -d' ' -f1; }
for p in s8-registrant user-dry s8-assigner s8-operator s8-ingest s8-sup-dev s8-sup-gr old-reviewer; do mk "$p"; done
# clients also hold the fixture sprint's repo; an agent is bound to exactly one repo
pr() { local repos='["ieo-poc-test", "console-coordinator"]'; [ "$2" = agent ] && repos='["ieo-poc-test"]'
  printf '{"name": "%s", "token_sha256": "%s", "kind": "%s", "scopes": %s, "repo_ids": %s%s}' "$1" "$(dg "$1")" "$2" "$3" "$repos" "${4:-}"; }
echo "{\"principals\": [$(pr s8-registrant service '["sprints:register", "sprints:read"]'), $(pr old-reviewer user '["review:semantic", "sprints:read"]')]}" > "$W/tokens-old.json"
echo "{\"principals\": [$(pr s8-registrant service '["sprints:register", "sprints:read"]'), $(pr old-reviewer user '["review:semantic", "sprints:read"]'), $(pr user-dry user '["review:semantic", "sprints:read", "roles:control"]'), $(pr s8-assigner service '["coordinator:assign"]'), $(pr s8-operator service '["roles:control"]'), $(pr s8-ingest service '["evidence:ingest"]'), $(pr s8-sup-dev agent '["checkpoint:report"]' ', "role": "development"'), $(pr s8-sup-gr agent '["checkpoint:report"]' ', "role": "guardrail"')]}" > "$W/tokens-new.json"
DBURL="host=127.0.0.1 port=$PGPORT user=ieo dbname=$DB password=$(cat "$TEST_DB_PASSWORD_FILE")"
# 1. the old coordinator as a host process (like 58090: setsid, pid file), fixture rows through its API
podman exec "$PGC" createdb -U ieo "$DB"; ok $? "test database $DB created on $PGPORT"
( export IEO_DB_URL="$DBURL"; setsid nohup "$OLD" serve --listen "127.0.0.1:$CPORT" --tokens "$W/tokens-old.json" \
    --manifest "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" --policy "$ROOT/contracts/judgment-policy-v0.json" --workers 2 \
    >> "$W/out/coord-old.log" 2>&1 < /dev/null & echo $! > "$W/coord.pid" )
for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$CPORT/v0/health" && break; sleep 0.2; done
COORD="http://127.0.0.1:$CPORT"; TOKDIR="$W/tok"; OUT="$W/out"; LEDGER="$W/ledger-0205"; ADAPTER_HEADER="$W/run/adapter.header"
. "$HERE/lib-step8.sh"
python3 -I -c "import json,sys; m=json.load(open(sys.argv[1])); json.dump(m['sprints'][0], open(sys.argv[2],'w'))" "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" "$W/out/fixture-sprint.json"
st=$(req fixture s8-registrant POST '/v0/sprints?mode=draft' "$W/out/fixture-sprint.json" fixture-0001); [ "$st" = 201 ]; ok $? "old build: a fixture draft sprint registered ($st)"
# 2. backup and the migration rehearsal (EXECUTION_PLAN.md §3, §5.2 step 1b)
podman exec "$PGC" pg_dump -U ieo -d "$DB" -Fc > "$W/backup/ieo.dump"; ok $? "pg_dump -Fc ($(stat -c %s "$W/backup/ieo.dump") bytes)"
podman exec -i "$PGC" pg_restore --list < "$W/backup/ieo.dump" > "$W/backup/ieo.dump.list"; ok $? "pg_restore --list ($(grep -c 'TABLE DATA' "$W/backup/ieo.dump.list") tables with data)"
TEST_DB_PASSWORD_FILE="$TEST_DB_PASSWORD_FILE" bash "$HERE/rehearse-migration.sh" "$W/backup/ieo.dump" "$BIN_DIR/ieo-coord" 58291 > "$W/out/rehearsal.log" 2>&1; ok $? "migration rehearsal on a scratch copy"
sed 's/^/    /' "$W/out/rehearsal.log"
# 4a. the role containers (the real launcher; fake claude and credentials; test coordinator port)
CLAUDE_BIN="$W/fake/claude" SECRETS_DIR="$W/fake/secrets" NAME_PREFIX=ieo-s8dry QUOTA_GROUP_ID=dry-run-quota \
  SUP_DEV_TOKEN_FILE="$W/tok/s8-sup-dev.token" SUP_GR_TOKEN_FILE="$W/tok/s8-sup-gr.token" COORD_PORT="$CPORT" \
  bash "$HERE/run-step8-roles.sh" "$BIN_DIR" "$W/run"; ok $? "run-step8-roles.sh starts both role containers"
mkdir -p "$LEDGER"
python3 -I - "$W" "$(($(date +%s) - 60))" <<'PY'
import json, sys
w, at = sys.argv[1], int(sys.argv[2])
e = json.load(open(w + "/run/roles-entry.json"))
e["budget_plan"] = {"budget": "02-05", "ledger_dir": w + "/ledger-0205",
                    "first_observation": {"stream": w + "/fake/first.jsonl", "observed_unix": at},
                    "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}
json.dump({"ieo-poc-test": e}, open(w + "/roles.json", "w"), indent=1)
PY
# 3. capture, stop, start the new build (EXECUTION_PLAN.md §5.2 steps 2–4)
bash "$HERE/coord-switch.sh" capture "$W/coord.pid" "$OLD" "$CPORT" "$W/private"; ok $? "capture of the old process (argv, env: 0600, names only shown)"
bash "$HERE/coord-switch.sh" stop "$W/coord.pid" 30; ok $? "old coordinator stopped"
bash "$HERE/coord-switch.sh" start "$BIN_DIR/ieo-coord" "$W/private" "$W/coord.pid" "$W/out/coord-new.log" --tokens "$W/tokens-new.json" -- --roles "$W/roles.json"; ok $? "new build started with the captured argv/env, new tokens, --roles"
grep -q "user=ieo" "$W/private/env" && [ "$(stat -c %a "$W/private/env")" = 600 ]; ok $? "the captured environment holds the database URL, mode 0600"
# 4b. the run: register → the user's review → prepare → runs (the real driver)
export COORD TOKDIR OUT LEDGER ADAPTER_HEADER
bash "$HERE/run-step8.sh" register; ok $? "run-step8.sh register"
DIG=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['digest'])" "$HERE/../sprint-POC-S8-01-r1.json")
echo "{\"decision\": \"passed\", \"reason\": \"dry run\", \"digest\": \"$DIG\"}" > "$OUT/review-body.json"
st=$(req review user-dry POST /v0/sprints/POC-S8-01/revisions/1/semantic-review "$OUT/review-body.json"); [ "$st" = 201 ]; ok $? "the user principal's semantic review ($st)"
bash "$HERE/run-step8.sh" prepare; ok $? "run-step8.sh prepare"
OBS_UNIX=$(($(date +%s) - 60)) bash "$HERE/run-step8.sh" runs; ok $? "run-step8.sh runs (R1 → R2 → G1, every §6.2 check)"
# 5. the driver's checks stop on a doctored copy (an overage signal; a tool beyond Read/Grep/Glob)
for case in overage tool; do
  d="$W/doctored-$case"; mkdir -p "$d/streams"; cp "$LEDGER/records.jsonl" "$d/"
  python3 -I - "$d" "$LEDGER" "$case" <<'PY'
import json, sys
d, ledger, case = sys.argv[1:]
recs = [json.loads(l) for l in open(d + "/records.jsonl")]
for r in recs:
    if r["attempt_id"] == "s8-r1":
        lines = open(r["stream"]["path"]).read().splitlines()
        if case == "overage":
            lines = [l.replace('"overageStatus":"rejected"', '"overageStatus":"allowed"') for l in lines]
        else:
            lines = [l.replace('"name":"Read"', '"name":"Bash"') for l in lines]
        p = d + "/streams/s8-r1.jsonl"; open(p, "w").write("\n".join(lines) + "\n"); r["stream"]["path"] = p
open(d + "/records.jsonl", "w").write("".join(json.dumps(r) + "\n" for r in recs))
PY
  python3 -I "$HERE/check-run.py" "$d" "$OUT" s8-r1 development S8-DEV-A1 > "$d/check.out" 2>&1
  [ $? = 1 ] && grep -q "CHECK FAILED" "$d/check.out"; ok $? "check-run.py stops on a doctored run ($case): $(grep -m1 'CHECK FAILED' "$d/check.out" | cut -c1-90)"
done
# 6. rollback rehearsal: stop the new build (it waits for runs in progress), the old one on the migrated DB
bash "$HERE/coord-switch.sh" stop "$W/coord.pid" 900; ok $? "new build stopped"
grep -q "ieo-coord: stopped" "$W/out/coord-new.log"; ok $? "new build log: stopped (graceful)"
bash "$HERE/coord-switch.sh" start "$OLD" "$W/private" "$W/coord.pid" "$W/out/coord-rollback.log"; ok $? "rollback: the old build starts with the captured argv/env on the migrated database"
st=$(req after-rollback s8-registrant GET /v0/sprints/POC-S8-01/revisions/1); [ "$st" = 200 ]; ok $? "rollback: the old build reads the step-8 sprint ($st)"
bash "$HERE/coord-switch.sh" stop "$W/coord.pid" 30; ok $? "old build stopped"
echo "## result: $pass passed, $fail failed"
[ "$fail" = 0 ]
