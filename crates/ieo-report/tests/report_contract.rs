//! 01-08 conformance. Fixtures are the package's role examples
//! (`docs/poc/reports/contract_examples`), mutated per case.

use ieo_report::{
    AssignmentBook, ClaimDisposition, CompletionFacts, Denied, IssuedAssignment, Notification,
    Principal, ReproDisposition, ReviewCurrency, authorize_claim, authorize_repro,
    completion_summary, may_read_repro, may_read_sprint, notification, render_human,
    review_currency, validate_report,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

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

// --- claim authorization (POST /v0/agent/reports → 403/409) ---

fn agent(repo: &str, role: &str) -> Principal {
    Principal::Agent {
        repo_id: repo.into(),
        role: role.into(),
    }
}
fn issued(id: &str, generation: u64, sha: &str) -> IssuedAssignment {
    IssuedAssignment {
        assignment_id: id.into(),
        generation,
        target_sha: sha.into(),
    }
}
/// SHA of the current assignment = the example bodies' `target_sha`.
const CUR: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OLD0: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OLDER: &str = "cccccccccccccccccccccccccccccccccccccccc";
/// fixture-assignment gen 1 is current (SHA CUR); gen 0 of it (OLD0) and "older" (OLDER) were issued before.
fn book() -> AssignmentBook {
    AssignmentBook {
        issued: vec![
            issued("older", 1, OLDER),
            issued("fixture-assignment", 0, OLD0),
            issued("fixture-assignment", 1, CUR),
        ],
        current: Some(("fixture-assignment".into(), 1)),
    }
}
fn current() -> Result<ClaimDisposition, Denied> {
    Ok(ClaimDisposition::Current {
        target_sha: CUR.into(),
    })
}

#[test]
fn claim_authorization() {
    let (dev, guard) = (
        agent("fixture-repo", "development"),
        agent("fixture-repo", "guardrail"),
    );
    assert_eq!(
        authorize_claim(&ex("development_progress"), &dev, &book(), None),
        current()
    );
    assert_eq!(
        authorize_claim(&ex("guardrail_review"), &guard, &book(), None),
        current()
    );
    assert_eq!(
        authorize_claim(&ex("guardrail_review"), &dev, &book(), None),
        Err(Denied::RoleMismatch)
    );
    assert_eq!(
        authorize_claim(&ex("coordinator_verdict"), &guard, &book(), None),
        Err(Denied::VerifiedFromAgentDenied)
    );
    assert_eq!(
        authorize_claim(
            &ex("development_progress"),
            &agent("other", "development"),
            &book(),
            None
        ),
        Err(Denied::RepoScopeDenied)
    );
    // body actor name is not trusted
    let mut r = ex("guardrail_review");
    r["actor"]["name"] = json!("development-agent");
    assert_eq!(
        authorize_claim(&r, &dev, &book(), None),
        Err(Denied::RoleMismatch)
    );
    // client credentials cannot submit claims
    let client = Principal::Client {
        repos: BTreeSet::from(["fixture-repo".to_string()]),
        can_register: true,
    };
    assert_eq!(
        authorize_claim(&ex("development_progress"), &client, &book(), None),
        Err(Denied::RoleMismatch)
    );
}

#[test]
fn late_claim_from_issued_old_assignment_is_preserved_not_judged() {
    let dev = agent("fixture-repo", "development");
    // gen 0 of the same assignment, and a different older assignment: stored with their own SHA
    let mut r = ex("development_progress");
    r["generation"] = json!(0);
    r["body"]["target_sha"] = json!(OLD0);
    let d = authorize_claim(&r, &dev, &book(), None).unwrap();
    assert_eq!(
        d,
        ClaimDisposition::PreservedLate {
            assignment_id: "fixture-assignment".into(),
            generation: 0,
            target_sha: OLD0.into()
        }
    );
    assert_eq!(d.http_status(), 202);
    assert!(!d.counts_for_current_judgment());
    r["assignment_id"] = json!("older");
    r["generation"] = json!(1);
    r["body"]["target_sha"] = Value::Null; // no body SHA: attributed to the assignment's recorded SHA
    let d = authorize_claim(&r, &dev, &book(), None).unwrap();
    assert!(
        matches!(d, ClaimDisposition::PreservedLate { ref target_sha, .. } if target_sha == OLDER)
    );
    assert!(!d.counts_for_current_judgment());
    // the current one counts
    let d = authorize_claim(&ex("development_progress"), &dev, &book(), None).unwrap();
    assert!(d.counts_for_current_judgment());
    // (authorize_claim takes `&AssignmentBook`, so it cannot change which assignment is current.)
}

#[test]
fn body_sha_must_match_the_assignment_sha() {
    let (dev, guard) = (
        agent("fixture-repo", "development"),
        agent("fixture-repo", "guardrail"),
    );
    // an approve of another SHA must not become an approve of the current assignment
    let mut g = ex("guardrail_review");
    g["body"]["target_sha"] = json!(OLD0);
    assert_eq!(
        authorize_claim(&g, &guard, &book(), None),
        Err(Denied::AssignmentShaMismatch)
    );
    assert_eq!(Denied::AssignmentShaMismatch.http_status(), 409);
    // late claim naming the current SHA instead of its own assignment's SHA
    let mut r = ex("development_progress");
    r["generation"] = json!(0);
    r["body"]["target_sha"] = json!(CUR);
    assert_eq!(
        authorize_claim(&r, &dev, &book(), None),
        Err(Denied::AssignmentShaMismatch)
    );
    // matching SHA is accepted
    assert_eq!(
        authorize_claim(&ex("guardrail_review"), &guard, &book(), None),
        current()
    );
    // serialized codes are the contract's codes
    assert_eq!(
        serde_json::to_value(Denied::ContentConflict).unwrap_or_default(),
        json!("EVENT_CONTENT_CONFLICT")
    );
    assert_eq!(
        serde_json::to_value(Denied::AssignmentShaMismatch).unwrap_or_default(),
        json!("ASSIGNMENT_SHA_MISMATCH")
    );
}

#[test]
fn never_issued_assignment_or_generation_is_409() {
    let dev = agent("fixture-repo", "development");
    let mut r = ex("development_progress");
    r["generation"] = json!(2); // future/never issued
    assert_eq!(
        authorize_claim(&r, &dev, &book(), None),
        Err(Denied::StaleAssignment)
    );
    assert_eq!(Denied::StaleAssignment.http_status(), 409);
    r["assignment_id"] = json!("another");
    r["generation"] = json!(1);
    assert_eq!(
        authorize_claim(&r, &dev, &book(), None),
        Err(Denied::StaleAssignment)
    );
    // no assignment history at all
    assert_eq!(
        authorize_claim(
            &ex("development_progress"),
            &dev,
            &AssignmentBook::default(),
            None
        ),
        Err(Denied::StaleAssignment)
    );
    // an issued assignment of another role's book is not in this book
    let other_role = AssignmentBook {
        issued: vec![issued("guard-only", 1, "g1")],
        current: Some(("guard-only".into(), 1)),
    };
    assert_eq!(
        authorize_claim(&ex("development_progress"), &dev, &other_role, None),
        Err(Denied::StaleAssignment)
    );
}

#[test]
fn same_id_replay_is_checked_before_assignment() {
    let dev = agent("fixture-repo", "development");
    let r = ex("development_progress");
    // assignment has moved on to generation 2 since the claim was stored
    let mut moved = book();
    moved.issued.push(issued("fixture-assignment", 2, "d2"));
    moved.current = Some(("fixture-assignment".into(), 2));
    let d = authorize_claim(&r, &dev, &moved, Some(&r)).unwrap();
    assert_eq!(d, ClaimDisposition::Replay);
    assert_eq!(d.http_status(), 200);
    assert!(!d.counts_for_current_judgment());
    // same ID with a never-issued assignment: still a content conflict (replay check comes first)
    let mut unissued = r.clone();
    unissued["assignment_id"] = json!("never-issued");
    assert_eq!(
        authorize_claim(&unissued, &dev, &moved, Some(&r)),
        Err(Denied::ContentConflict)
    );
    // same ID, different content
    let mut changed = r.clone();
    changed["next_action"] = json!("다른 내용");
    assert_eq!(
        authorize_claim(&changed, &dev, &moved, Some(&r)),
        Err(Denied::ContentConflict)
    );
    assert_eq!(Denied::ContentConflict.http_status(), 409);
    // authorization still comes first: another repo's agent cannot replay
    assert_eq!(
        authorize_claim(&r, &agent("other", "development"), &moved, Some(&r)),
        Err(Denied::RepoScopeDenied)
    );
}

#[test]
fn agents_cannot_read_client_views_and_repro_is_guardrail_only() {
    let guard = agent("fixture-repo", "guardrail");
    assert_eq!(
        may_read_sprint(&guard, "fixture-repo"),
        Err(Denied::AgentReadDenied)
    );
    let client = Principal::Client {
        repos: BTreeSet::from(["fixture-repo".to_string()]),
        can_register: false,
    };
    assert_eq!(may_read_sprint(&client, "fixture-repo"), Ok(()));
    assert_eq!(
        may_read_sprint(&client, "other"),
        Err(Denied::RepoScopeDenied)
    );

    let req = json!({"repo_id": "fixture-repo", "assignment_id": "fixture-assignment", "generation": 1, "target_sha": CUR});
    assert_eq!(
        authorize_repro(&req, &guard, &book(), None),
        Ok(ReproDisposition::Queue {
            target_sha: CUR.into()
        })
    );
    assert_eq!(
        authorize_repro(&req, &agent("fixture-repo", "development"), &book(), None),
        Err(Denied::RoleMismatch)
    );
    assert_eq!(
        authorize_repro(&req, &agent("other", "guardrail"), &book(), None),
        Err(Denied::RepoScopeDenied)
    );
    // a NEW repro request on an old (issued) generation would start stale work: 409
    let old = json!({"repo_id": "fixture-repo", "assignment_id": "fixture-assignment", "generation": 0, "target_sha": OLD0});
    assert_eq!(
        authorize_repro(&old, &guard, &book(), None),
        Err(Denied::StaleAssignment)
    );
    // current assignment but a SHA other than the assignment's: new work outside it, 409
    let mut other_sha = req.clone();
    other_sha["target_sha"] = json!("ffff");
    assert_eq!(
        authorize_repro(&other_sha, &guard, &book(), None),
        Err(Denied::StaleAssignment)
    );
    // ...but re-submitting an existing request is an idempotent re-query, even after the assignment moved on
    assert_eq!(
        authorize_repro(&old, &guard, &book(), Some(&old)),
        Ok(ReproDisposition::Replay)
    );
    let mut altered = old.clone();
    altered["generation"] = json!(1);
    assert_eq!(
        authorize_repro(&altered, &guard, &book(), Some(&old)),
        Err(Denied::ContentConflict)
    );
    assert_eq!(may_read_repro(&guard, "fixture-repo"), Ok(()));
    assert_eq!(
        may_read_repro(&agent("other", "guardrail"), "fixture-repo"),
        Err(Denied::RepoScopeDenied)
    );
    assert_eq!(
        may_read_repro(&agent("fixture-repo", "development"), "fixture-repo"),
        Err(Denied::RoleMismatch)
    );
    assert_eq!(
        may_read_repro(&client, "fixture-repo"),
        Err(Denied::RepoScopeDenied)
    );
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
        "시한이 지나거나 응답이 없으면 적용할 기본 조치: 응답 전까지 범위·기한 유지",
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
        "] 상태 변경 ·",
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

// --- review of 8637403: added cases ---

#[test]
fn completion_summary_scopes() {
    let all = CompletionFacts {
        head_sha: "c".repeat(40),
        ci_passed_on_head: true,
        review: Some(ReviewCurrency::ApprovedAtCurrentSha),
        merged_sha: Some("d".repeat(40)),
        main_ci_passed: true,
        required_outputs_present: true,
        followups_accepted: true,
    };
    assert!(completion_summary(&all).starts_with("작업 완료:"));
    let mut no_outputs = all.clone();
    no_outputs.required_outputs_present = false;
    let t = completion_summary(&no_outputs);
    assert!(
        t.contains("main CI 통과 · 후속 작업 인수만 확인")
            && t.contains("미완료: 필수 산출물 확인"),
        "{t}"
    );
    // 01-08 N2: a merge without a current-SHA approval stays visible, and the gap is named.
    let mut merged_unreviewed = all.clone();
    merged_unreviewed.review = Some(ReviewCurrency::ReviewNeededOnCurrentSha {
        stale_approvals: vec!["e".repeat(40)],
    });
    let t = completion_summary(&merged_unreviewed);
    assert!(
        t.contains(&format!("병합 {}", "d".repeat(7)))
            && t.contains("미완료: 현재 SHA cccccc")
            && t.contains("독립 검토 승인 (이전 SHA 승인 eeeeeee은 현재 판정에 쓰지 않음)")
            && !t.starts_with("작업 완료"),
        "{t}"
    );
    // every missing scope is listed, not only the first
    let mut two_gaps = all.clone();
    two_gaps.ci_passed_on_head = false;
    two_gaps.main_ci_passed = false;
    let t = completion_summary(&two_gaps);
    assert!(
        t.contains("미완료: 현재 SHA cccccc") && t.contains(" · 병합 SHA의 main CI 통과"),
        "{t}"
    );
    assert_eq!(
        completion_summary(&CompletionFacts::default())
            .split('.')
            .next(),
        Some("확인된 완료 범위 없음")
    );
}

#[test]
fn headline_states_evidence_level_and_claimed_completion() {
    let mut r = ex("development_progress");
    r["operational_state"] = json!("완료");
    let text = render_human(&r);
    let first = text.lines().next().unwrap_or_default();
    assert!(
        first.contains("[claim · 판정 아님]") && first.contains("완료 주장(판정 아님)"),
        "{first}"
    );
    let v = render_human(&ex("coordinator_verdict"));
    assert!(
        v.lines()
            .next()
            .unwrap_or_default()
            .contains("[verified 판정]")
    );
    assert!(v.contains("증거 대조 결과: 설명용 대조 결과"));
}

#[test]
fn delay_reasons_are_rendered() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["deadline_state"] = json!("set");
    r["schedule"]["deadline_at"] = json!("2026-11-30T00:00:00+09:00");
    r["schedule"]["status"] = json!("at_risk");
    r["schedule"]["delay_reasons"] = json!(["검토 대기 2일"]);
    r["schedule"]["affected_tasks"] = json!(["07-03"]);
    assert!(validate_report(&r).is_empty(), "{:?}", validate_report(&r));
    let text = render_human(&r);
    assert!(
        text.contains("지연 사유: 검토 대기 2일 · 지연 영향 작업: 07-03"),
        "{text}"
    );
}

#[test]
fn options_mismatch_alone_is_rejected() {
    let mut r = ex("decision_request");
    r["decision"]["options"] = json!(["유지: 기존 범위 유지", "변경: 다른 영향"]);
    let errs = validate_report(&r);
    assert!(
        errs.iter().any(|e| e.field == "decision.options"),
        "{errs:?}"
    );
}

#[test]
fn bad_next_check_at_is_rejected() {
    let mut r = ex("coordinator_verdict");
    r["next_check_at"] = json!("2026-11-12T09:00:00+-1:00");
    assert!(
        validate_report(&r)
            .iter()
            .any(|e| e.field == "next_check_at")
    );
}

#[test]
fn fractional_seconds_are_ordered() {
    let mut r = ex("coordinator_verdict");
    r["schedule"]["forecast"] = json!({"state": "estimated",
        "completion_range": {"earliest_at": "2026-11-14T00:00:00.9+09:00", "latest_at": "2026-11-14T00:00:00.1+09:00"},
        "evidence": ["fixture://m"], "reason": null});
    assert!(
        invalid(&r),
        "reversed within the same second must be rejected"
    );
    let mut late = ex("coordinator_verdict");
    late["observed_at"] = json!("2026-11-13T00:00:00.5+09:00");
    late["schedule"] = json!({"deadline_state": "set", "deadline_at": "2026-11-13T00:00:00+09:00", "status": "late",
        "remaining_tasks": 1, "remaining_reason": null, "delay_reasons": ["설명용"], "affected_tasks": ["FIXTURE-TASK"],
        "forecast": {"state": "unestimated", "completion_range": null, "evidence": [], "reason": "없음"}});
    assert!(
        validate_report(&late).is_empty(),
        "late by half a second is late: {:?}",
        validate_report(&late)
    );
}

#[test]
fn material_changes_notify_and_reorders_do_not() {
    let base = verdict_waiting("CI_RUNNING");
    let check = |f: &dyn Fn(&mut Value), expect: &str| {
        let mut n = base.clone();
        f(&mut n);
        match notification(Some(&base), &n) {
            Notification::Notify(w) => assert!(w.contains(&expect), "{expect}: {w:?}"),
            Notification::Suppress => panic!("{expect} suppressed"),
        }
    };
    check(&|n| n["phase"] = json!("병합"), "phase");
    check(
        &|n| n["operational_state"] = json!("작업 중"),
        "operational_state",
    );
    check(&|n| n["reason_code"] = json!("QUOTA_WAIT"), "reason_code");
    check(&|n| n["next_owner"] = json!("사용자"), "next_owner");
    check(
        &|n| n["decision"]["default_action"] = json!("다른 기본 조치"),
        "decision",
    );
    check(
        &|n| n["decision"]["deadline_at"] = json!("2026-11-20T00:00:00+09:00"),
        "decision",
    );
    check(
        &|n| n["body"]["evidence_comparison"] = json!("새 대조"),
        "body_verdict",
    );
    check(&|n| n["schedule"]["remaining_tasks"] = json!(3), "schedule");
    // withdrawal of a decision
    let mut with_dec = base.clone();
    with_dec["decision"]["required"] = json!(true);
    assert!(
        matches!(notification(Some(&with_dec), &base), Notification::Notify(w) if w.contains(&"decision_withdrawn"))
    );
    // guardrail verdict flip
    let g = ex("guardrail_review");
    let mut g2 = g.clone();
    g2["body"]["verdict"] = json!("approve");
    g2["body"]["findings"] = json!([]);
    assert!(
        matches!(notification(Some(&g), &g2), Notification::Notify(w) if w.contains(&"body_verdict"))
    );
    // reordering lists alone is not material
    let mut a = base.clone();
    a["evidence"] = json!(["x", "y"]);
    a["unknown_facts"] = json!(["p", "q"]);
    let mut b = a.clone();
    b["evidence"] = json!(["y", "x"]);
    b["unknown_facts"] = json!(["q", "p"]);
    assert_eq!(notification(Some(&a), &b), Notification::Suppress);
}
