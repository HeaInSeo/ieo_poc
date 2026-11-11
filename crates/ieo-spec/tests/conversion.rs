//! 01-07 conformance for Human_Sprint_Authoring_v0.md (human input → v0 draft).

use ieo_spec::{
    CheckMode, Code, ConversionContext, Registry, check_registration, convert_human_input,
    spec_digest,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn ctx(profile_items: &[&str]) -> ConversionContext {
    ConversionContext {
        sprint_id: "NK-01".into(),
        sprint_number: 11,
        revision: 1,
        plan_version: "v1.3.3".into(),
        repository_url: None,
        timezone: "Asia/Seoul".into(),
        track: "핵심".into(),
        entry_condition: None,
        acceptance_profile_id: "nk-profile".into(),
        acceptance_profile_revision: 1,
        profile_items: profile_items.iter().map(|s| s.to_string()).collect(),
        delegation: json!({"allowed": ["승인된 범위의 구현"], "forbidden": ["범위 밖 repo 쓰기"], "approval_scope_ref": "User_Decisions_2026-11-11.md"}),
        limits: None,
        human_validation: json!({"required": false, "timing": "해당 없음", "criteria_ref": "Human_Sprint_Authoring_v0.md"}),
        sources: json!([{"kind": "human_input", "ref": "fixture://human-input", "revision": "1",
                          "digest": "0000000000000000000000000000000000000000000000000000000000000000", "observed_at": null}]),
        registered_tasks: BTreeMap::from([("01-07".to_string(), "v1.3.3".to_string())]),
        input_ref: "fixture://human-input".into(),
    }
}

fn input() -> Value {
    json!({
        "title": "설명용 예시 스프린트",
        "repo": "nodekit",
        "goal": "설명용 목표(실제 백로그 아님)",
        "out_of_scope": ["새 기능"],
        "decisions": ["납기 변경 없음"],
        "deadline": {"state": "none", "at": null, "reason": "사용자가 기한을 두지 않음"},
        "tasks": [
            {"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"},
            {"name": "문구 교정", "output": "수정본·시험 로그", "done_when": "승인 요구와 일치",
             "how_to_check": "기존 검사와 실제 CLI 입력 대조", "after": ["검사 준비", "01-07"],
             "expected_duration": {"min_minutes": 10, "max_minutes": 20, "evidence": ["fixture://past-measurement"]}}
        ]
    })
}

fn registry_for(items: &[&str]) -> Registry {
    let mut r = Registry::new();
    r.add_profile("nk-profile", 1, items.iter().map(|s| s.to_string()));
    // Make 01-07 resolvable as a registered task of another sprint.
    r.add_spec(
        &json!({"sprint_id": "POC-01", "plan_version": "v1.3.3", "revision": 4, "digest": "x",
                       "tasks": [{"task_id": "01-07", "prerequisites": []}]}),
    );
    r
}

fn rejected_codes(v: &Value) -> Vec<(Code, String)> {
    match convert_human_input(v, &ctx(&[])) {
        Ok(_) => vec![],
        Err(r) => r.into_iter().map(|x| (x.code, x.field)).collect(),
    }
}

#[test]
fn converts_to_valid_draft_with_ids_estimates_and_digest() {
    let rec = match convert_human_input(&input(), &ctx(&["11-01.accept", "11-02.accept"])) {
        Ok(r) => r,
        Err(e) => panic!("{e:?}"),
    };
    let s = &rec.spec;
    assert_eq!(s["registration_state"], "draft");
    assert_eq!(
        rec.name_to_task_id.get("검사 준비").map(String::as_str),
        Some("11-01")
    );
    assert_eq!(s["tasks"][1]["prerequisites"][0]["task_id"], "11-01");
    assert_eq!(s["tasks"][1]["prerequisites"][1]["task_id"], "01-07");
    // minutes × 60, evidence kept
    assert_eq!(
        s["tasks"][1]["estimate"]["range_seconds"],
        json!({"min": 600, "max": 1200})
    );
    assert_eq!(s["tasks"][1]["estimate"]["state"], "measured");
    // no estimate → unestimated with reason, never 0
    assert_eq!(s["tasks"][0]["estimate"]["state"], "unestimated");
    assert_eq!(s["tasks"][0]["estimate"]["range_seconds"], Value::Null);
    // deadline none keeps its reason, not turned into "undecided" or a date
    assert_eq!(s["schedule"]["deadline_state"], "none");
    assert_eq!(
        s["schedule"]["unresolved_reason"],
        "사용자가 기한을 두지 않음"
    );
    // digest computed by the converter
    assert_eq!(s["digest"].as_str(), Some(spec_digest(s).as_str()));
    // user wording preserved verbatim
    assert_eq!(s["tasks"][1]["acceptance"]["criteria"], "승인 요구와 일치");
    // unrepresentable human fields preserved in the record
    assert_eq!(
        rec.preserved_unrepresented["out_of_scope"],
        json!(["새 기능"])
    );
    // unknown bindings listed, nothing invented
    assert!(
        rec.unresolved_bindings
            .iter()
            .any(|b| b.contains("repository_url"))
    );
    assert!(rec.unresolved_bindings.iter().any(|b| b.contains("한도")));
    assert!(rec.semantic_review_required.is_empty());

    // The converted draft passes the registration check (draft mode) …
    let reg = registry_for(&["11-01.accept", "11-02.accept"]);
    let out = check_registration(s, &reg, CheckMode::Draft);
    assert!(out.rejections.is_empty(), "{:?}", out.rejections);
    // … and is refused for executable registration while bindings remain.
    let out = check_registration(s, &reg, CheckMode::Executable);
    assert!(
        !out.rejections.is_empty() && out.rejections.iter().all(|r| r.code == Code::UnboundDraft)
    );
}

#[test]
fn criteria_outside_profile_go_to_semantic_review_not_approved() {
    let rec = match convert_human_input(&input(), &ctx(&["11-01.accept"])) {
        Ok(r) => r,
        Err(e) => panic!("{e:?}"),
    };
    assert_eq!(rec.semantic_review_required, vec!["11-02".to_string()]);
    let reg = registry_for(&["11-01.accept"]);
    let out = check_registration(&rec.spec, &reg, CheckMode::Draft);
    let got: Vec<_> = out
        .rejections
        .iter()
        .map(|r| (r.code, r.field.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![(Code::UnknownProfileItem, "tasks[1].acceptance.item_ids[0]")]
    );
}

#[test]
fn unresolved_prerequisite_matches_documented_example() {
    let mut v = input();
    v["tasks"][1]["after"] = json!(["없는 작업"]);
    let err = convert_human_input(&v, &ctx(&[])).err().unwrap_or_default();
    assert_eq!(err.len(), 1);
    assert_eq!(err[0].code, Code::UnresolvedPrerequisite);
    assert_eq!(err[0].field, "tasks[1].after[0]");
    assert!(err[0].retryable_after_edit && !err[0].correction.is_empty());
}

#[test]
fn duplicate_task_names() {
    let mut v = input();
    v["tasks"][1]["name"] = json!("검사 준비");
    v["tasks"][1]["after"] = json!([]);
    assert_eq!(
        rejected_codes(&v),
        vec![(Code::DuplicateTaskName, "tasks[1].name".to_string())]
    );
}

#[test]
fn reversed_estimate() {
    let mut v = input();
    v["tasks"][1]["expected_duration"]["min_minutes"] = json!(30);
    assert_eq!(
        rejected_codes(&v),
        vec![(
            Code::EstimateRangeReversed,
            "tasks[1].expected_duration".to_string()
        )]
    );
}

#[test]
fn estimate_without_evidence_is_format_error() {
    let mut v = input();
    v["tasks"][1]["expected_duration"]["evidence"] = json!([]);
    let got = rejected_codes(&v);
    assert_eq!(
        got,
        vec![(
            Code::SchemaViolation,
            "tasks[1].expected_duration.evidence".to_string()
        )]
    );
}

#[test]
fn missing_required_human_field() {
    let mut v = input();
    if let Some(t) = v["tasks"][0].as_object_mut() {
        t.remove("done_when");
    }
    assert_eq!(
        rejected_codes(&v),
        vec![(Code::MissingField, "tasks[0].done_when".to_string())]
    );
}

#[test]
fn deadline_set_requires_timestamp() {
    let mut v = input();
    v["deadline"] = json!({"state": "set", "at": "not-a-date", "reason": null});
    let got = rejected_codes(&v);
    assert!(
        got.iter()
            .any(|(c, f)| *c == Code::SchemaViolation && f == "deadline.at"),
        "{got:?}"
    );
    v["deadline"] = json!({"state": "set", "at": "2026-12-01T18:00:00+09:00", "reason": null});
    assert!(rejected_codes(&v).is_empty());
}

#[test]
fn undecided_deadline_needs_reason() {
    let mut v = input();
    v["deadline"] = json!({"state": "undecided", "at": null, "reason": null});
    let got = rejected_codes(&v);
    assert!(
        got.iter().any(|(c, _)| *c == Code::SchemaViolation),
        "{got:?}"
    );
}

#[test]
fn cycle_among_human_tasks() {
    let mut v = input();
    v["tasks"][0]["after"] = json!(["문구 교정"]);
    let got = rejected_codes(&v);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, Code::DependencyCycle);
}

#[test]
fn ids_are_stable_for_same_input() {
    let a = convert_human_input(&input(), &ctx(&[]))
        .map(|r| r.spec["digest"].clone())
        .unwrap_or_default();
    let b = convert_human_input(&input(), &ctx(&[]))
        .map(|r| r.spec["digest"].clone())
        .unwrap_or_default();
    assert_eq!(a, b);
}
