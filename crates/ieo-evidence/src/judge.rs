//! Judgment of one run. Exit code 0 alone is never success.

use crate::record::{
    ArtifactCheck, Expected, Judgment, JudgmentStatus, MarkerCheck, ProcessKind, ProcessOutcome,
    ResultEvent,
};

pub struct Inputs<'a> {
    pub expected: Expected,
    pub model_request: bool,
    pub process: &'a ProcessOutcome,
    pub result: Option<&'a ResultEvent>,
    pub artifacts: &'a [ArtifactCheck],
    pub markers: &'a [MarkerCheck],
}

pub fn judge(i: &Inputs) -> Judgment {
    let p = i.process;
    // Process-level failures, all listed.
    let mut failures = Vec::new();
    match p.kind {
        ProcessKind::LaunchFailed => failures.push(format!(
            "기동 실패: {}",
            p.launch_error.as_deref().unwrap_or("원인 미상")
        )),
        ProcessKind::TimedOut => failures.push(format!(
            "시간 초과({} ms, TERM {} · KILL {})",
            p.timeout_ms.unwrap_or(0),
            p.term_sent,
            p.kill_sent
        )),
        ProcessKind::Signaled => failures.push(format!("신호로 종료: {:?}", p.signal)),
        ProcessKind::Exited => {
            if p.exit_code != Some(0) {
                failures.push(format!("종료 코드 {:?}", p.exit_code));
            }
        }
    }
    let result_present = i.result.is_some_and(|r| r.present);
    let result_error = i.result.and_then(|r| r.is_error) == Some(true);
    if result_error {
        failures.push(format!(
            "결과 사건 is_error=true (subtype {:?})",
            i.result.and_then(|r| r.subtype.clone())
        ));
    }
    // Missing required evidence, all listed.
    let mut missing = Vec::new();
    if i.model_request && !result_present && i.expected != Expected::NoResult {
        missing.push("필수 결과 사건 없음".to_owned());
    }
    if let Some(r) = i.result
        && r.rate_limit_statuses.iter().any(|s| s != "allowed")
    {
        // Not a failure by itself, but never hidden.
        missing.push(format!("한도 신호: {:?}", r.rate_limit_statuses));
    }
    for a in i.artifacts.iter().filter(|a| !a.present) {
        missing.push(format!("필수 산출물 없음: {}", a.path));
    }
    for m in i.markers.iter().filter(|m| !m.found) {
        missing.push(format!("필수 표식 없음: /{}/", m.pattern));
    }
    if p.output_incomplete {
        missing.push(
            "출력 관이 수집 종료 뒤에도 열려 있음(그룹 밖 자손): 저장된 출력이 잘렸을 수 있음"
                .to_owned(),
        );
    }
    if p.stray_group_killed {
        // Recorded, not blocking: the command itself finished.
        missing.push("명령 뒤에 남은 프로세스 그룹 구성원을 KILL함".to_owned());
    }
    // A rate-limit line is informational; it must not turn success into incomplete.
    let blocking_missing: Vec<&String> = missing
        .iter()
        .filter(|m| !m.starts_with("한도 신호") && !m.starts_with("명령 뒤에 남은"))
        .collect();

    let (status, extra) = match i.expected {
        Expected::Success => {
            if !failures.is_empty() {
                (JudgmentStatus::Failed, None)
            } else if !blocking_missing.is_empty() {
                (JudgmentStatus::Incomplete, None)
            } else {
                (JudgmentStatus::Succeeded, None)
            }
        }
        exp => {
            let observed = match exp {
                Expected::NonzeroExit => {
                    p.kind == ProcessKind::Exited && p.exit_code.is_some_and(|c| c != 0)
                }
                Expected::Timeout => p.kind == ProcessKind::TimedOut,
                Expected::NoResult => p.kind != ProcessKind::LaunchFailed && !result_present,
                Expected::ResultError => result_present && result_error,
                Expected::Success => unreachable!(),
            };
            let ok = observed && blocking_missing.is_empty();
            let note = format!(
                "기대 {exp:?}: 관측 {}",
                if observed { "일치" } else { "불일치" }
            );
            (
                if ok {
                    JudgmentStatus::ExpectedMatched
                } else {
                    JudgmentStatus::ExpectedNotMatched
                },
                Some(note),
            )
        }
    };
    let mut reasons: Vec<String> = extra.into_iter().collect();
    reasons.extend(failures);
    reasons.extend(missing);
    Judgment {
        expected: i.expected,
        status,
        counts_as_success: matches!(
            status,
            JudgmentStatus::Succeeded | JudgmentStatus::ExpectedMatched
        ),
        reasons,
    }
}
