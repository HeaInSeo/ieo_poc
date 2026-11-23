# 02-06 수용 기록

| 항목 | 증거 |
| --- | --- |
| 기준 교정 이력 | `884c641` 반려 후 `8bd2498a1f9104339b1017c7c66cdc699d0692a5` 독립 반려. 신규 Git 객체 전송과 PR당 대체 1회는 확인했고 병합 SHA 증명과 영수증 귀속을 남겼다. |
| 최종 독립 검토 | Mencius, `926d1fba54d74fca3f72c91287f87b160f7f4bdc` APPROVE. 마지막 교정·독립 검토 각 1회. |
| 격리 시험 | 관련 통합 104/104(검토 생명주기 5/5 포함), 교정별 회귀 검출 2/2, Clippy 통과. |
| PR | [#121](https://github.com/HeaInSeo/ieo_poc/pull/121), [정확한 승인 SHA의 CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45164938977) |
| 병합 | `9dc4f517ae8bba2fb50ea4669db597c0c4565a95`, [동일 SHA main CI 4/4](https://github.com/HeaInSeo/ieo_poc/actions/runs/45166161403) |

## 확인 범위

승인·배정·권한·검토 상태를 영속 원장과 연결했다. 로컬 bare 원격에 없던 교정 커밋을 전송하고 응답 유실 뒤 원 요청을 대조했다. 병합 기록은 승인 SHA가 두 번째 부모인 병합 커밋과 PR·main 이력을 대조했다. 증거가 없거나 다르면 unknown을 유지해 후속 쓰기를 차단한다. 새 키에서 원 주체·배정 ID·세대가 각각 다르면 거부하며, 같은 후보의 일치하는 요청은 저장된 결과를 반환한다. timeout 자동 대체는 PR당 1회이며 새 head의 정상 검토는 배정된다.

## 이월

실제 GitHub 권한 회수·head 경쟁·역할 격리는 04-07/P07, 보호 규칙·App 설정은 05-03/P23에서 검증한다. 별도 Claude → Codex 실측은 조건 대기다. 다음 본계획 작업은 02-07이다.
