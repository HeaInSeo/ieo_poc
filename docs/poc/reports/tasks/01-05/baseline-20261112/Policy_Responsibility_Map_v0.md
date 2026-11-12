# 01-05 최소 정책·책임 대응표 v0

계획 v1.3.3-A1 · 01-05 run `baseline-20261112` · NPG 고정 커밋 `e5075aff`([Source_Baseline.md](Source_Baseline.md)) · 조항 위치 조사 원본 [evidence/npg-clauses-e5075aff.md](evidence/npg-clauses-e5075aff.md)

## 읽는 법

- **행**은 계획 §1.1 처분표의 15개 책임과 P21 표의 두 원본 영역(역할 지침, 공통 권한 원칙)을 합친 17개다. 처분과 새 책임 위치는 계획을 그대로 옮겼고, 이 문서가 새로 정한 것은 없다.
- **NPG 조항**은 고정 커밋의 `파일:줄`이다. (remote)는 패키지에 사본이 없어 고정 커밋에서 직접 읽은 파일이다. 조항 원문 인용은 조사 원본에 있다.
- **검증 범위**는 그 책임을 시험하는 P 실험이다(계획 §1.1).
- **독립 기대값**: 01-05는 기대값을 정하지 않는다. 각 P의 정상·위반·경계 기대값은 동결 대장 L03에 따라 개발 맥락이 없는 독립 담당이 그 실험의 첫 실행 전에 고정한다. 표에는 그 연결과, NPG에 이미 있는 **역사적 기대값**(NPG 구현을 위한 원본 시험이며, 새 코디네이터의 기대값이 아니다)만 적는다.
- **공백**: 원본에 조항이 없거나, 조항이 이름보다 좁은 경우다. 공백은 고정 원본에서 옮길 근거가 없다는 뜻이다. 새 기준은 독립 기대값으로 따로 정해야 한다.

## 대응표

| # | 기존 책임 | 처분(계획 §1.1) | NPG 조항 @e5075aff | 새 책임 위치 | 검증 범위 | 독립 기대값 | 공백·주의 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Notion 본문 쓰기의 사전 이미지·readback·중단 | 폐기 | README.md:271-291 | 없음. 실행 상태는 DB에 둔다 | P04·P21(Notion 쓰기 0건) | L03(P21 첫 실행 전). 기대: Notion 쓰기 자격·호출 0 | 원본은 원자 CAS가 아니라 검사·탐지 장벽이다(README.md:291) |
| 2 | Notion 사건별 reconcile 헬퍼 | 형태 변경 | README.md:52-54, :477. templates/central-supervisor.md:45 (remote) | 외부 요청 ID와 수신기 조회·중복 억제 계약(01-08) | P06·P13·P27 | L03 | 원본 헬퍼는 사건 하나에 묶이고 기본은 검사 전용이다. 이식하지 않는다 |
| 3 | 결과 미확인 쓰기의 무근거 재전송 금지 | 보존 | README.md:289, :362-363. AUTOMATIC_FOLLOWTHROUGH.md:159. OPERATIONS.md:144-145. central-supervisor.md:29-30 (remote) | 코디네이터의 요청별 결과 대조, 의존 범위 보류 | P06·P13 | L03. 역사적: NPG CI의 미확정 쓰기 시험(ci.yml:27-28 묶음) | — |
| 4 | 소유권·lease·이전 실행자 차단 | 보존 | central-supervisor.md:24-27 (remote). AUTOMATIC_FOLLOWTHROUGH.md:60-62. broker.cjs:455-458 (remote) | DB의 실행 인스턴스·권한 세대(01-08 배정 세대, L08) | P04·P05·P27 | L03. 역사적: `test_fenced_dispatch.cjs` | lease(최대 5분)는 미배치 review-timeout 원장의 조항이다(REVIEW_TIMEOUT_OPERATIONS.md:12-13 remote) |
| 5 | 요청 전달과 실제 인수의 구분 | 보존 | AUTOMATIC_FOLLOWTHROUGH.md:295-296. SOURCE_REVIEW_BOOTSTRAP.md:42. central-supervisor.md:71-72 (remote). README.md:213-215 | 전달 영수증과 실제 인수 기록 분리(01-08 claim, 02-05) | P03·P21 | L03 | — |
| 6 | 지정한 PR·head/base/tree만 검토 | 보존 | SOURCE_REVIEW_BOOTSTRAP.md:22-23. source_review_request.cjs:12-14, :30 (remote) | 후보 SHA와 검토·검사 증거 연결(01-08 ASSIGNMENT_SHA_MISMATCH, 02-06) | P07·P21·P23 | L03 | **이름보다 좁다**: 원본은 NPG 저장소와 Guardrail 작업 하나에 하드코딩됐다(source_review_request.cjs:5-6). 일반 제품 PR 규칙의 근거로 쓰지 않는다 |
| 7 | 출처 확인과 불변 설치 | 보존 | README.md:329-336. AUTOMATIC_FOLLOWTHROUGH.md:190-193. CENTRAL_SCHEDULER_MIGRATION.md:47. OPERATIONS.md:39-40. ci.yml:37-45 | Git SHA·이미지 digest·승인 설정 연결 | P08·P18·P19 | L03. 역사적: `test_provenance.py`, `verify_provenance.py` | — |
| 8 | 저장소별 독립 진행 | 보존 | central-supervisor.md:31, :34-36 (remote). README.md:197, :201-203 | 실제 공유 상태와 의존 조건에만 보류 전파 | P04·P06·P26 | L03 | 원본에도 전역 canonical 장벽이 남아 있다(README.md:202-203). 새 구조의 공유 상태 정의는 독립 기대값으로 정한다 |
| 9 | 활성 수정 PR 하나 | 보존 | AGENTS.md:5 | repo당 열린 수정 PR 하나·작업 하나에 PR 하나(PoC 기본 프로파일) | P21 | L03 | **이름보다 좁다**: AGENTS.md:5는 NPG 저장소 자신에게만 적용된다. worker repo의 단일 PR 게이트는 정의 없이 “유지”만 적혀 있다(AUTOMATIC_FOLLOWTHROUGH.md:287-289) |
| 10 | 독립 검토·CI 이후 병합과 main CI 확인 | 보존·보완 | CENTRAL_SCHEDULER_MIGRATION.md:41-43, :58-59. AUTOMATIC_FOLLOWTHROUGH.md:211, :218-219, :276-277. OPERATIONS.md:64-65. AGENTS.md:11 | 코드 검증·병합·병합 커밋 main CI·산출물·인수를 각각 확인(02-06, 02-07) | P07·P14·P23·P26·P28 | L03 | NPG 코드는 병합하지 않고 병합 사실만 확인한다. main CI 조항은 PR25 설치 관문이다. “보완”(병합 실행·인수) 부분은 원본 근거가 없는 새 기준이다 |
| 11 | 완료·RELEASE 뒤 직접 후속 전부의 처분과 이월 | 보존·보완 | central-supervisor.md:65-66, :72-73 (remote). README.md:206-234. AUTOMATIC_FOLLOWTHROUGH.md:270-271 | 승인 스프린트 DAG의 후속 처분(02-08) | P28 | L03. 역사적: `supervision_gate_cases.cjs` | 원본도 “생략된 DAG 후속을 찾지 못한다”는 한계를 적는다(README.md:233-234). “보완”이 이 공백을 다룬다 |
| 12 | 검토 timeout·대체 한도·늦은 결과 연결 | 보존·보완 | OPERATIONS.md:126-162. REVIEW_TIMEOUT_OPERATIONS.md:165-168 (remote) | 작업 ID·후보 SHA·배정 세대별 timeout·대체 정책(02-06, 01-08 늦은 결과 보존) | P07 | L03. 시험 프로파일 제안값(30분 경고·60분 초과·대체 1회)은 T01에서 확정. 역사적: `review_timeout_policy_cases.cjs`·ledger·operational cases | **원본에서 미채택**: OPERATIONS.md:126 “(미채택 정책)”, deployment/review-timeout-operational.json:3 `OPERATIONAL_CANDIDATE_NOT_DEPLOYED`(remote). 운영 근거가 아니라 후보 설계 근거다 |
| 13 | 사용량 게이트와 해소 후 재기동 | 형태 변경 | SOURCE_REVIEW_BOOTSTRAP.md:33, :35-36. AUTOMATIC_FOLLOWTHROUGH.md:287-289 | 런타임 어댑터 한도 분류, 재개 스케줄러(런타임 계약 §7, F06) | P05·1.2절 | L03. F06 재개 간격(공급자 신호 우선, 30분·30분·최대 10회)은 동결값 | **공백(해소 후 재기동)**: 원본은 기존 게이트에 맡기고 바꾸지 않는다(README.md:268-269). 재기동 규칙의 원본 근거가 없다. F06과 계약 §7이 새 기준이다 |
| 14 | 읽기 전용 큐 감시와 코디네이터 정지 알림 | 형태 변경 | templates/queue-watch.md:1, :7-8, :16-17 (remote). AUTOMATIC_FOLLOWTHROUGH.md:200-201 | 별도 수명·권한의 외부 감시기(02-08) | P28 | L03(감지 기한·재조회 간격은 T01·T05) | **부분 공백(정지 알림)**: 코디네이터 자체가 멈췄을 때 알리는 조항과 알림 채널이 없다. REVIEW_TIMEOUT_TRANSITION.md:97(remote) “no scheduler or notification is provided” |
| 15 | 사람이 보는 큐·허브·대시보드 | 형태 변경 | README.md:164-165. AUTOMATIC_FOLLOWTHROUGH.md:228-229 | DB 읽기 전용 현황 조회와 보고 수신기(01-04 E3, 02-07) | P12·P13·P26 | L03(P12는 L09) | **이름보다 좁다**: 원본에서 이것들은 NPG가 쓰는 Notion 대상일 뿐, 사람용 내용 정의가 없다. 현황 내용은 계획 1.5절과 P12 기준(L09·L03)으로 새로 정한다 |
| 16 | 역할별 지침·템플릿 | 형태 변경(P21: 버전 고정 역할·repo 지침) | AGENTS.md:13. central-supervisor.md:17-18 (remote). OPERATIONS.md:92-98. source_review_request.cjs:37-42 (remote) | 버전이 고정된 역할·repo 지침(03-02, 런타임 계약 §4.1 RO 설정) | P21·P25 | L03 | **공백**: Operator·Guardrail worker 역할 지침 파일이 저장소에 없다. 실제 프롬프트는 비공개다(OPERATIONS.md:14). 템플릿은 중앙 감독·큐 감시 둘뿐이다 |
| 17 | 브로커의 공통 권한 원칙 | 형태 변경(P21: GitHub/수신기 외부 쓰기 허용 검사) | broker.cjs:224-229, :242-244, :355-357 (remote). policy.json:3-35 (remote). README.md:280, :319-325, :339-342. OPERATIONS.md:213-216 | GitHub·수신기 외부 쓰기 허용 검사(01-08, 05-03) | P21 | L03. 대상 API가 바뀐 쓰기 정책은 독립 기대값으로 재검증(P21) | **이름보다 좁다**: 허용 목록은 Notion 본문 편집만 다룬다. GitHub 등 일반 외부 쓰기 허용 목록은 원본에 없다. 고정 문서 작성 시점에 배치된 정책은 비공개였고 저장소 사본과 달랐다(OPERATIONS.md:29-31, 그 시점 서술. 현재 배치 상태는 조회하지 않음) |

## 집계

| 구분 | 행 |
| --- | --- |
| 원본 조항이 책임을 직접 뒷받침 | 1·2·3·4·5·7·8·10·11·12 (10행). 다만 12는 미채택 후보, 10·11은 “보완” 부분이 새 기준 |
| 조항이 있으나 이름보다 좁음 | 6·9·15·17 (4행) |
| 공백·부분 공백 | 13·14·16 (3행) |

- 공백과 좁은 행은 01-05의 결함이 아니다. 고정 원본이 말하지 않는 범위를 드러낸 것이다.
- 이 행들은 P21·P28·P05·P07의 독립 기대값을 정할 때(L03) 원본 근거 없이 새로 정해야 하는 항목이다.
- 해당 실험 보고서는 이 표의 행 번호와 이 문서의 버전(v0, 이 커밋)을 인용한다.
