#!/usr/bin/env bash
# Only the kind cluster with the isolated kubeconfig and expected named nodes.
set -euo pipefail
config=$1
fixture=$2
evidence=$3
probe_dir=$(dirname "$0")
k=(kubectl --kubeconfig "$config" --context kind-ieo-x06 --request-timeout=180s)
"${k[@]}" config view --minify -o json | jq -e '
  .clusters[0] | .name == "kind-ieo-x06" and (.cluster.server | startswith("https://127.0.0.1:"))
' >/dev/null
expected_nodes=$'ieo-x06-control-plane\nieo-x06-worker\nieo-x06-worker2'
test "$("${k[@]}" get nodes -o custom-columns=NAME:.metadata.name --no-headers | sort)" = "$expected_nodes"
"${k[@]}" get nodes -o json > "$evidence/nodes.json"
"${k[@]}" get daemonsets -A -o json > "$evidence/daemonsets.json"
test "$("${k[@]}" -n kube-system get daemonset kindnet -o 'jsonpath={.status.numberReady}')" = 3
if jq -e '.items | any(.metadata.name | contains("cilium"))' "$evidence/daemonsets.json" >/dev/null; then
  exit 1
fi
ns=("${k[@]}" -n x06)
collect() {
  "${ns[@]}" get pods,services,endpointslices -o json > "$evidence/objects.json" || true
  for pod in driver grpc-coord http-coord pg; do
    "${ns[@]}" logs "$pod" --timestamps > "$evidence/$pod.log" 2>&1 || true
  done
}
trap collect EXIT
prepare_at=$SECONDS
"${k[@]}" create namespace x06
"${ns[@]}" create configmap coord-config --from-file="$fixture"
"${ns[@]}" create -f "$probe_dir/postgres.yaml"
"${ns[@]}" wait --for=condition=Ready pod/pg --timeout=120s
for mode in http grpc; do
  sed "s/__MODE__/$mode/g" "$probe_dir/coordinator.yaml" | "${ns[@]}" create -f -
done
"${ns[@]}" wait --for=condition=Ready pod/grpc-coord pod/http-coord --timeout=60s
test "$((SECONDS - prepare_at))" -le 180
pod_uid=$("${ns[@]}" get pod grpc-coord -o 'jsonpath={.metadata.uid}')
pod_ip=$("${ns[@]}" get pod grpc-coord -o 'jsonpath={.status.podIP}')
printf 'prepare_seconds=%s grpc_uid=%s grpc_ip=%s\n' "$((SECONDS - prepare_at))" "$pod_uid" "$pod_ip" | tee "$evidence/prepared.txt"
"${ns[@]}" run driver --restart=Never --image=localhost/ieo-x06:fixture --image-pull-policy=Never   --overrides='{"spec":{"nodeName":"ieo-x06-worker2","automountServiceAccountToken":false}}'   --env=X06_ISOLATED_KIND=ieo-x06 --env="X06_GRPC_POD_UID=$pod_uid" --env="X06_GRPC_POD_IP=$pod_ip"   --command -- /grpc-tests kind::product_paths --ignored --exact --nocapture
driver_deadline=$((SECONDS + 145))
phase=Pending
while (( SECONDS <= driver_deadline )); do
  phase=$("${ns[@]}" get pod driver -o 'jsonpath={.status.phase}')
  case "$phase" in
    Succeeded|Failed) break ;;
  esac
  sleep 1
done
collect
trap - EXIT
cat "$evidence/driver.log"
if [[ "$phase" != Succeeded ]]; then
  echo "Driver phase: $phase" >&2
  cat "$evidence/grpc-coord.log" "$evidence/http-coord.log"
  exit 1
fi
jq -e --arg uid "$pod_uid" --arg ip "$pod_ip" '
  [.items[] | select(.kind == "EndpointSlice" and .metadata.labels."kubernetes.io/service-name" == "grpc-coord")]
  | map(.endpoints[] | select(.conditions.ready == true)
      | {uid: .targetRef.uid, addresses}) == [{uid:$uid,addresses:[$ip]}]
' "$evidence/objects.json"
grep -F '1 passed; 0 failed;' "$evidence/driver.log" | tee "$evidence/result.txt"
