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

/// An answer from a role supervisor: HTTP status and body, or why it could not be reached.
type Answer = Result<(u16, Value), String>;

pub(crate) fn call(target: &Value, method: &str, path: &str) -> Answer {
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
    let groups = quota_groups(t, &out);
    (
        200,
        json!({"repo_id": repo, "roles": out, "quota_groups": groups}),
    )
}

/// Provider status per quota group (runtime contract §3, §7; 02-04 decision item 4). Roles whose
/// credentials share a provider limit carry the same `quota_group_id`; the limit is one, so a
/// `usage_limit` seen by any member applies to every member (`affected_roles`). The group is
/// `available` only when a member observed it and none reports `usage_limit`; otherwise
/// `unknown`. `auth_or_billing` belongs to one credential and is listed per role, not spread.
/// When the coordinator's `--roles` entry names the expected group, a role reporting another one
/// (or none) is flagged.
pub fn quota_groups(t: &Value, out: &serde_json::Map<String, Value>) -> Value {
    let mut groups: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();
    let mut mismatches = Vec::new();
    for r in ROLES {
        let s = &out[r]["status"];
        let reported = s["quota_group_id"].as_str();
        let expected = t[r]["quota_group_id"].as_str();
        if let Some(e) = expected
            && reported != Some(e)
        {
            mismatches.push(json!({"role": r, "expected": e, "reported": reported}));
        }
        let Some(g) = reported.or(expected) else {
            continue;
        };
        let ps = s["provider_status_or_unknown"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        groups
            .entry(g.to_string())
            .or_default()
            .push((r.to_string(), ps));
    }
    let list: Vec<Value> = groups
        .into_iter()
        .map(|(g, members)| {
            let any = |v: &str| members.iter().any(|(_, p)| p == v);
            let status = if any("usage_limit") {
                "usage_limit"
            } else if any("available") {
                "available"
            } else {
                "unknown"
            };
            json!({"quota_group_id": g,
                   "roles": members.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),
                   "provider_status_or_unknown": status,
                   "affected_roles": if status == "usage_limit" {
                       json!(members.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>()) } else { json!([]) },
                   "auth_or_billing_roles": members.iter().filter(|(_, p)| p == "auth_or_billing")
                       .map(|(r, _)| r.clone()).collect::<Vec<_>>(),
                   "member_status": members.iter().map(|(r, p)| json!({"role": r, "provider_status_or_unknown": p}))
                       .collect::<Vec<_>>()})
        })
        .collect();
    json!({"groups": list, "mismatches": mismatches})
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
    // the stop goes to both roles first (neither waits for the other, review adf760e M-1); a
    // stop that did not reach a role or was refused is reported at once, not waited for
    let sent: Vec<(&str, Answer)> = ROLES
        .iter()
        .map(|r| (*r, call(&t[*r], "POST", "/v0/role/stop")))
        .collect();
    let t0 = Instant::now();
    for (r, sent) in sent {
        let last = match &sent {
            Err(e) => Err(e.clone()),
            Ok((st, b)) if *st != 200 && *st != 202 => Ok((*st, b.clone())),
            Ok(_) => loop {
                let s = call(&t[r], "GET", "/v0/role/status");
                match &s {
                    Ok((200, v)) if v["running"].is_null() => break s,
                    _ if t0.elapsed() >= wait => break s,
                    _ => std::thread::sleep(poll),
                }
            },
        };
        let report = match (&sent, &last) {
            (Ok((st, _)), Ok((200, v))) => {
                let safe = &v["safe_shutdown"];
                if !v["running"].is_null() {
                    reasons.push(format!("{r}: still running after {} s", wait.as_secs()));
                }
                // the central record must hold the saved checkpoint (§2.1, decision item 9)
                if safe["checkpoint_reported"] == false {
                    reasons.push(format!("{r}: checkpoint not reported to the coordinator"));
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
                reasons.push(format!("{r}: stop or status answered {st}"));
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
