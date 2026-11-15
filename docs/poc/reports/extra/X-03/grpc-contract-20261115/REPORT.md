# X-03 · gRPC 내부 API 계약과 분리 영향 확인

| 항목 | 기록 |
| --- | --- |
| 계획·작업 | v1.3.3-A4 · 추가 작업 X-03(설계, 선행 02-03) · 73·12 완료 수 미산입 · 근거 [사용자 결정 2026-11-14 · gRPC](../../../../User_Decisions_2026-11-14_grpc.md), [계획](../../../../Plan_gRPC_Internal_API_v0.md) §4, [개정 A4](../../../../Amendment_v1.3.3-A4.md) |
| 수용 기준 | 장부 §7 X-03 행 |
| 산출물 | proto [`contracts/proto/ieo/internal/v0/supervisor.proto`](../../../../../../contracts/proto/ieo/internal/v0/supervisor.proto) · lint·호환 규칙 [`contracts/proto/buf.yaml`](../../../../../../contracts/proto/buf.yaml) · 계약 설명 [`contracts/ieo-internal-v0.md`](../../../../../../contracts/ieo-internal-v0.md) |
| 검사 | `buf lint`·`buf build`·`protoc` 통과([evidence/proto-checks.txt](evidence/proto-checks.txt)) |
| 모델 호출 | 0회. 실제 호출·예비 회차·결제 없음 |
| 상태 | 계약 초안. 구현 없음(X-04) |

## 1. 수용 기준 대응

| 수용 기준(장부 §7 X-03) | 위치 |
| --- | --- |
| proto와 설명 | proto 파일 + 설명 문서. 설명 문서도 계약의 일부다 |
| 서비스·메시지 정의, 버전 호환(필드 번호 재사용 금지, 비호환 변경은 새 패키지) | 설명 §1.1·§2. `ieo.internal.v0.SupervisorService`의 RPC 다섯 개. `reserved` 규칙과 `buf breaking`(WIRE_JSON) |
| 인증과 역할별 권한(HTTP와 같은 자격·범위) | 설명 §3. metadata Bearer, 같은 `auth::Credentials`, 새 범위 없음 |
| 상태 코드 대응과 기존 오류 code의 상세 사유 | 설명 §4. HTTP 상태 + `code`로 정하는 대응표, RPC별 `code` 표, trailer `ieo-error-bin`(`Error` 메시지) |
| 작업별 재시도 가능 여부와 멱등(연결 실패만으로 재실행 금지, 재조회로 반영 확인) | 설명 §5. RPC별 표, “결과 불명”의 정의, 02-04 대기 사본 규칙과의 대응 |
| 제한 시간과 취소(커밋 전 취소는 롤백, 커밋 뒤 끊김은 재조회) | 설명 §6. 서버 최대 처리 시간 15초(초안), 커밋 전 확인 |
| 기존 HTTP/JSON 계약 유지, 업무 규칙 재정의 없음 | 설명 §1.4·§2. HTTP 계약·시험 변경 없음. JSON 문서는 바이트로 실어 같은 스키마로 검사 |
| 미리 분리할 코드와 그 영향 | 설명 §9(main 3157c89에서 다시 확인) |

## 2. 이번에 정한 것

계획이 X-03에 넘긴 결정이다.

1. **gRPC 표면**: 에이전트 보고, 재현 요청 제출·조회, 체크포인트 보고, 최신 체크포인트 조회. 계획 이후 02-04가 만든 체크포인트 작업 둘을 넣었다.
2. **스프린트 상태·사건 조회는 넣지 않는다.** 에이전트 자격의 `/v0/sprints*` 금지를 유지한다. supervisor의 배정 정보는 02-05의 자기 배정 조회로 얻는다(설명 §1.2).
3. **재현 요청의 supervisor 재사용**(PR #41 검토 b3cb083): 재사용한다. guardrail supervisor만, 지금의 `agent:repro`로(설명 §3).
4. **새 작업의 HTTP 형태**: 02-05의 새 작업도 HTTP와 gRPC를 함께 둔다(설명 §1.4).
5. **오류 상세 형식**: `google.rpc.Status` 대신 `ieo-error-bin` trailer에 HTTP 상태와 기존 `code`·`rejections[]`를 그대로 싣는다(설명 §4.3).
6. **전송 쪽 동작**: 크기 상한, DB 연결 실패는 `UNAVAILABLE`, 헬스는 `grpc.health.v1`(설명 §7).
7. **스트림**: v0에는 없다. 더할 때의 규칙만 정했다(설명 §8.1).

## 3. 코드 읽기로 확인한 것

계약을 쓰며 main 3157c89의 처리 코드를 다시 읽었다.
- 같은 `code`가 작업에 따라 HTTP 상태가 다르다(`DIGEST_MISMATCH`: 등록 422, 체크포인트 409). 그래서 gRPC 상태를 `code`만으로 정하지 않고 HTTP 상태와 함께 정했다.
- 재현 요청에서 대상 SHA가 다르면 `ASSIGNMENT_SHA_MISMATCH`가 아니라 `STALE_ASSIGNMENT`다. 형식 검사(`SCHEMA_VIOLATION`)는 권한·식별 검사를 통과한 새 요청에만 한다.
- 보고와 재현 요청의 중복 판정은 파싱한 JSON 값을 비교한다. 체크포인트의 digest는 받은 바이트로 계산한다. 두 사실이 “두 전송에서 같은 요청은 같은 요청”의 근거다(설명 §2).
- 계획 §3 이후 `handle`에 작업별 실행 모양 차이가 생겼다: DB 없는 역할 제어, 트랜잭션 밖의 supervisor 호출 뒤 트랜잭션(`compare`). 작업 표가 이를 표현해야 한다(설명 §9).

## 4. 넘기는 것과 한계

- **X-04**: 작업 표 분리, tonic 어댑터, 커밋 전 확인, supervisor `CoordinatorClient` 경계, 공통 회귀(설명 §10), CI의 `buf lint`·`buf breaking`.
- **02-05**: 새 작업(자기 배정 조회·인수, 세션 상태)을 작업 표와 이 서비스에 추가. 토큰을 쥐는 프로세스의 분리.
- **한계**(설명 §11): 라이브러리 지원은 아직 확인하지 않았다. 15초·재시도 값은 초안이다. 크기 초과 응답이 HTTP와 다르다. 오류 상세는 자체 trailer다. TLS는 정식 설계의 몫이다.
- **검사의 범위**: lint·build는 정의의 형식만 확인한다. HTTP와 같은 동작인지는 구현(X-04)의 공통 회귀가 확인한다.
- 02-05의 선행·수용 기준은 바꾸지 않는다. X-04가 늦으면 02-05는 HTTP로 진행한다.
