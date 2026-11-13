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
    CheckMode, CheckState, ConversionContext, Registry, canonical_json, check_registration,
    convert_human_input,
};
use postgres::{Client, GenericClient, Transaction};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Static configuration loaded at start-up.
pub struct Ctx {
    pub policy: Policy,
    /// Plan manifest; only its `inspection_profiles` are read (approved acceptance profiles).
    pub manifest: Value,
    /// Approved per-repo bindings for human-input conversion (01-07 §3), keyed by repo_id.
    pub bindings: Value,
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
        // The database message never contains credentials (the URL is not part of it).
        Resp {
            status: 500,
            body: json!({"code": "INTERNAL", "message": format!("database error: {e}")}),
        }
    }
}

type R = Result<Resp, Resp>;

fn ok(status: u16, body: Value) -> R {
    Ok(Resp { status, body })
}

fn problem(status: u16, code: &str, message: &str) -> Resp {
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

fn one(status: u16, code: &str, field: &str, message: &str, correction: &str) -> Resp {
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

fn is_sha(s: &str) -> bool {
    (40..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn parse_body(body: &[u8]) -> Result<Value, Resp> {
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

fn str_field<'a>(v: &'a Value, k: &str, max: usize) -> Result<&'a str, Resp> {
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

fn only_keys(v: &Value, allowed: &[&str]) -> Result<(), Resp> {
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

fn i64_of(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Transaction-scoped advisory lock (released at commit/rollback).
fn lock(tx: &mut Transaction, key: &str) -> Result<(), Resp> {
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        &[&key],
    )?;
    Ok(())
}

fn history(tx: &mut Transaction, actor: &str, action: &str, subject: &Value) -> Result<(), Resp> {
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
// Router
// ---------------------------------------------------------------------------

pub fn handle(db: &mut Client, ctx: &Ctx, who: Option<&Caller>, req: &Req) -> Resp {
    let segs: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    if req.method == "GET" && segs == ["v0", "health"] {
        // Liveness only: the process answers and the DB is reachable. This is not P04.
        return match db.query_one("SELECT v FROM schema_meta WHERE k = 'schema_version'", &[]) {
            Ok(row) => Resp {
                status: 200,
                body: json!({"status": "up", "schema_version": row.get::<_, String>(0),
                             "policy_digest": ctx.policy.digest,
                             "scope": "liveness only; not a P04 result"}),
            },
            Err(e) => e.into(),
        };
    }
    let Some(who) = who else {
        return problem(
            401,
            "UNAUTHENTICATED",
            "missing or unknown bearer credential",
        );
    };
    let route = (req.method, segs.as_slice());
    let run = |db: &mut Client, f: &dyn Fn(&mut Transaction) -> R| -> Resp {
        let mut tx = match db.transaction() {
            Ok(t) => t,
            Err(e) => return e.into(),
        };
        match f(&mut tx) {
            Ok(r) => match tx.commit() {
                Ok(()) => r,
                Err(e) => e.into(),
            },
            Err(r) => {
                let _ = tx.rollback();
                r
            }
        }
    };
    match route {
        ("POST", ["v0", "sprints"]) => run(db, &|tx| register(tx, ctx, who, req)),
        ("POST", ["v0", "sprint-inputs"]) => run(db, &|tx| convert(tx, ctx, who, req)),
        ("GET", ["v0", "sprints", id, "revisions", rev]) => {
            run(db, &|tx| get_revision(tx, who, id, rev))
        }
        ("POST", ["v0", "sprints", id, "revisions", rev, "semantic-review"]) => {
            run(db, &|tx| semantic_review(tx, who, id, rev, req))
        }
        ("GET", ["v0", "sprints", id, "state"]) => run(db, &|tx| sprint_state(tx, who, id)),
        ("GET", ["v0", "sprints", id, "events"]) => run(db, &|tx| list_events(tx, who, id, req)),
        ("POST", ["v0", "agent", "reports"]) => run(db, &|tx| submit_report(tx, who, req)),
        ("POST", ["v0", "agent", "repro-requests"]) => run(db, &|tx| submit_repro(tx, who, req)),
        ("GET", ["v0", "agent", "repro-requests", id]) => run(db, &|tx| get_repro(tx, who, id)),
        ("POST", ["v0", "coordinator", "assignments"]) => run(db, &|tx| assign(tx, ctx, who, req)),
        ("POST", ["v0", "coordinator", "evidence"]) => run(db, &|tx| ingest(tx, who, req)),
        ("POST", ["v0", "coordinator", "judgments"]) => run(db, &|tx| judge(tx, ctx, who, req)),
        ("GET", ["v0", "coordinator", "outbox"]) => run(db, &|tx| list_outbox(tx, who, req)),
        ("POST", ["v0", "coordinator", "outbox", id, "delivery"]) => {
            run(db, &|tx| deliver(tx, who, id, req))
        }
        _ => problem(404, "NOT_FOUND", "no such endpoint"),
    }
}

fn need_client_scope(who: &Caller, scope: &str, sprints_path: bool) -> Result<(), Resp> {
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

fn idem_key<'a>(req: &Req<'a>) -> Result<&'a str, Resp> {
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

/// Shared by `registerSprint` and `convertHumanInput`. Returns (status, RegistrationResult).
fn register_spec(
    tx: &mut Transaction,
    ctx: &Ctx,
    who: &Caller,
    spec: &Value,
    mode: CheckMode,
) -> Result<(u16, Value), Resp> {
    let reg = load_registry(tx, ctx)?;
    let out = check_registration(spec, &reg, mode);
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
        CheckState::Rejected => {
            let list = out
                .rejections
                .iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect();
            return Err(rejected(status, list));
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
    let (status, result) = register_spec(tx, ctx, who, &body, mode)?;
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
    if !b.is_object() {
        return Err(one(
            422,
            "UNBOUND_DRAFT",
            "repo",
            "no approved conversion binding for this repo",
            "the repo's binding (sprint number, delegation, sources, profile, limits) must be approved first",
        ));
    }
    let sprint_id = b["sprint_id"].as_str().unwrap_or_default().to_string();
    let next_rev = latest_revision(tx, &sprint_id)?.unwrap_or(0) + 1;
    let mut registered = BTreeMap::new();
    for row in tx.query(
        "SELECT DISTINCT ON (sprint_id) spec FROM sprint_revisions ORDER BY sprint_id, revision DESC",
        &[],
    )? {
        let spec: Value = row.get(0);
        for t in spec["tasks"].as_array().into_iter().flatten() {
            registered.insert(
                t["task_id"].as_str().unwrap_or_default().to_string(),
                spec["plan_version"].as_str().unwrap_or_default().to_string(),
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
        sprint_id,
        sprint_number: u8::try_from(b["sprint_number"].as_u64().unwrap_or_default()).unwrap_or(0),
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
        input_ref: format!("sprint-input:{body_digest}"),
    };
    let rec = convert_human_input(&body, &cctx).map_err(|rs| {
        let status = rs.iter().map(|r| r.code.http_status()).min().unwrap_or(400);
        rejected(
            status,
            rs.iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
        )
    })?;
    let (status, registration) = register_spec(tx, ctx, who, &rec.spec, CheckMode::Draft)?;
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
    if registrant == who.name {
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
    json!({"request_id": row.get::<_, String>("request_id"), "state": row.get::<_, String>("state"),
           "target_sha": row.get::<_, String>("target_sha"), "runner_image_digest": null,
           "exit_code": null, "raw_log_refs": [], "finished_at": null})
}

fn submit_repro(tx: &mut Transaction, who: &Caller, req: &Req) -> R {
    let (repo, role) = agent_identity(who, "agent:repro")?;
    let body = parse_body(req.body)?;
    lock(tx, &format!("scope:{repo}:{role}"))?;
    let book = load_book(tx, &repo, &role)?;
    let stored = match body["request_id"].as_str() {
        Some(id) => tx.query_opt(
            "SELECT request_id, state, target_sha, body FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
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
                "SELECT request_id, state, target_sha FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
                &[&repo, &id],
            )?;
            ok(202, repro_status(&row))
        }
    }
}

fn get_repro(tx: &mut Transaction, who: &Caller, id: &str) -> R {
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
        "SELECT request_id, state, target_sha FROM repro_requests WHERE repo_id = $1 AND request_id = $2",
        &[&repo, &id],
    )? {
        Some(row) => ok(200, repro_status(&row)),
        None => Err(problem(404, "NOT_FOUND", "unknown request")),
    }
}

// ---------------------------------------------------------------------------
// Assignments: ownership head, permission generation, prerequisites, attempts
// ---------------------------------------------------------------------------

fn find_task<'a>(spec: &'a Value, task_id: &str) -> Option<&'a Value> {
    spec["tasks"]
        .as_array()?
        .iter()
        .find(|t| t["task_id"] == task_id)
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
        ],
    )?;
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
    lock(tx, &format!("scope:{repo}:{role}"))?;
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
    if latest_revision(tx, &sprint_id)?.is_some_and(|l| l > sprint_rev) {
        return Err(one(
            409,
            "REVISION_SUPERSEDED",
            "sprint_revision",
            "a newer revision is active",
            "assign from the latest revision",
        ));
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
    let Some(task) = find_task(&spec, &task_id) else {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "task_id",
            "the task is not in this revision",
            "fix task_id",
        ));
    };
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
        let met: bool = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM judgments WHERE task_id = $1 AND policy_digest = $2
                   AND ($3 = false OR outcome = 'accepted'))",
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
        "SELECT issued, limit_reached FROM task_attempts WHERE sprint_id = $1 AND task_id = $2 AND role = $3",
        &[&sprint_id, &task_id, &role],
    )?;
    let (issued, reached) =
        attempts.map_or((0, false), |r| (r.get::<_, i64>(0), r.get::<_, bool>(1)));
    let Some(ceiling) = spec["limits"]["correction_attempts"].as_i64() else {
        return Err(one(
            422,
            "UNBOUND_DRAFT",
            "limits.correction_attempts",
            "the attempt ceiling is not bound",
            "bind limits before execution",
        ));
    };
    if issued > ceiling {
        if reached {
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
            "UPDATE task_attempts SET limit_reached = true WHERE sprint_id = $1 AND task_id = $2 AND role = $3",
            &[&sprint_id, &task_id, &role],
        )?;
        history(
            tx,
            &who.name,
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
    tx.execute(
        "INSERT INTO assignments (assignment_id, generation, repo_id, role, sprint_id, sprint_revision,
            task_id, target_sha, issued_by, policy_digest, request)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        &[&aid, &i64_of(generation), &repo, &role, &sprint_id, &sprint_rev, &task_id, &sha,
          &who.name, &ctx.policy.digest, &body],
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
    history(
        tx,
        &who.name,
        "assignment.issued",
        &json!({"assignment_id": aid, "generation": generation, "repo_id": repo, "role": role,
                    "sprint_id": sprint_id, "task_id": task_id, "target_sha": sha,
                    "replaced": book.current}),
    )?;
    ok(
        201,
        json!({"outcome": "issued", "assignment_id": aid, "generation": generation,
                   "repo_id": repo, "role": role, "sprint_id": sprint_id,
                   "sprint_revision": sprint_rev, "task_id": task_id, "target_sha": sha,
                   "policy_digest": ctx.policy.digest, "attempts_issued": issued + 1,
                   "ceiling": ceiling}),
    )
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
    r["event_id"] = json!(format!("limit-{sprint_id}-{task_id}-{role}"));
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

fn validate_evidence(v: &Value) -> Result<(String, String), Resp> {
    let kind = str_field(v, "kind", 32)?.to_string();
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
            only_keys(v, &keys(&["sha", "conclusion", "source_ref"]))?;
            conclusion()?;
            str_field(v, "source_ref", 512)?;
            sha_of("sha")?
        }
        "run_record" => {
            only_keys(v, &keys(&["sha", "source_ref", "record"]))?;
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
                &keys(&["sha", "reviewer", "author", "state", "source_ref"]),
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
        "merge" => {
            only_keys(v, &keys(&["head_sha", "merged_sha", "source_ref"]))?;
            str_field(v, "source_ref", 512)?;
            sha_of("merged_sha")?;
            sha_of("head_sha")?
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
fn ci_ok(
    tx: &mut Transaction,
    repo: &str,
    kinds: &[&str],
    sha: &str,
    used: &mut Vec<String>,
) -> Result<bool, Resp> {
    let kinds: Vec<String> = kinds.iter().map(|s| s.to_string()).collect();
    let rows = tx.query(
        "SELECT DISTINCT ON (body->>'source_ref') evidence_id, kind, body FROM evidence
         WHERE repo_id = $1 AND kind = ANY($2) AND sha = $3
         ORDER BY body->>'source_ref', seq DESC",
        &[&repo, &kinds, &sha],
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
    let spec: Value = rev_row.get(1);
    let Some(task) = find_task(&spec, &task_id).cloned() else {
        return Err(one(
            422,
            "UNKNOWN_TASK",
            "task_id",
            "the task is not in the active revision",
            "fix task_id",
        ));
    };
    // G2: the head SHA is the coordinator's record for the current development assignment
    let dev = load_book(tx, &repo, "development")?;
    let head = match &dev.current {
        Some((a, g)) => tx.query_opt(
            "SELECT target_sha, assignment_id, generation FROM assignments
             WHERE assignment_id = $1 AND generation = $2 AND sprint_id = $3 AND task_id = $4",
            &[a, &i64_of(*g), &sprint_id, &task_id],
        )?,
        None => None,
    };
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
    let mut used = Vec::new();
    let ci = ci_ok(tx, &repo, &["ci_run", "run_record"], &head_sha, &mut used)?;
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
    for r in tx.query(
        "SELECT DISTINCT ON (body->>'reviewer') evidence_id, sha, body->>'state' FROM evidence
         WHERE repo_id = $1 AND kind = 'review' AND body->>'reviewer' <> body->>'author'
         ORDER BY body->>'reviewer', seq DESC",
        &[&repo],
    )? {
        let state: String = r.get(2);
        let verdict = match state.as_str() {
            "approved" => "approve",
            "changes_requested" => "reject",
            _ => continue,
        };
        reviews.push((r.get(1), verdict.to_string()));
        used.push(format!("evidence:{}", r.get::<_, String>(0)));
    }
    let blocked = reviews
        .iter()
        .any(|(s, v)| *s == head_sha && (v == "reject" || v == "needs_confirmation"));
    let review = if blocked {
        ReviewCurrency::ReviewNeededOnCurrentSha {
            stale_approvals: vec![],
        }
    } else {
        review_currency(&head_sha, &reviews)
    };
    let merged: Option<String> = tx
        .query_opt(
            "SELECT body->>'merged_sha', evidence_id FROM evidence WHERE repo_id = $1 AND kind = 'merge' AND sha = $2
             ORDER BY seq DESC LIMIT 1",
            &[&repo, &head_sha],
        )?
        .map(|r| {
            used.push(format!("evidence:{}", r.get::<_, String>(1)));
            r.get(0)
        });
    let main_ci = match &merged {
        Some(m) => ci_ok(tx, &repo, &["main_ci"], m, &mut used)?,
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
    let summary = completion_summary(&facts);
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
        "INSERT INTO judgments (repo_id, judgment_id, request, sprint_id, task_id, outcome, head_sha,
            merged_sha, facts, policy_digest, event_sprint, event_seq, judged_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[&repo, &jid, &body, &sprint_id, &task_id, &outcome, &head_sha, &merged, &facts_json,
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

/// Apply the schema (idempotent `CREATE ... IF NOT EXISTS`).
pub fn migrate(db: &mut impl GenericClient) -> Result<(), postgres::Error> {
    db.batch_execute(include_str!("schema.sql"))
}
