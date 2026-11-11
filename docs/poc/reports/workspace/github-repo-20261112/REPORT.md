# GitHub 원격 저장소 생성·연결 · github-repo-20261112

2026-11-12 KST · **작업 완료 수에 넣지 않는 환경 준비 기록.**
사용자 결정(2026-11-12 대화): ① `HeaInSeo/ieo_poc` 비공개 생성·로컬 연결·기존 main 이력 보존·이후 1.9절 규칙 적용 → ② 같은 날 후속 지시로 **공개(public)로 변경**.

| 항목 | 실측 |
| --- | --- |
| 생성 | `gh repo create HeaInSeo/ieo_poc --private --source . --remote origin` → https://github.com/HeaInSeo/ieo_poc · viewerPermission ADMIN |
| 이력 보존 | `git push -u origin main` → 원격 `refs/heads/main` = `0858fb0b52a38930d1f64377b396e3748cfa5b3e` (로컬 두 커밋 d92b414 → 0858fb0 그대로) |
| 병합 방식 | merge commit만 허용, squash/rebase 금지, 병합 후 브랜치 삭제 ([설정 후](evidence/repo-settings-after.json)). 승인된 head SHA가 그대로 main 이력에 남게 한다 |
| Actions | 활성, 모든 action 허용 ([actions-permissions.json](evidence/actions-permissions.json)) |
| 비공개 시점 보호 설정(**전환 전 스냅샷**) | HTTP 403 “Upgrade to GitHub Pro or make this repository public” ([protection](evidence/branch-protection-get.txt) · [rulesets](evidence/rulesets-get.txt)) |
| 공개 전환 | GitHub `PublicEvent` 2026-11-11T23:40:07Z. 개발 에이전트의 `gh repo edit --visibility public`(약 23:42Z) 실행 시 이미 공개였을 가능성이 높다(전환 주체는 기록에서 확인하지 않음). 결과 PUBLIC ([visibility-public.json](evidence/visibility-public.json) · [events](evidence/repo-events.tsv)) |
| 공개 후 보호 설정 | 사용 가능: protection 404 “Branch not protected”, rulesets `[]` ([전](evidence/branch-protection-get-public.txt) · [rulesets](evidence/rulesets-get-public.txt)) |
| main 보호 적용 | PR 필수(승인 수 0) · stale 승인 무효화 · 필수 체크 2개(strict: 최신 main 기준) · 관리자 포함 강제 · force push/삭제 금지 ([branch-protection-set.json](evidence/branch-protection-set.json)) |

공개 전 점검: 전체 커밋에서 토큰/키 패턴 검색 0건. 공개되는 개인 정보는 커밋 작성자 메일(git 설정값)이며, 참조 NPG repo `HeaInSeo/notion-projection-guard`는 이미 공개다. 기준선 커밋에 제3자 논문 PDF `07_supporting_research/FoundationDB_SIGMOD21.pdf` 1건이 포함돼 함께 공개된다(이력 보존 지시로 이력 재작성은 하지 않음).

## 시간 순서(GitHub 서버 시각, UTC)

| 시각 | 사건 |
| --- | --- |
| 23:3x | 비공개 생성·main push·보호 설정 403 관측(위 전환 전 스냅샷) |
| 23:40:07 | PublicEvent |
| 23:41:41 | PR #1 생성(head 66842d1 — 그 보고는 여전히 비공개/403으로 기술해 검토에서 반려됨) |
| 23:41:55 | `chatgpt-codex-connector[bot]` 자동 검토 요약 댓글(66842d1, 발견 없음) |
| 23:43:23 | head 9b372bd(공개·보호 기록) → 66842d1 검토 배정 무효화 댓글 |

서버 시계는 NTP 동기(chrony, 오차 0.0001초)이며 GitHub API 시각과 대조했다.

## 기존 외부 자동화

이 계정에 **ChatGPT Codex GitHub 연동(`chatgpt-codex-connector[bot]`)**이 설치돼 PR 생성 시 자동 검토를 단다. 사용자 계정의 기존 자동화이므로 변경하지 않는다. 이 PoC의 독립 검토 기록(검토 세션 → PR 댓글·원장)과 구분하며 보조 정보로만 참조한다. 이후 P07 등에서 “검토 주체” 판정에 섞이지 않도록 원장에 별도 표기한다.

## 1.9절 규칙 적용 방식과 한계

- 작업 하나에 PR 하나, 열린 수정 PR 하나. 독립 검토는 별도 검토 세션이 PR의 **head SHA**를 대상으로 수행하고 결과를 PR 댓글(대상 SHA 명시)로 남긴다. 새 head가 생기면 이전 검토는 무효로 기록하고 새 SHA로 다시 검토한다.
- 병합은 `PUT /repos/{owner}/{repo}/pulls/{n}/merge`의 `sha`=승인 head SHA 조건부 병합으로만 한다. 필수 체크·PR 경유는 서버가 강제한다.
- 진행 원장(state.json)·생성 화면·검토 기록도 main 보호 때문에 별도 **보고 PR**로 반영한다. 보고 PR은 1.9의 “수정 PR”과 구분하고, 검토 중인 수정 PR의 head를 바꾸지 않는다(DEV_AGENT_HANDOFF §6).
- **남은 한계**: GitHub 계정이 하나라 PR 작성자와 검토 세션이 같은 계정이다. GitHub의 Approve는 자기 PR에 제출할 수 없으므로 승인 수 0으로 두고 독립 검토는 PR 댓글+원장 기록으로 남긴다. 서버가 “독립 승인”을 강제하지는 않는다(P07/P23에서 별도 자격으로 다룰 사항).
- 보호 설정의 실제 거부 시험(main 직접 push 등)은 아직 하지 않았다. 설정값 관측만 있다(P23/05-03 범위).
- 공개로 STATUS.md를 로그인 없이 열 수 있다: `https://github.com/HeaInSeo/ieo_poc/blob/main/docs/poc/progress/STATUS.md` 익명 요청 HTTP 200. 화면은 main에 병합된 원장 기준이다.
