//! Replays independent P01 design verdicts through the HTTP gate and PostgreSQL.
mod common;

use common::{Env, SHA1, code, spec, start};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/poc/reports/tasks/04-01/p01"
);

fn assignment_count(env: &Env) -> i64 {
    env.db()
        .query_one("SELECT count(*) FROM assignments", &[])
        .expect("count assignments")
        .get(0)
}

fn assign(env: &Env, revision: u64) -> (u16, Value) {
    env.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({
            "repo_id": "repo-a", "role": "development", "assignment_id": "P01-A",
            "generation": 0, "sprint_id": "POC-01", "sprint_revision": revision,
            "task_id": "01-01", "target_sha": SHA1, "expected_current": null
        }),
    )
}

fn revision_case(
    env: &Env,
    directory: &str,
    case: &str,
    revision: u64,
    verdict: &Value,
    expected: &Value,
) {
    let source = format!("{directory}/{case}.md");
    let bytes = fs::read(Path::new(FIXTURES).join(&source)).expect("frozen design card");
    let design_digest = hex::encode(Sha256::digest(&bytes));
    let mut request = spec("repo-a", revision, 1);
    request["sources"] = json!([{
        "kind": "provided_document", "ref": source, "revision": revision.to_string(),
        "digest": design_digest, "observed_at": null
    }]);
    request["digest"] = json!(ieo_spec::spec_digest(&request));
    let (status, intake) = env.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        &format!("p01-reg-{case}-{revision}"),
        &request,
    );
    assert_eq!(status, 201, "{case}: {intake}");
    assert_eq!(intake["state"], "ACCEPTED_FOR_SEMANTIC_REVIEW");
    let before = assignment_count(env);
    let (status, pending) = assign(env, revision);
    assert_eq!(
        (status, code(&pending).as_str()),
        (422, "SEMANTIC_REVIEW_NOT_PASSED")
    );
    assert_eq!(assignment_count(env), before);

    assert_eq!(verdict["verdict"], expected[0], "{case}: blind verdict");
    assert_eq!(verdict["requirement"], expected[1], "{case}: finding");
    let decision = match verdict["verdict"].as_str() {
        Some("pass") => "passed",
        Some("fail") => "rejected",
        other => panic!("unknown verdict {other:?}"),
    };
    let reason = verdict["reason"].as_str().expect("reported review reason");
    let (status, reviewed) = env.post(
        "bob",
        &format!("/v0/sprints/POC-01/revisions/{revision}/semantic-review"),
        &json!({"decision": decision, "reason": reason, "digest": request["digest"]}),
    );
    assert_eq!(status, 201, "{case}: {reviewed}");
    let review = env.db().query_one(
        "SELECT reviewer, decision, digest, reason FROM semantic_reviews WHERE sprint_id='POC-01' AND revision=$1",
        &[&(revision as i64)],
    ).expect("review history");
    assert_ne!(review.get::<_, String>(0), "alice");
    assert_eq!(review.get::<_, String>(1), decision);
    assert_eq!(review.get::<_, String>(2), request["digest"]);
    assert_eq!(review.get::<_, String>(3), reason);

    let (status, outcome) = assign(env, revision);
    let added = assignment_count(env) - before;
    assert_eq!(
        added,
        expected[2].as_i64().expect("expected row count"),
        "{case}"
    );
    if decision == "passed" {
        assert_eq!(status, 201, "{case}: {outcome}");
        let row = env
            .db()
            .query_one(
                "SELECT sprint_revision FROM assignments WHERE assignment_id='P01-A'",
                &[],
            )
            .expect("persisted assignment");
        assert_eq!(row.get::<_, i64>(0), revision as i64);
    } else {
        assert_eq!(
            (status, code(&outcome).as_str()),
            (422, "SEMANTIC_REVIEW_NOT_PASSED")
        );
    }
    println!(
        "{}",
        json!({
            "case": case, "stage": directory, "revision": revision, "design_digest": design_digest,
            "sprint_digest": request["digest"], "pending_status": 422, "review_status": 201,
            "reviewer": review.get::<_, String>(0), "decision": decision,
            "assignment_status": status, "assignment_code": code(&outcome), "assignments_added": added
        })
    );
}

#[test]
fn p01_independent_design_verdicts_gate_implementation_assignments() {
    let expectations: Value = serde_json::from_str(include_str!(
        "../../../docs/poc/reports/tasks/04-01/p01/expectations.json"
    ))
    .expect("frozen expectations");
    let observed: Value = serde_json::from_str(include_str!(
        "../../../docs/poc/reports/tasks/04-01/p01/reviews/verdicts.json"
    ))
    .expect("independent verdict transcript");
    let initial: BTreeMap<String, Value> =
        serde_json::from_value(expectations["initial"].clone()).expect("initial cases");
    assert_eq!(initial.len(), 4);
    for (case, expected) in initial {
        let Some(env) = start() else { return };
        revision_case(
            &env,
            "candidates",
            &case,
            5,
            &observed["initial"][&case],
            &expected,
        );
        if let Some(corrected) = expectations["corrected"].get(&case) {
            for (index, stage) in [
                "correction-1",
                "correction-2",
                "corrected",
                "correction-4",
                "correction-5",
                "correction-6",
                "correction-7",
                "correction-8",
                "correction-9",
                "correction-10",
                "correction-11",
            ]
            .iter()
            .enumerate()
            {
                let Some(verdict) = observed[*stage].get(&case) else {
                    continue;
                };
                let blocked = json!(["fail", verdict["requirement"], 0]);
                let expected = if verdict["verdict"] == "pass" {
                    corrected
                } else {
                    &blocked
                };
                revision_case(&env, stage, &case, 6 + index as u64, verdict, expected);
            }
            let expected_terminal = if case == "F03" { 0 } else { 1 };
            assert_eq!(
                assignment_count(&env),
                expected_terminal,
                "{case}: observed terminal state"
            );
        }
    }
}
