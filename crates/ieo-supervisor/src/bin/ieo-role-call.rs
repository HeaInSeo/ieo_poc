//! `ieo-role-call --plan <plan.json> --task <id> --role <role> --op start|reconnect
//!  --attempt <id> --session <id> --prompt-file <f> --coordinator-file <f> [--recovery-id <id>]
//!  [--spare-reason <text> --replaces <attempt>]`
//!
//! One budgeted call to a role service (see `ieo_supervisor::caller`). `--recovery-id`: a start
//! over a run that did not finish carries a person's recovery id (02-05 step 7b; the service
//! refuses such a start without one). `--spare-reason`/`--replaces`: the 02-05 budget's spare
//! run (user decision 2026-11-17 (나)), both or neither; the shared budget check decides. Prints a summary
//! without secrets. Exit 0: recorded, nothing to look at; 2: nothing started (refused or held);
//! 3: recorded with `stop_required`; 1: an error (bad arguments, or a failure the caller could
//! not record — its in-flight marker then blocks every further call). **Anything but 0 or 2
//! means stop**: a person decides before the next call.
//!
//! `ieo-role-call reevaluate --plan <plan.json> --attempt <id> --decision <ref>
//!  [--child-transcript <agentId>=<path>]...`: re-evaluate a recorded run from its kept stream
//! and child transcripts (no model call) and append a correction if it is now measured and
//! consistent (exit 0); otherwise nothing is written (exit 1).
//!
//! `ieo-role-call check --plan <plan.json>`: the guards of a call without a call, read only
//! (`caller::check`): no call in flight, the ledger reads with every correction measured
//! again, no record asks for a person, and every role's service is idle with its last attempt
//! and checkpoint attempt known. Exit 0: clear; 2: refused (the reason is printed); 1: an
//! error. `call.sh` runs it under `caller.lock` before a container restart.

use ieo_supervisor::caller::{self, Call, Called, Plan};
use serde_json::{Value, json};
use std::time::Duration;

fn main() {
    std::process::exit(match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ieo-role-call: {e}");
            1
        }
    });
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .ok_or(format!("{k} missing"))
    };
    let read = |k: &str| -> Result<String, String> {
        let p = get(k)?;
        std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))
    };
    let plan_v: Value = serde_json::from_str(&read("--plan")?).map_err(|e| e.to_string())?;
    let plan = Plan::from_json(&plan_v)?;
    if args.first().map(String::as_str) == Some("check") {
        return Ok(match caller::check(&plan, &caller::Http)? {
            None => {
                println!("{}", json!({"clear": true}));
                0
            }
            Some(why) => {
                println!("{}", json!({"clear": false, "reason": why}));
                2
            }
        });
    }
    if args.first().map(String::as_str) == Some("reevaluate") {
        let mut transcripts = std::collections::BTreeMap::new();
        for (i, a) in args.iter().enumerate() {
            if a == "--child-transcript" {
                let v = args.get(i + 1).ok_or("--child-transcript value")?;
                let (id, p) = v
                    .split_once('=')
                    .ok_or("--child-transcript AGENT_ID=PATH")?;
                transcripts.insert(id.to_owned(), std::path::PathBuf::from(p));
            }
        }
        let c = caller::reevaluate(&plan, &get("--attempt")?, &transcripts, &get("--decision")?)?;
        println!(
            "{}",
            json!({"corrected": c["corrects"], "before": c["before"]["usage_run"],
                   "after": c["after"]["usage"]["run"], "attribution": c["after"]["usage"]["attribution_check"],
                   "decision": c["decision"]})
        );
        return Ok(0);
    }
    let c = Call {
        task_id: get("--task")?,
        role: get("--role")?,
        op: get("--op")?,
        attempt_id: get("--attempt")?,
        session_id: get("--session")?,
        prompt: read("--prompt-file")?,
        coordinator: serde_json::from_str(&read("--coordinator-file")?)
            .map_err(|e| format!("coordinator: {e}"))?,
    };
    let mut extra = json!({});
    if let Some(i) = args.iter().position(|a| a == "--recovery-id") {
        let id = args.get(i + 1).ok_or("--recovery-id value")?;
        let ok = (1..=64).contains(&id.len())
            && id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok {
            return Err("--recovery-id: 1-64 of [A-Za-z0-9_-]".into());
        }
        extra["recovery_id"] = json!(id);
    }
    // a flag's value is the next argument, never another flag (review b38e8df I-2)
    let flag = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .map(|i| {
                args.get(i + 1)
                    .filter(|v| !v.starts_with("--"))
                    .cloned()
                    .ok_or(format!("{k} value"))
            })
            .transpose()
    };
    let spare = [
        flag("--spare-reason")?,
        flag("--replaces")?,
        flag("--evidence-ref")?,
        flag("--approver")?,
    ];
    match spare {
        [None, None, None, None] => {}
        [
            Some(reason),
            Some(replaces),
            Some(evidence_ref),
            Some(approver),
        ] => {
            for (k, v, max) in [
                ("--spare-reason", &reason, 2000),
                ("--evidence-ref", &evidence_ref, 2000),
                ("--approver", &approver, 128),
            ] {
                if v.trim().is_empty() || v.len() > max {
                    return Err(format!("{k}: 1-{max} bytes"));
                }
            }
            // the person who approves the spare names themself; this path is inside the trust
            // boundary (like --recovery-id) and has no authenticated identity: the record says
            // so and keeps the local user name too (review b38e8df M-1, user decision
            // 2026-11-17 addition)
            let host_user = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
            extra["spare"] = json!({"reason": reason, "replaces": replaces,
                "evidence_ref": evidence_ref,
                "by": {"kind": "user", "name": approver, "authenticated": false,
                       "via": "ieo-role-call", "host_user": host_user},
                "approved_at": ieo_evidence::time::now()});
        }
        _ => {
            return Err(
                "--spare-reason, --replaces, --evidence-ref and --approver go together".into(),
            );
        }
    }
    match caller::call_with_extra(
        &plan,
        &c,
        &caller::Http,
        Duration::from_secs(2),
        Duration::from_secs(60),
        &extra,
    )? {
        Called::NotStarted(why) => {
            println!("{}", json!({"started": false, "reason": why}));
            Ok(2)
        }
        Called::Recorded(r) => {
            let summary = json!({"started": true, "attempt_id": r["attempt_id"],
                "role": r["role"], "session": r["session"], "outcome": r["outcome"]["stop"],
                "duration_ms": r["duration_ms"], "usage_run": r["usage"]["run"],
                "result": r["outcome"]["result"],
                "stop_required": r["stop_required"], "stop_reason": r["stop_reason"],
                "records": caller::records_path(&plan)});
            println!("{summary}");
            Ok(if r["stop_required"] == true { 3 } else { 0 })
        }
    }
}
