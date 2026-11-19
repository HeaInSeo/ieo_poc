# 실험·후속 후보 결과

계획 v1.3.3 · snapshot `08-04-acceptance-20261119` · 원장 SHA-256 `698811a23d7467f629acada38b42eebf9dd555bb4d448037758236e89d328b29`

자동 생성 화면 · 원본은 state.json · 손편집 금지

## 주계획 실험

### P01 · 요구사항·설계 오류 발견

연결 작업: ["04-01"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P02 · 문서 버전 적용과 전달 누락

연결 작업: ["04-02"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P03 · 실제 개발 에이전트와 외부 DB 연결

연결 작업: ["04-03"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P04 · 트랜잭션, 재시도, repo 간 동시성

연결 작업: ["04-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P05 · PID 1과 에이전트 장애 복구

연결 작업: ["04-05","04-15"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P06 · 외부 요청 결과 유실과 격리된 자동 재개

연결 작업: ["04-06"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P07 · 현재 커밋의 필수 CI와 독립 적대적 검토

연결 작업: ["04-07"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P08 · 이미지와 품질 프로파일의 재현성

연결 작업: ["03-07","05-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P09 · eBPF 관측과 출처 확인

연결 작업: ["08-03"] · 실행 격리 합성 회차 3건 기록 · 판정 PASS · 평가 종료 True · 처분 미판정

최근 보고: [보고](../reports/tasks/08-03/p09-observation-20261119/REPORT.md)

#### 회차 P09-08-03-44514382183 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P09-08-03-44514382183 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | FAIL |
| candidate_sha | 28fb685e18587e023b4c0e3e8b19fe117882f142 |
| verification_scope | 초기 판정 실패 |
| latest_report | [보고](../reports/tasks/08-03/p09-observation-20261119/REPORT.md) |
| evidence | ["https://github.com/HeaInSeo/ieo_poc/actions/runs/44514382183","reports/tasks/08-03/p09-observation-20261119/evidence/44514382183-fail.tar.gz"] |

#### 회차 P09-08-03-44515801821 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P09-08-03-44515801821 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | PASS |
| candidate_sha | f7c7766638c39276d28907d3dc327c4443438261 |
| verification_scope | 18/18 · 독립 검토 반려 |
| latest_report | [보고](../reports/tasks/08-03/p09-observation-20261119/REPORT.md) |
| evidence | ["https://github.com/HeaInSeo/ieo_poc/actions/runs/44515801821","reports/tasks/08-03/p09-observation-20261119/evidence/44515801821-pass-intermediate.tar.gz"] |

#### 회차 P09-08-03-44516175173 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P09-08-03-44516175173 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | PASS |
| candidate_sha | 0d23d73d73e1237de86657eb64dad01b5145fbd4 |
| verification_scope | 18/18 · 독립 검토 승인 |
| latest_report | [보고](../reports/tasks/08-03/p09-observation-20261119/REPORT.md) |
| evidence | ["https://github.com/HeaInSeo/ieo_poc/actions/runs/44516175173","reports/tasks/08-03/p09-observation-20261119/evidence/44516175173-pass.tar.gz"] |

### P10 · 보안 위반 탐지와 실제 차단

연결 작업: ["08-04"] · 실행 격리 합성 회차 2건 기록 · 판정 PASS · 평가 종료 True · 처분 미판정

최근 보고: [보고](../reports/tasks/08-04/p10-defensive-20261119/REPORT.md)

#### 회차 P10-08-04-44519648688 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P10-08-04-44519648688 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | FAIL |
| candidate_sha | 955244841295d515a49adc2d1252d45f03ac2130 |
| verification_scope | 정책 비교에서 중단. 시도 0건. |
| latest_report | [보고](../reports/tasks/08-04/p10-defensive-20261119/REPORT.md) |
| evidence | ["https://github.com/HeaInSeo/ieo_poc/actions/runs/44519648688","reports/tasks/08-04/p10-defensive-20261119/evidence/44519648688-evidence.tar.gz"] |

#### 회차 P10-08-04-44520476538 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P10-08-04-44520476538 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | PASS |
| candidate_sha | b0ddefaf7882118a1fa1e69e22d38b2a46fb3c12 |
| verification_scope | 7/7 · 격리 합성 TCP 탐지·차단. |
| latest_report | [보고](../reports/tasks/08-04/p10-defensive-20261119/REPORT.md) |
| evidence | ["https://github.com/HeaInSeo/ieo_poc/actions/runs/44520476538","reports/tasks/08-04/p10-defensive-20261119/evidence/44520476538-evidence.tar.gz"] |

### P11 · 선언 정책과 애플리케이션 권한 경계

연결 작업: ["08-05"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P12 · 사용자 과업과 보고 이해도

연결 작업: ["04-09","07-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P13 · 완료 판정과 보고 전달

연결 작업: ["04-08"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P14 · 종단 실증

연결 작업: ["08-06"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P16 · eBPF의 추가 발견과 Aya 구현 가치

연결 작업: ["08-08"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P17 · 도그푸딩으로 코디네이터의 다음 변경 구현

연결 작업: ["06-01"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P18 · 자기 변경의 권한 분리와 복구

연결 작업: ["08-01"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P19 · GHCR 게시, 고정 digest 배포, 승격

연결 작업: ["06-02","08-07"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P20 · 신규 저장소 첫 셋업과 첫 PR에서 공통 품질 기준 적용

연결 작업: ["06-03","06-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P21 · NPG 책임·정책의 이전 형태와 동작 보존

연결 작업: ["04-10"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P22 · 공통 개발·검증 이미지의 구조와 재현성

연결 작업: ["03-06","03-08"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P23 · GitHub Actions의 거짓 성공 방지

연결 작업: ["05-01","05-02","05-03","06-03","06-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P24 · 내장 정책으로 신규 저장소 설정·검증·수정 반복

연결 작업: ["06-05","06-06"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P25 · 초기 정책 문서·카탈로그의 충실도와 정확성

연결 작업: ["03-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P26 · 실제 백로그의 산출과 자동화 충분성

연결 작업: ["07-01","07-02","07-03"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P27 · NPG 전환·책임 종료·복귀

연결 작업: ["07-06","07-07"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P28 · 직접 후속의 처분·이월과 코디네이터 정지 감지

연결 작업: ["04-11","04-12","07-05"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P33 · 결과 처분·코드 재사용·최종 스프린트 연결 검증

연결 작업: ["09-02","09-05"] · 실행 합성 회차 4건 기록 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: [보고](../reports/tasks/09-02/submission-review-20261119/REPORT.md)

#### 회차 P33-09-02-SYNTHETIC-01 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P33-09-02-SYNTHETIC-01 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | FAIL |
| checks | 28/32 |
| latest_report | [보고](../reports/tasks/09-02/p33-synthetic/REPORT.md) |

#### 회차 P33-09-02-SYNTHETIC-02 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P33-09-02-SYNTHETIC-02 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | PASS |
| checks | 32/32 |
| candidate_sha | 043546583aa46bf038f51122fc5dbe166d4bbc97 |
| base_recheck | 28/32 |
| prior_run_id | P33-09-02-SYNTHETIC-01 |
| latest_report | [보고](../reports/tasks/09-02/independent-retest/REPORT.md) |

#### 회차 P33-09-02-SYNTHETIC-03 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P33-09-02-SYNTHETIC-03 |
| revision | 1 |
| execution_kind | synthetic |
| verdict | FAIL |
| checks | 32/34 |
| latest_report | [보고](../reports/tasks/09-02/remaining-synthetic/REPORT.md) |

#### 회차 P33-09-02-SYNTHETIC-04 / revision 1

| 필드 | 실제 기록 |
| --- | --- |
| run_id | P33-09-02-SYNTHETIC-04 |
| revision | 1 |
| execution_kind | synthetic |
| observed_at | 2026-11-19T00:02:46+09:00 |
| verdict | PASS |
| checks | 34/34 |
| candidate_sha | 4f0468f9afde904791c3af32ffc7dcd008d77e1c |
| prior_run_id | P33-09-02-SYNTHETIC-03 |
| verification_scope | 교정 담당의 동결 재실행. 해당 SHA의 독립 코드 승인은 중앙 인계로 확인. |
| latest_report | [보고](../reports/tasks/09-02/submission-review-20261119/REPORT.md) |
| evidence | ["reports/tasks/09-02/submission-review-20261119/exact-4f-frozen.txt","reports/tasks/09-02/remaining-correction-20261119/REPORT.md"] |

### P34 · repo 수명 동안 에이전트 지속성

연결 작업: ["04-13"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P35 · 자식 에이전트의 역할·보고·복원 경계

연결 작업: ["04-14"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P36 · 표준 형식의 작성과 결정 지원

연결 작업: ["04-16","07-04"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

## 후속 후보

### FX-GRAPHIFY · Graphify 코드 전용 그래프 효과 비교

연결 작업: ["F01","F02"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P32 · SkillOps 계약·관계·유지보수

연결 작업: ["F04","F05"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P31 · SkillOpt 제한 수정·독립 평가

연결 작업: ["F06","F07"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P29 · Harness² 후보 비교·조합

연결 작업: ["F08","F09"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

### P30 · Trace2Skill 실제 기록·재사용 스킬

연결 작업: ["F10","F11"] · 실행 미실행 · 판정 미판정 · 평가 종료 False · 처분 미판정

최근 보고: 없음 · 미실행

회차 없음 · 미실행

[현황](STATUS.md) · [작업 상태](TASKS.md)
