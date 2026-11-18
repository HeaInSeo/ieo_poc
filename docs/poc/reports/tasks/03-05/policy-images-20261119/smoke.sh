#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 2 || ! $1 =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
    echo 'Usage: smoke.sh BUILD_RUN_ID NEW_LOG_DIRECTORY' >&2
    exit 2
fi
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
# shellcheck source=inputs.env
source "$here/inputs.env"
tag="localhost/ieo-policy-0305-$1"
mkdir -- "$2"
logs=$(realpath -- "$2")
scratch=$(mktemp -d "${TMPDIR:-/tmp}/ieo-policy-smoke.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
fixtures=docs/poc/reports/tasks/03-03/go-rust-20261118/fixtures
git -C "$repo" archive "$POLICY_SOURCE" "$fixtures/go" "$fixtures/rust" | tar -xf - -C "$scratch"
cp -a "$scratch/$fixtures/go" "$scratch/go-skip"
cp -a "$scratch/$fixtures/rust" "$scratch/rust-ignore"
sed -i '/func TestDouble(t \*testing.T) {/a\	t.Skip("T04 required test skip control")' "$scratch/go-skip/double_test.go"
sed -i '/fn doubles_three()/i\    #[ignore]' "$scratch/rust-ignore/src/lib.rs"
cp -a "$scratch/$fixtures/go" "$scratch/go-missing"
cp -a "$scratch/$fixtures/rust" "$scratch/rust-missing"
sed -i 's/TestDouble(/TestOther(/' "$scratch/go-missing/double_test.go"
sed -i 's/fn doubles_three(/fn another_test(/' "$scratch/rust-missing/src/lib.rs"
args=(--rm "--pull=never" "--network=none" --read-only "--cap-drop=all"
      "--security-opt=no-new-privileges" --user 65532:65532
      --tmpfs "/tmp:rw,exec,nosuid,nodev,mode=1777")

observe() {
    local label=$1 variant=$2 profile=$3 expected=$4
    local fixture=${5:-} match=${6:-} rc=0 image
    local -a mount=()
    image=$(podman image inspect "$tag:$variant" --format '{{.Id}}')
    if [[ -n $fixture ]]; then
        mount=(-v "$fixture:/work:ro,Z")
    fi
    podman run "${args[@]}" "${mount[@]}" "$image" "$profile" >"$logs/$label.log" 2>&1 || rc=$?
    printf '%s rc=%s expected=%s image=%s profile=%s\n' "$label" "$rc" "$expected" "$image" "$profile" | tee -a "$logs/results.txt"
    [[ $rc == "$expected" ]] || { cat "$logs/$label.log" >&2; return 1; }
    if [[ -n $match ]]; then
        grep -Fq -- "$match" "$logs/$label.log" || { cat "$logs/$label.log" >&2; return 1; }
    fi
}

for variant in go rust combined; do
    observe "info-$variant" "$variant" info 0 "" '"catalog": "0.2.2"'
done
observe go-go go go 0 "$scratch/$fixtures/go" "PASS profile=go"
observe go-combined combined go 0 "$scratch/$fixtures/go" "PASS profile=go"
observe rust-rust rust rust 0 "$scratch/$fixtures/rust" "PASS profile=rust"
observe rust-combined combined rust 0 "$scratch/$fixtures/rust" "PASS profile=rust"
observe wrong-stack-go rust go 2 "" "Go is absent"
observe wrong-stack-rust go rust 2 "" "Rust is absent"
observe required-go-skip go go 1 "$scratch/go-skip" "'TestDouble': ['skip']"
observe required-rust-ignore rust rust 1 "$scratch/rust-ignore" "'tests::doubles_three': ['ignored']"
observe required-go-missing go go 2 "$scratch/go-missing" "INCONCLUSIVE required Go test results: {'TestDouble': []}"
observe required-rust-missing rust rust 2 "$scratch/rust-missing" "INCONCLUSIVE required Rust test results: {'tests::doubles_three': []}"
