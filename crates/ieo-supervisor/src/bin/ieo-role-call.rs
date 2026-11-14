//! `ieo-role-call --plan <plan.json> --task <id> --role <role> --op start|reconnect
//!  --attempt <id> --session <id> --prompt-file <f> --coordinator-file <f>`
//!
//! One budgeted call to a role service (see `ieo_supervisor::caller`). Prints a summary
//! without secrets. Exit 0: recorded, nothing to look at; 2: nothing started (refused or held);
//! 3: recorded with `stop_required` (a person decides before the next call); 1: usage error.

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
                "stop_required": r["stop_required"], "stop_reason": r["stop_reason"],
                "records": caller::records_path(&plan)});
            println!("{summary}");
            Ok(if r["stop_required"] == true { 3 } else { 0 })
        }
    }
}
