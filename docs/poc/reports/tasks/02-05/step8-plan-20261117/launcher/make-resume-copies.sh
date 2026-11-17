#!/bin/bash
# 02-05 step 8 resume (RESUME_PLAN.md §9): the read-only copies the resume runs from, the new
# SHA256SUMS, and the exact permission-rule change (old lines out, new lines in).
#   make-resume-copies.sh <repo checkout at the merged SHA> <D> <bin dir of that SHA> <rules out file>
#   env OLD_SUPERVISOR: the ieo-supervisor the role containers run now (46799b0's); its SHA-256 is
#   pinned in the rollback rule (PR #91 review 4b427c5 F-10)
# <D> is /opt/rust/src/github.com/HeaInSeo/ieo_poc-data in the real run (a scratch directory in the
# dry run). Writes:
#   <D>/step8-resume/   launcher/ (all), sprint-POC-S8-01-r1.json, EXECUTION_PLAN.md, RESUME_PLAN.md,
#                       MANUAL_STEPS.md, evidence/r1-usage-verification.md — byte-identical to the checkout
#   <D>/step8-ops/      obs-check.sh and rate_limit_policy.py of the checkout (replacing the old ones),
#                       SHA256SUMS over every file of step8-resume and step8-ops (step8-46799b0 stays
#                       as evidence, outside the check)
#   both trees chmod -R a-w; <rules out file>: {"remove": [...], "add": [...]} (JSON)
# Refuses an existing step8-resume (a second run would hide what was checked). Prints hashes only.
set -euo pipefail
SRC=$(realpath "$1"); D=$(realpath "$2"); BIN=$(realpath "$3"); RULES=$(realpath -m "$4")
P="$SRC/docs/poc/reports/tasks/02-05/step8-plan-20261117"
[ -f "$P/launcher/run-step8-resume.sh" ] || { echo "STOP: $SRC has no resume launcher"; exit 3; }
for b in ieo-coord ieo-supervisor ieo-role-call; do [ -x "$BIN/$b" ] || { echo "STOP: $BIN/$b missing"; exit 3; }; done
[ -e "$D/step8-resume" ] && { echo "STOP: $D/step8-resume exists"; exit 3; }
# every byte from the commit (PR #91 review 4b427c5 F-5): a checkout with changes is refused
[ -z "$(git -C "$SRC" status --porcelain)" ] || { echo "STOP: $SRC has uncommitted changes"; exit 3; }
OLD_SUP_SHA=$(sha256sum < "${OLD_SUPERVISOR:?the old ieo-supervisor (46799b0)}" | cut -c1-64)
SHA=$(git -C "$SRC" rev-parse HEAD)
mkdir -p "$D/step8-resume/evidence"
# only what the commit holds (never an untracked file such as a __pycache__), from its blobs
R=docs/poc/reports/tasks/02-05/step8-plan-20261117
{ git -C "$SRC" ls-files -s -- "$R/launcher"
  git -C "$SRC" ls-files -s -- "$R/sprint-POC-S8-01-r1.json" "$R/EXECUTION_PLAN.md" "$R/RESUME_PLAN.md" \
    "$R/MANUAL_STEPS.md" "$R/evidence/r1-usage-verification.md"; } | while read -r mode _ _ path; do
  to="$D/step8-resume/${path#"$R"/}"; mkdir -p "$(dirname "$to")"
  git -C "$SRC" show "HEAD:$path" > "$to"
  if [ "$mode" = 100755 ]; then chmod 0755 "$to"; else chmod 0644 "$to"; fi
done
# every copied file equals the commit's blob
( cd "$D/step8-resume" && find . -type f | sort ) | while read -r f; do
  rel=${f#./}; case "$rel" in evidence/*) src="evidence/${rel#evidence/}";; *) src="$rel";; esac
  [ "$(git -C "$SRC" show "HEAD:docs/poc/reports/tasks/02-05/step8-plan-20261117/$src" | sha256sum | cut -c1-64)" = \
    "$(sha256sum < "$D/step8-resume/$rel" | cut -c1-64)" ] || { echo "STOP: $rel differs from $SHA"; exit 3; }
done
mkdir -p "$D/step8-ops"
# the step-8 files it replaces are kept as they were (review 4b427c5 F-11)
if [ -e "$D/step8-ops/SHA256SUMS" ] && [ ! -e "$D/step8-ops-46799b0" ]; then
  cp -a "$D/step8-ops" "$D/step8-ops-46799b0"; chmod -R a-w "$D/step8-ops-46799b0"
fi
chmod -R u+w "$D/step8-ops"
# from the blob-verified copy, not the working tree (review 4b427c5 F-5)
cp "$D/step8-resume/launcher/obs-check.sh" "$D/step8-ops/obs-check.sh"
cp "$D/step8-resume/launcher/rate_limit_policy.py" "$D/step8-ops/rate_limit_policy.py"
chmod 0555 "$D/step8-ops/obs-check.sh"; chmod 0444 "$D/step8-ops/rate_limit_policy.py"
( cd "$D" && { find step8-resume -type f | sort; echo step8-ops/obs-check.sh; echo step8-ops/rate_limit_policy.py; } | xargs sha256sum > step8-ops/SHA256SUMS )
chmod -R a-w "$D/step8-resume" "$D/step8-ops"
SUMS=$(sha256sum < "$D/step8-ops/SHA256SUMS" | cut -c1-64)
SUP=$(sha256sum < "$BIN/ieo-supervisor" | cut -c1-64)
echo "# commit $SHA · step8-resume $(find "$D/step8-resume" -type f | wc -l) files · SHA256SUMS $SUMS ($(wc -l < "$D/step8-ops/SHA256SUMS") files) · ieo-supervisor $SUP · ieo-coord $(sha256sum < "$BIN/ieo-coord" | cut -c1-64) · ieo-role-call $(sha256sum < "$BIN/ieo-role-call" | cut -c1-64)"
python3 -I - "$D/step8-resume/MANUAL_STEPS.md" "$D" "$BIN" "$SUMS" "$SUP" "$SHA" "$RULES" "$OLD_SUP_SHA" <<'PY'
import json, re, sys
manual, D, BIN, SUMS, SUP, SHA, out, OLD_SUP = sys.argv[1:9]
old = json.loads(re.search(r"```json\n(.*?)```", open(manual).read(), re.S).group(1))["permissions"]["allow"]
W = "/opt/rust/ieo_poc_runs/02-05-s8-20261118"
L = f"bash {D}/step8-resume/launcher"
new = [
    f"Bash(cd {D})",
    f"Bash(echo '{SUMS}  step8-ops/SHA256SUMS')",
    "Bash(sha256sum --quiet --strict -c -)",
    "Bash(sha256sum --quiet --strict -c step8-ops/SHA256SUMS)",
    "Bash(find step8-resume step8-ops -perm /222)",
    "Bash(wc -l)",
    "Bash(grep -qx 0)",
    "Bash(echo \"copies ok\")",
    # the coordinator: stop (it waits for runs in progress), the new build, the rollback to M
    f"Bash({L}/coord-switch.sh stop {D}/coord.pid 900)",
    f"Bash({L}/coord-switch.sh start {BIN}/ieo-coord {D}/private/s8/coord-778ee47 {D}/coord.pid {D}/logs/coord-{SHA[:12]}.log --tokens {D}/private/s8/tokens-s8.json -- --roles {D}/private/s8/roles.json)",
    f"Bash({L}/coord-switch.sh start {D}/bin/ieo-coord-M {D}/private/s8/coord-778ee47 {D}/coord.pid {D}/logs/coord-M-rollback.log --tokens {D}/private/s8/tokens-s8.json -- --roles {D}/private/s8/roles.json)",
    # the role containers: the new supervisor in place, and back
    f"Bash({L}/restart-roles.sh swap {W} {BIN}/ieo-supervisor {SUP})",
    f"Bash({L}/restart-roles.sh rollback {W} {OLD_SUP})",
    # the resume driver and its environment
    f"Bash(export COORD=http://127.0.0.1:58090)",
    f"Bash(export TOKDIR={D}/private/s8)",
    f"Bash(export OUT={D}/s8-out)",
    f"Bash(export LEDGER={D}/ledger-0205)",
    f"Bash(export ADAPTER_HEADER={D}/private/s8/adapter.header)",
    f"Bash(export PLAN={D}/private/s8/plan-0205.json)",
    f"Bash(export ROLE_CALL={BIN}/ieo-role-call)",
    f"Bash(export TRANSCRIPT={D}/s8-evidence/s8-r1-transcript.jsonl)",
    "Bash(export ORIGINAL_PATH=/opt/rust/ieo_poc_runs/02-05-s8-20261118/development/state/claude/projects/-state-work-ieo-poc-test/5c991c71-b6e2-4cfb-bd76-6bb18a36e090.jsonl)",
    "Bash(export ORIGINAL_SHA256=806746fd5b120020796a41527c7f01322a7f2de2c40d591731b060244824dc68)",
    "Bash(export DECISION=docs/poc/User_Decisions_2026-11-17_r1-settlement.md)",
    f"Bash(export CHECK_REPORT={D}/step8-resume/evidence/r1-usage-verification.md)",
    f"Bash(export COORD_ROLES={D}/private/s8/roles.json)",
    "Bash(export CHECK_VERDICT=\"CONFIRMED WITH CAVEAT\")",
    "Bash(export DRY=1)",
] + [f"Bash({L}/run-step8-resume.sh {p})" for p in ("settle", "r1b-body", "r1b-check", "r2g1")]
json.dump({"commit": SHA, "sums": SUMS, "remove": old, "add": new}, open(out, "w"), indent=1)
print(f"# rules: remove {len(old)} old allow lines, add {len(new)} new ones -> {out}")
PY
