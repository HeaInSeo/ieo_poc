//! The role's pending external requests (02-05 step 6, PLAN §3.2 step 6; runtime contract
//! revision 13 §16).
//!
//! **What is tracked.** The requests the role sends to the coordinator through this supervisor
//! and whose answer is not known: checkpoint reports (`checkpoint_report`, a staged takeover
//! included) and first-action reports (`first_action_report`). An entry is added before the
//! request is sent and removed when the coordinator answers (stored, duplicate, or refused with
//! a 4xx) or when a later re-query shows the outcome (the central comparison before a start or
//! reconnect; a first-action report is sent again with the same body). A server error or no
//! answer leaves it on the list. The agent progress reports and reproduction requests of the
//! report contract do not pass through this supervisor today (no code path sends them), so none
//! can be pending; they join this list when they do.
//!
//! **What is not.** Network requests the model makes itself (Bash, tools) are not observable
//! here: reported as `not_observable`, never as "none". The GitHub push token path (GH1) is
//! 05-03's.
//!
//! The list lives on the role volume (`/state/pending-requests.json`), so a restarted service
//! still knows it. A file that cannot be read is reported as `unreadable` and stops a reconnect
//! (fail closed).

use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Pending {
    path: PathBuf,
    gate: Mutex<()>,
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending").field("path", &self.path).finish()
    }
}

/// One tracked request: id, kind, the time it was sent (supervisor clock), its target, and what
/// a re-query needs (`detail`).
pub fn entry(request_id: &str, kind: &str, target: &str, detail: Value) -> Value {
    json!({"request_id": request_id, "kind": kind, "sent_at": ieo_evidence::time::now(),
           "target": target, "detail": detail})
}

impl Pending {
    pub fn new(path: PathBuf) -> Pending {
        Pending {
            path,
            gate: Mutex::new(()),
        }
    }

    fn read(&self) -> Result<Vec<Value>, String> {
        match std::fs::read(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("pending requests: {e}")),
            Ok(b) => match serde_json::from_slice::<Value>(&b) {
                Ok(Value::Array(a)) => Ok(a),
                _ => Err("pending requests: not a JSON list".into()),
            },
        }
    }

    fn write(&self, list: &[Value]) -> Result<(), String> {
        crate::central::durable_write(
            &self.path,
            &serde_json::to_vec_pretty(list).unwrap_or_default(),
        )
        .map_err(|e| format!("pending requests: {e}"))
    }

    /// The list (`Err` when the file cannot be read: the caller fails closed).
    pub fn list(&self) -> Result<Vec<Value>, String> {
        let _g = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        self.read()
    }

    /// Add (or replace) an entry by its request id.
    pub fn add(&self, e: Value) -> Result<(), String> {
        let _g = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut l = self.read()?;
        l.retain(|x| x["request_id"] != e["request_id"]);
        l.push(e);
        self.write(&l)
    }

    /// Remove the entries for which `gone` is true.
    pub fn remove_where(&self, gone: impl Fn(&Value) -> bool) -> Result<(), String> {
        let _g = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut l = self.read()?;
        let n = l.len();
        l.retain(|x| !gone(x));
        if l.len() != n {
            self.write(&l)?;
        }
        Ok(())
    }

    pub fn remove(&self, request_id: &str) -> Result<(), String> {
        self.remove_where(|x| x["request_id"] == request_id)
    }

    /// The list as the status and checkpoints report it: the entries without their re-query
    /// details, or `"unreadable"`.
    pub fn report(&self) -> Value {
        match self.list() {
            Ok(l) => json!(
                l.iter()
                    .map(|e| json!({"request_id": e["request_id"], "kind": e["kind"],
                                    "sent_at": e["sent_at"], "target": e["target"]}))
                    .collect::<Vec<_>>()
            ),
            Err(_) => json!("unreadable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_kept_on_the_volume_and_removed_by_id() {
        let d = std::env::temp_dir().join(format!("ieo-pending-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        let p = Pending::new(d.join("pending.json"));
        assert_eq!(p.report(), json!([]));
        p.add(entry(
            "a",
            "checkpoint_report",
            "coordinator",
            json!({"seq": 1}),
        ))
        .expect("add");
        p.add(entry("b", "first_action_report", "coordinator", json!({})))
            .expect("add");
        p.add(entry(
            "a",
            "checkpoint_report",
            "coordinator",
            json!({"seq": 1}),
        ))
        .expect("same id replaces");
        // a restarted service reads the same list
        let again = Pending::new(d.join("pending.json"));
        assert_eq!(again.list().expect("list").len(), 2);
        assert!(
            again.report()[0]["detail"].is_null(),
            "details are not reported"
        );
        again.remove("a").expect("remove");
        assert_eq!(again.report()[0]["request_id"], "b");
        std::fs::write(d.join("pending.json"), "{").expect("break");
        assert_eq!(again.report(), json!("unreadable"));
        assert!(again.add(entry("c", "x", "y", json!({}))).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
