#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 3 || ! $3 =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
    echo "Usage: build.sh GO_TOOLCHAIN_DIR RUST_TOOLCHAIN_DIR RUN_ID" >&2
    exit 2
fi
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
# shellcheck source=inputs.env
source "$here/inputs.env"
go_root=$(realpath -- "$1")
rust_root=$(realpath -- "$2")
tag="localhost/ieo-policy-0305-$3"
workspace=$(mktemp -d "${TMPDIR:-/tmp}/ieo-policy-build.XXXXXXXX")
clear_tree() {
    # Module-cache toolchain directories can have mode 0555.
    find "$1" -type d -exec chmod u+w {} +
    rm -rf -- "$1"
}
trap 'clear_tree "$workspace"' EXIT

[[ $(podman image inspect "$BASE_NAME@$BASE_DIGEST" --format '{{.Id}} {{.Digest}} {{.Os}}/{{.Architecture}}') == "$BASE_ID $BASE_DIGEST linux/amd64" ]]
git -C "$repo" merge-base --is-ancestor "$P25_APPROVED_HEAD" "$POLICY_SOURCE"
[[ $(GOTOOLCHAIN=local GOENV=off "$go_root/bin/go" version) == "go version $GO_VERSION linux/amd64" ]]
[[ $("$rust_root/bin/rustc" -V) == "rustc $RUST_VERSION "* ]]
[[ $("$rust_root/bin/cargo" -V) == "cargo $RUST_VERSION "* ]]
[[ $("$rust_root/bin/rustfmt" -V) == "rustfmt $RUSTFMT_VERSION "* ]]
[[ $("$rust_root/bin/clippy-driver" -V) == "clippy $CLIPPY_VERSION "* ]]
for variant in go rust combined; do
    if podman image exists "$tag:$variant"; then
        echo "candidate tag is already in use: $tag:$variant" >&2
        exit 2
    fi
done

stage_toolchain() {
    local input=$1 output=$2 sha=$3 actual
    local archive="$workspace/toolchain.tar"
    mkdir -- "$output"
    tar --sort=name --mtime=@0 --numeric-owner --owner=0 --group=0 \
        --format=gnu -C "$input" -cf "$archive" .
    actual=$(sha256sum "$archive")
    [[ ${actual%% *} == "$sha" ]] || { echo "toolchain digest mismatch: $input" >&2; return 2; }
    tar --no-same-owner -xf "$archive" -C "$output"
    rm -- "$archive"
}

p25=docs/poc/reports/tasks/03-04/p25-20261119
p1=docs/poc/reports/tasks/03-01/catalog-contract-20261118
p2=docs/poc/reports/tasks/03-02/common-policy-20261118
p3=docs/poc/reports/tasks/03-03/go-rust-20261118
runtime=docs/poc/reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md
evidence=docs/poc/reports/tasks/02-01/collector-20261112/REPORT.md
mapfile -t approved_inputs < <(git -C "$repo" show "$POLICY_SOURCE:$p25/inputs.sha256" | cut -c 67-)
mkdir "$workspace/bundle"
git -C "$repo" archive "$POLICY_SOURCE" -- "${approved_inputs[@]}" \
    "$p25"/{INDEX.md,catalog-use.json,GO_POLICY.md,RUST_POLICY.md,EXPECTATIONS.md,inputs.sha256} \
    "$p1"/ACCEPTANCE.md "$p2"/ACCEPTANCE.md "$p3"/{ACCEPTANCE.md,check-go-format.sh,evidence/isolation.txt} \
    "$runtime" "$evidence" | tar -xf - -C "$workspace/bundle"
(
    cd -- "$workspace/bundle"
    sha256sum --quiet -c "$p25/inputs.sha256"
    find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ../bundle.sha256
)
cp -- "$here"/{Containerfile,Containerfile.combined,verify.py,profiles.json,inputs.env} "$workspace/"
printf 'policy_source=%s approved_head=%s base=%s\n' "$POLICY_SOURCE" "$P25_APPROVED_HEAD" "$BASE_DIGEST"
(
    cd -- "$here"
    sha256sum inputs.env Containerfile Containerfile.combined build.sh verify.py profiles.json workflow-candidate.yml
)
printf 'bundle manifest '
sha256sum "$workspace/bundle.sha256" | cut -d ' ' -f 1
build_args=("--pull=never" "--network=none" --layers --timestamp 0
    --label "org.opencontainers.image.revision=$POLICY_SOURCE"
    --label 'org.opencontainers.image.title=ieo_poc T04 policy image candidate')
inspect() {
    podman image inspect "$tag:$1" --format '{{index .RepoTags 0}} {{.Id}} {{.Digest}} {{.Size}}'
}

# Stage one toolchain at a time. The combined build reuses immutable local images.
for variant in go rust; do
    if [[ $variant == go ]]; then
        stage_toolchain "$go_root" "$workspace/go" "$GO_ARCHIVE_SHA256"
    else
        stage_toolchain "$rust_root" "$workspace/rust" "$RUST_ARCHIVE_SHA256"
    fi
    podman build "${build_args[@]}" --build-arg "BASE_IMAGE=$BASE_NAME@$BASE_DIGEST" \
        --target "$variant" --tag "$tag:$variant" "$workspace"
    clear_tree "$workspace/$variant"
    inspect "$variant"
done

go_image=$(podman image inspect "$tag:go" --format '{{.Id}}')
rust_image=$(podman image inspect "$tag:rust" --format '{{.Id}}')
podman build "${build_args[@]}" --file "$workspace/Containerfile.combined" \
    --build-arg "GO_IMAGE=sha256:$go_image" --build-arg "RUST_IMAGE=sha256:$rust_image" \
    --tag "$tag:combined" "$workspace"
inspect combined
