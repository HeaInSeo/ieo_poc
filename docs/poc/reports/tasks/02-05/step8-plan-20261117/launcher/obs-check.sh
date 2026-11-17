#!/bin/bash
# 02-05 step 8 (MANUAL_STEPS.md): the checks that read the run records and the captured
# coordinator environment, so that the agent can run them through ONE narrow allow rule
# (`bash $D/step8-ops/obs-check.sh`, no arguments; all paths are fixed below).
#   [1] the first observation D6 (02-04): exactly one D6 stream, every rate_limit_event allowed
#       with overage rejected and not in use, the D6 record ended at 1794698043; installs a copy
#       (0600, only when those hold) at $D/ledger-0205-obs/D6.jsonl (an existing copy must be identical) and prints its SHA-256.
#   [2] nothing of 02-04 in flight: no inflight.json, no process holding caller.lock, no ieo-role-call.
#   [3] the database of the captured coordinator (after `coord-switch.sh capture`): user, dbname,
#       host and port of IEO_DB_URL from $D/private/s8/coord-778ee47/env0. Never the password (on a
#       parse error only the exception type).
#   [4] the role workdir (after run-step8-roles.sh): exactly one /opt/rust/ieo_poc_runs/02-05-s8-*
#       with roles-entry.json; when both roles point at W/adapter.token and the header has its form,
#       installs adapter.header (0600) and roles-entry.json (0644) into $D/private/s8/ (existing
#       copies must be identical). The adapter token stays in the workdir
#       (the coordinator reads it there).
# [3] and [4] are skipped (not failed) before their step. Prints counts, True/False, names, paths
# and SHA-256 of non-secret files only: no token, credential, password or environment value.
# Exit 0 when every check holds, 1 otherwise. Re-running is safe (installs are idempotent).
set -uo pipefail
D=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data
RUNS=/opt/rust/ieo_poc_runs
OBS_UNIX=1794698043
fails=0
fail() { echo "FAIL: $*"; fails=$((fails + 1)); }
# install src dst mode: copy once; an existing copy must be byte-identical
put() {
  if [ -e "$2" ]; then
    if cmp -s "$1" "$2"; then echo "  $2 already installed, identical"; else fail "$2 exists and differs from $1"; fi
  else
    if install -m "$3" "$1" "$2"; then echo "  installed $2 ($3)"; else fail "cannot install $2"; fi
  fi
}

echo "[1] first observation D6"
shopt -s nullglob
streams=("$RUNS"/*/ledger/streams/D6.jsonl)
echo "  D6 streams: ${#streams[@]}"; for p in "${streams[@]}"; do echo "  $p"; done
if [ "${#streams[@]}" -ne 1 ]; then
  fail "expected exactly one D6 stream"
else
  P=${streams[0]}; R=$(dirname "$(dirname "$P")")/records.jsonl
  if python3 -I - "$P" "$R" "$OBS_UNIX" <<'PY'
import json, sys
p, r, obs = sys.argv[1], sys.argv[2], int(sys.argv[3])
ev = [json.loads(l)["rate_limit_info"] for l in open(p) if '"rate_limit_event"' in l]
ok_ev = len(ev) >= 1 and all(e.get("status") == "allowed" and e.get("overageStatus") == "rejected"
                             and e.get("isUsingOverage") is False for e in ev)
print(f"  rate_limit_events {len(ev)}; all allowed, overage rejected, not in use: {ok_ev}")
d = [x for x in (json.loads(l) for l in open(r) if l.strip()) if x.get("attempt_id") == "D6"]
ended = d[0].get("ended_unix") if len(d) == 1 else None
print(f"  D6 records {len(d)}; ended_unix {ended}; equals {obs}: {ended == obs}")
sys.exit(0 if ok_ev and ended == obs else 1)
PY
  then
    mkdir -p -m 700 "$D/ledger-0205-obs"
    put "$P" "$D/ledger-0205-obs/D6.jsonl" 0600
    [ -e "$D/ledger-0205-obs/D6.jsonl" ] && echo "  sha256 $(sha256sum < "$D/ledger-0205-obs/D6.jsonl" | cut -c1-64) $D/ledger-0205-obs/D6.jsonl"
  else
    fail "D6 stream or record check (no copy installed)"
  fi
fi

echo "[2] 02-04 in flight"
ledgers=("$RUNS"/02-04-*/ledger)
echo "  02-04 ledgers: ${#ledgers[@]}"; for l in "${ledgers[@]}"; do echo "  $l"; done
[ "${#ledgers[@]}" -ge 1 ] || fail "no 02-04 ledger found"
for l in "${ledgers[@]}"; do
  if [ -e "$l/inflight.json" ]; then fail "$l/inflight.json exists"; else echo "  inflight.json absent: True"; fi
  if [ -e "$l/caller.lock" ]; then
    fuser "$l/caller.lock" >/dev/null 2>&1; rc=$?
    echo "  processes holding caller.lock: $([ $rc -eq 1 ] && echo none || echo some) (fuser exit $rc)"
    [ $rc -eq 1 ] || fail "$l/caller.lock is held"
  else
    echo "  caller.lock absent"
  fi
done
n=$(pgrep -xc ieo-role-call); echo "  ieo-role-call processes: $n"
[ "$n" = 0 ] || fail "ieo-role-call is running"

echo "[3] database of the captured coordinator"
E=$D/private/s8/coord-778ee47/env0
if [ ! -e "$E" ]; then
  echo "  $E not captured yet: skipped"
else
  python3 -I - "$E" <<'PY' || fail "IEO_DB_URL in $E"
import sys, urllib.parse
# on any parse error only the exception type is printed: a message could quote part of the value
try:
    env = dict(kv.split("=", 1) for kv in open(sys.argv[1], "rb").read().decode(errors="replace").split("\0") if "=" in kv)
    u = env.get("IEO_DB_URL")
    if not u:
        print("  IEO_DB_URL present: False"); sys.exit(1)
    if "://" in u:
        p = urllib.parse.urlparse(u)
        f = ("url", p.username, p.path.lstrip("/"), p.hostname, p.port, p.password is not None)
    else:
        kv = dict(x.split("=", 1) for x in u.split() if "=" in x)
        f = ("key=value", kv.get("user"), kv.get("dbname"), kv.get("host"), kv.get("port"), "password" in kv)
except Exception as e:
    print("  IEO_DB_URL not parseable:", type(e).__name__); sys.exit(1)
print("  form %s; user %s dbname %s host %s port %s ; password set: %s" % f)
PY
fi

echo "[4] role workdir"
entries=("$RUNS"/02-05-s8-*/roles-entry.json)
if [ "${#entries[@]}" -eq 0 ]; then
  echo "  no $RUNS/02-05-s8-*/roles-entry.json yet: skipped"
elif [ "${#entries[@]}" -gt 1 ]; then
  fail "more than one role workdir: ${entries[*]}"
else
  W=$(dirname "${entries[0]}"); echo "  W=$W"
  if python3 -I - "$W" <<'PY'
import json, re, sys
w = sys.argv[1]
try:
    e = json.load(open(f"{w}/roles-entry.json"))
    h = open(f"{w}/adapter.header").read()
except Exception as x:
    print("  roles-entry.json or adapter.header not readable:", type(x).__name__); sys.exit(1)
ok = set(e) == {"development", "guardrail"} and all(e[r].get("adapter_token_file") == f"{w}/adapter.token" for r in e)
print("  roles", sorted(e), "; quota_group_id", {r: e[r].get("quota_group_id") for r in sorted(e)},
      "; adapter_token_file in W:", ok)
form = re.fullmatch(r"Authorization: Bearer [0-9a-f]{48}\n", h) is not None
print("  adapter.header form ok:", form)
sys.exit(0 if ok and form else 1)
PY
  then
    mkdir -p -m 700 "$D/private/s8"
    put "$W/adapter.header" "$D/private/s8/adapter.header" 0600
    put "$W/roles-entry.json" "$D/private/s8/roles-entry.json" 0644
    echo "  sha256 $(sha256sum < "$D/private/s8/roles-entry.json" | cut -c1-64) $D/private/s8/roles-entry.json"
  else
    fail "roles-entry.json or adapter.header of $W (nothing installed)"
  fi
fi

if [ "$fails" -eq 0 ]; then echo "## obs-check: PASS"; exit 0; fi
echo "## obs-check: FAIL ($fails)"; exit 1
