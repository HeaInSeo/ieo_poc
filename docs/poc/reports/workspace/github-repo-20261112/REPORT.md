# GitHub 원격 저장소 생성·연결 · github-repo-20261112

2026-11-12 KST · 사용자 결정(2026-11-12 대화): `HeaInSeo/ieo_poc` 비공개 생성·로컬 연결·기존 main 이력 보존·이후 1.9절 규칙 적용. **작업 완료 수에 넣지 않는 환경 준비 기록.**

| 항목 | 실측 |
| --- | --- |
| 생성 | `gh repo create HeaInSeo/ieo_poc --private --source . --remote origin` → https://github.com/HeaInSeo/ieo_poc · PRIVATE · viewerPermission ADMIN |
| 이력 보존 | `git push -u origin main` → 원격 `refs/heads/main` = `0858fb0b52a38930d1f64377b396e3748cfa5b3e` (로컬 두 커밋 d92b414 → 0858fb0 그대로) |
| 병합 방식 | merge commit만 허용, squash/rebase 금지, 병합 후 브랜치 삭제 ([설정 후](evidence/repo-settings-after.json)). 승인된 head SHA가 그대로 main 이력에 남게 한다 |
| Actions | 활성, 모든 action 허용 ([actions-permissions.json](evidence/actions-permissions.json)) |
| 브랜치 보호·rulesets | **사용 불가**: HTTP 403 “Upgrade to GitHub Pro or make this repository public” ([protection](evidence/branch-protection-get.txt) · [rulesets](evidence/rulesets-get.txt)) |

## 1.9절 규칙 적용 방식과 한계

- 작업 하나에 PR 하나, 열린 수정 PR 하나. 독립 검토는 별도 검토 세션이 PR의 **head SHA**를 대상으로 수행하고 결과를 PR 댓글(대상 SHA 명시)로 남긴다.
- 병합은 `PUT /repos/{owner}/{repo}/pulls/{n}/merge`의 `sha`=승인 head SHA 조건부 병합으로만 한다. 불일치 시 병합하지 않는다.
- **한계**: 비공개+Free 계정이라 필수 체크·승인 강제·직접 push 차단을 서버가 강제하지 못한다. 현재 CI 성공 확인은 병합 전 절차(해당 head SHA의 check-runs 대조)로 수행하며, 서버 강제가 아니다. GitHub 계정이 하나라 독립 검토자는 GitHub의 Approve를 제출할 수 없다(자기 PR 승인 불가) → 댓글+원장 기록으로 대체한다.
- 진행 원장(state.json)·생성 화면·검토 기록 커밋은 1.9 수정 PR과 분리한 보고 커밋으로 병합 후 main에 직접 반영한다(DEV_AGENT_HANDOFF §6: 보고 갱신으로 검토 중 head를 바꾸지 않음).
- 이 한계는 05-01~05-03(P23 보호 설정 검증)의 착수 전 사용자 결정 대상이다: GitHub Pro/Team 업그레이드, 공개 전환, 또는 별도 시험 repo/조직. 01 스프린트 진행을 막지는 않는다.
