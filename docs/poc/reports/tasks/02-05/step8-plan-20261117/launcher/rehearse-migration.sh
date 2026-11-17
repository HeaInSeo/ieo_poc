#!/bin/bash
# 02-05 step 8: rehearse the backup and the new build's migration on the TEST PostgreSQL before
# anything live changes (EXECUTION_PLAN.md §5.2 step 1b, PR #86 review 3e4e3d6 M-2).
# Usage: rehearse-migration.sh <pg_dump -Fc file> <new ieo-coord binary> [scratch port] [live-counts.txt]
# With live-counts.txt (`table=count` per line, from the live database at backup time, §3) the
# restored counts must equal it. TOKENS_FILE and BINDINGS_FILE for the serve check: the new
# tokens.json and the captured --bindings file (EXECUTION_PLAN.md §5.2 step 4), so the merged
# principals and check_bindings are exercised before the live start.
# Restores the dump into a scratch database of the test container (never 55433), counts the rows
# of every table, runs `ieo-coord migrate`, counts again (existing tables unchanged, the 11 new
# tables present), starts `ieo-coord serve` on the scratch port against it (health 200, SIGTERM →
# exit 0) and drops the scratch database. Env: TEST_DB_PASSWORD_FILE (never printed), optional
# TEST_PG_CONTAINER (ieo-poc-0203-pg), TEST_PG_PORT (55432), MANIFEST, POLICY, TOKENS_FILE, BINDINGS_FILE.
set -uo pipefail
DUMP=$1 COORD=$2 SPORT=${3:-58291} LIVE=${4:-}
PGC=${TEST_PG_CONTAINER:-ieo-poc-0203-pg}; PGPORT=${TEST_PG_PORT:-55432}
: "${TEST_DB_PASSWORD_FILE:?}"
[ "$PGPORT" = 55433 ] && { echo "refusing the persistent database port"; exit 1; }
[ "$SPORT" = 58090 ] && { echo "refusing the live coordinator port"; exit 1; }
ROOT=$(cd "$(dirname "$0")/../../../../../../.." && pwd)
MANIFEST=${MANIFEST:-$ROOT/docs/poc/Console_Coordinator_Sprints_v0.json}; POLICY=${POLICY:-$ROOT/contracts/judgment-policy-v0.json}
DB=ieo_t_s8rehearse_$$; T=$(mktemp -d); CPID=
trap '[ -n "$CPID" ] && kill -KILL "$CPID" 2>/dev/null; podman exec "$PGC" dropdb -U ieo --if-exists --force "$DB" >/dev/null 2>&1; rm -rf "$T"' EXIT
fail=0; ok() { if [ "$1" = 0 ]; then echo "PASS $2"; else echo "FAIL $2"; fail=1; fi; }
exact() { podman exec "$PGC" psql -U ieo -d "$DB" -At -c "SELECT c.relname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind = 'r' AND n.nspname = 'public' ORDER BY 1" |
  while read -r t; do echo "$t=$(podman exec "$PGC" psql -U ieo -d "$DB" -Atc "SELECT count(*) FROM \"$t\"")"; done; }
echo "# migration rehearsal · dump $(basename "$DUMP") ($(stat -c %s "$DUMP") bytes, sha256 $(sha256sum < "$DUMP" | cut -c1-12)) · new build $(sha256sum < "$COORD" | cut -c1-12) · $PGPORT/$DB"
podman exec "$PGC" createdb -U ieo "$DB"; ok $? "scratch database created"
podman exec -i "$PGC" pg_restore -U ieo -d "$DB" --no-owner --no-privileges --exit-on-error < "$DUMP"; ok $? "the dump restores (pg_restore --exit-on-error)"
exact > "$T/before"; echo "tables before: $(wc -l < "$T/before")"
if [ -n "$LIVE" ]; then diff -q <(sort "$LIVE") <(sort "$T/before") >/dev/null; ok $? "the restored row counts equal the live counts at backup ($(wc -l < "$LIVE") tables)"; fi
IEO_DB_URL="host=127.0.0.1 port=$PGPORT user=ieo dbname=$DB password=$(cat "$TEST_DB_PASSWORD_FILE")" "$COORD" migrate; ok $? "ieo-coord migrate of the new build"
exact > "$T/after"
grep -Fxvf "$T/after" "$T/before" > "$T/changed"; [ ! -s "$T/changed" ]; ok $? "every table that existed keeps its row count ($(wc -l < "$T/before") tables)"
missing=0; for t in role_checkpoints role_tamper_events role_first_actions role_session_status role_deliveries role_delivery_results role_takeovers assignment_run_usage assignment_limit_events role_recoveries role_recovery_uses; do
  grep -q "^$t=" "$T/after" || { echo "missing $t"; missing=1; }; done
[ "$missing" = 0 ]; ok $? "the 11 new tables exist"
podman exec "$PGC" psql -U ieo -d "$DB" -Atc "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = 'evidence_kind_check'" | grep -q branch_head; ok $? "evidence.kind takes branch_head"
echo '{"principals": []}' > "$T/tokens.json"
sargs=(serve --listen "127.0.0.1:$SPORT" --tokens "${TOKENS_FILE:-$T/tokens.json}" --manifest "$MANIFEST" --policy "$POLICY")
[ -n "${BINDINGS_FILE:-}" ] && sargs+=(--bindings "$BINDINGS_FILE")
echo "serve check with tokens $( [ -n "${TOKENS_FILE:-}" ] && echo "$TOKENS_FILE" || echo '(none)' ), bindings ${BINDINGS_FILE:-(none)}"
IEO_DB_URL="host=127.0.0.1 port=$PGPORT user=ieo dbname=$DB password=$(cat "$TEST_DB_PASSWORD_FILE")" \
  "$COORD" "${sargs[@]}" 2> "$T/serve.log" &
CPID=$!
st=000; for _ in $(seq 1 100); do st=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$SPORT/v0/health"); [ "$st" = 200 ] && break; sleep 0.2; done
[ "$st" = 200 ]; ok $? "the new build serves on the migrated copy ($st)"
kill -TERM "$CPID"; wait "$CPID"; code=$?; CPID=; [ "$code" = 0 ]; ok $? "and stops on SIGTERM (exit $code)"
[ "$fail" = 0 ] && echo "## rehearsal: PASS" || { echo "## rehearsal: FAIL"; tail -5 "$T/serve.log"; }
exit "$fail"
