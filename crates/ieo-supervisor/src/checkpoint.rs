//! Checkpoint record and the reconnect decision (runtime contract §5).

use serde_json::{Value, json};

/// Fields of §5-1. Kept as JSON so the coordinator can store it as is.
#[allow(clippy::too_many_arguments)]
pub fn record(
    session_id: &str,
    assignment_id: &str,
    generation: u64,
    target_sha: &str,
    sprint: (&str, u64, &str),
    settings_digest: &str,
    worktree: (&str, &str),
    last_event_offset: u64,
    session_cumulative: Value,
) -> Value {
    json!({"format": "ieo-checkpoint/v0", "session_id": session_id, "assignment_id": assignment_id,
           "generation": generation, "target_sha": target_sha,
           "sprint_id": sprint.0, "sprint_revision": sprint.1, "spec_digest": sprint.2,
           "settings_digest": settings_digest, "worktree_head": worktree.0, "diff_sha256": worktree.1,
           "last_event_offset": last_event_offset, "active_children": [], "pending_external_requests": [],
           "session_cumulative": session_cumulative})
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconnect {
    /// `--resume <session_id>` (the only resume key, §10 item 5).
    Resume { session_id: String },
    /// No checkpoint: clean and start the coordinator-issued new session.
    NewSession,
    /// Directive items changed since the checkpoint (§5-6): do not resume; clean, start a new
    /// session and report `config_tamper_detected`.
    TamperDetected {
        old_session_id: String,
        checkpoint_digest: String,
        observed_digest: String,
    },
    /// The checkpoint's generation is older than the coordinator's current one.
    StaleGeneration,
    /// The checkpoint names another spec digest than the coordinator record: external
    /// reconciliation before anything runs (§5-2).
    ExternalReconciliation,
    /// The same session is running; answer with status instead of starting `--resume` (§5-5).
    AlreadyRunning { pid: u32 },
}

/// Decide what `reconnect` does. `observed_digest` is the directive digest measured *before*
/// cleanup; `running_pid` comes from the session lock.
pub fn decide(
    checkpoint: Option<&Value>,
    current_generation: u64,
    coordinator_spec_digest: &str,
    observed_digest: &str,
    running_pid: Option<u32>,
) -> Reconnect {
    let Some(cp) = checkpoint else {
        return Reconnect::NewSession;
    };
    if let Some(pid) = running_pid {
        return Reconnect::AlreadyRunning { pid };
    }
    if cp["generation"]
        .as_u64()
        .is_none_or(|g| g < current_generation)
    {
        return Reconnect::StaleGeneration;
    }
    if cp["spec_digest"].as_str() != Some(coordinator_spec_digest) {
        return Reconnect::ExternalReconciliation;
    }
    let session_id = cp["session_id"].as_str().unwrap_or_default().to_string();
    let checkpoint_digest = cp["settings_digest"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if checkpoint_digest != observed_digest {
        return Reconnect::TamperDetected {
            old_session_id: session_id,
            checkpoint_digest,
            observed_digest: observed_digest.into(),
        };
    }
    Reconnect::Resume { session_id }
}
