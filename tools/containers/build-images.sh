#!/bin/bash
# Build the 02-04 images from one approved commit: coordinator, role-development,
# role-guardrail. Usage: build-images.sh <commit> <target-dir> [debug|release]
# The commit must be checked out cleanly in this worktree (no local changes). Prints the image
# ids and digests; labels carry the revision and binary SHA-256.
set -euo pipefail
COMMIT=$1; TARGET=$2; PROFILE=${3:-release}
CLAUDE=${CLAUDE_BIN:-/opt/rust/ieo_poc_probe/bin/claude-2.1.322}
# the Claude Code CLI the runtime contract and 01-04 used (2.1.322)
CLAUDE_SHA=${CLAUDE_SHA256:-bfb3703ad5380242328459bc62ada4bde8ae7ba57c6e1eeadb9c00e08874c478}
[ "$(sha256sum "$CLAUDE" | cut -c1-64)" = "$CLAUDE_SHA" ] || { echo "the claude CLI is not the expected file ($CLAUDE_SHA)"; exit 1; }
cd "$(git rev-parse --show-toplevel)"
[ "$(git rev-parse HEAD)" = "$(git rev-parse "$COMMIT")" ] || { echo "HEAD is not $COMMIT"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "the worktree has local changes"; exit 1; }
REV=$(git rev-parse HEAD)
flag=""; [ "$PROFILE" = release ] && flag="--release"
CARGO_TARGET_DIR=$TARGET cargo build --locked $flag -q -p ieo-coord -p ieo-supervisor --bins
OUT=$TARGET/$PROFILE
CTX=$(mktemp -d)
trap 'rm -rf "$CTX"' EXIT
cp "$OUT/ieo-coord" "$OUT/ieo-supervisor" "$CTX/"
cp "$CLAUDE" "$CTX/claude"
cp tools/containers/Containerfile.* "$CTX/"
sha() { sha256sum "$1" | cut -c1-64; }
C=$(sha "$CTX/ieo-coord"); S=$(sha "$CTX/ieo-supervisor"); L=$(sha "$CTX/claude")
tag=${REV:0:7}
podman build -q -f "$CTX/Containerfile.coordinator" --build-arg REVISION="$REV" --build-arg COORD_SHA256="$C" \
  -t "localhost/ieo-poc/coordinator:$tag" "$CTX" >/dev/null
for role in development guardrail; do
  podman build -q -f "$CTX/Containerfile.role" --target "$role" --build-arg REVISION="$REV" \
    --build-arg SUPERVISOR_SHA256="$S" --build-arg CLAUDE_SHA256="$L" \
    -t "localhost/ieo-poc/role-$role:$tag" "$CTX" >/dev/null
done
echo "revision $REV profile $PROFILE"
echo "ieo-coord $C"; echo "ieo-supervisor $S"; echo "claude $L"
for i in coordinator role-development role-guardrail; do
  echo "localhost/ieo-poc/$i:$tag $(podman image inspect --format '{{.Id}} {{.Digest}} user={{.Config.User}}' "localhost/ieo-poc/$i:$tag")"
done
