//! `ieo-role-call --plan <plan.json> --task <id> --role <role> --op start|reconnect
//!  --attempt <id> --session <id> --prompt-file <f> --coordinator-file <f>`
//!
//! One budgeted call to a role service (see `ieo_supervisor::caller`). Prints a summary
//! without secrets. Exit 0: recorded, nothing to look at; 2: nothing started (refused or held);
//! 3: recorded with `stop_required`; 1: an error (bad arguments, or a failure the caller could
//! not record — its in-flight marker then blocks every further call). **Anything but 0 or 2
//! means stop**: a person decides before the next call.
//!
//! `ieo-role-call reevaluate --plan <plan.json> --attempt <id> --decision <ref>
//!  [--child-transcript <agentId>=<path>]...`: re-evaluate a recorded run from its kept stream
//! and child transcripts (no model call) and append a correction if it is now measured and
//! consistent (exit 0); otherwise nothing is written (exit 1).

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
    match caller::call(&plan, &c, &caller::Http, Duration::from_secs(2))? {
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
