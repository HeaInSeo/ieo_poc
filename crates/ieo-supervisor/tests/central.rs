//! X-04: the supervisor's coordinator clients classify answers the same way (contracts/
//! ieo-internal-v0.md §5): a refusal the coordinator decided (HTTP 4xx, or the gRPC trailer's
//! `http_status` 400–499, or the client's own size pre-check) versus an unknown outcome (5xx,
//! any status without a trailer, transport failure). Nothing is resent.

use ieo_supervisor::central::{Central, Refusal};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tonic::Code;

#[derive(Clone)]
enum Script {
    /// An answer of the coordinator's handling: HTTP status and body (gRPC: status + trailer).
    Answer(u16, Value),
    /// gRPC only: a status without a trailer (library, deadline, cancellation).
    Bare(Code),
}

#[derive(Clone)]
struct Scripted {
    next: Arc<Mutex<VecDeque<Script>>>,
    calls: Arc<AtomicUsize>,
}

impl Scripted {
    fn pop(&self) -> Script {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.next
            .lock()
            .expect("script")
            .pop_front()
            .unwrap_or(Script::Answer(
                500,
                json!({"code": "INTERNAL", "message": "empty"}),
            ))
    }
}

#[tonic::async_trait]
impl ieo_proto::v0::supervisor_service_server::SupervisorService for Scripted {
    async fn submit_agent_report(
        &self,
        _: tonic::Request<ieo_proto::v0::SubmitAgentReportRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::SubmitAgentReportResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn submit_repro_request(
        &self,
        _: tonic::Request<ieo_proto::v0::SubmitReproRequestRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::SubmitReproRequestResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn get_repro_request(
        &self,
        _: tonic::Request<ieo_proto::v0::GetReproRequestRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::GetReproRequestResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn report_checkpoint(
        &self,
        r: tonic::Request<ieo_proto::v0::ReportCheckpointRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::ReportCheckpointResponse>, tonic::Status> {
        // the client sends its deadline and credential
        assert!(r.metadata().get("grpc-timeout").is_some());
        assert_eq!(
            r.metadata()
                .get("authorization")
                .and_then(|v| v.to_str().ok()),
            Some("Bearer sup-token")
        );
        match self.pop() {
            Script::Answer(st @ 200..=299, v) => Ok(tonic::Response::new(
                ieo_proto::checkpoint_answer_from_json(st, &v),
            )),
            Script::Answer(st, v) => Err(ieo_proto::status_with_error(
                ieo_proto::grpc_code(st, &v),
                st,
                &v,
            )),
            Script::Bare(c) => Err(tonic::Status::new(c, "no trailer")),
        }
    }
    async fn get_latest_checkpoint(
        &self,
        _: tonic::Request<ieo_proto::v0::GetLatestCheckpointRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::GetLatestCheckpointResponse>, tonic::Status> {
        match self.pop() {
            Script::Answer(200, v) => Ok(tonic::Response::new(ieo_proto::latest_from_json(&v))),
            Script::Answer(st, v) => Err(ieo_proto::status_with_error(
                ieo_proto::grpc_code(st, &v),
                st,
                &v,
            )),
            Script::Bare(c) => Err(tonic::Status::new(c, "no trailer")),
        }
    }
}

struct Fakes {
    scripted: Scripted,
    http: Central,
    grpc: Central,
    _dir: Dir,
}

struct Dir(std::path::PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fakes(name: &str) -> Fakes {
    let scripted = Scripted {
        next: Arc::new(Mutex::new(VecDeque::new())),
        calls: Arc::new(AtomicUsize::new(0)),
    };
    // HTTP
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let http_url = format!("http://{}", server.server_addr().to_ip().expect("ip"));
    let s = scripted.clone();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            match s.pop() {
                Script::Answer(st, v) => {
                    let _ = req.respond(
                        tiny_http::Response::from_string(v.to_string()).with_status_code(st),
                    );
                }
                Script::Bare(_) => drop(req),
            }
        }
    });
    // gRPC
    let (tx, rx) = std::sync::mpsc::channel();
    let s = scripted.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async move {
            let incoming =
                tonic::transport::server::TcpIncoming::bind("127.0.0.1:0".parse().expect("a"))
                    .expect("bind");
            let _ = tx.send(incoming.local_addr().expect("addr"));
            let _ = tonic::transport::Server::builder()
                .add_service(
                    ieo_proto::v0::supervisor_service_server::SupervisorServiceServer::new(s),
                )
                .serve_with_incoming(incoming)
                .await;
        });
    });
    let grpc_url = format!("http://{}", rx.recv().expect("grpc"));
    let dir = std::env::temp_dir().join(format!("ieo-central-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let token = dir.join("token");
    std::fs::write(&token, "sup-token\n").expect("token");
    let central = |url: &str, transport: &str| {
        Central::from_json(&json!({"url": url, "token_file": token, "transport": transport}))
            .expect("config")
    };
    Fakes {
        http: central(&http_url, "http"),
        grpc: central(&grpc_url, "grpc"),
        scripted,
        _dir: Dir(dir),
    }
}

impl Fakes {
    fn script(&self, s: Script) {
        self.scripted.next.lock().expect("s").push_back(s);
    }
    fn calls(&self) -> usize {
        self.scripted.calls.load(Ordering::SeqCst)
    }
}

/// What the staged-copy rule (serve.rs) reads from a refusal: refused by the coordinator (a
/// 4xx status) or not.
fn decided(r: &Refusal) -> Option<u64> {
    r.body["coordinator_status"].as_u64()
}

const RAW: &[u8] = br#"{"format":"ieo-checkpoint/v0","seq":1}"#;

#[test]
fn answers_are_classified_alike_over_both_transports() {
    let f = fakes("classify");
    let stored = json!({"stored": true, "seq": 1, "checkpoint_sha256": "ab",
                        "tamper_recorded": false, "current_assignment": true});
    let dup = json!({"duplicate": true, "seq": 1, "checkpoint_sha256": "ab",
                     "tamper_recorded": false});
    let gap = json!({"rejections": [{"code": "CHECKPOINT_SEQUENCE_GAP", "field": "seq",
        "message": "m", "correction": "c", "retryable_after_edit": true}]});
    let internal = json!({"code": "INTERNAL", "message": "database error"});
    for (script, want) in [
        (Script::Answer(201, stored.clone()), Ok(stored.clone())),
        (Script::Answer(200, dup.clone()), Ok(dup.clone())),
        (Script::Answer(409, gap.clone()), Err(Some(409))),
        (
            Script::Answer(403, json!({"code": "ROLE_MISMATCH", "message": "m"})),
            Err(Some(403)),
        ),
        (Script::Answer(500, internal.clone()), Err(Some(500))),
    ] {
        for c in [&f.http, &f.grpc] {
            f.script(script.clone());
            let r = c.report("repo-a", "development", 1, RAW);
            match (&r, &want) {
                (Ok(v), Ok(w)) => assert_eq!(v, w),
                (Err(e), Err(w)) => {
                    assert_eq!(decided(e), *w, "{e:?}");
                    assert_eq!(e.body["code"], "EXTERNAL_RECONCILIATION_REQUIRED");
                }
                _ => panic!("{r:?} vs {want:?}"),
            }
        }
    }
    // the refusal body is the coordinator's answer, unchanged, on both transports
    f.script(Script::Answer(409, gap.clone()));
    let h = f
        .http
        .report("repo-a", "development", 1, RAW)
        .expect_err("h");
    f.script(Script::Answer(409, gap.clone()));
    let g = f
        .grpc
        .report("repo-a", "development", 1, RAW)
        .expect_err("g");
    assert_eq!(h, g);
    assert_eq!(g.body["coordinator_answer"], gap);
}

#[test]
fn a_status_without_a_trailer_is_unknown_and_is_not_resent() {
    let f = fakes("bare");
    // OUT_OF_RANGE: tonic 0.14's message-size refusal; RESOURCE_EXHAUSTED: other libraries'
    for code in [
        Code::Unavailable,
        Code::DeadlineExceeded,
        Code::Cancelled,
        Code::ResourceExhausted,
        Code::OutOfRange,
        Code::Unknown,
        Code::Internal,
    ] {
        let before = f.calls();
        f.script(Script::Bare(code));
        let r = f
            .grpc
            .report("repo-a", "development", 1, RAW)
            .expect_err("unknown");
        assert_eq!(r.body["code"], "NOT_OBSERVED", "{code:?}: {r:?}");
        assert_eq!(decided(&r), None, "{code:?}: not a decided refusal");
        assert_eq!(f.calls(), before + 1, "{code:?}: sent once, never resent");
    }
    // HTTP: an answer that never comes (the connection is dropped) is unknown the same way
    let before = f.calls();
    f.script(Script::Bare(Code::Unavailable));
    let r = f.http.report("repo-a", "development", 1, RAW);
    let r = r.expect_err("unknown");
    // tiny_http answers a dropped request with 500: an answer, but not a decided refusal
    assert!(
        r.body["code"] == "NOT_OBSERVED" || decided(&r) == Some(500),
        "{r:?}"
    );
    assert_eq!(f.calls(), before + 1);
    // a coordinator that is not there
    let gone = Central::from_json(
        &json!({"url": "http://127.0.0.1:9", "token_file": "/nonexistent",
                                          "transport": "grpc"}),
    )
    .expect("config");
    let r = gone
        .report("repo-a", "development", 1, RAW)
        .expect_err("no answer");
    assert_eq!(r.body["code"], "NOT_OBSERVED", "{r:?}");
    // latest: a bare status is an error (NOT_OBSERVED upstream), NO_CHECKPOINT is none
    f.script(Script::Bare(Code::Unavailable));
    assert!(f.grpc.latest("repo-a", "development").is_err());
    f.script(Script::Answer(
        404,
        json!({"code": "NO_CHECKPOINT", "message": "none"}),
    ));
    assert_eq!(f.grpc.latest("repo-a", "development"), Ok(None));
    let rec = json!({"repo_id": "repo-a", "role": "development", "seq": 3,
        "checkpoint_sha256": "ab", "phase": "after_run", "session_id": "s", "assignment_id": "A",
        "generation": 1, "sprint_id": "P", "sprint_revision": 5, "spec_digest": "d",
        "target_sha": "t", "settings_digest": "e", "reported_by": "x",
        "received_at": "2026-11-15T00:00:00Z", "tamper_events": 0, "current_assignment": true,
        "tamper_recorded": false});
    f.script(Script::Answer(200, rec.clone()));
    assert_eq!(f.grpc.latest("repo-a", "development"), Ok(Some(rec)));
}

#[test]
fn the_size_pre_check_refuses_locally_like_the_http_400() {
    let f = fakes("size");
    // a checkpoint whose HTTP body (compact JSON, escaped) exceeds 1 MiB although the file
    // itself does not
    let quotes = format!(
        r#"{{"format":"ieo-checkpoint/v0","seq":1,"pad":"{}"}}"#,
        "\\\"".repeat(300_000)
    );
    assert!(quotes.len() < ieo_proto::MAX_BODY);
    assert!(ieo_proto::checkpoint_request_body(1, quotes.as_bytes()).len() > ieo_proto::MAX_BODY);
    let before = f.calls();
    let r = f
        .grpc
        .report("repo-a", "development", 1, quotes.as_bytes())
        .expect_err("refused");
    assert_eq!(f.calls(), before, "nothing was sent");
    assert_eq!(decided(&r), Some(400), "a decided refusal: {r:?}");
    assert_eq!(
        r.body["coordinator_answer"],
        ieo_proto::body_too_large_json()
    );
    // the same file over HTTP: the coordinator's front end answers the same 400 (scripted here
    // with the front end's body)
    f.script(Script::Answer(400, ieo_proto::body_too_large_json()));
    let h = f
        .http
        .report("repo-a", "development", 1, quotes.as_bytes())
        .expect_err("refused");
    assert_eq!(h, r, "the same refusal");
    // a path value over the handler's limit: refused locally too
    let long = "r".repeat(ieo_proto::MAX_REPO_ID + 1);
    let r = f
        .grpc
        .report(&long, "development", 1, RAW)
        .expect_err("refused");
    assert_eq!(decided(&r), Some(400), "{r:?}");
    assert_eq!(f.calls(), before + 1, "only the HTTP call went out");
}

#[test]
fn the_transport_is_http_unless_configured() {
    let c = |v: Value| Central::from_json(&v);
    let base = json!({"url": "http://x", "token_file": "/t"});
    let mut grpc = base.clone();
    grpc["transport"] = json!("grpc");
    let mut http = base.clone();
    http["transport"] = json!("http");
    assert_eq!(c(base.clone()), c(http));
    assert_ne!(c(base.clone()), c(grpc));
    let mut bad = base;
    bad["transport"] = json!("quic");
    assert!(c(bad).is_err());
    assert_eq!(c(json!("local_only")).map(|c| c.mode()), Ok("local_only"));
}
