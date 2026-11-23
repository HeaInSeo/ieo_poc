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
    event["event_id"] = json!("new-head");
    event["head_sha"] = json!(SHA2);
    event["action"] = json!("invalidate");
    assert_eq!(e.post("assigner", path, &event).0, 201);
    assert_eq!(
        assign(&e, "guardrail", "G3", 0, "01-01", SHA2, json!(null)).0,
        201
    );
    event["event_id"] = json!("new-review");
    event["assignment_id"] = json!("G3");
    event["action"] = json!("reassign");
    let (status, new_review) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{new_review}");
    assert_eq!(new_review["phase"], "Pending");
    age();
    event["event_id"] = json!("new-head-timeout");
    event["action"] = json!("timeout");
    let (status, timed_out) = e.post("assigner", path, &event);
    assert_eq!(status, 201, "{timed_out}");
    assert_eq!(timed_out["replacement_candidate"], false);
    assert_eq!(timed_out["phase"], "Blocked");
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

fn fixture_git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn github_correction_transfers_new_objects_and_reconciles_lost_push_and_merge() {
    use std::sync::{Arc, Mutex};

    if base_config().is_none() {
        return;
    }
    let files = tempfile::tempdir().unwrap();
    let dev = files.path().join("developer");
    let remote = files.path().join("remote");
    std::fs::create_dir(&dev).unwrap();
    std::fs::create_dir(&remote).unwrap();
    fixture_git(&dev, &["init", "-b", "topic/test"]);
    fixture_git(&dev, &["commit", "--allow-empty", "-m", "base"]);
    let old = fixture_git(&dev, &["rev-parse", "HEAD"]);
    fixture_git(&remote, &["init", "--bare"]);
    fixture_git(
        &dev,
        &[
            "push",
            remote.to_str().unwrap(),
            "HEAD:refs/heads/topic/test",
        ],
    );
    std::fs::write(dev.join("correction.txt"), "new correction\n").unwrap();
    fixture_git(&dev, &["add", "correction.txt"]);
    fixture_git(&dev, &["commit", "-m", "correction"]);
    let sha = fixture_git(&dev, &["rev-parse", "HEAD"]);
    let absent = std::process::Command::new("git")
        .current_dir(&remote)
        .args(["cat-file", "-e", &sha])
        .output()
        .unwrap();
    assert!(!absent.status.success());
    let token = files.path().join("token");
    let bundle = files.path().join("candidate.bundle");
    std::fs::write(&token, "fixture-only").unwrap();
    fixture_git(
        &dev,
        &["bundle", "create", bundle.to_str().unwrap(), "HEAD"],
    );

    let fixture = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = fixture.server_addr().to_ip().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::clone(&seen);
    let fixture_repo = remote.clone();
    let old_fixture_head = old.clone();
    let worker = std::thread::spawn(move || {
        let mut merge_calls = 0;
        let mut commit_reads = 0;
        for index in 0..17 {
            let mut req = fixture
                .recv_timeout(std::time::Duration::from_secs(12))
                .unwrap()
                .unwrap();
            let mut body = String::new();
            req.as_reader().read_to_string(&mut body).unwrap();
            requests.lock().unwrap().push((
                req.method().as_str().to_owned(),
                req.url().to_owned(),
                serde_json::from_str::<serde_json::Value>(&body).unwrap_or(serde_json::Value::Null),
            ));
            let current = fixture_git(&fixture_repo, &["rev-parse", "refs/heads/topic/test"]);
            let (status, response) = match (req.method().as_str(), req.url()) {
                ("GET", "/repos/example/fixture/pulls/7") => (
                    200,
                    json!({
                    "state":if merge_calls == 2 {"closed"} else {"open"},
                    "merged":merge_calls == 2,"merge_commit_sha":MERGE,
                    "head":{"ref":"topic/test","sha":if index == 1 { &old_fixture_head } else { &current },
                            "repo":{"full_name":"example/fixture"}},
                    "base":{"ref":"main","repo":{"full_name":"example/fixture"}}}),
                ),
                ("GET", "/repos/example/fixture/git/ref/heads/topic/test") => (
                    200,
                    json!({"ref":"refs/heads/topic/test","object":{"sha":current}}),
                ),
                (
                    "GET",
                    "/repos/example/fixture/compare/4444444444444444444444444444444444444444...main",
                ) => (200, json!({"status":"ahead","base_commit":{"sha":MERGE}})),
                (
                    "GET",
                    "/repos/example/fixture/git/commits/4444444444444444444444444444444444444444",
                ) => {
                    commit_reads += 1;
                    match commit_reads {
                        1 => (404, json!({})),
                        _ => (
                            200,
                            json!({"sha":MERGE,"parents":[{"sha":"5555555555555555555555555555555555555555"},
                                {"sha":if commit_reads == 2 { &old_fixture_head } else { &current }}]}),
                        ),
                    }
                }
                ("PUT", "/repos/example/fixture/pulls/7/merge") => {
                    merge_calls += 1;
                    if merge_calls == 1 {
                        (409, json!({"message":"Head branch was modified"}))
                    } else {
                        // The write has landed, but the write response is unavailable.
                        (502, json!({}))
                    }
                }
                _ => (500, json!({})),
            };
            req.respond(
                tiny_http::Response::from_string(response.to_string()).with_status_code(status),
            )
            .unwrap();
        }
    });
    let config = ieo_coord::github_write::Config::loopback(
        port,
        json!({
            "repo-a":{"repository":"example/fixture","push_token_file":token,"merge_token_file":token,
                "push_bundle_file":bundle,"required_checks":["fixture/check"]}
        }),
        remote.clone(),
    );
    let mut e = start_with_github(Some(config.clone())).unwrap();
    ready_sprint(&e, 3);
    let development = |e: &Env, generation, target: &str, expected| {
        e.post(
            "assigner",
            "/v0/coordinator/assignments",
            &json!({
            "repo_id":"repo-a","role":"development","assignment_id":"D1","generation":generation,
            "sprint_id":"POC-01","sprint_revision":5,"task_id":"01-01","target_sha":target,
            "expected_current":expected,"branch_ref":"refs/heads/topic/test"}),
        )
    };
    assert_eq!(development(&e, 0, &old, json!(null)).0, 201);
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", &old, json!(null)).0,
        201
    );
    let transition = "/v0/coordinator/review-transitions";
    let mut event = json!({"event_id":"submit","repo_id":"repo-a","sprint_id":"POC-01",
        "task_id":"01-01","head_sha":old,"pull_request":7,"assignment_id":"G1","generation":0,"action":"submit"});
    assert_eq!(e.post("assigner", transition, &event).0, 201);
    let mut write = json!({"repo_id":"repo-a","sprint_id":"POC-01","task_id":"01-01","head_sha":sha,
        "pull_request":7,"assignment_id":"D1","generation":0});
    let push = "/v0/coordinator/github/push";
    let merge = "/v0/coordinator/github/merge";
    let reconcile = "/v0/coordinator/github/reconcile";
    assert_eq!(e.post_k("gr-a", push, "unauthorized", &write).0, 403);
    assert_eq!(e.post_k("dev-a", push, "pending", &write).0, 409);

    let mut rejection = report("guardrail", "reject-claim", "G1", 0, "01-01", &old);
    rejection["body"]["verdict"] = json!("reject");
    rejection["body"]["findings"] = json!([{"finding_id":"F1","violated_condition":"fixture",
        "reproduction":{"command":"fixture","input":"fixture"},"expected":"yes","actual":"no",
        "severity":"P2","confidence":"reproduced","original_log_ref":"logs/1.txt"}]);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &rejection).0, 202);
    event["event_id"] = json!("rejected");
    event["action"] = json!("reject");
    event["report_id"] = json!("reject-claim");
    event["report_revision"] = json!(1);
    assert_eq!(e.post("assigner", transition, &event).0, 201);

    // Crash boundary: Git receives the new commit, but the durable result update fails.
    e.db()
        .batch_execute(
            "CREATE FUNCTION lose_push_receipt() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'fixture lost result'; END; $$;
         CREATE TRIGGER lose_push_receipt BEFORE UPDATE ON github_write_intents
         FOR EACH ROW WHEN (OLD.request_id='correction') EXECUTE FUNCTION lose_push_receipt();",
        )
        .unwrap();
    assert_eq!(e.post_k("dev-a", push, "correction", &write).0, 500);
    assert_eq!(
        fixture_git(&remote, &["rev-parse", "refs/heads/topic/test"]),
        sha
    );
    assert_eq!(
        fixture_git(&remote, &["show", "refs/heads/topic/test:correction.txt"]),
        "new correction"
    );
    assert_eq!(e.post_k("dev-a", push, "other-key", &write).0, 409);
    e.db()
        .batch_execute("DROP TRIGGER lose_push_receipt ON github_write_intents")
        .unwrap();
    e.reopen_github_server(config.clone());
    let push_intent = json!({"repo_id":"repo-a","request_id":"correction"});
    assert_eq!(e.post("judge", reconcile, &push_intent).0, 202);
    let (status, confirmed) = e.post("judge", reconcile, &push_intent);
    assert_eq!(status, 200, "{confirmed}");
    assert_eq!(confirmed["outcome"], "pushed");
    assert_eq!(confirmed["reconciled"], true);
    assert_eq!(e.post("judge", reconcile, &push_intent).1, confirmed);
    assert_eq!(e.post_k("dev-a", push, "correction", &write).1, confirmed);
    assert_eq!(
        e.post_k("dev-a", push, "duplicate-new-key", &write).1,
        confirmed
    );

    for (caller, key, body) in [
        ("dev-a-other", "different-principal", write.clone()),
        ("dev-a", "different-assignment", {
            let mut body = write.clone();
            body["assignment_id"] = json!("other-assignment");
            body
        }),
        ("dev-a", "different-generation", {
            let mut body = write.clone();
            body["generation"] = json!(1);
            body
        }),
    ] {
        let (status, rejected) = e.post_k(caller, push, key, &body);
        assert_eq!(status, 409, "{rejected}");
        assert!(
            rejected
                .to_string()
                .contains("GITHUB_WRITE_RECEIPT_MISMATCH")
        );
    }

    assert_eq!(
        development(&e, 1, &sha, json!({"assignment_id":"D1","generation":0})).0,
        201
    );
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G2",
            0,
            "01-01",
            &sha,
            json!({"assignment_id":"G1","generation":0})
        )
        .0,
        201
    );
    write["generation"] = json!(1);
    event["event_id"] = json!("submit-correction");
    event["assignment_id"] = json!("G2");
    event["head_sha"] = json!(sha);
    event["action"] = json!("submit");
    event.as_object_mut().unwrap().remove("report_id");
    event.as_object_mut().unwrap().remove("report_revision");
    assert_eq!(e.post("assigner", transition, &event).0, 201);
    assert_eq!(e.post_k("dev-a", push, "pending-new", &write).0, 409);
    assert_eq!(e.post_k("judge", merge, "before-approval", &write).0, 409);
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "approved-claim", "G2", 0, "01-01", &sha)
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
                "sha":sha,"conclusion":"success","pull_request":7,"source_ref":"fixture/check"
            })
        )
        .0,
        201
    );
    let (status, refused) = e.post_k("judge", merge, "merge-raced", &write);
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["github_status"], 409, "{refused}");
    assert_eq!(e.post_k("judge", merge, "merge-raced", &write).1, refused);

    let (status, lost) = e.post_k("judge", merge, "merge-lost", &write);
    assert_eq!(status, 502, "{lost}");
    assert_eq!(lost["outcome"], "unknown");
    assert_eq!(e.post_k("judge", merge, "merge-another-key", &write).0, 409);
    e.reopen_github_server(config);
    let merge_intent = json!({"repo_id":"repo-a","request_id":"merge-lost"});
    for _ in 0..2 {
        let (status, pending) = e.post("judge", reconcile, &merge_intent);
        assert_eq!(status, 202, "{pending}");
        assert_eq!(pending["outcome"], "unknown");
        assert_eq!(
            e.post_k("judge", merge, "merge-still-blocked", &write).0,
            409
        );
        assert_eq!(
            e.post_k("judge", merge, "merge-lost", &write).1["outcome"],
            "unknown"
        );
    }
    let (status, reconciled) = e.post("judge", reconcile, &merge_intent);
    assert_eq!(status, 200, "{reconciled}");
    assert_eq!(reconciled["merged_sha"], MERGE);
    assert_eq!(e.post("judge", reconcile, &merge_intent).1, reconciled);
    assert_eq!(e.post_k("judge", merge, "merge-lost", &write).1, reconciled);
    assert_eq!(
        e.post_k("judge", merge, "merge-duplicate-new-key", &write)
            .1,
        reconciled
    );
    let (status, judgment) = judge(&e, "merge-reconciled", "01-01");
    assert_eq!(status, 201, "{judgment}");
    assert_eq!(judgment["merged_sha"], MERGE);
    assert_eq!(judgment["outcome"], "not_accepted");
    worker.join().unwrap();
    let seen = seen.lock().unwrap();
    let puts: Vec<_> = seen
        .iter()
        .filter(|(method, _, _)| method == "PUT")
        .collect();
    assert_eq!(puts.len(), 2);
    assert!(
        puts.iter()
            .all(|entry| entry.2 == json!({"sha":sha,"merge_method":"merge"}))
    );
}
