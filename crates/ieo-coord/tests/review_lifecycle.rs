mod common;
use common::*;
use serde_json::json;

#[test]
fn review_transitions_use_current_assignments_and_preserve_late_reports() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "D1", 0, "01-01", SHA1, json!(null)).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, json!(null)).0,
        201
    );
    let path = "/v0/coordinator/review-transitions";
    let first = json!({"event_id":"submit-01","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","head_sha":SHA1,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("dev-a", path, &first).0, 403);
    let (status, result) = e.post("assigner", path, &first);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["developer_push_allowed"], false);
    assert_eq!(e.post("assigner", path, &first), (200, result));

    assert_eq!(
        assign(
            &e,
            "development",
            "D1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id":"D1","generation":0})
        )
        .0,
        201
    );
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id":"G1","generation":0})
        )
        .0,
        201
    );
    let changed = json!({"event_id":"head-02","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","head_sha":SHA2,"assignment_id":"G1","generation":1,"action":"head_changed"});
    let (status, result) = e.post("assigner", path, &changed);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["unauthorized_head_change"], true);

    let old = report("guardrail", "old-report", "G1", 0, "01-01", SHA1);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &old).0, 202);
    let mut late = first.clone();
    late["event_id"] = json!("late-verdict");
    late["action"] = json!("approve");
    late["report_id"] = json!("old-report");
    let (status, result) = e.post("assigner", path, &late);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["outcome"], "preserved_late");
    assert_eq!(result["phase"], "Pending");
}
