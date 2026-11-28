//! Request handlers. One database transaction per request: a 2xx answer commits, any
//! other answer rolls back, so a refused request leaves no row behind.
//!
//! Check order follows the 01-08 contract: 401 → 403 → (agent paths: same-ID replay →
//! assignment class) → 400 format → 409 identity → 422 reference/meaning.

use crate::auth::{Caller, Kind};
use crate::policy::Policy;
use ieo_report::{
    AssignmentBook, ClaimDisposition, CompletionFacts, Denied, IssuedAssignment, ReproDisposition,
    ReviewCurrency, authorize_claim, authorize_repro, completion_summary, review_currency,
    validate_report,
};
use ieo_spec::{
    CheckMode, CheckOutcome, CheckState, ConversionContext, Registry, canonical_json,
    check_registration, convert_human_input,
};
use postgres::{Client, GenericClient, Transaction};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Static configuration loaded at start-up.
pub struct Ctx {
    /// Connection settings for committing a one-shot recovery use while delivery holds its fence.
    pub db_config: postgres::Config,
    pub policy: Policy,
    /// Plan manifest; only its `inspection_profiles` are read (approved acceptance profiles).
    pub manifest: Value,
    /// Approved per-repo bindings for human-input conversion (01-07 §3), keyed by repo_id.
    pub bindings: Value,
    /// Role supervisors per repo (`--roles`, runtime contract §7), for repo-scope status/stop.
    pub roles: Value,
    pub github_writes: Option<crate::github_write::Config>,
}

pub struct Req<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub idempotency_key: Option<&'a str>,
    pub body: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resp {
    pub status: u16,
    pub body: Value,
}

impl From<postgres::Error> for Resp {
    fn from(e: postgres::Error) -> Self {
        if e.code() == Some(&postgres::error::SqlState::QUERY_CANCELED) {
            QUERY_CANCELED.with(|c| c.set(true));
            return Resp {
                status: 500,
                body: json!({"code": "INTERNAL",
                             "message": "database error: statement timeout or cancellation (SQLSTATE 57014); this step's transaction was rolled back"}),
            };
        }
        // The database message never contains credentials (the URL is not part of it).
        Resp {
            status: 500,
            body: json!({"code": "INTERNAL", "message": format!("database error: {e}")}),
        }
    }
}

pub(crate) type R = Result<Resp, Resp>;

pub(crate) fn ok(status: u16, body: Value) -> R {
    Ok(Resp { status, body })
}

pub(crate) fn problem(status: u16, code: &str, message: &str) -> Resp {
    Resp {
        status,
        body: json!({"code": code, "message": message}),
    }
}

fn rej(code: &str, field: &str, message: &str, correction: &str) -> Value {
    json!({"code": code, "field": field, "message": message, "correction": correction,
           "retryable_after_edit": true})
}

fn rejected(status: u16, rejections: Vec<Value>) -> Resp {
    Resp {
        status,
        body: json!({ "rejections": rejections }),
    }
}

pub(crate) fn one(status: u16, code: &str, field: &str, message: &str, correction: &str) -> Resp {
    rejected(status, vec![rej(code, field, message, correction)])
}

fn denied(d: Denied) -> Resp {
    let code = serde_json::to_value(d)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let status = d.http_status();
    if status == 403 {
        problem(403, &code, "credential lacks the repo, role or report type")
    } else {
        one(
            status,
            &code,
            "",
            "assignment or event identity check failed",
            "re-read the current assignment and resubmit with its identity",
        )
    }
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub(crate) fn is_sha(s: &str) -> bool {
    (40..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn parse_body(body: &[u8]) -> Result<Value, Resp> {
    match serde_json::from_slice::<Value>(body) {
        Ok(v @ Value::Object(_)) => Ok(v),
        _ => Err(one(
            400,
            "SCHEMA_VIOLATION",
            "",
            "request body is not a JSON object",
            "send a JSON object",
        )),
    }
}

pub(crate) fn str_field<'a>(v: &'a Value, k: &str, max: usize) -> Result<&'a str, Resp> {
    match v[k].as_str() {
        Some(s) if !s.is_empty() && s.len() <= max => Ok(s),
        _ => Err(one(
            400,
            "MISSING_FIELD",
            k,
            &format!("`{k}` must be a non-empty string of at most {max} bytes"),
            &format!("supply `{k}`"),
        )),
    }
}

fn u64_field(v: &Value, k: &str) -> Result<u64, Resp> {
    v[k].as_u64().ok_or_else(|| {
        one(
            400,
            "MISSING_FIELD",
            k,
            &format!("`{k}` must be a non-negative integer"),
            &format!("supply `{k}`"),
        )
    })
}

pub(crate) fn only_keys(v: &Value, allowed: &[&str]) -> Result<(), Resp> {
    let extra: Vec<&String> = v
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys())
        .filter(|k| !allowed.contains(&k.as_str()))
        .collect();
    match extra.first() {
        None => Ok(()),
        Some(k) => Err(one(
            400,
            "SCHEMA_VIOLATION",
            k,
            "unknown field",
            "remove the field",
        )),
    }
}

pub(crate) fn i64_of(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Transaction-scoped advisory lock (released at commit/rollback).
pub(crate) fn lock(tx: &mut Transaction, key: &str) -> Result<(), Resp> {
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        &[&key],
    )?;
    Ok(())
}

pub(crate) fn history(
    tx: &mut Transaction,
    actor: &str,
    action: &str,
    subject: &Value,
) -> Result<(), Resp> {
    tx.execute(
        "INSERT INTO history (actor, action, subject) VALUES ($1, $2, $3)",
        &[&actor, &action, subject],
    )?;
    Ok(())
}

fn now(tx: &mut Transaction) -> Result<String, Resp> {
    Ok(tx.query_one("SELECT iso(now())", &[])?.get(0))
}

pub struct NewEvent<'a> {
    pub sprint_id: &'a str,
    pub scope_repo: &'a str,
    pub scope_role: &'a str,
    pub report: &'a Value,
    pub disposition: Option<&'a str>,
    pub assignment: Option<(&'a str, u64)>,
    pub attributed_sha: Option<&'a str>,
    pub policy_digest: Option<&'a str>,
}

/// Append an event. The per-sprint lock is held until commit, so sequence numbers become
/// visible in order (a cursor never skips an event committed later with a lower number).
fn insert_event(tx: &mut Transaction, e: &NewEvent) -> Result<i64, Resp> {
    lock(tx, &format!("sprint:{}", e.sprint_id))?;
    let seq: i64 = tx
        .query_one(
            "SELECT coalesce(max(sprint_seq), 0) + 1 FROM events WHERE sprint_id = $1",
            &[&e.sprint_id],
        )?
        .get(0);
    let (aid, generation) = match e.assignment {
        Some((a, g)) => (Some(a), Some(i64_of(g))),
        None => (None, None),
    };
    tx.execute(
        "INSERT INTO events (sprint_id, sprint_seq, scope_repo, scope_role, event_id, event_revision,
            task_id, report_type, evidence_level, disposition, assignment_id, generation,
            attributed_sha, policy_digest, report)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        &[
            &e.sprint_id,
            &seq,
            &e.scope_repo,
            &e.scope_role,
            &e.report["event_id"].as_str().unwrap_or_default(),
            &e.report["event_revision"].as_i64().unwrap_or(1),
            &e.report["task_id"].as_str(),
            &e.report["report_type"].as_str().unwrap_or_default(),
            &e.report["evidence_level"].as_str().unwrap_or_default(),
            &e.disposition,
            &aid,
            &generation,
            &e.attributed_sha,
            &e.policy_digest,
            e.report,
        ],
    )?;
    Ok(seq)
}

// ---------------------------------------------------------------------------
// Operation table (X-04): one route per operation, shared by the HTTP and gRPC adapters
// ---------------------------------------------------------------------------

/// Every operation the coordinator serves, with its path parameters. The HTTP adapter finds it
/// with [`route`]; the gRPC adapter names it directly (contracts/ieo-internal-v0.md §9). Both
/// run it through [`dispatch`], so the business rules live in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation<'a> {
    Health,
    RoleStatus {
        repo: &'a str,
    },
    RoleStop {
        repo: &'a str,
    },
    Register,
    Convert,
    ImportNotion,
    RecordException,
    GetRevision {
        id: &'a str,
        rev: &'a str,
    },
    SemanticReview {
        id: &'a str,
        rev: &'a str,
    },
    SprintState {
        id: &'a str,
    },
    ListEvents {
        id: &'a str,
    },
    SubmitReport,
    SubmitRepro,
    GetRepro {
        id: &'a str,
        receipt: bool,
    },
    ReproLog {
        key: &'a str,
        stream: &'a str,
    },
    Assign,
    ReportCheckpoint {
        repo: &'a str,
        role: &'a str,
    },
    LatestCheckpoint {
        repo: &'a str,
        role: &'a str,
    },
    CompareCheckpoint {
        repo: &'a str,
        role: &'a str,
    },
    /// 02-05 step 2: the supervisor's own assignment, first action and session status.
    OwnAssignment {
        repo: &'a str,
        role: &'a str,
    },
    RecordFirstAction {
        repo: &'a str,
        role: &'a str,
    },
    ReportSessionStatus {
        repo: &'a str,
        role: &'a str,
    },
    /// 02-05 step 3: deliver the current assignment to the role supervisor (HTTP only).
    DeliverAssignment {
        repo: &'a str,
        role: &'a str,
    },
    /// 02-05 step 7b: a person approves a recovery; read one (HTTP only).
    ApproveRecovery {
        repo: &'a str,
        role: &'a str,
    },
    GetRecovery {
        repo: &'a str,
        role: &'a str,
        id: &'a str,
    },
    /// 02-05 step 4: the connection of one assignment generation (HTTP only, read only).
    AssignmentChain {
        assignment_id: &'a str,
        generation: &'a str,
    },
    Ingest,
    ReviewTransition,
    GithubPush,
    GithubMerge,
    GithubReconcile,
    Judge,
    PlanFollowup,
    ReconcileFollowups,
    ListOutbox,
    Deliver {
        id: &'a str,
    },
}

/// How an operation runs (contracts/ieo-internal-v0.md §9, "작업마다 다른 실행 모양").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// No credential, outside a transaction (liveness).
    Unauthenticated,
    /// Authenticated, no database (repo-scope role control).
    NoDatabase,
    /// Authorization, then a call outside the transaction, then one transaction (`compare`).
    CallThenTransaction,
    /// One transaction: a 2xx answer commits, anything else rolls back.
    Transaction,
    /// Authorization, the request recorded (transaction 1), the judgment, the budget check and
    /// the supervisor call outside any transaction, the result recorded (transaction 2)
    /// (deliverAssignment, 02-05 step 3).
    Delivery,
}

impl Operation<'_> {
    pub fn shape(&self) -> Shape {
        match self {
            Operation::Health => Shape::Unauthenticated,
            Operation::RoleStatus { .. } => Shape::NoDatabase,
            // the stops first, then one read transaction (02-05 step 6)
            Operation::RoleStop { .. } => Shape::CallThenTransaction,
            Operation::CompareCheckpoint { .. } => Shape::CallThenTransaction,
            Operation::DeliverAssignment { .. } => Shape::Delivery,
            // the supervisor's status first, then one transaction (02-05 step 7b)
            Operation::ApproveRecovery { .. } => Shape::CallThenTransaction,
            _ => Shape::Transaction,
        }
    }
}

/// The operation an HTTP method and path name (`None`: no such endpoint).
pub fn route<'a>(method: &str, path: &'a str) -> Option<Operation<'a>> {
    let segs: Vec<&'a str> = path.trim_matches('/').split('/').collect();
    let op = match (method, segs.as_slice()) {
        ("GET", ["v0", "health"]) => Operation::Health,
        ("GET", ["v0", "repos", repo, "roles"]) => Operation::RoleStatus { repo },
        ("POST", ["v0", "repos", repo, "stop"]) => Operation::RoleStop { repo },
        ("POST", ["v0", "sprints"]) => Operation::Register,
        ("POST", ["v0", "sprint-inputs"]) => Operation::Convert,
        ("POST", ["v0", "imports", "notion"]) => Operation::ImportNotion,
        ("POST", ["v0", "coordinator", "exceptions"]) => Operation::RecordException,
        ("GET", ["v0", "sprints", id, "revisions", rev]) => Operation::GetRevision { id, rev },
        ("POST", ["v0", "sprints", id, "revisions", rev, "semantic-review"]) => {
            Operation::SemanticReview { id, rev }
        }
        ("GET", ["v0", "sprints", id, "state"]) => Operation::SprintState { id },
        ("GET", ["v0", "sprints", id, "events"]) => Operation::ListEvents { id },
        ("POST", ["v0", "agent", "reports"]) => Operation::SubmitReport,
        ("POST", ["v0", "agent", "repro-requests"]) => Operation::SubmitRepro,
        ("GET", ["v0", "agent", "repro-requests", id]) => {
            Operation::GetRepro { id, receipt: false }
        }
        ("GET", ["v0", "agent", "repro-requests", id, "receipt"]) => {
            Operation::GetRepro { id, receipt: true }
        }
        ("GET", ["v0", "agent", "repro-logs", key, stream]) => Operation::ReproLog { key, stream },
        ("POST", ["v0", "coordinator", "assignments"]) => Operation::Assign,
        // central checkpoint record (runtime contract §2.1, §5-2; 02-04 decision item 9)
        ("POST", ["v0", "repos", repo, "roles", role, "checkpoints"]) => {
            Operation::ReportCheckpoint { repo, role }
        }
        ("GET", ["v0", "repos", repo, "roles", role, "checkpoints", "latest"]) => {
            Operation::LatestCheckpoint { repo, role }
        }
        ("GET", ["v0", "repos", repo, "roles", role, "checkpoints", "compare"]) => {
            Operation::CompareCheckpoint { repo, role }
        }
        // the supervisor's own assignment and session facts (02-05 step 2, PLAN §3.1)
        ("GET", ["v0", "repos", repo, "roles", role, "assignment"]) => {
            Operation::OwnAssignment { repo, role }
        }
        ("POST", ["v0", "repos", repo, "roles", role, "first-actions"]) => {
            Operation::RecordFirstAction { repo, role }
        }
        ("POST", ["v0", "repos", repo, "roles", role, "session-status"]) => {
            Operation::ReportSessionStatus { repo, role }
        }
        // the coordinator's delivery of the current assignment (02-05 step 3)
        ("POST", ["v0", "repos", repo, "roles", role, "deliveries"]) => {
            Operation::DeliverAssignment { repo, role }
        }
        // the official recovery after a crash (02-05 step 7b)
        ("POST", ["v0", "repos", repo, "roles", role, "recoveries"]) => {
            Operation::ApproveRecovery { repo, role }
        }
        ("GET", ["v0", "repos", repo, "roles", role, "recoveries", id]) => {
            Operation::GetRecovery { repo, role, id }
        }
        // the connection of one assignment generation (02-05 step 4)
        (
            "GET",
            [
                "v0",
                "coordinator",
                "assignments",
                assignment_id,
                "generations",
                generation,
                "chain",
            ],
        ) => Operation::AssignmentChain {
            assignment_id,
            generation,
        },
        ("POST", ["v0", "coordinator", "evidence"]) => Operation::Ingest,
        ("POST", ["v0", "coordinator", "review-transitions"]) => Operation::ReviewTransition,
        ("POST", ["v0", "coordinator", "github", "push"]) => Operation::GithubPush,
        ("POST", ["v0", "coordinator", "github", "merge"]) => Operation::GithubMerge,
        ("POST", ["v0", "coordinator", "github", "reconcile"]) => Operation::GithubReconcile,
        ("POST", ["v0", "coordinator", "judgments"]) => Operation::Judge,
        ("POST", ["v0", "coordinator", "followups"]) => Operation::PlanFollowup,
        ("POST", ["v0", "coordinator", "followups", "reconcile"]) => Operation::ReconcileFollowups,
        ("GET", ["v0", "coordinator", "outbox"]) => Operation::ListOutbox,
        ("POST", ["v0", "coordinator", "outbox", id, "delivery"]) => Operation::Deliver { id },
        _ => return None,
    };
    Some(op)
}

/// Why a transport abandoned an operation's transaction before it committed (gRPC deadline or
/// cancellation, contracts/ieo-internal-v0.md §6). Never produced on the HTTP path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abort {
    DeadlineExceeded,
    Cancelled,
}

/// Transport hooks around the one transaction of an operation (contracts/ieo-internal-v0.md
/// §6.2). The HTTP path uses [`NoGuard`]: every check passes, so its behaviour is unchanged.
pub trait TxGuard {
    /// Before the transaction opens (e.g. too little time left: nothing runs).
    fn before_begin(&self) -> Result<(), Abort> {
        Ok(())
    }
    /// `SET LOCAL statement_timeout` for the transaction, if any.
    fn statement_timeout(&self) -> Option<std::time::Duration> {
        None
    }
    /// Right before commit: a passed deadline or a cancelled call rolls back.
    fn before_commit(&self) -> Result<(), Abort> {
        Ok(())
    }
    /// A statement failed with SQLSTATE 57014 (`query_canceled`): `Some` replaces the 500
    /// answer (the guard's own statement timeout fired). `None` keeps the 500.
    fn query_canceled(&self) -> Option<Abort> {
        None
    }
}

/// The HTTP path's guard: no deadline, no cancellation.
pub struct NoGuard;

impl TxGuard for NoGuard {}

thread_local! {
    /// Set when a statement of the current transaction failed with SQLSTATE 57014.
    static QUERY_CANCELED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The 401 answer for a missing or unknown bearer credential.
pub fn unauthenticated() -> Resp {
    problem(
        401,
        "UNAUTHENTICATED",
        "missing or unknown bearer credential",
    )
}

/// A one-rejection answer (`{"rejections": [...]}`), for a transport's own refusals.
pub fn rejection(status: u16, code: &str, field: &str, message: &str, correction: &str) -> Resp {
    one(status, code, field, message, correction)
}

/// The 404 answer for a path that names no operation.
pub fn no_such_endpoint() -> Resp {
    problem(404, "NOT_FOUND", "no such endpoint")
}

/// One transaction: a 2xx answer (`Ok`) commits after the guard's pre-commit check, any other
/// answer rolls back.
pub(crate) fn run_tx(
    db: &mut Client,
    guard: &dyn TxGuard,
    f: &dyn Fn(&mut Transaction) -> R,
) -> Result<Resp, Abort> {
    guard.before_begin()?;
    let mut tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return Ok(e.into()),
    };
    if let Some(limit) = guard.statement_timeout() {
        let ms = limit.as_millis().max(1);
        if let Err(e) = tx.batch_execute(&format!("SET LOCAL statement_timeout = {ms}")) {
            let _ = tx.rollback();
            return Ok(e.into());
        }
    }
    QUERY_CANCELED.with(|c| c.set(false));
    match f(&mut tx) {
        Ok(r) => {
            if let Err(a) = guard.before_commit() {
                let _ = tx.rollback();
                return Err(a);
            }
            match tx.commit() {
                Ok(()) => Ok(r),
                Err(e) => Ok(e.into()),
            }
        }
        Err(r) => {
            let _ = tx.rollback();
            if QUERY_CANCELED.with(|c| c.replace(false))
                && let Some(a) = guard.query_canceled()
            {
                return Err(a);
            }
            Ok(r)
        }
    }
}

/// Upper bound of one statement on the HTTP path, advisory-lock waits included (PR #74 review
/// fe924b7 I-5). A statement past it fails with SQLSTATE 57014 and the operation answers 500
/// with its transaction rolled back (a multi-statement transaction can take a multiple of it).
/// deliverAssignment does not rely on this sum for its 120 s in-flight window: it sends its call
/// only before its start deadline (75 s, `delivery::bounds`) and records the result right after it (review
/// 97ff0bc L-2).
pub const HTTP_STATEMENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The HTTP path's guard: no deadline and no cancellation, but every statement is bounded by
/// [`HTTP_STATEMENT_TIMEOUT`].
pub struct HttpGuard;

impl TxGuard for HttpGuard {
    fn statement_timeout(&self) -> Option<std::time::Duration> {
        Some(HTTP_STATEMENT_TIMEOUT)
    }
}

/// HTTP entry: route the path, then [`dispatch`] with no deadline and bounded statements.
pub fn handle(db: &mut Client, ctx: &Ctx, who: Option<&Caller>, req: &Req) -> Resp {
    match dispatch(db, ctx, who, route(req.method, req.path), req, &HttpGuard) {
        Ok(r) => r,
        // HttpGuard never aborts (a statement timeout is the 500 of the failed statement)
        Err(_) => problem(500, "INTERNAL", "transaction abandoned"),
    }
}

/// Liveness only: the process answers and the DB is reachable. This is not P04.
fn health(db: &mut Client, ctx: &Ctx) -> Resp {
    match db.query_one("SELECT v FROM schema_meta WHERE k = 'schema_version'", &[]) {
        Ok(row) => Resp {
            status: 200,
            body: json!({"status": "up", "schema_version": row.get::<_, String>(0),
                         "policy_digest": ctx.policy.digest,
                         "scope": "liveness only; not a P04 result"}),
        },
        Err(e) => e.into(),
    }
}

/// Run one operation: liveness without a credential, then 401, then 404 for no operation,
/// then the operation in its own execution shape. `req` carries the body, query and
/// Idempotency-Key; its path is not read again.
pub fn dispatch(
    db: &mut Client,
    ctx: &Ctx,
    who: Option<&Caller>,
    op: Option<Operation>,
    req: &Req,
    guard: &dyn TxGuard,
) -> Result<Resp, Abort> {
    if op == Some(Operation::Health) {
        return Ok(health(db, ctx));
    }
    let Some(who) = who else {
        return Ok(unauthenticated());
    };
    let Some(op) = op else {
        return Ok(no_such_endpoint());
    };
    let run = |db: &mut Client, f: &dyn Fn(&mut Transaction) -> R| run_tx(db, guard, f);
    match op {
        // repo-scope role control (§7), service and user credentials with `roles:control`:
        // RoleStatus uses no database; RoleStop sends the stops, then reads the repo's
        // deliveries of unknown outcome in one transaction (02-05 step 6)
        Operation::RoleStatus { repo } | Operation::RoleStop { repo } => {
            if let Err(r) = need_client_scope(who, "roles:control", false) {
                return Ok(r);
            }
            // an empty repo list is no repo, as everywhere else (review adf760e B1)
            if !who.in_repo(repo) {
                return Ok(problem(
                    403,
                    "SCOPE_DENIED",
                    "repo outside the credential's repos",
                ));
            }
            if matches!(op, Operation::RoleStatus { .. }) {
                let (status, body) = crate::roles::status(&ctx.roles, repo);
                return Ok(Resp { status, body });
            }
            let (status, mut body) = crate::roles::stop(
                &ctx.roles,
                repo,
                std::time::Duration::from_secs(30),
                std::time::Duration::from_millis(250),
            );
            // the coordinator's own external requests of the repo (02-05 step 6): deliveries
            // whose call's outcome is not known (no result yet, or `unknown`). The stops are
            // already sent: a failed read never replaces the stop report (review 1b362ec M-1);
            // the list is then "unreadable", a reason says why, and the stop stays unconfirmed.
            let pending = run(db, &|tx| ok(200, json!(pending_deliveries(tx, repo)?)));
            if status == 200 {
                let mut why = Vec::new();
                match &pending {
                    Ok(p) if p.status == 200 => {
                        for d in p.body.as_array().into_iter().flatten() {
                            why.push(format!(
                                "coordinator: delivery {} to {} has no known outcome ({})",
                                d["request_id"].as_str().unwrap_or_default(),
                                d["target"].as_str().unwrap_or_default(),
                                d["state"].as_str().unwrap_or_default()
                            ));
                        }
                        body["coordinator_pending_external_requests"] = p.body.clone();
                    }
                    failed => {
                        let cause = match failed {
                            Ok(p) => format!(
                                "{} {}",
                                p.status,
                                p.body["message"].as_str().unwrap_or_default()
                            ),
                            Err(a) => format!("{a:?}"),
                        };
                        why.push(format!(
                            "coordinator: pending deliveries could not be read ({cause})"
                        ));
                        body["coordinator_pending_external_requests"] = json!("unreadable");
                    }
                }
                if let Some(u) = body["unconfirmed"].as_array_mut() {
                    u.extend(why.into_iter().map(Value::from));
                }
                body["safe_shutdown_confirmed"] =
                    json!(body["unconfirmed"].as_array().is_some_and(Vec::is_empty));
            }
            Ok(Resp { status, body })
        }
        Operation::Health => Ok(health(db, ctx)),
        Operation::Register => run(db, &|tx| register(tx, ctx, who, req)),
        Operation::Convert => run(db, &|tx| convert(tx, ctx, who, req)),
        Operation::ImportNotion => run(db, &|tx| import_notion(tx, ctx, who, req)),
        Operation::RecordException => run(db, &|tx| record_exception(tx, who, req)),
        Operation::GetRevision { id, rev } => run(db, &|tx| get_revision(tx, who, id, rev)),
        Operation::SemanticReview { id, rev } => {
            run(db, &|tx| semantic_review(tx, who, id, rev, req))
        }
        Operation::SprintState { id } => run(db, &|tx| sprint_state(tx, who, id)),
        Operation::ListEvents { id } => run(db, &|tx| list_events(tx, who, id, req)),
        Operation::SubmitReport => run(db, &|tx| submit_report(tx, who, req)),
        Operation::SubmitRepro => run(db, &|tx| submit_repro(tx, who, req)),
        Operation::GetRepro { id, receipt } => run(db, &|tx| get_repro(tx, who, id, receipt)),
        Operation::ReproLog { key, stream } => run(db, &|tx| repro_log(tx, who, key, stream)),
        Operation::Assign => run(db, &|tx| assign(tx, ctx, who, req)),
        Operation::ReportCheckpoint { repo, role } => {
            run(db, &|tx| report_checkpoint(tx, who, repo, role, req))
        }
        Operation::LatestCheckpoint { repo, role } => {
            run(db, &|tx| latest_checkpoint(tx, who, repo, role))
        }
        Operation::CompareCheckpoint { repo, role } => {
            // authorization first; the supervisor is asked outside the database transaction
            // (review 48b1d0e I-2)
            if let Err(r) = need_compare(who, repo, role) {
                return Ok(r);
            }
            let local = role_status(ctx, repo, role);
            run(db, &|tx| compare_checkpoint(tx, repo, role, &local))
        }
        Operation::OwnAssignment { repo, role } => {
            run(db, &|tx| own_assignment(tx, who, repo, role))
        }
        Operation::RecordFirstAction { repo, role } => {
            run(db, &|tx| record_first_action(tx, who, repo, role, req))
        }
        Operation::ReportSessionStatus { repo, role } => {
            run(db, &|tx| report_session_status(tx, who, repo, role, req))
        }
        Operation::DeliverAssignment { repo, role } => {
            crate::delivery::deliver(db, ctx, who, repo, role, req, guard)
        }
        Operation::ApproveRecovery { repo, role } => {
            crate::recovery::approve(db, ctx, who, repo, role, req, guard)
        }
        Operation::GetRecovery { repo, role, id } => {
            run(db, &|tx| crate::recovery::get(tx, who, repo, role, id))
        }
        Operation::AssignmentChain {
            assignment_id,
            generation,
        } => run(db, &|tx| {
            crate::chain::assignment_chain(tx, who, &ctx.roles, assignment_id, generation)
        }),
        Operation::Ingest => run(db, &|tx| ingest(tx, who, req)),
        Operation::ReviewTransition => {
            run(db, &|tx| crate::review_store::record(tx, who, req.body))
        }
        Operation::GithubPush | Operation::GithubMerge => {
            crate::github_write::execute(db, ctx, who, req, guard, op == Operation::GithubMerge)
        }
        Operation::GithubReconcile => {
            run(db, &|tx| crate::github_write::reconcile(tx, ctx, who, req))
        }
        Operation::Judge => {
            let reply = run(db, &|tx| judge(tx, ctx, who, req))?;
            if reply.status == 201
                && let Ok(body) = parse_body(req.body)
                && let (Some(repo), Some(task), Some(event)) = (
                    body["repo_id"].as_str(),
                    body["task_id"].as_str(),
                    reply.body["event_id"].as_str(),
                )
            {
                // The original judgment is already committed. Missing or failed
                // dispatch is recoverable by the separately scoped watcher.
                if !crate::followups::process(db, ctx, repo, Some((task, event)), guard)
                    .is_ok_and(|response| response.status == 200)
                {
                    eprintln!("followup event dispatch deferred to periodic reconciliation");
                }
            }
            Ok(reply)
        }
        Operation::PlanFollowup => run(db, &|tx| crate::followups::plan(tx, ctx, who, req)),
        Operation::ReconcileFollowups => crate::followups::reconcile(db, ctx, who, req, guard),
        Operation::ListOutbox => run(db, &|tx| list_outbox(tx, who, req)),
        Operation::Deliver { id } => run(db, &|tx| deliver(tx, who, id, req)),
    }
}

/// The deliveries of a repo whose call's outcome is not known (02-05 step 6): no result recorded
/// yet (in flight, or its result transaction was lost), or the newest result `unknown`.
pub(crate) fn pending_deliveries(tx: &mut Transaction, repo: &str) -> Result<Vec<Value>, Resp> {
    Ok(tx
        .query(
            "SELECT d.delivery_id, d.role, d.attempt_id, iso(d.requested_at),
                    (SELECT outcome FROM role_delivery_results r WHERE r.delivery_id = d.delivery_id
                     ORDER BY n DESC LIMIT 1)
             FROM role_deliveries d WHERE d.repo_id = $1 ORDER BY d.seq",
            &[&repo],
        )?
        .iter()
        .filter_map(|r| {
            let outcome: Option<String> = r.get(4);
            let state = match outcome.as_deref() {
                None => "no result recorded",
                Some("unknown") => "result unknown",
                _ => return None,
            };
            Some(
                json!({"request_id": r.get::<_, String>(0), "kind": "delivery",
                "sent_at": r.get::<_, String>(3),
                "target": format!("{} supervisor start/reconnect", r.get::<_, String>(1)),
                "attempt_id": r.get::<_, String>(2), "state": state}),
            )
        })
        .collect())
}

pub(crate) fn need_client_scope(who: &Caller, scope: &str, sprints_path: bool) -> Result<(), Resp> {
    if who.kind == Kind::Agent {
        return Err(if sprints_path {
            problem(
                403,
                "AGENT_READ_DENIED",
                "agent credentials cannot use /v0/sprints*",
            )
        } else {
            problem(
                403,
                "SCOPE_DENIED",
                "agent credentials cannot use coordinator endpoints",
            )
        });
    }
    if !who.has(scope) {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            &format!("scope `{scope}` required"),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Registration (01-07 checks; acceptance = intake for semantic review only)
// ---------------------------------------------------------------------------

fn load_registry(tx: &mut Transaction, ctx: &Ctx) -> Result<Registry, Resp> {
    let mut reg = Registry::new();
    reg.add_profiles_from_manifest(&ctx.manifest);
    for row in tx.query(
        "SELECT spec, mode FROM sprint_revisions ORDER BY sprint_id, revision",
        &[],
    )? {
        let spec: Value = row.get(0);
        let mode: String = row.get(1);
        let m = if mode == "executable" {
            CheckMode::Executable
        } else {
            CheckMode::Draft
        };
        reg.add_spec_with_mode(&spec, m);
    }
    Ok(reg)
}

fn latest_revision(tx: &mut Transaction, sprint_id: &str) -> Result<Option<i64>, Resp> {
    Ok(tx
        .query_one(
            "SELECT max(revision) FROM sprint_revisions WHERE sprint_id = $1",
            &[&sprint_id],
        )?
        .get(0))
}

pub(crate) fn idem_key<'a>(req: &Req<'a>) -> Result<&'a str, Resp> {
    match req.idempotency_key {
        Some(k) if (8..=128).contains(&k.len()) => Ok(k),
        _ => Err(one(
            400,
            "MISSING_FIELD",
            "Idempotency-Key",
            "Idempotency-Key header (8-128 bytes) is required",
            "send an Idempotency-Key header",
        )),
    }
}

fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// A stored registration answer re-evaluated against the current registry.
fn reevaluate(tx: &mut Transaction, mut result: Value) -> Result<Value, Resp> {
    let sprint = result["sprint_id"].as_str().unwrap_or_default().to_string();
    let rev = result["revision"].as_i64().unwrap_or_default();
    if let Some(l) = latest_revision(tx, &sprint)?
        && l > rev
    {
        result["state"] = json!("SUPERSEDED");
        result["superseded_by"] = json!(l);
    }
    Ok(result)
}

/// The repo that a binding reserves this sprint_id for (02-05 C2, 02-03 N2), if any.
pub fn reserved_for<'a>(ctx: &'a Ctx, sprint_id: &str) -> Option<&'a str> {
    ctx.bindings
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, b)| b["sprint_id"].as_str() == Some(sprint_id))
        .map(|(repo, _)| repo.as_str())
}

/// Start-up check of the repo bindings against the stored sprints (X-01 U3, 02-05 C2). A
/// binding's sprint_id may not be (1) a test id that a Notion import owns (the conversion path
/// would write into it), (2) a sprint registered for another repo (RG2 would refuse the binding's
/// repo and the reservation every other repo, so nobody could register it), or (3) named by
/// another repo's binding too. The coordinator refuses to start; the operator fixes the binding.
pub fn check_bindings(db: &mut impl GenericClient, bindings: &Value) -> Result<(), String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for (repo, b) in bindings.as_object().into_iter().flatten() {
        let Some(sid) = b["sprint_id"].as_str() else {
            continue;
        };
        if let Some(first) = seen.insert(sid, repo) {
            problems.push(format!(
                "binding sprint_id named by two repos ({first}, {repo}: {sid})"
            ));
        }
        let imported: bool = db
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM notion_imports WHERE test_id = $1)",
                &[&sid],
            )
            .map_err(|e| format!("startup check: {e}"))?
            .get(0);
        if imported {
            problems.push(format!(
                "binding sprint_id owned by a Notion import ({repo}: {sid})"
            ));
        }
        let owner: Option<String> = db
            .query_opt(
                "SELECT repo_id FROM sprint_revisions WHERE sprint_id = $1 LIMIT 1",
                &[&sid],
            )
            .map_err(|e| format!("startup check: {e}"))?
            .map(|r| r.get(0));
        if let Some(o) = owner.filter(|o| o != repo) {
            problems.push(format!(
                "binding sprint_id registered for another repo ({repo}: {sid}, owned by {o})"
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("startup check: {}", problems.join("; ")))
    }
}

/// Which repo a sprint_id belongs to, checked before anything else of the body (so a malformed
/// body that names another repo's sprint stays 409, as before 02-05):
/// - RG2 (review 1e98f17 F1): a sprint_id belongs to the repo of its first registered revision;
///   a revision that names another repo would hand the sprint to that repo;
/// - 02-05 C2: a sprint_id that a repo binding names is reserved for that repo, also before its
///   first registration. The start-up check (`check_bindings`) keeps the two rules consistent.
fn guard_sprint_repo(tx: &mut Transaction, ctx: &Ctx, spec: &Value) -> Result<(), Resp> {
    let Some(sid) = spec["sprint_id"].as_str() else {
        return Ok(());
    };
    let repo = spec["target"]["repo_id"].as_str();
    let owner: Option<String> = match tx.query_opt(
        "SELECT repo_id FROM sprint_revisions WHERE sprint_id = $1 LIMIT 1",
        &[&sid],
    )? {
        Some(r) => Some(r.get(0)),
        None => reserved_for(ctx, sid).map(str::to_string),
    };
    if owner.is_some_and(|o| Some(o.as_str()) != repo) {
        return Err(one(
            409,
            "SPRINT_REPO_CONFLICT",
            "target.repo_id",
            "this sprint_id is registered or reserved for another repo",
            "use a new sprint_id for this repo",
        ));
    }
    Ok(())
}

/// The checks of a registration, without writing: the sprint's repo (RG2, C2 reservation), then
/// the 01-07 checks (format → digest → revision → references → binding for `mode=executable`).
/// A rejection is the error; any other outcome (new revision, `AlreadyRegistered`,
/// `Superseded`) is returned for the caller's further checks and `save_spec`.
fn check_spec(
    tx: &mut Transaction,
    ctx: &Ctx,
    spec: &Value,
    mode: CheckMode,
) -> Result<CheckOutcome, Resp> {
    guard_sprint_repo(tx, ctx, spec)?;
    let reg = load_registry(tx, ctx)?;
    let out = check_registration(spec, &reg, mode);
    if out.state == CheckState::Rejected {
        let list = out
            .rejections
            .iter()
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect();
        return Err(rejected(out.http_status(), list));
    }
    Ok(out)
}

/// Shared by `registerSprint` and `convertHumanInput`. Returns (status, RegistrationResult).
fn register_spec(
    tx: &mut Transaction,
    ctx: &Ctx,
    who: &Caller,
    spec: &Value,
    mode: CheckMode,
) -> Result<(u16, Value), Resp> {
    let out = check_spec(tx, ctx, spec, mode)?;
    save_spec(tx, who, spec, mode, out)
}

/// Store a checked registration (a new revision, or a draft-stored revision promoted to
/// executable); `AlreadyRegistered` and `Superseded` write nothing.
fn save_spec(
    tx: &mut Transaction,
    who: &Caller,
    spec: &Value,
    mode: CheckMode,
    out: CheckOutcome,
) -> Result<(u16, Value), Resp> {
    let status = out.http_status();
    let mode_s = if mode == CheckMode::Executable {
        "executable"
    } else {
        "draft"
    };
    let sprint_id = spec["sprint_id"].as_str().unwrap_or_default();
    let revision = spec["revision"].as_i64().unwrap_or_default();
    let digest = out.computed_digest.clone().unwrap_or_default();
    let state = match out.state {
        // check_spec returns every rejection as its error; never stored
        CheckState::Rejected => {
            return Err(problem(
                500,
                "INTERNAL",
                "a rejected registration reached the store",
            ));
        }
        CheckState::AcceptedForSemanticReview => {
            let existing = tx.execute(
                "UPDATE sprint_revisions SET mode = 'executable'
                 WHERE sprint_id = $1 AND revision = $2 AND digest = $3 AND mode = 'draft' AND $4 = 'executable'",
                &[&sprint_id, &revision, &digest, &mode_s],
            )?;
            if existing == 0 {
                tx.execute(
                    "INSERT INTO sprint_revisions (sprint_id, revision, repo_id, digest, spec, mode,
                        registration_state, registered_by)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
                    &[
                        &sprint_id,
                        &revision,
                        &spec["target"]["repo_id"].as_str().unwrap_or_default(),
                        &digest,
                        spec,
                        &mode_s,
                        &spec["registration_state"].as_str().unwrap_or("draft"),
                        &who.name,
                    ],
                )?;
            }
            history(
                tx,
                &who.name,
                "sprint.registered",
                &json!({"sprint_id": sprint_id, "revision": revision, "digest": digest, "mode": mode_s}),
            )?;
            "ACCEPTED_FOR_SEMANTIC_REVIEW"
        }
        CheckState::AlreadyRegistered => "ALREADY_REGISTERED",
        CheckState::Superseded => "SUPERSEDED",
    };
    let mut result = json!({"sprint_id": sprint_id, "revision": revision, "digest": digest,
                            "state": state, "mode": mode_s});
    if let Some(s) = out.superseded_by {
        result["superseded_by"] = json!(s);
    }
    Ok((status, result))
}

fn idempotent_lookup(
    tx: &mut Transaction,
    who: &Caller,
    endpoint: &str,
    key: &str,
    body_digest: &str,
    mode: &str,
) -> Result<Option<Value>, Resp> {
    let row = tx.query_opt(
        "SELECT body_digest, mode, response FROM idempotency
         WHERE principal = $1 AND endpoint = $2 AND key = $3",
        &[&who.name, &endpoint, &key],
    )?;
    match row {
        None => Ok(None),
        Some(r) => {
            let d: String = r.get(0);
            let m: String = r.get(1);
            if d == body_digest && m == mode {
                Ok(Some(r.get(2)))
            } else {
                Err(one(
                    409,
                    "IDEMPOTENCY_KEY_CONFLICT",
                    "Idempotency-Key",
                    "this key was used with a different body or mode",
                    "use a new key for a different request",
                ))
            }
        }
    }
}

fn idempotent_store(
    tx: &mut Transaction,
    who: &Caller,
    endpoint: &str,
    key: &str,
    body_digest: &str,
    mode: &str,
    response: &Value,
) -> Result<(), Resp> {
    tx.execute(
        "INSERT INTO idempotency (principal, endpoint, key, body_digest, mode, response)
         VALUES ($1,$2,$3,$4,$5,$6)",
        &[&who.name, &endpoint, &key, &body_digest, &mode, response],
    )?;
    Ok(())
}

fn register(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "sprints:register", true)?;
    let body = parse_body(req.body)?;
    let Some(repo) = body["target"]["repo_id"].as_str() else {
        return Err(one(
            400,
            "MISSING_FIELD",
            "target.repo_id",
            "target.repo_id is required",
            "supply target.repo_id",
        ));
    };
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    let key = idem_key(req)?;
    let mode = match query_param(req.query, "mode") {
        None | Some("draft") => CheckMode::Draft,
        Some("executable") => CheckMode::Executable,
        Some(_) => {
            return Err(one(
                400,
                "SCHEMA_VIOLATION",
                "mode",
                "mode must be draft or executable",
                "use mode=draft or mode=executable",
            ));
        }
    };
    let mode_s = if mode == CheckMode::Executable {
        "executable"
    } else {
        "draft"
    };
    // registrations are rare; one lock keeps cross-sprint task-ID checks consistent
    lock(tx, "registry")?;
    let body_digest = sha256_hex(&canonical_json(&body));
    if let Some(stored) = idempotent_lookup(tx, who, "registerSprint", key, &body_digest, mode_s)? {
        let result = reevaluate(tx, stored)?;
        return ok(200, result);
    }
    // 02-05 U2 (X-01 review 74bd796): sprint repo (RG2, C2 reservation) → the 01-07 checks →
    // import ownership → import gate → store. Ownership and the gate apply to every outcome the
    // 01-07 checks do not reject — a new revision (201), `AlreadyRegistered` and `Superseded`
    // (200) — so moving them after the 01-07 checks changes only which code a request that
    // fails several checks gets first, never whether it is refused (review 3c017a4 N-2).
    let out = check_spec(tx, ctx, &body, mode)?;
    guard_import_owned(tx, &body)?;
    // C2 (review 9a421b7 F1): the gate follows the Notion sources a spec cites, whatever its
    // sprint_id, so a copy of a gated task under another sprint id is gated too.
    if mode == CheckMode::Executable {
        notion_gate(
            tx,
            &body,
            body["sprint_id"].as_str().unwrap_or_default(),
            repo,
        )?;
    }
    let (status, result) = save_spec(tx, who, &body, mode, out)?;
    idempotent_store(
        tx,
        who,
        "registerSprint",
        key,
        &body_digest,
        mode_s,
        &result,
    )?;
    ok(status, result)
}

fn convert(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "sprints:register", true)?;
    let body = parse_body(req.body)?;
    let Some(repo) = body["repo"].as_str() else {
        return Err(one(
            400,
            "MISSING_FIELD",
            "repo",
            "repo is required",
            "supply repo",
        ));
    };
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    let key = idem_key(req)?;
    lock(tx, "registry")?;
    let body_digest = sha256_hex(&canonical_json(&body));
    if let Some(mut stored) =
        idempotent_lookup(tx, who, "convertHumanInput", key, &body_digest, "draft")?
    {
        stored["registration"] = reevaluate(tx, stored["registration"].take())?;
        return ok(200, stored);
    }
    let b = &ctx.bindings[repo];
    // a binding's sprint_id is reserved against imports, so this sprint is never import-owned
    let sprint_id = b["sprint_id"].as_str().unwrap_or_default().to_string();
    let (status, rec, registration) = convert_and_register(
        tx,
        ctx,
        who,
        &body,
        repo,
        Imported {
            sprint_id,
            sprint_number: None,
            extra_sources: vec![],
            extra_unresolved: vec![],
            input_ref: format!("sprint-input:{body_digest}"),
        },
    )?;
    let result = json!({"conversion": rec, "registration": registration});
    idempotent_store(
        tx,
        who,
        "convertHumanInput",
        key,
        &body_digest,
        "draft",
        &result,
    )?;
    ok(status, result)
}

/// The Notion page id a URL names (shared strict rule, `ieo_spec::notion_url`).
pub fn url_page_id(url: &str) -> Option<String> {
    ieo_spec::notion_url::page_id(url)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `POST /v0/imports/notion` (X-01): register a task read from Notion as a **draft** sprint
/// under a test ID that is distinct from the Notion task ID. The importer sends the source
/// identity (page, URL, last edit, content digest) and the human input it derived; the server
/// applies the repo's binding, adds the Notion page as a source and the importer's unresolved
/// reasons, and registers through the normal 01-07 checks. Same page + same content → 200
/// replay (nothing new). Changed content → the next revision; earlier revisions stay stored.
/// Nothing here can make a spec executable: that still needs `mode=executable` registration,
/// bound limits, no unresolved reasons and an independent semantic review.
fn import_notion(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    // X-01 C2: only the import service writes trusted collection records
    need_client_scope(who, "notion:import", true)?;
    let body = parse_body(req.body)?;
    let input = &body["input"];
    let Some(repo) = input["repo"].as_str() else {
        return Err(one(
            400,
            "MISSING_FIELD",
            "input.repo",
            "input.repo is required",
            "supply input.repo",
        ));
    };
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    only_keys(
        &body,
        &["test_id", "source", "input", "unresolved", "gates"],
    )?;
    let test_id = str_field(&body, "test_id", 64)?.to_string();
    if !test_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "test_id",
            "test_id is [A-Za-z0-9._-]",
            "fix test_id",
        ));
    }
    let src = &body["source"];
    only_keys(
        src,
        &[
            "system",
            "page_id",
            "url",
            "last_edited_time",
            "content_digest",
            "source_task_id",
            "fetched_via",
            "observed_at",
        ],
    )?;
    if src["system"] != "notion" {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "source.system",
            "source.system must be notion",
            "fix source",
        ));
    }
    let page_id = str_field(src, "page_id", 64)?.to_string();
    // review f571d1e L3: one canonical form, so a page cannot be imported twice under different
    // spellings (dashed, upper case) and slip past the one-test-id rule
    if !(page_id.len() == 32
        && page_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "source.page_id",
            "not a Notion page id in canonical form (32 lowercase hex, no dashes)",
            "send the page id without dashes, in lower case",
        ));
    }
    let url = str_field(src, "url", 1024)?.to_string();
    // X-02 prep (user decision 2026-11-14 §5): the page the URL names must be the imported page
    if url_page_id(&url).as_deref() != Some(page_id.as_str()) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "source.url",
            "the page id in the URL does not match source.page_id",
            "send the URL of the imported page",
        ));
    }
    let edited = str_field(src, "last_edited_time", 64)?.to_string();
    let digest = str_field(src, "content_digest", 64)?.to_string();
    if !is_hex64(&digest) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "source.content_digest",
            "not a SHA-256 hex",
            "fix the digest",
        ));
    }
    let source_task_id = str_field(src, "source_task_id", 256)?.to_string();
    if test_id == source_task_id {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "test_id",
            "the test id must differ from the Notion task id",
            "choose a separate test id",
        ));
    }
    str_field(src, "fetched_via", 128)?;
    let list = |k: &str| -> Result<Vec<String>, Resp> {
        match &body[k] {
            Value::Null => Ok(vec![]),
            Value::Array(a) if a.iter().all(|x| x.as_str().is_some_and(|s| !s.is_empty())) => Ok(a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()),
            _ => Err(one(
                400,
                "SCHEMA_VIOLATION",
                k,
                "must be a list of non-empty strings",
                "fix the list",
            )),
        }
    };
    // C2: queue gates and missing facts are recorded separately by the import service
    let gates = list("gates")?;
    let missing = list("unresolved")?;
    let unresolved: Vec<String> = match &body["unresolved"] {
        Value::Null => vec![],
        Value::Array(a) if a.iter().all(|x| x.as_str().is_some_and(|s| !s.is_empty())) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => {
            return Err(one(
                400,
                "SCHEMA_VIOLATION",
                "unresolved",
                "unresolved is a list of strings",
                "fix unresolved",
            ));
        }
    };
    lock(tx, "registry")?;
    // review 9a421b7 F2: a replay is only the page's *latest* import; content that returns to an
    // earlier state (A→B→A) becomes a new revision so its gates apply again
    if let Some(row) = tx.query_opt(
        "SELECT test_id, response FROM notion_imports WHERE page_id = $1 AND content_digest = $2
           AND (test_id, sprint_revision) = (SELECT test_id, sprint_revision FROM notion_imports
                 WHERE page_id = $1 ORDER BY sprint_revision DESC, imported_at DESC LIMIT 1)",
        &[&page_id, &digest],
    )? {
        if row.get::<_, &str>(0) != test_id {
            return Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "test_id",
                "this Notion page content was imported under another test_id",
                "use the test_id of the earlier import",
            ));
        }
        // a replay answers only within the repo the import belongs to (review b8b619a R2)
        let stored_repo: Option<String> = tx
            .query_opt(
                "SELECT repo_id FROM sprint_revisions WHERE sprint_id = $1 LIMIT 1",
                &[&test_id],
            )?
            .map(|r| r.get(0));
        if stored_repo.as_deref() != Some(repo) {
            return Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "input.repo",
                "this Notion page content was imported for another repo",
                "import under the repo of the earlier import",
            ));
        }
        let mut stored: Value = row.get(1);
        stored["registration"] = reevaluate(tx, stored["registration"].take())?;
        stored["replay"] = json!(true);
        return ok(200, stored);
    }
    if let Some(row) = tx.query_opt(
        "SELECT test_id FROM notion_imports WHERE page_id = $1 AND test_id <> $2 LIMIT 1",
        &[&page_id, &test_id],
    )? {
        let _ = row;
        return Err(one(
            409,
            "EVENT_CONTENT_CONFLICT",
            "test_id",
            "this Notion page was imported under another test_id",
            "use the test_id of the earlier import",
        ));
    }
    if let Some(row) = tx.query_opt(
        "SELECT page_id FROM notion_imports WHERE test_id = $1 LIMIT 1",
        &[&test_id],
    )? && row.get::<_, &str>(0) != page_id
    {
        return Err(one(
            409,
            "EVENT_CONTENT_CONFLICT",
            "test_id",
            "this test_id belongs to another Notion page",
            "use a new test_id",
        ));
    }
    // a test id is never an existing sprint that no Notion import created (review b8b619a R1)
    let imported_before: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM notion_imports WHERE test_id = $1)",
            &[&test_id],
        )?
        .get(0);
    let reserved = reserved_for(ctx, &test_id).is_some();
    if !imported_before && (reserved || latest_revision(tx, &test_id)?.is_some()) {
        return Err(one(
            409,
            "EVENT_CONTENT_CONFLICT",
            "test_id",
            "test_id names an existing sprint that was not created by a Notion import",
            "use a new test_id",
        ));
    }
    // task-ID prefix: the test sprint keeps the number it was first given; a new test sprint
    // takes the first number of the binding's import pool that no other sprint uses
    let b = &ctx.bindings[repo];
    let existing: Option<String> = tx
        .query_opt(
            "SELECT spec->'tasks'->0->>'task_id' FROM sprint_revisions WHERE sprint_id = $1
             ORDER BY revision DESC LIMIT 1",
            &[&test_id],
        )?
        .and_then(|r| r.get(0));
    let number = match existing.and_then(|t| t.get(..2).and_then(|n| n.parse::<u8>().ok())) {
        Some(n) => n,
        None => {
            let pool: Vec<u8> = b["import_sprint_numbers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect();
            let mut free = None;
            for n in pool {
                let prefix = format!("{n:02}-%");
                let used: bool = tx
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM sprint_revisions, jsonb_array_elements(spec->'tasks') t
                           WHERE t->>'task_id' LIKE $1)",
                        &[&prefix],
                    )?
                    .get(0);
                if !used {
                    free = Some(n);
                    break;
                }
            }
            let Some(n) = free else {
                return Err(one(
                    422,
                    "UNBOUND_DRAFT",
                    "input.repo",
                    "the repo binding has no free import sprint number",
                    "add numbers to the binding's import_sprint_numbers",
                ));
            };
            n
        }
    };
    let observed = src["observed_at"].as_str().map(str::to_string);
    let notion_source = json!({"kind": "notion_page", "ref": url, "revision": edited,
                               "digest": digest, "observed_at": observed});
    let (status, rec, registration) = convert_and_register(
        tx,
        ctx,
        who,
        input,
        repo,
        Imported {
            sprint_id: test_id.clone(),
            sprint_number: Some(number),
            extra_sources: vec![notion_source],
            extra_unresolved: gates.iter().chain(unresolved.iter()).cloned().collect(),
            input_ref: format!("notion:{page_id}@{digest}"),
        },
    )?;
    let import = json!({"test_id": test_id, "source_task_id": source_task_id, "page_id": page_id,
                        "url": url, "last_edited_time": edited, "content_digest": digest});
    let result = json!({"import": import, "conversion": rec, "registration": registration,
                        "replay": false});
    let rev = registration["revision"].as_i64().unwrap_or_default();
    tx.execute(
        "INSERT INTO notion_imports (page_id, content_digest, test_id, url, last_edited_time,
            source, sprint_revision, response, imported_by, gates, missing, imported_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, clock_timestamp())",
        &[
            &page_id,
            &digest,
            &test_id,
            &url,
            &edited,
            src,
            &rev,
            &result,
            &who.name,
            &json!(gates),
            &json!(missing),
        ],
    )?;
    history(
        tx,
        &who.name,
        "notion.imported",
        &json!({"test_id": test_id, "page_id": page_id, "content_digest": digest, "revision": rev}),
    )?;
    ok(status, result)
}

/// A sprint created by a Notion import (X-01) accepts a `/v0/sprints` registration only when the
/// spec is derived from its latest import: the same tasks and the same `notion_page` source digest.
/// That is how an imported draft becomes executable once its limits are bound and its reasons are
/// resolved (review 7a48a4f T1); any other content → 409 (review 38b671b S2). Runs after
/// `check_spec` (02-05 U2), so the body is well-formed and names the sprint's own repo; the repo
/// test below only keeps the guard self-contained.
fn guard_import_owned(tx: &mut Transaction, spec: &Value) -> Result<(), Resp> {
    let sid = spec["sprint_id"].as_str().unwrap_or_default();
    let Some(row) = tx.query_opt(
        "SELECT n.content_digest, s.repo_id, s.spec FROM notion_imports n
           JOIN sprint_revisions s ON s.sprint_id = n.test_id AND s.revision = n.sprint_revision
         WHERE n.test_id = $1 ORDER BY n.sprint_revision DESC, n.imported_at DESC LIMIT 1",
        &[&sid],
    )?
    else {
        return Ok(());
    };
    let digest: String = row.get(0);
    let owner: String = row.get(1);
    let imported: Value = row.get(2);
    if spec["target"]["repo_id"].as_str() != Some(owner.as_str()) {
        return Ok(());
    }
    let same_source = spec["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|x| x["kind"] == "notion_page" && x["digest"].as_str() == Some(digest.as_str()));
    if same_source && spec["tasks"] == imported["tasks"] {
        return Ok(());
    }
    Err(one(
        409,
        "EVENT_CONTENT_CONFLICT",
        "sprint_id",
        "this sprint was created by a Notion import; only a spec derived from its latest import \
         (same tasks and notion_page source) may be registered here",
        "re-import the Notion task, or keep its tasks and source and bind only limits/readiness",
    ))
}

/// Candidate Notion page ids (32 lowercase hex) in any string of a source, dashed or not: every
/// 32-character window of each hex run, so a page id inside a URL slug is found. Over-inclusion
/// is harmless (only ids of imported pages match).
fn page_ids_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(t) => {
            let flat: String = t
                .chars()
                .filter(|c| *c != '-')
                .collect::<String>()
                .to_lowercase();
            for run in flat.split(|c: char| !c.is_ascii_hexdigit()) {
                for i in 0..=run.len().saturating_sub(32) {
                    if run.len() >= 32 {
                        out.push(run[i..i + 32].to_string());
                    }
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| page_ids_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| page_ids_in(x, out)),
        _ => {}
    }
}

/// X-01 C2: the execution gate, from server records only — never from spec text a registrar can
/// edit. The Notion pages concerned are (1) those imported for this sprint, (2) every imported page
/// whose id appears in any field of any source, whatever its `kind` or digest (review 7afd4d1
/// G1), and (3) every imported page whose content digest a source cites, unless the latest
/// import of a page that source names carries that digest (review e908a8c H1, aa38041 I1). A copy whose sources
/// no longer cite the gated page's id nor its imported content digest — both dropped, or both
/// rewritten (including to another imported page's id and digest) — is not recognised; it rests
/// on the registrar and the semantic review. "Latest
/// import" is ordered the same way everywhere: revision first (assigned under the registry lock,
/// so a clock that jumps backwards cannot make an older release look newer than a later
/// re-gating import — review f0c4e87 J2), then `imported_at` (stamped with `clock_timestamp()`
/// after the lock). A page imported under more than one test id (possible only in data from before
/// that was refused and page ids were required in canonical form) has no comparable revision
/// order and is refused (review 6352413 K1, f571d1e L3). For each page the **latest**
/// import decides: missing facts always block (`IMPORT_INCOMPLETE`); queue gates block
/// (`IMPORT_GATED`) unless a user-approved exception exists for exactly this test sprint, repo,
/// page and that import's content digest. A later trusted import without gates is the release
/// record.
fn notion_gate(
    tx: &mut Transaction,
    spec: &Value,
    sprint_id: &str,
    repo: &str,
) -> Result<(), Resp> {
    let mut pages: Vec<String> = tx
        .query(
            "SELECT DISTINCT page_id FROM notion_imports WHERE test_id = $1",
            &[&sprint_id],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect();
    for source in spec["sources"].as_array().into_iter().flatten() {
        let mut ids = Vec::new();
        page_ids_in(source, &mut ids);
        let named: Vec<String> = tx
            .query(
                "SELECT DISTINCT page_id FROM notion_imports WHERE page_id = ANY($1)",
                &[&ids],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect();
        // the cited content digest also decides, unless the latest import of one of the named
        // pages carries it (review e908a8c H1: a reference rewritten to another imported page
        // must not hide the page whose content is cited; review 7afd4d1 G2: identical content
        // of a page the source names does not pull in other pages; review aa38041 I1: only the
        // named page's latest import counts, not content it carried once)
        let digest = source["digest"].as_str().unwrap_or_default().to_string();
        let carried: bool = !named.is_empty()
            && tx
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM (SELECT DISTINCT ON (page_id) content_digest FROM notion_imports
                       WHERE page_id = ANY($1) ORDER BY page_id, sprint_revision DESC, imported_at DESC) l
                     WHERE l.content_digest = $2)",
                    &[&named, &digest],
                )?
                .get(0);
        pages.extend(named);
        if !carried {
            pages.extend(
                tx.query(
                    "SELECT DISTINCT page_id FROM notion_imports WHERE content_digest = $1",
                    &[&digest],
                )?
                .iter()
                .map(|r| r.get::<_, String>(0)),
            );
        }
    }
    pages.sort();
    pages.dedup();
    for page in pages {
        // review 6352413 K1: a page imported under more than one test id (only possible before
        // that was refused) has no comparable revision order; it fails closed
        let test_ids: i64 = tx
            .query_one(
                "SELECT count(DISTINCT test_id) FROM notion_imports WHERE page_id = $1",
                &[&page],
            )?
            .get(0);
        if test_ids > 1 {
            return Err(one(
                409,
                "IMPORT_GATED",
                "sprint_id",
                "a cited Notion page was imported under more than one test id; its latest import cannot be determined",
                "resolve the earlier duplicate import outside the API before executing this page's work",
            ));
        }
        let row = tx.query_one(
            "SELECT gates, missing, response, test_id, content_digest FROM notion_imports
             WHERE page_id = $1 ORDER BY sprint_revision DESC, imported_at DESC LIMIT 1",
            &[&page],
        )?;
        let (gates, missing): (Value, Value) = match (
            row.get::<_, Option<Value>>(0),
            row.get::<_, Option<Value>>(1),
        ) {
            (Some(g), Some(m)) => (g, m),
            _ => {
                // imported before C2: derive from the import service's stored response
                let response: Value = row.get(2);
                let reasons: Vec<String> = response["conversion"]["unresolved_bindings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                let (g, m) = legacy_split(&reasons);
                (json!(g), json!(m))
            }
        };
        let test_id: String = row.get(3);
        let latest_digest: String = row.get(4);
        if missing.as_array().is_some_and(|m| !m.is_empty()) {
            return Err(one(
                409,
                "IMPORT_INCOMPLETE",
                "sprint_id",
                &format!(
                    "the latest import of a cited Notion page lacks required facts: {missing}"
                ),
                "re-import once the Notion task states them; an exception does not lift this",
            ));
        }
        if gates.as_array().is_some_and(|g| !g.is_empty()) {
            // review 9a421b7 F4, 7afd4d1 G2: the exception is bound to the page and the import
            // content it approved
            let excepted: bool = test_id == sprint_id
                && tx
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM exec_exceptions WHERE test_id = $1 AND repo_id = $2
                           AND lifts = 'queue_gates' AND page_id = $3 AND content_digest = $4)",
                        &[&sprint_id, &repo, &page, &latest_digest],
                    )?
                    .get(0);
            if !excepted {
                return Err(one(
                    409,
                    "IMPORT_GATED",
                    "sprint_id",
                    &format!(
                        "the latest import of a cited Notion page records unreleased queue gates: {gates}"
                    ),
                    "wait for a trusted import that records the release, or a user-approved exception for this test and content",
                ));
            }
        }
    }
    Ok(())
}

/// Split the reasons of an import recorded before C2 into (queue gates, missing facts), using the
/// importer's own fixed reason templates. The converter's binding items are resolved by binding,
/// not by the import. Anything else is treated as a missing fact (fail closed).
pub fn legacy_split(reasons: &[String]) -> (Vec<String>, Vec<String>) {
    const GATES: [&str; 4] = [
        "Notion 큐 구획 '",
        "Notion 큐 구획 없음",
        "Notion 작업 상태 '",
        "Notion 차단 대상 미해제:",
    ];
    const BINDING: [&str; 4] = [
        "실제 repository_url",
        "시간/토큰/교정 한도",
        "착수 조건",
        "limits",
    ];
    let mut gates = Vec::new();
    let mut missing = Vec::new();
    for r in reasons {
        if GATES.iter().any(|p| r.starts_with(p)) {
            gates.push(r.clone());
        } else if !BINDING.contains(&r.as_str()) {
            missing.push(r.clone());
        }
    }
    (gates, missing)
}

/// `POST /v0/coordinator/exceptions` (X-01 C2): a person records a PoC execution exception for one
/// imported test sprint. It is a separate record; the Notion task and its gates stay as they are.
fn record_exception(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "exceptions:approve", false)?;
    if who.kind != Kind::User {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "exceptions are approved by a person",
        ));
    }
    let body = parse_body(req.body)?;
    only_keys(
        &body,
        &[
            "exception_id",
            "test_id",
            "repo_id",
            "lifts",
            "scope",
            "decision_ref",
        ],
    )?;
    let repo = str_field(&body, "repo_id", 256)?.to_string();
    if !who.in_repo(&repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    let id = str_field(&body, "exception_id", 128)?.to_string();
    let test_id = str_field(&body, "test_id", 64)?.to_string();
    if body["lifts"] != "queue_gates" {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "lifts",
            "lifts must be queue_gates",
            "use queue_gates",
        ));
    }
    if !body["scope"].is_object() {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "scope",
            "scope is an object (branch, purpose, limits)",
            "fix scope",
        ));
    }
    let decision_ref = str_field(&body, "decision_ref", 512)?.to_string();
    lock(tx, "registry")?;
    if let Some(row) = tx.query_opt(
        "SELECT request FROM exec_exceptions WHERE exception_id = $1",
        &[&id],
    )? {
        let stored: Value = row.get(0);
        return if stored == body {
            ok(200, json!({"exception_id": id, "replay": true}))
        } else {
            Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "exception_id",
                "exception_id used with other content",
                "use a new id",
            ))
        };
    }
    let latest: Option<(String, String, String)> = tx
        .query_opt(
            "SELECT s.repo_id, n.content_digest, n.page_id FROM notion_imports n JOIN sprint_revisions s
               ON s.sprint_id = n.test_id AND s.revision = n.sprint_revision
             WHERE n.test_id = $1 ORDER BY n.sprint_revision DESC, n.imported_at DESC LIMIT 1",
            &[&test_id],
        )?
        .map(|r| (r.get(0), r.get(1), r.get(2)));
    let Some((owner, approved_digest, page_id)) = latest.filter(|(o, _, _)| *o == repo) else {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "test_id",
            "no imported test sprint with this id in this repo",
            "name an imported test sprint",
        ));
    };
    let _ = owner;
    tx.execute(
        "INSERT INTO exec_exceptions (exception_id, test_id, repo_id, lifts, scope, decision_ref, request,
            approved_by, content_digest, page_id)
         VALUES ($1,$2,$3,'queue_gates',$4,$5,$6,$7,$8,$9)",
        &[&id, &test_id, &repo, &body["scope"], &decision_ref, &body, &who.name, &approved_digest, &page_id],
    )?;
    history(
        tx,
        &who.name,
        "exception.approved",
        &json!({"exception_id": id, "test_id": test_id, "repo_id": repo, "decision_ref": decision_ref, "approved_content_digest": approved_digest, "approved_page_id": page_id}),
    )?;
    ok(
        201,
        json!({"exception_id": id, "test_id": test_id, "repo_id": repo, "lifts": "queue_gates", "approved_content_digest": approved_digest, "approved_page_id": page_id, "replay": false}),
    )
}

/// Per-request additions to a repo binding (Notion import, X-01).
struct Imported {
    sprint_id: String,
    /// Two-digit task-ID prefix; `None` = the binding's `sprint_number`.
    sprint_number: Option<u8>,
    extra_sources: Vec<Value>,
    extra_unresolved: Vec<String>,
    input_ref: String,
}

/// Convert a human input with the repo's approved binding and register the result as a
/// draft. Extra sources and unresolved reasons are added before the digest is computed, so
/// a spec with any unresolved reason can never be stored as `ready` (01-07 checks).
fn convert_and_register(
    tx: &mut Transaction,
    ctx: &Ctx,
    who: &Caller,
    input: &Value,
    repo: &str,
    imp: Imported,
) -> Result<(u16, ieo_spec::ConversionRecord, Value), Resp> {
    let b = &ctx.bindings[repo];
    if !b.is_object() {
        return Err(one(
            422,
            "UNBOUND_DRAFT",
            "repo",
            "no approved conversion binding for this repo",
            "the repo's binding (sprint number, delegation, sources, profile, limits) must be approved first",
        ));
    }
    let next_rev = latest_revision(tx, &imp.sprint_id)?.unwrap_or(0) + 1;
    // tasks of *other* sprints (a sprint's own earlier revision is not a conflict)
    let mut registered = BTreeMap::new();
    for row in tx.query(
        "SELECT DISTINCT ON (sprint_id) spec FROM sprint_revisions WHERE sprint_id <> $1
         ORDER BY sprint_id, revision DESC",
        &[&imp.sprint_id],
    )? {
        let spec: Value = row.get(0);
        for t in spec["tasks"].as_array().into_iter().flatten() {
            registered.insert(
                t["task_id"].as_str().unwrap_or_default().to_string(),
                spec["plan_version"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            );
        }
    }
    let pid = b["acceptance_profile_id"].as_str().unwrap_or_default();
    let prev = b["acceptance_profile_revision"]
        .as_u64()
        .unwrap_or_default();
    let profile_items = ctx.manifest["inspection_profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["profile_id"] == pid && p["revision"].as_u64() == Some(prev))
        .and_then(|p| p["items"].as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let cctx = ConversionContext {
        sprint_id: imp.sprint_id.clone(),
        sprint_number: imp.sprint_number.unwrap_or_else(|| {
            u8::try_from(b["sprint_number"].as_u64().unwrap_or_default()).unwrap_or(0)
        }),
        revision: u64::try_from(next_rev).unwrap_or_default(),
        plan_version: b["plan_version"].as_str().unwrap_or_default().to_string(),
        repository_url: b["repository_url"].as_str().map(str::to_string),
        timezone: b["timezone"].as_str().unwrap_or("Asia/Seoul").to_string(),
        track: b["track"].as_str().unwrap_or("핵심").to_string(),
        entry_condition: b["entry_condition"].as_str().map(str::to_string),
        acceptance_profile_id: pid.to_string(),
        acceptance_profile_revision: prev,
        profile_items,
        delegation: b["delegation"].clone(),
        limits: b.get("limits").filter(|l| l.is_object()).cloned(),
        human_validation: b["human_validation"].clone(),
        sources: b["sources"].clone(),
        registered_tasks: registered,
        input_ref: imp.input_ref.clone(),
    };
    let mut rec = convert_human_input(input, &cctx).map_err(|rs| {
        let status = rs.iter().map(|r| r.code.http_status()).min().unwrap_or(400);
        rejected(
            status,
            rs.iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
        )
    })?;
    if !imp.extra_sources.is_empty() || !imp.extra_unresolved.is_empty() {
        if let Some(src) = rec.spec["sources"].as_array_mut() {
            src.extend(imp.extra_sources.iter().cloned());
        }
        for u in &imp.extra_unresolved {
            if !rec.unresolved_bindings.contains(u) {
                rec.unresolved_bindings.push(u.clone());
            }
        }
        rec.spec["unresolved_bindings"] = json!(rec.unresolved_bindings);
        if !rec.unresolved_bindings.is_empty() {
            rec.spec["registration_state"] = json!("draft");
        }
        rec.spec["digest"] = json!(ieo_spec::spec_digest(&rec.spec));
    }
    let (status, registration) = register_spec(tx, ctx, who, &rec.spec, CheckMode::Draft)?;
    Ok((status, rec, registration))
}

/// Sprint row visible to this client, else 404 (a sprint of another repo is not revealed).
fn visible_sprint(tx: &mut Transaction, who: &Caller, sprint_id: &str) -> Result<String, Resp> {
    let row = tx.query_opt(
        "SELECT repo_id FROM sprint_revisions WHERE sprint_id = $1 ORDER BY revision DESC LIMIT 1",
        &[&sprint_id],
    )?;
    match row {
        Some(r) if who.in_repo(r.get::<_, &str>(0)) => Ok(r.get(0)),
        _ => Err(problem(
            404,
            "NOT_FOUND",
            "unknown sprint or not visible to this principal",
        )),
    }
}

fn get_revision(tx: &mut Transaction, who: &Caller, id: &str, rev: &str) -> R {
    need_client_scope(who, "sprints:read", true)?;
    visible_sprint(tx, who, id)?;
    let Ok(rev) = rev.parse::<i64>() else {
        return Err(problem(404, "NOT_FOUND", "unknown revision"));
    };
    let row = tx.query_opt(
        "SELECT spec, digest, registration_state, review_state, iso(registered_at)
         FROM sprint_revisions WHERE sprint_id = $1 AND revision = $2",
        &[&id, &rev],
    )?;
    let Some(r) = row else {
        return Err(problem(404, "NOT_FOUND", "unknown revision"));
    };
    ok(
        200,
        json!({"spec": r.get::<_, Value>(0), "digest": r.get::<_, String>(1),
                   "registration_state": r.get::<_, String>(2), "review_state": r.get::<_, String>(3),
                   "registered_at": r.get::<_, String>(4)}),
    )
}

/// Independent semantic review of one registered revision (P01). Separate from the
/// format/reference check; the registrant cannot review its own registration (G3).
fn semantic_review(tx: &mut Transaction, who: &Caller, id: &str, rev: &str, req: &Req) -> R {
    need_client_scope(who, "review:semantic", true)?;
    visible_sprint(tx, who, id)?;
    let Ok(rev) = rev.parse::<i64>() else {
        return Err(problem(404, "NOT_FOUND", "unknown revision"));
    };
    lock(tx, "registry")?;
    let Some(row) = tx.query_opt(
        "SELECT digest, registered_by FROM sprint_revisions WHERE sprint_id = $1 AND revision = $2",
        &[&id, &rev],
    )?
    else {
        return Err(problem(404, "NOT_FOUND", "unknown revision"));
    };
    let digest: String = row.get(0);
    let registrant: String = row.get(1);
    // whoever registered or promoted this revision cannot review it (review 1e98f17 F10)
    let registered_it: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM history WHERE action = 'sprint.registered' AND actor = $1
               AND subject->>'sprint_id' = $2 AND (subject->>'revision')::bigint = $3)",
            &[&who.name, &id, &rev],
        )?
        .get(0);
    if registrant == who.name || registered_it {
        return Err(problem(
            403,
            "SELF_REVIEW_DENIED",
            "the registrant cannot review its own registration (policy G3)",
        ));
    }
    let body = parse_body(req.body)?;
    only_keys(&body, &["decision", "reason", "digest"])?;
    let decision = str_field(&body, "decision", 16)?;
    if decision != "passed" && decision != "rejected" {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "decision",
            "decision must be passed or rejected",
            "use passed or rejected",
        ));
    }
    let reason = str_field(&body, "reason", 4000)?;
    // RG2: the approval names the exact digest it covers
    let named = str_field(&body, "digest", 64)?;
    if named != digest {
        return Err(one(
            409,
            "DIGEST_MISMATCH",
            "digest",
            "the review names a digest other than the stored revision's",
            "review the stored revision and name its digest",
        ));
    }
    if let Some(prev) = tx.query_opt(
        "SELECT decision, reason, reviewer FROM semantic_reviews WHERE sprint_id = $1 AND revision = $2",
        &[&id, &rev],
    )? {
        let same = prev.get::<_, &str>(0) == decision
            && prev.get::<_, &str>(1) == reason
            && prev.get::<_, &str>(2) == who.name;
        return if same {
            ok(200, json!({"sprint_id": id, "revision": rev, "digest": digest, "decision": decision}))
        } else {
            Err(one(409, "EVENT_CONTENT_CONFLICT", "decision",
                    "this revision already has a semantic review", "a revision is reviewed once"))
        };
    }
    if latest_revision(tx, id)?.is_some_and(|l| l > rev) {
        return Err(one(
            409,
            "REVISION_SUPERSEDED",
            "revision",
            "a newer revision is active",
            "review the latest revision",
        ));
    }
    tx.execute(
        "INSERT INTO semantic_reviews (sprint_id, revision, digest, decision, reason, reviewer)
         VALUES ($1,$2,$3,$4,$5,$6)",
        &[&id, &rev, &digest, &decision, &reason, &who.name],
    )?;
    let state = if decision == "passed" {
        "semantic_review_passed"
    } else {
        "semantic_review_rejected"
    };
    tx.execute(
        "UPDATE sprint_revisions SET review_state = $3 WHERE sprint_id = $1 AND revision = $2",
        &[&id, &rev, &state],
    )?;
    history(
        tx,
        &who.name,
        "sprint.semantic_review",
        &json!({"sprint_id": id, "revision": rev, "digest": digest, "decision": decision}),
    )?;
    ok(
        201,
        json!({"sprint_id": id, "revision": rev, "digest": digest, "decision": decision}),
    )
}

// ---------------------------------------------------------------------------
// Read model
// ---------------------------------------------------------------------------

fn sprint_state(tx: &mut Transaction, who: &Caller, id: &str) -> R {
    need_client_scope(who, "sprints:read", true)?;
    visible_sprint(tx, who, id)?;
    let rev = latest_revision(tx, id)?.unwrap_or(1);
    let verified: Vec<Value> = tx
        .query(
            "SELECT report FROM (
               SELECT DISTINCT ON (task_id) report, sprint_seq FROM events
               WHERE sprint_id = $1 AND evidence_level = 'verified'
               ORDER BY task_id, sprint_seq DESC) t ORDER BY sprint_seq",
            &[&id],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect();
    // claims stored as current (late claims are history, not state); still claims, not verdicts
    let claims: Vec<Value> = tx
        .query(
            "SELECT report FROM (
               SELECT DISTINCT ON (task_id, scope_role) report, sprint_seq FROM events
               WHERE sprint_id = $1 AND evidence_level = 'claim' AND disposition = 'current'
               ORDER BY task_id, scope_role, sprint_seq DESC) t ORDER BY sprint_seq",
            &[&id],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let last: Option<String> = tx
        .query_one(
            "SELECT iso(max(stored_at)) FROM events WHERE sprint_id = $1",
            &[&id],
        )?
        .get(0);
    // delivery of the latest verified report; "unknown" when there is none yet
    let delivery: Option<String> = tx
        .query_opt(
            "SELECT o.state FROM outbox o JOIN events e
               ON e.sprint_id = o.sprint_id AND e.sprint_seq = o.sprint_seq
             WHERE e.sprint_id = $1 AND e.evidence_level = 'verified'
             ORDER BY e.sprint_seq DESC LIMIT 1",
            &[&id],
        )?
        .map(|r| r.get(0));
    ok(
        200,
        json!({"sprint_id": id, "sprint_revision": rev, "verified": verified,
                   "latest_claims": claims, "last_observed_at": last,
                   "delivery": delivery.unwrap_or_else(|| "unknown".into())}),
    )
}

fn list_events(tx: &mut Transaction, who: &Caller, id: &str, req: &Req) -> R {
    need_client_scope(who, "sprints:read", true)?;
    visible_sprint(tx, who, id)?;
    let limit = match query_param(req.query, "limit") {
        None => 50,
        Some(s) => match s.parse::<i64>() {
            Ok(n) if (1..=200).contains(&n) => n,
            _ => {
                return Err(one(
                    400,
                    "SCHEMA_VIOLATION",
                    "limit",
                    "limit must be 1..200",
                    "use a limit between 1 and 200",
                ));
            }
        },
    };
    let max: i64 = tx
        .query_one(
            "SELECT coalesce(max(sprint_seq), 0) FROM events WHERE sprint_id = $1",
            &[&id],
        )?
        .get(0);
    let after = match query_param(req.query, "cursor") {
        None => 0,
        Some(c) => match c.strip_prefix('s').and_then(|n| n.parse::<i64>().ok()) {
            Some(n) if (0..=max).contains(&n) && c == format!("s{n}") => n,
            _ => {
                return Err(one(
                    400,
                    "INVALID_CURSOR",
                    "cursor",
                    "cursor was not issued by this server",
                    "use next_cursor from a previous page",
                ));
            }
        },
    };
    let rows = tx.query(
        "SELECT sprint_seq, report FROM events WHERE sprint_id = $1 AND sprint_seq > $2
         ORDER BY sprint_seq LIMIT $3",
        &[&id, &after, &limit],
    )?;
    let last = rows.last().map_or(after, |r| r.get::<_, i64>(0));
    let events: Vec<Value> = rows.iter().map(|r| r.get(1)).collect();
    ok(
        200,
        json!({"events": events, "next_cursor": format!("s{last}"), "has_more": last < max}),
    )
}

// ---------------------------------------------------------------------------
// Agent paths (01-08 rules from ieo-report, persisted)
// ---------------------------------------------------------------------------

fn load_book(tx: &mut Transaction, repo: &str, role: &str) -> Result<AssignmentBook, Resp> {
    let issued = tx
        .query(
            "SELECT assignment_id, generation, target_sha FROM assignments
             WHERE repo_id = $1 AND role = $2 ORDER BY issued_at",
            &[&repo, &role],
        )?
        .iter()
        .map(|r| IssuedAssignment {
            assignment_id: r.get(0),
            generation: u64::try_from(r.get::<_, i64>(1)).unwrap_or_default(),
            target_sha: r.get(2),
        })
        .collect();
    let current = tx
        .query_opt(
            "SELECT assignment_id, generation FROM assignment_heads WHERE repo_id = $1 AND role = $2",
            &[&repo, &role],
        )?
        .map(|r| (r.get(0), u64::try_from(r.get::<_, i64>(1)).unwrap_or_default()));
    Ok(AssignmentBook { issued, current })
}

fn agent_identity(who: &Caller, scope: &str) -> Result<(String, String), Resp> {
    match (who.kind, &who.role) {
        (Kind::Agent, Some(role)) if who.has(scope) => Ok((
            who.repos.iter().next().cloned().unwrap_or_default(),
            role.clone(),
        )),
        (Kind::Agent, Some(_)) => Err(problem(
            403,
            "SCOPE_DENIED",
            &format!("scope `{scope}` required"),
        )),
        _ => Err(problem(
            403,
            "ROLE_MISMATCH",
            "only role agent credentials submit here",
        )),
    }
}

fn receipt(row: &postgres::Row, head: Option<&(String, u64)>) -> Value {
    let disposition: String = row.get("disposition");
    let aid: String = row.get("assignment_id");
    let generation = u64::try_from(row.get::<_, i64>("generation")).unwrap_or_default();
    // re-evaluated at response time (PR #10 N3): current only while that (assignment,
    // generation) is still the head
    let counts =
        disposition == "current" && head.is_some_and(|(a, g)| *a == aid && *g == generation);
    json!({"event_id": row.get::<_, String>("event_id"),
           "event_revision": row.get::<_, i64>("event_revision"),
           "stored_at": row.get::<_, String>("stored_at"), "evidence_level": "claim",
           "disposition": disposition, "assignment_id": aid, "generation": generation,
           "attributed_target_sha": row.get::<_, String>("attributed_sha"),
           "counts_for_current_judgment": counts})
}

fn stored_claim(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    body: &Value,
) -> Result<Option<postgres::Row>, Resp> {
    let (Some(eid), Some(erev)) = (body["event_id"].as_str(), body["event_revision"].as_i64())
    else {
        return Ok(None);
    };
    Ok(tx.query_opt(
        "SELECT report, event_id, event_revision, iso(stored_at) AS stored_at, disposition,
                assignment_id, generation, attributed_sha
         FROM events WHERE scope_repo = $1 AND scope_role = $2 AND event_id = $3 AND event_revision = $4",
        &[&repo, &role, &eid, &erev],
    )?)
}

fn submit_report(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    let (repo, role) = agent_identity(who, "agent:report")?;
    let body = parse_body(req.body)?;
    lock(tx, &format!("scope:{repo}:{role}"))?;
    let book = load_book(tx, &repo, &role)?;
    let stored = stored_claim(tx, &repo, &role, &body)?;
    let stored_report: Option<Value> = stored.as_ref().map(|r| r.get("report"));
    let disposition =
        authorize_claim(&body, &who.principal(), &book, stored_report.as_ref()).map_err(denied)?;
    let (disp, aid, generation, sha) = match disposition {
        ClaimDisposition::Replay => {
            let Some(row) = stored else {
                return Err(problem(500, "INTERNAL", "replay without a stored row"));
            };
            return ok(200, receipt(&row, book.current.as_ref()));
        }
        ClaimDisposition::Current { target_sha } => {
            let (a, g) = book.current.clone().unwrap_or_default();
            ("current", a, g, target_sha)
        }
        ClaimDisposition::PreservedLate {
            assignment_id,
            generation,
            target_sha,
        } => ("preserved_late", assignment_id, generation, target_sha),
    };
    let rs = validate_report(&body);
    if !rs.is_empty() {
        let status = rs.iter().map(|r| r.code.http_status()).min().unwrap_or(400);
        return Err(rejected(
            status,
            rs.iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
        ));
    }
    // the claim must name the sprint/revision/task the coordinator recorded for that assignment
    let a = tx.query_one(
        "SELECT sprint_id, sprint_revision, task_id FROM assignments
         WHERE assignment_id = $1 AND generation = $2",
        &[&aid, &i64_of(generation)],
    )?;
    for (field, recorded) in [
        ("sprint_id", json!(a.get::<_, String>(0))),
        ("sprint_revision", json!(a.get::<_, i64>(1))),
        ("task_id", json!(a.get::<_, String>(2))),
    ] {
        if body[field] != recorded {
            return Err(one(
                422,
                "REPORT_SEMANTIC_VIOLATION",
                field,
                "the claim names a different sprint/revision/task than its assignment",
                "use the values recorded for the assignment",
            ));
        }
    }
    let sprint_id: String = a.get(0);
    insert_event(
        tx,
        &NewEvent {
            sprint_id: &sprint_id,
            scope_repo: &repo,
            scope_role: &role,
            report: &body,
            disposition: Some(disp),
            assignment: Some((&aid, generation)),
            attributed_sha: Some(&sha),
            policy_digest: None,
        },
    )?;
    history(
        tx,
        &who.name,
        "claim.stored",
        &json!({"event_id": body["event_id"], "event_revision": body["event_revision"],
                    "disposition": disp, "assignment_id": aid, "generation": generation}),
    )?;
    let row = stored_claim(tx, &repo, &role, &body)?
        .ok_or_else(|| problem(500, "INTERNAL", "stored claim not readable"))?;
    ok(202, receipt(&row, book.current.as_ref()))
}

fn validate_repro(v: &Value) -> Result<(), Resp> {
    only_keys(
        v,
        &[
            "request_id",
            "repo_id",
            "assignment_id",
            "generation",
            "target_sha",
            "bundle_digest",
            "command",
            "expected",
            "timeout_seconds",
        ],
    )?;
    let id = str_field(v, "request_id", 128)?;
    if id.len() < 8 {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "request_id",
            "request_id is 8-128 bytes",
            "use a longer request_id",
        ));
    }
    str_field(v, "repo_id", 256)?;
    str_field(v, "assignment_id", 128)?;
    u64_field(v, "generation")?;
    if !is_sha(str_field(v, "target_sha", 64)?) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "target_sha",
            "not a lowercase hex SHA",
            "fix target_sha",
        ));
    }
    let bd = str_field(v, "bundle_digest", 64)?;
    if bd.len() != 64 || !is_sha(bd) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "bundle_digest",
            "not a SHA-256 hex",
            "fix bundle_digest",
        ));
    }
    let cmd_ok = v["command"].as_array().is_some_and(|a| {
        !a.is_empty() && a.iter().all(|x| x.as_str().is_some_and(|s| !s.is_empty()))
    });
    if !cmd_ok {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "command",
            "command is a non-empty list of strings",
            "fix command",
        ));
    }
    str_field(v, "expected", 4000)?;
    if !v["timeout_seconds"].is_null()
        && !v["timeout_seconds"]
            .as_u64()
            .is_some_and(|t| (1..=3600).contains(&t))
    {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "timeout_seconds",
            "timeout_seconds is 1..3600",
            "fix timeout_seconds",
        ));
    }
    Ok(())
}

fn repro_status(row: &postgres::Row) -> Value {
    let result: Option<Value> = row.get("result");
    let result = result.unwrap_or(Value::Null);
    let key: Option<String> = row.get("run_key");
    let logs: Vec<_> = ["stdout", "stderr"]
        .into_iter()
        .filter_map(|stream| {
            result[stream].as_str()?;
            Some(format!("/v0/agent/repro-logs/{}/{stream}", key.as_deref()?))
        })
        .collect();
    json!({"request_id": row.get::<_, String>("request_id"), "state": row.get::<_, String>("state"),
           "target_sha": row.get::<_, String>("target_sha"), "runner_image_digest": result["runner_image_digest"],
           "exit_code": result["exit_code"], "raw_log_refs": logs, "finished_at": result["finished_at"]})
}

fn submit_repro(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    let (repo, role) = agent_identity(who, "agent:repro")?;
    let body = parse_body(req.body)?;
    lock(tx, &format!("scope:{repo}:{role}"))?;
    let book = load_book(tx, &repo, &role)?;
    let stored = match body["request_id"].as_str() {
        Some(id) => tx.query_opt(
            "SELECT request_id, state, target_sha, body, result, run_key FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
            &[&repo, &id],
        )?,
        None => None,
    };
    let stored_body: Option<Value> = stored.as_ref().map(|r| r.get("body"));
    let disposition =
        authorize_repro(&body, &who.principal(), &book, stored_body.as_ref()).map_err(denied)?;
    match disposition {
        ReproDisposition::Replay => {
            let Some(row) = stored else {
                return Err(problem(500, "INTERNAL", "replay without a stored row"));
            };
            ok(200, repro_status(&row))
        }
        ReproDisposition::Queue { target_sha } => {
            validate_repro(&body)?;
            let id = body["request_id"].as_str().unwrap_or_default();
            tx.execute(
                "INSERT INTO repro_requests (repo_id, request_id, body, state, target_sha)
                 VALUES ($1,$2,$3,'queued',$4)",
                &[&repo, &id, &body, &target_sha],
            )?;
            history(
                tx,
                &who.name,
                "repro.queued",
                &json!({"request_id": id, "target_sha": target_sha}),
            )?;
            let row = tx.query_one(
                "SELECT request_id, state, target_sha, result, run_key FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
                &[&repo, &id],
            )?;
            ok(202, repro_status(&row))
        }
    }
}

fn get_repro(tx: &mut Transaction, who: &Caller, id: &str, receipt: bool) -> R {
    match (who.kind, who.role.as_deref()) {
        (Kind::Agent, Some("guardrail")) if who.has("agent:repro") => {}
        (Kind::Agent, _) => {
            return Err(problem(403, "ROLE_MISMATCH", "guardrail credentials only"));
        }
        _ => {
            return Err(problem(
                403,
                "REPO_SCOPE_DENIED",
                "guardrail credentials only",
            ));
        }
    }
    let repo = who.repos.iter().next().cloned().unwrap_or_default();
    match tx.query_opt(
        "SELECT request_id, state, target_sha, result, run_key, body FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
        &[&repo, &id],
    )? {
        Some(row) => {
            let mut status = repro_status(&row);
            if receipt {
                let request: Value = row.get("body");
                status["request_sha256"] = json!(ieo_supervisor::central::sha256(
                    &serde_json::to_vec(&request).expect("valid stored JSON"),
                ));
            }
            ok(200, status)
        },
        None => Err(problem(404, "NOT_FOUND", "unknown request")),
    }
}

fn repro_log(tx: &mut Transaction, who: &Caller, key: &str, stream: &str) -> R {
    let (repo, role) = agent_identity(who, "agent:repro")?;
    if role != "guardrail" {
        return Err(problem(403, "ROLE_MISMATCH", "guardrail credentials only"));
    }
    if !matches!(stream, "stdout" | "stderr") {
        return Err(problem(404, "NOT_FOUND", "unknown log stream"));
    }
    let row = tx.query_opt(
        "SELECT result->>$3 FROM repro_requests WHERE repo_id=$1 AND run_key=$2",
        &[&repo, &key, &stream],
    )?;
    match row.and_then(|row| row.get::<_, Option<String>>(0)) {
        Some(log) => ok(200, json!({"encoding":"hex","content":log})),
        None => Err(problem(404, "NOT_FOUND", "unknown log stream")),
    }
}

// ---------------------------------------------------------------------------
// Assignments: ownership head, permission generation, prerequisites, attempts
// ---------------------------------------------------------------------------

pub(crate) fn find_task<'a>(spec: &'a Value, task_id: &str) -> Option<&'a Value> {
    spec["tasks"]
        .as_array()?
        .iter()
        .find(|t| t["task_id"] == task_id)
}

/// The assignment-level limit keys (02-05 step 5, PLAN §3.2 step 5).
const LIMIT_KEYS: [&str; 2] = ["elapsed_seconds", "total_tokens"];

/// The optional `limits` of an assignment request: absent or null, or an object with
/// `elapsed_seconds` and/or `total_tokens` (each null or an integer ≥ 1).
fn assignment_limits_field(v: &Value) -> Result<Value, Resp> {
    if v.is_null() {
        return Ok(json!({}));
    }
    let bad = || {
        one(
            400,
            "SCHEMA_VIOLATION",
            "limits",
            "limits is null or {elapsed_seconds, total_tokens}, each null or an integer ≥ 1",
            "fix limits",
        )
    };
    let m = v.as_object().ok_or_else(bad)?;
    if m.keys().any(|k| !LIMIT_KEYS.contains(&k.as_str()))
        || m.values()
            .any(|x| !(x.is_null() || x.as_u64().is_some_and(|n| n >= 1)))
    {
        return Err(bad());
    }
    Ok(v.clone())
}

/// The assignment's limits: each asked value, else the revision's. An asked value above the
/// revision's, or where the revision has none, is 422 `LIMITS_ABOVE_SPEC` (the PoC sets no
/// numbers of its own: policy `numeric_values`).
fn effective_limits(spec: &Value, asked: &Value) -> Result<Value, Resp> {
    let mut out = json!({});
    for k in LIMIT_KEYS {
        let from_spec = spec["limits"][k].as_u64();
        out[k] = match (asked[k].as_u64(), from_spec) {
            (None, s) => json!(s),
            (Some(a), Some(s)) if a <= s => json!(a),
            (Some(a), s) => {
                return Err(one(
                    422,
                    "LIMITS_ABOVE_SPEC",
                    &format!("limits.{k}"),
                    &format!(
                        "{k} {a} is above the revision's {}",
                        s.map_or("unset value".to_string(), |s| s.to_string())
                    ),
                    "ask for at most the revision's limit",
                ));
            }
        };
    }
    Ok(out)
}

fn assign(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "coordinator:assign", false)?;
    let body = parse_body(req.body)?;
    let repo = str_field(&body, "repo_id", 256)?.to_string();
    if !who.in_repo(&repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    issue_assignment(tx, ctx, &who.name, Some(who), body)
}

pub(crate) fn issue_assignment(
    tx: &mut Transaction,
    ctx: &Ctx,
    issuer: &str,
    guardrail_actor: Option<&Caller>,
    body: Value,
) -> R {
    let repo = str_field(&body, "repo_id", 256)?.to_owned();
    only_keys(
        &body,
        &[
            "repo_id",
            "role",
            "assignment_id",
            "generation",
            "sprint_id",
            "sprint_revision",
            "task_id",
            "target_sha",
            "expected_current",
            "limits",
            "branch_ref",
        ],
    )?;
    let asked_limits = assignment_limits_field(&body["limits"])?;
    // the branch the assignment's work is pushed to (02-05 step 7): the GitHub head of this
    // branch is the final basis of a development resume; optional, absent → not observed
    if !body["branch_ref"].is_null() {
        branch_ref_of(&body, "branch_ref")?;
    }
    let role = str_field(&body, "role", 16)?.to_string();
    if role != "development" && role != "guardrail" {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "role",
            "role is development or guardrail",
            "fix role",
        ));
    }
    let aid = str_field(&body, "assignment_id", 128)?.to_string();
    let generation = u64_field(&body, "generation")?;
    let sprint_id = str_field(&body, "sprint_id", 256)?.to_string();
    let sprint_rev = i64_of(u64_field(&body, "sprint_revision")?);
    let task_id = str_field(&body, "task_id", 32)?.to_string();
    let sha = str_field(&body, "target_sha", 64)?.to_string();
    if !is_sha(&sha) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "target_sha",
            "not a lowercase hex SHA",
            "fix target_sha",
        ));
    }
    let expected = match &body["expected_current"] {
        Value::Null => None,
        v @ Value::Object(_) => Some((
            str_field(v, "assignment_id", 128)?.to_string(),
            u64_field(v, "generation")?,
        )),
        _ => {
            return Err(one(
                400,
                "SCHEMA_VIOLATION",
                "expected_current",
                "expected_current is null or {assignment_id, generation}",
                "name the head you expect to replace",
            ));
        }
    };
    if role == "guardrail" {
        lock(tx, &format!("scope:{repo}:development"))?;
    }
    lock(tx, &format!("scope:{repo}:{role}"))?;
    // the registry lock keeps the revision checks below stable until commit (review 1e98f17 F5)
    // and serializes all assignments, so one assignment_id raced by two roles (F6) cannot
    // both pass the unused-id check. Order: scope → registry.
    lock(tx, "registry")?;
    // same (assignment, generation) again: historical re-query, never a re-activation
    if let Some(row) = tx.query_opt(
        "SELECT request FROM assignments WHERE assignment_id = $1 AND generation = $2",
        &[&aid, &i64_of(generation)],
    )? {
        let stored: Value = row.get(0);
        return if stored == body {
            let book = load_book(tx, &repo, &role)?;
            let current = book.current == Some((aid.clone(), generation));
            ok(
                200,
                json!({"outcome": "already_issued", "assignment_id": aid, "generation": generation,
                           "is_current": current}),
            )
        } else {
            Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "assignment_id",
                "this assignment/generation was issued with different content",
                "issue a new generation",
            ))
        };
    }
    // the sprint revision must be active, executable and semantically reviewed
    let Some(rev_row) = tx.query_opt(
        "SELECT repo_id, mode, review_state, spec FROM sprint_revisions WHERE sprint_id = $1 AND revision = $2",
        &[&sprint_id, &sprint_rev],
    )?
    else {
        return Err(one(422, "UNKNOWN_SPRINT_REVISION", "sprint_revision", "no such registered revision",
                       "register the revision first"));
    };
    if rev_row.get::<_, &str>(0) != repo {
        return Err(one(
            422,
            "REPO_MISMATCH",
            "repo_id",
            "the revision targets another repo",
            "assign within the revision's repo",
        ));
    }
    // a pinned continuation is bounded by the latest revision's attempt ceiling (review b8b619a R3)
    let mut ceiling_from_latest: Option<Value> = None;
    if let Some(latest) = latest_revision(tx, &sprint_id)?
        && latest > sprint_rev
    {
        // X-01: a running assignment stays on the revision it started from. A later revision
        // (e.g. a re-imported Notion task) never changes it; only its own next generation may
        // continue on that pinned revision. New assignments need the latest revision.
        let pinned = generation > 0
            && tx
                .query_opt(
                    "SELECT 1 FROM assignments WHERE assignment_id = $1 AND generation = $2
                       AND sprint_id = $3 AND sprint_revision = $4",
                    &[&aid, &i64_of(generation - 1), &sprint_id, &sprint_rev],
                )?
                .is_some();
        if !pinned {
            return Err(one(
                409,
                "REVISION_SUPERSEDED",
                "sprint_revision",
                "a newer revision is active",
                "assign from the latest revision",
            ));
        }
        // the ceiling comes from the newest revision that could itself run (executable and
        // semantically reviewed); a draft — e.g. a re-imported Notion task — changes nothing for
        // a running execution (review 38b671b S1)
        let _ = latest;
        if let Some(r) = tx.query_opt(
            "SELECT spec FROM sprint_revisions WHERE sprint_id = $1 AND revision > $2
               AND mode = 'executable' AND review_state = 'semantic_review_passed'
             ORDER BY revision DESC LIMIT 1",
            &[&sprint_id, &sprint_rev],
        )? {
            let l: Value = r.get(0);
            ceiling_from_latest = Some(l["limits"]["correction_attempts"].clone());
        }
    }
    if rev_row.get::<_, &str>(1) != "executable" {
        return Err(one(
            422,
            "UNBOUND_DRAFT",
            "sprint_revision",
            "the revision is registered as draft only",
            "register it with mode=executable",
        ));
    }
    if rev_row.get::<_, &str>(2) != "semantic_review_passed" {
        return Err(one(
            422,
            "SEMANTIC_REVIEW_NOT_PASSED",
            "sprint_revision",
            "format checks passed but the independent semantic review has not",
            "obtain a passed semantic review",
        ));
    }
    let spec: Value = rev_row.get(3);
    // C2: execution is allowed only when the server's Notion records allow it (review 9a421b7 F1)
    notion_gate(tx, &spec, &sprint_id, &repo)?;
    let Some(task) = find_task(&spec, &task_id) else {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "task_id",
            "the task is not in this revision",
            "fix task_id",
        ));
    };
    // 02-05 step 5: the assignment's limits, never above the revision's (no new numbers)
    let limits = effective_limits(&spec, &asked_limits)?;
    // ownership: compare-and-swap on the (repo, role) head
    let book = load_book(tx, &repo, &role)?;
    if book.current != expected {
        return Err(one(
            409,
            "OWNERSHIP_CONFLICT",
            "expected_current",
            &format!("the current head is {:?}", book.current),
            "re-read the head and retry with it as expected_current",
        ));
    }
    if generation == 0 {
        if book.issued.iter().any(|a| a.assignment_id == aid)
            || tx
                .query_opt(
                    "SELECT 1 FROM assignments WHERE assignment_id = $1",
                    &[&aid],
                )?
                .is_some()
        {
            return Err(one(
                409,
                "OWNERSHIP_CONFLICT",
                "assignment_id",
                "assignment_id already used; a new generation must follow the head",
                "use a fresh assignment_id or the next generation",
            ));
        }
    } else {
        let follows = book.current == Some((aid.clone(), generation - 1));
        if !follows {
            return Err(one(
                409,
                "OWNERSHIP_CONFLICT",
                "generation",
                "a generation must directly follow the current head of the same assignment",
                "use the head's generation + 1",
            ));
        }
        let prev = tx.query_one(
            "SELECT sprint_id, task_id FROM assignments WHERE assignment_id = $1 AND generation = $2",
            &[&aid, &i64_of(generation - 1)],
        )?;
        if prev.get::<_, &str>(0) != sprint_id || prev.get::<_, &str>(1) != task_id {
            return Err(one(
                422,
                "REPORT_SEMANTIC_VIOLATION",
                "task_id",
                "a new generation keeps the assignment's sprint and task",
                "issue a new assignment_id for another task",
            ));
        }
    }
    // G1: prerequisites are met only by verified verdicts under this policy
    let mut unmet = Vec::new();
    for p in task["prerequisites"].as_array().into_iter().flatten() {
        let pid = p["task_id"].as_str().unwrap_or_default();
        let need_accepted = p["required_state"] == "accepted";
        // the verdict must have judged the prerequisite as it is defined *now*: the fields a verdict
        // depends on (outputs, acceptance, exit_condition, verification, requirement_ids) must be
        // equal in the verdict's revision and the sprint's latest revision (review 0fa839d N3).
        // Labels and bookkeeping (status, evidence, estimate, references) never void a verdict
        // (review 4bc8081 R1). A task missing from either revision compares as NULL: unmet.
        let met: bool = tx
            .query_one(
                "SELECT EXISTS (
                   SELECT 1 FROM judgments j
                   JOIN sprint_revisions jr ON jr.sprint_id = j.sprint_id AND jr.revision = j.sprint_revision
                   JOIN LATERAL (SELECT spec FROM sprint_revisions l WHERE l.sprint_id = j.sprint_id
                                 ORDER BY l.revision DESC LIMIT 1) lr ON true
                   WHERE j.task_id = $1 AND j.policy_digest = $2 AND ($3 = false OR j.outcome = 'accepted')
                     AND (SELECT jsonb_build_object('outputs', t->'outputs', 'acceptance', t->'acceptance', 'exit_condition', t->'exit_condition', 'verification', t->'verification', 'requirement_ids', t->'requirement_ids')
                            FROM jsonb_array_elements(jr.spec->'tasks') t WHERE t->>'task_id' = $1)
                       = (SELECT jsonb_build_object('outputs', t->'outputs', 'acceptance', t->'acceptance', 'exit_condition', t->'exit_condition', 'verification', t->'verification', 'requirement_ids', t->'requirement_ids')
                            FROM jsonb_array_elements(lr.spec->'tasks') t WHERE t->>'task_id' = $1))",
                &[&pid, &ctx.policy.digest, &need_accepted],
            )?
            .get(0);
        if !met {
            unmet.push(rej(
                "PREREQUISITE_UNMET",
                &format!("prerequisites[{pid}]"),
                &format!("no verified verdict for {pid} ({})", p["required_state"]),
                "wait for the prerequisite's verified verdict",
            ));
        }
    }
    if !unmet.is_empty() {
        return Err(rejected(409, unmet));
    }
    // G4/G5: cumulative attempts per (sprint, task, role)
    let attempts = tx.query_opt(
        "SELECT issued FROM task_attempts WHERE sprint_id = $1 AND task_id = $2 AND role = $3",
        &[&sprint_id, &task_id, &role],
    )?;
    let issued = attempts.map_or(0, |r| r.get::<_, i64>(0));
    let ceiling_value = ceiling_from_latest
        .clone()
        .unwrap_or_else(|| spec["limits"]["correction_attempts"].clone());
    let Some(ceiling) = ceiling_value.as_i64() else {
        return Err(one(
            422,
            "UNBOUND_DRAFT",
            "limits.correction_attempts",
            "the attempt ceiling is not bound",
            "bind limits before execution",
        ));
    };
    if issued > ceiling {
        // one decision request per (task, role, ceiling): a ceiling raised by a newer
        // revision and reached again asks again (review 1e98f17 F6)
        // asked already at this ceiling (any earlier time, not only the last one: review 0fa839d N1)
        let asked: bool = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM events WHERE scope_repo = $1 AND scope_role = 'coordinator'
                   AND event_id = $2)",
                &[&repo, &format!("limit-{sprint_id}-{task_id}-{role}-c{ceiling}")],
            )?
            .get(0);
        if asked {
            return Err(one(
                409,
                "LIMIT_REACHED",
                "limits.correction_attempts",
                "the attempt ceiling was reached; a user decision is pending",
                "record a user decision",
            ));
        }
        let decision = limit_decision_request(
            tx, &spec, &sprint_id, sprint_rev, &task_id, &repo, &role, ceiling,
        )?;
        let seq = insert_event(
            tx,
            &NewEvent {
                sprint_id: &sprint_id,
                scope_repo: &repo,
                scope_role: "coordinator",
                report: &decision,
                disposition: None,
                assignment: None,
                attributed_sha: None,
                policy_digest: Some(&ctx.policy.digest),
            },
        )?;
        tx.execute(
            "INSERT INTO outbox (sprint_id, sprint_seq, destination) VALUES ($1,$2,'client')",
            &[&sprint_id, &seq],
        )?;
        tx.execute(
            "UPDATE task_attempts SET limit_reached = true, reached_ceiling = $4
             WHERE sprint_id = $1 AND task_id = $2 AND role = $3",
            &[&sprint_id, &task_id, &role, &ceiling],
        )?;
        history(
            tx,
            issuer,
            "task.limit_reached",
            &json!({"sprint_id": sprint_id, "task_id": task_id, "role": role, "issued": issued,
                        "ceiling": ceiling}),
        )?;
        return ok(
            202,
            json!({"outcome": "limit_reached", "decision_event_id": decision["event_id"],
                              "attempts_issued": issued, "ceiling": ceiling}),
        );
    }
    let eligible_at = if guardrail_actor.is_none() {
        Some(
            tx.query_one("SELECT iso(clock_timestamp())", &[])?
                .get::<_, String>(0),
        )
    } else {
        None
    };
    tx.execute(
        "INSERT INTO assignments (assignment_id, generation, repo_id, role, sprint_id, sprint_revision,
            task_id, target_sha, issued_by, policy_digest, request, limits)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        &[&aid, &i64_of(generation), &repo, &role, &sprint_id, &sprint_rev, &task_id, &sha,
          &issuer, &ctx.policy.digest, &body, &limits],
    )?;
    tx.execute(
        "INSERT INTO assignment_heads (repo_id, role, assignment_id, generation) VALUES ($1,$2,$3,$4)
         ON CONFLICT (repo_id, role) DO UPDATE SET assignment_id = $3, generation = $4",
        &[&repo, &role, &aid, &i64_of(generation)],
    )?;
    tx.execute(
        "INSERT INTO task_attempts (sprint_id, task_id, role, issued) VALUES ($1,$2,$3,1)
         ON CONFLICT (sprint_id, task_id, role) DO UPDATE SET issued = task_attempts.issued + 1",
        &[&sprint_id, &task_id, &role],
    )?;
    if role == "guardrail" {
        let actor = guardrail_actor.ok_or_else(|| {
            problem(
                403,
                "SCOPE_DENIED",
                "Guardrail assignment requires an explicit caller",
            )
        })?;
        crate::review_store::reserve_replacement(tx, actor, &body)?;
    }
    history(
        tx,
        issuer,
        "assignment.issued",
        &json!({"assignment_id": aid, "generation": generation, "repo_id": repo, "role": role,
                    "sprint_id": sprint_id, "task_id": task_id, "target_sha": sha,
                    "replaced": book.current}),
    )?;
    let mut issued = json!({"outcome": "issued", "assignment_id": aid, "generation": generation,
                   "repo_id": repo, "role": role, "sprint_id": sprint_id,
                   "sprint_revision": sprint_rev, "task_id": task_id, "target_sha": sha,
                   "policy_digest": ctx.policy.digest, "attempts_issued": issued + 1,
                   "ceiling": ceiling});
    if let Some(time) = eligible_at {
        issued["eligible_at"] = json!(time);
    }
    ok(201, issued)
}

fn base_report(
    spec: &Value,
    sprint_id: &str,
    sprint_rev: i64,
    task_id: &str,
    repo: &str,
    observed_at: &str,
) -> Value {
    let sched = &spec["schedule"];
    let deadline_state = sched["deadline_state"].as_str().unwrap_or("undecided");
    let status = if deadline_state == "none" {
        "no_deadline"
    } else {
        "unassessed"
    };
    json!({
        "schema_version": "console.report.v0", "contract_revision": 4,
        "event_revision": 1, "sprint_id": sprint_id, "sprint_revision": sprint_rev,
        "task_id": task_id, "repo_id": repo, "assignment_id": null, "generation": null,
        "actor": {"role": "coordinator", "name": "ieo-coord"},
        "observed_at": observed_at, "channel": "coordinator_to_client", "evidence_level": "verified",
        "reason_code": null, "evidence": [], "unknown_facts": [], "affected_tasks": [],
        "resume_condition": null, "next_check_at": null,
        "schedule": {"deadline_state": deadline_state,
                     "deadline_at": if deadline_state == "set" { sched["deadline_at"].clone() } else { Value::Null },
                     "status": status, "remaining_tasks": null,
                     "remaining_reason": "02-03 records no schedule forecast",
                     "delay_reasons": [], "affected_tasks": [],
                     "forecast": {"state": "unestimated", "completion_range": null, "evidence": [],
                                  "reason": "no measured throughput in the coordinator yet"}}
    })
}

#[allow(clippy::too_many_arguments)]
fn limit_decision_request(
    tx: &mut Transaction,
    spec: &Value,
    sprint_id: &str,
    sprint_rev: i64,
    task_id: &str,
    repo: &str,
    role: &str,
    ceiling: i64,
) -> Result<Value, Resp> {
    let at = now(tx)?;
    let mut r = base_report(spec, sprint_id, sprint_rev, task_id, repo, &at);
    let question = format!(
        "{task_id}의 {role} 시도가 누적 상한 {ceiling}회(교정)에 도달했다. 어떻게 진행할까?"
    );
    let options = [
        (
            "정지 유지",
            "추가 세대를 발급하지 않는다. 현재 담당은 유지된다.",
        ),
        (
            "상한 변경 후 재개",
            "사용자가 새 상한을 승인해 새 명세 revision으로 등록하면 재개한다.",
        ),
    ];
    r["event_id"] = json!(format!("limit-{sprint_id}-{task_id}-{role}-c{ceiling}"));
    r["phase"] = json!("교정");
    r["operational_state"] = json!("사람 결정 대기");
    r["affected_tasks"] = json!([task_id]);
    r["next_action"] = json!("사용자 결정 대기");
    r["next_owner"] = json!("user");
    r["decision"] = json!({"required": true, "question": question,
                           "options": options.iter().map(|(l, i)| format!("{l}: {i}")).collect::<Vec<_>>(),
                           "recommendation": "정지 유지", "deadline_at": null,
                           "default_action": "정지 유지"});
    r["report_type"] = json!("decision_request");
    r["body"] = json!({"question": question,
                       "options": options.iter().map(|(l, i)| json!({"label": l, "impact": i})).collect::<Vec<_>>(),
                       "recommendation": "정지 유지",
                       "rationale": "판정 정책 G4/G5: 상한 도달 시 자동으로 계속하지 않는다.",
                       "deadline_at": null, "no_deadline_reason": "상한 도달 처리는 기한을 두지 않는다(정책 G5).",
                       "default_action": "정지 유지"});
    let problems = validate_report(&r);
    if !problems.is_empty() {
        return Err(problem(
            500,
            "INTERNAL",
            &format!("decision_request invalid: {problems:?}"),
        ));
    }
    Ok(r)
}

// ---------------------------------------------------------------------------
// Evidence ingest (GitHub / execution)
// ---------------------------------------------------------------------------

const CONCLUSIONS: [&str; 7] = [
    "success",
    "failure",
    "cancelled",
    "timed_out",
    "skipped",
    "neutral",
    "action_required",
];

/// A full branch ref (`refs/heads/<name>`, at most 256 bytes, no whitespace or control
/// characters, no `..`): the assignment's branch and a `branch_head`'s ref (02-05 step 7).
pub(crate) fn branch_ref_of<'a>(v: &'a Value, field: &str) -> Result<&'a str, Resp> {
    let r = str_field(v, field, 256)?;
    let name = r.strip_prefix("refs/heads/").unwrap_or_default();
    if name.is_empty()
        || name.contains("..")
        || name.starts_with('/')
        || name.ends_with('/')
        || r.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            field,
            "not a branch ref (refs/heads/<name>)",
            "send the full branch ref",
        ));
    }
    Ok(r)
}

fn validate_evidence(v: &Value) -> Result<(String, String), Resp> {
    let kind = str_field(v, "kind", 32)?.to_string();
    if let Some(pr) = v.get("pull_request")
        && !pr.as_i64().is_some_and(|number| number > 0)
    {
        return Err(problem(
            400,
            "SCHEMA_VIOLATION",
            "pull_request must be a positive integer",
        ));
    }
    let sha_of = |k: &str| -> Result<String, Resp> {
        let s = str_field(v, k, 64)?;
        if is_sha(s) {
            Ok(s.to_string())
        } else {
            Err(one(
                400,
                "SCHEMA_VIOLATION",
                k,
                "not a lowercase hex SHA",
                "fix the SHA",
            ))
        }
    };
    let conclusion = || -> Result<(), Resp> {
        let c = str_field(v, "conclusion", 32)?;
        if CONCLUSIONS.contains(&c) {
            Ok(())
        } else {
            Err(one(
                400,
                "SCHEMA_VIOLATION",
                "conclusion",
                "unknown conclusion",
                "use a GitHub conclusion",
            ))
        }
    };
    fn keys(extra: &[&'static str]) -> Vec<&'static str> {
        ["evidence_id", "repo_id", "kind"]
            .iter()
            .chain(extra)
            .copied()
            .collect()
    }
    let sha = match kind.as_str() {
        "ci_run" | "main_ci" => {
            only_keys(
                v,
                &keys(&["sha", "conclusion", "source_ref", "pull_request"]),
            )?;
            conclusion()?;
            str_field(v, "source_ref", 512)?;
            sha_of("sha")?
        }
        "run_record" => {
            only_keys(v, &keys(&["sha", "source_ref", "record", "pull_request"]))?;
            str_field(v, "source_ref", 512)?;
            if !v["record"]["judgment"]["counts_as_success"].is_boolean()
                || v["record"]["format"].as_str().is_none()
            {
                return Err(one(
                    400,
                    "SCHEMA_VIOLATION",
                    "record",
                    "record must be an ieo-evidence record with judgment.counts_as_success",
                    "send the collector's record",
                ));
            }
            sha_of("sha")?
        }
        "review" => {
            only_keys(
                v,
                &keys(&[
                    "sha",
                    "reviewer",
                    "author",
                    "state",
                    "source_ref",
                    "pull_request",
                ]),
            )?;
            str_field(v, "reviewer", 256)?;
            str_field(v, "author", 256)?;
            str_field(v, "source_ref", 512)?;
            let st = str_field(v, "state", 32)?;
            if !["approved", "changes_requested", "commented", "dismissed"].contains(&st) {
                return Err(one(
                    400,
                    "SCHEMA_VIOLATION",
                    "state",
                    "unknown review state",
                    "use a GitHub review state",
                ));
            }
            sha_of("sha")?
        }
        "pr_review_state" => {
            only_keys(
                v,
                &keys(&["sha", "pull_request", "decision", "reviewers", "source_ref"]),
            )?;
            if v.get("pull_request").is_none() {
                return Err(problem(400, "SCHEMA_VIOLATION", "pull_request is required"));
            }
            str_field(v, "source_ref", 512)?;
            let decision = str_field(v, "decision", 32)?;
            let reviewers = v["reviewers"]
                .as_array()
                .filter(|list| {
                    list.len() <= 128
                        && list.iter().all(|reviewer| {
                            reviewer
                                .as_str()
                                .is_some_and(|name| !name.is_empty() && name.len() <= 256)
                        })
                })
                .ok_or_else(|| problem(400, "SCHEMA_VIOLATION", "invalid PR reviewers"))?;
            if !["APPROVED", "CHANGES_REQUESTED", "REVIEW_REQUIRED"].contains(&decision)
                || (decision == "CHANGES_REQUESTED") == reviewers.is_empty()
            {
                return Err(problem(
                    400,
                    "SCHEMA_VIOLATION",
                    "inconsistent PR review decision",
                ));
            }
            sha_of("sha")?
        }
        "merge" => {
            only_keys(
                v,
                &keys(&["head_sha", "merged_sha", "source_ref", "pull_request"]),
            )?;
            str_field(v, "source_ref", 512)?;
            sha_of("merged_sha")?;
            sha_of("head_sha")?
        }
        // 02-05 step 7: the head of one GitHub branch as the evidence service read it (trusted
        // service input; no live GitHub read in this PoC)
        "branch_head" => {
            only_keys(v, &keys(&["sha", "ref", "source_ref"]))?;
            str_field(v, "source_ref", 512)?;
            branch_ref_of(v, "ref")?;
            sha_of("sha")?
        }
        "output" => {
            only_keys(v, &keys(&["sha", "task_id", "artifact_id", "ref"]))?;
            str_field(v, "task_id", 32)?;
            str_field(v, "artifact_id", 256)?;
            str_field(v, "ref", 1024)?;
            sha_of("sha")?
        }
        "followups" => {
            only_keys(v, &keys(&["sha", "task_id", "accepted", "ref"]))?;
            str_field(v, "task_id", 32)?;
            str_field(v, "ref", 1024)?;
            if !v["accepted"].is_boolean() {
                return Err(one(
                    400,
                    "SCHEMA_VIOLATION",
                    "accepted",
                    "accepted is a boolean",
                    "fix accepted",
                ));
            }
            sha_of("sha")?
        }
        _ => {
            return Err(one(
                400,
                "SCHEMA_VIOLATION",
                "kind",
                "unknown evidence kind",
                "use a listed kind",
            ));
        }
    };
    Ok((kind, sha))
}

fn ingest(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "evidence:ingest", false)?;
    let body = parse_body(req.body)?;
    let repo = str_field(&body, "repo_id", 256)?.to_string();
    if !who.in_repo(&repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    let id = str_field(&body, "evidence_id", 256)?.to_string();
    let (kind, sha) = validate_evidence(&body)?;
    if matches!(
        kind.as_str(),
        "branch_head" | "ci_run" | "run_record" | "review" | "pr_review_state"
    ) {
        lock(tx, &format!("delivery-evidence:{repo}"))?;
    }
    lock(tx, &format!("evidence:{repo}:{id}"))?;
    if let Some(row) = tx.query_opt(
        "SELECT body FROM evidence WHERE repo_id = $1 AND evidence_id = $2",
        &[&repo, &id],
    )? {
        let stored: Value = row.get(0);
        return if stored == body {
            ok(
                200,
                json!({"evidence_id": id, "kind": kind, "sha": sha, "replay": true}),
            )
        } else {
            Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "evidence_id",
                "this evidence_id was stored with different content",
                "use a new evidence_id",
            ))
        };
    }
    tx.execute(
        "INSERT INTO evidence (repo_id, evidence_id, kind, sha, body, ingested_by) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&repo, &id, &kind, &sha, &body, &who.name],
    )?;
    history(
        tx,
        &who.name,
        "evidence.ingested",
        &json!({"evidence_id": id, "kind": kind, "sha": sha}),
    )?;
    ok(
        201,
        json!({"evidence_id": id, "kind": kind, "sha": sha, "replay": false}),
    )
}

// ---------------------------------------------------------------------------
// Verdicts: GitHub/run evidence + current claims → completion facts (policy G1-G7)
// ---------------------------------------------------------------------------

/// Latest row per `source_ref` among (kinds) at `sha`; true when non-empty and all succeed.
pub(crate) fn ci_ok(
    tx: &mut Transaction,
    repo: &str,
    kinds: &[&str],
    sha: &str,
    pull_request: Option<i64>,
    used: &mut Vec<String>,
) -> Result<bool, Resp> {
    let kinds: Vec<String> = kinds.iter().map(|s| s.to_string()).collect();
    let rows = tx.query(
        "SELECT DISTINCT ON (body->>'source_ref') evidence_id, kind, body FROM evidence
         WHERE repo_id = $1 AND kind = ANY($2) AND sha = $3
           AND ($4::bigint IS NULL OR body->>'pull_request' = $4::bigint::text)
         ORDER BY body->>'source_ref', seq DESC",
        &[&repo, &kinds, &sha, &pull_request],
    )?;
    let mut all = !rows.is_empty();
    for r in &rows {
        let kind: &str = r.get(1);
        let body: Value = r.get(2);
        let pass = if kind == "run_record" {
            // the collector's judgment, never exit status alone (02-01)
            body["record"]["judgment"]["counts_as_success"] == true
        } else {
            body["conclusion"] == "success"
        };
        all &= pass;
        used.push(format!("evidence:{}", r.get::<_, String>(0)));
    }
    Ok(all)
}

fn judge(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "judge:verify", false)?;
    if who.kind != Kind::Service {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "verdicts are recorded by a service principal (G3)",
        ));
    }
    let body = parse_body(req.body)?;
    only_keys(&body, &["judgment_id", "repo_id", "sprint_id", "task_id"])?;
    let repo = str_field(&body, "repo_id", 256)?.to_string();
    if !who.in_repo(&repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    let jid = str_field(&body, "judgment_id", 128)?.to_string();
    let sprint_id = str_field(&body, "sprint_id", 256)?.to_string();
    let task_id = str_field(&body, "task_id", 32)?.to_string();
    lock(tx, &format!("judgment:{repo}:{jid}"))?;
    if let Some(row) = tx.query_opt(
        "SELECT request, outcome, head_sha, merged_sha, facts FROM judgments WHERE repo_id = $1 AND judgment_id = $2",
        &[&repo, &jid],
    )? {
        let stored: Value = row.get(0);
        return if stored == body {
            ok(200, json!({"judgment_id": jid, "outcome": row.get::<_, String>(1),
                           "head_sha": row.get::<_, String>(2), "merged_sha": row.get::<_, Option<String>>(3),
                           "facts": row.get::<_, Value>(4), "replay": true}))
        } else {
            Err(one(409, "EVENT_CONTENT_CONFLICT", "judgment_id",
                    "this judgment_id was used for another request", "use a new judgment_id"))
        };
    }
    // stable heads while judging (lock order: development, then guardrail)
    lock(tx, &format!("scope:{repo}:development"))?;
    lock(tx, &format!("scope:{repo}:guardrail"))?;
    let Some(rev) = latest_revision(tx, &sprint_id)? else {
        return Err(one(
            422,
            "UNKNOWN_SPRINT_REVISION",
            "sprint_id",
            "unknown sprint",
            "fix sprint_id",
        ));
    };
    let rev_row = tx.query_one(
        "SELECT repo_id, spec FROM sprint_revisions WHERE sprint_id = $1 AND revision = $2",
        &[&sprint_id, &rev],
    )?;
    if rev_row.get::<_, &str>(0) != repo {
        return Err(one(
            422,
            "REPO_MISMATCH",
            "repo_id",
            "the sprint targets another repo",
            "fix repo_id",
        ));
    }
    let latest_spec: Value = rev_row.get(1);
    if find_task(&latest_spec, &task_id).is_none() {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "task_id",
            "the task is not in the active revision",
            "fix task_id",
        ));
    }
    // G2: use the active development assignment for this task when present.
    // After development moves on, the task's last durable assignment is the
    // head for judgment only. Its history sequence reflects assignment order
    // under the role lock, even when request timestamps arrive out of order.
    let head = tx.query_opt(
        "SELECT a.target_sha, a.assignment_id, a.generation, a.sprint_revision
         FROM assignments a
         LEFT JOIN assignment_heads active ON
             (active.repo_id, active.role, active.assignment_id, active.generation) =
             (a.repo_id, a.role, a.assignment_id, a.generation)
         LEFT JOIN history h ON h.action='assignment.issued'
             AND h.subject->>'repo_id'=a.repo_id AND h.subject->>'role'=a.role
             AND h.subject->>'assignment_id'=a.assignment_id
             AND h.subject->>'generation'=a.generation::text
         WHERE a.repo_id=$1 AND a.role='development' AND a.sprint_id=$2 AND a.task_id=$3
           AND (active.assignment_id IS NOT NULL OR h.seq IS NOT NULL)
         ORDER BY (active.assignment_id IS NOT NULL) DESC, h.seq DESC NULLS LAST
         LIMIT 1",
        &[&repo, &sprint_id, &task_id],
    )?;
    let Some(head) = head else {
        return Err(one(
            409,
            "NO_CURRENT_ASSIGNMENT",
            "task_id",
            "the task has no current development assignment",
            "assign the task first",
        ));
    };
    let head_sha: String = head.get(0);
    let dev_aid: String = head.get(1);
    let dev_gen = u64::try_from(head.get::<_, i64>(2)).unwrap_or_default();
    // the verdict judges the revision the assignment was issued from (RG2; review 1e98f17 F3)
    let rev: i64 = head.get(3);
    let spec: Value = tx
        .query_one(
            "SELECT spec FROM sprint_revisions WHERE sprint_id = $1 AND revision = $2",
            &[&sprint_id, &rev],
        )?
        .get(0);
    let Some(task) = find_task(&spec, &task_id).cloned() else {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "task_id",
            "the task is not in the assigned revision",
            "fix task_id",
        ));
    };
    let mut used = Vec::new();
    let pull_request = crate::review_store::pull_request(tx, &repo, &sprint_id, &task_id)?;
    let ci = ci_ok(
        tx,
        &repo,
        &["ci_run", "run_record"],
        &head_sha,
        pull_request,
        &mut used,
    )?;
    // G3 review inputs (N4): the guardrail claim must be the *current* guardrail
    // (assignment, generation) at judgment time and for this task
    let mut reviews: Vec<(String, String)> = Vec::new();
    let gr = load_book(tx, &repo, "guardrail")?;
    if let Some((ga, gg)) = &gr.current
        && let Some(r) = tx.query_opt(
            "SELECT e.attributed_sha, e.report->'body'->>'verdict', e.event_id
             FROM events e JOIN assignments a ON a.assignment_id = e.assignment_id AND a.generation = e.generation
             WHERE e.scope_repo = $1 AND e.scope_role = 'guardrail' AND e.report_type = 'guardrail_review'
               AND e.disposition = 'current' AND e.assignment_id = $2 AND e.generation = $3
               AND a.sprint_id = $4 AND a.task_id = $5
             ORDER BY e.stored_at DESC, e.sprint_seq DESC LIMIT 1",
            &[&repo, ga, &i64_of(*gg), &sprint_id, &task_id],
        )?
    {
        reviews.push((r.get(0), r.get(1)));
        used.push(format!("claim:{}", r.get::<_, String>(2)));
    }
    // GitHub reviews (review 1e98f17 F2): per independent reviewer, the latest
    // state-changing review *at the head SHA*. A later COMMENTED review does not clear a
    // changes_requested; DISMISSED does. Reviews of other SHAs or PRs never count.
    for r in tx.query(
        "SELECT DISTINCT ON (body->>'reviewer') evidence_id, body->>'state' FROM evidence
         WHERE repo_id = $1 AND kind = 'review' AND sha = $2 AND body->>'reviewer' <> body->>'author'
           AND body->>'state' IN ('approved', 'changes_requested', 'dismissed')
           AND ($3::bigint IS NULL OR body->>'pull_request' = $3::bigint::text)
         ORDER BY body->>'reviewer', seq DESC",
        &[&repo, &head_sha, &pull_request],
    )? {
        let state: String = r.get(1);
        let verdict = match state.as_str() {
            "approved" => "approve",
            "changes_requested" => "reject",
            _ => continue,
        };
        reviews.push((head_sha.clone(), verdict.to_string()));
        used.push(format!("evidence:{}", r.get::<_, String>(0)));
    }
    // independent approvals of this task's earlier SHAs: shown as "not used", never counted
    for r in tx.query(
        "SELECT DISTINCT sha FROM evidence
         WHERE repo_id = $1 AND kind = 'review' AND sha <> $2 AND body->>'state' = 'approved'
           AND body->>'reviewer' <> body->>'author'
           AND sha IN (SELECT target_sha FROM assignments WHERE repo_id = $1 AND role = 'development'
                       AND sprint_id = $3 AND task_id = $4)
         ORDER BY sha",
        &[&repo, &head_sha, &sprint_id, &task_id],
    )? {
        reviews.push((r.get(0), "approve".to_string()));
    }
    let lifecycle_blocked =
        crate::review_store::blocks_judgment(tx, &repo, &sprint_id, &task_id, &head_sha)?;
    let write_unresolved = match pull_request {
        Some(pr) => crate::github_write::has_unresolved(tx, &repo, pr)?,
        None => false,
    };
    let blocked = lifecycle_blocked
        || write_unresolved
        || reviews
            .iter()
            .any(|(s, v)| *s == head_sha && (v == "reject" || v == "needs_confirmation"));
    let review = if blocked {
        // keep the note about earlier-SHA approvals (review 1e98f17 F9)
        let mut stale: Vec<String> = reviews
            .iter()
            .filter(|(s, v)| *s != head_sha && v == "approve")
            .map(|(s, _)| s.clone())
            .collect();
        stale.dedup();
        ReviewCurrency::ReviewNeededOnCurrentSha {
            stale_approvals: stale,
        }
    } else {
        review_currency(&head_sha, &reviews)
    };
    let merge_receipt = tx.query_opt(
        "SELECT result->>'merged_sha', request_id FROM github_write_intents
         WHERE repo_id=$1 AND pull_request=$2 AND request->'candidate'->>'head_sha'=$3
           AND request->'candidate'->>'sprint_id'=$4 AND request->'candidate'->>'task_id'=$5
           AND result->>'outcome'='merged'",
        &[&repo, &pull_request, &head_sha, &sprint_id, &task_id],
    )?;
    let merged: Option<String> = if let Some(row) = merge_receipt {
        used.push(format!("github-write:{}", row.get::<_, String>(1)));
        row.get(0)
    } else {
        tx
        .query_opt(
            "SELECT body->>'merged_sha', evidence_id FROM evidence WHERE repo_id = $1 AND kind = 'merge' AND sha = $2
               AND ($3::bigint IS NULL OR body->>'pull_request' = $3::bigint::text)
             ORDER BY seq DESC LIMIT 1",
            &[&repo, &head_sha, &pull_request],
        )?
        .map(|r| {
            used.push(format!("evidence:{}", r.get::<_, String>(1)));
            r.get(0)
        })
    };
    let main_ci = match &merged {
        Some(m) => ci_ok(tx, &repo, &["main_ci"], m, None, &mut used)?,
        None => false,
    };
    // G7: only required outputs are judged
    let mut excluded = Vec::new();
    let mut outputs_ok = true;
    for o in task["outputs"].as_array().into_iter().flatten() {
        let art = o["artifact_id"].as_str().unwrap_or_default();
        if o["required"] != true {
            excluded.push(format!("optional output {art}"));
            continue;
        }
        let shas: Vec<String> = [Some(head_sha.clone()), merged.clone()]
            .into_iter()
            .flatten()
            .collect();
        let found = tx.query_opt(
            "SELECT evidence_id FROM evidence WHERE repo_id = $1 AND kind = 'output' AND sha = ANY($2)
               AND body->>'task_id' = $3 AND body->>'artifact_id' = $4 ORDER BY seq DESC LIMIT 1",
            &[&repo, &shas, &task_id, &art],
        )?;
        match found {
            Some(r) => used.push(format!("evidence:{}", r.get::<_, String>(0))),
            None => outputs_ok = false,
        }
    }
    let followups = match &merged {
        Some(m) => tx
            .query_opt(
                "SELECT (body->>'accepted')::boolean, evidence_id FROM evidence WHERE repo_id = $1
                   AND kind = 'followups' AND sha = $2 AND body->>'task_id' = $3 ORDER BY seq DESC LIMIT 1",
                &[&repo, m, &task_id],
            )?
            .is_some_and(|r| {
                used.push(format!("evidence:{}", r.get::<_, String>(1)));
                r.get::<_, bool>(0)
            }),
        None => false,
    };
    let facts = CompletionFacts {
        head_sha: head_sha.clone(),
        ci_passed_on_head: ci,
        review: Some(review.clone()),
        merged_sha: merged.clone(),
        main_ci_passed: main_ci,
        required_outputs_present: outputs_ok,
        followups_accepted: followups,
    };
    let mut summary = completion_summary(&facts);
    if matches!(review, ReviewCurrency::ApprovedAtCurrentSha)
        && let Some(pr) = pull_request
        && let Some(observation) = tx.query_opt(
            "SELECT evidence_id, body->>'decision' FROM evidence
             WHERE repo_id=$1 AND kind='pr_review_state'
               AND body->>'pull_request'=$2::bigint::text ORDER BY seq DESC LIMIT 1",
            &[&repo, &pr],
        )?
        && observation.get::<_, String>(1) == "CHANGES_REQUESTED"
    {
        used.push(format!("evidence:{}", observation.get::<_, String>(0)));
        summary.push_str(" · 외부 상태 불일치(PR 수준 변경 요청 미해결)");
    }
    let accepted = ci
        && matches!(review, ReviewCurrency::ApprovedAtCurrentSha)
        && merged.is_some()
        && main_ci
        && outputs_ok
        && followups;
    let outcome = if accepted { "accepted" } else { "not_accepted" };
    let facts_json = json!({"head_sha": head_sha, "ci_passed_on_head": ci, "review": review,
                            "merged_sha": merged, "main_ci_passed": main_ci,
                            "required_outputs_present": outputs_ok, "followups_accepted": followups,
                            "excluded_not_judged": excluded, "inputs": used});
    let at = now(tx)?;
    let mut r = base_report(&spec, &sprint_id, rev, &task_id, &repo, &at);
    r["event_id"] = json!(format!("verdict-{repo}-{jid}"));
    r["assignment_id"] = json!(dev_aid);
    r["generation"] = json!(dev_gen);
    r["phase"] = json!("보고");
    r["operational_state"] = json!(if accepted { "완료" } else { "작업 중" });
    r["evidence"] = json!(used);
    r["next_action"] = json!(if accepted {
        "후속 작업 착수 조건 확인"
    } else {
        "미완료 범위의 증거 대기"
    });
    r["next_owner"] = json!("coordinator");
    r["decision"] = json!({"required": false, "question": null, "options": [], "recommendation": null,
                           "deadline_at": null, "default_action": "현재 승인 범위 유지"});
    r["report_type"] = json!("coordinator_verdict");
    let excluded_text = if excluded.is_empty() {
        String::new()
    } else {
        format!(" 판정 제외(G7): {}.", excluded.join(", "))
    };
    r["body"] = json!({"evidence_comparison": format!("{summary}.{excluded_text}"),
                       "affected_tasks": [], "next_owner": "coordinator"});
    let problems = validate_report(&r);
    if !problems.is_empty() {
        return Err(problem(
            500,
            "INTERNAL",
            &format!("verdict report invalid: {problems:?}"),
        ));
    }
    // verdict, outbox entry and judgment row commit together or not at all
    let seq = insert_event(
        tx,
        &NewEvent {
            sprint_id: &sprint_id,
            scope_repo: &repo,
            scope_role: "coordinator",
            report: &r,
            disposition: None,
            assignment: Some((&dev_aid, dev_gen)),
            attributed_sha: Some(&head_sha),
            policy_digest: Some(&ctx.policy.digest),
        },
    )?;
    tx.execute(
        "INSERT INTO outbox (sprint_id, sprint_seq, destination) VALUES ($1,$2,'client')",
        &[&sprint_id, &seq],
    )?;
    tx.execute(
        "INSERT INTO judgments (repo_id, judgment_id, request, sprint_id, sprint_revision, task_id, outcome,
            head_sha, merged_sha, facts, policy_digest, event_sprint, event_seq, judged_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        &[&repo, &jid, &body, &sprint_id, &rev, &task_id, &outcome, &head_sha, &merged, &facts_json,
          &ctx.policy.digest, &sprint_id, &seq, &who.name],
    )?;
    history(
        tx,
        &who.name,
        "verdict.recorded",
        &json!({"judgment_id": jid, "task_id": task_id, "outcome": outcome, "head_sha": head_sha}),
    )?;
    ok(
        201,
        json!({"judgment_id": jid, "outcome": outcome, "head_sha": head_sha, "merged_sha": merged,
                   "summary": summary, "facts": facts_json, "event_id": r["event_id"],
                   "policy_digest": ctx.policy.digest, "replay": false}),
    )
}

// ---------------------------------------------------------------------------
// Outbox
// ---------------------------------------------------------------------------

fn list_outbox(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "outbox:deliver", false)?;
    let state = query_param(req.query, "state").unwrap_or("pending");
    if !["pending", "sent", "unknown"].contains(&state) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "state",
            "state is pending|sent|unknown",
            "fix state",
        ));
    }
    let repos: Vec<String> = who.repos.iter().cloned().collect();
    let items: Vec<Value> = tx
        .query(
            "SELECT o.outbox_id, o.sprint_id, o.sprint_seq, o.destination, o.state, e.report
             FROM outbox o JOIN events e ON e.sprint_id = o.sprint_id AND e.sprint_seq = o.sprint_seq
             WHERE o.state = $1 AND e.scope_repo = ANY($2) ORDER BY o.outbox_id",
            &[&state, &repos],
        )?
        .iter()
        .map(|r| json!({"outbox_id": r.get::<_, i64>(0), "sprint_id": r.get::<_, String>(1),
                        "sprint_seq": r.get::<_, i64>(2), "destination": r.get::<_, String>(3),
                        "state": r.get::<_, String>(4), "report": r.get::<_, Value>(5)}))
        .collect();
    ok(200, json!({ "items": items }))
}

fn deliver(tx: &mut Transaction, who: &Caller, id: &str, req: &Req) -> R {
    need_client_scope(who, "outbox:deliver", false)?;
    let Ok(id) = id.parse::<i64>() else {
        return Err(problem(404, "NOT_FOUND", "unknown outbox entry"));
    };
    let body = parse_body(req.body)?;
    only_keys(&body, &["state"])?;
    let next = str_field(&body, "state", 16)?.to_string();
    if next != "sent" && next != "unknown" {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "state",
            "state is sent or unknown",
            "fix state",
        ));
    }
    let Some(row) = tx.query_opt(
        "SELECT o.state, e.scope_repo FROM outbox o JOIN events e
           ON e.sprint_id = o.sprint_id AND e.sprint_seq = o.sprint_seq
         WHERE o.outbox_id = $1 FOR UPDATE OF o",
        &[&id],
    )?
    else {
        return Err(problem(404, "NOT_FOUND", "unknown outbox entry"));
    };
    if !who.in_repo(row.get::<_, &str>(1)) {
        return Err(problem(404, "NOT_FOUND", "unknown outbox entry"));
    }
    let cur: String = row.get(0);
    // pending → sent|unknown; unknown → sent; sent is final
    match (cur.as_str(), next.as_str()) {
        (c, n) if c == n => ok(
            200,
            json!({"outbox_id": id, "state": cur, "changed": false}),
        ),
        ("sent", _) => Err(one(
            409,
            "OUTBOX_STATE_CONFLICT",
            "state",
            "a delivered entry cannot become unknown",
            "leave it as sent",
        )),
        _ => {
            tx.execute(
                "UPDATE outbox SET state = $2, updated_at = now() WHERE outbox_id = $1",
                &[&id, &next],
            )?;
            history(
                tx,
                &who.name,
                "outbox.delivery",
                &json!({"outbox_id": id, "from": cur, "to": next}),
            )?;
            ok(
                200,
                json!({"outbox_id": id, "state": next, "changed": true}),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Central checkpoint record (runtime contract §2.1, §5-1, §5-2, §5-6; 02-04 decision item 9)
// ---------------------------------------------------------------------------

/// The supervisor's report credential for exactly this repo and role.
fn need_reporter(who: &Caller, repo: &str, role: &str) -> Result<(), Resp> {
    if who.kind != Kind::Agent || !who.has("checkpoint:report") {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "scope `checkpoint:report` required",
        ));
    }
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    if who.role.as_deref() != Some(role) {
        return Err(problem(
            403,
            "ROLE_MISMATCH",
            "the credential is bound to another role",
        ));
    }
    Ok(())
}

pub(crate) fn need_role(role: &str) -> Result<(), Resp> {
    if role == "development" || role == "guardrail" {
        Ok(())
    } else {
        Err(problem(
            404,
            "NOT_FOUND",
            "role is development or guardrail",
        ))
    }
}

pub(crate) fn latest_row(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
) -> Result<Option<Value>, Resp> {
    let row = tx.query_opt(
        "SELECT seq, checkpoint_sha256, phase, session_id, assignment_id, generation, sprint_id,
                sprint_revision, spec_digest, target_sha, settings_digest, reported_by, iso(received_at),
                (SELECT count(*) FROM role_tamper_events t WHERE t.repo_id = c.repo_id AND t.role = c.role),
                current_assignment,
                EXISTS (SELECT 1 FROM role_tamper_events t
                        WHERE t.repo_id = c.repo_id AND t.role = c.role AND t.seq = c.seq)
         FROM role_checkpoints c WHERE repo_id = $1 AND role = $2 ORDER BY seq DESC LIMIT 1",
        &[&repo, &role],
    )?;
    Ok(row.map(|r| {
        json!({"repo_id": repo, "role": role, "seq": r.get::<_, i64>(0),
               "checkpoint_sha256": r.get::<_, String>(1), "phase": r.get::<_, String>(2),
               "session_id": r.get::<_, String>(3), "assignment_id": r.get::<_, String>(4),
               "generation": r.get::<_, i64>(5), "sprint_id": r.get::<_, String>(6),
               "sprint_revision": r.get::<_, i64>(7), "spec_digest": r.get::<_, String>(8),
               "target_sha": r.get::<_, String>(9), "settings_digest": r.get::<_, String>(10),
               "reported_by": r.get::<_, String>(11), "received_at": r.get::<_, String>(12),
               "tamper_events": r.get::<_, i64>(13), "current_assignment": r.get::<_, bool>(14),
               "tamper_recorded": r.get::<_, bool>(15)})
    }))
}

/// `POST /v0/repos/{repo}/roles/{role}/checkpoints` `{seq, checkpoint_json}`: store the next
/// checkpoint of the sequence. The same sequence with the same bytes is a duplicate (200, nothing
/// changes); with other bytes 409 `EVENT_CONTENT_CONFLICT`; a lower sequence that is not stored
/// cannot exist (the sequence is contiguous); a later one than the next → 409
/// `CHECKPOINT_SEQUENCE_GAP`. Its identity must match the current assignment of this repo and
/// role (generation, sprint revision and digest, target SHA) and never go below the last stored
/// generation — except the later checkpoints of a run whose takeover was recorded under the
/// then-current assignment: they are stored with `current_assignment = false`.
fn report_checkpoint(tx: &mut Transaction, who: &Caller, repo: &str, role: &str, req: &Req) -> R {
    need_role(role)?;
    need_reporter(who, repo, role)?;
    let body = parse_body(req.body)?;
    only_keys(&body, &["seq", "checkpoint_json"])?;
    let seq = u64_field(&body, "seq")?;
    let raw = str_field(&body, "checkpoint_json", 1 << 20)?;
    let bad = |field: &str, msg: &str| {
        one(
            400,
            "SCHEMA_VIOLATION",
            field,
            msg,
            "report the checkpoint file as written",
        )
    };
    let cp: Value = serde_json::from_str(raw).map_err(|_| bad("checkpoint_json", "not JSON"))?;
    if cp["format"] != "ieo-checkpoint/v0" {
        return Err(bad(
            "checkpoint_json.format",
            "format must be ieo-checkpoint/v0",
        ));
    }
    if seq == 0 || cp["seq"].as_u64() != Some(seq) {
        return Err(bad(
            "seq",
            "seq must be ≥ 1 and equal the checkpoint's own seq",
        ));
    }
    if cp["repo_id"] != repo || cp["role"] != role {
        return Err(one(
            409,
            "REPO_MISMATCH",
            "checkpoint_json.repo_id",
            "the checkpoint names another repo or role than the path",
            "report to the checkpoint's own repo and role",
        ));
    }
    let t = |k: &str| -> Result<String, Resp> {
        match cp[k].as_str() {
            Some(x) if !x.is_empty() && x.len() <= 256 => Ok(x.to_string()),
            _ => Err(bad(
                &format!("checkpoint_json.{k}"),
                "missing or not a string",
            )),
        }
    };
    let n = |k: &str| -> Result<u64, Resp> {
        cp[k]
            .as_u64()
            .ok_or_else(|| bad(&format!("checkpoint_json.{k}"), "missing or not an integer"))
    };
    let phase = t("phase")?;
    if !matches!(phase.as_str(), "takeover" | "after_run" | "error") {
        return Err(bad(
            "checkpoint_json.phase",
            "phase is takeover, after_run or error",
        ));
    }
    let (session, aid, generation) = (t("session_id")?, t("assignment_id")?, n("generation")?);
    let (sprint, sprint_rev, spec_digest) =
        (t("sprint_id")?, n("sprint_revision")?, t("spec_digest")?);
    let (target, image, profile, profile_rev) = (
        t("target_sha")?,
        t("image_digest")?,
        t("profile_id")?,
        n("profile_revision")?,
    );
    let settings = t("settings_digest")?;
    // 02-05 step 4 (runtime contract revision 12): the delivery the run belongs to, and the
    // run's first action as a later checkpoint carries it
    let delivery_id = match &cp["delivery_id"] {
        Value::Null => None,
        Value::String(d) if !d.is_empty() && d.len() <= 128 => Some(d.clone()),
        _ => {
            return Err(bad(
                "checkpoint_json.delivery_id",
                "null or a delivery id (1-128 bytes)",
            ));
        }
    };
    // the attempt (the delivery's call) the run belongs to (review 5dbb3d8 M-1)
    let attempt_id = match &cp["attempt_id"] {
        Value::Null => None,
        Value::String(a) if !a.is_empty() && a.len() <= 128 => Some(a.clone()),
        _ => {
            return Err(bad(
                "checkpoint_json.attempt_id",
                "null or an attempt id (1-128 bytes)",
            ));
        }
    };
    let first_action = checkpoint_first_action(&cp["first_action"])
        .map_err(|m| bad("checkpoint_json.first_action", m))?;
    let sha = hex::encode(Sha256::digest(raw.as_bytes()));
    lock(tx, &format!("checkpoint:{repo}:{role}"))?;
    let latest = latest_row(tx, repo, role)?;
    let last_seq = latest.as_ref().and_then(|l| l["seq"].as_u64()).unwrap_or(0);
    if seq <= last_seq {
        let stored: String = tx
            .query_one(
                "SELECT checkpoint_sha256 FROM role_checkpoints WHERE repo_id=$1 AND role=$2 AND seq=$3",
                &[&repo, &role, &i64_of(seq)],
            )?
            .get(0);
        if stored == sha {
            // whether this checkpoint's own report recorded a tamper event: a supervisor clears
            // its pending detection only on `true` (review 681c1d3 L-1)
            let tamper_recorded: bool = tx
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM role_tamper_events
                                    WHERE repo_id=$1 AND role=$2 AND seq=$3)",
                    &[&repo, &role, &i64_of(seq)],
                )?
                .get(0);
            return ok(
                200,
                json!({"duplicate": true, "seq": seq, "checkpoint_sha256": sha,
                       "tamper_recorded": tamper_recorded}),
            );
        }
        return Err(one(
            409,
            "EVENT_CONTENT_CONFLICT",
            "checkpoint_json",
            &format!("checkpoint {seq} is already recorded with other content"),
            "stop and reconcile: the local checkpoint differs from the central record",
        ));
    }
    if seq != last_seq + 1 {
        return Err(one(
            409,
            "CHECKPOINT_SEQUENCE_GAP",
            "seq",
            &format!("the next checkpoint is {}", last_seq + 1),
            "report the missing checkpoints first, in order",
        ));
    }
    if let Some(g) = latest.as_ref().and_then(|l| l["generation"].as_u64())
        && generation < g
    {
        return Err(one(
            409,
            "STALE_ASSIGNMENT",
            "checkpoint_json.generation",
            &format!("generation {generation} is below the recorded {g}"),
            "reconnect under the current assignment",
        ));
    }
    // the current assignment of this repo and role (§5-2)
    let head = tx.query_opt(
        "SELECT assignment_id, generation FROM assignment_heads WHERE repo_id = $1 AND role = $2",
        &[&repo, &role],
    )?;
    let Some(h) = head else {
        return Err(one(
            409,
            "NO_CURRENT_ASSIGNMENT",
            "checkpoint_json.assignment_id",
            "no current assignment for this repo and role",
            "assign first",
        ));
    };
    let (h_aid, h_gen): (String, i64) = (h.get(0), h.get(1));
    let is_current = aid == h_aid && i64_of(generation) == h_gen;
    // a run that started under the then-current assignment ends under it: its later
    // checkpoints (after_run, error) continue the stored takeover of the same session and are
    // recorded as no longer current, so a reassignment during a run does not leave the role
    // with a checkpoint nobody can record (review 48b1d0e M-1). A takeover must be current.
    let continues = phase != "takeover"
        && latest.as_ref().is_some_and(|l| {
            l["phase"] == "takeover"
                && l["session_id"] == session.as_str()
                && l["assignment_id"] == aid.as_str()
                && l["generation"].as_u64() == Some(generation)
        });
    if !is_current && !continues {
        let stale = aid == h_aid && i64_of(generation) < h_gen;
        return Err(one(
            409,
            if stale {
                "STALE_ASSIGNMENT"
            } else {
                "CHECKPOINT_MISMATCH"
            },
            "checkpoint_json.generation",
            &format!("checkpoint {aid}/{generation} is not the current assignment {h_aid}/{h_gen}"),
            "reconnect under the current assignment",
        ));
    }
    // what the named assignment binds: issued for this repo and role, its sprint revision,
    // spec digest and target SHA
    let named = tx.query_opt(
        "SELECT a.sprint_id, a.sprint_revision, a.target_sha, s.digest
         FROM assignments a JOIN sprint_revisions s ON (s.sprint_id, s.revision) = (a.sprint_id, a.sprint_revision)
         WHERE a.assignment_id = $1 AND a.generation = $2 AND a.repo_id = $3 AND a.role = $4",
        &[&aid, &i64_of(generation), &repo, &role],
    )?;
    let Some(h) = named else {
        return Err(one(
            409,
            "CHECKPOINT_MISMATCH",
            "checkpoint_json.assignment_id",
            "the assignment was not issued for this repo and role",
            "reconnect under the current assignment",
        ));
    };
    let checks: [(&str, bool); 4] = [
        ("sprint_id", sprint == h.get::<_, String>(0)),
        ("sprint_revision", i64_of(sprint_rev) == h.get::<_, i64>(1)),
        ("target_sha", target == h.get::<_, String>(2)),
        ("spec_digest", spec_digest == h.get::<_, String>(3)),
    ];
    if let Some((k, _)) = checks.iter().find(|(_, same)| !same) {
        let code = match *k {
            "target_sha" => "ASSIGNMENT_SHA_MISMATCH",
            "spec_digest" => "DIGEST_MISMATCH",
            _ => "CHECKPOINT_MISMATCH",
        };
        return Err(one(
            409,
            code,
            &format!("checkpoint_json.{k}"),
            &format!("{k} differs from the assignment"),
            "reconnect under the current assignment",
        ));
    }
    // a takeover answers a delivery of this assignment and generation (02-05 step 4): a
    // request alone is never a takeover, and a takeover without a delivery is refused
    if phase == "takeover" {
        if let Some(expected) = cp.get("expected_checkpoint_sha256") {
            let stored = latest
                .as_ref()
                .map_or(&Value::Null, |l| &l["checkpoint_sha256"]);
            if expected != stored {
                return Err(problem(
                    409,
                    "CHECKPOINT_MISMATCH",
                    "the predecessor changed after this delivery was authorized",
                ));
            }
        }
        check_delivery(
            tx,
            repo,
            role,
            &aid,
            generation,
            delivery_id.as_deref(),
            attempt_id.as_deref(),
        )?;
        // a new session over an unfinished run (the latest checkpoint at `takeover`/`error`)
        // is a recovery: it names the approval its delivery used (02-05 step 7b, review
        // 185d12b L-1)
        let prev = tx
            .query_opt(
                "SELECT phase, seq FROM role_checkpoints WHERE repo_id = $1 AND role = $2
                 ORDER BY seq DESC LIMIT 1",
                &[&repo, &role],
            )?
            .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1)));
        if let Some(&(_, prev_seq)) = prev
            .as_ref()
            .filter(|(p, _)| matches!(p.as_str(), "takeover" | "error"))
        {
            // the use must be of exactly this checkpoint, approved for this assignment and
            // generation (review d336849 L-a): a delivery's use of another checkpoint never
            // covers this one
            let used: Option<String> = tx
                .query_opt(
                    "SELECT u.recovery_id FROM role_recovery_uses u
                     JOIN role_recoveries r USING (recovery_id)
                     WHERE u.delivery_id = $1 AND u.checkpoint_seq = $2
                       AND r.assignment_id = $3 AND r.generation = $4",
                    &[&delivery_id, &prev_seq, &aid, &i64_of(generation)],
                )?
                .map(|r| r.get(0));
            let named = cp["recovery_id"].as_str();
            if used.is_none() || used.as_deref() != named {
                return Err(one(
                    409,
                    "RECOVERY_REQUIRED",
                    "checkpoint_json.recovery_id",
                    &format!(
                        "the latest checkpoint {prev_seq} is an unfinished run: a takeover over \
                         it names the approval of that checkpoint its delivery used ({}), not {}",
                        used.as_deref().unwrap_or("none"),
                        named.unwrap_or("none")
                    ),
                    "approve a recovery and deliver with it (approveRecovery)",
                ));
            }
        }
    }
    tx.execute(
        "INSERT INTO role_checkpoints (repo_id, role, seq, checkpoint_sha256, phase, session_id, assignment_id,
            generation, sprint_id, sprint_revision, spec_digest, target_sha, image_digest, profile_id,
            profile_revision, settings_digest, checkpoint, reported_by, current_assignment)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",
        &[&repo, &role, &i64_of(seq), &sha, &phase, &session, &aid, &i64_of(generation), &sprint,
          &i64_of(sprint_rev), &spec_digest, &target, &image, &profile, &i64_of(profile_rev), &settings,
          &cp, &who.name, &is_current],
    )?;
    // the takeover row, in this transaction: one per delivery. A second takeover of the same
    // delivery is refused and its checkpoint row rolled back with it
    if phase == "takeover" {
        let d = delivery_id.as_deref().unwrap_or_default();
        let n = tx.execute(
            "INSERT INTO role_takeovers (delivery_id, repo_id, role, seq, checkpoint_sha256, session_id,
                assignment_id, generation, sprint_id, sprint_revision, spec_digest, reported_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT (delivery_id) DO NOTHING",
            &[&d, &repo, &role, &i64_of(seq), &sha, &session, &aid, &i64_of(generation), &sprint,
              &i64_of(sprint_rev), &spec_digest, &who.name],
        )?;
        if n != 1 {
            return Err(one(
                409,
                "DELIVERY_MISMATCH",
                "checkpoint_json.delivery_id",
                &format!("delivery {d} already has a takeover"),
                "a delivery is taken over once; start a new run through a new delivery",
            ));
        }
    }
    // a first action whose own report was lost is filled from the run's later checkpoint
    if let Some(fa) = &first_action
        && phase != "takeover"
        && attempt_id.as_deref().is_none_or(|a| fa["attempt_id"] == a)
    {
        let body = json!({"assignment_id": aid, "generation": generation,
            "attempt_id": fa["attempt_id"], "session_id": session,
            "event_offset": fa["event_offset"], "observed_at": fa["observed_at"],
            "kind": fa["kind"]});
        fill_first_action(tx, who, repo, role, &body, is_current)?;
    }
    // a tamper detection travels with the new session's checkpoints (§5-6); recorded once
    let tamper = &cp["config_tamper_detected"];
    let mut tamper_recorded = false;
    if !tamper.is_null() {
        let f = |k: &str| {
            tamper[k]
                .as_str()
                .filter(|x| !x.is_empty())
                .map(str::to_string)
        };
        let (Some(old), Some(new), Some(cd), Some(od)) = (
            f("old_session_id"),
            f("new_session_id"),
            f("checkpoint_digest"),
            f("observed_digest"),
        ) else {
            return Err(bad(
                "checkpoint_json.config_tamper_detected",
                "old/new session ids and both digests required",
            ));
        };
        if new != session {
            return Err(bad(
                "checkpoint_json.config_tamper_detected.new_session_id",
                "must be the checkpoint's session",
            ));
        }
        tamper_recorded = tx.execute(
            "INSERT INTO role_tamper_events (repo_id, role, new_session_id, old_session_id, checkpoint_digest,
                observed_digest, seq, reported_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
             ON CONFLICT (repo_id, role, new_session_id) DO NOTHING",
            &[&repo, &role, &new, &old, &cd, &od, &i64_of(seq), &who.name],
        )? == 1;
    }
    history(
        tx,
        &who.name,
        "checkpoint.report",
        &json!({"repo_id": repo, "role": role, "seq": seq, "checkpoint_sha256": sha, "phase": phase}),
    )?;
    ok(
        201,
        json!({"stored": true, "seq": seq, "checkpoint_sha256": sha, "tamper_recorded": tamper_recorded,
               "current_assignment": is_current}),
    )
}

/// The `first_action` of a checkpoint (02-05 step 4): absent, null (not known) or `"none"` → no
/// value; otherwise `{attempt_id, event_offset, observed_at, kind}`.
fn checkpoint_first_action(v: &Value) -> Result<Option<Value>, &'static str> {
    if v.is_null() || v == "none" {
        return Ok(None);
    }
    let ok = v.as_object().is_some_and(|m| {
        m.len() == 4
            && v["attempt_id"]
                .as_str()
                .is_some_and(|a| !a.is_empty() && a.len() <= 128)
            && v["event_offset"].is_u64()
            && v["observed_at"].as_str().is_some_and(is_utc_timestamp)
            && matches!(v["kind"].as_str(), Some("tool_use" | "text"))
    });
    if ok {
        Ok(Some(v.clone()))
    } else {
        Err("null, \"none\" or {attempt_id, event_offset, observed_at (…Z), kind tool_use|text}")
    }
}

/// The delivery a takeover names (02-05 step 4): recorded for this repo, role, assignment and
/// generation, and either still waiting for its result (the supervisor reports the takeover
/// while the coordinator records the answer) or answered `accepted` or `unknown`. Anything else
/// — none named, unknown, another assignment or generation, nothing sent — is 409 and leaves no
/// row. Read under the delivery lock, so a result is never recorded in between.
fn check_delivery(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    aid: &str,
    generation: u64,
    delivery_id: Option<&str>,
    attempt_id: Option<&str>,
) -> Result<(), Resp> {
    let field = "checkpoint_json.delivery_id";
    let Some(d) = delivery_id else {
        return Err(one(
            409,
            "DELIVERY_REQUIRED",
            field,
            "a takeover names the delivery it answers",
            "start runs through deliverAssignment; the supervisor carries its delivery_id",
        ));
    };
    lock(tx, &format!("delivery:{repo}:{role}"))?;
    let row = tx.query_opt(
        "SELECT repo_id, role, assignment_id, generation,
                (SELECT outcome FROM role_delivery_results r WHERE r.delivery_id = d.delivery_id
                 ORDER BY n DESC LIMIT 1),
                attempt_id
         FROM role_deliveries d WHERE delivery_id = $1",
        &[&d],
    )?;
    let mismatch = |why: String| {
        Err(one(
            409,
            "DELIVERY_MISMATCH",
            field,
            &why,
            "take over only what a delivery of the current assignment sent",
        ))
    };
    let Some(r) = row else {
        return mismatch(format!("no delivery {d} is recorded"));
    };
    let (r_repo, r_role, r_aid, r_gen): (String, String, String, i64) =
        (r.get(0), r.get(1), r.get(2), r.get(3));
    if r_repo != repo || r_role != role || r_aid != aid || r_gen != i64_of(generation) {
        return mismatch(format!(
            "delivery {d} is for {r_repo}/{r_role} {r_aid}/{r_gen}, not {repo}/{role} {aid}/{generation}"
        ));
    }
    // the takeover answers that delivery's own call: the same attempt (review 5dbb3d8 M-1). A
    // start that only names the delivery — another attempt, or none — never consumes it
    let r_attempt: String = r.get(5);
    if attempt_id != Some(r_attempt.as_str()) {
        return mismatch(format!(
            "delivery {d} called attempt {r_attempt}, not {}",
            attempt_id.unwrap_or("(no attempt_id)")
        ));
    }
    match r.get::<_, Option<String>>(4).as_deref() {
        None | Some("accepted" | "unknown") => Ok(()),
        Some(o) => mismatch(format!(
            "delivery {d} sent nothing to take over (its result is {o})"
        )),
    }
}

/// Insert one first action (both sources share the key, the body and the takeover condition).
fn insert_first_action(
    tx: &mut Transaction,
    who: &Caller,
    repo: &str,
    role: &str,
    body: &Value,
    current: bool,
    source: &str,
) -> Result<(), Resp> {
    let s = |k: &str| body[k].as_str().unwrap_or_default().to_string();
    let n = |k: &str| i64_of(body[k].as_u64().unwrap_or_default());
    tx.execute(
        "INSERT INTO role_first_actions (repo_id, role, assignment_id, generation, attempt_id,
            session_id, event_offset, observed_at, kind, body, current_assignment, reported_by,
            source)
         VALUES ($1,$2,$3,$4,$5,$6,$7,CAST($8::text AS timestamptz),$9,$10,$11,$12,$13)",
        &[
            &repo,
            &role,
            &s("assignment_id"),
            &n("generation"),
            &s("attempt_id"),
            &s("session_id"),
            &n("event_offset"),
            &s("observed_at"),
            &s("kind"),
            body,
            &current,
            &who.name,
            &source,
        ],
    )?;
    history(
        tx,
        &who.name,
        "first_action.record",
        &json!({"repo_id": repo, "role": role, "assignment_id": body["assignment_id"],
                "generation": body["generation"], "attempt_id": body["attempt_id"],
                "current_assignment": current, "source": source}),
    )?;
    Ok(())
}

/// Whether a takeover of `session` is stored under the assignment and generation **and linked to
/// the delivery that called `attempt`** (review 5dbb3d8 L-2, I-2). The one condition of both
/// first-action paths: the direct report (`recordFirstAction`) and the fill from an
/// `after_run`/`error` checkpoint (review debf413 I-8). EXISTS: a resumed session has one
/// takeover per run under the same generation.
fn takeover_linked(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    session: &str,
    aid: &str,
    generation: u64,
    attempt: &str,
) -> Result<bool, Resp> {
    Ok(tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM role_takeovers t
                 JOIN role_deliveries d ON d.delivery_id = t.delivery_id
                 WHERE t.repo_id = $1 AND t.role = $2 AND t.session_id = $3
                   AND t.assignment_id = $4 AND t.generation = $5 AND d.attempt_id = $6)",
            &[&repo, &role, &session, &aid, &i64_of(generation), &attempt],
        )?
        .get(0))
}

/// The first action an `after_run`/`error` checkpoint carries, recorded when none is recorded for
/// that attempt yet and the session's takeover is stored and linked to the delivery that called
/// that attempt (the report was lost or late). The checkpoint's own `attempt_id`, when present,
/// must be the first action's. A record
/// already there is kept as it is: the chain shows both when they differ.
fn fill_first_action(
    tx: &mut Transaction,
    who: &Caller,
    repo: &str,
    role: &str,
    body: &Value,
    current: bool,
) -> Result<(), Resp> {
    let (aid, generation) = (
        body["assignment_id"].as_str().unwrap_or_default(),
        body["generation"].as_u64().unwrap_or_default(),
    );
    let exists = tx
        .query_opt(
            "SELECT 1 FROM role_first_actions WHERE repo_id = $1 AND role = $2
               AND assignment_id = $3 AND generation = $4 AND attempt_id = $5",
            &[
                &repo,
                &role,
                &aid,
                &i64_of(generation),
                &body["attempt_id"].as_str().unwrap_or_default(),
            ],
        )?
        .is_some();
    // the session's takeover is stored and linked to the delivery that called this attempt
    // (review 5dbb3d8 L-2, I-2)
    let linked = takeover_linked(
        tx,
        repo,
        role,
        body["session_id"].as_str().unwrap_or_default(),
        aid,
        generation,
        body["attempt_id"].as_str().unwrap_or_default(),
    )?;
    if exists || !linked {
        return Ok(());
    }
    insert_first_action(tx, who, repo, role, body, current, "checkpoint")
}

/// `GET …/checkpoints/latest`: the reporter itself, or a `roles:control` client of the repo.
fn latest_checkpoint(tx: &mut Transaction, who: &Caller, repo: &str, role: &str) -> R {
    need_role(role)?;
    if who.kind == Kind::Agent {
        need_reporter(who, repo, role)?;
    } else {
        need_client_scope(who, "roles:control", false)?;
        if !who.in_repo(repo) {
            return Err(problem(
                403,
                "REPO_SCOPE_DENIED",
                "repo not in this credential's scope",
            ));
        }
    }
    match latest_row(tx, repo, role)? {
        Some(v) => ok(200, v),
        None => Err(problem(
            404,
            "NO_CHECKPOINT",
            "no checkpoint recorded for this repo and role",
        )),
    }
}

/// `GET …/checkpoints/compare` (`roles:control`): the coordinator's resume judgment for one role
/// (§5-2). It reads the supervisor's status and compares its local checkpoint (sequence,
/// digest) with the central record, and the central record with the current assignment.
/// `decision`: `resume_allowed`, `no_checkpoint` (a new session may start), `already_running`,
/// `stale_generation`, `external_reconciliation_required` or `not_observed`. Read-only.
pub(crate) fn need_compare(who: &Caller, repo: &str, role: &str) -> Result<(), Resp> {
    need_role(role)?;
    need_client_scope(who, "roles:control", false)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    Ok(())
}

/// The role supervisor's status, or why it was not observed.
pub(crate) fn role_status(ctx: &Ctx, repo: &str, role: &str) -> Result<Value, String> {
    let target = &ctx.roles[repo][role];
    if !target.is_object() {
        return Err("no role supervisor configured for this repo and role".into());
    }
    match crate::roles::call(target, "GET", "/v0/role/status") {
        Ok((200, s)) => Ok(s),
        Ok((st, _)) => Err(format!("the supervisor answered {st}")),
        Err(e) => Err(e),
    }
}

pub(crate) fn compare_checkpoint(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    local: &Result<Value, String>,
) -> R {
    // the same lock as the reports: the central record does not move between the reads
    lock(tx, &format!("checkpoint:{repo}:{role}"))?;
    let central = latest_row(tx, repo, role)?;
    let head = tx.query_opt(
        "SELECT assignment_id, generation FROM assignment_heads WHERE repo_id = $1 AND role = $2",
        &[&repo, &role],
    )?;
    let head = head.map(|h| (h.get::<_, String>(0), h.get::<_, i64>(1)));
    let (decision, reasons) = judge_resume(central.as_ref(), head.as_ref(), local.as_ref());
    // 02-05 step 7: the GitHub record as the final basis (development) and the review target
    // (guardrail)
    let (decision, reasons, github) = github_gate(
        tx,
        repo,
        role,
        decision,
        reasons,
        central.as_ref(),
        head.as_ref(),
    )?;
    ok(
        200,
        json!({"repo_id": repo, "role": role, "decision": decision, "reasons": reasons,
        "github": github,
        "central": central,
        "local": local.as_ref().ok().map(|s| json!({"checkpoint_seq": s["checkpoint_seq"],
            "checkpoint_sha256": s["checkpoint_sha256"], "running": s["running"],
            "central_checkpoint": s["central_checkpoint"]})),
        "current_assignment": head.map(|(a, g)| json!({"assignment_id": a, "generation": g}))}),
    )
}

/// The GitHub head comparison of a development resume (02-05 step 7, runtime contract revision
/// 14 §17): the expected SHA is the checkpoint's pushed SHA, or the assignment's target SHA when
/// nothing was pushed. Returns the decision (`resume_allowed`, `not_observed` or
/// `external_reconciliation_required`), the reason of a refusal, and the expected SHA with where
/// it came from. Pure: the caller reads the records.
pub fn github_judgment(
    branch_ref: Option<&str>,
    target_sha: &str,
    pushed_sha: &Value,
    head_sha: Option<&str>,
) -> (&'static str, Option<String>, Option<(String, &'static str)>) {
    let Some(branch) = branch_ref else {
        return (
            "not_observed",
            Some("the assignment names no branch_ref: no GitHub head to compare".into()),
            None,
        );
    };
    let (expected, from) = match pushed_sha {
        Value::Null => (target_sha.to_string(), "target_sha"),
        Value::String(p) if is_sha(p) => (p.clone(), "pushed_sha"),
        _ => {
            return (
                "external_reconciliation_required",
                Some("the checkpoint's pushed_sha is neither null nor a SHA".into()),
                None,
            );
        }
    };
    let Some(head) = head_sha else {
        return (
            "not_observed",
            Some(format!(
                "no GitHub record of {branch} (branch_head evidence)"
            )),
            Some((expected, from)),
        );
    };
    if head != expected {
        let what = if from == "pushed_sha" {
            "the checkpoint's pushed SHA"
        } else {
            "the assignment's target SHA (nothing pushed)"
        };
        return (
            "external_reconciliation_required",
            Some(format!(
                "the GitHub head of {branch} is {head}, not {what} {expected}"
            )),
            Some((expected, from)),
        );
    }
    ("resume_allowed", None, Some((expected, from)))
}

/// The CI result at one SHA from `ci_run` evidence only (latest per `source_ref`): `success`
/// when every one succeeded, `failure` when one did not, `none` without any. CI of other SHAs is
/// never read (02-05 step 7).
fn ci_at(tx: &mut Transaction, repo: &str, sha: &str) -> Result<Value, Resp> {
    let rows = tx.query(
        "SELECT DISTINCT ON (body->>'source_ref') evidence_id, body->>'conclusion' FROM evidence
         WHERE repo_id = $1 AND kind = 'ci_run' AND sha = $2
         ORDER BY body->>'source_ref', seq DESC",
        &[&repo, &sha],
    )?;
    let result = if rows.is_empty() {
        "none"
    } else if rows
        .iter()
        .all(|r| r.get::<_, Option<String>>(1).as_deref() == Some("success"))
    {
        "success"
    } else {
        "failure"
    };
    let ids: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
    Ok(json!({"sha": sha, "result": result, "evidence": ids}))
}

/// The agent's own CI claim at the head SHA: the newest current `development_progress` report of
/// the assignment whose `target_sha` is that SHA, read from its `internal_checks` (a FAIL →
/// `failure`; all PASS → `success`; otherwise no claim). Compared with the GitHub record, which
/// is the one used (02-05 step 7).
fn ci_claim(
    tx: &mut Transaction,
    repo: &str,
    head: &(String, i64),
    sha: &str,
    github: &str,
) -> Result<Value, Resp> {
    let row = tx.query_opt(
        "SELECT event_id, report->'body'->'internal_checks' FROM events
         WHERE scope_repo = $1 AND scope_role = 'development' AND report_type = 'development_progress'
           AND disposition = 'current' AND assignment_id = $2 AND generation = $3
           AND report->'body'->>'target_sha' = $4
         ORDER BY stored_at DESC, sprint_seq DESC LIMIT 1",
        &[&repo, &head.0, &head.1, &sha],
    )?;
    let Some(row) = row else {
        return Ok(Value::Null);
    };
    let checks: Value = row.get(1);
    let results: Vec<&str> = checks
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["result"].as_str())
        .collect();
    let claimed = if results.contains(&"FAIL") {
        "failure"
    } else if !results.is_empty() && results.iter().all(|r| *r == "PASS") {
        "success"
    } else {
        return Ok(Value::Null);
    };
    Ok(
        json!({"event_id": row.get::<_, String>(0), "claimed": claimed,
              "matches_github": claimed == github}),
    )
}

/// 02-05 step 7 on top of [`judge_resume`]. Development: a resume needs the assigned branch's
/// GitHub head (the newest `branch_head` evidence) to equal the checkpoint's pushed SHA (or the
/// target SHA when nothing was pushed); the CI result at that head and any differing agent CI
/// claim are recorded. Guardrail: nothing runs (`stale_target`) while the review target is not
/// the current development assignment's target SHA. The GitHub records are evidence-service
/// input, not a live GitHub read.
fn github_gate(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    decision: &'static str,
    reasons: Vec<String>,
    central: Option<&Value>,
    head: Option<&(String, i64)>,
) -> Result<(&'static str, Vec<String>, Value), Resp> {
    let Some(h) = head else {
        return Ok((decision, reasons, Value::Null));
    };
    let target = |tx: &mut Transaction, aid: &str, generation: i64| {
        tx.query_one(
            "SELECT target_sha, request->>'branch_ref' FROM assignments
             WHERE assignment_id = $1 AND generation = $2",
            &[&aid, &generation],
        )
        .map(|r| (r.get::<_, String>(0), r.get::<_, Option<String>>(1)))
    };
    if role == "guardrail" {
        if !matches!(
            decision,
            "resume_allowed" | "no_checkpoint" | "stale_generation"
        ) {
            return Ok((decision, reasons, Value::Null));
        }
        let (review, _) = target(tx, &h.0, h.1)?;
        let dev = tx
            .query_opt(
                "SELECT a.target_sha FROM assignment_heads h JOIN assignments a
                   ON (a.assignment_id, a.generation) = (h.assignment_id, h.generation)
                 WHERE h.repo_id = $1 AND h.role = 'development'",
                &[&repo],
            )?
            .map(|r| r.get::<_, String>(0));
        if dev.as_deref() == Some(review.as_str()) {
            return Ok((decision, reasons, Value::Null));
        }
        let why = match &dev {
            Some(d) => format!(
                "the review target {review} is not the current development assignment's target \
                 SHA {d}: a new review assignment is needed"
            ),
            None => format!(
                "the review target {review} has no current development assignment: a new review \
                 assignment is needed"
            ),
        };
        return Ok(("stale_target", vec![why], Value::Null));
    }
    if decision != "resume_allowed" {
        return Ok((decision, reasons, Value::Null));
    }
    let cp: Value = match central {
        Some(c) => tx
            .query_opt(
                "SELECT checkpoint FROM role_checkpoints WHERE repo_id = $1 AND role = $2 AND seq = $3",
                &[&repo, &role, &c["seq"].as_i64().unwrap_or(-1)],
            )?
            .map(|r| r.get(0))
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    let (d, why, github) = github_head_check(tx, repo, h, &cp)?;
    match why {
        None => Ok((d, reasons, github)),
        Some(w) => Ok((d, vec![w], github)),
    }
}

/// The GitHub head check of one development checkpoint against one assignment generation (02-05
/// step 7), reusable by any judgment that needs it (the recovery of step 7b among them): the
/// assignment's `branch_ref` and `target_sha`, the checkpoint's `pushed_sha`, and the newest
/// `branch_head` evidence of that branch. Returns the decision (`resume_allowed` on a match,
/// `external_reconciliation_required` on a mismatch, `not_observed` without a branch or a
/// record), the reason of a refusal, and the comparison with its evidence: `result`
/// (`match`/`mismatch`/`not_observed`), `source` (the input is evidence-service records, not a
/// live GitHub read), the head's evidence id, the CI result at that head and an agent's
/// differing CI claim. Read-only.
pub(crate) fn github_head_check(
    tx: &mut Transaction,
    repo: &str,
    assignment: &(String, i64),
    checkpoint: &Value,
) -> Result<(&'static str, Option<String>, Value), Resp> {
    let row = tx.query_one(
        "SELECT target_sha, request->>'branch_ref' FROM assignments
         WHERE assignment_id = $1 AND generation = $2",
        &[&assignment.0, &assignment.1],
    )?;
    let (target_sha, branch) = (row.get::<_, String>(0), row.get::<_, Option<String>>(1));
    let gh_head = match &branch {
        Some(b) => tx.query_opt(
            "SELECT evidence_id, sha, body->>'source_ref' FROM evidence
             WHERE repo_id = $1 AND kind = 'branch_head' AND body->>'ref' = $2
             ORDER BY seq DESC LIMIT 1",
            &[&repo, b],
        )?,
        None => None,
    }
    .map(|r| {
        (
            r.get::<_, String>(0),
            r.get::<_, String>(1),
            r.get::<_, Option<String>>(2),
        )
    });
    // a checkpoint that names no branch observed no push (runtime contract item 22): its
    // `pushed_sha`, if any, is not read and the target SHA is expected (review 47e4e3f L-4)
    let pushed = if checkpoint["branch_ref"].is_string() {
        checkpoint["pushed_sha"].clone()
    } else {
        Value::Null
    };
    let (d, why, expected) = github_judgment(
        branch.as_deref(),
        &target_sha,
        &pushed,
        gh_head.as_ref().map(|x| x.1.as_str()),
    );
    // the checkpoint's pushed SHA is the assignment branch's only when the checkpoint names that
    // branch (review f0e0ca9 L-2): another branch is never read as this one's; without a branch
    // the checkpoint observed no push, and the target SHA is expected for that reason
    let cp_branch = checkpoint["branch_ref"].as_str();
    let (d, why) = match (cp_branch, branch.as_deref()) {
        (Some(c), Some(a)) if c != a && d != "not_observed" => (
            "external_reconciliation_required",
            Some(format!(
                "the checkpoint's branch_ref {c} is not the assignment's {a}: its pushed SHA is \
                 not this branch's"
            )),
        ),
        _ => (d, why),
    };
    let provenance = match (cp_branch, &checkpoint["pushed_sha"]) {
        (Some(c), _) if branch.as_deref().is_some_and(|a| a != c) => {
            "checkpoint names another branch"
        }
        (Some(_), Value::String(_)) => "pushed SHA recorded by the checkpoint",
        (Some(_), _) => "no push observed on the branch (target SHA expected)",
        (None, _) => "the checkpoint names no branch: no push observed (target SHA expected)",
    };
    let result = match d {
        "resume_allowed" => "match",
        "not_observed" => "not_observed",
        _ => "mismatch",
    };
    let mut github = json!({
        "result": result,
        "source": "evidence:ingest (trusted service input; not a live GitHub read)",
        "assignment_id": assignment.0, "generation": assignment.1,
        "branch_ref": branch, "worktree_head": checkpoint["worktree_head"],
        "pushed_sha": checkpoint["pushed_sha"],
        "expected_sha": expected.as_ref().map(|e| e.0.clone()),
        "expected_from": expected.as_ref().map(|e| e.1),
        "checkpoint_branch_ref": checkpoint["branch_ref"], "push_provenance": provenance,
        "head": gh_head.as_ref().map(|(id, sha, src)| json!({"evidence_id": id, "sha": sha,
                                                                "source_ref": src})),
        "matches": d == "resume_allowed", "ci": null, "ci_claim": null});
    if let Some((_, sha, _)) = &gh_head {
        let ci = ci_at(tx, repo, sha)?;
        let ci_result = ci["result"].as_str().unwrap_or("none").to_string();
        github["ci"] = ci;
        github["ci_claim"] = ci_claim(tx, repo, assignment, sha, &ci_result)?;
    }
    Ok((d, why, github))
}

/// The §5-2 resume judgment from the central record, the current assignment and the
/// supervisor's status (or why it was not observed).
pub fn judge_resume(
    central: Option<&Value>,
    head: Option<&(String, i64)>,
    local: Result<&Value, &String>,
) -> (&'static str, Vec<String>) {
    let s = match local {
        Err(e) => return ("not_observed", vec![format!("supervisor status: {e}")]),
        Ok(s) => s,
    };
    if !s["running"].is_null() {
        return (
            "already_running",
            vec!["the role is running an attempt".into()],
        );
    }
    let lseq = s["checkpoint_seq"].as_u64();
    let lsha = s["checkpoint_sha256"].as_str();
    let reconcile = |r: String| ("external_reconciliation_required", vec![r]);
    let Some(c) = central else {
        return match (lseq, lsha) {
            (None, None) => ("no_checkpoint", vec![]),
            _ => reconcile("the role has a local checkpoint the coordinator never recorded".into()),
        };
    };
    let cseq = c["seq"].as_u64().unwrap_or(0);
    let Some(lseq) = lseq else {
        return reconcile(format!(
            "the coordinator holds checkpoint {cseq}; the role reports none"
        ));
    };
    if lseq < cseq {
        return reconcile(format!(
            "local checkpoint {lseq} is older than the central {cseq}"
        ));
    }
    if lseq > cseq {
        return reconcile(format!(
            "local checkpoint {lseq} is not yet recorded (central {cseq})"
        ));
    }
    if lsha != c["checkpoint_sha256"].as_str() {
        return reconcile(format!(
            "checkpoint {lseq}: local digest differs from the central record"
        ));
    }
    let Some((aid, generation)) = head else {
        return reconcile("no current assignment for this repo and role".into());
    };
    let cgen = c["generation"].as_i64().unwrap_or(-1);
    // an older generation whose last run did not finish is a run to reconcile, not merely stale:
    // its children, external requests and session record were never observed (§6), so no new
    // session may start over it either (02-05 step 3, review c11e734 M-1)
    if c["assignment_id"] == aid.as_str() && cgen < *generation && c["phase"] != "after_run" {
        return reconcile(format!(
            "the last run of the older generation {cgen} did not finish (phase {})",
            c["phase"]
        ));
    }
    if c["assignment_id"] == aid.as_str() && cgen < *generation {
        return (
            "stale_generation",
            vec![format!(
                "checkpoint generation {cgen} < current {generation}"
            )],
        );
    }
    if c["assignment_id"] != aid.as_str() || cgen != *generation {
        return reconcile(format!(
            "the checkpoint's assignment {}/{cgen} is not the current {aid}/{generation}",
            c["assignment_id"]
        ));
    }
    if c["phase"] != "after_run" {
        return reconcile(format!(
            "the last run did not finish (phase {})",
            c["phase"]
        ));
    }
    ("resume_allowed", vec![])
}

// ---------------------------------------------------------------------------
// The supervisor's own assignment and session facts (02-05 step 2, PLAN §3.1, T1)
// ---------------------------------------------------------------------------
//
// Only the role supervisor's `checkpoint:report` credential of exactly this (repo, role) may call
// these (`need_reporter`); the model process never holds it (runtime contract §4.3). What the
// supervisor records here is a claim: it is stored with `evidence_level = claim` and never read
// as a completion signal (02-03 `judge` reads GitHub and run evidence only).

/// An RFC 3339 UTC timestamp as the supervisor writes it: `YYYY-MM-DDTHH:MM:SS[.fraction]Z`,
/// year 1970–9999, a real calendar day. Anything PostgreSQL would refuse is refused here first.
fn is_utc_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().unwrap_or(u32::MAX);
    if b.len() < 20
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'T'
        || !digits(11..13)
        || b[13] != b':'
        || !digits(14..16)
        || b[16] != b':'
        || !digits(17..19)
        || b[b.len() - 1] != b'Z'
    {
        return false;
    }
    let frac = &b[19..b.len() - 1];
    if !(frac.is_empty()
        || (frac.len() >= 2
            && frac.len() <= 10
            && frac[0] == b'.'
            && frac[1..].iter().all(u8::is_ascii_digit)))
    {
        return false;
    }
    let (y, m, d) = (num(0..4), num(5..7), num(8..10));
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    (1970..=9999).contains(&y)
        && (1..=days).contains(&d)
        && num(11..13) < 24
        && num(14..16) < 60
        && num(17..19) < 60
}

fn bad_field(field: &str, message: &str) -> Resp {
    one(
        400,
        "SCHEMA_VIOLATION",
        field,
        message,
        "report the value as the supervisor observed it",
    )
}

/// `GET /v0/repos/{repo}/roles/{role}/assignment` (getOwnAssignment): the current assignment of
/// this (repo, role) as the coordinator recorded it — head, sprint revision, digest and sources,
/// the revision's limits and the attempts issued. Read only. No head → 404
/// `NO_CURRENT_ASSIGNMENT`. Elapsed time and token consumption are not metered by the coordinator
/// yet (02-05 step 5). `last_delivery_id` and `expected_runtime` name the newest delivery of this
/// head (deliverAssignment, step 3) and the image and profile it expects; null without one.
fn own_assignment(tx: &mut Transaction, who: &Caller, repo: &str, role: &str) -> R {
    need_role(role)?;
    need_reporter(who, repo, role)?;
    let row = tx.query_opt(
        "SELECT a.assignment_id, a.generation, a.task_id, a.target_sha, a.sprint_id,
                a.sprint_revision, s.digest, s.spec, a.policy_digest, iso(a.issued_at),
                coalesce((SELECT issued FROM task_attempts t WHERE t.sprint_id = a.sprint_id
                            AND t.task_id = a.task_id AND t.role = a.role), 0)
         FROM assignment_heads h
         JOIN assignments a ON (a.assignment_id, a.generation) = (h.assignment_id, h.generation)
         JOIN sprint_revisions s ON (s.sprint_id, s.revision) = (a.sprint_id, a.sprint_revision)
         WHERE h.repo_id = $1 AND h.role = $2",
        &[&repo, &role],
    )?;
    let Some(r) = row else {
        return Err(problem(
            404,
            "NO_CURRENT_ASSIGNMENT",
            "no current assignment for this repo and role",
        ));
    };
    let spec: Value = r.get(7);
    // exactly the SprintSpec v0 source keys, so both transports carry the same value
    let sources: Vec<Value> = spec["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| {
            json!({"kind": x["kind"], "ref": x["ref"], "revision": x["revision"],
                   "digest": x["digest"], "observed_at": x["observed_at"]})
        })
        .collect();
    let limit = |k: &str| spec["limits"][k].as_u64().map_or(Value::Null, |n| json!(n));
    // the assignment's own limits (02-05 step 5), else the revision's
    let eff = crate::limits::state(tx, &r.get::<_, String>(0), r.get::<_, i64>(1), None)?["limits"]
        .clone();
    // the newest delivery of this head (02-05 step 3) and the image and profile it expects
    let delivery = tx.query_opt(
        "SELECT delivery_id, coordinator_record FROM role_deliveries
         WHERE repo_id = $1 AND role = $2 AND assignment_id = $3 AND generation = $4
         ORDER BY seq DESC LIMIT 1",
        &[&repo, &role, &r.get::<_, String>(0), &r.get::<_, i64>(1)],
    )?;
    let (last_delivery_id, expected_runtime) = match delivery {
        Some(d) => {
            let c: Value = d.get(1);
            (
                json!(d.get::<_, String>(0)),
                json!({"image_digest": c["image_digest"], "profile_id": c["profile_id"],
                       "profile_revision": c["profile_revision"]}),
            )
        }
        None => (Value::Null, Value::Null),
    };
    ok(
        200,
        json!({"repo_id": repo, "role": role, "assignment_id": r.get::<_, String>(0),
               "generation": r.get::<_, i64>(1), "task_id": r.get::<_, String>(2),
               "target_sha": r.get::<_, String>(3), "sprint_id": r.get::<_, String>(4),
               "sprint_revision": r.get::<_, i64>(5), "spec_digest": r.get::<_, String>(6),
               "sources": sources,
               "limits": {"elapsed_seconds": eff["elapsed_seconds"],
                          "total_tokens": eff["total_tokens"],
                          "correction_attempts": limit("correction_attempts")},
               "attempts_issued": r.get::<_, i64>(10),
               "policy_digest": r.get::<_, String>(8), "issued_at": r.get::<_, String>(9),
               "last_delivery_id": last_delivery_id, "expected_runtime": expected_runtime}),
    )
}

/// The assignment named by a supervisor fact was issued to this (repo, role); `true` when it is
/// the current head. Never issued → 409 `STALE_ASSIGNMENT` (as for agent claims).
fn issued_here(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    aid: &str,
    generation: u64,
    field: &str,
) -> Result<bool, Resp> {
    let issued = tx
        .query_opt(
            "SELECT 1 FROM assignments WHERE assignment_id = $1 AND generation = $2
               AND repo_id = $3 AND role = $4",
            &[&aid, &i64_of(generation), &repo, &role],
        )?
        .is_some();
    if !issued {
        return Err(one(
            409,
            "STALE_ASSIGNMENT",
            field,
            &format!("{aid}/{generation} was never issued to this repo and role"),
            "report under an assignment issued to this role",
        ));
    }
    Ok(tx
        .query_opt(
            "SELECT 1 FROM assignment_heads WHERE repo_id = $1 AND role = $2
               AND assignment_id = $3 AND generation = $4",
            &[&repo, &role, &aid, &i64_of(generation)],
        )?
        .is_some())
}

/// `POST /v0/repos/{repo}/roles/{role}/first-actions` (recordFirstAction): the first actual action
/// of one attempt as the supervisor observed it in the stream (event offset, supervisor clock,
/// `tool_use` or `text`). Key (assignment_id, generation, attempt_id) within the credential's
/// (repo, role): the same content again → 200 with the stored receipt, other content → 409
/// `EVENT_CONTENT_CONFLICT`. The session's takeover checkpoint must be stored under that
/// assignment and generation (else 409 `CHECKPOINT_MISMATCH`); like the later checkpoints of a
/// run, a first action after the assignment moved on is stored with `current_assignment: false`.
/// A claim.
fn record_first_action(tx: &mut Transaction, who: &Caller, repo: &str, role: &str, req: &Req) -> R {
    need_role(role)?;
    need_reporter(who, repo, role)?;
    let body = parse_body(req.body)?;
    only_keys(
        &body,
        &[
            "assignment_id",
            "generation",
            "attempt_id",
            "session_id",
            "event_offset",
            "observed_at",
            "kind",
        ],
    )?;
    let aid = str_field(&body, "assignment_id", 128)?;
    let generation = u64_field(&body, "generation")?;
    let attempt = str_field(&body, "attempt_id", 128)?;
    let session = str_field(&body, "session_id", 256)?;
    u64_field(&body, "event_offset")?;
    let observed = str_field(&body, "observed_at", 64)?;
    if !is_utc_timestamp(observed) {
        return Err(bad_field(
            "observed_at",
            "observed_at is an RFC 3339 UTC timestamp (…Z)",
        ));
    }
    let kind = body["kind"].as_str().unwrap_or_default();
    if kind != "tool_use" && kind != "text" {
        return Err(bad_field("kind", "kind is tool_use or text"));
    }
    lock(tx, &format!("checkpoint:{repo}:{role}"))?;
    let receipt = |r: &postgres::Row| {
        json!({"assignment_id": r.get::<_, String>(0), "generation": r.get::<_, i64>(1),
               "attempt_id": r.get::<_, String>(2), "session_id": r.get::<_, String>(3),
               "evidence_level": "claim", "current_assignment": r.get::<_, bool>(4),
               "received_at": r.get::<_, String>(5)})
    };
    let select = "SELECT assignment_id, generation, attempt_id, session_id, current_assignment,
                         iso(received_at), body
                  FROM role_first_actions WHERE repo_id = $1 AND role = $2
                    AND assignment_id = $3 AND generation = $4 AND attempt_id = $5";
    let key: [&(dyn postgres::types::ToSql + Sync); 5] =
        [&repo, &role, &aid, &i64_of(generation), &attempt];
    if let Some(r) = tx.query_opt(select, &key)? {
        return if r.get::<_, Value>(6) == body {
            ok(200, receipt(&r))
        } else {
            Err(one(
                409,
                "EVENT_CONTENT_CONFLICT",
                "attempt_id",
                "the first action of this attempt is already recorded with other content",
                "keep the first observation; a later action is not a first action",
            ))
        };
    }
    let current = issued_here(tx, repo, role, aid, generation, "assignment_id")?;
    // the fill's condition (review debf413 I-8): the session's takeover, linked to the delivery
    // that called this attempt
    if !takeover_linked(tx, repo, role, session, aid, generation, attempt)? {
        return Err(one(
            409,
            "CHECKPOINT_MISMATCH",
            "session_id",
            "no takeover of this session under this assignment and generation is linked to the \
             delivery that called this attempt",
            "report the takeover checkpoint of the delivered attempt first",
        ));
    }
    insert_first_action(tx, who, repo, role, &body, current, "report")?;
    let r = tx.query_one(select, &key)?;
    ok(201, receipt(&r))
}

/// The status items a session status carries (runtime contract §7 status), every one required
/// (`null` where nothing is known).
const SESSION_STATUS_KEYS: [&str; 12] = [
    "observed_at",
    "lifecycle",
    "session_id",
    "assignment_id",
    "generation",
    "quota_group_id",
    "provider_status_or_unknown",
    "last_actual_action_at",
    "checkpoint_seq",
    "checkpoint_sha256",
    "pending_external_requests",
    "active_children",
];

/// Format of a session status. Returns the named assignment, if any.
fn check_session_status(v: &Value) -> Result<Option<(String, u64)>, Resp> {
    only_keys(v, &SESSION_STATUS_KEYS)?;
    if let Some(k) = SESSION_STATUS_KEYS.iter().find(|k| v.get(**k).is_none()) {
        return Err(one(
            400,
            "MISSING_FIELD",
            k,
            &format!("`{k}` is required (null when not known)"),
            &format!("supply `{k}`"),
        ));
    }
    let observed = str_field(v, "observed_at", 64)?;
    if !is_utc_timestamp(observed) {
        return Err(bad_field(
            "observed_at",
            "observed_at is an RFC 3339 UTC timestamp (…Z)",
        ));
    }
    if !matches!(
        v["lifecycle"].as_str(),
        Some("idle" | "running" | "stopping")
    ) {
        return Err(bad_field(
            "lifecycle",
            "lifecycle is idle, running or stopping",
        ));
    }
    let nullable_str = |k: &str, max: usize| -> Result<(), Resp> {
        if v[k].is_null() {
            Ok(())
        } else {
            str_field(v, k, max).map(|_| ())
        }
    };
    nullable_str("session_id", 256)?;
    str_field(v, "quota_group_id", 128)?;
    str_field(v, "provider_status_or_unknown", 64)?;
    if !v["last_actual_action_at"].is_null()
        && !v["last_actual_action_at"]
            .as_str()
            .is_some_and(is_utc_timestamp)
    {
        return Err(bad_field(
            "last_actual_action_at",
            "null or an RFC 3339 UTC timestamp (…Z)",
        ));
    }
    let assignment = match (&v["assignment_id"], &v["generation"]) {
        (Value::Null, Value::Null) => None,
        _ => Some((
            str_field(v, "assignment_id", 128)?.to_string(),
            u64_field(v, "generation")?,
        )),
    };
    match (&v["checkpoint_seq"], &v["checkpoint_sha256"]) {
        (Value::Null, Value::Null) => {}
        (s, d) => {
            if s.as_u64().is_none_or(|n| n == 0) {
                return Err(bad_field(
                    "checkpoint_seq",
                    "null or an integer ≥ 1, with its digest",
                ));
            }
            if !d.as_str().is_some_and(|d| d.len() == 64 && is_sha(d)) {
                return Err(bad_field(
                    "checkpoint_sha256",
                    "null or 64 lowercase hex, with its sequence",
                ));
            }
        }
    }
    for k in ["pending_external_requests", "active_children"] {
        let ok = v[k] == "not_tracked"
            || v[k]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_object));
        if !ok {
            return Err(bad_field(
                k,
                "\"not_tracked\" or a list of objects (never 0 for not tracked)",
            ));
        }
    }
    Ok(assignment)
}

/// `POST /v0/repos/{repo}/roles/{role}/session-status` (reportSessionStatus): append one session
/// status to the coordinator's history of this (repo, role), so the last report and its time
/// stay visible when the supervisor cannot be reached. Key: `Idempotency-Key` (gRPC
/// `idempotency_key`) within the credential's (repo, role): the same status again → 200 with the
/// stored receipt, another status → 409 `IDEMPOTENCY_KEY_CONFLICT`. A status observed at or before
/// an already stored one is kept as history with `stale_on_arrival: true` and is never the current
/// value (02-04 item 4 freshness). A named assignment must have been issued to this (repo, role)
/// (else 409 `STALE_ASSIGNMENT`); an earlier one is kept with `current_assignment: false`. A claim.
fn report_session_status(
    tx: &mut Transaction,
    who: &Caller,
    repo: &str,
    role: &str,
    req: &Req,
) -> R {
    need_role(role)?;
    need_reporter(who, repo, role)?;
    let key = idem_key(req)?;
    let body = parse_body(req.body)?;
    let assignment = check_session_status(&body)?;
    lock(tx, &format!("checkpoint:{repo}:{role}"))?;
    let receipt = |r: &postgres::Row| {
        json!({"idempotency_key": r.get::<_, String>(0),
               "observed_at": r.get::<_, Value>(1)["observed_at"],
               "evidence_level": "claim", "stale_on_arrival": r.get::<_, bool>(2),
               "current_assignment": r.get::<_, Option<bool>>(3),
               "received_at": r.get::<_, String>(4)})
    };
    let select = "SELECT idempotency_key, status, stale_on_arrival, current_assignment,
                         iso(received_at)
                  FROM role_session_status WHERE repo_id = $1 AND role = $2 AND idempotency_key = $3";
    if let Some(r) = tx.query_opt(select, &[&repo, &role, &key])? {
        return if r.get::<_, Value>(1) == body {
            ok(200, receipt(&r))
        } else {
            Err(one(
                409,
                "IDEMPOTENCY_KEY_CONFLICT",
                "Idempotency-Key",
                "this key was used with another status",
                "use a new key for a new status",
            ))
        };
    }
    let current = match &assignment {
        Some((aid, generation)) => Some(issued_here(
            tx,
            repo,
            role,
            aid,
            *generation,
            "assignment_id",
        )?),
        None => None,
    };
    let observed = body["observed_at"].as_str().unwrap_or_default();
    let stale: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM role_session_status WHERE repo_id = $1 AND role = $2
                              AND observed_at >= CAST($3::text AS timestamptz))",
            &[&repo, &role, &observed],
        )?
        .get(0);
    let (aid, generation) = match &assignment {
        Some((a, g)) => (Some(a.as_str()), Some(i64_of(*g))),
        None => (None, None),
    };
    tx.execute(
        "INSERT INTO role_session_status (repo_id, role, idempotency_key, status, observed_at,
            assignment_id, generation, current_assignment, stale_on_arrival, reported_by)
         VALUES ($1,$2,$3,$4,CAST($5::text AS timestamptz),$6,$7,$8,$9,$10)",
        &[
            &repo,
            &role,
            &key,
            &body,
            &observed,
            &aid,
            &generation,
            &current,
            &stale,
            &who.name,
        ],
    )?;
    history(
        tx,
        &who.name,
        "session_status.report",
        &json!({"repo_id": repo, "role": role, "idempotency_key": key,
                "stale_on_arrival": stale, "current_assignment": current}),
    )?;
    let r = tx.query_one(select, &[&repo, &role, &key])?;
    ok(201, receipt(&r))
}

/// Apply the schema (idempotent `CREATE ... IF NOT EXISTS`).
pub fn migrate(db: &mut impl GenericClient) -> Result<(), postgres::Error> {
    db.batch_execute(include_str!("schema.sql"))
}

#[cfg(test)]
mod tests {
    use super::{github_judgment, judge_resume, legacy_split};
    use serde_json::{Value, json};

    /// 02-05 step 7: the GitHub head of the assigned branch against the checkpoint's pushed SHA
    /// (or the target SHA when nothing was pushed).
    #[test]
    fn the_github_head_is_the_final_basis_of_a_development_resume() {
        let (t, p, f) = ("1".repeat(40), "3".repeat(40), "4".repeat(40));
        let b = Some("refs/heads/feat/x");
        let d =
            |branch, pushed: &Value, head: Option<&str>| github_judgment(branch, &t, pushed, head);
        // no branch named, or no GitHub record: not observed
        assert_eq!(d(None, &Value::Null, Some(&t)).0, "not_observed");
        assert_eq!(d(b, &Value::Null, None).0, "not_observed");
        // nothing pushed: the head must be the target SHA
        assert_eq!(
            d(b, &Value::Null, Some(&t)),
            ("resume_allowed", None, Some((t.clone(), "target_sha")))
        );
        let (dec, why, _) = d(b, &Value::Null, Some(&p));
        assert_eq!(dec, "external_reconciliation_required");
        assert!(why.is_some_and(|w| w.contains("nothing pushed")));
        // pushed: the head must be the pushed SHA
        assert_eq!(
            d(b, &json!(p), Some(&p)),
            ("resume_allowed", None, Some((p.clone(), "pushed_sha")))
        );
        assert_eq!(
            d(b, &json!(p), Some(&t)).0,
            "external_reconciliation_required"
        );
        assert_eq!(
            d(b, &json!(f), Some(&p)).0,
            "external_reconciliation_required"
        );
        // a pushed_sha that is not a SHA is never taken as "nothing pushed"
        for bad in [json!("HEAD"), json!(5), json!(p.to_uppercase())] {
            assert_eq!(
                d(b, &bad, Some(&t)).0,
                "external_reconciliation_required",
                "{bad}"
            );
        }
    }

    /// The resume judgment by the central checkpoint's assignment, generation and phase (the local
    /// checkpoint matches it). Review c11e734 M-1: an older generation whose last run did not
    /// finish is reconciliation, not `stale_generation`; every other outcome is as before.
    #[test]
    fn an_unfinished_run_of_an_older_generation_is_reconciliation() {
        let head = ("A1".to_string(), 2i64);
        let local = json!({"running": null, "checkpoint_seq": 5, "checkpoint_sha256": "x"});
        let judge = |aid: &str, generation: i64, phase: &str| {
            let c = json!({"seq": 5, "checkpoint_sha256": "x", "assignment_id": aid,
                           "generation": generation, "phase": phase});
            judge_resume(Some(&c), Some(&head), Ok(&local)).0
        };
        assert_eq!(judge("A1", 1, "after_run"), "stale_generation");
        assert_eq!(
            judge("A1", 1, "takeover"),
            "external_reconciliation_required"
        );
        assert_eq!(judge("A1", 1, "error"), "external_reconciliation_required");
        assert_eq!(judge("A1", 2, "after_run"), "resume_allowed");
        assert_eq!(
            judge("A1", 2, "takeover"),
            "external_reconciliation_required"
        );
        assert_eq!(
            judge("A1", 3, "after_run"),
            "external_reconciliation_required"
        );
        assert_eq!(
            judge("B1", 1, "after_run"),
            "external_reconciliation_required"
        );
    }

    #[test]
    fn legacy_reasons_split_fail_closed() {
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (g, m) = legacy_split(&r(&[
            "Notion 큐 구획 '조건 대기': 현재 실행 대상이 아님",
            "Notion 작업 상태 '결정대기': 실행 재개가 아님",
            "Notion 차단 대상 미해제: X",
            "실제 repository_url",
            "시간/토큰/교정 한도",
            "착수 조건",
        ]));
        assert_eq!(g.len(), 3);
        assert!(m.is_empty(), "{m:?}");
        // importer missing-fact templates and unknown text both block (fail closed)
        let (g, m) = legacy_split(&r(&[
            "수용 기준(필수 인수 판정 표)을 원본에서 읽지 못함",
            "선행 해제됨",
        ]));
        assert!(g.is_empty());
        assert_eq!(m.len(), 2);
    }
}
