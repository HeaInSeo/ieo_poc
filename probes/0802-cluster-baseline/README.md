# 08-02 · 격리 클러스터와 시험 초기 상태

[08-02 준비 보고서](../../docs/poc/reports/tasks/08-02/cluster-baseline-20261119/REPORT.md)의 범위에서 사용한다. 이 절차는 네트워크와 관측 장비의 초기 상태를 확인한다. 실제 정책 차단과 행위 귀속의 수용은 08-03·08-04에서 검사한다.

## 고정 버전과 설치

전용 amd64 VM에서 Docker Engine, cgroup v2, Linux 5.14 이상, 읽을 수 있는 `/sys/kernel/btf/vmlinux`, BPF·tracefs를 확인한다. X-07에서 사용한 GitHub 공개 저장소의 무료 표준 Ubuntu 24.04 VM이 후보이며, VM 밖의 호스트 설정 변경은 필요하지 않다. `kind` 0.31.0의 amd64 실행 파일 SHA-256은 `eb244cbafcc157dff60cf68693c14c9a75c4e6e6fedaf9cd71c58117cb93e3fa`다. `kubectl`은 v1.35.0 체크섬을 확인한다. [kind 릴리스](https://github.com/kubernetes-sigs/kind/releases/tag/v0.31.0) · [Tetragon 커널 요건](https://tetragon.io/docs/installation/faq/)

```bash
set -euo pipefail
export KIND_EXPERIMENTAL_PROVIDER=docker
lab=$(mktemp -d)
cfg="$lab/kubeconfig"
test -r /sys/kernel/btf/vmlinux
test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs
test "$(docker info --format '{{.CgroupVersion}}')" = 2
clusters=$(kind get clusters)
if printf '%s\n' "$clusters" | grep -Fxq ieo-0802; then exit 1; fi
kind create cluster --config probes/0802-cluster-baseline/kind.yaml --kubeconfig "$cfg" \
  --image docker.io/kindest/node:v1.35.0@sha256:452d707d4862f52530247495d180205e029056831160e22870e37e3f6c1ac31f
k=(kubectl --kubeconfig "$cfg" --context kind-ieo-0802)
trap 'kind delete cluster --name ieo-0802 --kubeconfig "$cfg"' EXIT
"${k[@]}" config view --minify -o json | jq -e '.clusters[0].cluster.server | startswith("https://127.0.0.1:")'
"${k[@]}" get node -o json | jq -e '[.items[].metadata.name] | sort == ["ieo-0802-control-plane","ieo-0802-worker","ieo-0802-worker2","ieo-0802-worker3"]'
```

전용 VM 안에 `ieo-0802`가 처음 생성된 경우에만 아래를 진행한다. 네 개의 노드 이름과 loopback API 주소를 대조한다. CNI 차트와 관측기 차트를 검증한 뒤 순서대로 설치한다. [Cilium kind 설정](https://docs.cilium.io/en/stable/installation/kind/) · [Tetragon Kubernetes 설치](https://tetragon.io/docs/installation/kubernetes/)

```bash
set -euo pipefail
helm pull oci://quay.io/cilium/charts/cilium --version 1.20.5 --destination "$lab" 2>&1 | tee "$lab/cilium-pull.log"
grep -Fx 'Digest: sha256:873a782bf1064b08fbb05f64c4ed50fcb66d6f05f981f817bddd91f038b69935' "$lab/cilium-pull.log"
curl -fsSL --output "$lab/tetragon-1.8.0.tgz" https://helm.cilium.io/tetragon-1.8.0.tgz
printf '%s  %s\n' 38a10bca4bffd3efac7cbcc7f99b24dbcd3ef76402aa7bbb72c0b2e2dc0c8111 "$lab/tetragon-1.8.0.tgz" | sha256sum -c
api_ip=$(docker inspect -f '{{ (index .NetworkSettings.Networks "kind").IPAddress }}' ieo-0802-control-plane)
helm install cilium "$lab/cilium-1.20.5.tgz" --kubeconfig "$cfg" --kube-context kind-ieo-0802 -n kube-system \
  -f probes/0802-cluster-baseline/cilium-values.yaml --set-string "k8sServiceHost=$api_ip" --set k8sServicePort=6443
"${k[@]}" wait --for=condition=Ready node --all --timeout=300s
"${k[@]}" -n kube-system wait --for='jsonpath={.status.numberReady}=4' daemonset/cilium --timeout=300s
helm install tetragon "$lab/tetragon-1.8.0.tgz" --kubeconfig "$cfg" --kube-context kind-ieo-0802 -n kube-system \
  -f probes/0802-cluster-baseline/tetragon-values.yaml
"${k[@]}" -n kube-system wait --for='jsonpath={.status.numberReady}=4' daemonset/tetragon --timeout=300s
"${k[@]}" wait --for=condition=Established crd/tracingpoliciesnamespaced.cilium.io --timeout=120s
```

## 초기 상태 캡처

1. `python3 probes/0802-cluster-baseline/manifests.py initial` 출력과 `observe` 출력을 전용 증거 디렉터리에 기록한다. 전용 컨텍스트에 `initial`, `observe` 순으로 적용한다. 네임스페이스에서 Pod 8개가 120초 안에 Ready이고 재시작 0인지 확인한다. 수신기 Ready는 로컬 파일로 확인한다. 초기 수신기 로그는 `listening` 1개, `accepted` 및 `received` 0개가 기대값이다.
2. 노드별 kernel·BTF·cgroup, 실제 Cilium/Tetragon daemonset과 `imageID`, namespace Restricted v1.35, Pod 명세·Pod UID·컨테이너 ID·IP·node·서비스 계정, CNI 정책과 역할별 허용 상태를 원본으로 남긴다. 각 노드 agent의 `tetra tracingpolicy list`에 관측 정책이 활성 상태인지 확인하고, 소속 Pod의 실행/파일/연결 시도 전후 `export-stdout` 원본과 노드별 지표를 보존한다. [Tetragon 측정 지표](https://tetragon.io/docs/reference/metrics/)
3. `promotion-slot` ConfigMap은 `digest=""`, `state=empty`로 고정한다. 전용 VM 관리자만 수행하는 `kubectl auth can-i patch configmap/promotion-slot -n ieo-0802 --as=system:serviceaccount:ieo-0802:promoter`는 `yes`를, 같은 명령의 publisher·puller·역할 계정과 promoter의 `patch pods`는 `no`를 기대한다. 호출에는 반드시 위 kubeconfig 및 context를 전달한다. 조회 결과와 초기 ConfigMap을 저장하며 외부 registry 자격은 이 시험에 주입하지 않는다.
4. 08-03에서 합성 요청 ID로 각 Pod의 초기 관측을 대조한다. 08-04의 실제 차단 차례에는 `enforce` 출력으로 역할별 egress 정책을 **같은 이름에 적용**한다. 역할마다 해당 수신기만 남으며, `control`은 네 수신기에 대한 기존 허용을 유지한다. `initial`을 다시 적용해 정책을 복귀시킬 수 있다. 사례는 Pod IP와 TCP 18080만 참조한다. [NetworkPolicy의 결합 규칙](https://kubernetes.io/docs/concepts/services-networking/network-policies/#behavior-of-to-and-from-selectors)
5. 증거를 보존하고 트랩의 전용 클러스터 삭제 로그와 노드가 남지 않았음을 확인한다. 초기화 검증에서 빠진 조건은 관측 불가로 기록한다.

TCP 관측 훅은 연결 시도의 증거다. 클라이언트 결과와 수신기의 `accepted`·`received`는 별도로 대조한다. 프로세스는 역할 Pod의 raw JSON과 Tetragon 프로세스 메타데이터를 결합해 식별한다. 원본에서 `ringbuf_events_lost`, `ringbuf_queue_events_lost`, listener overflow 및 Pod metadata fetch 오류를 가능한 항목별로 전후 대조한다. 초기 상태 또는 지표가 확보되지 않으면 유효 관측 범위가 성립할 때까지 후속 개별 판정을 유보한다. [TCP 훅 예제](https://github.com/cilium/tetragon/blob/v1.8.0/examples/tracingpolicy/tcp-connect.yaml) · [파일 훅 예제](https://github.com/cilium/tetragon/blob/v1.8.0/examples/tracingpolicy/openat_write.yaml)
