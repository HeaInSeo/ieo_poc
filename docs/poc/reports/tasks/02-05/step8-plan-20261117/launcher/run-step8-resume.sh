#!/bin/bash
# 02-05 step 8 resume (RESUME_PLAN.md §9; user decision 2026-11-17 r1-settlement (가)/(A)):
#   run-step8-resume.sh settle      append the settlement of s8-r1 (ieo-role-call settle; DRY=1: --dry-run)
#   run-step8-resume.sh r1b-body    write the R1b delivery body (new session, the spare marker) and
#                                   print the command the USER runs with their own token
#   run-step8-resume.sh r1b-check   after the user's delivery: its answer, the wait, every §6.2 check
#   run-step8-resume.sh r2g1        observation deadline (s8-r1's end + 7 d) → R2 (R1b's session) → G1
# Any failed check is a hard stop (exit 3, "STOP: <reason>"): nothing further is sent, no retry, no
# second spare. The checks count the ledger's EFFECTIVE records (`ieo-role-call records`), so the
# settled s8-r1 counts as settled. Tokens come from header files and are never printed.
# Env: COORD, TOKDIR, OUT, LEDGER, ADAPTER_HEADER (as run-step8.sh), PLAN (the 02-05 plan for
# ieo-role-call), ROLE_CALL (the new ieo-role-call); settle: TRANSCRIPT, ORIGINAL_PATH,
# ORIGINAL_SHA256, DECISION, CHECK_REPORT, CHECK_VERDICT, COORD_ROLES (the coordinator's --roles
# file: after the settlement the ledger is read again with the coordinator's own budget plan).
# Before each call the coordinator's observation (the newest record's stream) must be younger
# than 7 days minus 10 minutes: s8-r1's before R1b, s8-r1b's before R2, s8-r2's before G1.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib-step8.sh"
PHASE=${1:?settle|r1b-body|r1b-check|r2g1}
COORD=${COORD:-http://127.0.0.1:58090}
: "${TOKDIR:?}" "${OUT:?}" "${LEDGER:?}" "${ADAPTER_HEADER:?}" "${PLAN:?}" "${ROLE_CALL:?}"
REPO_ID=${REPO_ID:-ieo-poc-test}
PORT_DEV=${PORT_DEV:-58221}; PORT_GR=${PORT_GR:-58222}; PROMPTS=${PROMPTS:-$HERE/prompts}
mkdir -p "$OUT"
export R1_ATTEMPT=s8-r1b SETTLED=s8-r1 EFFECTIVE_RECORDS="$OUT/effective-records.jsonl"
effective() { "$ROLE_CALL" records --plan "$PLAN" > "$EFFECTIVE_RECORDS.tmp" && mv -f "$EFFECTIVE_RECORDS.tmp" "$EFFECTIVE_RECORDS"; }
# the observation the coordinator uses for the next call: the newest record (in ledger order) with
# a kept stream; it must be younger than 7 days minus a 10-minute margin (review 4b427c5 F-3)
deadline() { # what is about to be sent
  effective || stop "the ledger's effective records do not read"
  python3 -I - "$EFFECTIVE_RECORDS" "$1" <<'PY' || stop "$1: the observation is too old (above)"
import json, sys, time, datetime
recs = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
newest = next(r for r in reversed(recs) if (r.get("stream") or {}).get("path") and isinstance(r.get("ended_unix"), int))
end = newest["ended_unix"] + 604800
left = end - 600 - int(time.time())
iso = datetime.datetime.utcfromtimestamp(end).strftime("%Y-%m-%dT%H:%M:%SZ")
print(f"{sys.argv[2]}: observation = {newest['attempt_id']}'s stream, valid until {iso} ({left // 60} min left before the margin)")
sys.exit(0 if left > 0 else 1)
PY
}
# the spare of s8-r1b is a person's and replaces s8-r1
spare_check() {
  python3 -I - "$EFFECTIVE_RECORDS" <<'PY' || stop "s8-r1b is not the spare replacing s8-r1"
import json, sys
r = [x for x in (json.loads(l) for l in open(sys.argv[1]) if l.strip()) if x.get("attempt_id") == "s8-r1b"][0]
s = r.get("spare") or {}
print("s8-r1b spare: replaces", s.get("replaces"), "by", (s.get("by") or {}).get("kind"), (s.get("by") or {}).get("name"))
sys.exit(0 if s.get("replaces") == "s8-r1" and (s.get("by") or {}).get("kind") == "user" else 1)
PY
}
# after a delivery was accepted: wait, settle, the answers, every check (run-step8.sh run_one's tail)
after() { # tag role assignment attempt port
  local tag=$1 role=$2 aid=$3 att=$4 port=$5
  if [ -f "$LEDGER/records.jsonl" ] && grep -q "\"attempt_id\":\"$att\"" "$LEDGER/records.jsonl"; then
    role_status "$port" "status-$att"
  else
    role_wait "$port" "$att" || stop "$att: the role did not finish"
  fi
  settled "$att" || stop "$att: no ledger record, or the in-flight marker stays"
  req "roles-$att" s8-operator GET "/v0/repos/$REPO_ID/roles" >/dev/null
  req "chain-$aid-$att" s8-operator GET "/v0/coordinator/assignments/$aid/generations/0/chain" >/dev/null
  effective || stop "$att: the ledger's effective records do not read"
  python3 -I "$HERE/check-run.py" "$LEDGER" "$OUT" "$att" "$role" "$aid" || stop "$att: a run check failed (above)"
}
run_one() { # tag role assignment attempt key port decision op prompt-file
  local tag=$1 role=$2 aid=$3 att=$4 key=$5 port=$6 dec=$7 op=$8 pf=$9 sid st got
  if [ -e "$OUT/$tag-body.json" ]; then sid=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['session_id'])" "$OUT/$tag-body.json")
  else sid=$(cat /proc/sys/kernel/random/uuid); fi
  [[ "$sid" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] || stop "$att: session id is not a lowercase UUID"
  [ -e "$OUT/$tag-body.json" ] || python3 -I -c "import json,sys; a=sys.argv; json.dump({'assignment_id': a[1], 'generation': 0, 'attempt_id': a[2], 'session_id': a[3], 'prompt': open(a[4]).read().strip()}, open(a[5], 'w'))" \
    "$aid" "$att" "$sid" "$pf" "$OUT/$tag-body.json"
  st=$(req "$tag" s8-operator POST "/v0/repos/$REPO_ID/roles/$role/deliveries" "$OUT/$tag-body.json" "$key")
  got=$(j "$tag" "(d['result']['decision'], d['result']['op'], d['result']['outcome'])" 2>/dev/null)
  { [ "$st" = 201 ] || [ "$st" = 200 ]; } && [ "$got" = "('$dec', '$op', 'accepted')" ] || stop "$att: delivery answered $st $got: $(head -c 600 "$OUT/$tag.json")"
  echo "$att: $got"
  after "$tag" "$role" "$aid" "$att" "$port"
}
case "$PHASE" in
settle)
  extra=(); [ "${DRY:-0}" = 1 ] && extra=(--dry-run)
  err=$(mktemp)
  if ! out=$("$ROLE_CALL" settle --plan "$PLAN" --attempt s8-r1 --transcript "${TRANSCRIPT:?}" \
    --original-path "${ORIGINAL_PATH:?}" --original-sha256 "${ORIGINAL_SHA256:?}" --decision "${DECISION:?}" \
    --independent-check "${CHECK_REPORT:?}" --independent-verdict "${CHECK_VERDICT:?}" "${extra[@]}" 2> "$err"); then
    why=$(cat "$err"); rm -f "$err"; n=$(wc -l < "$LEDGER/records.jsonl")
    # review 4b427c5 F-6: an appended line whose read-back failed is not "nothing written"
    case "$why" in
    *"was appended"*) stop "settle FAILED AFTER APPENDING (records.jsonl now $n lines): the ledger refuses until a person inspects it: $why";;
    *) stop "the settlement was refused, nothing written (records.jsonl $n lines): $why";;
    esac
  fi
  rm -f "$err"
  printf '%s\n' "$out" | head -1
  effective || stop "the ledger's effective records do not read"
  if [ "${DRY:-0}" != 1 ]; then
    # the coordinator's own reading (PR #90 review 7b904de N-2): its budget plan from its --roles
    # file, exactly as it reads the ledger, must count s8-r1 as settled before R1b
    python3 -I - "${COORD_ROLES:?}" "$REPO_ID" "$OUT/plan-coordinator.json" <<'PY' || stop "no budget plan in ${COORD_ROLES}"
import json, sys
roles, repo, out = sys.argv[1:4]
e = json.load(open(roles))[repo]
bp = e["budget_plan"]
plan = {"budget": bp["budget"], "ledger_dir": bp["ledger_dir"],
        "roles": {r: {"url": e[r]["url"], "adapter_token_file": e[r]["adapter_token_file"]}
                  for r in ("development", "guardrail")}}
if "first_observation" in bp:
    plan["first_observation"] = bp["first_observation"]
json.dump(plan, open(out, "w"), indent=1)
print("coordinator's ledger:", bp["ledger_dir"])
PY
    "$ROLE_CALL" records --plan "$OUT/plan-coordinator.json" > "$OUT/effective-coordinator.jsonl" \
      || stop "the ledger does not read with the coordinator's budget plan"
    python3 -I - "$OUT/effective-coordinator.jsonl" <<'PY' || stop "the coordinator's reading does not count s8-r1 as settled"
import json, sys
r = [x for x in (json.loads(l) for l in open(sys.argv[1]) if l.strip()) if x.get("attempt_id") == "s8-r1"][0]
s = r.get("settlement") or {}
print("coordinator's reading: s8-r1", r["usage"]["run"]["state"], "| settlement", s.get("policy"))
sys.exit(0 if s.get("policy") == "guard_stop_on_old_rule_warning" and r["usage"]["run"]["state"] == "measured" else 1)
PY
  fi
  python3 -I - "$EFFECTIVE_RECORDS" "${DRY:-0}" <<'PY' || stop "s8-r1 does not count through its settlement"
import json, sys
recs = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
r = [x for x in recs if x.get("attempt_id") == "s8-r1"][0]
s = r.get("settlement") or {}
if sys.argv[2] == "1":
    print("dry run: s8-r1 still", r["usage"]["run"]["state"]); sys.exit(0)
t = r["usage"]["run"].get("tokens") or {}
print("s8-r1 counts:", r["usage"]["run"]["state"], t, "stop_required", r["stop_required"], r["stop_reason"],
      "| settlement", s.get("policy"), s.get("independent_check", {}).get("verdict"))
sys.exit(0 if s.get("policy") == "guard_stop_on_old_rule_warning" and r["usage"]["run"]["state"] == "measured"
         and r["stop_required"] is True and r["stop_reason"] == "no result event" else 1)
PY
  ;;
r1b-body)
  deadline s8-r1b
  python3 -I - "$EFFECTIVE_RECORDS" <<'PY' || stop "s8-r1 is not settled: the spare cannot replace it"
import json, sys
r = [x for x in (json.loads(l) for l in open(sys.argv[1]) if l.strip()) if x.get("attempt_id") == "s8-r1"][0]
sys.exit(0 if (r.get("settlement") or {}).get("policy") == "guard_stop_on_old_rule_warning" else 1)
PY
  if [ ! -e "$OUT/r1b-body.json" ]; then
    sid=$(cat /proc/sys/kernel/random/uuid)
    python3 -I - "$sid" "$PROMPTS/R1.txt" "$OUT/r1b-body.json" <<'PY'
import json, sys
sid, pf, out = sys.argv[1:4]
json.dump({"assignment_id": "S8-DEV-A1", "generation": 0, "attempt_id": "s8-r1b", "session_id": sid,
           "prompt": open(pf).read().strip(), "new_session": True,
           "spare": {"reason": "s8-r1 was stopped by the run guard on allowed_warning under the rule before "
                               "runtime contract rev 19; settled from its session transcript (independent check "
                               "CONFIRMED WITH CAVEAT); rerun as a new session (user decision 2026-11-17 "
                               "r1-settlement (A), special case for s8-r1 only)",
                     "replaces": "s8-r1",
                     "evidence_ref": "docs/poc/User_Decisions_2026-11-17_r1-settlement.md; "
                                     "docs/poc/reports/tasks/02-05/step8-plan-20261117/RESUME_PLAN.md; "
                                     "docs/poc/reports/tasks/02-05/step8-plan-20261117/evidence/r1-usage-verification.md"}},
          open(out, "w"))
PY
  fi
  echo "R1b body: $OUT/r1b-body.json (session $(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['session_id'])" "$OUT/r1b-body.json"), new_session, spare replaces s8-r1)"
  cat <<CMD
# the USER runs (own token, never printed; the coordinator records the person and the time):
T=/opt/rust/src/github.com/HeaInSeo/ieo_poc-data/private/user/heain.token
curl -sS --max-time 150 -o $OUT/r1b.json -w 'http %{http_code}\n' \\
  -H @<(printf 'Authorization: Bearer %s\n' "\$(cat "\$T")") -H 'Content-Type: application/json' -H 'Idempotency-Key: s8-dlv-r1b-0001' \\
  -X POST $COORD/v0/repos/$REPO_ID/roles/development/deliveries --data @$OUT/r1b-body.json
python3 -I -c 'import json; d=json.load(open("$OUT/r1b.json")); r=d.get("result") or {}; print(r.get("decision"), r.get("op"), r.get("outcome"), d.get("code"), d.get("message"))'
# expected: "http 201" (or 200 for the same request again), then "resume_allowed start accepted None None"
CMD
  ;;
r1b-check)
  [ -s "$OUT/r1b.json" ] || stop "no answer of the user's R1b delivery in $OUT/r1b.json"
  got=$(j r1b "(d['result']['decision'], d['result']['op'], d['result']['outcome'])" 2>/dev/null)
  [ "$got" = "('resume_allowed', 'start', 'accepted')" ] || stop "s8-r1b: the delivery answered $got: $(head -c 600 "$OUT/r1b.json")"
  echo "s8-r1b: $got"
  after r1b development S8-DEV-A1 s8-r1b "$PORT_DEV"
  spare_check
  ;;
r2g1)
  # review 4b427c5 F-2: R1b's checks again first (the answers r1b-check saved), nothing is sent
  # unless every one of them holds; R2's own checks gate G1 (run_one stops on any)
  [ -s "$OUT/status-s8-r1b.json" ] && [ -s "$OUT/chain-S8-DEV-A1-s8-r1b.json" ] || stop "no r1b-check answers in $OUT: run r1b-check first"
  effective || stop "the ledger's effective records do not read"
  python3 -I "$HERE/check-run.py" "$LEDGER" "$OUT" s8-r1b development S8-DEV-A1 || stop "s8-r1b: a run check fails: no R2 (above)"
  spare_check
  deadline s8-r2
  run_one r2 development S8-DEV-A1 s8-r2 s8-dlv-r2-0001 "$PORT_DEV" resume_allowed reconnect "$PROMPTS/R2.txt"
  python3 -I "$HERE/check-run.py" --g1-prompt "$LEDGER" "$PROMPTS/G1-head.txt" "$OUT/G1.txt" || stop "G1 prompt (above)"
  deadline s8-g1
  run_one g1 guardrail S8-GR-A1 s8-g1 s8-dlv-g1-0001 "$PORT_GR" no_checkpoint start "$OUT/G1.txt"
  echo "R1b, R2 and G1 recorded and checked"
  ;;
*) stop "unknown phase $PHASE";;
esac
