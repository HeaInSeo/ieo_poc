//! X-04 HTTP·gRPC common regression (contracts/ieo-internal-v0.md §10; ledger §7 X-04) against
//! a real PostgreSQL, the real HTTP server and the real gRPC adapter on 127.0.0.1.
//!
//! The same request is the same bytes: the gRPC message, and the HTTP body the adapter builds
//! from it (`ieo_proto::repro_request_json`, `ieo_proto::checkpoint_request_body`, the report
//! bytes unchanged). Answers are compared as (HTTP status, body): a gRPC answer is read back
//! from its `ieo-error-bin` trailer, or from the message for OK. Uses `IEO_TEST_DB_URL` like
//! coordinator.rs (skipped without it outside CI).

mod common;
use common::*;

use ieo_coord::api::{self, Abort, Operation, Req, TxGuard};
use ieo_coord::grpc::{Connect, GrpcConfig, GrpcServer};
use ieo_proto::v0::supervisor_service_client::SupervisorServiceClient;
use ieo_proto::v0::{
    GetLatestCheckpointRequest, GetOwnAssignmentRequest, GetReproRequestRequest, Outcome,
    RecordFirstActionRequest, ReportCheckpointRequest, ReportSessionStatusRequest,
    SubmitAgentReportRequest, SubmitReproRequestRequest,
};
use postgres::NoTls;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tonic::transport::Channel;
use tonic::{Code, Status};

// ---------------------------------------------------------------------------
// gRPC side of an environment
// ---------------------------------------------------------------------------

struct G {
    rt: tokio::runtime::Runtime,
    server: Option<GrpcServer>,
    url: String,
    /// The worker connector refuses while set (DB unavailable, §7.2).
    db_down: Arc<AtomicBool>,
    /// application_name of the adapter's connections (to terminate them).
    app_name: String,
}

fn grpc_with(e: &Env, cfg: GrpcConfig) -> G {
    let db_down = Arc::new(AtomicBool::new(false));
    let app_name = format!("ieo-grpc-{}", e.dbname);
    let mut c = e.cfg.clone();
    c.application_name(&app_name);
    let down = Arc::clone(&db_down);
    let connect: Connect = Arc::new(move || {
        if down.load(Ordering::SeqCst) {
            Err("test: database down".into())
        } else {
            c.connect(NoTls).map_err(|e| e.to_string())
        }
    });
    let server =
        GrpcServer::start("127.0.0.1:0", Arc::clone(&e.app), connect, cfg).expect("grpc server");
    let url = format!("http://{}", server.addr);
    G {
        rt: tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime"),
        server: Some(server),
        url,
        db_down,
        app_name,
    }
}

fn grpc(e: &Env) -> G {
    grpc_with(
        e,
        GrpcConfig {
            workers: 2,
            health_interval: Duration::from_millis(200),
            ..GrpcConfig::default()
        },
    )
}

impl Drop for G {
    fn drop(&mut self) {
        if let Some(s) = self.server.take() {
            s.stop();
        }
    }
}

/// One RPC, described once for both transports.
#[derive(Clone, Debug)]
enum Call {
    Report(Vec<u8>),
    Repro(SubmitReproRequestRequest),
    GetRepro(String),
    Checkpoint {
        repo: String,
        role: &'static str,
        seq: u64,
        raw: Vec<u8>,
    },
    Latest {
        repo: String,
        role: &'static str,
    },
    /// 02-05 step 2
    Own {
        repo: String,
        role: &'static str,
    },
    FirstAction(RecordFirstActionRequest),
    Status {
        repo: String,
        role: &'static str,
        key: String,
        raw: Vec<u8>,
    },
}

fn request<T>(e: &Env, who: Option<&str>, m: T, timeout: Option<Duration>) -> tonic::Request<T> {
    let mut r = tonic::Request::new(m);
    if let Some(w) = who {
        let t = e.tokens.get(w).cloned().unwrap_or_else(|| w.to_string());
        r.metadata_mut().insert(
            "authorization",
            format!("Bearer {t}").parse().expect("metadata"),
        );
    }
    if let Some(t) = timeout {
        r.set_timeout(t);
    }
    r
}

impl G {
    fn channel(&self, url: &str) -> Channel {
        self.rt
            .block_on(
                Channel::from_shared(url.to_string())
                    .expect("url")
                    .connect(),
            )
            .expect("connect")
    }

    fn client(&self) -> SupervisorServiceClient<Channel> {
        SupervisorServiceClient::new(self.channel(&self.url))
    }

    /// The call over gRPC: `Ok((HTTP status the outcome stands for, the HTTP-shaped body))`.
    async fn send(
        cl: &mut SupervisorServiceClient<Channel>,
        e: &Env,
        who: Option<&str>,
        c: &Call,
        timeout: Option<Duration>,
    ) -> Result<(u16, Value), Status> {
        let replayed = |o: i32| o == Outcome::Replayed as i32;
        Ok(match c {
            Call::Report(raw) => {
                let m = SubmitAgentReportRequest {
                    report_json: raw.clone(),
                };
                let a = cl
                    .submit_agent_report(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (
                    if replayed(a.outcome) { 200 } else { 202 },
                    ieo_proto::receipt_to_json(&a.receipt.unwrap_or_default()),
                )
            }
            Call::Repro(m) => {
                let a = cl
                    .submit_repro_request(request(e, who, m.clone(), timeout))
                    .await?
                    .into_inner();
                (
                    if replayed(a.outcome) { 200 } else { 202 },
                    ieo_proto::repro_to_json(&a.status.unwrap_or_default()),
                )
            }
            Call::GetRepro(id) => {
                let m = GetReproRequestRequest {
                    request_id: id.clone(),
                };
                let a = cl
                    .get_repro_request(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (200, ieo_proto::repro_to_json(&a.status.unwrap_or_default()))
            }
            Call::Checkpoint {
                repo,
                role,
                seq,
                raw,
            } => {
                let m = ReportCheckpointRequest {
                    repo_id: repo.clone(),
                    role: ieo_proto::role_of(role) as i32,
                    seq: *seq,
                    checkpoint_json: raw.clone(),
                };
                let a = cl
                    .report_checkpoint(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (
                    if replayed(a.outcome) { 200 } else { 201 },
                    ieo_proto::checkpoint_answer_to_json(&a),
                )
            }
            Call::Latest { repo, role } => {
                let m = GetLatestCheckpointRequest {
                    repo_id: repo.clone(),
                    role: ieo_proto::role_of(role) as i32,
                };
                let a = cl
                    .get_latest_checkpoint(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (200, ieo_proto::latest_to_json(&a))
            }
            Call::Own { repo, role } => {
                let m = GetOwnAssignmentRequest {
                    repo_id: repo.clone(),
                    role: ieo_proto::role_of(role) as i32,
                };
                let a = cl
                    .get_own_assignment(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (200, ieo_proto::own_assignment_to_json(&a))
            }
            Call::FirstAction(m) => {
                let a = cl
                    .record_first_action(request(e, who, m.clone(), timeout))
                    .await?
                    .into_inner();
                (
                    if replayed(a.outcome) { 200 } else { 201 },
                    ieo_proto::first_action_receipt_to_json(&a.receipt.unwrap_or_default()),
                )
            }
            Call::Status {
                repo,
                role,
                key,
                raw,
            } => {
                let m = ReportSessionStatusRequest {
                    repo_id: repo.clone(),
                    role: ieo_proto::role_of(role) as i32,
                    idempotency_key: key.clone(),
                    status_json: raw.clone(),
                };
                let a = cl
                    .report_session_status(request(e, who, m, timeout))
                    .await?
                    .into_inner();
                (
                    if replayed(a.outcome) { 200 } else { 201 },
                    ieo_proto::session_status_receipt_to_json(&a.receipt.unwrap_or_default()),
                )
            }
        })
    }

    fn raw(
        &self,
        e: &Env,
        who: Option<&str>,
        c: &Call,
        timeout: Option<Duration>,
    ) -> Result<(u16, Value), Status> {
        prepare(e, c);
        let mut cl = self.client();
        self.rt.block_on(Self::send(&mut cl, e, who, c, timeout))
    }

    /// The call over gRPC as (HTTP status, body); a trailer-less status is (0, its code).
    fn call(&self, e: &Env, who: Option<&str>, c: &Call) -> (u16, Value) {
        normalize(answer(self.raw(e, who, c, None)))
    }
}

/// (HTTP status, body) of a gRPC result. The gRPC code must be the §4.1 mapping of the trailer.
fn answer(r: Result<(u16, Value), Status>) -> (u16, Value) {
    match r {
        Ok(a) => a,
        Err(st) => match ieo_proto::error_from_status(&st) {
            Some(err) => {
                let body = ieo_proto::body_of(&err);
                let http = u16::try_from(err.http_status).expect("status");
                if st.code() != Code::Unavailable {
                    assert_eq!(
                        st.code(),
                        ieo_proto::grpc_code(http, &body),
                        "{http} {body}"
                    );
                }
                (http, body)
            }
            None => (0, json!({"grpc": format!("{:?}", st.code())})),
        },
    }
}

/// A takeover's fixture delivery (02-05 step 4, [`ensure_delivery`]), recorded before the call
/// (outside any async context).
fn prepare(e: &Env, c: &Call) {
    if let Call::Checkpoint { raw, .. } = c {
        ensure_delivery(e, raw);
    }
}

/// The same call over HTTP, with the bytes the adapter would build.
fn over_http(e: &Env, who: Option<&str>, c: &Call) -> (u16, Value) {
    prepare(e, c);
    let role_path = |r: &str| ieo_proto::role_name(ieo_proto::role_of(r) as i32);
    let mut key = None;
    let (m, p, b) = match c {
        Call::Report(raw) => ("POST", "/v0/agent/reports".to_string(), raw.clone()),
        Call::Repro(r) => (
            "POST",
            "/v0/agent/repro-requests".to_string(),
            serde_json::to_vec(&ieo_proto::repro_request_json(r)).expect("json"),
        ),
        Call::GetRepro(id) => ("GET", format!("/v0/agent/repro-requests/{id}"), vec![]),
        Call::Checkpoint {
            repo,
            role,
            seq,
            raw,
        } => (
            "POST",
            format!("/v0/repos/{repo}/roles/{}/checkpoints", role_path(role)),
            ieo_proto::checkpoint_request_body(*seq, raw),
        ),
        Call::Latest { repo, role } => (
            "GET",
            format!(
                "/v0/repos/{repo}/roles/{}/checkpoints/latest",
                role_path(role)
            ),
            vec![],
        ),
        Call::Own { repo, role } => (
            "GET",
            format!("/v0/repos/{repo}/roles/{}/assignment", role_path(role)),
            vec![],
        ),
        Call::FirstAction(r) => (
            "POST",
            format!(
                "/v0/repos/{}/roles/{}/first-actions",
                r.repo_id,
                ieo_proto::role_name(r.role)
            ),
            serde_json::to_vec(&ieo_proto::first_action_request_json(r)).expect("json"),
        ),
        Call::Status {
            repo,
            role,
            key: k,
            raw,
        } => {
            // an empty gRPC field is no header (contract §8.2)
            key = Some(k.as_str()).filter(|k| !k.is_empty());
            (
                "POST",
                format!("/v0/repos/{repo}/roles/{}/session-status", role_path(role)),
                raw.clone(),
            )
        }
    };
    normalize(e.call_raw_k(who, m, &p, key, &b))
}

/// Timestamps differ between runs; everything else must be equal.
fn normalize((st, mut b): (u16, Value)) -> (u16, Value) {
    if let Some(m) = b.as_object_mut() {
        m.remove("stored_at");
        m.remove("received_at");
        m.remove("issued_at");
    }
    (st, b)
}

/// Every table's row count and the contents of the tables the surface writes (timestamps
/// removed).
fn snapshot(db: &mut postgres::Client) -> Value {
    let mut out = serde_json::Map::new();
    for t in [
        "sprint_revisions",
        "semantic_reviews",
        "idempotency",
        "assignments",
        "assignment_heads",
        "task_attempts",
        "evidence",
        "judgments",
        "outbox",
    ] {
        let n: i64 = db
            .query_one(&format!("SELECT count(*) FROM {t}"), &[])
            .expect("count")
            .get(0);
        out.insert(t.into(), json!(n));
    }
    for (t, strip) in [
        ("events", "stored_at"),
        ("repro_requests", "created_at"),
        ("role_checkpoints", "received_at"),
        ("role_tamper_events", "received_at"),
        ("role_first_actions", "received_at"),
        ("role_session_status", "received_at"),
        ("role_takeovers", "received_at"),
        ("history", "at,seq"),
    ] {
        let v: Value = db
            .query_one(
                &format!(
                    "SELECT coalesce(jsonb_agg(x ORDER BY x::text), '[]'::jsonb) FROM
                     (SELECT to_jsonb(t) - '{{{strip}}}'::text[] AS x FROM {t} t) s"
                ),
                &[],
            )
            .expect("snapshot")
            .get(0);
        out.insert(t.into(), v);
    }
    Value::Object(out)
}

fn report_call(role: &str, eid: &str, aid: &str, generation: u64, task: &str, sha: &str) -> Call {
    Call::Report(serde_json::to_vec(&report(role, eid, aid, generation, task, sha)).expect("j"))
}

fn report_with(eid: &str, generation: u64, sha: &str, edit: impl Fn(&mut Value)) -> Call {
    let mut r = report("development", eid, "A1", generation, "01-01", sha);
    edit(&mut r);
    Call::Report(serde_json::to_vec(&r).expect("j"))
}

fn repro_msg(id: &str, generation: u64, sha: &str) -> SubmitReproRequestRequest {
    SubmitReproRequestRequest {
        request_id: id.into(),
        repo_id: "repo-a".into(),
        assignment_id: "G1".into(),
        generation,
        target_sha: sha.into(),
        bundle_digest: "a".repeat(64),
        command: vec!["cargo".into(), "test".into()],
        expected: "test fails on the bug".into(),
        timeout_seconds: None,
    }
}

fn cp_raw(seq: u64, generation: u64, digest: &str, extra: Value) -> Vec<u8> {
    checkpoint(seq, generation, digest, extra).into_bytes()
}

fn cp(seq: u64, raw: Vec<u8>) -> Call {
    Call::Checkpoint {
        repo: "repo-a".into(),
        role: "development",
        seq,
        raw,
    }
}

fn latest(repo: &str, role: &'static str) -> Call {
    Call::Latest {
        repo: repo.into(),
        role,
    }
}

// --- 02-05 step 2 calls

fn own(repo: &str, role: &'static str) -> Call {
    Call::Own {
        repo: repo.into(),
        role,
    }
}

fn fa_msg(aid: &str, generation: u64, attempt: &str, session: &str) -> RecordFirstActionRequest {
    RecordFirstActionRequest {
        repo_id: "repo-a".into(),
        role: ieo_proto::role_of("development") as i32,
        assignment_id: aid.into(),
        generation,
        attempt_id: attempt.into(),
        session_id: session.into(),
        event_offset: 3,
        observed_at: "2026-11-15T01:00:00.250Z".into(),
        kind: ieo_proto::first_action_kind_of("tool_use") as i32,
    }
}

fn fa(aid: &str, generation: u64, attempt: &str, session: &str) -> Call {
    Call::FirstAction(fa_msg(aid, generation, attempt, session))
}

/// A session status of repo-a's development supervisor (runtime contract §7 items).
fn status_doc(observed: &str, assignment: Option<(&str, u64)>) -> Value {
    json!({"observed_at": observed, "lifecycle": "idle", "session_id": "s1",
        "assignment_id": assignment.map(|a| a.0), "generation": assignment.map(|a| a.1),
        "quota_group_id": "test-shared", "provider_status_or_unknown": "unknown",
        "last_actual_action_at": null, "checkpoint_seq": 1, "checkpoint_sha256": "a".repeat(64),
        "pending_external_requests": "not_tracked", "active_children": "not_tracked"})
}

fn status_raw(key: &str, raw: Vec<u8>) -> Call {
    Call::Status {
        repo: "repo-a".into(),
        role: "development",
        key: key.into(),
        raw,
    }
}

fn status(key: &str, doc: &Value) -> Call {
    status_raw(key, serde_json::to_vec(doc).expect("json"))
}

fn status_with(key: &str, edit: impl Fn(&mut Value)) -> Call {
    let mut d = status_doc("2026-11-15T02:00:00Z", Some(("A1", 0)));
    edit(&mut d);
    status(key, &d)
}

enum Step {
    /// Run over HTTP in both environments (assignments).
    Setup(&'static str, Box<dyn Fn(&Env)>),
    Do(&'static str, Option<&'static str>, Call),
}

fn setup(label: &'static str, f: impl Fn(&Env) + 'static) -> Step {
    Step::Setup(label, Box::new(f))
}

fn assign_ok(e: &Env, role: &str, aid: &str, generation: u64, sha: &str, prev: Value) {
    let (st, b) = assign(e, role, aid, generation, "01-01", sha, prev);
    assert_eq!(st, 201, "{b}");
}

/// Every reachable row of contract §4.2, the role permissions, idempotency, stale generations,
/// rollback after a write, size limits and path values.
fn scenario(digest: &str) -> Vec<Step> {
    use Step::Do;
    let d = digest.to_string();
    let cpx = |seq: u64, generation: u64, extra: Value| cp(seq, cp_raw(seq, generation, &d, extra));
    let big = "x".repeat(1 << 20);
    let quotes = "\"".repeat(400_000);
    vec![
        // --- before any assignment
        Do(
            "latest: none",
            Some("sup-dev-a"),
            latest("repo-a", "development"),
        ),
        Do(
            "checkpoint: no assignment",
            Some("sup-dev-a"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        // 02-05 step 2 before any assignment
        Do(
            "own assignment: none",
            Some("sup-dev-a"),
            own("repo-a", "development"),
        ),
        Do(
            "first action: never issued",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "session status: no assignment named",
            Some("sup-dev-a"),
            status("st-000000", &status_doc("2026-11-15T00:00:00Z", None)),
        ),
        Do(
            "session status: never issued",
            Some("sup-dev-a"),
            status(
                "st-000001",
                &status_doc("2026-11-15T00:10:00Z", Some(("A1", 0))),
            ),
        ),
        setup("assign A1/G1 gen 0", |e| {
            assign_ok(e, "development", "A1", 0, SHA1, Value::Null);
            assign_ok(e, "guardrail", "G1", 0, SHA1, Value::Null);
        }),
        // --- SubmitAgentReport
        Do(
            "report: no credential",
            None,
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: unknown token",
            Some("not-a-token"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: client credential",
            Some("alice"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: supervisor credential",
            Some("sup-dev-a"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: other repo",
            Some("dev-b"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: other role",
            Some("gr-a"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: stored",
            Some("dev-a"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: replay",
            Some("dev-a"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: content conflict",
            Some("dev-a"),
            report_with("dev-1", 0, SHA1, |r| r["next_action"] = json!("different")),
        ),
        Do(
            "report: verified from agent",
            Some("dev-a"),
            report_with("dev-v", 0, SHA1, |r| {
                r["evidence_level"] = json!("verified")
            }),
        ),
        Do(
            "report: never issued",
            Some("dev-a"),
            report_call("development", "dev-x", "ZZ", 0, "01-01", SHA1),
        ),
        Do(
            "report: wrong task",
            Some("dev-a"),
            report_call("development", "dev-z", "A1", 0, "01-02", SHA1),
        ),
        Do(
            "report: bad phase",
            Some("dev-a"),
            report_with("dev-bad", 0, SHA1, |r| r["phase"] = json!("없는 단계")),
        ),
        Do(
            "report: missing field",
            Some("dev-a"),
            report_with("dev-m", 0, SHA1, |r| {
                if let Some(m) = r.as_object_mut() {
                    m.remove("next_action");
                }
            }),
        ),
        Do(
            "report: not JSON",
            Some("dev-a"),
            Call::Report(b"not json".to_vec()),
        ),
        Do(
            "report: not UTF-8",
            Some("dev-a"),
            Call::Report(vec![0xff, 0xfe, b'{', b'}']),
        ),
        Do(
            "report: over 1 MiB, no credential",
            None,
            report_with("dev-big", 0, SHA1, |r| {
                r["next_action"] = json!(big.clone())
            }),
        ),
        Do(
            "report: over 1 MiB",
            Some("dev-a"),
            report_with("dev-big", 0, SHA1, |r| {
                r["next_action"] = json!(big.clone())
            }),
        ),
        // --- SubmitReproRequest
        Do(
            "repro: no credential",
            None,
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: development credential",
            Some("dev-a"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: client credential",
            Some("alice"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: other repo",
            Some("gr-b"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: queued",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: replay",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: content conflict",
            Some("gr-a"),
            Call::Repro(SubmitReproRequestRequest {
                expected: "other".into(),
                ..repro_msg("repro-0001", 0, SHA1)
            }),
        ),
        Do(
            "repro: other target sha",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0002", 0, SHA2)),
        ),
        Do(
            "repro: empty command",
            Some("gr-a"),
            Call::Repro(SubmitReproRequestRequest {
                command: vec![],
                ..repro_msg("repro-0003", 0, SHA1)
            }),
        ),
        Do(
            "repro: short id",
            Some("gr-a"),
            Call::Repro(repro_msg("abc", 0, SHA1)),
        ),
        Do(
            "repro: empty id",
            Some("gr-a"),
            Call::Repro(repro_msg("", 0, SHA1)),
        ),
        Do(
            "repro: timeout 0",
            Some("gr-a"),
            Call::Repro(SubmitReproRequestRequest {
                timeout_seconds: Some(0),
                ..repro_msg("repro-0004", 0, SHA1)
            }),
        ),
        Do(
            "repro: timeout 60",
            Some("gr-a"),
            Call::Repro(SubmitReproRequestRequest {
                timeout_seconds: Some(60),
                ..repro_msg("repro-0005", 0, SHA1)
            }),
        ),
        Do(
            "repro: over 1 MiB",
            Some("gr-a"),
            Call::Repro(SubmitReproRequestRequest {
                expected: big.clone(),
                ..repro_msg("repro-0006", 0, SHA1)
            }),
        ),
        // --- GetReproRequest
        Do(
            "get repro: found",
            Some("gr-a"),
            Call::GetRepro("repro-0001".into()),
        ),
        Do(
            "get repro: other repo",
            Some("gr-b"),
            Call::GetRepro("repro-0001".into()),
        ),
        Do(
            "get repro: development",
            Some("dev-a"),
            Call::GetRepro("repro-0001".into()),
        ),
        Do(
            "get repro: client",
            Some("alice"),
            Call::GetRepro("repro-0001".into()),
        ),
        Do(
            "get repro: no credential",
            None,
            Call::GetRepro("repro-0001".into()),
        ),
        Do(
            "get repro: unknown",
            Some("gr-a"),
            Call::GetRepro("repro-9999".into()),
        ),
        // P-1: an empty id and a `/` are no operation on HTTP: 404 after authentication
        Do(
            "get repro: empty id",
            Some("gr-a"),
            Call::GetRepro(String::new()),
        ),
        Do(
            "get repro: empty id, development",
            Some("dev-a"),
            Call::GetRepro(String::new()),
        ),
        Do(
            "get repro: empty id, no credential",
            None,
            Call::GetRepro(String::new()),
        ),
        Do(
            "get repro: slash",
            Some("dev-a"),
            Call::GetRepro("repro/0001".into()),
        ),
        // --- ReportCheckpoint
        Do(
            "checkpoint: model credential",
            Some("dev-a"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: other role",
            Some("sup-gr-a"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: client",
            Some("operator"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: no credential",
            None,
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: other repo path",
            Some("sup-dev-a"),
            Call::Checkpoint {
                repo: "repo-b".into(),
                role: "development",
                seq: 1,
                raw: cp_raw(1, 0, digest, json!({"phase": "takeover"})),
            },
        ),
        Do(
            "checkpoint: empty repo",
            Some("sup-dev-a"),
            Call::Checkpoint {
                repo: String::new(),
                role: "development",
                seq: 1,
                raw: cp_raw(1, 0, digest, json!({"phase": "takeover"})),
            },
        ),
        Do(
            "checkpoint: slash in repo",
            Some("sup-dev-a"),
            Call::Checkpoint {
                repo: "repo-a/x".into(),
                role: "development",
                seq: 1,
                raw: cp_raw(1, 0, digest, json!({"phase": "takeover"})),
            },
        ),
        Do(
            "checkpoint: unknown role",
            Some("sup-dev-a"),
            Call::Checkpoint {
                repo: "repo-a".into(),
                role: "manager",
                seq: 1,
                raw: cp_raw(1, 0, digest, json!({"phase": "takeover"})),
            },
        ),
        Do("checkpoint: gap", Some("sup-dev-a"), cpx(2, 0, json!({}))),
        Do(
            "checkpoint: stored",
            Some("sup-dev-a"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: duplicate",
            Some("sup-dev-a"),
            cpx(1, 0, json!({"phase": "takeover"})),
        ),
        Do(
            "checkpoint: content conflict",
            Some("sup-dev-a"),
            cpx(1, 0, json!({"phase": "error"})),
        ),
        Do(
            "checkpoint: target sha",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"target_sha": SHA2})),
        ),
        Do(
            "checkpoint: spec digest",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"spec_digest": "0".repeat(64)})),
        ),
        Do(
            "checkpoint: sprint revision",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"sprint_revision": 4})),
        ),
        Do(
            "checkpoint: generation",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"generation": 5})),
        ),
        Do(
            "checkpoint: assignment",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"assignment_id": "A9"})),
        ),
        Do(
            "checkpoint: repo mismatch",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"repo_id": "repo-b"})),
        ),
        // §3 order (I-7): REPO_MISMATCH before the other field checks, after format and seq
        Do(
            "checkpoint: repo mismatch, no phase",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"repo_id": "repo-b", "phase": null})),
        ),
        Do(
            "checkpoint: repo mismatch, other format",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"repo_id": "repo-b", "format": "other"})),
        ),
        Do(
            "checkpoint: repo mismatch, other seq",
            Some("sup-dev-a"),
            cp(2, cp_raw(3, 0, digest, json!({"repo_id": "repo-b"}))),
        ),
        Do(
            "checkpoint: seq 0",
            Some("sup-dev-a"),
            cp(0, cp_raw(0, 0, digest, json!({}))),
        ),
        Do(
            "checkpoint: phase",
            Some("sup-dev-a"),
            cpx(2, 0, json!({"phase": "running"})),
        ),
        Do(
            "checkpoint: not JSON",
            Some("sup-dev-a"),
            cp(2, b"not json".to_vec()),
        ),
        Do("checkpoint: empty", Some("sup-dev-a"), cp(2, Vec::new())),
        Do(
            "checkpoint: escaped body over 1 MiB",
            None,
            cpx(2, 0, json!({"pad": quotes.clone()})),
        ),
        // a write, then a refusal: everything rolls back
        Do(
            "checkpoint: tamper with another session",
            Some("sup-dev-a"),
            cpx(
                2,
                0,
                json!({"phase": "takeover",
            "config_tamper_detected": {"old_session_id": "s1", "new_session_id": "other",
            "checkpoint_digest": "a".repeat(64), "observed_digest": "b".repeat(64)}, "session_id": "s2"}),
            ),
        ),
        Do(
            "checkpoint: tamper recorded",
            Some("sup-dev-a"),
            cpx(
                2,
                0,
                json!({"phase": "takeover",
            "config_tamper_detected": {"old_session_id": "s1", "new_session_id": "s2",
            "checkpoint_digest": "a".repeat(64), "observed_digest": "b".repeat(64)}, "session_id": "s2"}),
            ),
        ),
        // --- GetOwnAssignment (02-05 step 2)
        Do(
            "own assignment: supervisor",
            Some("sup-dev-a"),
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: guardrail supervisor",
            Some("sup-gr-a"),
            own("repo-a", "guardrail"),
        ),
        Do(
            "own assignment: other role's supervisor",
            Some("sup-gr-a"),
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: model credential",
            Some("dev-a"),
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: client with roles:control",
            Some("operator"),
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: assigner",
            Some("assigner"),
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: no credential",
            None,
            own("repo-a", "development"),
        ),
        Do(
            "own assignment: other repo",
            Some("sup-dev-a"),
            own("repo-b", "development"),
        ),
        Do(
            "own assignment: unknown role",
            Some("sup-dev-a"),
            own("repo-a", "manager"),
        ),
        Do(
            "own assignment: slash",
            Some("sup-dev-a"),
            own("repo-a/x", "development"),
        ),
        // --- RecordFirstAction (02-05 step 2): s1 and s2 have stored takeovers under A1/0
        Do(
            "first action: no credential",
            None,
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: model credential",
            Some("dev-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: other role's supervisor",
            Some("sup-gr-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: client",
            Some("operator"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: other repo",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                repo_id: "repo-b".into(),
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: unknown role",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                role: 0,
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: unspecified kind",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                kind: 0,
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: bad time",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                observed_at: "2026-02-30T00:00:00Z".into(),
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: local time",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                observed_at: "2026-11-15T10:00:00+09:00".into(),
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: empty attempt",
            Some("sup-dev-a"),
            fa("A1", 0, "", "s1"),
        ),
        Do(
            "first action: never issued assignment",
            Some("sup-dev-a"),
            fa("ZZ", 0, "att-1", "s1"),
        ),
        Do(
            "first action: guardrail's assignment",
            Some("sup-dev-a"),
            fa("G1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: session without takeover",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s9"),
        ),
        Do(
            "first action: stored",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: replay",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: content conflict",
            Some("sup-dev-a"),
            Call::FirstAction(RecordFirstActionRequest {
                event_offset: 9,
                ..fa_msg("A1", 0, "att-1", "s1")
            }),
        ),
        Do(
            "first action: content conflict, other session",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s2"),
        ),
        // --- ReportSessionStatus (02-05 step 2)
        Do(
            "session status: no credential",
            None,
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: model credential",
            Some("dev-a"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: other role's supervisor",
            Some("sup-gr-a"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: client",
            Some("operator"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: other repo",
            Some("sup-dev-a"),
            Call::Status {
                repo: "repo-b".into(),
                role: "development",
                key: "st-000002".into(),
                raw: serde_json::to_vec(&status_doc("2026-11-15T02:00:00Z", Some(("A1", 0))))
                    .expect("json"),
            },
        ),
        Do(
            "session status: no key",
            Some("sup-dev-a"),
            status_with("", |_| {}),
        ),
        Do(
            "session status: short key",
            Some("sup-dev-a"),
            status_with("short", |_| {}),
        ),
        Do(
            "session status: stored",
            Some("sup-dev-a"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: replay",
            Some("sup-dev-a"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: replay as other bytes",
            Some("sup-dev-a"),
            status_raw(
                "st-000002",
                serde_json::to_vec_pretty(&status_doc("2026-11-15T02:00:00Z", Some(("A1", 0))))
                    .expect("json"),
            ),
        ),
        Do(
            "session status: key conflict",
            Some("sup-dev-a"),
            status_with("st-000002", |d| d["lifecycle"] = json!("running")),
        ),
        Do(
            "session status: older than the stored one",
            Some("sup-dev-a"),
            status_with("st-000003", |d| {
                d["observed_at"] = json!("2026-11-15T01:59:59.999Z")
            }),
        ),
        Do(
            "session status: same time as the stored one",
            Some("sup-dev-a"),
            status_with("st-000004", |d| d["lifecycle"] = json!("stopping")),
        ),
        Do(
            "session status: never issued",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["assignment_id"] = json!("ZZ")),
        ),
        Do(
            "session status: bad lifecycle",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["lifecycle"] = json!("sleeping")),
        ),
        Do(
            "session status: zero for not tracked",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["active_children"] = json!(0)),
        ),
        Do(
            "session status: missing item",
            Some("sup-dev-a"),
            status_with("st-000005", |d| {
                d.as_object_mut().map(|m| m.remove("last_actual_action_at"));
            }),
        ),
        Do(
            "session status: unknown item",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["mood"] = json!("fine")),
        ),
        Do(
            "session status: digest without sequence",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["checkpoint_seq"] = Value::Null),
        ),
        Do(
            "session status: generation without assignment",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["assignment_id"] = Value::Null),
        ),
        Do(
            "session status: not JSON",
            Some("sup-dev-a"),
            status_raw("st-000005", b"not json".to_vec()),
        ),
        Do(
            "session status: not UTF-8",
            Some("sup-dev-a"),
            status_raw("st-000005", vec![0xff, 0xfe, b'{', b'}']),
        ),
        Do(
            "session status: over 1 MiB",
            Some("sup-dev-a"),
            status_with("st-000005", |d| d["session_id"] = json!(big.clone())),
        ),
        Do(
            "session status: running with a first action",
            Some("sup-dev-a"),
            status_with("st-000006", |d| {
                d["observed_at"] = json!("2026-11-15T02:05:00Z");
                d["lifecycle"] = json!("running");
                d["last_actual_action_at"] = json!("2026-11-15T02:04:00.5Z");
                d["active_children"] = json!([]);
                d["pending_external_requests"] = json!([{"request_id": "r1"}]);
            }),
        ),
        // --- GetLatestCheckpoint
        Do(
            "latest: reporter",
            Some("sup-dev-a"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: operator",
            Some("operator"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: operator without repo",
            Some("operator-norepo"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: client without scope",
            Some("alice"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: model credential",
            Some("dev-a"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: other role",
            Some("sup-gr-a"),
            latest("repo-a", "development"),
        ),
        Do(
            "latest: unknown role",
            Some("sup-dev-a"),
            latest("repo-a", "manager"),
        ),
        Do(
            "latest: none for guardrail",
            Some("sup-gr-a"),
            latest("repo-a", "guardrail"),
        ),
        Do(
            "latest: no credential",
            None,
            latest("repo-a", "development"),
        ),
        Do(
            "latest: slash",
            Some("sup-dev-a"),
            latest("repo/a", "development"),
        ),
        // --- a new generation: old results preserved, excluded from the current judgment
        setup("A1/G1 gen 1", |e| {
            assign_ok(
                e,
                "development",
                "A1",
                1,
                SHA2,
                json!({"assignment_id": "A1", "generation": 0}),
            );
            assign_ok(
                e,
                "guardrail",
                "G1",
                1,
                SHA2,
                json!({"assignment_id": "G1", "generation": 0}),
            );
        }),
        Do(
            "report: replay after the move",
            Some("dev-a"),
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: late claim preserved",
            Some("dev-a"),
            report_call("development", "dev-late", "A1", 0, "01-01", SHA1),
        ),
        Do(
            "report: sha of another generation",
            Some("dev-a"),
            report_call("development", "dev-y", "A1", 1, "01-01", SHA1),
        ),
        Do(
            "report: current gen",
            Some("dev-a"),
            report_call("development", "dev-2", "A1", 1, "01-01", SHA2),
        ),
        Do(
            "repro: replay after the move",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        ),
        Do(
            "repro: stale generation",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0007", 0, SHA1)),
        ),
        Do(
            "repro: current gen, old sha",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0008", 1, SHA1)),
        ),
        Do(
            "repro: current gen",
            Some("gr-a"),
            Call::Repro(repro_msg("repro-0009", 1, SHA2)),
        ),
        Do(
            // the run whose takeover was recorded under generation 0 keeps its next checkpoint,
            // stored as not current (preserved, excluded)
            "checkpoint: the old run's next one, not current",
            Some("sup-dev-a"),
            cpx(3, 0, json!({"session_id": "s2"})),
        ),
        Do(
            "checkpoint: old generation after that",
            Some("sup-dev-a"),
            cpx(4, 0, json!({"session_id": "s2"})),
        ),
        Do(
            "checkpoint: new generation takeover",
            Some("sup-dev-a"),
            cpx(
                4,
                1,
                json!({"session_id": "s3", "phase": "takeover", "target_sha": SHA2}),
            ),
        ),
        Do(
            "latest: after the move",
            Some("sup-dev-a"),
            latest("repo-a", "development"),
        ),
        // --- 02-05 step 2 after the move: the old run's facts are kept, not current
        Do(
            "own assignment: after the move",
            Some("sup-dev-a"),
            own("repo-a", "development"),
        ),
        Do(
            "first action: replay after the move",
            Some("sup-dev-a"),
            fa("A1", 0, "att-1", "s1"),
        ),
        Do(
            "first action: the old run's, not current",
            Some("sup-dev-a"),
            fa("A1", 0, "att-2", "s2"),
        ),
        Do(
            "first action: new generation",
            Some("sup-dev-a"),
            fa("A1", 1, "att-3", "s3"),
        ),
        Do(
            "first action: new generation, old session",
            Some("sup-dev-a"),
            fa("A1", 1, "att-4", "s2"),
        ),
        Do(
            "session status: replay after the move",
            Some("sup-dev-a"),
            status_with("st-000002", |_| {}),
        ),
        Do(
            "session status: old generation, not current",
            Some("sup-dev-a"),
            status_with("st-000007", |d| {
                d["observed_at"] = json!("2026-11-15T03:00:00Z")
            }),
        ),
        Do(
            "session status: new generation",
            Some("sup-dev-a"),
            status_with("st-000008", |d| {
                d["observed_at"] = json!("2026-11-15T03:01:00Z");
                d["generation"] = json!(1);
                d["session_id"] = json!("s3");
            }),
        ),
    ]
}

#[test]
fn both_transports_give_the_same_answers_and_the_same_database() {
    let Some(h) = start() else { return };
    let Some(r) = start() else { return };
    let g = grpc(&r);
    let digest = spec("repo-a", 5, 3)["digest"]
        .as_str()
        .expect("digest")
        .to_string();
    for e in [&h, &r] {
        ready_sprint(e, 3);
    }
    let mut seen = std::collections::BTreeSet::new();
    let (mut dh, mut dr) = (h.db(), r.db());
    for step in scenario(&digest) {
        match step {
            Step::Setup(label, f) => {
                f(&h);
                f(&r);
                eprintln!("setup: {label}");
            }
            Step::Do(label, who, call) => {
                let before = snapshot(&mut dh);
                let a = over_http(&h, who, &call);
                let t0 = Instant::now();
                let b = g.call(&r, who, &call);
                let took = t0.elapsed();
                assert_eq!(a, b, "{label}: HTTP vs gRPC");
                let after = (snapshot(&mut dh), snapshot(&mut dr));
                assert_eq!(after.0, after.1, "{label}: database state");
                if !(200..300).contains(&a.0) {
                    assert_eq!(before, after.0, "{label}: a refusal changes nothing");
                }
                let code = a.1["code"]
                    .as_str()
                    .or_else(|| a.1["rejections"][0]["code"].as_str())
                    .unwrap_or("OK")
                    .to_string();
                seen.insert((a.0, code));
                // the handling time over gRPC (contract §6.1 records it)
                eprintln!("{label}: {} {} [grpc {} ms]", a.0, a.1, took.as_millis());
            }
        }
    }
    // the §4.2 rows this scenario reaches (500 INTERNAL needs a server fault: see the
    // database-unavailable and deadline tests)
    for want in [
        (401, "UNAUTHENTICATED"),
        (403, "SCOPE_DENIED"),
        (403, "REPO_SCOPE_DENIED"),
        (403, "ROLE_MISMATCH"),
        (403, "VERIFIED_FROM_AGENT_DENIED"),
        (400, "SCHEMA_VIOLATION"),
        (400, "MISSING_FIELD"),
        (404, "NOT_FOUND"),
        (404, "NO_CHECKPOINT"),
        (409, "EVENT_CONTENT_CONFLICT"),
        (409, "STALE_ASSIGNMENT"),
        (409, "ASSIGNMENT_SHA_MISMATCH"),
        (409, "CHECKPOINT_SEQUENCE_GAP"),
        (409, "CHECKPOINT_MISMATCH"),
        (409, "NO_CURRENT_ASSIGNMENT"),
        (409, "DIGEST_MISMATCH"),
        (409, "REPO_MISMATCH"),
        (422, "REPORT_SEMANTIC_VIOLATION"),
        (404, "NO_CURRENT_ASSIGNMENT"),
        (409, "IDEMPOTENCY_KEY_CONFLICT"),
    ] {
        assert!(
            seen.contains(&(want.0, want.1.to_string())),
            "not reached: {want:?} in {seen:?}"
        );
    }
    // stale generation: the late claim is preserved and excluded from the current judgment
    let late: Value = h
        .db()
        .query_one(
            "SELECT to_jsonb(e) FROM events e WHERE event_id = 'dev-late'",
            &[],
        )
        .expect("late")
        .get(0);
    assert_eq!(late["disposition"], "preserved_late", "{late}");
    let (_, st) = h.get("alice", "/v0/sprints/POC-01/state");
    let (_, st2) = r.get("alice", "/v0/sprints/POC-01/state");
    assert_eq!(st["latest_claims"], st2["latest_claims"]);
    let ids: Vec<&str> = st2["latest_claims"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["event_id"].as_str())
        .collect();
    assert!(!ids.contains(&"dev-late"), "{st2}");
}

#[test]
fn a_record_stored_over_one_transport_replays_over_the_other() {
    let Some(e) = start() else { return };
    let g = grpc(&e);
    ready_sprint(&e, 3);
    assign_ok(&e, "development", "A1", 0, SHA1, Value::Null);
    assign_ok(&e, "guardrail", "G1", 0, SHA1, Value::Null);
    let digest = spec("repo-a", 5, 3)["digest"]
        .as_str()
        .expect("digest")
        .to_string();
    let stored_then_replayed = |first_http: bool, who: &str, c: Call, stored: u16| {
        let (a, b) = if first_http {
            (over_http(&e, Some(who), &c), g.call(&e, Some(who), &c))
        } else {
            (g.call(&e, Some(who), &c), over_http(&e, Some(who), &c))
        };
        assert_eq!(a.0, stored, "{a:?}");
        let before = e.counts();
        assert_eq!(b.0, 200, "{b:?}");
        let again = if first_http {
            g.call(&e, Some(who), &c)
        } else {
            over_http(&e, Some(who), &c)
        };
        assert_eq!(again, b, "a replay answers the stored record each time");
        assert_eq!(e.counts(), before, "no new rows");
        (a, b)
    };
    // reports: the same report sent as other bytes (key order, spacing) is the same report
    let r1 = report("development", "dev-1", "A1", 0, "01-01", SHA1);
    let (a, b) = stored_then_replayed(
        true,
        "dev-a",
        Call::Report(serde_json::to_vec(&r1).expect("j")),
        202,
    );
    assert_eq!(a.1, b.1, "the stored receipt");
    let pretty = serde_json::to_vec_pretty(&r1).expect("j");
    assert_eq!(g.call(&e, Some("dev-a"), &Call::Report(pretty)).0, 200);
    stored_then_replayed(
        false,
        "dev-a",
        report_call("development", "dev-2", "A1", 0, "01-01", SHA1),
        202,
    );
    // repro requests: typed fields over gRPC, the same JSON value over HTTP
    stored_then_replayed(
        true,
        "gr-a",
        Call::Repro(repro_msg("repro-0001", 0, SHA1)),
        202,
    );
    stored_then_replayed(
        false,
        "gr-a",
        Call::Repro(SubmitReproRequestRequest {
            timeout_seconds: Some(30),
            ..repro_msg("repro-0002", 0, SHA1)
        }),
        202,
    );
    // §2: an explicit null cannot be sent over gRPC, so that stored request conflicts there
    let mut with_null = ieo_proto::repro_request_json(&repro_msg("repro-0003", 0, SHA1));
    with_null["timeout_seconds"] = Value::Null;
    assert_eq!(
        e.post("gr-a", "/v0/agent/repro-requests", &with_null).0,
        202
    );
    let (st, b) = g.call(
        &e,
        Some("gr-a"),
        &Call::Repro(repro_msg("repro-0003", 0, SHA1)),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("EVENT_CONTENT_CONFLICT")),
        "{b}"
    );
    assert_eq!(
        g.call(&e, Some("gr-a"), &Call::GetRepro("repro-0003".into()))
            .0,
        200,
        "confirmed with GetReproRequest instead (§5)"
    );
    // checkpoints: the exact bytes, the same digest
    let c1 = cp(1, cp_raw(1, 0, &digest, json!({"phase": "takeover"})));
    let (a, _) = stored_then_replayed(false, "sup-dev-a", c1, 201);
    let c2 = cp(2, cp_raw(2, 0, &digest, json!({})));
    let (b, _) = stored_then_replayed(true, "sup-dev-a", c2, 201);
    let (_, l) = over_http(&e, Some("sup-dev-a"), &latest("repo-a", "development"));
    assert_eq!(
        (l["seq"].as_u64(), &l["checkpoint_sha256"]),
        (Some(2), &b.1["checkpoint_sha256"])
    );
    assert_ne!(a.1["checkpoint_sha256"], b.1["checkpoint_sha256"]);
    // 02-05 step 2: first actions (typed fields, the same JSON value) and session statuses
    // (the status bytes, the Idempotency-Key)
    stored_then_replayed(false, "sup-dev-a", fa("A1", 0, "att-1", "s1"), 201);
    stored_then_replayed(
        true,
        "sup-dev-a",
        status(
            "st-000001",
            &status_doc("2026-11-15T00:00:00Z", Some(("A1", 0))),
        ),
        201,
    );
    stored_then_replayed(
        false,
        "sup-dev-a",
        status("st-000002", &status_doc("2026-11-15T00:01:00Z", None)),
        201,
    );
}

/// 02-05 step 2 (PLAN §3.1): what the three supervisor operations answer and store. The
/// supervisor's facts are claims: they create no event, change no sprint state and are never a
/// completion signal.
#[test]
fn supervisor_operations_answer_the_assignment_and_store_claims() {
    let Some(e) = start() else { return };
    let g = grpc(&e);
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    assign_ok(&e, "development", "A1", 0, SHA1, Value::Null);
    // reads: both transports give the same answer
    let both = |who: &str, c: &Call| {
        let a = over_http(&e, Some(who), c);
        let b = g.call(&e, Some(who), c);
        assert_eq!(a, b, "{c:?}");
        a
    };
    // writes: alternately over gRPC and HTTP (the replays are in the test above)
    let n = std::cell::Cell::new(0);
    let write = |c: &Call| {
        n.set(n.get() + 1);
        if n.get() % 2 == 1 {
            g.call(&e, Some("sup-dev-a"), c)
        } else {
            over_http(&e, Some("sup-dev-a"), c)
        }
    };
    let (st, a) = both("sup-dev-a", &own("repo-a", "development"));
    assert_eq!(st, 200, "{a}");
    assert_eq!(
        a,
        json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A1",
            "generation": 0, "task_id": "01-01", "target_sha": SHA1, "sprint_id": "POC-01",
            "sprint_revision": 5, "spec_digest": digest, "sources": s["sources"],
            "limits": {"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 3},
            "attempts_issued": 1, "policy_digest": e.app.ctx.policy.digest,
            "last_delivery_id": null, "expected_runtime": null})
    );
    let (_, gr) = both("sup-gr-a", &own("repo-a", "guardrail"));
    assert_eq!(gr["code"], "NO_CURRENT_ASSIGNMENT", "{gr}");
    // only the (repo, role) supervisor credential
    for (who, c, want) in [
        ("dev-a", own("repo-a", "development"), (403, "SCOPE_DENIED")),
        (
            "operator",
            own("repo-a", "development"),
            (403, "SCOPE_DENIED"),
        ),
        (
            "sup-gr-a",
            own("repo-a", "development"),
            (403, "ROLE_MISMATCH"),
        ),
        (
            "sup-dev-a",
            own("repo-b", "development"),
            (403, "REPO_SCOPE_DENIED"),
        ),
        ("dev-a", fa("A1", 0, "att-1", "s1"), (403, "SCOPE_DENIED")),
        (
            "sup-gr-a",
            fa("A1", 0, "att-1", "s1"),
            (403, "ROLE_MISMATCH"),
        ),
        (
            "dev-a",
            status("st-000009", &status_doc("2026-11-15T00:00:00Z", None)),
            (403, "SCOPE_DENIED"),
        ),
        (
            "sup-gr-a",
            status("st-000009", &status_doc("2026-11-15T00:00:00Z", None)),
            (403, "ROLE_MISMATCH"),
        ),
    ] {
        let (st, b) = both(who, &c);
        assert_eq!((st, code(&b).as_str()), want, "{who} {c:?}");
    }
    let state = |e: &Env| e.get("alice", "/v0/sprints/POC-01/state").1;
    let before = (state(&e), e.counts());
    // no takeover of s1 stored yet: refused
    let (st, r) = write(&fa("A1", 0, "att-1", "s1"));
    assert_eq!((st, code(&r).as_str()), (409, "CHECKPOINT_MISMATCH"), "{r}");
    // the takeover of s1, then its first action and a status
    let c1 = cp(1, cp_raw(1, 0, &digest, json!({"phase": "takeover"})));
    assert_eq!(over_http(&e, Some("sup-dev-a"), &c1).0, 201);
    let (st, r) = write(&fa("A1", 0, "att-1", "s1"));
    assert_eq!(
        (st, r["evidence_level"].as_str()),
        (201, Some("claim")),
        "{r}"
    );
    assert_eq!(r["current_assignment"], true);
    let other = Call::FirstAction(RecordFirstActionRequest {
        event_offset: 9,
        ..fa_msg("A1", 0, "att-1", "s1")
    });
    let (st, r) = write(&other);
    assert_eq!(
        (st, code(&r).as_str()),
        (409, "EVENT_CONTENT_CONFLICT"),
        "{r}"
    );
    let st1 = status(
        "st-000001",
        &status_doc("2026-11-15T02:00:00Z", Some(("A1", 0))),
    );
    let (st, r) = write(&st1);
    assert_eq!(
        (st, &r["stale_on_arrival"], &r["current_assignment"]),
        (201, &json!(false), &json!(true)),
        "{r}"
    );
    let (st, r) = write(&status_with("st-000001", |d| {
        d["lifecycle"] = json!("running")
    }));
    assert_eq!(
        (st, code(&r).as_str()),
        (409, "IDEMPOTENCY_KEY_CONFLICT"),
        "{r}"
    );
    let older = status("st-000002", &status_doc("2026-11-15T01:00:00Z", None));
    let (st, r) = write(&older);
    assert_eq!(
        (st, &r["stale_on_arrival"], &r["current_assignment"]),
        (201, &json!(true), &Value::Null),
        "{r}"
    );
    // expected values the transport comparison alone does not pin (PR #73 review a349dc6 L-1):
    // R2: the same observed_at as a stored status is stale too ("at or before")
    let same = status(
        "st-000004",
        &status_doc("2026-11-15T02:00:00Z", Some(("A1", 0))),
    );
    let (st, r) = write(&same);
    assert_eq!((st, &r["stale_on_arrival"]), (201, &json!(true)), "{r}");
    // R5: an impossible calendar day is a 400 before PostgreSQL sees it (never a 500)
    for when in [
        "2026-02-30T00:00:00Z",
        "2025-02-29T00:00:00Z",
        "2026-04-31T00:00:00Z",
    ] {
        let (st, r) = write(&status("st-000005", &status_doc(when, None)));
        assert_eq!(
            (st, r["rejections"][0]["code"].as_str()),
            (400, Some("SCHEMA_VIOLATION")),
            "{when} {r}"
        );
    }
    // R4: a takeover of another session under the same assignment and generation is not this
    // session's takeover
    let (st, r) = write(&fa("A1", 0, "att-2", "s2"));
    assert_eq!((st, code(&r).as_str()), (409, "CHECKPOINT_MISMATCH"), "{r}");
    // claims only: no event, no sprint state change, no judgment input
    let mut db = e.db();
    let levels: Vec<String> = db
        .query(
            "SELECT evidence_level FROM role_first_actions UNION ALL
             SELECT evidence_level FROM role_session_status",
            &[],
        )
        .expect("levels")
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(levels, ["claim", "claim", "claim", "claim"]);
    let after = (state(&e), e.counts());
    assert_eq!(after.0, before.0, "sprint state unchanged");
    let changed: Vec<&str> = before
        .1
        .iter()
        .zip(&after.1)
        .filter(|(x, y)| x != y)
        .map(|(x, _)| x.0.as_str())
        .collect();
    assert_eq!(
        changed,
        [
            "history",
            "role_checkpoints",
            "role_first_actions",
            "role_session_status",
            // the takeovers' links to their (fixture) deliveries, also claims (02-05 step 4)
            "role_takeovers"
        ]
    );
    // a new generation: the own assignment follows the head; the old run's facts stay, not current
    assign_ok(
        &e,
        "development",
        "A1",
        1,
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    let (_, a) = both("sup-dev-a", &own("repo-a", "development"));
    assert_eq!(
        (&a["generation"], &a["target_sha"], &a["attempts_issued"]),
        (&json!(1), &json!(SHA2), &json!(2)),
        "{a}"
    );
    let (st, r) = write(&fa("A1", 0, "att-1", "s1"));
    assert_eq!(
        (st, &r["current_assignment"]),
        (200, &json!(true)),
        "stored value"
    );
    let c2 = cp(2, cp_raw(2, 0, &digest, json!({"phase": "after_run"})));
    assert_eq!(over_http(&e, Some("sup-dev-a"), &c2).0, 201);
    let (st, r) = write(&fa("A1", 0, "att-1b", "s1"));
    assert_eq!((st, &r["current_assignment"]), (201, &json!(false)), "{r}");
    let late = status(
        "st-000003",
        &status_doc("2026-11-15T03:00:00Z", Some(("A1", 0))),
    );
    let (st, r) = write(&late);
    assert_eq!(
        (st, &r["stale_on_arrival"], &r["current_assignment"]),
        (201, &json!(false), &json!(false)),
        "{r}"
    );
}

// ---------------------------------------------------------------------------
// Deadline, cancellation and lost answers (§6)
// ---------------------------------------------------------------------------

/// Holds the advisory lock a checkpoint report takes, from another session.
fn hold_checkpoint_lock(e: &Env) -> postgres::Client {
    let mut db = e.db();
    db.execute(
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        &[&"checkpoint:repo-a:development"],
    )
    .expect("lock");
    db
}

fn release(mut db: postgres::Client) {
    db.execute(
        "SELECT pg_advisory_unlock(hashtextextended($1, 0))",
        &[&"checkpoint:repo-a:development"],
    )
    .expect("unlock");
}

fn checkpoints(e: &Env) -> i64 {
    e.db()
        .query_one("SELECT count(*) FROM role_checkpoints", &[])
        .expect("count")
        .get(0)
}

fn ready_for_checkpoints(e: &Env) -> String {
    ready_sprint(e, 3);
    assign_ok(e, "development", "A1", 0, SHA1, Value::Null);
    spec("repo-a", 5, 3)["digest"]
        .as_str()
        .expect("digest")
        .to_string()
}

#[test]
fn a_statement_past_the_deadline_rolls_back_as_deadline_exceeded() {
    let Some(e) = start() else { return };
    let g = grpc(&e);
    let digest = ready_for_checkpoints(&e);
    let c1 = cp(1, cp_raw(1, 0, &digest, json!({"phase": "takeover"})));
    let lock = hold_checkpoint_lock(&e);
    let t0 = Instant::now();
    let r = g.raw(&e, Some("sup-dev-a"), &c1, Some(Duration::from_millis(600)));
    let took = t0.elapsed();
    let st = r.expect_err("deadline");
    assert_eq!(st.code(), Code::DeadlineExceeded, "{st:?}");
    assert!(ieo_proto::error_from_status(&st).is_none(), "no trailer");
    // the server's own answer (rolled back before its handling deadline), not the client's
    // timer: a timing bound alone has only the 50 ms margin (PR #64 review c6c0656 L-1)
    assert!(
        st.message().contains("rolled back"),
        "answered by the server: {st:?}"
    );
    assert!(took < Duration::from_secs(5), "bounded: {took:?}");
    release(lock);
    assert_eq!(checkpoints(&e), 0, "rolled back");
    // the server's own limit applies without a client deadline (15 s by default; 700 ms here)
    let g2 = grpc_with(
        &e,
        GrpcConfig {
            workers: 1,
            max_deadline: Duration::from_millis(700),
            health_interval: Duration::from_secs(5),
        },
    );
    let lock = hold_checkpoint_lock(&e);
    let st = g2.raw(&e, Some("sup-dev-a"), &c1, None).expect_err("cap");
    assert_eq!(st.code(), Code::DeadlineExceeded, "{st:?}");
    release(lock);
    assert_eq!(checkpoints(&e), 0);
    // too little time left: nothing starts
    let st = g
        .raw(&e, Some("sup-dev-a"), &c1, Some(Duration::from_millis(20)))
        .expect_err("short");
    assert!(
        matches!(st.code(), Code::DeadlineExceeded | Code::Cancelled),
        "{st:?}"
    );
    assert_eq!(checkpoints(&e), 0);
    // with time, the same report is stored
    assert_eq!(g.call(&e, Some("sup-dev-a"), &c1).0, 201);
}

#[test]
fn a_call_cancelled_before_commit_rolls_back() {
    let Some(e) = start() else { return };
    // one worker: the next call runs after the cancelled one has finished
    let g = grpc_with(
        &e,
        GrpcConfig {
            workers: 1,
            health_interval: Duration::from_secs(60),
            ..GrpcConfig::default()
        },
    );
    let digest = ready_for_checkpoints(&e);
    let c1 = cp(1, cp_raw(1, 0, &digest, json!({"phase": "takeover"})));
    prepare(&e, &c1);
    let lock = hold_checkpoint_lock(&e);
    let mut cl = g.client();
    // the client gives up (drops the call: RST_STREAM) while the handler waits for the lock
    let r = g.rt.block_on(async {
        tokio::time::timeout(
            Duration::from_millis(400),
            G::send(&mut cl, &e, Some("sup-dev-a"), &c1, None),
        )
        .await
    });
    assert!(r.is_err(), "the client gave up");
    std::thread::sleep(Duration::from_millis(200));
    release(lock);
    // the handler then writes, sees the cancellation before commit and rolls back
    let (st, b) = g.call(&e, Some("sup-dev-a"), &latest("repo-a", "development"));
    assert_eq!(
        (st, b["code"].as_str()),
        (404, Some("NO_CHECKPOINT")),
        "{b}"
    );
    assert_eq!(checkpoints(&e), 0);
}

#[test]
fn a_transaction_guard_that_refuses_the_commit_leaves_nothing() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assign_ok(&e, "development", "A1", 0, SHA1, Value::Null);
    struct Late;
    impl TxGuard for Late {
        fn before_commit(&self) -> Result<(), Abort> {
            Err(Abort::DeadlineExceeded)
        }
    }
    let body =
        serde_json::to_vec(&report("development", "dev-1", "A1", 0, "01-01", SHA1)).expect("j");
    let who = e
        .app
        .creds
        .authenticate(Some(&format!("Bearer {}", e.tokens["dev-a"])));
    let req = Req {
        method: "POST",
        path: "/v0/agent/reports",
        query: "",
        idempotency_key: None,
        body: &body,
    };
    let before = e.counts();
    let mut db = e.db();
    let r = api::dispatch(
        &mut db,
        &e.app.ctx,
        who,
        Some(Operation::SubmitReport),
        &req,
        &Late,
    );
    assert_eq!(r, Err(Abort::DeadlineExceeded));
    assert_eq!(e.counts(), before, "rolled back");
    // the HTTP path's guard always passes
    let r = api::handle(&mut db, &e.app.ctx, who, &req);
    assert_eq!(r.status, 202, "{:?}", r.body);
}

/// A TCP relay that can drop everything the server sends: the server commits and answers, the
/// client never sees the answer (a connection lost after commit).
struct Relay {
    addr: String,
    blackhole: Arc<AtomicBool>,
}

fn relay(g: &G, upstream: std::net::SocketAddr) -> Relay {
    let blackhole = Arc::new(AtomicBool::new(false));
    let listener =
        g.rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let bh = Arc::clone(&blackhole);
    g.rt.spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((client, _)) = listener.accept().await {
            let Ok(server) = tokio::net::TcpStream::connect(upstream).await else {
                continue;
            };
            let (mut cr, mut cw) = client.into_split();
            let (mut sr, mut sw) = server.into_split();
            tokio::spawn(async move {
                let _ = tokio::io::copy(&mut cr, &mut sw).await;
            });
            let bh = Arc::clone(&bh);
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16 * 1024];
                loop {
                    let n = match sr.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    if !bh.load(Ordering::SeqCst) && cw.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    Relay { addr, blackhole }
}

#[test]
fn an_answer_lost_after_commit_is_found_by_its_key_and_not_stored_twice() {
    let Some(e) = start() else { return };
    let g = grpc(&e);
    let digest = ready_for_checkpoints(&e);
    let addr = g.server.as_ref().expect("server").addr;
    let rl = relay(&g, addr);
    let mut cl = SupervisorServiceClient::new(g.channel(&format!("http://{}", rl.addr)));
    // the connection is up (an answered call through the relay)
    let warm = g.rt.block_on(G::send(
        &mut cl,
        &e,
        Some("sup-dev-a"),
        &latest("repo-a", "development"),
        None,
    ));
    assert_eq!(answer(warm).0, 404);
    let calls = [
        (
            "sup-dev-a",
            cp(1, cp_raw(1, 0, &digest, json!({"phase": "takeover"}))),
        ),
        (
            "dev-a",
            report_call("development", "dev-1", "A1", 0, "01-01", SHA1),
        ),
    ];
    for (who, c) in &calls {
        prepare(&e, c);
        let before = e.counts();
        rl.blackhole.store(true, Ordering::SeqCst);
        let lost = g.rt.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(2),
                G::send(&mut cl, &e, Some(who), c, Some(Duration::from_secs(5))),
            )
            .await
        });
        assert!(lost.is_err(), "the client saw no answer");
        // the server committed anyway
        assert_ne!(e.counts(), before, "the request was applied");
        let applied = e.counts();
        // the same key again, directly: the stored result, nothing new (§5, §6.1)
        let (st, b) = g.call(&e, Some(who), c);
        assert_eq!(st, 200, "{b}");
        assert_eq!(over_http(&e, Some(who), c).0, 200);
        assert_eq!(e.counts(), applied, "no second record");
    }
    let (_, l) = g.call(&e, Some("sup-dev-a"), &latest("repo-a", "development"));
    assert_eq!(l["seq"], 1, "{l}");
}

// ---------------------------------------------------------------------------
// Size limits, path values, database and health (§3, §7)
// ---------------------------------------------------------------------------

#[test]
fn transport_limits_and_utf8_refusals() {
    let Some(e) = start() else { return };
    let g = grpc(&e);
    let digest = ready_for_checkpoints(&e);
    // over the 1 MiB + 64 KiB message limit: the library refuses without a trailer (§7.1);
    // HTTP answers the same bytes with its 400 before authentication
    let mut r = report("development", "dev-big", "A1", 0, "01-01", SHA1);
    r["next_action"] = json!("x".repeat(ieo_proto::MAX_MESSAGE));
    let huge = Call::Report(serde_json::to_vec(&r).expect("j"));
    let st = g.raw(&e, Some("dev-a"), &huge, None).expect_err("limit");
    // tonic 0.14 answers OUT_OF_RANGE (other gRPC libraries: RESOURCE_EXHAUSTED)
    assert!(
        matches!(st.code(), Code::OutOfRange | Code::ResourceExhausted),
        "{st:?}"
    );
    assert!(ieo_proto::error_from_status(&st).is_none());
    assert_eq!(over_http(&e, Some("dev-a"), &huge).0, 400);
    // checkpoint bytes that are not UTF-8: refused after authentication, gRPC only (§2)
    let mut raw = cp_raw(1, 0, &digest, json!({"phase": "takeover"}));
    raw.push(0xff);
    let bad = cp(1, raw);
    let (st, b) = g.call(&e, Some("sup-dev-a"), &bad);
    assert_eq!(
        (
            st,
            b["rejections"][0]["code"].as_str(),
            b["rejections"][0]["field"].as_str()
        ),
        (400, Some("SCHEMA_VIOLATION"), Some("checkpoint_json")),
        "{b}"
    );
    assert_eq!(g.call(&e, None, &bad).0, 401, "authentication first");
    // `?` and `#` in a path value: 404 after authentication; HTTP would read another path
    // (P-1). A stored request id with `?` cannot be read by either transport.
    let q = repro_msg("repro-0001?x", 0, SHA1);
    assign_ok(&e, "guardrail", "G1", 0, SHA1, Value::Null);
    assert_eq!(g.call(&e, Some("gr-a"), &Call::Repro(q)).0, 202);
    for id in ["repro-0001?x", "repro#0001", "a/b", ""] {
        let (st, b) = g.call(&e, Some("gr-a"), &Call::GetRepro(id.into()));
        assert_eq!(
            (st, b["code"].as_str()),
            (404, Some("NOT_FOUND")),
            "{id}: {b}"
        );
        assert_eq!(g.call(&e, None, &Call::GetRepro(id.into())).0, 401, "{id}");
    }
    for repo in ["repo-a?x", "repo-a#x"] {
        let (st, _) = g.call(&e, Some("sup-dev-a"), &latest(repo, "development"));
        assert_eq!(st, 404, "{repo}");
    }
}

#[test]
fn database_unavailable_is_unavailable_with_its_trailer_and_health_follows() {
    use tonic_health::pb::health_check_response::ServingStatus;
    use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};
    let Some(e) = start() else { return };
    let g = grpc_with(
        &e,
        GrpcConfig {
            workers: 1,
            health_interval: Duration::from_millis(100),
            ..GrpcConfig::default()
        },
    );
    let health = |service: &str| -> Result<i32, Code> {
        let mut h = HealthClient::new(g.channel(&g.url));
        g.rt.block_on(h.check(HealthCheckRequest {
            service: service.into(),
        }))
        .map(|r| r.into_inner().status)
        .map_err(|s| s.code())
    };
    let serving = ServingStatus::Serving as i32;
    let not_serving = ServingStatus::NotServing as i32;
    assert_eq!(health(""), Ok(serving));
    assert_eq!(health(ieo_proto::SERVICE_NAME), Ok(serving));
    assert_eq!(health("other.Service"), Err(Code::NotFound));
    // the database goes away: the worker's connection is cut and cannot be re-opened
    g.db_down.store(true, Ordering::SeqCst);
    e.db()
        .execute(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name = $1",
            &[&g.app_name],
        )
        .expect("terminate");
    let wait = |want: i32| {
        let t0 = Instant::now();
        while health("") != Ok(want) {
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "health stays {:?}",
                health("")
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    wait(not_serving);
    assert_eq!(health(ieo_proto::SERVICE_NAME), Ok(not_serving));
    let st = g
        .raw(
            &e,
            Some("sup-dev-a"),
            &latest("repo-a", "development"),
            None,
        )
        .expect_err("down");
    assert_eq!(st.code(), Code::Unavailable, "{st:?}");
    let err = ieo_proto::error_from_status(&st).expect("trailer");
    let http = ieo_coord::http::db_unavailable();
    assert_eq!(
        (
            u16::try_from(err.http_status).ok(),
            ieo_proto::body_of(&err)
        ),
        (Some(http.status), http.body),
        "the HTTP 500 answer, unchanged"
    );
    // even an unauthenticated call: nothing runs without a database (§7.0 order)
    let st = g
        .raw(&e, None, &latest("repo-a", "development"), None)
        .expect_err("down");
    assert_eq!(st.code(), Code::Unavailable);
    // back
    g.db_down.store(false, Ordering::SeqCst);
    wait(serving);
    let (st, b) = g.call(&e, Some("sup-dev-a"), &latest("repo-a", "development"));
    assert_eq!(
        (st, b["code"].as_str()),
        (404, Some("NO_CHECKPOINT")),
        "{b}"
    );
}

/// The supervisor's size pre-check records exactly the HTTP front end's refusal (§7.1).
#[test]
fn the_client_pre_check_refusal_is_the_front_end_answer() {
    let r = ieo_coord::http::body_too_large();
    assert_eq!((r.status, r.body), (400, ieo_proto::body_too_large_json()));
}
