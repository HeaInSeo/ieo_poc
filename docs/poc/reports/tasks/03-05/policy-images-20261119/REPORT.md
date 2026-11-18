# 03-05 정책 내장 이미지와 CI 후보

`POC-REQ-03-05 / 03-05.accept / poc-acceptance-v1.3.3@4`. T04 산출물은 통합 이미지 한 개, 공통 기반에서 파생된 Go·Rust 이미지 두 개, 공통 실행기·프로파일 및 워크플로 후보다. 03-04에서 확인한 범용·Go·Rust·`HeaInSeo/ieo_poc` 범위의 자료를 고정한다. [03-04 수용](../../03-04/p25-20261119/ACCEPTANCE.md) · [02-04 수용](../../02-04/acceptance-20261115/ACCEPTANCE.md) · [실험 계획 T04·P22·P23](../../../../Console_Coordinator_PoC_Validation_Plan.md)

## 입력과 구성

| 입력 | 고정 방법 |
| --- | --- |
| 기준 자료 | #102 병합 `0d8d74b0360854839b74f49d95459764d9a14c80`, P25 승인 `2aac476e7fad82f3b36fedb14dee1ffdd4eed508`, 색인 v0.2.2. 승인된 커밋이 병합 커밋의 선조인지를 확인한다. 문서·색인과 명시한 근거 파일은 Git 보관본에서 가져오고 P25 입력 해시와 이미지에 내장한 파일 목록의 해시를 점검한다. 원본의 역사적 링크는 저장소에서 확인할 수 있다. |
| OS·공용 도구 | Debian bookworm linux/amd64 계열 기반을 OCI manifest digest와 image ID로 고정했다. 기반의 Go 1.26.9 실행 경로를 제거하고 지정한 변형에만 새 도구를 넣었다. GCC 12.2.0, Git 2.39.5와 Python 버전은 이미지 내 실행 기록으로 확인한다. |
| Go | Go 1.26.10. 정규화한 로컬 도구 전체 tar의 SHA-256을 빌드 전에 확인한다. |
| Rust | rustc/Cargo 1.99.1, rustfmt 1.10.0-stable, Clippy 0.1.99. 정규화한 로컬 도구 전체 tar의 SHA-256을 빌드 전에 확인한다. |
| 프로파일·워크플로 | 프로파일 v0.1.0은 03-03의 외부 의존성 없는 소형 Go 모듈과 Rust 크레이트를 대상으로 한다. 정책 실행기는 정해진 명령을 쓰고 버전·맥락을 출력한다. 빌드 보고가 Containerfile, 실행기, 프로파일, 워크플로의 SHA-256을 남긴다. |

[잠금 값](inputs.env) · [Containerfile](Containerfile) · [공통 실행기](verify.py) · [프로파일](profiles.json) · [P25 색인](../../03-04/p25-20261119/INDEX.md)

Go는 형식·vet·기능 검사를, Rust는 형식·Clippy·기능 검사를 실행한다. 프로파일의 필수 테스트 이름이 성공으로 정확히 한 번 관측돼야 한다. 기록에는 실행한 명령과 원본 출력, 실패 시 종료 코드를 남긴다. 빌드와 시험 모두 네트워크를 차단했다. 시험 컨테이너의 루트와 입력을 읽기 전용으로 마운트하고 UID 65532와 임시 캐시를 사용한다.

## 로컬 재현

고정 기반이 Podman 로컬 저장소에 있어야 한다. 해시와 버전이 맞는 Go·Rust 도구 디렉터리를 넘긴다. 빌드 ID에는 새 값을 사용한다.

```bash
./build.sh "$GO_TOOLCHAIN_DIR" "$RUST_TOOLCHAIN_DIR" local-01
./smoke.sh local-01 /tmp/ieo-policy-smoke-local-01-logs
```

[빌드 도구](build.sh) · [격리 시험 도구](smoke.sh)

## 워크플로 후보

`workflow-candidate.yml`은 PR 원본 head 또는 main push SHA를 체크아웃하고 실행 전에 실제 head를 대조한다. 기본 GitHub 토큰의 권한은 읽기로 제한하고 checkout 자격을 보관하지 않는다. 컨테이너 참조는 digest가 필수다. 연결할 격리 저장소의 `T04_POLICY_IMAGE`, `T04_POLICY_PROFILE`이 정해져야 한다. 외부 의존성이나 DB가 있는 대상은 별도 승인 프로파일과 의존성 입력 경로가 필요하다. 워크플로는 후보 위치에서 YAML·셸을 검사했다. 레지스트리 업로드와 Actions 활성화는 대기한다. [워크플로 후보](workflow-candidate.yml)

## 빌드와 격리 시험 결과

Podman 4.9.4-rhel, linux/amd64. 빌드 ID `20261119-r5`. 내장 정책 파일 목록의 SHA-256은 `7a9ef56123a2f32ef4dd596af6e17735cb19a55facfbe745b269da72c7d6ab41`다. 아래 크기는 로컬 이미지 검사에서 관측한 바이트다.

| 후보 | OCI manifest digest | 이미지 크기(바이트) |
| --- | --- | ---: |
| go | `sha256:74ee35f8bd046fab1e777624ef1bc28b7389ef060ef8b65f8579fe8ac2919bfb` | 1,092,296,164 |
| rust | `sha256:3f32fc97df0e89737c1b2b8386a9859efba037daf544db372e2d19b62f9c66db` | 1,513,538,534 |
| combined | `sha256:6ab2fe6cfe05744c636e3ed1459fd23b2c80af807aeee29088843e13f371f784` | 1,738,450,787 |

격리 대조 13/13이 기대 결과와 일치했다. 시작 점검과 문서 해시는 이미지 3개에서 통과했다. Go·Rust 예제는 언어별 이미지와 통합 이미지에서 모두 통과했다. 맞지 않는 두 스택은 거부됐다. 필수 테스트의 skip·ignored를 관측한 두 경우는 FAIL(exit 1), 해당 테스트 이름이 기록에 없던 두 경우는 INCONCLUSIVE(exit 2)로 중단됐다. [빌드 원본](evidence/build.log) · [13건 결과](evidence/results.txt) · [통합 버전](evidence/info-combined.log)

첫 통합 빌드에서 Rust 계층을 복제할 때 디스크가 부족했다. 빌드할 때 도구를 하나씩 풀고, 통합 단계에서는 확정한 개별 이미지 ID를 사용해 Rust 파생에 Go 계층만 추가하도록 바꿨다. rootless 시험에서 pids cgroup이 위임되지 않아 해당 플래그를 빼고 네트워크 차단·읽기 전용·비특권 사용자로 같은 이미지를 실행했다. 최종 빌드와 시험은 `/tmp`의 별도 Podman graph·runroot에서 진행했고, 기존 이미지는 읽기 전용으로 참조했다. 기본 그래프의 이전 실패 산출물은 여유 공간 부족으로 정상 삭제 명령이 중단됐다. [제약 관측](evidence/host-limits.txt)

컨테이너에서 내장 색인을 `/opt/ieo-policy/source/docs/poc/reports/tasks/03-04/p25-20261119/INDEX.md`에서 읽을 수 있다. 실행기의 `info`는 내장 파일 해시, 프로파일과 실제 도구 버전을 확인한다.

## 후속 검증 범위

03-06에서 두 역할의 별도 작업공간·권한·자격 분리, 생략·비호환 프로파일, 같은 입력의 독립 재현을 P22-B로 확인한다. 승인된 CI 설정과 예외, 기존 CI의 실제 보호 동작은 P23에서 검증한다. 셔플·레이스·DB·문서 시험과 냉·온 캐시 비용 비교도 각 승인 범위에서 별도 계측한다. 모델 호출 0건, 추가 청구 USD 0.
