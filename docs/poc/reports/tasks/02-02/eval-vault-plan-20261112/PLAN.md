# 02-02 · 비공개 평가 세트 봉인 계정 계획 (계획 문서, 미적용)

| 항목 | 기록 |
| --- | --- |
| 결정 근거 | 사용자 결정(2026-11-12): 방안 1 “별도 계정/호스트” 채택, 우선 현재 서버의 별도 검증용 리눅스 계정. 02-02 수용 기준 유지 |
| 상태 | **계획.** 계정은 아직 없다. 아래 §1만 실제 확인 결과이고 §3~§5는 계획이다 |
| 완료 수 | 환경 준비이며 PoC 완료 수에 넣지 않는다 |

## 1. 현재 서버 읽기 전용 확인 (실제 결과, [host-check.txt](evidence/host-check.txt))

| 확인 | 결과 | 봉인에 대한 뜻 |
| --- | --- | --- |
| 개발 계정 | `heain`(uid 1001), **wheel 그룹** | — |
| sudo | `(ALL) ALL`(비밀번호 필요, `sudo -n true` 실패) + **`(ALL) SETENV: NOPASSWD: /usr/local/bin/kind`** | **차단 조건.** `kind`를 root로 비밀번호 없이 실행할 수 있으면 root 권한 컨테이너로 호스트 경로를 마운트할 수 있다. 이 규칙이 있는 동안 이 서버의 어떤 별도 계정도 개발 에이전트(heain)로부터 봉인됐다고 판정하지 않는다. 실제 우회는 시도하지 않았다(규칙 판독만) |
| 비밀번호 sudo | 에이전트는 비밀번호를 모른다. 셸에 tty가 없어 사용자의 tty 시각 기록을 재사용하지 못하는 것으로 본다(RHEL 기본 `timestamp_type=tty`, 설정 파일은 읽을 수 없어 미확인) | 조건부 경계. 사용자가 이 Claude 세션의 `!` 명령으로 sudo를 쓰지 않는다는 운용 조건이 필요 |
| 계정 출처 | `files sss`, sssd 비활성 → 로컬 계정 생성 가능(root 필요) | — |
| 기존 계정 | ghrunner(1000), heain(1001) | 새 계정은 두 계정의 그룹에 넣지 않는다 |
| 기본 권한 | `HOME_MODE 0700`, `UMASK 022` | 보관 계정 셸에서 `umask 077`을 쓴다 |
| 디스크 | `/home` 98%(여유 749M), `/` 90%(7.2G) | 보관소는 `/srv/ieo-eval`(루트 파일 시스템) |
| `/proc` | hidepid 없음, `ptrace_scope` 0 | 다른 UID의 environ·메모리는 읽을 수 없지만 **명령줄은 보인다** → 정답·경로·자격을 argv에 넣지 않는다 |
| SELinux | Enforcing | 보관 경로 라벨은 생성 뒤 확인 |
| 컨테이너 | rootless podman 4.9.4, subuid는 ghrunner·heain만 | 보관 계정용 subuid 범위가 필요(root) |
| sshd | 활성(설정 파일 읽기 불가) | 보관 계정은 비밀번호 잠금·키 없음으로 원격 로그인 불가로 둔다 |
| 01-03 기록 정정 | 01-03 보고는 “sudo는 없고”라고 적었다. 사실과 다르다(비밀번호 sudo와 kind NOPASSWD 있음) | 01-04 P2 격리 판정은 컨테이너 안 역할 UID 사이의 판정이라 영향 없음. 원장에 정정 기록 |

**결론.** 현재 서버는 **kind NOPASSWD 규칙을 heain에서 제거하거나 다른 관리 계정으로 옮긴 뒤에만** 별도 계정 봉인이 가능하다. 그렇게 하지 않으면 별도 호스트(§6)가 필요하다.

## 2. 위협 모형(개발 에이전트 = heain으로 실행되는 모든 프로세스)

막아야 할 것: 보관소 파일 읽기, 보관 계정으로 전환(su·sudo·ssh), 보관 계정 프로세스의 환경·메모리 읽기, 명령줄·임시 파일·출력 경로로의 유출, 후보 프로그램 실행 중 정답·채점기·보관 자격 접근, 사용자가 이 세션에 붙여 넣는 출력으로의 유출. 막지 않는 것(범위 밖): root 침해, 사용자 본인의 실수 공개.

## 3. 계정·경로 설계(계획)

| 대상 | 값 | 이유 |
| --- | --- | --- |
| 보관·채점 계정 | `ieoeval`(새 UID, 개인 그룹 `ieoeval`, wheel·heain·ghrunner 그룹 미가입, sudo 권한 없음) | heain과 다른 UID |
| 로그인 | 비밀번호 잠금(`passwd -l`), `~/.ssh` 없음. 사용자만 **별도 터미널**에서 `sudo -iu ieoeval`(사용자 비밀번호)로 들어간다 | 에이전트는 계정 전환 수단이 없다 |
| 봉인 보관소 | `/srv/ieo-eval/vault`(소유 ieoeval, 0700), 작업 `/srv/ieo-eval/work`(0700) | 개발 저장소·Git worktree 밖 |
| 공개 메타데이터 | `/srv/ieo-eval-pub`(소유 ieoeval, 0755): 세트 버전, 사례 수, 묶음 digest, 채점 결과 요약만 | 개발 쪽이 읽을 최소 정보 |
| 후보 입력함 | `/srv/ieo-eval-inbox`(소유 heain, 0755): 후보 바이너리와 그 SHA-256 | 보관 계정이 복사하고 digest 대조 |
| 후보 실행 | ieoeval의 rootless podman 컨테이너: `--network none --read-only`, 시험 입력 디렉터리만 읽기 전용 마운트, 출력 디렉터리 하나만 쓰기, 환경 비움, 보관소·홈·자격 미마운트 | 후보는 입력만 받는다. 비교는 컨테이너 밖 채점 경로에서 |
| 독립 담당 | ieoeval 계정에서 사용자가 시작한 별도 Claude 세션(자격은 사용자가 그 계정에 별도 로그인, heain의 자격·설정 공유 안 함). 개발 맥락 없이 승인된 요구사항·판정 규칙만 받는다 | L03·D10 |

## 4. 사용자가 실행할 명령(root, 계획)

```bash
# 0) 차단 조건 해소: heain의 kind NOPASSWD 규칙 제거 또는 다른 계정으로 이동 (visudo로 해당 줄 편집)
sudo visudo -f <kind 규칙이 있는 파일>      # '(ALL) SETENV: NOPASSWD: /usr/local/bin/kind' 행 처리
sudo -l -U heain                            # NOPASSWD 항목이 없어야 함

# 1) 계정
sudo useradd -m -U -s /bin/bash ieoeval
sudo passwd -l ieoeval
sudo usermod --add-subuids 231072-296607 --add-subgids 231072-296607 ieoeval
sudo loginctl enable-linger ieoeval          # rootless podman 사용 시

# 2) 경로
sudo install -d -o ieoeval -g ieoeval -m 0700 /srv/ieo-eval /srv/ieo-eval/vault /srv/ieo-eval/work
sudo install -d -o ieoeval -g ieoeval -m 0755 /srv/ieo-eval-pub
sudo install -d -o heain   -g heain   -m 0755 /srv/ieo-eval-inbox
ls -ldZ /srv/ieo-eval /srv/ieo-eval/vault /srv/ieo-eval-pub /srv/ieo-eval-inbox
```

운용 조건: 보관 계정 작업은 **이 Claude 세션이 아닌 별도 터미널**에서만 하고(`!` 명령 금지), 보관 계정 출력은 이 세션에 붙여 넣지 않는다. 공개할 것은 `/srv/ieo-eval-pub`에만 쓴다.

## 5. 검증 절차(계획 — 결과는 적용 뒤 별도 기록)

| # | 확인 | 수행 주체 | 기대 |
| --- | --- | --- | --- |
| V1 | 개발 계정 접근 거부: `ls`/`cat` `/srv/ieo-eval{,/vault}`·`/home/ieoeval`, `su ieoeval`, `sudo -n -u ieoeval true`, `sudo -n -l`에 NOPASSWD 없음, ieoeval 프로세스의 `/proc/<pid>/environ` | 개발 에이전트(heain) | 모두 거부. 표식 파일(사용자가 vault에 둔 무작위 이름 파일)의 내용·이름을 얻지 못함 |
| V2 | 보관 계정 정상 접근: vault 쓰기·읽기, `umask 077`, 공개 디렉터리에 digest 쓰기 | 사용자(별도 터미널, ieoeval) | 성공. 결과는 `/srv/ieo-eval-pub/vault-check.json`(digest·시각만) |
| V3 | 후보의 정답 접근 거부: 후보 컨테이너 안에서 `/srv/ieo-eval`, `/home/ieoeval`, 환경 변수, 네트워크를 시도 | ieoeval이 실행, 결과는 공개 디렉터리 | 경로 없음/거부, 환경 비어 있음, 네트워크 없음. 입력·출력 디렉터리만 보임 |
| V4 | 공개 메타데이터 최소성: 공개 디렉터리·저장소에 사례 내용·기대값·정답을 추정할 이름이 없음 | 독립 검토 | 버전·수·digest만 |

## 6. 별도 호스트 대안(현재 서버가 불가할 때)

kind 규칙을 유지해야 하면: heain이 root·sudo를 갖지 않는 다른 호스트(예: 사용자가 관리하는 VM)에 `ieoeval`을 두고 같은 §3~§5를 적용한다. 후보 바이너리는 공개 저장소의 고정 커밋에서 그 호스트가 직접 빌드하거나 CI 산출물을 SHA-256으로 받는다. 개발 쪽으로는 공개 메타데이터만 돌아온다.

## 7. 02-02 진행 방식

- 봉인·접근 경계 확인(V1~V4) 전: 공개·작성·validation 세트, 수집기·판정기 외부 대조, 02-01 이월 교정, 누출 검사 도구를 진행한다. 비공개 세트 관련 항목은 **조건 대기**.
- 비공개 세트: 승인된 명세·판정 규칙으로 기대 결과를 미리 확정할 수 있는 사례만. 기대값마다 근거(명세 조항·독립 계산·검산). 정답이 불명확한 사례는 미확정으로 분리하고 제외 수·이유 기록. 첫 평가 실행 전에 버전·digest 동결. 통과를 개발 전체의 정확성 보장으로 해석하지 않는다.
- 02-02 검수 완료는 기존 수용 기준 전부와 V1~V4 확인 뒤에만.
