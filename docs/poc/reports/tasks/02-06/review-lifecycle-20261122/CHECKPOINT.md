# 02-06 검수 체크포인트

| 항목 | 증거 |
| --- | --- |
| 기준 교정 이력 | `884c641` 반려 후 `8bd2498a1f9104339b1017c7c66cdc699d0692a5` 독립 반려. 신규 Git 객체 전송과 PR당 대체 1회는 확인했고 병합 SHA 증명과 영수증 귀속을 남겼다. |
| 최종 독립 검토 | Mencius, `926d1fba54d74fca3f72c91287f87b160f7f4bdc` APPROVE. 마지막 교정·독립 검토 각 1회. |
| 격리 시험 | 관련 통합 104/104(검토 생명주기 5/5 포함), 교정별 회귀 검출 2/2, Clippy 통과. |
| PR | [#121](https://github.com/HeaInSeo/ieo_poc/pull/121), [정확한 승인 SHA의 CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45164938977) |
| 병합 | `9dc4f517ae8bba2fb50ea4669db597c0c4565a95`, [동일 SHA main CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45166161403) |

## 확인 범위

승인·배정·권한·검토 상태를 영속 원장과 연결했다. 로컬 bare 원격에 없던 교정 커밋을 전송하고 응답 유실 뒤 원 요청을 대조했다. 병합 기록은 승인 SHA가 두 번째 부모인 병합 커밋과 PR·main 이력을 대조했다. 증거가 없거나 다르면 unknown을 유지해 후속 쓰기를 차단한다. 새 키에서 원 주체·배정 ID·세대가 각각 다르면 거부하며, 같은 후보의 일치하는 요청은 저장된 결과를 반환한다. timeout 자동 대체는 PR당 1회이며 새 head의 정상 검토는 배정된다.

## 전체 수용 차단

원장 초안 `b654f83`에 대한 독립 검토는 반려였다. 기존 계획 §구현 체크포인트가 요구하는 재현 요청 실행기와 영속 상태 전이가 남아 있다. 현재 `repro_requests` API에는 `queued` 등록·조회가 있다. 별도 후속 담당으로 옮기는 승인 근거는 없다. 이 항목의 구현·격리 시험·독립 검토 또는 명시적 이월 결정이 있어야 전체 수용을 다시 판정한다.

02-07 로컬 작업공간 `/tmp/ieo-0207-judgment-outbox`, 브랜치 `task/0207-judgment-outbox`, 커밋 `2becce0e71f1f237395fb1190ce3d915dad305ea`는 착수 계획과 F3 재현·초기 교정을 보존한다. 교정 전 `NO_CURRENT_ASSIGNMENT`를 재현했고 교정 후 코디네이터 통합 시험 83/83이 통과했다. 공식 착수는 02-06 수용까지 보류한다.

## 이월

실제 GitHub 권한 회수·head 경쟁·역할 격리는 04-07/P07, 보호 규칙·App 설정은 05-03/P23에서 검증한다. 별도 Claude → Codex 실측은 조건 대기다. 02-06 수용 뒤 다음 본계획 작업은 02-07이다.

## 최종 수용

[재현 실행기와 02-06.accept 전체 대조](REPRO_RUNNER.md)의 구현을 `022bd4164f993cd29fe7ea15b78d11667f50bdb5`에서 독립 승인했다. 기존 후보의 회귀 166건과 최종 SHA의 표적 시험 35건에 실제 로컬 Podman 일회성 실행, 준비·명령 실패, DB 결과 반영 실패 복원, supervisor 재시작 후 GET 대조, 같은 SHA의 N5 갱신 경쟁에서 병합 전송 0건, rootless 확인 실패 시 시작 차단, 기본 60초와 301·3,600초의 실효 설정 확인이 포함된다.

[PR #123](https://github.com/HeaInSeo/ieo_poc/pull/123) · [PR CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45377454250) · 병합 `744e0a2f93ca197a81c03195f2482c0dafb4c480` · [동일 SHA main CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45378776886)

실제 권한 회수·head 경쟁·역할 Pod 격리는 04-07/P07, 보호 규칙·App 설정은 05-03/P23, N5 실제 수집과 F4는 1b, 외부 상태 표시 문구는 02-07이다. 별도 Claude → Codex 전환 시험은 조건 대기를 유지한다.

## 최종 수용 근거 보강

#122 초안의 독립 검토에서 확인한 supervisor 로그 경로와 설정 이미지 교체 복구를 교정했다. 로컬 두 이미지와 격리 DB에서 응답의 stdout·stderr URL을 그대로 조회했다. 로그를 같은 repo·Guardrail 권한으로 제한했다. 이미지 A 실행의 결과 저장을 실패시킨 다음 B 설정에서 원 run_key와 A 이미지 digest로 복구했고, 저장은 한 건으로 유지됐다. 별도 격리 시험에서 이미지 A의 종료 컨테이너와 예약 행을 준비하고 B 설정의 실제 worker 프로세스를 두 번 새로 기동해 같은 A 결과와 양쪽 로그의 저장 한 건을 확인했다.

[PR #124](https://github.com/HeaInSeo/ieo_poc/pull/124)의 `d6c97eaf43954b2075b3e60ee3b930755d7e0d5c` 독립 승인 · [필수 CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45382943599) · 병합 `f8022573631c478ef6dc845d64dc0d82dee8f027` · [동일 SHA main CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45384251719)
