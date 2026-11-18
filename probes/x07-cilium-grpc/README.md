# X-07 · Cilium Envoy를 거치는 gRPC 시험

[동결 기준](../../docs/poc/reports/extra/X-07/criteria.md)을 사용한다. X-05의 tonic 서버, 요청 생성기, Kubernetes 장애 주입, 판정기를 재사용한다. `client-vip HOST`는 DNS의 IPv4 Service 주소 한 개에 연결하고, 각 TCP 연결을 기록한다. `run.py`는 시험 시작 전에 별도 L4 Service로 대조를 수행하고, Cilium이 만든 설정과 해당 노드의 활성 Envoy 설정을 보존한 뒤 L7 구간을 실행한다.

## 재현

격리 amd64 VM에 Docker Engine, cgroup v2, kind 0.31.0, Helm 3, Kubernetes 1.35와 호환되는 kubectl이 필요하다. 커널과 cgroup 요구 사항 및 핀 값은 동결 기준을 따른다. [격리 시험에서 실행한 워크플로](https://github.com/HeaInSeo/ieo_poc/blob/86df06cab9f7ec53ccdc505b6a1d495c8bd35eee/.github/workflows/x07-cilium-trial.yml)에 전체 명령이 있다.

1. `cargo build --locked -p ieo-x05-probe`로 공용 시험 바이너리를 빌드하고 [시험 이미지](../x05-kind-grpc/Containerfile)에 넣어 `localhost/ieo-x05:fixture`로 태그한다.
2. Docker provider와 [kind 구성](kind.yaml)으로 전용 `ieo-x07` 클러스터를 만든다. kubeconfig는 별도 파일로 지정한다. 시험 이미지를 `kind load docker-image localhost/ieo-x05:fixture --name ieo-x07`로 전달한다.
3. 고정 Cilium 차트를 [values](cilium-values.yaml)로 설치하고, Docker kind 네트워크의 control plane IP와 API 포트 6443을 각각 `k8sServiceHost`, `k8sServicePort`로 지정한다.
4. 노드와 Cilium 준비 뒤 `python3 probes/x07-cilium-grpc/run.py --kubeconfig 전용파일 --evidence 새디렉터리`를 실행한다. 마지막 `run.jsonl`의 결과, 별도 L4 클라이언트 로그, L7 클라이언트의 실제 연결 ID 및 노드의 Envoy 설정을 대조한다.
5. `kind delete cluster --name ieo-x07 --kubeconfig 전용파일`로 전용 노드를 정리한다.
