//! Central checkpoint record (runtime contract §2.1, §5-1, §5-2, §5-6; 02-04 decision item 9).
//!
//! The coordinator's record is the canonical one. The supervisor reports every checkpoint it
//! writes (sequence number, exact bytes; the coordinator computes the digest and links the
//! repo, role, assignment, generation, sprint revision, spec digest and target SHA), and
//! compares its local checkpoint with the coordinator's latest record before anything starts:
//!
//! * same sequence, same digest → consistent;
//! * the local one is ahead → the missing reports are sent from the local archive, in order,
//!   and the comparison is repeated (a refusal stops: external reconciliation);
//! * the central one is ahead, the digests differ, the local checkpoint is missing while the
//!   central record has one, or the archive lacks a reported sequence → external
//!   reconciliation (`EXTERNAL_RECONCILIATION_REQUIRED`), nothing runs;
//! * the coordinator cannot be reached → `NOT_OBSERVED`, nothing runs.
//!
//! `local_only` (explicit in the role configuration) turns this off for tests; the status then
//! says the comparison is not made.
//!
//! Transport (X-04): `checkpoint_central.transport` is `"http"` (default; the 02-04 client) or
//! `"grpc"` (`ieo.internal.v0.SupervisorService`, contracts/ieo-internal-v0.md). Both clients sit
//! behind [`CoordinatorClient`] and return the answer as HTTP status and body, so the meaning
//! of an answer (stored, refused with 4xx, unknown) is decided here once for both.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Every coordinator call has this deadline (contracts/ieo-internal-v0.md §6.1).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// How the supervisor reaches the coordinator (`checkpoint_central.transport`, default HTTP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Http,
    Grpc,
}

/// One coordinator call's result as the HTTP path would see it: the HTTP status and body
/// (over gRPC: from the outcome, or from the `ieo-error-bin` trailer), or no answer at all.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Answered(u16, Value),
    /// Transport failure, deadline, or a gRPC status without a trailer: the outcome is unknown.
    NoAnswer(String),
}

/// The coordinator calls of the supervisor (X-04 boundary; contracts/ieo-internal-v0.md §1.1,
/// §5). Implementations move bytes only: what an answer means is decided once, in
/// [`Central::latest`] and [`Central::report`]. Neither implementation resends anything.
pub trait CoordinatorClient: std::fmt::Debug + Send + Sync {
    /// `GET …/checkpoints/latest` / `GetLatestCheckpoint`.
    fn latest_checkpoint(&self, repo: &str, role: &str) -> Reply;
    /// `POST …/checkpoints` / `ReportCheckpoint` with the exact checkpoint bytes.
    fn report_checkpoint(&self, repo: &str, role: &str, seq: u64, raw: &[u8]) -> Reply;
    /// `GET …/assignment` / `GetOwnAssignment` (02-05 step 3): the current assignment of this
    /// (repo, role) as the coordinator's database holds it.
    fn own_assignment(&self, repo: &str, role: &str) -> Reply;
    /// `POST …/first-actions` / `RecordFirstAction` (02-05 step 4) with the HTTP body
    /// `{assignment_id, generation, attempt_id, session_id, event_offset, observed_at, kind}`.
    /// A client that does not implement it has no answer (the run goes on; the `after_run`
    /// checkpoint carries the same values).
    fn record_first_action(&self, _repo: &str, _role: &str, _body: &Value) -> Reply {
        Reply::NoAnswer("first-action reports are not supported by this client".into())
    }
    /// Transport, URL and credential file (configuration identity).
    fn describe(&self) -> String;
}

#[derive(Debug, Clone)]
pub enum Central {
    /// No central record (tests only; reported as a capability gap).
    LocalOnly,
    /// The coordinator, over the configured transport.
    Coordinator(Arc<dyn CoordinatorClient>),
}

impl PartialEq for Central {
    fn eq(&self, other: &Central) -> bool {
        match (self, other) {
            (Central::LocalOnly, Central::LocalOnly) => true,
            (Central::Coordinator(a), Central::Coordinator(b)) => a.describe() == b.describe(),
            _ => false,
        }
    }
}

impl Eq for Central {}

/// Why the comparison stops a start: the HTTP status and body the adapter answers with.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub status: u16,
    pub body: Value,
}

fn reconcile(reason: impl Into<String>) -> Refusal {
    Refusal {
        status: 409,
        body: json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED", "reason": reason.into(),
                     "external_reconciliation_required": true}),
    }
}

fn not_observed(reason: impl Into<String>) -> Refusal {
    Refusal {
        status: 503,
        body: json!({"code": "NOT_OBSERVED", "reason": reason.into()}),
    }
}

fn read_token(token_file: &Path) -> Result<String, String> {
    std::fs::read_to_string(token_file)
        .map(|t| t.trim().to_string())
        .map_err(|e| format!("report credential: {}", e.kind()))
}

/// The 02-04 HTTP client (`ureq`, 10 s).
#[derive(Debug)]
pub struct HttpClient {
    pub url: String,
    pub token_file: PathBuf,
}

impl HttpClient {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Reply {
        let token = match read_token(&self.token_file) {
            Ok(t) => t,
            Err(e) => return Reply::NoAnswer(e),
        };
        let req = ureq::request(method, &format!("{}{path}", self.url))
            .timeout(CALL_TIMEOUT)
            .set("Authorization", &format!("Bearer {token}"));
        let r = match body {
            Some(b) => req.send_json(b.clone()),
            None => req.call(),
        };
        let r = match r {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Reply::NoAnswer(format!("transport: {}", e.kind())),
        };
        let st = r.status();
        let v = r.into_json().unwrap_or(Value::Null);
        Reply::Answered(st, v)
    }

    fn path(repo: &str, role: &str) -> String {
        format!("/v0/repos/{repo}/roles/{role}/checkpoints")
    }
}

impl CoordinatorClient for HttpClient {
    fn latest_checkpoint(&self, repo: &str, role: &str) -> Reply {
        self.call("GET", &format!("{}/latest", Self::path(repo, role)), None)
    }

    fn report_checkpoint(&self, repo: &str, role: &str, seq: u64, raw: &[u8]) -> Reply {
        // a JSON string carries UTF-8 only: bytes that are not are sent as their lossy
        // conversion (the supervisor writes checkpoints with serde_json, so they are UTF-8)
        let body = json!({"seq": seq, "checkpoint_json": String::from_utf8_lossy(raw)});
        self.call("POST", &Self::path(repo, role), Some(&body))
    }

    fn own_assignment(&self, repo: &str, role: &str) -> Reply {
        self.call(
            "GET",
            &format!("/v0/repos/{repo}/roles/{role}/assignment"),
            None,
        )
    }

    fn record_first_action(&self, repo: &str, role: &str, body: &Value) -> Reply {
        self.call(
            "POST",
            &format!("/v0/repos/{repo}/roles/{role}/first-actions"),
            Some(body),
        )
    }

    fn describe(&self) -> String {
        format!("http {} {}", self.url, self.token_file.display())
    }
}

/// The gRPC client (X-04): `ieo.internal.v0.SupervisorService` with a 10 s deadline, no
/// automatic resend. An answer is read the way HTTP reads it (§5): a trailer's `http_status`
/// and body, the outcome of an OK answer; a status without a trailer is no answer (unknown).
#[derive(Debug)]
pub struct GrpcClient {
    pub url: String,
    pub token_file: PathBuf,
    rt: Mutex<Option<tokio::runtime::Runtime>>,
}

impl GrpcClient {
    pub fn new(url: String, token_file: PathBuf) -> GrpcClient {
        GrpcClient {
            url,
            token_file,
            rt: Mutex::new(None),
        }
    }

    fn request<T>(m: T, token: &str) -> Result<tonic::Request<T>, String> {
        let mut r = tonic::Request::new(m);
        r.set_timeout(CALL_TIMEOUT);
        r.metadata_mut().insert(
            "authorization",
            format!("Bearer {token}")
                .parse()
                .map_err(|_| "report credential: not a header value".to_string())?,
        );
        Ok(r)
    }

    /// Connect and make one call within [`CALL_TIMEOUT`].
    fn run<F, Fut>(&self, call: F) -> Reply
    where
        F: FnOnce(
            ieo_proto::v0::supervisor_service_client::SupervisorServiceClient<
                tonic::transport::Channel,
            >,
        ) -> Fut,
        Fut: std::future::Future<Output = Reply>,
    {
        let mut rt = self
            .rt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if rt.is_none() {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => *rt = Some(r),
                Err(e) => return Reply::NoAnswer(format!("grpc runtime: {e}")),
            }
        }
        let Some(rt) = rt.as_ref() else {
            return Reply::NoAnswer("grpc runtime".into());
        };
        let url = self.url.clone();
        rt.block_on(async move {
            let attempt = async move {
                let ch = tonic::transport::Endpoint::from_shared(url)
                    .map_err(|e| format!("grpc url: {e}"))?
                    .connect_timeout(CALL_TIMEOUT)
                    .connect()
                    .await
                    .map_err(|e| format!("transport: {e}"))?;
                let cl = ieo_proto::v0::supervisor_service_client::SupervisorServiceClient::new(ch)
                    .max_decoding_message_size(ieo_proto::MAX_MESSAGE);
                Ok::<Reply, String>(call(cl).await)
            };
            match tokio::time::timeout(CALL_TIMEOUT, attempt).await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => Reply::NoAnswer(e),
                Err(_) => Reply::NoAnswer("transport: deadline (10 s)".into()),
            }
        })
    }
}

/// A non-OK status as the HTTP path would see it (§4.3, §5).
fn reply_of(st: &tonic::Status) -> Reply {
    match ieo_proto::error_from_status(st) {
        Some(e) => Reply::Answered(
            u16::try_from(e.http_status).unwrap_or(500),
            ieo_proto::body_of(&e),
        ),
        None => Reply::NoAnswer(format!("grpc {:?}: {}", st.code(), st.message())),
    }
}

impl CoordinatorClient for GrpcClient {
    fn latest_checkpoint(&self, repo: &str, role: &str) -> Reply {
        let token = match read_token(&self.token_file) {
            Ok(t) => t,
            Err(e) => return Reply::NoAnswer(e),
        };
        let m = ieo_proto::v0::GetLatestCheckpointRequest {
            repo_id: repo.to_string(),
            role: ieo_proto::role_of(role) as i32,
        };
        let req = match Self::request(m, &token) {
            Ok(r) => r,
            Err(e) => return Reply::NoAnswer(e),
        };
        self.run(|mut cl| async move {
            match cl.get_latest_checkpoint(req).await {
                Ok(r) => Reply::Answered(200, ieo_proto::latest_to_json(r.get_ref())),
                Err(st) => reply_of(&st),
            }
        })
    }

    fn report_checkpoint(&self, repo: &str, role: &str, seq: u64, raw: &[u8]) -> Reply {
        // §7.1 pre-check: the request HTTP would send (compact JSON) must fit the 1 MiB body
        // limit, and the path value the handler's 256 bytes; otherwise it is refused here, as
        // HTTP refuses it (400), and nothing is sent
        if repo.len() > ieo_proto::MAX_REPO_ID {
            return Reply::Answered(
                400,
                json!({"rejections": [{"code": "SCHEMA_VIOLATION", "field": "repo_id",
                    "message": "repo_id over 256 bytes (client pre-check)",
                    "correction": "use the configured repo id", "retryable_after_edit": true}]}),
            );
        }
        if ieo_proto::checkpoint_request_body(seq, raw).len() > ieo_proto::MAX_BODY {
            return Reply::Answered(400, ieo_proto::body_too_large_json());
        }
        let token = match read_token(&self.token_file) {
            Ok(t) => t,
            Err(e) => return Reply::NoAnswer(e),
        };
        let m = ieo_proto::v0::ReportCheckpointRequest {
            repo_id: repo.to_string(),
            role: ieo_proto::role_of(role) as i32,
            seq,
            checkpoint_json: raw.to_vec(),
        };
        let req = match Self::request(m, &token) {
            Ok(r) => r,
            Err(e) => return Reply::NoAnswer(e),
        };
        self.run(|mut cl| async move {
            match cl.report_checkpoint(req).await {
                Ok(r) => Reply::Answered(
                    ieo_proto::status_of_outcome(r.get_ref().outcome),
                    ieo_proto::checkpoint_answer_to_json(r.get_ref()),
                ),
                Err(st) => reply_of(&st),
            }
        })
    }

    fn own_assignment(&self, repo: &str, role: &str) -> Reply {
        let token = match read_token(&self.token_file) {
            Ok(t) => t,
            Err(e) => return Reply::NoAnswer(e),
        };
        let m = ieo_proto::v0::GetOwnAssignmentRequest {
            repo_id: repo.to_string(),
            role: ieo_proto::role_of(role) as i32,
        };
        let req = match Self::request(m, &token) {
            Ok(r) => r,
            Err(e) => return Reply::NoAnswer(e),
        };
        self.run(|mut cl| async move {
            match cl.get_own_assignment(req).await {
                Ok(r) => Reply::Answered(200, ieo_proto::own_assignment_to_json(r.get_ref())),
                Err(st) => reply_of(&st),
            }
        })
    }

    fn record_first_action(&self, repo: &str, role: &str, body: &Value) -> Reply {
        let token = match read_token(&self.token_file) {
            Ok(t) => t,
            Err(e) => return Reply::NoAnswer(e),
        };
        let s = |k: &str| body[k].as_str().unwrap_or_default().to_string();
        let n = |k: &str| body[k].as_u64().unwrap_or_default();
        let m = ieo_proto::v0::RecordFirstActionRequest {
            repo_id: repo.to_string(),
            role: ieo_proto::role_of(role) as i32,
            assignment_id: s("assignment_id"),
            generation: n("generation"),
            attempt_id: s("attempt_id"),
            session_id: s("session_id"),
            event_offset: n("event_offset"),
            observed_at: s("observed_at"),
            kind: ieo_proto::first_action_kind_of(&s("kind")) as i32,
        };
        let req = match Self::request(m, &token) {
            Ok(r) => r,
            Err(e) => return Reply::NoAnswer(e),
        };
        self.run(|mut cl| async move {
            match cl.record_first_action(req).await {
                Ok(r) => Reply::Answered(
                    ieo_proto::status_of_outcome(r.get_ref().outcome),
                    ieo_proto::first_action_receipt_to_json(
                        &r.get_ref().receipt.clone().unwrap_or_default(),
                    ),
                ),
                Err(st) => reply_of(&st),
            }
        })
    }

    fn describe(&self) -> String {
        format!("grpc {} {}", self.url, self.token_file.display())
    }
}

impl Central {
    pub fn from_json(v: &Value) -> Result<Central, String> {
        match v {
            Value::String(s) if s == "local_only" => Ok(Central::LocalOnly),
            Value::Object(_) => {
                let url = v["url"]
                    .as_str()
                    .ok_or("config.checkpoint_central.url missing")?
                    .trim_end_matches('/')
                    .to_string();
                let token_file: PathBuf = v["token_file"]
                    .as_str()
                    .ok_or("config.checkpoint_central.token_file missing")?
                    .into();
                let transport = match v["transport"].as_str() {
                    None | Some("http") => Transport::Http,
                    Some("grpc") => Transport::Grpc,
                    Some(_) => {
                        return Err(
                            "config.checkpoint_central.transport must be \"http\" or \"grpc\""
                                .into(),
                        );
                    }
                };
                Ok(Central::Coordinator(match transport {
                    Transport::Http => Arc::new(HttpClient { url, token_file }),
                    Transport::Grpc => Arc::new(GrpcClient::new(url, token_file)),
                }))
            }
            _ => Err(
                "config.checkpoint_central must be {url, token_file[, transport]} or \"local_only\" \
                 (runtime contract §2.1: the coordinator's record is canonical)"
                    .into(),
            ),
        }
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Central::LocalOnly => "local_only",
            Central::Coordinator(_) => "coordinator",
        }
    }

    /// The coordinator's latest record (`None` when it has none).
    pub fn latest(&self, repo: &str, role: &str) -> Result<Option<Value>, String> {
        let Central::Coordinator(c) = self else {
            return Err("no coordinator configured".into());
        };
        match c.latest_checkpoint(repo, role) {
            Reply::NoAnswer(e) => Err(e),
            Reply::Answered(200, v) => Ok(Some(v)),
            Reply::Answered(404, v) if v["code"] == "NO_CHECKPOINT" => Ok(None),
            Reply::Answered(st, v) => Err(format!("latest answered {st}: {}", code_of(&v))),
        }
    }

    /// Report one checkpoint (its exact bytes). `Ok` for a stored or duplicate report. A
    /// refusal carries the coordinator's status (`coordinator_status`): only a 4xx is a refusal
    /// the coordinator decided; a 5xx or no answer leaves the outcome unknown (02-04 rule;
    /// contracts/ieo-internal-v0.md §5).
    pub fn report(&self, repo: &str, role: &str, seq: u64, raw: &[u8]) -> Result<Value, Refusal> {
        let reply = match self {
            Central::Coordinator(c) => c.report_checkpoint(repo, role, seq, raw),
            Central::LocalOnly => Reply::NoAnswer("no coordinator configured".into()),
        };
        match reply {
            Reply::NoAnswer(e) => Err(not_observed(format!("checkpoint report {seq}: {e}"))),
            Reply::Answered(200 | 201, v) => Ok(v),
            Reply::Answered(st, v) => Err(reconcile(format!(
                "the coordinator refused checkpoint {seq} ({st} {})",
                code_of(&v)
            ))
            .with("coordinator_status", json!(st))
            .with("coordinator_answer", v)),
        }
    }
}

impl Central {
    /// The coordinator's current assignment of this (repo, role) (02-05 step 3). No answer →
    /// `NOT_OBSERVED`; no current assignment or any other refusal → external reconciliation.
    /// `local_only` has none to give: `Ok(None)` (the comparison is a reported capability gap).
    pub fn own_assignment(&self, repo: &str, role: &str) -> Result<Option<Value>, Refusal> {
        let Central::Coordinator(c) = self else {
            return Ok(None);
        };
        match c.own_assignment(repo, role) {
            Reply::NoAnswer(e) => Err(not_observed(format!("current assignment: {e}"))),
            Reply::Answered(200, v) => Ok(Some(v)),
            Reply::Answered(st, v) => Err(reconcile(format!(
                "the coordinator gave no current assignment ({st} {})",
                code_of(&v)
            ))
            .with("coordinator_status", json!(st))),
        }
    }
}

impl Central {
    /// Report the first action of an attempt (02-05 step 4): `{"ok": true, "status", "receipt"}`
    /// for a stored (201) or same (200) record; otherwise `{"ok": false, …}` with the status and
    /// answer, or the transport error. Never stops the run.
    pub fn record_first_action(&self, repo: &str, role: &str, body: &Value) -> Value {
        let Central::Coordinator(c) = self else {
            return json!({"ok": false, "reason": "local_only: no coordinator"});
        };
        match c.record_first_action(repo, role, body) {
            Reply::Answered(st @ (200 | 201), v) => json!({"ok": true, "status": st, "receipt": v}),
            Reply::Answered(st, v) => {
                json!({"ok": false, "status": st, "code": code_of(&v), "answer": v})
            }
            Reply::NoAnswer(e) => json!({"ok": false, "reason": e}),
        }
    }
}

/// The start or reconnect request's coordinator record against the coordinator's current
/// assignment (02-05 step 3, PLAN §3 step 3): assignment, generation, sprint revision and spec
/// digest must be the current ones, and the image and profile those the newest delivery
/// expects (when the coordinator recorded one). An older generation of the same assignment is
/// `STALE_GENERATION`; any other difference is external reconciliation. Nothing runs on either.
pub fn check_assignment(
    own: &Value,
    record: &crate::checkpoint::CoordinatorRecord<'_>,
) -> Result<(), Refusal> {
    if own["assignment_id"] == record.assignment_id
        && own["generation"]
            .as_u64()
            .is_some_and(|g| record.generation < g)
    {
        return Err(Refusal {
            status: 409,
            body: json!({"code": "STALE_GENERATION",
                "reason": format!("generation {} is older than the coordinator's current {}",
                                  record.generation, own["generation"])}),
        });
    }
    let mut want = vec![
        ("assignment_id", json!(record.assignment_id)),
        ("generation", json!(record.generation)),
        ("sprint_id", json!(record.sprint_id)),
        ("sprint_revision", json!(record.sprint_revision)),
        ("spec_digest", json!(record.spec_digest)),
    ];
    let exp = &own["expected_runtime"];
    if exp.is_object() {
        want.extend([
            ("image_digest", json!(record.image_digest)),
            ("profile_id", json!(record.profile_id)),
            ("profile_revision", json!(record.profile_revision)),
        ]);
    }
    for (k, mine) in want {
        let theirs = if own.get(k).is_some() {
            &own[k]
        } else {
            &exp[k]
        };
        if *theirs != mine {
            return Err(reconcile(format!(
                "coordinator record {k} {mine} is not the coordinator's current {theirs}"
            )));
        }
    }
    Ok(())
}

/// The error code of a coordinator answer: `code`, or the first rejection's (review 48b1d0e L-4).
fn code_of(v: &Value) -> String {
    v["code"]
        .as_str()
        .or_else(|| v["rejections"][0]["code"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

impl Refusal {
    fn with(mut self, k: &str, v: Value) -> Refusal {
        self.body[k] = v;
        self
    }
}

/// The archive of every checkpoint written: `<state>/checkpoints/<seq>.json`.
pub fn archive_path(dir: &Path, seq: u64) -> PathBuf {
    dir.join(format!("{seq:020}.json"))
}

/// Highest archived sequence (0 when none).
pub fn archived_max(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .and_then(|n| n.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0)
}

/// Write `bytes` to `path` durably: temp file, fsync, rename, directory fsync.
pub fn durable_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(dir)?.sync_all()
}

pub fn sha256(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Compare the local checkpoint (`checkpoint.json` bytes, if any) with the coordinator's
/// latest record, sending missing reports from `archive` first. Returns what was compared.
pub fn compare(
    central: &Central,
    repo: &str,
    role: &str,
    local: Option<&[u8]>,
    archive: &Path,
) -> Result<Value, Refusal> {
    if *central == Central::LocalOnly {
        return Ok(json!({"compared": false, "mode": "local_only"}));
    }
    let local = match local {
        None => None,
        Some(b) => {
            let v: Value = serde_json::from_slice(b)
                .map_err(|_| reconcile("the local checkpoint cannot be read"))?;
            let Some(seq) = v["seq"].as_u64() else {
                return Err(reconcile(
                    "the local checkpoint has no sequence number (written before central reporting)",
                ));
            };
            Some((seq, sha256(b)))
        }
    };
    let latest = central
        .latest(repo, role)
        .map_err(|e| not_observed(format!("central checkpoint record: {e}")))?;
    let central_at = latest.as_ref().map(|v| {
        (
            v["seq"].as_u64().unwrap_or(0),
            v["checkpoint_sha256"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )
    });
    let (cseq, csha) = central_at.clone().unwrap_or((0, String::new()));
    let Some((lseq, lsha)) = local else {
        if cseq > 0 {
            return Err(reconcile(format!(
                "the coordinator holds checkpoint {cseq} but there is no local checkpoint"
            )));
        }
        return Ok(json!({"compared": true, "central_seq": 0, "local_seq": null, "sent": 0}));
    };
    if cseq > lseq {
        return Err(reconcile(format!(
            "the local checkpoint {lseq} is older than the coordinator's {cseq} (rolled back or replaced)"
        )));
    }
    if cseq == lseq {
        if csha != lsha {
            return Err(reconcile(format!(
                "the local checkpoint {lseq} differs from the coordinator's record (digest)"
            )));
        }
        return Ok(json!({"compared": true, "central_seq": cseq, "local_seq": lseq, "sent": 0}));
    }
    // local is ahead: the coordinator's last record must be in the archive unchanged
    if cseq > 0 {
        let b = std::fs::read(archive_path(archive, cseq))
            .map_err(|_| reconcile(format!("archived checkpoint {cseq} (reported) is missing")))?;
        if sha256(&b) != csha {
            return Err(reconcile(format!(
                "archived checkpoint {cseq} differs from the coordinator's record"
            )));
        }
    }
    let mut sent = 0u64;
    for s in cseq + 1..=lseq {
        let b = std::fs::read(archive_path(archive, s))
            .map_err(|_| reconcile(format!("archived checkpoint {s} is missing")))?;
        if s == lseq && sha256(&b) != lsha {
            return Err(reconcile(format!(
                "archived checkpoint {s} differs from the local checkpoint"
            )));
        }
        central.report(repo, role, s, &b)?;
        sent += 1;
    }
    Ok(json!({"compared": true, "central_seq": cseq, "local_seq": lseq, "sent": sent}))
}
