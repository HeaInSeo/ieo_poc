# 08-02 · 클러스터·노드 관측·시험 서버 준비 후보

`POC-REQ-08-02 / 08-02.accept / poc-acceptance-v1.3.3@4 / T05`. 격리 kind의 관측·정책과 합성 시험 초기 상태를 고정했다. 로컬 구성 검증은 PASS이고 새 클러스터 초기화 관측은 대기 상태다. [재현 절차](../../../../../../probes/0802-cluster-baseline/README.md)

## 착수 근거와 시험 환경

| 항목 | 확인 및 고정 |
| --- | --- |
| 01-06 | 기준 main `7920687472b9003d9668b44071616acec8c4c51c`의 검수 완료·verified. [시험실·증거 보존](../../01-06/lab-20261112/REPORT.md) |
| 03-06 | 기준 main에서 검수 완료·verified. #106 독립 승인 `24eb5ec`의 PR CI 4/4와 병합 `9e220a4`의 정확한 main CI 4/4를 대조했다. [수용 기록](../../03-06/p22-basic-20261119/ACCEPTANCE.md) · [PR #106](https://github.com/HeaInSeo/ieo_poc/pull/106) · [병합 SHA main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44415947698) |
| 개발 호스트 | `4.18.0-553.27.1.el8_lustre.x86_64`, cgroup v2, `/sys/kernel/btf/vmlinux` 읽기 가능. 조사 시 `/tmp`·루트 4.8 GiB, `/home` 1.1 GiB 여유. X-05의 rootless kind cgroup 위임 제약도 적용돼 호스트에서 새 kind를 시작하지 않았다. [X-05 호스트 근거](../../../extra/X-05/evidence/host-preflight.log) |
| 격리 실행 후보 | X-07에서 4노드 kind와 Cilium 1.20.5를 실행한 공개 저장소 무료 표준 Ubuntu 24.04 VM 유형을 재사용한다. 그 실행은 Docker/cgroup v2와 `7.0.0-1017-azure`를 관측했다. 08-02는 다른 클러스터 이름 `ieo-0802`와 전용 kubeconfig를 쓰며, 기동 전에 새 VM의 커널·BTF·cgroup을 다시 수집한다. [X-07 수용 및 실측](../../../extra/X-07/ACCEPTANCE.md) |
| 구성과 관측기 | kind 0.31.0, Kubernetes 1.35.0, Cilium 1.20.5, Tetragon 1.8.0과 이미지 digest를 고정했다. Cilium은 CNI 정책 집행을 맡고, Tetragon은 파일 열기·TCP 훅과 프로세스 이벤트를 관측한다. Kubernetes API를 통한 Pod 메타데이터와 Unix gRPC 소켓을 사용한다. [커널 요건](https://tetragon.io/docs/installation/faq/) · [Tetragon v1.8.0](https://github.com/cilium/tetragon/releases/tag/v1.8.0) |

## 초기 상태와 승인 범위

| 대상 | 초기 상태 및 기대값 |
| --- | --- |
| Pod 주체 | `development`, `guardrail`, `product`는 각자 다른 ServiceAccount·UID·worker를 사용한다. `control`은 관측 정책의 Pod 선택 대상 밖에 있다. 수신기는 세 역할별 허용 목적지와 공통 비교 목적지 `denied`의 네 Pod다. 전체 fixture Pod 8개는 Restricted v1.35, 비루트, 기본 seccomp, 모든 capability 제거, 자동 ServiceAccount 토큰 마운트 해제를 적용한다. [Kubernetes Restricted](https://kubernetes.io/docs/concepts/security/pod-security-standards/) |
| 네트워크 | Pod IPv4의 TCP 18080 한 경로만 사용한다. 초기에는 각 역할과 대조군이 네 수신기에 도달하는 구성을 선택한다. 후속 집행 상태는 역할별 egress 정책을 같은 이름으로 교체해 각 역할에 자기 수신기만 두고 대조군을 유지한다. 준비 상태의 수신기에는 `listening` 1개, `accepted`·`received` 0개를 요구한다. |
| 노드 관측 | `observe-fixture`는 역할 Pod의 `tcp_connect`와 고정 canary 파일의 `sys_openat`을 관측한다. 프로세스 실행·종료는 Tetragon의 기본 이벤트로 수집한다. 노드, Pod UID/컨테이너 ID, 프로세스 PID/시작 시각/UID, 합성 ID를 연결할 필드를 고정했다. 파일 열기나 연결 훅과 수신기 수락은 다른 증거로 기록한다. |
| 게시 주체 | 합성 `publisher` 계정은 fixture 클러스터에서 RoleBinding이 없다. 후속 격리 registry의 후보 슬롯에 한정한 게시 자격을 전제로 하며, 실제 registry 자격·패키지·태그는 초기 상태에 없다. |
| Pull 주체 | 합성 `puller`는 fixture 클러스터에서 RoleBinding이 없다. 후속 읽기 자격은 고정한 후보 경로의 digest pull로 한정한다. 초기에는 실제 토큰, pull secret, 캐시된 후보 선언이 없다. 공개 fixture 이미지는 인증 없는 digest pull 경로를 사용한다. |
| 승격 주체 | 합성 `promoter`만 네임스페이스의 `promotion-slot` ConfigMap 한 개를 조회·패치할 수 있도록 선언했다. 초기 값은 빈 digest와 `state=empty`다. 이 계정에는 Pod나 Deployment 수정 권한이 없다. 실제 승격에서 검토 SHA와 독립 승인, 검증한 digest, 실행 Pod digest를 대조하는 기준은 08-07에 넘긴다. |

이 후보는 격리 방어 준비다. 운영 클러스터, 실제 모델 및 외부 registry 쓰기를 사용하지 않았으며 신규 비용을 발생시키지 않았다. 실제 registry 권한과 운영 승격 자격은 별도 승인으로 다룬다.

## 로컬 검증과 후속 증거

| 검증 | 관측 |
| --- | --- |
| 공급물 | 공식 Helm 인덱스로 Cilium 1.20.5 차트 파일 SHA-256, Tetragon 1.8.0 차트 SHA-256을 대조했다. 공식 chart로 로컬 렌더링했다. Tetragon 이미지 digest 세 개, namespace JSON 필터·Unix 소켓과 Cilium kube-proxy replacement를 출력에서 확인했다. |
| 관측 명세 | `TracingPolicyNamespaced`를 Tetragon 1.8.0 차트의 CRD 구조 스키마로 검증했다. 이 검사는 정책의 커널 로드 여부를 관측하지 않는다. |
| 픽스처 | Pod 8개 및 egress 예상 경로 32/32를 명세에서 대조했다. 랩 밖 목적지·DNS 거부 입력을 포함해 주소 제한 5/5를 확인했다. 합성 승격 표시 권한과 로컬 TCP 메아리·수신 증거도 통과했다. [로컬 확인 출력](evidence/local-checks.txt) |
| 실환경 준비 판정 | 08-02 전용 VM과 네 노드의 기동, Cilium/Tetragon 4/4, observer 정책 로드, Pod 8/8 및 수신기 초기 상태, 실제 API의 합성 권한 응답은 미실행이다. 재현 절차는 이 시점의 증거와 철거를 요구한다. |

커널 로드·초기 상태 수용 후 08-03이 이벤트 귀속·관측 누락을, 08-04가 연결 수락과 차단을, 08-05가 PSA·앱 권한을 각각 검증한다. 공유 원장의 08-02 수용 상태는 별도 검토 및 통합 절차를 따른다.
