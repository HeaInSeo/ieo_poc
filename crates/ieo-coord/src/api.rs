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
        ("POST", ["v0", "imports", "notion"]) => run(db, &|tx| import_notion(tx, ctx, who, req)),
        ("POST", ["v0", "coordinator", "exceptions"]) => {
            run(db, &|tx| record_exception(tx, who, req))
        }
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
    // review 1e98f17 F1: a sprint_id belongs to one repo for life (RG2); a revision that
    // names another repo would hand the sprint to that repo
    if let Some(sid) = spec["sprint_id"].as_str()
        && let Some(r) = tx.query_opt(
            "SELECT repo_id FROM sprint_revisions WHERE sprint_id = $1 LIMIT 1",
            &[&sid],
        )?
        && Some(r.get::<_, &str>(0)) != spec["target"]["repo_id"].as_str()
    {
        return Err(one(
            409,
            "SPRINT_REPO_CONFLICT",
            "target.repo_id",
            "this sprint_id is registered for another repo",
            "use a new sprint_id for this repo",
        ));
    }
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
    guard_import_owned(tx, &body, mode)?;
    // C2 (review 9a421b7 F1): the gate follows the Notion sources a spec cites, whatever its
    // sprint_id, so a copy of a gated task under another sprint id is gated too. Format errors
    // keep their 400 (checked by register_spec).
    if mode == CheckMode::Executable
        && ieo_spec::schema::format_rejections(ieo_spec::schema::sprint_spec_validator(), &body)
            .is_empty()
    {
        let repo = body["target"]["repo_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        notion_gate(
            tx,
            &body,
            body["sprint_id"].as_str().unwrap_or_default(),
            &repo,
        )?;
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
    let url = str_field(src, "url", 1024)?.to_string();
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
    let reserved = ctx
        .bindings
        .as_object()
        .into_iter()
        .flatten()
        .any(|(_, b)| b["sprint_id"].as_str() == Some(test_id.as_str()));
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
/// resolved (review 7a48a4f T1); any other content → 409 (review 38b671b S2). Malformed bodies
/// and other repos fall through to the normal checks (400 / `SPRINT_REPO_CONFLICT`, T2/T5).
fn guard_import_owned(tx: &mut Transaction, spec: &Value, mode: CheckMode) -> Result<(), Resp> {
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
    if spec["target"]["repo_id"].as_str() != Some(owner.as_str())
        || !ieo_spec::schema::format_rejections(ieo_spec::schema::sprint_spec_validator(), spec)
            .is_empty()
    {
        return Ok(());
    }
    let same_source = spec["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|x| x["kind"] == "notion_page" && x["digest"].as_str() == Some(digest.as_str()));
    if same_source && spec["tasks"] == imported["tasks"] {
        let _ = mode;
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
/// after the lock; it only orders pages imported under two test ids before that was refused). For each page the **latest**
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
    // G2: the head SHA is the coordinator's record for the current development assignment
    let dev = load_book(tx, &repo, "development")?;
    let head = match &dev.current {
        Some((a, g)) => tx.query_opt(
            "SELECT target_sha, assignment_id, generation, sprint_revision FROM assignments
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
    // GitHub reviews (review 1e98f17 F2): per independent reviewer, the latest
    // state-changing review *at the head SHA*. A later COMMENTED review does not clear a
    // changes_requested; DISMISSED does. Reviews of other SHAs or PRs never count.
    for r in tx.query(
        "SELECT DISTINCT ON (body->>'reviewer') evidence_id, body->>'state' FROM evidence
         WHERE repo_id = $1 AND kind = 'review' AND sha = $2 AND body->>'reviewer' <> body->>'author'
           AND body->>'state' IN ('approved', 'changes_requested', 'dismissed')
         ORDER BY body->>'reviewer', seq DESC",
        &[&repo, &head_sha],
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
    let blocked = reviews
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

/// Apply the schema (idempotent `CREATE ... IF NOT EXISTS`).
pub fn migrate(db: &mut impl GenericClient) -> Result<(), postgres::Error> {
    db.batch_execute(include_str!("schema.sql"))
}

#[cfg(test)]
mod tests {
    use super::legacy_split;

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
