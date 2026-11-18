# Shared by run-step8.sh (the real run) and dry-run-step8.sh: requests with header files (no token
# in any argv, never printed), bounded resends on no answer with the SAME key and body. Set STEP8_SINGLE_POST=1 to
# send each POST once. Waits for the role and the ledger. Sourced, not run.
# Needs: COORD (base URL), TOKDIR (<principal>.header files, 0600), OUT (answers), LEDGER (the 02-05
# ledger dir), ADAPTER_HEADER (the role services' adapter header file).
stop() { echo "STOP: $*"; exit 3; }
# req <tag> <principal> <method> <path> [body file] [idempotency key] → HTTP status; body in $OUT/<tag>.json
req() { local tag=$1 who=$2 m=$3 p=$4 body=${5:-} key=${6:-} st n
  local args=(-s -o "$OUT/$tag.json" -w '%{http_code}' --max-time 150 -X "$m" -H @"$TOKDIR/$who.header")
  [ -n "$body" ] && args+=(-H 'Content-Type: application/json' --data @"$body")
  [ -n "$key" ] && args+=(-H "Idempotency-Key: $key")
  for n in 1 2 3; do
    if ! st=$(curl "${args[@]}" "$COORD$p"); then
      # Even a printed HTTP 200 can have an incomplete response. Preserve the bytes for inspection.
      if [ -e "$OUT/$tag.json" ]; then
        mv -f "$OUT/$tag.json" "$OUT/$tag.incomplete.json" || { echo 000; return; }
      fi
      if [ "$st" != 000 ]; then echo 000; return; fi
    fi
    [ "$m" = POST ] && [ "${STEP8_SINGLE_POST:-0}" = 1 ] && break
    # no answer at all (000): the same request again; with a key it reads the stored answer
    [ "$st" != 000 ] && break
    [ -z "$key" ] && [ "$m" != GET ] && break
    echo "# $tag: no answer, sending the same request again ($n)" >&2; sleep 2
  done
  echo "$st"; }
# j <tag> <python expression over d> → value
j() { python3 -I -c "import json,sys; d=json.load(open(sys.argv[1])); print(eval(sys.argv[2]))" "$OUT/$1.json" "$2"; }
role_status() { curl -fsS --max-time 5 -H @"$ADAPTER_HEADER" "http://127.0.0.1:$1/v0/role/status" > "$OUT/$2.json"; }
role_wait() { local port=$1 a=$2; for _ in $(seq 1 3600); do
    role_status "$port" "status-$a" &&
    j "status-$a" "d['running'] is None and (d.get('last') or {}).get('attempt_id') == '$a'" | grep -q True && return 0; sleep 0.5; done; return 1; }
settled() { for _ in $(seq 1 1800); do [ -f "$LEDGER/records.jsonl" ] && grep -q "\"attempt_id\":\"$1\"" "$LEDGER/records.jsonl" && [ ! -e "$LEDGER/inflight.json" ] && return 0; sleep 0.5; done; return 1; }
