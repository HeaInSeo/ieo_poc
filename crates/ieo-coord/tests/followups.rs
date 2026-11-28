mod common;
use common::*;
use serde_json::{Value, json};
use std::process::Command;

const PLANS: &str = "/v0/coordinator/followups";
const RECONCILE: &str = "/v0/coordinator/followups/reconcile";

fn setup(e: &Env) -> Value {
    ready_sprint(e, 3);
    assert_eq!(
        assign(e, "development", "D1", 0, "01-01", SHA1, Value::Null).0,
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

fn run_watcher(e: &Env) {
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
