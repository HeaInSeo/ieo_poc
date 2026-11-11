# Rust 툴체인 설치·고정 · rust-toolchain-20261112

2026-11-12 KST · 사용자 결정(2026-11-12 대화): heain 사용자 범위·rustup·`--no-modify-path`·minimal+rustfmt+clippy·정확한 버전 기록·`rust-toolchain.toml` 고정·디스크 확인. **작업 완료 수에 넣지 않는 환경 준비 기록.**

## 디스크 확인과 설치 위치

| 파일시스템 | 설치 전 여유 | 설치 후 여유 | 판단 |
| --- | --- | --- | --- |
| `/home` (rl-home) | 1.6 GiB (96%) | 1.6 GiB (96%) | 툴체인+레지스트리+빌드에 부족 → 사용하지 않음 |
| `/` (rl-root, `/opt/rust` 포함) | 15 GiB (78%) | 15 GiB (79%) | 사용 |

[설치 전](evidence/disk-before.txt) · [설치 후](evidence/disk-after.txt) · [고정 후](evidence/pin-1.99.1.txt)

- `RUSTUP_HOME=/opt/rust/rustup`, `CARGO_HOME=/opt/rust/cargo` (소유 heain, root 권한·시스템 변경 없음).
- `~/.rustup → /opt/rust/rustup`, `~/.cargo → /opt/rust/cargo` 심볼릭 링크로 기본 경로를 유지했다. 셸 설정 파일은 수정하지 않았다(`--no-modify-path`). 사용 시 `export PATH=/opt/rust/cargo/bin:$PATH`.
- 제안 단계의 `~/.cargo` 실저장 위치를 디스크 실측 때문에 `/opt/rust`로 바꾼 것이다. 되돌리려면 링크와 두 디렉터리를 지우면 된다.

## 설치 명령과 버전

`rustup-init.sh`(SHA-256: [rustup-init.sha256.txt](evidence/rustup-init.sha256.txt)) `-y --no-modify-path --profile minimal --default-toolchain stable -c rustfmt -c clippy` → exit 0 ([로그](evidence/rustup-install.log)).
이어서 고정 채널 `1.99.1`을 설치하고 같은 비트의 `stable` 중복을 제거했다 ([pin-1.99.1.txt](evidence/pin-1.99.1.txt)).

| 구성 | 정확한 버전 |
| --- | --- |
| rustup | 1.29.3 (edbc831ce 2026-11-02) |
| rustc | 1.99.1 (58cf8f9c3 2026-10-13) · commit 58cf8f9c35b355199727627d07ae34a4e2846d67 · LLVM 23.1.1 |
| cargo | 1.99.1 (5f94df478 2026-08-27) |
| rustfmt | 1.10.0-stable (58cf8f9c35 2026-10-13) |
| clippy | 0.1.99 (58cf8f9c35 2026-10-13) |
| 설치 구성요소 | cargo · clippy · rust-std · rustc · rustfmt (x86_64-unknown-linux-gnu) |

고정: 저장소 루트 `rust-toolchain.toml` (channel 1.99.1, profile minimal, rustfmt·clippy). CI의 `rust-toolchain` job이 같은 버전 해석을 확인한다.

## 사용량과 빌드 여유

설치 실측: rustup 617 MiB + cargo 21 MiB = 약 638 MiB.
빌드 산출물(`target/`, 프로젝트 루트·rl-root)과 cargo 레지스트리는 아직 실측 없음. 비동기 HTTP·PostgreSQL 클라이언트를 쓰는 작업공간의 debug 빌드는 통상 수 GiB 규모로 **추정**하며 15 GiB 여유 안이다. 첫 빌드 뒤 `du`로 실측해 갱신한다. `/home`은 계속 사용하지 않는다.
