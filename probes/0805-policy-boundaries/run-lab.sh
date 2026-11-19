#!/usr/bin/env bash
set -euo pipefail

probe=$(cd -- "$(dirname -- "$0")" && pwd)
baseline="$probe/../0802-cluster-baseline/run-lab.sh"
# Source the accepted runner in a dedicated shell. Its EXIT trap covers the
# baseline and all subsequent P11 checks, and $0 resolves its own resources.
bash -c 'source "$0" "$1"; python3 -B "$2/verify.py" "$1/kubeconfig" "$1/evidence/p11"' \
    "$baseline" "${1:?provide a fresh absolute run directory on the disposable VM}" "$probe"
