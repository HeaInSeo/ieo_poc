//! Repo-scope role control against fake role supervisors over HTTP (no database, no model).

use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "adapter-token";

/// A fake role supervisor: `running` until a stop arrives (it then ends after `stop_ms`),
/// reporting a safe-shutdown block. Records the requests it saw.
fn fake(name: &'static str, stop_ms: u64, children: Value) -> (String, Arc<Mutex<Vec<String>>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", server.server_addr().to_ip().expect("ip"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        let mut stopped_at: Option<std::time::Instant> = None;
        for req in server.incoming_requests() {
            let auth = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.as_str().to_string());
            log.lock()
                .expect("log")
                .push(format!("{} {}", req.method(), req.url()));
            if auth.as_deref() != Some(&format!("Bearer {TOKEN}")) {
                let _ = req.respond(tiny_http::Response::from_string("{}").with_status_code(401));
                continue;
            }
            let running = stopped_at.is_none_or(|t| t.elapsed() < Duration::from_millis(stop_ms));
            let body = if req.url() == "/v0/role/stop" {
                stopped_at.get_or_insert_with(std::time::Instant::now);
                json!({"stopping": {"attempt_id": "a1"}})
            } else {
                json!({"role": name, "running": if running { json!({"attempt_id": "a1"}) } else { Value::Null },
                       "safe_shutdown": {"running": running, "checkpoint_phase": if running { "takeover" } else { "after_run" },
                                         "checkpoint_saved": !running, "active_children": children,
                                         "pending_external_requests": "not_tracked"}})
            };
            let _ = req.respond(tiny_http::Response::from_string(body.to_string()));
        }
    });
    (url, seen)
}

fn roles(dev: &str, gr: &str, dir: &std::path::Path) -> Value {
    let f = dir.join("token");
    std::fs::write(&f, TOKEN).expect("token");
    json!({"repo-a": {"development": {"url": dev, "adapter_token_file": f},
                      "guardrail": {"url": gr, "adapter_token_file": f}}})
}

fn tmp(n: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ieo-roles-{n}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("dir");
    d
}

#[test]
fn repo_stop_sends_a_role_stop_to_each_and_reports_each_role() {
    let d = tmp("stop");
    let (dev, dev_seen) = fake("development", 300, json!("not_tracked"));
    let (gr, gr_seen) = fake("guardrail", 100, json!("not_tracked"));
    let r = roles(&dev, &gr, &d);
    let (st, b) = ieo_coord::roles::stop(
        &r,
        "repo-a",
        Duration::from_secs(5),
        Duration::from_millis(50),
    );
    assert_eq!(st, 200);
    for (role, seen) in [("development", &dev_seen), ("guardrail", &gr_seen)] {
        let s = seen.lock().expect("seen");
        assert!(s.iter().any(|x| x == "POST /v0/role/stop"), "{role}: {s:?}");
        assert_eq!(b["roles"][role]["running_after"], false, "{b}");
        assert_eq!(
            b["roles"][role]["safe_shutdown"]["checkpoint_saved"], true,
            "{b}"
        );
    }
    // children and external requests are not tracked: stopped, but not confirmed safe
    assert_eq!(b["safe_shutdown_confirmed"], false, "{b}");
    let u: Vec<String> = serde_json::from_value(b["unconfirmed"].clone()).expect("u");
    assert!(
        u.iter().any(|x| x.contains("development: active_children")),
        "{u:?}"
    );
    assert!(
        u.iter()
            .any(|x| x.contains("guardrail: pending_external_requests")),
        "{u:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_role_that_does_not_stop_in_time_or_is_unreachable_is_reported() {
    let d = tmp("slow");
    let (dev, _) = fake("development", 60_000, json!([]));
    let r = roles(&dev, "http://127.0.0.1:9", &d);
    let (_, b) = ieo_coord::roles::stop(
        &r,
        "repo-a",
        Duration::from_millis(400),
        Duration::from_millis(50),
    );
    assert_eq!(b["roles"]["development"]["running_after"], true, "{b}");
    assert_eq!(b["roles"]["guardrail"]["reachable"], false, "{b}");
    let u: Vec<String> = serde_json::from_value(b["unconfirmed"].clone()).expect("u");
    assert!(
        u.iter().any(|x| x.contains("development: still running")),
        "{u:?}"
    );
    assert!(
        u.iter().any(|x| x.contains("guardrail: unreachable")),
        "{u:?}"
    );
    // an unknown repo
    assert_eq!(ieo_coord::roles::status(&r, "repo-z").0, 404);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn status_reads_both_roles_with_the_adapter_token() {
    let d = tmp("status");
    let (dev, _) = fake("development", 0, json!([]));
    let (gr, _) = fake("guardrail", 0, json!([]));
    let r = roles(&dev, &gr, &d);
    let (st, b) = ieo_coord::roles::status(&r, "repo-a");
    assert_eq!(st, 200);
    assert_eq!(
        b["roles"]["development"]["status"]["role"], "development",
        "{b}"
    );
    assert_eq!(
        b["roles"]["guardrail"]["status"]["role"], "guardrail",
        "{b}"
    );
    // a wrong token is the supervisor's 401, reported as such (the token is read per call)
    std::fs::write(d.join("token"), "wrong").expect("w");
    let (_, b) = ieo_coord::roles::status(&r, "repo-a");
    assert_eq!(b["roles"]["development"]["http_status"], 401, "{b}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn both_roles_get_their_stop_before_either_is_waited_for() {
    // M-1: guardrail's stop arrives while development is still stopping
    let d = tmp("order");
    let (dev, dev_seen) = fake("development", 1_000, json!("not_tracked"));
    let (gr, gr_seen) = fake("guardrail", 100, json!("not_tracked"));
    let r = roles(&dev, &gr, &d);
    let t0 = std::time::Instant::now();
    let h = std::thread::spawn(move || {
        ieo_coord::roles::stop(
            &r,
            "repo-a",
            Duration::from_secs(5),
            Duration::from_millis(50),
        )
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        gr_seen
            .lock()
            .expect("s")
            .iter()
            .any(|x| x == "POST /v0/role/stop"),
        "guardrail not stopped yet"
    );
    assert!(
        t0.elapsed() < Duration::from_millis(900),
        "the check ran too late to tell"
    );
    let (_, b) = h.join().expect("join");
    assert_eq!(b["roles"]["development"]["running_after"], false, "{b}");
    assert!(
        dev_seen
            .lock()
            .expect("s")
            .iter()
            .any(|x| x == "POST /v0/role/stop")
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_refused_stop_is_reported_without_waiting() {
    // M-1: a token mismatch (401) is not polled for the whole wait
    let d = tmp("refused");
    let (dev, _) = fake("development", 0, json!([]));
    let (gr, _) = fake("guardrail", 0, json!([]));
    let r = roles(&dev, &gr, &d);
    std::fs::write(d.join("token"), "wrong").expect("w");
    let t0 = std::time::Instant::now();
    let (_, b) = ieo_coord::roles::stop(
        &r,
        "repo-a",
        Duration::from_secs(10),
        Duration::from_millis(50),
    );
    assert!(
        t0.elapsed() < Duration::from_secs(3),
        "waited for a refused stop"
    );
    assert_eq!(b["roles"]["development"]["http_status"], 401, "{b}");
    assert_eq!(b["safe_shutdown_confirmed"], false);
    let u: Vec<String> = serde_json::from_value(b["unconfirmed"].clone()).expect("u");
    assert!(
        u.iter()
            .any(|x| x.contains("development: stop or status answered 401")),
        "{u:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_unreachable_role_is_not_waited_for() {
    let d = tmp("nowait");
    let (dev, _) = fake("development", 0, json!([]));
    let r = roles(&dev, "http://127.0.0.1:9", &d);
    let t0 = std::time::Instant::now();
    let (_, b) = ieo_coord::roles::stop(
        &r,
        "repo-a",
        Duration::from_secs(10),
        Duration::from_millis(50),
    );
    assert!(
        t0.elapsed() < Duration::from_secs(5),
        "waited for an unreachable role"
    );
    assert_eq!(b["roles"]["guardrail"]["reachable"], false, "{b}");
    let _ = std::fs::remove_dir_all(&d);
}
