#!/bin/bash
# 02-05 step 8 resume (RESUME_PLAN.md §5): swap the role containers' ieo-supervisor in place.
#   restart-roles.sh swap <workdir> <new ieo-supervisor> <its sha256> <old sha256>
#   restart-roles.sh rollback <workdir> <expected sha256 of the kept binary>
# The run guard runs inside the role containers (W/bin is mounted read-only at /opt/ieo), so the
# warning rule needs the new binary there. The containers and their volumes stay: the
# development role keeps its local checkpoint (s8-r1 after_run), which the coordinator compares.
#   swap:     both containers running and idle (status 200, `running` present and null) → the
#             current binary kept as W/bin/ieo-supervisor.prev (once; an existing .prev must be
#             that same binary) → both containers stopped (podman stop, their stop timeout) →
#             the new binary copied in next to it, its SHA-256 checked, renamed over
#             W/bin/ieo-supervisor → both started → each answers idle and runs the binary with
#             that SHA-256. Any failure after the first stop prints the exact rollback command.
#   rollback: from any state a swap can leave (PR #91 review 4b427c5 F-1): a container that is
#             stopped, or running but not answering (crashing), or answering idle, is accepted;
#             a role that reports a run or whose process list cannot confirm no Claude process is refused.
#             Then W/bin/ieo-supervisor.prev (with the expected SHA-256) goes back in
#             the same way, running containers are stopped, both are started and checked.
# Never touches W/<role>/cred, never removes or recreates a container or a volume, prints no
# token. Env: NAME_PREFIX (ieo-s8), REPO_ID (ieo-poc-test), PORT_DEV/PORT_GR (58221/58222).
# Exit 0 done; 3 STOP (nothing more is done; the message says what state it left).
set -uo pipefail
STOPPED=no
die() {
  echo "STOP: $*"
  if [ "$MODE" = swap ] && [ "$STOPPED" = yes ]; then
    echo "ROLLBACK: bash $0 rollback $W${KEPT:+ $KEPT}"
  fi
  exit 3
}
MODE=${1:-}; W=$(realpath -m "${2:-}")
NAME_PREFIX=${NAME_PREFIX:-ieo-s8}; REPO_ID=${REPO_ID:-ieo-poc-test}
PORT_DEV=${PORT_DEV:-58221}; PORT_GR=${PORT_GR:-58222}
declare -A PORT=([development]=$PORT_DEV [guardrail]=$PORT_GR)
BIN="$W/bin/ieo-supervisor"; PREV="$W/bin/ieo-supervisor.prev"; HDR="$W/adapter.header"
KEPT=""
case "$MODE" in
swap) NEW=$(realpath "${3:?new ieo-supervisor}"); WANT=${4:?its sha256}; KEPT=${5:?old sha256};;
rollback)
  NEW=$PREV; [ -f "$PREV" ] || die "no $PREV to roll back to"; WANT=${3:?old sha256};;
*) echo "usage: restart-roles.sh swap <workdir> <new ieo-supervisor> <sha256> <old sha256> | rollback <workdir> <old sha256>"; exit 2;;
esac
if [ ! -f "$BIN" ] || [ ! -f "$HDR" ]; then die "$W is not a role workdir (no bin/ieo-supervisor or adapter.header)"; fi
[ "$(sha256sum < "$NEW" | cut -c1-64)" = "$WANT" ] || die "$NEW does not have the expected SHA-256 $WANT"
name() { echo "$NAME_PREFIX-$REPO_ID-$1"; }
status() { curl -s --max-time 5 -o "$2" -w '%{http_code}' -H @"$HDR" "http://127.0.0.1:${PORT[$1]}/v0/role/status"; }
# answer: idle (200, `running` present and null: review 4b427c5 F-7) | busy (200, a run) | none
answer() {
  local f st rc; f=$(mktemp) || { echo none; return; }
  st=$(status "$1" "$f") || st=000
  python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if "running" in d and d["running"] is None else 4 if d.get("running") is not None else 5)' "$f" 2>/dev/null
  rc=$?; rm -f "$f"
  if [ "$st" = 200 ] && [ $rc = 0 ]; then echo idle; elif [ "$st" = 200 ] && [ $rc = 4 ]; then echo busy; else echo none; fi; }
running() { # the container's state: true | false (a missing container is a STOP)
  podman inspect -f '{{.State.Running}}' "$(name "$1")" 2>/dev/null || die "no container $(name "$1")"; }
check_processes() {
  local processes rc
  processes=$(podman top "$(name "$1")" pid args 2>/dev/null) \
    || die "could not inspect processes in role $1: nothing changed"
  awk '
    NR == 1 { if (NF != 2 || $1 != "PID" || $2 != "COMMAND") invalid = 1; next }
    NF < 2 || $1 !~ /^[1-9][0-9]*$/ || seen[$1]++ { invalid = 1; next }
    /(^|[[:space:]])(\/opt\/claude\/claude|claude|\[claude\])([[:space:]]|$)/ { busy = 1 }
    END {
      if (invalid || !seen[1]) exit 2
      if (busy) exit 3
    }
  ' <<< "$processes"
  rc=$?
  case "$rc" in
    0) ;;
    3) die "role $1 has a claude process: nothing changed";;
    *) die "unexpected process list in role $1: nothing changed";;
  esac
}
declare -A WAS
for r in development guardrail; do
  WAS[$r]=$(running "$r")
  a=none; [ "${WAS[$r]}" = true ] && a=$(answer "$r")
  case "$MODE:${WAS[$r]}:$a" in
  swap:true:idle|rollback:true:idle|rollback:true:none) check_processes "$r";;
  rollback:false:none) ;;
  *:true:busy) die "role $r reports a run: nothing changed";;
  *) die "role $r is not idle (container running: ${WAS[$r]}, answer: $a): nothing changed";;
  esac
done
OLD=$(sha256sum < "$BIN" | cut -c1-64)
echo "# role binary: $OLD → $WANT ($MODE) in $W/bin"
if [ "$MODE" = swap ]; then
  if [ -e "$PREV" ]; then
    [ "$(sha256sum < "$PREV" | cut -c1-64)" = "$KEPT" ] \
      || die "$PREV does not match the expected rollback SHA-256: nothing changed"
    [[ "$OLD" == "$KEPT" || "$OLD" == "$WANT" ]] \
      || die "current binary is neither the expected old nor new binary: nothing changed"
  else
    [ "$OLD" = "$KEPT" ] || die "current binary does not match the expected rollback SHA-256: nothing changed"
    cp -p "$BIN" "$PREV" && [ "$(sha256sum < "$PREV" | cut -c1-64)" = "$KEPT" ] \
      || die "could not keep $PREV: role binaries and containers unchanged"
  fi
fi
for r in development guardrail; do
  [ "${WAS[$r]}" = true ] || continue
  STOPPED=yes
  podman stop "$(name "$r")" >/dev/null || die "podman stop $r failed"
done
STOPPED=yes
echo "# both role containers stopped"
if ! { cp "$NEW" "$BIN.new" && chmod 755 "$BIN.new" && [ "$(sha256sum < "$BIN.new" | cut -c1-64)" = "$WANT" ] && mv -f "$BIN.new" "$BIN"; }; then
  rm -f "$BIN.new"; die "binary not replaced ($BIN has the SHA-256 $(sha256sum < "$BIN" | cut -c1-64); containers stopped)"
fi
echo "# $BIN now $(sha256sum < "$BIN" | cut -c1-64)"
for r in development guardrail; do
  podman start "$(name "$r")" >/dev/null || die "podman start $r failed"
done
for r in development guardrail; do
  up=no
  for _ in $(seq 1 100); do [ "$(answer "$r")" = idle ] && { up=yes; break; }; sleep 0.2; done
  [ "$up" = yes ] || die "role $r does not answer idle after the restart"
  got=$(podman exec "$(name "$r")" sha256sum /opt/ieo/ieo-supervisor 2>/dev/null | cut -c1-64)
  [ "$got" = "$WANT" ] || die "role $r runs a binary with SHA-256 '$got', not $WANT"
  echo "role $r: up, idle, /opt/ieo/ieo-supervisor $WANT"
done
echo "# done ($MODE)${KEPT:+; rollback: bash $0 rollback $W $KEPT}"
