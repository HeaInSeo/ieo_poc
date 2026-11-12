<!-- 보관본: PR #19 독립 검토 f873fa4 원문 · 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/19#issuecomment-6821185238 (머리말·끝 안내 생략) -->
# 독립 검토: PR #19 (01-06) 재검토 — head f873fa4

- 검토자 역할: Guardrail(독립 검토). 작성에 참여하지 않았다. 01-06 검토 2회차(상한 4)
- 대상: `task/01-06-lab` f873fa4(f873fa47f9dc…). 직전 검토는 b301843 approve였다(review-pr19-b301843.md). 범위는 `b301843..f873fa4` 차이와 자동 검토(codex) 지적 3건(4548522731 P1, …738 P2, …750 P2)이다.
- 수행 방식:
  - `git archive f873fa4` 트리를 스크래치에 풀어 썼다.
  - `inventory`·`verify`를 실행했다. `check-init`은 증거 사본에만 실행했다(이 명령은 대상 폴더에 `init-check.json`을 쓰므로 저장소 증거 폴더에는 실행하지 않았다).
  - reset·init·archive는 실행하지 않았다. 그 거부 경로는 코드를 읽어 확인했다.
  - 모델 요청은 0회다. 저장소·GitHub·systemd·podman·시험실에 쓰지 않았다.
- 검토 시각: 2026-11-12

VERDICT: approve

자동 검토의 P1·P2 3건은 모두 교정되었다. 새 P1·P2는 없다. 비차단 P3는 4건이다.

## 1. 자동 검토 지적별 확인

### P1(4548522731): 하위 probe가 시험실 밖으로 갈 수 있음 → 교정됨
- `child_env()`(lab.py)가 `IEO_PROBE_BASE=/opt/rust/ieo_poc_probe/roles`, `IEO_PROBE_SECRETS=/opt/rust/ieo_poc_secrets`로 고정하고, 다른 상속값은 거부한다. init의 세 단계 모두 이 환경으로 실행한다(`sh(cmd, env=env)`).
- 직접 시험(모듈을 import해 `child_env()`만 호출, 부수 효과 없음):

  | 상속 환경 | 결과 |
  | --- | --- |
  | 없음 | 고정값 |
  | `IEO_PROBE_BASE=/tmp/x` | 거부 |
  | `IEO_PROBE_SECRETS=/tmp/y` | 거부 |
  | 같은 값 | 허용 |
  | 끝에 `/`가 붙은 값 | 거부(보수적) |

- probe.py·probe2.py가 읽는 환경 변수는 이 둘뿐이다(`grep environ`). probe2의 `ROOT = P.BASE.parent`도 따라서 고정된다.

### P2(4548522738): 상대 경로 `--dest` → 교정됨
- `Path(a.dest).expanduser().resolve()`로 한 번 풀고, mkdir·tar·해시에 같은 절대 경로를 쓴다. 셸에 넣는 tar 경로·LAB·`--exclude`는 `shlex.quote`로 감싼다(`present`는 상수 목록).
- 저장소 안 목적지는 거부한다(`dest == REPO or REPO in dest.parents`). 시험실 안 목적지 거부도 유지된다.
- 개발 측 증거 `lab-guard-checks.txt`: 다른 cwd에서 상대 경로로 보관한 결과가 287항목, sha 일치다.
  - 그 시험 보관본(스크래치 `reltest`)은 지금 남아 있지 않다. `find /tmp/claude-1001 /opt/rust -name lab-state.tar.gz` 결과는 정식 보관본 하나뿐이다.

### P2(4548522750): init이 E/E1b 결과를 보지 않음 → 교정됨
- `check_init()`은 다음을 판정한다.
  - `pg_init_rc == 0`
  - 23개 시험마다 필수 표식과 금지 표식
  - E1b-main-unchanged가 `seed_main`과 같은지
  - 시험 누락과 예상 밖 시험
- init은 마지막에 이를 실행하고, 하나라도 어긋나면 비0으로 끝난다.
- 커밋된 init-1·init-2 증거(E.json·E1b.json) 사본에 `check-init`을 실행했다. 둘 다 `all_ok`, rc 0이고, 내가 만든 `init-check.json`이 커밋본과 JSON 단위로 같다.
- **변이 시험(직접 작성)**: 23개 시험 각각에서 출력을 비우면 모두 검출됐다. 거부 시험 11개에 금지 표식(`PUSH_MAIN_OK`, `WRITE_OK`, `REACH_OK`, `VISIBLE /srv/a.git` 등)을 덧붙여도 모두 검출됐다(미검출 0). 아래 경우도 모두 검출됐다.
  - E3-no-token 응답이 201인 경우
  - `X-Stored-Count: 2`인 경우
  - main SHA가 바뀐 경우
  - 시험 하나가 빠진 경우
  - `pg_init_rc=None`인 경우
- 개발 측 음성 대조 2건(pg rc 3, main push 성공 표식)도 실패로 판정된다(`lab-guard-checks.txt`).

### 처분 댓글
- 세 댓글(4548575056/…322/…561)은 문구가 같다. 모두 “기존 실행은 이 변수를 설정하지 않은 환경에서 했고”라고 적었는데, 이 문장은 P1에만 맞는다.
- 두 P2에 대한 실제 근거는 다르다. 기존 실행은 절대 경로 dest를 썼고, init-1·2는 사후 check-init을 통과했다. 이 근거는 REPORT §4에 정확히 적혀 있으므로 저장소 산출물 문제는 아니다.

## 2. 이전 검토(b301843) P3 처리 확인

| 이전 P3 | 처리 | 확인 |
| --- | --- | --- |
| P3-1 “2회 보관” 표현 | REPORT.md:15를 “보관 1회 + 정리→초기화→대조 2회”로 고침 | 맞음 |
| P3-2 기록 위치 표 필드 | Lab_Procedure 표에 형식이 생긴 시점(PR #16 이후)과 01-04 부재(result.json, result_missing/judgeable, markers)를 명시 | 01-04 실제 파일과 맞음 |
| P3-3 자격 제외 | `*credentials*` 제외와 사후 목록 검사 `credentials` 추가 | 맞음 |
| P3-4 저장소 안 dest | 거부 추가 | 맞음. 단 현재 checkout만 해당(P3-A) |
| P3-5 환경 변수·reset 순서 | child_env, `--archived` sha256 확인 / `--no-archive-needed` | 맞음. 단 MANIFEST 신선도는 미확인(P3-B) |
| P3-6 항상 참 검사 | 제거 | 현재 verify는 24개 검사, 현 inventory·inv-2·inv-3 모두 all_ok |
| P3-7 오래된 참조·CI | ASSETS.md에 정리 뒤 상태와 보관본 sha 기재, REPORT §5에 01-04 재현 명령 한계 기재, CI 단계 추가 | 맞음 |

## 3. 그 밖의 확인
- **CI(f873fa4)**: run 43191496035(pull_request, headSha f873fa47…)가 success다. 새 단계 “render_progress.py behaviour tests (01-06, temporary copies)”가 success이고, 로그에 `RESULT PASS 0`이 있다. 나머지 job(rust fmt/clippy/test, toolchain pin)도 success다.
- **로컬(f873fa4 트리)**:
  - `test_render_progress.py`: RESULT PASS 0
  - `render_progress.py --check`: ok
  - `validate_package.py`: rc 0
  - `validate_contracts.py`: rc 0
  - render_progress.py sha256은 `6f2ac232…`로 b301843과 같다. Amendment 항목은 그대로 유효하다.
- **현재 시험실**: f873fa4의 `inventory`는 `at`을 빼면 inv-3과 모든 필드가 같고, `verify`는 all_ok다.
- **위생**:
  - 새·변경 파일에서 토큰 2개·E 비밀 5개의 실제 값과 일치하는 것은 0건이다. 대상 파일은 init-check.json ×2, lab-guard-checks.txt, REPORT, Lab_Procedure, lab.py, ASSETS.md다.
  - 호스트 oauthAccount 식별 필드(email·UUID·이름·조직명)와 일치하는 것도 0건이다. ASSETS.md에 있는 `organizationRole` 값은 다섯 글자 일반 단어이고 b301843에도 있었다.
  - `lab-guard-checks.txt`의 경로는 `/tmp/x`와 `<scratchpad>`로 가려져 있다.

## 4. 발견 사항

### P3-A 저장소 안 보관 거부가 lab.py를 실행한 checkout에만 걸린다
- 위치: `probes/lab/lab.py:98`
- 근거: `REPO = Path(__file__).resolve().parents[2]`다. `git worktree list`를 보면 공유 작업공간 `/opt/rust/src/github.com/HeaInSeo/ieo_poc`(main)와 `ieo_poc-0105`·`-fix`·`-val`이 0106과 별도 경로다. 0106에서 lab.py를 실행하면서 dest를 공유 작업공간 안으로 주면 거부되지 않는다.
- 영향: 절차는 dest를 `/opt/rust/ieo_poc_archive/...`로 정한다. 실수할 때만 문제가 된다.
- 권고: `git worktree list --porcelain`의 모든 경로를 거부하거나, `/opt/rust/src` 아래를 거부한다.

### P3-B reset `--archived`가 MANIFEST의 신선도와 범위를 확인하지 않는다
- 위치: `probes/lab/lab.py:119-129`
- 근거: tar가 있고 sha256이 맞기만 하면 통과한다. 그래서 오래된 MANIFEST(예: 이번 09:13Z 보관본)를 다시 넘겨도, 그 뒤 실험이 만든 상태를 보관하지 않고 지울 수 있다. 절차는 “2의 MANIFEST”라고 적지만 코드는 이를 강제하지 않는다.
- 권고: MANIFEST `at`이 현재 시험실의 최신 mtime 이후인지 확인하거나, `names_digest`를 현재 목록과 비교한다.

### P3-C check_init의 거부 표식은 거부 원인을 보지 않는다
- 위치: `probes/lab/lab.py:161`, `:168` 등
- 근거: `PUSH_MAIN_REJECTED`는 `git push`가 어떤 이유로든 실패하면 찍힌다(probe2.py:287 `... || echo PUSH_MAIN_REJECTED`). 예를 들어 hook이 없어도 다른 원인으로 push가 실패하면 통과한다.
  - E1(repo A)에는 E1b-main-unchanged 같은 main 불변 확인이 없다.
  - 실제 출력에는 `pre-receive hook declined`, `permission denied for table events`가 있으므로 원인 문자열을 필수 표식에 넣을 수 있다.
- 영향: 01-04 표식 설계를 그대로 물려받은 것이다. 이번 init-1·2 출력은 올바른 원인을 보여 준다.
- 권고(선택): 원인 문자열을 필수 표식에 추가한다.

### P3-D verify-1·2 증거는 이전 lab.py로 만들었다
- 위치: `docs/poc/reports/tasks/01-06/lab-20261112/evidence/verify-1.json`, `verify-2.json`
- 근거: 두 파일에는 검사가 25개 있고, 지금 제거된 항상 참 검사가 들어 있다. 현재 코드는 24개 검사다.
- 확인: 같은 inv-2·inv-3에 f873fa4의 verify를 다시 실행하면 둘 다 all_ok다. 결론은 바뀌지 않는다.
- 권고: REPORT §4에 “verify-1·2는 b301843 lab.py 출력”이라고 한 줄 적는다.

## 5. 종합 판정
- 자동 검토의 P1(하위 probe 범위)과 두 P2(상대 dest, init 결과 판정)가 코드로 교정됐다. 교정은 직접 시험과 변이 시험으로 확인했다.
- 이전 검토의 P3도 모두 반영됐다.
- 생성기는 b301843과 같다. CI(새 시험 단계 포함)가 성공했다. 비밀이나 식별자 유출은 없다.
- 남은 P3 4건은 방어 심화와 기록 보완이라 수용을 막지 않는다.

**01-06 수용 승인(approve)**: f873fa4 기준.
