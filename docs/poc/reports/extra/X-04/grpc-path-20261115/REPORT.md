# X-04 · gRPC 경로 구현과 HTTP·gRPC 공통 회귀

| 항목 | 기록 |
| --- | --- |
| 계획·작업 | v1.3.3-A4 · 추가 작업 X-04(구현, 선행 X-03) · 73·12 완료 수 미산입 · 근거 [사용자 결정 2026-11-14 · gRPC](../../../../User_Decisions_2026-11-14_grpc.md), [계획](../../../../Plan_gRPC_Internal_API_v0.md) §3·§6 |
| 수용 기준 | 장부 §7 X-04 행. 계획 §6의 supervisor 클라이언트 전환도 X-04 범위(장부 기준에는 없음, PR #41 검토 b3cb083) |
| 계약 | [`contracts/ieo-internal-v0.md`](../../../../../../contracts/ieo-internal-v0.md)(X-03, 이번 구현으로 고친 곳은 그 문서 §12) |
| 시험 | [evidence/test.log](evidence/test.log): fmt·clippy(-D warnings)·전체 시험(시험 DB 사용) |
| 모델 호출 | 0회. 실제 호출·예비 회차·결제 없음. 영속 PoC DB(55433)와 코디네이터 서비스는 건드리지 않음 |
| 상태 | 구현 완료, 독립 검토 전 |

## 1. 구현

| 단계 | 내용 |
| --- | --- |
| 1 작업 표(동작 변경 없음) | `ieo_coord::api`: `Operation`·실행 모양(`Shape`)·`route`·`dispatch`. `handle` = `route` + `dispatch`. 트랜잭션 실행기에 커밋 전 확인(`TxGuard`). HTTP는 늘 통과하는 `NoGuard`. 기존 HTTP 시험은 의미 변경 없이 통과 |
| 2 gRPC 서버 | 같은 `ieo-coord` 바이너리의 선택 플래그 `--grpc-listen ADDR [--grpc-workers N]`. 없으면 지금과 같은 HTTP 서버다. 새 crate `ieo-proto`: protox로 proto를 읽어 빌드 때 생성(CI·개발에 protoc 불필요), §4.1 상태 대응·`ieo-error-bin`·JSON 변환을 한 곳에. 어댑터는 HTTP와 같은 본문을 만들어 `api::dispatch`를 부른다(업무 규칙 없음). 연결을 쥔 작업자 풀, deadline(min(클라이언트, 15초), 처리 기한 = deadline − 50ms), `SET LOCAL statement_timeout`, SQLSTATE 57014 → `DEADLINE_EXCEEDED`, 취소 감지(요청 future drop → 표지, 트랜잭션 시작 전·커밋 직전 확인), 메시지 상한 1 MiB + 64 KiB, `tonic-health`(5초 주기 DB 읽기) |
| 3 supervisor 클라이언트 | `central.rs`: `CoordinatorClient` 경계, `HttpClient`(02-04 코드 그대로)와 `GrpcClient`. 설정 `checkpoint_central.transport` = `http`(기본)·`grpc`. 거부 확정/결과 불명 분류는 `Central` 한 곳(4xx = 대기 사본 삭제, 그 밖 = 유지). gRPC는 trailer의 `http_status`로 나누고 trailer 없는 상태는 결과 불명. 크기·repo_id 사전 계산을 넘으면 보내지 않고 HTTP 400과 같은 거부 확정. 자동 재전송 없음, 확인 읽기 자동 반복 0회 |
| 4 공통 회귀 | `crates/ieo-coord/tests/grpc.rs`(실제 PostgreSQL·HTTP 서버·gRPC 어댑터), `tests/operations.rs`, `checkpoint_e2e.rs`의 gRPC 변형, supervisor `tests/serve.rs`의 FakeCentral 시험 3건을 두 전송으로, `tests/central.rs` |
| 5 계약 문구·CI | 계약 §3·§4.2·§4.3·§5·§6·§7·§9~§12(X-03 검토 8e3e817의 P-1·P-2·I-7~I-10과 구현 사실). CI에 필수 아닌 작업 “proto buf lint and breaking” 추가(buf 1.47.2 릴리스 바이너리, sha256 고정 — 공식 sha256.txt와 실제 내려받은 파일의 해시가 같음을 확인). 필수 체크 이름·보호 규칙은 바꾸지 않음 |

## 2. 수용 기준 대응

| 장부 §7 X-04 | 근거 |
| --- | --- |
| X-03 계약대로 gRPC 경로 추가 | 단계 2. RPC 5개 |
| 업무 규칙 재사용, 다시 구현하지 않음 | 어댑터는 `api::dispatch`만 부른다. 상태 대응·trailer·변환은 `ieo-proto` 한 곳 |
| 기존 HTTP 시험 그대로 통과 | 기존 시험 의미 변경 없음(`tests/coordinator.rs`의 도우미는 `tests/common`으로 기계적으로 옮김). HTTP 동작 변경이 없어 OpenAPI 그대로 |
| 같은 요청에 같은 판정과 DB 상태 | `grpc.rs` 94회 호출 동등 시나리오: 같은 바이트를 HTTP·gRPC로 각자의 DB에 보내 매 호출 뒤 답(상태·code·rejections·메시지)과 DB 내용이 같음 |
| 인증·역할 권한 | 401, 403 네 code, development의 재현 요청, 다른 repo·역할, 클라이언트 자격의 에이전트 작업 |
| 중복 배정 방지와 멱등 | 한 전송으로 저장하고 다른 전송으로 재전송 → 200·`REPLAYED`, 새 행 없음(보고·재현 요청·체크포인트). `EVENT_CONTENT_CONFLICT`. 명시적 `null` 재현 요청의 `ALREADY_EXISTS` |
| 실패 시 트랜잭션 롤백 | 쓰기 뒤 거부(변조 기록 형식)의 롤백, 커밋 전 deadline·취소의 롤백, 가드 거부의 롤백 |
| 옛 세대 결과 보존과 현재 판정 제외 | `preserved_late`·`counts_for_current_judgment: false`, `current_assignment: false`, 스프린트 상태에서 제외 |
| 연결 중단 뒤 재전송의 중복 반영 방지(전송 성공과 서버 반영이 다른 경우 포함) | TCP 중계가 서버의 응답을 버린다 → 서버는 커밋했다 → 같은 키로 gRPC·HTTP 재조회 200, 두 번째 기록 없음 |
| supervisor 클라이언트 전환(계획 §6) | 단계 3. 02-04 체크포인트 시험이 두 전송으로 통과, 코디네이터 바이너리 + 역할 서비스 gRPC 종단 시험 |

## 3. 계획과 다르거나 남은 것

- **확인 읽기 자동 반복은 0회**로 정했다. 계약의 “최대 3회” 안이고 HTTP 클라이언트와 사용자 결정 §7(자동 재시도를 더하지 않음)에 맞다.
- **크기 초과의 라이브러리 상태는 `OUT_OF_RANGE`**다(tonic 0.14). 계약과 클라이언트는 `RESOURCE_EXHAUSTED`와 같이 결과 불명으로 다룬다.
- **deadline 시각에 tonic은 `CANCELLED`**로 답한다. 그래서 처리 기한을 50ms 앞당겨 서버의 `DEADLINE_EXCEEDED`가 먼저 나가게 했다.
- **`statement_timeout`은 트랜잭션 시작 때 한 번** 정한다. 문장 사이의 초과는 커밋 전 확인이 잡는다.
- **“마지막 문장과 커밋 사이에 deadline이 지나는” 경로**는 gRPC로 결정적으로 일으킬 수 없어, 가드를 넣은 `dispatch` 시험으로 확인했다.
- **처리의 500 `INTERNAL`**은 장애 주입 없이 일으킬 수 없어 동등 시나리오에 없다. DB 연결 실패의 `UNAVAILABLE`은 시험했다.
- **supervisor gRPC 클라이언트는 체크포인트 RPC 둘만** 쓴다. 지금 supervisor가 부르는 것이 그 둘이다. 보고·재현 요청의 supervisor 클라이언트는 02-05(프로세스 분리)에서 필요해지면 더한다. 코디네이터는 RPC 5개를 모두 제공한다.
- **헬스**: 작업자가 모두 바쁘면 헬스 읽기가 시간 초과로 `NOT_SERVING`이 될 수 있다(계약 §7.3).
- **02-05는 막지 않는다.** HTTP는 그대로이고 gRPC는 선택이다. 02-05의 선행·수용 기준은 바뀌지 않는다.
- **X-05·X-06(kind 분산·제품 경로 재검증)**은 이 작업 범위가 아니다.
