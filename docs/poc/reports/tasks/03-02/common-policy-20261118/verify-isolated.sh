#!/usr/bin/env bash
# Run from the repository root. Requires a cached CI PostgreSQL image.
set -euo pipefail
export CI=1

collectors=(
  launch_failure_still_writes_record
  timeout_sends_term_and_is_failure
  exit_zero_without_required_evidence_is_incomplete
  normal_success_records_everything
  nonzero_exit_is_failure_with_stderr_kept
)
coordinators=(
  authentication_and_scope_refusals_change_nothing
  verdict_uses_github_and_run_evidence_for_the_recorded_sha_only
  review_inputs_are_current_independent_and_for_the_head
  github_reviews_count_only_at_the_head_and_comments_do_not_clear_changes_requested
)
cargo test --offline --locked -j 2 -p ieo-evidence --test collector -- --exact "${collectors[@]}"
cargo test --offline --locked -j 2 -p ieo-supervisor --test supervisor -- \
  --exact role_arguments_use_edit_paths_and_session_id_only

scratch=$(mktemp -d)
container=
cleanup() {
  if [[ -n "$container" ]]; then podman stop "$container" >/dev/null; fi
  rm -f "$scratch/unset.log"
  rmdir "$scratch"
}
trap cleanup EXIT

# Prove that the harness refuses to count a skipped test as a pass.
if env -u IEO_TEST_DB_URL cargo test --offline --locked -j 2 -p ieo-coord --test coordinator -- \
  --exact "${coordinators[0]}" >"$scratch/unset.log" 2>&1; then
  printf '%s\n' 'FAIL DB absence was not rejected' >&2
  exit 1
fi
grep -F 'IEO_TEST_DB_URL must be set in CI' "$scratch/unset.log"
printf '%s\n' 'PASS unset DB fails the selected test with CI=1'

image=docker.io/library/postgres:17-bookworm@sha256:665dc7b37ac52a17e298511414c21d23ffa4e6a6ef6225b3e5d59c7afb35e994
container=$(podman run -d --rm --pull=never --tmpfs /var/lib/postgresql/data:rw,size=256m \
  -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=ieo -e POSTGRES_DB=ieo \
  -p 127.0.0.1::5432 "$image" postgres -c log_statement=ddl)
printf 'PostgreSQL image: %s\n' "$image"
for _ in $(seq 1 30); do
  if podman exec "$container" pg_isready -h 127.0.0.1 -U ieo -d ieo >/dev/null; then break; fi
  sleep 1
done
podman exec "$container" pg_isready -h 127.0.0.1 -U ieo -d ieo
binding=$(podman port "$container" 5432/tcp)
[[ "$binding" =~ ^127\.0\.0\.1:[1-9][0-9]*$ ]]
export IEO_TEST_DB_URL="host=127.0.0.1 port=${binding##*:} user=ieo dbname=ieo"
printf 'CI=%s | IEO_TEST_DB_URL=%s\n' "$CI" "$IEO_TEST_DB_URL"
cargo test --offline --locked -j 2 -p ieo-coord --test coordinator -- \
  --test-threads=1 --exact --nocapture "${coordinators[@]}"

observed=$(podman logs "$container" 2>&1 | awk '
  /statement: CREATE DATABASE ieo_t_[0-9_]+$/ {print $(NF-2), $(NF-1), $NF}
  /statement: DROP DATABASE (IF EXISTS )?ieo_t_[0-9_]+/ {
    match($0, /ieo_t_[0-9_]+/)
    print "DROP DATABASE", substr($0, RSTART, RLENGTH)
  }
')
printf '%s\n' "$observed"
created=$(printf '%s\n' "$observed" | awk '$1=="CREATE" {print $3}' | sort -u)
dropped=$(printf '%s\n' "$observed" | awk '$1=="DROP" {print $3}' | sort -u)
[[ $(printf '%s\n' "$created" | wc -l) -eq ${#coordinators[@]} ]]
[[ "$created" == "$dropped" ]]
remaining=$(podman exec "$container" psql -X -U ieo -d ieo -tAc   "SELECT count(*) FROM pg_database WHERE left(datname, 6) = 'ieo_t_'")
[[ "$remaining" == 0 ]]
printf "Remaining isolated test databases: %s\n" "$remaining"
printf '%s\n' 'PASS server observed four distinct test databases created and dropped'
podman stop "$container" >/dev/null
container=
printf '%s\n' 'PASS disposable PostgreSQL stopped'
