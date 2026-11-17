//! Delivery of the current assignment to a role supervisor (02-05 step 3, PLAN §3 step 3):
//! `POST /v0/repos/{repo}/roles/{role}/deliveries` (deliverAssignment). HTTP only: an operator
//! (`roles:control`) operation, never a supervisor one (contracts/ieo-internal-v0.md §1.2).
//!
//! The shape: authorization → the request recorded (transaction 1) → the resume judgment
//! (`compare`, `crate::api::judge_resume`) → the shared budget check
//! ([`ieo_supervisor::caller::admit`]) → the supervisor's `start` or `reconnect` (outside any
//! transaction) → its answer, or `unknown` when it was lost, recorded (transaction 2). A delivery
//! is what the coordinator itself did, so it is stored as `verified`; it is not a takeover: the
//! supervisor's takeover checkpoint names this delivery (`delivery_id`, sent with the start) and
//! the coordinator links the two when it stores the takeover (step 4).
//!
//! * The coordinator record (assignment, generation, sprint revision, spec digest, image,
//!   profile), the target SHA and the limits are built from the database and the coordinator's
//!   configuration, never from the request.
//! * `no_checkpoint` → `start`; `resume_allowed` → `reconnect` (the supervisor resumes by the
//!   checkpoint's session id only, C1). `new_session: true` asks for a `start` also on
//!   `resume_allowed` and `stale_generation`. `already_running`, `external_reconciliation_required`,
//!   `not_observed` (and `stale_generation` without `new_session`) call nothing.
//! * The budget check runs here, outside the role containers, with the code the 02-04 host caller
//!   uses; an accepted run is then finished, measured and charged in the background by the same
//!   library, which holds the budget's ledger lock until the run's record is written.
//! * The same Idempotency-Key with the same body answers the stored delivery (200). If its result
//!   is `unknown` (or missing), the supervisor is asked for the attempt's stored answer
//!   (`GET /v0/role/attempts/{id}`), which never starts anything, and the answer is recorded.
//!
//! Configuration (`--roles`): each role entry also names `image_digest`, `profile_id` and
//! `profile_revision`; the repo entry has `budget_plan` (the 02-04 caller's plan without role
//! URLs: `budget`, `ledger_dir`, `first_observation`, `roles.<role>.share`).

use crate::api::{
    Abort, Ctx, R, Req, Resp, TxGuard, compare_checkpoint, history, idem_key, lock, need_compare,
    ok, one, only_keys, parse_body, problem, role_status, run_tx, str_field,
};
use crate::auth::Caller;
use ieo_supervisor::caller::{self, Call, Plan, Sent};
use postgres::{Client, Transaction};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

/// Deliveries in progress and runs an accepted delivery started whose record is still being
/// finished by a thread of this process (review 63fefc1 M-1 (a)): the shutdown waits for them,
/// bounded.
static FINISHING: AtomicUsize = AtomicUsize::new(0);

/// The latest time by which every counted delivery or run should be done (its own bound); the
/// shutdown waits at least until then (review 4531772 L-3).
static LATEST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// The caller's finishing margin: `After::finish` waits the run window plus this, and after a
/// stop it sent, this again.
pub const FINISH_MARGIN_SECS: u64 = 60;

/// Slack after the caller's waits for fetching the stream and transcripts and writing the record.
pub const FINISH_SLACK_SECS: u64 = 60;

/// The shutdown wait without a counted run (review 4531772 L-3): the 02-05 run window (600 s =
/// timeout + grace) + 2 × the finishing margin + the slack = 780 s. A counted run with a longer
/// window (02-04: 1,800 s, X-02: 3,600 s) extends it to its own bound ([`shutdown_wait`]).
pub const SHUTDOWN_WAIT_DEFAULT_SECS: u64 = 600 + 2 * FINISH_MARGIN_SECS + FINISH_SLACK_SECS;

/// How long a run with this window (timeout + grace) may take to be finished and recorded.
pub fn finish_bound(window: Duration) -> Duration {
    window + Duration::from_secs(2 * FINISH_MARGIN_SECS + FINISH_SLACK_SECS)
}

/// One delivery in progress or one run being finished; dropped (also on a panic) when it ends.
struct Finishing;

impl Finishing {
    /// Counted from now; `within` is how long it may take.
    fn start(within: Duration) -> Finishing {
        FINISHING.fetch_add(1, Ordering::SeqCst);
        let until = std::time::Instant::now() + within;
        let mut l = LATEST.lock().unwrap_or_else(|e| e.into_inner());
        if l.is_none_or(|t| t < until) {
            *l = Some(until);
        }
        Finishing
    }
}

/// The shutdown wait: `IEO_COORD_SHUTDOWN_WAIT_SECS` when set, else the longer of
/// [`SHUTDOWN_WAIT_DEFAULT_SECS`] and the time left until the latest counted bound.
pub fn shutdown_wait() -> Duration {
    if let Some(s) = std::env::var("IEO_COORD_SHUTDOWN_WAIT_SECS")
        .ok()
        .and_then(|w| w.parse().ok())
    {
        return Duration::from_secs(s);
    }
    let left = LATEST
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map(|t| t.saturating_duration_since(std::time::Instant::now()))
        .unwrap_or_default();
    left.max(Duration::from_secs(SHUTDOWN_WAIT_DEFAULT_SECS))
}

impl Drop for Finishing {
    fn drop(&mut self) {
        FINISHING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Set when the process is stopping: deliverAssignment starts nothing new (503).
static STOPPING: AtomicBool = AtomicBool::new(false);

/// The process is stopping (SIGTERM/SIGINT): new deliveries are refused with 503
/// `COORDINATOR_STOPPING` while the deliveries in progress and the runs already accepted are
/// finished (review 63fefc1 M-1 (a)). A delivery is counted **before** it reads this flag, so
/// one that passed the check is waited for (review 4531772 L-4).
pub fn begin_stop() {
    STOPPING.store(true, Ordering::SeqCst);
}

/// How many accepted runs are still being finished in this process.
pub fn finishing() -> usize {
    FINISHING.load(Ordering::SeqCst)
}

/// Wait until no accepted run is being finished, at most `limit`; the number still being
/// finished when it returns (0: all records written).
pub fn wait_finishing(limit: Duration) -> usize {
    let t0 = std::time::Instant::now();
    loop {
        let n = finishing();
        if n == 0 || t0.elapsed() >= limit {
            return n;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

const BODY_KEYS: [&str; 9] = [
    "assignment_id",
    "generation",
    "attempt_id",
    "session_id",
    "prompt",
    "logical_name",
    "new_session",
    "recovery_id",
    "spare",
];

/// An id the supervisor accepts as a file name and CLI argument (its `safe_id`).
fn safe_id(s: &str) -> bool {
    s.len() <= 64
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn bad(field: &str, message: &str) -> Resp {
    one(
        400,
        "SCHEMA_VIOLATION",
        field,
        message,
        "send the delivery as the contract describes it",
    )
}

/// The checked request body.
fn check_body(req: &Req) -> Result<Value, Resp> {
    let body = parse_body(req.body)?;
    only_keys(&body, &BODY_KEYS)?;
    str_field(&body, "assignment_id", 128)?;
    if !body["generation"].is_u64() {
        return Err(one(
            400,
            "MISSING_FIELD",
            "generation",
            "`generation` must be a non-negative integer",
            "supply `generation`",
        ));
    }
    for k in ["attempt_id", "session_id"] {
        if !str_field(&body, k, 64).is_ok_and(safe_id) {
            return Err(bad(
                k,
                &format!("`{k}` must be 1-64 of [A-Za-z0-9_-] starting with a letter or digit"),
            ));
        }
    }
    str_field(&body, "prompt", 64 * 1024)?;
    if !body["logical_name"].is_null() {
        str_field(&body, "logical_name", 128)?;
    }
    if !(body["new_session"].is_null() || body["new_session"].is_boolean()) {
        return Err(bad("new_session", "`new_session` must be a boolean"));
    }
    // 02-05 step 7b: the approval this delivery uses (a new session over an unfinished
    // checkpoint); the supervisor carries it as a file-safe id
    if !body["recovery_id"].is_null()
        && !str_field(&body, "recovery_id", 64).is_ok_and(|r| r.starts_with("R-") && safe_id(r))
    {
        return Err(bad(
            "recovery_id",
            "`recovery_id` is an approved recovery's id (R-…)",
        ));
    }
    // the 02-05 budget's spare run (user decision 2026-11-17 (나)): a reason and the failed
    // attempt it replaces; the shared budget check decides
    if !body["spare"].is_null() {
        let s = &body["spare"];
        let text = |k: &str| {
            s[k].as_str()
                .is_some_and(|r| !r.trim().is_empty() && r.len() <= 2000)
        };
        let ok = s.as_object().is_some_and(|o| o.len() == 3)
            && text("reason")
            && text("evidence_ref")
            && s["replaces"].as_str().is_some_and(safe_id);
        if !ok {
            return Err(bad(
                "spare",
                "`spare` is {reason, evidence_ref (each 1-2000 bytes), replaces (the failed \
                 attempt id)}",
            ));
        }
    }
    Ok(body)
}

/// The delivery's spare marker for the shared budget check, with the person who attached it
/// (the record keeps it; review b38e8df M-1).
fn spare_extra(spare: &Value, who: &Caller) -> Value {
    if spare.is_null() {
        return json!({});
    }
    let mut s = spare.clone();
    // the caller's own kind (review 63fefc1 I-1): the budget gate's `kind == "user"` check is a
    // second, independent check behind the 403 of deliverAssignment
    let kind = match who.kind {
        crate::auth::Kind::User => "user",
        crate::auth::Kind::Service => "service",
        crate::auth::Kind::Agent => "agent",
    };
    s["by"] = json!({"name": who.name, "kind": kind, "via": "deliverAssignment"});
    s["approved_at"] = json!(ieo_evidence::time::now());
    json!({"spare": s})
}

/// The coordinator's configuration of this role: the supervisor and the image and profile it
/// is expected to run.
fn target<'a>(ctx: &'a Ctx, repo: &str, role: &str) -> Result<&'a Value, Resp> {
    let t = &ctx.roles[repo][role];
    let ok = t["url"].is_string()
        && t["adapter_token_file"].is_string()
        && t["image_digest"].is_string()
        && t["profile_id"].is_string()
        && t["profile_revision"].is_u64();
    if ok {
        Ok(t)
    } else {
        Err(problem(
            404,
            "NOT_FOUND",
            "no role supervisor with image and profile configured for this repo and role",
        ))
    }
}

/// The budget plan of `repo` for the shared budget check: `budget_plan` with the role URLs and
/// adapter credentials of the coordinator's role configuration (one source for both).
fn budget_plan(ctx: &Ctx, repo: &str) -> Result<Plan, String> {
    let mut v = ctx.roles[repo]["budget_plan"].clone();
    if !v.is_object() {
        return Err("no budget plan is configured for this repo".into());
    }
    let names: Vec<String> = v["roles"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    // every role the coordinator controls in this repo is in the plan, and only those: the
    // budget check asks each of them to be idle with its runs known (review c11e734 L-2)
    let configured: Vec<String> = crate::roles::ROLES
        .iter()
        .filter(|r| ctx.roles[repo][**r].is_object())
        .map(|r| r.to_string())
        .collect();
    if names != configured {
        return Err(format!(
            "the budget plan's roles {names:?} are not the configured roles {configured:?}"
        ));
    }
    for r in names {
        let t = &ctx.roles[repo][&r];
        v["roles"][&r]["url"] = t["url"].clone();
        v["roles"][&r]["adapter_token_file"] = t["adapter_token_file"].clone();
    }
    Plan::from_json(&v).map_err(|e| format!("budget plan: {e}"))
}

fn delivery_id(repo: &str, role: &str, request_id: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{repo}\0{role}\0{request_id}").as_bytes(),
    ));
    format!("D-{}", &h[..32])
}

/// The delivery with its newest result.
fn load(tx: &mut Transaction, id: &str) -> Result<Value, Resp> {
    let d = tx.query_one(
        "SELECT delivery_id, request_id, repo_id, role, assignment_id, generation, attempt_id,
                session_id, logical_name, coordinator_record, target_sha, cause, requested_by,
                iso(requested_at), evidence_level
         FROM role_deliveries WHERE delivery_id = $1",
        &[&id],
    )?;
    let r = tx.query_opt(
        "SELECT n, decision, reasons, op, outcome, limits, sent_sha256, supervisor_status,
                supervisor_body, requeried, iso(recorded_at), github
         FROM role_delivery_results WHERE delivery_id = $1 ORDER BY n DESC LIMIT 1",
        &[&id],
    )?;
    let result = r.map(|r| {
        json!({"n": r.get::<_, i32>(0), "decision": r.get::<_, Option<String>>(1),
               "reasons": r.get::<_, Value>(2), "op": r.get::<_, Option<String>>(3),
               "outcome": r.get::<_, String>(4), "limits": r.get::<_, Option<Value>>(5),
               "sent_sha256": r.get::<_, Option<String>>(6),
               "supervisor": r.get::<_, Option<i32>>(7).map(|st| json!({"http_status": st,
                   "body": r.get::<_, Option<Value>>(8)})),
               "requeried": r.get::<_, bool>(9), "recorded_at": r.get::<_, String>(10),
               "github": r.get::<_, Option<Value>>(11)})
    });
    Ok(
        json!({"delivery_id": d.get::<_, String>(0), "request_id": d.get::<_, String>(1),
        "repo_id": d.get::<_, String>(2), "role": d.get::<_, String>(3),
        "assignment_id": d.get::<_, String>(4), "generation": d.get::<_, i64>(5),
        "attempt_id": d.get::<_, String>(6), "session_id": d.get::<_, String>(7),
        "logical_name": d.get::<_, String>(8), "coordinator_record": d.get::<_, Value>(9),
        "target_sha": d.get::<_, String>(10), "cause": d.get::<_, String>(11),
        "requested_by": d.get::<_, String>(12), "requested_at": d.get::<_, String>(13),
        "evidence_level": d.get::<_, String>(14), "result": result,
        // the approval this delivery used (02-05 step 7b), or null
        "recovery": crate::recovery::of_delivery(tx, id)?}),
    )
}

/// Transaction 1: the replay of a stored request (200, `{"replay": id}`), or the new request
/// checked against the current assignment and recorded (201, the stored delivery). The
/// coordinator record and the target SHA come from the database and the role configuration.
/// What a new request is checked with: the role's configuration, and what the budget ledger
/// says before the delivery (02-05 step 5) — its identity and its records when no call is
/// running (`None`: busy, or no budget plan).
struct LedgerView<'a> {
    target: &'a Value,
    id: Option<&'a str>,
    idle: Option<&'a [Value]>,
}

fn record_request(
    tx: &mut Transaction,
    who: &Caller,
    repo: &str,
    role: &str,
    key: &str,
    body: &Value,
    ledger: &LedgerView,
) -> R {
    let t = ledger.target;
    lock(tx, &format!("delivery:{repo}:{role}"))?;
    if let Some(row) = tx.query_opt(
        "SELECT delivery_id, body FROM role_deliveries
         WHERE repo_id = $1 AND role = $2 AND request_id = $3",
        &[&repo, &role, &key],
    )? {
        if row.get::<_, Value>(1) != *body {
            return Err(one(
                409,
                "IDEMPOTENCY_KEY_CONFLICT",
                "Idempotency-Key",
                "this Idempotency-Key was used for another delivery",
                "use a new Idempotency-Key for a new delivery",
            ));
        }
        return ok(200, json!({"replay": row.get::<_, String>(0)}));
    }
    let head = tx.query_opt(
        "SELECT a.assignment_id, a.generation, a.task_id, a.target_sha, a.sprint_id,
                a.sprint_revision, s.digest, s.spec, a.request->>'branch_ref'
         FROM assignment_heads h
         JOIN assignments a ON (a.assignment_id, a.generation) = (h.assignment_id, h.generation)
         JOIN sprint_revisions s ON (s.sprint_id, s.revision) = (a.sprint_id, a.sprint_revision)
         WHERE h.repo_id = $1 AND h.role = $2",
        &[&repo, &role],
    )?;
    let Some(h) = head else {
        return Err(problem(
            404,
            "NO_CURRENT_ASSIGNMENT",
            "no current assignment for this repo and role",
        ));
    };
    let (aid, generation) = (h.get::<_, String>(0), h.get::<_, i64>(1));
    if body["assignment_id"] != aid.as_str() || body["generation"] != generation {
        return Err(one(
            409,
            "STALE_ASSIGNMENT",
            "assignment_id",
            &format!(
                "{}/{} is not the current assignment {aid}/{generation}",
                body["assignment_id"], body["generation"]
            ),
            "re-read the current assignment and deliver it",
        ));
    }
    let attempt = body["attempt_id"].as_str().unwrap_or_default();
    if tx
        .query_opt(
            "SELECT 1 FROM role_deliveries WHERE repo_id = $1 AND role = $2 AND attempt_id = $3",
            &[&repo, &role, &attempt],
        )?
        .is_some()
    {
        return Err(one(
            409,
            "ATTEMPT_ID_CONFLICT",
            "attempt_id",
            "another delivery already used this attempt_id",
            "use a new attempt_id",
        ));
    }
    // an attempt id the budget ledger already holds (another caller of the budget, e.g. the
    // 02-04 host caller) is never a delivery's: its record would be taken for this delivery's
    // (review 97ff0bc I-2)
    if ledger
        .idle
        .is_some_and(|recs| recs.iter().any(|r| r["attempt_id"] == attempt))
    {
        return Err(one(
            409,
            "ATTEMPT_ID_CONFLICT",
            "attempt_id",
            "the budget ledger already has a call with this attempt_id",
            "use a new attempt_id",
        ));
    }
    // 02-05 step 5: the assignment's limits (its finished runs were settled just before); a
    // refusal rolls this transaction back (nothing recorded)
    let st = crate::limits::state(tx, &aid, generation, ledger.id)?;
    crate::limits::room(&st)?;
    let record = json!({"generation": generation, "assignment_id": aid,
        "sprint_id": h.get::<_, String>(4), "sprint_revision": h.get::<_, i64>(5),
        "spec_digest": h.get::<_, String>(6), "image_digest": t["image_digest"],
        "profile_id": t["profile_id"], "profile_revision": t["profile_revision"],
        "task_id": h.get::<_, String>(2),
        // the branch the work is pushed to (02-05 step 7); null when the assignment names none
        "branch_ref": h.get::<_, Option<String>>(8),
        // the assignment's own limits (review 97ff0bc I-1)
        "limits": {"elapsed_seconds": st["limits"]["elapsed_seconds"],
                   "total_tokens": st["limits"]["total_tokens"]}});
    let id = delivery_id(repo, role, key);
    let logical = body["logical_name"]
        .as_str()
        .map_or_else(|| format!("{repo}-{role}"), str::to_string);
    tx.execute(
        "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body, assignment_id,
             generation, attempt_id, session_id, logical_name, coordinator_record, target_sha,
             requested_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        &[
            &id,
            &repo,
            &role,
            &key,
            body,
            &aid,
            &generation,
            &attempt,
            &body["session_id"].as_str().unwrap_or_default(),
            &logical,
            &record,
            &h.get::<_, String>(3),
            &who.name,
        ],
    )?;
    history(
        tx,
        &who.name,
        "delivery.requested",
        &json!({"delivery_id": id, "repo_id": repo, "role": role}),
    )?;
    ok(201, load(tx, &id)?)
}

/// One result row of a delivery (the next `n`); the stored delivery with it.
struct Outcome<'a> {
    decision: Option<&'a str>,
    reasons: Value,
    op: Option<&'a str>,
    outcome: &'a str,
    limits: Option<Value>,
    sent_sha256: Option<String>,
    supervisor: Option<(u16, Value)>,
    requeried: bool,
    /// The judgment's GitHub comparison (02-05 step 7), or none.
    github: Option<Value>,
}

fn record_result(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    id: &str,
    o: &Outcome,
    status: u16,
    expect_n: Option<i32>,
) -> R {
    lock(tx, &format!("delivery:{repo}:{role}"))?;
    // a re-query adds its row only when no other result was recorded since it read the
    // delivery: a newer answer is never buried (review c11e734 L-3)
    if let Some(n) = expect_n {
        let now: i32 = tx
            .query_one(
                "SELECT coalesce(max(n), 0) FROM role_delivery_results WHERE delivery_id = $1",
                &[&id],
            )?
            .get(0);
        if now != n {
            return ok(status, load(tx, id)?);
        }
    }
    let st = o.supervisor.as_ref().map(|(s, _)| i32::from(*s));
    let sb = o.supervisor.as_ref().map(|(_, b)| b.clone());
    tx.execute(
        "INSERT INTO role_delivery_results (delivery_id, n, decision, reasons, op, outcome, limits,
             sent_sha256, supervisor_status, supervisor_body, requeried, github)
         VALUES ($1, coalesce((SELECT max(n) FROM role_delivery_results WHERE delivery_id = $1), 0)
                 + 1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        &[
            &id,
            &o.decision,
            &o.reasons,
            &o.op,
            &o.outcome,
            &o.limits,
            &o.sent_sha256,
            &st,
            &sb,
            &o.requeried,
            &o.github,
        ],
    )?;
    ok(status, load(tx, id)?)
}

/// `deliverAssignment`. Order: 404 role → 403 → 400 Idempotency-Key → 400 body → 404 role
/// configuration → transaction 1 (replay or 409 conflict, 404 `NO_CURRENT_ASSIGNMENT`, 409
/// `STALE_ASSIGNMENT`, 409 `ATTEMPT_ID_CONFLICT`, the request recorded) → judgment → budget →
/// supervisor → transaction 2.
pub fn deliver(
    db: &mut Client,
    ctx: &Ctx,
    who: &Caller,
    repo: &str,
    role: &str,
    req: &Req,
    guard: &dyn TxGuard,
) -> Result<Resp, Abort> {
    // counted before the stop check (review 4531772 L-4): a shutdown that starts after the check
    // waits for this delivery, and a run it starts is counted by its own guard before this one
    // is dropped. Its bound: the in-flight window of a delivery
    let _delivery = Finishing::start(Duration::from_secs(bounds().0));
    let pre = (|| {
        need_compare(who, repo, role)?;
        // stopping: the runs already accepted are being finished; nothing new starts and nothing
        // is recorded (review 63fefc1 M-1 (a))
        if STOPPING.load(Ordering::SeqCst) {
            return Err(problem(
                503,
                "COORDINATOR_STOPPING",
                "the coordinator is stopping; deliver again after it restarts",
            ));
        }
        let key = idem_key(req)?;
        let body = check_body(req)?;
        // the spare marker answers a failed run that asks for a person: a person attaches it
        // (review b38e8df M-1, as a recovery approval); nothing is recorded or called otherwise
        if !body["spare"].is_null() && who.kind != crate::auth::Kind::User {
            return Err(problem(
                403,
                "HUMAN_APPROVAL_REQUIRED",
                "a spare run is attached by a person (a user credential), not a service",
            ));
        }
        let t = target(ctx, repo, role)?;
        Ok::<_, Resp>((key, body, t))
    })();
    let (key, body, t) = match pre {
        Ok(x) => x,
        Err(r) => return Ok(r),
    };
    // the in-flight window of this delivery starts here (review 97ff0bc L-2)
    let begun = std::time::Instant::now();
    // the budget ledger's identity and its records when no call is running (step 5)
    let ledger_id = crate::limits::ledger_id(&ctx.roles[repo]["budget_plan"]);
    let idle = budget_plan(ctx, repo)
        .ok()
        .and_then(|p| caller::records_when_idle(&p).ok().flatten());
    // the finished runs of the named assignment are settled first, in their own transaction:
    // measured facts, kept whatever the delivery's answer (the refusal itself records nothing
    // but its limit event). A replay of a stored request settles nothing.
    if let (Some(recs), Some(aid), Some(lid)) = (
        idle.as_deref(),
        body["assignment_id"].as_str(),
        ledger_id.as_deref(),
    ) {
        let settled = run_tx(db, guard, &|tx| {
            let replay = tx
                .query_opt(
                    "SELECT 1 FROM role_deliveries WHERE repo_id = $1 AND role = $2
                       AND request_id = $3",
                    &[&repo, &role, &key],
                )?
                .is_some();
            if !replay {
                crate::limits::settle(tx, repo, role, aid, recs, lid, bounds().0)?;
            }
            ok(200, json!({}))
        })?;
        if settled.status != 200 {
            return Ok(settled);
        }
    }
    let view = LedgerView {
        target: t,
        id: ledger_id.as_deref(),
        idle: idle.as_deref(),
    };
    let first = run_tx(db, guard, &|tx| {
        record_request(tx, who, repo, role, key, &body, &view)
    })?;
    let limit_refusal = first.status == 409
        && matches!(
            first.body["rejections"][0]["code"].as_str(),
            Some("ASSIGNMENT_LIMIT_REACHED" | "ASSIGNMENT_USAGE_UNMEASURED")
        );
    if limit_refusal {
        // the refusal stays visible as one limit event; nothing else is recorded
        let generation = body["generation"].as_u64().map_or(0, crate::api::i64_of);
        let aid = body["assignment_id"].as_str().unwrap_or_default();
        let ev = run_tx(db, guard, &|tx| {
            crate::limits::record_event(tx, aid, generation, &first, &who.name)?;
            ok(200, json!({}))
        })?;
        if ev.status != 200 {
            return Ok(ev);
        }
        return Ok(first);
    }
    match first.status {
        201 => {}
        200 => {
            let id = first.body["replay"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            return replay(db, guard, repo, role, &id, t);
        }
        _ => return Ok(first),
    }
    let d = first.body;
    let id = d["delivery_id"].as_str().unwrap_or_default().to_string();
    // the resume judgment: the supervisor's status outside any transaction, then the central
    // record and the head under the checkpoint lock
    // 02-05 step 7b: a delivery naming an approved recovery starts a new session over the
    // unfinished checkpoint, and nothing else; the approval is checked again at each judgment
    let recovery = body["recovery_id"].as_str().map(str::to_string);
    let judge = |db: &mut Client| -> Result<Resp, Abort> {
        let local = role_status(ctx, repo, role);
        let j = run_tx(db, guard, &|tx| compare_checkpoint(tx, repo, role, &local))?;
        let Some(rid) = recovery.as_deref().filter(|_| j.status == 200) else {
            return Ok(j);
        };
        let u = run_tx(db, guard, &|tx| {
            let r = crate::recovery::usable(tx, repo, role, rid, &local, &id)?;
            ok(
                200,
                match r {
                    Ok(g) => json!({"ok": true, "github": g}),
                    Err(why) => json!({"ok": false, "why": why}),
                },
            )
        })?;
        if u.status != 200 {
            return Ok(u);
        }
        let mut b = j.body;
        if u.body["ok"] == true {
            b["decision"] = json!("recovery_allowed");
            b["reasons"] = json!([format!(
                "recovery {rid}: a new session over the unfinished checkpoint, approved by a person"
            )]);
            b["github"] = u.body["github"].clone();
        } else {
            let mut why = vec![u.body["why"].clone()];
            why.extend(b["reasons"].as_array().cloned().unwrap_or_default());
            b["reasons"] = json!(why);
            b["recovery_refused"] = json!(true);
        }
        Ok(Resp {
            status: 200,
            body: b,
        })
    };
    let judged = judge(db)?;
    if judged.status != 200 {
        return Ok(judged);
    }
    let decision = judged.body["decision"].as_str().unwrap_or_default();
    let reasons = judged.body["reasons"].clone();
    // the GitHub comparison of the judgment (02-05 step 7): recorded with the result
    let github = Some(judged.body["github"].clone()).filter(|g| !g.is_null());
    let new_session = body["new_session"] == true;
    let op = choose_op(decision, new_session, &judged.body);
    let base = |outcome| Outcome {
        decision: Some(decision),
        reasons: reasons.clone(),
        op,
        outcome,
        limits: None,
        sent_sha256: None,
        supervisor: None,
        requeried: false,
        github: github.clone(),
    };
    let Some(op) = op else {
        let o = base("not_called");
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    };
    let budget_refused = |why: String| {
        let mut o = base("budget_refused");
        o.reasons = json!([why]);
        o
    };
    // the shared budget check (the 02-04 caller's code), outside the role containers
    let rec = &d["coordinator_record"];
    let call = Call {
        task_id: rec["task_id"].as_str().unwrap_or_default().into(),
        role: role.into(),
        op: op.into(),
        attempt_id: d["attempt_id"].as_str().unwrap_or_default().into(),
        session_id: d["session_id"].as_str().unwrap_or_default().into(),
        prompt: body["prompt"].as_str().unwrap_or_default().into(),
        coordinator: json!({"generation": rec["generation"], "assignment_id": rec["assignment_id"],
            "sprint_id": rec["sprint_id"], "sprint_revision": rec["sprint_revision"],
            "spec_digest": rec["spec_digest"], "image_digest": rec["image_digest"],
            "profile_id": rec["profile_id"], "profile_revision": rec["profile_revision"]}),
    };
    let admitted = budget_plan(ctx, repo).and_then(|plan| {
        let adm = caller::admit_with(
            &plan,
            role,
            &call.attempt_id,
            &caller::Http,
            &spare_extra(&body["spare"], who),
        )??;
        Ok((plan, adm))
    });
    let (plan, mut adm) = match admitted {
        Ok(x) => x,
        Err(why) => {
            let o = budget_refused(why);
            return run_tx(db, guard, &|tx| {
                record_result(tx, repo, role, &id, &o, 201, None)
            });
        }
    };
    // the judgment again, now that the budget's lock is held: a run that ended (or anything
    // else that changed) while the check waited must not be acted on by the earlier judgment
    // (review c11e734 L-3)
    let again = judge(db)?;
    if again.status != 200 {
        drop(adm);
        return Ok(again);
    }
    let decision2 = again.body["decision"].as_str().unwrap_or_default();
    if decision2 != decision || choose_op(decision2, new_session, &again.body) != Some(op) {
        drop(adm);
        let mut why = vec![json!(format!(
            "the judgment changed while the budget check waited ({decision} → {decision2})"
        ))];
        why.extend(
            again.body["reasons"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
        let o = Outcome {
            decision: Some(decision2),
            reasons: json!(why),
            op: None,
            outcome: "not_called",
            limits: None,
            sent_sha256: None,
            supervisor: None,
            requeried: false,
            github: Some(again.body["github"].clone()).filter(|g| !g.is_null()),
        };
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    }
    // the assignment's remaining amounts bound the run (02-05 step 5): settled again from the
    // records the budget check read under its lock, all generations
    let aid = d["assignment_id"].as_str().unwrap_or_default().to_string();
    let generation = d["generation"].as_i64().unwrap_or_default();
    let lid = ledger_id.clone().unwrap_or_default();
    let st = run_tx(db, guard, &|tx| {
        crate::limits::settle(tx, repo, role, &aid, adm.records(), &lid, bounds().0)?;
        ok(200, crate::limits::state(tx, &aid, generation, Some(&lid))?)
    })?;
    if st.status != 200 {
        drop(adm);
        return Ok(st);
    }
    let room = crate::limits::room(&st.body).map_err(|r| {
        r.body["rejections"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    });
    let lowered = room.and_then(|(secs, tokens)| {
        adm.lower(secs, tokens)
            .map_err(|why| format!("the assignment's remaining limits: {why}"))
    });
    if let Err(why) = lowered {
        drop(adm);
        let o = budget_refused(format!("assignment limits: {why}"));
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    }
    // the start is sent only while the call (bounded by the adapter's 30 s) still ends inside
    // the in-flight window a replay waits for (review 97ff0bc L-2): later, nothing is called
    let (in_flight, deadline) = bounds();
    if begun.elapsed() > Duration::from_secs(deadline) {
        drop(adm);
        let mut o = base("not_called");
        o.reasons = json!([format!(
            "the delivery took {} s before its call: past the {} s start deadline (the call must \
             end inside the {in_flight} s a replay waits)",
            begun.elapsed().as_secs(),
            deadline
        )]);
        o.op = None;
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    }
    // the GitHub comparison of the judgment made under the budget's lock (review f0e0ca9 I-6)
    let github = Some(again.body["github"].clone()).filter(|g| !g.is_null());
    // the delivery id travels with the start (02-05 step 4): the supervisor carries it on the
    // run's checkpoints and the takeover is linked to this delivery
    let mut extra = json!({"target_sha": d["target_sha"], "delivery_id": id});
    // 02-05 step 7: the assignment's branch (the supervisor records the pushed SHA of it), and on
    // a resume the GitHub head and the CI result at it, as the judgment read them
    if let Some(b) = rec["branch_ref"].as_str() {
        extra["branch_ref"] = json!(b);
    }
    if let Some(g) = &github {
        extra["github"] = json!({"branch_ref": g["branch_ref"], "head_sha": g["head"]["sha"],
                                 "ci": g["ci"]["result"]});
    }
    let mut sent = adm.body(&call);
    for (k, v) in extra.as_object().into_iter().flatten() {
        sent[k] = v.clone();
    }
    // the approval's one use, committed before the start is sent (02-05 step 7b): a second
    // delivery, a replay or a restart can never start a second run with it
    if let Some(rid) = &recovery {
        let g = github.clone().unwrap_or(Value::Null);
        let used = run_tx(db, guard, &|tx| {
            crate::recovery::record_use(tx, repo, role, rid, &id, &g)
        })?;
        if used.status != 200 {
            drop(adm);
            let mut o = base("not_called");
            o.op = None;
            o.reasons = json!([format!(
                "recovery {rid}: {}",
                used.body["message"].as_str().unwrap_or("not usable")
            )]);
            return run_tx(db, guard, &|tx| {
                record_result(tx, repo, role, &id, &o, 201, None)
            });
        }
        extra["recovery_id"] = json!(rid);
        sent["recovery_id"] = json!(rid);
    }
    let sent_sha256 = hex::encode(Sha256::digest(
        serde_json::to_vec(&sent).unwrap_or_default(),
    ));
    let mut o = base("unknown");
    o.github = github.clone();
    o.limits = Some(sent["limits"].clone());
    o.sent_sha256 = Some(sent_sha256);
    match adm.start_with(&plan, &call, &caller::Http, &extra) {
        Ok(Sent::Accepted { answer, running }) => {
            o.outcome = "accepted";
            o.supervisor = Some((202, answer));
            // the run is finished, measured and charged by the same library; the ledger lock
            // stays held until its record is written. The shutdown waits for it (bounded)
            let window = Duration::from_millis(
                sent["limits"]["timeout_ms"].as_u64().unwrap_or_default()
                    + sent["limits"]["grace_ms"].as_u64().unwrap_or_default(),
            );
            let g = Finishing::start(finish_bound(window));
            std::thread::spawn(move || {
                let _g = g;
                let a = running.attempt_id().to_string();
                match running.finish(
                    &caller::Http,
                    Duration::from_millis(250),
                    Duration::from_secs(FINISH_MARGIN_SECS),
                ) {
                    Ok(r) => eprintln!(
                        "ieo-coord: delivery attempt {a} recorded (stop_required {})",
                        r["stop_required"]
                    ),
                    Err(e) => eprintln!("ieo-coord: delivery attempt {a}: record failed: {e}"),
                }
            });
        }
        Ok(Sent::Refused { status, body, .. }) => {
            o.outcome = "refused";
            o.supervisor = Some((status, body));
        }
        Ok(Sent::Lost(r)) => {
            o.reasons = json!([r["stop_reason"]]);
        }
        Err(e) => {
            o.reasons = json!([format!("the call could not be completed: {e}")]);
        }
    }
    run_tx(db, guard, &|tx| {
        record_result(tx, repo, role, &id, &o, 201, None)
    })
}

/// The operation the judgment allows. `new_session` asks for a start where a resume is allowed,
/// or over an older generation whose last run finished (`after_run`); never where the judgment
/// refuses (review c11e734 M-1: `judge_resume` already reports an unfinished older run as
/// reconciliation; the phase is checked here again so a start never covers one).
fn choose_op(decision: &str, new_session: bool, judged: &Value) -> Option<&'static str> {
    // a delivery naming a recovery does that or nothing (02-05 step 7b)
    if judged["recovery_refused"] == true {
        return None;
    }
    match (decision, new_session) {
        ("recovery_allowed", _) => Some("start"),
        ("no_checkpoint", _) => Some("start"),
        ("resume_allowed", false) => Some("reconnect"),
        ("resume_allowed", true) => Some("start"),
        ("stale_generation", true) if judged["central"]["phase"] == "after_run" => Some("start"),
        _ => None,
    }
}

/// A delivery without a result younger than this is still in flight: a replay answers 409
/// `DELIVERY_IN_PROGRESS` instead of re-querying, and the assignment's settlement leaves it for
/// later (02-05 step 5).
pub(crate) const IN_FLIGHT_DEFAULT: u64 = 120;

/// The start or reconnect is sent only this long after the delivery began (review 97ff0bc L-2,
/// 1548061 I-3): the call is bounded by the adapter's 30 s timeout and the result is then
/// recorded in one short transaction (each statement ≤ 10 s on HTTP), with 5 s to spare, so the
/// result exists before a replay re-queries: 75 + 30 + 10 + 5 = 120. Everything before the call
/// (the request, the judgments, the budget check with its status reads, the settlements) has
/// to fit in these 75 s; a delivery that does not is recorded as `not_called`.
pub(crate) const START_DEADLINE_DEFAULT: u64 = 75;

/// The call and the result's transaction after the start deadline: 30 s + 10 s + 5 s.
const AFTER_START: u64 = 45;

/// The two bounds in seconds: (in flight, start deadline). The defaults, unless the coordinator
/// process sets `IEO_COORD_DELIVERY_IN_FLIGHT_SECS` and `IEO_COORD_DELIVERY_START_DEADLINE_SECS`
/// (meant for tests, to reach these paths without waiting minutes) to values that keep
/// deadline + 45 ≤ in flight and are no longer than the defaults (only shorter bounds, which
/// fail closed; review af68c93 C-1); any other setting is ignored with a message. The
/// coordinator calls this at start-up and logs the effective bounds when they are not the
/// defaults.
pub fn bounds() -> (u64, u64) {
    static B: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
    *B.get_or_init(|| {
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u64>().ok());
        let (f, d) = (
            env("IEO_COORD_DELIVERY_IN_FLIGHT_SECS"),
            env("IEO_COORD_DELIVERY_START_DEADLINE_SECS"),
        );
        let b = checked_bounds(f, d);
        if b != (IN_FLIGHT_DEFAULT, START_DEADLINE_DEFAULT) {
            eprintln!(
                "ieo-coord: delivery bounds in effect: in flight {} s, start deadline {} s \
                 (defaults {IN_FLIGHT_DEFAULT} s / {START_DEADLINE_DEFAULT} s)",
                b.0, b.1
            );
        } else if (f, d) != (None, None) {
            eprintln!(
                "ieo-coord: delivery bounds {f:?}/{d:?} ignored (both are needed, at most \
                 {IN_FLIGHT_DEFAULT} s / {START_DEADLINE_DEFAULT} s, deadline + {AFTER_START} \
                 <= in flight): using the defaults"
            );
        }
        b
    })
}

/// The bounds a setting gives: both values, each at most its default, with deadline ≥ 1 and
/// deadline + 45 ≤ in flight; else the defaults.
fn checked_bounds(in_flight: Option<u64>, deadline: Option<u64>) -> (u64, u64) {
    match (in_flight, deadline) {
        (Some(f), Some(d))
            if d >= 1
                && f <= IN_FLIGHT_DEFAULT
                && d <= START_DEADLINE_DEFAULT
                && d.saturating_add(AFTER_START) <= f =>
        {
            (f, d)
        }
        _ => (IN_FLIGHT_DEFAULT, START_DEADLINE_DEFAULT),
    }
}

/// The same request again: the stored delivery, or — when its result is `unknown` or was never
/// written — the supervisor's stored answer for the attempt, recorded as the next result.
fn replay(
    db: &mut Client,
    guard: &dyn TxGuard,
    repo: &str,
    role: &str,
    id: &str,
    t: &Value,
) -> Result<Resp, Abort> {
    let stored = run_tx(db, guard, &|tx| {
        let mut d = load(tx, id)?;
        let young: bool = tx
            .query_one(
                "SELECT now() - requested_at < make_interval(secs => $2) FROM role_deliveries
                 WHERE delivery_id = $1",
                &[&id, &(bounds().0 as f64)],
            )?
            .get(0);
        d["_young"] = json!(young);
        ok(200, d)
    })?;
    if stored.status != 200 {
        return Ok(stored);
    }
    let mut d = stored.body;
    let young = d["_young"] == true;
    if let Some(m) = d.as_object_mut() {
        m.remove("_young");
    }
    let res = &d["result"];
    if res.is_null() && young {
        return Ok(one(
            409,
            "DELIVERY_IN_PROGRESS",
            "Idempotency-Key",
            "this delivery is still in progress (no result recorded yet)",
            "send the same request again later",
        ));
    }
    let expect_n = Some(
        res["n"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
    );
    if !res.is_null() && res["outcome"] != "unknown" {
        return Ok(Resp {
            status: 200,
            body: d,
        });
    }
    let attempt = d["attempt_id"].as_str().unwrap_or_default();
    let sent = res["sent_sha256"].as_str();
    let mut o = Outcome {
        decision: res["decision"].as_str(),
        reasons: json!([]),
        op: res["op"].as_str(),
        outcome: "unknown",
        limits: Some(res["limits"].clone()).filter(|l| !l.is_null()),
        sent_sha256: sent.map(str::to_string),
        supervisor: None,
        requeried: true,
        github: Some(res["github"].clone()).filter(|g| !g.is_null()),
    };
    match crate::roles::call(t, "GET", &format!("/v0/role/attempts/{attempt}")) {
        // the stored answer counts only for exactly what this delivery sent; without a recorded
        // digest (the coordinator stopped before transaction 2) it stays unknown
        Ok((200, v)) if sent.is_some_and(|s| v["request_sha256"] == s) => {
            o.outcome = "accepted";
            o.op = if v["op"] == "reconnect" {
                Some("reconnect")
            } else {
                Some("start")
            };
            o.supervisor = Some((202, v["answer"].clone()));
        }
        Ok((200, _)) if sent.is_none() => {
            o.reasons = json!([
                "the supervisor holds an accepted attempt, but this delivery has no recorded request digest to compare"
            ]);
        }
        Ok((200, _)) => {
            o.reasons = json!(["the supervisor holds another request under this attempt_id"]);
        }
        Ok((404, _)) => {
            o.outcome = "not_accepted";
            o.reasons = json!(["the supervisor holds no accepted attempt with this id"]);
        }
        Ok((st, b)) => {
            o.reasons = json!([format!("the supervisor answered {st} {}", b["code"])]);
        }
        Err(e) => o.reasons = json!([format!("the supervisor was not reached: {e}")]),
    }
    run_tx(db, guard, &|tx| {
        record_result(tx, repo, role, id, &o, 200, expect_n)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_bounds_keep_the_call_inside_the_in_flight_window() {
        // the defaults: 75 + 45 = 120
        assert_eq!(START_DEADLINE_DEFAULT + AFTER_START, IN_FLIGHT_DEFAULT);
        assert_eq!(checked_bounds(None, None), (120, 75));
        assert_eq!(checked_bounds(Some(47), Some(2)), (47, 2));
        for (f, d) in [
            (Some(46), Some(2)),
            (Some(47), Some(0)),
            (Some(47), None),
            (None, Some(2)),
            // never longer than the defaults (review af68c93 C-1)
            (Some(86_400), Some(86_355)),
            (Some(121), Some(75)),
            (Some(120), Some(76)),
        ] {
            assert_eq!(checked_bounds(f, d), (120, 75), "{f:?} {d:?}");
        }
    }

    /// Review b38e8df M-1: the budget check gets the person who attached the spare.
    #[test]
    fn the_spare_names_the_person_who_attached_it() {
        let who = Caller {
            name: "alice-ops".into(),
            kind: crate::auth::Kind::User,
            scopes: Default::default(),
            repos: Default::default(),
            role: None,
        };
        assert_eq!(spare_extra(&Value::Null, &who), json!({}));
        let mut got = spare_extra(
            &json!({"reason": "env", "replaces": "a7", "evidence_ref": "rep#1"}),
            &who,
        );
        let at = got["spare"]["approved_at"].take();
        assert!(at.as_str().is_some_and(|a| a.ends_with('Z')), "{at}");
        assert_eq!(
            got,
            json!({"spare": {"reason": "env", "replaces": "a7", "evidence_ref": "rep#1",
                "by": {"name": "alice-ops", "kind": "user", "via": "deliverAssignment"},
                "approved_at": null}})
        );
        // a service's kind is written as it is: the gate refuses it on its own
        let svc = Caller {
            kind: crate::auth::Kind::Service,
            ..who.clone()
        };
        let got = spare_extra(&json!({"reason": "env"}), &svc);
        assert_eq!(got["spare"]["by"]["kind"], "service");
    }

    /// Review 63fefc1 M-1 (a): the shutdown waits for the runs being finished, bounded.
    #[test]
    fn the_shutdown_waits_for_the_runs_being_finished() {
        // review 4531772 L-3: 600 s window + 2 × 60 s margin + 60 s slack
        assert_eq!(SHUTDOWN_WAIT_DEFAULT_SECS, 780);
        assert_eq!(
            finish_bound(Duration::from_secs(600)),
            Duration::from_secs(SHUTDOWN_WAIT_DEFAULT_SECS)
        );
        // a counted run with a longer bound extends the wait to it
        let long = Finishing::start(Duration::from_secs(5_000));
        assert!(shutdown_wait() >= Duration::from_secs(4_990));
        drop(long);
        let g = Finishing::start(Duration::from_secs(1));
        assert!(finishing() >= 1);
        let t0 = std::time::Instant::now();
        assert!(wait_finishing(Duration::from_millis(200)) >= 1, "bounded");
        assert!(t0.elapsed() >= Duration::from_millis(200));
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(g);
        });
        assert_eq!(wait_finishing(Duration::from_secs(10)), 0);
        h.join().expect("join");
    }
}
