# 08-05 / P11 · 배포 정책과 애플리케이션 권한

`POC-REQ-08-05 / 08-05.accept / poc-acceptance-v1.3.3@4`. 기준 main `3bd07a17040d09a7a6ac4373968f22ace08bdb1b`에서 01-01과 08-02 검수 완료를 확인했다. 이 병합 SHA의 필수 main CI는 4/4였다. [01-01 근거](../../01-01/scope-20261112/REPORT.md) · [08-02 근거](../../08-02/cluster-baseline-20261119/REPORT.md) · [main CI](https://github.com/HeaInSeo/ieo_poc/actions/runs/44509084334)

## 판정 범위

기존 `840d649`의 개발·Guardrail 서버 dry-run 대조로 시작했다. 사례와 기준은 실행 전에 `5cdc60a32390f21e2084a5d61ab470b13dedddac`에 고정했다. 08-02의 kind 4노드, Kubernetes 1.35.0·Cilium 1.20.5·Tetragon 1.8.0 및 fixture를 공개 표준 Ubuntu 24.04 전용 VM에서 재현했다. [고정 기준과 실행](../../../../../../probes/0805-policy-boundaries/README.md) · [공개 표준 러너 요금](https://docs.github.com/en/billing/concepts/product-billing/github-actions)

| 대조 | 판정 대상 |
| --- | --- |
| 입장 제어 | Restricted v1.35에서 두 정상 명세를 수용하고, 역할별로 `allowPrivilegeEscalation`만 `true`인 두 명세를 거부한다. 모두 서버 dry-run으로 확인하며 실제 역할 Pod의 준비 상태와 UID·ServiceAccount를 보존한다. [Kubernetes Restricted](https://v1-35.docs.kubernetes.io/docs/concepts/security/pod-security-standards/) |
| 네트워크 | 두 역할과 두 수신기의 TCP 18080 연결을 초기·집행·복귀 상태에서 비교한다. 역할별 egress를 같은 이름으로 교체한다. 클라이언트 결과와 수신기 수락·수신 원본을 함께 판정한다. [Kubernetes NetworkPolicy](https://v1-35.docs.kubernetes.io/docs/concepts/services-networking/network-policies/) |
| 합성 앱 | 네트워크 복귀 후 역할별 소유자가 있는 HTTP 객체에서 읽기·쓰기 응답과 원본 상태를 대조한다. 전용 합성 토큰은 해당 VM에서만 생성되며, 원본 증거에 Secret과 토큰을 남기지 않는다. |
| 파일·자격 격리 | P07 원장 회차와 역할별 거부 원본이 없어 **미검증**이다. 이 증거 없이 08-05 전체를 수용하지 않는다. 앱 결과의 범위는 이번 합성 fixture이며 제품 인가 결과는 별도 근거다. |

운영 클러스터, 실제 제품 계정·DB, 실제 모델과 유료 러너는 사용하지 않았다. 08-03 산출물과 공유 원장은 별도 담당의 변경 범위다.

## 실측

고정 SHA `5cdc60a`의 공개 표준 Ubuntu 24.04 러너에서 커널 `7.0.0-1017-azure`, cgroup v2, BTF를 확인했다. 08-02 초기화 절차를 통과한 뒤 P11을 실행했고, 전용 kind 노드 4개를 삭제해 잔존 노드 0개를 확인했다. [격리 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44516157756) · [실행 workflow](https://github.com/HeaInSeo/ieo_poc/blob/5cdc60a32390f21e2084a5d61ab470b13dedddac/.github/workflows/0805-isolated-p11.yml) · [철거 로그](evidence/cleanup.txt)

| 대조 | 결과 |
| --- | --- |
| PSA 4/4 | 개발·Guardrail 정상 명세 2건을 수용하고 한 필드 변형 2건을 Restricted v1.35의 `allowPrivilegeEscalation` 오류로 거부했다. dry-run 객체 저장 0건. 실제 역할 Pod 두 개의 서로 다른 UID·서비스 계정과 재시작 0을 확인했다. |
| 역할별 네트워크 12/12 | 초기 4건과 복귀 4건이 성공했다. 집행 중 자기 수신기 연결 2건은 성공했고 상대 수신기 연결 2건은 시간 초과였다. 두 거부 사례의 수신기 수락·수신은 각각 0건이었다. |
| 합성 앱 12/12 | 정상 응답 8건, 상대 객체 읽기·쓰기의 403 응답 4건. 403 본문에는 거부 표식만 있었으며 소유자 전후 조회에서 두 객체가 그대로였다. 각 소유자의 자기 객체 쓰기와 읽기는 성공했고 서버 감사 원본의 주체·대상·응답이 일치했다. |
| 파일·자격 격리 | P07의 독립 거부 증거 대기. 08-05 검수 완료 및 P11 평가 종결은 이 후보의 범위에 포함하지 않는다. |

원본 압축본 SHA-256은 `e1776fc87b250f241b36b067cba3db8dbe417439d176c755ecfb31d38d68e694`다. Actions 체크섬과 일치한다. 역할별 입장 제어 명세·응답, 케이스별 연결 결과·수신 원본, 앱 감사 기록과 초기화 증거를 보존했다. [원본](evidence/isolated-p11.tar.gz) · [재집계](evidence/recheck.json) · [로컬 앱 대조](evidence/local-app-check.txt)
