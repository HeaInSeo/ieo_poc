mod common;
use common::*;
use serde_json::{Value, json};
use std::process::Command;

const PLANS: &str = "/v0/coordinator/followups";
const RECONCILE: &str = "/v0/coordinator/followups/reconcile";

fn setup(e: &Env) -> Value {
    ready_sprint(e, 3);
    plan(e, Value::Null)
}

fn plan(e: &Env, previous: Value) -> Value {
    assert_eq!(
        assign(e, "development", "D1", 0, "01-01", SHA1, previous).0,
        201
    );
    assert_eq!(
        assign(e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "approved", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    json!({"plan_id":"successor","trigger_task":"01-01","assignment":{
        "repo_id":"repo-a","role":"development","assignment_id":"D2","generation":0,
        "sprint_id":"POC-01","sprint_revision":5,"task_id":"01-02","target_sha":SHA2,
        "expected_current":{"assignment_id":"D1","generation":0}
    }})
}

fn disposition(e: &Env) -> Value {
    e.db()
        .query_one(
            "SELECT disposition FROM followup_plans WHERE plan_id='successor'",
            &[],
        )
        .unwrap()
        .get(0)
}

fn count(e: &Env) -> i64 {
    e.db()
        .query_one(
            "SELECT count(*) FROM assignments WHERE assignment_id='D2'",
            &[],
        )
        .unwrap()
        .get(0)
}

fn run_watcher(e: &Env) -> String {
    let dir = tempfile::tempdir().unwrap();
    let token = dir.path().join("token");
    std::fs::write(&token, &e.tokens["watcher"]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ieo-followup-watch"))
        .arg(format!("http://{}", e.addr))
        .arg(&token)
        .args(["repo-a", "60", "--once"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn accepted_verdict_assigns_without_a_periodic_tick() {
    let Some(e) = start() else { return };
    let plan = setup(&e);
    assert_eq!(e.post("watcher", PLANS, &plan).0, 403);
    assert_eq!(e.post("assigner", PLANS, &plan).0, 201);
    assert_eq!(
        e.post("watcher", RECONCILE, &json!({"repo_id":"repo-b"})).0,
        403
    );
    assert_eq!(
        e.post("assigner", RECONCILE, &json!({"repo_id":"repo-a"}))
            .0,
        403
    );
    assert_eq!(
        judge(&e, "missing-ci", "01-01").1["outcome"],
        "not_accepted"
    );
    assert_eq!(
        e.post("watcher", RECONCILE, &json!({"repo_id":"repo-a"})).0,
        200
    );
    assert_eq!(disposition(&e)["reason"], "predecessor_not_accepted");
    assert_eq!(count(&e), 0);
    full_evidence(&e, SHA1, "complete");
    let accepted = judge(&e, "verified", "01-01").1;
    assert_eq!(accepted["outcome"], "accepted", "{accepted}");
    assert_eq!(count(&e), 1);
    let state = disposition(&e);
    assert_eq!(state["cause"], "event");
    assert_eq!(state["event_id"], accepted["event_id"]);
    assert_eq!(judge(&e, "verified", "01-01").0, 200);
    run_watcher(&e);
    assert_eq!(disposition(&e), state);
    assert_eq!(count(&e), 1);
}

#[test]
fn restarted_watcher_recovers_a_missed_event_without_reissuing() {
    let Some(mut e) = start() else { return };
    let plan = setup(&e);
    full_evidence(&e, SHA1, "complete");
    assert_eq!(judge(&e, "before-plan", "01-01").1["outcome"], "accepted");
    assert_eq!(e.post("assigner", PLANS, &plan).0, 201);
    assert_eq!(count(&e), 0);
    e.db()
        .batch_execute(
            "CREATE FUNCTION fail_followup_write() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'injected followup commit failure'; END; $$;
         CREATE TRIGGER fail_followup_write BEFORE UPDATE ON followup_plans
         FOR EACH ROW WHEN (NEW.assigned_at IS NOT NULL) EXECUTE FUNCTION fail_followup_write();",
        )
        .unwrap();
    assert_eq!(
        e.post("watcher", RECONCILE, &json!({"repo_id":"repo-a"})).0,
        500
    );
    assert_eq!(count(&e), 0);
    assert_eq!(disposition(&e)["state"], "waiting");
    e.db()
        .batch_execute("DROP TRIGGER fail_followup_write ON followup_plans")
        .unwrap();
    run_watcher(&e);
    let state = disposition(&e);
    assert_eq!(state["cause"], "reconcile");
    e.reopen_server(None);
    assert_eq!(judge(&e, "before-plan", "01-01").0, 200);
    assert_eq!(count(&e), 1);
    run_watcher(&e);
    assert_eq!(count(&e), 1);
    assert_eq!(disposition(&e), state);
}

#[test]
fn verdict_and_watcher_compete_on_one_durable_assignment() {
    let Some(e) = start() else { return };
    assert_eq!(e.post("assigner", PLANS, &setup(&e)).0, 201);
    full_evidence(&e, SHA1, "complete");
    let mut db = e.db();
    let mut tx = db.transaction().unwrap();
    tx.query_one(
        "SELECT plan_id FROM followup_plans WHERE plan_id='successor' FOR UPDATE",
        &[],
    )
    .unwrap();
    std::thread::scope(|scope| {
        let event = scope.spawn(|| judge(&e, "racing-event", "01-01"));
        let began = std::time::Instant::now();
        while !e
            .db()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM judgments WHERE judgment_id='racing-event')",
                &[],
            )
            .unwrap()
            .get::<_, bool>(0)
        {
            assert!(began.elapsed() < std::time::Duration::from_secs(3));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let periodic = scope.spawn(|| run_watcher(&e));
        let observer = &mut e.db();
        let began = std::time::Instant::now();
        loop {
            let blocked: i64 = observer.query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database()
                   AND wait_event_type='Lock' AND query LIKE '%FROM followup_plans WHERE repo_id=%FOR UPDATE%'",
                &[],
            ).unwrap().get(0);
            if blocked == 2 {
                break;
            }
            assert!(
                began.elapsed() < std::time::Duration::from_secs(4),
                "waiting paths: {blocked}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        tx.commit().unwrap();
        assert_eq!(event.join().unwrap().1["outcome"], "accepted");
        periodic.join().unwrap();
    });
    assert_eq!(count(&e), 1);
    let assigned = disposition(&e);
    assert_eq!(assigned["state"], "assigned");
    run_watcher(&e);
    assert_eq!(disposition(&e), assigned);
    assert_eq!(count(&e), 1);
}

#[test]
fn the_last_required_verdict_wakes_a_waiting_plan_without_a_watcher() {
    last_verdict("accepted");
}

#[test]
fn recorded_but_not_accepted_last_result_also_wakes_the_plan() {
    last_verdict("result_recorded");
}

fn last_verdict(required: &str) {
    let Some(e) = start() else { return };
    let mut spec = spec("repo-a", 5, 3);
    let tasks = spec["tasks"].as_array_mut().unwrap();
    let other = tasks
        .iter_mut()
        .find(|task| task["task_id"] == "01-07")
        .unwrap();
    other["prerequisites"] = json!([]);
    let outputs = other["outputs"].as_array().unwrap().clone();
    tasks[1]["prerequisites"]
        .as_array_mut()
        .unwrap()
        .push(json!({"task_id":"01-07","plan_version":"v1.3.3","required_state":required}));
    spec["digest"] = json!(ieo_spec::spec_digest(&spec));
    assert_eq!(
        e.post_k(
            "alice",
            "/v0/sprints?mode=executable",
            "two-prerequisites",
            &spec
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &json!({"decision":"passed","reason":"fixture","digest":spec["digest"]})
        )
        .0,
        201
    );
    assert_eq!(
        assign(&e, "development", "old", 0, "01-07", SHA3, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "assigner",
            PLANS,
            &plan(&e, json!({"assignment_id":"old","generation":0}))
        )
        .0,
        201
    );
    full_evidence(&e, SHA1, "trigger-complete");
    assert_eq!(judge(&e, "trigger-ready", "01-01").1["outcome"], "accepted");
    assert_eq!(count(&e), 0);
    assert_eq!(
        disposition(&e)["reason"]["rejections"][0]["code"],
        "PREREQUISITE_UNMET"
    );

    if required == "accepted" {
        for (id, kind, body) in [
            (
                "other-ci",
                "ci_run",
                json!({"sha":SHA3,"conclusion":"success","source_ref":"fixture/ci"}),
            ),
            (
                "other-review",
                "review",
                json!({"sha":SHA3,"reviewer":"independent","author":"developer","state":"approved","source_ref":"fixture/review"}),
            ),
            (
                "other-merge",
                "merge",
                json!({"head_sha":SHA3,"merged_sha":MERGE,"source_ref":"fixture/pr"}),
            ),
            (
                "other-followups",
                "followups",
                json!({"sha":MERGE,"task_id":"01-07","accepted":true,"ref":"fixture/followups"}),
            ),
        ] {
            assert_eq!(evidence(&e, id, kind, body).0, 201);
        }
        for (index, output) in outputs.iter().filter(|o| o["required"] == true).enumerate() {
            assert_eq!(evidence(&e, &format!("other-output-{index}"), "output",
            json!({"sha":SHA3,"task_id":"01-07","artifact_id":output["artifact_id"],"ref":"fixture/output"})).0, 201);
        }
    }
    let last = judge(&e, "other-ready", "01-07").1;
    assert_eq!(
        last["outcome"],
        if required == "accepted" {
            "accepted"
        } else {
            "not_accepted"
        },
        "{last}"
    );
    assert_eq!(count(&e), 1);
    assert_eq!(disposition(&e)["event_id"], last["event_id"]);
    assert!(e.db().query_one(
        "SELECT (p.disposition->>'prerequisite_at')::timestamptz <= e.stored_at
           AND e.stored_at <= (p.disposition->>'event_received_at')::timestamptz
           AND (p.disposition->>'event_received_at')::timestamptz <= (p.disposition->>'eligible_at')::timestamptz
           AND (p.disposition->>'eligible_at')::timestamptz <= p.assigned_at
         FROM followup_plans p JOIN events e ON e.event_id=p.disposition->>'event_id'
         WHERE p.plan_id='successor'",
        &[],
    ).unwrap().get::<_, bool>(0));
}

#[test]
fn attempt_ceiling_stays_waiting_with_one_durable_decision_request() {
    let Some(mut e) = start() else { return };
    assert_eq!(e.post("assigner", PLANS, &setup(&e)).0, 201);
    e.db().execute("INSERT INTO task_attempts (sprint_id, task_id, role, issued) VALUES('POC-01','01-02','development',4)", &[]).unwrap();
    full_evidence(&e, SHA1, "limit");
    assert_eq!(judge(&e, "at-limit", "01-01").1["outcome"], "accepted");
    let state = disposition(&e);
    assert_eq!(state["state"], "waiting");
    let id = state["reason"]["decision_event_id"].as_str().unwrap();
    e.reopen_server(None);
    run_watcher(&e);
    run_watcher(&e);
    assert_eq!(disposition(&e), state);
    assert_eq!(count(&e), 0);
    assert_eq!(e.db().query_one(
        "SELECT count(*) FROM events e JOIN outbox o ON o.sprint_id=e.sprint_id AND o.sprint_seq=e.sprint_seq WHERE e.event_id=$1",
        &[&id],
    ).unwrap().get::<_,i64>(0), 1);
}

#[test]
fn responsive_watcher_distinguishes_stalled_delivery_from_waiting() {
    let Some(e) = start() else { return };
    assert_eq!(e.post("assigner", PLANS, &setup(&e)).0, 201);
    assert!(run_watcher(&e).is_empty());
    full_evidence(&e, SHA1, "observe");
    assert_eq!(judge(&e, "observe", "01-01").1["outcome"], "accepted");
    assert!(run_watcher(&e).is_empty());
    ensure_delivery(
        &e,
        &serde_json::to_vec(&json!({
            "delivery_id":"D-t-followup","phase":"takeover","repo_id":"repo-a","role":"development",
            "assignment_id":"D2","generation":0,"session_id":"fixture"
        }))
        .unwrap(),
    );
    let mut db = e.db();
    db.execute(
        "INSERT INTO role_delivery_results(delivery_id,n,reasons,outcome,recorded_at)
       VALUES('D-t-followup',1,'[]','budget_refused',now()-interval '2 minutes')",
        &[],
    )
    .unwrap();
    assert!(run_watcher(&e).is_empty());
    db.execute(
        "INSERT INTO role_delivery_results(delivery_id,n,reasons,outcome,recorded_at)
       VALUES('D-t-followup',2,'[]','accepted',now()-interval '2 minutes')",
        &[],
    )
    .unwrap();
    assert!(run_watcher(&e).contains("followup stalled before first action: D-t-followup"));
    db.execute("INSERT INTO role_first_actions(repo_id,role,assignment_id,generation,attempt_id,session_id,
       event_offset,observed_at,kind,body,current_assignment,reported_by)
       VALUES('repo-a','development','D2',0,'D-t-followup','fixture',1,now(),'text','{}',true,'fixture')", &[]).unwrap();
    assert!(run_watcher(&e).is_empty());
}
