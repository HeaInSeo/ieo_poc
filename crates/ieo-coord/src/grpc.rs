//! gRPC adapter for `ieo.internal.v0.SupervisorService` (X-04; contracts/ieo-internal-v0.md).
//!
//! The adapter adds a transport, never a rule. Each RPC builds the same JSON body the HTTP
//! operation takes, names its [`Operation`] directly and runs it through [`api::dispatch`] — the
//! function the HTTP front end runs. What the adapter does itself, in the HTTP front end's order
//! (§7.0):
//!
//! 1. a worker with a database connection (re-opened when closed; none → `UNAVAILABLE` with
//!    the HTTP 500 answer in the trailer, §7.2);
//! 2. the 1 MiB limit on the HTTP-equivalent body, before authentication (§7.1);
//! 3. the bearer credential from metadata `authorization`, resolved by the same
//!    [`crate::auth::Credentials`] (401);
//! 4. path values HTTP cannot carry (404) and checkpoint bytes that are not UTF-8 (400);
//! 5. the operation, with a [`TxGuard`] for the deadline and cancellation (§6).
//!
//! Synchronous handlers run on a pool of worker threads that own one PostgreSQL connection
//! each, like the HTTP workers. A call's future waits for its job; when the client goes away
//! (RST_STREAM drops the future) a flag tells the job, and the transaction rolls back at its
//! pre-commit check.

use crate::api::{self, Abort, Operation, Req, Resp, TxGuard};
use crate::http::{App, body_too_large, db_unavailable};
use ieo_proto::v0::supervisor_service_server::{SupervisorService, SupervisorServiceServer};
use ieo_proto::v0::{
    GetLatestCheckpointRequest, GetLatestCheckpointResponse, GetReproRequestRequest,
    GetReproRequestResponse, ReportCheckpointRequest, ReportCheckpointResponse,
    SubmitAgentReportRequest, SubmitAgentReportResponse, SubmitReproRequestRequest,
    SubmitReproRequestResponse,
};
use ieo_proto::{grpc_code, status_with_error};
use postgres::Client;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use tonic::{Code, Request, Response, Status};

/// Opens a database connection for a worker (the binary: the configured URL; tests: a switch).
pub type Connect = Arc<dyn Fn() -> Result<Client, String> + Send + Sync>;

/// Server limits (contracts/ieo-internal-v0.md §6, §7.3).
#[derive(Debug, Clone)]
pub struct GrpcConfig {
    /// Worker threads, one database connection each.
    pub workers: usize,
    /// The server's longest handling time: the deadline is min(client deadline, this).
    pub max_deadline: Duration,
    /// How often the health status re-reads `schema_meta`.
    pub health_interval: Duration,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        GrpcConfig {
            workers: 4,
            max_deadline: Duration::from_secs(15),
            health_interval: Duration::from_secs(5),
        }
    }
}

/// The transaction must be able to answer before the deadline: with less than this left
/// nothing starts, and the statement timeout and the pre-commit check end this much earlier
/// (tonic's own timeout layer drops the call at the deadline itself and answers CANCELLED).
pub const ANSWER_MARGIN: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Worker pool
// ---------------------------------------------------------------------------

type Job = Box<dyn FnOnce(Option<&mut Client>) + Send>;

struct Pool {
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
}

impl Pool {
    /// Opens every worker's connection first (a server that cannot reach its database does
    /// not start, as on HTTP).
    fn start(connect: &Connect, workers: usize) -> Result<Pool, String> {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..workers.max(1) {
            let mut db = Some(connect().map_err(|e| format!("db connect: {e}"))?);
            let rx = Arc::clone(&rx);
            let connect = Arc::clone(connect);
            std::thread::spawn(move || {
                loop {
                    let job = {
                        let r = rx.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        match r.recv() {
                            Ok(j) => j,
                            Err(_) => return,
                        }
                    };
                    // a dropped connection is re-opened; until then calls get UNAVAILABLE
                    if db.as_ref().is_none_or(Client::is_closed) {
                        db = match connect() {
                            Ok(c) => Some(c),
                            Err(e) => {
                                eprintln!("ieo-coord grpc: database reconnect failed: {e}");
                                None
                            }
                        };
                    }
                    job(db.as_mut());
                }
            });
        }
        Ok(Pool {
            jobs: Mutex::new(Some(tx)),
        })
    }

    /// Run `f` on a worker. `None`: the pool is stopped.
    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(Option<&mut Client>) -> T + Send + 'static,
    ) -> Option<T> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job: Job = Box::new(move |db| {
            let _ = tx.send(f(db));
        });
        self.jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()?
            .send(job)
            .ok()?;
        rx.await.ok()
    }

    fn stop(&self) {
        self.jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

// ---------------------------------------------------------------------------
// Deadline and cancellation (§6)
// ---------------------------------------------------------------------------

/// The guard of one call: a work deadline ([`ANSWER_MARGIN`] before the call's deadline) and
/// the cancellation flag its future sets when dropped.
struct CallGuard {
    work_deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

impl TxGuard for CallGuard {
    fn before_begin(&self) -> Result<(), Abort> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Abort::Cancelled);
        }
        if Instant::now() >= self.work_deadline {
            return Err(Abort::DeadlineExceeded);
        }
        Ok(())
    }

    fn statement_timeout(&self) -> Option<Duration> {
        Some(self.work_deadline.saturating_duration_since(Instant::now()))
    }

    fn before_commit(&self) -> Result<(), Abort> {
        self.before_begin()
    }

    fn query_canceled(&self) -> Option<Abort> {
        Some(if self.cancelled.load(Ordering::SeqCst) {
            Abort::Cancelled
        } else {
            Abort::DeadlineExceeded
        })
    }
}

/// Sets the flag when the call's future goes away (finished, or dropped on RST_STREAM or by
/// tonic's timeout layer).
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// The adapter
// ---------------------------------------------------------------------------

/// One RPC as the HTTP operation it stands for.
enum Rpc {
    SubmitReport {
        report: Vec<u8>,
    },
    SubmitRepro {
        body: Vec<u8>,
    },
    GetRepro {
        id: String,
    },
    ReportCheckpoint {
        repo: String,
        role: &'static str,
        seq: u64,
        raw: Vec<u8>,
    },
    LatestCheckpoint {
        repo: String,
        role: &'static str,
    },
}

impl Rpc {
    fn operation(&self) -> Operation<'_> {
        match self {
            Rpc::SubmitReport { .. } => Operation::SubmitReport,
            Rpc::SubmitRepro { .. } => Operation::SubmitRepro,
            Rpc::GetRepro { id } => Operation::GetRepro { id },
            Rpc::ReportCheckpoint { repo, role, .. } => Operation::ReportCheckpoint { repo, role },
            Rpc::LatestCheckpoint { repo, role } => Operation::LatestCheckpoint { repo, role },
        }
    }

    fn method(&self) -> &'static str {
        match self {
            Rpc::GetRepro { .. } | Rpc::LatestCheckpoint { .. } => "GET",
            _ => "POST",
        }
    }

    /// The HTTP path (for the request record only: the operation is chosen directly).
    fn path(&self) -> String {
        match self {
            Rpc::SubmitReport { .. } => "/v0/agent/reports".into(),
            Rpc::SubmitRepro { .. } => "/v0/agent/repro-requests".into(),
            Rpc::GetRepro { id } => format!("/v0/agent/repro-requests/{id}"),
            Rpc::ReportCheckpoint { repo, role, .. } => {
                format!("/v0/repos/{repo}/roles/{role}/checkpoints")
            }
            Rpc::LatestCheckpoint { repo, role } => {
                format!("/v0/repos/{repo}/roles/{role}/checkpoints/latest")
            }
        }
    }

    /// The HTTP-equivalent JSON body (§2, §7.1).
    fn body(&self) -> Vec<u8> {
        match self {
            Rpc::SubmitReport { report } => report.clone(),
            Rpc::SubmitRepro { body } => body.clone(),
            Rpc::ReportCheckpoint { seq, raw, .. } => ieo_proto::checkpoint_request_body(*seq, raw),
            Rpc::GetRepro { .. } | Rpc::LatestCheckpoint { .. } => Vec::new(),
        }
    }

    /// Path values HTTP cannot carry as the same operation (§3, X-03 review P-1): `/`, `?` or
    /// `#` anywhere, and an empty `request_id` (the last path segment; HTTP trims it away). An
    /// empty `repo_id` is an inner segment and reaches the handler, as on HTTP.
    fn path_values_ok(&self) -> bool {
        let clean = |v: &str| !v.contains(['/', '?', '#']);
        match self {
            Rpc::GetRepro { id } => !id.is_empty() && clean(id),
            Rpc::ReportCheckpoint { repo, .. } | Rpc::LatestCheckpoint { repo, .. } => clean(repo),
            Rpc::SubmitReport { .. } | Rpc::SubmitRepro { .. } => true,
        }
    }
}

enum Answer {
    /// The answer of the operation or of the adapter's own checks (HTTP status and body).
    Resp(Resp),
    /// The deadline passed or the call was cancelled before commit: rolled back.
    Aborted(Abort),
    /// No database connection: nothing ran.
    DbUnavailable,
}

/// The adapter's checks in order, then the operation (§7.0).
fn adapt(
    db: Option<&mut Client>,
    app: &App,
    bearer: Option<&str>,
    rpc: &Rpc,
    guard: &dyn TxGuard,
) -> Answer {
    let Some(db) = db else {
        return Answer::DbUnavailable;
    };
    let body = rpc.body();
    if body.len() > ieo_proto::MAX_BODY {
        return Answer::Resp(body_too_large());
    }
    let Some(who) = app.creds.authenticate(bearer) else {
        return Answer::Resp(api::unauthenticated());
    };
    if !rpc.path_values_ok() {
        return Answer::Resp(api::no_such_endpoint());
    }
    if let Rpc::ReportCheckpoint { raw, .. } = rpc
        && std::str::from_utf8(raw).is_err()
    {
        return Answer::Resp(api::rejection(
            400,
            "SCHEMA_VIOLATION",
            "checkpoint_json",
            "checkpoint_json is not UTF-8",
            "report the checkpoint file as written",
        ));
    }
    let path = rpc.path();
    let req = Req {
        method: rpc.method(),
        path: &path,
        query: "",
        idempotency_key: None,
        body: &body,
    };
    match api::dispatch(db, &app.ctx, Some(who), Some(rpc.operation()), &req, guard) {
        Ok(r) => Answer::Resp(r),
        Err(a) => Answer::Aborted(a),
    }
}

struct Coordinator {
    app: Arc<App>,
    pool: Arc<Pool>,
    max_deadline: Duration,
}

impl Coordinator {
    /// Run one RPC; `Ok` is a 2xx answer, anything else the status of §4.1.
    async fn execute<T>(&self, request: &Request<T>, rpc: Rpc) -> Result<Resp, Status> {
        let started = Instant::now();
        let md = request.metadata();
        let budget = md
            .get("grpc-timeout")
            .and_then(|v| v.to_str().ok())
            .and_then(ieo_proto::parse_grpc_timeout)
            .map_or(self.max_deadline, |d| d.min(self.max_deadline));
        let bearer = md
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let cancelled = Arc::new(AtomicBool::new(false));
        let _on_drop = CancelOnDrop(Arc::clone(&cancelled));
        let guard = CallGuard {
            work_deadline: (started + budget)
                .checked_sub(ANSWER_MARGIN)
                .unwrap_or(started),
            cancelled,
        };
        let app = Arc::clone(&self.app);
        let answer = self
            .pool
            .run(move |db| adapt(db, &app, bearer.as_deref(), &rpc, &guard))
            .await;
        match answer {
            Some(Answer::Resp(r)) if (200..300).contains(&r.status) => Ok(r),
            Some(Answer::Resp(r)) => Err(status_with_error(
                grpc_code(r.status, &r.body),
                r.status,
                &r.body,
            )),
            Some(Answer::DbUnavailable) => {
                let r = db_unavailable();
                Err(status_with_error(Code::Unavailable, r.status, &r.body))
            }
            Some(Answer::Aborted(Abort::DeadlineExceeded)) => Err(Status::deadline_exceeded(
                "deadline reached before commit: rolled back",
            )),
            Some(Answer::Aborted(Abort::Cancelled)) => Err(Status::cancelled(
                "call cancelled before commit: rolled back",
            )),
            None => Err(Status::unavailable("coordinator is stopping")),
        }
    }
}

#[tonic::async_trait]
impl SupervisorService for Coordinator {
    async fn submit_agent_report(
        &self,
        request: Request<SubmitAgentReportRequest>,
    ) -> Result<Response<SubmitAgentReportResponse>, Status> {
        let report = request.get_ref().report_json.clone();
        let r = self.execute(&request, Rpc::SubmitReport { report }).await?;
        Ok(Response::new(SubmitAgentReportResponse {
            outcome: ieo_proto::outcome_of(r.status) as i32,
            receipt: Some(ieo_proto::receipt_from_json(&r.body)),
        }))
    }

    async fn submit_repro_request(
        &self,
        request: Request<SubmitReproRequestRequest>,
    ) -> Result<Response<SubmitReproRequestResponse>, Status> {
        let body = serde_json::to_vec(&ieo_proto::repro_request_json(request.get_ref()))
            .unwrap_or_default();
        let r = self.execute(&request, Rpc::SubmitRepro { body }).await?;
        Ok(Response::new(SubmitReproRequestResponse {
            outcome: ieo_proto::outcome_of(r.status) as i32,
            status: Some(ieo_proto::repro_from_json(&r.body)),
        }))
    }

    async fn get_repro_request(
        &self,
        request: Request<GetReproRequestRequest>,
    ) -> Result<Response<GetReproRequestResponse>, Status> {
        let id = request.get_ref().request_id.clone();
        let r = self.execute(&request, Rpc::GetRepro { id }).await?;
        Ok(Response::new(GetReproRequestResponse {
            status: Some(ieo_proto::repro_from_json(&r.body)),
        }))
    }

    async fn report_checkpoint(
        &self,
        request: Request<ReportCheckpointRequest>,
    ) -> Result<Response<ReportCheckpointResponse>, Status> {
        let m = request.get_ref();
        let rpc = Rpc::ReportCheckpoint {
            repo: m.repo_id.clone(),
            role: ieo_proto::role_name(m.role),
            seq: m.seq,
            raw: m.checkpoint_json.clone(),
        };
        let r = self.execute(&request, rpc).await?;
        Ok(Response::new(ieo_proto::checkpoint_answer_from_json(
            r.status, &r.body,
        )))
    }

    async fn get_latest_checkpoint(
        &self,
        request: Request<GetLatestCheckpointRequest>,
    ) -> Result<Response<GetLatestCheckpointResponse>, Status> {
        let m = request.get_ref();
        let rpc = Rpc::LatestCheckpoint {
            repo: m.repo_id.clone(),
            role: ieo_proto::role_name(m.role),
        };
        let r = self.execute(&request, rpc).await?;
        Ok(Response::new(ieo_proto::latest_from_json(&r.body)))
    }
}

// ---------------------------------------------------------------------------
// Health (§7.3) and the server
// ---------------------------------------------------------------------------

/// The same read as HTTP `/v0/health` (`schema_meta`), on a worker. `true`: SERVING.
async fn db_healthy(pool: &Pool, app: &Arc<App>, limit: Duration) -> bool {
    let app = Arc::clone(app);
    let check = pool.run(move |db| {
        let db = db?;
        let req = Req {
            method: "GET",
            path: "/v0/health",
            query: "",
            idempotency_key: None,
            body: &[],
        };
        api::dispatch(
            db,
            &app.ctx,
            None,
            Some(Operation::Health),
            &req,
            &api::NoGuard,
        )
        .ok()
    });
    matches!(
        tokio::time::timeout(limit, check).await,
        Ok(Some(Some(Resp { status: 200, .. })))
    )
}

/// A running gRPC server. Dropping it (or [`GrpcServer::stop`]) shuts it down.
pub struct GrpcServer {
    pub addr: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GrpcServer {
    /// Bind `listen` and serve `SupervisorService` and `grpc.health.v1.Health` on a thread of
    /// its own (a tokio runtime; the handlers run on the worker pool).
    pub fn start(
        listen: &str,
        app: Arc<App>,
        connect: Connect,
        cfg: GrpcConfig,
    ) -> Result<GrpcServer, String> {
        let addr: SocketAddr = listen
            .parse()
            .map_err(|e| format!("--grpc-listen {listen}: {e}"))?;
        let pool = Arc::new(Pool::start(&connect, cfg.workers)?);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| format!("grpc runtime: {e}"))?;
        let incoming = rt
            .block_on(async { tonic::transport::server::TcpIncoming::bind(addr) })
            .map_err(|e| format!("grpc listen {listen}: {e}"))?;
        let addr = incoming
            .local_addr()
            .map_err(|e| format!("grpc listen {listen}: {e}"))?;
        let (reporter, health) = tonic_health::server::health_reporter();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let svc = Coordinator {
            app: Arc::clone(&app),
            pool: Arc::clone(&pool),
            max_deadline: cfg.max_deadline,
        };
        let thread = std::thread::spawn(move || {
            rt.block_on(async move {
                // the first status is read before serving; then every interval
                let names = ["", ieo_proto::SERVICE_NAME];
                let interval = cfg.health_interval;
                let limit = interval.max(Duration::from_secs(1));
                let ok = db_healthy(&pool, &app, limit).await;
                set_health(&reporter, &names, ok).await;
                let (hp, ha) = (Arc::clone(&pool), Arc::clone(&app));
                let checker = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(interval).await;
                        let ok = db_healthy(&hp, &ha, limit).await;
                        set_health(&reporter, &names, ok).await;
                    }
                });
                let server = SupervisorServiceServer::new(svc)
                    .max_decoding_message_size(ieo_proto::MAX_MESSAGE)
                    .max_encoding_message_size(ieo_proto::MAX_MESSAGE);
                let r = tonic::transport::Server::builder()
                    .timeout(cfg.max_deadline)
                    .add_service(health)
                    .add_service(server)
                    .serve_with_incoming_shutdown(incoming, async {
                        let _ = rx.await;
                    })
                    .await;
                if let Err(e) = r {
                    eprintln!("ieo-coord grpc: {e}");
                }
                checker.abort();
                pool.stop();
            });
        });
        Ok(GrpcServer {
            addr,
            shutdown: Some(tx),
            thread: Some(thread),
        })
    }

    /// Stop accepting, let the calls in progress finish, and wait.
    pub fn stop(mut self) {
        self.shut();
    }

    fn shut(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for GrpcServer {
    fn drop(&mut self) {
        self.shut();
    }
}

async fn set_health(r: &tonic_health::server::HealthReporter, names: &[&str], ok: bool) {
    let st = if ok {
        tonic_health::ServingStatus::Serving
    } else {
        tonic_health::ServingStatus::NotServing
    };
    for n in names {
        r.set_service_status(*n, st).await;
    }
}
