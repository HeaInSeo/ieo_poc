#!/bin/bash
# 02-05 step 8: stopping and starting the coordinator host process (EXECUTION_PLAN.md §5.2, §5.4).
#   coord-switch.sh capture <pid file> <expected binary> <port> <out dir>
#       [read] checks that the pid is that binary listening on 127.0.0.1:<port>, shows its parent
#       and cgroup (stops when it runs as a systemd service: a unit would restart it), counts the
#       established clients, and copies its argv, environment and working directory into <out dir>
#       (argv0, env0: NUL-separated as in /proc; cwd; all 0600). Every path argument (--tokens,
#       --manifest, --profiles, --bindings, --policy, --roles) must exist from that cwd. Prints
#       names and paths only, never an environment value.
#   coord-switch.sh stop <pid file> [timeout s]
#       SIGTERM, wait, SIGKILL after the timeout (default 30; the new build waits for runs in
#       progress: give it 900). Refuses a pid file whose process is gone.
#   coord-switch.sh start <binary> <out dir> <pid file> <log> [--tokens F] [-- extra args…]
#       starts <binary> detached in the captured working directory with exactly the captured
#       environment (exported by shell builtins, so no value is on any command line; exported shell
#       functions BASH_FUNC_* and names that are no identifiers are dropped) and the captured argv
#       (argv[0] replaced, --tokens replaced when given, extra args appended); writes the pid file
#       and waits for GET /v0/health = 200 on the --listen port.
set -uo pipefail
die() { echo "STOP: $*"; exit 3; }
PATHFLAGS=" --tokens --manifest --profiles --bindings --policy --roles "
case "${1:-}" in
capture)
  pf=$2 bin=$3 port=$4 out=$5
  pid=$(cat "$pf") || die "no pid file $pf"
  [ -d "/proc/$pid" ] || die "pid $pid is not running"
  exe=$(readlink "/proc/$pid/exe") || die "cannot read /proc/$pid/exe"
  [ "$exe" = "$(realpath "$bin")" ] || die "pid $pid runs $exe, not $bin"
  mapfile -d '' -t argv < "/proc/$pid/cmdline"
  listen=""; for i in "${!argv[@]}"; do [ "${argv[$i]}" = --listen ] && listen=${argv[$((i + 1))]:-}; done
  [ "$listen" = "127.0.0.1:$port" ] || die "pid $pid listens on '$listen', not 127.0.0.1:$port"
  ppid=$(awk '{print $4}' "/proc/$pid/stat")
  echo "pid $pid: $exe; parent $ppid ($(cat "/proc/$ppid/comm" 2>/dev/null || echo ?))"
  cg=$(cat "/proc/$pid/cgroup"); echo "cgroup: $cg"
  # a *.service unit (other than the user manager itself) would restart the process we stop
  for c in $(echo "${cg#*::}" | tr '/' ' '); do
    case "$c" in user@[0-9]*.service) ;; *.service) die "pid $pid runs in the systemd unit $c: stop it through the unit, not here";; esac
  done
  echo "established clients on :$port: $(ss -Htn state established "( sport = :$port )" | wc -l)"
  cwd=$(readlink "/proc/$pid/cwd") || die "cannot read /proc/$pid/cwd"
  for i in "${!argv[@]}"; do
    case "$PATHFLAGS" in *" ${argv[$i]} "*)
      v=${argv[$((i + 1))]:-}; case "$v" in /*) p=$v;; *) p="$cwd/$v";; esac
      [ -e "$p" ] || die "${argv[$i]} $v does not exist from $cwd"
      echo "${argv[$i]} $v$( [ "$p" = "$v" ] || echo " (relative to $cwd)")";;
    esac
  done
  mkdir -p "$out"; chmod 700 "$out"
  ( umask 077; cp "/proc/$pid/cmdline" "$out/argv0"; cp "/proc/$pid/environ" "$out/env0"; printf '%s' "$cwd" > "$out/cwd" )
  n=0; f=0; while IFS= read -r -d '' kv; do n=$((n + 1)); case "$kv" in BASH_FUNC_*) f=$((f + 1));; esac; done < "$out/env0"
  grep -qz '^IEO_DB_URL=' "$out/env0" || die "the process has no IEO_DB_URL"
  echo "captured ${#argv[@]} argv entries, $n environment entries ($f exported shell functions, dropped at start; IEO_DB_URL present; values not shown), cwd $cwd";;
stop)
  pf=$2; t=${3:-30}
  pid=$(cat "$pf") || die "no pid file $pf"
  [ -d "/proc/$pid" ] || die "pid $pid is not running"
  kill -TERM "$pid"
  for _ in $(seq 1 $((t * 5))); do [ -d "/proc/$pid" ] || break; sleep 0.2; done
  if [ -d "/proc/$pid" ]; then echo "pid $pid still running after $t s: SIGKILL"; kill -KILL "$pid"; sleep 1; fi
  [ -d "/proc/$pid" ] && die "pid $pid survives SIGKILL"
  echo "pid $pid stopped";;
start)
  bin=$(realpath "$2") out=$(realpath "$3") pf=$(realpath -m "$4") log=$(realpath -m "$5"); shift 5
  tok=""; extra=()
  if [ "${1:-}" = --tokens ]; then tok=$(realpath "$2"); shift 2; fi
  [ "${1:-}" = -- ] && { shift; extra=("$@"); }
  mapfile -d '' -t argv < "$out/argv0"; cwd=$(cat "$out/cwd")
  args=("${argv[@]:1}")
  if [ -n "$tok" ]; then
    for i in "${!args[@]}"; do [ "${args[$i]}" = --tokens ] && args[$((i + 1))]=$tok; done
  fi
  args+=("${extra[@]}")
  port=""; for i in "${!args[@]}"; do [ "${args[$i]}" = --listen ] && port=${args[$((i + 1))]##*:}; done
  [ -n "$port" ] || die "no --listen in the captured argv"
  ss -Htln "( sport = :$port )" | grep -q . && die "port $port is still in use"
  [ -d "$cwd" ] || die "the captured working directory $cwd is gone"
  (
    # only the captured environment: clear this one, then export each captured entry with the
    # export builtin (no child process sees a value in its argv)
    for v in $(compgen -e); do unset "$v" 2>/dev/null; done
    while IFS= read -r -d '' kv; do
      name=${kv%%=*}
      case "$name" in BASH_FUNC_*) continue;; esac
      [[ "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
      export "$kv"
    done < "$out/env0"
    cd "$cwd" || exit 1
    exec /usr/bin/setsid /usr/bin/nohup "$bin" "${args[@]}" >> "$log" 2>&1 < /dev/null
  ) &
  pid=$!; echo "$pid" > "$pf"
  for _ in $(seq 1 150); do
    [ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/v0/health")" = 200 ] && { echo "pid $pid: $bin healthy on 127.0.0.1:$port in $cwd (log $log)"; exit 0; }
    [ -d "/proc/$pid" ] || die "$bin exited at start (log $log)"
    sleep 0.2
  done
  die "$bin not healthy after 30 s (log $log)";;
*) echo "usage: coord-switch.sh capture|stop|start …"; exit 2;;
esac
