# X-06 · 제품 코디네이터 gRPC와 PostgreSQL 검증

| 항목 | 기록 |
| --- | --- |
| 착수 근거 | X-04 검수 완료, X-05 병합 `d27fa96504b568fd3156d6c5ac21e7d2f333ff9e`의 main CI 4/4. [X-05 병합 CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44317465418) |
| 동결 기준 | `e95bd8e`. [기준](criteria.md) |
| 시험 환경 | X-05에서 확인한 개발 호스트의 rootless cgroup 위임 제약을 고려해 표준 Ubuntu 러너에 전용 kind 클러스터를 띄웠다. rootless Podman, kind 0.31.0, Kubernetes 1.35.0, 기본 kindnet으로 control plane 1개와 worker 2개를 구성했다. [구성 및 재현](../../../../../probes/x06-kind-coord/README.md) |
| 제품 경로 | `ieo-coord serve --grpc-listen` 제품 바이너리와 워크스페이스의 tonic stub을 사용했다. 별도 PostgreSQL의 빈 DB 두 개에서 HTTP와 gRPC를 대조했다. |
| 범위 | 단항 업무 응답과 저장 상태를 검증한다. Pod 분산·스트림 재개는 X-05 근거로 연결한다. 실사용 모델 호출과 운영 환경 사용은 0건이다. [X-05 보고](../X-05/REPORT.md) |

## 첫 실행과 픽스처 교정

실행 `44319415250`의 제품 Pod, PostgreSQL 및 클라이언트가 준비됐다. 정상 창 100번째 사례의 첫 행동이 기존 인수 체크포인트의 전달과 연결되지 않은 `att-1`을 보냈고, 두 전송 모두 409 `CHECKPOINT_MISMATCH`를 반환했다. 다음 사례에서 재생 응답 200 기대가 깨지며 중단됐다. `3a6c2a6`에서 세대 0·1 인수 픽스처의 첫 행동을 각 전달이 호출한 시도 ID로 연결했다. 거부 사례의 잘못된 ID와 사전 동결한 판정은 유지했다. [첫 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44319415250), [원본 증거](evidence/first-attempt.tar.gz)

두 번째 실행 `44320613419`에서 첫 행동 저장 201과 재생 200을 확인했다. 이어질 두 번째 인수에는 아직 완료되지 않은 직전 체크포인트에 대한 복구 승인과 전달의 사용 기록이 필요했다. 기존 픽스처에는 그 두 행이 없어 두 전송이 409 `RECOVERY_REQUIRED`를 반환했고, 그 인수가 만드는 기록을 요구하는 후속 사례에서 중단됐다. `a870b4b`에서 같은 임시 DB 각각에 해당 승인·사용 관계를 준비했다. 코디네이터의 실제 체크포인트 호출과 응답 비교는 그대로 실행한다. [두 번째 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44320613419), [원본 증거](evidence/second-attempt.tar.gz)

## 동결 기준 실측 · PASS

세 번째 실행의 head는 `a870b4b30ecc03f64d188113a41525db786871f4`다. 세 노드에 기본 kindnet이 Ready였으며, 제품 Pod와 드라이버는 다른 worker에 있었다. 제품 Pod UID `1c7e9142-b8f2-4a6f-aaba-1c442af37185`의 주소 `10.244.2.3:50051`을 클라이언트 DNS와 Kubernetes EndpointSlice에서 대조했다. 워크로드 준비 7초, 드라이버 시험 7.02초였으며 시험 후 전용 노드 세 개를 삭제했다. [실행 및 증거 수집](https://github.com/HeaInSeo/ieo_poc/actions/runs/44321595437), [시험 워크플로](https://github.com/HeaInSeo/ieo_poc/blob/a870b4b30ecc03f64d188113a41525db786871f4/.github/workflows/x06-kind-trial.yml)

| 판정 | 관측 |
| --- | --- |
| HTTP/gRPC 및 PostgreSQL | 157/157 대응 요청을 4.707초에 완료했다. 기록된 가장 긴 gRPC 측 관측은 49ms였다. 인증 거부, repo·역할 제한, 멱등 재생에서 응답과 스냅샷 비교 차이 0건이었다. `dev-late`는 `preserved_late`로 저장됐고 현재 `latest_claims`에는 없었다. |
| 배정 중복 | 같은 배정은 200 `already_issued`, 별도 후보는 409 `OWNERSHIP_CONFLICT`였다. 두 전송군의 배정 행과 머리가 그대로였으며, gRPC 자기 배정은 `A1` 세대 0을 반환했다. |
| 커밋 전 롤백 | 잠금 대기 중 체크포인트 순번 5를 600ms 기한으로 호출했다. 서버가 `DEADLINE_EXCEEDED` 및 롤백 응답을 반환했고, DB 전후 차이 0건과 머리 순번 4를 확인했다. |
| 커밋 후 응답 유실 | 릴레이가 응답을 전달하지 않았다. 독립 DB 조회에서 `repro-x06-response-lost`의 커밋 행 1건을 확인했다. 정상 채널에서 같은 키로 확인 조회 1회 뒤 명시 재요청 1회가 200(재생)을 반환했다. 최종 행 1건, 재생 전후 스냅샷 차이 0건이었다. |

원본 아카이브는 Pod·EndpointSlice·kindnet 스냅샷, 제품 프로세스와 PostgreSQL 로그, 157건의 순번과 응답, 장애·복구 판정을 담는다. 실행이 게시한 SHA-256 `2b1364c756e91c05e5e4384e882455bf797cbea65b98142a591be66b7d52e063`와 일치함을 확인했다. 원본에서 사례 수, 최대 관측 시간, Pod 배치, 배정·롤백·재생 판정을 다시 확인했다. [원본](evidence/kind-pass.tar.gz), [재계산 요약](evidence/kind-summary.json)
