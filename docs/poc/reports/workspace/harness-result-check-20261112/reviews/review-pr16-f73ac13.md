<!-- 보관본: PR #16 독립 검토 f73ac13 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/16#issuecomment-6819231087 (머리말·끝 교정 안내 생략) -->
# PR #16 독립 재검토 (Guardrail) — fix/01-04-followups @ f73ac13

- 검토 대상: head `f73ac13`. 직전 검토는 `8cd805e`(reject)이고, 이번 diff는 `8cd805e..f73ac13`(커밋 1개, 계약과 REPORT만 변경)이다. base는 main `2f3af51`.
- 방법: `git archive f73ac13` 트리에서 검토했다. 두 역할 볼륨의 `CLAUDE_CONFIG_DIR`은 `podman unshare`로 이름·크기·시각만 읽었다(`projects/` 포함, 값 미출력). 모델 요청·probe 실행·수정·커밋·push는 하지 않았다.
- 4번째 검토이므로 이전 차단 사항뿐 아니라 PR 전체(2f3af51..f73ac13, 10개 파일)를 다시 훑어 남은 차단을 찾았다.
- 검토일: 2026-11-12

VERDICT: approve

## 결론

8cd805e의 P2-B는 해소됐다. 정리하면 다음과 같다.

- CLI 내장 플러그인 기록 `plugins/store/*_builtin-*.json`을 digest에서 제외했다.
- 제외 목록이 관측 범위에 한정된 것임을 명시하고, 새 자동 기록이 나오면 02-04 검수 전에 분류하는 규칙을 두었다.
- (c)를 네 종류의 정상 실행으로 넓혔고, (b)가 제외 경로를 쓰지 않도록 했다.

PR 전체에 남은 P1·P2는 없다. 아래 P3 두 건은 비차단이며, 02-04의 시험 (c)와 분류 규칙 안에서 다뤄진다.

## 확인

1. **01-04 원본 불변**: `git diff 2f3af51 f73ac13 -- docs/poc/reports/tasks/01-04`는 0바이트다.
2. **영향 감사**: `audit_result_events.py` 출력이 증거 파일과 같다. 독립 집계 결과도 이전과 같다(35건 모두 result 사건 있음, 비0 종료는 P2c 하나, 모델 없는 시험 33/33에 표식, 산출물 출력 비어 있지 않음).
3. **회귀 시험**: 아카이브 트리에서 `RESULT PASS 0`이다. 이번 diff에 코드 변경은 없다.
4. **P2-B 근거**: 개발·Guardrail 두 볼륨 모두 `plugins/store/cc-plugin-agents-md_builtin-b049b47b3618.json`이 있다. 이 파일이 `plugins/` 아래 유일한 파일이다. 01-04 L1·G3b 원본과도 일치한다.
5. **(b)·(c)와 §5-6의 일관성**: 대상, 제외, `.claude.json` 키, 관측 한정 규칙, 한계, 첫 기동 처리가 서로 모순되지 않는다. §10 항목 3의 완료 조건 (a)·(b)·(c)는 §5-6과 맞는다.
6. **PR 전체 재점검**: 하네스(`artifact()`, 결과 사건 선택, 묶음 경로), REPORT §1~§6, 계약 rev 6의 §10 6개 항목과 근거, ASSETS(GH1~GH5)를 다시 확인했다. 이전 검토 이후 회귀나 새 모순은 없다.
7. **검증·스캔**:
   - `validate_package.py` rc=0, `render_progress.py --check` rc=0.
   - 비밀 16개 값(역할 토큰 2, E 비밀 5, oauthAccount 9)을 f73ac13 트리와 대조했고, 일치는 0건이다.
   - 커밋 메시지에서 `sk-ant`는 0건이다.

## 발견 사항 (비차단)

### P3-C "지시성 대상 안의 자동 기록은 `plugins/store` 하나뿐"이라는 관측은 `projects/`를 제외하고 한 것이다. 빈 `projects/*/memory/` 디렉터리도 CLI가 자동으로 만든다

- 위치:
  - `docs/poc/reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md:141`
  - `docs/poc/reports/workspace/harness-result-check-20261112/REPORT.md` §6 첫 항목
- 근거: 두 볼륨에서 다음 빈 디렉터리를 확인했다(파일 0개).
  - 개발: `projects/-state-work/memory/`(생성 epoch 1794467142), `projects/-state-work-g2/memory/`(1794468797)
  - Guardrail: `projects/-state-work/memory/`, `projects/-state-guardrail-cwd/memory/`(1794468786)
- 생성 시각은 각 cwd의 첫 실행 시각과 맞는다. 이 실행들은 자동 기억 기본값(켜짐)으로 돌았고, ADDENDUM §6에서 init `memory_paths.auto`가 이 경로를 가리켰다.
- 계약대로 `autoMemoryEnabled:false`일 때도 이 디렉터리가 생기는지는 확인되지 않았다. AM 시도는 이미 디렉터리가 있던 `/state/work`에서 돌았다.
- 디렉터리가 생기고 digest가 빈 디렉터리 이름을 포함하면, (c)에서 변조로 오판한다.
- 비차단 이유: §5-6의 관측 한정 규칙과 (c)가 바로 이 경우를 02-04 검수 전에 잡아 분류하도록 정해 두었다.
- 권고:
  - REPORT §6의 서술에 "`projects/` 제외 목록"이라는 범위를 적는다.
  - digest를 "파일 내용 기준(빈 디렉터리 무시)"으로 정해 두면 이 경우가 자연히 해소된다.

### P3-D 정리 보존 범위가 `projects/<cwd>/*.jsonl`뿐이다. 자식 에이전트 기록 `projects/<cwd>/<session_id>/subagents/`는 지워진다

- 근거: 개발 볼륨에 `projects/-state-work/29603ca9-…/subagents`가 있다(T 시도 23의 자식 기록).
- 계약 §5-6과 §4.1의 정리 규칙은 이 디렉터리를 지운다. 이것이 부모 세션의 `--resume`에 영향을 주는지는 01-04에서 시험하지 않았다. 01-04의 G3b 재개 세션에는 자식이 없었다.
- (c)의 "자식 에이전트 호출 뒤 reconnect"가 이것을 드러낼 것이므로 비차단이다. 02-04에서 필요하면 보존 범위에 `projects/<cwd>/<session_id>/subagents/`를 추가하는 결정을 기록하면 된다.

## 요약

- P2-B는 해소됐고, PR 전체에 남은 차단 사항은 없다. 따라서 **approve**한다.
- P3 두 건은 02-04의 시험 (c)와 관측 한정 분류 규칙이 다룬다.
  - P3-C: 빈 `projects/*/memory/` 디렉터리의 자동 생성. 관측 범위를 명시하거나 digest를 파일 기준으로 정하기를 권고한다.
  - P3-D: 정리 때 자식 에이전트 기록이 삭제되는 문제.
