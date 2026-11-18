# X-06 · kind에서 제품 코디네이터 gRPC 검증

[실행 전 동결 기준](../../docs/poc/reports/extra/X-06/criteria.md)에 따라 두 `ieo-coord` 제품 프로세스, 새 PostgreSQL 및 시험 드라이버를 실행한다. 각 coordinator Pod는 같은 제품 바이너리와 구성을 사용하고 독립 DB를 바라본다. 드라이버는 다른 worker에서 Headless Service의 주소로 제품 API를 호출한다. 정상 비교 창의 gRPC 채널은 재사용한다.

`crates/ieo-coord/tests/grpc/kind.rs`의 `write_config`는 X-04의 자격·정책·스프린트 픽스처를 제품 바이너리가 읽는 파일로 직렬화한다. 모든 자격과 작업 데이터는 X-06 임시 클러스터용이다. 일반 `cargo test`는 클러스터가 필요한 두 시험을 건너뛴다.

## 재현

시험 실행 커밋의 워크플로가 Linux amd64 빌드, digest 고정 이미지, rootless kind 생성, 검증 및 클러스터 정리를 순서대로 수행한다. 지원되는 rootless Podman 호스트에서는 동일한 순서로 실행할 수 있다.

1. 잠긴 워크스페이스에서 `cargo build --locked -p ieo-coord --bin ieo-coord`와 `cargo test --locked -p ieo-coord --test grpc --no-run --message-format=json`으로 제품 바이너리와 `grpc` 테스트 실행 파일을 만든다.
2. 비어 있는 디렉터리를 만들고 `X06_CONFIG_OUT=디렉터리 grpc-테스트-바이너리 kind::write_config --ignored --exact`로 구성 파일을 쓴다. 빌드 컨텍스트에 제품 바이너리를 `ieo-coord`, 테스트 바이너리를 `grpc-tests`로 복사한다. [Containerfile](Containerfile)로 `localhost/ieo-x06:fixture` 이미지를 만든 뒤 아카이브로 내보낸다.
3. `KIND_EXPERIMENTAL_PROVIDER=podman` 및 위임된 사용자 scope에서 [kind.yaml](kind.yaml)로 클러스터를 만든다. kubeconfig는 전용 파일로 지정한다. `kind load image-archive --name ieo-x06`으로 아카이브를 전달하고 세 노드와 기본 kindnet의 Ready를 확인한다.
4. repo 루트에서 `bash probes/x06-kind-coord/run.sh 전용-kubeconfig 구성-디렉터리 증거-디렉터리`를 실행한다. 증거 디렉터리는 미리 만든다. 종료 시 `driver.log`와 `objects.json`, 각 coordinator의 로그로 사례와 Pod 식별자를 확인한다.
5. `kind delete cluster --name ieo-x06 --kubeconfig 전용-kubeconfig`로 시험 클러스터와 임시 DB를 정리한다.

전송 손실은 시험 드라이버의 TCP 릴레이가 한 연결의 응답을 차단해 만들고, 독립 DB 관측과 정상 제품 채널의 조회를 거쳐 같은 키를 명시적으로 한 번 재요청한다. 롤백 시험은 전용 PostgreSQL의 자문 잠금을 사용한다.
