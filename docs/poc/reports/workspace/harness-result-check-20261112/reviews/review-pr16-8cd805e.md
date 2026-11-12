<!-- 보관본: PR #16 독립 검토 8cd805e 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/16#issuecomment-6819195113 (머리말·끝 교정 안내 생략) -->
# PR #16 독립 재검토 (Guardrail) — fix/01-04-followups @ 8cd805e

- 검토 대상: head `8cd805e`, 직전 검토 `d579f97`(reject). 이번 diff는 `d579f97..8cd805e`(c97fb76, 8cd805e)다. base는 main `2f3af51`.
- 방법: `git archive 8cd805e` 트리에서 검토했다. 개발 역할 볼륨의 `CLAUDE_CONFIG_DIR`은 `podman unshare`로 디렉터리 목록과 `.claude.json` **키 이름만** 읽었다. 값은 출력하지 않았다. 모델 요청·probe 실행·수정·커밋·push는 하지 않았다.
- 검토일: 2026-11-12

VERDICT: reject

## 결론

d579f97의 P3-A와 P3-B는 해소됐다. P2-A도 대부분 해소됐다. digest를 지시성 항목으로 한정했고, `.claude.json`은 키 단위로 비교하며, 시험 (c)는 "정상 실행 뒤 reconnect"로 바뀌었다.

그러나 digest 대상에 넣은 `plugins/`는 CLI가 정상 실행 때 스스로 파일을 만드는 곳이다. 01-04 L1과 G3b 원본, 그리고 현재 역할 볼륨에서 이를 확인했다. 그런데 제외 목록에는 이 파일이 없다. 그래서 정리 직후 템플릿 digest와 정상 실행 뒤 digest가 여전히 다르고, 시험 (c)는 계약대로 구현하면 실패한다(P2-B). 이것 하나로 reject한다. 한 줄 수정으로 해소된다.

## 재확인 (문제 없음)

1. **01-04 원본 불변**: `git diff 2f3af51 8cd805e -- docs/poc/reports/tasks/01-04`는 0바이트다.
2. **영향 감사**: `audit_result_events.py` 출력이 증거 파일과 같다. 독립 집계 결과는 이전과 같다(35건, 모두 result 사건 있음, 비0 종료는 P2c 하나).
3. **회귀 시험**:
   - 아카이브 트리에서 rc=0, PASS 53, `RESULT PASS 0`이다. 새 사례 `artifact_missing_dir_marker_stops`를 포함한다.
   - PASS/FAIL 줄이 증거 파일과 같고, 출력에 `sk-ant`는 0건이다.
4. **P3-A 해소**:
   - `HOOKS`와 G1 산출물은 `DIR_OK`를 필수 표식으로 요구한다. 위치는 probe2.py:77, :506-509, :535, :560, :569다.
   - G3 목록(:563), N4(:595), L1(:615)은 `set -o pipefail`을 쓴다.
5. **P3-B 해소**: `test_result_check.py:4`와 REPORT §1.3이 `podman unshare` 사용을 명시한다.
6. **`.claude.json` 지시성 키**:
   - 현재 개발 역할 `.claude.json`의 최상위 키는 15개다(`cachedGrowthBookFeatures`, `machineID`, `pluginUsage` 등).
   - `mcpServers`와 `projects` 키가 없다. 계약의 "정상 실행에서 지시성 키는 비어 있었다"는 서술과 맞는다.
7. **검증·스캔**:
   - `validate_package.py` rc=0, `render_progress.py --check` rc=0.
   - 비밀 16개 값(역할 토큰 2, E 비밀 5, oauthAccount 9)을 8cd805e 트리와 대조했고, 일치는 0건이다.
   - `d579f97..8cd805e` diff와 커밋 메시지에서 `sk-ant-` 형태는 0건이다.

## 발견 사항

### P2-B (차단) digest 대상 `plugins/`에 CLI가 정상 실행 때 만드는 `plugins/store/…_builtin-….json`이 들어간다. 제외 목록에는 이 파일이 없어서 시험 (c)가 실패한다

- 위치: `docs/poc/reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md:139`(대상 목록의 `plugins/`)와 `:141`(제외 목록)
- 근거:
  - 01-04 `L.json`의 `L1.volume_files_written`(시도 16, 변조 없는 정상 1회 실행)에 `./claude/plugins/store/cc-plugin-agents-md_builtin-b049b47b3618.json`이 있다.
  - 01-04 `G.json`의 `G3b-rerun.config_dir_before_wipe`에 `./plugins`와 `./plugins/store`가 있다.
  - 현재 개발 역할 볼륨에도 있다. G3b의 wipe는 `projects` 외 모든 항목을 지웠는데, 그 뒤의 정상 실행들(N·L·C·T·R·AM)이 `plugins/store/cc-plugin-agents-md_builtin-b049b47b3618.json`을 다시 만들었다.
- 계약은 "CLI가 정상 실행마다 쓰는 항목은 대상에서 뺀다. 01-04 L1·G3b에서 관측했다"라고 적었다. 그러나 같은 L1·G3b 관측에 있는 `plugins/store/`는 빠져 있다.
- 결과: 정리 직후 템플릿에는 `plugins/`가 없다. 정상 실행 뒤에는 `plugins/store/…json`이 생긴다. 따라서 지시성 digest가 달라지고, 매 reconnect가 "변조 탐지"가 된다. 정상 실행 뒤 같은 session_id로 재개해야 한다는 (c)와 모순된다.
- 요청: 다음 중 하나로 정하고 근거(L1·G3b)를 적는다.
  - CLI 내장 플러그인 기록(`plugins/store/*_builtin-*.json`)을 제외 목록에 넣는다.
  - 또는 `plugins/`의 지시성 범위를 사용자가 설치하거나 활성화한 플러그인 정의(예: 설치 목록, marketplace, 플러그인 디렉터리)로 한정한다.

  어느 쪽이든 (b)의 표식 주입에 `plugins/`를 쓰는 경우와 겹치지 않게 한다.

### 참고 (비차단)

- 제외 목록은 01-04 시점 CLI(2.1.322)에서 관측한 것이다. 계약이 이미 한계로 적었듯이, 02-04의 시험 (c)가 실제 판정 근거가 된다. 시험 (c)는 "정상 실행 1회 이상"이 아니라 서로 다른 종류의 정상 실행(도구 사용, 자식 에이전트, `/compact`)을 포함해야 제외 목록의 누락이 드러난다. 완료 조건에 이를 한 줄 추가하기를 권한다.

## 요약

- 영향 감사, 원본 보존, 하네스 교정(P2-1·P3-A·P3-B 포함)은 모두 확인됐다.
- 남은 차단은 P2-B 하나다. §5-6의 digest 대상 `plugins/`에 CLI가 정상 실행 때 만드는 `plugins/store/…_builtin-….json`(01-04 L1·G3b 원본과 현재 볼륨에서 확인)이 들어가, 정상 재개 시험 (c)와 모순된다.
- 이 파일을 제외 목록에 넣거나 `plugins/` 범위를 한정하면 approve할 수 있다.
