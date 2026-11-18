# X-05 gRPC 시험 픽스처

기준은 [실행 전 동결 문서](../../docs/poc/reports/extra/X-05/criteria.md)에서 확인한다. 클라이언트 하나가 Pod의 IPv4 주소를 Headless Service DNS에서 조회하고 동일한 tonic 균형 채널에 갱신한다. 서버는 합성 읽기 값과 Pod 이름, UID, 세대, 요청 ID를 돌려준다. 스트림은 시작 순번을 받으며 보류 읽기는 선택된 서버가 내려갈 때까지 응답을 대기한다.

## 실행

Linux amd64에서 워크스페이스의 잠긴 Rust 도구체인, Python 3.9 이상, rootless Podman, kind, Kubernetes 1.35와 호환되는 kubectl을 사용한다. rootless kind에 필요한 `cpu`, `memory`, `pids` 위임을 먼저 확인한다. 아래 명령은 repo 루트에서 실행한다. 별도 kubeconfig와 컨테이너 이미지 저장소로 기존 환경과 분리한다. kind를 실행하는 Podman 저장소에는 전용 kind 노드 이미지가 들어간다. [kind rootless 공식 문서](https://kind.sigs.k8s.io/docs/user/rootless/)

```bash
set -euo pipefail
x05_dir=$(mktemp -d /tmp/ieo-x05.XXXXXX)
x05_probe=probes/x05-kind-grpc
export KIND_EXPERIMENTAL_PROVIDER=podman
df -h / "$HOME"
systemd-run --user --scope --quiet -p Delegate=yes podman info   --format '{{.Host.Security.Rootless}} {{.Host.CgroupControllers}}'

CARGO_TARGET_DIR="$x05_dir/target" CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0   cargo build --locked -p ieo-x05-probe
mkdir "$x05_dir/image"
install -m 0755 "$x05_dir/target/debug/ieo-x05-probe" "$x05_dir/image/ieo-x05-probe"
strip "$x05_dir/image/ieo-x05-probe"
cp "$x05_probe/Containerfile" "$x05_dir/image/"
x05_podman=(podman --root "$x05_dir/graph" --runroot "$x05_dir/run" --storage-driver vfs)
"${x05_podman[@]}" build --network none -t localhost/ieo-x05:fixture "$x05_dir/image"
"${x05_podman[@]}" save --format docker-archive -o "$x05_dir/fixture.tar" localhost/ieo-x05:fixture
sha256sum "$x05_dir/fixture.tar"

systemd-run --user --scope --quiet -p Delegate=yes kind create cluster   --config "$x05_probe/kind.yaml" --kubeconfig "$x05_dir/kubeconfig" --wait 120s
systemd-run --user --scope --quiet -p Delegate=yes kind load image-archive   --name ieo-x05 "$x05_dir/fixture.tar"
kubectl --kubeconfig "$x05_dir/kubeconfig" --context kind-ieo-x05 wait   --for=condition=Ready node --all --timeout=120s
kubectl --kubeconfig "$x05_dir/kubeconfig" --context kind-ieo-x05 -n kube-system   rollout status daemonset/kindnet --timeout=120s
python3 "$x05_probe/run.py" --kubeconfig "$x05_dir/kubeconfig" --evidence "$x05_dir/evidence"
```

`run.py`는 전용 kubeconfig와 4개 노드의 상태를 확인한 뒤 전용 namespace를 생성한다. 각 변경에서 Ready Pod의 UID·IP와 EndpointSlice를 대조한다. `run.jsonl`에는 단계별 결과가, `client.jsonl`에는 연결 주소·시도·응답·시각이 저장된다. 마지막 판정은 `run.jsonl`의 `result` 행에 있다. 증거 수집 후 전용 클러스터를 삭제한다.

```bash
systemd-run --user --scope --quiet -p Delegate=yes kind delete cluster   --name ieo-x05 --kubeconfig "$x05_dir/kubeconfig"
```

## 루프백 검증

`cargo test --locked -p ieo-x05-probe`는 로컬 실소켓의 주소 추가·제거를 시험한다. `python3 probes/x05-kind-grpc/loopback.py 바이너리경로 새증거디렉터리`는 한 TCP 주소에서 시험 서버 프로세스를 교체해 연결 복구, 순번 재개 및 명시적 단항 재요청을 시험한다. kind의 Ready 주소 전파와 멀티 노드 분포에는 위 클러스터 시험을 사용한다.
