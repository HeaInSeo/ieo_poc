# X-07 · Cilium Envoy gRPC 분산·복구 시험 기준

기준 main `ad7628c5f5f4342e9cd3a41a9a1785b842db77c1`의 진행 원장에서 X-05와 X-06 수용을 확인했다. X-05의 동일 기능·정확성 기준으로 Cilium L7 경로를 별도 실행한다. [X-05 기준](../X-05/criteria.md) · [X-05 실측](../X-05/REPORT.md) · [X-06 수용](../X-06/ACCEPTANCE.md) · [사용자 결정 §6](../../../User_Decisions_2026-11-14_grpc.md)

## 고정 조합

- 공개 repo의 무료 표준 GitHub-hosted Ubuntu 24.04 amd64 VM 1대, VM에 설치된 Docker Engine, cgroup v2, Linux 5.14 이상을 사용한다. API는 VM의 loopback에 바인딩한다. 전용 kubeconfig로 control plane 1개·worker 3개를 만든다. 개발 호스트에서는 X-05의 rootless Podman 위임 제약을 재사용하고 전용 VM에서만 Cilium을 시작한다. [Cilium kind 문서](https://docs.cilium.io/en/stable/installation/kind/) · [Cilium 시스템 요구 사항](https://docs.cilium.io/en/stable/operations/system_requirements/) · [표준 러너 요금](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
- kind 0.31.0과 Kubernetes 1.35.0 node image `sha256:452d707d4862f52530247495d180205e029056831160e22870e37e3f6c1ac31f`를 사용한다. Cilium 1.20.5의 OCI chart `sha256:873a782bf1064b08fbb05f64c4ed50fcb66d6f05f981f817bddd91f038b69935`를 검증한다. 기본 CNI와 kube-proxy 설치를 끄고 Cilium에 `ipam.mode=kubernetes`, `kubeProxyReplacement=true`, `socketLB.hostNamespaceOnly=true`를 둔다. [kind 릴리스](https://github.com/kubernetes-sigs/kind/releases/tag/v0.31.0) · [Cilium Kubernetes 호환성](https://docs.cilium.io/en/stable/network/kubernetes/compatibility/)
- Cilium의 Service L7 기능과 Envoy를 켜고, `ClusterIP` Service에 `service.cilium.io/lb-l7=enabled`, `service.cilium.io/lb-l7-algorithm=round_robin`을 설정한다. X-05와 같은 tonic 시험 서버와 메시지를 사용한다. 클라이언트는 Service DNS를 IPv4 VIP 하나로 확인하고, 그 VIP에 설정한 직결 채널 하나를 재사용한다. TCP 연결 성립을 클라이언트 자체가 기록한다. [L7 Service 문서](https://docs.cilium.io/en/stable/network/servicemesh/envoy-load-balancing/) · [1.20.5 Envoy 생성 소스](https://github.com/cilium/cilium/blob/v1.20.5/operator/pkg/ciliumenvoyconfig/envoy_config.go)
- k8s API·격리 클러스터·시험 Pod 이외의 서비스에 시험 요청을 보내지 않는다. 운영 서비스·DB·모델 실행은 0회다. 자원 구성이 실패하면 VM 안의 관련 로그를 남기고 격리 클러스터를 철거한다.

## 관측 및 판정

| 항목 | 기준 |
| --- | --- |
| 환경 증거 | 노드 4개 및 Cilium agent·Envoy Ready, 기존 CNI 및 kube-proxy 없음. 서비스의 실제 annotation, 생성 CiliumEnvoyConfig와 클라이언트 노드의 실제 Envoy cluster 설정을 보존한다. 300초 안에 Cilium과 노드, Service 준비 후 120초 안에 해당 Envoy 설정 확인. |
| L4 대조 | 같은 Pod 집합을 가리키는 별도 `ClusterIP` Service로 일반 읽기 30건을 100ms 간격으로 발행한다. 성공 30건, 서로 같은 Ready Pod UID, 요청 ID 일치, 연속 표본 동안 같은 TCP 연결. |
| Cilium L7 | X-05의 초기·증설·스트림 Pod 교체·롤아웃·단항 Pod 교체를 같은 순서로 실행한다. 각 정상 창은 300건, 발행 29~31초, 전체 33초 이내, 성공 300건, 기대 UID·세대 및 요청 ID 일치, 균등 기대치 ±10%p. 각 정상 창의 첫 발행부터 마지막 결과까지 새 TCP 연결은 0개다. |
| 변경·복구 | X-05의 Ready/EndpointSlice 수렴 120초, 그 뒤 정상 단항 회복 30초, 변경 중 일반 호출 성공 사이 간격 10초를 적용한다. 기존 스트림 중단 및 동일 채널에서 명시적 재개 각각 10초 이내, 원래 UID 고정 및 새 UID에서 순번 5개 연속. 보류 읽기 삭제 후 전송 오류 10초 이내, 자동 재실행 0, 동일 ID 명시 재요청 1회·2초 이내, 최종 처리 시작 1건. |

Ready 집합과 기대 UID가 다른 성공 응답, 표본 누락·시간 위반 또는 예기치 않은 재생은 실패로 처리한다. L7 판정은 생성된 서비스 설정과 해당 노드의 활성 Envoy 설정, L4 고정 대조 및 L7에서 여러 Pod의 실측 응답을 함께 요구한다. 시험 환경이 이 관측에 필요한 구성을 시작하지 못하면 INCONCLUSIVE와 차단 단계·로그를 기록한다. 시작된 기능 시험의 기준 위반은 FAIL이다. 필수 기준을 모두 충족하면 PASS다. X-05·X-06의 판정은 별도로 유효하다.
