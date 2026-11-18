#!/usr/bin/env bash
# Use a private, empty scratch directory and cached, pinned toolchain roots.
set -euo pipefail
[[ $# -eq 3 ]] || { echo 'Usage: bash run-isolated.sh GO_ROOT RUST_ROOT EMPTY_SCRATCH' >&2; exit 2; }
go_root=$(realpath "$1")
rust_root=$(realpath "$2")
scratch=$(realpath "$3")
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
[[ -x "$go_root/bin/go" && -x "$rust_root/bin/cargo" ]]
[[ -d "$scratch" && -z "$(find "$scratch" -mindepth 1 -maxdepth 1 -print -quit)" ]]
mkdir -p "$scratch"/{home,cargo-home,go-cache,go-mod-cache,results}
exec env -i PATH=/usr/bin bwrap --unshare-net --unshare-pid --die-with-parent \
  --ro-bind /usr /usr --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --ro-bind "$(realpath /usr/bin/ld)" /etc/alternatives/ld \
  --proc /proc --dev /dev --tmpfs /tmp \
  --ro-bind "$here" /policy --ro-bind "$go_root" /go --ro-bind "$rust_root" /rust \
  --bind "$scratch" /scratch --chdir /scratch \
  --setenv HOME /scratch/home --setenv PATH /rust/bin:/go/bin:/usr/bin \
  --setenv GOTOOLCHAIN local --setenv GOENV off --setenv GOWORK off --setenv GOPROXY off \
  --setenv GOCACHE /scratch/go-cache --setenv GOMODCACHE /scratch/go-mod-cache \
  --setenv GOMAXPROCS 2 --setenv CGO_ENABLED 1 --setenv CARGO_HOME /scratch/cargo-home \
  --setenv CARGO_TARGET_DIR /scratch/cargo-target --setenv LC_ALL C \
  -- /bin/bash /policy/verify-cases.sh
