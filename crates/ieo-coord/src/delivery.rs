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
use std::time::Duration;

const BODY_KEYS: [&str; 7] = [
    "assignment_id",
    "generation",
    "attempt_id",
    "session_id",
    "prompt",
    "logical_name",
    "new_session",
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
    Ok(body)
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
                supervisor_body, requeried, iso(recorded_at)
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
               "requeried": r.get::<_, bool>(9), "recorded_at": r.get::<_, String>(10)})
    });
    Ok(
        json!({"delivery_id": d.get::<_, String>(0), "request_id": d.get::<_, String>(1),
        "repo_id": d.get::<_, String>(2), "role": d.get::<_, String>(3),
        "assignment_id": d.get::<_, String>(4), "generation": d.get::<_, i64>(5),
        "attempt_id": d.get::<_, String>(6), "session_id": d.get::<_, String>(7),
        "logical_name": d.get::<_, String>(8), "coordinator_record": d.get::<_, Value>(9),
        "target_sha": d.get::<_, String>(10), "cause": d.get::<_, String>(11),
        "requested_by": d.get::<_, String>(12), "requested_at": d.get::<_, String>(13),
        "evidence_level": d.get::<_, String>(14), "result": result}),
    )
}

/// Transaction 1: the replay of a stored request (200, `{"replay": id}`), or the new request
/// checked against the current assignment and recorded (201, the stored delivery). The
/// coordinator record and the target SHA come from the database and the role configuration.
fn record_request(
    tx: &mut Transaction,
    who: &Caller,
    repo: &str,
    role: &str,
    key: &str,
    body: &Value,
    t: &Value,
) -> R {
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
                a.sprint_revision, s.digest, s.spec
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
    let spec: Value = h.get(7);
    let record = json!({"generation": generation, "assignment_id": aid,
        "sprint_id": h.get::<_, String>(4), "sprint_revision": h.get::<_, i64>(5),
        "spec_digest": h.get::<_, String>(6), "image_digest": t["image_digest"],
        "profile_id": t["profile_id"], "profile_revision": t["profile_revision"],
        "task_id": h.get::<_, String>(2),
        "limits": {"elapsed_seconds": spec["limits"]["elapsed_seconds"],
                   "total_tokens": spec["limits"]["total_tokens"]}});
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
             sent_sha256, supervisor_status, supervisor_body, requeried)
         VALUES ($1, coalesce((SELECT max(n) FROM role_delivery_results WHERE delivery_id = $1), 0)
                 + 1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
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
    let pre = (|| {
        need_compare(who, repo, role)?;
        let key = idem_key(req)?;
        let body = check_body(req)?;
        let t = target(ctx, repo, role)?;
        Ok::<_, Resp>((key, body, t))
    })();
    let (key, body, t) = match pre {
        Ok(x) => x,
        Err(r) => return Ok(r),
    };
    let first = run_tx(db, guard, &|tx| {
        record_request(tx, who, repo, role, key, &body, t)
    })?;
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
    let judge = |db: &mut Client| {
        let local = role_status(ctx, repo, role);
        run_tx(db, guard, &|tx| compare_checkpoint(tx, repo, role, &local))
    };
    let judged = judge(db)?;
    if judged.status != 200 {
        return Ok(judged);
    }
    let decision = judged.body["decision"].as_str().unwrap_or_default();
    let reasons = judged.body["reasons"].clone();
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
        let adm = caller::admit(&plan, role, &call.attempt_id, &caller::Http)??;
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
        };
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    }
    // the revision's limits bound one run too (the remaining amounts are step 5)
    if let Err(why) = adm.lower(
        rec["limits"]["elapsed_seconds"].as_u64(),
        rec["limits"]["total_tokens"].as_u64(),
    ) {
        drop(adm);
        let o = budget_refused(format!("the sprint revision's limits: {why}"));
        return run_tx(db, guard, &|tx| {
            record_result(tx, repo, role, &id, &o, 201, None)
        });
    }
    // the delivery id travels with the start (02-05 step 4): the supervisor carries it on the
    // run's checkpoints and the takeover is linked to this delivery
    let extra = json!({"target_sha": d["target_sha"], "delivery_id": id});
    let mut sent = adm.body(&call);
    sent["target_sha"] = d["target_sha"].clone();
    sent["delivery_id"] = json!(id);
    let sent_sha256 = hex::encode(Sha256::digest(
        serde_json::to_vec(&sent).unwrap_or_default(),
    ));
    let mut o = base("unknown");
    o.limits = Some(sent["limits"].clone());
    o.sent_sha256 = Some(sent_sha256);
    match adm.start_with(&plan, &call, &caller::Http, &extra) {
        Ok(Sent::Accepted { answer, running }) => {
            o.outcome = "accepted";
            o.supervisor = Some((202, answer));
            // the run is finished, measured and charged by the same library; the ledger lock
            // stays held until its record is written
            std::thread::spawn(move || {
                let a = running.attempt_id().to_string();
                match running.finish(
                    &caller::Http,
                    Duration::from_millis(250),
                    Duration::from_secs(60),
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
    match (decision, new_session) {
        ("no_checkpoint", _) => Some("start"),
        ("resume_allowed", false) => Some("reconnect"),
        ("resume_allowed", true) => Some("start"),
        ("stale_generation", true) if judged["central"]["phase"] == "after_run" => Some("start"),
        _ => None,
    }
}

/// A delivery without a result younger than this is still in flight: a replay answers 409
/// `DELIVERY_IN_PROGRESS` instead of re-querying (the call itself is bounded well below it).
const IN_FLIGHT: &str = "120 seconds";

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
                &format!(
                    "SELECT now() - requested_at < interval '{IN_FLIGHT}' FROM role_deliveries
                     WHERE delivery_id = $1"
                ),
                &[&id],
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
