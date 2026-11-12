//! 01-07 conformance: each case mutates the real POC-01 spec from the plan
//! manifest so exactly the intended rule fires. Field locations are part of
//! the contract.

use ieo_spec::{CheckMode, Code, Registry, check_registration, spec_digest};
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../docs/poc/Console_Coordinator_Sprints_v0.json");

fn manifest() -> Value {
    serde_json::from_str(MANIFEST).unwrap_or(Value::Null)
}

fn sprint(i: usize) -> Value {
    manifest()["sprints"][i].clone()
}

/// Registry = profiles + every sprint except `except_id`.
fn registry(except_id: &str) -> Registry {
    let m = manifest();
    let mut r = Registry::new();
    r.add_profiles_from_manifest(&m);
    for s in m["sprints"].as_array().into_iter().flatten() {
        if s["sprint_id"] != except_id {
            r.add_spec(s);
        }
    }
    r
}

fn redigest(mut s: Value) -> Value {
    s["digest"] = json!(spec_digest(&s));
    s
}

fn codes(spec: &Value, mode: CheckMode) -> Vec<(Code, String)> {
    check_registration(spec, &registry("POC-01"), mode)
        .rejections
        .into_iter()
        .map(|r| (r.code, r.field))
        .collect()
}

#[test]
fn baseline_poc01_draft_accepted_and_digest_matches_package() {
    let s = sprint(0);
    let out = check_registration(&s, &registry("POC-01"), CheckMode::Draft);
    assert!(out.rejections.is_empty(), "{:?}", out.rejections);
    assert_eq!(out.http_status(), 201);
    assert_eq!(out.computed_digest.as_deref(), s["digest"].as_str());
}

#[test]
fn all_nine_package_digests_reproduce() {
    for s in manifest()["sprints"].as_array().into_iter().flatten() {
        assert_eq!(
            Some(spec_digest(s).as_str()),
            s["digest"].as_str(),
            "{}",
            s["sprint_id"]
        );
    }
}

#[test]
fn missing_field_is_distinguished() {
    let mut s = sprint(0);
    if let Some(t) = s["tasks"][0].as_object_mut() {
        t.remove("verification");
    }
    let out = check_registration(&redigest(s), &registry("POC-01"), CheckMode::Draft);
    assert_eq!(out.http_status(), 400);
    let got: Vec<_> = out
        .rejections
        .iter()
        .map(|r| (r.code, r.field.as_str()))
        .collect();
    assert_eq!(got, vec![(Code::MissingField, "tasks[0].verification")]);
}

#[test]
fn schema_violation_other_than_missing() {
    let mut s = sprint(0);
    s["tasks"][0]["kind"] = json!("기타");
    let got = codes(&redigest(s), CheckMode::Draft);
    assert_eq!(
        got,
        vec![(Code::SchemaViolation, "tasks[0].kind".to_string())]
    );
}

#[test]
fn digest_mismatch() {
    let mut s = sprint(0);
    s["title"] = json!("changed without new digest");
    assert_eq!(
        codes(&s, CheckMode::Draft),
        vec![(Code::DigestMismatch, "digest".to_string())]
    );
}

#[test]
fn duplicate_task_id_within_spec() {
    let mut s = sprint(0);
    let copy = s["tasks"][7].clone(); // 01-08 again
    if let Some(a) = s["tasks"].as_array_mut() {
        a.push(copy);
    }
    assert_eq!(
        codes(&redigest(s), CheckMode::Draft),
        vec![(Code::DuplicateTaskId, "tasks[8].task_id".to_string())]
    );
}

#[test]
fn duplicate_task_id_across_registered_sprint() {
    let mut s = sprint(0);
    let mut foreign = s["tasks"][7].clone();
    foreign["task_id"] = json!("02-01"); // belongs to registered POC-02
    if let Some(a) = s["tasks"].as_array_mut() {
        a.push(foreign);
    }
    let got = codes(&redigest(s), CheckMode::Draft);
    assert!(
        got.contains(&(Code::TaskIdConflict, "tasks[8].task_id".to_string())),
        "{got:?}"
    );
}

#[test]
fn undefined_prerequisite() {
    let mut s = sprint(0);
    let n = s["tasks"][0]["prerequisites"]
        .as_array()
        .map_or(0, Vec::len);
    if let Some(a) = s["tasks"][0]["prerequisites"].as_array_mut() {
        a.push(json!({"task_id": "99-99", "plan_version": "v1.3.3", "required_state": "accepted"}));
    }
    assert_eq!(
        codes(&redigest(s), CheckMode::Draft),
        vec![(
            Code::UndefinedPrerequisite,
            format!("tasks[0].prerequisites[{n}].task_id")
        )]
    );
}

#[test]
fn prerequisite_with_wrong_plan_version_is_undefined() {
    let mut s = sprint(0);
    // 01-02 requires 01-01; ask for an old plan version of it.
    s["tasks"][1]["prerequisites"][0]["plan_version"] = json!("v1.3.2");
    assert_eq!(
        codes(&redigest(s), CheckMode::Draft),
        vec![(
            Code::UndefinedPrerequisite,
            "tasks[1].prerequisites[0].task_id".to_string()
        )]
    );
}

#[test]
fn dependency_cycle() {
    let mut s = sprint(0);
    // 01-08 → 01-07 → 01-01; make 01-01 depend on 01-08.
    if let Some(a) = s["tasks"][0]["prerequisites"].as_array_mut() {
        a.push(json!({"task_id": "01-08", "plan_version": "v1.3.3", "required_state": "accepted"}));
    }
    let got = codes(&redigest(s), CheckMode::Draft);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, Code::DependencyCycle);
}

#[test]
fn unknown_profile_item_and_unknown_profile() {
    let mut s = sprint(0);
    s["tasks"][0]["acceptance"]["item_ids"] = json!(["01-01.not-in-profile"]);
    s["tasks"][1]["acceptance"]["profile_revision"] = json!(3);
    assert_eq!(
        codes(&redigest(s), CheckMode::Draft),
        vec![
            (
                Code::UnknownProfileItem,
                "tasks[0].acceptance.item_ids[0]".to_string()
            ),
            (
                Code::UnknownProfileItem,
                "tasks[1].acceptance.profile_id".to_string()
            ),
        ]
    );
}

#[test]
fn unbound_draft_rejected_only_for_executable_registration() {
    let s = sprint(0);
    assert!(codes(&s, CheckMode::Draft).is_empty());
    let got = codes(&s, CheckMode::Executable);
    let n_bindings = s["unresolved_bindings"].as_array().map_or(0, Vec::len);
    assert_eq!(got.len(), 1 + n_bindings);
    assert!(got.iter().all(|(c, _)| *c == Code::UnboundDraft));
    assert_eq!(got[0].1, "registration_state");
}

fn bound_ready(mut s: Value) -> Value {
    s["registration_state"] = json!("ready");
    s["target"]["repository_url"] = json!("https://github.com/HeaInSeo/ieo_poc");
    s["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 2, "unresolved_reason": null});
    s["unresolved_bindings"] = json!([]);
    s
}

#[test]
fn bound_ready_spec_accepted_for_executable_registration() {
    let s = redigest(bound_ready(sprint(0)));
    let out = check_registration(&s, &registry("POC-01"), CheckMode::Executable);
    assert!(out.rejections.is_empty(), "{:?}", out.rejections);
    assert_eq!(out.http_status(), 201);
}

#[test]
fn ready_without_repository_url_fails_format_layer() {
    let mut s = bound_ready(sprint(0));
    s["target"]["repository_url"] = Value::Null;
    let out = check_registration(&redigest(s), &registry("POC-01"), CheckMode::Executable);
    assert_eq!(out.http_status(), 400);
    assert!(
        out.rejections
            .iter()
            .all(|r| r.code == Code::SchemaViolation)
    );
    assert!(
        out.rejections
            .iter()
            .any(|r| r.field == "target.repository_url"),
        "{:?}",
        out.rejections
    );
}

#[test]
fn revision_identity_conflict_and_idempotent_resubmit() {
    let s = sprint(0);
    let mut reg = registry("POC-01");
    reg.add_spec(&s);
    let same = check_registration(&s, &reg, CheckMode::Draft);
    assert_eq!(same.http_status(), 200);

    let mut changed = s.clone();
    changed["title"] = json!("same revision, different content");
    let changed = redigest(changed);
    let out = check_registration(&changed, &reg, CheckMode::Draft);
    assert_eq!(out.http_status(), 409);
    assert!(
        out.rejections
            .iter()
            .any(|r| r.code == Code::RevisionConflict && r.field == "revision")
    );
}

#[test]
fn every_rejection_has_field_and_correction() {
    let mut s = sprint(0);
    s["tasks"][0]["acceptance"]["item_ids"] = json!(["x"]);
    if let Some(a) = s["tasks"][0]["prerequisites"].as_array_mut() {
        a.push(json!({"task_id": "99-99", "plan_version": "v1.3.3", "required_state": "accepted"}));
    }
    let out = check_registration(&redigest(s), &registry("POC-01"), CheckMode::Executable);
    assert!(!out.rejections.is_empty());
    for r in &out.rejections {
        assert!(
            !r.message.is_empty() && !r.correction.is_empty() && r.retryable_after_edit,
            "{r:?}"
        );
    }
}

// --- regressions from independent review of fbe1f24 ---

#[test]
fn draft_registered_spec_is_not_idempotent_for_executable_intake() {
    // F2: same content registered as draft must not short-circuit an executable request.
    let s = sprint(0);
    let mut reg = registry("POC-01");
    reg.add_spec(&s); // stored as draft
    let out = check_registration(&s, &reg, CheckMode::Executable);
    assert_eq!(out.http_status(), 422);
    assert!(out.rejections.iter().all(|r| r.code == Code::UnboundDraft));
    // Executable-registered + executable request stays idempotent.
    let ready = redigest(bound_ready(sprint(0)));
    let mut reg2 = registry("POC-01");
    reg2.add_spec_with_mode(&ready, CheckMode::Executable);
    assert_eq!(
        check_registration(&ready, &reg2, CheckMode::Executable).http_status(),
        200
    );
}

#[test]
fn spec_estimate_range_order_checked() {
    // F3
    let mut s = sprint(0);
    s["tasks"][0]["estimate"] = json!({"state": "measured", "range_seconds": {"min": 1200, "max": 600},
                                       "evidence": ["fixture://m"], "reason": null});
    assert_eq!(
        codes(&redigest(s), CheckMode::Draft),
        vec![(
            Code::EstimateRangeReversed,
            "tasks[0].estimate.range_seconds".to_string()
        )]
    );
}

#[test]
fn non_integer_numbers_rejected_before_digest() {
    // F6: float formatting differs between implementations; v0 allows integers only.
    let mut s = sprint(0);
    s["tasks"][0]["evidence"] = json!([{"ratio": 1e-7}]);
    let out = check_registration(&s, &registry("POC-01"), CheckMode::Draft);
    assert_eq!(out.http_status(), 400);
    assert_eq!(out.rejections[0].field, "tasks[0].evidence[0].ratio");
    let big: Value = serde_json::from_str(r#"{"n": 18446744073709551616}"#).unwrap_or(Value::Null);
    let mut s2 = sprint(0);
    s2["tasks"][0]["evidence"] = json!([big]);
    assert_eq!(
        check_registration(&s2, &registry("POC-01"), CheckMode::Draft).http_status(),
        400
    );
}

#[test]
fn lower_revision_after_higher_is_conflict() {
    let s = sprint(0); // revision 4
    let mut reg = registry("POC-01");
    reg.add_spec(&s);
    let mut lower = s.clone();
    lower["revision"] = json!(3);
    lower["title"] = json!("older revision, new content");
    let out = check_registration(&redigest(lower), &reg, CheckMode::Draft);
    assert_eq!(out.http_status(), 409);
    assert!(
        out.rejections
            .iter()
            .any(|r| r.code == Code::RevisionConflict)
    );
}

#[test]
fn ready_with_unresolved_bindings_is_format_error() {
    // F7: for `ready`, unresolved bindings are excluded by the schema (400); UNBOUND_DRAFT is for drafts.
    let mut s = bound_ready(sprint(0));
    s["unresolved_bindings"] = json!(["실제 repository_url"]);
    let out = check_registration(&redigest(s), &registry("POC-01"), CheckMode::Executable);
    assert_eq!(out.http_status(), 400);
    assert!(
        out.rejections
            .iter()
            .any(|r| r.code == Code::SchemaViolation && r.field == "unresolved_bindings")
    );
}
