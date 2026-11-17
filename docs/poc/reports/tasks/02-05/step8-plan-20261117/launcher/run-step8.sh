#!/bin/bash
# 02-05 step 8: the run itself (EXECUTION_PLAN.md §5.2 steps 6 and 8, §6), in three phases:
#   run-step8.sh register   the step-8 sprint revision (then the user's semantic review)
#   run-step8.sh prepare    review passed? roles reachable, central, no quota mismatch → the two
#                           assignments and the branch_head input
#   run-step8.sh runs       observation deadline → R1 → R2 → G1, each followed by every §6.2 check
# Any failed check is a hard stop (exit 3, "STOP: <reason>"): nothing further is sent, no retry.
# A request that got no answer is sent again only with the same key and body (it reads the stored
# answer; it never starts a second run). Tokens come from header files and are never printed.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib-step8.sh"
PHASE=${1:?register|prepare|runs}
COORD=${COORD:-http://127.0.0.1:58090}
: "${TOKDIR:?}" "${OUT:?}" "${LEDGER:?}" "${ADAPTER_HEADER:?}"
REPO_ID=${REPO_ID:-ieo-poc-test}; SPRINT=${SPRINT:-POC-S8-01}; REV=${REV:-1}; TASK=${TASK:-90-01}
TARGET_SHA=${TARGET_SHA:-2ab9d8760012516f37674a9446ca2fb2699973c0}; BRANCH_REF=${BRANCH_REF:-refs/heads/main}
PORT_DEV=${PORT_DEV:-58221}; PORT_GR=${PORT_GR:-58222}
SPEC=${SPEC:-$HERE/../sprint-POC-S8-01-r1.json}; PROMPTS=${PROMPTS:-$HERE/prompts}
mkdir -p "$OUT"
case "$PHASE" in
register)
  st=$(req reg s8-registrant POST '/v0/sprints?mode=executable' "$SPEC" s8-reg-0001)
  [ "$st" = 201 ] || [ "$st" = 200 ] || stop "registration answered $st: $(cat "$OUT/reg.json")"
  dig=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['digest'])" "$SPEC")
  grep -q "$dig" "$OUT/reg.json" || stop "the registered digest is not the spec's $dig"
  echo "registered $SPRINT r$REV ($st), digest $dig: the user's semantic review is next (EXECUTION_PLAN.md §2.2)";;
prepare)
  st=$(req sprint s8-registrant GET "/v0/sprints/$SPRINT/revisions/$REV")
  [ "$st" = 200 ] && grep -q semantic_review_passed "$OUT/sprint.json" || stop "$SPRINT has no passed semantic review yet ($st)"
  st=$(req roles s8-operator GET "/v0/repos/$REPO_ID/roles")
  [ "$st" = 200 ] || stop "role status answered $st"
  python3 -I - "$OUT/roles.json" <<'PY' || stop "role services not ready (above)"
import json, sys
d = json.load(open(sys.argv[1]))
bad = [f"{r}: unreachable" for r, x in d["roles"].items() if not x.get("reachable") or "status" not in x]
bad += [f"{r}: local_only" for r, x in d["roles"].items() if x.get("status", {}).get("central_checkpoint", {}).get("mode") == "local_only"]
bad += [f"{r}: running" for r, x in d["roles"].items() if x.get("status", {}).get("running") is not None]
bad += [f"quota mismatch {m}" for m in d["quota_groups"].get("mismatches", [])]
print("\n".join(bad) if bad else "roles ready: both reachable, central, idle, no quota mismatch")
sys.exit(1 if bad else 0)
PY
  asg() { printf '{"repo_id": "%s", "role": "%s", "assignment_id": "%s", "generation": 0, "sprint_id": "%s", "sprint_revision": %s, "task_id": "%s", "target_sha": "%s", "expected_current": %s, "limits": {"elapsed_seconds": %s, "total_tokens": %s}%s}\n' \
      "$REPO_ID" "$1" "$2" "$SPRINT" "$REV" "$TASK" "$TARGET_SHA" "$3" "$4" "$5" "$6" > "$OUT/asg-$1-body.json"; }
  asg development S8-DEV-A1 "${EXPECTED_CURRENT_DEV:-null}" 1800 150000 ", \"branch_ref\": \"$BRANCH_REF\""
  asg guardrail S8-GR-A1 "${EXPECTED_CURRENT_GR:-null}" 600 50000 ""
  for r in development guardrail; do
    k=s8-asg-dev-0001; [ "$r" = guardrail ] && k=s8-asg-gr-0001
    st=$(req "asg-$r" s8-assigner POST /v0/coordinator/assignments "$OUT/asg-$r-body.json" "$k")
    [ "$st" = 201 ] || [ "$st" = 200 ] || stop "$r assignment answered $st: $(cat "$OUT/asg-$r.json")"
  done
  printf '{"evidence_id": "s8-gh-head-1", "repo_id": "%s", "kind": "branch_head", "sha": "%s", "ref": "%s", "source_ref": "%s"}\n' \
    "$REPO_ID" "$TARGET_SHA" "$BRANCH_REF" "${HEAD_SOURCE:-github:HeaInSeo/ieo_poc_test $BRANCH_REF (git ls-remote)}" > "$OUT/gh-body.json"
  st=$(req gh s8-ingest POST /v0/coordinator/evidence "$OUT/gh-body.json")
  [ "$st" = 201 ] || [ "$st" = 200 ] || stop "branch_head answered $st: $(cat "$OUT/gh.json")"
  echo "prepared: S8-DEV-A1, S8-GR-A1, branch_head $TARGET_SHA";;
runs)
  : "${OBS_UNIX:?observed_unix of the first observation}"
  # the no-overage observation is valid for 7 days; R1 must be admitted with a margin of 10 min
  [ "$(date +%s)" -lt $((OBS_UNIX + 604800 - 600)) ] || stop "the first observation is too old for R1 (deadline $(date -u -d @$((OBS_UNIX + 604800)) +%FT%TZ))"
  run_one() { # tag role assignment attempt key port decision op prompt-file
    local tag=$1 role=$2 aid=$3 att=$4 key=$5 port=$6 dec=$7 op=$8 pf=$9 sid st got
    if [ -e "$OUT/$tag-body.json" ]; then sid=$(python3 -I -c "import json,sys; print(json.load(open(sys.argv[1]))['session_id'])" "$OUT/$tag-body.json")
    else sid=$(cat /proc/sys/kernel/random/uuid); fi
    [[ "$sid" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] || stop "$att: session id is not a lowercase UUID"
    # the body is written once and kept: a resend uses the same key and the same bytes
    [ -e "$OUT/$tag-body.json" ] || python3 -I -c "import json,sys; a=sys.argv; json.dump({'assignment_id': a[1], 'generation': 0, 'attempt_id': a[2], 'session_id': a[3], 'prompt': open(a[4]).read().strip()}, open(a[5], 'w'))" \
      "$aid" "$att" "$sid" "$pf" "$OUT/$tag-body.json"
    st=$(req "$tag" s8-operator POST "/v0/repos/$REPO_ID/roles/$role/deliveries" "$OUT/$tag-body.json" "$key")
    got=$(j "$tag" "(d['result']['decision'], d['result']['op'], d['result']['outcome'])" 2>/dev/null)
    { [ "$st" = 201 ] || [ "$st" = 200 ]; } && [ "$got" = "('$dec', '$op', 'accepted')" ] || stop "$att: delivery answered $st $got: $(head -c 600 "$OUT/$tag.json")"
    echo "$att: $got"
    # a rerun after this attempt was recorded (review a5f9402 I-3): the stored answer above, no wait
    if [ -f "$LEDGER/records.jsonl" ] && grep -q "\"attempt_id\":\"$att\"" "$LEDGER/records.jsonl"; then
      role_status "$port" "status-$att"
    else
      role_wait "$port" "$att" || stop "$att: the role did not finish"
    fi
    settled "$att" || stop "$att: no ledger record, or the in-flight marker stays"
    st=$(req "roles-$att" s8-operator GET "/v0/repos/$REPO_ID/roles")
    st=$(req "chain-$aid-$att" s8-operator GET "/v0/coordinator/assignments/$aid/generations/0/chain")
    python3 -I "$HERE/check-run.py" "$LEDGER" "$OUT" "$att" "$role" "$aid" || stop "$att: a run check failed (above)"
  }
  run_one r1 development S8-DEV-A1 s8-r1 s8-dlv-r1-0001 "$PORT_DEV" no_checkpoint start "$PROMPTS/R1.txt"
  run_one r2 development S8-DEV-A1 s8-r2 s8-dlv-r2-0001 "$PORT_DEV" resume_allowed reconnect "$PROMPTS/R2.txt"
  # G1: the developer's answers as numbered claims, one per output line, at most 2 KB
  python3 -I "$HERE/check-run.py" --g1-prompt "$LEDGER" "$PROMPTS/G1-head.txt" "$OUT/G1.txt" || stop "G1 prompt (above)"
  run_one g1 guardrail S8-GR-A1 s8-g1 s8-dlv-g1-0001 "$PORT_GR" no_checkpoint start "$OUT/G1.txt"
  echo "R1, R2 and G1 recorded and checked";;
*) stop "unknown phase $PHASE";;
esac
