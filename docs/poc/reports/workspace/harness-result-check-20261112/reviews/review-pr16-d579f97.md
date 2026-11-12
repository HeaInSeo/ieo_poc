<!-- 보관본: PR #16 독립 검토 d579f97 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/16#issuecomment-6819149410 (머리말·끝 교정 안내 생략) -->
# PR #16 독립 재검토 (Guardrail) — fix/01-04-followups @ d579f97

- 검토 대상: head `d579f97`, 직전 검토 대상 `f64c413`(reject), base main `2f3af51`
- 방법: `git archive d579f97` 트리에서 검토했다. 모델 요청·probe 단계 실행 없음. 수정·커밋·push 없음.
- 검토일: 2026-11-12

VERDICT: reject

## 결론

f64c413에서 지적한 P2-1과 P3 8건은 모두 해소됐다. P2-2도 원래 지적한 모순(표식 주입 시험이 재개 성공을 요구하던 문제)과 완료 조건 누락은 해소됐다.

그런데 새로 정한 변조 탐지 방법(§5-6)이 01-04 원본 증거와 맞지 않는다. 이대로면 정상 재개 시험 (c)를 통과할 수 없다(아래 P2-A). 이것 하나로 reject한다. 수정은 문서 몇 줄이면 되고, 실험을 다시 할 필요는 없다.

## 재확인 (문제 없음)

1. **01-04 원본 불변**: `git diff 2f3af51 d579f97 -- docs/poc/reports/tasks/01-04`는 0바이트다.
2. **영향 감사**:
   - 독립 스크립트로 다시 계산했다. 35개 디렉터리 모두 result 사건이 있고, 비0 종료는 P2c 하나다.
   - `audit_result_events.py` 출력은 증거 파일과 바이트 단위로 같다.
3. **산출물 행 확인**(REPORT §1.2 신규 행): 원본과 일치한다.
   - U 3건: `DENIED_FILE_ABSENT`가 있다.
   - G1: `0`, `BAD_WRITE_ABSENT`.
   - G2·G3b-rerun: `NO_HOOK_FILE`. G3a 계열: hook 파일 이름이 있다.
   - N4: 줄 수가 있다.
   - L1: diff 14줄, 볼륨 8줄.
4. **회귀 시험**:
   - 실행 명령: `.venv-poc/bin/python -I probes/01-04/test_result_check.py`. 결과는 rc=0, PASS 52, `RESULT PASS 0`, 33건 확인이다.
   - PASS/FAIL 줄이 증거 파일과 같고, 출력에 `sk-ant`는 0건이다.
5. **P2-1 해소**:
   - `probe2.py`에 `unshare(..., check=False)` 호출이 0건 남았다.
   - `artifact()`(probe2.py:60-68)는 rc≠0이거나 필수 표식이 빠지면 정지한다.
   - G1 대안 분기(probe2.py:514-515)는 `HOOK_COUNT`를 파싱한다. 표식이 없으면 그 앞의 `artifact()`가 이미 정지시킨다.
   - `podman diff`가 실패해도 정지한다.
   - G3 라벨 필수 표식(`container_file_t`, `^700 20001$`)은 01-04 원본 `G3-rerun_label_check`와 맞는다.
6. **P3-1·P3-2 해소**:
   - 마지막 `num_turns≥1` 결과를 쓴다(probe.py:122-123, probe2.py:134-136). N4b의 실제 형태를 합성 시험으로 확인했다.
   - `finish`가 `expected_outcome`을 넘기고, `stop_batch` 경로도 시험에 들어갔다.
7. **P3-3~P3-8 해소**: 표식 한계, probe.py 서술, 낡은 "가정" 문구, 항목 1의 직접·간접 근거 구분과 02-04 시험 이관, GH1~GH5 이름과 원장 기록 시점, 잠금 파일 한계가 모두 반영됐다.
8. **기타**:
   - 검증: `validate_package.py` rc=0, `render_progress.py --check` rc=0.
   - 비밀 스캔: 16개 값(역할 토큰 2, E 비밀 5, oauthAccount 9)을 d579f97 트리 전체와 대조했고, 일치는 0건이다.
   - 2f3af51..d579f97 diff와 커밋 메시지에서 `sk-ant-` 토큰 형태도 0건이다.

## 발견 사항

### P2-A (차단) §5-6 변조 탐지 digest 범위가 CLI가 정상 실행 때 쓰는 파일을 포함한다. 그래서 매 reconnect가 "변조"로 판정되고, §10 항목 3의 시험 (c)를 달성할 수 없다

- 위치:
  - `docs/poc/reports/tasks/01-02/runtime-contract-20261112/Runtime_Contract_v0.md:137-138`(§5-6 "방법")
  - 같은 파일 `:210`(§10 항목 3 완료 조건 (c))
- 계약 내용: digest는 "`CLAUDE_CONFIG_DIR`에서 세션 기록(`projects/<cwd>/*.jsonl`)을 뺀 나머지"를 대상으로 한다. 비교 대상은 마지막 체크포인트의 `settings_digest`, 즉 "정리 직후 템플릿 상태"다. 둘이 다르면 `--resume` 없이 새 세션으로 시작한다.
- 01-04 원본 증거로 보면, 변조 없는 정상 1회 실행만으로도 CLI가 `CLAUDE_CONFIG_DIR`에 jsonl 외 파일을 쓴다.
  - `L.json` `L1.volume_files_written`(시도 16): `claude/.claude.json`, `claude/backups/.claude.json.backup.*`, `claude/plugins/store/…json`, `claude/policy-limits.json.{signature,stamp}.json`, `claude/remote-settings.json`, `claude/remote-settings.json.synced-at`.
  - `G.json` `G3b-rerun.config_dir_before_wipe`: 위 파일들에 더해 `session-env/…`, `sessions`, `shell-snapshots`, `.last-cleanup`가 보인다.
- 따라서 정리 직후 템플릿 digest와 다음 기동 직전 digest는 정상 운영에서도 항상 다르다. 결과는 두 가지다.
  - (c) "변조 없이 reconnect하면 같은 session_id로 재개된다"는 계약대로 구현하면 실패한다.
  - 반대로 (c)를 맞추려고 구현에서 범위를 임의로 줄이면, 계약에 없는 판단이 된다.
  - 어느 쪽이든 §5-2(reconnect는 `--resume`)와 §4.2(재개 지원)와도 충돌한다.
- 요청: 다음 중 하나로 정하고, (a)~(c) 완료 조건에 그 기준을 넣는다.
  - **지시성 파일 집합만** digest한다. 예: `settings.json`, `CLAUDE.md`, `agents/`, `skills/`, `commands/`, `hooks`, `.mcp.json`, `plugins/`(CLI가 만드는 `plugins/store` 처리 포함), `projects/*/memory/`.
  - `.claude.json`처럼 CLI가 다시 쓰지만 지시성 키(예: `mcpServers`, 프로젝트별 허용 목록)를 담을 수 있는 파일은 그 키만 비교하는지 여부를 명시한다.
  - 또는 비교 기준을 "정리 직후"가 아니라 "직전 실행 종료 시 supervisor가 기록한 digest"로 바꾼다. 다만 이 방식은 실행 중 변조를 정상으로 흡수하므로, 같은 컨테이너 개발 코드에 대한 한계를 §2.1처럼 적어야 한다.

### P3-A `artifact()` 스크립트 일부가 파이프·`|| true`로 하위 명령 실패를 가린다

- 위치:
  - `probes/01-04/probe2.py:77`: `HOOKS`의 `ls {d}/state/home | grep HOOK-FIRED || true`
  - G1의 `HOOK_COUNT=$(ls … | grep -c HOOK-FIRED)`
  - `:615` L1: `find … | sort && echo LISTED`
- 대상 디렉터리가 없거나 읽기 실패해도 표식(`HOOKS_BEGIN/END`, `HOOK_COUNT=0`, `LISTED`)이 출력되고 rc=0이 된다. 그러면 "hook 0건"이나 "파일 없음"으로 읽힌다.
- 역할 볼륨이 없으면 다른 단계가 먼저 실패할 가능성이 높아 차단 사유는 아니다. 그래도 `set -o pipefail`을 쓰거나 `test -d … && echo DIR_OK`를 필수 표식으로 추가하면 P2-1의 취지가 완결된다.

### P3-B 회귀 시험이 이제 podman을 쓰는데 문서는 "podman 없이"라고 적었다

- 위치: `probes/01-04/test_result_check.py:4`, `docs/poc/reports/workspace/harness-result-check-20261112/REPORT.md:37`
- 새 시험 8(`artifact_*`)은 `P2.artifact()`를 통해 `podman unshare bash -c …`를 실행한다. 모델·토큰과는 무관하다.
- 문구를 "모델·토큰 없이, `podman unshare`만 사용"으로 고치면 된다.

## 요약

- 영향 감사(01-04 판정 영향 없음)와 원본 보존은 유지된다. f64c413의 P2-1과 P3 8건은 해소됐다.
- 남은 차단은 P2-A 하나다. §5-6의 digest 범위가 CLI가 정상 실행 때 쓰는 파일(01-04 L1·G3b 원본에 기록됨)을 포함해서, 정상 재개 시험 (c)와 모순된다.
- digest 대상을 지시성 파일 집합으로 한정하거나 비교 기준을 바꾸면 approve할 수 있다.
