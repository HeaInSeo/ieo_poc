#!/bin/bash
# Container dry run of RUN_PLAN.md with a fake claude and fake OAuth-form credentials: the same
# run-real.sh, ieo-role-call, call.sh and summarize.py as the real runs, no model call.
# Usage: dry-run.sh <ieo-supervisor binary> <scratch workdir>
set -uo pipefail
BIN=$1; W=$2
HERE=$(cd "$(dirname "$0")" && pwd)
PY=/opt/rust/src/github.com/HeaInSeo/ieo_poc/.venv-poc/bin/python
podman unshare rm -rf "$W"; mkdir -p "$W/fake/secrets"
for r in development guardrail; do echo "sk-ant-oat01-fake-$r-not-a-credential" > "$W/fake/secrets/$r.token"; chmod 600 "$W/fake/secrets/$r.token"; done
# fake claude: session totals grow per session (kept on the role volume), /compact compacts
cat > "$W/fake/claude" <<'FAKE'
#!/bin/sh
prompt=$(head -c 65536)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
f="/state/fake-cum-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"%s","tools":["Read","Agent"],"memory_paths":[],"skills":[]}\n' "$sid" "$(pwd)"
case "$prompt" in
  */compact*) i=$(($1+5)); o=$(($2+5)); r=$3; c=$(($4+5)); u='0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0'
     printf '{"type":"system","subtype":"compact_boundary","session_id":"%s"}\n' "$sid";;
  *) i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10)); u='3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20'
     printf '{"type":"assistant","message":{"id":"m-%s-%s","usage":{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n' "$sid" "$i";;
esac
echo "$i $o $r $c" > "$f"
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","result":"fake","usage":{"input_tokens":%s},"modelUsage":{"m":{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}},"total_cost_usd":0.01}\n' "$sid" "$u" "$i" "$o" "$r" "$c"
FAKE
chmod 755 "$W/fake/claude"
echo '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}' > "$W/fake/first.jsonl"
echo "# 02-04 real-session dry run (fake claude, fake credentials, no model call) · $(date -u +%FT%TZ)"
CLAUDE_BIN="$W/fake/claude" SECRETS_DIR="$W/fake/secrets" NAME_PREFIX=ieo-dry PORT_DEV=58221 PORT_GR=58222 \
  FIRST_OBSERVATION="$W/fake/first.jsonl" FIRST_OBSERVATION_UNIX=$(($(date +%s) - 60)) \
  bash "$HERE/run-real.sh" "$BIN" "$W/run" || exit 1
for a in D1 G1 D2 D3 D4 D5 D6; do
  printf '%s: ' "$a"; bash "$HERE/call.sh" "$W/run" "$a" | tail -1
done
echo "## checks"
"$PY" - "$W/run" <<'PYEOF'
import json, sys
w = sys.argv[1]
recs = [json.loads(l) for l in open(w + "/ledger/records.jsonl") if l.strip()]
by = {r["attempt_id"]: r for r in recs}
ok = lambda c, m: print(("PASS " if c else "FAIL ") + m)
ok(len(recs) == 7 and not any(r["stop_required"] for r in recs), "7 records, none asks for a person")
ok(all(r["usage"]["run"]["state"] == "measured" for r in recs), "every run measured")
s1 = by["D1"]["session"]["session_id"]
ok(all(by[a]["session"] == {"mode": "resume", "session_id": s1, "resume_base_attempt": p}
       for a, p in [("D2", "D1"), ("D3", "D2"), ("D4", "D3"), ("D5", "D4")]), "D2..D5 resume D1's session on the previous run's base")
t = lambda r: r["usage"]["run"]["tokens"]
keys = ["input", "output", "cache_read", "cache_creation"]
total = {k: sum(t(by[a])[k] for a in ["D1", "D2", "D3", "D4", "D5"]) for k in keys}
ok(total == by["D5"]["usage"]["session"]["tokens"], f"§11 item 7: sum of D1..D5 shares {total} = D5 session total")
d6 = by["D6"]
ok(d6["session"]["mode"] == "new" and d6["session"]["session_id"] != s1 and d6["outcome"]["tamper"]["config_tamper_detected"] is True,
   "D6: tamper detected, new session")
ok(by["D1"]["outcome"]["config_cleanup"]["removed"] > 0 and by["D1"]["outcome"]["config_cleanup"]["credentials_removed"] == [], "D1: planted config-dir entries removed (no credential file among them)")
ok(by["G1"]["role"] == "guardrail" and by["G1"]["session"]["mode"] == "new", "G1 on the Guardrail role")
ok(all("CLAUDE_CODE_OAUTH_TOKEN" in r["outcome"]["env_names"] and not any(n.startswith("ANTHROPIC_") for n in r["outcome"]["env_names"]) for r in recs), "environment: OAuth variable, no API key")
print("records:", len(recs), "· spent:", json.dumps({k: sum(t(r)[k] for r in recs) for k in keys}))
PYEOF
echo "## next call after the plan: refused or held? (should still start: shares not used up)"
grep -c . "$W/run/ledger/records.jsonl"
echo "## secrets in summaries and container logs: $(cat "$W/run/summaries/"*.json | grep -c 'sk-ant-oat01-fake' || true) / $( (podman logs ieo-dry-fixture0204-development; podman logs ieo-dry-fixture0204-guardrail) 2>&1 | grep -c 'sk-ant-oat01-fake' || true)"
for r in development guardrail; do
  t0=$(date +%s.%N); podman stop ieo-dry-fixture0204-$r >/dev/null; t1=$(date +%s.%N)
  echo "$r: podman stop $(python3 -c "print(round($t1-$t0,1))") s, exit $(podman inspect ieo-dry-fixture0204-$r --format '{{.State.ExitCode}}'), stop timeout $(podman inspect ieo-dry-fixture0204-$r --format '{{.Config.StopTimeout}}') s"
done
podman rm -f ieo-dry-fixture0204-development ieo-dry-fixture0204-guardrail >/dev/null
echo "# done"
