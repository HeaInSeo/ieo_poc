//! Repo-scope control of the role supervisors (runtime contract §7, 02-04): the coordinator
//! asks both role supervisors of a repo for their status and sends each a role-scope `stop`;
//! each role's safe-shutdown report is returned separately. The coordinator never stops one
//! role through the other. Start and reconnect come with assignments (02-05).
//!
//! Configuration (`--roles FILE`): `{"<repo_id>": {"development": {"url": "http://…",
//! "adapter_token_file": "…"}, "guardrail": {…}}}`. Token values are read per call and never
//! logged or returned.

use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub const ROLES: [&str; 2] = ["development", "guardrail"];

fn call(target: &Value, method: &str, path: &str) -> Result<(u16, Value), String> {
    let url = target["url"].as_str().ok_or("role url missing")?;
    let file = target["adapter_token_file"]
        .as_str()
        .ok_or("role adapter_token_file missing")?;
    let token =
        std::fs::read_to_string(file).map_err(|e| format!("adapter token: {}", e.kind()))?;
    let req = ureq::request(method, &format!("{url}{path}"))
        .timeout(Duration::from_secs(10))
        .set("Authorization", &format!("Bearer {}", token.trim()));
    let r = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(format!("transport: {}", e.kind())),
    };
    let st = r.status();
    let v = r
        .into_json()
        .map_err(|e| format!("response: {}", e.kind()))?;
    Ok((st, v))
}

fn targets<'a>(roles: &'a Value, repo: &str) -> Result<&'a Value, (u16, Value)> {
    let t = &roles[repo];
    if !t.is_object() {
        return Err((
            404,
            json!({"code": "NOT_FOUND", "message": "no role supervisors configured for this repo"}),
        ));
    }
    Ok(t)
}

/// Status of both roles of `repo`.
pub fn status(roles: &Value, repo: &str) -> (u16, Value) {
    let t = match targets(roles, repo) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let mut out = serde_json::Map::new();
    for r in ROLES {
        out.insert(
            r.into(),
            match call(&t[r], "GET", "/v0/role/status") {
                Ok((200, s)) => json!({"reachable": true, "status": s}),
                Ok((st, b)) => json!({"reachable": true, "http_status": st, "body": b}),
                Err(e) => json!({"reachable": false, "error": e}),
            },
        );
    }
    (200, json!({"repo_id": repo, "roles": out}))
}

/// Repo-scope stop: a role-scope `stop` to each supervisor, then each role's status until it no
/// longer runs (bounded by `wait`). Safe shutdown is confirmed only when every role answered, runs
/// nothing, has its checkpoint saved, and reports its children and external requests — which the
/// supervisors do not track yet, so it is reported unconfirmed with the reasons.
pub fn stop(roles: &Value, repo: &str, wait: Duration, poll: Duration) -> (u16, Value) {
    let t = match targets(roles, repo) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let mut out = serde_json::Map::new();
    let mut reasons: Vec<String> = Vec::new();
    for r in ROLES {
        let sent = call(&t[r], "POST", "/v0/role/stop");
        let t0 = Instant::now();
        // a role the stop could not reach is reported at once, not waited for
        let last = if let Err(e) = &sent {
            Err(e.clone())
        } else {
            loop {
                let s = call(&t[r], "GET", "/v0/role/status");
                match &s {
                    Ok((200, v)) if v["running"].is_null() => break s,
                    _ if t0.elapsed() >= wait => break s,
                    _ => std::thread::sleep(poll),
                }
            }
        };
        let report = match (&sent, &last) {
            (Ok((st, _)), Ok((200, v))) => {
                let safe = &v["safe_shutdown"];
                if !v["running"].is_null() {
                    reasons.push(format!("{r}: still running after {} s", wait.as_secs()));
                }
                if safe["checkpoint_saved"] != true {
                    reasons.push(format!(
                        "{r}: checkpoint not saved ({})",
                        safe["checkpoint_phase"]
                    ));
                }
                for k in ["active_children", "pending_external_requests"] {
                    if safe[k] != json!([]) && safe[k] != json!(0) {
                        reasons.push(format!("{r}: {k} {}", safe[k]));
                    }
                }
                json!({"stop_http_status": st, "running_after": !v["running"].is_null(),
                       "safe_shutdown": safe})
            }
            (Err(e), _) | (_, Err(e)) => {
                reasons.push(format!("{r}: unreachable ({e})"));
                json!({"reachable": false, "error": e})
            }
            (_, Ok((st, b))) => {
                reasons.push(format!("{r}: status {st}"));
                json!({"http_status": st, "body": b})
            }
        };
        out.insert(r.into(), report);
    }
    (
        200,
        json!({"repo_id": repo, "roles": out,
               "safe_shutdown_confirmed": reasons.is_empty(), "unconfirmed": reasons}),
    )
}
