# 작업공간 준비·기존 상태 대조 · prep-20261112

2026-11-12 KST · 계획 v1.3.3 · 작업 ID 없음(준비 기록). **73/12 분모의 어떤 작업 완료로도 세지 않는다.**

## 결론

| 항목 | 실측 | 판정 |
| --- | --- | --- |
| 작업 루트 | `/opt/rust/src/github.com/HeaInSeo/ieo_poc` (소유 heain:root, 사용자 heain uid 1001) | 확인 |
| 적용 AGENTS.md/CLAUDE.md | 작업 루트와 상위 경로에 없음. `05_npg_evidence/repo_documents/repo/AGENTS.md`는 NPG repo의 역사적 증거이며 이 작업공간의 지시가 아님 | 적용 대상 없음 |
| 기존 파일 | 루트에 ZIP 1개만 존재. 기존 작업/소스 없음 | 덮어쓴 것 없음 |
| ZIP SHA-256 | `122b031d…7e494e9` · 기대값과 일치 | [zip-sha256.txt](evidence/zip-sha256.txt) |
| 배치 | 압축 내부 최상위 `Console_Coordinator_v1_3_3_2026-11-12/`를 제거하고 `docs/poc/`에 121개 파일 배치. 원본 ZIP은 루트에 보존 | 확인 |
| 묶음 무결성 | `sha256sum -c SHA256SUMS.txt` 120/120 OK, exit 0 | [sha256sums-check.txt](evidence/sha256sums-check.txt) |
| Git | 기존 repo 없음 → 로컬 `git init -b main`, 묶음 기준선 커밋 `d92b4143fee672aead6f1260f89e1ef6fd5a0d12`. PDF 원문·ZIP·venv는 .gitignore | 로컬만 |
| GitHub 원격 | `gh` 계정 HeaInSeo 로그인(scopes repo/workflow 등). `HeaInSeo/ieo_poc` 저장소 **없음**(exit 1). 생성하지 않음 | [gh-auth-status.txt](evidence/gh-auth-status.txt) · [gh-repo-view-ieo_poc.txt](evidence/gh-repo-view-ieo_poc.txt) |
| 문서 검산 | `validate_package.py` exit 0 (73노드/188간선·순환 0·schema 오류 0·ledger_spec_match·generated_views_match, `runtime_probe: NOT_RUN`) | [stdout](evidence/validate_package.stdout.txt) · [exit](evidence/validate_package.exit.txt) |
| 화면 검산 | `render_progress.py --check` exit 0 (5개 화면 일치) | [stdout](evidence/render_check.stdout.txt) · [exit](evidence/render_check.exit.txt) |
| 검산 환경 | 시스템 python3 3.6.8 대신 프로젝트 전용 `.venv-poc` (uv, CPython 3.12.13, jsonschema 4.26.0) | 확인 |

문서/생성기 검산 PASS는 **실제 런타임 검증이 아니다.** 런타임 probe와 사람 시험은 미실행이다.

## 진척 대조

전달된 상태(01-01 검수 완료, 주계획 1/73, 후속 0/12, 01-02 런타임 선택 대기)는 `progress/state.json`(SHA-256 `69a2e82b…10591c3`, snapshot v1.3.3-EXEC-0001)과 일치한다. 01-01 `verified`·증거 2건, 01-02 `조건 대기`·`claim`, 나머지 71개 주계획·12개 후속은 미착수. 인계 문서·실행 장부의 0/73은 등록 전 기준선이며 원장을 우선한다. 01-01을 재수행하지 않았다.

## 개발 호스트 환경(관측만, 런타임 기동·로그인 없음)

| 항목 | 관측 |
| --- | --- |
| OS/자원 | Rocky Linux 8.10 · 64 vCPU · 메모리 62 GiB · `/` 여유 15 GiB(78% 사용) |
| Rust | cargo/rustc/rustup 없음 → 02 구현 전 설치 필요 ([rust-toolchain.txt](evidence/rust-toolchain.txt)) |
| 컨테이너/K8s | podman 4.9.4 · kubectl v1.35.0 client (docker 없음) |
| Claude Code CLI | `2.1.322` ([claude-version.txt](evidence/claude-version.txt)) — 버전 출력만. 세션/인증 확인 안 함 |
| Codex CLI | `codex-cli 0.166.0` ([codex-version.txt](evidence/codex-version.txt)) — 버전 출력만 |

01-02 보고의 Codex 증거는 다른(Work 로컬) 환경 값이었고, 위가 이 개발 서버의 첫 설치 관측이다. 설치 사실은 무인 권한·동시 세션·자격/한도 분리의 지원을 뜻하지 않는다(01-04 probe 대상).

## 열람 경로

STATUS.md는 서버 내부 경로로만 존재한다. 사용자가 여는 고정 링크(웹/공유 파일)는 확인하지 못했고 새 공개 채널을 만들지 않았다 → `REPORT_VISIBILITY_UNKNOWN`. 같은 개발 대화에 요약을 제공한다.

## 다음

- 01-07(선행 01-01만): 런타임 선택과 독립이므로 착수 가능 → 이후 01-08.
- 01-02: 사용자 런타임 선택 대기. 01-03은 01-02·01-07·01-08 검수 완료 뒤.
