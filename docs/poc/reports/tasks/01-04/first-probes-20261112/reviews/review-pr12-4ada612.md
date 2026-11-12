<!-- 보관본: PR #12 독립 재검토(Guardrail) 4ada612 원문. 게시 댓글 https://github.com/HeaInSeo/ieo_poc/pull/12#issuecomment-6817424132 -->
# PR #12 독립 재검토 (Guardrail) · 01-04 첫 세 probe · 교정 head

- 검토 SHA: `4ada61204d6226dfbc0634260f7076a70b801ee3` (branch `task/01-04-probes`)
- base: `main` `c729474`. 직전 검토: `0303f3e`(reject, [review-pr12-0303f3e.md])
- 날짜: 2026-11-12
- 범위: diff `0303f3e..4ada612`(8개 파일)
  - ADDENDUM.md(신규)
  - PROBE_PLAN.md
  - REPORT.md
  - evidence/P2.json
  - evidence/P3-identity.json
  - reviews/review-pr12-0303f3e.md
  - probes/01-04/identity_claims.py
  - probes/01-04/probe.py
- 제약: 모델 요청 0회, probe.py p1/p2/p3 미실행(예산 9/30 그대로), 저장소 변경 없음, 비밀 값 미출력(동등 비교와 해시만 사용).
- CI: head `4ada612`에서 세 job 모두 SUCCESS(gh 확인).

VERDICT: approve

직전 검토의 차단 지적 P2-1은 해소됐다. 새 P1·P2 지적은 없다. 세 판정(P1 지원, P2 성립 확인, P3 공유 확인)은 0303f3e 검토에서 재계산한 그대로이고, 이번 diff는 판정 입력(attempts/, P1.json, P3.json, budget.json)을 바꾸지 않았다.

---

## 1. P2-1 교정 확인

### 1.1 4ada612 트리에서 토큰 전체 해시가 사라졌는가 — 확인
- 4ada612 트리 전체(335개 파일)를 대조했다. 대조한 값은 다음과 같다(값 미출력).
  - 두 토큰 원문, 접두 뒤 본문, 중간 32자
  - `sha256(token)` 전체 64자와 앞 32자, `sha256(token+"\n")`
  - `sk-ant-` 패턴
  - 역할별 userID
  - JWS `sub`·`ws`·`aud` 원문(대시 포함·제외)
  - 호스트 로그인의 이메일·accountUuid·organizationUuid·organizationName
- 결과: 이 PR의 파일에서는 **모두 0건**이다.
- 줄 단위로 따로 위치를 찾아도 전체 해시는 0303f3e의 P2.json:260·:300에만 있다. 4ada612의 P2.json에는 없다.
- `git show 4ada612` diff에는 삭제된 줄(`-`)로 전체 해시가 보인다. 이것은 0303f3e 이력의 내용이며 §1.3에서 다룬다.
- P2.json 변경은 두 `G=` 값을 `<앞 12자>[sha256-truncated]`로 바꾼 2줄뿐이다. 남은 12자는 setup.json의 지문(`2456b85e9111`, `da968d393412`)과 같다. 출력·판정 필드는 바뀌지 않았다.

### 1.2 하네스 scrub가 전체 해시와 P2 argv를 덮는가 — 확인
- `probes/01-04/probe.py:71-74`: `Ledger.scrub`가 각 토큰의 `sha256(...).hexdigest()`를 `앞12자[sha256-truncated]`로 바꾼다. 토큰 원문이 있으면 지금처럼 기록을 거부한다.
- `probe.py:220`: P2 시험의 argv가 `json.loads(led.scrub(json.dumps(argv)))`로 저장 전에 scrub를 거친다.
- 시도 기록 argv는 원래 `probe.py` `record_attempt`에서 scrub를 거치고 있었다.
- 검토자 실행 확인: 4ada612의 probe.py를 scratchpad로 꺼내 실제 토큰으로 `Ledger`를 만들었다(모델 호출 없음, 증거 디렉터리는 scratchpad).
  - 전체 해시를 담은 argv → 전체 해시 제거 True, `앞12자[sha256-truncated]` 보존 True (두 역할 모두)
  - 토큰 원문을 담은 문자열 → `SECRET GUARD`로 거부 (두 역할 모두)

### 1.3 공개된 0303f3e 이력의 처리를 정직하게 적었는가 — 확인
- ADDENDUM.md §3은 다음을 명시한다.
  - 0303f3e는 이미 공개 브랜치에 push돼 이력에 전체 해시가 남는다.
  - 이력은 재작성하지 않는다(D11과 같은 원칙).
  - 위험은 "후보 토큰 확인용"으로 한정된다.
  - 토큰 교체는 자격 소유자인 사용자가 결정하고, 교체하면 이번 증거의 지문은 옛 토큰의 것으로 남는다.
- 사실과 맞고 과장·축소가 없다. "force push가 막혀 있다"는 검토자가 확인하지 않았지만 처분에는 영향이 없다.

## 2. P3-identity.json 재생성 확인

- **출처 기록**: `captured_at` 07:18:21Z, 실행 명령, 원본 3개 파일의 SHA-256 앞 12자·크기·mtime이 있다. 검토자가 지금 볼륨에서 다시 계산한 값과 6개 모두 같다.
  - 서명 파일: development `ee8a51efdf59`, guardrail `676918238bd8`
  - policy-limits.json: 두 역할 모두 `d81ac49dc74e`
  - stamp: development `a45409eb765e`, guardrail `f45f8c891566`
  - 0303f3e 검토 때 검토자가 계산한 서명 파일 해시와도 같다.
- **서명**: 두 역할 모두 `signature_by_leaf: "Verified OK"`이며, 검토자의 독립 openssl 검증 결과와 같다.
  - `chain_verify_system_ca`는 `unable to get local issuer certificate` 원문 그대로다.
  - ADDENDUM §1·REPORT §5는 이 결과의 범위를 정확히 적었다. 리프 키 서명과 무변조까지만 보이고, 공급자 루트까지의 신뢰 사슬은 확인하지 못했다.
  - 두 역할의 서명 지문, 리프 인증서 지문, `kid` 지문이 모두 다르다(`comparison.signature = different`).
- **클레임 의미 불리언**: 두 역할 모두 `sub==accountUuid` true, `aud==organizationUuid` true이고 나머지 4개는 false다. 검토자가 0303f3e 검토에서 독립적으로 얻은 동등 비교 결과와 같다.
- **누출 점검**: 출력 파일에 UUID 형식 문자열이 0건이다. 클레임은 형태와 12자 지문만 남았다.
- **비교 로직**: `comparison`은 이제 지문이 아니라 원래 클레임 값끼리 비교하고 same/different만 출력한다. 정확도가 올라갔다.

## 3. 그 밖의 교정 확인

- **PROBE_PLAN.md**: `git diff 93a44ad 4ada612 -- PROBE_PLAN.md`가 비어 있다. 93a44ad와 **바이트 동일**하다. 사후 내용은 ADDENDUM.md로 옮겼다(P3-5 해소).
- **REPORT.md**:
  - P3-1: "4쌍(8건)"으로 고쳤다.
  - P3-2: 근거를 `sub`(계정)·`aud`(org)로 바꾸고 `ws`를 제외했다.
  - P3-6 일부: "컨테이너 7건 + 호스트 3건"으로 고쳤다.
  - P3-7: 서명 검증 결과와 한계를 반영했다.
  - 계획 인용에서 "§5"를 빼고 ADDENDUM을 링크했다.
  - 판정·수치는 바뀌지 않았다.
- **보관된 직전 검토(reviews/review-pr12-0303f3e.md)**: 검토자 원본과 비교하면 첫 줄 보관 주석만 다르고 본문은 동일하다. 이 파일에도 비밀·식별 값이 없다(§1.1 대조에 포함).
- **미룬 지적**: P3-3(2d 부분 문자열 검사), P3-4(2b errno), P3-6 나머지(`budget.log` 이름, meta의 누적 수, 시도 05 표식, auth status 원문). ADDENDUM §2가 이것들을 01-04 나머지 probe 계획을 고정할 때 반영할 항목으로 기록했다. 직전 검토에서 병합 후 처리 가능으로 분류한 것과 일치한다.

## 4. 발견 사항

P1·P2 없음.

### P3-8 토큰 교체 결정을 사용자에게 실제로 전달할 것
- 위치: ADDENDUM.md §3.
- 근거: 문서는 교체 여부를 사용자 결정으로 남긴다. 저장소 문서에만 있으면 사용자가 보지 못할 수 있다. 병합 보고에 "0303f3e 공개 이력에 두 토큰의 SHA-256 전체가 남음(복원 불가, 후보 확인만 가능), 교체 여부 결정 요청"을 한 줄로 넣기를 권한다.

### P3-9 scrub가 막는 해시 형태는 하나뿐
- 위치: `probe.py:71-74`.
- 근거: 끝 공백을 제거한 토큰의 소문자 hex SHA-256만 가린다. 대문자 hex, 개행을 포함한 토큰의 해시, 다른 해시 함수는 막지 않는다. 현재 하네스가 만드는 형태는 이것뿐이라 이번 증거에는 영향이 없다.
- 01-04 나머지 probe에서는 해시를 argv 대신 컨테이너 안 읽기 전용 파일로 넘기는 방식(직전 검토 P2-1 (b)의 대안)을 함께 검토하기를 권한다.

### P3-3·P3-4·P3-6(나머지)
직전 검토의 지적을 그대로 이월한다. ADDENDUM §2에 반영 시점이 기록돼 있다.

## 5. 결론
P2-1의 (a) 증거 가림, (b) 하네스 교정, (c) 공개 이력 처분 기록을 모두 확인했다. P3-identity의 출처·서명·의미 대조가 검토자의 독립 결과와 일치하고, PROBE_PLAN.md는 93a44ad와 바이트 동일하다. head `4ada612` 기준으로 승인한다. 승인은 이 SHA에만 유효하다.
