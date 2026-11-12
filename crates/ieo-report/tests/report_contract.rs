//! 01-08 conformance. Fixtures are the package's role examples
//! (`docs/poc/reports/contract_examples`), mutated per case.

use ieo_report::{
    AgentCredential, ClaimDenied, Notification, ReviewCurrency, authorize_claim, notification,
    render_human, review_currency, validate_report,
};
use serde_json::{Value, json};

fn ex(name: &str) -> Value {
    let src = match name {
        "development_progress" => {
            include_str!("../../../docs/poc/reports/contract_examples/development_progress.json")
        }
        "guardrail_review" => {
            include_str!("../../../docs/poc/reports/contract_examples/guardrail_review.json")
        }
        "coordinator_verdict" => {
            include_str!("../../../docs/poc/reports/contract_examples/coordinator_verdict.json")
        }
        _ => include_str!("../../../docs/poc/reports/contract_examples/decision_request.json"),
    };
    serde_json::from_str(src).unwrap_or(Value::Null)
}

fn invalid(r: &Value) -> bool {
    !validate_report(r).is_empty()
}

#[test]
fn four_role_examples_are_valid() {
    for n in [
        "development_progress",
        "guardrail_review",
        "coordinator_verdict",
        "decision_request",
    ] {
        let errs = validate_report(&ex(n));
        assert!(errs.is_empty(), "{n}: {errs:?}");
    }
}

// --- the 12 invalid cases of scripts/validate_contracts.py, same names ---

#[test]
fn old_actor_string() {
    let mut r = ex("development_progress");
    r["actor"] = json!("developer");
    assert!(invalid(&r));
}

#[test]
fn missing_contract_revision() {
    let mut r = ex("development_progress");
    if let Some(m) = r.as_object_mut() {
        m.remove("contract_revision");
    }
    assert!(invalid(&r));
}

#[test]
fn developer_claims_verified() {
    let mut r = ex("development_progress");
    r["evidence_level"] = json!("verified");
    assert!(invalid(&r));
}

#[test]
fn wrong_actor_role() {
    let mut r = ex("guardrail_review");
    r["actor"]["role"] = json!("development");
    assert!(invalid(&r));
}

#[test]
fn finding_lacks_original_log() {
    let mut r = ex("guardrail_review");
    if let Some(m) = r["body"]["findings"][0].as_object_mut() {
        m.remove("original_log_ref");
    }
    assert!(invalid(&r));
}

#[test]
fn reject_without_findings() {
    let mut r = ex("guardrail_review");
    r["body"]["findings"] = json!([]);
    assert!(invalid(&r));
}

#[test]
fn estimated_without_measurement() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["forecast"]["state"] = json!("estimated");
    r["schedule"]["forecast"]["completion_range"] = json!({"earliest_at": "2026-11-13T00:00:00+09:00", "latest_at": "2026-11-14T00:00:00+09:00"});
    assert!(invalid(&r));
}

#[test]
fn reversed_forecast() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["forecast"] = json!({"state": "estimated",
        "completion_range": {"earliest_at": "2026-11-14T00:00:00+09:00", "latest_at": "2026-11-13T00:00:00+09:00"},
        "evidence": ["fixture://past-measurement"], "reason": null});
    assert!(invalid(&r));
}

#[test]
fn delay_without_reason() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["deadline_state"] = json!("set");
    r["schedule"]["deadline_at"] = json!("2026-11-13T00:00:00+09:00");
    r["schedule"]["status"] = json!("at_risk");
    assert!(invalid(&r));
}

#[test]
fn undecided_shown_normal() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["status"] = json!("on_track");
    assert!(invalid(&r));
}

#[test]
fn decision_mismatch() {
    let mut r = ex("decision_request");
    r["decision"]["question"] = json!("different");
    assert!(invalid(&r));
}

#[test]
fn decision_deadline_is_not_a_date() {
    let mut r = ex("decision_request");
    r["decision"]["deadline_at"] = json!("not-a-date");
    r["body"]["deadline_at"] = json!("not-a-date");
    r["body"]["no_deadline_reason"] = Value::Null;
    assert!(invalid(&r));
}

// --- additional semantic rules ---

#[test]
fn late_before_deadline_rejected() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["deadline_state"] = json!("set");
    r["schedule"]["deadline_at"] = json!("2026-11-30T00:00:00+09:00");
    r["schedule"]["status"] = json!("late");
    r["schedule"]["delay_reasons"] = json!(["설명용"]);
    r["schedule"]["affected_tasks"] = json!(["FIXTURE-TASK"]);
    assert!(invalid(&r));
}

#[test]
fn recommendation_must_be_an_option() {
    let mut r = ex("decision_request");
    r["body"]["recommendation"] = json!("없는 선택");
    r["decision"]["recommendation"] = json!("없는 선택");
    assert!(invalid(&r));
}

// --- claim authorization (POST /v0/agent/reports → 403) ---

fn dev() -> AgentCredential {
    AgentCredential {
        repo_id: "fixture-repo".into(),
        role: "development".into(),
    }
}
fn guard() -> AgentCredential {
    AgentCredential {
        repo_id: "fixture-repo".into(),
        role: "guardrail".into(),
    }
}

#[test]
fn claim_authorization() {
    assert_eq!(authorize_claim(&ex("development_progress"), &dev()), Ok(()));
    assert_eq!(authorize_claim(&ex("guardrail_review"), &guard()), Ok(()));
    // developer credential posting a guardrail review
    assert_eq!(
        authorize_claim(&ex("guardrail_review"), &dev()),
        Err(ClaimDenied::RoleMismatch)
    );
    // agent credential posting a coordinator verdict (verified)
    assert_eq!(
        authorize_claim(&ex("coordinator_verdict"), &guard()),
        Err(ClaimDenied::VerifiedFromAgentDenied)
    );
    // other repo
    let other = AgentCredential {
        repo_id: "other".into(),
        role: "development".into(),
    };
    assert_eq!(
        authorize_claim(&ex("development_progress"), &other),
        Err(ClaimDenied::RepoScopeDenied)
    );
    // body actor name is not trusted: renaming does not change the outcome
    let mut r = ex("guardrail_review");
    r["actor"]["name"] = json!("development-agent");
    assert_eq!(authorize_claim(&r, &dev()), Err(ClaimDenied::RoleMismatch));
}

// --- design log test inputs (4) ---

fn verdict_waiting(reason: &str) -> Value {
    let mut r = ex("coordinator_verdict");
    r["operational_state"] = json!("작업을 보유한 대기");
    r["reason_code"] = json!(reason);
    r["next_check_at"] = json!("2026-11-12T12:00:00+09:00");
    r["unknown_facts"] = json!([]);
    r
}

/// Input 1: CI passed on the current commit, independent approval only on an older commit.
#[test]
fn input1_current_ci_with_old_sha_approval_needs_current_review() {
    let (old, head) = ("a".repeat(40), "b".repeat(40));
    let cur = review_currency(&head, &[(old.clone(), "approve".into())]);
    assert_eq!(
        cur,
        ReviewCurrency::ReviewNeededOnCurrentSha {
            stale_approvals: vec![old.clone()]
        }
    );

    let mut r = verdict_waiting("REVIEW_PENDING");
    r["phase"] = json!("독립 검토");
    r["evidence"] = json!([
        format!("ci:run-1@{head}:success"),
        format!("review:approve@{old}")
    ]);
    r["next_action"] = json!(format!(
        "현재 SHA {head}에 독립 검토 재배정. CI 통과까지만 확인됨"
    ));
    r["next_owner"] = json!("Guardrail");
    assert!(validate_report(&r).is_empty(), "{:?}", validate_report(&r));
    let text = render_human(&r);
    assert!(
        text.contains("REVIEW_PENDING") && text.contains("CI 통과까지만") && text.contains(&head)
    );
    assert!(
        !text.contains("/완료"),
        "must not read as completed:\n{text}"
    );
}

/// Input 2: response to an external request lost; original result unknown.
#[test]
fn input2_lost_response_is_held_without_user_resend() {
    let mut r = verdict_waiting("EXTERNAL_RECONCILIATION");
    r["unknown_facts"] = json!(["검토 인계 요청의 외부 반영 여부 미확인"]);
    r["next_action"] = json!(
        "원래 요청 결과를 대조한 뒤 인계 재개 여부 판단. 그때까지 같은 요청을 다시 보내지 않음"
    );
    r["next_owner"] = json!("코디네이터");
    r["resume_condition"] = json!("원 요청 ID의 외부 결과 확인");
    assert!(validate_report(&r).is_empty());
    let text = render_human(&r);
    assert!(text.contains("사용자 결정 불필요"));
    assert!(text.contains("외부 반영 여부 미확인"));
    assert!(text.contains("다시 보내지 않음"));
    assert!(text.contains("원 요청 ID의 외부 결과 확인"));
}

/// Input 3: approved schedule cannot hold; scope/schedule decision needed.
#[test]
fn input3_decision_request_is_actionable() {
    let mut r = ex("decision_request");
    let opts = json!([{"label": "범위 축소", "impact": "07-03 이월, 기한 유지"},
                      {"label": "기한 연장", "impact": "범위 유지, 기한 +3일"}]);
    r["body"]["question"] = json!("범위를 줄일까, 기한을 늘릴까?");
    r["body"]["options"] = opts;
    r["body"]["recommendation"] = json!("기한 연장");
    r["body"]["rationale"] = json!("남은 작업이 모두 필수 기능");
    r["body"]["deadline_at"] = json!("2026-11-14T18:00:00+09:00");
    r["body"]["no_deadline_reason"] = Value::Null;
    r["body"]["default_action"] = json!("응답 전까지 범위·기한 유지, 07-03 착수 보류");
    r["decision"] = json!({"required": true, "question": r["body"]["question"], "recommendation": "기한 연장",
        "deadline_at": "2026-11-14T18:00:00+09:00", "default_action": r["body"]["default_action"],
        "options": ["범위 축소: 07-03 이월, 기한 유지", "기한 연장: 범위 유지, 기한 +3일"]});
    let errs = validate_report(&r);
    assert!(errs.is_empty(), "{errs:?}");
    let text = render_human(&r);
    for needle in [
        "결정 필요",
        "범위를 줄일까",
        "범위 축소: 07-03 이월",
        "기한 연장: 범위 유지",
        "권고: 기한 연장 — 남은 작업이 모두 필수 기능",
        "결정 시한: 2026-11-14T18:00:00+09:00",
        "응답이 없을 때 기본 조치: 응답 전까지 범위·기한 유지",
    ] {
        assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    }
}

/// Input 4: same event observed again without new evidence or impact → no extra notification.
#[test]
fn input4_reobservation_is_suppressed() {
    let first = verdict_waiting("CI_RUNNING");
    assert!(matches!(
        notification(None, &first),
        Notification::Notify(_)
    ));

    let mut again = first.clone();
    again["event_revision"] = json!(2);
    again["observed_at"] = json!("2026-11-12T00:30:00+09:00");
    again["next_check_at"] = json!("2026-11-12T13:00:00+09:00");
    assert_eq!(notification(Some(&first), &again), Notification::Suppress);

    let mut evid = again.clone();
    evid["evidence"] = json!(["ci:run-2:failure"]);
    assert_eq!(
        notification(Some(&first), &evid),
        Notification::Notify(vec!["new_evidence"])
    );

    let mut dec = again.clone();
    dec["decision"]["required"] = json!(true);
    assert!(
        matches!(notification(Some(&first), &dec), Notification::Notify(w) if w.contains(&"decision_newly_required"))
    );

    let mut resume = again;
    resume["resume_condition"] = json!("main CI 성공");
    assert_eq!(
        notification(Some(&first), &resume),
        Notification::Notify(vec!["resume_condition"])
    );
}

#[test]
fn render_has_five_parts_in_order() {
    let text = render_human(&ex("guardrail_review"));
    let idx: Vec<usize> = [
        "**상태 변경",
        "확인한 사실과 영향",
        "다음 행동",
        "결정 요청",
        "확인 근거",
    ]
    .iter()
    .filter_map(|h| text.find(h))
    .collect();
    assert_eq!(idx.len(), 5, "{text}");
    assert!(idx.windows(2).all(|w| w[0] < w[1]), "{text}");
    assert!(text.contains("다음 확인 시각(해결 예상 시각 아님)"));
    assert!(text.contains("대상 SHA: aaaaaaaa"));
}
