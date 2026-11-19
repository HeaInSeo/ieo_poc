# 08-02 · 격리 클러스터와 시험 초기 상태

[08-02 준비 보고서](../../docs/poc/reports/tasks/08-02/cluster-baseline-20261119/REPORT.md)의 범위에서 사용한다. 이 절차는 격리 클러스터의 초기 상태를 확인한다. 행위 귀속, 실제 차단, 배포·앱 권한 검증은 08-03~08-05에서 수행한다.

## 실행 환경

전용 amd64 VM에 Python 3.11 이상, rootful Docker Engine과 cgroup v2, Linux 5.14 이상, 읽을 수 있는 BTF, Helm 3, `kind` 0.31.0과 `kubectl` v1.35.0이 필요하다. 공개 저장소의 무료 표준 Ubuntu 24.04 러너는 X-07의 격리 실행에 사용했다. kind amd64 파일의 SHA-256은 `eb244cbafcc157dff60cf68693c14c9a75c4e6e6fedaf9cd71c58117cb93e3fa`다. kubectl은 v1.35.0의 공식 체크섬과 대조한다. [kind 릴리스](https://github.com/kubernetes-sigs/kind/releases/tag/v0.31.0) · [kubectl 설치](https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/) · [Tetragon 커널 요건](https://tetragon.io/docs/installation/faq/)

```bash
lab=$(mktemp -d)
python3 -B probes/0802-cluster-baseline/check.py
bash probes/0802-cluster-baseline/run-lab.sh "$lab/run"
```

`run-lab.sh`는 기존 `ieo-0802` 노드가 없는지 먼저 확인하고, kind 생성 전에 정리 트랩을 설치한다. 전용 kubeconfig와 loopback API만 사용한다. Cilium·Tetragon 공식 차트는 [고정 버전과 체크섬](lab-pins.env)으로 대조한다. 성공·실패 시 전용 노드를 삭제하고 노드 잔존 여부를 `evidence/cleanup.txt`에 기록한다. [Cilium kind 설정](https://docs.cilium.io/en/stable/installation/kind/) · [Tetragon 설치](https://tetragon.io/docs/installation/kubernetes/)

## 실행 전 판정 기준

| 항목 | 기대값 |
| --- | --- |
| 환경 | Linux·BTF·cgroup v2의 VM 및 4개 노드 기록. Kubernetes v1.35.0 노드 4/4, Cilium 4/4, Tetragon agent 4/4, Tetragon operator Ready. CNI 300초, 관측기 agent 300초, operator 180초 한도. |
| 초기 정책 | Cilium endpoint 8개가 Ready이고 초기 NetworkPolicy 6개가 생성 명세와 일치. 집행용 교체 명세는 서버 dry-run으로만 검사하며 활성 설정은 그대로 유지. Tetragon의 네 agent가 `ieo-0802/observe-fixture`를 활성 상태로 보고. 초기 Pod 준비, endpoint 준비, 정책 활성에 각 120초. |
| 픽스처 | Pod 8개 Ready·재시작 0, 실제 `imageID`와 컨테이너 ID 보존. 수신기 4개 각각 처음에는 `listening` 1개, `accepted`·`received` 0개. 역할의 허용 수신기, 대조군의 금지 비교 수신기에 대한 초기 연결 2건은 각각 합성 ID를 되돌리고 수신 원본을 남긴다. |
| 권한·캐시 | 실제 Kubernetes API의 합성 RBAC 조회 24건에서 승격 주체의 표시 조회·패치 2건만 허용. 표시 digest 빈 값·`state=empty`, fixture 네임스페이스 Secret 0개와 imagePullSecrets 없음. 네 노드에 `registry.invalid/ieo-0802/candidates` 캐시 항목 0개. |
| 정리 | 전용 kind 삭제 성공과 남은 노드 0개. 원본은 `evidence/`에 남기며 kubeconfig를 포함하지 않는다. |

관측 정책은 역할 Pod의 TCP 훅과 고정 canary 파일 열기를 수집한다. 이후 08-04에서 역할별 egress를 같은 이름으로 교체해 네트워크의 실제 차단을 검증한다. 대상은 Pod IPv4와 TCP 18080이다. 후속 관측은 Pod UID·컨테이너 ID·PID·프로세스 시작 시각·UID·합성 ID를 결합하고, Tetragon 상태와 사용 가능한 손실 지표를 대조한다. TCP 훅과 수신기 `accepted`는 각각 연결 시도와 수락의 증거다. [TCP 훅 예제](https://github.com/cilium/tetragon/blob/v1.8.0/examples/tracingpolicy/tcp-connect.yaml) · [파일 훅 예제](https://github.com/cilium/tetragon/blob/v1.8.0/examples/tracingpolicy/openat_write.yaml) · [Tetragon 지표](https://tetragon.io/docs/reference/metrics/) · [NetworkPolicy](https://kubernetes.io/docs/concepts/services-networking/network-policies/)
