# X-05 · 기본 kind gRPC 서비스 발견·분산·복구

| 항목 | 기록 |
| --- | --- |
| 작업 기준 | `b3846434a46c3d0f89ae08feb1fc2ab8edeb1ad6`의 진행 원장에서 X-03 검수 완료(`0c243ee`), X-04 검수 완료(`84c9668`) 확인. [진행 원장](../../../progress/STATUS.md) |
| 실행 기준 | [gRPC 계획 §5~§7](../../../Plan_gRPC_Internal_API_v0.md), [시험 기준](criteria.md). 첫 실행 전 `8d31eca`에서 동결 |
| 픽스처 | [실행 방법과 구성](../../../../../probes/x05-kind-grpc/README.md). Headless Service Ready 주소, worker 3개, kindnet, 재사용 클라이언트 한 개. 롤링 업데이트는 동일 시험용 이미지에서 Pod 템플릿의 revision 값을 변경 |
| 범위 | 합성 gRPC 읽기와 순번 스트림. 제품 DB·업무 쓰기 계약은 X-06, 물리 장애·실제 서버 간 네트워크·운영 처리량은 별도 검증 범위 |

## 잠긴 의존성 확인

`Cargo.lock`의 tonic 0.14.9와 tower 0.5.3을 개발 호스트의 crate 캐시와 대조했다. 파일 SHA-256은 각각 `e2cdfc673aabd30fe8fee92e65837b644c37c5d47f0f97ae6d5e0f1237d15e1b`, `ebe5ef63511595f1344e2d5cfa636d973292adc0eec1f0ad45fae9f0851ab1d4`로 잠금 값과 일치했다.

| 동작 | 소스와 적용 |
| --- | --- |
| 주소 발견 | IPv4 Headless Service DNS가 Ready Pod 주소를 제공한다. 픽스처가 Tokio `lookup_host`의 IPv4 결과 집합을 1초 간격으로 확인한다. 조회 실패나 시간 초과 시 마지막 정상 집합을 유지한다. [Kubernetes DNS 문서](https://kubernetes.io/docs/concepts/services-networking/dns-pod-service/) |
| 주소 변경 | tonic `channel/mod.rs:144–169`의 `balance_channel`은 `Channel`과 change sender를 반환한다. 사용해야 할 공개 열거형은 `tonic::transport::channel::Change`다. `service/discover.rs:55–69`는 추가할 endpoint를 연결 객체로 만들고 Tower의 Insert/Remove로 변환한다. 클라이언트가 주소 집합의 차이에 이 API를 사용한다. [tonic 소스의 채널](https://github.com/hyperium/tonic/blob/4c0f455c264fe2ec9f9ab5d0cc7fd2824d5daf70/tonic/src/transport/channel/mod.rs#L144-L169), [변경 통지](https://github.com/hyperium/tonic/blob/4c0f455c264fe2ec9f9ab5d0cc7fd2824d5daf70/tonic/src/transport/channel/service/discover.rs#L36-L69) |
| 새 RPC 분포 | tonic `channel/mod.rs:215–228`의 `Balance::new`는 tower P2C를 사용한다. tower `p2c/service.rs:105–184,208–259`는 키로 삽입·삭제하고 준비된 두 연결을 임의로 뽑아 부하를 비교한다. tonic `service/connection.rs:162–168`의 부하값은 항상 0이므로 이 경우 표본 중 먼저 뽑힌 연결이 선택된다. 균등 수용 편차는 실측 분포에 적용한다. 클라이언트 복제본은 같은 buffer 및 작업을 공유한다. [Tower P2C 문서](https://docs.rs/tower/0.5.3/tower/balance/p2c/struct.Balance.html), [Tower 선택 소스](https://github.com/tower-rs/tower/blob/4b0a6b0e688bd177eb2c9c97f5268dd9703c66fc/tower/src/balance/p2c/service.rs#L105-L259), [tonic 부하값](https://github.com/hyperium/tonic/blob/4c0f455c264fe2ec9f9ab5d0cc7fd2824d5daf70/tonic/src/transport/channel/service/connection.rs#L162-L168) |
| 감지와 재연결 | tonic `service/connection.rs:59–76,114–117`은 keepalive를 HTTP/2 builder에 넘기고 `Reconnect`를 사용한다. `service/reconnect.rs:86–175`는 연결 준비 오류 후 다음 준비 단계에서 재접속할 수 있다. 이미 전달한 RPC의 future는 상위 호출에 결과를 반환한다. 재개·재요청은 픽스처의 명시적 호출이다. [keepalive 설정](https://github.com/hyperium/tonic/blob/4c0f455c264fe2ec9f9ab5d0cc7fd2824d5daf70/tonic/src/transport/channel/service/connection.rs#L59-L76), [Reconnect](https://github.com/hyperium/tonic/blob/4c0f455c264fe2ec9f9ab5d0cc7fd2824d5daf70/tonic/src/transport/channel/service/reconnect.rs#L86-L175) |

## 개발 호스트

첫 확인에서 여유는 `/home` 2.9 GiB, `/` 6.5 GiB였다. kind 생성 직전에 각각 2.9 GiB, 6.2 GiB였다. Podman 4.9.4는 rootless cgroup v2로 동작했고 kind는 `v0.32.0-alpha+9145d421e0d4f7`였다. 일회성 사용자 scope의 `Delegate` 값은 `yes`였고 `cgroup.controllers`는 비어 있었다. root에서 하위로 활성화된 컨트롤러는 `memory pids`였고 `user.slice` 아래에는 둘도 전파되지 않았다. kind는 노드 및 이미지의 준비 전 사전 검사에서 거부했다. 이 kind 버전은 rootless Podman 4 이상에서 `cpu`·`memory`·`pids`가 모두 보고되는지 확인한다. 전용 node 컨테이너와 kubeconfig는 생성되지 않았다. [호스트 증거](evidence/host-preflight.log), [kind 생성 로그](evidence/kind-create.log), [kind rootless 문서](https://kind.sigs.k8s.io/docs/user/rootless/), [kind provider 소스](https://github.com/kubernetes-sigs/kind/blob/9145d421e0d4f7/pkg/cluster/internal/providers/podman/provider.go#L367-L426)

## 루프백 결과

- tonic 채널 하나에서 실제 TCP 서버 세 개의 주소 집합을 `{a,b}`에서 `{b,c}`로 바꿨다. 교체 전후 각각 150회에서 기대한 서버만 관측했다. 빠진 주소의 서버는 검증 중에도 살아 있었다. [Rust 실행 로그](evidence/loopback-addresses.log)
- 같은 주소의 서버 프로세스를 종료했을 때 기존 스트림은 1 ms 후 HTTP/2 오류를 반환했다. 마지막 순번 5를 사용한 명시 호출로 새 서버에서 6~10을 연속 수신했다. 단항 호출도 종료 1 ms 후 전송 오류를 반환했다. 서버 로그는 명시적 재요청 전 0건, 후 1건을 확인했고 같은 요청 ID로 응답을 받았다. 마지막 창에서는 29,900 ms 동안 발행한 300건이 모두 성공했다. [루프백 실행 요약](evidence/loopback.jsonl)
- 잠긴 픽스처의 Cargo 빌드·fmt·Clippy, kind/워크로드 YAML 파싱, 문서 패키지·진행 원장 렌더링 검사를 통과했다. 시스템에 glibc 정적 링크 라이브러리가 없어 amd64 distroless C/C++ 런타임의 고정 digest를 사용했다. 전용 임시 Podman 저장소에서 34,926,751바이트의 nonroot 이미지를 만들었다. 외부 네트워크 없이 이미지 내부에서 로컬 주소 해석과 gRPC 응답 19건을 관측했다.

## 기본 kind 실측 · PASS

개발 호스트의 컨트롤러 위임 제약이 확인되어 공개 저장소의 무료 표준 `ubuntu-24.04` 러너에서 같은 기준과 픽스처로 실행했다. `cpu`·`memory`·`pids`가 위임된 rootless Podman, kind 0.31.0, digest 고정 Kubernetes 1.35.0, 기본 kindnet을 사용했다. 네 노드와 kindnet이 준비된 뒤 168.676초 동안 시험했고 전용 노드 네 개를 삭제했다. 실행 head는 `035d35d3efd54ff5b3b747f81bb1a2a51c6af015`다. 일회용 실행 워크플로는 이 head에 보존했다. [실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44301093184), [워크플로](https://github.com/HeaInSeo/ieo_poc/blob/035d35d3efd54ff5b3b747f81bb1a2a51c6af015/.github/workflows/x05-kind-trial.yml), [표준 러너 요금](https://docs.github.com/en/billing/concepts/product-billing/github-actions)

| 관측 창 | Ready Pod별 성공 건수 | 변경에서 Ready·EndpointSlice 수렴까지 | 변경 중 최장 성공 간격 / 일시 오류 |
| --- | --- | --- | --- |
| 초기 2개 | 142 / 158 | — | — |
| 2→3 증설 | 103 / 88 / 109 | 1.516초 | 106ms / 0건 |
| 스트림 Pod 삭제·교체 | 104 / 104 / 92 | 3.243초 | 201ms / 2건 |
| v1→v2 롤링 업데이트 | 106 / 99 / 95 | 5.807초 | 115ms / 0건 |
| 보류 단항 Pod 삭제·교체 | 88 / 107 / 105 | 2.248초 | 201ms / 3건 |

- 처음부터 끝까지 동일한 클라이언트 Pod와 연결 균형 채널을 사용했다. 각 창의 발행 간격 합계는 29.900초였고 마지막 결과까지 29.901~29.902초였다. 1,500건이 모두 성공했고 응답 요청 ID와 기대 UID·세대가 일치했다. 균등 비율에서 가장 큰 편차는 4.00%p였다. 각 변경 후 Ready 집합의 전체 UID에서 30초 안에 응답을 받았다.
- 스트림을 제공하던 Pod를 삭제한 뒤 2.072초에 기존 호출의 전송 오류를 관측했다. 원래 스트림에서 관측한 마지막 순번 13을 보내 같은 채널로 호출을 새로 열었다. 다른 Pod에서 14~18을 연속 관측했다.
- 보류 읽기는 타깃 서버의 처리 시작 1건을 확인한 뒤 Pod를 삭제했다. 2.068초 뒤 원래 호출의 전송 오류가 반환됐다. 생존·교체 Pod 로그에서 명시 재요청 전 같은 ID의 호출은 0건이었다. 명시적으로 같은 ID를 한 번 호출해 다른 v2 Pod에서 같은 의미 값을 받았고, 현재 전체 서버의 해당 ID 처리 시작 합계는 1건이었다.

원본 아카이브에는 Kubernetes 노드·CNI·Pod·EndpointSlice 스냅샷, 클라이언트 RPC 및 주소 로그, 단항 서버 로그가 들어 있다. 실행 로그에 출력된 아카이브의 SHA-256 `06f7fc596c0ec89a4dc4f3fd737bc7e6f65c6b772014af5fc6e4f49670173cbf`를 대조했다. 이 원본으로 Pod별 분포와 관측 창, 스트림 순번, 재요청 횟수를 다시 계산해 실시간 판정과 일치함을 확인했다. [원본 아카이브](evidence/kind-evidence.tar.gz), [단계 로그](evidence/kind-run.jsonl), [재계산 결과](evidence/kind-summary.json)

이 결과로 X-05의 기본 경로에는 Headless Service와 기존 tonic 변경 통지·균형 채널을 사용한다. HTTP/2 keepalive는 구성했으며 이번 장애 주입은 Pod 강제 종료였다. X-05 검수와 원장의 완료 반영은 독립 검토, 필수 CI, 승인 head 병합과 병합 SHA의 main CI 확인을 따른다.

## 수용 기록 · 2026-11-18

PR #97은 head `b5075384b1e1229eff4034cb7a8fb95fe9d1d7c6`의 독립 승인과 PR CI 4/4를 확인한 뒤 해당 head를 조건으로 병합했다. 병합 SHA `d27fa96504b568fd3156d6c5ac21e7d2f333ff9e`에서 main CI 4/4가 통과했다. X-05를 검수 완료로 기록한다. [PR 승인·병합 기록](https://github.com/HeaInSeo/ieo_poc/pull/97), [PR CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44303120976), [병합 SHA main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44317465418)
