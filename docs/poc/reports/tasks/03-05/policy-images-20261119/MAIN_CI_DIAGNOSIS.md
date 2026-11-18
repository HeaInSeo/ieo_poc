# #103 첫 main CI 정산 시험 조사

병합 SHA `ed15dabafc2c2cb55eb5ac7bbb3afa09a2263dae`의 첫 main CI는 3/4였다. 기존 `ieo-supervisor` 정산 시험 `not_enough_evidence_writes_nothing`에서 거부는 관측됐지만 `in flight` 문구 단정에 실패했다. 첫 로그는 반환 오류의 다른 내용을 출력하지 않는다. PR의 부모와 병합 트리에서 Rust 실행·시험 코드, Cargo 고정값과 CI 워크플로가 같은 바이트였다. [GitHub 첫 실행](https://github.com/HeaInSeo/ieo_poc/actions/runs/44404757078/attempts/1)

같은 병합 소스의 격리 사본에서 실패한 단정에 오류 출력만 추가했다. 8개 시험 스레드와 exec 지연을 둔 추적의 14번째 회차에서 동일 단정이 `another caller holds the ledger`로 실패했다. 시험 스레드가 `caller.lock`을 잡은 상태에서 다른 시험이 자식을 만들었다. 부모의 닫기 약 1.7ms 뒤, 자식 exec가 완료되기 전에 다음 `flock(LOCK_EX|LOCK_NB)`이 `EAGAIN`을 받았다. 이 구간에서 자식은 닫기 전 파일 디스크립터를 상속한다. 추적은 exec 진입에 20ms 지연을 넣은 대조이며 수치 자체를 GitHub 실행의 관측값으로 사용하지 않는다. [추적·시험 발췌와 원본 해시](evidence/main-ci-settlement-reproduction.txt)

별도 통제에서는 잠금을 잡았을 때와 풀고 진행 중 표식을 확인했을 때 모두 거부됐다. 장부 바이트와 진행 중 표식이 보존됐고 정산 증거 디렉터리는 생성되지 않았다. 기존 코드는 잠금 획득에 실패하면 기록 읽기와 쓰기에 앞서 반환한다. 재현한 현상은 안전 거부가 잠깐 앞선 경합이다. GitHub 원본에 반환 오류와 syscall이 기록되지 않아 첫 실행의 세부 시간 순서는 확인할 수 없다.

지연 없는 격리 병렬 반복 600회에서 15회가 실패했고, 한 시험은 잠금 거부 문구를 출력했고, 다른 시험은 CLI 성공 단정에서 실패했다. 합성 잠금 대조를 추가한 격리 사본에서 직렬 반복 50/50이 통과했다. 별도 교정의 범위는 이 통합 시험 바이너리 안에서 서브프로세스가 있는 시험을 직렬화하고, 잠금 경합과 진행 중 표식의 무기록 거부를 각각 주장하는 것이다. 실행 코드와 정산 계약은 기존 값으로 검증했다. 실제 모델 호출 및 운영 작업 0건.

같은 병합 SHA의 main CI 전체 재실행은 4/4를 통과했다. [재실행 2회차](https://github.com/HeaInSeo/ieo_poc/actions/runs/44404757078/attempts/2)
