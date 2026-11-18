# X-07 · Cilium L7 gRPC 서비스 분산·복구

**기능 시험 PASS.** 4개 노드의 격리 kind에서 동일 채널의 정상 L7 읽기 1,500건이 기대 Pod에 정확히 균등하게 분산됐다. 시험 완료 시 노드 4개를 제거했다. [성공 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44346080838) · [환경 및 정리 로그](evidence/runtime.log)

| 항목 | 기록 |
| --- | --- |
| 착수 근거 | 기준 main `ad7628c`의 진행 원장에서 X-05·X-06 검수 완료. [X-05 시험](../X-05/REPORT.md) · [X-06 수용](../X-06/ACCEPTANCE.md) |
| 시험 기준 | `5aa3587`에서 실행 전 동결. [관측 및 판정](criteria.md) |
| 개발 호스트 | 시작 시 `/home` 2.8 GiB, 루트 3.5 GiB 여유. 커널 `4.18.0-553.27.1.el8_lustre.x86_64`. X-05에서 전용 rootless Podman scope에 필요한 cgroup 컨트롤러가 위임되지 않아 노드 생성 전에 kind가 중단됐다. 이 근거를 재사용했다. [X-05 호스트 기록](../X-05/evidence/host-preflight.log) |
| 격리 조합 | 공개 GitHub repo의 표준 Ubuntu 24.04 러너, Docker Engine, 고정 kind 0.31.0·Kubernetes 1.35.0, Cilium 1.20.5의 고정 OCI 차트. VM 안의 전용 kubeconfig 및 클러스터를 사용한다. [재현](../../../../../probes/x07-cilium-grpc/README.md) · [지원 조합](https://docs.cilium.io/en/stable/network/kubernetes/compatibility/) |

Cilium의 서비스 L7 annotation 경로를 선택했다. Cilium 1.20.5의 operator 소스는 Ready endpoint를 Envoy EDS로 전하고 downstream 프로토콜을 선택하는 upstream HTTP 설정을 만든다. 이 시험은 Service별 round robin과 h2c를 확인한다. 비교 기준은 Pod별 주소를 직접 분산하는 X-05와 동일한 표본·정확성 조건이다. X-07 클라이언트는 고정 Service VIP 한 개를 사용하며 실제 소켓 연결을 기록한다. [Cilium Envoy 생성 소스](https://github.com/cilium/cilium/blob/v1.20.5/operator/pkg/ciliumenvoyconfig/envoy_config.go) · [Cilium L7 Service 문서](https://docs.cilium.io/en/stable/network/servicemesh/envoy-load-balancing/)

## 픽스처 준비

루프백에서 X-05의 동일 채널 주소 추가·제거 회귀와 새 VIP 클라이언트의 30건·TCP 연결 1개를 확인했다. 잠긴 tonic·tower 버전은 유지했다. 첫 표준 러너 실행은 빌드 단계의 Docker 기본 파일 이름과 저장소 `Containerfile`이 달라 멈췄다. Docker 파일 인자를 명시했고, Kubernetes와 같은 v1.35.0 kubectl을 사용하도록 고정했다. [첫 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44343935985) · [오류 로그](evidence/first-run-build.log)

두 번째 실행에서 Cilium agent와 Envoy가 네 노드에서 Ready였고, 클라이언트 노드의 Envoy에서 이 Service의 HTTP/2·round-robin 클러스터가 활성화됐다. 두 백엔드까지 Ready였으나 대조군 클라이언트가 `ParseIntError`로 종료됐다. Kubernetes가 Service 이름에서 주입하는 `PROBE_PORT`는 `tcp://…` 형식이고 픽스처는 숫자 포트를 기대한다. X-07 클라이언트 Pod에서 `enableServiceLinks: false`로 이 주입을 끄고 Service DNS를 사용하도록 했다. 기준과 서버 동작은 유지했다. [두 번째 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44344449923) · [실패 원본](evidence/control-start-fail.tar.gz) · [Kubernetes 1.35의 Service 변수 생성 소스](https://github.com/kubernetes/kubernetes/blob/v1.35.0/pkg/kubelet/envvars/envvars.go#L74-L104)

세 번째 실행은 Helm의 대기가 반환됐을 때 agent 4/4와 Envoy 2/4를 관측했다. 동결한 환경 준비 기준을 충족하기 전에 기능 시험을 시작하려 해 INCONCLUSIVE로 종료됐다. 실행 절차는 기존 300초 환경 준비 한도에서 agent·Envoy 4/4와 CoreDNS Ready까지 기다리도록 교정했다. [세 번째 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44345324477) · [환경 증거](evidence/readiness-inconclusive.tar.gz)

## 실측

실행 커밋 `86df06cab9f7ec53ccdc505b6a1d495c8bd35eee`에서 Cilium과 CoreDNS가 설치 단계 약 64초 안에 준비됐다. VM의 Linux 커널은 `7.0.0-1017-azure`, Docker cgroup은 v2였다. 클라이언트 노드 `ieo-x07-worker3`에서 생성된 EDS 클러스터와 Envoy에 활성화된 `x07-probe/probe` HTTP/2 설정을 확인했다. 주석이 없는 L4 대조군에서는 같은 Pod 집합을 대상으로 한 TCP 연결에서 30/30건이 한 Pod로 갔다. L7은 Service VIP `10.96.227.17:50051`을 통한 TCP 연결 `10.244.3.132:59512` 하나가 스트림과 장애 전환을 포함한 전 과정에서 유지됐다. [원본](evidence/verified-run.tar.gz) · [판정 로그](evidence/run.jsonl)

| 정상 구간 | Pod별 성공 | 변경 후 Ready 수렴 | 변경 중 일반 호출의 최대 성공 간격 | 변경 중 일반 호출 오류 |
| --- | --- | --- | --- | --- |
| 초기 | 150 / 150 | — | — | — |
| 복제본 2 → 3 | 100 / 100 / 100 | 1,201 ms | 119 ms | 0 |
| 스트림 Pod 삭제·교체 | 100 / 100 / 100 | 3,282 ms | 198 ms | 1 |
| 롤링 업데이트 v1 → v2 | 100 / 100 / 100 | 4,073 ms | 299 ms | 5 |
| 보류 단항 호출 Pod 삭제·교체 | 100 / 100 / 100 | 2,320 ms | 103 ms | 0 |

각 정상 구간은 300/300건 성공, 최대 분포 편차 0%p, 발행 29,899~29,901 ms, 마지막 결과까지 29,902~29,903 ms였다. 모든 요청 ID와 Ready Pod UID·세대를 대조했다. 변경 중 오류 6건은 정상 구간 표본 밖에서 발생했다. [원본 재집계](evidence/recheck.json)

- 스트림은 삭제 후 2,091 ms에 `Internal`로 중단됐다. 기존 Pod에서 순번 13까지 받았고, 다른 Pod에서 14부터 명시적으로 재개한 뒤 연속 5개를 받는 데 1,003 ms가 걸렸다.
- 보류 단항 호출은 삭제 후 2,094 ms에 `Unavailable`을 받았다. 살아 있는 Pod 로그에서 자동 재실행은 0건이었다. 같은 ID로 한 번 명시적으로 재요청해 3 ms에 성공했고 서버 처리 시작 기록은 1건이었다.
- 원본 압축본의 SHA-256은 `e72dad8e5897d22afea54c83926318951c0643d76e3e76d12ce7d7b91090b62f`이다. GitHub 로그에 찍힌 체크섬과 일치한다. [성공 실행 로그](https://github.com/HeaInSeo/ieo_poc/actions/runs/44346080838/job/134948709972) · [루프백 회귀](evidence/loopback-test.log)

판정 범위는 위 격리 조합의 시험용 읽기·스트림이며, 운영 배포 및 실제 모델 호출은 0건이다. 독립 검토·PR 제출·공통 원장 반영은 중앙 작업의 통합 순서를 따른다.
