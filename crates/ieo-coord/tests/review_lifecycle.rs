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
        "task_id":"01-01","pull_request":7,"head_sha":SHA1,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("dev-a", path, &first).0, 403);
    let (status, result) = e.post("assigner", path, &first);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["developer_push_allowed"], false);
    let mut wrong_pr = first.clone();
    wrong_pr["event_id"] = json!("other-pr");
    wrong_pr["pull_request"] = json!(8);
    let (status, denied) = e.post("assigner", path, &wrong_pr);
    assert_eq!(status, 409);
    assert_eq!(denied["code"], "REVIEW_PR_MISMATCH");

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
            "G2",
            0,
            "01-01",
            SHA2,
            json!({"assignment_id":"G1","generation":0})
        )
        .0,
        201
    );
    let mut invalidation = first.clone();
    invalidation["event_id"] = json!("invalidate-02");
    invalidation["head_sha"] = json!(SHA2);
    invalidation["action"] = json!("invalidate");
    let (status, invalidated) = e.post("assigner", path, &invalidation);
    assert_eq!(status, 201, "{invalidated}");
    assert_eq!(invalidated["phase"], "Blocked");
    assert_eq!(invalidated["unauthorized_head_change"], true);

    let changed = json!({"event_id":"head-02","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","pull_request":7,"head_sha":SHA2,"assignment_id":"G2","generation":0,"action":"reassign"});
    let (status, result) = e.post("assigner", path, &changed);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["phase"], "Pending");
    assert_eq!(result["unauthorized_head_change"], true);

    let old = report("guardrail", "old-report", "G1", 0, "01-01", SHA1);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &old).0, 202);
    let mut late = first.clone();
    late["event_id"] = json!("late-verdict");
    late["action"] = json!("approve");
    late["report_id"] = json!("old-report");
    late["report_revision"] = json!(1);
    let (status, result) = e.post("assigner", path, &late);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["outcome"], "preserved_late");
    assert_eq!(result["phase"], "Pending");
}

#[test]
fn pending_review_blocks_judgment_until_the_current_guardrail_report_is_recorded() {
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
    let mut transition = json!({"event_id":"submit-01","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","pull_request":7,"head_sha":SHA1,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("assigner", path, &transition).0, 201);
    full_evidence(&e, SHA1, "review-lifecycle");
    assert_eq!(judge(&e, "pending", "01-01").1["outcome"], "not_accepted");

    transition["event_id"] = json!("approve-01");
    transition["action"] = json!("approve");
    transition["report_id"] = json!("current-report");
    transition["report_revision"] = json!(1);
    assert_eq!(e.post("assigner", path, &transition).0, 409);
    let claim = report("guardrail", "current-report", "G1", 0, "01-01", SHA1);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &claim).0, 202);
    let (status, result) = e.post("assigner", path, &transition);
    assert_eq!(status, 201, "{result}");
    assert_eq!(result["approved_head"], SHA1);
    assert_eq!(
        judge(&e, "without-pr-evidence", "01-01").1["outcome"],
        "not_accepted"
    );
    for number in [8, 7] {
        assert_eq!(evidence(&e, &format!("pr-{number}-ci"), "ci_run",
            json!({"sha":SHA1,"source_ref":"actions/pr-check","conclusion":"success","pull_request":number})).0, 201);
        assert_eq!(evidence(&e, &format!("pr-{number}-merge"), "merge",
            json!({"head_sha":SHA1,"merged_sha":MERGE,"source_ref":"pull/merge","pull_request":number})).0, 201);
        let expected = if number == 7 {
            "accepted"
        } else {
            "not_accepted"
        };
        assert_eq!(
            judge(&e, &format!("judge-pr-{number}"), "01-01").1["outcome"],
            expected
        );
    }
}

#[test]
fn timeout_fences_the_old_assignment_and_replacement_is_spent_once() {
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
    let mut event = json!({"event_id":"submit","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","pull_request":7,"head_sha":SHA1,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("assigner", path, &event).0, 201);
    event["event_id"] = json!("timeout-1");
    event["action"] = json!("timeout");
    assert_eq!(e.post("assigner", path, &event).0, 409);

    let age = || {
        e.db()
            .batch_execute(
                "ALTER TABLE review_transitions DISABLE TRIGGER review_transitions_append_only;
         UPDATE review_transitions SET recorded_at=clock_timestamp() - interval '61 minutes';
         ALTER TABLE review_transitions ENABLE TRIGGER review_transitions_append_only;",
            )
            .unwrap()
    };
    age();
    let (status, timed_out) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{timed_out}");
    assert_eq!(timed_out["replacement_candidate"], true);
    assert_eq!(e.post("assigner", path, &event), (200, timed_out));
    assert_eq!(
        e.db()
            .query_one(
                "SELECT count(*) FROM assignment_heads WHERE role='guardrail'",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );

    let (status, wrong) = assign(&e, "guardrail", "WRONG", 0, "01-01", SHA2, json!(null));
    assert_eq!(status, 409, "{wrong}");
    let active = e
        .db()
        .query_one(
            "SELECT count(*) FROM assignment_heads WHERE role='guardrail'",
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(active, 0);
    assert_eq!(
        assign(&e, "guardrail", "G2", 0, "01-01", SHA1, json!(null)).0,
        201
    );
    event["event_id"] = json!("replacement");
    event["assignment_id"] = json!("G2");
    event["action"] = json!("replace");
    let (status, replaced) = e.post("assigner", path, &event);
    assert_eq!(status, 409, "{replaced}");
    assert_eq!(replaced["code"], "REVIEW_REPLACEMENT_REQUIRES_ASSIGNMENT");
    age();
    event["event_id"] = json!("timeout-2");
    event["action"] = json!("timeout");
    let (status, blocked) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{blocked}");
    assert_eq!(blocked["phase"], "Blocked");
    assert_eq!(blocked["replacement_candidate"], false);
}

#[test]
fn timed_out_review_can_invalidate_and_reassign_a_new_head() {
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
    let mut event = json!({"event_id":"submit","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","head_sha":SHA1,"pull_request":7,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("assigner", path, &event).0, 201);
    e.db()
        .batch_execute(
            "ALTER TABLE review_transitions DISABLE TRIGGER review_transitions_append_only;
         UPDATE review_transitions SET recorded_at=clock_timestamp() - interval '61 minutes';
         ALTER TABLE review_transitions ENABLE TRIGGER review_transitions_append_only;",
        )
        .unwrap();
    event["event_id"] = json!("expired");
    event["action"] = json!("timeout");
    assert_eq!(e.post("assigner", path, &event).0, 201);

    let late_report = report("guardrail", "expired-claim", "G1", 0, "01-01", SHA1);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &late_report).0, 202);
    event["event_id"] = json!("late");
    event["action"] = json!("approve");
    event["report_id"] = json!("expired-claim");
    event["report_revision"] = json!(1);
    let (status, late) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{late}");
    assert_eq!(late["outcome"], "preserved_late");
    assert!(late["approved_head"].is_null());

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
    event.as_object_mut().unwrap().remove("report_id");
    event.as_object_mut().unwrap().remove("report_revision");
    event["head_sha"] = json!(SHA2);
    event["event_id"] = json!("changed");
    event["action"] = json!("invalidate");
    let (status, invalidated) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{invalidated}");
    assert_eq!(invalidated["phase"], "Blocked");
    assert_eq!(
        assign(&e, "guardrail", "G2", 0, "01-01", SHA2, json!(null)).0,
        201
    );
    event["event_id"] = json!("new");
    event["action"] = json!("reassign");
    event["assignment_id"] = json!("G2");
    let (status, assigned) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{assigned}");
    assert_eq!(assigned["phase"], "Pending");
    assert_eq!(assigned["unauthorized_head_change"], true);
}

#[test]
fn github_writes_enforce_phase_and_send_the_approved_sha_as_a_server_condition() {
    use std::io::ErrorKind;
    use std::sync::{Arc, Mutex};

    if base_config().is_none() {
        return;
    }
    let fixture = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = fixture.server_addr().to_ip().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::clone(&seen);
    let worker = std::thread::spawn(move || {
        for _ in 0..4 {
            let mut req = fixture
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap();
            let mut body = String::new();
            req.as_reader().read_to_string(&mut body).unwrap();
            requests.lock().unwrap().push((
                req.method().as_str().to_owned(),
                req.url().to_owned(),
                serde_json::from_str::<serde_json::Value>(&body).unwrap_or(serde_json::Value::Null),
            ));
            let (status, response) = match (req.method().as_str(), req.url()) {
                ("GET", "/repos/example/fixture/pulls/7") => (
                    200,
                    json!({"state":"open",
                    "head":{"ref":"topic/test","sha":SHA1,"repo":{"full_name":"example/fixture"}},
                    "base":{"ref":"main","repo":{"full_name":"example/fixture"}}}),
                ),
                ("PATCH", "/repos/example/fixture/git/refs/heads/topic/test") => {
                    (200, json!({"object":{"sha":SHA1}}))
                }
                ("PUT", "/repos/example/fixture/pulls/7/merge") =>
                // Simulate the head changing after GET but before GitHub checks the PUT.
                {
                    (
                        409,
                        json!({"message":"Head branch was modified", "current_head":SHA2}),
                    )
                }
                _ => (500, json!({})),
            };
            req.respond(
                tiny_http::Response::from_string(response.to_string()).with_status_code(status),
            )
            .unwrap();
        }
    });
    let token = std::env::temp_dir().join(format!(
        "ieo-github-write-fixture-{}-{port}",
        std::process::id()
    ));
    std::fs::write(&token, "fixture-only").unwrap();
    let config = ieo_coord::github_write::Config::loopback(
        port,
        json!({
            "repo-a":{"repository":"example/fixture","push_token_file":token,"merge_token_file":token,"required_checks":["fixture/check"]}
        }),
    );
    let e = start_with_github(Some(config)).unwrap();
    ready_sprint(&e, 3);
    let (status, assigned) = e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id":"repo-a","role":"development","assignment_id":"D1","generation":0,
            "sprint_id":"POC-01","sprint_revision":5,"task_id":"01-01","target_sha":SHA1,
            "expected_current":null,"branch_ref":"refs/heads/topic/test"}),
    );
    assert_eq!(status, 201, "{assigned}");
    let transition = "/v0/coordinator/review-transitions";
    let mut event = json!({"event_id":"opening","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","head_sha":SHA1,"pull_request":7,"assignment_id":"D1","generation":0,"action":"open"});
    assert_eq!(e.post("assigner", transition, &event).0, 201);
    let write = json!({"repo_id":"repo-a","sprint_id":"POC-01","task_id":"01-01","head_sha":SHA1,
        "pull_request":7,"assignment_id":"D1","generation":0});
    let push = "/v0/coordinator/github/push";
    let merge = "/v0/coordinator/github/merge";
    assert_eq!(e.post_k("gr-a", push, "unauthorized", &write).0, 403);
    let (status, pushed) = e.post_k("dev-a", push, "first", &write);
    assert_eq!(status, 200, "{pushed}");
    assert_eq!(pushed["outcome"], "pushed");
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, json!(null)).0,
        201
    );
    event["event_id"] = json!("submit");
    event["assignment_id"] = json!("G1");
    event["action"] = json!("submit");
    assert_eq!(e.post("assigner", transition, &event).0, 201);
    assert_eq!(e.post_k("dev-a", push, "pending", &write).0, 409);
    assert_eq!(e.post_k("judge", merge, "before-approval", &write).0, 409);
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "approved-claim", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    event["event_id"] = json!("approval");
    event["action"] = json!("approve");
    event["report_id"] = json!("approved-claim");
    event["report_revision"] = json!(1);
    assert_eq!(e.post("assigner", transition, &event).0, 201);
    assert_eq!(
        evidence(
            &e,
            "ci-pr-7",
            "ci_run",
            json!({
                "sha":SHA1,"conclusion":"success","pull_request":7,"source_ref":"fixture/check"
            })
        )
        .0,
        201
    );
    let (status, refused) = e.post_k("judge", merge, "merge-raced", &write);
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["github_status"], 409);
    assert_eq!(e.post_k("judge", merge, "merge-raced", &write).1, refused);
    worker.join().unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[1].2, json!({"sha":SHA1,"force":false}));
    assert_eq!(seen[3].2, json!({"sha":SHA1}));
    std::fs::remove_file(token)
        .unwrap_or_else(|error| assert_eq!(error.kind(), ErrorKind::NotFound));
}
