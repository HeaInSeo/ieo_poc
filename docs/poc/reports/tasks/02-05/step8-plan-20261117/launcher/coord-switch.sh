#!/bin/bash
# 02-05 step 8: stopping and starting the coordinator host process (EXECUTION_PLAN.md §5.2, §5.4).
#   coord-switch.sh capture <pid file> <expected binary> <port> <out dir>
#       [read] checks that the pid is that binary listening on 127.0.0.1:<port>, shows its parent
#       and the established clients, and copies its argv and environment into <out dir>/argv and
#       <out dir>/env (0600, NUL-separated → one per line). Prints names only, never a value.
#   coord-switch.sh stop <pid file> [timeout s]
#       SIGTERM, wait, SIGKILL after the timeout (default 30; the new build waits for runs in
#       progress, so give it 900). Refuses a pid file whose process is gone.
#   coord-switch.sh start <binary> <out dir> <pid file> <log> [--tokens F] [-- extra args…]
#       starts <binary> detached (setsid nohup, stdin /dev/null, appending to <log>) with exactly
#       the captured environment and argv (argv[0] replaced, --tokens replaced when given, extra
#       args appended), writes the pid file and waits for GET /v0/health = 200 on the --listen port.
set -uo pipefail
die() { echo "STOP: $*"; exit 3; }
case "${1:-}" in
capture)
  pf=$2 bin=$3 port=$4 out=$5
  pid=$(cat "$pf") || die "no pid file $pf"
  [ -d "/proc/$pid" ] || die "pid $pid is not running"
  exe=$(readlink "/proc/$pid/exe") || die "cannot read /proc/$pid/exe"
  [ "$exe" = "$(realpath "$bin")" ] || die "pid $pid runs $exe, not $bin"
  tr '\0' '\n' < "/proc/$pid/cmdline" | grep -qx -- "127.0.0.1:$port" || die "pid $pid does not listen on 127.0.0.1:$port"
  ppid=$(awk '{print $4}' "/proc/$pid/stat")
  echo "pid $pid: $exe; parent $ppid ($(cat "/proc/$ppid/comm" 2>/dev/null || echo ?))"
  echo "established clients on :$port: $(ss -Htn state established "( sport = :$port )" | wc -l)"
  mkdir -p "$out"; chmod 700 "$out"
  ( umask 077; tr '\0' '\n' < "/proc/$pid/cmdline" > "$out/argv"; tr '\0' '\n' < "/proc/$pid/environ" > "$out/env" )
  grep -q '^IEO_DB_URL=' "$out/env" || die "the process has no IEO_DB_URL"
  echo "captured $(wc -l < "$out/argv") argv entries and $(wc -l < "$out/env") environment variables (IEO_DB_URL present; values not shown)";;
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
  bin=$2 out=$3 pf=$4 log=$5; shift 5
  tok=""; extra=()
  if [ "${1:-}" = --tokens ]; then tok=$2; shift 2; fi
  [ "${1:-}" = -- ] && { shift; extra=("$@"); }
  mapfile -t argv < "$out/argv"; mapfile -t envs < "$out/env"
  args=("${argv[@]:1}")
  if [ -n "$tok" ]; then
    for i in "${!args[@]}"; do [ "${args[$i]}" = --tokens ] && args[$((i + 1))]=$tok; done
  fi
  args+=("${extra[@]}")
  port=""; for i in "${!args[@]}"; do [ "${args[$i]}" = --listen ] && port=${args[$((i + 1))]##*:}; done
  [ -n "$port" ] || die "no --listen in the captured argv"
  ss -Htln "( sport = :$port )" | grep -q . && die "port $port is still in use"
  env -i "${envs[@]}" setsid nohup "$bin" "${args[@]}" >> "$log" 2>&1 < /dev/null &
  pid=$!; echo "$pid" > "$pf"
  for _ in $(seq 1 150); do
    [ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/v0/health")" = 200 ] && { echo "pid $pid: $bin healthy on 127.0.0.1:$port (log $log)"; exit 0; }
    [ -d "/proc/$pid" ] || die "$bin exited at start (log $log)"
    sleep 0.2
  done
  die "$bin not healthy after 30 s (log $log)";;
*) echo "usage: coord-switch.sh capture|stop|start …"; exit 2;;
esac
