//! Internal gRPC API v0 (contracts/ieo-internal-v0.md, contracts/proto). Shared by the
//! coordinator's adapter and the supervisor's client so both sides use one copy of:
//!
//! * the generated `ieo.internal.v0` messages, client and server (built from the .proto by
//!   protox at build time, no protoc);
//! * the HTTP → gRPC status table (§4.1) and the `ieo-error-bin` trailer (§4.3);
//! * the JSON ↔ message conversions of the operations: the coordinator answers in JSON
//!   (the HTTP body), the adapter carries the same values as typed fields, and a client that
//!   needs the HTTP shape back (the supervisor's records) gets the same JSON.
//!
//! No business rule lives here.

use serde_json::{Value, json};
use tonic::{Code, Status};

#[allow(clippy::all, clippy::pedantic, missing_docs)]
pub mod v0 {
    tonic::include_proto!("ieo.internal.v0");
}

use v0::{
    AssignmentLimits, ClaimReceipt, Disposition, FirstActionKind, FirstActionReceipt,
    GetLatestCheckpointResponse, GetOwnAssignmentResponse, Outcome, Rejection,
    ReportCheckpointResponse, ReproState, ReproStatus, Role, RuntimeExpectation,
    SessionStatusReceipt, SpecSource,
};

/// The fully qualified service name (health service names, §7.3).
pub const SERVICE_NAME: &str = "ieo.internal.v0.SupervisorService";
/// The binary trailer that carries an [`v0::Error`] (§4.3).
pub const ERROR_TRAILER: &str = "ieo-error-bin";
/// The HTTP-equivalent JSON body limit, the same as the HTTP front end's (§7.1).
pub const MAX_BODY: usize = 1 << 20;
/// The transport message limit: a safety bound above [`MAX_BODY`] (§7.1).
pub const MAX_MESSAGE: usize = MAX_BODY + 64 * 1024;
/// Longest `repo_id` / `request_id` a client sends (the handler and OpenAPI limits, §7.1).
pub const MAX_REPO_ID: usize = 256;
pub const MAX_REQUEST_ID: usize = 128;

/// The gRPC status for an HTTP answer (§4.1). `body` decides the 409 groups.
pub fn grpc_code(http_status: u16, body: &Value) -> Code {
    match http_status {
        200..=299 => Code::Ok,
        400 => Code::InvalidArgument,
        401 => Code::Unauthenticated,
        403 => Code::PermissionDenied,
        404 => Code::NotFound,
        409 => {
            let codes: Vec<&str> = body["rejections"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r["code"].as_str())
                .chain(body["code"].as_str())
                .collect();
            let all_in = |set: &[&str]| !codes.is_empty() && codes.iter().all(|c| set.contains(c));
            if all_in(&["EVENT_CONTENT_CONFLICT", "IDEMPOTENCY_KEY_CONFLICT"]) {
                Code::AlreadyExists
            } else if all_in(&["OWNERSHIP_CONFLICT", "OUTBOX_STATE_CONFLICT"]) {
                Code::Aborted
            } else {
                Code::FailedPrecondition
            }
        }
        422 => Code::FailedPrecondition,
        503 => Code::Unavailable,
        _ => Code::Internal,
    }
}

/// The [`v0::Error`] for an HTTP answer: the status, and the Problem `code`/`message` or the
/// rejection list, unchanged.
pub fn error_of(http_status: u16, body: &Value) -> v0::Error {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    v0::Error {
        http_status: u32::from(http_status),
        code: s(&body["code"]),
        message: s(&body["message"]),
        rejections: body["rejections"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| Rejection {
                code: s(&r["code"]),
                field: s(&r["field"]),
                message: s(&r["message"]),
                correction: s(&r["correction"]),
                retryable_after_edit: r["retryable_after_edit"].as_bool().unwrap_or_default(),
            })
            .collect(),
    }
}

/// The HTTP body an [`v0::Error`] stands for: `{"rejections": [...]}` or `{"code", "message"}`.
pub fn body_of(e: &v0::Error) -> Value {
    if e.rejections.is_empty() {
        json!({"code": e.code, "message": e.message})
    } else {
        json!({"rejections": e.rejections.iter().map(|r| json!({"code": r.code, "field": r.field,
            "message": r.message, "correction": r.correction,
            "retryable_after_edit": r.retryable_after_edit})).collect::<Vec<_>>()})
    }
}

/// A non-OK status carrying the HTTP answer in the `ieo-error-bin` trailer. `code` is normally
/// [`grpc_code`]; the DB-unavailable answer passes `Unavailable` (§7.2).
pub fn status_with_error(code: Code, http_status: u16, body: &Value) -> Status {
    let e = error_of(http_status, body);
    let first = e
        .rejections
        .first()
        .map(|r| format!("{} ({}): {}", r.code, r.field, r.message))
        .unwrap_or_else(|| format!("{}: {}", e.code, e.message));
    let mut md = tonic::metadata::MetadataMap::new();
    md.insert_bin(
        ERROR_TRAILER,
        tonic::metadata::MetadataValue::from_bytes(&prost::Message::encode_to_vec(&e)),
    );
    Status::with_metadata(code, format!("HTTP {http_status} {first}"), md)
}

/// The [`v0::Error`] in a status's trailer, if there is one that decodes.
pub fn error_from_status(s: &Status) -> Option<v0::Error> {
    let v = s.metadata().get_bin(ERROR_TRAILER)?;
    let b = v.to_bytes().ok()?;
    <v0::Error as prost::Message>::decode(b.as_ref()).ok()
}

/// 200 → replayed, 201/202 → stored.
pub fn outcome_of(http_status: u16) -> Outcome {
    if http_status == 200 {
        Outcome::Replayed
    } else {
        Outcome::Stored
    }
}

/// The HTTP status an outcome stands for in the supervisor's records (`Stored` → 201: the
/// checkpoint report's own status).
pub fn status_of_outcome(o: i32) -> u16 {
    if o == Outcome::Replayed as i32 {
        200
    } else {
        201
    }
}

/// `development` / `guardrail` / anything else (`""`: an unknown role, answered 404 by the
/// handler as on HTTP).
pub fn role_name(role: i32) -> &'static str {
    match Role::try_from(role) {
        Ok(Role::Development) => "development",
        Ok(Role::Guardrail) => "guardrail",
        _ => "",
    }
}

pub fn role_of(name: &str) -> Role {
    match name {
        "development" => Role::Development,
        "guardrail" => Role::Guardrail,
        _ => Role::Unspecified,
    }
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap_or_default()
}

/// `ClaimReceipt` JSON → message.
pub fn receipt_from_json(v: &Value) -> ClaimReceipt {
    ClaimReceipt {
        event_id: s(&v["event_id"]),
        event_revision: u(&v["event_revision"]),
        stored_at: s(&v["stored_at"]),
        evidence_level: s(&v["evidence_level"]),
        disposition: match v["disposition"].as_str() {
            Some("current") => Disposition::Current,
            Some("preserved_late") => Disposition::PreservedLate,
            _ => Disposition::Unspecified,
        } as i32,
        assignment_id: s(&v["assignment_id"]),
        generation: u(&v["generation"]),
        attributed_target_sha: s(&v["attributed_target_sha"]),
        counts_for_current_judgment: v["counts_for_current_judgment"]
            .as_bool()
            .unwrap_or_default(),
    }
}

pub fn receipt_to_json(r: &ClaimReceipt) -> Value {
    json!({"event_id": r.event_id, "event_revision": r.event_revision, "stored_at": r.stored_at,
           "evidence_level": r.evidence_level,
           "disposition": match Disposition::try_from(r.disposition) {
               Ok(Disposition::Current) => json!("current"),
               Ok(Disposition::PreservedLate) => json!("preserved_late"),
               _ => Value::Null,
           },
           "assignment_id": r.assignment_id, "generation": r.generation,
           "attributed_target_sha": r.attributed_target_sha,
           "counts_for_current_judgment": r.counts_for_current_judgment})
}

const REPRO_STATES: [(&str, ReproState); 5] = [
    ("queued", ReproState::Queued),
    ("running", ReproState::Running),
    ("finished", ReproState::Finished),
    ("runner_failed", ReproState::RunnerFailed),
    (
        "rejected_stale_generation",
        ReproState::RejectedStaleGeneration,
    ),
];

/// `ReproStatus` JSON → message.
pub fn repro_from_json(v: &Value) -> ReproStatus {
    ReproStatus {
        request_id: s(&v["request_id"]),
        state: REPRO_STATES
            .iter()
            .find(|(n, _)| v["state"] == *n)
            .map(|(_, st)| *st)
            .unwrap_or(ReproState::Unspecified) as i32,
        target_sha: s(&v["target_sha"]),
        runner_image_digest: v["runner_image_digest"].as_str().map(str::to_string),
        exit_code: v["exit_code"].as_i64().and_then(|c| i32::try_from(c).ok()),
        raw_log_refs: v["raw_log_refs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        finished_at: v["finished_at"].as_str().map(str::to_string),
    }
}

pub fn repro_to_json(r: &ReproStatus) -> Value {
    json!({"request_id": r.request_id,
           "state": REPRO_STATES.iter().find(|(_, st)| *st as i32 == r.state).map(|(n, _)| *n),
           "target_sha": r.target_sha, "runner_image_digest": r.runner_image_digest,
           "exit_code": r.exit_code, "raw_log_refs": r.raw_log_refs, "finished_at": r.finished_at})
}

/// The checkpoint report's JSON answer → message (200 `{duplicate, seq, checkpoint_sha256,
/// tamper_recorded}`,
/// 201 `{stored, seq, checkpoint_sha256, tamper_recorded, current_assignment}`).
pub fn checkpoint_answer_from_json(http_status: u16, v: &Value) -> ReportCheckpointResponse {
    ReportCheckpointResponse {
        outcome: outcome_of(http_status) as i32,
        seq: u(&v["seq"]),
        checkpoint_sha256: s(&v["checkpoint_sha256"]),
        tamper_recorded: v["tamper_recorded"].as_bool().unwrap_or_default(),
        current_assignment: v["current_assignment"].as_bool().unwrap_or_default(),
    }
}

/// The same JSON the HTTP path answers, from the message.
pub fn checkpoint_answer_to_json(r: &ReportCheckpointResponse) -> Value {
    if r.outcome == Outcome::Replayed as i32 {
        json!({"duplicate": true, "seq": r.seq, "checkpoint_sha256": r.checkpoint_sha256,
               "tamper_recorded": r.tamper_recorded})
    } else {
        json!({"stored": true, "seq": r.seq, "checkpoint_sha256": r.checkpoint_sha256,
               "tamper_recorded": r.tamper_recorded, "current_assignment": r.current_assignment})
    }
}

/// The latest central record JSON → message.
pub fn latest_from_json(v: &Value) -> GetLatestCheckpointResponse {
    GetLatestCheckpointResponse {
        repo_id: s(&v["repo_id"]),
        role: role_of(v["role"].as_str().unwrap_or_default()) as i32,
        seq: u(&v["seq"]),
        checkpoint_sha256: s(&v["checkpoint_sha256"]),
        phase: s(&v["phase"]),
        session_id: s(&v["session_id"]),
        assignment_id: s(&v["assignment_id"]),
        generation: u(&v["generation"]),
        sprint_id: s(&v["sprint_id"]),
        sprint_revision: u(&v["sprint_revision"]),
        spec_digest: s(&v["spec_digest"]),
        target_sha: s(&v["target_sha"]),
        settings_digest: s(&v["settings_digest"]),
        reported_by: s(&v["reported_by"]),
        received_at: s(&v["received_at"]),
        tamper_events: u(&v["tamper_events"]),
        current_assignment: v["current_assignment"].as_bool().unwrap_or_default(),
        tamper_recorded: v["tamper_recorded"].as_bool().unwrap_or_default(),
    }
}

pub fn latest_to_json(r: &GetLatestCheckpointResponse) -> Value {
    json!({"repo_id": r.repo_id, "role": role_name(r.role), "seq": r.seq,
           "checkpoint_sha256": r.checkpoint_sha256, "phase": r.phase, "session_id": r.session_id,
           "assignment_id": r.assignment_id, "generation": r.generation, "sprint_id": r.sprint_id,
           "sprint_revision": r.sprint_revision, "spec_digest": r.spec_digest,
           "target_sha": r.target_sha, "settings_digest": r.settings_digest,
           "reported_by": r.reported_by, "received_at": r.received_at,
           "tamper_events": r.tamper_events, "current_assignment": r.current_assignment,
           "tamper_recorded": r.tamper_recorded})
}

/// The HTTP-equivalent body of `SubmitReproRequest` (§2): every non-optional field always, an
/// unset optional field absent.
pub fn repro_request_json(r: &v0::SubmitReproRequestRequest) -> Value {
    let mut v = json!({"request_id": r.request_id, "repo_id": r.repo_id,
        "assignment_id": r.assignment_id, "generation": r.generation, "target_sha": r.target_sha,
        "bundle_digest": r.bundle_digest, "command": r.command, "expected": r.expected});
    if let Some(t) = r.timeout_seconds {
        v["timeout_seconds"] = json!(t);
    }
    v
}

/// The HTTP-equivalent body of `ReportCheckpoint`, compact serde_json (§7.1, review I-8): the
/// bytes the size limit measures on both sides. Bytes that are not UTF-8 are sized as their
/// lossy conversion (what the 02-04 HTTP client sends); the adapter refuses them after
/// authentication.
pub fn checkpoint_request_body(seq: u64, checkpoint: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({"seq": seq, "checkpoint_json": String::from_utf8_lossy(checkpoint)}))
        .unwrap_or_default()
}

/// The HTTP front end's answer to a body over [`MAX_BODY`] (ieo-coord `http::body_too_large`):
/// the client pre-check records the same refusal (§7.1).
pub fn body_too_large_json() -> Value {
    json!({"rejections": [{"code": "SCHEMA_VIOLATION", "field": "",
        "message": "body too large or unreadable", "correction": "send at most 1 MiB",
        "retryable_after_edit": true}]})
}

// ---------------------------------------------------------------------------
// 02-05 step 2: the supervisor's own assignment, first action and session status
// ---------------------------------------------------------------------------

fn opt_s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn opt_u(v: &Value) -> Option<u64> {
    v.as_u64()
}

/// getOwnAssignment JSON → message.
pub fn own_assignment_from_json(v: &Value) -> GetOwnAssignmentResponse {
    let l = &v["limits"];
    GetOwnAssignmentResponse {
        repo_id: s(&v["repo_id"]),
        role: role_of(v["role"].as_str().unwrap_or_default()) as i32,
        assignment_id: s(&v["assignment_id"]),
        generation: u(&v["generation"]),
        task_id: s(&v["task_id"]),
        target_sha: s(&v["target_sha"]),
        sprint_id: s(&v["sprint_id"]),
        sprint_revision: u(&v["sprint_revision"]),
        spec_digest: s(&v["spec_digest"]),
        sources: v["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| SpecSource {
                kind: s(&x["kind"]),
                r#ref: s(&x["ref"]),
                revision: s(&x["revision"]),
                digest: s(&x["digest"]),
                observed_at: opt_s(&x["observed_at"]),
            })
            .collect(),
        limits: Some(AssignmentLimits {
            elapsed_seconds: opt_u(&l["elapsed_seconds"]),
            total_tokens: opt_u(&l["total_tokens"]),
            correction_attempts: opt_u(&l["correction_attempts"]),
        }),
        attempts_issued: u(&v["attempts_issued"]),
        policy_digest: s(&v["policy_digest"]),
        issued_at: s(&v["issued_at"]),
        last_delivery_id: opt_s(&v["last_delivery_id"]),
        expected_runtime: v["expected_runtime"]
            .is_object()
            .then(|| RuntimeExpectation {
                image_digest: s(&v["expected_runtime"]["image_digest"]),
                profile_id: s(&v["expected_runtime"]["profile_id"]),
                profile_revision: u(&v["expected_runtime"]["profile_revision"]),
            }),
    }
}

pub fn own_assignment_to_json(r: &GetOwnAssignmentResponse) -> Value {
    let l = r.limits.unwrap_or_default();
    json!({"repo_id": r.repo_id, "role": role_name(r.role), "assignment_id": r.assignment_id,
           "generation": r.generation, "task_id": r.task_id, "target_sha": r.target_sha,
           "sprint_id": r.sprint_id, "sprint_revision": r.sprint_revision,
           "spec_digest": r.spec_digest,
           "sources": r.sources.iter().map(|x| json!({"kind": x.kind, "ref": x.r#ref,
               "revision": x.revision, "digest": x.digest, "observed_at": x.observed_at}))
               .collect::<Vec<_>>(),
           "limits": {"elapsed_seconds": l.elapsed_seconds, "total_tokens": l.total_tokens,
                      "correction_attempts": l.correction_attempts},
           "attempts_issued": r.attempts_issued, "policy_digest": r.policy_digest,
           "issued_at": r.issued_at, "last_delivery_id": r.last_delivery_id,
           "expected_runtime": r.expected_runtime.as_ref().map(|e| json!({
               "image_digest": e.image_digest, "profile_id": e.profile_id,
               "profile_revision": e.profile_revision}))})
}

/// `tool_use` / `text` / `""` (unspecified or unknown: refused by the handler as on HTTP).
pub fn first_action_kind_name(k: i32) -> &'static str {
    match FirstActionKind::try_from(k) {
        Ok(FirstActionKind::ToolUse) => "tool_use",
        Ok(FirstActionKind::Text) => "text",
        _ => "",
    }
}

pub fn first_action_kind_of(name: &str) -> FirstActionKind {
    match name {
        "tool_use" => FirstActionKind::ToolUse,
        "text" => FirstActionKind::Text,
        _ => FirstActionKind::Unspecified,
    }
}

/// The HTTP-equivalent body of `RecordFirstAction` (§2): every field, always.
pub fn first_action_request_json(r: &v0::RecordFirstActionRequest) -> Value {
    json!({"assignment_id": r.assignment_id, "generation": r.generation,
           "attempt_id": r.attempt_id, "session_id": r.session_id,
           "event_offset": r.event_offset, "observed_at": r.observed_at,
           "kind": first_action_kind_name(r.kind)})
}

pub fn first_action_receipt_from_json(v: &Value) -> FirstActionReceipt {
    FirstActionReceipt {
        assignment_id: s(&v["assignment_id"]),
        generation: u(&v["generation"]),
        attempt_id: s(&v["attempt_id"]),
        session_id: s(&v["session_id"]),
        evidence_level: s(&v["evidence_level"]),
        current_assignment: v["current_assignment"].as_bool().unwrap_or_default(),
        received_at: s(&v["received_at"]),
    }
}

pub fn first_action_receipt_to_json(r: &FirstActionReceipt) -> Value {
    json!({"assignment_id": r.assignment_id, "generation": r.generation,
           "attempt_id": r.attempt_id, "session_id": r.session_id,
           "evidence_level": r.evidence_level, "current_assignment": r.current_assignment,
           "received_at": r.received_at})
}

pub fn session_status_receipt_from_json(v: &Value) -> SessionStatusReceipt {
    SessionStatusReceipt {
        idempotency_key: s(&v["idempotency_key"]),
        observed_at: s(&v["observed_at"]),
        evidence_level: s(&v["evidence_level"]),
        stale_on_arrival: v["stale_on_arrival"].as_bool().unwrap_or_default(),
        current_assignment: v["current_assignment"].as_bool(),
        received_at: s(&v["received_at"]),
    }
}

pub fn session_status_receipt_to_json(r: &SessionStatusReceipt) -> Value {
    json!({"idempotency_key": r.idempotency_key, "observed_at": r.observed_at,
           "evidence_level": r.evidence_level, "stale_on_arrival": r.stale_on_arrival,
           "current_assignment": r.current_assignment, "received_at": r.received_at})
}

/// Parse a `grpc-timeout` metadata value (`<= 8 digits><H|M|S|m|u|n>`).
pub fn parse_grpc_timeout(v: &str) -> Option<std::time::Duration> {
    use std::time::Duration;
    let (num, unit) = v.split_at(v.len().checked_sub(1)?);
    if num.is_empty() || num.len() > 8 || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u64 = num.parse().ok()?;
    Some(match unit {
        "H" => Duration::from_secs(n.saturating_mul(3600)),
        "M" => Duration::from_secs(n.saturating_mul(60)),
        "S" => Duration::from_secs(n),
        "m" => Duration::from_millis(n),
        "u" => Duration::from_micros(n),
        "n" => Duration::from_nanos(n),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_table_follows_section_4_1() {
        let rj = |codes: &[&str]| json!({"rejections": codes.iter().map(|c| json!({"code": c})).collect::<Vec<_>>()});
        let p = |c: &str| json!({"code": c, "message": "m"});
        for (st, body, want) in [
            (200, json!({}), Code::Ok),
            (202, json!({}), Code::Ok),
            (400, rj(&["SCHEMA_VIOLATION"]), Code::InvalidArgument),
            (401, p("UNAUTHENTICATED"), Code::Unauthenticated),
            (403, p("SCOPE_DENIED"), Code::PermissionDenied),
            (404, p("NO_CHECKPOINT"), Code::NotFound),
            (409, rj(&["EVENT_CONTENT_CONFLICT"]), Code::AlreadyExists),
            (
                409,
                rj(&["EVENT_CONTENT_CONFLICT", "IDEMPOTENCY_KEY_CONFLICT"]),
                Code::AlreadyExists,
            ),
            (409, rj(&["OWNERSHIP_CONFLICT"]), Code::Aborted),
            (
                409,
                rj(&["EVENT_CONTENT_CONFLICT", "STALE_ASSIGNMENT"]),
                Code::FailedPrecondition,
            ),
            (
                409,
                rj(&["CHECKPOINT_SEQUENCE_GAP"]),
                Code::FailedPrecondition,
            ),
            (
                422,
                rj(&["REPORT_SEMANTIC_VIOLATION"]),
                Code::FailedPrecondition,
            ),
            (500, p("INTERNAL"), Code::Internal),
            (503, p("X"), Code::Unavailable),
        ] {
            assert_eq!(grpc_code(st, &body), want, "{st} {body}");
        }
    }

    #[test]
    fn the_trailer_carries_the_http_answer_unchanged() {
        let body = json!({"rejections": [{"code": "SCHEMA_VIOLATION", "field": "seq",
            "message": "m", "correction": "c", "retryable_after_edit": true}]});
        let st = status_with_error(grpc_code(400, &body), 400, &body);
        assert_eq!(st.code(), Code::InvalidArgument);
        let e = error_from_status(&st).expect("trailer");
        assert_eq!((e.http_status, body_of(&e)), (400, body));
        let p = json!({"code": "NO_CHECKPOINT", "message": "none"});
        let e = error_of(404, &p);
        assert_eq!(body_of(&e), p);
        assert!(error_from_status(&Status::unavailable("x")).is_none());
    }

    #[test]
    fn grpc_timeout_values_parse() {
        use std::time::Duration;
        assert_eq!(parse_grpc_timeout("10S"), Some(Duration::from_secs(10)));
        assert_eq!(parse_grpc_timeout("250m"), Some(Duration::from_millis(250)));
        assert_eq!(parse_grpc_timeout("1H"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_grpc_timeout("123456789S"), None);
        assert_eq!(parse_grpc_timeout("S"), None);
        assert_eq!(parse_grpc_timeout(""), None);
        assert_eq!(parse_grpc_timeout("5x"), None);
    }

    #[test]
    fn json_conversions_round_trip() {
        let latest = json!({"repo_id": "r", "role": "guardrail", "seq": 3,
            "checkpoint_sha256": "ab", "phase": "after_run", "session_id": "s",
            "assignment_id": "A", "generation": 1, "sprint_id": "P", "sprint_revision": 5,
            "spec_digest": "d", "target_sha": "t", "settings_digest": "e", "reported_by": "x",
            "received_at": "2026-11-15T00:00:00Z", "tamper_events": 0, "current_assignment": true,
            "tamper_recorded": false});
        assert_eq!(latest_to_json(&latest_from_json(&latest)), latest);
        let stored = json!({"stored": true, "seq": 2, "checkpoint_sha256": "ab",
                            "tamper_recorded": false, "current_assignment": true});
        assert_eq!(
            checkpoint_answer_to_json(&checkpoint_answer_from_json(201, &stored)),
            stored
        );
        let dup = json!({"duplicate": true, "seq": 2, "checkpoint_sha256": "ab",
                         "tamper_recorded": true});
        assert_eq!(
            checkpoint_answer_to_json(&checkpoint_answer_from_json(200, &dup)),
            dup
        );
        let repro = json!({"request_id": "r", "state": "queued", "target_sha": "t",
            "runner_image_digest": null, "exit_code": null, "raw_log_refs": [], "finished_at": null});
        assert_eq!(repro_to_json(&repro_from_json(&repro)), repro);
        let own = json!({"repo_id": "r", "role": "development", "assignment_id": "A",
            "generation": 2, "task_id": "01-01", "target_sha": "t", "sprint_id": "P",
            "sprint_revision": 5, "spec_digest": "d",
            "sources": [{"kind": "k", "ref": "x", "revision": "1", "digest": "e",
                         "observed_at": null},
                        {"kind": "k", "ref": "y", "revision": "2", "digest": "f",
                         "observed_at": "2026-11-15T00:00:00Z"}],
            "limits": {"elapsed_seconds": 3600, "total_tokens": null, "correction_attempts": 3},
            "attempts_issued": 1, "policy_digest": "p", "issued_at": "2026-11-15T00:00:00Z",
            "last_delivery_id": null, "expected_runtime": null});
        assert_eq!(own_assignment_to_json(&own_assignment_from_json(&own)), own);
        let mut own = own;
        own["last_delivery_id"] = json!("D-1");
        own["expected_runtime"] = json!({"image_digest": "sha256:i", "profile_id": "p",
                                         "profile_revision": 2});
        assert_eq!(own_assignment_to_json(&own_assignment_from_json(&own)), own);
        let fa = json!({"assignment_id": "A", "generation": 0, "attempt_id": "a1",
            "session_id": "s1", "evidence_level": "claim", "current_assignment": false,
            "received_at": "2026-11-15T00:00:00Z"});
        assert_eq!(
            first_action_receipt_to_json(&first_action_receipt_from_json(&fa)),
            fa
        );
        for cur in [json!(true), json!(false), Value::Null] {
            let ss = json!({"idempotency_key": "k-000001", "observed_at": "2026-11-15T00:00:00Z",
                "evidence_level": "claim", "stale_on_arrival": true, "current_assignment": cur,
                "received_at": "2026-11-15T00:00:00Z"});
            assert_eq!(
                session_status_receipt_to_json(&session_status_receipt_from_json(&ss)),
                ss
            );
        }
        for k in ["tool_use", "text"] {
            assert_eq!(first_action_kind_name(first_action_kind_of(k) as i32), k);
        }
        assert_eq!(first_action_kind_name(0), "");
        assert_eq!(first_action_kind_name(99), "");
    }
}
