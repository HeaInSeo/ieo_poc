# 작업·실험 결과 보고

아직 실제 실행 보고는 없다. `templates/`는 작성 양식이며 결과 증거가 아니다.

실제 회차는 `tasks/<task_id>/<run_id>/REPORT.md` 또는 `experiments/<experiment_id>/<run_id>/REPORT.md`와 같은 폴더의 `result.json`, `evidence/`에 저장한다. 작업 보고는 해당 실험 보고를 링크해 로그/비용을 중복하지 않는다. 임의 외부 로그를 그대로 공개하지 않고 비밀 값을 제외한 증거를 연결한다.

저장 뒤 [진행 원장과 결과 인덱스](../progress/STATUS.md)를 갱신하고 같은 개발 대화에 링크와 요약을 남긴다. 경로는 실제 repo에서 `docs/poc/reports/`다. 이미 완료한 회차 보고를 덮어쓰지 않는다.

구조화 결과의 시작 양식은 [result.template.json](templates/result.template.json)이다. is_template=true인 파일은 결과 인덱스의 실제 회차로 등록하지 않는다. 실제 회차는 run_id·관측값·증거와 함께 is_template=false로 작성한다. 원장 파일 참조는 docs/poc 루트 기준이며 progress의 링크는 ../reports/...로 만든다.
