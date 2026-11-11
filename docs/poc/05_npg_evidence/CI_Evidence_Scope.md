# NPG 문서·CI 증거 범위

수집일: 2026-11-10T18:39:42+09:00

- 고정 소스: `e5075affbc87ed650a755558b8491828e6783862`
- 원본 실행: [Notion projection guard CI](https://github.com/HeaInSeo/notion-projection-guard/actions/runs/41469168311)
- 실행 상태: `completed / success`
- 실행 생성 시각: 2026-11-02T15:11:40Z

| 실행 | Node 테스트 수 | Node 실패 | Python | 원본 로그 |
| --- | --- | --- | --- | --- |
| Node 22 tests and source provenance | 1087 + 511 + 134 = 1732 | 0 | 17 tests · OK | [Node22_Job.log](Node22_Job.log) |
| Node 24 tests and source provenance | 1087 + 511 + 134 = 1732 | 0 | 17 tests · OK | [Node24_Job.log](Node24_Job.log) |

이는 GitHub에 남은 기존 실행 기록이며, 이번에 새로 실행한 테스트가 아니다. 같은 테스트를 Node 22와 24에서 반복한 것이므로 3464개의 서로 다른 테스트라고 집계하지 않는다. CI 성공은 현재 서버 설치 SHA·소유권·미확정 외부 요청·중앙 자동화 정상 동작의 증거가 아니다. 해당 운영 상태는 별도 확인이 필요하다. PoC P21 기준선 실험이나 P23 거짓 성공 방지 실험의 완료 증거로 대체하지 않는다.

NPG 안내서와 과거 검증 문서는 문서 내부의 날짜·기준 소스를 따라 읽는다. 모든 문서가 현재 운영 실측을 뜻하지는 않는다.
