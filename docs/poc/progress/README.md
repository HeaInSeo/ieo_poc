# 진행 원장과 생성 화면

[STATUS.md](STATUS.md)부터 읽는다. 주계획 73·후속 12를 따로 표시하며 합산 진행률은 없다. TASKS는 양쪽 작업 전수, RESULTS는 주계획 31개 실험·후속 5개 후보의 모든 회차를 연결한다.

진행 상태의 유일한 원본은 state.json이다. docs/poc 루트에서 `python3 scripts/render_progress.py`를 실행하면 STATUS/TASKS/RESULTS와 원장 events/resume에서 파생한 events.jsonl/RESUME.md가 만들어진다. 화면을 손편집하지 않는다. `--check`로 재생성 예상값과 일치 여부를 확인한다. 다음 의존 작업 착수 전·제어를 돌려받는 지점마다 실행한다. 원시 증거는 reports/에 불변 참조로 보존한다.

생성 중단은 원장으로 재생성한다. 서로 다른 snapshot_id 또는 원장 SHA-256을 최신 전체 완료로 합치지 않는다. 생성 성공은 실험/독립 검수 성공이 아니다. 현재 실제 작업·probe·실험은 미실행이다.
