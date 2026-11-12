# PR #7 독립 재검토 (Guardrail) · 01-03 단계별 동결 v1.3.3-A1 교정본

- 검토 SHA: `814a5fb` (branch `task/01-03-staged-freeze`)
- 기준(base): `main` `925a0c0`
- 이전 검토: `5c3af86` reject (`review-pr7-5c3af86.md`)
- 검토일: 2026-11-12 KST
- 권위 문서: `docs/poc/User_Decisions_2026-11-12_0103.md` (사용자 원문)
- 범위: `git diff 925a0c0 814a5fb`(57개 파일, +1403/−76), 그중 교정분 `5c3af86..814a5fb`(47개 파일). 깨끗한 worktree에서 검증기·Rust 시험·명세 검사를 다시 실행했다.

VERDICT: approve

## 요약

이전 P2 4건은 모두 고쳐졌다. 이전 P3 8건 중 6건은 고쳐졌고, 2건(P3-7 원장, P3-8 자기 선언성)은 의도대로 기록만 하거나 후속으로 넘겼다. 새로 찾은 P1·P2는 없다. 새 P3는 9건이며 모두 병합 뒤 후속으로 처리해도 된다.

- 9개 스프린트가 모두 rev 5다. digest를 직접 재계산했고 9/9 일치한다.
- 프로파일 rev 4는 base와 같다. rev 5는 정확히 7개 항목만 다르다.
- 장부와 명세 rev 5의 수용 기준이 같다(검증기가 문자열로 대조한다).
- 판정표는 장부, 명세, 런타임 계약 §8, `probe_gate_table.json` 네 곳에서 같다.
- 사용자 금지 사항을 지켰다. “모든 실험값 동결” 보고는 없고, 작업 수는 73/31·12/5 그대로다.

## 1. 이전 지적의 처리 상태

| 이전 | 상태 | 근거(814a5fb) |
| --- | --- | --- |
| P2-1 단계별 동결 미반영·차단 미강제 | **고침** | ① 장부 `:28`에 A1 읽기 규칙을 넣었다. ② EARLY-01 문단(`:33`)을 “01-03에서 채택, 첫 실행 전 동결 증거(L02)”로 고쳤다. ③ 04-09·04-16·07-02·07-03·07-04 수용 기준(장부 `:121,128,162-164`, 명세 rev 5 프로파일·criteria·exit_condition)에 “첫 실행 전에 L0x 동결 증거가 없으면 시작하지 않는다”를 넣었다. 07-04의 “01-03에서 고정한 기준”은 “그 동결 기준”으로 바꿨다. ④ POC-02~09 `limits.unresolved_reason`을 L01 문구로 통일했다(9개 모두 rev 5). ⑤ 명세 최상위 `amendments[0].reading_rule`(`Sprints_v0.json:4685-4690`)을 추가했다. ⑥ P36 Protocol `:9`와 HANDOFF `:104`를 고쳤다. ⑦ `freeze_register.json`의 각 later 항목에 `enforced_by`를 넣었고, REPORT `:58`에 “차단의 강제 위치”를 적었다. 기계 강제는 여전히 L01(UNBOUND_DRAFT)뿐이고 나머지는 수용 기준 문장이다. 보고서가 이 사실을 정직하게 밝히고 있다(→ 새 P3-N2·N3) |
| P2-2 D8 후보 준비 연기 | **고침** | `P26_Candidates.md`에 관측 시각·HEAD SHA가 있는 스냅샷과 원본 `gh` JSON 28개를 첨부했다. REPORT `:54-56`과 L04 `candidate_snapshot`은 이를 “스냅샷이며 동결 표본 아님, 07-01 재대조”로 정리했다. NodeKit 후보가 부족하다는 사실(3건)도 숨기지 않았다. 값은 바꾸지 않았고 사용자 결정 사항으로 올렸다 |
| P2-3 §8.2 판정 어휘·극성 | **고침** | `Runtime_Contract_v0.md:168`에서 probe 2를 “자격·세션·저장소 접근 격리”로 바꿨다. 판정 어휘는 성립 확인/실패/미확인이고, “지원/미지원 어휘는 쓰지 않는다”를 명시했다. 시험 대상에 역할 볼륨(작업 저장소·scratch)과 다른 역할 세션의 재개를 포함했다 |
| P2-4 D4 “모든” 약화 | **고침** | 다음 세 곳을 모두 “자기 repo를 포함한 코디네이터의 **모든** 실제 실행 등록·배정 전”으로 고쳤다: `freeze_register.json` L01 `freeze_before`·`blocks`, REPORT `:42`, 9개 명세 `unresolved_reason`. 배정 경로를 무엇이 막는지는 아직 적지 않았다(→ P3-N3) |
| P3-1 F07 정의 분리 | 고침 | REPORT `:23`, `freeze_register` F07 `note`: “동결한 것은 수치뿐, 분리는 해석이며 사용자 확인 대상”. `result.json` unresolved에도 들어 있다 |
| P3-2 도구·MCP·자식 탐지 | 고침 | `Probe_Budget_Rules.md:26`: `--strict-mcp-config --mcp-config <빈 파일>`과 `--forward-subagent-text`를 쓰고, “모든 자식 요청을 잡는다고 주장하지 않는다”고 적었다. §5(`:46-48`)는 P2를 OS 경계(UID·컨테이너)에서 시험하고 미확인이면 중단한다고 정했다. 이 문서의 플래그 8개는 모두 `claude-help.txt`에 있다(아래 명령표) |
| P3-3 벽시계·rate limit·timeout | 고침 | `:11`: 7,200초에 F06 대기가 포함된다. `:12`: `timeout -k 10 290`이므로 최대 300초다. `:43`: probe 구간에서는 §7의 TRANSIENT backoff를 쓰지 않는다 |
| P3-4 예산 소진 규칙 | 고침 | 장부·명세 `01-04.accept`와 런타임 계약 `:178`에 “예산 소진으로 판정하지 못한 행은 관측 불가(②는 미확인)”를 추가했다 |
| P3-5 확인표 긴장 | 고침 | `Runtime_and_Operational_Probe_Checklist.md:23`에 04-15 판정과 01-04 한도 구조 판정이 별개라는 괄호를 넣었다 |
| P3-6 L08 축약 | 고침 | `freeze_register` L08과 REPORT `:49`에 “현재 판정에 사용하지 않고”와 “재활성화로 이어지지 않게”를 넣었다 |
| P3-7 진행 원장 미해결 표시 | 미처리(의도) | 원장 갱신은 별도 보고 PR에서 한다. 차단 사유가 아니다 |
| P3-8 무결성 검사 자기 선언성 | 기록됨 | `Amendment_v1.3.3-A1.md:16`: “정합 검사일 뿐 진위 검사가 아니다… git 이력·PR·독립 검토로 확인”. 한계 자체는 남아 있다(→ P3-N1) |

## 2. 새 검토 결과

### 집중 확인 항목(문제 없음)

- **9개 스프린트 rev 5와 digest.** base 대비 바뀐 필드는 다음뿐이다.
  - 모든 스프린트: `revision` 4→5, `sources[3]`(결정 원문 `d179dfd6…`, 실제 SHA-256 일치), `limits.unresolved_reason`, `digest`.
  - 수용 기준이 바뀐 7개 작업: `acceptance.criteria`·`exit_condition`·`profile_revision`.
  - 직접 재계산한 digest(canonical JSON, sort_keys, `(',',':')`, ensure_ascii=False, `digest` 제외, SHA-256)는 9/9 일치한다. 예: POC-01 `058bfd3e…fb60`, POC-07 `8a3a976d…1545`. `ieo-spec`의 `computed_digest`와 `validate.txt`도 같은 값이다.
- **프로파일 rev 5.** rev 4는 base와 같다(객체 동일). rev 4와 rev 5는 키 집합이 같고, 차이는 `01-03/01-04/04-09/04-16/07-02/07-03/07-04.accept` 7개 항목뿐이다. 이 목록은 `Amendment_v1.3.3-A1.json`의 `changed_items`·`changed_acceptance`와 같다.
- **검증기 v1.3.2 대비 변경 대조(`validate_package.py:40-41`).** 이 검사는 base의 검사와 **동등하며 약화가 아니다**. 이유는 다음과 같다.
  - base는 “현재 criteria ≠ v1.3.2”인 집합을 10개로 고정했다. 지금은 같은 10개 집합을 rev 4 항목 기준으로 고정한다.
  - `:51`은 rev 4와 rev 5의 차이를 고정한다. `:60`은 rev 5를 쓰는 작업을 `changed_acceptance`로 한정하고, rev 4 항목을 쓰는 작업은 criteria = exit_condition = 그 항목임을 계속 단언한다. 따라서 rev 4를 쓰는 63개 작업에 대한 제약은 base와 같다.
  - 바뀐 7개 작업은 “v1.3.2→rev 4”와 “rev 4→rev 5” 두 단계로 나눠 회계한다. 현재 criteria로 비교했다면 04-09·07-02·07-03이 v1.3.2 차이 집합에 끼어 고정 목록과 어긋났을 것이다. 이를 피하려는 변경으로서 타당하다.
  - 약해진 점은 하나다. 5c3af86에서는 rev 4→5 차이 집합이 하드코딩돼 있었는데, 지금은 개정 색인 JSON에서 읽는다(→ P3-N1).
- **최상위 `amendments` 키.** 스키마 검증은 스프린트마다 하므로(`validate_package.py`가 `s` 단위로 검증) 최상위 키는 스키마 대상이 아니다. `ieo-spec check-manifest`는 `manifest["sprints"]`만 읽으므로 영향이 없다(9×201). 이 키는 digest 범위 밖이다(→ P3-N2).
- **장부 읽기 규칙·EARLY-01.** 장부 `:28` 읽기 규칙의 범위는 “수치·표본·예산·기대값”이다. 사용자 원문 “후속 실험의 표본·예산·기대값은 각각 첫 실행 전에 동결 증거”와 일치한다. EARLY-01 `:33`의 임시 예산 28,800초·검토 4회·3,000,000 토큰은 D5와 같다. 완료 예측이라는 표현은 없다.
- **P36 Protocol `:9`.** D9 수치는 결정안과 같다(5/6, ×1.2, 300초, 질문 ≤ 자유, 비열등 −1, 치명적 오해 0, 재시험 1회). “초기 탐색 기준”으로 한정했고, 0700 봉인 주장은 하지 않았다. `:40,54`의 “01-03에서 고정”은 `:9`의 단계별 동결 문단 아래에 있어 덮인다.
- **런타임 계약 §8.** 행 1·2·3의 판정 어휘가 판정표와 일치한다. 판정표 문단에 예산 소진 규칙이 있다. §7 TRANSIENT와의 우선순위는 예산 규칙에 적혀 있다.
- **Probe_Budget_Rules 플래그.** 문서에 나오는 플래그 8개(`--tools`, `--strict-mcp-config`, `--mcp-config`, `--forward-subagent-text`, `--max-budget-usd`, `--fallback-model`, `--version`, `--help`)는 모두 `01-02/.../evidence/claude-help.txt`에 있다. `--tools`는 “from the built-in set… "" to disable all tools”(`:255-258`), `--forward-subagent-text`는 “parent_tool_use_id set (only works with --print and --output-format=stream-json)”(`:107-110`)로 문서의 설명과 맞다.
- **freeze_register.json.** stage_0는 8개, later는 9개다. L01·L04·L08 문구가 사용자 원문과 일치한다. 나머지 변경은 대부분 JSON 재포맷이다(값 동일).
- **P26 후보와 원본 JSON 대조(표본 확인).**
  - NodeKit main `0e99c37c…`, sori master `2fa7f5e1…`, 기타 브랜치와 열린 PR(#102, #48)이 `*_branches.json`·`*_prs_open.json`과 일치한다.
  - 열린 이슈는 NodeKit 8건, sori 5건으로 합계 13건이고, assignee는 모두 비어 있다.
  - sori #45의 go.mod 변경은 `aadcd13c`(PR #44, 2026-10-25)다.
  - 본문 인용이 원문에 있음을 확인했다: #38 “one modifying PR”·`_busy_timeout`, #11 “repo slot”, #8 “Required decision”, #32 “Central assigns”, NodeKit #94 “정책 결정 대기”, #95 “허가하지 않”.
  - PR #48의 변경 파일이 문서 기재와 같다.
  - 종류별 집계(sori 기능 2·결함 2·계약 1·실패 복구 1, NodeKit 0·1·2·0)가 후보 본문과 맞다.
  - 후보 문서는 시간·비용·보류 한도 값을 제안하지 않았다. D8의 “NodeKit 6건”을 채울 수 없다는 사실을 값 변경 없이 사용자 결정으로 올렸다.
- **민감 정보.**
  - 원본 JSON에서 토큰·키 패턴은 0건이다(`AKiA…` 1건은 PGP 서명 본문 안의 우연한 문자열이다).
  - 원본 저장소 NodeKit·sori·ieo_poc는 모두 PUBLIC이다(`gh repo view`).
  - Notion 페이지 링크와 개인 이메일은 원본 공개 이슈·커밋 메타데이터에 이미 공개된 것이다(→ P3-N7).
- **host-isolation-check.txt.** 모델 호출 0회 주장과 일치한다. 기록은 sudo 불가와 rootless podman·sub-UID 존재뿐이고, REPORT `:63`은 “실제 동작은 01-04에서 확인”으로 과대 주장을 하지 않는다. 비밀은 없다(→ P3-N8).

### 새 지적 사항(P1·P2 없음)

#### P3-N1 · rev 4→5 차이 집합의 자기 선언, rev 4 항목 내용 미고정
- `docs/poc/scripts/validate_package.py:51,60`
- 5c3af86은 `['01-03.accept','01-04.accept']`를 하드코딩했다. 지금은 `Amendment_v1.3.3-A1.json`의 `changed_acceptance`를 그대로 신뢰한다.
- rev 4 항목 중 지금 어떤 작업도 참조하지 않는 7개는 내용이 고정되지 않는다(v1.3.2 대비 같음/다름만 확인한다).
- 권고: rev 4 프로파일의 canonical digest(`e80e4b12513591882b7616859b0e74ef142cccade888fbc3f5a8b9b009f8ada1`)를 검증기에 고정한다. 이전 P3-8과 같은 계열이다.

#### P3-N2 · 명세 읽기 규칙이 스프린트 digest 밖에 있음
- `docs/poc/Console_Coordinator_Sprints_v0.json:4685-4690`
- `amendments`는 최상위 키라서 어떤 스프린트 digest에도 묶이지 않는다. 실행 등록되는 스프린트 객체에도 실리지 않는다.
- L02·L03의 `enforced_by`(“장부·명세의 A1 읽기 규칙”)는 이 비결속 문장에 기대고 있다. 각 스프린트의 `sources[3]`(결정 원문)이 digest로 결속돼 있으므로 차단 요건은 아니다.
- 권고: 01-07 계약 또는 L07에서 읽기 규칙을 스프린트 수준 정책 참조로 옮길지 결정한다.

#### P3-N3 · later 차단은 L01 외에 기계 강제가 아님. 배정 경로를 막는 주체가 명시되지 않음
- `freeze_register.json:137`(L01 `enforced_by`)은 등록 검사만 적었다. 배정을 막는 것은 무엇인지(“배정은 실행 가능 등록된 명세만 허용”과 같은 규칙) 문장이 없다.
- L04·L05·L06·L09는 수용 기준 문장, L02·L03은 읽기 규칙이다. 04-16의 선행(`04-09·01-03`)은 여전히 01-03 수용만으로 충족된다.
- 이 사실을 REPORT `:58`이 정직하게 적었으므로 수용한다.
- 권고: 02-03/L07 구현 때 원장 동결 증거 필드나 배정 전 검사를 추가한다.

#### P3-N4 · 개정 색인·보고서의 낡은 문구
- `docs/poc/Amendment_v1.3.3-A1.json:122`: Sprints 항목 `reason`이 여전히 “POC-01 rev 5(01-03·01-04 수용 기준…)”다. 실제는 9개 스프린트 rev 5와 7개 항목이다.
- `reports/tasks/01-03/staged-freeze-20261112/REPORT.md:63`: “개정 10개”라고 쓰여 있지만, 검증기 출력과 `validate.txt`는 `amended: 11`이다.
- `docs/poc/Amendment_v1.3.3-A1.md:24`(§3 읽는 법): 적용 범위가 아직 “계획·장부”다. 명세에는 별도 `reading_rule`이 있고 §1 표에는 명세가 들어 있다.

#### P3-N5 · 01-03에 배정됐으나 동결 대장에 없는 항목
- `Console_Coordinator_PoC_Validation_Plan.md:76`: 진행·체크포인트 보고 “임계값은 01-03에서 고정한다”.
- 같은 파일 `:1024`: idle 운영 정책 “01-03/P34/P28-M”.
- `V1_3_3_Changes_and_Validation.md:28`의 P36 행.
- 계획 끝 A1 문단이 일반 규칙으로 덮지만, 이 항목들에 대응하는 later 항목(담당·시점·차단)이 없다. 그래서 읽기 규칙이 가리키는 대장 항목이 없다.
- 권고: 다음 개정에서 later 항목을 추가하거나 “해당 없음/다른 작업 소관”을 명시한다. 이것은 작업 수를 늘리는 변경이 아니다.

#### P3-N6 · F06 최대 10회와 7,200초의 관계, §5 계수 문구
- `Probe_Budget_Rules.md:11,43`: 대기가 7,200초에 포함되므로 F06의 30분 간격 재개는 probe 구간에서 사실상 최대 3회다. 10회는 도달할 수 없다. 모순은 아니지만 한 줄 명시를 권한다.
- `:48`: “모델 호출 없이 하는 파일 시험은… (CLI 기동으로 센다)”에서 파일 접근 시험은 CLI 기동이 아닐 수 있다. “모델 없는 실행으로 따로 센다”가 정확하다.

#### P3-N7 · p26-raw 원본의 최소화
- `evidence/p26-raw/NodeKit_commits.json`, `sori_commits.json`, `sori_commits_gomod.json`(합계 약 790KB)은 커밋 API 전체를 담고 있다. 개인 이메일 `seoyhaein@gmail.com`(106회), PGP 서명, avatar URL이 들어 있다.
- 원 공개 저장소에 이미 공개된 정보이므로 차단 요건은 아니다. 그래도 공개 repo에 재배포할 필요는 없다.
- 권고: 후속에서 sha·date·message·PR 번호만 남긴 축약본으로 교체하는 것을 검토한다. 이력 재작성은 하지 않는다.

#### P3-N8 · host-isolation-check.txt의 호스트 식별 정보
- `evidence/host-isolation-check.txt:2-3,13-14`에 사용자명, UID/GID, 그룹(wheel·ollama), SELinux 컨텍스트, sub-UID 범위가 공개 repo에 기록돼 있다. 비밀은 아니다.
- 권고: 향후 증거에서는 그룹 목록을 줄이는 정도의 최소화를 검토한다.

#### P3-N9 · P26_Candidates의 문서 근거 재현성
- `P26_Candidates.md:17`: 코드 줄 번호 같은 문서 근거는 커밋에 포함되지 않은 shallow clone(`p26/nodekit`, `p26/sori`)에서 읽었다.
- HEAD SHA가 고정돼 있어 재현할 수 있으므로 문제는 작다. 07-01 재대조 때 같은 SHA로 확인하면 된다.

## 3. 실행한 명령과 결과

| 명령 | 결과 |
| --- | --- |
| `git diff --stat 925a0c0 814a5fb` / `5c3af86 814a5fb` | 57개 파일 +1403/−76 / 47개 파일 +659/−157 |
| `git worktree add <scratchpad>/wt7b 814a5fb` | clean worktree(`git status` 변경 없음) |
| `.venv-poc/bin/python scripts/validate_package.py`(wt7b/docs/poc) | rc=0. `package_integrity` = delivered 120 / amended 11 / absent 4 / v1.3.3-A1. 9개 digest가 rev 5. `validate.txt`와의 차이는 `gitignored_paper_pdfs_absent`(0 대 4)뿐이다. 작업공간과 clean worktree의 차이이므로 설계상 정상이다 |
| `.venv-poc/bin/python scripts/render_progress.py --check` | `{"ok": true, ...}` rc=0 |
| `PATH=/opt/rust/cargo/bin:$PATH cargo test -q`(별도 target dir) | 1+30+4+0+16+21+0+0 = 72 passed, 0 failed |
| `cargo run -q -p ieo-spec --bin ieo-spec -- check-manifest docs/poc/Console_Coordinator_Sprints_v0.json` | rc=0. 9개 모두 201 `ACCEPTED_FOR_SEMANTIC_REVIEW`, 모두 revision 5 |
| 같은 명령 `--executable` | rc=1. 9개 모두 422 `UNBOUND_DRAFT`(“시간/토큰/교정 한도” 미바인딩 포함). `executable-check.txt`와 일치 |
| Python 재계산: 9개 스프린트 digest | 9/9 일치(POC-01 `058bfd3e…`, POC-07 `8a3a976d…`) |
| Python: 프로파일 rev 4와 base 비교, rev 4/5 차이, base 대비 스프린트 필드 차이 | rev 4 동일. 차이 7개 항목. 스프린트 차이는 위 “집중 확인”의 범위뿐이다 |
| `sha256sum User_Decisions_2026-11-12_0103.md` | `d179dfd6…21f0`(명세 출처 digest와 일치) |
| `Probe_Budget_Rules.md`의 플래그와 `claude-help.txt` 대조 | 8/8 존재 |
| p26-raw JSON 표본 대조(브랜치·PR·이슈·assignee·go.mod 커밋·본문 인용) | 모두 일치 |
| `gh repo view HeaInSeo/{NodeKit,sori,ieo_poc} --json visibility`(읽기 전용) | 셋 다 PUBLIC |
| 비밀 패턴 검색(p26-raw) | 0건. `AKiA…`는 PGP 서명 안의 문자열이다 |
| `git ls-tree -r 814a5fb \| grep -iE '\.pdf$\|/p26/'` | 0건 |
| `git worktree remove` + target dir 삭제 | 제거 완료 |
