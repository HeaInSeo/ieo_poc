#!/usr/bin/env bash
set -euo pipefail

probe=$(cd -- "$(dirname -- "$0")" && pwd)
run_dir=${1:?provide a fresh absolute run directory on the disposable VM}
[[ "$run_dir" = /* ]]
mkdir "$run_dir"
evidence="$run_dir/evidence"
mkdir "$evidence"
cfg="$run_dir/kubeconfig"
cluster=ieo-0802
ctx=kind-ieo-0802
export KIND_EXPERIMENTAL_PROVIDER=docker
nodes=(ieo-0802-control-plane ieo-0802-worker ieo-0802-worker2 ieo-0802-worker3)
k=(kubectl --kubeconfig "$cfg" --context "$ctx" --request-timeout=25s)

{
    python3 -c 'import sys; assert sys.version_info >= (3, 11); print(sys.version)'
    df -h / "$run_dir"
    test -r /sys/kernel/btf/vmlinux
    test "$(uname -m)" = x86_64
    kernel=$(uname -r)
    test "$(printf '%s\n' 5.14 "${kernel%%-*}" | sort -V | head -1)" = 5.14
    test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs
    test "$(docker info --format '{{.CgroupVersion}}')" = 2
    test "$(docker info --format '{{json .SecurityOptions}}' | jq '[.[] | contains("rootless")] | any')" = false
    uname -r
    stat -c 'BTF bytes=%s' /sys/kernel/btf/vmlinux
    docker info --format 'cgroup={{.CgroupVersion}} driver={{.CgroupDriver}} security={{json .SecurityOptions}}'
    kind version | tee "$run_dir/kind-version" | grep -E '^kind v0\.31\.0 '
    kubectl version --client -o json | jq -e '.clientVersion.gitVersion == "v1.35.0"'
    helm version --short
} | tee "$evidence/vm.txt"

clusters=$(kind get clusters)
if printf '%s\n' "$clusters" | grep -Fxq "$cluster"; then
    echo "The disposable VM already contains $cluster" >&2
    exit 1
fi

cleanup() {
    local status=$?
    trap - EXIT
    set +e
    if [[ -s "$cfg" ]]; then
        "${k[@]}" get pods,daemonsets -A -o wide > "$evidence/pods-final.txt" 2>&1
        "${k[@]}" get events -A --sort-by=.lastTimestamp > "$evidence/events-final.txt" 2>&1
        "${k[@]}" -n kube-system logs -l app.kubernetes.io/name=tetragon -c tetragon --tail=60 --prefix > "$evidence/tetragon-agent-final.txt" 2>&1
        "${k[@]}" -n "$cluster" logs -l fixture-kind=receiver --tail=30 --prefix > "$evidence/receivers-final.txt" 2>&1
    fi
    {
        kind delete cluster --name "$cluster" --kubeconfig "$cfg"
        local deleted=$?
        local remaining
        remaining=$(docker ps -a --filter "label=io.x-k8s.kind.cluster=$cluster" --format '{{.Names}}')
        local listed=$?
        printf 'nodes_remaining=%s\n' "$remaining"
        if (( deleted != 0 || listed != 0 )) || [[ -n "$remaining" ]]; then
            status=1
        fi
    } > "$evidence/cleanup.txt" 2>&1
    cat "$evidence/cleanup.txt"
    exit "$status"
}
trap cleanup EXIT

kind create cluster --config "$probe/kind.yaml" --kubeconfig "$cfg" \
    --image docker.io/kindest/node:v1.35.0@sha256:452d707d4862f52530247495d180205e029056831160e22870e37e3f6c1ac31f
. "$probe/lab-pins.env"
{
    for node in "${nodes[@]}"; do
        docker exec "$node" sh -ec 'printf "%s " "$(hostname)"; uname -r; test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs; stat -fc "cgroup=%T" /sys/fs/cgroup; test -r /sys/kernel/btf/vmlinux; stat -c "BTF bytes=%s" /sys/kernel/btf/vmlinux; readlink /proc/self/ns/cgroup'
    done
} | tee "$evidence/node-kernel.txt"
"${k[@]}" config view --minify -o json | jq -e '.clusters[0].cluster.server | startswith("https://127.0.0.1:")' >/dev/null

curl -fsSL --retry 3 "https://helm.cilium.io/cilium-$CILIUM_VERSION.tgz" -o "$run_dir/cilium.tgz"
curl -fsSL --retry 3 "https://helm.cilium.io/tetragon-$TETRAGON_VERSION.tgz" -o "$run_dir/tetragon.tgz"
{
    printf '%s  %s\n' "$CILIUM_CHART_SHA256" "$run_dir/cilium.tgz"
    printf '%s  %s\n' "$TETRAGON_CHART_SHA256" "$run_dir/tetragon.tgz"
} | tee "$evidence/charts.sha256" | sha256sum -c
api_ip=$(docker inspect -f '{{ (index .NetworkSettings.Networks "kind").IPAddress }}' ieo-0802-control-plane)
helm install cilium "$run_dir/cilium.tgz" --kubeconfig "$cfg" --kube-context "$ctx" -n kube-system \
    -f "$probe/cilium-values.yaml" --set-string "k8sServiceHost=$api_ip" --set k8sServicePort=6443
"${k[@]}" wait --request-timeout=0 --for=condition=Ready node --all --timeout=300s
"${k[@]}" -n kube-system wait --request-timeout=0 --for='jsonpath={.status.numberReady}=4' daemonset/cilium --timeout=300s
helm install tetragon "$run_dir/tetragon.tgz" --kubeconfig "$cfg" --kube-context "$ctx" -n kube-system \
    -f "$probe/tetragon-values.yaml"
"${k[@]}" -n kube-system wait --request-timeout=0 --for='jsonpath={.status.numberReady}=4' daemonset/tetragon --timeout=300s
"${k[@]}" -n kube-system rollout status --request-timeout=0 deployment/tetragon-operator --timeout=180s
"${k[@]}" wait --request-timeout=0 --for=condition=Established crd/tracingpoliciesnamespaced.cilium.io --timeout=120s

case "${2:-}" in
    --p09) python3 -B "$probe/../0803-observation/run.py" "$cfg" "$evidence" ;;
    "") python3 -B "$probe/verify-live.py" "$cfg" "$evidence" ;;
    *) echo "unknown fixture phase" >&2; exit 2 ;;
esac
for node in "${nodes[@]}"; do
    docker exec "$node" crictl images -o json > "$evidence/$node-images.json"
    jq -e --arg repo "$CANDIDATE_REPO" '[.images[] | (.repoTags[]?,.repoDigests[]?) | select(startswith($repo))] | length == 0' \
        "$evidence/$node-images.json" >/dev/null
done
echo '0802 runtime checks passed; teardown follows'
