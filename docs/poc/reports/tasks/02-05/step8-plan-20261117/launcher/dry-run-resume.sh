#!/bin/bash
# Dry run of the step-8 resume (RESUME_PLAN.md §9) end to end on test systems only, with the SAME
# scripts as the real resume (copied by make-resume-copies.sh and run from the copies):
#   A. the state of 2026-11-17: the 46799b0 coordinator (M) and role supervisors; R1 stopped by the
#      old guard on the real s8-r1 allowed_warning event (unmeasured, no result), its session
#      transcript on the role volume
#   B. the copies, SHA256SUMS and the rule change (apply-perm-resume.py on a settings copy)
#   C. restart-roles.sh swap (same containers and volumes, the new supervisor), the coordinator
#      switch to the new build with the captured argv/env/cwd
#   D. the settlement (dry run, then written) from an evidence copy of the transcript; R1b by the
#      user principal (new session, spare replacing s8-r1); R2 resumes R1b's session; G1
#   E. rollback rehearsal: the old supervisor back, the old coordinator on the same ledger (it
#      does not know the settlement: it refuses the next call; fail closed)
# No model call, nothing of 58090/55433/58221/58222, no secrets, no run records.
# Usage: dry-run-resume.sh <46799b0 bin dir: ieo-supervisor, ieo-coord> <new bin dir: ieo-supervisor,
#        ieo-coord, ieo-role-call> <scratch workdir>
# Env: TEST_DB_PASSWORD_FILE (never printed); optional TEST_PG_CONTAINER, TEST_PG_PORT, COORD_PORT
# (58290), PORT_DEV/PORT_GR (58231/58232).
set -uo pipefail
OLD_BIN=$(realpath "$1"); NEW_BIN=$(realpath "$2"); W=$(realpath -m "$3")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../../../../../.." && pwd)
PGC=${TEST_PG_CONTAINER:-ieo-poc-0203-pg}; PGPORT=${TEST_PG_PORT:-55432}; CPORT=${COORD_PORT:-58290}
export PORT_DEV=${PORT_DEV:-58231} PORT_GR=${PORT_GR:-58232} NAME_PREFIX=ieo-s8dry
: "${TEST_DB_PASSWORD_FILE:?}"
[ "$PGPORT" = 55433 ] && { echo "refusing the persistent database port"; exit 1; }
[ "$CPORT" = 58090 ] && { echo "refusing the live coordinator port"; exit 1; }
case " $PORT_DEV $PORT_GR " in *" 58221 "*|*" 58222 "*) echo "refusing the live role ports"; exit 1;; esac
if [ -e "$W" ] && [ ! -e "$W/fake/claude" ]; then echo "$W is not a dry-run workdir: refusing to delete it"; exit 1; fi
for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
[ -e "$W" ] && { chmod -R u+w "$W" 2>/dev/null; podman unshare rm -rf "$W"; }
mkdir -p "$W/fake/secrets" "$W/tok" "$W/out" "$W/private" "$W/bin-old" "$W/bin-new" "$W/cwd" "$W/D" "$W/evidence"; chmod 700 "$W"
cp "$OLD_BIN/ieo-supervisor" "$OLD_BIN/ieo-coord" "$W/bin-old/"
cp "$NEW_BIN/ieo-supervisor" "$NEW_BIN/ieo-coord" "$NEW_BIN/ieo-role-call" "$W/bin-new/"
DB=ieo_t_s8res_$$
cleanup() {
  [ -s "$W/coord.pid" ] && kill -KILL "$(cat "$W/coord.pid")" 2>/dev/null
  for r in development guardrail; do podman rm -f "ieo-s8dry-ieo-poc-test-$r" >/dev/null 2>&1 || true; done
  podman exec "$PGC" dropdb -U ieo --if-exists --force "$DB" >/dev/null 2>&1 || true
}
trap cleanup EXIT
pass=0; fail=0
ok() { if [ "$1" = 0 ]; then pass=$((pass+1)); echo "PASS $2"; else fail=$((fail+1)); echo "FAIL $2"; fi; }
sha() { sha256sum < "$1" | cut -c1-64; }
echo "# 02-05 step 8 RESUME dry run (fake claude, test PostgreSQL $PGPORT/$DB, test coordinator 127.0.0.1:$CPORT, roles $PORT_DEV/$PORT_GR, no model call) · $(date -u +%FT%TZ)"
echo "# branch $(git -C "$ROOT" rev-parse --short HEAD) · old (46799b0) supervisor $(sha "$W/bin-old/ieo-supervisor" | cut -c1-12) coordinator $(sha "$W/bin-old/ieo-coord" | cut -c1-12) · new supervisor $(sha "$W/bin-new/ieo-supervisor" | cut -c1-12) coordinator $(sha "$W/bin-new/ieo-coord" | cut -c1-12)"
for r in development guardrail; do echo "sk-ant-oat01-fake-$r-not-a-credential" > "$W/fake/secrets/$r.token"; chmod 600 "$W/fake/secrets/$r.token"; done
WARN=$(tr -d '\n' < "$ROOT/crates/ieo-supervisor/tests/fixtures/s8-r1-rate-limit-event.jsonl")
ALLOWED='{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}'
# fake claude: the first development run is s8-r1 of 2026-11-17 (a transcript like the CLI's, then the
# real allowed_warning event, then it would go on: the old guard stops it); later development runs
# get the same warning and finish; the Guardrail gets "allowed"
cat > "$W/fake/claude" <<FAKE
#!/bin/sh
prompt=\$(head -c 65536)
sid=\$(printf '%s\n' "\$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=\$0}')
tools=\$(printf '%s\n' "\$@" | awk 'prev=="--tools"{print; exit} {prev=\$0}')
[ -n "\$tools" ] || tools="Task,Bash,Glob,Grep,Read,Edit,Write"
tl=\$(printf '%s' "\$tools" | awk -F, '{for(i=1;i<=NF;i++) printf "%s\"%s\"", (i>1?",":""), \$i}')
f="/state/fake-cum-\$sid"
set -- \$(cat "\$f" 2>/dev/null || echo 0 0 0 0)
i=\$((\$1+3)); o=\$((\$2+7)); r=\$((\$3+20)); c=\$((\$4+10))
printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"%s","tools":[%s],"memory_paths":[],"skills":[]}\n' "\$sid" "\$(pwd)" "\$tl"
case "\$(pwd)" in */work/*) dev=yes;; *) dev=no;; esac
if [ \$dev = yes ] && [ ! -e /state/fake-warned ]; then
  : > /state/fake-warned
  t="\$CLAUDE_CONFIG_DIR/projects/\$(pwd | tr / -)"; mkdir -p "\$t"
  { printf '{"type":"custom-title","sessionId":"%s"}\n' "\$sid"
    printf '{"type":"api-request","id":"req-dry-1","sessionId":"%s","timestamp":"2026-11-17T12:19:47.084Z"}\n' "\$sid"
    for b in 0 1; do printf '{"type":"assistant","sessionId":"%s","isSidechain":false,"requestRef":"req-dry-1","apiBlockIndex":%s,"message":{"id":"m-warn-%s","usage":{"input_tokens":3,"output_tokens":30,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "\$sid" "\$b" "\$sid"; done
    printf '{"type":"cost-state","sessionId":"%s","modelUsage":{"m":{"inputTokens":3,"outputTokens":30,"cacheCreationInputTokens":10,"cacheReadInputTokens":20}}}\n' "\$sid"
    printf '{"type":"user","sessionId":"%s","isSidechain":false}\n' "\$sid"
  } > "\$t/\$sid.jsonl"
  printf '{"type":"assistant","message":{"id":"m-warn-%s","content":[{"type":"tool_use","id":"t-w","name":"Read","input":{"file_path":"README.md"}}],"usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "\$sid"
  printf '%s\n' '$WARN'
  sleep 30
  exit 0
fi
printf '{"type":"assistant","message":{"id":"m-%s-%s","content":[{"type":"tool_use","id":"t-%s","name":"Read","input":{"file_path":"README.md"}}],"usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "\$sid" "\$i" "\$i"
echo "\$i \$o \$r \$c" > "\$f"
if [ \$dev = yes ]; then printf '%s\n' '$WARN'; else printf '%s\n' '$ALLOWED'; fi
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","result":"fake line one\\\\nfake line two","usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20},"modelUsage":{"m":{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}},"total_cost_usd":0.01}\n' "\$sid" "\$i" "\$o" "\$r" "\$c"
FAKE
chmod 755 "$W/fake/claude"
echo "$ALLOWED" > "$W/fake/first.jsonl"
mk() { ( umask 077; head -c 32 /dev/urandom | base64 | tr -d '\n=' > "$W/tok/$1.token"
         printf 'Authorization: Bearer %s\n' "$(cat "$W/tok/$1.token")" > "$W/tok/$1.header" ); }
dg() { printf '%s' "$(cat "$W/tok/$1.token")" | sha256sum | cut -d' ' -f1; }
for p in s8-registrant user-dry s8-assigner s8-operator s8-ingest s8-sup-dev s8-sup-gr; do mk "$p"; done
pr() { printf '{"name": "%s", "token_sha256": "%s", "kind": "%s", "scopes": %s, "repo_ids": ["ieo-poc-test"]%s}' "$1" "$(dg "$1")" "$2" "$3" "${4:-}"; }
echo "{\"principals\": [$(pr s8-registrant service '["sprints:register", "sprints:read"]'), $(pr user-dry user '["review:semantic", "sprints:read", "roles:control"]'), $(pr s8-assigner service '["coordinator:assign"]'), $(pr s8-operator service '["roles:control"]'), $(pr s8-ingest service '["evidence:ingest"]'), $(pr s8-sup-dev agent '["checkpoint:report"]' ', "role": "development"'), $(pr s8-sup-gr agent '["checkpoint:report"]' ', "role": "guardrail"')]}" > "$W/cwd/tokens.json"
DBURL="host=127.0.0.1 port=$PGPORT user=ieo dbname=$DB password=$(cat "$TEST_DB_PASSWORD_FILE")"
podman exec "$PGC" createdb -U ieo "$DB"; ok $? "test database $DB created on $PGPORT"
# ---- A. the state of 2026-11-17 ----
CLAUDE_BIN="$W/fake/claude" SECRETS_DIR="$W/fake/secrets" QUOTA_GROUP_ID=dry-run-quota \
  SUP_DEV_TOKEN_FILE="$W/tok/s8-sup-dev.token" SUP_GR_TOKEN_FILE="$W/tok/s8-sup-gr.token" COORD_PORT="$CPORT" \
  bash "$HERE/run-step8-roles.sh" "$W/bin-old" "$W/run"; ok $? "old (46799b0) role supervisors up (run-step8-roles.sh)"
python3 -I - "$W" "$(($(date +%s) - 60))" <<'PY'
import json, sys
w, at = sys.argv[1], int(sys.argv[2])
e = json.load(open(w + "/run/roles-entry.json"))
e["budget_plan"] = {"budget": "02-05", "ledger_dir": w + "/ledger-0205",
                    "first_observation": {"stream": w + "/fake/first.jsonl", "observed_unix": at},
                    "roles": {"development": {"max_window_seconds": 600}, "guardrail": {"max_window_seconds": 600}}}
json.dump({"ieo-poc-test": e}, open(w + "/cwd/roles.json", "w"), indent=1)
json.dump({"budget": "02-05", "ledger_dir": w + "/ledger-0205",
           "first_observation": {"stream": w + "/fake/first.jsonl", "observed_unix": at},
           "roles": {r: {"url": e[r]["url"], "adapter_token_file": e[r]["adapter_token_file"]} for r in ("development", "guardrail")}},
          open(w + "/private/plan-0205.json", "w"), indent=1)
PY
mkdir -p "$W/ledger-0205"
( cd "$W/cwd" || exit 1; export IEO_DB_URL="$DBURL"
  setsid nohup "$W/bin-old/ieo-coord" serve --listen "127.0.0.1:$CPORT" --tokens tokens.json --roles roles.json \
    --manifest "$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json" --policy "$ROOT/contracts/judgment-policy-v0.json" --workers 2 \
    >> "$W/out/coord-old.log" 2>&1 < /dev/null & echo $! > "$W/coord.pid" )
for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$CPORT/v0/health" && break; sleep 0.2; done
export COORD="http://127.0.0.1:$CPORT" TOKDIR="$W/tok" OUT="$W/out" LEDGER="$W/ledger-0205" ADAPTER_HEADER="$W/run/adapter.header"
. "$HERE/lib-step8.sh"
bash "$HERE/run-step8.sh" register >/dev/null; ok $? "old build: register"
DIG=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['digest'])" "$HERE/../sprint-POC-S8-01-r1.json")
echo "{\"decision\": \"passed\", \"reason\": \"dry run\", \"digest\": \"$DIG\"}" > "$OUT/review-body.json"
st=$(req review user-dry POST /v0/sprints/POC-S8-01/revisions/1/semantic-review "$OUT/review-body.json"); [ "$st" = 201 ]; ok $? "semantic review ($st)"
bash "$HERE/run-step8.sh" prepare >/dev/null; ok $? "old build: prepare"
OBS_UNIX=$(($(date +%s) - 60)) bash "$HERE/run-step8.sh" runs > "$OUT/runs-old.log" 2>&1; rc=$?
[ $rc = 3 ] && grep -q 'STOP: s8-r1' "$OUT/runs-old.log"; ok $? "old build: R1 stops like 2026-11-17 (exit $rc)"
python3 -I - "$LEDGER/records.jsonl" <<'PY'; ok $? "s8-r1 recorded like the real one: guard stop on allowed_warning (old rule), unmeasured, no result"
import json, sys
r = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
assert len(r) == 1 and r[0]["attempt_id"] == "s8-r1", r
r = r[0]
assert r["outcome"]["stopped_by_guard"] is True and "allowed_warning" in r["outcome"]["stop"], r["outcome"]
assert r["usage"]["run"]["state"] == "unmeasured" and r["stop_required"] is True and r["result_event"]["present"] is False
print("  ", r["stop_reason"][:160])
PY
R1_SID=$(python3 -I -c "import json,sys; print(json.loads(open(sys.argv[1]).readline())['session']['session_id'])" "$LEDGER/records.jsonl")
R1_END=$(python3 -I -c "import json,sys; print(json.loads(open(sys.argv[1]).readline())['ended_unix'])" "$LEDGER/records.jsonl")
bash "$HERE/coord-switch.sh" capture "$W/coord.pid" "$W/bin-old/ieo-coord" "$CPORT" "$W/private/coord-old" >/dev/null; ok $? "capture of the running old coordinator (like private/s8/coord-778ee47)"
# ---- B. copies, SHA256SUMS, the rule change ----
bash "$HERE/make-resume-copies.sh" "$ROOT" "$W/D" "$W/bin-new" "$W/rules.json" > "$OUT/copies.log" 2>&1; ok $? "make-resume-copies.sh: $(tail -1 "$OUT/copies.log")"
L="$W/D/step8-resume/launcher"
SUMS=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['sums'])" "$W/rules.json")
( cd "$W/D" && echo "$SUMS  step8-ops/SHA256SUMS" | sha256sum --quiet --strict -c - && sha256sum --quiet --strict -c step8-ops/SHA256SUMS && find step8-resume step8-ops -perm /222 | wc -l | grep -qx 0 ); ok $? "the gate passes on the copies (self-hash, every file, read-only)"
python3 -I - "$ROOT/docs/poc/reports/tasks/02-05/step8-plan-20261117/MANUAL_STEPS.md" "$W/settings.json" <<'PY'
import json, re, sys
j = json.loads(re.search(r"```json\n(.*?)```", open(sys.argv[1]).read(), re.S).group(1))
j["permissions"]["additionalDirectories"] = ["/x"]; j["hooks"] = {"SessionStart": []}
json.dump(j, open(sys.argv[2], "w"), indent=2)
PY
python3 -I "$L/apply-perm-resume.py" "$W/rules.json" "$W/settings.json" > "$OUT/perm.log"; rc=$?
python3 -I - "$W/settings.json" "$W/rules.json" <<'PY'; ok $(( rc | $? )) "apply-perm-resume.py on a settings copy: $(tail -1 "$OUT/perm.log" | cut -c1-80)"
import json, sys
s = json.load(open(sys.argv[1])); r = json.load(open(sys.argv[2]))
a = s["permissions"]["allow"]
assert all(a.count(x) == 1 for x in r["add"]) and len(a) == len(r["add"]), (len(a), len(r["add"]))
assert s["permissions"]["deny"] and s["hooks"] == {"SessionStart": []} and s["permissions"]["additionalDirectories"] == ["/x"]
PY
python3 -I "$L/apply-perm-resume.py" "$W/rules.json" "$W/settings.json" | grep -q "already applied"; ok $? "apply-perm-resume.py again: already applied, nothing changed"
# ---- C. the new supervisor in the same containers; the coordinator switch ----
NEWSUP=$(sha "$W/bin-new/ieo-supervisor")
bash "$L/restart-roles.sh" swap "$W/run" "$W/bin-new/ieo-supervisor" "$NEWSUP" > "$OUT/restart-roles.log" 2>&1; ok $? "restart-roles.sh swap: $(grep -c 'up, idle' "$OUT/restart-roles.log") roles up and idle on the new binary"
podman unshare test -s "$W/run/development/state/checkpoint.json"; ok $? "the development volume kept its local checkpoint (s8-r1 after_run)"
bash "$L/coord-switch.sh" stop "$W/coord.pid" 900 >/dev/null; ok $? "old coordinator stopped"
bash "$L/coord-switch.sh" start "$W/bin-new/ieo-coord" "$W/private/coord-old" "$W/coord.pid" "$OUT/coord-new.log" --tokens "$W/cwd/tokens.json" -- --roles "$W/cwd/roles.json" >/dev/null; ok $? "new coordinator started with the captured argv/env/cwd, --tokens, --roles"
# ---- D. settlement, R1b (the user), R2, G1 ----
podman unshare cat "$W/run/development/state/claude/projects/-state-work-ieo-poc-test/$R1_SID.jsonl" > "$W/evidence/transcript.jsonl"; chmod 0400 "$W/evidence/transcript.jsonl"
ORIG_SHA=$(podman unshare sha256sum "$W/run/development/state/claude/projects/-state-work-ieo-poc-test/$R1_SID.jsonl" | cut -c1-64)
[ "$ORIG_SHA" = "$(sha "$W/evidence/transcript.jsonl")" ]; ok $? "evidence copy of the R1 transcript equals the original in the volume"
echo "# dry-run verification report" > "$W/evidence/verification.md"
export PLAN="$W/private/plan-0205.json" ROLE_CALL="$W/bin-new/ieo-role-call" TRANSCRIPT="$W/evidence/transcript.jsonl" \
  ORIGINAL_PATH="$W/run/development/state/claude/projects/-state-work-ieo-poc-test/$R1_SID.jsonl" ORIGINAL_SHA256="$ORIG_SHA" \
  DECISION=docs/poc/User_Decisions_2026-11-17_r1-settlement.md CHECK_REPORT="$W/evidence/verification.md" CHECK_VERDICT="CONFIRMED WITH CAVEAT"
n0=$(wc -l < "$LEDGER/records.jsonl")
DRY=1 bash "$L/run-step8-resume.sh" settle > "$OUT/settle-dry.log" 2>&1; rc=$?
[ $rc = 0 ] && [ "$(wc -l < "$LEDGER/records.jsonl")" = "$n0" ]; ok $? "settle, dry run: nothing written ($(tail -1 "$OUT/settle-dry.log"))"
bash "$L/run-step8-resume.sh" settle > "$OUT/settle.log" 2>&1; ok $? "settle: $(tail -1 "$OUT/settle.log" | cut -c1-140)"
bash "$L/run-step8-resume.sh" settle > /dev/null 2>&1; [ $? = 3 ]; ok $? "settle again: refused (already settled, nothing written)"
bash "$L/run-step8-resume.sh" r1b-body > "$OUT/r1b-body.log" 2>&1; ok $? "r1b-body: the body and the user's command"
grep -q "Idempotency-Key: s8-dlv-r1b-0001" "$OUT/r1b-body.log"; ok $? "the printed user command carries the R1b key and the user's token file"
# the user's command, with the dry run's user principal (same request, same key)
st=$(req r1b user-dry POST /v0/repos/ieo-poc-test/roles/development/deliveries "$OUT/r1b-body.json" s8-dlv-r1b-0001); ok "$([ "$st" = 201 ] || [ "$st" = 200 ]; echo $?)" "the user's R1b delivery ($st)"
bash "$L/run-step8-resume.sh" r1b-check > "$OUT/r1b-check.log" 2>&1; ok $? "r1b-check: $(grep -m1 's8-r1b:' "$OUT/r1b-check.log") · $(grep -m1 'spare:' "$OUT/r1b-check.log")"
grep -q "WARNING s8-r1b" "$OUT/r1b-check.log"; ok $? "R1b got the allowed_warning event: recorded and printed as a warning, not a stop"
OBS_UNIX=$R1_END bash "$L/run-step8-resume.sh" r2g1 > "$OUT/r2g1.log" 2>&1; ok $? "r2g1: $(tail -1 "$OUT/r2g1.log")"
"$W/bin-new/ieo-role-call" records --plan "$PLAN" > "$OUT/final-records.jsonl"
python3 -I - "$OUT/final-records.jsonl" <<'PY'; ok $? "the budget counts 4 runs once each: s8-r1 settled (failed), s8-r1b spare, s8-r2 resumes s8-r1b, s8-g1"
import json, sys
recs = {r["attempt_id"]: r for r in (json.loads(l) for l in open(sys.argv[1]) if l.strip())}
assert sorted(recs) == ["s8-g1", "s8-r1", "s8-r1b", "s8-r2"], sorted(recs)
r1, r1b, r2 = recs["s8-r1"], recs["s8-r1b"], recs["s8-r2"]
assert r1["settlement"]["policy"] == "guard_stop_on_old_rule_warning" and r1["usage"]["run"]["state"] == "measured"
assert r1["stop_required"] is True and r1["stop_reason"] == "no result event"
assert r1b["spare"]["replaces"] == "s8-r1" and r1b["spare"]["by"]["kind"] == "user" and r1b["rate_limit_warnings"]
assert r2["session"]["mode"] == "resume" and r2["session"]["session_id"] == r1b["session"]["session_id"]
assert all(recs[a]["usage"]["run"]["state"] == "measured" for a in recs)
t = sum(recs[a]["usage"]["run"]["tokens"][k] for a in recs for k in ("input", "output", "cache_creation"))
print("   billable over the 4 runs:", t, "| R1 settled", r1["usage"]["run"]["tokens"])
PY
# ---- E. rollback rehearsal ----
bash "$L/restart-roles.sh" rollback "$W/run" > "$OUT/rollback-roles.log" 2>&1; ok $? "restart-roles.sh rollback: the old supervisor back in the same containers"
bash "$L/coord-switch.sh" stop "$W/coord.pid" 900 >/dev/null; ok $? "new coordinator stopped"
bash "$L/coord-switch.sh" start "$W/bin-old/ieo-coord" "$W/private/coord-old" "$W/coord.pid" "$OUT/coord-rollback.log" --tokens "$W/cwd/tokens.json" -- --roles "$W/cwd/roles.json" >/dev/null; ok $? "rollback: the old coordinator starts on the same ledger and database"
python3 -I -c "import json,sys; json.dump({'assignment_id': 'S8-DEV-A1', 'generation': 0, 'attempt_id': 's8-x', 'session_id': '$(cat /proc/sys/kernel/random/uuid)', 'prompt': 'x'}, open(sys.argv[1], 'w'))" "$OUT/x-body.json"
st=$(req x s8-operator POST /v0/repos/ieo-poc-test/roles/development/deliveries "$OUT/x-body.json" s8-dlv-x-0001)
python3 -I - "$OUT/x.json" "$LEDGER/records.jsonl" <<'PY'; ok $? "rollback: the old build does not know the settlement: it refuses the next call, nothing starts (fail closed)"
import json, sys
d = json.load(open(sys.argv[1])); r = d.get("result") or {}
n = sum(1 for l in open(sys.argv[2]) if '"attempt_id":"s8-x"' in l)
print("   old build answered:", r.get("outcome"), r.get("decision"), (r.get("reasons") or d.get("code") or "")[:2] if isinstance(r.get("reasons") or d.get("code"), list) else (d.get("code") or r.get("reasons")))
sys.exit(0 if r.get("outcome") != "accepted" and n == 0 else 1)
PY
bash "$L/coord-switch.sh" stop "$W/coord.pid" 900 >/dev/null; ok $? "old coordinator stopped"
echo "## result: $pass passed, $fail failed"
[ "$fail" = 0 ]
