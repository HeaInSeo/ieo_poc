# 01-05 고정 원본·기준선 목록 v0

계획 v1.3.3-A1 · 01-05 run `baseline-20261112` · 조회 원본: [fixed-source-compare.txt](evidence/fixed-source-compare.txt)

**고정 원본은 “그 시점에 그 커밋에 있었던 내용”의 증거다.** 현재 설치 버전, 소유권, 미확정 외부 요청, 자동화의 정상 동작을 증명하지 않는다. 현재 운영은 [Current_Operations_20261112.md](Current_Operations_20261112.md)에서 따로 확인했다. 실제로 두 상태는 다르다. 이 서버의 NPG 작업 사본은 고정 커밋을 조상으로 갖지 않는다.

## 1. NPG 고정 커밋

| 항목 | 값 | 확인 |
| --- | --- | --- |
| 저장소 | `HeaInSeo/notion-projection-guard`(public) | GitHub API |
| 고정 커밋 | `e5075affbc87ed650a755558b8491828e6783862`. committer 2026-11-02T15:11:35Z. “fix: validate migrated central execution without changing incident ownership (#25)” | GitHub API. 계획 §1.1과 P21이 이 커밋으로 고정했다 |
| 고정 커밋 파일 수 | blob 143개 | git tree API |
| 원격 `main`(조회 시점) | `e5075aff…`와 같다 | GitHub API. 원격 main이 같다는 사실도 설치 증거가 아니다 |

## 2. 패키지 사본과 고정 커밋의 대조

`docs/poc/05_npg_evidence/`의 사본을 고정 커밋의 blob과 sha256 전체로 비교했다. 10개 모두 같다.

| 패키지 사본 | 고정 커밋 경로 | sha256(앞 16자) | 대조 |
| --- | --- | --- | --- |
| repo_documents/repo/AGENTS.md | AGENTS.md | `520a26595d8d2008` | 같음 |
| …/AUTOMATIC_FOLLOWTHROUGH.md | 같은 이름 | `299841daf91e6783` | 같음 |
| …/CENTRAL_SCHEDULER_MIGRATION.md | 같은 이름 | `8e30ff126a01e2c8` | 같음 |
| …/OPERATIONS.md | 같은 이름 | `309f1bc299c0c474` | 같음 |
| …/README.md | 같은 이름 | `60d88dfbc5cd8633` | 같음 |
| …/REVIEW_TIMEOUT_VERIFICATION.md | 같은 이름 | `4f74d7523b88cb15` | 같음 |
| …/SOURCE_REVIEW_BOOTSTRAP.md | 같은 이름 | `6cd56399f2910720` | 같음 |
| …/VERIFICATION_20261012.md | 같은 이름 | `12b12abc32ba342a` | 같음 |
| …/VERIFICATION_20261020_OPERATIONS.md | 같은 이름 | `f882ea84eabdbcaa` | 같음 |
| ci.yml | .github/workflows/ci.yml | `4895d8afb58db824` | 같음 |

그 밖의 파일(policy.json, broker.cjs, source_review_request.cjs, templates/* 등)은 패키지에 사본이 없다. 조항 대응표는 이 파일들을 고정 커밋에서 직접 읽어 인용했다. 표에서 (remote)로 표시했다.

## 3. 기존 CI 기준선

| 항목 | 값 | 사용 범위 |
| --- | --- | --- |
| 실행 | run `41469168311` “Notion projection guard CI”. push, main, head `e5075aff`, attempt 1, 2026-11-02T15:11:40Z, `completed/success`. 패키지 CI_Run.json과 API 재조회가 같다 | P21 “기준선 증거를 먼저 확인”의 역사적 기준선 |
| job | Node 22, Node 24 “tests and source provenance” 모두 success. 원본 로그는 Node22_Job.log(`ead9c8e0800d28a6…`)·Node24_Job.log(`5c8c4482bc1e7106…`) | 같다 |
| 시험 수 | job마다 node 1087+511+134=1732, python 17. 두 job은 같은 시험의 반복이므로 3464건으로 세지 않는다([CI_Evidence_Scope.md](../../../../05_npg_evidence/CI_Evidence_Scope.md)) | 같다 |
| 워크플로 | ci.yml:3-7(push main·`migration/**`·`fix/**`, PR, dispatch), :13-19(ubuntu-24.04, node [22,24], fail-fast false), :27-36(`node --test` 세 묶음과 검증기 둘), :37-45(provenance) | 같다 |

- **이 CI 성공이 뜻하지 않는 것**:
  - 현재 서버에 설치된 버전
  - 소유권
  - 미확정 외부 요청
  - 중앙 자동화의 정상 동작
  - 새 코디네이터 P21·P23의 통과
- 실패·건너뜀이 없으므로 그 이유로 제외할 규칙도 없다.
- 도구·입력·원본에 차이가 있거나 회귀가 우려되면 격리 재실행을 선택한다(P21). 이번 run은 재실행하지 않았다. 역사적 기준선을 미실행으로 덮어쓰지 않는다.

## 4. 설계 원본

| 원본 | 버전 | 쓰는 곳 |
| --- | --- | --- |
| [NPG_Design_Operations_Guide.docx](../../../../05_npg_evidence/NPG_Design_Operations_Guide.docx) | sha256 `e7a1dba44aa2b908…`(패키지) | 문서 내부 날짜·기준 소스를 따라 읽는다. 현재 운영 실측이 아니다 |
| [Console_Automation_Design_Log.md](../../../../02_design_sources/Console_Automation_Design_Log.md) | sha256 `0719b992ccc10d75…` | 설계 로그 POC-01~05(EARLY-01) |
| [Console_Coordinator_Architecture.png](../../../../02_design_sources/Console_Coordinator_Architecture.png) | sha256 `c8c2caa949fd0397…` | 구조도 |
| 계획 [§1.1 처분표](../../../../Console_Coordinator_PoC_Validation_Plan.md)·P21 표 | 계획 v1.3.3(A1) | 책임별 처분(폐기·형태 변경·보존·보존·보완)의 정본. 대응표 v0의 행 구성 |

패키지 원본 전체의 무결성은 `docs/poc/SHA256SUMS.txt`가 따로 고정한다. 이 문서는 고정 커밋과의 대조를 더한 것이다.
