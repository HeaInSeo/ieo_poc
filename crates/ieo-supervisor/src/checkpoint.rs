//! Checkpoint record and the reconnect decision (runtime contract §5, §6).

use serde_json::{Value, json};

/// The §5-1 fields. `active_children` and `pending_external_requests` come from the adapter's
/// stream reading (§6); the reconnect decision refuses to resume while either is non-empty.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkpoint<'a> {
    pub session_id: &'a str,
    pub generation: u64,
    pub assignment_id: &'a str,
    pub target_sha: &'a str,
    pub sprint_id: &'a str,
    pub sprint_revision: u64,
    pub spec_digest: &'a str,
    pub image_digest: &'a str,
    pub profile_id: &'a str,
    pub profile_revision: u64,
    pub settings_digest: &'a str,
    pub worktree_head: &'a str,
    pub diff_sha256: &'a str,
    pub last_event_offset: u64,
    pub active_children: Vec<Value>,
    pub pending_external_requests: Vec<Value>,
    /// Last run's session cumulative usage (`modelUsage` sum, `total_cost_usd`), §11 item 7.
    pub session_cumulative: Value,
}

/// The checkpoint as JSON, stored by the coordinator as is.
pub fn record(c: &Checkpoint<'_>) -> Value {
    json!({"format": "ieo-checkpoint/v0", "session_id": c.session_id, "generation": c.generation,
           "assignment_id": c.assignment_id, "target_sha": c.target_sha,
           "sprint_id": c.sprint_id, "sprint_revision": c.sprint_revision, "spec_digest": c.spec_digest,
           "image_digest": c.image_digest, "profile_id": c.profile_id,
           "profile_revision": c.profile_revision, "settings_digest": c.settings_digest,
           "worktree_head": c.worktree_head, "diff_sha256": c.diff_sha256,
           "last_event_offset": c.last_event_offset, "active_children": c.active_children,
           "pending_external_requests": c.pending_external_requests,
           "session_cumulative": c.session_cumulative})
}

/// What the coordinator holds for the current assignment (§5-2): every recorded digest and
/// identity of the checkpoint must match it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinatorRecord<'a> {
    pub generation: u64,
    pub assignment_id: &'a str,
    pub sprint_id: &'a str,
    pub sprint_revision: u64,
    pub spec_digest: &'a str,
    pub image_digest: &'a str,
    pub profile_id: &'a str,
    pub profile_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconnect {
    /// `--resume <session_id>` (the only resume key, §10 item 5).
    Resume { session_id: String },
    /// No checkpoint: clean and start the coordinator-issued new session.
    NewSession,
    /// Directive items changed (or could not be read) since the checkpoint (§5-6): do not
    /// resume; clean, start a new session and report `config_tamper_detected`.
    TamperDetected {
        old_session_id: String,
        checkpoint_digest: String,
        observed_digest: String,
    },
    /// The checkpoint's generation is older than the coordinator's current one (§5-2).
    StaleGeneration,
    /// The checkpoint does not match the coordinator record, is incomplete, or has open
    /// children/external requests: external reconciliation before anything runs (§5-2, §6).
    ExternalReconciliation { reason: String },
    /// The same session is running; answer with status instead of starting `--resume` (§5-5).
    AlreadyRunning { pid: Option<u32> },
}

fn reconcile(reason: impl Into<String>) -> Reconnect {
    Reconnect::ExternalReconciliation {
        reason: reason.into(),
    }
}

/// Decide what `reconnect` does. `observed_digest` is [`crate::configdir::observed`] measured
/// *before* cleanup; `running` comes from the session lock (`Some` when it is held).
pub fn decide(
    checkpoint: Option<&Value>,
    coordinator: &CoordinatorRecord<'_>,
    observed_digest: &str,
    running: Option<Option<u32>>,
) -> Reconnect {
    let Some(cp) = checkpoint else {
        return Reconnect::NewSession;
    };
    if let Some(pid) = running {
        return Reconnect::AlreadyRunning { pid };
    }
    let Some(generation) = cp["generation"].as_u64() else {
        return reconcile("checkpoint has no generation");
    };
    if generation < coordinator.generation {
        return Reconnect::StaleGeneration;
    }
    if generation > coordinator.generation {
        return reconcile(format!(
            "checkpoint generation {generation} is newer than the coordinator's {}",
            coordinator.generation
        ));
    }
    for (key, want) in [
        ("assignment_id", json!(coordinator.assignment_id)),
        ("sprint_id", json!(coordinator.sprint_id)),
        ("sprint_revision", json!(coordinator.sprint_revision)),
        ("spec_digest", json!(coordinator.spec_digest)),
        ("image_digest", json!(coordinator.image_digest)),
        ("profile_id", json!(coordinator.profile_id)),
        ("profile_revision", json!(coordinator.profile_revision)),
    ] {
        if cp[key] != want {
            return reconcile(format!(
                "{key} {} differs from the coordinator's {want}",
                cp[key]
            ));
        }
    }
    let session_id = match cp["session_id"].as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return reconcile("checkpoint has no session_id"),
    };
    // only a digest written by cleanup (64 hex) can be compared; never an "unreadable" value
    let Some(checkpoint_digest) = cp["settings_digest"]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return reconcile("checkpoint has no valid settings_digest");
    };
    if checkpoint_digest != observed_digest {
        return Reconnect::TamperDetected {
            old_session_id: session_id,
            checkpoint_digest: checkpoint_digest.into(),
            observed_digest: observed_digest.into(),
        };
    }
    for key in ["active_children", "pending_external_requests"] {
        match cp[key].as_array() {
            Some(a) if a.is_empty() => {}
            Some(a) => return reconcile(format!("{} open {key} (§6)", a.len())),
            None => return reconcile(format!("checkpoint has no {key}")),
        }
    }
    Reconnect::Resume { session_id }
}
