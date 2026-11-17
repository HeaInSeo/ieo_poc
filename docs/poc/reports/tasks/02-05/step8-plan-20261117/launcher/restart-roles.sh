#!/bin/bash
# 02-05 step 8 resume (RESUME_PLAN.md §5): swap the role containers' ieo-supervisor in place.
#   restart-roles.sh swap <workdir> <new ieo-supervisor> <its sha256>
#   restart-roles.sh rollback <workdir>
# The run guard runs inside the role containers (W/bin is mounted read-only at /opt/ieo), so the
# warning rule needs the new binary there. The containers and their volumes stay: the
# development role keeps its local checkpoint (s8-r1 after_run), which the coordinator compares.
#   swap:     both roles idle (status 200, running null) → the current binary kept as
#             W/bin/ieo-supervisor.prev (once; an existing .prev must be that same binary) →
#             both containers stopped (podman stop, their stop timeout) → the new binary copied in
#             next to it, its SHA-256 checked, renamed over W/bin/ieo-supervisor → both started →
#             each answers status 200 with running null and runs the binary with that SHA-256.
#   rollback: the same with W/bin/ieo-supervisor.prev as the binary.
# Never touches W/<role>/cred, never removes or recreates a container or a volume, prints no
# token. Env: NAME_PREFIX (ieo-s8), REPO_ID (ieo-poc-test), PORT_DEV/PORT_GR (58221/58222).
# Exit 0 done; 3 STOP (nothing more is done; the message says what state it left).
set -uo pipefail
die() { echo "STOP: $*"; exit 3; }
MODE=${1:-}; W=$(realpath -m "${2:-}")
NAME_PREFIX=${NAME_PREFIX:-ieo-s8}; REPO_ID=${REPO_ID:-ieo-poc-test}
PORT_DEV=${PORT_DEV:-58221}; PORT_GR=${PORT_GR:-58222}
declare -A PORT=([development]=$PORT_DEV [guardrail]=$PORT_GR)
BIN="$W/bin/ieo-supervisor"; PREV="$W/bin/ieo-supervisor.prev"; HDR="$W/adapter.header"
case "$MODE" in
swap) NEW=$(realpath "${3:?new ieo-supervisor}"); WANT=${4:?its sha256};;
rollback) NEW=$PREV; [ -f "$PREV" ] || die "no $PREV to roll back to"; WANT=$(sha256sum < "$PREV" | cut -c1-64);;
*) echo "usage: restart-roles.sh swap <workdir> <new ieo-supervisor> <sha256> | rollback <workdir>"; exit 2;;
esac
[ -f "$BIN" ] && [ -f "$HDR" ] || die "$W is not a role workdir (no bin/ieo-supervisor or adapter.header)"
[ "$(sha256sum < "$NEW" | cut -c1-64)" = "$WANT" ] || die "$NEW does not have the SHA-256 $WANT"
status() { curl -s --max-time 5 -o "$2" -w '%{http_code}' -H @"$HDR" "http://127.0.0.1:${PORT[$1]}/v0/role/status"; }
idle() { # role: status 200 and nothing running
  local f; f=$(mktemp); local st; st=$(status "$1" "$f")
  python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get("running") is None else 1)' "$f" 2>/dev/null
  local rc=$?; rm -f "$f"; [ "$st" = 200 ] && [ $rc = 0 ]; }
for r in development guardrail; do
  idle "$r" || die "role $r is not idle (or does not answer): nothing changed"
done
OLD=$(sha256sum < "$BIN" | cut -c1-64)
echo "# role binary: $OLD → $WANT ($MODE) in $W/bin"
if [ "$MODE" = swap ]; then
  if [ -e "$PREV" ]; then
    [ "$(sha256sum < "$PREV" | cut -c1-64)" = "$OLD" ] || [ "$OLD" = "$WANT" ] \
      || die "$PREV exists and is not the current binary: nothing changed (a person looks)"
  else
    cp -p "$BIN" "$PREV" && [ "$(sha256sum < "$PREV" | cut -c1-64)" = "$OLD" ] || die "could not keep $PREV: nothing changed"
  fi
fi
for r in development guardrail; do
  podman stop "$NAME_PREFIX-$REPO_ID-$r" >/dev/null || die "podman stop $r failed (the other role may be stopped; start it with podman start)"
done
echo "# both role containers stopped"
cp "$NEW" "$BIN.new" && chmod 755 "$BIN.new" && [ "$(sha256sum < "$BIN.new" | cut -c1-64)" = "$WANT" ] && mv -f "$BIN.new" "$BIN" \
  || die "binary not replaced (containers stopped; the old binary is still $BIN unless the rename happened: check its SHA-256, then podman start both)"
echo "# $BIN now $(sha256sum < "$BIN" | cut -c1-64)"
for r in development guardrail; do
  podman start "$NAME_PREFIX-$REPO_ID-$r" >/dev/null || die "podman start $r failed"
done
for r in development guardrail; do
  up=no
  for _ in $(seq 1 100); do idle "$r" && { up=yes; break; }; sleep 0.2; done
  [ "$up" = yes ] || die "role $r does not answer idle after the restart"
  got=$(podman exec "$NAME_PREFIX-$REPO_ID-$r" sha256sum /opt/ieo/ieo-supervisor 2>/dev/null | cut -c1-64)
  [ "$got" = "$WANT" ] || die "role $r runs a binary with SHA-256 '$got', not $WANT"
  echo "role $r: up, idle, /opt/ieo/ieo-supervisor $WANT"
done
echo "# done ($MODE); rollback: restart-roles.sh rollback $W"
